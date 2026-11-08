//! [`PersonPicker`]: one typeahead for people, teams and service users.

use std::cell::RefCell;
use std::time::Duration;

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use orbital_core_components::{
    Persona, PersonaConfig, PersonaSecondaryText, PersonaSize, Tag, TagAppearance, TagSize,
};
use orbital_primitives::{
    Avatar, AvatarColor, AvatarConfig, Badge, BadgeAppearance, BadgeColor, BadgeSize, Button,
    ButtonAppearance, ButtonSize, Combobox, ComboboxAppearance, ComboboxBind, ComboboxOption,
    ComboboxOptionGroup, Field, Flex, FlexAlign, FlexGap, FlexJustify, FlexWrap,
};

use uf_product::components::Caption1;

use super::server::search_people;
use super::types::{
    empty_group_text, group_label, is_searchable, kind_badge, source_caption, PickedPrincipal,
    PickerGroup, PrincipalKind,
};

/// Rows a search asks for per kind.
const SEARCH_LIMIT: u32 = 20;
/// Wait after the last keystroke before searching.
const DEBOUNCE: Duration = Duration::from_millis(200);
/// Recent picks kept for the empty box.
const MAX_RECENTS: usize = 5;
/// Rows remembered from earlier listings, so a pick still resolves after the list moved on.
const MAX_SHOWN: usize = 200;

thread_local! {
    // Browser-only memory of this page's picks. The server never writes it, so one
    // person's picks can't reach another person's render.
    static RECENTS: RefCell<Vec<PickedPrincipal>> = const { RefCell::new(Vec::new()) };
}

fn recent_picks() -> Vec<PickedPrincipal> {
    RECENTS.with_borrow(Clone::clone)
}

fn remember(pick: &PickedPrincipal) {
    if cfg!(feature = "ssr") || matches!(pick.kind, PrincipalKind::Extra(_)) {
        return;
    }
    RECENTS.with_borrow_mut(|recents| {
        recents.retain(|p| p.key() != pick.key());
        recents.insert(0, pick.clone());
        recents.truncate(MAX_RECENTS);
    });
}

/// One option row in the open list.
#[derive(Clone)]
pub struct Row {
    pub principal: PickedPrincipal,
    /// Shown but not pickable (the caller's `exclude`).
    pub disabled: bool,
    /// Kind badge text and color, when `kind_badges` is on.
    pub badge: Option<(&'static str, BadgeColor)>,
    /// Caller-supplied chip, for `extra_groups` rows.
    pub chip: Option<ViewFn>,
    /// Text the combobox filters and announces the row by.
    pub text: String,
}

/// One labelled group in the open list.
#[derive(Clone)]
pub struct Group {
    pub key: String,
    pub label: String,
    pub rows: Vec<Row>,
    /// Shown in place of rows when a searched source found nothing.
    pub empty_text: Option<&'static str>,
}

/// Everything the open list shows.
#[derive(Clone, Default)]
pub struct Listing {
    pub groups: Vec<Group>,
    /// A single disabled line when there are no groups: a hint, "Searching…" or no match.
    pub status: Option<&'static str>,
}

impl Listing {
    /// Every principal a row offers that can be picked.
    fn pickable(&self) -> impl Iterator<Item = &PickedPrincipal> {
        self.groups
            .iter()
            .flat_map(|g| &g.rows)
            .filter(|r| !r.disabled)
            .map(|r| &r.principal)
    }

    /// The pickable principal behind option value `key`.
    #[cfg(test)]
    fn pick(&self, key: &str) -> Option<PickedPrincipal> {
        self.groups
            .iter()
            .flat_map(|g| &g.rows)
            .find(|r| !r.disabled && r.principal.key() == key)
            .map(|r| r.principal.clone())
    }
}

pub struct ListingInput<'a> {
    pub extra: &'a [PickerGroup],
    pub kinds: &'a [PrincipalKind],
    pub query: &'a str,
    /// Server rows, when the server has answered for `query`.
    pub results: Option<&'a [PickedPrincipal]>,
    pub recents: &'a [PickedPrincipal],
    pub picked: &'a [PickedPrincipal],
    pub exclude: &'a [PickedPrincipal],
    pub show_sources: bool,
    pub kind_badges: bool,
}

