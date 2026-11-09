//! App-bar authentication menu, auth dialogs, and step-up for Unified Field hosts.
//!
//! This crate renders the signed-in avatar menu, hosts sign-in/sign-up/logout dialogs,
//! and mounts step-up confirmation before sensitive mutations. Session state comes from
//! `uf_product` auth context; identity backends stay in **lepton-auth** / **higgs**.
//!
//! ## Features
//!
//! - **App-bar auth menu** — Renders the avatar dropdown with profile, account settings,
//!   and sign-in/up actions for anonymous sessions. Wire it once at host boot through
//!   `uf_integrations::provide_shell_auth_menu`. [Get started](#wire-auth-menu-at-boot)
//! - **Auth dialog controller** — The menu and route gates share one `AuthDialog`
//!   signal through `AuthDialogController`. Bind the controller from
//!   `UnifiedFieldShellLayout` so `RequireAuthenticated` opens the same dialog as the
//!   avatar menu. [Guide](#share-auth-dialog-with-gates)
//! - **Step-up dialog shell** — Mounts `StepUpDialog` and provides `StepUpController` so
//!   hosts can call `request` before a sensitive mutation. [Guide](#step-up-before-sensitive-mutation)
//! - **Post-auth path sanitizer** — Rejects open-redirect bypasses in post-auth navigate
//!   targets and AuthDialog referers. [Guide](#sanitize-post-auth-navigate-path)
//! - **Frozen referer retention** — Keeps the gated path when WASM remounts rewrite the
//!   live URL to `/auth/signin`. [Guide](#freeze-post-auth-referer)
//! - **People search** — The `lepton_people` search source finds people by display name
//!   for person pickers, without emails. [`people::sources`] names it and
//!   the service user and team sources. [Module docs](people)
//! - **Person picker** — [`people::PersonPicker`] is one typeahead for people, teams and
//!   service users, with Persona rows, chips, disabled rows and caller-supplied groups.
//!   [Module docs](people)
//!
//! ## Wire auth menu at boot
//!
//! The app-bar menu is the primary auth entry for signed-in and anonymous users. Call
//! `uf_integrations::provide_shell_auth_menu` once at host boot (before routed pages
//! mount) and pair the slot with `lepton_app::UserAppRoutes` and
//! `lepton_auth_app::LeptonAuthRoutes` under the host `Router`.
//!
//! On SSR hosts that send verification mail, inject delivery first with
//! [`provide_auth_services`](../lepton_auth/index.html#boot-delivery-email-only) from
//! **lepton-auth** so sign-up and password-reset dialogs can enqueue email.
//!
//! **Prerequisites:** `ssr` and/or `hydrate` on this crate; `uf_product` session context
//! from `provide_auth_context` at boot.
//!
//! ```rust,ignore
//! use lepton_shell::AppBarUserMenu;
//! use lepton_app::UserAppRoutes;
//! use lepton_auth_app::LeptonAuthRoutes;
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
//! On success the app bar shows the avatar menu and `AuthDialog` / `StepUpDialog` mount
//! beside routed pages. Omit `provide_shell_auth_menu` when the host has no auth chrome.
//!
//! ## Share auth dialog with gates
//!
//! Route gates such as `uf_product::routes::RequireAuthenticated` open the same dialog
//! as the avatar menu when you bind the `AuthDialogController` from
//! `UnifiedFieldShellLayout`. [`AppBarUserMenu`] reuses that controller when present and
//! only calls `provide_auth_dialog_controller` as a fallback for standalone mounts.
//!
//! **Prerequisites:** shell layout that provides `AuthDialogController` in Leptos context.
//!
//! ```rust,ignore
//! use lepton_shell::AppBarUserMenu;
//! use uf_product::routes::RequireAuthenticated;
//! use uf_product::AuthDialogController;
//! use leptos::prelude::*;
//!
//! #[component]
//! fn GatedPage() -> impl IntoView {
//!     view! {
//!         <RequireAuthenticated permission_name="demo.read">
//!             <p>"Secret content"</p>
//!         </RequireAuthenticated>
//!     }
//! }
//!
//! // AppBarUserMenu reads use_context::<AuthDialogController>() and calls
//! // controller.open_signin() from the menu or from the gate — same signal.
//! ```
//!
//! When an anonymous visitor hits the gate, `RequireAuthenticated` calls
//! `controller.open_signin()` and the menu-mounted `AuthDialog` opens. A disconnected
//! `Default` controller would leave the gate talking to a different signal than the menu.
//!
//! ## Step-up before sensitive mutation
//!
//! [`AppBarUserMenu`] mounts `StepUpDialog` and provides `StepUpController` when the
//! host has not already installed one. Sensitive server actions (account wipe, TOTP
//! disable, OAuth unlink) call `StepUpController::request` before proceeding; the dialog
//! collects a second factor and resumes the action on success. Account Settings does
//! not drive step-up today — hosts wire `request` on the mutations they protect.
//!
//! **Prerequisites:** `lepton-auth-ui` step-up feature on the SSR graph; session context
//! from `uf_product`.
//!
//! ```rust,ignore
//! use lepton_auth_ui::{provide_step_up_controller, StepUpController};
//! use leptos::prelude::*;
//!
//! #[component]
//! fn SensitiveAction() -> impl IntoView {
//!     // AppBarUserMenu calls provide_step_up_controller when none is in context.
//!     let step_up = use_context::<StepUpController>().expect("AppBarUserMenu mounts this");
//!     let open = step_up.open();
//!     view! {
//!         <button on:click=move |_| {
//!             step_up.request("wipe_account", move || {
//!                 // run server action after step-up succeeds
//!             });
//!         }>
//!             "Wipe account"
//!         </button>
//!         <p>{move || if open().get() { "Step-up open" } else { "Idle" }}</p>
//!     }
//! }
//! ```
//!
//! On success `open().get()` becomes true while the dialog collects the factor; the
//! callback runs after verification. Missing `StepUpController` context means the host
//! forgot to mount [`AppBarUserMenu`] or `provide_step_up_controller`.
//!
//! ## Sanitize post-auth navigate path
//!
//! [`sanitize_post_auth_navigate_path`] wraps `lepton_auth::routes::sanitize_referer_path`
//! and rejects additional open-redirect bypasses (control characters, `://` smuggling,
//! backslashes) before AuthDialog referers or post-auth navigations run.
//!
//! **Prerequisites:** none beyond the helper import.
//!
//! ```rust
//! use lepton_shell::sanitize_post_auth_navigate_path;
//!
//! assert_eq!(
//!     sanitize_post_auth_navigate_path(Some("/user/profile".to_string())),
//!     "/user/profile"
//! );
//! assert_eq!(
//!     sanitize_post_auth_navigate_path(Some("//evil.example".to_string())),
//!     "/"
//! );
//! ```
//!
//! Safe in-app paths (including query strings) pass through; protocol-relative and
//! smuggled URLs fall back to `"/"`. [`AppBarUserMenu`] snapshots the sanitized path when
//! the auth dialog opens.
//!
//! ## Freeze post-auth referer
//!
//! [`retain_frozen_post_auth_referer`] keeps the first gated path when a later read
//! sanitizes to `"/"`. WASM remounts can rewrite the live URL to `/auth/signin` after the
//! user started on a product route such as `/tag`; freezing prevents dropping the return
//! target. `lepton_auth_app` route hosts use the same helper — see their
//! [Route hosts](../lepton_auth_app/index.html#route-hosts) section.
//!
//! **Prerequisites:** capture the frozen path when the dialog or auth page first opens.
//!
//! ```rust
//! use lepton_shell::retain_frozen_post_auth_referer;
//!
//! assert_eq!(retain_frozen_post_auth_referer("/tag", "/auth/signin"), "/tag");
//! assert_eq!(retain_frozen_post_auth_referer("/", "/gate/auth-required"), "/gate/auth-required");
//! ```
//!
//! When `frozen` is not `"/"`, the frozen value wins; otherwise the incoming path is
//! sanitized through [`sanitize_post_auth_navigate_path`].
//!
//! ## Feature flags
//!
//! | Flag | Effect |
//! |------|--------|
//! | `hydrate` | Client-side Leptos split for this crate and `uf-product` / `lepton-auth-ui` deps. |
//! | `ssr` | Server-side Leptos split; required for auth server fns behind the dialogs. |
//!
//! ## Examples
//!
//! Start with [Wire auth menu at boot](#wire-auth-menu-at-boot). Path/auth/inventory smoke
//! (Axum oneshot, no Leptos UI mount):
//! [`examples/lepton-mount-host`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/examples/lepton-mount-host).
//! Real mount of all three crates + Playwright: workspace
//! [`lepton-uf-app-e2e`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/lepton-uf-app-e2e).
//!
//! Surface contracts: `tests/product_surface.rs`, `tests/shell_step_up_surface.rs`.

