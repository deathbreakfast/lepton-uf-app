//! The "Change access" dialog on a service user's page: add it to the groups behind the
//! access choices it doesn't hold yet.
//!
//! Taking access away happens per group with the Remove button on the Groups card.

use leptos::prelude::*;
use uf_product::components::{Body1, Caption2};
use uf_product::primitives::*;

use super::error::friendly_error;
use super::request_access::RequestGroupAccess;
use super::server::{grant_service_user_access, service_user_access_choices};
use super::step_up::spawn_with_step_up;
use super::types::AccessChoiceView;

/// Button that opens the dialog. `held` is the keys of the choices the service user
/// already has.
#[component]
pub(super) fn ChangeAccess(
    service_user_id: String,
    handle: String,
    held: Vec<String>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <div data-testid="service-user-detail-change-access">
            <Button
                appearance=ButtonAppearance::Secondary
                icon=icondata::AiSafetyOutlined
                on_click=Callback::new(move |_| open.set(true))
            >
                "Change access"
            </Button>
        </div>
        <Dialog open=open>
            <DialogSurface>
                <DialogBody>
                    <DialogTitle>{format!("Change what @{handle} can do")}</DialogTitle>
                    <Show when=move || open.get()>
                        <Choices
                            service_user_id=service_user_id.clone()
                            handle=handle.clone()
                            held=held.clone()
                            open=open
                            on_changed=on_changed
                        />
                    </Show>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}

#[component]
fn Choices(
    service_user_id: String,
    handle: String,
    held: Vec<String>,
    open: RwSignal<bool>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let id = service_user_id.clone();
    let choices = Resource::new(move || id.clone(), service_user_access_choices);
    view! {
        <Suspense fallback=|| view! {
            <DialogContent>
                <Skeleton><SkeletonItem /></Skeleton>
            </DialogContent>
        }>
            {move || choices.get().map(|res| match res {
                Ok(list) => view! {
                    <Form
                        service_user_id=service_user_id.clone()
                        handle=handle.clone()
                        choices=list
                        held=held.clone()
                        open=open
                        on_changed=on_changed
                    />
                }.into_any(),
                Err(e) => view! {
                    <DialogContent>
                        <div data-testid="service-user-change-access-error">
                            <MessageBar intent=MessageBarIntent::Error>{friendly_error(&e.to_string())}</MessageBar>
                        </div>
                    </DialogContent>
                }.into_any(),
            })}
        </Suspense>
    }
}

#[component]
fn Form(
    service_user_id: String,
    handle: String,
    choices: Vec<AccessChoiceView>,
    held: Vec<String>,
    open: RwSignal<bool>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let error = RwSignal::new(None::<String>);
    let pending = RwSignal::new(false);
    // Token-only choices live on tokens, so only group choices are listed here.
    let (open_rows, held_rows): (Vec<_>, Vec<_>) = choices
        .into_iter()
        .filter(|c| c.group_name.is_some())
        .partition(|c| !held.contains(&c.key));
    let picked: Vec<(AccessChoiceView, RwSignal<bool>)> = open_rows
        .into_iter()
        .map(|c| (c, RwSignal::new(false)))
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
    let nothing_selected = Signal::derive(move || keys().is_empty());

    let save = move |_| {
        let keys = keys();
        if keys.is_empty() {
            return;
        }
        error.set(None);
        pending.set(true);
        let service_user_id = service_user_id.clone();
        spawn_with_step_up(
            error,
            Callback::new(move |()| {
                pending.set(false);
                open.set(false);
                on_changed.run(());
            }),
            move || grant_service_user_access(service_user_id.clone(), keys.clone()),
        );
    };

    let addable = picked.with_value(|all| {
        all.iter()
            .map(|(c, on)| {
                let test_id = format!("service-user-change-access-{}", c.key);
                let (label, help, on, grantable) =
                    (c.label.clone(), c.help.clone(), *on, c.grantable);
                let group = c.group_name.clone().unwrap_or_default();
                let url = c.request_url.clone();
                let handle = handle.clone();
                view! {
                    <div data-testid=test_id>
                        <Flex vertical=true gap=FlexGap::Small>
                            <Checkbox
                                checked=on
                                label=label
                                disabled=!grantable
                                size=Signal::from(CheckboxSize::Medium)
                            />
                            {if grantable {
                                view! { <Caption2>{help}</Caption2> }.into_any()
                            } else {
                                view! {
                                    <Caption2>
                                        <RequestGroupAccess group=group handle=handle url=url />
                                    </Caption2>
                                }.into_any()
                            }}
                        </Flex>
                    </div>
                }
            })
            .collect_view()
    });
    let none_left = picked.with_value(Vec::is_empty);
    let held_list = held_rows
        .into_iter()
        .map(|c| {
            let test_id = format!("service-user-change-access-held-{}", c.key);
            view! {
                <div data-testid=test_id>
                    <Checkbox checked=true label=c.label disabled=true size=Signal::from(CheckboxSize::Medium) />
                </div>
            }
        })
        .collect_view();

    view! {
        <DialogContent>
            <Flex vertical=true gap=FlexGap::Large>
                {none_left.then(|| view! {
                    <div data-testid="service-user-change-access-none">
                        <Body1>"There's nothing left to add. Remove it from a group to take access away."</Body1>
                    </div>
                })}
                {addable}
                {held_list}
                {move || error.get().map(|e| {
                    let (intent, text) = if e.contains("STEP_UP:") {
                        (MessageBarIntent::Warning, "Access wasn't added. Choose Add access to try again.".to_string())
                    } else {
                        (MessageBarIntent::Error, friendly_error(&e).to_string())
                    };
                    view! {
                        <div data-testid="service-user-change-access-error">
                            <MessageBar intent=intent>{text}</MessageBar>
                        </div>
                    }
                })}
            </Flex>
        </DialogContent>
        <DialogActions>
            <Button
                appearance=ButtonAppearance::Secondary
                on_click=Callback::new(move |_| open.set(false))
            >
                "Cancel"
            </Button>
            <div data-testid="service-user-change-access-submit">
                <Button
                    appearance=ButtonAppearance::Primary
                    disabled=nothing_selected
                    loading=pending
                    on_click=Callback::new(save)
                >
                    "Add access"
                </Button>
            </div>
        </DialogActions>
    }
}