/// Whether a local row (a caller option or a recent pick) matches what's typed.
fn matches_typed(query: &str, texts: &[&str]) -> bool {
    let needle = query.trim().to_lowercase();
    needle.is_empty() || texts.iter().any(|t| t.to_lowercase().contains(&needle))
}

/// Groups for the open list: caller groups first, then recent picks on a short query or
/// People, Teams and Service users once the server answered. Picked rows are left out;
/// excluded rows stay, disabled.
pub fn build_listing(input: &ListingInput<'_>) -> Listing {
    let picked = |p: &PickedPrincipal| input.picked.iter().any(|x| x.key() == p.key());
    let excluded = |p: &PickedPrincipal| input.exclude.iter().any(|x| x.key() == p.key());
    let row = |principal: PickedPrincipal, text: String, chip: Option<ViewFn>| Row {
        disabled: excluded(&principal),
        badge: if input.kind_badges {
            kind_badge(&principal.kind)
        } else {
            None
        },
        chip,
        text,
        principal,
    };

    let mut groups = Vec::new();
    for (i, group) in input.extra.iter().enumerate() {
        let rows: Vec<Row> = group
            .options
            .iter()
            .filter(|o| {
                let secondary = o.secondary.as_deref().unwrap_or_default();
                matches_typed(input.query, &[&o.label, secondary])
            })
            .map(|o| (o.to_principal(&group.label), o))
            .filter(|(p, _)| !picked(p))
            .map(|(p, o)| {
                let text = match &o.secondary {
                    Some(secondary) => format!("{} {secondary}", o.label),
                    None => o.label.clone(),
                };
                row(p, text, Some(o.chip.clone()))
            })
            .collect();
        if !rows.is_empty() {
            groups.push(Group {
                key: format!("extra-{i}"),
                label: group.label.clone(),
                rows,
                empty_text: None,
            });
        }
    }

    if !is_searchable(input.query) {
        let rows: Vec<Row> = input
            .recents
            .iter()
            .filter(|p| input.kinds.contains(&p.kind) && !picked(p))
            .filter(|p| matches_typed(input.query, &[&p.label]))
            .map(|p| row(p.clone(), p.label.clone(), None))
            .collect();
        if !rows.is_empty() {
            groups.push(Group {
                key: "recent".to_string(),
                label: "Recent".to_string(),
                rows,
                empty_text: None,
            });
        }
        let status = groups
            .is_empty()
            .then_some("Type 2 or more letters to search.");
        return Listing { groups, status };
    }

    let Some(results) = input.results else {
        let status = groups.is_empty().then_some("Searching…");
        return Listing { groups, status };
    };

    // The server already matched these rows, so the combobox's own text filter must
    // keep them: their filter text carries the query.
    let query = input.query.trim();
    for kind in input.kinds {
        let rows: Vec<Row> = results
            .iter()
            .filter(|p| p.kind == *kind && !picked(p))
            .map(|p| row(p.clone(), format!("{} {query}", p.label), None))
            .collect();
        let label = if input.show_sources {
            source_caption(kind).map_or_else(|| group_label(kind), str::to_string)
        } else {
            group_label(kind)
        };
        if !rows.is_empty() {
            groups.push(Group {
                key: kind.key().to_string(),
                label,
                rows,
                empty_text: None,
            });
        } else if input.show_sources {
            groups.push(Group {
                key: kind.key().to_string(),
                label,
                rows,
                empty_text: empty_group_text(kind),
            });
        }
    }
    let status = groups.is_empty().then_some("No one matches.");
    Listing { groups, status }
}

/// The message a failed search shows.
fn search_error_text(error: &ServerFnError) -> String {
    match error {
        ServerFnError::ServerError(msg) => msg.clone(),
        _ => "Search isn't working right now. Try again.".to_string(),
    }
}

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

