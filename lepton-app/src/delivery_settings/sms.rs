//! Text-message card: Twilio mode, SIDs, secret, test send and save.

use lepton_auth::delivery_settings::{SmsCandidate, SmsMode, SmsSettingsView, WriteOnlySecret};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use uf_product::components::Subtitle2;
use uf_product::primitives::*;

use super::secret_field::SecretField;
use super::server::{save_sms_settings, test_send_sms};
use super::step_up::spawn_with_fresh_code;
use super::{friendly_error, Outcome};

fn mode_name(m: SmsMode) -> &'static str {
    match m {
        SmsMode::Off => "off",
        SmsMode::Verify => "verify",
        SmsMode::Messages => "messages",
    }
}

fn mode_from(name: &str) -> SmsMode {
    match name {
        "verify" => SmsMode::Verify,
        "messages" => SmsMode::Messages,
        _ => SmsMode::Off,
    }
}

/// Text-message settings card.
#[component]
pub fn SmsCard(
    /// Current settings.
    view: SmsSettingsView,
    /// Bumped after a successful save so the page reloads.
    reload: RwSignal<u32>,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
) -> impl IntoView {
    let secret_status = RwSignal::new(view.twilio_api_secret.clone());
    let mode = RwSignal::new(Some(mode_name(view.mode).to_string()));
    let account_sid = RwSignal::new(view.twilio_account_sid.clone());
    let api_key_sid = RwSignal::new(view.twilio_api_key_sid.clone());
    let verify_sid = RwSignal::new(view.twilio_verify_service_sid.clone());
    let from = RwSignal::new(view.twilio_from.clone());
    let secret = RwSignal::new(String::new());
    let recipient = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(message) = error.get() {
            outcome.set(Some(Outcome::Failed(friendly_error(&message))));
        }
    });

    let candidate = move || {
        let typed = secret.get_untracked();
        SmsCandidate {
            mode: mode_from(&mode.get_untracked().unwrap_or_default()),
            twilio_account_sid: account_sid.get_untracked(),
            twilio_api_key_sid: api_key_sid.get_untracked(),
            twilio_verify_service_sid: verify_sid.get_untracked(),
            twilio_from: from.get_untracked(),
            twilio_api_secret: (!typed.trim().is_empty()).then(|| WriteOnlySecret::new(typed)),
            test_recipient: recipient.get_untracked(),
        }
    };

    let send_test = move |_: leptos::ev::MouseEvent| {
        busy.set(true);
        outcome.set(None);
        let c = candidate();
        spawn_local_scoped(async move {
            match test_send_sms(c).await {
                Ok(()) => outcome.set(Some(Outcome::Done(
                    "Test code sent. Check the phone you entered.".to_string(),
                ))),
                Err(e) => outcome.set(Some(Outcome::Failed(friendly_error(&e.to_string())))),
            }
            busy.set(false);
        });
    };

    let save = move |_: leptos::ev::MouseEvent| {
        outcome.set(None);
        let c = candidate();
        spawn_with_fresh_code(
            error,
            Callback::new(move |()| {
                secret.set(String::new());
                outcome.set(Some(Outcome::Done(
                    "Text-message settings saved.".to_string(),
                )));
                reload.update(|n| *n += 1);
            }),
            move |code| {
                let c = c.clone();
                async move { save_sms_settings(c, code).await }
            },
        );
    };

    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Text messages (Twilio)"</Subtitle2>
            </CardHeader>
            <CardContent>
                <div data-testid="delivery-sms-card">
                    <Flex vertical=true gap=FlexGap::Medium>
                        <Field label="Send text messages with">
                            <RadioGroup bind=RadioGroupBind::from(mode)>
                                <Radio value="off".to_string() label="Off".to_string() />
                                <Radio value="verify".to_string() label="Twilio Verify".to_string() />
                                <Radio value="messages".to_string() label="Twilio Messages".to_string() />
                            </RadioGroup>
                        </Field>
                        <Show when=move || mode.get().as_deref() != Some("off")>
                            <Flex vertical=true gap=FlexGap::Medium>
                                <Field label="Account SID">
                                    <div data-testid="delivery-sms-account-sid">
                                        <Input bind=account_sid appearance=InputAppearance::with_placeholder("AC...") />
                                    </div>
                                </Field>
                                <Field label="API key SID">
                                    <div data-testid="delivery-sms-api-key-sid">
                                        <Input bind=api_key_sid appearance=InputAppearance::with_placeholder("SK...") />
                                    </div>
                                </Field>
                                <SecretField label="API secret" status=secret_status.get() value=secret test_id="delivery-sms-secret" />
                                <Show when=move || mode.get().as_deref() == Some("verify")>
                                    <Field label="Verify service SID">
                                        <div data-testid="delivery-sms-verify-sid">
                                            <Input bind=verify_sid appearance=InputAppearance::with_placeholder("VA...") />
                                        </div>
                                    </Field>
                                </Show>
                                <Show when=move || mode.get().as_deref() == Some("messages")>
                                    <Field label="Sender number">
                                        <div data-testid="delivery-sms-from">
                                            <Input bind=from appearance=InputAppearance::with_placeholder("+15550100") />
                                        </div>
                                    </Field>
                                </Show>
                                <Field label="Send test to">
                                    <div data-testid="delivery-sms-recipient">
                                        <Input bind=recipient appearance=InputAppearance::with_placeholder("+15550100") />
                                    </div>
                                </Field>
                            </Flex>
                        </Show>
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-sms-outcome"))}
                    </Flex>
                </div>
            </CardContent>
            <CardFooter>
                <Flex gap=FlexGap::Small>
                    <Show when=move || mode.get().as_deref() != Some("off")>
                        <div data-testid="delivery-sms-test">
                            <Button appearance=ButtonAppearance::Secondary disabled=Signal::derive(move || busy.get()) on_click=Callback::new(send_test)>
                                "Send test code"
                            </Button>
                        </div>
                    </Show>
                    <div data-testid="delivery-sms-save">
                        <Button appearance=ButtonAppearance::Primary on_click=Callback::new(save)>"Save text-message settings"</Button>
                    </div>
                </Flex>
            </CardFooter>
        </Card>
    }
}
