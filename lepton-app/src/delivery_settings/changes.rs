//! What one press of Save sends, and what came back for each section.
//!
//! The page keeps every section's form values and compares them with what it
//! loaded. Only sections that differ go to the server, so an unchanged email
//! section never sends a test message. One authenticator code covers the whole
//! save, because a code can be used only once.

use lepton_auth::delivery_settings::{
    EmailCandidate, EmailSettingsView, OAuthCandidate, OAuthSettingsView, SecretStatus,
    SmsCandidate, SmsSettingsView,
};
use serde::{Deserialize, Serialize};

/// Port the form shows when none is stored yet.
pub const DEFAULT_SMTP_PORT: u16 = 587;

/// Sections the operator changed. `None` means leave that section alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryChanges {
    /// New email settings.
    pub email: Option<EmailCandidate>,
    /// New text-message settings.
    pub sms: Option<SmsCandidate>,
    /// New sign-in provider settings.
    pub oauth: Option<OAuthCandidate>,
}

impl DeliveryChanges {
    /// True when no section changed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.email.is_none() && self.sms.is_none() && self.oauth.is_none()
    }
}

/// What happened to one section on Save.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectionOutcome {
    /// The section had no changes and was not sent.
    #[default]
    Unchanged,
    /// The section was tested (where it sends anything) and saved.
    Saved,
    /// The section was not saved; the text is the server error.
    Failed(String),
}

/// One [`SectionOutcome`] per section.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliverySaveReport {
    /// Email.
    pub email: SectionOutcome,
    /// Text messages.
    pub sms: SectionOutcome,
    /// Sign-in providers.
    pub oauth: SectionOutcome,
}

impl DeliverySaveReport {
    /// True when at least one section was saved, so the page should reload.
    #[must_use]
    pub fn any_saved(&self) -> bool {
        [&self.email, &self.sms, &self.oauth]
            .into_iter()
            .any(|o| *o == SectionOutcome::Saved)
    }
}

/// Email form values as loaded, with no new secrets typed.
#[must_use]
pub fn email_baseline(view: &EmailSettingsView) -> EmailCandidate {
    EmailCandidate {
        driver: view.driver,
        smtp_host: view.smtp_host.clone(),
        smtp_port: if view.smtp_port == 0 {
            DEFAULT_SMTP_PORT
        } else {
            view.smtp_port
        },
        smtp_tls: view.smtp_tls,
        smtp_username: view.smtp_username.clone(),
        from_email: view.from_email.clone(),
        from_name: view.from_name.clone(),
        smtp_password: None,
        sendgrid_api_key: None,
        test_recipient: String::new(),
    }
}

/// Text-message form values as loaded, with no new secret typed.
#[must_use]
pub fn sms_baseline(view: &SmsSettingsView) -> SmsCandidate {
    SmsCandidate {
        mode: view.mode,
        twilio_account_sid: view.twilio_account_sid.clone(),
        twilio_api_key_sid: view.twilio_api_key_sid.clone(),
        twilio_verify_service_sid: view.twilio_verify_service_sid.clone(),
        twilio_from: view.twilio_from.clone(),
        twilio_api_secret: None,
        test_recipient: String::new(),
    }
}

/// Sign-in provider values as loaded, with no new secrets typed.
#[must_use]
pub fn oauth_baseline(view: &OAuthSettingsView) -> OAuthCandidate {
    OAuthCandidate {
        google_client_id: view.google.client_id.clone(),
        github_client_id: view.github.client_id.clone(),
        google_client_secret: None,
        github_client_secret: None,
    }
}

/// `current` when it differs from `baseline` in anything but the test recipient.
#[must_use]
pub fn email_change(baseline: &EmailCandidate, current: EmailCandidate) -> Option<EmailCandidate> {
    let compared = EmailCandidate {
        test_recipient: String::new(),
        ..current.clone()
    };
    (compared != *baseline).then_some(current)
}

/// `current` when it differs from `baseline` in anything but the test recipient.
#[must_use]
pub fn sms_change(baseline: &SmsCandidate, current: SmsCandidate) -> Option<SmsCandidate> {
    let compared = SmsCandidate {
        test_recipient: String::new(),
        ..current.clone()
    };
    (compared != *baseline).then_some(current)
}

/// `current` when it differs from `baseline`.
#[must_use]
pub fn oauth_change(baseline: &OAuthCandidate, current: OAuthCandidate) -> Option<OAuthCandidate> {
    (current != *baseline).then_some(current)
}

/// Status line for a write-only secret row.
///
/// `pending` is true once the operator typed a new value that Save has not sent yet.
#[must_use]
pub fn secret_status_text(status: &SecretStatus, pending: bool) -> String {
    if pending {
        return "New value ready to save".to_string();
    }
    match (status.is_set, status.rotated_at) {
        (true, Some(at)) => format!("Set, rotated {}", at.format("%Y-%m-%d")),
        (true, None) => "Set".to_string(),
        (false, _) => "Not set".to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use lepton_auth::delivery_settings::{EmailDriverChoice, WriteOnlySecret};

    #[test]
    fn loaded_values_with_only_a_test_recipient_are_not_a_change() {
        let view = EmailSettingsView {
            driver: EmailDriverChoice::Smtp,
            smtp_host: "smtp.example.com".into(),
            ..Default::default()
        };
        let base = email_baseline(&view);
        let current = EmailCandidate {
            test_recipient: "ops@example.com".into(),
            ..base.clone()
        };
        assert_eq!(email_change(&base, current), None);
    }

    #[test]
    fn an_unset_port_shows_and_compares_as_the_default() {
        let base = email_baseline(&EmailSettingsView::default());
        assert_eq!(base.smtp_port, DEFAULT_SMTP_PORT);
        assert_eq!(email_change(&base, base.clone()), None);
    }

    #[test]
    fn a_typed_secret_alone_is_a_change_and_keeps_the_test_recipient() {
        let base = sms_baseline(&SmsSettingsView::default());
        let current = SmsCandidate {
            twilio_api_secret: Some(WriteOnlySecret::new("s3cret")),
            test_recipient: "+15550100".into(),
            ..base.clone()
        };
        let sent = sms_change(&base, current).expect("a new secret is a change");
        assert_eq!(sent.test_recipient, "+15550100");
    }

    #[test]
    fn a_changed_client_id_is_an_oauth_change() {
        let base = oauth_baseline(&OAuthSettingsView::default());
        let current = OAuthCandidate {
            google_client_id: "1234-abcd.apps".into(),
            ..base.clone()
        };
        assert!(oauth_change(&base, current).is_some());
        assert_eq!(oauth_change(&base, base.clone()), None);
    }

    #[test]
    fn empty_changes_and_reports() {
        assert!(DeliveryChanges::default().is_empty());
        assert!(!DeliverySaveReport::default().any_saved());
        let report = DeliverySaveReport {
            sms: SectionOutcome::Saved,
            email: SectionOutcome::Failed("x".into()),
            ..Default::default()
        };
        assert!(report.any_saved());
    }

    #[test]
    fn secret_status_never_holds_a_value() {
        let at = chrono::DateTime::parse_from_rfc3339("2026-10-02T08:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let set = SecretStatus {
            is_set: true,
            rotated_at: Some(at),
        };
        assert_eq!(secret_status_text(&set, false), "Set, rotated 2026-10-02");
        assert_eq!(secret_status_text(&set, true), "New value ready to save");
        assert_eq!(
            secret_status_text(&SecretStatus::default(), false),
            "Not set"
        );
    }
}
