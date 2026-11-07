//! One write-only secret: shows set or not set, never the value.

use lepton_auth::delivery_settings::SecretStatus;
use leptos::prelude::*;
use uf_product::components::Caption1;
use uf_product::primitives::*;
use uf_product::AdaptiveSheet;

use super::changes::secret_status_text;

/// `label` as it reads after a verb: "Password" becomes "password", "API key" stays.
fn mid_sentence(label: &str) -> String {
    let mut chars = label.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if !second.is_uppercase() => first
            .to_lowercase()
            .chain(label[first.len_utf8()..].chars())
            .collect(),
        _ => label.to_string(),
    }
}

/// Status line plus a Set or Replace button that opens a one-field sheet.
///
/// The stored value never reaches the browser, so there is nothing to reveal.
/// What the operator types goes into `value` and stays there until Save sends
/// it; leaving it empty keeps the stored secret.
///
/// Test ids: `{test_id}` wraps the row, `{test_id}-open` the button,
/// `{test_id}-dialog` the sheet, `{test_id}-input` the field and
/// `{test_id}-confirm` the sheet's Set button.
#[component]
pub fn SecretRow(
    /// Field label.
    label: &'static str,
    /// Set or not set, and when it last changed.
    status: SecretStatus,
    /// Receives only what the operator types.
    value: RwSignal<String>,
    /// `data-testid` prefix.
    test_id: &'static str,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let draft = RwSignal::new(String::new());
    let is_set = status.is_set;
    let status_text = move || secret_status_text(&status, !value.with(|v| v.trim().is_empty()));
    let button_label = if is_set { "Replace" } else { "Set" };
    let title = format!("{button_label} {}", mid_sentence(label));

    let apply = move |_: leptos::ev::MouseEvent| {
        value.set(draft.get_untracked());
        draft.set(String::new());
        open.set(false);
    };
    let cancel = move |_: leptos::ev::MouseEvent| {
        draft.set(String::new());
        open.set(false);
    };

    view! {
        <Field label=label>
            <div data-testid=test_id>
                <Flex gap=FlexGap::Medium align=FlexAlign::Center wrap=FlexWrap::Wrap>
                    <Caption1>{status_text}</Caption1>
                    <div data-testid=format!("{test_id}-open")>
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on_click=Callback::new(move |_| open.set(true))
                        >
                            {button_label}
                        </Button>
                    </div>
                </Flex>
            </div>
        </Field>
        <AdaptiveSheet
            open=open
            title=move || title.clone()
            testid=format!("{test_id}-dialog")
            size=DrawerSize::Small
            actions=move || view! {
                <Button appearance=ButtonAppearance::Secondary on_click=Callback::new(cancel)>
                    "Cancel"
                </Button>
                <div data-testid=format!("{test_id}-confirm")>
                    <Button appearance=ButtonAppearance::Primary on_click=Callback::new(apply)>
                        "Set"
                    </Button>
                </div>
            }
        >
            <Flex vertical=true gap=FlexGap::Small>
                <Field label=label>
                    <div data-testid=format!("{test_id}-input")>
                        <Input
                            bind=draft
                            appearance=InputAppearance {
                                input_type: Signal::from(InputType::Password),
                                ..Default::default()
                            }
                        />
                    </div>
                </Field>
                <Caption1>"Stored in Neutrino. The value is never shown again."</Caption1>
            </Flex>
        </AdaptiveSheet>
    }
}

#[cfg(test)]
mod tests {
    use super::mid_sentence;

    #[test]
    fn labels_read_mid_sentence_and_acronyms_stay() {
        assert_eq!(mid_sentence("Password"), "password");
        assert_eq!(mid_sentence("API key"), "API key");
    }
}
