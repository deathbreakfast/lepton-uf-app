//! Lab service user wiring: the Gauge service user gate, a "Spectra readers" stand-in group, a
//! [`ServiceUserAccessBackend`] that keeps tokens and group listings in memory, and a seed
//! route.
//!
//! Lab-only. The token store is a stand-in for a host's real token service:
//! it hands out random secrets, lists and revokes them per service user, and forgets them
//! on restart. Group grants go through Gauge for real, under the signed-in
//! person's Valence. Gauge can't yet list a service user's groups and permissions, so the
//! detail page reads a memory copy of what was granted here or seeded by a test.
//!
//! Boot also registers a no-op Valence deletion dispatcher, so Delete works: the deleted
//! service user stays hidden from reads and nothing purges it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lepton_app::service_users::{
    AccessKind, ClientSetupStep, MintedToken, RevealOnce, ServiceUserAccessBackend,
    ServiceUserAccessChoice, ServiceUserAccessError, ServiceUserGroupView,
    ServiceUserPermissionView, ServiceUserTokenView, TokenTtl,
};
use lepton_auth::service_users::ServiceUserError;
use lepton_identity::generated::ServiceUserAccountStatus;
use lepton_identity::service_users::ServiceUserId;
use valence::{Actor, Model, Valence};

/// Lab stand-in for the host's Spectra readers group.
pub const LAB_READERS_GROUP_ID: &str = "spectra_readers";
const LAB_READERS_GROUP_NAME: &str = "Spectra readers";
const LAB_READERS_GROUP_DESCRIPTION: &str = "Lab stand-in for the host's Spectra readers group.";
const LAB_READ_PERMISSION: &str = "lab.spectra.query";
const LAB_READ_PERMISSION_DESCRIPTION: &str = "Query lab Spectra events.";
/// Most active tokens per service user, matching the production token service.
const MAX_TOKENS: usize = 5;

static CHOICES: &[ServiceUserAccessChoice] = &[
    ServiceUserAccessChoice {
        key: "spectra-read",
        label: "Read Spectra metrics and events",
        summary: "read Spectra metrics and events",
        help: "Joins Spectra readers. Its token can query Spectra.",
        kind: AccessKind::GroupMembership {
            group_id: LAB_READERS_GROUP_ID,
            group_name: LAB_READERS_GROUP_NAME,
        },
        token_scopes: &["spectra:read"],
    },
    ServiceUserAccessChoice {
        key: "magnon-read",
        label: "Read Magnon projects and tasks",
        summary: "read Magnon projects and tasks",
        help: "Its token can read Magnon.",
        kind: AccessKind::TokenOnly,
        token_scopes: &["magnon:read"],
    },
    ServiceUserAccessChoice {
        key: "magnon-write",
        label: "Change Magnon projects and tasks",
        summary: "change Magnon projects and tasks",
        help: "Its token can create and edit Magnon tasks.",
        kind: AccessKind::TokenOnly,
        token_scopes: &["magnon:read", "magnon:write"],
    },
];

/// Groups and permissions the detail page lists for one service user.
#[derive(Clone, Default)]
struct LabMembership {
    groups: Vec<ServiceUserGroupView>,
    permissions: Vec<ServiceUserPermissionView>,
}

/// In-memory tokens and group listings per service user id.
#[derive(Default)]
pub struct LabTokens {
    by_service_user: Mutex<HashMap<String, Vec<ServiceUserTokenView>>>,
    memberships: Mutex<HashMap<String, LabMembership>>,
}

impl LabTokens {
    /// Lab seed: list `groups` groups and `permissions` permissions for `service_user`,
    /// replacing what it had.
    pub fn seed_memberships(&self, service_user: &str, groups: usize, permissions: usize) {
        let group_names: Vec<String> = (0..groups).map(|i| format!("Lab group {i:03}")).collect();
        let membership = LabMembership {
            groups: group_names
                .iter()
                .enumerate()
                .map(|(i, name)| ServiceUserGroupView {
                    id: format!("lab_group_{i:03}"),
                    name: name.clone(),
                    description: format!("Seeded group {i}."),
                })
                .collect(),
            permissions: (0..permissions)
                .map(|i| ServiceUserPermissionView {
                    name: format!("lab.seeded.perm_{i:03}"),
                    description: format!("Seeded permission {i}."),
                    via_group: group_names
                        .get(i % groups.max(1))
                        .cloned()
                        .unwrap_or_default(),
                })
                .collect(),
        };
        if let Ok(mut m) = self.memberships.lock() {
            m.insert(service_user.to_string(), membership);
        }
    }

