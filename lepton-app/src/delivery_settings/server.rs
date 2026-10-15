//! Server functions behind the "Sign-in and delivery" page.
//!
//! Each one recovers the signed-in operator through Higgs and hands that
//! request's Valence to the host's [`DeliverySettingsBackend`]. Nothing here
//! rebinds to System. Writes also need a fresh authenticator code, because a
//! changed SMTP host or provider key can redirect password-reset mail.

use lepton_auth::delivery_settings::{
    DeliverySettingsView, EmailCandidate, OAuthCandidate, SmsCandidate,
};
use leptos::prelude::*;

#[cfg(feature = "ssr")]
use std::sync::Arc;

#[cfg(feature = "ssr")]
use lepton_auth::delivery_settings::{DeliveryError, DeliverySettingsBackend};

#[cfg(feature = "ssr")]
fn backend() -> Result<Arc<dyn DeliverySettingsBackend>, ServerFnError> {
    use_context::<Arc<dyn DeliverySettingsBackend>>().ok_or_else(|| {
        ServerFnError::new("reason_class=not_configured: sign-in and delivery settings are not set up on this host")
    })
}

#[cfg(feature = "ssr")]
fn map_err(e: DeliveryError) -> ServerFnError {
    ServerFnError::new(e.to_string())
}

#[cfg(feature = "ssr")]
async fn request_valence() -> Result<valence::Valence, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    ctx.valence()
        .map_err(|e| ServerFnError::new(format!("reason_class=session: {e}")))
}

#[cfg(feature = "ssr")]
async fn require_fresh_code(totp_code: &str) -> Result<(), ServerFnError> {
    lepton_auth::verify_fresh_totp(totp_code)
        .await
        .map_err(|e| e.to_server_fn_error())
}

/// Current settings. Secrets appear only as set or not set plus a last-changed time.
#[uf_product_macros::server(permission = "LeptonManageDelivery")]
pub async fn get_delivery_settings() -> Result<DeliverySettingsView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let v = request_valence().await?;
        backend()?.view(&v).await.map_err(map_err)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Send one test email with the values on the form. Saves nothing.
#[uf_product_macros::server(permission = "LeptonManageDelivery")]
pub async fn test_send_email(
    /// Values to test.
    candidate: EmailCandidate,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let v = request_valence().await?;
        backend()?.test_email(&v, &candidate).await.map_err(map_err)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = candidate;
        Err(ServerFnError::new("server only"))
    }
}

/// Send one test text message with the values on the form. Saves nothing.
#[uf_product_macros::server(permission = "LeptonManageDelivery")]
pub async fn test_send_sms(
    /// Values to test.
    candidate: SmsCandidate,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let v = request_valence().await?;
        backend()?.test_sms(&v, &candidate).await.map_err(map_err)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = candidate;
        Err(ServerFnError::new("server only"))
    }
}

/// Test, then save, email settings.
#[uf_product_macros::server(permission = "LeptonManageDelivery", step_up = "fresh")]
pub async fn save_email_settings(
    /// Values to save.
    candidate: EmailCandidate,
    /// Fresh authenticator code.
    totp_code: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        require_fresh_code(&totp_code).await?;
        let v = request_valence().await?;
        backend()?.save_email(&v, candidate).await.map_err(map_err)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (candidate, totp_code);
        Err(ServerFnError::new("server only"))
    }
}

/// Test, then save, text-message settings.
#[uf_product_macros::server(permission = "LeptonManageDelivery", step_up = "fresh")]
pub async fn save_sms_settings(
    /// Values to save.
    candidate: SmsCandidate,
    /// Fresh authenticator code.
    totp_code: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        require_fresh_code(&totp_code).await?;
        let v = request_valence().await?;
        backend()?.save_sms(&v, candidate).await.map_err(map_err)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (candidate, totp_code);
        Err(ServerFnError::new("server only"))
    }
}

/// Save OAuth provider settings.
#[uf_product_macros::server(permission = "LeptonManageDelivery", step_up = "fresh")]
pub async fn save_oauth_settings(
    /// Values to save.
    candidate: OAuthCandidate,
    /// Fresh authenticator code.
    totp_code: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        require_fresh_code(&totp_code).await?;
        let v = request_valence().await?;
        backend()?.save_oauth(&v, candidate).await.map_err(map_err)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (candidate, totp_code);
        Err(ServerFnError::new("server only"))
    }
}

/// Create the next authenticator seal key version.
#[uf_product_macros::server(permission = "LeptonManageDelivery", step_up = "fresh")]
pub async fn rotate_totp_seal_key(
    /// Fresh authenticator code.
    totp_code: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        require_fresh_code(&totp_code).await?;
        let v = request_valence().await?;
        backend()?.rotate_totp_key(&v).await.map_err(map_err)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = totp_code;
        Err(ServerFnError::new("server only"))
    }
}
