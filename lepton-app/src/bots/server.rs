//! Server functions behind the Bots pages.
//!
//! Each one recovers the signed-in person through Lepton and Higgs and passes
//! that request's Valence to Lepton's bot calls and to the host's
//! [`BotAccessBackend`](super::BotAccessBackend). Nothing here switches to a
//! system actor. A bot's own token never reaches these functions: bearer tokens
//! only work on the host's `/api/v1` routes, and Lepton refuses any caller that
//! isn't a signed-in user.

use leptos::prelude::*;

use super::types::{
    AccessChoiceView, BotRow, BotsOverview, HandleCheck, MintedTokenView, TokenTtl,
};

#[cfg(feature = "ssr")]
mod ssr {
    use std::sync::Arc;

    use lepton_auth::bots::BotError;
    use lepton_identity::bots::BotId;
    use lepton_identity::generated::{Bot, BotStatus};
    use leptos::prelude::ServerFnError;
    use valence::Valence;

    use crate::bots::port::{use_bot_access_backend, BotAccessBackend, BotAccessError};
    use crate::bots::types::{BotRow, BotState};

    /// Signed-in person's request Valence and email.
    pub async fn session() -> Result<(Valence, String), ServerFnError> {
        let (ctx, user) = lepton_auth::require_auth_user().await?;
        let v = lepton_auth::user_valence(&ctx)?;
        Ok((v, user.email))
    }

    pub fn backend() -> Result<Arc<dyn BotAccessBackend>, ServerFnError> {
        use_bot_access_backend()
            .ok_or_else(|| access_err("backend", &BotAccessError::NotConfigured))
    }

    pub fn bot_err(op: &'static str, e: &BotError) -> ServerFnError {
        if matches!(e, BotError::Store | BotError::Gate(_)) {
            tracing::warn!(target: "lepton_app.bots", op, reason_class = e.reason_class(), "bot server fn failed");
        }
        ServerFnError::new(e.to_string())
    }

    pub fn access_err(op: &'static str, e: &BotAccessError) -> ServerFnError {
        if matches!(e, BotAccessError::Store { .. }) {
            tracing::warn!(target: "lepton_app.bots", op, reason_class = e.reason_class(), "bot server fn failed");
        }
        ServerFnError::new(e.to_string())
    }

    pub fn parse_id(raw: &str) -> Result<BotId, ServerFnError> {
        BotId::parse(raw).map_err(|_| ServerFnError::new(BotError::NotFound.to_string()))
    }

    pub fn row(bot: &Bot) -> BotRow {
        let id = bot.id().map_or_else(String::new, |rid| {
            valence::extract_id_from_record(rid).unwrap_or_else(|_| rid.id().to_string())
        });
        BotRow {
            id,
            handle: bot.handle().clone(),
            display_name: bot.display_name().clone(),
            description: bot.description().cloned(),
            state: match bot.status() {
                BotStatus::Active => BotState::Active,
                BotStatus::Suspended => BotState::Paused,
            },
            created_on: bot.created_at().format("%Y-%m-%d").to_string(),
        }
    }
}

/// The viewer's bots and what they may do on this page.
#[uf_product_macros::server]
pub async fn get_bots_overview() -> Result<BotsOverview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (v, owner_email) = ssr::session().await?;
        let bots = lepton_auth::bots::list_my_bots(&v)
            .await
            .map_err(|e| ssr::bot_err("overview", &e))?;
        let can_create = match lepton_identity::bots::installed_bot_gate() {
            Some(gate) => gate.may_create_bot(&v).await.unwrap_or(false),
            None => false,
        };
        Ok(BotsOverview {
            bots: bots.iter().map(ssr::row).collect(),
            limit: u32::try_from(lepton_auth::bots::MAX_BOTS_PER_OWNER).unwrap_or(u32::MAX),
            can_create,
            tokens_configured: crate::bots::port::use_bot_access_backend().is_some(),
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
pub async fn check_bot_handle(
    /// Handle as typed, without the `@`.
    handle: String,
) -> Result<HandleCheck, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use lepton_auth::bots::BotError;
        use lepton_identity::bots::BotHandle;

        use crate::bots::types::HandleProblem;

        let (v, _) = ssr::session().await?;
        let parsed = match BotHandle::parse(handle.trim()) {
            Ok(parsed) => parsed,
            Err(e) => {
                return Ok(HandleCheck::Invalid {
                    problem: HandleProblem::from_reason(e.reason()),
                })
            }
        };
        match lepton_auth::bots::suggest_bot_handle(&v, parsed.as_str()).await {
            Ok(free) => Ok(HandleCheck::from_suggestion(parsed.as_str(), free.as_str())),
            Err(BotError::HandleTaken { suggestion }) => Ok(HandleCheck::Taken {
                handle: parsed.as_str().to_string(),
                suggestion: suggestion.map(|s| s.as_str().to_string()),
            }),
            Err(BotError::InvalidHandle(e)) => Ok(HandleCheck::Invalid {
                problem: HandleProblem::from_reason(e.reason()),
            }),
            Err(e) => Err(ssr::bot_err("check_handle", &e)),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = handle;
        Err(ServerFnError::new("server only"))
    }
}

/// Create a bot owned by the viewer. Step 1 of the create flow.
#[uf_product_macros::server]
pub async fn create_bot_profile(
    /// Handle without the `@`.
    handle: String,
    /// Name shown next to the bot's actions.
    display_name: String,
    /// What the bot is for.
    description: Option<String>,
) -> Result<BotRow, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (v, _) = ssr::session().await?;
        let bot = lepton_auth::bots::create_bot(
            &v,
            lepton_auth::bots::NewBot {
                handle: handle.trim(),
                display_name: &display_name,
                description: description.as_deref(),
            },
        )
        .await
        .map_err(|e| ssr::bot_err("create", &e))?;
        Ok(ssr::row(&bot))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (handle, display_name, description);
        Err(ServerFnError::new("server only"))
    }
}

