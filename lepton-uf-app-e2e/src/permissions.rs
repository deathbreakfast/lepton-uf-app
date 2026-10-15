//! Lab wiring for Gauge permissions on the Playwright host.
//!
//! Production hosts sync manifests and install the Gauge-backed permission
//! backend through their own boot (and `gauge_app::wire_gauge_permissions`).
//! This host has no Gauge admin app, so it carries the two small pieces the
//! gated "Sign-in and delivery" page needs.

use gauge::manifest_sync::{PermissionDomainInput, PermissionInput, PermissionManifestInput};
use leptos::prelude::ServerFnError;
use uf_product::AppRegistry;
use valence::Valence;

/// Gauge-backed permission backend for `uf_product` gates.
pub struct LabPermissionBackend;

#[async_trait::async_trait]
impl uf_product::permissions::PermissionBackend for LabPermissionBackend {
    async fn has_permission(&self, permission_name: &str) -> Result<bool, ServerFnError> {
        let ctx = higgs::Higgs::from_request().await?;
        let valence = ctx
            .valence()
            .map_err(|e| ServerFnError::new(format!("Failed to build Valence: {e}")))?;
        gauge::service::actor_can(&valence, permission_name)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to check permission: {e}")))
    }
}

/// Sync every registered app's permission manifest into Gauge tables.
///
/// # Errors
///
/// Returns when Gauge Valence create or lookup fails during sync.
pub async fn sync_registered_manifests(system: &Valence) -> anyhow::Result<()> {
    let mut manifests = Vec::new();
    for registration in AppRegistry::auto_discover().iter() {
        let Some(provider) = registration.permission_manifest else {
            continue;
        };
        let manifest = provider();
        let domains = manifest
            .domains
            .iter()
            .map(|domain| PermissionDomainInput {
                key: domain.key.to_string(),
                name: domain.name.to_string(),
                description: domain.description.to_string(),
                permissions: domain
                    .permissions
                    .iter()
                    .map(|p| PermissionInput {
                        name: p.name.to_string(),
                        description: p.description.to_string(),
                    })
                    .collect(),
            })
            .collect();
        manifests.push(PermissionManifestInput {
            app_id: manifest.app_id.to_string(),
            domains,
        });
    }
    gauge::manifest_sync::sync_permission_manifests(system, &manifests).await?;
    Ok(())
}
