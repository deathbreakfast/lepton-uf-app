//! Well-known search source ids for picking people, service users and teams.
//!
//! Pass these to `uf_integrations::SearchSourcePicker` (or to `SearchSourceKey`) so a
//! product names a source without depending on the crate that registers it.

/// People, by display name. Registered by this crate as [`super::PeopleSearchSourceId`].
pub const PEOPLE: &str = "lepton_people";

/// Service users the signed-in person can read. Registered by `lepton-app`, which also
/// keeps the older `service_user_search_source` id working.
pub const SERVICE_USERS: &str = "lepton_service_users";

/// Polaron teams. Registered by Polaron; listed here so pickers can name every kind of
/// principal from one place.
pub const TEAMS: &str = "polaron_team";
