//! [`PickedChips`]: the picked principals, each a chip with a remove button.

use leptos::prelude::*;
use orbital_core_components::{Tag, TagAppearance, TagSize};
use orbital_primitives::{
    Avatar, AvatarColor, AvatarConfig, Button, ButtonAppearance, ButtonSize, Flex, FlexAlign,
    FlexGap, FlexWrap,
};

use super::types::{PickedPrincipal, PickerGroup, PrincipalKind};

/// The caller's chip for an `extra_groups` pick, or an avatar for everyone else.
fn chip_media(pick: &PickedPrincipal, extra: &[PickerGroup]) -> AnyView {
    let extra_chip = match &pick.kind {
        PrincipalKind::Extra(group) => extra
            .iter()
            .filter(|g| g.label == *group)
            .flat_map(|g| &g.options)
            .find(|o| o.id == pick.id)
            .map(|o| o.chip.clone()),
        _ => None,
    };
    match extra_chip {
        Some(chip) => chip.run(),
        None => view! {
            <Avatar config=AvatarConfig {
                src: pick.image_url.clone(),
                size: Some(20),
                color: AvatarColor::Neutral,
                ..AvatarConfig::name(pick.label.clone())
            } />
        }
        .into_any(),
    }
}

/// Picked principals as chips; `on_remove` gets the removed pick's key.
#[component]
pub fn PickedChips(
    value: RwSignal<Vec<PickedPrincipal>>,
    extra_groups: Signal<Vec<PickerGroup>>,
    disabled: Signal<bool>,
    on_remove: Callback<String>,
) -> impl IntoView {
    let chips = move || {
        let extra = extra_groups.get();
        value
            .get()
            .into_iter()
            .map(|p| {
                let media = chip_media(&p, &extra);
                let key = p.key();
                let remove_key = key.clone();
                let label = p.label;
                view! {
                    <span data-testid="person-picker-chip" data-key=key data-kind=p.kind.key()>
                        <Tag size=TagSize::Small appearance=TagAppearance::Outline>
                            <Flex gap=FlexGap::Small align=FlexAlign::Center>
                                {media}
                                <span>{label.clone()}</span>
                                <Button
                                    appearance=ButtonAppearance::Transparent
                                    size=ButtonSize::Small
                                    icon=icondata::AiCloseOutlined
                                    disabled=disabled
                                    attr:aria-label=format!("Remove {label}")
                                    attr:data-testid="person-picker-chip-remove"
                                    on_click=Callback::new(move |_| on_remove.run(remove_key.clone()))
                                />
                            </Flex>
                        </Tag>
                    </span>
                }
            })
            .collect_view()
    };
    view! {
        <Show when=move || value.with(|v| !v.is_empty())>
            <Flex
                gap=FlexGap::Small
                wrap=FlexWrap::Wrap
                align=FlexAlign::Center
                attr:data-testid="person-picker-chips"
            >
                {chips}
            </Flex>
        </Show>
    }
}