    /// Note that `service_user` joined the lab readers group.
    fn record_readers_grant(&self, service_user: &str) -> Result<(), ServiceUserAccessError> {
        let mut m = self
            .memberships
            .lock()
            .map_err(|_| ServiceUserAccessError::Store {
                reason_class: "lab_lock",
            })?;
        let entry = m.entry(service_user.to_string()).or_default();
        if entry.groups.iter().any(|g| g.id == LAB_READERS_GROUP_ID) {
            return Ok(());
        }
        entry.groups.push(ServiceUserGroupView {
            id: LAB_READERS_GROUP_ID.to_string(),
            name: LAB_READERS_GROUP_NAME.to_string(),
            description: LAB_READERS_GROUP_DESCRIPTION.to_string(),
        });
        entry.permissions.push(ServiceUserPermissionView {
            name: LAB_READ_PERMISSION.to_string(),
            description: LAB_READ_PERMISSION_DESCRIPTION.to_string(),
            via_group: LAB_READERS_GROUP_NAME.to_string(),
        });
        Ok(())
    }

    fn membership(&self, service_user: &str) -> Result<LabMembership, ServiceUserAccessError> {
        let m = self
            .memberships
            .lock()
            .map_err(|_| ServiceUserAccessError::Store {
                reason_class: "lab_lock",
            })?;
        Ok(m.get(service_user).cloned().unwrap_or_default())
    }

    /// Lab seed: replace `service_user`'s tokens with `n` labelled ones. Even-numbered
    /// tokens were last used today; the rest never were. Returns their public ids.
    pub fn seed(&self, service_user: &str, n: usize) -> Vec<String> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let expires = (chrono::Utc::now() + chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();
        let tokens: Vec<ServiceUserTokenView> = (0..n)
            .map(|i| ServiceUserTokenView {
                public_id: lab_public_id(),
                label: format!("Seeded token {i}"),
                created_on: today.clone(),
                expires_on: expires.clone(),
                last_used_on: (i % 2 == 0).then(|| today.clone()),
            })
            .collect();
        let ids = tokens.iter().map(|t| t.public_id.clone()).collect();
        if let Ok(mut m) = self.by_service_user.lock() {
            m.insert(service_user.to_string(), tokens);
        }
        ids
    }

    fn lock(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, HashMap<String, Vec<ServiceUserTokenView>>>,
        ServiceUserAccessError,
    > {
        self.by_service_user
            .lock()
            .map_err(|_| ServiceUserAccessError::Store {
                reason_class: "lab_lock",
            })
    }

    fn add(
        &self,
        service_user: &str,
        token: ServiceUserTokenView,
    ) -> Result<(), ServiceUserAccessError> {
        let mut m = self.lock()?;
        let tokens = m.entry(service_user.to_string()).or_default();
        if tokens.len() >= MAX_TOKENS {
            return Err(ServiceUserAccessError::TooManyTokens { limit: 5 });
        }
        tokens.push(token);
        Ok(())
    }

    fn list(
        &self,
        service_user: &str,
    ) -> Result<Vec<ServiceUserTokenView>, ServiceUserAccessError> {
        Ok(self.lock()?.get(service_user).cloned().unwrap_or_default())
    }

    /// Remove `public_id` from `service_user`'s tokens. A token of another service user
    /// is refused and kept.
    fn revoke(&self, service_user: &str, public_id: &str) -> Result<(), ServiceUserAccessError> {
        let mut m = self.lock()?;
        if let Some(tokens) = m.get_mut(service_user) {
            if let Some(pos) = tokens.iter().position(|t| t.public_id == public_id) {
                tokens.remove(pos);
                return Ok(());
            }
        }
        if m.values()
            .any(|tokens| tokens.iter().any(|t| t.public_id == public_id))
        {
            return Err(ServiceUserAccessError::Forbidden);
        }
        Err(ServiceUserAccessError::TokenNotFound)
    }
}

/// Fresh lab token public id.
fn lab_public_id() -> String {
    let id = ServiceUserId::new_v4();
    format!("tok_{}", &id.as_str()[..12])
}

/// Lab [`ServiceUserAccessBackend`].
pub struct LabServiceUserAccess {
    tokens: Arc<LabTokens>,
}

impl LabServiceUserAccess {
    /// Backend sharing `tokens` with the seed route.
    #[must_use]
    pub const fn new(tokens: Arc<LabTokens>) -> Self {
        Self { tokens }
    }
}

fn gauge_err(e: &anyhow::Error, choice: &'static str) -> ServiceUserAccessError {
    match e.downcast_ref::<gauge::service::GaugeServiceError>() {
        Some(gauge::service::GaugeServiceError::NotAuthorized { .. }) => {
            ServiceUserAccessError::NotGrantable { choice }
        }
        Some(gauge::service::GaugeServiceError::NotFound { .. }) => {
            ServiceUserAccessError::ServiceUserNotFound
        }
        _ => ServiceUserAccessError::Store {
            reason_class: "gauge",
        },
    }
}

#[async_trait::async_trait]
impl ServiceUserAccessBackend for LabServiceUserAccess {
    fn choices(&self) -> &'static [ServiceUserAccessChoice] {
        CHOICES
    }

