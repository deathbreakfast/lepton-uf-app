//! The `lepton_people` search source: display-name search over user profiles.

// The enum and its `key()` exist without `ssr` so hydrate UI can build picker source
// lists; the provider submission stays behind `ssr` inside the macro expansion. The macro
// takes no doc attributes, so the generated items live in an undocumented module and
// `PeopleSearchSourceId` is documented where it's re-exported.
#[allow(missing_docs)]
mod ids {
    #[cfg(feature = "ssr")]
    use super::PeopleSearchSource;

    uf_product_macros::define_search_sources! {
        enum PeopleSearchSourceId {
            People => {
                id: "lepton_people",
                label: "People",
                description: "Searches people by display name",
                provider: PeopleSearchSource
            }
        }
    }
}

/// Registered id of the people search source. `PeopleSearchSourceId::People.key()` names
/// it for `SearchSourcePicker`; the id string is [`crate::people::sources::PEOPLE`].
pub use ids::PeopleSearchSourceId;

#[cfg(feature = "ssr")]
mod rules;

#[cfg(feature = "ssr")]
pub mod rate_limit;

#[cfg(feature = "ssr")]
#[allow(missing_docs)]
mod metric {
    use spectra::spectra_metric;

    spectra_metric! {
        LeptonPeopleSearchTotal {
            store: "default",
            name: "lepton_people_search_total",
            version: "0.1.0",
            description: "Counter: people searches run through the lepton_people search source (person pickers). A rise in rate_limited means someone is paging through the directory; a rise in error points at Valence reads of user profiles. Labels: outcome (ok, empty, short_query, rate_limited, anonymous, error).",
        }
    }
}

#[cfg(feature = "ssr")]
mod ssr {
    use std::time::Instant;

    use lepton_identity::generated::{User, UserProfile, UserStatus};
    use uf_search_core::{SearchSourceError, SearchSourceFuture, SearchSourceItem};
    use valence::ownership::OwnershipService;
    use valence::{Model, Valence};

    use super::metric::LeptonPeopleSearchTotalRecorder;
    use super::rate_limit::PEOPLE_SEARCH_LIMITER;
    use super::rules::{
        looks_like_email, match_rank, short_id, SearchOutcome, MAX_PEOPLE, MIN_QUERY_CHARS,
    };
    use crate::people::sources::PEOPLE;

    /// [`uf_search_core::SearchSourceProvider`] over people's display names. Runs with the
    /// caller's request Valence and never switches to a system actor.
    pub struct PeopleSearchSource;

    /// Bare id of `rid`, without the table prefix.
    fn bare(rid: &valence::RecordId) -> String {
        valence::extract_id_from_record(rid).unwrap_or_else(|_| rid.id().to_string())
    }

    /// Whether the user is still listed: not mid-deletion and, where the viewer may read
    /// the account status, active.
    async fn is_listed(v: &Valence, user_id: &str) -> Result<bool, SearchSourceError> {
        match OwnershipService::pending_deletion_gate("user", user_id, v).await {
            Ok(()) => {}
            Err(valence::Error::PendingDeletion(_)) => return Ok(false),
            Err(e) => return Err(e.into()),
        }
        let user = User::get(
            user_id,
            v,
            valence::use_!(r"In **people search**, we **read the person's account status** so suspended or unverified accounts are left out of person pickers. Only the status is used, and only where the viewer may already read it."),
        )
        .await?;
        Ok(match user {
            None => false,
            Some(user) => !matches!(
                user.status(),
                Some(UserStatus::Disabled | UserStatus::PendingVerification)
            ),
        })
    }

    async fn search(
        v: &Valence,
        query_text: &str,
        max_results: u32,
    ) -> (
        SearchOutcome,
        Result<Vec<SearchSourceItem>, SearchSourceError>,
    ) {
        // Only a signed-in person gets rows; System and anonymous callers get nothing.
        let Some(actor_id) = v
            .actor()
            .user_id()
            .map(|id| id.split_once(':').map_or(id, |(_, bare)| bare).to_string())
        else {
            return (SearchOutcome::Anonymous, Ok(Vec::new()));
        };
        let query = query_text.trim().to_lowercase();
        if max_results == 0 || query.chars().count() < MIN_QUERY_CHARS {
            return (SearchOutcome::ShortQuery, Ok(Vec::new()));
        }
        if !PEOPLE_SEARCH_LIMITER.try_take(&actor_id, Instant::now()) {
            return (
                SearchOutcome::RateLimited,
                Err(SearchSourceError::msg(
                    "Too many people searches. Try again in a moment.",
                )),
            );
        }
        let cap = (max_results as usize).min(MAX_PEOPLE);
        match collect(v, &query, cap).await {
            Ok(items) if items.is_empty() => (SearchOutcome::Empty, Ok(items)),
            Ok(items) => (SearchOutcome::Found, Ok(items)),
            Err(e) => (SearchOutcome::Error, Err(e)),
        }
    }

    async fn collect(
        v: &Valence,
        query: &str,
        cap: usize,
    ) -> Result<Vec<SearchSourceItem>, SearchSourceError> {
        // In-memory backends only apply equality predicates and no backend matches
        // case-insensitively, so read the profiles the caller may see and match here. The
        // query clamps the fetch to the platform maximum until Lepton has a name index.
        let profiles = UserProfile::query(
            v,
            valence::use_!(r"In **people search**, we **list profiles by display name** so a person picker can find someone by the start of their name. Only display names are shown; email addresses and legal names are never read for this."),
        )
        .limit(valence::MAX_QUERY_LIMIT)
        .await?;

        let mut ranked: Vec<(u8, String, UserProfile)> = profiles
            .into_iter()
            .filter(|p| !looks_like_email(p.display_name()))
            .filter_map(|p| {
                let rank = match_rank(query, p.display_name())?;
                Some((rank, p.display_name().trim().to_lowercase(), p))
            })
            .collect();
        ranked.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));

        let mut out = Vec::new();
        for (_, _, profile) in ranked {
            let user_id = bare(profile.user_thing());
            if !is_listed(v, &user_id).await? {
                continue;
            }
            out.push(SearchSourceItem {
                source_id: PEOPLE.to_string(),
                id: user_id.clone(),
                title: profile.display_name().trim().to_string(),
                description: Some(short_id(&user_id)),
                kind: "user".to_string(),
                image_url: None,
            });
            if out.len() >= cap {
                break;
            }
        }
        Ok(out)
    }

    impl uf_search_core::SearchSourceProvider for PeopleSearchSource {
        fn query<'a>(
            &'a self,
            v: &'a Valence,
            query_text: &'a str,
            max_results: u32,
        ) -> SearchSourceFuture<'a> {
            Box::pin(async move {
                let (outcome, result) = search(v, query_text, max_results).await;
                LeptonPeopleSearchTotalRecorder::record(
                    1,
                    serde_json::json!({ "outcome": outcome.label() }),
                );
                result
            })
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::PeopleSearchSource;

#[cfg(all(test, feature = "ssr"))]
mod tests;
