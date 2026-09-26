//! Product Account Settings + auth-app surface contracts (sibling crates).
//!
//! Lives under `lepton-shell` so CI can run without compiling `lepton-app` /
//! `lepton-auth-app` when host integration deps are mid-churn. Domain happy/sad
//! for wipe / TOTP / OAuth / confirm stay in the lepton kit.

#![allow(clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_app(rel: &str) -> String {
    let path = workspace_root().join("lepton-app").join("src").join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Concatenate module sources after a file→directory split (e.g. `totp_section/`).
fn read_app_module(dir: &str, files: &[&str]) -> String {
    files
        .iter()
        .map(|f| read_app(&format!("{dir}/{f}")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_auth_app(rel: &str) -> String {
    let path = workspace_root()
        .join("lepton-auth-app")
        .join("src")
        .join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn account_settings_composes_sections_happy_path() {
    let page = read_app("account_settings/mod.rs");
    for needle in [
        "TotpSettingsSection",
        "ConnectedAccountsSection",
        "SecurityDevicesSection",
        "AccountWipeSection",
        "ConfirmAccountPrompt",
    ] {
        assert!(
            page.contains(needle),
            "AccountSettingsPage must compose {needle}"
        );
    }

    let lib = read_app("lib.rs");
    assert!(
        lib.contains("account-settings") && lib.contains("confirm-account"),
        "UserAppRoutes must mount account-settings and confirm-account"
    );
    let confirm = read_app("confirm_account.rs");
    assert!(
        confirm.contains("ConfirmAccountFunnel") || confirm.contains("ConfirmAccountPage"),
        "confirm-account page must wrap lepton-auth-ui confirm funnel"
    );
}

#[test]
fn account_settings_testid_contracts_happy_path() {
    let wipe = read_app("wipe_section.rs");
    for id in [
        "account-wipe-section",
        "account-wipe-form",
        "account-wipe-submit",
        "account-wipe-error",
        "WipeAccount",
        "WIPE_CONFIRM_PHRASE",
        "confirm_phrase",
        "current_password",
    ] {
        assert!(wipe.contains(id), "wipe_section missing contract `{id}`");
    }

    let totp = read_app_module("totp_section", &["mod.rs", "steps.rs", "helpers.rs"]);
    for id in [
        "totp-settings-section",
        "totp-settings-setup",
        "totp-settings-confirm",
        "totp-settings-code",
        "totp-settings-error",
        "totp-settings-enabled",
        "totp-settings-disable",
    ] {
        assert!(totp.contains(id), "totp_section missing contract `{id}`");
    }

    let oauth = read_app("connected_accounts_section.rs");
    for id in [
        "connected-accounts-section",
        "connected-accounts-link-google",
        "connected-accounts-link-github",
        "connected-accounts-unlink",
        "connected-accounts-error",
    ] {
        assert!(
            oauth.contains(id),
            "connected_accounts_section missing contract `{id}`"
        );
    }

    let devices = read_app("devices_section.rs");
    for id in ["devices-section", "devices-list", "devices-error"] {
        assert!(
            devices.contains(id),
            "devices_section missing contract `{id}`"
        );
    }

    let settings = read_app_module("account_settings", &["mod.rs", "email.rs", "password.rs"]);
    for id in [
        "account-verify-email-form",
        "account-email-unverified-banner",
        "account-masked-email",
    ] {
        assert!(
            settings.contains(id),
            "account_settings missing contract `{id}`"
        );
    }
}

#[test]
fn account_settings_missing_wipe_action_sad_path() {
    let wipe = read_app("wipe_section.rs");
    assert!(
        wipe.contains("ServerAction::<WipeAccount>") || wipe.contains("ServerAction<WipeAccount>"),
        "wipe UI must bind WipeAccount server action (regression would drop kit e2e parity)"
    );
    assert!(
        !wipe.contains("unimplemented!"),
        "wipe section must not ship unimplemented placeholders"
    );
}

#[test]
fn account_settings_totp_qr_uses_server_svg_sad_if_client_markup() {
    let totp = read_app_module("totp_section", &["mod.rs", "steps.rs", "helpers.rs"]);
    assert!(
        totp.contains("totp-settings-qr"),
        "TOTP enroll must expose qr testid for kit/product e2e"
    );
    assert!(
        totp.contains("data-otpauth") || totp.contains("otpauth_uri"),
        "QR path must carry server otpauth (not an empty client-only image)"
    );
    assert!(
        totp.contains("safe_totp_qr_svg"),
        "QR path must gate SVG before inner_html"
    );
    assert!(
        totp.contains("QR_SVG_ALLOWED_TAGS") && totp.contains("QR_SVG_ALLOWED_ATTRS"),
        "QR SVG gate must use tag/attribute allowlists"
    );
}

#[test]
fn auth_routes_mount_core_pages_happy_path() {
    let routes = read_auth_app("routes.rs");
    for needle in [
        "signup",
        "signin",
        "logout",
        "oauth/callback",
        "reset/request",
        "reset/confirm",
        "OAuthCallbackPage",
        "SignupPage",
        "SigninPage",
    ] {
        assert!(
            routes.contains(needle),
            "LeptonAuthRoutes missing `{needle}`"
        );
    }
}

#[test]
fn oauth_callback_uses_kit_content_happy_path() {
    let page = read_auth_app("pages/oauth_callback.rs");
    assert!(
        page.contains("OAuthCallbackContent"),
        "OAuth callback page must compose lepton-auth-ui OAuthCallbackContent"
    );
}

#[test]
fn auth_routes_drop_oauth_callback_sad_path() {
    let routes = read_auth_app("routes.rs");
    assert!(
        routes.contains(r#"path!("oauth/callback")"#) || routes.contains("oauth/callback"),
        "removing OAuth callback breaks link/login completion for hosts"
    );
    assert!(
        !routes.contains("unimplemented!"),
        "auth routes must not ship unimplemented placeholders"
    );
}

#[test]
fn user_routes_mount_profile_appearance_happy_path() {
    let lib = read_app("lib.rs");
    for needle in [
        r#"path!("user")"#,
        r#"path!("profile")"#,
        r#"path!("appearance")"#,
        r#"path!("account-settings")"#,
        r#"path!("confirm-account")"#,
        "ProfileRoute",
        "AppearanceRoute",
        "AccountSettingsRoute",
        "ConfirmAccountRoute",
    ] {
        assert!(lib.contains(needle), "UserAppRoutes missing `{needle}`");
    }
}

#[test]
fn user_routes_drop_appearance_sad_path() {
    let lib = read_app("lib.rs");
    assert!(
        lib.contains(r#"path!("appearance")"#) || lib.contains("appearance"),
        "dropping appearance leaves settings without theme preferences"
    );
    assert!(
        lib.contains(r#"path!("profile")"#) || lib.contains("profile"),
        "dropping profile breaks the default settings homepage"
    );
    assert!(
        !lib.contains("unimplemented!"),
        "user routes must not ship unimplemented placeholders"
    );
}

#[test]
fn user_layout_nav_and_confirm_prompt_happy_path() {
    let layout = read_app("layout.rs");
    for id in [
        "user-app-layout-root",
        "nav-user-profile",
        "nav-lepton-appearance",
        "nav-user-account-settings",
        "ConfirmAccountPrompt",
        "AppBarUserMenu",
    ] {
        assert!(layout.contains(id), "UserLayout missing contract `{id}`");
    }
}

#[test]
fn user_layout_missing_nav_sad_path() {
    let layout = read_app("layout.rs");
    assert!(
        layout.contains("nav-user-profile")
            && layout.contains("nav-lepton-appearance")
            && layout.contains("nav-user-account-settings"),
        "dropping a left-nav link hides a settings page from operators"
    );
    assert!(
        layout.contains("ConfirmAccountPrompt"),
        "layout must keep soft-confirm prompt for unverified sessions"
    );
}

#[test]
fn profile_page_testid_and_update_action_happy_path() {
    let profile = read_app("profile.rs");
    for id in [
        "profile-container",
        "profile-photo-section",
        "profile-display-name",
        "profile-submit",
        "profile-success",
        "profile-error",
        "UpdateMyProfile",
        "GetMyProfile",
    ] {
        assert!(profile.contains(id), "profile page missing contract `{id}`");
    }
    assert!(
        profile.contains("ServerAction::<UpdateMyProfile>")
            || profile.contains("ServerAction<UpdateMyProfile>"),
        "profile must bind UpdateMyProfile server action"
    );
}

#[test]
fn profile_empty_display_name_rejected_sad_path() {
    let profile = read_app("profile.rs");
    assert!(
        profile.contains("validate_display_name")
            && profile.contains("Display name cannot be empty"),
        "UpdateMyProfile must reject empty display names"
    );
    assert!(
        profile.contains("Display name is too long") && profile.contains("MAX_DISPLAY_NAME_CHARS"),
        "UpdateMyProfile must reject over-long display names"
    );
    assert!(
        profile.contains("ServerFnError::Args") || profile.contains("ServerFnError::new"),
        "empty display name must surface as a typed server-fn error"
    );
    assert!(
        profile.contains("lepton_auth::paths::SIGNIN") || profile.contains("SIGNIN"),
        "anonymous visitors must be redirected to sign-in"
    );
    assert!(
        profile.contains("reason_class=") && profile.contains("profile_server_err"),
        "profile server fns must return opaque reason_class errors"
    );
    assert!(
        profile.contains("profile id missing") || profile.contains("profile_id"),
        "get_my_profile must fail when profile id is absent"
    );
}

#[test]
fn appearance_page_testid_contracts_happy_path() {
    let appearance = read_app("appearance.rs");
    for id in [
        "appearance-page",
        "appearance-save",
        "appearance-preset-",
        "save_my_appearance",
        "get_my_appearance",
    ] {
        assert!(
            appearance.contains(id),
            "appearance page missing contract `{id}`"
        );
    }
}

#[test]
fn appearance_anon_redirects_to_signin_sad_path() {
    let appearance = read_app("appearance.rs");
    assert!(
        appearance.contains("is_authenticated")
            && (appearance.contains("lepton_auth::paths::SIGNIN") || appearance.contains("SIGNIN")),
        "anonymous appearance visits must navigate to sign-in"
    );
}

#[test]
fn account_settings_anon_redirects_to_signin_sad_path() {
    let lazy_routes = read_app("lazy_routes.rs");
    let block = lazy_routes
        .split("struct AccountSettingsRoute")
        .nth(1)
        .and_then(|rest| rest.split("struct ConfirmAccountRoute").next())
        .expect("AccountSettingsRoute route struct missing");
    assert!(
        block.contains("RequireAuthenticated"),
        "anonymous account-settings visits must be gated by RequireAuthenticated"
    );
}

#[test]
fn confirm_account_anon_redirects_to_signin_sad_path() {
    let lazy_routes = read_app("lazy_routes.rs");
    let block = lazy_routes
        .split("struct ConfirmAccountRoute")
        .nth(1)
        .expect("ConfirmAccountRoute route struct missing");
    assert!(
        block.contains("RequireAuthenticated"),
        "anonymous confirm-account visits must be gated by RequireAuthenticated"
    );
}

#[test]
fn auth_page_container_testids_happy_path() {
    let signin = read_auth_app("pages/signin.rs");
    let signup = read_auth_app("pages/signup.rs");
    let logout = read_auth_app("pages/logout.rs");
    let oauth = read_auth_app("pages/oauth_callback.rs");
    let reset = read_auth_app("pages/password_reset.rs");
    assert!(
        signin.contains("signin-container"),
        "signin page must expose signin-container"
    );
    assert!(
        signup.contains("signup-container"),
        "signup page must expose signup-container"
    );
    assert!(
        logout.contains("logout-container"),
        "logout page must expose logout-container"
    );
    assert!(
        oauth.contains("oauth-callback-container"),
        "oauth callback must expose oauth-callback-container"
    );
    assert!(
        reset.contains("password-reset-request-container")
            && reset.contains("password-reset-confirm-container"),
        "password reset pages must expose request/confirm containers"
    );
}

#[test]
fn auth_page_container_drop_sad_path() {
    let signin = read_auth_app("pages/signin.rs");
    assert!(
        signin.contains("AuthRouteHost") && signin.contains("AuthDialogKind::Signin"),
        "signin must host AuthDialog via AuthRouteHost"
    );
    assert!(
        !signin.contains("unimplemented!"),
        "auth pages must not ship unimplemented placeholders"
    );
}

#[test]
fn auth_hosts_sanitize_referer_happy_path() {
    let host = read_auth_app("pages/auth_route_host.rs");
    let oauth = read_auth_app("pages/oauth_callback.rs");
    let reset = read_auth_app("pages/password_reset_route_host.rs");
    for (label, src) in [
        ("AuthRouteHost", host.as_str()),
        ("OAuthCallbackPage", oauth.as_str()),
        ("PasswordResetRouteHost", reset.as_str()),
    ] {
        assert!(
            src.contains("sanitize_post_auth_navigate_path"),
            "{label} must sanitize referer paths via sanitize_post_auth_navigate_path"
        );
        assert!(
            src.contains("parse_referer_from_search"),
            "{label} must parse referer from the query string"
        );
    }
    assert!(
        host.contains("retain_frozen_post_auth_referer"),
        "AuthRouteHost must freeze the first specific referer so remounts onto /auth/signin cannot drop it"
    );
    assert!(
        host.contains("trigger_refresh"),
        "AuthRouteHost must refresh the client session after sign-in so gated routes do not keep the anonymous gate"
    );
}

#[test]
fn auth_hosts_raw_search_navigate_sad_path() {
    let host = read_auth_app("pages/auth_route_host.rs");
    assert!(
        host.contains("sanitize_post_auth_navigate_path(parse_referer_from_search"),
        "AuthRouteHost must not navigate on unsanitized location.search"
    );
    assert!(
        !host.contains("navigate(&location.search") && !host.contains("navigate(location.search"),
        "open-redirect risk if close navigates using raw search"
    );
}
