//! Collect a fresh authenticator code, then run a write with it.

use lepton_auth_ui::{use_step_up_controller, StepUpFactors, StepUpPolicy, StepUpRequest};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;

fn request() -> StepUpRequest {
    StepUpRequest {
        title: "Confirm it's you".to_string(),
        description: Some(
            "Enter the code from your authenticator app to save sign-in and delivery settings."
                .to_string(),
        ),
        policy: StepUpPolicy::Totp,
    }
}

/// Open the step-up dialog, then call `action` with the code the operator typed.
///
/// `on_ok` runs after a successful call; `error` receives the server's message
/// when the call fails after the dialog closed.
pub fn spawn_with_fresh_code<F, Fut>(
    error: RwSignal<Option<String>>,
    on_ok: Callback<()>,
    action: F,
) where
    F: Fn(String) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), ServerFnError>> + 'static,
{
    let Some(ctrl) = use_step_up_controller() else {
        error.set(Some(
            "Two-factor confirmation is not available on this page. Reload and try again."
                .to_string(),
        ));
        return;
    };
    ctrl.request(
        request(),
        Callback::new(move |factors: StepUpFactors| {
            let action = action.clone();
            spawn_local_scoped(async move {
                match action(factors.totp_code).await {
                    Ok(()) => {
                        ctrl.complete_success();
                        on_ok.run(());
                    }
                    Err(e) => {
                        let message = e.to_string();
                        if message.contains("STEP_UP:") {
                            // Wrong or reused code: keep the dialog open to retry.
                            ctrl.report_error(message);
                        } else {
                            ctrl.complete_success();
                            error.set(Some(message));
                        }
                    }
                }
            });
        }),
    );
}
