//! Step 1: name the service user, pick a free handle, say what it's for.

use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use leptos_router::hooks::use_navigate;
use orbital_base_components::{IconSize, ThemeColor};
use uf_product::components::{Body1, Caption2, Subtitle2};
use uf_product::primitives::*;

use crate::paths;
use crate::service_users::error::friendly_error;
use crate::service_users::server::{check_service_user_handle, create_service_user_profile};
use crate::service_users::types::{HandleCheck, ServiceUserRow};

/// How long typing must pause before the handle is checked.
const CHECK_DELAY_MS: u64 = 400;

#[derive(Clone, Debug, PartialEq, Eq)]
enum HandleState {
    Empty,
    Checking,
    Checked(HandleCheck),
    Failed,
}

#[component]
pub(super) fn ProfileStep(
    owner_email: String,
    on_created: Callback<ServiceUserRow>,
) -> impl IntoView {
    let navigate = use_navigate();
    let display_name = RwSignal::new(String::new());
    let handle = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let handle_state = RwSignal::new(HandleState::Empty);
    let error = RwSignal::new(None::<String>);
    let pending = RwSignal::new(false);
    let generation = StoredValue::new(0u64);

    // Check the handle once typing pauses; drop answers to older keystrokes.
    Effect::new(move |_| {
        let typed = handle.get();
        generation.update_value(|g| *g += 1);
        let mine = generation.get_value();
        if typed.trim().is_empty() {
            handle_state.set(HandleState::Empty);
            return;
        }
        handle_state.set(HandleState::Checking);
        set_timeout(
            move || {
                // The step may be gone by now (the service user was created).
                if generation.try_get_value() != Some(mine) {
                    return;
                }
                spawn_local_scoped(async move {
                    let result = check_service_user_handle(typed).await;
                    if generation.try_get_value() == Some(mine) {
                        handle_state.set(match result {
                            Ok(check) => HandleState::Checked(check),
                            Err(_) => HandleState::Failed,
                        });
                    }
                });
            },
            std::time::Duration::from_millis(CHECK_DELAY_MS),
        );
    });

    let can_submit = Signal::derive(move || {
        !pending.get()
            && !display_name.with(|n| n.trim().is_empty())
            && matches!(
                handle_state.get(),
                HandleState::Checked(HandleCheck::Free { .. })
            )
    });

    let submit = move |_| {
        error.set(None);
        pending.set(true);
        let name = display_name.get_untracked();
        let h = handle.get_untracked();
        let d = description.get_untracked();
        let d = (!d.trim().is_empty()).then_some(d);
        spawn_local_scoped(async move {
            // On success step 2 replaces this card, so only a failure touches
            // its signals.
            match create_service_user_profile(h.clone(), name, d).await {
                Ok(service_user) => on_created.run(service_user),
                Err(e) => {
                    let raw = e.to_string();
                    if raw.contains("reason_class=handle_taken") {
                        // Someone took it since the last check: show a fresh suggestion.
                        if let Ok(check) = check_service_user_handle(h).await {
                            handle_state.set(HandleState::Checked(check));
                        }
                    }
                    error.set(Some(friendly_error(&raw).to_string()));
                    pending.set(false);
                }
            }
        });
    };

    let status = move || {
        match handle_state.get() {
        HandleState::Empty => ().into_any(),
        HandleState::Checking => view! {
            <Spinner size=SpinnerSize::ExtraTiny label="Checking the handle".to_string() />
            <Caption2>"Checking…"</Caption2>
        }
        .into_any(),
        HandleState::Failed => view! {
            <Caption2>"Couldn't check this handle. Keep typing or try again."</Caption2>
        }
        .into_any(),
        HandleState::Checked(HandleCheck::Free { handle: h }) => view! {
            <Icon icon=icondata::AiCheckCircleOutlined size=IconSize::Size16 color=ThemeColor::PaletteGreenForeground1 />
            <Caption2>{format!("@{h} is free")}</Caption2>
        }
        .into_any(),
        HandleState::Checked(HandleCheck::Taken { handle: h, suggestion }) => view! {
            <Icon icon=icondata::AiCloseCircleOutlined size=IconSize::Size16 color=ThemeColor::PaletteRedForeground1 />
            <Caption2>{format!("@{h} is taken.")}</Caption2>
            {suggestion.map(|s| {
                let label = format!("Use @{s}");
                view! {
                    <div data-testid="service-user-handle-use-suggestion">
                        <Button
                            appearance=ButtonAppearance::Subtle
                            size=ButtonSize::Small
                            on_click=Callback::new(move |_| handle.set(s.clone()))
                        >
                            {label}
                        </Button>
                    </div>
                }
            })}
        }
        .into_any(),
        HandleState::Checked(HandleCheck::Invalid { problem }) => view! {
            <Icon icon=icondata::AiCloseCircleOutlined size=IconSize::Size16 color=ThemeColor::PaletteRedForeground1 />
            <Caption2>{problem.message()}</Caption2>
        }
        .into_any(),
    }
    };

    view! {
        <Card>
            <CardHeader><Subtitle2>"Name it"</Subtitle2></CardHeader>
            <CardContent>
                <Flex vertical=true gap=FlexGap::Medium>
                    <div data-testid="service-user-display-name">
                        <Field label="Display name" required=true>
                            <Input
                                bind=InputBind { name: "display_name".into(), ..InputBind::new(display_name) }
                                appearance=InputAppearance::with_placeholder("Spectra agent")
                            />
                        </Field>
                    </div>
                    <div data-testid="service-user-handle">
                        <Field label="Handle" required=true>
                            <Input
                                bind=InputBind { name: "handle".into(), ..InputBind::new(handle) }
                                appearance=InputAppearance::with_placeholder("spectra-agent")
                            >
                                <InputPrefix slot><Body1>"@"</Body1></InputPrefix>
                            </Input>
                        </Field>
                    </div>
                    <div data-testid="service-user-handle-status" aria-live="polite">
                        <Flex gap=FlexGap::Small align=FlexAlign::Center wrap=FlexWrap::Wrap>
                            {status}
                        </Flex>
                    </div>
                    <div data-testid="service-user-description">
                        <Field label="Description">
                            <Textarea
                                bind=TextareaBind::new(description)
                                appearance=TextareaAppearance {
                                    placeholder: "What this service user does".to_string().into(),
                                    rows: 3.into(),
                                    resize: TextareaResize::Vertical.into(),
                                    ..Default::default()
                                }
                            />
                        </Field>
                        <Caption2>"Optional. Up to 500 characters."</Caption2>
                    </div>
                    <Field label="Owner">
                        <div data-testid="service-user-owner"><Body1>{format!("You ({owner_email})")}</Body1></div>
                    </Field>
                    {move || error.get().map(|e| view! {
                        <div data-testid="service-user-profile-error">
                            <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
                        </div>
                    })}
                </Flex>
            </CardContent>
            <CardFooter>
                <Flex gap=FlexGap::Small>
                    <Button
                        appearance=ButtonAppearance::Secondary
                        on_click=Callback::new(move |_| navigate(paths::SERVICE_USERS, Default::default()))
                    >
                        "Cancel"
                    </Button>
                    <div data-testid="service-user-create-submit">
                        <Button
                            appearance=ButtonAppearance::Primary
                            loading=pending
                            disabled=Signal::derive(move || !can_submit.get())
                            on_click=Callback::new(submit)
                        >
                            "Create service user"
                        </Button>
                    </div>
                </Flex>
            </CardFooter>
        </Card>
    }
}
