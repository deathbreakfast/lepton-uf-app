//! Email card: driver, SMTP details, From address, test send and save.

use lepton_auth::delivery_settings::{
    EmailCandidate, EmailDriverChoice, EmailSettingsView, WriteOnlySecret,
};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use uf_product::components::Subtitle2;
use uf_product::primitives::*;

use super::secret_field::SecretField;
use super::server::{save_email_settings, test_send_email};
use super::step_up::spawn_with_fresh_code;
use super::{friendly_error, Outcome};

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

fn typed(signal: RwSignal<String>) -> Option<WriteOnlySecret> {
    let value = signal.get_untracked();
    (!value.trim().is_empty()).then(|| WriteOnlySecret::new(value))
}

/// Email settings card.
#[component]
pub fn EmailCard(
    /// Current settings.
    view: EmailSettingsView,
    /// Bumped after a successful save so the page reloads.
    reload: RwSignal<u32>,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
) -> impl IntoView {
    let smtp_password_status = RwSignal::new(view.smtp_password.clone());
    let sendgrid_key_status = RwSignal::new(view.sendgrid_api_key.clone());
    let driver = RwSignal::new(Some(driver_name(view.driver).to_string()));
    let host = RwSignal::new(view.smtp_host.clone());
    let port = RwSignal::new(if view.smtp_port == 0 {
        "587".to_string()
    } else {
        view.smtp_port.to_string()
    });
    let tls = RwSignal::new(view.smtp_tls);
    let username = RwSignal::new(view.smtp_username.clone());
    let from_email = RwSignal::new(view.from_email.clone());
    let from_name = RwSignal::new(view.from_name.clone());
    let smtp_password = RwSignal::new(String::new());
    let sendgrid_key = RwSignal::new(String::new());
    let recipient = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    Effect::new(move |_| {
        if let Some(message) = error.get() {
            outcome.set(Some(Outcome::Failed(friendly_error(&message))));
        }
    });

    let candidate = move || EmailCandidate {
        driver: driver_from(&driver.get_untracked().unwrap_or_default()),
        smtp_host: host.get_untracked(),
        smtp_port: port.get_untracked().trim().parse().unwrap_or(0),
        smtp_tls: tls.get_untracked(),
        smtp_username: username.get_untracked(),
        from_email: from_email.get_untracked(),
        from_name: from_name.get_untracked(),
        smtp_password: typed(smtp_password),
        sendgrid_api_key: typed(sendgrid_key),
        test_recipient: recipient.get_untracked(),
    };

    let send_test = move |_: leptos::ev::MouseEvent| {
        busy.set(true);
        outcome.set(None);
        let c = candidate();
        spawn_local_scoped(async move {
            match test_send_email(c).await {
                Ok(()) => outcome.set(Some(Outcome::Done(
                    "Test message sent. Check the inbox you entered.".to_string(),
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
                smtp_password.set(String::new());
                sendgrid_key.set(String::new());
                outcome.set(Some(Outcome::Done("Email settings saved.".to_string())));
                reload.update(|n| *n += 1);
            }),
            move |code| {
                let c = c.clone();
                async move { save_email_settings(c, code).await }
            },
        );
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
                            <Flex vertical=true gap=FlexGap::Medium>
                                <Field label="SMTP host">
                                    <div data-testid="delivery-email-host">
                                        <Input bind=host appearance=InputAppearance::with_placeholder("smtp.example.com") />
                                    </div>
                                </Field>
                                <Field label="Port">
                                    <div data-testid="delivery-email-port">
                                        <Input bind=port />
                                    </div>
                                </Field>
                                <Switch bind=SwitchBind::from(tls) label="Use TLS" />
                                <Field label="Username">
                                    <div data-testid="delivery-email-username">
                                        <Input bind=username />
                                    </div>
                                </Field>
                                <SecretField label="Password" status=smtp_password_status.get() value=smtp_password test_id="delivery-email-password" />
                            </Flex>
                        </Show>
                        <Show when=move || driver.get().as_deref() == Some("sendgrid")>
                            <SecretField label="API key" status=sendgrid_key_status.get() value=sendgrid_key test_id="delivery-email-sendgrid-key" />
                        </Show>
                        <Show when=move || driver.get().as_deref() != Some("off")>
                            <Flex vertical=true gap=FlexGap::Medium>
                                <Field label="From address">
                                    <div data-testid="delivery-email-from">
                                        <Input bind=from_email appearance=InputAppearance::with_placeholder("cell@example.com") />
                                    </div>
                                </Field>
                                <Field label="From name">
                                    <div data-testid="delivery-email-from-name">
                                        <Input bind=from_name />
                                    </div>
                                </Field>
                                <Field label="Send test to">
                                    <div data-testid="delivery-email-recipient">
                                        <Input bind=recipient appearance=InputAppearance::with_placeholder("you@example.com") />
                                    </div>
                                </Field>
                            </Flex>
                        </Show>
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-email-outcome"))}
                    </Flex>
                </div>
            </CardContent>
            <CardFooter>
                <Flex gap=FlexGap::Small>
                    <Show when=move || driver.get().as_deref() != Some("off")>
                        <div data-testid="delivery-email-test">
                            <Button appearance=ButtonAppearance::Secondary disabled=Signal::derive(move || busy.get()) on_click=Callback::new(send_test)>
                                "Send test"
                            </Button>
                        </div>
                    </Show>
                    <div data-testid="delivery-email-save">
                        <Button appearance=ButtonAppearance::Primary on_click=Callback::new(save)>"Save email settings"</Button>
                    </div>
                </Flex>
            </CardFooter>
        </Card>
    }
}
