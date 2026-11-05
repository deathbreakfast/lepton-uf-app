//! Tell a person without an authenticator app why group access and tokens are locked.
//!
//! Adding a service user to a group and minting a token both ask for an authenticator
//! code, so someone who hasn't set one up can't finish either step.

use lepton_auth::actions::totp::get_totp_settings_status;
use leptos::prelude::*;
use uf_product::primitives::*;

use crate::paths;

/// True once the server says the viewer has no authenticator enrolled.
///
/// Stays false while loading and when the status call fails, so the page behaves as it
/// did before and the step-up dialog is still the backstop.
pub(super) fn use_authenticator_missing() -> Signal<bool> {
    let status = Resource::new(|| (), |()| async { get_totp_settings_status().await });
    Signal::derive(move || matches!(status.get(), Some(Ok(s)) if !s.totp_enabled))
}

/// Warning that `action` needs an authenticator app, with a link to set one up.
#[component]
pub(super) fn NeedsAuthenticator(action: &'static str) -> impl IntoView {
    view! {
        <div data-testid="service-user-needs-authenticator">
            <MessageBar intent=MessageBarIntent::Warning>
                "You need an authenticator app to "{action}". "
                <Link href=paths::ACCOUNT_SETTINGS>"Set one up in account settings"</Link>
                " and come back."
            </MessageBar>
        </div>
    }
}
