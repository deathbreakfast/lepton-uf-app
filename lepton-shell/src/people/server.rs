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
/// Fails when the caller isn't signed in, or when a source refuses the search (for
/// example, too many people searches in a burst).
#[server(SearchPeople)]
pub async fn search_people(
    query: String,
    kinds: Vec<PrincipalKind>,
    limit: u32,
) -> Result<Vec<PickedPrincipal>, ServerFnError> {
    use lepton_auth::{require_auth_user, user_valence};
    use uf_search_core::{SearchSourceKey, SearchSourceRegistry};

    let (ctx, _user) = require_auth_user()
        .await
        .map_err(|_| ServerFnError::new("Sign in to search people."))?;
    if !is_searchable(&query) {
        return Ok(Vec::new());
    }
    let v = user_valence(&ctx)?;
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
            image_url: None,
        }));
    }
    Ok(out)
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
