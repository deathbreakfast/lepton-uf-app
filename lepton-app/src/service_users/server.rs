//! Server functions behind the Service users pages.
//!
//! Each one recovers the signed-in person through Lepton and Higgs and passes
//! that request's Valence to Lepton's service user calls and to the host's
//! [`ServiceUserAccessBackend`](super::ServiceUserAccessBackend). Nothing here switches to a
//! system actor. A service user's own token never reaches these functions: bearer tokens
//! only work on the host's `/api/v1` routes, and Lepton refuses any caller that
//! isn't a signed-in user.

use leptos::prelude::*;

use super::types::{
    AccessChoiceView, HandleCheck, MintedTokenView, ServiceUserRow, ServiceUsersOverview, TokenTtl,
};

#[cfg(feature = "ssr")]
mod ssr {
    use std::sync::Arc;

    use lepton_auth::service_users::ServiceUserError;
    use lepton_identity::generated::{ServiceUserAccount, ServiceUserAccountStatus};
    use lepton_identity::service_users::ServiceUserId;
    use leptos::prelude::ServerFnError;
    use valence::Valence;

    use crate::service_users::port::{
        use_service_user_access_backend, ServiceUserAccessBackend, ServiceUserAccessError,
    };
    use crate::service_users::types::{ServiceUserRow, ServiceUserState};

    /// Signed-in person's request Valence and email.
    pub async fn session() -> Result<(Valence, String), ServerFnError> {
        let (ctx, user) = lepton_auth::require_auth_user().await?;
        let v = lepton_auth::user_valence(&ctx)?;
        Ok((v, user.email))
    }

    pub fn backend() -> Result<Arc<dyn ServiceUserAccessBackend>, ServerFnError> {
        use_service_user_access_backend()
            .ok_or_else(|| access_err("backend", &ServiceUserAccessError::NotConfigured))
    }

    pub fn service_user_err(op: &'static str, e: &ServiceUserError) -> ServerFnError {
        if matches!(e, ServiceUserError::Store | ServiceUserError::Gate(_)) {
            tracing::warn!(target: "lepton_app.service_users", op, reason_class = e.reason_class(), "service user server fn failed");
        }
        ServerFnError::new(e.to_string())
    }

    pub fn access_err(op: &'static str, e: &ServiceUserAccessError) -> ServerFnError {
        if matches!(e, ServiceUserAccessError::Store { .. }) {
            tracing::warn!(target: "lepton_app.service_users", op, reason_class = e.reason_class(), "service user server fn failed");
        }
        ServerFnError::new(e.to_string())
    }

    pub fn parse_id(raw: &str) -> Result<ServiceUserId, ServerFnError> {
        ServiceUserId::parse(raw)
            .map_err(|_| ServerFnError::new(ServiceUserError::NotFound.to_string()))
    }

    pub fn row(service_user: &ServiceUserAccount) -> ServiceUserRow {
        let id = service_user.id().map_or_else(String::new, |rid| {
            valence::extract_id_from_record(rid).unwrap_or_else(|_| rid.id().to_string())
        });
        ServiceUserRow {
            id,
            handle: service_user.handle().clone(),
            display_name: service_user.display_name().clone(),
            description: service_user.description().cloned(),
            state: match service_user.status() {
                ServiceUserAccountStatus::Active => ServiceUserState::Active,
                ServiceUserAccountStatus::Suspended => ServiceUserState::Paused,
            },
            created_on: service_user.created_at().format("%Y-%m-%d").to_string(),
        }
    }
}

/// The viewer's service users and what they may do on this page.
#[uf_product_macros::server]
pub async fn get_service_users_overview() -> Result<ServiceUsersOverview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (v, owner_email) = ssr::session().await?;
        let service_users = lepton_auth::service_users::list_my_service_users(&v)
            .await
            .map_err(|e| ssr::service_user_err("overview", &e))?;
        let can_create = match lepton_identity::service_users::installed_service_user_gate() {
            Some(gate) => gate.may_create_service_user(&v).await.unwrap_or(false),
            None => false,
        };
        Ok(ServiceUsersOverview {
            service_users: service_users.iter().map(ssr::row).collect(),
            limit: u32::try_from(lepton_auth::service_users::MAX_SERVICE_USERS_PER_OWNER)
                .unwrap_or(u32::MAX),
            can_create,
            tokens_configured: crate::service_users::port::use_service_user_access_backend()
                .is_some(),
            owner_email,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Whether `handle` is free, taken (with a free alternative), or breaks a rule.
#[uf_product_macros::server]
pub async fn check_service_user_handle(
    /// Handle as typed, without the `@`.
    handle: String,
) -> Result<HandleCheck, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use lepton_auth::service_users::ServiceUserError;
        use lepton_identity::service_users::ServiceUserHandle;

        use crate::service_users::types::HandleProblem;

        let (v, _) = ssr::session().await?;
        let parsed = match ServiceUserHandle::parse(handle.trim()) {
            Ok(parsed) => parsed,
            Err(e) => {
                return Ok(HandleCheck::Invalid {
                    problem: HandleProblem::from_reason(e.reason()),
                })
            }
        };
        match lepton_auth::service_users::suggest_service_user_handle(&v, parsed.as_str()).await {
            Ok(free) => Ok(HandleCheck::from_suggestion(parsed.as_str(), free.as_str())),
            Err(ServiceUserError::HandleTaken { suggestion }) => Ok(HandleCheck::Taken {
                handle: parsed.as_str().to_string(),
                suggestion: suggestion.map(|s| s.as_str().to_string()),
            }),
            Err(ServiceUserError::InvalidHandle(e)) => Ok(HandleCheck::Invalid {
                problem: HandleProblem::from_reason(e.reason()),
            }),
            Err(e) => Err(ssr::service_user_err("check_handle", &e)),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = handle;
        Err(ServerFnError::new("server only"))
    }
}

/// Create a service user owned by the viewer. Step 1 of the create flow.
#[uf_product_macros::server]
pub async fn create_service_user_profile(
    /// Handle without the `@`.
    handle: String,
    /// Name shown next to the service user's actions.
    display_name: String,
    /// What the service user is for.
    description: Option<String>,
) -> Result<ServiceUserRow, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (v, _) = ssr::session().await?;
        let service_user = lepton_auth::service_users::create_service_user(
            &v,
            lepton_auth::service_users::NewServiceUser {
                handle: handle.trim(),
                display_name: &display_name,
                description: description.as_deref(),
            },
        )
        .await
        .map_err(|e| ssr::service_user_err("create", &e))?;
        Ok(ssr::row(&service_user))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (handle, display_name, description);
        Err(ServerFnError::new("server only"))
    }
}

