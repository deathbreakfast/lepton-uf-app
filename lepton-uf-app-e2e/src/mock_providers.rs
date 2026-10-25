//! Lab stand-ins for SendGrid and Twilio Verify, plus the control routes
//! Playwright uses to read what was sent and to make a provider fail.
//!
//! The Twilio route also forwards each code to the SMS sink (`UF_SMS_SINK_URL`, default `:8099`), so the
//! existing "read the OTP from the sink" assertions keep working when the cell's
//! text messages go through Twilio Verify settings. Lab-only: never mount these
//! on a production host.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Serialize;

/// One provider call as the lab saw it.
#[derive(Clone, Debug, Serialize)]
pub struct Call {
    /// `sendgrid` or `twilio`.
    pub provider: String,
    /// Raw `Authorization` header value.
    pub authorization: String,
    /// Recipient address or number.
    pub to: String,
}

/// Shared recorder and failure switches.
#[derive(Default)]
pub struct MockProviders {
    calls: Mutex<Vec<Call>>,
    failing: Mutex<HashMap<String, u16>>,
}

impl MockProviders {
    fn record(&self, call: Call) {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }

    fn failure_for(&self, provider: &str) -> Option<u16> {
        self.failing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(provider)
            .copied()
    }
}

fn authorization(headers: &HeaderMap) -> String {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

async fn sendgrid_send(
    Extension(mock): Extension<Arc<MockProviders>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let to = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v.pointer("/personalizations/0/to/0/email")
                .and_then(|e| e.as_str().map(str::to_string))
        })
        .unwrap_or_default();
    mock.record(Call {
        provider: "sendgrid".into(),
        authorization: authorization(&headers),
        to,
    });
    mock.failure_for("sendgrid")
        .and_then(|s| StatusCode::from_u16(s).ok())
        .unwrap_or(StatusCode::ACCEPTED)
}

async fn twilio_verifications(
    Extension(mock): Extension<Arc<MockProviders>>,
    Path(_service_sid): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    let form: HashMap<String, String> = url::form_urlencoded::parse(&body)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let to = form.get("To").cloned().unwrap_or_default();
    let code = form.get("CustomCode").cloned();
    mock.record(Call {
        provider: "twilio".into(),
        authorization: authorization(&headers),
        to: to.clone(),
    });
    if let Some(status) = mock
        .failure_for("twilio")
        .and_then(|s| StatusCode::from_u16(s).ok())
    {
        return (
            status,
            Json(serde_json::json!({ "code": 20003, "message": "lab failure" })),
        );
    }
    // Forward to the SMS sink so OTP assertions that read it keep working.
    let payload = serde_json::json!({
        "to_e164": to,
        "body": code.as_deref().map(|c| format!("Your verification code is: {c}")).unwrap_or_default(),
        "otp_code": code,
    });
    if let Ok(client) = reqwest::Client::builder().build() {
        let _ = client
            .post(format!(
                "{}/v1/messages",
                crate::boot::lab_env("UF_SMS_SINK_URL", "http://127.0.0.1:8099")
            ))
            .json(&payload)
            .send()
            .await;
    }
    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "sid": "VElab" })),
    )
}

async fn list_calls(Extension(mock): Extension<Arc<MockProviders>>) -> Json<Vec<Call>> {
    Json(
        mock.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
    )
}

async fn reset(Extension(mock): Extension<Arc<MockProviders>>) -> StatusCode {
    mock.calls
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    mock.failing
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    StatusCode::NO_CONTENT
}

async fn fail(
    Extension(mock): Extension<Arc<MockProviders>>,
    Path((provider, status)): Path<(String, u16)>,
) -> StatusCode {
    mock.failing
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(provider, status);
    StatusCode::NO_CONTENT
}

/// Routes for the mock providers and their control endpoints.
pub fn routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/mock/sendgrid/v3/mail/send", post(sendgrid_send))
        .route(
            "/mock/twilio/v2/Services/{service_sid}/Verifications",
            post(twilio_verifications),
        )
        .route("/api/test/mock-providers", get(list_calls))
        .route("/api/test/mock-providers/reset", post(reset))
        .route(
            "/api/test/mock-providers/fail/{provider}/{status}",
            post(fail),
        )
}
