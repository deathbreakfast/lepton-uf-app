use crate::profile_photo_display::ProfilePhotoDisplay;
use crate::profile_photo_upload::ProfilePhotoUpload;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use serde::{Deserialize, Serialize};
use uf_product::components::{Card, CardContent, ContentContainer, Subtitle2, Title3};
use uf_product::primitives::*;
use uf_product::{use_auth_context, use_auth_state};

/// Payload for [`get_my_profile`]: display name, Valence profile id, and optional photo file id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    /// User-facing display name (editable via [`update_my_profile`]).
    pub display_name: String,
    /// Valence `UserProfile` record id (string form).
    pub profile_id: String,
    /// Meson File id when an active photo exists (fed to `<MesonImg>`).
    pub photo_file_id: Option<String>,
}

/// Max display name length (aligned with lepton-identity `UserProfile` MaxLength(255)).
pub const MAX_DISPLAY_NAME_CHARS: usize = 255;

/// Validate a display name before Valence update (empty / over-long).
///
/// Returns the trimmed name on success.
///
/// # Errors
///
/// Returns `Err` with a stable message when:
///
/// - the trimmed name is empty (`"Display name cannot be empty"`);
/// - the trimmed name exceeds [`MAX_DISPLAY_NAME_CHARS`] (`"Display name is too long"`).
pub fn validate_display_name(display_name: &str) -> Result<&str, &'static str> {
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        return Err("Display name cannot be empty");
    }
    if trimmed.chars().count() > MAX_DISPLAY_NAME_CHARS {
        return Err("Display name is too long");
    }
    Ok(trimmed)
}

/// Browser-facing profile server-fn error (lepton `reason_class=` style).
///
/// Drops Valence/backend detail so MessageBars never echo internal errors.
#[cfg(any(feature = "ssr", test))]
fn profile_server_err(reason_class: &'static str, message: &'static str) -> ServerFnError {
    ServerFnError::new(format!("reason_class={reason_class}: {message}"))
}

/// Load (or lazily create) the signed-in user's [`ProfileData`].
///
/// # Errors
///
/// Returns [`ServerFnError`] when auth is missing, Valence query/create fails,
/// profile construction fails, or the persisted row has no id. Messages use
/// opaque `reason_class=` strings suitable for UI MessageBars.
#[server(GetMyProfile)]
pub async fn get_my_profile() -> Result<ProfileData, ServerFnError> {
    use chrono::Utc;
    use lepton_auth::{require_auth_user, user_valence};
    use lepton_identity::generated::UserProfile;
    use valence::{Model, RecordPredicate};

    let (ctx, user) = require_auth_user().await?;
    let user_thing = user.id.clone();
    let user_email = user.email.clone();
    let v = user_valence(&ctx)?;

    let existing = UserProfile::query(&v, valence::use_!(r"On your **account profile** page we **load your display name and profile photo reference** so you can view and edit them. If you have never opened this page, we **create a profile** from your account email first. The photo file itself is fetched through the files API when a photo is set."))
        .where_user(RecordPredicate::Equals(user_thing.clone()))
        .first()
        .await
        .map_err(|_| profile_server_err("profile_query", "failed to load profile"))?;

    let profile = match existing {
        Some(p) => p,
        None => {
            let new_profile = UserProfile::new(
                user_thing,
                user_email.clone(),
                user_email,
                Utc::now(),
                Utc::now(),
                None,
            )
            .map_err(|_| profile_server_err("profile_build", "failed to build profile"))?;

            UserProfile::create(new_profile, &v, valence::use_!(r"On your **account profile** page, if you have never opened it before, we **create a profile** from your account email so you can set a **display name** and photo. You use this on the profile page."))
                .await
                .map_err(|_| profile_server_err("profile_create", "failed to create profile"))?
        }
    };

    let profile_id = profile
        .id()
        .map(|t| t.id().to_string())
        .ok_or_else(|| profile_server_err("profile_id", "profile id missing"))?;

    let photo_file_id = profile.active_photo().map(|rid| rid.id().to_string());

    Ok(ProfileData {
        display_name: profile.display_name().to_string(),
        profile_id,
        photo_file_id,
    })
}

