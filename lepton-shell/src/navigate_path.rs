//! Post-auth navigate-path hardening for Lepton UI hosts.
//!
//! Wraps [`lepton_auth::routes::sanitize_referer_path`] and rejects additional
//! open-redirect bypasses (control characters, `://` smuggling) that some
//! browsers treat as off-origin navigations.

use lepton_auth::paths::USER_CONFIRM_ACCOUNT;
use lepton_auth::routes::{
    confirm_account_path_with_referer, parse_referer_from_search, sanitize_referer_path,
};

/// Default landing `lepton-auth` picks when the sign-in referer is `"/"`.
pub const WELCOME: &str = "/welcome";

/// Sanitize a post-auth redirect / AuthDialog referer for in-app navigation.
///
/// See the crate guide [Sanitize post-auth navigate path](index.html#sanitize-post-auth-navigate-path).
/// Invalid values fall back to `"/"`.
#[must_use]
pub fn sanitize_post_auth_navigate_path(referer: Option<String>) -> String {
    let sanitized = sanitize_referer_path(referer);
    if is_extra_safe_in_app_path(&sanitized) {
        sanitized
    } else {
        "/".to_string()
    }
}

/// Keep a captured in-app path when a later read sanitizes to `"/"`.
///
/// See the crate guide [Freeze post-auth referer](index.html#freeze-post-auth-referer).
/// Split WASM remounts the hidden `referer` field, and the live URL can become
/// `/auth/signin` (rejected by sanitize) after the user started on a gated
/// product route such as `/tag`.
#[must_use]
pub fn retain_frozen_post_auth_referer(frozen: &str, incoming: &str) -> String {
    if frozen != "/" && !frozen.is_empty() {
        frozen.to_string()
    } else {
        sanitize_post_auth_navigate_path(Some(incoming.to_string()))
    }
}

/// Decide where to go once the sign-in form has navigated to the server's `redirect_to`.
///
/// See the crate guide [Return after sign-in](index.html#return-after-sign-in).
/// `referer` is the path frozen when sign-in started; `landed` is the path and query
/// the form navigated to. Returns `None` to stay on `landed`.
///
/// - Confirm-account without its own `referer` gets the frozen referer attached, so
///   finishing the funnel still returns to the starting page.
/// - The default landings (`/welcome`, `/`) give way to a specific frozen referer.
/// - Anything else is the server's choice and stays.
///
/// Both inputs go through [`sanitize_post_auth_navigate_path`], so an off-origin
/// referer counts as `"/"` and never produces a navigation.
#[must_use]
pub fn post_signin_target(referer: &str, landed: &str) -> Option<String> {
    let referer = sanitize_post_auth_navigate_path(Some(referer.to_string()));
    if referer == "/" {
        return None;
    }
    let (landed_path, landed_search) = landed.split_once('?').unwrap_or((landed, ""));
    if landed_path == USER_CONFIRM_ACCOUNT {
        if parse_referer_from_search(landed_search).is_some() {
            return None;
        }
        return Some(confirm_account_path_with_referer(&referer));
    }
    let landed = sanitize_post_auth_navigate_path(Some(landed.to_string()));
    if (landed == "/" || landed == WELCOME) && landed != referer {
        return Some(referer);
    }
    None
}

/// Extra checks beyond lepton-auth: ASCII controls/whitespace and `://` smuggling.
fn is_extra_safe_in_app_path(path: &str) -> bool {
    if !path.starts_with('/') || path.starts_with("//") {
        return false;
    }
    if path.contains('\\') {
        return false;
    }
    if path.bytes().any(|b| b <= 0x20 || b == 0x7f) {
        return false;
    }
    if path.contains("://") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        is_extra_safe_in_app_path, post_signin_target, retain_frozen_post_auth_referer,
        sanitize_post_auth_navigate_path,
    };

    #[test]
    fn sanitize_keeps_safe_in_app_path_happy() {
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/user/profile".to_string())),
            "/user/profile"
        );
    }

    #[test]
    fn sanitize_rejects_protocol_relative_sad() {
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("//evil.example".to_string())),
            "/"
        );
    }

    #[test]
    fn sanitize_rejects_backslash_control_and_url_smuggle_sad() {
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/\\evil.example".to_string())),
            "/"
        );
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/\tevil.example".to_string())),
            "/"
        );
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/https://evil.example".to_string())),
            "/"
        );
        // Embedded whitespace (not only leading/trailing trim edges).
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/user/\x0bprofile".to_string())),
            "/"
        );
    }

    #[test]
    fn extra_safe_allows_query_on_path_happy() {
        assert!(is_extra_safe_in_app_path("/counter?tab=1"));
    }

    #[test]
    fn sanitize_keeps_gated_product_paths_happy() {
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/tag".to_string())),
            "/tag"
        );
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/tag/".to_string())),
            "/tag/"
        );
        assert_eq!(
            sanitize_post_auth_navigate_path(Some("/gate/auth-required".to_string())),
            "/gate/auth-required"
        );
    }

    #[test]
    fn retain_frozen_keeps_gated_path_when_live_url_becomes_signin_sad() {
        assert_eq!(retain_frozen_post_auth_referer("/tag/", "/"), "/tag/");
        assert_eq!(
            retain_frozen_post_auth_referer("/tag", "/auth/signin"),
            "/tag"
        );
        assert_eq!(
            retain_frozen_post_auth_referer("/", "/gate/auth-required"),
            "/gate/auth-required"
        );
        assert_eq!(retain_frozen_post_auth_referer("/", "/auth/signin"), "/");
    }

    #[test]
    fn post_signin_target_returns_to_deep_link_with_query_happy() {
        let deep = "/gate/auth-required?tab=history&x=1";
        assert_eq!(post_signin_target(deep, "/welcome"), Some(deep.to_string()));
        assert_eq!(post_signin_target(deep, "/"), Some(deep.to_string()));
    }

    #[test]
    fn post_signin_target_stays_when_already_on_referer_happy() {
        let deep = "/gate/auth-required?tab=history";
        assert_eq!(post_signin_target(deep, deep), None);
        assert_eq!(post_signin_target("/", "/"), None);
    }

    #[test]
    fn post_signin_target_carries_referer_through_confirm_account_happy() {
        assert_eq!(
            post_signin_target("/gate/auth-required?tab=history", "/user/confirm-account"),
            Some("/user/confirm-account?referer=%2Fgate%2Fauth-required%3Ftab%3Dhistory".into())
        );
        assert_eq!(
            post_signin_target("/tag", "/user/confirm-account?referer=%2Ftag"),
            None
        );
    }

    #[test]
    fn post_signin_target_keeps_server_choice_without_referer_happy() {
        assert_eq!(post_signin_target("/", "/welcome"), None);
        assert_eq!(post_signin_target("/", "/user/confirm-account"), None);
        assert_eq!(post_signin_target("/tag", "/user/profile"), None);
    }

    #[test]
    fn post_signin_target_ignores_off_origin_referers_sad() {
        for evil in [
            "//evil.example",
            "/\\evil.example",
            "/https://evil.example",
            "/\tevil.example",
            "/auth/signin",
            "https://evil.example/x",
        ] {
            assert_eq!(post_signin_target(evil, "/welcome"), None, "{evil}");
            assert_eq!(
                post_signin_target(evil, "/user/confirm-account"),
                None,
                "{evil}"
            );
        }
    }
}
