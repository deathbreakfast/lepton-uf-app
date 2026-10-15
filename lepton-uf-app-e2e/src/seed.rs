//! Thin mount of [`lepton_test_support`] seed HTTP for Playwright.
//!
//! Domain logic lives in `lepton-test-support`. Do not mount this route on
//! production product hosts.

use std::sync::Arc;

use lepton_test_support::{SeedError, SeedValence};
use valence::Valence;

use crate::boot::{system_valence, AppState};

pub use lepton_test_support::{seed_data, SeedRequest, SeedResponse};

impl SeedValence for AppState {
    fn seed_valence(&self) -> Result<Valence, SeedError> {
        system_valence(Arc::clone(&self.valence_router), &self.default_backend_key).map_err(|_| {
            SeedError::Persistence {
                operation: "system_valence",
            }
        })
    }
}

/// Request body for [`delivery_admin`].
#[derive(serde::Deserialize)]
pub struct DeliveryAdminRequest {
    /// Email of an existing user to add to `lepton.delivery.admins`.
    pub email: String,
}

/// Lab-only: add the user with `email` to the delivery admins group.
///
/// Stands in for what an operator does on the cell (a super user adds a member).
/// Do not mount on a production host.
pub async fn delivery_admin(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(body): axum::Json<DeliveryAdminRequest>,
) -> axum::http::StatusCode {
    use axum::http::StatusCode;
    use valence::Model;

    let Ok(system) = system_valence(
        Arc::clone(&state.valence_router),
        &state.default_backend_key,
    ) else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let group = gauge::generated::PermissionGroup::get(
        uf_lepton_delivery_boot::DELIVERY_ADMINS_GROUP_ID,
        &system,
        valence::use_!(r"**Test:** Lab host **loads the delivery admins group** so a Playwright run can add a user to it, standing in for an operator doing that on a cell. Only the lab harness uses this route."),
    )
    .await;
    let Ok(Some(group)) = group else {
        return StatusCode::NOT_FOUND;
    };
    match gauge::super_user::seed_super_user_member_by_email(&system, &group, &body.email).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::NOT_FOUND,
    }
}
