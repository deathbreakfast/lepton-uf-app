//! Run a write that needs a recent authenticator confirmation.
//!
//! Group changes ask for a short confirmation window, the same one Gauge's
//! group screens use. The first try runs straight away; when the server asks
//! for confirmation, the step-up dialog opens, and the write runs once more
//! after the code is accepted.

use lepton_auth::actions::step_up::verify_step_up_totp;
use lepton_auth_ui::{use_step_up_controller, StepUpFactors, StepUpPolicy, StepUpRequest};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;

fn is_challenge(err: &ServerFnError) -> bool {
    let text = err.to_string();
    text.contains("STEP_UP:step_up_required") || text.contains("STEP_UP:step_up_expired")
}

fn request() -> StepUpRequest {
    StepUpRequest {
        title: "Confirm it's you".to_string(),
        description: Some(
            "Enter the code from your authenticator app to add your bot to a group.".to_string(),
        ),
        policy: StepUpPolicy::Totp,
    }
}

/// Run `action`; on a confirmation request, open step-up, then retry once.
///
/// `on_ok` runs after success. `error` receives the server's message when the
/// call fails for another reason, or when the person closes the dialog.
pub fn spawn_with_step_up<F, Fut>(error: RwSignal<Option<String>>, on_ok: Callback<()>, action: F)
where
    F: Fn() -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), ServerFnError>> + 'static,
{
    let ctrl = use_step_up_controller();
    spawn_local_scoped(async move {
        match action().await {
            Ok(()) => on_ok.run(()),
            Err(err) if is_challenge(&err) => {
                let Some(ctrl) = ctrl else {
                    error.set(Some(err.to_string()));
                    return;
                };
                // Shown if the dialog is closed without confirming.
                error.set(Some(err.to_string()));
                ctrl.request(
                    request(),
                    Callback::new(move |factors: StepUpFactors| {
                        let action = action.clone();
                        spawn_local_scoped(async move {
                            if let Err(e) = verify_step_up_totp(factors.totp_code).await {
                                ctrl.report_error(e.to_string());
                                return;
                            }
                            match action().await {
                                Ok(()) => {
                                    ctrl.complete_success();
                                    error.set(None);
                                    on_ok.run(());
                                }
                                Err(e) if is_challenge(&e) => ctrl.report_error(e.to_string()),
                                Err(e) => {
                                    ctrl.complete_success();
                                    error.set(Some(e.to_string()));
                                }
                            }
                        });
                    }),
                );
            }
            Err(err) => error.set(Some(err.to_string())),
        }
    });
}
