//! "Sign-in and delivery" settings page.
//!
//! Lets a delivery admin choose how Lepton sends email and text messages,
//! which OAuth providers are offered, and when to rotate the authenticator
//! seal key. Secrets are write-only: the page shows set or not set and when
//! each last changed, never the value; a new value is typed into a one-field
//! sheet and kept in the browser only until Save. One Save at the foot sends
//! every changed section with one fresh authenticator code, and a section
//! whose test send fails is not saved. Operators without an authenticator are
//! asked to turn on two-factor sign-in first.
//!
//! The page works only on hosts that provide a
//! [`DeliverySettingsBackend`](lepton_auth::delivery_settings) through Leptos
//! context; on others the server functions answer that settings are not set up.

mod changes;
mod email;
mod oauth;
mod secret_field;
mod server;
mod sms;
mod step_up;
mod totp_key;

use lepton_auth::delivery_settings::DeliverySettingsView;
use leptos::prelude::*;
use uf_product::components::{Body1, Caption1, ContentContainer, SkeletonItemSize, Title3};
use uf_product::primitives::*;
use uf_product::AdaptiveSheet;

use crate::paths;
use crate::service_users::use_authenticator_missing;
use changes::{
    email_baseline, email_change, oauth_baseline, oauth_change, sms_baseline, sms_change,
};
use email::{EmailCard, EmailForm};
use oauth::{OAuthCard, OAuthForm};
use sms::{SmsCard, SmsForm};
use step_up::spawn_with_fresh_code;
use totp_key::TotpKeyCard;

pub use changes::{DeliveryChanges, DeliverySaveReport, SectionOutcome};
pub use server::{
    get_delivery_settings, rotate_totp_seal_key, save_delivery_settings, save_email_settings,
    save_oauth_settings, save_sms_settings, test_send_email, test_send_sms,
};

/// Result line shown inside a card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The action worked.
    Done(String),
    /// The action failed; text is safe to show.
    Failed(String),
    /// Nothing happened, and the text says why.
    Note(String),
}