/// One of the viewer's bots (or any bot, for people who manage every bot).
#[uf_product_macros::server]
pub async fn get_bot_row(
    /// Bot id.
    bot_id: String,
) -> Result<BotRow, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&bot_id)?;
        let bot = lepton_auth::bots::get_bot(&v, &id)
            .await
            .map_err(|e| ssr::bot_err("get", &e))?;
        Ok(ssr::row(&bot))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = bot_id;
        Err(ServerFnError::new("server only"))
    }
}

/// The access choices this site offers, and which ones the viewer may give.
#[uf_product_macros::server]
pub async fn bot_access_choices() -> Result<Vec<AccessChoiceView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::bots::port::AccessKind;

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

/// Add the bot to the groups behind the picked choices. Step 2 of the create
/// flow. Needs a recent authenticator confirmation, like other group changes.
/// Token-only choices are ignored here; they go on the token in step 3.
#[uf_product_macros::server(step_up)]
pub async fn grant_bot_access(
    /// Bot id.
    bot_id: String,
    /// Picked choice keys.
    keys: Vec<String>,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::bots::port::{resolve_choices, AccessKind, BotAccessError};

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&bot_id)?;
        let backend = ssr::backend()?;
        let chosen =
            resolve_choices(backend.choices(), &keys).map_err(|e| ssr::access_err("grant", &e))?;
        // Load first so a bot the viewer can't see fails the same way everywhere.
        lepton_auth::bots::get_bot(&v, &id)
            .await
            .map_err(|e| ssr::bot_err("grant", &e))?;
        for choice in &chosen {
            if let AccessKind::GroupMembership { group_id, .. } = choice.kind {
                let allowed = backend
                    .can_grant_group(&v, group_id)
                    .await
                    .map_err(|e| ssr::access_err("grant", &e))?;
                if !allowed {
                    return Err(ssr::access_err(
                        "grant",
                        &BotAccessError::NotGrantable { choice: choice.key },
                    ));
                }
                backend
                    .grant_group(&v, &id, group_id, choice.key)
                    .await
                    .map_err(|e| ssr::access_err("grant", &e))?;
            }
        }
        tracing::info!(
            target: "lepton_app.bots",
            bot_id = id.as_str(),
            choice_count = chosen.len(),
            "bot access granted"
        );
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (bot_id, keys);
        Err(ServerFnError::new("server only"))
    }
}

/// Mint a token for the bot. Step 3 of the create flow, and the "New token"
/// button on the Bots page. The response is the only time the secret is sent.
#[uf_product_macros::server]
pub async fn mint_bot_token(
    /// Bot id.
    bot_id: String,
    /// Where the token will be used, for example "Claude Code on laptop".
    label: String,
    /// How long it lasts.
    ttl: TokenTtl,
    /// Picked choice keys; their token grants go on the token.
    keys: Vec<String>,
) -> Result<MintedTokenView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::bots::port::{resolve_choices, summarize, token_scopes, BotAccessError};

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&bot_id)?;
        let backend = ssr::backend()?;
        let label = label.trim();
        if label.is_empty() || label.chars().count() > 80 {
            return Err(ssr::access_err("mint", &BotAccessError::InvalidLabel));
        }
        let chosen =
            resolve_choices(backend.choices(), &keys).map_err(|e| ssr::access_err("mint", &e))?;
        let scopes = token_scopes(&chosen);
        if scopes.is_empty() {
            return Err(ssr::access_err("mint", &BotAccessError::NoAccessChosen));
        }
        let bot = lepton_auth::bots::get_bot(&v, &id)
            .await
            .map_err(|e| ssr::bot_err("mint", &e))?;
        let minted = backend
            .mint_token(&v, &id, label, ttl, &scopes)
            .await
            .map_err(|e| ssr::access_err("mint", &e))?;
        tracing::info!(
            target: "lepton_app.bots",
            bot_id = id.as_str(),
            token_public_id = minted.public_id.as_str(),
            ttl_days = ttl.days(),
            scope_count = scopes.len(),
            "bot token minted"
        );
        Ok(MintedTokenView {
            public_id: minted.public_id,
            secret: minted.secret,
            expires_on: minted.expires_at.format("%Y-%m-%d").to_string(),
            can_do: summarize(&chosen),
            setup: backend.client_setup(bot.handle(), &chosen),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (bot_id, label, ttl, keys);
        Err(ServerFnError::new("server only"))
    }
}
