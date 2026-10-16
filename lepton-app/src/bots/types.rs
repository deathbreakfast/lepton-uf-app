//! Values the bot pages and their server functions exchange.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How long a new bot token stays valid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenTtl {
    /// 30 days. The default.
    #[default]
    Days30,
    /// 90 days, the longest a bot token may live.
    Days90,
}

impl TokenTtl {
    /// Length in days.
    #[must_use]
    pub const fn days(self) -> u16 {
        match self {
            Self::Days30 => 30,
            Self::Days90 => 90,
        }
    }

    /// Parse the value a radio button submits (`"30"` or `"90"`).
    #[must_use]
    pub fn from_days_str(raw: &str) -> Option<Self> {
        match raw {
            "30" => Some(Self::Days30),
            "90" => Some(Self::Days90),
            _ => None,
        }
    }
}

/// Whether a bot may act.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotState {
    /// Its tokens work.
    Active,
    /// Its owner paused it; its tokens are refused.
    Paused,
}

/// One bot as the list and the token page show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotRow {
    /// Bot id (a UUID).
    pub id: String,
    /// Handle without the `@`, for example `spectra-bot`.
    pub handle: String,
    /// Name shown next to the bot's actions.
    pub display_name: String,
    /// What the bot is for.
    pub description: Option<String>,
    /// Active or paused.
    pub state: BotState,
    /// Creation date, `YYYY-MM-DD` in UTC.
    pub created_on: String,
}

/// Everything the Bots page needs in one load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotsOverview {
    /// The viewer's bots, oldest first.
    pub bots: Vec<BotRow>,
    /// Most bots one person may own.
    pub limit: u32,
    /// Whether the viewer may create bots at all.
    pub can_create: bool,
    /// Whether this site can give bots tokens.
    pub tokens_configured: bool,
    /// The viewer's email, shown as the owner while creating a bot.
    pub owner_email: String,
}

impl BotsOverview {
    /// True when the viewer owns the most bots allowed.
    #[must_use]
    pub fn at_limit(&self) -> bool {
        u32::try_from(self.bots.len()).map_or(true, |n| n >= self.limit)
    }
}

/// Why a handle can't be used, as the form explains it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandleProblem {
    /// Shorter than 3 characters.
    TooShort,
    /// Longer than 32 characters.
    TooLong,
    /// Has a character outside `a-z`, `0-9` and single dashes.
    InvalidChar,
    /// Doesn't end in `bot`.
    MissingBotSuffix,
    /// Uses a word kept for the platform, such as `admin` or `support`.
    Reserved,
}

impl HandleProblem {
    /// Map the `reason()` text of a Lepton handle error.
    #[must_use]
    pub fn from_reason(reason: &str) -> Self {
        match reason {
            "too_short" => Self::TooShort,
            "too_long" => Self::TooLong,
            "missing_bot_suffix" => Self::MissingBotSuffix,
            "reserved" => Self::Reserved,
            _ => Self::InvalidChar,
        }
    }

    /// Sentence shown under the handle field.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::TooShort | Self::TooLong | Self::InvalidChar => {
                "Use 3 to 32 lowercase letters, numbers or single dashes."
            }
            Self::MissingBotSuffix => "Handles end in bot, for example spectra-bot.",
            Self::Reserved => "That handle is kept for the platform. Try another.",
        }
    }
}

/// Answer to "can I use this handle?".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandleCheck {
    /// Nobody uses it.
    Free {
        /// The handle as it will be saved.
        handle: String,
    },
    /// Another bot uses it.
    Taken {
        /// The handle as typed, normalized.
        handle: String,
        /// A free alternative, when one was found.
        suggestion: Option<String>,
    },
    /// It breaks a handle rule.
    Invalid {
        /// Which rule.
        problem: HandleProblem,
    },
}

impl HandleCheck {
    /// Classify the result of asking Lepton for a free handle close to `typed`.
    ///
    /// Lepton answers with `typed` itself when it is free, or with the nearest
    /// free variant when it isn't.
    #[must_use]
    pub fn from_suggestion(typed: &str, suggestion: &str) -> Self {
        let typed = typed.trim().to_ascii_lowercase();
        if typed == suggestion {
            Self::Free { handle: typed }
        } else {
            Self::Taken {
                handle: typed,
                suggestion: Some(suggestion.to_string()),
            }
        }
    }
}

