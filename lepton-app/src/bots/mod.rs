//! Bots pages: create a bot, choose what it can do, and give it a token.
//!
//! A bot is an account that tools like Claude Code use to call a site for
//! someone, with only the access its owner gives it. These pages let a
//! signed-in person list their bots at `/user/bots`, create one in three steps
//! at `/user/bots/new` (name it, choose access, get a token), and get another
//! token at `/user/bots/{bot_id}/token`. The token is shown once and never
//! again.
//!
//! Who may create bots comes from Lepton's
//! [`BotGate`](lepton_identity::bots::BotGate), which the host installs at boot
//! (Gauge's gate grants it to super users and the "Bot creators" group). What a
//! bot may do, and how it gets a token, depend on the site, so the host
//! describes both through a [`BotAccessBackend`]. The detail page, pausing and
//! deleting are not part of these pages yet.
//!
//! Host setup and examples live on the crate root:
//! [Install a bot access backend](crate#install-a-bot-access-backend).

mod create;
mod error;
mod list_page;
mod port;
mod server;
mod step_up;
mod token;
mod types;

pub use create::CreateBotPage;
pub use error::friendly_error;
pub use list_page::BotsPage;
#[cfg(feature = "ssr")]
pub use port::{provide_bot_access_backend, use_bot_access_backend, BotAccessBackend, MintedToken};
pub use port::{
    resolve_choices, summarize, token_scopes, AccessKind, BotAccessChoice, BotAccessError,
};
pub use server::{
    bot_access_choices, check_bot_handle, create_bot_profile, get_bot_row, get_bots_overview,
    grant_bot_access, mint_bot_token,
};
pub use token::{BotTokenPage, TokenForm, TokenReveal};
pub use types::{
    AccessChoiceView, BotRow, BotState, BotsOverview, ClientSetupStep, HandleCheck, HandleProblem,
    MintedTokenView, RevealOnce, TokenTtl,
};
