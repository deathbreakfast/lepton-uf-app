//! Shared TOTP UI helpers (step enum, clipboard, error text, QR SVG gate).

use leptos::prelude::ServerFnError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TotpUiStep {
    Idle,
    Scan,
    Confirm,
    Recovery,
    Enabled,
    Disable,
    RegenConfirm,
}

pub(super) use crate::clipboard::copy_text;

pub(super) fn server_err(err: &ServerFnError) -> String {
    err.to_string()
}

/// Tags allowed inside a TOTP QR SVG (`inner_html` sink).
const QR_SVG_ALLOWED_TAGS: &[&str] = &[
    "svg", "path", "rect", "circle", "line", "polyline", "polygon", "g", "defs", "title", "desc",
];

/// Attributes allowed on QR SVG elements (exact names, lowercased).
const QR_SVG_ALLOWED_ATTRS: &[&str] = &[
    "xmlns",
    "viewbox",
    "width",
    "height",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "d",
    "x",
    "y",
    "cx",
    "cy",
    "r",
    "points",
    "transform",
    "shape-rendering",
    "fill-rule",
    "clip-rule",
    "opacity",
    "role",
    "aria-hidden",
    "aria-label",
];

/// Allow only a minimal QR SVG markup string into `inner_html`.
///
/// Server-generated QR SVGs from lepton-auth are trusted today; this gate keeps the
/// XSS sink closed if a future response is tampered with or a dependency regresses.
/// Uses a tag/attribute allowlist (not a deny-list).
#[must_use]
pub(super) fn safe_totp_qr_svg(svg: &str) -> Option<&str> {
    let trimmed = svg.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return None;
    }
    if !trimmed.starts_with("<svg") {
        return None;
    }
    let lower_end = trimmed.to_ascii_lowercase();
    if !lower_end.ends_with("</svg>") {
        return None;
    }

    let mut rest = trimmed;
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        if after.is_empty() {
            return None;
        }
        // Comment or doctype — reject (QR SVGs do not need them).
        if after.starts_with('!') || after.starts_with('?') {
            return None;
        }
        let closing = after.starts_with('/');
        let tag_body = if closing { &after[1..] } else { after };
        let tag_name_end = tag_body
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(tag_body.len());
        if tag_name_end == 0 {
            return None;
        }
        let tag_name = tag_body[..tag_name_end].to_ascii_lowercase();
        if !QR_SVG_ALLOWED_TAGS.contains(&tag_name.as_str()) {
            return None;
        }

        let close_angle = after.find('>')?;
        let attrs_region = if closing {
            &after[1 + tag_name_end..close_angle]
        } else {
            &after[tag_name_end..close_angle]
        };
        if !closing && !attrs_are_allowed(attrs_region) {
            return None;
        }
        rest = &after[close_angle + 1..];
    }

    Some(trimmed)
}

fn attrs_are_allowed(attrs_region: &str) -> bool {
    let trimmed = attrs_region.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return true;
    }
    let mut chars = trimmed.char_indices().peekable();
    while let Some((_, c)) = chars.peek().copied() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let start = chars.peek().map(|(i, _)| *i).unwrap_or(0);
        while matches!(
            chars.peek(),
            Some((_, c)) if c.is_ascii_alphanumeric() || *c == '-' || *c == ':'
        ) {
            chars.next();
        }
        let end = chars.peek().map(|(i, _)| *i).unwrap_or(trimmed.len());
        let name = trimmed[start..end].to_ascii_lowercase();
        if name.is_empty() || name.starts_with("on") {
            return false;
        }
        // Reject namespaced attrs other than plain `xmlns` (no xlink:href, xmlns:xlink).
        if name.contains(':') {
            return false;
        }
        if !QR_SVG_ALLOWED_ATTRS.contains(&name.as_str()) {
            return false;
        }

        while matches!(chars.peek(), Some((_, c)) if c.is_whitespace()) {
            chars.next();
        }
        if matches!(chars.peek(), Some((_, '='))) {
            chars.next();
            while matches!(chars.peek(), Some((_, c)) if c.is_whitespace()) {
                chars.next();
            }
            let Some((_, quote)) = chars.next() else {
                return false;
            };
            if quote != '"' && quote != '\'' {
                return false;
            }
            let mut value = String::new();
            loop {
                let Some((_, ch)) = chars.next() else {
                    return false;
                };
                if ch == quote {
                    break;
                }
                value.push(ch);
            }
            let value_lower = value.to_ascii_lowercase();
            if value_lower.contains("javascript:")
                || value_lower.contains("data:text/html")
                || value_lower.contains("data:image/svg")
            {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::safe_totp_qr_svg;

    #[test]
    fn safe_totp_qr_svg_allows_plain_svg_happy() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#;
        assert_eq!(safe_totp_qr_svg(svg), Some(svg));
    }

    #[test]
    fn safe_totp_qr_svg_allows_path_qr_happy() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 21 21" shape-rendering="crispEdges"><path fill="#000000" d="M0 0h7v7H0z"/></svg>"##;
        assert_eq!(safe_totp_qr_svg(svg), Some(svg));
    }

    #[test]
    fn safe_totp_qr_svg_rejects_script_and_handlers_sad() {
        assert!(safe_totp_qr_svg(r"<svg><script>alert(1)</script></svg>").is_none());
        assert!(safe_totp_qr_svg(r#"<svg onload="alert(1)"></svg>"#).is_none());
        assert!(safe_totp_qr_svg(r"<div>not svg</div>").is_none());
        assert!(safe_totp_qr_svg("").is_none());
    }

    #[test]
    fn safe_totp_qr_svg_rejects_foreign_and_use_sad() {
        assert!(safe_totp_qr_svg(r"<svg><foreignObject></foreignObject></svg>").is_none());
        assert!(safe_totp_qr_svg(r##"<svg><use href="#x"/></svg>"##).is_none());
        assert!(safe_totp_qr_svg(r#"<svg><a href="javascript:alert(1)"></a></svg>"#).is_none());
    }

    #[test]
    fn safe_totp_qr_svg_rejects_onmouseover_sad() {
        assert!(safe_totp_qr_svg(r#"<svg onmouseover="alert(1)"></svg>"#).is_none());
        assert!(
            safe_totp_qr_svg(r#"<svg><rect onfocus="alert(1)" width="1" height="1"/></svg>"#)
                .is_none()
        );
    }
}
