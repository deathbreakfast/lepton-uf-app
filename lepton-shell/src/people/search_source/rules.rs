//! Matching and labelling rules for people search, kept free of I/O so they unit-test.

/// Shortest trimmed query, in characters, that runs a search.
pub const MIN_QUERY_CHARS: usize = 2;

/// Most people one search returns, whatever the caller asks for.
pub const MAX_PEOPLE: usize = 50;

/// How well `display_name` matches the lowercased, trimmed `query`, or `None` when it
/// doesn't. `Some(0)`: the whole name starts with the query. `Some(1)`: every word of the
/// query starts some word of the name ("ada lov" in "Ada Lovelace").
pub fn match_rank(query: &str, display_name: &str) -> Option<u8> {
    let name = display_name.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }
    if name.starts_with(query) {
        return Some(0);
    }
    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    query
        .split_whitespace()
        .all(|q| words.iter().any(|w| w.starts_with(q)))
        .then_some(1)
}

/// The first six characters of a bare user id, shown to tell people with the same name
/// apart (the same short id Gauge's principal labels use).
pub fn short_id(user_id: &str) -> String {
    user_id.chars().take(6).collect()
}

/// Whether a display name looks like an email address. Older profiles were seeded with
/// the account email as their display name; search never shows or matches those.
pub fn looks_like_email(display_name: &str) -> bool {
    display_name.contains('@')
}

/// How one search ended, for the `lepton_people_search_total` counter's `outcome` label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOutcome {
    /// Returned at least one person.
    Found,
    /// Ran, and matched nobody.
    Empty,
    /// The query was under [`MIN_QUERY_CHARS`] (or `max_results` was 0).
    ShortQuery,
    /// The caller ran out of searches for now.
    RateLimited,
    /// The caller isn't a signed-in person.
    Anonymous,
    /// A Valence read failed.
    Error,
}

impl SearchOutcome {
    /// Label value recorded on the Spectra counter.
    pub fn label(self) -> &'static str {
        match self {
            Self::Found => "ok",
            Self::Empty => "empty",
            Self::ShortQuery => "short_query",
            Self::RateLimited => "rate_limited",
            Self::Anonymous => "anonymous",
            Self::Error => "error",
        }
    }
}
