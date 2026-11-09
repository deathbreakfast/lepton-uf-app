//! Product route table for the `lepton-uf-app` Playwright host.
//!
//! Mounts the real product crates over the harness boot in [`crate::boot`]:
//! `lepton_app::UserAppRoutes`, `lepton_auth_app::LeptonAuthRoutes`, and
//! `lepton_shell::AppBarUserMenu` in the `uf_integrations` shell auth slot. There are
//! no fixture components here, so what Playwright drives is what a host ships.

use lepton_app::UserAppRoutes;
use lepton_auth::paths::{SIGNIN, SIGNUP};
use lepton_auth_app::LeptonAuthRoutes;
use lepton_shell::AppBarUserMenu;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use uf_integrations::{
    provide_shell_auth_menu, HostAuthMenu, ShellAppBar, ShellAuthMenu, UnifiedFieldAppBar,
    UnifiedFieldNotFoundPage, UnifiedFieldShellLayout,
};
use uf_product::components::ContentContainer;

use crate::person_picker::PersonPickerDemoPage;
use uf_product::primitives::{Body1, Flex, FlexAlign, FlexGap, FlexWrap, Link, Title3};
use uf_product::routes::RequireAuthenticated;
use uf_product::telemetry::UfAppRouteEntry;
use uf_product::{
    init_appearance_resource, init_auth_resource, orbital_shell, provide_appearance_context,
    provide_auth_context, AppearanceThemeController, AuthSession, OrbitalTemplate,
    UF_SHELL_BRAND_SEED,
};

/// Route brand seeds for [`AppearanceThemeController`] (mirrors deployable host wiring).
///
/// Without this controller, save updates the nested preview theme but a hard refresh
/// never re-applies prefs to the shell `ThemeInjection`.
static E2E_ROUTE_TABLE: &[UfAppRouteEntry] = &[UfAppRouteEntry {
    app_id: "lepton-app",
    app_name: "Lepton",
    route_prefix: "/user",
    brand_seed: UF_SHELL_BRAND_SEED,
}];

/// SSR document shell (Orbital first-paint baseline, boot overlay, hydration scripts).
pub fn shell(options: LeptosOptions) -> impl IntoView {
    orbital_shell(options, || view! { <App/> })
}

/// Root product composition: session + appearance context, shell auth menu, routes.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    let auth = provide_auth_context(AuthSession::default());
    let _auth_resource = init_auth_resource(&auth);
    let appearance = provide_appearance_context();
    let _appearance_resource = init_appearance_resource(appearance);
    provide_shell_auth_menu(|| view! { <AppBarUserMenu /> });

    view! {
        <OrbitalTemplate>
            <Stylesheet id="leptos" href="/pkg/lepton-uf-app-e2e.css"/>
            <Title text="lepton-uf-app e2e"/>
            <Router>
                <AppearanceThemeController routes=E2E_ROUTE_TABLE />
                <Routes fallback=|| view! { <HostNotFoundPage /> }>
                    <Route path=path!("/") view=HomePage/>
                    <Route path=path!("gate/auth-required") view=GateAuthRequiredPage/>
                    <Route path=path!("dev/person-picker") view=PersonPickerDemoPage/>
                    <UserAppRoutes />
                    <LeptonAuthRoutes />
                </Routes>
            </Router>
        </OrbitalTemplate>
    }
}

/// Product 404 with the shared app bar and auth menu.
#[component]
fn HostNotFoundPage() -> impl IntoView {
    view! {
        <UnifiedFieldNotFoundPage>
            <ShellAuthMenu slot:auth_menu>
                <AppBarUserMenu />
            </ShellAuthMenu>
        </UnifiedFieldNotFoundPage>
    }
}

/// Landing page carrying the app-bar auth menu that shell scenarios drive.
#[component]
fn HomePage() -> impl IntoView {
    view! {
        <UnifiedFieldShellLayout>
            <ShellAppBar slot>
                <UnifiedFieldAppBar app_name="Lepton product e2e".to_string()>
                    <ShellAuthMenu slot:auth_menu>
                        <HostAuthMenu />
                    </ShellAuthMenu>
                </UnifiedFieldAppBar>
            </ShellAppBar>
            <ContentContainer max_width="900px" data_testid="home-root">
                <Flex vertical=true gap=FlexGap::Medium align=FlexAlign::Start>
                    <Title3>"lepton-uf-app e2e"</Title3>
                    <Body1>
                        "Product host for the shell auth menu, /user settings, and /auth routes."
                    </Body1>
                    <Flex gap=FlexGap::Medium align=FlexAlign::Center wrap=FlexWrap::Wrap>
                        <Link href=SIGNIN>"Sign in"</Link>
                        <Link href=SIGNUP>"Sign up"</Link>
                        <Link href="/user">"User home"</Link>
                        <Link href="/user/account-settings">"Account settings"</Link>
                        <Link href="/gate/auth-required">"Auth-required gate"</Link>
                    </Flex>
                </Flex>
            </ContentContainer>
        </UnifiedFieldShellLayout>
    }
}

/// Thin `RequireAuthenticated` page for gate → in-place AuthDialog Playwright coverage.
#[component]
fn GateAuthRequiredPage() -> impl IntoView {
    view! {
        <UnifiedFieldShellLayout>
            <ShellAppBar slot>
                <UnifiedFieldAppBar app_name="Lepton product e2e".to_string()>
                    <ShellAuthMenu slot:auth_menu>
                        <HostAuthMenu />
                    </ShellAuthMenu>
                </UnifiedFieldAppBar>
            </ShellAppBar>
            <RequireAuthenticated requires_email_verification=true>
                <ContentContainer max_width="900px" data_testid="gate-auth-required-content">
                    <Flex vertical=true gap=FlexGap::Medium align=FlexAlign::Start>
                        <Title3>"Gated content"</Title3>
                        <Body1>"Signed-in users see this after the access gate."</Body1>
                    </Flex>
                </ContentContainer>
            </RequireAuthenticated>
        </UnifiedFieldShellLayout>
    }
}
