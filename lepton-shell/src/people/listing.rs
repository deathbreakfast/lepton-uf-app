//! What the open list shows: groups, rows and status lines, worked out from the query,
//! the server's rows, recent picks and the caller's options.

use leptos::prelude::ViewFn;
use orbital_primitives::BadgeColor;

use super::types::{
    empty_group_text, group_label, is_searchable, kind_badge, source_caption, PickedPrincipal,
    PickerGroup, PrincipalKind,
};

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
    pub fn pickable(&self) -> impl Iterator<Item = &PickedPrincipal> {
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

/// Shared rows and inputs for the listing and picker tests.
#[cfg(test)]
pub mod fixtures {
    use super::*;
    use crate::people::types::{kind_badge, PickerOption};

    pub fn principal(kind: PrincipalKind, id: &str, label: &str) -> PickedPrincipal {
        PickedPrincipal {
            kind,
            id: id.into(),
            label: label.into(),
            secondary: None,
            image_url: None,
        }
    }

    pub fn noor() -> PickedPrincipal {
        principal(PrincipalKind::User, "u1", "Noor Haddad")
    }

    pub fn atlas() -> PickedPrincipal {
        principal(PrincipalKind::Team, "t1", "Atlas team")
    }

    pub fn extra_group() -> PickerGroup {
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

    pub const BOTH: [PrincipalKind; 2] = [PrincipalKind::User, PrincipalKind::Team];

    pub fn input<'a>(query: &'a str, results: Option<&'a [PickedPrincipal]>) -> ListingInput<'a> {
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

    pub fn badge(kind: &PrincipalKind) -> Option<(&'static str, &'static str)> {
        kind_badge(kind).map(|(text, color)| (text, color.as_str()))
    }

    pub fn labels(listing: &Listing) -> Vec<String> {
        listing.groups.iter().map(|g| g.label.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

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
}
