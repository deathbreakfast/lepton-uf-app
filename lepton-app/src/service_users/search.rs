//! Search source that lists the service users the signed-in person can read.
//!
//! Other products (a team's Add people dialog, for example) pass
//! [`SERVICE_USER_SEARCH_SOURCE_ID`] to `uf_integrations::SearchSourcePicker` to let
//! someone pick one of their service users. Each hit carries the service user's
//! display name as its title and its owner in the description.
//!
//! Who sees what follows Lepton's read rule for service users. An owner sees their own;
//! a user the host's [`ServiceUserGate`](lepton_identity::service_users::ServiceUserGate)
//! grants manage-any sees every owner's. Nobody else gets a row. A service user acting
//! with its own token gets nothing at all, and neither does a service user whose
//! deletion is still running. The query runs with the caller's request Valence and never
//! switches to a system actor.
//!
//! ```rust,ignore
//! use lepton_app::service_users::ServiceUserSearchSourceId;
//! use uf_integrations::SearchSourcePicker;
//! use leptos::prelude::*;
//!
//! // `ServiceUserSearchSourceId::ServiceUser.key()` names the source for the picker;
//! // the id string is `lepton_app::service_users::SERVICE_USER_SEARCH_SOURCE_ID`.
//! let sources = vec![ServiceUserSearchSourceId::ServiceUser.key()];
//! # let _ = sources;
//! ```

/// Stable id of the service user search source, for `SearchSourcePicker` and
/// `SearchSourceKey`.
pub const SERVICE_USER_SEARCH_SOURCE_ID: &str = "service_user_search_source";

// The enum and its `key()` exist without `ssr` so hydrate UI can build picker source
// lists; the provider submission stays behind `ssr` inside the macro expansion.
uf_product_macros::define_search_sources! {
    enum ServiceUserSearchSourceId {
        ServiceUser => {
            id: "service_user_search_source",
            label: "Service users",
            description: "Searches the service users you can see",
            provider: ServiceUserSearchSource
        }
    }
}

/// Whether a service user's name or handle contains the lowercased `needle`.
#[cfg(feature = "ssr")]
fn matches(needle: &str, display_name: &str, handle: &str) -> bool {
    needle.is_empty()
        || display_name.to_lowercase().contains(needle)
        || handle.to_lowercase().contains(needle)
}

#[cfg(feature = "ssr")]
mod ssr {
    use lepton_identity::generated::{ServiceUserAccount, ServiceUserAccountStatus, UserProfile};
    use lepton_identity::service_users::installed_service_user_gate;
    use uf_search_core::{SearchSourceError, SearchSourceFuture, SearchSourceItem};
    use valence::ownership::OwnershipService;
    use valence::Valence;

    use super::{matches, SERVICE_USER_SEARCH_SOURCE_ID};

    /// [`uf_search_core::SearchSourceProvider`] over the service users the caller can read.
    pub struct ServiceUserSearchSource;

    /// Bare id of `rid`, without the table prefix.
    fn bare(rid: &valence::RecordId) -> String {
        valence::extract_id_from_record(rid).unwrap_or_else(|_| rid.id().to_string())
    }

    /// Display name of `owner_id`, or `None` when the caller cannot read it.
    async fn owner_name(v: &Valence, owner_id: &str) -> Option<String> {
        let profiles = UserProfile::get_from_user_id(
            owner_id,
            v,
            valence::use_!(r"In the **service user** search, we **read the owner's display name** so the person choosing a service user can tell whose it is. People who may read that profile see the name; otherwise the owner shows as another user."),
        )
        .await
        .ok()?;
        profiles
            .first()
            .map(|p| p.display_name().trim().to_string())
            .filter(|name| !name.is_empty())
    }

    impl uf_search_core::SearchSourceProvider for ServiceUserSearchSource {
        fn query<'a>(
            &'a self,
            v: &'a Valence,
            query_text: &'a str,
            max_results: u32,
        ) -> SearchSourceFuture<'a> {
            Box::pin(async move {
                // Only a signed-in person gets rows. A service user's token, System and
                // anonymous callers get nothing, whatever Valence would let them read.
                let Some(actor_id) = v
                    .actor()
                    .user_id()
                    .map(|id| id.split_once(':').map_or(id, |(_, bare)| bare).to_string())
                else {
                    return Ok(Vec::new());
                };
                if max_results == 0 {
                    return Ok(Vec::new());
                }
                let manage_any = match installed_service_user_gate() {
                    Some(gate) => gate
                        .may_manage_any_service_user(v)
                        .await
                        .map_err(|e| SearchSourceError::msg(e.to_string()))?,
                    None => false,
                };
                let needle = query_text.trim().to_lowercase();
                // In-memory backends only apply equality predicates, so fetch the rows the
                // caller may read and filter here. Valence's read rule already scopes this to
                // the caller's own service users (or every owner's, for an administrator). An
                // undersized window would silently drop matches that sort after it, so take the
                // whole readable set; the query clamps the fetch to the platform maximum.
                let rows = ServiceUserAccount::query(
                    v,
                    valence::use_!(r"In the **service user** search, we **list service users** so a person can pick one of their own (or any, for an administrator) and show it by display name and owner. Service users the person may not read are never included."),
                )
                .limit(valence::MAX_QUERY_LIMIT)
                .await?;

                let mut out = Vec::new();
                for row in rows {
                    let (Some(rid), Some(owner)) = (row.id(), row.user()) else {
                        continue;
                    };
                    let id = bare(rid);
                    let owner_id = bare(owner);
                    // Defence in depth beside Valence's read rule: own rows, or all with
                    // manage-any.
                    if !manage_any && owner_id != actor_id {
                        continue;
                    }
                    if !matches(&needle, row.display_name(), row.handle()) {
                        continue;
                    }
                    // A deleted service user stays in query results until its deletion run
                    // finishes; `ServiceUserAccount::get` hides it, so search must too.
                    match OwnershipService::pending_deletion_gate("service_user_account", &id, v)
                        .await
                    {
                        Ok(()) => {}
                        Err(valence::Error::PendingDeletion(_)) => continue,
                        Err(e) => return Err(e.into()),
                    }
                    let owner_label = if owner_id == actor_id {
                        "you".to_string()
                    } else {
                        owner_name(v, &owner_id)
                            .await
                            .unwrap_or_else(|| "another user".to_string())
                    };
                    let paused = matches!(row.status(), ServiceUserAccountStatus::Suspended);
                    out.push(SearchSourceItem {
                        source_id: SERVICE_USER_SEARCH_SOURCE_ID.to_string(),
                        id,
                        title: row.display_name().clone(),
                        description: Some(format!(
                            "Owned by {owner_label}{}",
                            if paused { " (paused)" } else { "" }
                        )),
                        kind: "service_user".to_string(),
                    });
                    if out.len() >= max_results as usize {
                        break;
                    }
                }
                Ok(out)
            })
        }
    }
}

#[cfg(feature = "ssr")]
pub use ssr::ServiceUserSearchSource;

#[cfg(all(test, feature = "ssr"))]
mod tests;
