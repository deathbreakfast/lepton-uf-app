//! Account Settings page: email, password, TOTP, OAuth, devices, and Owner wipe.
//!
//! Composes kit server fns from `lepton_auth::actions`. Discoverability and concern
//! table: crate root [`crate`]. Host mount recipe: workspace README.

mod email;
mod password;

use email::{ChangeEmailCard, EmailOverviewCard};
use lepton_auth::actions::account::{
    get_account_settings_overview, ChangePassword, RequestEmailChange, RequestEmailVerification,
    VerifyEmailToken,
};
#[cfg(feature = "hydrate")]
use lepton_auth::token_url::{
    read_token_from_window_location, strip_legacy_token_query_from_address_bar,
};
use lepton_auth_ui::ConfirmAccountPrompt;
use leptos::prelude::*;
#[cfg(not(feature = "hydrate"))]
use leptos_router::hooks::use_location;
use password::ChangePasswordCard;
use uf_product::components::{ContentContainer, Title3};
use uf_product::primitives::*;

use crate::connected_accounts_section::ConnectedAccountsSection;
use crate::devices_section::SecurityDevicesSection;
use crate::totp_section::TotpSettingsSection;
use crate::wipe_section::AccountWipeSection;

/// `/user/account-settings` page: overview, change password, email change/verification.
///
/// The lazy route wraps this view in [`uf_product::routes::RequireAuthenticated`].
#[component]
pub fn AccountSettingsPage() -> impl IntoView {
    #[cfg(not(feature = "hydrate"))]
    let location = use_location();
    let overview = Resource::new(|| (), |_| get_account_settings_overview());
    let change_password_action = ServerAction::<ChangePassword>::new();
    let change_email_action = ServerAction::<RequestEmailChange>::new();
    let resend_action = ServerAction::<RequestEmailVerification>::new();
    let verify_action = ServerAction::<VerifyEmailToken>::new();
    let token_from_query = Memo::new(move |_| {
        #[cfg(feature = "hydrate")]
        {
            read_token_from_window_location()
        }
        #[cfg(not(feature = "hydrate"))]
        {
            let raw = location.search.get();
            let trimmed = raw.trim_start_matches('?');
            if trimmed.is_empty() {
                return String::new();
            }
            for (key, value) in url::form_urlencoded::parse(trimmed.as_bytes()) {
                if key == "token" {
                    return value.into_owned();
                }
            }
            String::new()
        }
    });
    let verify_token_input = RwSignal::new(String::new());
    Effect::new(move |_| {
        let from_url = token_from_query.get();
        if !from_url.is_empty() {
            verify_token_input.set(from_url);
        }
    });
    #[cfg(feature = "hydrate")]
    Effect::new(move |_| {
        if !token_from_query.get().is_empty() {
            strip_legacy_token_query_from_address_bar();
        }
    });
    let verify_email_succeeded = RwSignal::new(false);
    // `ServerAction` is `Copy`; the `move` closures capture their own copies.
    Effect::new(move |_| {
        let _pending = verify_action.pending().get();
        if matches!(verify_action.value().get(), Some(Ok(()))) {
            verify_email_succeeded.set(true);
        }
    });

    view! {
        <ContentContainer max_width="900px" data_testid="account-settings-container">
            <Flex vertical=true gap=FlexGap::Large>
                <Title3>"Account Settings"</Title3>
                <ConfirmAccountPrompt />
                <Show when=move || verify_email_succeeded.get()>
                    <div data-testid="account-email-verification-complete">
                        <MessageBar intent=MessageBarIntent::Success>
                            "Email verification complete."
                        </MessageBar>
                    </div>
                </Show>

                <Suspense fallback=move || view! {
                    <Skeleton>
                        <SkeletonItem height="84px".to_string() />
                    </Skeleton>
                }>
                    {move || {
                        match overview.get() {
                            Some(Ok(account)) => {
                                view! {
                                    <Flex vertical=true gap=FlexGap::Large>
                                        <EmailOverviewCard
                                            masked_email=account.masked_email.clone()
                                            role_badge=account.role_badge.clone()
                                            email_verified=account.email_verified
                                            token_from_query=token_from_query
                                            verify_action=verify_action
                                            verify_token_input=verify_token_input
                                            resend_action=resend_action
                                        />
                                        <ChangeEmailCard change_email_action=change_email_action />
                                        <ChangePasswordCard change_password_action=change_password_action />
                                        <TotpSettingsSection/>
                                        <ConnectedAccountsSection/>
                                        <SecurityDevicesSection/>
                                        <AccountWipeSection/>
                                    </Flex>
                                }
                                    .into_any()
                            }
                            Some(Err(err)) => {
                                view! {
                                    <MessageBar intent=MessageBarIntent::Error>
                                        "Failed to load account settings: " {err.to_string()}
                                    </MessageBar>
                                }
                                    .into_any()
                            }
                            None => ().into_any(),
                        }
                    }}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}
