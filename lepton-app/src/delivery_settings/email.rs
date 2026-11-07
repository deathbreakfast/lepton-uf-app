//! Email card: driver, SMTP details, From address and a test send.

use lepton_auth::delivery_settings::{
    EmailCandidate, EmailDriverChoice, EmailSettingsView, WriteOnlySecret,
};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use uf_product::components::Subtitle2;
use uf_product::primitives::*;

use super::secret_field::SecretRow;
use super::server::test_send_email;
use super::{friendly_error, FormGrid, Outcome};

fn driver_name(d: EmailDriverChoice) -> &'static str {
    match d {
        EmailDriverChoice::Off => "off",
        EmailDriverChoice::Smtp => "smtp",
        EmailDriverChoice::SendGrid => "sendgrid",
        EmailDriverChoice::DirectMx => "direct_mx",
    }
}

fn driver_from(name: &str) -> EmailDriverChoice {
    match name {
        "smtp" => EmailDriverChoice::Smtp,
        "sendgrid" => EmailDriverChoice::SendGrid,
        "direct_mx" => EmailDriverChoice::DirectMx,
        _ => EmailDriverChoice::Off,
    }
}

/// A typed secret, or `None` to keep the stored one.
pub(super) fn typed(signal: RwSignal<String>) -> Option<WriteOnlySecret> {
    let value = signal.get_untracked();
    (!value.trim().is_empty()).then(|| WriteOnlySecret::new(value))
}

/// The email section's form values. The page owns them so one Save covers every card.
#[derive(Clone, Copy)]
pub struct EmailForm {
    driver: RwSignal<Option<String>>,
    host: RwSignal<String>,
    port: RwSignal<String>,
    tls: RwSignal<bool>,
    username: RwSignal<String>,
    from_email: RwSignal<String>,
    from_name: RwSignal<String>,
    smtp_password: RwSignal<String>,
    sendgrid_key: RwSignal<String>,
    recipient: RwSignal<String>,
}

impl EmailForm {
    /// Form filled from loaded values.
    pub fn new(base: &EmailCandidate) -> Self {
        Self {
            driver: RwSignal::new(Some(driver_name(base.driver).to_string())),
            host: RwSignal::new(base.smtp_host.clone()),
            port: RwSignal::new(base.smtp_port.to_string()),
            tls: RwSignal::new(base.smtp_tls),
            username: RwSignal::new(base.smtp_username.clone()),
            from_email: RwSignal::new(base.from_email.clone()),
            from_name: RwSignal::new(base.from_name.clone()),
            smtp_password: RwSignal::new(String::new()),
            sendgrid_key: RwSignal::new(String::new()),
            recipient: RwSignal::new(String::new()),
        }
    }

    /// What the form holds now.
    pub fn candidate(&self) -> EmailCandidate {
        EmailCandidate {
            driver: driver_from(&self.driver.get_untracked().unwrap_or_default()),
            smtp_host: self.host.get_untracked(),
            smtp_port: self.port.get_untracked().trim().parse().unwrap_or(0),
            smtp_tls: self.tls.get_untracked(),
            smtp_username: self.username.get_untracked(),
            from_email: self.from_email.get_untracked(),
            from_name: self.from_name.get_untracked(),
            smtp_password: typed(self.smtp_password),
            sendgrid_api_key: typed(self.sendgrid_key),
            test_recipient: self.recipient.get_untracked(),
        }
    }
}

/// Email settings card.
#[component]
pub fn EmailCard(
    /// Current settings, for the secret rows.
    view: EmailSettingsView,
    /// Form values.
    form: EmailForm,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let driver = form.driver;
    let password_status = view.smtp_password;
    let sendgrid_status = view.sendgrid_api_key;

    let send_test = move |_: leptos::ev::MouseEvent| {
        busy.set(true);
        outcome.set(None);
        let c = form.candidate();
        let to = c.test_recipient.clone();
        spawn_local_scoped(async move {
            match test_send_email(c).await {
                Ok(()) => outcome.set(Some(Outcome::Done(format!("Test email sent to {to}.")))),
                Err(e) => outcome.set(Some(Outcome::Failed(friendly_error(&e.to_string())))),
            }
            busy.set(false);
        });
    };

    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Email"</Subtitle2>
            </CardHeader>
            <CardContent>
                <div data-testid="delivery-email-card">
                    <Flex vertical=true gap=FlexGap::Medium>
                        <Field label="Send email with">
                            <RadioGroup bind=RadioGroupBind::from(driver)>
                                <Radio value="off".to_string() label="Off".to_string() />
                                <Radio value="smtp".to_string() label="SMTP".to_string() />
                                <Radio value="sendgrid".to_string() label="SendGrid".to_string() />
                                <Radio value="direct_mx".to_string() label="Direct to recipient".to_string() />
                            </RadioGroup>
                        </Field>
                        <Show when=move || driver.get().as_deref() == Some("smtp")>
                            <FormGrid>
                                <GridItem>
                                    <Field label="SMTP host">
                                        <div data-testid="delivery-email-host">
                                            <Input bind=form.host appearance=InputAppearance::with_placeholder("smtp.example.com") />
                                        </div>
                                    </Field>
                                </GridItem>
                                <GridItem>
                                    <Field label="Port">
                                        <div data-testid="delivery-email-port">
                                            <Input bind=form.port />
                                        </div>
                                    </Field>
                                </GridItem>
                                <GridItem>
                                    <Field label="Username">
                                        <div data-testid="delivery-email-username">
                                            <Input bind=form.username />
                                        </div>
                                    </Field>
                                </GridItem>
                                <GridItem>
                                    <Switch bind=SwitchBind::from(form.tls) label="Use TLS" />
                                </GridItem>
                            </FormGrid>
                            <SecretRow label="Password" status=password_status.clone() value=form.smtp_password test_id="delivery-email-password" />
                        </Show>
                        <Show when=move || driver.get().as_deref() == Some("sendgrid")>
                            <SecretRow label="API key" status=sendgrid_status.clone() value=form.sendgrid_key test_id="delivery-email-sendgrid-key" />
                        </Show>
                        <Show when=move || driver.get().as_deref() != Some("off")>
                            <FormGrid>
                                <GridItem>
                                    <Field label="From address">
                                        <div data-testid="delivery-email-from">
                                            <Input bind=form.from_email appearance=InputAppearance::with_placeholder("cell@example.com") />
                                        </div>
                                    </Field>
                                </GridItem>
                                <GridItem>
                                    <Field label="From name">
                                        <div data-testid="delivery-email-from-name">
                                            <Input bind=form.from_name />
                                        </div>
                                    </Field>
                                </GridItem>
                            </FormGrid>
                            <Flex gap=FlexGap::Small align=FlexAlign::End wrap=FlexWrap::Wrap>
                                <Field label="Send test to">
                                    <div data-testid="delivery-email-recipient">
                                        <Input bind=form.recipient appearance=InputAppearance::with_placeholder("you@example.com") />
                                    </div>
                                </Field>
                                <div data-testid="delivery-email-test">
                                    <Button appearance=ButtonAppearance::Secondary disabled=Signal::derive(move || busy.get()) on_click=Callback::new(send_test)>
                                        "Send test"
                                    </Button>
                                </div>
                            </Flex>
                        </Show>
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-email-outcome"))}
                    </Flex>
                </div>
            </CardContent>
        </Card>
    }
}
