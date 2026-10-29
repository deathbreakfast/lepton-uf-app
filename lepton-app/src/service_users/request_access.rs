//! Sentences that send a person to Gauge's request-access form instead of
//! telling them to ask someone.

use leptos::prelude::*;
use uf_product::primitives::*;

use super::port::SERVICE_USER_CREATORS_GROUP;

/// "Request access to the Service user creators group." The first words link
/// to `url` when the host has a permissions UI.
#[component]
pub(super) fn RequestCreatorAccess(url: Option<String>) -> impl IntoView {
    let rest = format!(" to the {SERVICE_USER_CREATORS_GROUP} group.");
    match url {
        Some(href) => view! { <Link href=href>"Request access"</Link>{rest} }.into_any(),
        None => view! { {format!("Request access{rest}")} }.into_any(),
    }
}

/// "Only owners of {group} can add service users to it. You can request access
/// for @{handle} instead." The word "request access" links to `url` when set.
#[component]
pub(super) fn RequestGroupAccess(
    group: String,
    handle: String,
    url: Option<String>,
) -> impl IntoView {
    let lead = format!("Only owners of {group} can add service users to it. You can ");
    let tail = format!(" for @{handle} instead.");
    match url {
        Some(href) => view! { {lead}<Link href=href>"request access"</Link>{tail} }.into_any(),
        None => view! { {format!("{lead}request access{tail}")} }.into_any(),
    }
}
