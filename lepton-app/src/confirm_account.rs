//! `/user/confirm-account` product page (Orbital / uf_product shell).

use lepton_auth_ui::ConfirmAccountPage as ConfirmAccountFunnel;
use leptos::prelude::*;
use uf_product::components::ContentContainer;

/// Guided account confirm funnel under the user settings shell.
///
/// The lazy route wraps this view in [`uf_product::routes::RequireAuthenticated`].
#[component]
pub fn ConfirmAccountPage() -> impl IntoView {
    view! {
        <ContentContainer max_width="900px">
            <ConfirmAccountFunnel />
        </ContentContainer>
    }
}
