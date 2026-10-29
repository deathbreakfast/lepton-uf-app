//! The host port that gives service users access and tokens.
//!
//! Service users, their handles and who may create them come from Lepton. What a service user may
//! *do* on a given site, and how it gets a token, belong to the host: one site
//! grants Spectra through a group, another has no Spectra at all. The host says
//! so by implementing [`ServiceUserAccessBackend`] and providing it per request with
//! [`provide_service_user_access_backend`].

use std::fmt;

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use std::sync::Arc;

#[cfg(feature = "ssr")]
use lepton_identity::service_users::ServiceUserId;
#[cfg(feature = "ssr")]
use leptos::prelude::{provide_context, use_context};
#[cfg(feature = "ssr")]
use valence::Valence;

#[cfg(feature = "ssr")]
use super::types::{ClientSetupStep, RevealOnce, TokenTtl};

/// How a choice gives the service user access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    /// The service user joins a Gauge group and holds what the group holds. Only an owner
    /// of the group or a super user can add it.
    GroupMembership {
        /// Gauge group id, for example `spectra_readers`.
        group_id: &'static str,
        /// Group name people recognise, for example "Spectra readers".
        group_name: &'static str,
    },
    /// The access lives on the token only. Anyone who owns the service user can give it.
    TokenOnly,
}

/// One thing a service user can be allowed to do on this site, in plain language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceUserAccessChoice {
    /// Stable key, for example `spectra-read`.
    pub key: &'static str,
    /// Checkbox label, for example "Read Spectra metrics and events".
    pub label: &'static str,
    /// Short phrase that completes "This token can …", for example
    /// "read Spectra metrics and events".
    pub summary: &'static str,
    /// One sentence saying what picking it does.
    pub help: &'static str,
    /// Group membership or token only.
    pub kind: AccessKind,
    /// Token grants this choice adds, for example `["spectra:read"]`.
    pub token_scopes: &'static [&'static str],
}

/// A group the service user is a member of, as its detail page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceUserGroupView {
    /// Gauge group id, for example `spectra_readers`.
    pub id: String,
    /// Group name, for example "Spectra readers".
    pub name: String,
    /// What the group is for. Empty when the group has no description.
    pub description: String,
}

/// A permission the service user holds through one of its groups.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceUserPermissionView {
    /// Permission name, for example `spectra.query`.
    pub name: String,
    /// What the permission allows. Empty when it has no description.
    pub description: String,
    /// Name of the group the permission comes through.
    pub via_group: String,
}

/// Why the host could not give access or a token.
///
/// `Display` starts with `reason_class=<class>` so the pages can explain it
/// without showing server text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceUserAccessError {
    /// No [`ServiceUserAccessBackend`] was provided on this request.
    NotConfigured,
    /// The caller isn't allowed to do this (not a signed-in user, or not the
    /// service user's owner).
    Forbidden,
    /// The service user doesn't exist or the caller can't see it.
    ServiceUserNotFound,
    /// The service user is paused, so it can't get tokens.
    ServiceUserSuspended,
    /// The service user already has the most active tokens allowed.
    TooManyTokens {
        /// The limit.
        limit: u32,
    },
    /// The token label is empty or too long.
    InvalidLabel,
    /// Nothing was picked for the token to do.
    NoAccessChosen,
    /// A key that isn't one of the host's choices.
    UnknownChoice,
    /// The caller may not add service users to this group.
    NotGrantable {
        /// Key of the refused choice.
        choice: &'static str,
    },
    /// Storage or lookup failure; `reason_class` names the cause.
    Store {
        /// Safe, low-cardinality cause.
        reason_class: &'static str,
    },
}

impl ServiceUserAccessError {
    /// Stable class for logs and the page's error text.
    #[must_use]
    pub const fn reason_class(&self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Forbidden => "forbidden",
            Self::ServiceUserNotFound => "not_found",
            Self::ServiceUserSuspended => "suspended",
            Self::TooManyTokens { .. } => "token_limit",
            Self::InvalidLabel => "label",
            Self::NoAccessChosen => "no_access",
            Self::UnknownChoice => "unknown_choice",
            Self::NotGrantable { .. } => "not_grantable",
            Self::Store { .. } => "store",
        }
    }
}

impl fmt::Display for ServiceUserAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class = self.reason_class();
        match self {
            Self::TooManyTokens { limit } => {
                write!(f, "reason_class={class}: at most {limit} active tokens")
            }
            Self::NotGrantable { choice } => {
                write!(f, "reason_class={class}: may not grant {choice}")
            }
            Self::Store { reason_class } => write!(f, "reason_class={class}: {reason_class}"),
            _ => write!(f, "reason_class={class}"),
        }
    }
}