/// One access choice as the create flow shows it to this viewer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessChoiceView {
    /// Stable key sent back when the choice is picked.
    pub key: String,
    /// Checkbox label, for example "Read Spectra metrics and events".
    pub label: String,
    /// One sentence under the label saying what picking it does.
    pub help: String,
    /// Group the bot joins, when the choice works through a group.
    pub group_name: Option<String>,
    /// Whether this viewer may give the choice. False means only a group owner
    /// or a super user can.
    pub grantable: bool,
}

/// A token secret that is shown once.
///
/// `Debug` never prints the value, so the secret can't land in a log through a
/// stray `{:?}`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RevealOnce(String);

impl RevealOnce {
    /// Wrap a freshly minted secret.
    #[must_use]
    pub const fn new(secret: String) -> Self {
        Self(secret)
    }

    /// The secret text, for the reveal and copy button only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RevealOnce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RevealOnce(<redacted>)")
    }
}

/// A command block that connects a tool to the bot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSetupStep {
    /// What the block does, for example "Add Spectra to Claude Code".
    pub title: String,
    /// Commands to paste. Uses an environment variable, never the secret.
    pub code: String,
}

/// A new token, returned once by `mint_bot_token`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintedTokenView {
    /// Public token id, safe to show and log.
    pub public_id: String,
    /// The secret. Shown once, never stored by the page.
    pub secret: RevealOnce,
    /// Expiry date, `YYYY-MM-DD` in UTC.
    pub expires_on: String,
    /// What the token can do, for example "read Spectra metrics and events".
    pub can_do: String,
    /// Commands for connecting tools.
    pub setup: Vec<ClientSetupStep>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn handle_check_free_when_suggestion_equals_input() {
        assert_eq!(
            HandleCheck::from_suggestion(" Spectra-Bot ", "spectra-bot"),
            HandleCheck::Free {
                handle: "spectra-bot".into()
            }
        );
    }

    #[test]
    fn handle_check_taken_carries_suggestion() {
        assert_eq!(
            HandleCheck::from_suggestion("spectra-bot", "spectra-2-bot"),
            HandleCheck::Taken {
                handle: "spectra-bot".into(),
                suggestion: Some("spectra-2-bot".into())
            }
        );
    }

    #[test]
    fn handle_problem_maps_every_reason() {
        assert_eq!(
            HandleProblem::from_reason("too_short"),
            HandleProblem::TooShort
        );
        assert_eq!(
            HandleProblem::from_reason("too_long"),
            HandleProblem::TooLong
        );
        assert_eq!(
            HandleProblem::from_reason("missing_bot_suffix"),
            HandleProblem::MissingBotSuffix
        );
        assert_eq!(
            HandleProblem::from_reason("reserved"),
            HandleProblem::Reserved
        );
        assert_eq!(
            HandleProblem::from_reason("invalid_char"),
            HandleProblem::InvalidChar
        );
        assert!(HandleProblem::MissingBotSuffix.message().contains("bot"));
    }

    #[test]
    fn reveal_once_debug_redacts() {
        let secret = RevealOnce::new("fbtk_owner.abcdef".into());
        let printed = format!("{secret:?}");
        assert!(!printed.contains("abcdef"), "{printed}");
        assert_eq!(secret.expose(), "fbtk_owner.abcdef");
    }

    #[test]
    fn minted_token_view_debug_hides_secret_but_serializes_it() {
        let view = MintedTokenView {
            public_id: "tok_1".into(),
            secret: RevealOnce::new("fbtk_owner.abcdef".into()),
            expires_on: "2026-11-14".into(),
            can_do: "read Spectra".into(),
            setup: vec![],
        };
        assert!(!format!("{view:?}").contains("abcdef"));
        let json = serde_json::to_value(&view).expect("serialize");
        assert_eq!(json["secret"], "fbtk_owner.abcdef");
    }

    #[test]
    fn token_ttl_days() {
        assert_eq!(TokenTtl::Days30.days(), 30);
        assert_eq!(TokenTtl::Days90.days(), 90);
        assert_eq!(TokenTtl::from_days_str("90"), Some(TokenTtl::Days90));
        assert_eq!(TokenTtl::from_days_str("365"), None);
    }

    #[test]
    fn overview_at_limit() {
        let mut o = BotsOverview {
            bots: vec![],
            limit: 1,
            can_create: true,
            tokens_configured: true,
            owner_email: "a@example.test".into(),
        };
        assert!(!o.at_limit());
        o.bots.push(BotRow {
            id: "x".into(),
            handle: "x-bot".into(),
            display_name: "X".into(),
            description: None,
            state: BotState::Active,
            created_on: "2026-10-15".into(),
        });
        assert!(o.at_limit());
    }
}
