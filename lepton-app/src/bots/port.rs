//! The host port that gives bots access and tokens.
//!
//! Bots, their handles and who may create them come from Lepton. What a bot may
//! *do* on a given site, and how it gets a token, belong to the host: one site
//! grants Spectra through a group, another has no Spectra at all. The host says
//! so by implementing [`BotAccessBackend`] and providing it per request with
//! [`provide_bot_access_backend`].

use std::fmt;

#[cfg(feature = "ssr")]
use std::sync::Arc;

#[cfg(feature = "ssr")]
use lepton_identity::bots::BotId;
#[cfg(feature = "ssr")]
use leptos::prelude::{provide_context, use_context};
#[cfg(feature = "ssr")]
use valence::Valence;

#[cfg(feature = "ssr")]
use super::types::{ClientSetupStep, RevealOnce, TokenTtl};

/// How a choice gives the bot access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    /// The bot joins a Gauge group and holds what the group holds. Only an owner
    /// of the group or a super user can add it.
    GroupMembership {
        /// Gauge group id, for example `spectra_readers`.
        group_id: &'static str,
        /// Group name people recognise, for example "Spectra readers".
        group_name: &'static str,
    },
    /// The access lives on the token only. Anyone who owns the bot can give it.
    TokenOnly,
}

/// One thing a bot can be allowed to do on this site, in plain language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotAccessChoice {
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

/// Why the host could not give access or a token.
///
/// `Display` starts with `reason_class=<class>` so the pages can explain it
/// without showing server text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotAccessError {
    /// No [`BotAccessBackend`] was provided on this request.
    NotConfigured,
    /// The caller isn't allowed to do this (not a signed-in user, or not the
    /// bot's owner).
    Forbidden,
    /// The bot doesn't exist or the caller can't see it.
    BotNotFound,
    /// The bot is paused, so it can't get tokens.
    BotSuspended,
    /// The bot already has the most active tokens allowed.
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
    /// The caller may not add bots to this group.
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

impl BotAccessError {
    /// Stable class for logs and the page's error text.
    #[must_use]
    pub const fn reason_class(&self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Forbidden => "forbidden",
            Self::BotNotFound => "not_found",
            Self::BotSuspended => "suspended",
            Self::TooManyTokens { .. } => "token_limit",
            Self::InvalidLabel => "label",
            Self::NoAccessChosen => "no_access",
            Self::UnknownChoice => "unknown_choice",
            Self::NotGrantable { .. } => "not_grantable",
            Self::Store { .. } => "store",
        }
    }
}

impl fmt::Display for BotAccessError {
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

impl std::error::Error for BotAccessError {}

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

/// What a host implements so people can give their bots access and tokens.
///
/// Every method receives the **request** Valence, so the host's own privacy
/// rules and permission checks apply to the signed-in person. Nothing here may
/// switch to a system actor.
#[cfg(feature = "ssr")]
#[async_trait::async_trait]
pub trait BotAccessBackend: Send + Sync {
    /// The choices this site offers, in display order.
    fn choices(&self) -> &'static [BotAccessChoice];

    /// Whether the caller may add bots to `group_id`.
    ///
    /// # Errors
    ///
    /// [`BotAccessError::Store`] when the answer can't be looked up.
    async fn can_grant_group(&self, v: &Valence, group_id: &str) -> Result<bool, BotAccessError>;

    /// Add `bot` to `group_id`.
    ///
    /// # Errors
    ///
    /// [`BotAccessError::NotGrantable`] or [`BotAccessError::BotNotFound`] when
    /// the caller may not, [`BotAccessError::Store`] otherwise.
    async fn grant_group(
        &self,
        v: &Valence,
        bot: &BotId,
        group_id: &'static str,
        choice: &'static str,
    ) -> Result<(), BotAccessError>;

    /// Mint a token for `bot` carrying `scopes`.
    ///
    /// # Errors
    ///
    /// [`BotAccessError::Forbidden`], [`BotAccessError::BotNotFound`],
    /// [`BotAccessError::BotSuspended`], [`BotAccessError::TooManyTokens`],
    /// [`BotAccessError::InvalidLabel`] or [`BotAccessError::Store`].
    async fn mint_token(
        &self,
        v: &Valence,
        bot: &BotId,
        label: &str,
        ttl: TokenTtl,
        scopes: &[&'static str],
    ) -> Result<MintedToken, BotAccessError>;

    /// Commands that connect a tool to the bot, given what its token can do.
    /// They must refer to the token through an environment variable, never
    /// include the secret.
    fn client_setup(&self, handle: &str, chosen: &[BotAccessChoice]) -> Vec<ClientSetupStep>;
}

/// Provide the host's [`BotAccessBackend`] for this request.
///
/// Call it where the host provides its other per-request context, next to the
/// permission backend. Without it the Bots pages still list and create bots,
/// and say that this site can't give bots tokens.
#[cfg(feature = "ssr")]
pub fn provide_bot_access_backend(backend: Arc<dyn BotAccessBackend>) {
    provide_context(backend);
}

/// The backend provided for this request, if any.
#[cfg(feature = "ssr")]
#[must_use]
pub fn use_bot_access_backend() -> Option<Arc<dyn BotAccessBackend>> {
    use_context::<Arc<dyn BotAccessBackend>>()
}

/// Look up choices by key, keeping the host's order.
///
/// # Errors
///
/// [`BotAccessError::UnknownChoice`] for a key the host doesn't offer.
pub fn resolve_choices(
    all: &'static [BotAccessChoice],
    keys: &[String],
) -> Result<Vec<BotAccessChoice>, BotAccessError> {
    if keys.iter().any(|k| !all.iter().any(|c| c.key == k)) {
        return Err(BotAccessError::UnknownChoice);
    }
    Ok(all
        .iter()
        .filter(|c| keys.iter().any(|k| k == c.key))
        .copied()
        .collect())
}

/// Token grants for a set of choices, without repeats.
#[must_use]
pub fn token_scopes(chosen: &[BotAccessChoice]) -> Vec<&'static str> {
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
pub fn summarize(chosen: &[BotAccessChoice]) -> String {
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

    static CHOICES: &[BotAccessChoice] = &[
        BotAccessChoice {
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
        BotAccessChoice {
            key: "magnon-read",
            label: "Read Magnon",
            summary: "read Magnon projects and tasks",
            help: "Token can read Magnon.",
            kind: AccessKind::TokenOnly,
            token_scopes: &["magnon:read"],
        },
        BotAccessChoice {
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
            Err(BotAccessError::UnknownChoice)
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
            BotAccessError::TooManyTokens { limit: 5 }.to_string(),
            "reason_class=token_limit: at most 5 active tokens"
        );
        assert_eq!(
            BotAccessError::NotConfigured.to_string(),
            "reason_class=not_configured"
        );
        assert_eq!(
            BotAccessError::Store {
                reason_class: "token_store"
            }
            .to_string(),
            "reason_class=store: token_store"
        );
    }
}
