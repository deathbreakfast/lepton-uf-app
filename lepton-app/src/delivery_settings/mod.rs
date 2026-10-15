//! "Sign-in and delivery" settings page.
//!
//! Lets a delivery admin choose how Lepton sends email and text messages,
//! which OAuth providers are offered, and when to rotate the authenticator
//! seal key. Secrets are write-only: the page shows set or not set and when
//! each last changed, never the value. Every write asks for a fresh
//! authenticator code, and a failed test send changes nothing.
//!
//! The page works only on hosts that provide a
//! [`DeliverySettingsBackend`](lepton_auth::delivery_settings) through Leptos
//! context; on others the server functions answer that settings are not set up.

mod email;
mod oauth;
mod secret_field;
mod server;
mod sms;
mod step_up;
mod totp_key;

use leptos::prelude::*;
use uf_product::components::{ContentContainer, SkeletonItemSize, Title3};
use uf_product::primitives::*;

use email::EmailCard;
use oauth::OAuthCard;
use sms::SmsCard;
use totp_key::TotpKeyCard;

pub use server::{
    get_delivery_settings, rotate_totp_seal_key, save_email_settings, save_oauth_settings,
    save_sms_settings, test_send_email, test_send_sms,
};

/// Result line shown inside a card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The action worked.
    Done(String),
    /// The action failed; text is safe to show.
    Failed(String),
}

impl Outcome {
    /// Render as a `MessageBar` inside a `data-testid` wrapper.
    pub fn into_message_bar(self, test_id: &'static str) -> impl IntoView {
        let (intent, text) = match self {
            Self::Done(t) => (MessageBarIntent::Success, t),
            Self::Failed(t) => (MessageBarIntent::Error, t),
        };
        view! {
            <div data-testid=test_id>
                <MessageBar intent=intent>{text}</MessageBar>
            </div>
        }
    }
}

/// Turn a server error into text an operator can act on.
///
/// Server errors carry a stable `reason_class=` prefix; anything unrecognised
/// gets a generic line rather than raw server text.
#[must_use]
pub fn friendly_error(raw: &str) -> String {
    let class = raw
        .split("reason_class=")
        .nth(1)
        .and_then(|rest| rest.split([':', ' ']).next())
        .unwrap_or("");
    if raw.contains("STEP_UP:") {
        return "That authenticator code was not accepted. Try the next code.".to_string();
    }
    match class {
        "permission_denied" => {
            "You don't have permission to manage sign-in and delivery.".to_string()
        }
        "step_up_required" => "Enter a fresh authenticator code to save changes.".to_string(),
        "invalid" => {
            let detail = raw
                .split("reason_class=invalid:")
                .nth(1)
                .unwrap_or("")
                .trim();
            format!("Check the form: {detail}.")
        }
        "test_send_failed" => {
            let why = if raw.contains("(unauthorized)") {
                "the provider rejected the credentials"
            } else if raw.contains("(unverified_sender)") {
                "the provider does not accept that sender"
            } else if raw.contains("(invalid_recipient)") {
                "the destination is not valid"
            } else if raw.contains("(network)") {
                "the provider could not be reached"
            } else {
                "the provider refused the message"
            };
            format!("The test message was not sent: {why}. Nothing was saved.")
        }
        "save_not_applied" => {
            "A secret changed but the settings did not save. Save again to finish.".to_string()
        }
        "secret_store" => "The secret vault refused the change.".to_string(),
        "not_configured" => {
            "Sign-in and delivery settings are not set up on this host.".to_string()
        }
        _ => "Something went wrong. Nothing was saved.".to_string(),
    }
}

/// True when a load error means the viewer lacks the permission.
fn is_denied(raw: &str) -> bool {
    // The server-function gate answers `permission_denied::<name>`; the backend
    // answers `reason_class=permission_denied`. A failed check is a load error.
    raw.contains("permission_denied")
}

/// `/user/delivery` page.
///
/// Wrap in [`uf_product::routes::RequireAuthenticated`] at the route.
#[component]
pub fn DeliverySettingsPage() -> impl IntoView {
    let reload = RwSignal::new(0u32);
    // Result lines live here so a save's reload does not wipe them.
    let email_outcome = RwSignal::new(None::<Outcome>);
    let sms_outcome = RwSignal::new(None::<Outcome>);
    let oauth_outcome = RwSignal::new(None::<Outcome>);
    let totp_outcome = RwSignal::new(None::<Outcome>);
    let settings = Resource::new(move || reload.get(), |_| get_delivery_settings());

    view! {
        <ContentContainer max_width="900px" data_testid="delivery-settings-container">
            <Flex vertical=true gap=FlexGap::Large>
                <Title3>"Sign-in and delivery"</Title3>
                <Suspense fallback=move || view! {
                    <Skeleton>
                        <SkeletonItem size=Signal::from(SkeletonItemSize::S48) />
                    </Skeleton>
                }>
                    {move || settings.get().map(|result| match result {
                        Ok(view) => view! {
                            <Flex vertical=true gap=FlexGap::Large>
                                <EmailCard view=view.email.clone() reload=reload outcome=email_outcome />
                                <SmsCard view=view.sms.clone() reload=reload outcome=sms_outcome />
                                <OAuthCard view=view.oauth.clone() reload=reload outcome=oauth_outcome />
                                <TotpKeyCard view=view.totp_key.clone() reload=reload outcome=totp_outcome />
                            </Flex>
                        }.into_any(),
                        Err(e) if is_denied(&e.to_string()) => view! {
                            <div data-testid="delivery-denied">
                                <MessageBar intent=MessageBarIntent::Error>
                                    "You don't have permission to manage sign-in and delivery. Ask a super user to add you to lepton.delivery.admins."
                                </MessageBar>
                            </div>
                        }.into_any(),
                        Err(e) => view! {
                            <div data-testid="delivery-load-error">
                                <MessageBar intent=MessageBarIntent::Error>
                                    {friendly_error(&e.to_string())}
                                </MessageBar>
                            </div>
                        }.into_any(),
                    })}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn denied_is_recognized_from_the_gate_payload_and_the_backend_class() {
        assert!(is_denied(
            "ServerError|permission_denied::LeptonManageDelivery"
        ));
        assert!(!is_denied(
            "ServerError|permission_check_failed::LeptonManageDelivery"
        ));
        assert!(is_denied("reason_class=permission_denied: not allowed"));
        assert!(!is_denied("reason_class=store: settings store failed"));
    }

    #[test]
    fn test_send_failure_explains_the_cause_and_says_nothing_was_saved() {
        let text =
            friendly_error("reason_class=test_send_failed: email test send failed (unauthorized)");
        assert!(text.contains("rejected the credentials"));
        assert!(text.contains("Nothing was saved"));
    }

    #[test]
    fn unknown_server_text_is_never_echoed_to_the_page() {
        let text = friendly_error("boom: password=hunter2 host=10.0.0.5");
        assert!(!text.contains("hunter2"));
        assert!(!text.contains("10.0.0.5"));
    }

    #[test]
    fn bad_authenticator_code_asks_for_the_next_code() {
        let text = friendly_error("STEP_UP:step_up_invalid: invalid or reused totp code");
        assert!(text.contains("next code"));
    }
}
