//! Option views for the open list: Persona rows, caller rows and status lines.

use leptos::prelude::*;
use orbital_core_components::{Persona, PersonaConfig, PersonaSecondaryText, PersonaSize};
use orbital_primitives::{
    AvatarColor, Badge, BadgeAppearance, BadgeSize, ComboboxOption, ComboboxOptionGroup, Flex,
    FlexAlign, FlexGap, FlexJustify, SpacingHorizontal, SpacingInset, SpacingVertical,
};
use uf_product::components::Caption1;

use super::listing::{Listing, Row};

/// A disabled option line whose filter text carries the query, so it shows for any input.
fn status_option(value: String, text: &'static str, query: &str) -> impl IntoView {
    view! {
        <ComboboxOption
            text=format!("{text} {query}")
            value=value
            disabled=Signal::from(true)
        >
            <span data-testid="person-picker-status">
                <Caption1>{text}</Caption1>
            </span>
        </ComboboxOption>
    }
}

fn row_body(row: &Row) -> AnyView {
    let secondary = row.principal.secondary.clone().unwrap_or_default();
    match row.chip.clone() {
        Some(chip) => view! {
            <Flex gap=FlexGap::Small align=FlexAlign::Center>
                {chip.run()}
                <Caption1>{secondary}</Caption1>
            </Flex>
        }
        .into_any(),
        None => view! {
            <Persona config=PersonaConfig {
                name: Some(row.principal.label.clone()),
                avatar_src: row.principal.image_url.clone(),
                size: PersonaSize::Medium,
                color: AvatarColor::Neutral,
                ..PersonaConfig::named(row.principal.label.clone())
            }>
                <PersonaSecondaryText slot>{secondary}</PersonaSecondaryText>
            </Persona>
        }
        .into_any(),
    }
}

fn row_view(row: Row) -> impl IntoView {
    let key = row.principal.key();
    let kind = row.principal.kind.key();
    let badge = row.badge.map(|(text, color)| {
        view! {
            <span data-testid="person-picker-badge">
                <Badge appearance=BadgeAppearance::Outline color=color size=BadgeSize::Small>
                    {text}
                </Badge>
            </span>
        }
    });
    let body = row_body(&row);
    // The vertical inset keeps every row a 44 px touch target on a phone.
    view! {
        <ComboboxOption text=row.text value=key.clone() disabled=Signal::from(row.disabled)>
            <Flex
                justify=FlexJustify::SpaceBetween
                align=FlexAlign::Center
                full_width=true
                padding=SpacingInset::symmetric(SpacingHorizontal::None, SpacingVertical::XS)
                attr:data-testid="person-picker-row"
                attr:data-key=key
                attr:data-kind=kind
                attr:data-disabled=row.disabled.to_string()
            >
                {body}
                {badge}
            </Flex>
        </ComboboxOption>
    }
}

/// The open list's groups, then its status line.
pub fn listing_view(listing: Listing, query: &str) -> impl IntoView {
    let Listing { groups, status } = listing;
    let groups = groups
        .into_iter()
        .map(|group| {
            let empty = group
                .empty_text
                .map(|text| status_option(format!("__empty__:{}", group.key), text, query));
            let rows = group.rows.into_iter().map(row_view).collect_view();
            view! {
                <ComboboxOptionGroup
                    label=group.label
                    attr:data-testid="person-picker-group"
                    attr:data-group=group.key.clone()
                >
                    {rows}
                    {empty}
                </ComboboxOptionGroup>
            }
        })
        .collect_view();
    let status = status.map(|text| status_option("__status__".to_string(), text, query));
    view! { {groups} {status} }
}
