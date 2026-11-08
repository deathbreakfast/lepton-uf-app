//! Values a [`super::PersonPicker`] reads and writes, and the fixed copy it shows.

use leptos::prelude::ViewFn;
use orbital_primitives::BadgeColor;
use serde::{Deserialize, Serialize};

use super::sources;

/// What kind of principal a pick names.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrincipalKind {
    /// A person, from the `lepton_people` source.
    User,
    /// A team, from the `polaron_team` source.
    Team,
    /// A service user, from the `lepton_service_users` source.
    ServiceUser,
    /// A caller-supplied option from [`PickerGroup`]; carries the group's label.
    Extra(String),
}

impl PrincipalKind {
    /// The searchable kinds, in the order the picker lists their groups.
    pub const SEARCHABLE: [PrincipalKind; 3] = [
        PrincipalKind::User,
        PrincipalKind::Team,
        PrincipalKind::ServiceUser,
    ];

    /// Search source id for this kind, or `None` for [`PrincipalKind::Extra`].
    pub fn source_id(&self) -> Option<&'static str> {
        match self {
            Self::User => Some(sources::PEOPLE),
            Self::Team => Some(sources::TEAMS),
            Self::ServiceUser => Some(sources::SERVICE_USERS),
            Self::Extra(_) => None,
        }
    }

    /// Short key used in option values and `data-kind` attributes.
    pub fn key(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Team => "team",
            Self::ServiceUser => "service_user",
            Self::Extra(_) => "extra",
        }
    }
}

/// One picked person, team, service user or caller-supplied option.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PickedPrincipal {
    /// What this pick names.
    pub kind: PrincipalKind,
    /// Id within its kind (a bare user, team or service user id).
    pub id: String,
    /// Display name.
    pub label: String,
    /// Line under the name: a short id, a team size or a service user's owner. Never an
    /// email address.
    pub secondary: Option<String>,
    /// Photo URL for the avatar; initials show when it's `None`.
    pub image_url: Option<String>,
}

impl PickedPrincipal {
    /// Stable key across kinds, such as `user:abc123`.
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind.key(), self.id)
    }
}

/// A caller-supplied group of options, shown above People.
#[derive(Clone)]
pub struct PickerGroup {
    /// Group header.
    pub label: String,
    /// Options in the order shown.
    pub options: Vec<PickerOption>,
}

/// One caller-supplied option in a [`PickerGroup`].
#[derive(Clone)]
pub struct PickerOption {
    /// Id, unique within its group.
    pub id: String,
    /// Text the option is matched and announced by.
    pub label: String,
    /// Optional line beside the chip.
    pub secondary: Option<String>,
    /// What the row and the picked chip draw, such as a brand `Tag`.
    pub chip: ViewFn,
}

impl PickerOption {
    /// The pick this option makes, with `group` as its [`PrincipalKind::Extra`] label.
    pub fn to_principal(&self, group: &str) -> PickedPrincipal {
        PickedPrincipal {
            kind: PrincipalKind::Extra(group.to_string()),
            id: self.id.clone(),
            label: self.label.clone(),
            secondary: self.secondary.clone(),
            image_url: None,
        }
    }
}

/// Plain group header for `kind`.
pub fn group_label(kind: &PrincipalKind) -> String {
    match kind {
        PrincipalKind::User => "People".to_string(),
        PrincipalKind::Team => "Teams".to_string(),
        PrincipalKind::ServiceUser => "Service users".to_string(),
        PrincipalKind::Extra(label) => label.clone(),
    }
}

/// Group header with the search source named, used when `show_sources` is on. `None` for
/// caller-supplied groups, which keep their own label.
pub fn source_caption(kind: &PrincipalKind) -> Option<&'static str> {
    match kind {
        PrincipalKind::User => Some("People · Lepton users"),
        PrincipalKind::Team => Some("Teams · Polaron teams you belong to"),
        PrincipalKind::ServiceUser => Some("Service users · Lepton service users"),
        PrincipalKind::Extra(_) => None,
    }
}

