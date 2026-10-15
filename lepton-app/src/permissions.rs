//! Permission manifest for the Lepton settings app.

use uf_product_macros::UfPermissionManifest;

/// Permissions Lepton's own settings pages declare.
///
/// Synced into the `lepton` domain at host boot. Server functions gate with
/// `#[uf_product_macros::server(permission = "LeptonManageDelivery")]`.
#[allow(clippy::expl_impl_clone_on_copy)]
#[derive(UfPermissionManifest)]
#[permission_manifest(
    domain_key = "lepton",
    domain_name = "Lepton",
    domain_description = "Sign-in and delivery administration"
)]
pub enum LeptonPermission {
    /// Change email, text-message, OAuth provider and authenticator key settings.
    #[permission(description = "Manage sign-in and delivery settings")]
    LeptonManageDelivery,
}