fn row_view(row: Row) -> impl IntoView {
    let key = row.principal.key();
    let kind = row.principal.kind.key();
    let badge = row.badge.map(|(text, color)| {
        view! {
            <span data-testid="person-picker-badge">
                <Badge
                    appearance=BadgeAppearance::Outline
                    color=color
                    size=BadgeSize::Small
                >
                    {text}
                </Badge>
            </span>
        }
    });
    let secondary = row.principal.secondary.clone().unwrap_or_default();
    let body = match row.chip {
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
    };
    view! {
        <ComboboxOption
            text=row.text
            value=key.clone()
            disabled=Signal::from(row.disabled)
        >
            <div
                data-testid="person-picker-row"
                data-key=key
                data-kind=kind
                data-disabled=row.disabled.to_string()
                style="min-height: 44px; display: flex; align-items: center; flex: 1;"
            >
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center full_width=true>
                    {body}
                    {badge}
                </Flex>
            </div>
        </ComboboxOption>
    }
}

/// One typeahead for people, teams and service users.
///
/// Rows are Orbital `Persona`s grouped People, Teams and Service users, with a name and a
/// secondary line (a short id, a team size, "Service user · owned by …"), never an email.
/// The server search starts after two characters, 200 ms after the last keystroke; an
/// empty box lists only this page's recent picks and the caller's `extra_groups`, never
/// the directory. Picks show as chips with a remove button, and Backspace in an empty
/// box removes the last one. Up and Down move, Enter picks, Esc closes.
///
/// ```rust,ignore
/// use lepton_shell::people::{PersonPicker, PickedPrincipal};
/// use leptos::prelude::*;
///
/// let owners = RwSignal::new(Vec::<PickedPrincipal>::new());
/// view! { <PersonPicker value=owners teams=true label="Owners" /> };
/// ```
// The props are the picker's public API: one flag per searchable kind and per mode.
#[allow(clippy::fn_params_excessive_bools)]
#[component]
pub fn PersonPicker(
    /// Picked principals. Single mode holds at most one.
    value: RwSignal<Vec<PickedPrincipal>>,
    /// A new pick replaces the old one instead of adding a chip.
    #[prop(default = false)]
    single: bool,
    /// Search people (`lepton_people`).
    #[prop(default = true)]
    people: bool,
    /// Search teams (`polaron_team`).
    #[prop(default = false)]
    teams: bool,
    /// Search service users (`lepton_service_users`).
    #[prop(default = false)]
    service_users: bool,
    /// Rows shown disabled, such as existing members.
    #[prop(optional, into)]
    exclude: Signal<Vec<PickedPrincipal>>,
    /// Caller-supplied groups shown above People.
    #[prop(optional, into)]
    extra_groups: Signal<Vec<PickerGroup>>,
    /// Input placeholder.
    #[prop(optional, into)]
    placeholder: MaybeProp<String>,
    /// Field label.
    #[prop(optional, into)]
    label: MaybeProp<String>,
    /// Disables the input and the chips' remove buttons.
    #[prop(optional, into)]
    disabled: Signal<bool>,
    /// Called with the new value after each pick or removal.
    #[prop(optional)]
    on_change: Option<Callback<Vec<PickedPrincipal>>>,
    /// Name each group's search source in its header, and show searched-but-empty groups.
    #[prop(default = false)]
    show_sources: bool,
    /// End each row with a Person, Team or Service user badge.
    #[prop(default = false)]
    kind_badges: bool,
) -> impl IntoView {
    let kinds: Vec<PrincipalKind> = [(people, PrincipalKind::User), (teams, PrincipalKind::Team)]
        .into_iter()
        .chain([(service_users, PrincipalKind::ServiceUser)])
        .filter_map(|(on, kind)| on.then_some(kind))
        .collect();
    let kinds = StoredValue::new(kinds);

    let query = RwSignal::new(String::new());
    let selected = RwSignal::new(Vec::<String>::new());
    let results = RwSignal::new(None::<(String, Vec<PickedPrincipal>)>);
    let search_error = RwSignal::new(None::<String>);
    let recent = RwSignal::new(Vec::<PickedPrincipal>::new());
    let generation = StoredValue::new(0_u64);
    // Rows the list has offered. Single mode writes the option's text into the box before
    // the pick is read, which re-lists for that text, so picks resolve against this.
    let shown = StoredValue::new(Vec::<PickedPrincipal>::new());

    Effect::new(move |_| recent.set(recent_picks()));

    Effect::new(move |_| {
        let q = query.get();
        generation.update_value(|g| *g += 1);
        let current = generation.get_value();
        search_error.set(None);
        if !is_searchable(&q) {
            results.set(None);
            return;
        }
        let q = q.trim().to_string();
        set_timeout(
            move || {
                if generation.try_get_value() != Some(current) {
                    return;
                }
                leptos::task::spawn_local(async move {
                    let outcome = search_people(q.clone(), kinds.get_value(), SEARCH_LIMIT).await;
                    if generation.try_get_value() != Some(current) {
                        return;
                    }
                    match outcome {
                        Ok(rows) => {
                            results.try_set(Some((q, rows)));
                        }
                        Err(e) => {
                            results.try_set(Some((q, Vec::new())));
                            search_error.try_set(Some(search_error_text(&e)));
                        }
                    }
                });
            },
            DEBOUNCE,
        );
    });

    let listing = Signal::derive(move || {
        let q = query.get();
        let answered = results.get();
        let answered = answered
            .as_ref()
            .filter(|(asked, _)| asked == q.trim())
            .map(|(_, rows)| rows.as_slice());
        extra_groups.with(|extra| {
            exclude.with(|exclude| {
                value.with(|picked| {
                    recent.with(|recents| {
                        kinds.with_value(|kinds| {
                            build_listing(&ListingInput {
                                extra,
                                kinds,
                                query: &q,
                                results: answered,
                                recents,
                                picked,
                                exclude,
                                show_sources,
                                kind_badges,
                            })
                        })
                    })
                })
            })
        })
    });

    let commit = move |next: Vec<PickedPrincipal>| {
        value.set(next.clone());
        if let Some(cb) = on_change {
            cb.run(next);
        }
    };

    Effect::new(move |_| {
        let keys = selected.get();
        let Some(key) = keys.last().cloned() else {
            return;
        };
        let pick = shown.with_value(|rows| rows.iter().find(|p| p.key() == key).cloned());
        selected.set(Vec::new());
        query.set(String::new());
        let Some(pick) = pick else {
            return;
        };
        let next = value.with_untracked(|current| {
            if single {
                vec![pick.clone()]
            } else if current.iter().any(|p| p.key() == pick.key()) {
                current.clone()
            } else {
                let mut next = current.clone();
                next.push(pick.clone());
                next
            }
        });
        remember(&pick);
        recent.set(recent_picks());
        commit(next);
    });

    let remove = move |key: String| {
        let next = value.with_untracked(|current| {
            current
                .iter()
                .filter(|p| p.key() != key)
                .cloned()
                .collect::<Vec<_>>()
        });
        commit(next);
    };

    let on_keydown = move |ev: KeyboardEvent| {
        if ev.key() == "Escape" {
            // The combobox doesn't close on Escape; leaving the input closes it, and
            // focus goes straight back so the keyboard path carries on.
            use leptos::wasm_bindgen::JsCast;
            if let Some(input) = ev
                .target()
                .and_then(|t| t.dyn_into::<leptos::web_sys::HtmlElement>().ok())
            {
                let _ = input.blur();
                let _ = input.focus();
            }
            return;
        }
        if ev.key() != "Backspace" || disabled.get_untracked() {
            return;
        }
        if !query.with_untracked(String::is_empty) {
            return;
        }
        if let Some(last) = value.with_untracked(|v| v.last().map(PickedPrincipal::key)) {
            remove(last);
        }
    };

    let chips = move || {
        let extra = extra_groups.get();
        value
            .get()
            .into_iter()
            .map(|p| {
                let key = p.key();
                let kind = p.kind.key();
                let label = p.label.clone();
                let extra_chip = match &p.kind {
                    PrincipalKind::Extra(group) => extra
                        .iter()
                        .filter(|g| g.label == *group)
                        .flat_map(|g| &g.options)
                        .find(|o| o.id == p.id)
                        .map(|o| o.chip.clone()),
                    _ => None,
                };
                let media = match extra_chip {
                    Some(chip) => chip.run(),
                    None => view! {
                        <Avatar config=AvatarConfig {
                            src: p.image_url.clone(),
                            size: Some(20),
                            color: AvatarColor::Neutral,
                            ..AvatarConfig::name(label.clone())
                        } />
                    }
                    .into_any(),
                };
                let remove_key = key.clone();
                view! {
                    <span data-testid="person-picker-chip" data-key=key data-kind=kind>
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
                                    on_click=Callback::new(move |_| remove(remove_key.clone()))
                                />
                            </Flex>
                        </Tag>
                    </span>
                }
            })
            .collect_view()
    };

    let options = move || {
        let q = query.get();
        let listing = listing.get();
        shown.update_value(|rows| {
            for p in listing.pickable() {
                if !rows.iter().any(|r| r.key() == p.key()) {
                    rows.push(p.clone());
                }
            }
            let overflow = rows.len().saturating_sub(MAX_SHOWN);
            rows.drain(..overflow);
        });
        let Listing { groups, status } = listing;
        let groups = groups
            .into_iter()
            .map(|group| {
                let empty = group
                    .empty_text
                    .map(|text| status_option(format!("__empty__:{}", group.key), text, &q));
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
        let status = status.map(|text| status_option("__status__".to_string(), text, &q));
        view! { {groups} {status} }
    };

    let placeholder = Signal::derive(move || {
        placeholder
            .get()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or_else(|| "Search by name".to_string())
    });
    let appearance = ComboboxAppearance {
        placeholder: MaybeProp::from(placeholder),
        disabled,
        clearable: false,
        multiselect: Signal::from(!single),
        ..Default::default()
    };

    view! {
        <div data-testid="person-picker" style="width: 100%;">
            <Field label=label>
                <Flex vertical=true gap=FlexGap::Small full_width=true>
                    <div on:keydown=on_keydown style="width: 100%;">
                        <Combobox
                            bind=ComboboxBind::new(query, selected)
                            appearance=appearance
                            class="lepton-person-picker__combobox"
                        >
                            {options}
                        </Combobox>
                    </div>
                    <Show when=move || value.with(|v| !v.is_empty())>
                        <div data-testid="person-picker-chips">
                            <Flex gap=FlexGap::Small wrap=FlexWrap::Wrap align=FlexAlign::Center>
                                {chips}
                            </Flex>
                        </div>
                    </Show>
                    {move || search_error.get().map(|msg| view! {
                        <span data-testid="person-picker-error" role="alert">
                            <Caption1>{msg}</Caption1>
                        </span>
                    })}
                </Flex>
            </Field>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::types::PickerOption;

    fn principal(kind: PrincipalKind, id: &str, label: &str) -> PickedPrincipal {
        PickedPrincipal {
            kind,
            id: id.into(),
            label: label.into(),
            secondary: None,
            image_url: None,
        }
    }

    fn noor() -> PickedPrincipal {
        principal(PrincipalKind::User, "u1", "Noor Haddad")
    }

    fn atlas() -> PickedPrincipal {
        principal(PrincipalKind::Team, "t1", "Atlas team")
    }

    fn extra_group() -> PickerGroup {
        PickerGroup {
            label: "From this event".into(),
            options: vec![PickerOption {
                id: "author".into(),
                label: "Author".into(),
                secondary: Some("trigger.task.author".into()),
                chip: ViewFn::default(),
            }],
        }
    }

    const BOTH: [PrincipalKind; 2] = [PrincipalKind::User, PrincipalKind::Team];

    fn input<'a>(query: &'a str, results: Option<&'a [PickedPrincipal]>) -> ListingInput<'a> {
        ListingInput {
            extra: &[],
            kinds: &BOTH,
            query,
            results,
            recents: &[],
            picked: &[],
            exclude: &[],
            show_sources: false,
            kind_badges: false,
        }
    }

    fn badge(kind: &PrincipalKind) -> Option<(&'static str, &'static str)> {
        kind_badge(kind).map(|(text, color)| (text, color.as_str()))
    }

    fn labels(listing: &Listing) -> Vec<String> {
        listing.groups.iter().map(|g| g.label.clone()).collect()
    }

    #[test]
    fn source_caption_per_kind_happy() {
        assert_eq!(
            source_caption(&PrincipalKind::User),
            Some("People · Lepton users")
        );
        assert_eq!(
            source_caption(&PrincipalKind::Team),
            Some("Teams · Polaron teams you belong to")
        );
        assert_eq!(
            source_caption(&PrincipalKind::ServiceUser),
            Some("Service users · Lepton service users")
        );
        let results = [noor(), atlas()];
        let listing = build_listing(&ListingInput {
            show_sources: true,
            ..input("no", Some(&results))
        });
        assert_eq!(
            labels(&listing),
            [
                "People · Lepton users",
                "Teams · Polaron teams you belong to"
            ]
        );
    }

    #[test]
    fn source_caption_off_keeps_plain_labels_sad() {
        let results = [noor()];
        let listing = build_listing(&input("no", Some(&results)));
        assert_eq!(labels(&listing), ["People"]);
        assert!(listing.groups.iter().all(|g| g.empty_text.is_none()));
    }

    #[test]
    fn empty_searched_group_shows_header_with_sources_happy() {
        let results = [noor()];
        let listing = build_listing(&ListingInput {
            show_sources: true,
            ..input("no", Some(&results))
        });
        let teams = &listing.groups[1];
        assert_eq!(teams.label, "Teams · Polaron teams you belong to");
        assert!(teams.rows.is_empty());
        assert_eq!(teams.empty_text, Some("No teams match."));
        assert_eq!(
            empty_group_text(&PrincipalKind::User),
            Some("No people match.")
        );
        assert_eq!(
            empty_group_text(&PrincipalKind::ServiceUser),
            Some("No service users match.")
        );
    }

    #[test]
    fn empty_group_hidden_before_server_answers_sad() {
        let listing = build_listing(&ListingInput {
            show_sources: true,
            ..input("no", None)
        });
        assert!(listing.groups.is_empty());
        assert_eq!(listing.status, Some("Searching…"));
    }

    #[test]
    fn kind_badge_label_per_kind_happy() {
        assert_eq!(
            badge(&PrincipalKind::User),
            Some(("Person", BadgeColor::Subtle.as_str()))
        );
        assert_eq!(
            badge(&PrincipalKind::Team),
            Some(("Team", BadgeColor::Informative.as_str()))
        );
        assert_eq!(
            badge(&PrincipalKind::ServiceUser),
            Some(("Service user", BadgeColor::Subtle.as_str()))
        );
        let results = [noor(), atlas()];
        let listing = build_listing(&ListingInput {
            kind_badges: true,
            ..input("no", Some(&results))
        });
        let badges: Vec<_> = listing
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .map(|r| r.badge.map(|(text, _)| text))
            .collect();
        assert_eq!(badges, [Some("Person"), Some("Team")]);
    }

    #[test]
    fn kind_badges_off_draws_no_badge_sad() {
        let results = [noor(), atlas()];
        let listing = build_listing(&input("no", Some(&results)));
        assert!(listing
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .all(|r| r.badge.is_none()));
    }

    #[test]
    fn extra_group_rows_have_no_badge_sad() {
        let extra = [extra_group()];
        let results = [noor()];
        let listing = build_listing(&ListingInput {
            extra: &extra,
            kind_badges: true,
            ..input("au", Some(&results))
        });
        assert_eq!(labels(&listing), ["From this event", "People"]);
        let extra_row = &listing.groups[0].rows[0];
        assert_eq!(extra_row.principal.key(), "extra:author");
        assert!(extra_row.badge.is_none());
        assert!(extra_row.chip.is_some());
        assert!(kind_badge(&PrincipalKind::Extra("x".into())).is_none());
        assert!(listing.groups[1].rows[0].badge.is_some());
    }

    #[test]
    fn extra_rows_follow_typed_text_happy() {
        let extra = [extra_group()];
        let by_secondary = build_listing(&ListingInput {
            extra: &extra,
            ..input("task.auth", None)
        });
        assert_eq!(labels(&by_secondary), ["From this event"]);
        assert_eq!(
            by_secondary.groups[0].rows[0].text,
            "Author trigger.task.author"
        );
    }

    #[test]
    fn extra_rows_not_matching_are_left_out_sad() {
        let extra = [extra_group()];
        let listing = build_listing(&ListingInput {
            extra: &extra,
            ..input("noor", Some(&[]))
        });
        assert!(listing.groups.is_empty());
        assert_eq!(listing.status, Some("No one matches."));
    }

    #[test]
    fn empty_box_lists_recents_not_directory_happy() {
        let recents = [noor()];
        let results = [atlas()];
        let listing = build_listing(&ListingInput {
            recents: &recents,
            ..input("", Some(&results))
        });
        assert_eq!(labels(&listing), ["Recent"]);
        assert_eq!(listing.groups[0].rows[0].principal, noor());
    }

    #[test]
    fn empty_box_without_recents_lists_no_one_sad() {
        let listing = build_listing(&input(" ", None));
        assert!(listing.groups.is_empty());
        assert_eq!(listing.status, Some("Type 2 or more letters to search."));
        let one_letter = build_listing(&input("n", None));
        assert!(one_letter.groups.is_empty());
    }

    #[test]
    fn recents_of_other_kinds_are_left_out_sad() {
        let recents = [principal(PrincipalKind::ServiceUser, "s1", "Deploy bot")];
        let listing = build_listing(&ListingInput {
            recents: &recents,
            ..input("", None)
        });
        assert!(listing.groups.is_empty());
    }

    #[test]
    fn excluded_rows_are_disabled_and_unpickable_happy() {
        let results = [noor(), atlas()];
        let exclude = [noor()];
        let listing = build_listing(&ListingInput {
            exclude: &exclude,
            ..input("no", Some(&results))
        });
        assert!(listing.groups[0].rows[0].disabled);
        assert!(!listing.groups[1].rows[0].disabled);
        assert_eq!(listing.pick("user:u1"), None);
        assert_eq!(listing.pick("team:t1"), Some(atlas()));
    }

    #[test]
    fn picked_rows_are_hidden_sad() {
        let results = [noor(), atlas()];
        let picked = [noor()];
        let listing = build_listing(&ListingInput {
            picked: &picked,
            ..input("no", Some(&results))
        });
        assert_eq!(labels(&listing), ["Teams"]);
        assert_eq!(listing.pick("user:u1"), None);
    }

    #[test]
    fn server_rows_keep_query_in_filter_text_happy() {
        let results = [principal(PrincipalKind::ServiceUser, "s1", "Seeded 0")];
        let listing = build_listing(&ListingInput {
            kinds: &[PrincipalKind::ServiceUser],
            ..input("deploy-bot", Some(&results))
        });
        assert_eq!(listing.groups[0].rows[0].text, "Seeded 0 deploy-bot");
    }

    #[test]
    fn no_match_shows_status_line_sad() {
        let listing = build_listing(&input("zz", Some(&[])));
        assert!(listing.groups.is_empty());
        assert_eq!(listing.status, Some("No one matches."));
    }

    #[test]
    fn server_error_text_keeps_server_message_happy() {
        let e = ServerFnError::new("Sign in to search people.");
        assert_eq!(search_error_text(&e), "Sign in to search people.");
    }

    #[test]
    fn server_error_text_hides_transport_detail_sad() {
        let e: ServerFnError = ServerFnError::Request("connection reset".into());
        assert_eq!(
            search_error_text(&e),
            "Search isn't working right now. Try again."
        );
    }
}
