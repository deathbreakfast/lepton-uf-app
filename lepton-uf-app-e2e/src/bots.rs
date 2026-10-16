//! Lab bot wiring: the Gauge bot gate, a "Spectra readers" stand-in group, a
//! [`BotAccessBackend`] that keeps tokens in memory, and a seed route.
//!
//! Lab-only. The token store is a stand-in for a host's real token service:
//! it hands out random secrets, counts them per bot, and forgets them on
//! restart. Group grants go through Gauge for real, under the signed-in
//! person's Valence.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lepton_app::bots::{
    AccessKind, BotAccessBackend, BotAccessChoice, BotAccessError, ClientSetupStep, MintedToken,
    RevealOnce, TokenTtl,
};
use lepton_auth::bots::BotError;
use lepton_identity::bots::BotId;
use lepton_identity::generated::BotStatus;
use valence::{Actor, Model, Valence};

/// Lab stand-in for the host's Spectra readers group.
pub const LAB_READERS_GROUP_ID: &str = "spectra_readers";
const LAB_READ_PERMISSION: &str = "lab.spectra.query";
/// Most active tokens per bot, matching the production token service.
const MAX_TOKENS: usize = 5;

static CHOICES: &[BotAccessChoice] = &[
    BotAccessChoice {
        key: "spectra-read",
        label: "Read Spectra metrics and events",
        summary: "read Spectra metrics and events",
        help: "Joins Spectra readers. Its token can query Spectra.",
        kind: AccessKind::GroupMembership {
            group_id: LAB_READERS_GROUP_ID,
            group_name: "Spectra readers",
        },
        token_scopes: &["spectra:read"],
    },
    BotAccessChoice {
        key: "magnon-read",
        label: "Read Magnon projects and tasks",
        summary: "read Magnon projects and tasks",
        help: "Its token can read Magnon.",
        kind: AccessKind::TokenOnly,
        token_scopes: &["magnon:read"],
    },
    BotAccessChoice {
        key: "magnon-write",
        label: "Change Magnon projects and tasks",
        summary: "change Magnon projects and tasks",
        help: "Its token can create and edit Magnon tasks.",
        kind: AccessKind::TokenOnly,
        token_scopes: &["magnon:read", "magnon:write"],
    },
];

/// In-memory token counts per bot id.
#[derive(Default)]
pub struct LabTokens {
    by_bot: Mutex<HashMap<String, usize>>,
}

impl LabTokens {
    /// Lab seed: pretend `bot` already has `n` tokens.
    pub fn set_count(&self, bot: &str, n: usize) {
        if let Ok(mut m) = self.by_bot.lock() {
            m.insert(bot.to_string(), n);
        }
    }

    fn add(&self, bot: &str) -> Result<(), BotAccessError> {
        let mut m = self.by_bot.lock().map_err(|_| BotAccessError::Store {
            reason_class: "lab_lock",
        })?;
        let n = m.entry(bot.to_string()).or_insert(0);
        if *n >= MAX_TOKENS {
            return Err(BotAccessError::TooManyTokens { limit: 5 });
        }
        *n += 1;
        Ok(())
    }
}

/// Lab [`BotAccessBackend`].
pub struct LabBotAccess {
    tokens: Arc<LabTokens>,
}

impl LabBotAccess {
    /// Backend sharing `tokens` with the seed route.
    #[must_use]
    pub const fn new(tokens: Arc<LabTokens>) -> Self {
        Self { tokens }
    }
}

fn gauge_err(e: &anyhow::Error, choice: &'static str) -> BotAccessError {
    match e.downcast_ref::<gauge::service::GaugeServiceError>() {
        Some(gauge::service::GaugeServiceError::NotAuthorized { .. }) => {
            BotAccessError::NotGrantable { choice }
        }
        Some(gauge::service::GaugeServiceError::NotFound { .. }) => BotAccessError::BotNotFound,
        _ => BotAccessError::Store {
            reason_class: "gauge",
        },
    }
}

