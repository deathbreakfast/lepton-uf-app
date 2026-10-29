//! `/user/service-users/:service_user_id`: one service user, its groups and permissions, and
//! Pause and Resume.
//!
//! Groups and permissions come from the host's
//! [`ServiceUserAccessBackend`](super::ServiceUserAccessBackend). A host that can't list them
//! yet keeps the default methods, and both cards show their empty state. Each card can hold
//! hundreds of rows, so its rows scroll inside the card and the page stays short.

use lepton_auth_ui::{use_step_up_controller, StepUpFactors, StepUpPolicy, StepUpRequest};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use leptos_router::hooks::{use_navigate, use_params_map};
use serde::{Deserialize, Serialize};
use uf_product::components::{
    Body1, Body1Strong, Caption2, ContentContainer, EmptyState, EmptyStateCallToAction, ScrollArea,
    SkeletonItemSize, Subtitle2, Title3, ToastOptions,
};
use uf_product::primitives::*;

use super::error::friendly_error;
use super::list_page::ServiceUserBadge;
use super::port::{ServiceUserGroupView, ServiceUserPermissionView};
use super::types::{ServiceUserRow, ServiceUserState};
use crate::paths;

/// Rows show inside a scroll area this tall: about 20 rows on a desktop and 10 on a phone.
const LIST_MAX_HEIGHT: &str = "max-height: min(800px, 50dvh);";

/// Everything the detail page shows, in one load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceUserDetail {
    /// The service user.
    pub service_user: ServiceUserRow,
    /// Groups it is a member of, by name.
    pub groups: Vec<ServiceUserGroupView>,
    /// Permissions it holds through those groups, by name.
    pub permissions: Vec<ServiceUserPermissionView>,
}

/// One service user with its groups and permissions. Fails as not found for a service user
/// the viewer can't see.
#[uf_product_macros::server]
pub async fn get_service_user_detail(
    /// Service user id.
    service_user_id: String,
) -> Result<ServiceUserDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::server::ssr;

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let service_user = lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("detail", &e))?;
        let (mut groups, mut permissions) =
            match crate::service_users::port::use_service_user_access_backend() {
                Some(backend) => (
                    backend
                        .service_user_groups(&v, &id)
                        .await
                        .map_err(|e| ssr::access_err("detail", &e))?,
                    backend
                        .service_user_permissions(&v, &id)
                        .await
                        .map_err(|e| ssr::access_err("detail", &e))?,
                ),
                None => (Vec::new(), Vec::new()),
            };
        groups.sort_by(|a, b| a.name.cmp(&b.name));
        permissions.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ServiceUserDetail {
            service_user: ssr::row(&service_user),
            groups,
            permissions,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = service_user_id;
        Err(ServerFnError::new("server only"))
    }
}

/// Pause a service user so its tokens stop working.
#[uf_product_macros::server]
pub async fn pause_service_user(
    /// Service user id.
    service_user_id: String,
) -> Result<ServiceUserRow, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::server::ssr;

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let service_user = lepton_auth::service_users::suspend_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("pause", &e))?;
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            "service user paused"
        );
        Ok(ssr::row(&service_user))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = service_user_id;
        Err(ServerFnError::new("server only"))
    }
}

/// Resume a paused service user so its tokens work again. Needs a fresh authenticator code,
/// like minting a token.
#[uf_product_macros::server(step_up = "fresh")]
pub async fn resume_service_user(
    /// Service user id.
    service_user_id: String,
    /// Fresh authenticator code confirming the resume.
    totp_code: String,
) -> Result<ServiceUserRow, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::server::ssr;

        ssr::require_fresh_code(&totp_code).await?;
        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let service_user = lepton_auth::service_users::resume_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("resume", &e))?;
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            "service user resumed"
        );
        Ok(ssr::row(&service_user))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (service_user_id, totp_code);
        Err(ServerFnError::new("server only"))
    }
}

/// Explain a Pause or Resume failure. `pausing` says which one was tried.
fn change_error(raw: &str, pausing: bool) -> &'static str {
    if raw.contains("reason_class=transition") {
        if pausing {
            "It's already paused. Reload to see its current state."
        } else {
            "It's already active. Reload to see its current state."
        }
    } else {
        friendly_error(raw)
    }
}

