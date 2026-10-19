//! Get a token for a service user: step 3 of the create flow, and the "New token" page.

mod reveal;

use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use leptos_router::hooks::{use_navigate, use_params_map};
use uf_product::components::{
    Body1, Body1Strong, Caption2, ContentContainer, EmptyState, EmptyStateCallToAction,
    RadioGroupLayout, SkeletonItemSize, Subtitle2, Title3,
};
use uf_product::primitives::*;

pub use reveal::TokenReveal;

use super::error::friendly_error;
use super::list_page::ServiceUserBadge;
use super::server::{get_service_user_row, mint_service_user_token, service_user_access_choices};
use super::types::{AccessChoiceView, MintedTokenView, ServiceUserRow, ServiceUserState, TokenTtl};
use crate::paths;

/// Label, expiry and what the token can do; then "Create token".
#[component]
pub fn TokenForm(
    service_user: ServiceUserRow,
    preselected: Vec<String>,
    on_minted: Callback<MintedTokenView>,
) -> impl IntoView {
    let choices = Resource::new(|| (), |()| service_user_access_choices());
    let title = format!("Get a token for @{}", service_user.handle);
    let service_user_id = service_user.id;
    view! {
        <Card>
            <CardHeader>
                <Subtitle2>{title}</Subtitle2>
                <CardHeaderDescription slot>
                    <Caption2>"Tools send this token to act as the service user. A service user can have up to 5 active tokens."</Caption2>
                </CardHeaderDescription>
            </CardHeader>
            <Suspense fallback=|| view! {
                <CardContent><Skeleton><SkeletonItem size=Signal::from(SkeletonItemSize::S48) /></Skeleton></CardContent>
            }>
                {move || choices.get().map(|res| match res {
                    Ok(list) => view! {
                        <Fields service_user_id=service_user_id.clone() choices=list preselected=preselected.clone() on_minted=on_minted />
                    }.into_any(),
                    Err(e) => view! {
                        <CardContent>
                            <div data-testid="service-user-token-error">
                                <MessageBar intent=MessageBarIntent::Error>{friendly_error(&e.to_string())}</MessageBar>
                            </div>
                        </CardContent>
                    }.into_any(),
                })}
            </Suspense>
        </Card>
    }
}

#[component]
fn Fields(
    service_user_id: String,
    choices: Vec<AccessChoiceView>,
    preselected: Vec<String>,
    on_minted: Callback<MintedTokenView>,
) -> impl IntoView {
    let label = RwSignal::new(String::new());
    let ttl = RwSignal::new(Some("30".to_string()));
    let error = RwSignal::new(None::<String>);
    let pending = RwSignal::new(false);
    let picked: Vec<(AccessChoiceView, RwSignal<bool>)> = choices
        .into_iter()
        .map(|c| {
            let on = preselected.contains(&c.key);
            (c, RwSignal::new(on))
        })
        .collect();
    let picked = StoredValue::new(picked);
    let keys = move || {
        picked.with_value(|all| {
            all.iter()
                .filter(|(_, on)| on.get())
                .map(|(c, _)| c.key.clone())
                .collect::<Vec<_>>()
        })
    };
    let can_submit = Signal::derive(move || {
        !pending.get() && !label.with(|l| l.trim().is_empty()) && !keys().is_empty()
    });

    let submit = move |_| {
        error.set(None);
        pending.set(true);
        let id = service_user_id.clone();
        let l = label.get_untracked();
        let t = ttl
            .get_untracked()
            .as_deref()
            .and_then(TokenTtl::from_days_str)
            .unwrap_or_default();
        let k = keys();
        spawn_local_scoped(async move {
            // On success the reveal replaces this form, so only a failure
            // touches the form's own signals.
            match mint_service_user_token(id, l, t, k).await {
                Ok(token) => on_minted.run(token),
                Err(e) => {
                    error.set(Some(friendly_error(&e.to_string()).to_string()));
                    pending.set(false);
                }
            }
        });
    };

    let rows = picked.with_value(|all| {
        all.iter()
            .map(|(c, on)| {
                let test_id = format!("service-user-token-scope-{}", c.key);
                let (label, on) = (c.label.clone(), *on);
                view! {
                    <div data-testid=test_id>
                        <Checkbox checked=on label=label size=Signal::from(CheckboxSize::Medium) />
                    </div>
                }
            })
            .collect_view()
    });

    view! {
        <CardContent>
            <Flex vertical=true gap=FlexGap::Medium>
                <div data-testid="service-user-token-label">
                    <Field label="Label" required=true>
                        <Input
                            bind=InputBind { name: "label".into(), ..InputBind::new(label) }
                            appearance=InputAppearance::with_placeholder("Claude Code on laptop")
                        />
                    </Field>
                </div>
                <div data-testid="service-user-token-ttl">
                    <Field label="Expires">
                        <RadioGroup bind=RadioGroupBind::from(ttl) layout=RadioGroupLayout::Horizontal>
                            <Radio value="30".to_string() label="30 days".to_string() />
                            <Radio value="90".to_string() label="90 days".to_string() />
                        </RadioGroup>
                    </Field>
                </div>
                <Field label="This token can">
                    <Flex vertical=true gap=FlexGap::Small>{rows}</Flex>
                </Field>
                {move || error.get().map(|e| view! {
                    <div data-testid="service-user-token-error">
                        <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
                    </div>
                })}
            </Flex>
        </CardContent>
        <CardFooter>
            <div data-testid="service-user-token-submit">
                <Button
                    appearance=ButtonAppearance::Primary
                    loading=pending
                    disabled=Signal::derive(move || !can_submit.get())
                    on_click=Callback::new(submit)
                >
                    "Create token"
                </Button>
            </div>
        </CardFooter>
    }
}