/// What a searched group shows when it found nothing, used when `show_sources` is on.
pub fn empty_group_text(kind: &PrincipalKind) -> Option<&'static str> {
    match kind {
        PrincipalKind::User => Some("No people match."),
        PrincipalKind::Team => Some("No teams match."),
        PrincipalKind::ServiceUser => Some("No service users match."),
        PrincipalKind::Extra(_) => None,
    }
}

/// Badge text and color ending each row when `kind_badges` is on. Caller-supplied rows get
/// none.
pub fn kind_badge(kind: &PrincipalKind) -> Option<(&'static str, BadgeColor)> {
    match kind {
        PrincipalKind::User => Some(("Person", BadgeColor::Subtle)),
        PrincipalKind::Team => Some(("Team", BadgeColor::Informative)),
        PrincipalKind::ServiceUser => Some(("Service user", BadgeColor::Subtle)),
        PrincipalKind::Extra(_) => None,
    }
}

/// Fewest characters that start a server search.
pub const MIN_QUERY_CHARS: usize = 2;

/// Most rows a search returns per kind.
#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
pub const MAX_PER_KIND: u32 = 50;

/// Whether `query` is long enough to search the server.
pub fn is_searchable(query: &str) -> bool {
    query.trim().chars().count() >= MIN_QUERY_CHARS
}

/// `true` when `text` looks like it holds an email address.
#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
pub fn looks_like_email(text: &str) -> bool {
    text.split_whitespace().any(|word| {
        word.split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'))
    })
}

/// The secondary line a search result shows, never an email address.
#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
pub fn secondary_line(kind: &PrincipalKind, description: Option<&str>) -> Option<String> {
    let description = description
        .map(str::trim)
        .filter(|d| !d.is_empty() && !looks_like_email(d));
    match kind {
        PrincipalKind::ServiceUser => Some(match description {
            Some(d) => match d.strip_prefix("Owned by ") {
                Some(owner) => format!("Service user · owned by {owner}"),
                None => format!("Service user · {d}"),
            },
            None => "Service user".to_string(),
        }),
        _ => description.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_key_is_kind_and_id_happy() {
        let p = PickedPrincipal {
            kind: PrincipalKind::Team,
            id: "t1".into(),
            label: "Atlas".into(),
            secondary: None,
            image_url: None,
        };
        assert_eq!(p.key(), "team:t1");
    }

    #[test]
    fn extra_option_becomes_extra_principal_happy() {
        let option = PickerOption {
            id: "author".into(),
            label: "Author".into(),
            secondary: None,
            chip: ViewFn::default(),
        };
        let p = option.to_principal("From this event");
        assert_eq!(p.kind, PrincipalKind::Extra("From this event".into()));
        assert_eq!(p.key(), "extra:author");
    }

    #[test]
    fn source_ids_match_registered_sources_happy() {
        assert_eq!(PrincipalKind::User.source_id(), Some(sources::PEOPLE));
        assert_eq!(PrincipalKind::Team.source_id(), Some(sources::TEAMS));
        assert_eq!(
            PrincipalKind::ServiceUser.source_id(),
            Some(sources::SERVICE_USERS)
        );
        assert_eq!(PrincipalKind::Extra("x".into()).source_id(), None);
    }

    #[test]
    fn searchable_needs_two_characters_happy() {
        assert!(is_searchable("no"));
        assert!(is_searchable("  noor "));
    }

    #[test]
    fn short_or_blank_query_is_not_searchable_sad() {
        assert!(!is_searchable(""));
        assert!(!is_searchable("n"));
        assert!(!is_searchable("  n  "));
    }

    #[test]
    fn service_user_secondary_names_owner_happy() {
        assert_eq!(
            secondary_line(&PrincipalKind::ServiceUser, Some("Owned by you")),
            Some("Service user · owned by you".into())
        );
        assert_eq!(
            secondary_line(&PrincipalKind::Team, Some("4 members")),
            Some("4 members".into())
        );
    }

    #[test]
    fn secondary_drops_email_addresses_sad() {
        assert_eq!(
            secondary_line(&PrincipalKind::User, Some("noor@example.com")),
            None
        );
        assert_eq!(
            secondary_line(&PrincipalKind::ServiceUser, Some("ops@example.com")),
            Some("Service user".into())
        );
        assert_eq!(secondary_line(&PrincipalKind::User, Some("   ")), None);
    }
}