impl std::error::Error for ServiceUserAccessError {}

/// A token the host just minted. The secret leaves the server once, in the
/// response to the person who asked for it.
#[cfg(feature = "ssr")]
#[derive(Debug)]
#[must_use = "the secret is only available here; show it or it is lost"]
pub struct MintedToken {
    /// Public token id, safe to show and log.
    pub public_id: String,
    /// The secret.
    pub secret: RevealOnce,
    /// When the token stops working.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// What a host implements so people can give their service users access and tokens.
///
/// Every method receives the **request** Valence, so the host's own privacy
/// rules and permission checks apply to the signed-in person. Nothing here may
/// switch to a system actor.
#[cfg(feature = "ssr")]
#[async_trait::async_trait]
pub trait ServiceUserAccessBackend: Send + Sync {
    /// The choices this site offers, in display order.
    fn choices(&self) -> &'static [ServiceUserAccessChoice];

    /// Whether the caller may add service users to `group_id`.
    ///
    /// # Errors
    ///
    /// [`ServiceUserAccessError::Store`] when the answer can't be looked up.
    async fn can_grant_group(
        &self,
        v: &Valence,
        group_id: &str,
    ) -> Result<bool, ServiceUserAccessError>;

    /// Add `service_user` to `group_id`.
    ///
    /// # Errors
    ///
    /// [`ServiceUserAccessError::NotGrantable`] or
    /// [`ServiceUserAccessError::ServiceUserNotFound`] when the caller may not,
    /// [`ServiceUserAccessError::Store`] otherwise.
    async fn grant_group(
        &self,
        v: &Valence,
        service_user: &ServiceUserId,
        group_id: &'static str,
        choice: &'static str,
    ) -> Result<(), ServiceUserAccessError>;

