//! `/user/service-users/:service_user_id`: one service user, its groups, permissions and
//! tokens, Pause and Resume, token Revoke and Rotate, group Remove, Change access, and Delete.
//!
//! The header shows the owner and the day any token was last used. Groups, permissions and
//! tokens come from the host's
//! [`ServiceUserAccessBackend`](super::ServiceUserAccessBackend). A host that can't list them
//! yet keeps the default methods, and each card shows its empty state. Each card can hold
//! hundreds of rows, so its rows scroll inside the card and the page stays short.
//!
//! Revoke and Rotate go through the host's
//! [`revoke_token`](super::ServiceUserAccessBackend::revoke_token) and
//! [`rotate_token`](super::ServiceUserAccessBackend::rotate_token). A rotated token's secret
//! shows once at the top of the page. Remove takes the service user out of a group, and
//! Change access adds it to the groups behind the choices it doesn't hold yet.
//!
//! Delete asks for the handle to be typed, and the server checks it again.

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

use super::change_access::ChangeAccess;
use super::error::friendly_error;
use super::list_page::ServiceUserBadge;
use super::port::{ServiceUserGroupView, ServiceUserPermissionView, ServiceUserTokenView};
use super::step_up::{spawn_with_fresh_code, spawn_with_step_up};
use super::token::TokenReveal;
use super::types::MintedTokenView;
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
    /// Tokens that still work, newest first.
    pub tokens: Vec<ServiceUserTokenView>,
    /// Who owns it: "you", the owner's display name, or "another user".
    pub owner: String,
    /// Day any of its tokens was last used, `YYYY-MM-DD`, or `None` when none ever was.
    pub last_used_on: Option<String>,
    /// Keys of the access choices it holds today.
    pub held_access: Vec<String>,
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
        let (mut groups, mut permissions, mut tokens, held_access) =
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
                    backend
                        .list_tokens(&v, &id)
                        .await
                        .map_err(|e| ssr::access_err("detail", &e))?,
                    backend
                        .current_access(&v, &id)
                        .await
                        .map_err(|e| ssr::access_err("detail", &e))?
                        .into_iter()
                        .map(str::to_string)
                        .collect::<Vec<_>>(),
                ),
                None => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            };
        let owner = ssr::owner_label(&v, &service_user).await;
        let last_used_on = tokens.iter().filter_map(|t| t.last_used_on.clone()).max();
        groups.sort_by(|a, b| a.name.cmp(&b.name));
        permissions.sort_by(|a, b| a.name.cmp(&b.name));
        tokens.sort_by(|a, b| {
            b.created_on
                .cmp(&a.created_on)
                .then_with(|| a.label.cmp(&b.label))
        });
        Ok(ServiceUserDetail {
            service_user: ssr::row(&service_user),
            groups,
            permissions,
            tokens,
            owner,
            last_used_on,
            held_access,
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

/// Delete a service user. `confirm_handle` is the handle the person typed to confirm; a
/// mismatch is refused before anything changes.
#[uf_product_macros::server]
pub async fn delete_service_user(
    /// Service user id.
    service_user_id: String,
    /// Handle as typed in the confirm dialog, with or without the `@`.
    confirm_handle: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::server::ssr;

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let service_user = lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("delete", &e))?;
        if !handle_matches(&confirm_handle, service_user.handle()) {
            return Err(ServerFnError::new(
                "reason_class=confirm: typed handle does not match",
            ));
        }
        lepton_auth::service_users::delete_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("delete", &e))?;
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            "service user deleted"
        );
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (service_user_id, confirm_handle);
        Err(ServerFnError::new("server only"))
    }
}

/// Revoke one of a service user's tokens so it stops working now.
#[uf_product_macros::server]
pub async fn revoke_service_user_token(
    /// Service user id.
    service_user_id: String,
    /// Public id of the token to revoke.
    public_id: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::server::ssr;

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let backend = ssr::backend()?;
        // Load first so a service user the viewer can't see fails as not found.
        lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("revoke", &e))?;
        backend
            .revoke_token(&v, &id, &public_id)
            .await
            .map_err(|e| ssr::access_err("revoke", &e))?;
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            token_public_id = public_id.as_str(),
            "service user token revoked"
        );
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (service_user_id, public_id);
        Err(ServerFnError::new("server only"))
    }
}

