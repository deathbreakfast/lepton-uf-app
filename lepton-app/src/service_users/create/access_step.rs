//! Step 2: choose what the service user can do, in plain language.
//!
//! Choices that work through a group add the service user to that group now. Choices
//! that live on the token carry over to step 3. A choice the viewer can't give
//! stays visible, switched off, with who to ask.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use orbital_base_components::IconSize;
use uf_product::components::{Caption2, SkeletonItemSize, Subtitle2};
use uf_product::primitives::*;

use crate::paths;
use crate::service_users::error::friendly_error;
use crate::service_users::server::{grant_service_user_access, service_user_access_choices};
use crate::service_users::step_up::spawn_with_step_up;
use crate::service_users::types::{AccessChoiceView, ServiceUserRow};

#[component]
pub(super) fn AccessStep(
    service_user: ServiceUserRow,
    tokens_configured: bool,
    on_next: Callback<Vec<String>>,
) -> impl IntoView {
    let navigate = use_navigate();
    let title = format!("What can @{} do?", service_user.handle);
    if !tokens_configured {
        return view! {
            <Card>
                <CardHeader><Subtitle2>{title}</Subtitle2></CardHeader>
                <CardContent>
                    <div data-testid="service-user-access-not-configured">
                        <MessageBar intent=MessageBarIntent::Warning>
                            "Your service user is created. This site isn't set up to give service users access or tokens yet."
                        </MessageBar>
                    </div>
                </CardContent>
                <CardFooter>
                    <Button
                        appearance=ButtonAppearance::Primary
                        on_click=Callback::new(move |_| navigate(paths::SERVICE_USERS, Default::default()))
                    >
                        "Back to service users"
                    </Button>
                </CardFooter>
            </Card>
        }
        .into_any();
    }

    let choices = Resource::new(|| (), |()| service_user_access_choices());
    view! {
        <Card>
            <CardHeader>
                <Subtitle2>{title}</Subtitle2>
                <CardHeaderDescription slot>
                    <Caption2>"Pick what it needs and nothing more."</Caption2>
                </CardHeaderDescription>
            </CardHeader>
            <Suspense fallback=|| view! {
                <CardContent>
                    <Skeleton><SkeletonItem size=Signal::from(SkeletonItemSize::S48) /></Skeleton>
                </CardContent>
            }>
                {move || choices.get().map(|res| match res {
                    Ok(list) => view! { <ChoiceForm service_user_id=service_user.id.clone() choices=list on_next=on_next /> }.into_any(),
                    Err(e) => view! {
                        <CardContent>
                            <div data-testid="service-user-access-error">
                                <MessageBar intent=MessageBarIntent::Error>{friendly_error(&e.to_string())}</MessageBar>
                            </div>
                        </CardContent>
                    }.into_any(),
                })}
            </Suspense>
        </Card>
    }
    .into_any()
}

#[component]
fn ChoiceForm(
    service_user_id: String,
    choices: Vec<AccessChoiceView>,
    on_next: Callback<Vec<String>>,
) -> impl IntoView {
    let picked: Vec<(AccessChoiceView, RwSignal<bool>)> = choices
        .into_iter()
        .map(|c| (c, RwSignal::new(false)))
        .collect();
    let picked = StoredValue::new(picked);
    let error = RwSignal::new(None::<String>);

    let selected = move || {
        picked.with_value(|all| {
            all.iter()
                .filter(|(c, on)| c.grantable && on.get())
                .map(|(c, _)| (c.key.clone(), c.group_name.is_some()))
                .collect::<Vec<_>>()
        })
    };
    let nothing_selected = Signal::derive(move || selected().is_empty());

    let next = move |_| {
        let chosen = selected();
        let keys: Vec<String> = chosen.iter().map(|(k, _)| k.clone()).collect();
        if !chosen.iter().any(|(_, group)| *group) {
            on_next.run(keys);
            return;
        }
        error.set(None);
        let service_user_id = service_user_id.clone();
        let keys_for_next = keys.clone();
        spawn_with_step_up(
            error,
            Callback::new(move |()| on_next.run(keys_for_next.clone())),
            move || grant_service_user_access(service_user_id.clone(), keys.clone()),
        );
    };

    let rows = picked.with_value(|all| {
        all.iter()
            .filter(|(c, _)| c.grantable)
            .map(|(c, on)| {
                let test_id = format!("service-user-access-{}", c.key);
                let (label, help, on) = (c.label.clone(), c.help.clone(), *on);
                view! {
                    <div data-testid=test_id>
                        <Flex vertical=true gap=FlexGap::Small>
                            <Checkbox checked=on label=label size=Signal::from(CheckboxSize::Medium) />
                            <Caption2>{help}</Caption2>
                        </Flex>
                    </div>
                }
            })
            .collect_view()
    });
    let blocked = picked.with_value(|all| {
        all.iter()
            .filter(|(c, _)| !c.grantable)
            .map(|(c, _)| (c.clone(), c.group_name.clone().unwrap_or_default()))
            .collect::<Vec<_>>()
    });
    let any_blocked = !blocked.is_empty();
    let blocked_rows = blocked
        .into_iter()
        .map(|(c, group)| {
            let test_id = format!("service-user-access-blocked-{}", c.key);
            view! {
                <div data-testid=test_id>
                    <Flex vertical=true gap=FlexGap::Small>
                        <Checkbox checked=false label=c.label.clone() disabled=true size=Signal::from(CheckboxSize::Medium) />
                        <Flex gap=FlexGap::Small align=FlexAlign::Start>
                            <Icon icon=icondata::AiLockOutlined size=IconSize::Size16 />
                            <Caption2>
                                {format!("Only an owner of {group} or a super user can add service users to it. Ask one of them.")}
                            </Caption2>
                        </Flex>
                    </Flex>
                </div>
            }
        })
        .collect_view();

    view! {
        <CardContent>
            <Flex vertical=true gap=FlexGap::Large>
                {rows}
                <Show when=move || any_blocked><Divider /></Show>
                {blocked_rows}
                {move || error.get().map(|e| {
                    let (intent, text) = if e.contains("STEP_UP:") {
                        (MessageBarIntent::Warning, "Access wasn't added. Choose Continue to try again.".to_string())
                    } else {
                        (MessageBarIntent::Error, friendly_error(&e).to_string())
                    };
                    view! {
                        <div data-testid="service-user-access-error">
                            <MessageBar intent=intent>{text}</MessageBar>
                        </div>
                    }
                })}
            </Flex>
        </CardContent>
        <CardFooter>
            <Flex gap=FlexGap::Small>
                <div data-testid="service-user-access-skip">
                    <Button
                        appearance=ButtonAppearance::Secondary
                        on_click=Callback::new(move |_| on_next.run(Vec::new()))
                    >
                        "Skip for now"
                    </Button>
                </div>
                <div data-testid="service-user-access-continue">
                    <Button
                        appearance=ButtonAppearance::Primary
                        disabled=nothing_selected
                        on_click=Callback::new(next)
                    >
                        "Continue"
                    </Button>
                </div>
            </Flex>
        </CardFooter>
    }
}
