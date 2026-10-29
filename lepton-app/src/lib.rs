#![recursion_limit = "256"]
#![allow(missing_docs)] // uf_app! / routes macros emit undocumented associated items
#![allow(clippy::too_long_first_doc_paragraph)]
#![deny(clippy::missing_errors_doc)]
//! `/user` settings routes and pages for Unified Field hosts.
//!
//! Registers profile, appearance, account settings, and confirm-account under `/user`
//! via [`UserAppRoutes`]. Account mutations call **lepton-auth** server actions; Orbital
//! supplies layout chrome only. Mount with `lepton_shell::AppBarUserMenu` and
//! `lepton_auth_app::LeptonAuthRoutes`. Workspace overview: crate README at the
//! `lepton-uf-app` root.
//!
//! ## Features
//!
//! - **User settings routes** — Nested `/user` routes for profile, appearance, account
//!   settings, and confirm-account inside the host `Router`. Mount once at host boot
//!   alongside auth routes. [Get started](#mount-user-routes)
//! - **Profile page** — Offers display name and profile photo editing backed by
//!   [`get_my_profile`] and [`update_my_profile`]. [Guide](#profile-settings)
//! - **Appearance page** — Provides color mode, brand source, and live theme preview
//!   with persisted preferences for signed-in users. [Guide](#appearance-preferences)
//! - **Account settings page** — Provides email, password, TOTP, OAuth links, devices,
//!   and owner wipe through **lepton-auth** server actions. [Guide](#account-settings)
//! - **Confirm account page** — Provides a guided soft-confirm funnel under the user
//!   settings shell for signed-in users. [Guide](#confirm-account-funnel)
//! - **Service user management pages** — Let a signed-in person create service users, choose what
//!   each one can do, and give it a token that is shown once. The host decides
//!   the access choices and issues tokens. [Get started](#install-a-service-user-access-backend)
//! - **WASM route prefetch** — Warms lazy-loaded `/user` WASM chunks before first
//!   navigation on split builds. [Guide](#lazy-route-prefetch)
//!
//! ## Mount user routes
//!
//! [`UserAppRoutes`] registers the `/user` tree under your host `Router` / `Routes`.
//! Call it once at host boot (before routed pages mount) alongside
//! `lepton_auth_app::LeptonAuthRoutes` and wire `lepton_shell::AppBarUserMenu` through
//! `uf_integrations::provide_shell_auth_menu`.
//!
//! **Prerequisites:** `ssr` and/or `hydrate` on this crate; `uf_product` session context
//! from `provide_auth_context` at boot; matching auth routes for sign-in flows.
//!
//! ```rust,ignore
//! use lepton_app::UserAppRoutes;
//! use lepton_auth_app::LeptonAuthRoutes;
//! use lepton_shell::AppBarUserMenu;
//! use leptos::prelude::*;
//! use leptos_router::components::{Router, Routes};
//! use uf_integrations::provide_shell_auth_menu;
//!
//! // Once at host boot, before routed pages mount:
//! provide_shell_auth_menu(|| view! { <AppBarUserMenu /> });
//!
//! view! {
//!     <Router>
//!         <Routes fallback=|| view! { /* host 404 */ }>
//!             <UserAppRoutes />
//!             <LeptonAuthRoutes />
//!         </Routes>
//!     </Router>
//! }
//! ```
//!
//! On success the host serves `/user/profile`, `/user/appearance`, `/user/account-settings`,
//! and `/user/confirm-account` under [`UserLayout`]. `uf_app!` registers inventory id
//! `lepton-app` at `/user/account-settings`.
//!
//! ## Profile settings
//!
//! [`ProfilePage`] loads the signed-in user's [`ProfileData`] through [`get_my_profile`]
//! and persists display-name edits via [`update_my_profile`]. Photo upload uses Valence
//! file storage; anonymous visitors redirect to sign-in.
//!
//! **Prerequisites:** authenticated session; SSR graph with **lepton-auth** profile actions.
//!
//! ```rust,ignore
//! use lepton_app::{get_my_profile, update_my_profile, ProfilePage};
//! use leptos::prelude::*;
//!
//! #[component]
//! fn UserProfile() -> impl IntoView {
//!     view! { <ProfilePage /> }
//! }
//!
//! // Server fns back the page:
//! async fn load_and_save() -> Result<(), ServerFnError> {
//!     let profile = get_my_profile().await?;
//!     update_my_profile(profile.display_name).await?;
//!     assert!("profile-container".len() > 0);
//!     Ok(())
//! }
//! ```
//!
//! Failed loads surface a MessageBar; validation errors from [`validate_display_name`]
//! return stable `reason_class=` strings at the Leptos boundary.
//!
//! ## Appearance preferences
//!
//! [`AppearancePage`] provides color mode and brand controls for signed-in users. It
//! uses `get_my_appearance` to restore saved choices, enables live theme preview, and
//! calls `save_my_appearance` from **uf-product** to persist edits. Anonymous visitors
//! redirect to sign-in.
//!
//! **Prerequisites:** authenticated session; **uf-product** appearance services on the SSR
//! graph.
//!
//! ```rust,ignore
//! use lepton_app::AppearancePage;
//! use uf_product::services::save_my_appearance;
//! use leptos::prelude::*;
//!
//! #[component]
//! fn UserAppearance() -> impl IntoView {
//!     view! { <AppearancePage /> }
//! }
//!
//! async fn persist_light_mode() -> Result<(), ServerFnError> {
//!     save_my_appearance("light".into(), "product".into(), None).await?;
//!     assert!("appearance-page".len() > 0);
//!     Ok(())
//! }
//! ```
//!
//! Save failures show an info MessageBar with the server error; the draft preview reverts
//! only after a failed round-trip.
//!
//! ## Account settings
//!
//! [`AccountSettingsPage`] composes email, password, [`TotpSettingsSection`], OAuth links,
//! devices, and owner wipe from **lepton-auth** server actions. The page loads an overview
//! resource and mounts each section when data is ready.
//!
//! **Prerequisites:** authenticated session; enable `lepton-auth/totp` on the SSR graph for
//! authenticator UI (included in this crate's `ssr` feature).
//!
//! ```rust,ignore
//! use lepton_app::{AccountSettingsPage, TotpSettingsSection};
//! use leptos::prelude::*;
//!
//! #[component]
//! fn UserAccountSettings() -> impl IntoView {
//!     view! {
//!         <AccountSettingsPage />
//!     }
//! }
//!
//! // TotpSettingsSection mounts inside AccountSettingsPage when overview loads.
//! // Success renders data-testid="account-settings-container".
//! assert!("account-settings".len() > 0);
//! ```
//!
//! Sensitive mutations (wipe, TOTP disable, OAuth unlink) expect hosts to call
//! `StepUpController::request` before proceeding — see **lepton-shell** step-up guide.
//! OAuth link CTAs need `oauth-google` / `oauth-github` on this crate.
//!
//! ## Confirm account funnel
//!
//! [`ConfirmAccountPage`] wraps `lepton_auth_ui::ConfirmAccountPage` as
//! `ConfirmAccountFunnel` under the user settings shell. Use it when a signed-in user must
//! complete soft account confirmation before accessing product features.
//!
//! **Prerequisites:** authenticated session; **lepton-auth-ui** confirm funnel on the SSR
//! graph.
//!
//! ```rust,ignore
//! use lepton_app::ConfirmAccountPage;
//! use lepton_auth_ui::ConfirmAccountPage as ConfirmAccountFunnel;
//! use leptos::prelude::*;
//!
//! #[component]
//! fn UserConfirmAccount() -> impl IntoView {
//!     view! { <ConfirmAccountPage /> }
//! }
//!
//! // ConfirmAccountFunnel renders inside ConfirmAccountPage at /user/confirm-account.
//! assert!("/user/confirm-account".contains("confirm-account"));
//! ```
//!
//! Anonymous visitors redirect to sign-in; completed confirmation navigates per the funnel's
//! configured outcome path.
//!
//! ## Lazy route prefetch
//!
//! [`prefetch_family`] warms the shared WASM chunk for `/user/*` lazy routes by calling
//! `ProfileRoute::preload`. Hosts on `cargo leptos --split` builds should call it from
//! WASM bootstrap before the first `/user` navigation.
//!
//! **Prerequisites:** `hydrate` feature and lazy route types from this crate.
//!
//! ```rust,ignore
//! use lepton_app::{prefetch_family, ProfileRoute};
//!
//! async fn warm_user_settings_chunks() {
//!     prefetch_family().await;
//!     ProfileRoute::preload().await;
//!     assert!(std::mem::size_of::<ProfileRoute>() > 0);
//! }
//! ```
//!
//! After prefetch, navigation to `/user/profile` avoids a visible chunk-load stall. Omit
//! prefetch on SSR-only hosts that do not ship WASM splits.
//!
//! ## Install a service user access backend
//!
//! Provide a [`service_users::ServiceUserAccessBackend`] on every request, next to the permission
//! backend, so the Service users pages can offer access choices and mint tokens. Do it
//! once per host, in the closure that provides per-request context to Leptos
//! server functions. Without a backend the pages still list and create service users
//! and say that the site can't give service users tokens yet.
//!
//! **Prerequisites:** `ssr` on this crate; [`UserAppRoutes`]
//! mounted; a Lepton service user gate installed at boot; a token service that can issue
//! a token owned by a service user.
//!
//! ```rust,ignore
//! use std::sync::{Arc, Mutex};
//!
//! use lepton_app::service_users::{
//!     provide_service_user_access_backend, AccessKind, ServiceUserAccessBackend, ServiceUserAccessChoice,
//!     ServiceUserAccessError, ClientSetupStep, MintedToken, RevealOnce, TokenTtl,
//! };
//! use lepton_identity::service_users::ServiceUserId;
//! use valence::Valence;
//!
//! static CHOICES: &[ServiceUserAccessChoice] = &[ServiceUserAccessChoice {
//!     key: "metrics-read",
//!     label: "Read metrics",
//!     summary: "read metrics",
//!     help: "Joins Metric readers. Its token can query metrics.",
//!     kind: AccessKind::GroupMembership { group_id: "metric_readers", group_name: "Metric readers" },
//!     token_scopes: &["metrics:read"],
//! }];
//!
//! /// Hands out tokens from memory and keeps group grants in Gauge.
//! #[derive(Default)]
//! struct SiteServiceUsers {
//!     issued: Mutex<Vec<(String, Vec<&'static str>)>>,
//! }
//!
//! #[async_trait::async_trait]
//! impl ServiceUserAccessBackend for SiteServiceUsers {
//!     fn choices(&self) -> &'static [ServiceUserAccessChoice] {
//!         CHOICES
//!     }
//!
//!     async fn can_grant_group(&self, v: &Valence, group_id: &str) -> Result<bool, ServiceUserAccessError> {
//!         gauge::service::can_add_group_member_service_user(group_id, v)
//!             .await
//!             .map_err(|_| ServiceUserAccessError::Store { reason_class: "gauge_lookup" })
//!     }
//!
//!     async fn grant_group(
//!         &self,
//!         v: &Valence,
//!         service_user: &ServiceUserId,
//!         group_id: &'static str,
//!         choice: &'static str,
//!     ) -> Result<(), ServiceUserAccessError> {
//!         gauge::service::add_group_member_service_user(group_id, service_user, v)
//!             .await
//!             .map_err(|_| ServiceUserAccessError::NotGrantable { choice })
//!     }
//!
//!     async fn mint_token(
//!         &self,
//!         _v: &Valence,
//!         service_user: &ServiceUserId,
//!         _label: &str,
//!         ttl: TokenTtl,
//!         scopes: &[&'static str],
//!     ) -> Result<MintedToken, ServiceUserAccessError> {
//!         let secret = format!("site_{}", service_user.as_str());
//!         self.issued.lock().unwrap().push((secret.clone(), scopes.to_vec()));
//!         Ok(MintedToken {
//!             public_id: format!("tok_{}", service_user.as_str()),
//!             secret: RevealOnce::new(secret),
//!             expires_at: chrono::Utc::now() + chrono::Duration::days(i64::from(ttl.days())),
//!         })
//!     }
//!
//!     fn client_setup(&self, handle: &str, _chosen: &[ServiceUserAccessChoice]) -> Vec<ClientSetupStep> {
//!         vec![ClientSetupStep {
//!             title: format!("Add @{handle} to Claude Code"),
//!             code: "claude mcp add-json --scope user site '{\"type\":\"http\",\"url\":\"https://example.test/api/v1/mcp\",\"headers\":{\"Authorization\":\"Bearer ${SITE_AGENT_TOKEN}\"}}'".into(),
//!         }]
//!     }
//! }
//!
//! // Inside the host's per-request context closure:
//! provide_service_user_access_backend(Arc::new(SiteServiceUsers::default()));
//! ```
//!
//! After this, `/user/service-users/new` offers "Read metrics". Picking it adds the service user
//! to Metric readers (after the person confirms with their authenticator), and
//! the token step returns a [`service_users::MintedTokenView`] whose `can_do` reads "read
//! metrics" and whose `setup` holds the command above.
//!
//! ## When a service user access choice can't be given
//!
//! [`service_users::ServiceUserAccessBackend::can_grant_group`] answers per viewer. When it
//! says no, the choice stays on the page, switched off, with "Only an owner of Metric readers
//! or a super user can add service users to it." The server refuses the same choice with
//! `reason_class=not_grantable`, so the page is never the only check.
//!
//! ```rust,ignore
//! let views = lepton_app::service_users::service_user_access_choices().await?;
//! let metrics = views.iter().find(|c| c.key == "metrics-read").expect("offered");
//! assert_eq!(metrics.grantable, false); // viewer doesn't own Metric readers
//! ```
//!
//! ## Feature flags
//!
//! | Flag | Effect |
//! |------|--------|
//! | `webauthn` | Shows Add passkey UI in [`SecurityDevicesSection`]; host must also enable `lepton-auth/webauthn` on SSR (not hydrate). |
//! | `oauth-google` | Google link CTAs in [`ConnectedAccountsSection`] and matching `lepton-auth-ui` OAuth buttons. |
//! | `oauth-github` | GitHub link CTAs in [`ConnectedAccountsSection`] and matching `lepton-auth-ui` OAuth buttons. |
//! | `hydrate` / `ssr` | Standard Leptos client/server split for this crate and its uf-product / lepton-auth deps. |
//!
//! ## Pages
//!
//! | Path | Page | Module (private) |
//! |------|------|-------------------|
//! | `/user` | Redirect to [`paths::ACCOUNT_SETTINGS`] | — |
//! | `/user/profile` | [`ProfilePage`] | `profile` |
//! | `/user/appearance` | [`AppearancePage`] | `appearance` (+ `appearance_preview` gallery) |
//! | `/user/account-settings` | [`AccountSettingsPage`] | `account_settings` (+ `connected_accounts_section`, `devices_section`, `totp_section`, `wipe_section`) |
//! | `/user/confirm-account` | [`ConfirmAccountPage`] | `confirm_account` |
//! | `/user/service-users` | [`service_users::ServiceUsersPage`] | [`service_users`] |
//! | `/user/service-users/new` | [`service_users::CreateServiceUserPage`] | [`service_users`] |
//! | `/user/service-users/{service_user_id}` | [`service_users::ServiceUserDetailPage`] | [`service_users`] |
//! | `/user/service-users/{service_user_id}/token` | [`service_users::ServiceUserTokenPage`] | [`service_users`] |
//!
//! [`UserLayout`] wraps these under the app-bar/left-nav shell; WASM code-split lazy
//! views ([`AccountSettingsRoute`], [`AppearanceRoute`], [`ProfileRoute`],
//! [`ConfirmAccountRoute`]) live in the private `lazy_routes` module.
//!
//! Profile and account mutations return `ServerFnError` at the Leptos boundary.
//!
//! ## Examples
//!
//! Start with [Mount user routes](#mount-user-routes). Path protect + inventory smoke
//! (Axum oneshot, no lazy WASM chunks):
//! [`examples/lepton-mount-host`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/examples/lepton-mount-host).
//! Lazy routes + Playwright; kit for deferred domain matrices: workspace
//! [`lepton-uf-app-e2e`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/lepton-uf-app-e2e).

