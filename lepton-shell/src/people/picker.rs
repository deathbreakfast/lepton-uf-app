//! [`PersonPicker`]: one typeahead for people, teams and service users.

use std::cell::RefCell;
use std::time::Duration;

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use orbital_primitives::{Combobox, ComboboxAppearance, ComboboxBind, Field, Flex, FlexGap};
use uf_product::components::Caption1;

use super::chips::PickedChips;
use super::listing::{build_listing, Listing, ListingInput};
use super::rows::listing_view;
use super::server::search_people;
use super::types::{is_searchable, PickedPrincipal, PickerGroup, PrincipalKind};

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

/// The message a failed search shows.
fn search_error_text(error: &ServerFnError) -> String {
    match error {
        ServerFnError::ServerError(msg) => msg.clone(),
        _ => "Search isn't working right now. Try again.".to_string(),
    }
}

/// The kinds a picker searches, in group order.
fn searched_kinds(people: bool, teams: bool, service_users: bool) -> Vec<PrincipalKind> {
    [
        (people, PrincipalKind::User),
        (teams, PrincipalKind::Team),
        (service_users, PrincipalKind::ServiceUser),
    ]
    .into_iter()
    .filter_map(|(on, kind)| on.then_some(kind))
    .collect()
}

/// The value after picking `pick`: single mode replaces, multi mode appends once.
fn with_pick(
    current: &[PickedPrincipal],
    pick: PickedPrincipal,
    single: bool,
) -> Vec<PickedPrincipal> {
    if single {
        return vec![pick];
    }
    let mut next = current.to_vec();
    if !next.iter().any(|p| p.key() == pick.key()) {
        next.push(pick);
    }
    next
}

/// Server answers for the query, keyed by the query they answer.
type Answers = RwSignal<Option<(String, Vec<PickedPrincipal>)>>;

/// Searches 200 ms after `query` stops changing, once it has two characters. Stale
/// answers are dropped.
fn debounced_search(
    query: RwSignal<String>,
    kinds: StoredValue<Vec<PrincipalKind>>,
) -> (Answers, RwSignal<Option<String>>) {
    let results: Answers = RwSignal::new(None);
    let search_error = RwSignal::new(None::<String>);
    let generation = StoredValue::new(0_u64);
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
        // The timer fires outside any owner; run the search under the picker's, so it
        // stops with the picker.
        let owner = Owner::current();
        set_timeout(
            move || {
                if generation.try_get_value() != Some(current) {
                    return;
                }
                let Some(owner) = owner else {
                    return;
                };
                owner.with(|| {
                    leptos::task::spawn_local_scoped(async move {
                        let outcome =
                            search_people(q.clone(), kinds.get_value(), SEARCH_LIMIT).await;
                        if generation.try_get_value() != Some(current) {
                            return;
                        }
                        let (rows, error) = match outcome {
                            Ok(rows) => (rows, None),
                            Err(e) => (Vec::new(), Some(search_error_text(&e))),
                        };
                        results.try_set(Some((q, rows)));
                        search_error.try_set(error);
                    })
                });
            },
            DEBOUNCE,
        );
    });
    (results, search_error)
}

/// Adds the listing's pickable rows to `shown`, keeping the newest [`MAX_SHOWN`].
fn remember_shown(shown: &mut Vec<PickedPrincipal>, listing: &Listing) {
    for p in listing.pickable() {
        if !shown.iter().any(|r| r.key() == p.key()) {
            shown.push(p.clone());
        }
    }
    let overflow = shown.len().saturating_sub(MAX_SHOWN);
    shown.drain(..overflow);
}

