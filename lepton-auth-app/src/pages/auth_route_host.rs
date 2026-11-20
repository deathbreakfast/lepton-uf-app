use lepton_auth::routes::parse_referer_from_search;
use lepton_auth_ui::{AuthDialog, AuthDialogCallbacks, AuthDialogKind};
use lepton_shell::{
    retain_frozen_post_auth_referer, sanitize_post_auth_navigate_path, use_return_after_signin,
};
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use uf_product::use_auth_context;

use crate::pages::AuthPageShell;

/// Shared wrapper for `/auth/signin`, `/auth/signup`, and `/auth/logout` routes.
///
/// Freezes the first specific `referer` query value (see
/// [`lepton_shell::retain_frozen_post_auth_referer`]) and calls `trigger_refresh` on
/// success so gated `/user/*` routes pick up the authenticated session. After sign-in,
/// [`lepton_shell::use_return_after_signin`] returns to the frozen referer when the
/// server landed on its default page, and attaches it to confirm-account.
#[component]
pub fn AuthRouteHost(initial_kind: AuthDialogKind, test_id: &'static str) -> impl IntoView {
    let navigate = use_navigate();
    let location = use_location();
    let auth = use_auth_context();
    let referer_path = RwSignal::new(sanitize_post_auth_navigate_path(parse_referer_from_search(
        &location.search.get_untracked(),
    )));
    Effect::new(move |_| {
        let incoming =
            sanitize_post_auth_navigate_path(parse_referer_from_search(&location.search.get()));
        referer_path.update(|frozen| {
            *frozen = retain_frozen_post_auth_referer(frozen, &incoming);
        });
    });
    let referer = Signal::derive(move || referer_path.get());

    let auth_dialog_open = RwSignal::new(true);
    let auth_dialog_kind = RwSignal::new(initial_kind);
    // The form navigates to the outcome's `redirect_to` itself; the close effect below
    // must not race it, so it only navigates when the dialog closes without success.
    let auth_finished = RwSignal::new(false);
    let return_after_signin = use_return_after_signin(false);

    Effect::new(move |was_open: Option<bool>| {
        let open = auth_dialog_open.get();
        if was_open == Some(true) && !open && !auth_finished.get() {
            let target = referer.get();
            navigate(&target, Default::default());
        }
        open
    });

    let close_auth_dialog = Callback::new(move |_| {
        auth_dialog_open.set(false);
    });
    let finish_auth_dialog = {
        let auth = auth.clone();
        Callback::new(move |_| {
            auth.trigger_refresh();
            if auth_dialog_kind.get_untracked() != AuthDialogKind::Logout {
                return_after_signin(referer_path.get_untracked());
            }
            auth_finished.set(true);
            auth_dialog_open.set(false);
        })
    };

    let switch_to_signin = Callback::new(move |_| {
        auth_dialog_kind.set(AuthDialogKind::Signin);
    });
    let switch_to_signup = Callback::new(move |_| {
        auth_dialog_kind.set(AuthDialogKind::Signup);
    });

    view! {
        <AuthPageShell chrome_interactive=false>
            <div data-testid=test_id>
                <AuthDialog
                    open=auth_dialog_open.into()
                    kind=auth_dialog_kind.into()
                    referer=referer
                    callbacks=AuthDialogCallbacks {
                        on_close: Some(close_auth_dialog),
                        on_success: Some(finish_auth_dialog),
                        on_switch_signin: Some(switch_to_signin),
                        on_switch_signup: Some(switch_to_signup),
                    }
                />
            </div>
        </AuthPageShell>
    }
}