use leptos::prelude::*;
use leptos_router::{components::*, path, Lazy};
use uf_product_macros::uf_app;

mod account_settings;
mod appearance;
mod appearance_preview;
mod clipboard;
mod confirm_account;
mod connected_accounts_section;
mod delivery_settings;
mod devices_section;
mod layout;
mod lazy_routes;
mod permissions;
mod profile;
mod profile_photo_display;
mod profile_photo_upload;
pub mod service_users;
mod totp_section;
mod wipe_section;

pub use account_settings::AccountSettingsPage;
pub use appearance::AppearancePage;
pub use confirm_account::ConfirmAccountPage;
pub use connected_accounts_section::ConnectedAccountsSection;
pub use delivery_settings::{
    get_delivery_settings, rotate_totp_seal_key, save_email_settings, save_oauth_settings,
    save_sms_settings, test_send_email, test_send_sms, DeliverySettingsPage,
};
pub use devices_section::SecurityDevicesSection;
pub use layout::UserLayout;
pub use lazy_routes::{
    prefetch_family, AccountSettingsRoute, AppearanceRoute, ConfirmAccountRoute,
    DeliverySettingsRoute, ProfileRoute, ServiceUserDetailRoute, ServiceUserNewRoute,
    ServiceUserTokenRoute, ServiceUsersRoute,
};
pub use permissions::LeptonPermission;
pub use profile::{
    get_my_profile, update_my_profile, validate_display_name, ProfileData, ProfilePage,
    MAX_DISPLAY_NAME_CHARS,
};
pub use totp_section::TotpSettingsSection;

