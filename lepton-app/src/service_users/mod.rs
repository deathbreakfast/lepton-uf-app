//! Service users pages: create a service user, choose what it can do, and give it a token.
//!
//! A service user is an account that a tool, such as an MCP client, a CI job, a webhook
//! consumer, or an agent, uses to call a site for someone, with only the access its owner
//! gives it. These pages let a
//! signed-in person list their service users at `/user/service-users`, create one in three steps
//! at `/user/service-users/new` (name it, choose access, get a token), and get another
//! token at `/user/service-users/{service_user_id}/token`. The token is shown once and never
//! again.
//!
//! Who may create service users comes from Lepton's
//! [`ServiceUserGate`](lepton_identity::service_users::ServiceUserGate), which the host
//! installs at boot (Gauge's gate grants it to super users and the "Service user creators"
//! group). What a service user may do, and how it gets a token, depend on the site, so the
//! host describes both through a [`ServiceUserAccessBackend`]. The detail page, pausing and
//! deleting are not part of these pages yet.
//!
//! Host setup and examples live on the crate root:
//! [Install a service user access backend](crate#install-a-service-user-access-backend).

mod create;
mod error;
mod list_page;
mod port;
mod server;
mod step_up;
mod token;
mod types;

pub use create::CreateServiceUserPage;
pub use error::friendly_error;
pub use list_page::{ServiceUserBadge, ServiceUsersPage};
#[cfg(feature = "ssr")]
pub use port::{
    provide_service_user_access_backend, use_service_user_access_backend, MintedToken,
    ServiceUserAccessBackend,
};
pub use port::{
    resolve_choices, summarize, token_scopes, AccessKind, ServiceUserAccessChoice,
    ServiceUserAccessError,
};
pub use server::{
    check_service_user_handle, create_service_user_profile, get_service_user_row,
    get_service_users_overview, grant_service_user_access, mint_service_user_token,
    service_user_access_choices,
};
pub use token::{ServiceUserTokenPage, TokenForm, TokenReveal};
pub use types::{
    AccessChoiceView, ClientSetupStep, HandleCheck, HandleProblem, MintedTokenView, RevealOnce,
    ServiceUserRow, ServiceUserState, ServiceUsersOverview, TokenTtl,
};
