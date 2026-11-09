//! Text-message card: Twilio mode, SIDs, secret and a test send.

use lepton_auth::delivery_settings::{SmsCandidate, SmsMode, SmsSettingsView};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use uf_product::components::Subtitle2;
use uf_product::primitives::*;

use super::email::typed;
use super::secret_field::SecretRow;
use super::server::test_send_sms;
use super::{friendly_error, FormGrid, Outcome};

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

/// The text-message section's form values. The page owns them so one Save covers every card.
#[derive(Clone, Copy)]
pub struct SmsForm {
    mode: RwSignal<Option<String>>,
    account_sid: RwSignal<String>,
    api_key_sid: RwSignal<String>,
    verify_sid: RwSignal<String>,
    from: RwSignal<String>,
    secret: RwSignal<String>,
    recipient: RwSignal<String>,
}

impl SmsForm {
    /// Form filled from loaded values.
    pub fn new(base: &SmsCandidate) -> Self {
        Self {
            mode: RwSignal::new(Some(mode_name(base.mode).to_string())),
            account_sid: RwSignal::new(base.twilio_account_sid.clone()),
            api_key_sid: RwSignal::new(base.twilio_api_key_sid.clone()),
            verify_sid: RwSignal::new(base.twilio_verify_service_sid.clone()),
            from: RwSignal::new(base.twilio_from.clone()),
            secret: RwSignal::new(String::new()),
            recipient: RwSignal::new(String::new()),
        }
    }

    /// What the form holds now.
    pub fn candidate(&self) -> SmsCandidate {
        SmsCandidate {
            mode: mode_from(&self.mode.get_untracked().unwrap_or_default()),
            twilio_account_sid: self.account_sid.get_untracked(),
            twilio_api_key_sid: self.api_key_sid.get_untracked(),
            twilio_verify_service_sid: self.verify_sid.get_untracked(),
            twilio_from: self.from.get_untracked(),
            twilio_api_secret: typed(self.secret),
            test_recipient: self.recipient.get_untracked(),
        }
    }
}

/// Text-message settings card.
#[component]
pub fn SmsCard(
    /// Current settings, for the secret row.
    view: SmsSettingsView,
    /// Form values.
    form: SmsForm,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let mode = form.mode;
    let secret_status = view.twilio_api_secret;

    let send_test = move |_: leptos::ev::MouseEvent| {
        busy.set(true);
        outcome.set(None);
        let c = form.candidate();
        let to = c.test_recipient.clone();
        spawn_local_scoped(async move {
            match test_send_sms(c).await {
                Ok(()) => outcome.set(Some(Outcome::Done(format!("Test code sent to {to}.")))),
                Err(e) => outcome.set(Some(Outcome::Failed(friendly_error(&e.to_string())))),
            }
            busy.set(false);
        });
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
                                <Radio value="verify".to_string() label="Verify".to_string() />
                                <Radio value="messages".to_string() label="Messages".to_string() />
                            </RadioGroup>
                        </Field>
                        <Show when=move || mode.get().as_deref() != Some("off")>
                            <FormGrid>
                                <GridItem>
                                    <Field label="Account SID">
                                        <div data-testid="delivery-sms-account-sid">
                                            <Input bind=form.account_sid appearance=InputAppearance::with_placeholder("AC...") />
                                        </div>
                                    </Field>
                                </GridItem>
                                <GridItem>
                                    <Field label="API key SID">
                                        <div data-testid="delivery-sms-api-key-sid">
                                            <Input bind=form.api_key_sid appearance=InputAppearance::with_placeholder("SK...") />
                                        </div>
                                    </Field>
                                </GridItem>
                            </FormGrid>
                            <SecretRow label="API secret" status=secret_status.clone() value=form.secret test_id="delivery-sms-secret" />
                            <Show when=move || mode.get().as_deref() == Some("verify")>
                                <Field label="Verify service SID">
                                    <div data-testid="delivery-sms-verify-sid">
                                        <Input bind=form.verify_sid appearance=InputAppearance::with_placeholder("VA...") />
                                    </div>
                                </Field>
                            </Show>
                            <Show when=move || mode.get().as_deref() == Some("messages")>
                                <Field label="Sender number">
                                    <div data-testid="delivery-sms-from">
                                        <Input bind=form.from appearance=InputAppearance::with_placeholder("+15550100") />
                                    </div>
                                </Field>
                            </Show>
                            <Flex gap=FlexGap::Small align=FlexAlign::End wrap=FlexWrap::Wrap>
                                <Field label="Send test to">
                                    <div data-testid="delivery-sms-recipient">
                                        <Input bind=form.recipient appearance=InputAppearance::with_placeholder("+15550100") />
                                    </div>
                                </Field>
                                <div data-testid="delivery-sms-test">
                                    <Button appearance=ButtonAppearance::Secondary disabled=Signal::derive(move || busy.get()) on_click=Callback::new(send_test)>
                                        "Send test code"
                                    </Button>
                                </div>
                            </Flex>
                        </Show>
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-sms-outcome"))}
                    </Flex>
                </div>
            </CardContent>
        </Card>
    }
}