uf_app! {
    name: "User Settings",
    id: "lepton-app",
    description: "User profile and account settings",
    icon: "👤",
    version: "0.2.0",
    routes: UserAppRoutes,
    route_path: "/user/account-settings",
    repository: "https://github.com/unified-field-dev/lepton-uf-app",
    permission_manifest: permissions::LeptonPermission,
}

/// Nested `/user` routes for a Unified Field host `Router` / `Routes` tree.
///
/// Drop in as a route child alongside `lepton_auth_app::LeptonAuthRoutes`. Pair
/// with `lepton_shell::AppBarUserMenu` via `uf_integrations::provide_shell_auth_menu`.
/// Leaf path constants live in [`paths`] (`PROFILE`, `APPEARANCE`,
/// `ACCOUNT_SETTINGS`, `CONFIRM_ACCOUNT`).
///
/// # Examples
///
/// Start with crate-root [Mount user routes](crate#mount-user-routes).
/// [`examples/lepton-mount-host`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/examples/lepton-mount-host)
/// asserts `/user` protect + `uf_app!` id (no route tree compile). Workspace
/// [`lepton-uf-app-e2e`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/lepton-uf-app-e2e)
/// (`Lazy::<ProfileRoute>` family + Playwright).
#[orbital_macros::orbital_routes_extract]
#[component(transparent)]
pub fn UserAppRoutes() -> impl leptos_router::MatchNestedRoutes + Clone {
    view! {
        <ParentRoute path=path!("user") view=UserLayout>
            <Route path=path!("") view=|| view! { <Redirect path=paths::ACCOUNT_SETTINGS /> } />
            <Route path=path!("profile") view={Lazy::<ProfileRoute>::new()} />
            <Route path=path!("appearance") view={Lazy::<AppearanceRoute>::new()} />
            <Route path=path!("account-settings") view={Lazy::<AccountSettingsRoute>::new()} />
            <Route path=path!("confirm-account") view={Lazy::<ConfirmAccountRoute>::new()} />
            <Route path=path!("delivery") view={Lazy::<DeliverySettingsRoute>::new()} />
            <Route path=path!("service-users") view={Lazy::<ServiceUsersRoute>::new()} />
            <Route path=path!("service-users/new") view={Lazy::<ServiceUserNewRoute>::new()} />
            <Route path=path!("service-users/:service_user_id") view={Lazy::<ServiceUserDetailRoute>::new()} />
            <Route path=path!("service-users/:service_user_id/token") view={Lazy::<ServiceUserTokenRoute>::new()} />
        </ParentRoute>
    }
    .into_inner()
}
