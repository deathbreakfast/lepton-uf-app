//! Lazy-loaded route views for WASM code-splitting (`cargo leptos --split`).

use leptos::prelude::*;
use leptos_router::{lazy_route, LazyRoute};

use crate::account_settings::AccountSettingsPage;
use crate::appearance::AppearancePage;
use crate::confirm_account::ConfirmAccountPage;
use crate::profile::ProfilePage;

/// Prefetch the user settings family WASM chunk (leaf pages share split modules).
pub async fn prefetch_family() {
    ProfileRoute::preload().await;
}

/// Lazy `/user/profile` page.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProfileRoute;

#[lazy_route]
impl LazyRoute for ProfileRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <ProfilePage /> }.into_any()
    }
}

/// Lazy `/user/appearance` page.
#[derive(Clone, Copy, Debug, Default)]
pub struct AppearanceRoute;

#[lazy_route]
impl LazyRoute for AppearanceRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <AppearancePage /> }.into_any()
    }
}

/// Lazy `/user/account-settings` page.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccountSettingsRoute;

#[lazy_route]
impl LazyRoute for AccountSettingsRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! {
            <uf_product::routes::RequireAuthenticated>
                <AccountSettingsPage />
            </uf_product::routes::RequireAuthenticated>
        }
        .into_any()
    }
}

/// Lazy `/user/confirm-account` page.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConfirmAccountRoute;

#[lazy_route]
impl LazyRoute for ConfirmAccountRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! {
            <uf_product::routes::RequireAuthenticated>
                <ConfirmAccountPage />
            </uf_product::routes::RequireAuthenticated>
        }
        .into_any()
    }
}
