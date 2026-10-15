//! Shell layout wrapping the `/user/*` pages with app-bar and left-nav.

use lepton_auth_ui::{ConfirmAccountPrompt, ConfirmAccountPromptVariant};
use lepton_shell::AppBarUserMenu;
use leptos::prelude::*;
use leptos_router::components::Outlet;
use uf_integrations::{
    ShellAppBar, ShellAuthMenu, ShellLeftNav, UnifiedFieldAppBar, UnifiedFieldShellLayout,
};
use uf_product::components::{
    Navigation, NavigationBody, NavigationConfig, NavigationLink, NavigationMaterial,
};

use crate::paths;

/// App-bar + left-nav shell for the `/user/*` route subtree; renders the matched child
/// route via [`Outlet`].
#[component]
pub fn UserLayout() -> impl IntoView {
    let selected_value = RwSignal::new(None::<String>);
    let open_categories = RwSignal::new(Vec::<String>::new());

    view! {
        <div data-testid="user-app-layout-root">
        <UnifiedFieldShellLayout>
            <ShellAppBar slot>
                <UnifiedFieldAppBar
                    app_name="Settings".to_string()
                    homepage_url="/user/profile".to_string()
                >
                    <ShellAuthMenu slot:auth_menu>
                        <AppBarUserMenu />
                    </ShellAuthMenu>
                </UnifiedFieldAppBar>
            </ShellAppBar>
            <ShellLeftNav slot>
                <Navigation config=NavigationConfig::new().with_selected_value(selected_value).with_open_categories(open_categories)>
                    <NavigationMaterial slot />
                    <NavigationBody slot>
                        <NavigationLink path=paths::PROFILE value=paths::PROFILE icon=icondata::AiUserOutlined test_id="nav-user-profile">"Profile"</NavigationLink>
                        <NavigationLink path=paths::APPEARANCE value=paths::APPEARANCE icon=icondata::AiEyeOutlined test_id="nav-lepton-appearance">"Appearance"</NavigationLink>
                        <NavigationLink path=paths::ACCOUNT_SETTINGS value=paths::ACCOUNT_SETTINGS icon=icondata::AiSettingOutlined test_id="nav-user-account-settings">"Account Settings"</NavigationLink>
                        <NavigationLink path=paths::DELIVERY value=paths::DELIVERY icon=icondata::AiMailOutlined test_id="nav-user-delivery">"Sign-in and delivery"</NavigationLink>
                    </NavigationBody>
                </Navigation>
            </ShellLeftNav>
            <ConfirmAccountPrompt variant=ConfirmAccountPromptVariant::Compact />
            <Outlet />
        </UnifiedFieldShellLayout>
        </div>
    }
}