/// One of the viewer's service users (or any service user, for people who manage every service
/// user).
#[uf_product_macros::server]
pub async fn get_service_user_row(
    /// Service user id.
    service_user_id: String,
) -> Result<ServiceUserRow, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let service_user = lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("get", &e))?;
        Ok(ssr::row(&service_user))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = service_user_id;
        Err(ServerFnError::new("server only"))
    }
}

/// The access choices this site offers, and which ones the viewer may give.
#[uf_product_macros::server]
pub async fn service_user_access_choices() -> Result<Vec<AccessChoiceView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::service_users::port::AccessKind;

        let (v, _) = ssr::session().await?;
        let backend = ssr::backend()?;
        let mut out = Vec::new();
        for choice in backend.choices() {
            let (grantable, group_name) = match choice.kind {
                AccessKind::GroupMembership {
                    group_id,
                    group_name,
                } => (
                    backend
                        .can_grant_group(&v, group_id)
                        .await
                        .map_err(|e| ssr::access_err("choices", &e))?,
                    Some(group_name.to_string()),
                ),
                AccessKind::TokenOnly => (true, None),
            };
            out.push(AccessChoiceView {
                key: choice.key.to_string(),
                label: choice.label.to_string(),
                help: choice.help.to_string(),
                group_name,
                grantable,
            });
        }
        Ok(out)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Add the service user to the groups behind the picked choices. Step 2 of the create
/// flow. Needs a recent authenticator confirmation, like other group changes.
/// Token-only choices are ignored here; they go on the token in step 3.
#[uf_product_macros::server(step_up)]
pub async fn grant_service_user_access(
    /// Service user id.
    service_user_id: String,
    /// Picked choice keys.
    keys: Vec<String>,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::service_users::port::{resolve_choices, AccessKind, ServiceUserAccessError};

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let backend = ssr::backend()?;
        let chosen =
            resolve_choices(backend.choices(), &keys).map_err(|e| ssr::access_err("grant", &e))?;
        // Load first so a service user the viewer can't see fails the same way everywhere.
        lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("grant", &e))?;
        for choice in &chosen {
            if let AccessKind::GroupMembership { group_id, .. } = choice.kind {
                let allowed = backend
                    .can_grant_group(&v, group_id)
                    .await
                    .map_err(|e| ssr::access_err("grant", &e))?;
                if !allowed {
                    return Err(ssr::access_err(
                        "grant",
                        &ServiceUserAccessError::NotGrantable { choice: choice.key },
                    ));
                }
                backend
                    .grant_group(&v, &id, group_id, choice.key)
                    .await
                    .map_err(|e| ssr::access_err("grant", &e))?;
            }
        }
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            choice_count = chosen.len(),
            "service user access granted"
        );
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (service_user_id, keys);
        Err(ServerFnError::new("server only"))
    }
}

/// Mint a token for the service user. Step 3 of the create flow, and the "New token"
/// button on the Service users page. The response is the only time the secret is sent.
#[uf_product_macros::server]
pub async fn mint_service_user_token(
    /// Service user id.
    service_user_id: String,
    /// Where the token will be used, for example "Claude Code on laptop".
    label: String,
    /// How long it lasts.
    ttl: TokenTtl,
    /// Picked choice keys; their token grants go on the token.
    keys: Vec<String>,
) -> Result<MintedTokenView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::service_users::port::{
            resolve_choices, summarize, token_scopes, ServiceUserAccessError,
        };

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let backend = ssr::backend()?;
        let label = label.trim();
        if label.is_empty() || label.chars().count() > 80 {
            return Err(ssr::access_err(
                "mint",
                &ServiceUserAccessError::InvalidLabel,
            ));
        }
        let chosen =
            resolve_choices(backend.choices(), &keys).map_err(|e| ssr::access_err("mint", &e))?;
        let scopes = token_scopes(&chosen);
        if scopes.is_empty() {
            return Err(ssr::access_err(
                "mint",
                &ServiceUserAccessError::NoAccessChosen,
            ));
        }
        let service_user = lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("mint", &e))?;
        let minted = backend
            .mint_token(&v, &id, label, ttl, &scopes)
            .await
            .map_err(|e| ssr::access_err("mint", &e))?;
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            token_public_id = minted.public_id.as_str(),
            ttl_days = ttl.days(),
            scope_count = scopes.len(),
            "service user token minted"
        );
        Ok(MintedTokenView {
            public_id: minted.public_id,
            secret: minted.secret,
            expires_on: minted.expires_at.format("%Y-%m-%d").to_string(),
            can_do: summarize(&chosen),
            setup: backend.client_setup(service_user.handle(), &chosen),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (service_user_id, label, ttl, keys);
        Err(ServerFnError::new("server only"))
    }
}