/// Replace one of a service user's tokens with a new one that carries the same access.
/// The old token stops working now, and the response is the only time the new secret is
/// sent. Needs a fresh authenticator code, like minting.
#[uf_product_macros::server(step_up = "fresh")]
pub async fn rotate_service_user_token(
    /// Service user id.
    service_user_id: String,
    /// Public id of the token to replace.
    public_id: String,
    /// Fresh authenticator code confirming the rotation.
    totp_code: String,
) -> Result<MintedTokenView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::port::{resolve_choices, summarize};
        use super::server::ssr;

        ssr::require_fresh_code(&totp_code).await?;
        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let backend = ssr::backend()?;
        // Load first so a service user the viewer can't see fails as not found.
        let service_user = lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("rotate", &e))?;
        let held: Vec<String> = backend
            .current_access(&v, &id)
            .await
            .map_err(|e| ssr::access_err("rotate", &e))?
            .into_iter()
            .map(str::to_string)
            .collect();
        let chosen =
            resolve_choices(backend.choices(), &held).map_err(|e| ssr::access_err("rotate", &e))?;
        let minted = backend
            .rotate_token(&v, &id, &public_id)
            .await
            .map_err(|e| ssr::access_err("rotate", &e))?;
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            old_token_public_id = public_id.as_str(),
            token_public_id = minted.public_id.as_str(),
            "service user token rotated"
        );
        Ok(MintedTokenView {
            public_id: minted.public_id,
            secret: minted.secret,
            expires_on: minted.expires_at.format("%Y-%m-%d").to_string(),
            can_do: summarize(&chosen),
            setup: backend.client_setup(service_user.handle(), &chosen),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (service_user_id, public_id, totp_code);
        Err(ServerFnError::new("server only"))
    }
}

/// Remove a service user from a group, so it stops holding what the group holds. Needs a
/// recent authenticator confirmation, like adding to a group, and only a group's owners
/// may do it.
#[uf_product_macros::server(step_up)]
pub async fn remove_service_user_from_group(
    /// Service user id.
    service_user_id: String,
    /// Gauge group id to leave.
    group_id: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::port::ServiceUserAccessError;
        use super::server::ssr;

        let (v, _) = ssr::session().await?;
        let id = ssr::parse_id(&service_user_id)?;
        let backend = ssr::backend()?;
        // Load first so a service user the viewer can't see fails as not found.
        lepton_auth::service_users::get_service_user(&v, &id)
            .await
            .map_err(|e| ssr::service_user_err("remove_group", &e))?;
        let allowed = backend
            .can_grant_group(&v, &group_id)
            .await
            .map_err(|e| ssr::access_err("remove_group", &e))?;
        if !allowed {
            return Err(ssr::access_err(
                "remove_group",
                &ServiceUserAccessError::Forbidden,
            ));
        }
        backend
            .revoke_group(&v, &id, &group_id)
            .await
            .map_err(|e| ssr::access_err("remove_group", &e))?;
        tracing::info!(
            target: "lepton_app.service_users",
            service_user_id = id.as_str(),
            group_id = group_id.as_str(),
            "service user removed from group"
        );
        Ok(())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (service_user_id, group_id);
        Err(ServerFnError::new("server only"))
    }
}

/// Whether `typed` names `handle`. A leading `@` and surrounding spaces are fine;
/// everything else must match exactly.
fn handle_matches(typed: &str, handle: &str) -> bool {
    let typed = typed.trim();
    typed.strip_prefix('@').unwrap_or(typed) == handle
}

/// Explain a Delete failure.
fn delete_error(raw: &str) -> &'static str {
    if raw.contains("reason_class=confirm") {
        "Type the handle exactly to delete it."
    } else {
        friendly_error(raw)
    }
}