impl Outcome {
    /// Render as a `MessageBar` inside a `data-testid` wrapper.
    pub fn into_message_bar(self, test_id: &'static str) -> impl IntoView {
        let (intent, text) = match self {
            Self::Done(t) => (MessageBarIntent::Success, t),
            Self::Failed(t) => (MessageBarIntent::Error, t),
            Self::Note(t) => (MessageBarIntent::Info, t),
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

/// Result line for one section after Save, or `None` when it was not sent.
fn section_message(outcome: &SectionOutcome, saved: &str) -> Option<Outcome> {
    match outcome {
        SectionOutcome::Unchanged => None,
        SectionOutcome::Saved => Some(Outcome::Done(saved.to_string())),
        SectionOutcome::Failed(raw) => Some(Outcome::Failed(friendly_error(raw))),
    }
}

/// Two-column `Grid` of form fields that drops to one column below Md.
#[component]
fn FormGrid(children: ChildrenFn) -> impl IntoView {
    let narrow = use_breakpoint_down(Breakpoint::Md);
    move || {
        let cols = if narrow.get() { 1 } else { 2 };
        let children = children.clone();
        view! { <Grid config=GridConfig::with_gaps(cols, 16, 12)>{children()}</Grid> }
    }
}

/// Result lines, owned by the page so a save's reload does not wipe them.
#[derive(Clone, Copy)]
struct Outcomes {
    email: RwSignal<Option<Outcome>>,
    sms: RwSignal<Option<Outcome>>,
    oauth: RwSignal<Option<Outcome>>,
    totp: RwSignal<Option<Outcome>>,
    save: RwSignal<Option<Outcome>>,
}

impl Outcomes {
    fn new() -> Self {
        Self {
            email: RwSignal::new(None),
            sms: RwSignal::new(None),
            oauth: RwSignal::new(None),
            totp: RwSignal::new(None),
            save: RwSignal::new(None),
        }
    }
}

/// Shown instead of the code prompt when the viewer has no authenticator yet.
#[component]
fn NeedsTwoFactorSheet(open: RwSignal<bool>) -> impl IntoView {
    view! {
        <AdaptiveSheet
            open=open
            title=|| "Turn on two-factor sign-in"
            testid="delivery-needs-two-factor"
            size=DrawerSize::Small
            actions=move || view! {
                <Button appearance=ButtonAppearance::Secondary on_click=Callback::new(move |_| open.set(false))>
                    "Cancel"
                </Button>
                <div data-testid="delivery-needs-two-factor-link">
                    <Link href=paths::ACCOUNT_SETTINGS>"Open account security"</Link>
                </div>
            }
        >
            <Body1>
                "Turn on two-factor sign-in to save changes here. Each change is confirmed with a code from your authenticator app."
            </Body1>
        </AdaptiveSheet>
    }
}

/// The four cards and the Save button, filled from one load of the settings.
#[component]
fn DeliveryForm(
    view: DeliverySettingsView,
    reload: RwSignal<u32>,
    outcomes: Outcomes,
    authenticator_missing: Signal<bool>,
    needs_two_factor: RwSignal<bool>,
) -> impl IntoView {
    let email_base = email_baseline(&view.email);
    let sms_base = sms_baseline(&view.sms);
    let oauth_base = oauth_baseline(&view.oauth);
    let email = EmailForm::new(&email_base);
    let sms = SmsForm::new(&sms_base);
    let oauth = OAuthForm::new(&oauth_base);
    let bases = StoredValue::new((email_base, sms_base, oauth_base));
    let error = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        if let Some(message) = error.get() {
            outcomes
                .save
                .set(Some(Outcome::Failed(friendly_error(&message))));
        }
    });

    let save = move |_: leptos::ev::MouseEvent| {
        outcomes.save.set(None);
        if authenticator_missing.get_untracked() {
            needs_two_factor.set(true);
            return;
        }
        let changes = bases.with_value(|(e, s, o)| DeliveryChanges {
            email: email_change(e, email.candidate()),
            sms: sms_change(s, sms.candidate()),
            oauth: oauth_change(o, oauth.candidate()),
        });
        if changes.is_empty() {
            outcomes.save.set(Some(Outcome::Note(
                "Nothing has changed, so there is nothing to save.".to_string(),
            )));
            return;
        }
        spawn_with_fresh_code(
            error,
            Callback::new(move |report: DeliverySaveReport| {
                for (signal, outcome, saved) in [
                    (outcomes.email, &report.email, "Email settings saved."),
                    (outcomes.sms, &report.sms, "Text-message settings saved."),
                    (
                        outcomes.oauth,
                        &report.oauth,
                        "Sign-in provider settings saved.",
                    ),
                ] {
                    if let Some(message) = section_message(outcome, saved) {
                        signal.set(Some(message));
                    }
                }
                if report.any_saved() {
                    reload.update(|n| *n += 1);
                }
            }),
            move |code| {
                let changes = changes.clone();
                async move { save_delivery_settings(changes, code).await }
            },
        );
    };

    view! {
        <Flex vertical=true gap=FlexGap::Large>
            <EmailCard view=view.email.clone() form=email outcome=outcomes.email />
            <SmsCard view=view.sms.clone() form=sms outcome=outcomes.sms />
            <OAuthCard view=view.oauth.clone() form=oauth outcome=outcomes.oauth />
            <TotpKeyCard
                view=view.totp_key.clone()
                reload=reload
                outcome=outcomes.totp
                authenticator_missing=authenticator_missing
                needs_two_factor=needs_two_factor
            />
            <Flex justify=FlexJustify::End align=FlexAlign::Center gap=FlexGap::Medium wrap=FlexWrap::Wrap>
                <div data-testid="delivery-save-hint">
                    <Caption1>
                        {move || if authenticator_missing.get() {
                            "Needs two-factor sign-in"
                        } else {
                            "Asks for two-factor"
                        }}
                    </Caption1>
                </div>
                <div data-testid="delivery-save">
                    <Button appearance=ButtonAppearance::Primary on_click=Callback::new(save)>"Save"</Button>
                </div>
            </Flex>
            {move || outcomes.save.get().map(|o| o.into_message_bar("delivery-save-outcome"))}
        </Flex>
    }
}

/// `/user/delivery` page.
///
/// Wrap in [`uf_product::routes::RequireAuthenticated`] at the route.
#[component]
pub fn DeliverySettingsPage() -> impl IntoView {
    let reload = RwSignal::new(0u32);
    let outcomes = Outcomes::new();
    let authenticator_missing = use_authenticator_missing();
    let needs_two_factor = RwSignal::new(false);
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
                            <DeliveryForm
                                view=view
                                reload=reload
                                outcomes=outcomes
                                authenticator_missing=authenticator_missing
                                needs_two_factor=needs_two_factor
                            />
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
            <NeedsTwoFactorSheet open=needs_two_factor />
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
    fn each_section_reports_saved_failed_or_nothing() {
        assert_eq!(section_message(&SectionOutcome::Unchanged, "Saved."), None);
        assert_eq!(
            section_message(&SectionOutcome::Saved, "Saved."),
            Some(Outcome::Done("Saved.".to_string()))
        );
        let failed = section_message(
            &SectionOutcome::Failed(
                "reason_class=test_send_failed: email test send failed (unauthorized)".into(),
            ),
            "Saved.",
        );
        assert!(matches!(failed, Some(Outcome::Failed(t)) if t.contains("Nothing was saved")));
    }

    #[test]
    fn bad_authenticator_code_asks_for_the_next_code() {
        let text = friendly_error("STEP_UP:step_up_invalid: invalid or reused totp code");
        assert!(text.contains("next code"));
    }
}