/// `/user/service-users/:service_user_id` page.
///
/// Wrap in [`uf_product::routes::RequireAuthenticated`] at the route.
#[component]
pub fn ServiceUserDetailPage() -> impl IntoView {
    let params = use_params_map();
    let detail = Resource::new(
        move || params.with(|p| p.get("service_user_id").unwrap_or_default()),
        get_service_user_detail,
    );
    let on_changed = Callback::new(move |()| detail.refetch());
    view! {
        <ContentContainer max_width="900px".to_string() data_testid="service-user-detail-page".to_string()>
            <Flex vertical=true gap=FlexGap::Large>
                <Title3>"Service user"</Title3>
                <Suspense fallback=|| view! {
                    <Skeleton>
                        <SkeletonItem size=Signal::from(SkeletonItemSize::S48) />
                        <SkeletonItem size=Signal::from(SkeletonItemSize::S48) />
                    </Skeleton>
                }>
                    {move || detail.get().map(|res| match res {
                        Ok(d) => view! { <Detail detail=d on_changed=on_changed /> }.into_any(),
                        Err(e) => view! { <NotFound raw=e.to_string() /> }.into_any(),
                    })}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

#[component]
fn Detail(detail: ServiceUserDetail, on_changed: Callback<()>) -> impl IntoView {
    let ServiceUserDetail {
        service_user,
        groups,
        permissions,
    } = detail;
    view! {
        <Identity service_user=service_user on_changed=on_changed />
        <GroupsCard groups=groups />
        <PermissionsCard permissions=permissions />
    }
}

#[component]
fn Identity(service_user: ServiceUserRow, on_changed: Callback<()>) -> impl IntoView {
    let navigate = use_navigate();
    let paused = service_user.state == ServiceUserState::Paused;
    let (badge_color, badge_text) = if paused {
        (BadgeColor::Warning, "Paused")
    } else {
        (BadgeColor::Success, "Active")
    };
    let subtitle = format!(
        "@{} · Created {}",
        service_user.handle, service_user.created_on
    );
    let token_path = paths::service_users_token(&service_user.id);
    let handle = service_user.handle.clone();
    let id = service_user.id.clone();
    view! {
        <div data-testid="service-user-detail-identity">
            <Card>
                <CardHeader>
                    <Flex gap=FlexGap::Small align=FlexAlign::Center>
                        <Subtitle2>{service_user.display_name.clone()}</Subtitle2>
                        <ServiceUserBadge />
                    </Flex>
                    <CardHeaderDescription slot>
                        <Caption2>{subtitle}</Caption2>
                    </CardHeaderDescription>
                    <CardHeaderAction slot>
                        <span data-testid="service-user-detail-state">
                            <Badge appearance=BadgeAppearance::Tint size=BadgeSize::Small color=badge_color>
                                {badge_text}
                            </Badge>
                        </span>
                    </CardHeaderAction>
                </CardHeader>
                {service_user.description.map(|d| view! { <CardContent><Body1>{d}</Body1></CardContent> })}
                <CardFooter>
                    <Flex gap=FlexGap::Small wrap=FlexWrap::Wrap>
                        <PauseResume id=id handle=handle paused=paused on_changed=on_changed />
                        <Button
                            appearance=ButtonAppearance::Subtle
                            icon=icondata::AiKeyOutlined
                            disabled=paused
                            on_click=Callback::new(move |_| navigate(&token_path, Default::default()))
                        >
                            "New token"
                        </Button>
                    </Flex>
                </CardFooter>
            </Card>
        </div>
    }
}

fn resume_request(handle: &str) -> StepUpRequest {
    StepUpRequest {
        title: "Confirm it's you".to_string(),
        description: Some(format!(
            "Enter the code from your authenticator app to resume @{handle}."
        )),
        policy: StepUpPolicy::Totp,
    }
}

/// Pause or Resume button, its confirm dialog, and the toast that follows.
#[component]
fn PauseResume(
    id: String,
    handle: String,
    paused: bool,
    on_changed: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let toaster = use_context::<ToasterInjection>();
    let step_up = use_step_up_controller();

    let (label, title, body, confirm_label) = if paused {
        (
            "Resume",
            format!("Resume @{handle}?"),
            "Its tokens work again straight away. You'll confirm with your authenticator code.",
            "Resume",
        )
    } else {
        (
            "Pause",
            format!("Pause @{handle}?"),
            "Its tokens stop working straight away. Nothing is deleted, and you can resume it later.",
            "Pause",
        )
    };
    let done_text = if paused {
        format!("@{handle} is active again")
    } else {
        format!("@{handle} is paused")
    };

    let done = Callback::new(move |()| {
        if let Some(toaster) = toaster {
            toaster
                .dispatch(ToastOptions::new(done_text.clone()).intent(ToastIntent::Success.into()));
        }
        on_changed.run(());
    });

    let confirm = {
        let (id, handle) = (id, handle);
        move |_| {
            open.set(false);
            error.set(None);
            if !paused {
                busy.set(true);
                let id = id.clone();
                spawn_local_scoped(async move {
                    let result = pause_service_user(id).await;
                    busy.set(false);
                    match result {
                        Ok(_) => done.run(()),
                        Err(e) => error.set(Some(change_error(&e.to_string(), true).to_string())),
                    }
                });
                return;
            }
            let Some(ctrl) = step_up else {
                error.set(Some(
                    "Two-factor confirmation is not available on this page. Reload and try again."
                        .to_string(),
                ));
                return;
            };
            let id = id.clone();
            ctrl.request(
                resume_request(&handle),
                Callback::new(move |factors: StepUpFactors| {
                    let id = id.clone();
                    spawn_local_scoped(async move {
                        match resume_service_user(id, factors.totp_code).await {
                            Ok(_) => {
                                ctrl.complete_success();
                                done.run(());
                            }
                            Err(e) => {
                                let message = e.to_string();
                                if message.contains("STEP_UP:") {
                                    // Wrong or reused code: keep the dialog open to retry.
                                    ctrl.report_error(message);
                                } else {
                                    ctrl.complete_success();
                                    error.set(Some(change_error(&message, false).to_string()));
                                }
                            }
                        }
                    });
                }),
            );
        }
    };

    let (appearance, icon) = if paused {
        (ButtonAppearance::Primary, icondata::AiPlayCircleOutlined)
    } else {
        (ButtonAppearance::Secondary, icondata::AiPauseCircleOutlined)
    };
    let action_test_id = if paused {
        "service-user-detail-resume"
    } else {
        "service-user-detail-pause"
    };

    view! {
        <div data-testid=action_test_id>
            <Button
                appearance=appearance
                icon=icon
                disabled=Signal::derive(move || busy.get())
                on_click=Callback::new(move |_| open.set(true))
            >
                {label}
            </Button>
        </div>
        {move || error.get().map(|e| view! {
            <div data-testid="service-user-detail-error">
                <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
            </div>
        })}
        <Dialog open=open>
            <DialogSurface>
                <DialogBody>
                    <DialogTitle>{title}</DialogTitle>
                    <DialogContent>
                        <div data-testid="service-user-detail-confirm">
                            <Body1>{body}</Body1>
                        </div>
                    </DialogContent>
                    <DialogActions>
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on_click=Callback::new(move |_| open.set(false))
                        >
                            "Cancel"
                        </Button>
                        <div data-testid="service-user-detail-confirm-submit">
                            <Button appearance=ButtonAppearance::Primary on_click=Callback::new(confirm)>
                                {confirm_label}
                            </Button>
                        </div>
                    </DialogActions>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}

/// A card whose rows scroll inside it, with the row count in its header.
#[component]
fn ListCard(
    title: &'static str,
    count: usize,
    description: &'static str,
    empty: &'static str,
    test_id: &'static str,
    children: Children,
) -> impl IntoView {
    let heading = format!("{title} ({count})");
    let scroll_id = format!("{test_id}-scroll");
    view! {
        <div data-testid=test_id>
            <Card>
                <CardHeader>
                    <Subtitle2>{heading}</Subtitle2>
                    <CardHeaderDescription slot>
                        <Caption2>{description}</Caption2>
                    </CardHeaderDescription>
                </CardHeader>
                <CardContent>
                    {if count == 0 {
                        view! {
                            <div data-testid=format!("{test_id}-empty")>
                                <Caption2>{empty}</Caption2>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            // Focusable so keyboard users can scroll the list.
                            <ScrollArea
                                style=LIST_MAX_HEIGHT
                                scroll_testid=scroll_id
                                attr:tabindex="0"
                                attr:role="region"
                                attr:aria-label=title
                            >
                                <Flex vertical=true gap=FlexGap::Small>{children()}</Flex>
                            </ScrollArea>
                        }.into_any()
                    }}
                </CardContent>
            </Card>
        </div>
    }
}

#[component]
fn GroupsCard(groups: Vec<ServiceUserGroupView>) -> impl IntoView {
    let count = groups.len();
    view! {
        <ListCard
            title="Groups"
            count=count
            description="The service user holds what these groups hold."
            empty="It isn't in any groups."
            test_id="service-user-detail-groups"
        >
            {groups.into_iter().map(|g| view! {
                <div data-testid="service-user-detail-group-row">
                    <Flex vertical=true>
                        <Body1Strong>{g.name}</Body1Strong>
                        {(!g.description.is_empty()).then(|| view! { <Caption2>{g.description}</Caption2> })}
                    </Flex>
                </div>
            }).collect_view()}
        </ListCard>
    }
}

#[component]
fn PermissionsCard(permissions: Vec<ServiceUserPermissionView>) -> impl IntoView {
    let count = permissions.len();
    view! {
        <ListCard
            title="Permissions"
            count=count
            description="What the service user can do, through its groups."
            empty="It has no permissions."
            test_id="service-user-detail-permissions"
        >
            {permissions.into_iter().map(|p| {
                let caption = if p.description.is_empty() {
                    format!("Through {}", p.via_group)
                } else {
                    format!("{} · Through {}", p.description, p.via_group)
                };
                view! {
                    <div data-testid="service-user-detail-permission-row">
                        <Flex vertical=true>
                            <Body1Strong>{p.name}</Body1Strong>
                            <Caption2>{caption}</Caption2>
                        </Flex>
                    </div>
                }
            }).collect_view()}
        </ListCard>
    }
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
        <div data-testid="service-user-detail-not-found">
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_error_explains_a_repeated_change() {
        let raw = "reason_class=transition: service user is already suspended";
        assert!(change_error(raw, true).contains("already paused"));
        assert!(change_error(raw, false).contains("already active"));
    }

    #[test]
    fn change_error_falls_back_to_friendly_error() {
        assert!(change_error("reason_class=not_found", true).contains("isn't yours"));
        assert!(change_error("reason_class=forbidden", false).contains("can't change"));
        assert!(change_error("disk on fire", true).starts_with("Something went wrong"));
    }
}