    fn request_access_url(&self, group_name: &str, for_handle: Option<&str>) -> Option<String> {
        Some(lepton_app::service_users::request_access_path(
            group_name, for_handle,
        ))
    }

    async fn can_grant_group(
        &self,
        v: &Valence,
        group_id: &str,
    ) -> Result<bool, ServiceUserAccessError> {
        gauge::service::can_add_group_member_service_user(group_id, v)
            .await
            .map_err(|_| ServiceUserAccessError::Store {
                reason_class: "gauge",
            })
    }

    async fn grant_group(
        &self,
        v: &Valence,
        service_user: &ServiceUserId,
        group_id: &'static str,
        choice: &'static str,
    ) -> Result<(), ServiceUserAccessError> {
        gauge::service::add_group_member_service_user(group_id, service_user, v)
            .await
            .map_err(|e| gauge_err(&e, choice))?;
        if group_id == LAB_READERS_GROUP_ID {
            self.tokens.record_readers_grant(service_user.as_str())?;
        }
        Ok(())
    }

    async fn service_user_groups(
        &self,
        _v: &Valence,
        service_user: &ServiceUserId,
    ) -> Result<Vec<ServiceUserGroupView>, ServiceUserAccessError> {
        // The caller already loaded the service user under its own Valence.
        Ok(self.tokens.membership(service_user.as_str())?.groups)
    }

    async fn service_user_permissions(
        &self,
        _v: &Valence,
        service_user: &ServiceUserId,
    ) -> Result<Vec<ServiceUserPermissionView>, ServiceUserAccessError> {
        Ok(self.tokens.membership(service_user.as_str())?.permissions)
    }

    async fn list_tokens(
        &self,
        _v: &Valence,
        service_user: &ServiceUserId,
    ) -> Result<Vec<ServiceUserTokenView>, ServiceUserAccessError> {
        // The caller already loaded the service user under its own Valence.
        self.tokens.list(service_user.as_str())
    }

    async fn revoke_token(
        &self,
        v: &Valence,
        service_user: &ServiceUserId,
        public_id: &str,
    ) -> Result<(), ServiceUserAccessError> {
        if !matches!(v.actor(), Actor::User { .. }) {
            return Err(ServiceUserAccessError::Forbidden);
        }
        self.tokens.revoke(service_user.as_str(), public_id)
    }

