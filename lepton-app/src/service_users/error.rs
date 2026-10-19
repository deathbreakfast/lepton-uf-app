//! Turn server errors from the service user pages into sentences people can act on.

/// The `reason_class=` value in a server error, or `""`.
fn reason_class(raw: &str) -> &str {
    raw.split("reason_class=")
        .nth(1)
        .and_then(|rest| rest.split([':', ' ']).next())
        .unwrap_or("")
}

/// Explain a service user page server error.
///
/// Server errors carry a stable `reason_class=` prefix. Anything unrecognised
/// gets a generic line, never the raw server text.
#[must_use]
pub fn friendly_error(raw: &str) -> &'static str {
    if raw.contains("STEP_UP:") {
        return "Confirm it's you with your authenticator code, then try again.";
    }
    match reason_class(raw) {
        "session" | "actor" => "Sign in again to manage service users.",
        "permission" => {
            "You can't create service users. Ask a super user to add you to the Service user creators group."
        }
        "not_configured" => "This site isn't set up for this yet. Ask a super user.",
        "handle" => "That handle can't be used. Handles use 3 to 32 lowercase letters, numbers or single dashes.",
        "handle_taken" => "Another service user already uses that handle. Pick another.",
        "quota" => "You have the most service users allowed. Delete one to make room.",
        "profile" => "Check the name and description. Names are 1 to 80 characters, descriptions up to 500.",
        "not_found" => "This service user doesn't exist, or it isn't yours to see.",
        "forbidden" => "You can't change this service user.",
        "suspended" => "This service user is paused. Resume it to give it tokens.",
        "token_limit" => "This service user already has 5 active tokens. Revoke one first.",
        "label" => "Give the token a label of up to 80 characters.",
        "no_access" => "Pick at least one thing the token can do.",
        "not_grantable" => {
            "You can't add service users to that group. Ask one of its owners or a super user."
        }
        _ => "Something went wrong. Nothing was changed. Try again.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_error_maps_every_reason_class() {
        let cases = [
            ("reason_class=session: x", "Sign in again"),
            ("reason_class=actor: only a signed-in user", "Sign in again"),
            (
                "reason_class=permission: not allowed",
                "Service user creators",
            ),
            ("reason_class=not_configured", "isn't set up"),
            (
                "reason_class=handle: invalid handle (reserved)",
                "single dashes",
            ),
            (
                "reason_class=handle_taken: handle already in use",
                "already uses",
            ),
            (
                "reason_class=quota: service user limit of 10 reached",
                "most service users allowed",
            ),
            ("reason_class=profile: invalid display_name", "1 to 80"),
            ("reason_class=not_found", "isn't yours"),
            ("reason_class=forbidden", "can't change"),
            ("reason_class=suspended", "paused"),
            (
                "reason_class=token_limit: at most 5 active tokens",
                "5 active tokens",
            ),
            ("reason_class=label", "label"),
            ("reason_class=no_access", "at least one"),
            (
                "reason_class=not_grantable: may not grant spectra-read",
                "owners",
            ),
            ("STEP_UP:step_up_required: window", "authenticator"),
        ];
        for (raw, expected) in cases {
            let text = friendly_error(raw);
            assert!(text.contains(expected), "{raw} -> {text}");
        }
    }

    #[test]
    fn friendly_error_unknown_text_is_generic() {
        let text = friendly_error("db exploded at /var/lib/secret.sqlite");
        assert!(text.starts_with("Something went wrong"), "{text}");
        assert!(!text.contains("sqlite"));
        assert!(friendly_error("reason_class=store: token_store").starts_with("Something"));
    }
}