#[async_trait::async_trait]
impl BotAccessBackend for LabBotAccess {
    fn choices(&self) -> &'static [BotAccessChoice] {
        CHOICES
    }

    async fn can_grant_group(&self, v: &Valence, group_id: &str) -> Result<bool, BotAccessError> {
        gauge::service::can_add_group_member_bot(group_id, v)
            .await
            .map_err(|_| BotAccessError::Store {
                reason_class: "gauge",
            })
    }

    async fn grant_group(
        &self,
        v: &Valence,
        bot: &BotId,
        group_id: &'static str,
        choice: &'static str,
    ) -> Result<(), BotAccessError> {
        gauge::service::add_group_member_bot(group_id, bot, v)
            .await
            .map_err(|e| gauge_err(&e, choice))
    }

    async fn mint_token(
        &self,
        v: &Valence,
        bot: &BotId,
        _label: &str,
        ttl: TokenTtl,
        _scopes: &[&'static str],
    ) -> Result<MintedToken, BotAccessError> {
        if !matches!(v.actor(), Actor::User { .. }) {
            return Err(BotAccessError::Forbidden);
        }
        let row = lepton_auth::bots::get_bot(v, bot)
            .await
            .map_err(|e| match e {
                BotError::NotFound => BotAccessError::BotNotFound,
                BotError::NotAUser => BotAccessError::Forbidden,
                _ => BotAccessError::Store {
                    reason_class: "bot_lookup",
                },
            })?;
        if row.status() == &BotStatus::Suspended {
            return Err(BotAccessError::BotSuspended);
        }
        self.tokens.add(bot.as_str())?;
        let id = BotId::new_v4();
        Ok(MintedToken {
            public_id: format!("tok_{}", &id.as_str()[..12]),
            secret: RevealOnce::new(format!("labtk_{}", id.as_str().replace('-', ""))),
            expires_at: chrono::Utc::now() + chrono::Duration::days(i64::from(ttl.days())),
        })
    }

    fn client_setup(&self, handle: &str, chosen: &[BotAccessChoice]) -> Vec<ClientSetupStep> {
        let var = format!("{}_TOKEN", handle.replace('-', "_").to_ascii_uppercase());
        let mut steps = Vec::new();
        if chosen.iter().any(|c| c.key == "spectra-read") {
            steps.push(ClientSetupStep {
                title: "Add Spectra to Claude Code".to_string(),
                code: format!(
                    "export {var}='paste-token-here'\nclaude mcp add-json --scope user lab-spectra \\\n  '{{\"type\":\"http\",\"url\":\"http://localhost:3140/spectra/api/v1/mcp\",\"headers\":{{\"Authorization\":\"Bearer ${{{var}}}\"}}}}'"
                ),
            });
        }
        if chosen.iter().any(|c| c.key.starts_with("magnon")) {
            steps.push(ClientSetupStep {
                title: "Add Magnon to Claude Code".to_string(),
                code: format!(
                    "export {var}='paste-token-here'\nclaude mcp add-json --scope user lab-magnon \\\n  '{{\"type\":\"http\",\"url\":\"http://localhost:3140/magnon/api/v1/mcp\",\"headers\":{{\"Authorization\":\"Bearer ${{{var}}}\"}}}}'"
                ),
            });
        }
        steps
    }
}

/// Boot step: Super User group, bot permissions and gate, and the lab
/// "Spectra readers" group with no owners (only super users can add bots).
///
/// # Errors
///
/// Returns when Gauge seeding fails.
pub async fn boot_bots(system: &Valence) -> anyhow::Result<()> {
    use gauge::manifest_sync::{PermissionDomainInput, PermissionInput, PermissionManifestInput};

    gauge::super_user::ensure_super_user_group(system).await?;
    let _stats = gauge::bots::seed_bot_permissions(system).await?;
    // A host process installs the gate once; the lab only boots once too.
    let _ = gauge::bots::install_gauge_bot_gate();
    gauge::manifest_sync::sync_permission_manifests(
        system,
        &[PermissionManifestInput {
            app_id: "lab-spectra".to_string(),
            domains: vec![PermissionDomainInput {
                key: "lab".to_string(),
                name: "Lab".to_string(),
                description: "Lab stand-in for a Spectra query permission.".to_string(),
                permissions: vec![PermissionInput {
                    name: LAB_READ_PERMISSION.to_string(),
                    description: "Query lab Spectra events.".to_string(),
                }],
            }],
        }],
    )
    .await?;
    gauge::resource_permissions::ensure_group_granted_permission(
        system,
        LAB_READERS_GROUP_ID,
        "Spectra readers",
        "Lab stand-in for the host's Spectra readers group.",
        LAB_READ_PERMISSION,
        &[],
    )
    .await
    .map_err(|e| anyhow::anyhow!("lab readers group: {e}"))?;
    Ok(())
}

/// Bare user id for the account whose primary email is `email`.
async fn user_id_for_email(system: &Valence, email: &str) -> anyhow::Result<String> {
    let rows = lepton_identity::generated::AccountEmail::query(system, valence::use_!(r"**Test:** Lab host **finds the account email row** a Playwright run just seeded, so it can act as that user when seeding bots. Only the lab harness uses this."))
        .where_address(valence::StringPredicate::Equals(email.to_string()))
        .await?;
    let email_id = rows
        .first()
        .and_then(|r| r.id().cloned())
        .ok_or_else(|| anyhow::anyhow!("no email row"))?;
    let user = lepton_identity::generated::User::query(system, valence::use_!(r"**Test:** Lab host **finds the user** behind a seeded email so a Playwright run can act as them when seeding bots. Only the lab harness uses this."))
        .where_primary_email(valence::RecordPredicate::Equals(email_id))
        .first()
        .await?
        .ok_or_else(|| anyhow::anyhow!("no user"))?;
    let rid = user
        .id()
        .ok_or_else(|| anyhow::anyhow!("user without id"))?;
    Ok(valence::extract_id_from_record(rid).unwrap_or_else(|_| rid.id().to_string()))
}