    async fn mint_token(
        &self,
        v: &Valence,
        service_user: &ServiceUserId,
        label: &str,
        ttl: TokenTtl,
        _scopes: &[&'static str],
    ) -> Result<MintedToken, ServiceUserAccessError> {
        if !matches!(v.actor(), Actor::User { .. }) {
            return Err(ServiceUserAccessError::Forbidden);
        }
        let row = lepton_auth::service_users::get_service_user(v, service_user)
            .await
            .map_err(|e| match e {
                ServiceUserError::NotFound => ServiceUserAccessError::ServiceUserNotFound,
                ServiceUserError::NotAUser => ServiceUserAccessError::Forbidden,
                _ => ServiceUserAccessError::Store {
                    reason_class: "service_user_lookup",
                },
            })?;
        if row.status() == &ServiceUserAccountStatus::Suspended {
            return Err(ServiceUserAccessError::ServiceUserSuspended);
        }
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::days(i64::from(ttl.days()));
        let public_id = lab_public_id();
        self.tokens.add(
            service_user.as_str(),
            ServiceUserTokenView {
                public_id: public_id.clone(),
                label: label.to_string(),
                created_on: now.format("%Y-%m-%d").to_string(),
                expires_on: expires_at.format("%Y-%m-%d").to_string(),
                last_used_on: None,
            },
        )?;
        let secret = ServiceUserId::new_v4();
        Ok(MintedToken {
            public_id,
            secret: RevealOnce::new(format!("labtk_{}", secret.as_str().replace('-', ""))),
            expires_at,
        })
    }

    fn client_setup(
        &self,
        handle: &str,
        chosen: &[ServiceUserAccessChoice],
    ) -> Vec<ClientSetupStep> {
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

/// Boot step: Super User group, service user permissions and gate, and the lab
/// "Spectra readers" group with no owners (only super users can add service users).
///
/// # Errors
///
/// Returns when Gauge seeding fails.
pub async fn boot_service_users(system: &Valence) -> anyhow::Result<()> {
    use gauge::manifest_sync::{PermissionDomainInput, PermissionInput, PermissionManifestInput};

    // Lab only: Delete needs a dispatcher. Nothing purges deleted rows here; Lepton keeps
    // them hidden from reads.
    valence::deletion::register_noop_deletion_dispatcher_for_tests();
    gauge::super_user::ensure_super_user_group(system).await?;
    let _stats = gauge::service_users::seed_service_user_permissions(system).await?;
    // A host process installs the gate once; the lab only boots once too.
    let _ = gauge::service_users::install_gauge_service_user_gate();
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
                    description: LAB_READ_PERMISSION_DESCRIPTION.to_string(),
                }],
            }],
        }],
    )
    .await?;
    gauge::resource_permissions::ensure_group_granted_permission(
        system,
        LAB_READERS_GROUP_ID,
        LAB_READERS_GROUP_NAME,
        LAB_READERS_GROUP_DESCRIPTION,
        LAB_READ_PERMISSION,
        &[],
    )
    .await
    .map_err(|e| anyhow::anyhow!("lab readers group: {e}"))?;
    Ok(())
}

/// Bare user id for the account whose primary email is `email`.
async fn user_id_for_email(system: &Valence, email: &str) -> anyhow::Result<String> {
    let rows = lepton_identity::generated::AccountEmail::query(system, valence::use_!(r"**Test:** Lab host **finds the account email row** a Playwright run just seeded, so it can act as that user when seeding service users. Only the lab harness uses this."))
        .where_address(valence::StringPredicate::Equals(email.to_string()))
        .await?;
    let email_id = rows
        .first()
        .and_then(|r| r.id().cloned())
        .ok_or_else(|| anyhow::anyhow!("no email row"))?;
    let user = lepton_identity::generated::User::query(system, valence::use_!(r"**Test:** Lab host **finds the user** behind a seeded email so a Playwright run can act as them when seeding service users. Only the lab harness uses this."))
        .where_primary_email(valence::RecordPredicate::Equals(email_id))
        .first()
        .await?
        .ok_or_else(|| anyhow::anyhow!("no user"))?;
    let rid = user
        .id()
        .ok_or_else(|| anyhow::anyhow!("user without id"))?;
    Ok(valence::extract_id_from_record(rid).unwrap_or_else(|_| rid.id().to_string()))
}

/// Request body for [`service_user_lab`].
#[derive(serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ServiceUserLabRequest {
    /// Add the user to Service user creators.
    JoinCreators { email: String },
    /// Make the user a super user (may add service users to any group).
    SuperUser { email: String },
    /// Create `count` service users owned by the user (who must be a Service user creator).
    SeedServiceUsers {
        email: String,
        prefix: String,
        count: usize,
        #[serde(default)]
        suspend: bool,
    },
    /// Replace the service user's tokens with `count` seeded ones.
    SeedTokens {
        service_user_id: String,
        count: usize,
    },
    /// List `groups` groups and `permissions` permissions on the service user's detail page.
    SeedMemberships {
        service_user_id: String,
        groups: usize,
        permissions: usize,
    },
    /// Public ids of the service user's tokens.
    Tokens { service_user_id: String },
    /// Whether the service user is `active` or `suspended`.
    Status { service_user_id: String },
    /// Whether the service user is in the lab Spectra readers group.
    InReaders { service_user_id: String },
}

