//! `/user/service-users`: the viewer's service users, with the empty, no-permission, limit and
//! not-set-up states.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use uf_product::components::{
    Body1Strong, Caption2, ContentContainer, EmptyState, EmptyStateCallToAction, SkeletonItemSize,
    Title3, EMPTYSTATE_LOCK_ILLUSTRATION,
};
use uf_product::primitives::*;

use super::error::friendly_error;
use super::server::get_service_users_overview;
use super::types::{ServiceUserRow, ServiceUserState, ServiceUsersOverview};
use crate::paths;

const INTRO: &str = "A service user is an account that a tool, such as an MCP client like Claude Code, a CI job, or a webhook consumer, uses to call this site for you, with only the access you give it.";

/// `/user/service-users` page.
///
/// Wrap in [`uf_product::routes::RequireAuthenticated`] at the route.
#[component]
pub fn ServiceUsersPage() -> impl IntoView {
    let overview = Resource::new(|| (), |()| get_service_users_overview());

    view! {
        <ContentContainer max_width="900px".to_string() data_testid="service-users-page".to_string()>
            <Flex vertical=true gap=FlexGap::Large>
                <Flex vertical=true gap=FlexGap::Small>
                    <Title3>"Service users"</Title3>
                    <Caption2>{INTRO}</Caption2>
                </Flex>
                <Suspense fallback=|| view! {
                    <Skeleton>
                        <SkeletonItem size=Signal::from(SkeletonItemSize::S48) />
                        <SkeletonItem size=Signal::from(SkeletonItemSize::S48) />
                    </Skeleton>
                }>
                    {move || overview.get().map(|res| match res {
                        Ok(o) => view! { <Overview overview=o /> }.into_any(),
                        Err(e) => view! {
                            <div data-testid="service-users-load-error">
                                <MessageBar intent=MessageBarIntent::Error>
                                    {format!("Couldn't load your service users. {}", friendly_error(&e.to_string()))}
                                </MessageBar>
                            </div>
                        }.into_any(),
                    })}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

#[component]
fn Overview(overview: ServiceUsersOverview) -> impl IntoView {
    let navigate = use_navigate();
    let at_limit = overview.at_limit();
    let can_create = overview.can_create;
    let limit = overview.limit;
    let count = format!(
        "{} of {} service users",
        overview.service_users.len(),
        overview.limit
    );
    let create_disabled = Signal::derive(move || at_limit || !can_create);

    if overview.service_users.is_empty() {
        return if can_create {
            let nav = navigate.clone();
            view! {
                <div data-testid="service-users-empty">
                    <EmptyState
                        icon=icondata::AiUserOutlined
                        message="No service users yet"
                        description=INTRO
                    >
                        <EmptyStateCallToAction slot:call_to_action>
                            <Button
                                appearance=ButtonAppearance::Primary
                                icon=icondata::AiPlusOutlined
                                on_click=Callback::new(move |_| nav(paths::SERVICE_USERS_NEW, Default::default()))
                            >
                                "Create service user"
                            </Button>
                        </EmptyStateCallToAction>
                    </EmptyState>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div data-testid="service-users-no-permission">
                    <EmptyState
                        illustration_src=EMPTYSTATE_LOCK_ILLUSTRATION
                        illustration_alt="Locked"
                        message="You can't create service users yet"
                        description="Ask a super user to add you to the Service user creators group."
                    />
                </div>
            }
            .into_any()
        };
    }

    view! {
        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center wrap=FlexWrap::Wrap gap=FlexGap::Medium>
            <div data-testid="service-users-count"><Body1Strong>{count}</Body1Strong></div>
            <Show when=move || can_create>
                {
                    let nav = navigate.clone();
                    view! {
                        <div data-testid="service-users-create">
                            <Button
                                appearance=ButtonAppearance::Primary
                                icon=icondata::AiPlusOutlined
                                disabled=create_disabled
                                on_click=Callback::new(move |_| nav(paths::SERVICE_USERS_NEW, Default::default()))
                            >
                                "Create service user"
                            </Button>
                        </div>
                    }
                }
            </Show>
        </Flex>
        <Show when=move || can_create && at_limit>
            <div data-testid="service-users-at-limit">
                <Caption2>{format!("You have {limit} service users, the most allowed. Delete one to make room.")}</Caption2>
            </div>
        </Show>
        <Show when=move || !can_create>
            <div data-testid="service-users-cannot-create">
                <MessageBar intent=MessageBarIntent::Info>
                    "You can't create more service users. Ask a super user to add you to the Service user creators group."
                </MessageBar>
            </div>
        </Show>
        <Show when=move || !overview.tokens_configured>
            <div data-testid="service-users-not-configured">
                <MessageBar intent=MessageBarIntent::Warning>
                    "This site isn't set up to give service users tokens yet. You can still create service users."
                </MessageBar>
            </div>
        </Show>
        <div data-testid="service-users-list">
            <Flex vertical=true gap=FlexGap::Medium>
                {overview.service_users.into_iter().map(|service_user| {
                    view! { <ServiceUserCard service_user=service_user tokens_configured=overview.tokens_configured /> }
                }).collect_view()}
            </Flex>
        </div>
    }
    .into_any()
}

#[component]
fn ServiceUserCard(service_user: ServiceUserRow, tokens_configured: bool) -> impl IntoView {
    let navigate = use_navigate();
    let paused = service_user.state == ServiceUserState::Paused;
    let (badge_color, badge_text) = match service_user.state {
        ServiceUserState::Active => (BadgeColor::Success, "Active"),
        ServiceUserState::Paused => (BadgeColor::Warning, "Paused"),
    };
    let token_path = paths::service_users_token(&service_user.id);
    let subtitle = format!(
        "{} · Created {}",
        service_user.display_name, service_user.created_on
    );
    let test_id = format!("service-user-card-{}", service_user.handle);

    view! {
        <div data-testid=test_id>
            <Card>
                <CardHeader>
                    <Flex gap=FlexGap::Small align=FlexAlign::Center>
                        <Body1Strong>{format!("@{}", service_user.handle)}</Body1Strong>
                        <ServiceUserBadge />
                    </Flex>
                    <CardHeaderDescription slot>
                        <Caption2>{subtitle}</Caption2>
                    </CardHeaderDescription>
                    <CardHeaderAction slot>
                        <Badge
                            appearance=BadgeAppearance::Tint
                            size=BadgeSize::Small
                            color=badge_color
                        >
                            {badge_text}
                        </Badge>
                    </CardHeaderAction>
                </CardHeader>
                {service_user.description.map(|d| view! { <CardContent><Body1>{d}</Body1></CardContent> })}
                <Show when=move || tokens_configured>
                    {
                        let nav = navigate.clone();
                        let path = token_path.clone();
                        view! {
                            <CardFooter>
                                <Flex vertical=true gap=FlexGap::Small align=FlexAlign::End>
                                    <Button
                                        appearance=ButtonAppearance::Subtle
                                        icon=icondata::AiKeyOutlined
                                        disabled=paused
                                        on_click=Callback::new(move |_| nav(&path, Default::default()))
                                    >
                                        "New token"
                                    </Button>
                                    <Show when=move || paused>
                                        <Caption2>"Paused service users can't get tokens."</Caption2>
                                    </Show>
                                </Flex>
                            </CardFooter>
                        }
                    }
                </Show>
            </Card>
        </div>
    }
}

/// Marks a handle as a service user so it is not read as a person's name.
#[component]
pub fn ServiceUserBadge() -> impl IntoView {
    view! {
        <span data-testid="service-user-badge">
            <Badge appearance=BadgeAppearance::Outline size=BadgeSize::Small color=BadgeColor::Informative>
                "Service user"
            </Badge>
        </span>
    }
}
