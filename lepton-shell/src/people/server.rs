//! `search_people`: the one server call behind [`super::PersonPicker`].

use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::types::{is_searchable, secondary_line, MAX_PER_KIND};
use super::types::{PickedPrincipal, PrincipalKind};

/// Searches people, teams and service users for a person picker.
///
/// Runs each requested kind's search source with the caller's own Valence, so every
/// source applies its own privacy rules and rate limit. Queries under two characters
/// return nothing, `limit` applies per kind (at most 50), and a host that doesn't link a
/// source gets no rows for it. Results never carry an email address.
///
/// # Errors
///
/// Fails when the request carries no Higgs session (the caller isn't signed in), or when
/// a source refuses the search (for example, too many people searches in a burst).
#[server(SearchPeople)]
pub async fn search_people(
    query: String,
    kinds: Vec<PrincipalKind>,
    limit: u32,
) -> Result<Vec<PickedPrincipal>, ServerFnError> {
    use uf_search_core::{SearchSourceKey, SearchSourceRegistry};

    let ctx = lepton_auth::higgs_ctx().await?;
    let v = signed_in_valence(&ctx)?;
    if !is_searchable(&query) {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, MAX_PER_KIND);
    let registry = SearchSourceRegistry::auto_discover();

    let mut out = Vec::new();
    for kind in dedupe_searchable(kinds) {
        let Some(source_id) = kind.source_id() else {
            continue;
        };
        // One source per call, so a busy kind can't use up another kind's limit.
        let keys = [SearchSourceKey::new(source_id, "")];
        let items = registry
            .query_many(&keys, &v, query.trim(), limit)
            .await
            .map_err(|e| {
                leptos::logging::warn!("search_people: {source_id} failed: {e}");
                ServerFnError::new(refusal_message(&e.to_string()))
            })?;
        out.extend(items.into_iter().map(|item| PickedPrincipal {
            secondary: secondary_line(&kind, item.description.as_deref()),
            kind: kind.clone(),
            id: item.id,
            label: item.title.trim().to_string(),
            image_url: item.image_url,
        }));
    }
    Ok(out)
}

/// The caller's own Valence, once the request carries a signed-in Higgs session.
///
/// Only the session snapshot counts, so a host that identifies callers through Higgs
/// works without an axum-login layer. Anonymous requests and explicit non-session actors
/// are refused.
#[cfg(feature = "ssr")]
fn signed_in_valence(ctx: &higgs::Higgs) -> Result<valence::Valence, ServerFnError> {
    if !ctx.host().is_authenticated() {
        return Err(ServerFnError::new("Sign in to search people."));
    }
    lepton_auth::user_valence(ctx)
}

/// Searchable kinds in picker order, each once.
#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
fn dedupe_searchable(kinds: Vec<PrincipalKind>) -> Vec<PrincipalKind> {
    PrincipalKind::SEARCHABLE
        .into_iter()
        .filter(|k| kinds.contains(k))
        .collect()
}

/// What the person sees when a source refuses: the rate-limit message as written,
/// anything else as a general retry line, so backend detail never reaches the page.
#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
fn refusal_message(error: &str) -> &'static str {
    if error.contains("Too many") {
        "Too many searches. Try again in a moment."
    } else {
        "Search isn't working right now. Try again."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "ssr")]
    mod session {
        #![allow(clippy::expect_used, clippy::unwrap_used)]

        use std::sync::Arc;

        use higgs::{Higgs, HiggsConfig, HiggsValenceFactory};
        use higgs_host::HostRequestCtx;
        use higgs_identity::SessionSnapshot;
        use valence::{Actor, DatabaseRouter, InMemoryBackend, Valence};

        use super::super::signed_in_valence;

        /// **Test:** builds in-memory Valence for whatever actor Higgs hands it.
        struct MemFactory;

        impl HiggsValenceFactory for MemFactory {
            fn build(&self, actor_json: &serde_json::Value) -> anyhow::Result<Valence> {
                let actor: Actor = serde_json::from_value(actor_json.clone())?;
                let mut router = DatabaseRouter::new();
                valence::register_backend_logical_names_slices(
                    &mut router,
                    Arc::new(InMemoryBackend::new()),
                    &[&["default"]],
                    valence::RegisterBackendLogicalNamesOptions::default(),
                );
                Ok(Valence::builder()
                    .database_router(Arc::new(router))
                    .default_backend_key(valence::router_key("default", valence::MEM_ENGINE_ID))
                    .with_actor(actor)
                    .build()?)
            }
        }

        fn higgs(host: HostRequestCtx) -> Higgs {
            let config = HiggsConfig::builder().valence_factory(MemFactory).build();
            Higgs::from_parts(host, Arc::new(config.expect("higgs config")))
        }

        fn router() -> Arc<DatabaseRouter> {
            Arc::new(DatabaseRouter::new())
        }

        #[test]
        fn higgs_session_without_axum_login_gets_own_valence_happy() {
            let ctx = higgs(HostRequestCtx {
                database_router: router(),
                session: Some(SessionSnapshot::new("user:ada0001", b"hash")),
                explicit_actor: None,
            });
            let v = signed_in_valence(&ctx).expect("signed in");
            assert_eq!(v.actor().user_id(), Some("user:ada0001"));
        }

        #[test]
        fn no_higgs_session_is_asked_to_sign_in_sad() {
            let ctx = higgs(HostRequestCtx {
                database_router: router(),
                session: None,
                explicit_actor: None,
            });
            let err = signed_in_valence(&ctx).expect_err("anonymous");
            assert!(err.to_string().contains("Sign in to search people."));
        }

        #[test]
        fn explicit_actor_without_session_is_refused_sad() {
            let host = HostRequestCtx::for_actor(
                router(),
                Actor::User {
                    user_id: "user:bot".into(),
                },
            )
            .expect("non-system actor");
            let err = signed_in_valence(&higgs(host)).expect_err("no session");
            assert!(err.to_string().contains("Sign in to search people."));
        }
    }

    #[test]
    fn dedupe_keeps_picker_order_happy() {
        let kinds = vec![
            PrincipalKind::ServiceUser,
            PrincipalKind::User,
            PrincipalKind::ServiceUser,
        ];
        assert_eq!(
            dedupe_searchable(kinds),
            vec![PrincipalKind::User, PrincipalKind::ServiceUser]
        );
    }

    #[test]
    fn dedupe_drops_extra_kinds_sad() {
        assert_eq!(
            dedupe_searchable(vec![PrincipalKind::Extra("x".into())]),
            vec![]
        );
    }

    #[test]
    fn refusal_message_keeps_rate_limit_happy() {
        assert_eq!(
            refusal_message("Too many people searches. Try again in a moment."),
            "Too many searches. Try again in a moment."
        );
    }

    #[test]
    fn refusal_message_hides_backend_detail_sad() {
        assert_eq!(
            refusal_message("valence: table user_profile unreachable"),
            "Search isn't working right now. Try again."
        );
    }
}