#![deny(clippy::missing_errors_doc)]

use lepton_auth::paths::{USER_ACCOUNT_SETTINGS, USER_PROFILE};
use lepton_auth_ui::{
    provide_step_up_controller, AuthDialog, AuthDialogCallbacks, AuthDialogKind, StepUpDialog,
};
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use orbital_base_components::initials_from_name;
use orbital_primitives::*;
use uf_product::provide_auth_dialog_controller;
use uf_product::use_app_bar_menu_extras;
use uf_product::use_auth_context;
use uf_product::AppBarCompactMenuExtras;
use uf_product::AuthDialogController;
use uf_product::AuthDialogIntent;

mod navigate_path;
pub mod people;
mod session_menu_items;
use session_menu_items::SessionMenuItems;

pub use navigate_path::{retain_frozen_post_auth_referer, sanitize_post_auth_navigate_path};

/// Signed-in/anonymous avatar menu for the host app bar.
///
/// Prefer [`crate`] [Wire auth menu at boot](index.html#wire-auth-menu-at-boot) via
/// `uf_integrations::provide_shell_auth_menu`. Pair with `lepton_app::UserAppRoutes` and
/// `lepton_auth_app::LeptonAuthRoutes`. Requires `uf_product` auth context.
///
/// Bind the [`AuthDialogController`] from `UnifiedFieldShellLayout` when present so
/// [`uf_product::routes::RequireAuthenticated`] opens this same dialog. If no
/// controller is in context, this menu provides one for its own `AuthDialog` —
/// never a disconnected `Default` handle that leaves the gate talking to a
/// different signal.
///
/// # Examples
///
/// Start with crate-root [Wire auth menu at boot](index.html#wire-auth-menu-at-boot).
/// [`examples/lepton-mount-host`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/examples/lepton-mount-host)
/// inventory JSON names `AppBarUserMenu` (does not compile or mount this component).
/// Workspace [`lepton-uf-app-e2e`](https://github.com/unified-field-dev/lepton-uf-app/tree/main/lepton-uf-app-e2e)
/// (`provide_shell_auth_menu` + Playwright avatar menu).
#[component]
pub fn AppBarUserMenu() -> impl IntoView {
    let navigate = use_navigate();
    let location = use_location();
    let auth = use_auth_context();
    let session = auth.session();
    let user_label = Memo::new(move |_| session.with(|session| session.display_label()));
    let current_path = Memo::new(move |_| {
        let mut path = location.pathname.get();
        let search = location.search.get();
        if !search.is_empty() {
            path.push_str(&search);
        }
        path
    });

    // Prefer the shell-provided controller so route gates open this same dialog.
    let controller =
        use_context::<AuthDialogController>().unwrap_or_else(provide_auth_dialog_controller);
    let _step_up = use_context::<lepton_auth_ui::StepUpController>()
        .unwrap_or_else(provide_step_up_controller);
    let auth_dialog_open = controller.open();
    let auth_dialog_intent = controller.intent();
    let auth_dialog_kind = Signal::derive(move || match auth_dialog_intent.get() {
        AuthDialogIntent::Signin => AuthDialogKind::Signin,
        AuthDialogIntent::Signup => AuthDialogKind::Signup,
        AuthDialogIntent::Logout => AuthDialogKind::Logout,
    });
    // Snapshot the gated path when the dialog opens. A live current_path
    // Signal follows `/auth/signin` (sanitizes to `/`) if the URL changes
    // under the open form.
    let post_auth_referer = RwSignal::new(String::from("/"));
    Effect::new(move |was_open: Option<bool>| {
        let open = auth_dialog_open.get();
        if open && was_open != Some(true) {
            post_auth_referer.set(sanitize_post_auth_navigate_path(Some(
                current_path.get_untracked(),
            )));
        }
        open
    });
    let referer = Signal::derive(move || post_auth_referer.get());
    let close_auth_dialog = Callback::new(move |_| {
        controller.close();
    });
    let switch_to_signin = Callback::new(move |_| {
        auth_dialog_intent.set(AuthDialogIntent::Signin);
    });
    let switch_to_signup = Callback::new(move |_| {
        auth_dialog_intent.set(AuthDialogIntent::Signup);
    });
    let pending_session_refresh = RwSignal::new(false);
    let refresh_and_close = {
        let auth = auth.clone();
        Callback::new(move |_| {
            auth.trigger_refresh();
            if auth_dialog_intent.get_untracked() == AuthDialogIntent::Logout {
                controller.close();
            } else {
                pending_session_refresh.set(true);
            }
        })
    };
    Effect::new(move |_| {
        if pending_session_refresh.get()
            && matches!(session.get(), uf_product::AuthSession::Authenticated(_))
        {
            pending_session_refresh.set(false);
            controller.close();
        }
    });

    view! {
        <Menu
            on_select=move |key: &str| {
                match key {
                    "profile" => {
                        navigate(USER_PROFILE, Default::default());
                    }
                    "signin" => {
                        controller.open_signin();
                    }
                    "logout" => {
                        controller.open_logout();
                    }
                    "signup" => {
                        controller.open_signup();
                    }
                    "account_settings" => {
                        navigate(USER_ACCOUNT_SETTINGS, Default::default());
                    }
                    "appearance_settings" => {
                        navigate(uf_product::paths::USER_APPEARANCE, Default::default());
                    }
                    _ => {}
                }
            }
        >
            <MenuTrigger slot>
                <div data-testid="user-avatar">
                    {move || {
                        let label = user_label.get();
                        view! {
                            <Avatar config=AvatarConfig {
                                initials: Some(initials_from_name(&label)),
                                name: Some(label),
                                shape: AvatarShape::Circular,
                                size: Some(32),
                                color: AvatarColor::Brand,
                                ..Default::default()
                            } />
                        }
                    }}
                </div>
            </MenuTrigger>
            {move || match use_app_bar_menu_extras() {
                // Build extras under this Menu's owner so MenuItem finds MenuInjection.
                Some(extras) if extras.show.get() => {
                    view! { <AppBarCompactMenuExtras /> }.into_any()
                }
                _ => ().into_any(),
            }}
            <SessionMenuItems session=session />
        </Menu>
        <AuthDialog
            open=auth_dialog_open.into()
            kind=auth_dialog_kind
            referer=referer
            callbacks=AuthDialogCallbacks {
                on_success: Some(refresh_and_close),
                on_close: Some(close_auth_dialog),
                on_switch_signin: Some(switch_to_signin),
                on_switch_signup: Some(switch_to_signup),
            }
        />
        <StepUpDialog/>
    }
}
