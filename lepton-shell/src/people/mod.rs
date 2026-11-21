//! Person pickers and the server-side people search behind them.
//!
//! [`PersonPicker`] is one typeahead for people, teams and service users. Bind it to a
//! `RwSignal<Vec<PickedPrincipal>>` and turn on the kinds it should search:
//!
//! ```rust,ignore
//! use lepton_shell::people::{PersonPicker, PickedPrincipal};
//! use leptos::prelude::*;
//!
//! let owners = RwSignal::new(Vec::<PickedPrincipal>::new());
//! view! { <PersonPicker value=owners teams=true label="Owners" show_sources=true kind_badges=true /> };
//! ```
//!
//! Rows are Orbital `Persona`s grouped People, Teams and Service users. `single` makes a
//! new pick replace the old one, `exclude` shows rows disabled (existing members, say)
//! and `extra_groups` adds the caller's own options above People. To search your own
//! source as the person types, pass a `query` signal and derive `extra_groups` from it:
//!
//! ```rust,ignore
//! use lepton_shell::people::{PersonPicker, PickedPrincipal, PickerGroup};
//! use leptos::prelude::*;
//!
//! let value = RwSignal::new(Vec::<PickedPrincipal>::new());
//! let query = RwSignal::new(String::new());
//! // `my_groups_matching` is the caller's own search, run for the current text.
//! let groups = Signal::derive(move || my_groups_matching(&query.get()));
//! view! { <PersonPicker value=value query=query extra_groups=groups /> };
//! ```
//!
//! `show_sources` names
//! each group's search source ("People · Lepton users") and keeps a searched group's
//! header when it found nothing; `kind_badges` ends each row with a Person, Team or
//! Service user badge. Both are off by default.
//!
//! The picker calls [`search_people`], which runs the `lepton_people`,
//! `lepton_service_users` and `polaron_team` sources with the caller's Valence. A host
//! that doesn't link a source gets no rows for it. The caller needs a Higgs session, and
//! no axum-login layer is required. Rows and chips show a person's profile photo when it
//! has passed the virus scan, and initials otherwise.
//!
//! ## The `lepton_people` source
//!
//! The `lepton_people` source finds signed-in users by display name: a case-insensitive
//! match on the start of the name or the start of any word in it. Each result carries the
//! display name as its title and the first six characters of the user id as a
//! disambiguator. Results never include an email address or legal name. `image_url` is
//! `/api/files/{id}` for the active profile photo when its file is `available`, and empty
//! while it waits for a scan, after it's quarantined, or when there's no photo.
//!
//! [`sources`] holds the ids a picker passes to `uf_integrations::SearchSourcePicker`, for
//! people and for the other principal kinds a person picker shows beside them:
//!
//! ```rust,ignore
//! use lepton_shell::people::sources;
//! use uf_search_core::SearchSourceKey;
//!
//! let keys = [sources::PEOPLE, sources::SERVICE_USERS, sources::TEAMS]
//!     .map(|id| SearchSourceKey { id: id.to_string(), label: String::new() });
//! # let _ = keys;
//! ```
//!
//! On the server the source answers only signed-in people. Queries shorter than two
//! characters return nothing, each search returns at most 50 people, and each person may
//! run 20 searches in a burst, refilled at 5 a second. Every search adds one to the Spectra
//! counter `lepton_people_search_total`, labelled with its `outcome`.
//!
//! Two limits remain until Lepton changes: a user's account status is readable only by
//! that user, so a suspended person still shows up for others. And without a name index
//! the source reads at most `valence::MAX_QUERY_LIMIT` profiles per search.

mod chips;
mod listing;
mod picker;
mod rows;
mod search_source;
mod server;
pub mod sources;
mod types;

pub use picker::PersonPicker;
pub use server::search_people;
pub use types::{
    empty_group_text, group_label, kind_badge, source_caption, PickedPrincipal, PickerGroup,
    PickerOption, PrincipalKind,
};

#[cfg(feature = "ssr")]
pub use search_source::PeopleSearchSource;
pub use search_source::PeopleSearchSourceId;