/// Explain a Revoke failure.
fn revoke_error(raw: &str) -> &'static str {
    if raw.contains("reason_class=token_not_found") {
        "That token is already gone. Reload to see the current list."
    } else {
        friendly_error(raw)
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
    // A rotated token's secret lives here, above the Suspense, so reloading the details
    // behind it doesn't drop it. Nothing stores it; leaving the page drops it.
    let revealed = RwSignal::new(None::<(String, MintedTokenView)>);
    view! {
        <ContentContainer max_width="900px".to_string() data_testid="service-user-detail-page".to_string()>
            <Flex vertical=true gap=FlexGap::Large>
                <Title3>"Service user"</Title3>
                {move || revealed.get().map(|(handle, token)| view! {
                    <TokenReveal handle=handle token=token />
                })}
                <Suspense fallback=|| view! {
                    <Skeleton>
                        <SkeletonItem size=Signal::from(SkeletonItemSize::S48) />
                        <SkeletonItem size=Signal::from(SkeletonItemSize::S48) />
                    </Skeleton>
                }>
                    {move || detail.get().map(|res| match res {
                        Ok(d) => view! { <Detail detail=d on_changed=on_changed revealed=revealed /> }.into_any(),
                        Err(e) => view! { <NotFound raw=e.to_string() /> }.into_any(),
                    })}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

#[component]
fn Detail(
    detail: ServiceUserDetail,
    on_changed: Callback<()>,
    revealed: RwSignal<Option<(String, MintedTokenView)>>,
) -> impl IntoView {
    let ServiceUserDetail {
        service_user,
        groups,
        permissions,
        tokens,
        owner,
        last_used_on,
        held_access,
    } = detail;
    let id = service_user.id.clone();
    let handle = service_user.handle.clone();
    view! {
        <Identity service_user=service_user owner=owner last_used_on=last_used_on on_changed=on_changed />
        <TokensCard service_user_id=id.clone() handle=handle.clone() tokens=tokens revealed=revealed on_changed=on_changed />
        <GroupsCard service_user_id=id handle=handle groups=groups held_access=held_access on_changed=on_changed />
        <PermissionsCard permissions=permissions />
    }
}

#[component]
fn Identity(
    service_user: ServiceUserRow,
    owner: String,
    last_used_on: Option<String>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let navigate = use_navigate();
    let paused = service_user.state == ServiceUserState::Paused;
    let (badge_color, badge_text) = if paused {
        (BadgeColor::Warning, "Paused")
    } else {
        (BadgeColor::Success, "Active")
    };
    let used = last_used_on.map_or_else(|| "Never used".to_string(), |d| format!("Last used {d}"));
    let subtitle = format!(
        "@{} · Owned by {owner} · Created {} · {used}",
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
                        <span data-testid="service-user-detail-meta"><Caption2>{subtitle}</Caption2></span>
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
                        <PauseResume id=id.clone() handle=handle.clone() paused=paused on_changed=on_changed />
                        <Button
                            appearance=ButtonAppearance::Subtle
                            icon=icondata::AiKeyOutlined
                            disabled=paused
                            on_click=Callback::new(move |_| navigate(&token_path, Default::default()))
                        >
                            "New token"
                        </Button>
                        <DeleteServiceUser id=id handle=handle />
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

/// Delete button and its typed-handle confirm dialog. Goes back to the list once
/// the service user is gone.
#[component]
fn DeleteServiceUser(id: String, handle: String) -> impl IntoView {
    let navigate = use_navigate();
    let open = RwSignal::new(false);
    let typed = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let toaster = use_context::<ToasterInjection>();

    let matches = {
        let handle = handle.clone();
        Signal::derive(move || typed.with(|t| handle_matches(t, &handle)))
    };
    let mismatch = Signal::derive(move || !matches.get() && typed.with(|t| !t.trim().is_empty()));
    let title = format!("Delete @{handle}?");
    let mismatch_text = format!("That doesn't match @{handle}.");
    let done_text = format!("@{handle} deleted");

    let confirm = move |_| {
        if !matches.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        let (id, confirm_handle) = (id.clone(), typed.get_untracked());
        let (navigate, done_text) = (navigate.clone(), done_text.clone());
        spawn_local_scoped(async move {
            let result = delete_service_user(id, confirm_handle).await;
            busy.set(false);
            match result {
                Ok(()) => {
                    open.set(false);
                    if let Some(toaster) = toaster {
                        toaster.dispatch(
                            ToastOptions::new(done_text).intent(ToastIntent::Success.into()),
                        );
                    }
                    navigate(paths::SERVICE_USERS, Default::default());
                }
                Err(e) => error.set(Some(delete_error(&e.to_string()).to_string())),
            }
        });
    };

    view! {
        <div data-testid="service-user-detail-delete">
            <Button
                appearance=ButtonAppearance::Subtle
                icon=icondata::AiDeleteOutlined
                on_click=Callback::new(move |_| {
                    typed.set(String::new());
                    error.set(None);
                    open.set(true);
                })
            >
                "Delete"
            </Button>
        </div>
        <Dialog open=open>
            <DialogSurface>
                <DialogBody>
                    <DialogTitle>{title}</DialogTitle>
                    <DialogContent>
                        <Flex vertical=true gap=FlexGap::Medium>
                            <div data-testid="service-user-delete-confirm">
                                <Body1>
                                    "Its tokens stop working and it can't be brought back. Type its handle to confirm."
                                </Body1>
                            </div>
                            <div data-testid="service-user-delete-handle">
                                <Field label="Handle" required=true>
                                    <Input
                                        bind=InputBind { name: "confirm_handle".into(), ..InputBind::new(typed) }
                                        appearance=InputAppearance::with_placeholder(handle.clone())
                                    />
                                </Field>
                            </div>
                            {move || {
                                let text = mismatch_text.clone();
                                mismatch.get().then(move || view! {
                                    <div data-testid="service-user-delete-mismatch">
                                        <Caption2>{text}</Caption2>
                                    </div>
                                })
                            }}
                            {move || error.get().map(|e| view! {
                                <div data-testid="service-user-delete-error">
                                    <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
                                </div>
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
                        <div data-testid="service-user-delete-submit">
                            <Button
                                appearance=ButtonAppearance::Primary
                                disabled=Signal::derive(move || !matches.get() || busy.get())
                                on_click=Callback::new(confirm)
                            >
                                "Delete"
                            </Button>
                        </div>
                    </DialogActions>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}

#[component]
fn TokensCard(
    service_user_id: String,
    handle: String,
    tokens: Vec<ServiceUserTokenView>,
    revealed: RwSignal<Option<(String, MintedTokenView)>>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let count = tokens.len();
    let error = RwSignal::new(None::<String>);
    view! {
        {move || error.get().map(|e| view! {
            <div data-testid="service-user-detail-token-error">
                <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
            </div>
        })}
        <ListCard
            title="Tokens"
            count=count
            description="Tokens that still work. Revoking one stops it straight away. Rotating one swaps it for a new token with the same access."
            empty="It has no tokens."
            test_id="service-user-detail-tokens"
        >
            {tokens.into_iter().map(|t| view! {
                <TokenRow
                    service_user_id=service_user_id.clone()
                    handle=handle.clone()
                    token=t
                    error=error
                    revealed=revealed
                    on_changed=on_changed
                />
            }).collect_view()}
        </ListCard>
    }
}

/// One token with its Revoke button, confirm dialog, and toast.
#[component]
fn TokenRow(
    service_user_id: String,
    handle: String,
    token: ServiceUserTokenView,
    error: RwSignal<Option<String>>,
    revealed: RwSignal<Option<(String, MintedTokenView)>>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let rotate_open = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let toaster = use_context::<ToasterInjection>();
    let ServiceUserTokenView {
        public_id,
        label,
        created_on,
        expires_on,
        last_used_on,
    } = token;
    let dates = format!("{public_id} · Created {created_on} · Expires {expires_on}");
    let used = last_used_on.map_or_else(|| "Never used".to_string(), |d| format!("Last used {d}"));
    let title = format!("Revoke “{label}”?");
    let rotate_title = format!("Rotate “{label}”?");
    let done_text = format!("Token “{label}” revoked");
    // `spawn_with_fresh_code` reports the raw server message; the card shows a sentence.
    let rotate_error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if let Some(raw) = rotate_error.get() {
            error.set(Some(revoke_error(&raw).to_string()));
        }
    });
    let rotate = {
        let (service_user_id, public_id) = (service_user_id.clone(), public_id.clone());
        move |_| {
            rotate_open.set(false);
            error.set(None);
            let (id, public_id, handle) =
                (service_user_id.clone(), public_id.clone(), handle.clone());
            // Rotating needs a fresh authenticator code; the dialog collects it.
            rotate_error.set(None);
            spawn_with_fresh_code(
                rotate_error,
                Callback::new(move |token: MintedTokenView| {
                    revealed.set(Some((handle.clone(), token)));
                    on_changed.run(());
                }),
                move |code| {
                    let (id, public_id) = (id.clone(), public_id.clone());
                    async move { rotate_service_user_token(id, public_id, code).await }
                },
            );
        }
    };

    let confirm = {
        let public_id = public_id.clone();
        move |_| {
            open.set(false);
            error.set(None);
            busy.set(true);
            let (id, public_id, done_text) = (
                service_user_id.clone(),
                public_id.clone(),
                done_text.clone(),
            );
            spawn_local_scoped(async move {
                let result = revoke_service_user_token(id, public_id).await;
                busy.set(false);
                match result {
                    Ok(()) => {
                        if let Some(toaster) = toaster {
                            toaster.dispatch(
                                ToastOptions::new(done_text).intent(ToastIntent::Success.into()),
                            );
                        }
                        on_changed.run(());
                    }
                    Err(e) => error.set(Some(revoke_error(&e.to_string()).to_string())),
                }
            });
        }
    };

    view! {
        <div data-testid="service-user-detail-token-row" data-token-id=public_id>
            <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center gap=FlexGap::Small wrap=FlexWrap::Wrap>
                <Flex vertical=true>
                    <Body1Strong>{label}</Body1Strong>
                    <Caption2>{dates}</Caption2>
                    <Caption2>{used}</Caption2>
                </Flex>
                <Flex gap=FlexGap::Small>
                    <div data-testid="service-user-detail-token-rotate">
                        <Button
                            appearance=ButtonAppearance::Secondary
                            disabled=Signal::derive(move || busy.get())
                            on_click=Callback::new(move |_| rotate_open.set(true))
                        >
                            "Rotate"
                        </Button>
                    </div>
                    <div data-testid="service-user-detail-token-revoke">
                        <Button
                            appearance=ButtonAppearance::Secondary
                            disabled=Signal::derive(move || busy.get())
                            on_click=Callback::new(move |_| open.set(true))
                        >
                            "Revoke"
                        </Button>
                    </div>
                </Flex>
            </Flex>
            <Dialog open=rotate_open>
                <DialogSurface>
                    <DialogBody>
                        <DialogTitle>{rotate_title}</DialogTitle>
                        <DialogContent>
                            <div data-testid="service-user-token-rotate-confirm">
                                <Body1>
                                    "The old token stops working straight away and a new one takes its place, with the same access. You'll confirm with your authenticator code, and see the new token once."
                                </Body1>
                            </div>
                        </DialogContent>
                        <DialogActions>
                            <Button
                                appearance=ButtonAppearance::Secondary
                                on_click=Callback::new(move |_| rotate_open.set(false))
                            >
                                "Cancel"
                            </Button>
                            <div data-testid="service-user-token-rotate-submit">
                                <Button appearance=ButtonAppearance::Primary on_click=Callback::new(rotate)>
                                    "Rotate"
                                </Button>
                            </div>
                        </DialogActions>
                    </DialogBody>
                </DialogSurface>
            </Dialog>
            <Dialog open=open>
                <DialogSurface>
                    <DialogBody>
                        <DialogTitle>{title}</DialogTitle>
                        <DialogContent>
                            <div data-testid="service-user-token-revoke-confirm">
                                <Body1>
                                    "It stops working straight away. Anything using it has to get a new token."
                                </Body1>
                            </div>
                        </DialogContent>
                        <DialogActions>
                            <Button
                                appearance=ButtonAppearance::Secondary
                                on_click=Callback::new(move |_| open.set(false))
                            >
                                "Cancel"
                            </Button>
                            <div data-testid="service-user-token-revoke-submit">
                                <Button appearance=ButtonAppearance::Primary on_click=Callback::new(confirm)>
                                    "Revoke"
                                </Button>
                            </div>
                        </DialogActions>
                    </DialogBody>
                </DialogSurface>
            </Dialog>
        </div>
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
fn GroupsCard(
    service_user_id: String,
    handle: String,
    groups: Vec<ServiceUserGroupView>,
    held_access: Vec<String>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let count = groups.len();
    let error = RwSignal::new(None::<String>);
    view! {
        <ChangeAccess
            service_user_id=service_user_id.clone()
            handle=handle.clone()
            held=held_access
            on_changed=on_changed
        />
        {move || error.get().map(|e| view! {
            <div data-testid="service-user-detail-group-error">
                <MessageBar intent=MessageBarIntent::Error>{
                    if e.contains("STEP_UP:") {
                        "Nothing was changed. Try again and confirm with your authenticator code.".to_string()
                    } else {
                        friendly_error(&e).to_string()
                    }
                }</MessageBar>
            </div>
        })}
        <ListCard
            title="Groups"
            count=count
            description="The service user holds what these groups hold."
            empty="It isn't in any groups."
            test_id="service-user-detail-groups"
        >
            {groups.into_iter().map(|g| view! {
                <GroupRow
                    service_user_id=service_user_id.clone()
                    handle=handle.clone()
                    group=g
                    error=error
                    on_changed=on_changed
                />
            }).collect_view()}
        </ListCard>
    }
}

/// One group with its Remove button and confirm dialog.
#[component]
fn GroupRow(
    service_user_id: String,
    handle: String,
    group: ServiceUserGroupView,
    error: RwSignal<Option<String>>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let toaster = use_context::<ToasterInjection>();
    let ServiceUserGroupView {
        id: group_id,
        name,
        description,
    } = group;
    let title = format!("Remove @{handle} from {name}?");
    let body = format!(
        "@{handle} stops holding what {name} holds. Tokens that relied on it lose that access straight away."
    );
    let done_text = format!("@{handle} removed from {name}");

    let confirm = {
        let group_id = group_id.clone();
        move |_| {
            open.set(false);
            error.set(None);
            let (id, group_id, done_text) =
                (service_user_id.clone(), group_id.clone(), done_text.clone());
            spawn_with_step_up(
                error,
                Callback::new(move |()| {
                    if let Some(toaster) = toaster {
                        toaster.dispatch(
                            ToastOptions::new(done_text.clone())
                                .intent(ToastIntent::Success.into()),
                        );
                    }
                    on_changed.run(());
                }),
                move || remove_service_user_from_group(id.clone(), group_id.clone()),
            );
        }
    };

    view! {
        <div data-testid="service-user-detail-group-row" data-group-id=group_id>
            <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center gap=FlexGap::Small wrap=FlexWrap::Wrap>
                <Flex vertical=true>
                    <Body1Strong>{name}</Body1Strong>
                    {(!description.is_empty()).then(|| view! { <Caption2>{description}</Caption2> })}
                </Flex>
                <div data-testid="service-user-detail-group-remove">
                    <Button
                        appearance=ButtonAppearance::Secondary
                        on_click=Callback::new(move |_| open.set(true))
                    >
                        "Remove"
                    </Button>
                </div>
            </Flex>
            <Dialog open=open>
                <DialogSurface>
                    <DialogBody>
                        <DialogTitle>{title}</DialogTitle>
                        <DialogContent>
                            <div data-testid="service-user-group-remove-confirm">
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
                            <div data-testid="service-user-group-remove-submit">
                                <Button appearance=ButtonAppearance::Primary on_click=Callback::new(confirm)>
                                    "Remove"
                                </Button>
                            </div>
                        </DialogActions>
                    </DialogBody>
                </DialogSurface>
            </Dialog>
        </div>
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
    fn handle_matches_allows_at_and_spaces_only() {
        assert!(handle_matches("spectra-agent", "spectra-agent"));
        assert!(handle_matches(" @spectra-agent ", "spectra-agent"));
        assert!(!handle_matches("", "spectra-agent"));
        assert!(!handle_matches("spectra", "spectra-agent"));
        assert!(!handle_matches("Spectra-Agent", "spectra-agent"));
        assert!(!handle_matches("@@spectra-agent", "spectra-agent"));
    }

    #[test]
    fn delete_error_explains_a_mismatch() {
        assert!(
            delete_error("reason_class=confirm: typed handle does not match")
                .contains("handle exactly")
        );
        assert!(delete_error("reason_class=not_found").contains("isn't yours"));
        assert!(delete_error("disk on fire").starts_with("Something went wrong"));
    }

    #[test]
    fn revoke_error_explains_a_missing_token() {
        assert!(revoke_error("reason_class=token_not_found").contains("already gone"));
        assert!(revoke_error("reason_class=forbidden").contains("can't change"));
        assert!(revoke_error("reason_class=not_configured").contains("aren't available"));
    }

    #[test]
    fn change_error_falls_back_to_friendly_error() {
        assert!(change_error("reason_class=not_found", true).contains("isn't yours"));
        assert!(change_error("reason_class=forbidden", false).contains("can't change"));
        assert!(change_error("disk on fire", true).starts_with("Something went wrong"));
    }
}