/// The combobox doesn't close on Escape; leaving the input closes it, and focus goes
/// straight back so the keyboard path carries on.
fn close_on_escape(ev: &KeyboardEvent) {
    use leptos::wasm_bindgen::JsCast;
    if let Some(input) = ev
        .target()
        .and_then(|t| t.dyn_into::<leptos::web_sys::HtmlElement>().ok())
    {
        let _ = input.blur();
        let _ = input.focus();
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
    let kinds = StoredValue::new(searched_kinds(people, teams, service_users));
    let query = RwSignal::new(String::new());
    let selected = RwSignal::new(Vec::<String>::new());
    let recent = RwSignal::new(Vec::<PickedPrincipal>::new());
    // Rows the list has offered. Single mode writes the option's text into the box before
    // the pick is read, which re-lists for that text, so picks resolve against this.
    let shown = StoredValue::new(Vec::<PickedPrincipal>::new());
    let (results, search_error) = debounced_search(query, kinds);
    Effect::new(move |_| recent.set(recent_picks()));

    let listing = Signal::derive(move || {
        let q = query.get();
        let answered = results.get();
        let answered = answered.as_ref().filter(|(asked, _)| asked == q.trim());
        let (extra, exclude, picked, recents) =
            (extra_groups.get(), exclude.get(), value.get(), recent.get());
        kinds.with_value(|kinds| {
            build_listing(&ListingInput {
                extra: &extra,
                kinds,
                query: &q,
                results: answered.map(|(_, rows)| rows.as_slice()),
                recents: &recents,
                picked: &picked,
                exclude: &exclude,
                show_sources,
                kind_badges,
            })
        })
    });

    let commit = move |next: Vec<PickedPrincipal>| {
        value.set(next.clone());
        if let Some(cb) = on_change {
            cb.run(next);
        }
    };
    let remove = Callback::new(move |key: String| {
        commit(value.with_untracked(|v| v.iter().filter(|p| p.key() != key).cloned().collect()));
    });

    Effect::new(move |_| {
        let Some(key) = selected.get().last().cloned() else {
            return;
        };
        selected.set(Vec::new());
        query.set(String::new());
        if let Some(pick) = shown.with_value(|rows| rows.iter().find(|p| p.key() == key).cloned()) {
            remember(&pick);
            recent.set(recent_picks());
            commit(value.with_untracked(|v| with_pick(v, pick, single)));
        }
    });

    let on_keydown = move |ev: KeyboardEvent| match ev.key().as_str() {
        "Escape" => close_on_escape(&ev),
        "Backspace" if !disabled.get_untracked() && query.with_untracked(String::is_empty) => {
            if let Some(last) = value.with_untracked(|v| v.last().map(PickedPrincipal::key)) {
                remove.run(last);
            }
        }
        _ => {}
    };

    let options = move || {
        let listing = listing.get();
        shown.update_value(|rows| remember_shown(rows, &listing));
        listing_view(listing, &query.get())
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
        <Flex vertical=true full_width=true attr:data-testid="person-picker">
            <Field label=label>
                <Flex vertical=true gap=FlexGap::Small full_width=true>
                    <Flex full_width=true on:keydown=on_keydown>
                        <Combobox
                            bind=ComboboxBind::new(query, selected)
                            appearance=appearance
                            class="lepton-person-picker__combobox"
                        >
                            {options}
                        </Combobox>
                    </Flex>
                    <PickedChips
                        value=value
                        extra_groups=extra_groups
                        disabled=disabled
                        on_remove=remove
                    />
                    {move || search_error.get().map(|msg| view! {
                        <span data-testid="person-picker-error" role="alert">
                            <Caption1>{msg}</Caption1>
                        </span>
                    })}
                </Flex>
            </Field>
        </Flex>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::listing::fixtures::*;
    use crate::people::types::{kind_badge, source_caption};
    use orbital_primitives::BadgeColor;

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

    #[test]
    fn single_pick_replaces_value_happy() {
        let next = with_pick(&[noor()], atlas(), true);
        assert_eq!(next, vec![atlas()]);
    }

    #[test]
    fn multi_pick_appends_once_sad() {
        let next = with_pick(&[noor()], noor(), false);
        assert_eq!(next, vec![noor()]);
        assert_eq!(with_pick(&[noor()], atlas(), false), vec![noor(), atlas()]);
    }

    #[test]
    fn searched_kinds_follow_flags_happy() {
        assert_eq!(
            searched_kinds(true, false, true),
            vec![PrincipalKind::User, PrincipalKind::ServiceUser]
        );
    }

    #[test]
    fn searched_kinds_all_off_searches_nothing_sad() {
        assert_eq!(searched_kinds(false, false, false), vec![]);
    }
}
