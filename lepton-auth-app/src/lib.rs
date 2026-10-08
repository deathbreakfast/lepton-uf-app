#![recursion_limit = "256"]
#![allow(missing_docs)] // uf_app! / routes macros emit undocumented associated items
#![allow(clippy::too_long_first_doc_paragraph)]
#![deny(clippy::missing_errors_doc)]
//! `/auth` authentication routes and pages for Unified Field hosts.
//!
//! Registers sign-in, sign-up, logout, password reset, and OAuth callback pages under
//! `/auth` via [`LeptonAuthRoutes`]. Identity and session logic live in **lepton-auth** /
//! **higgs**; delivery adapters and OAuth client config are host concerns. Pair with
//! `lepton_shell::AppBarUserMenu` and `lepton_app::UserAppRoutes`. Workspace mount
//! recipe: crate README at the `lepton-uf-app` root.
//!
//! Auth mutations live in **lepton-auth** / **lepton-auth-ui** and return
//! `ServerFnError` at the Leptos boundary; this crate mounts pages only.
//!
//! ## Features
//!
//! - **Authentication routes** — Registers sign-in, sign-up, logout, password reset,
//!   and OAuth callback under `/auth` in the host `Router`. Mount once at host boot
//!   alongside user routes. [Get started](#mount-auth-routes)
//! - **Auth route host** — Keeps the first gated `referer` query value on sign-in,
//!   sign-up, and logout pages so WASM remounts cannot drop the return path.
//!   [Guide](#auth-route-referer-freeze)
//! - **OAuth callback page** — Provides the provider handoff dialog at
//!   `/auth/oauth/callback`. [Guide](#oauth-callback)
//! - **Password reset route host** — Provides referer sanitization on password-reset pages
//!   so cancel navigates to a safe path without freezing the referer like sign-in routes.
//!   [Guide](#password-reset-referer)
//!
//! ## Mount auth routes
//!
//! [`LeptonAuthRoutes`] registers the `/auth` tree under your host `Router` / `Routes`.
//! Call it once at host boot (before routed pages mount) alongside
//! `lepton_app::UserAppRoutes` and wire `lepton_shell::AppBarUserMenu` through
//! `uf_integrations::provide_shell_auth_menu`.
//!
//! **Prerequisites:** `ssr` and/or `hydrate` on this crate; `uf_product` session context
//! from `provide_auth_context` at boot.
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
//! On success the host serves `/auth/signin`, `/auth/signup`, and related auth pages.
//! `uf_app!` registers id `orbital-auth` at `/auth/signin`. Bare `/auth` redirects to
//! sign-in. Set `UF_LEPTON_SIGNUP_DISABLED=1` to refuse new accounts (see lepton
//! `SECURITY.md`).
//!
//! ## Auth route referer freeze
//!
//! [`pages::AuthRouteHost`] wraps sign-in, sign-up, and logout routes. It freezes the
//! first specific `referer` query value with [`lepton_shell::retain_frozen_post_auth_referer`]
//! so WASM remounts onto `/auth/signin` cannot drop the gated return path. On auth success
//! it calls `trigger_refresh` so `/user/*` gates see the authenticated session.
//!
//! **Prerequisites:** `lepton-shell` on the SSR graph; session context from `uf_product`.
//!
//! ```rust,ignore
//! use lepton_auth_app::pages::AuthRouteHost;
//! use lepton_auth_ui::AuthDialogKind;
//! use lepton_shell::retain_frozen_post_auth_referer;
//! use leptos::prelude::*;
//!
//! #[component]
//! fn SigninRoute() -> impl IntoView {
//!     view! { <AuthRouteHost initial_kind=AuthDialogKind::Signin test_id="signin-page" /> }
//! }
//!
//! assert_eq!(retain_frozen_post_auth_referer("/tag", "/auth/signin"), "/tag");
//! ```
//!
//! When the user started on `/tag`, freezing keeps `/tag` as the close target even if the
//! live URL becomes `/auth/signin?referer=/tag`. [`pages::PasswordResetRouteHost`] does not
//! freeze — see [Password reset referer](#password-reset-referer).
//!
//! ## OAuth callback
//!
//! [`OAuthCallbackPage`] provides `lepton_auth_ui::OAuthCallbackContent` at
//! `/auth/oauth/callback`. It parses `provider`, `code`, `state`, and `referer` from the
//! query string and completes the OAuth handoff inside `AuthModalShell`.
//!
//! **Prerequisites:** matching `oauth-google` or `oauth-github` features on this crate and
//! host OAuth client config.
//!
//! ```rust,ignore
//! use lepton_auth_app::OAuthCallbackPage;
//! use lepton_auth_ui::OAuthCallbackContent;
//! use leptos::prelude::*;
//!
//! #[component]
//! fn OauthCallback() -> impl IntoView {
//!     view! { <OAuthCallbackPage /> }
//! }
//!
//! // OAuthCallbackPage renders data-testid="oauth-callback-container".
//! assert!("oauth-callback-container".len() > 0);
//! ```
//!
//! Success navigates to the sanitized referer; provider errors stay in the modal.
//!
//! ## Password reset referer
//!
//! [`pages::PasswordResetRouteHost`] sanitizes referer paths from the query string via
//! [`lepton_shell::sanitize_post_auth_navigate_path`] (defaulting to sign-in when the
//! sanitized path is `/`) and navigates back on close. It re-reads `location.search` on
//! each navigation — no referer freeze unlike [`pages::AuthRouteHost`].
//!
//! **Prerequisites:** `lepton-shell` on the SSR graph; SSR hosts that send reset mail need
//! [`provide_auth_services`](../lepton_auth/index.html#boot-delivery-email-only) from
//! **lepton-auth**.
//!
//! ```rust,ignore
//! use lepton_auth_app::pages::PasswordResetRouteHost;
//! use lepton_auth_ui::PasswordResetDialogKind;
//! use lepton_shell::sanitize_post_auth_navigate_path;
//! use leptos::prelude::*;
//!
//! #[component]
//! fn ResetRequest() -> impl IntoView {
//!     view! {
//!         <PasswordResetRouteHost
//!             initial_kind=PasswordResetDialogKind::Request
//!             test_id="password-reset-request"
//!         />
//!     }
//! }
//!
//! assert_eq!(sanitize_post_auth_navigate_path(Some("/".to_string())), "/");
//! ```
//!
//! Closing the dialog navigates to the sanitized referer or sign-in when referer was `/`.
//!
//! ## Feature flags
//!
//! | Flag | Effect |
//! |------|--------|
//! | `oauth-google` | Google OAuth sign-in buttons and callback wiring via `lepton-auth` / `lepton-auth-ui`. |
//! | `oauth-github` | GitHub OAuth sign-in buttons and callback wiring via `lepton-auth` / `lepton-auth-ui`. |
//! | `hydrate` / `ssr` | Standard Leptos client/server split for this crate and its uf-product / lepton-auth deps. |
//!
//! ## Route hosts
//!
//! | Path | Page |
//! |------|------|
//! | `/auth` | Redirect to [`paths::SIGNIN`] |
//! | `/auth/signin` | [`SigninPage`] |
//! | `/auth/signup` | [`SignupPage`] |
//! | `/auth/logout` | [`LogoutPage`] |
//! | `/auth/oauth/callback` | [`OAuthCallbackPage`] |
//! | `/auth/reset/request` | [`PasswordResetRequestPage`] |
//! | `/auth/reset/confirm` | [`PasswordResetConfirmPage`] |
//!
//! Sign-in, sign-up, logout, and password-reset pages mount through eager
//! [`pages::AuthRouteHost`] and [`pages::PasswordResetRouteHost`] wrappers (not WASM lazy
//! routes like `lepton_app::UserAppRoutes`).
//!
//! ## Examples
//!
//! Start with [Mount auth routes](#mount-auth-routes). Public `/auth` inventory smoke
//! (Axum oneshot, no auth page mount):
//! [`examples/lepton-mount-host`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/examples/lepton-mount-host).
//! Eager auth pages + Playwright; kit for deferred auth matrices: workspace
//! [`lepton-uf-app-e2e`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/lepton-uf-app-e2e).

use uf_product_macros::uf_app;

/// Page components for the `/auth/*` routes (signin, signup, logout, OAuth callback, password reset).
pub mod pages;
/// Nested `/auth` route wiring; see [`LeptonAuthRoutes`].
pub mod routes;

pub use pages::{
    LogoutPage, OAuthCallbackPage, PasswordResetConfirmPage, PasswordResetRequestPage, SigninPage,
    SignupPage,
};
pub use routes::paths;
pub use routes::LeptonAuthRoutes;

uf_app! {
    name: "Lepton Auth",
    id: "orbital-auth",
    description: "Authentication routes for Unified Field hosts (lepton/higgs)",
    icon: "🔐",
    version: "0.2.0",
    routes: LeptonAuthRoutes,
    route_path: "/auth/signin",
    repository: "https://github.com/unified-field-dev/lepton-uf-app",
}