/// Persist a new display name for the signed-in user's profile.
///
/// # Errors
///
/// - [`ServerFnError::Args`] when `display_name` is empty/whitespace.
/// - [`ServerFnError`] when auth is missing, the profile row is absent, validation
///   fails, or the Valence commit fails. Non-args failures use opaque
///   `reason_class=` messages (no Valence detail).
#[server(UpdateMyProfile)]
pub async fn update_my_profile(display_name: String) -> Result<(), ServerFnError> {
    use lepton_auth::{require_auth_user, user_valence};
    use lepton_identity::generated::UserProfile;
    use valence::RecordPredicate;

    let display_name = match validate_display_name(&display_name) {
        Ok(name) => name.to_string(),
        Err(msg) => return Err(ServerFnError::Args(msg.into())),
    };

    let (ctx, user) = require_auth_user().await?;
    let user_thing = user.id.clone();
    let v = user_valence(&ctx)?;

    let profile = UserProfile::query(&v, valence::use_!(r"On your **account profile** page we **load your display name and profile photo reference** so you can view and edit them. If you have never opened this page, we **create a profile** from your account email first. The photo file itself is fetched through the files API when a photo is set."))
        .where_user(RecordPredicate::Equals(user_thing))
        .first()
        .await
        .map_err(|_| profile_server_err("profile_query", "failed to load profile"))?
        .ok_or_else(|| profile_server_err("profile_missing", "profile not found"))?;

    profile
        .get_mutable(&v, valence::use_!(r"When you save on **account profile**, we **update your display name** on your profile record so the page shows what you just entered. Only you use this change on your profile; it is not published as a directory listing to other people from this save alone."))
        .set_display_name(display_name)
        .map_err(|_| profile_server_err("profile_validate", "display name rejected"))?
        .commit()
        .await
        .map_err(|_| profile_server_err("profile_update", "failed to update profile"))?;

    Ok(())
}

