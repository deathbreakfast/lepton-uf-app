//! `/user/bots`: the viewer's bots, with the empty, no-permission, limit and
//! not-set-up states.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use uf_product::components::{
    Body1Strong, Caption2, ContentContainer, EmptyState, EmptyStateCallToAction, SkeletonItemSize,
    Title3, EMPTYSTATE_LOCK_ILLUSTRATION,
};
use uf_product::primitives::*;

use super::error::friendly_error;
use super::server::get_bots_overview;
use super::types::{BotRow, BotState, BotsOverview};
use crate::paths;

const INTRO: &str = "A bot is an account that tools like Claude Code use to call this site for you, with only the access you give it.";

/// `/user/bots` page.
///
/// Wrap in [`uf_product::routes::RequireAuthenticated`] at the route.
#[component]
pub fn BotsPage() -> impl IntoView {
    let overview = Resource::new(|| (), |()| get_bots_overview());

    view! {
        <ContentContainer max_width="900px".to_string() data_testid="bots-page".to_string()>
            <Flex vertical=true gap=FlexGap::Large>
                <Flex vertical=true gap=FlexGap::Small>
                    <Title3>"Bots"</Title3>
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
                            <div data-testid="bots-load-error">
                                <MessageBar intent=MessageBarIntent::Error>
                                    {format!("Couldn't load your bots. {}", friendly_error(&e.to_string()))}
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
fn Overview(overview: BotsOverview) -> impl IntoView {
    let navigate = use_navigate();
    let at_limit = overview.at_limit();
    let can_create = overview.can_create;
    let count = format!("{} of {} bots", overview.bots.len(), overview.limit);
    let create_disabled = Signal::derive(move || at_limit || !can_create);

    if overview.bots.is_empty() {
        return if can_create {
            let nav = navigate.clone();
            view! {
                <div data-testid="bots-empty">
                    <EmptyState
                        icon=icondata::AiRobotOutlined
                        message="No bots yet"
                        description=INTRO
                    >
                        <EmptyStateCallToAction slot:call_to_action>
                            <Button
                                appearance=ButtonAppearance::Primary
                                icon=icondata::AiPlusOutlined
                                on_click=Callback::new(move |_| nav(paths::BOTS_NEW, Default::default()))
                            >
                                "Create bot"
                            </Button>
                        </EmptyStateCallToAction>
                    </EmptyState>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div data-testid="bots-no-permission">
                    <EmptyState
                        illustration_src=EMPTYSTATE_LOCK_ILLUSTRATION
                        illustration_alt="Locked"
                        message="You can't create bots yet"
                        description="Ask a super user to add you to the Bot creators group."
                    />
                </div>
            }
            .into_any()
        };
    }

    view! {
        <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center wrap=FlexWrap::Wrap gap=FlexGap::Medium>
            <div data-testid="bots-count"><Body1Strong>{count}</Body1Strong></div>
            <Show when=move || can_create>
                {
                    let nav = navigate.clone();
                    view! {
                        <div data-testid="bots-create">
                            <Button
                                appearance=ButtonAppearance::Primary
                                icon=icondata::AiPlusOutlined
                                disabled=create_disabled
                                on_click=Callback::new(move |_| nav(paths::BOTS_NEW, Default::default()))
                            >
                                "Create bot"
                            </Button>
                        </div>
                    }
                }
            </Show>
        </Flex>
        <Show when=move || can_create && at_limit>
            <div data-testid="bots-at-limit">
                <Caption2>"You have the most bots allowed. Delete one to make room."</Caption2>
            </div>
        </Show>
        <Show when=move || !can_create>
            <div data-testid="bots-cannot-create">
                <MessageBar intent=MessageBarIntent::Info>
                    "You can't create more bots. Ask a super user to add you to the Bot creators group."
                </MessageBar>
            </div>
        </Show>
        <Show when=move || !overview.tokens_configured>
            <div data-testid="bots-not-configured">
                <MessageBar intent=MessageBarIntent::Warning>
                    "This site isn't set up to give bots tokens yet. You can still create bots."
                </MessageBar>
            </div>
        </Show>
        <div data-testid="bots-list">
            <Flex vertical=true gap=FlexGap::Medium>
                {overview.bots.into_iter().map(|bot| {
                    view! { <BotCard bot=bot tokens_configured=overview.tokens_configured /> }
                }).collect_view()}
            </Flex>
        </div>
    }
    .into_any()
}

#[component]
fn BotCard(bot: BotRow, tokens_configured: bool) -> impl IntoView {
    let navigate = use_navigate();
    let paused = bot.state == BotState::Paused;
    let (badge_color, badge_text) = match bot.state {
        BotState::Active => (BadgeColor::Success, "Active"),
        BotState::Paused => (BadgeColor::Warning, "Paused"),
    };
    let token_path = paths::bots_token(&bot.id);
    let subtitle = format!("{} · Created {}", bot.display_name, bot.created_on);
    let test_id = format!("bot-card-{}", bot.handle);

    view! {
        <div data-testid=test_id>
            <Card>
                <CardHeader>
                    <Body1Strong>{format!("@{}", bot.handle)}</Body1Strong>
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
                {bot.description.map(|d| view! { <CardContent><Body1>{d}</Body1></CardContent> })}
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
                                        <Caption2>"Paused bots can't get tokens."</Caption2>
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