    /// Mint a token for `service_user` carrying `scopes`.
    ///
    /// # Errors
    ///
    /// [`ServiceUserAccessError::Forbidden`], [`ServiceUserAccessError::ServiceUserNotFound`],
    /// [`ServiceUserAccessError::ServiceUserSuspended`], [`ServiceUserAccessError::TooManyTokens`],
    /// [`ServiceUserAccessError::InvalidLabel`] or [`ServiceUserAccessError::Store`].
    async fn mint_token(
        &self,
        v: &Valence,
        service_user: &ServiceUserId,
        label: &str,
        ttl: TokenTtl,
        scopes: &[&'static str],
    ) -> Result<MintedToken, ServiceUserAccessError>;

    /// Groups `service_user` is a member of, for its detail page.
    ///
    /// Read them under `v` so the caller sees only what Gauge lets them see.
    /// The default lists none, and the page shows its empty state; a host that
    /// can't answer yet keeps the default.
    ///
    /// # Errors
    ///
    /// [`ServiceUserAccessError::ServiceUserNotFound`] or
    /// [`ServiceUserAccessError::Store`].
    async fn service_user_groups(
        &self,
        v: &Valence,
        service_user: &ServiceUserId,
    ) -> Result<Vec<ServiceUserGroupView>, ServiceUserAccessError> {
        let _ = (v, service_user);
        Ok(Vec::new())
    }

    /// Permissions `service_user` holds through its groups, for its detail page.
    ///
    /// A service user only holds what its groups hold, so list each permission
    /// with the group it comes through. Read them under `v`. The default lists
    /// none, and the page shows its empty state.
    ///
    /// # Errors
    ///
    /// [`ServiceUserAccessError::ServiceUserNotFound`] or
    /// [`ServiceUserAccessError::Store`].
    async fn service_user_permissions(
        &self,
        v: &Valence,
        service_user: &ServiceUserId,
    ) -> Result<Vec<ServiceUserPermissionView>, ServiceUserAccessError> {
        let _ = (v, service_user);
        Ok(Vec::new())
    }

    /// Commands that connect a tool to the service user, given what its token can do.
    /// They must refer to the token through an environment variable, never
    /// include the secret.
    fn client_setup(
        &self,
        handle: &str,
        chosen: &[ServiceUserAccessChoice],
    ) -> Vec<ClientSetupStep>;
}

/// Provide the host's [`ServiceUserAccessBackend`] for this request.
///
/// Call it where the host provides its other per-request context, next to the
/// permission backend. Without it the Service users pages still list and create service users,
/// and say that this site can't give service users tokens.
#[cfg(feature = "ssr")]
pub fn provide_service_user_access_backend(backend: Arc<dyn ServiceUserAccessBackend>) {
    provide_context(backend);
}

/// The backend provided for this request, if any.
#[cfg(feature = "ssr")]
#[must_use]
pub fn use_service_user_access_backend() -> Option<Arc<dyn ServiceUserAccessBackend>> {
    use_context::<Arc<dyn ServiceUserAccessBackend>>()
}

/// Look up choices by key, keeping the host's order.
///
/// # Errors
///
/// [`ServiceUserAccessError::UnknownChoice`] for a key the host doesn't offer.
pub fn resolve_choices(
    all: &'static [ServiceUserAccessChoice],
    keys: &[String],
) -> Result<Vec<ServiceUserAccessChoice>, ServiceUserAccessError> {
    if keys.iter().any(|k| !all.iter().any(|c| c.key == k)) {
        return Err(ServiceUserAccessError::UnknownChoice);
    }
    Ok(all
        .iter()
        .filter(|c| keys.iter().any(|k| k == c.key))
        .copied()
        .collect())
}

/// Token grants for a set of choices, without repeats.
#[must_use]
pub fn token_scopes(chosen: &[ServiceUserAccessChoice]) -> Vec<&'static str> {
    let mut scopes: Vec<&'static str> = Vec::new();
    for scope in chosen.iter().flat_map(|c| c.token_scopes.iter().copied()) {
        if !scopes.contains(&scope) {
            scopes.push(scope);
        }
    }
    scopes
}

/// "read Spectra metrics and events and read Magnon projects", for the
/// "This token can …" line.
#[must_use]
pub fn summarize(chosen: &[ServiceUserAccessChoice]) -> String {
    let parts: Vec<&str> = chosen.iter().map(|c| c.summary).collect();
    match parts.as_slice() {
        [] => String::new(),
        [one] => (*one).to_string(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    static CHOICES: &[ServiceUserAccessChoice] = &[
        ServiceUserAccessChoice {
            key: "spectra-read",
            label: "Read Spectra metrics and events",
            summary: "read Spectra metrics and events",
            help: "Joins Spectra readers.",
            kind: AccessKind::GroupMembership {
                group_id: "spectra_readers",
                group_name: "Spectra readers",
            },
            token_scopes: &["spectra:read"],
        },
        ServiceUserAccessChoice {
            key: "magnon-read",
            label: "Read Magnon",
            summary: "read Magnon projects and tasks",
            help: "Token can read Magnon.",
            kind: AccessKind::TokenOnly,
            token_scopes: &["magnon:read"],
        },
        ServiceUserAccessChoice {
            key: "magnon-write",
            label: "Change Magnon",
            summary: "change Magnon projects and tasks",
            help: "Token can change Magnon.",
            kind: AccessKind::TokenOnly,
            token_scopes: &["magnon:read", "magnon:write"],
        },
    ];

    #[test]
    fn resolve_keeps_host_order_and_splits_kinds() {
        let chosen =
            resolve_choices(CHOICES, &["magnon-read".into(), "spectra-read".into()]).expect("ok");
        assert_eq!(
            chosen.iter().map(|c| c.key).collect::<Vec<_>>(),
            ["spectra-read", "magnon-read"]
        );
        assert!(matches!(chosen[0].kind, AccessKind::GroupMembership { .. }));
        assert_eq!(chosen[1].kind, AccessKind::TokenOnly);
    }

    #[test]
    fn resolve_unknown_key_errors() {
        assert_eq!(
            resolve_choices(CHOICES, &["spectra-read".into(), "root".into()]),
            Err(ServiceUserAccessError::UnknownChoice)
        );
    }

    #[test]
    fn token_scopes_dedupe() {
        let chosen =
            resolve_choices(CHOICES, &["magnon-read".into(), "magnon-write".into()]).expect("ok");
        assert_eq!(token_scopes(&chosen), ["magnon:read", "magnon:write"]);
        assert_eq!(token_scopes(&[]), Vec::<&str>::new());
    }

    #[test]
    fn summarize_joins_in_plain_language() {
        assert_eq!(summarize(&[]), "");
        assert_eq!(summarize(&CHOICES[..1]), "read Spectra metrics and events");
        assert_eq!(
            summarize(&CHOICES[1..]),
            "read Magnon projects and tasks and change Magnon projects and tasks"
        );
        assert_eq!(
            summarize(CHOICES),
            "read Spectra metrics and events, read Magnon projects and tasks, and change Magnon projects and tasks"
        );
    }

    #[test]
    fn error_display_leads_with_reason_class() {
        assert_eq!(
            ServiceUserAccessError::TooManyTokens { limit: 5 }.to_string(),
            "reason_class=token_limit: at most 5 active tokens"
        );
        assert_eq!(
            ServiceUserAccessError::NotConfigured.to_string(),
            "reason_class=not_configured"
        );
        assert_eq!(
            ServiceUserAccessError::Store {
                reason_class: "token_store"
            }
            .to_string(),
            "reason_class=store: token_store"
        );
    }
}
