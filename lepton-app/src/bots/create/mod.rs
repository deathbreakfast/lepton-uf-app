//! `/user/bots/new`: create a bot in three steps (name it, choose access, get a
//! token). Each step saves on its own, so leaving part way keeps what was done:
//! the bot shows on the Bots page and "New token" picks up from step 3.

mod access_step;
mod profile_step;

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use uf_product::components::{
    Caption2, ContentContainer, EmptyState, EmptyStateCallToAction, SkeletonItemSize, Step,
    StepStatus, Stepper, Title3, EMPTYSTATE_LOCK_ILLUSTRATION,
};
use uf_product::primitives::*;

use super::error::friendly_error;
use super::server::get_bots_overview;
use super::token::{TokenForm, TokenReveal};
use super::types::{BotRow, BotsOverview, MintedTokenView};
use crate::paths;
use access_step::AccessStep;
use profile_step::ProfileStep;

/// Where the person is in the flow.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Stage {
    Profile,
    Access(BotRow),
    Token { bot: BotRow, keys: Vec<String> },
}

impl Stage {
    const fn number(&self) -> u8 {
        match self {
            Self::Profile => 1,
            Self::Access(_) => 2,
            Self::Token { .. } => 3,
        }
    }
}

/// Status of step `n` (1-based) while the person is on `current`.
const fn step_status(n: u8, current: u8, finished: bool) -> StepStatus {
    if finished || n < current {
        StepStatus::Done
    } else if n == current {
        StepStatus::Active
    } else {
        StepStatus::Pending
    }
}

/// `/user/bots/new` page.
///
/// Wrap in [`uf_product::routes::RequireAuthenticated`] at the route.
#[component]
pub fn CreateBotPage() -> impl IntoView {
    let overview = Resource::new(|| (), |()| get_bots_overview());

    view! {
        <ContentContainer max_width="900px".to_string() data_testid="bot-create-page".to_string()>
            <Flex vertical=true gap=FlexGap::Large>
                <Title3>"Create a bot"</Title3>
                <Suspense fallback=|| view! {
                    <Skeleton><SkeletonItem size=Signal::from(SkeletonItemSize::S48) /></Skeleton>
                }>
                    {move || overview.get().map(|res| match res {
                        Ok(o) => view! { <Gate overview=o /> }.into_any(),
                        Err(e) => view! {
                            <div data-testid="bot-create-load-error">
                                <MessageBar intent=MessageBarIntent::Error>
                                    {friendly_error(&e.to_string())}
                                </MessageBar>
                            </div>
                        }.into_any(),
                    })}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

/// Explain why the person can't start, or start the flow.
#[component]
fn Gate(overview: BotsOverview) -> impl IntoView {
    let navigate = use_navigate();
    if !overview.can_create {
        return view! {
            <div data-testid="bots-no-permission">
                <EmptyState
                    illustration_src=EMPTYSTATE_LOCK_ILLUSTRATION
                    illustration_alt="Locked"
                    message="You can't create bots yet"
                    description="Ask a super user to add you to the Bot creators group."
                />
            </div>
        }
        .into_any();
    }
    if overview.at_limit() {
        return view! {
            <div data-testid="bots-at-limit">
                <EmptyState
                    icon=icondata::AiRobotOutlined
                    message="You have the most bots allowed"
                    description="Each person can have 10 bots. Delete one to make room."
                >
                    <EmptyStateCallToAction slot:call_to_action>
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on_click=Callback::new(move |_| navigate(paths::BOTS, Default::default()))
                        >
                            "Back to bots"
                        </Button>
                    </EmptyStateCallToAction>
                </EmptyState>
            </div>
        }
        .into_any();
    }
    view! { <Flow owner_email=overview.owner_email tokens_configured=overview.tokens_configured /> }
        .into_any()
}

#[component]
fn Flow(owner_email: String, tokens_configured: bool) -> impl IntoView {
    let stage = RwSignal::new(Stage::Profile);
    let minted = RwSignal::new(None::<MintedTokenView>);

    let stepper = move || {
        let current = stage.with(Stage::number);
        let finished = minted.with(Option::is_some);
        view! {
            <div data-testid="bot-create-stepper">
            <Stepper vertical=false>
                <Step slot:steps label="Name it".to_string() status=step_status(1, current, finished) />
                <Step slot:steps label="Choose access".to_string() status=step_status(2, current, finished) />
                <Step slot:steps label="Get a token".to_string() status=step_status(3, current, finished) />
            </Stepper>
            </div>
        }
    };
    let progress = move || format!("Step {} of 3", stage.with(Stage::number));

    view! {
        {stepper}
        <div data-testid="bot-create-progress" aria-live="polite">
            <Caption2>{progress}</Caption2>
        </div>
        {move || match stage.get() {
            Stage::Profile => view! {
                <ProfileStep
                    owner_email=owner_email.clone()
                    on_created=Callback::new(move |bot| stage.set(Stage::Access(bot)))
                />
            }.into_any(),
            Stage::Access(bot) => view! {
                <AccessStep
                    bot=bot.clone()
                    tokens_configured=tokens_configured
                    on_next=Callback::new(move |keys| stage.set(Stage::Token { bot: bot.clone(), keys }))
                />
            }.into_any(),
            Stage::Token { bot, keys } => {
                let handle = bot.handle.clone();
                view! {
                    {move || match minted.get() {
                        Some(token) => view! {
                            <TokenReveal handle=handle.clone() token=token />
                        }.into_any(),
                        None => view! {
                            <TokenForm
                                bot=bot.clone()
                                preselected=keys.clone()
                                on_minted=Callback::new(move |t| minted.set(Some(t)))
                            />
                        }.into_any(),
                    }}
                }.into_any()
            }
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_status_moves_with_the_flow() {
        assert_eq!(step_status(1, 1, false), StepStatus::Active);
        assert_eq!(step_status(2, 1, false), StepStatus::Pending);
        assert_eq!(step_status(1, 2, false), StepStatus::Done);
        assert_eq!(step_status(3, 3, false), StepStatus::Active);
        assert_eq!(step_status(3, 3, true), StepStatus::Done);
    }
}
