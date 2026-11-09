//! Lab-only seeding for the person picker specs. Do not mount on a production host.
//!
//! This host doesn't link Polaron, so it registers a stand-in `polaron_team` search source
//! over teams the specs seed in memory. `POST /api/test/person-picker` names a seeded
//! person or adds a team.

use std::sync::{Arc, Mutex};

use lepton_identity::generated::UserProfile;
use uf_search_core::{SearchSourceFuture, SearchSourceItem};
use valence::{Model, RecordPredicate, Valence};

struct LabTeam {
    id: String,
    name: String,
    members: usize,
}

static LAB_TEAMS: Mutex<Vec<LabTeam>> = Mutex::new(Vec::new());

/// Stand-in for Polaron's team search: teams whose name has a word starting with the
/// query, for signed-in callers only.
pub struct LabTeamSearchSource;

uf_product_macros::define_search_sources! {
    enum LabTeamSearchSourceId {
        Teams => {
            id: "polaron_team",
            label: "Teams",
            description: "Lab stand-in for Polaron's team search",
            provider: LabTeamSearchSource
        }
    }
}

fn word_starts_with(name: &str, needle: &str) -> bool {
    name.to_lowercase()
        .split_whitespace()
        .any(|word| word.starts_with(needle))
}

impl uf_search_core::SearchSourceProvider for LabTeamSearchSource {
    fn query<'a>(
        &'a self,
        v: &'a Valence,
        query_text: &'a str,
        max_results: u32,
    ) -> SearchSourceFuture<'a> {
        Box::pin(async move {
            let needle = query_text.trim().to_lowercase();
            if v.actor().user_id().is_none() || needle.chars().count() < 2 {
                return Ok(Vec::new());
            }
            let teams = LAB_TEAMS.lock().map_err(|_| {
                uf_search_core::SearchSourceError::msg("lab team store is poisoned")
            })?;
            Ok(teams
                .iter()
                .filter(|t| word_starts_with(&t.name, &needle))
                .take(max_results as usize)
                .map(|t| SearchSourceItem {
                    source_id: lepton_shell::people::sources::TEAMS.to_string(),
                    id: t.id.clone(),
                    title: t.name.clone(),
                    description: Some(format!("{} members", t.members)),
                    kind: "team".to_string(),
                    image_url: None,
                })
                .collect())
        })
    }
}

/// Request body for [`person_picker_lab`].
#[derive(serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PersonPickerLabRequest {
    /// Set the display name of the already-seeded user with `email`.
    Person { email: String, display_name: String },
    /// Add a team to the stand-in team source.
    Team { name: String, members: usize },
}

async fn name_person(system: &Valence, email: &str, display_name: &str) -> anyhow::Result<String> {
    let user_id = crate::service_users::user_id_for_email(system, email).await?;
    let user_thing = valence::RecordId::new("user", user_id.as_str());
    let existing = UserProfile::query(system, valence::use_!(r"**Test:** Lab host **finds the profile** of a user a Playwright run just seeded, so it can give them a display name to search for. Only the lab harness uses this."))
        .where_user(RecordPredicate::Equals(user_thing.clone()))
        .first()
        .await?;
    match existing {
        Some(profile) => {
            profile
                .get_mutable(system, valence::use_!(r"**Test:** Lab host **sets a seeded user's display name** so the person picker specs can search for them. Only the lab harness uses this."))
                .set_display_name(display_name.to_string())
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .commit()
                .await?;
        }
        None => {
            let now = chrono::Utc::now();
            let profile = UserProfile::new(
                user_thing,
                display_name.to_string(),
                display_name.to_string(),
                now,
                now,
                None,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            UserProfile::create(profile, system, valence::use_!(r"**Test:** Lab host **creates a profile** with a display name for a seeded user so the person picker specs can search for them. Only the lab harness uses this."))
                .await?;
        }
    }
    Ok(user_id)
}

/// Lab-only seed route for the person picker specs.
pub async fn person_picker_lab(
    axum::extract::State(state): axum::extract::State<crate::boot::AppState>,
    axum::Json(body): axum::Json<PersonPickerLabRequest>,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let fail = |e: anyhow::Error| (axum::http::StatusCode::BAD_REQUEST, format!("{e:#}"));
    match body {
        PersonPickerLabRequest::Person {
            email,
            display_name,
        } => {
            let system = crate::boot::system_valence(
                Arc::clone(&state.valence_router),
                &state.default_backend_key,
            )
            .map_err(fail)?;
            let user_id = name_person(&system, &email, &display_name)
                .await
                .map_err(fail)?;
            Ok(axum::Json(serde_json::json!({ "user_id": user_id })))
        }
        PersonPickerLabRequest::Team { name, members } => {
            let mut teams = LAB_TEAMS
                .lock()
                .map_err(|_| fail(anyhow::anyhow!("lab team store is poisoned")))?;
            let id = format!("team-{}", teams.len() + 1);
            teams.push(LabTeam {
                id: id.clone(),
                name,
                members,
            });
            Ok(axum::Json(serde_json::json!({ "id": id })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::word_starts_with;

    #[test]
    fn team_matches_word_start_happy() {
        assert!(word_starts_with("Atlas Platform crew", "plat"));
    }

    #[test]
    fn team_ignores_mid_word_match_sad() {
        assert!(!word_starts_with("Atlas Platform crew", "form"));
    }
}