/// `/user/service-users/:service_user_id/token` page: a new token for an existing service user.
///
/// Wrap in [`uf_product::routes::RequireAuthenticated`] at the route.
#[component]
pub fn ServiceUserTokenPage() -> impl IntoView {
    let params = use_params_map();
    let service_user = Resource::new(
        move || params.with(|p| p.get("service_user_id").unwrap_or_default()),
        get_service_user_row,
    );
    view! {
        <ContentContainer max_width="900px".to_string() data_testid="service-user-token-page".to_string()>
            <Flex vertical=true gap=FlexGap::Large>
                <Title3>"New token"</Title3>
                <Suspense fallback=|| view! {
                    <Skeleton><SkeletonItem size=Signal::from(SkeletonItemSize::S48) /></Skeleton>
                }>
                    {move || service_user.get().map(|res| match res {
                        Ok(row) => view! { <ForServiceUser service_user=row /> }.into_any(),
                        Err(e) => view! { <NotFound raw=e.to_string() /> }.into_any(),
                    })}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

#[component]
fn ForServiceUser(service_user: ServiceUserRow) -> impl IntoView {
    let minted = RwSignal::new(None::<MintedTokenView>);
    if service_user.state == ServiceUserState::Paused {
        return view! {
            <div data-testid="service-user-token-paused">
                <MessageBar intent=MessageBarIntent::Warning>
                    {format!("@{} is paused. Resume it to give it tokens.", service_user.handle)}
                </MessageBar>
            </div>
        }
        .into_any();
    }
    let handle = service_user.handle.clone();
    let for_label = format!("@{}", service_user.handle);
    view! {
        <Flex gap=FlexGap::Small align=FlexAlign::Center>
            <Body1>"For"</Body1>
            <Body1Strong>{for_label}</Body1Strong>
            <ServiceUserBadge />
        </Flex>
        {move || match minted.get() {
            Some(token) => view! {
                <TokenReveal handle=handle.clone() token=token />
            }.into_any(),
            None => view! {
                <TokenForm service_user=service_user.clone() preselected=Vec::new() on_minted=Callback::new(move |t| minted.set(Some(t))) />
            }.into_any(),
        }}
    }
    .into_any()
}

#[component]
fn NotFound(raw: String) -> impl IntoView {
    let navigate = use_navigate();
    let gone = raw.contains("reason_class=not_found");
    let (message, description) = if gone {
        (
            "Service user not found",
            "This service user doesn't exist, or it isn't yours to see.",
        )
    } else {
        ("Couldn't load this service user", friendly_error(&raw))
    };
    view! {
        <div data-testid="service-user-not-found">
            <EmptyState icon=icondata::AiQuestionCircleOutlined message=message description=description>
                <EmptyStateCallToAction slot:call_to_action>
                    <Button
                        appearance=ButtonAppearance::Secondary
                        on_click=Callback::new(move |_| navigate(paths::SERVICE_USERS, Default::default()))
                    >
                        "Back to service users"
                    </Button>
                </EmptyStateCallToAction>
            </EmptyState>
        </div>
    }
}