/// Request body for [`bot_lab`].
#[derive(serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BotLabRequest {
    /// Add the user to Bot creators.
    JoinCreators { email: String },
    /// Make the user a super user (may add bots to any group).
    SuperUser { email: String },
    /// Create `count` bots owned by the user (who must be a Bot creator).
    SeedBots {
        email: String,
        prefix: String,
        count: usize,
        #[serde(default)]
        suspend: bool,
    },
    /// Pretend the bot already has `count` tokens.
    SeedTokens { bot_id: String, count: usize },
    /// Whether the bot is in the lab Spectra readers group.
    InReaders { bot_id: String },
}

/// Lab-only seed route for the Bots pages. Do not mount on a production host.
pub async fn bot_lab(
    axum::extract::State(state): axum::extract::State<crate::boot::AppState>,
    axum::Extension(tokens): axum::Extension<Arc<LabTokens>>,
    axum::Json(body): axum::Json<BotLabRequest>,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let fail = |e: anyhow::Error| (axum::http::StatusCode::BAD_REQUEST, format!("{e:#}"));
    let system = crate::boot::system_valence(
        Arc::clone(&state.valence_router),
        &state.default_backend_key,
    )
    .map_err(fail)?;
    let as_user = |user_id: String| {
        Valence::builder()
            .database_router(Arc::clone(&state.valence_router))
            .default_backend_key(state.default_backend_key.clone())
            .with_actor(Actor::User { user_id })
            .build()
            .map_err(|e| anyhow::anyhow!("{e}"))
    };
    let out = match body {
        BotLabRequest::JoinCreators { email } => {
            let group = gauge::generated::PermissionGroup::get(gauge::bots::BOT_CREATORS_GROUP_ID, &system, valence::use_!(r"**Test:** Lab host **loads the Bot creators group** so a Playwright run can add a user to it, standing in for a super user doing that. Only the lab harness uses this."))
                .await
                .map_err(|e| fail(e.into()))?
                .ok_or_else(|| fail(anyhow::anyhow!("no Bot creators group")))?;
            gauge::super_user::seed_super_user_member_by_email(&system, &group, &email)
                .await
                .map_err(fail)?;
            serde_json::json!({ "ok": true })
        }
        BotLabRequest::SuperUser { email } => {
            let group = gauge::super_user::ensure_super_user_group(&system)
                .await
                .map_err(fail)?;
            gauge::super_user::seed_super_user_member_by_email(&system, &group, &email)
                .await
                .map_err(fail)?;
            serde_json::json!({ "ok": true })
        }
        BotLabRequest::SeedBots {
            email,
            prefix,
            count,
            suspend,
        } => {
            let user_id = user_id_for_email(&system, &email).await.map_err(fail)?;
            let v = as_user(user_id).map_err(fail)?;
            let mut ids = Vec::new();
            for i in 0..count {
                let handle = format!("{prefix}-{i}-bot");
                let bot = lepton_auth::bots::create_bot(
                    &v,
                    lepton_auth::bots::NewBot {
                        handle: &handle,
                        display_name: &format!("Seeded {i}"),
                        description: None,
                    },
                )
                .await
                .map_err(|e| fail(e.into()))?;
                let id = bot
                    .id()
                    .map(|rid| {
                        valence::extract_id_from_record(rid)
                            .unwrap_or_else(|_| rid.id().to_string())
                    })
                    .unwrap_or_default();
                if suspend {
                    let bid = BotId::parse(&id).map_err(|e| fail(anyhow::anyhow!("{e}")))?;
                    lepton_auth::bots::suspend_bot(&v, &bid)
                        .await
                        .map_err(|e| fail(e.into()))?;
                }
                ids.push(serde_json::json!({ "id": id, "handle": handle }));
            }
            serde_json::json!({ "ok": true, "bots": ids })
        }
        BotLabRequest::SeedTokens { bot_id, count } => {
            tokens.set_count(&bot_id, count);
            serde_json::json!({ "ok": true })
        }
        BotLabRequest::InReaders { bot_id } => {
            let group = gauge::generated::PermissionGroup::get(LAB_READERS_GROUP_ID, &system, valence::use_!(r"**Test:** Lab host **loads the lab Spectra readers group** so a Playwright run can check whether a bot joined it. Only the lab harness uses this."))
                .await
                .map_err(|e| fail(e.into()))?
                .ok_or_else(|| fail(anyhow::anyhow!("no readers group")))?;
            let principal =
                valence::RecordId::new("permission_bot_principal", format!("bot:{bot_id}"));
            let members = group
                .get_members_record_ids(&system, valence::use_!(r"**Test:** Lab host **reads the lab Spectra readers member list** so a Playwright run can check whether a bot joined. Only the lab harness uses this."))
                .await
                .map_err(|e| fail(e.into()))?;
            serde_json::json!({ "member": members.contains(&principal) })
        }
    };
    Ok(axum::Json(out))
}