/// Lab-only seed route for the Service users pages. Do not mount on a production host.
pub async fn service_user_lab(
    axum::extract::State(state): axum::extract::State<crate::boot::AppState>,
    axum::Extension(tokens): axum::Extension<Arc<LabTokens>>,
    axum::Json(body): axum::Json<ServiceUserLabRequest>,
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
        ServiceUserLabRequest::JoinCreators { email } => {
            let group = gauge::generated::PermissionGroup::get(gauge::service_users::SERVICE_USER_CREATORS_GROUP_ID, &system, valence::use_!(r"**Test:** Lab host **loads the Service user creators group** so a Playwright run can add a user to it, standing in for a super user doing that. Only the lab harness uses this."))
                .await
                .map_err(|e| fail(e.into()))?
                .ok_or_else(|| fail(anyhow::anyhow!("no Service user creators group")))?;
            gauge::super_user::seed_super_user_member_by_email(&system, &group, &email)
                .await
                .map_err(fail)?;
            serde_json::json!({ "ok": true })
        }
        ServiceUserLabRequest::SuperUser { email } => {
            let group = gauge::super_user::ensure_super_user_group(&system)
                .await
                .map_err(fail)?;
            gauge::super_user::seed_super_user_member_by_email(&system, &group, &email)
                .await
                .map_err(fail)?;
            serde_json::json!({ "ok": true })
        }
        ServiceUserLabRequest::SeedServiceUsers {
            email,
            prefix,
            count,
            suspend,
        } => {
            let user_id = user_id_for_email(&system, &email).await.map_err(fail)?;
            let v = as_user(user_id).map_err(fail)?;
            let mut ids = Vec::new();
            for i in 0..count {
                let handle = format!("{prefix}-{i}");
                let service_user = lepton_auth::service_users::create_service_user(
                    &v,
                    lepton_auth::service_users::NewServiceUser {
                        handle: &handle,
                        display_name: &format!("Seeded {i}"),
                        description: None,
                    },
                )
                .await
                .map_err(|e| fail(e.into()))?;
                let id = service_user
                    .id()
                    .map(|rid| {
                        valence::extract_id_from_record(rid)
                            .unwrap_or_else(|_| rid.id().to_string())
                    })
                    .unwrap_or_default();
                if suspend {
                    let bid =
                        ServiceUserId::parse(&id).map_err(|e| fail(anyhow::anyhow!("{e}")))?;
                    lepton_auth::service_users::suspend_service_user(&v, &bid)
                        .await
                        .map_err(|e| fail(e.into()))?;
                }
                ids.push(serde_json::json!({ "id": id, "handle": handle }));
            }
            serde_json::json!({ "ok": true, "service_users": ids })
        }
        ServiceUserLabRequest::SeedTokens {
            service_user_id,
            count,
        } => {
            let ids = tokens.seed(&service_user_id, count);
            serde_json::json!({ "ok": true, "tokens": ids })
        }
        ServiceUserLabRequest::Tokens { service_user_id } => {
            let ids: Vec<String> = tokens
                .list(&service_user_id)
                .map_err(|e| fail(anyhow::anyhow!("{e}")))?
                .into_iter()
                .map(|t| t.public_id)
                .collect();
            serde_json::json!({ "tokens": ids })
        }
        ServiceUserLabRequest::SeedMemberships {
            service_user_id,
            groups,
            permissions,
        } => {
            tokens.seed_memberships(&service_user_id, groups, permissions);
            serde_json::json!({ "ok": true })
        }
        ServiceUserLabRequest::Status { service_user_id } => {
            let row = lepton_identity::generated::ServiceUserAccount::get(&service_user_id, &system, valence::use_!(r"**Test:** Lab host **reads a service user's status** so a Playwright run can check that a refused pause left it active. Only the lab harness uses this."))
                .await
                .map_err(|e| fail(e.into()))?
                .ok_or_else(|| fail(anyhow::anyhow!("no service user")))?;
            let state = match row.status() {
                ServiceUserAccountStatus::Active => "active",
                ServiceUserAccountStatus::Suspended => "suspended",
            };
            serde_json::json!({ "state": state })
        }
        ServiceUserLabRequest::InReaders { service_user_id } => {
            let group = gauge::generated::PermissionGroup::get(LAB_READERS_GROUP_ID, &system, valence::use_!(r"**Test:** Lab host **loads the lab Spectra readers group** so a Playwright run can check whether a service user joined it. Only the lab harness uses this."))
                .await
                .map_err(|e| fail(e.into()))?
                .ok_or_else(|| fail(anyhow::anyhow!("no readers group")))?;
            let principal = valence::RecordId::new(
                "permission_service_user_principal",
                format!("svc:{service_user_id}"),
            );
            let members = group
                .get_members_record_ids(&system, valence::use_!(r"**Test:** Lab host **reads the lab Spectra readers member list** so a Playwright run can check whether a service user joined. Only the lab harness uses this."))
                .await
                .map_err(|e| fail(e.into()))?;
            serde_json::json!({ "member": members.contains(&principal) })
        }
    };
    Ok(axum::Json(out))
}
