//! Server-side people search for person pickers.
//!
//! The `lepton_people` source finds signed-in users by display name: a case-insensitive
//! match on the start of the name or the start of any word in it. Each result carries the
//! display name as its title and the first six characters of the user id as a
//! disambiguator. Results never include an email address or legal name. Profile photos
//! aren't included yet: the search result type in uf-product gains an image field first.
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

mod search_source;
pub mod sources;

#[cfg(feature = "ssr")]
pub use search_source::PeopleSearchSource;
pub use search_source::PeopleSearchSourceId;
