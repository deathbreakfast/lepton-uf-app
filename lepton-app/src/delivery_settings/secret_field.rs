//! One write-only secret: shows set or not set, never the value.

use lepton_auth::delivery_settings::SecretStatus;
use leptos::prelude::*;
use uf_product::primitives::*;

/// Status badge plus a password field that is empty on every load.
///
/// Leaving the field empty keeps the stored secret. The stored value never
/// reaches the browser, so there is nothing to reveal.
#[component]
pub fn SecretField(
    /// Field label.
    label: &'static str,
    /// Set or not set, and when it last changed.
    status: SecretStatus,
    /// Receives only what the operator types.
    value: RwSignal<String>,
    /// `data-testid` for the input wrapper.
    test_id: &'static str,
) -> impl IntoView {
    let (badge, text, placeholder) = if status.is_set {
        let when = status
            .rotated_at
            .map(|at| format!("Set, changed {}", at.format("%Y-%m-%d")))
            .unwrap_or_else(|| "Set".to_string());
        (
            BadgeAppearance::Filled,
            when,
            "Type a new value to replace it",
        )
    } else {
        (
            BadgeAppearance::Outline,
            "Not set".to_string(),
            "Type a value",
        )
    };
    view! {
        <Field label=label>
            <Flex gap=FlexGap::Small align=FlexAlign::Center wrap=FlexWrap::Wrap>
                <Badge appearance=badge>{text}</Badge>
                <div data-testid=test_id>
                    <Input
                        bind=value
                        appearance=InputAppearance {
                            input_type: Signal::from(InputType::Password),
                            placeholder: placeholder.into(),
                            ..Default::default()
                        }
                    />
                </div>
            </Flex>
        </Field>
    }
}