/// `/user/profile` page: display name and profile photo editing.
#[component]
pub fn ProfilePage() -> impl IntoView {
    let auth_state = use_auth_state();
    let navigate = use_navigate();
    let auth = use_auth_context();

    Effect::new(move |_| {
        if !auth_state.with(|s| s.is_authenticated()) {
            navigate(lepton_auth::paths::SIGNIN, Default::default());
        }
    });

    let profile_resource = Resource::new(|| (), |_| get_my_profile());

    let update_action = ServerAction::<UpdateMyProfile>::new();

    Effect::new(move |_| {
        if update_action.value().get() == Some(Ok(())) {
            profile_resource.refetch();
            auth.trigger_refresh();
        }
    });

    let on_photo_uploaded = Callback::new(move |()| {
        profile_resource.refetch();
    });

    view! {
        <ContentContainer max_width="600px" data_testid="profile-container">
            <Flex vertical=true gap=FlexGap::Large>
                <Title3>"Edit Profile"</Title3>
                {move || match update_action.value().get() {
                    Some(Ok(())) => view! {
                        <div data-testid="profile-success">
                            <MessageBar intent=MessageBarIntent::Success>
                                "Profile updated successfully."
                            </MessageBar>
                        </div>
                    }.into_any(),
                    Some(Err(e)) => view! {
                        <div data-testid="profile-error">
                            <MessageBar intent=MessageBarIntent::Error>
                                {e.to_string()}
                            </MessageBar>
                        </div>
                    }.into_any(),
                    None => ().into_any(),
                }}
                <Suspense fallback=move || view! { <Spinner /> }>
                    {move || {
                        profile_resource.get().map(|result| {
                            match result {
                                Ok(profile) => {
                                    let display_name = profile.display_name.clone();
                                    let profile_id = profile.profile_id.clone();
                                    let photo_file_id = profile.photo_file_id.clone();
                                    let display_name_signal = Signal::derive({
                                        let dn = display_name.clone();
                                        move || dn.clone()
                                    });
                                    let photo_file_id_prop = MaybeProp::from(photo_file_id);
                                    let profile_id_signal = Signal::derive({
                                        let pid = profile_id.clone();
                                        move || pid.clone()
                                    });

                                    view! {
                                        <Flex vertical=true gap=FlexGap::Large>
                                            <Card>
                                                <CardContent>
                                                <div data-testid="profile-photo-section">
                                                <Flex
                                                    vertical=true
                                                    align=FlexAlign::Center
                                                    gap=FlexGap::Medium
                                                >
                                                    <Subtitle2>"Profile photo"</Subtitle2>
                                                    <ProfilePhotoDisplay
                                                        photo_file_id=photo_file_id_prop
                                                        display_name=display_name_signal
                                                    />
                                                    <ProfilePhotoUpload
                                                        profile_id=profile_id_signal
                                                        on_upload=on_photo_uploaded
                                                    />
                                                </Flex>
                                                </div>
                                                </CardContent>
                                            </Card>

                                            <ActionForm action=update_action>
                                                <Card>
                                                    <CardContent>
                                                    <Flex vertical=true gap=FlexGap::Medium>
                                                        <Subtitle2>"Display name"</Subtitle2>
                                                        <div data-testid="profile-display-name">
                                                            <Field label="Display Name" required=true>
                                                                <Input
                                                                    bind={
                                                                        let mut bind = InputBind::new(display_name);
                                                                        bind.name = "display_name".into();
                                                                        bind
                                                                    }
                                                                    appearance=InputAppearance::with_placeholder("Enter your display name")
                                                                />
                                                            </Field>
                                                        </div>
                                                        <div data-testid="profile-submit">
                                                            <Flex gap=FlexGap::Small>
                                                                <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>
                                                                    "Save"
                                                                </Button>
                                                            </Flex>
                                                        </div>
                                                    </Flex>
                                                    </CardContent>
                                                </Card>
                                            </ActionForm>
                                        </Flex>
                                    }.into_any()
                                }
                                Err(e) => {
                                    view! {
                                        <MessageBar intent=MessageBarIntent::Error>
                                            {format!("Failed to load profile: {e}")}
                                        </MessageBar>
                                    }.into_any()
                                }
                            }
                        })
                    }}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

#[cfg(test)]
mod tests {
    use super::{profile_server_err, validate_display_name, MAX_DISPLAY_NAME_CHARS};

    #[test]
    fn validate_display_name_accepts_trimmed_happy() {
        assert_eq!(validate_display_name("  Ada  "), Ok("Ada"));
    }

    #[test]
    fn validate_display_name_rejects_empty_sad() {
        assert_eq!(
            validate_display_name("   "),
            Err("Display name cannot be empty")
        );
    }

    #[test]
    fn validate_display_name_rejects_overlong_sad() {
        let long: String = "a".repeat(MAX_DISPLAY_NAME_CHARS + 1);
        assert_eq!(
            validate_display_name(&long),
            Err("Display name is too long")
        );
    }

    #[test]
    fn validate_display_name_accepts_exact_max_happy() {
        let exact: String = "a".repeat(MAX_DISPLAY_NAME_CHARS);
        assert_eq!(validate_display_name(&exact), Ok(exact.as_str()));
    }

    #[test]
    fn profile_server_err_is_opaque_reason_class_happy() {
        let err = profile_server_err("profile_query", "failed to load profile");
        let msg = err.to_string();
        assert!(msg.contains("reason_class=profile_query"));
        assert!(msg.contains("failed to load profile"));
        assert!(
            !msg.contains("valence"),
            "must not embed backend crate names"
        );
    }
}
