//! Two-factor key card: current seal key version and rotation.

use lepton_auth::delivery_settings::TotpKeyView;
use leptos::prelude::*;
use uf_product::components::{Body1, Caption1, Subtitle2};
use uf_product::primitives::*;

use super::server::rotate_totp_seal_key;
use super::step_up::spawn_with_fresh_code;
use super::{friendly_error, Outcome};

/// Status line for the seal key.
fn key_status(view: &TotpKeyView) -> String {
    if view.version == 0 {
        return "TOTP seal key: not created yet".to_string();
    }
    match view.rotated_at {
        Some(at) => format!(
            "TOTP seal key: version {}, rotated {}",
            view.version,
            at.format("%Y-%m-%d")
        ),
        None => format!("TOTP seal key: version {}", view.version),
    }
}

/// Authenticator seal key card.
#[component]
pub fn TotpKeyCard(
    /// Current key state.
    view: TotpKeyView,
    /// Bumped after a successful rotation so the page reloads.
    reload: RwSignal<u32>,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
    /// True when the viewer has no authenticator, so rotating can't be confirmed.
    authenticator_missing: Signal<bool>,
    /// Opens the page's "turn on two-factor" sheet.
    needs_two_factor: RwSignal<bool>,
) -> impl IntoView {
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if let Some(message) = error.get() {
            outcome.set(Some(Outcome::Failed(friendly_error(&message))));
        }
    });

    let rotate = move |_: leptos::ev::MouseEvent| {
        if authenticator_missing.get_untracked() {
            needs_two_factor.set(true);
            return;
        }
        outcome.set(None);
        spawn_with_fresh_code(
            error,
            Callback::new(move |()| {
                outcome.set(Some(Outcome::Done("Two-factor key rotated.".to_string())));
                reload.update(|n| *n += 1);
            }),
            move |code| async move { rotate_totp_seal_key(code).await },
        );
    };

    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Two-factor key"</Subtitle2>
            </CardHeader>
            <CardContent>
                <div data-testid="delivery-totp-card">
                    <Flex vertical=true gap=FlexGap::Medium>
                        <Flex gap=FlexGap::Medium align=FlexAlign::Center wrap=FlexWrap::Wrap>
                            <Caption1>{key_status(&view)}</Caption1>
                            <div data-testid="delivery-totp-rotate">
                                <Button appearance=ButtonAppearance::Secondary on_click=Callback::new(rotate)>
                                    "Rotate key"
                                </Button>
                            </div>
                        </Flex>
                        <Body1>
                            "Existing authenticator codes keep working. Each one is re-sealed with the new key the next time it is used."
                        </Body1>
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-totp-outcome"))}
                    </Flex>
                </div>
            </CardContent>
        </Card>
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn key_status_names_the_version_and_date() {
        let at = chrono::DateTime::parse_from_rfc3339("2026-09-30T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let view = TotpKeyView {
            version: 2,
            rotated_at: Some(at),
        };
        assert_eq!(
            key_status(&view),
            "TOTP seal key: version 2, rotated 2026-09-30"
        );
        assert_eq!(
            key_status(&TotpKeyView::default()),
            "TOTP seal key: not created yet"
        );
    }
}
