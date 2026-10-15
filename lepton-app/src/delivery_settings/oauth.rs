//! Sign-in providers card: Google and GitHub client ids and secrets.

use lepton_auth::delivery_settings::{OAuthCandidate, OAuthSettingsView, WriteOnlySecret};
use leptos::prelude::*;
use uf_product::components::Subtitle2;
use uf_product::primitives::*;

use super::secret_field::SecretField;
use super::server::save_oauth_settings;
use super::step_up::spawn_with_fresh_code;
use super::{friendly_error, Outcome};

/// OAuth provider settings card.
#[component]
pub fn OAuthCard(
    /// Current settings.
    view: OAuthSettingsView,
    /// Bumped after a successful save so the page reloads.
    reload: RwSignal<u32>,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
) -> impl IntoView {
    let google_id = RwSignal::new(view.google.client_id.clone());
    let github_id = RwSignal::new(view.github.client_id.clone());
    let google_secret = RwSignal::new(String::new());
    let github_secret = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        if let Some(message) = error.get() {
            outcome.set(Some(Outcome::Failed(friendly_error(&message))));
        }
    });

    let candidate = move || {
        let typed = |s: RwSignal<String>| {
            let v = s.get_untracked();
            (!v.trim().is_empty()).then(|| WriteOnlySecret::new(v))
        };
        OAuthCandidate {
            google_client_id: google_id.get_untracked(),
            github_client_id: github_id.get_untracked(),
            google_client_secret: typed(google_secret),
            github_client_secret: typed(github_secret),
        }
    };

    let save = move |_: leptos::ev::MouseEvent| {
        outcome.set(None);
        let c = candidate();
        spawn_with_fresh_code(
            error,
            Callback::new(move |()| {
                google_secret.set(String::new());
                github_secret.set(String::new());
                outcome.set(Some(Outcome::Done(
                    "Sign-in provider settings saved.".to_string(),
                )));
                reload.update(|n| *n += 1);
            }),
            move |code| {
                let c = c.clone();
                async move { save_oauth_settings(c, code).await }
            },
        );
    };

    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Sign-in providers"</Subtitle2>
            </CardHeader>
            <CardContent>
                <div data-testid="delivery-oauth-card">
                    <Flex vertical=true gap=FlexGap::Medium>
                        <Field label="Google client ID">
                            <div data-testid="delivery-oauth-google-id">
                                <Input bind=google_id />
                            </div>
                        </Field>
                        <SecretField label="Google client secret" status=view.google.secret.clone() value=google_secret test_id="delivery-oauth-google-secret" />
                        <Field label="GitHub client ID">
                            <div data-testid="delivery-oauth-github-id">
                                <Input bind=github_id />
                            </div>
                        </Field>
                        <SecretField label="GitHub client secret" status=view.github.secret.clone() value=github_secret test_id="delivery-oauth-github-secret" />
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-oauth-outcome"))}
                    </Flex>
                </div>
            </CardContent>
            <CardFooter>
                <div data-testid="delivery-oauth-save">
                    <Button appearance=ButtonAppearance::Primary on_click=Callback::new(save)>"Save sign-in providers"</Button>
                </div>
            </CardFooter>
        </Card>
    }
}
