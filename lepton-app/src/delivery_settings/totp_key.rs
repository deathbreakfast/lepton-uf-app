//! Authenticator key card: current version and rotation.

use lepton_auth::delivery_settings::TotpKeyView;
use leptos::prelude::*;
use uf_product::components::{Body1, Subtitle2};
use uf_product::primitives::*;

use super::server::rotate_totp_seal_key;
use super::step_up::spawn_with_fresh_code;
use super::{friendly_error, Outcome};

/// Authenticator seal key card.
#[component]
pub fn TotpKeyCard(
    /// Current key state.
    view: TotpKeyView,
    /// Bumped after a successful rotation so the page reloads.
    reload: RwSignal<u32>,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
) -> impl IntoView {
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if let Some(message) = error.get() {
            outcome.set(Some(Outcome::Failed(friendly_error(&message))));
        }
    });

    let rotate = move |_: leptos::ev::MouseEvent| {
        outcome.set(None);
        spawn_with_fresh_code(
            error,
            Callback::new(move |()| {
                outcome.set(Some(Outcome::Done(
                    "Authenticator key rotated.".to_string(),
                )));
                reload.update(|n| *n += 1);
            }),
            move |code| async move { rotate_totp_seal_key(code).await },
        );
    };

    let status = if view.version == 0 {
        "No key yet".to_string()
    } else {
        match view.rotated_at {
            Some(at) => format!(
                "Version {}, rotated {}",
                view.version,
                at.format("%Y-%m-%d")
            ),
            None => format!("Version {}", view.version),
        }
    };

    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Authenticator key"</Subtitle2>
            </CardHeader>
            <CardContent>
                <div data-testid="delivery-totp-card">
                    <Flex vertical=true gap=FlexGap::Medium>
                        <Flex gap=FlexGap::Small align=FlexAlign::Center>
                            <Badge appearance=BadgeAppearance::Filled>{status}</Badge>
                        </Flex>
                        <Body1>
                            "Existing authenticator codes keep working. Each one is re-sealed with the new key the next time it is used."
                        </Body1>
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-totp-outcome"))}
                    </Flex>
                </div>
            </CardContent>
            <CardFooter>
                <div data-testid="delivery-totp-rotate">
                    <Button appearance=ButtonAppearance::Primary on_click=Callback::new(rotate)>"Rotate key"</Button>
                </div>
            </CardFooter>
        </Card>
    }
}
