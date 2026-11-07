//! Sign-in providers card: a table of Google and GitHub with their client ids
//! and whether each client secret is set.

use std::collections::HashMap;
use std::sync::Arc;

use lepton_auth::delivery_settings::{OAuthCandidate, OAuthSettingsView, SecretStatus};
use leptos::prelude::*;
use orbital_data::{DataRecord, DataValue};
use orbital_datatable::{
    DataTable, DataTableColumnDef, DataTableEvents, DataTableFeatures, DataTableRowModel,
    DataTableSource, ListViewConfig, PagingMode,
};
use uf_product::components::{Caption1, Subtitle2};
use uf_product::primitives::*;
use uf_product::AdaptiveSheet;

use super::changes::secret_status_text;
use super::email::typed;
use super::Outcome;

/// One row of the providers table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Provider {
    Google,
    GitHub,
}

impl Provider {
    fn id(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::GitHub => "github",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::GitHub => "GitHub",
        }
    }

    fn from_id(id: &str) -> Self {
        if id == "github" {
            Self::GitHub
        } else {
            Self::Google
        }
    }
}

/// The providers section's form values. The page owns them so one Save covers every card.
#[derive(Clone, Copy)]
pub struct OAuthForm {
    google_id: RwSignal<String>,
    github_id: RwSignal<String>,
    google_secret: RwSignal<String>,
    github_secret: RwSignal<String>,
}

impl OAuthForm {
    /// Form filled from loaded values.
    pub fn new(base: &OAuthCandidate) -> Self {
        Self {
            google_id: RwSignal::new(base.google_client_id.clone()),
            github_id: RwSignal::new(base.github_client_id.clone()),
            google_secret: RwSignal::new(String::new()),
            github_secret: RwSignal::new(String::new()),
        }
    }

    /// What the form holds now.
    pub fn candidate(&self) -> OAuthCandidate {
        OAuthCandidate {
            google_client_id: self.google_id.get_untracked(),
            github_client_id: self.github_id.get_untracked(),
            google_client_secret: typed(self.google_secret),
            github_client_secret: typed(self.github_secret),
        }
    }

    fn signals(&self, p: Provider) -> (RwSignal<String>, RwSignal<String>) {
        match p {
            Provider::Google => (self.google_id, self.google_secret),
            Provider::GitHub => (self.github_id, self.github_secret),
        }
    }
}

/// Table record for one provider. Holds only public ids and set/not set text.
fn record(p: Provider, client_id: &str, status: &SecretStatus, pending: bool) -> DataRecord {
    let shown_id = if client_id.trim().is_empty() {
        "Not set up".to_string()
    } else {
        client_id.to_string()
    };
    let action = if status.is_set || !client_id.trim().is_empty() {
        "Replace"
    } else {
        "Set up"
    };
    DataRecord::new(
        p.id(),
        HashMap::from([
            ("provider".into(), DataValue::Text(p.name().into())),
            ("client_id".into(), DataValue::Text(shown_id)),
            (
                "secret".into(),
                DataValue::Text(secret_status_text(status, pending)),
            ),
            ("action".into(), DataValue::Text(action.into())),
        ]),
    )
}

/// OAuth provider settings card.
#[component]
pub fn OAuthCard(
    /// Current settings, for the secret status column.
    view: OAuthSettingsView,
    /// Form values.
    form: OAuthForm,
    /// Result line; owned by the page so it survives the reload after a save.
    outcome: RwSignal<Option<Outcome>>,
) -> impl IntoView {
    let statuses = StoredValue::new(view);
    let narrow = use_breakpoint_down(Breakpoint::Md);
    let build_rows = move || {
        statuses.with_value(|v| {
            [(Provider::Google, &v.google), (Provider::GitHub, &v.github)]
                .into_iter()
                .map(|(p, row)| {
                    let (id, secret) = form.signals(p);
                    let pending = !secret.with(|s| s.trim().is_empty());
                    DataTableRowModel::new(id.with(|id| record(p, id, &row.secret, pending)))
                })
                .collect::<Vec<_>>()
        })
    };
    let rows = RwSignal::new(build_rows());
    Effect::new(move |_| rows.set(build_rows()));

    let open = RwSignal::new(false);
    let editing = RwSignal::new(Provider::Google);
    let draft_id = RwSignal::new(String::new());
    let draft_secret = RwSignal::new(String::new());
    let open_for = Callback::new(move |id: String| {
        let p = Provider::from_id(&id);
        editing.set(p);
        draft_id.set(form.signals(p).0.get_untracked());
        draft_secret.set(String::new());
        open.set(true);
    });
    let apply = move |_: leptos::ev::MouseEvent| {
        let (id, secret) = form.signals(editing.get_untracked());
        id.set(draft_id.get_untracked().trim().to_string());
        let typed = draft_secret.get_untracked();
        if !typed.trim().is_empty() {
            secret.set(typed);
        }
        draft_secret.set(String::new());
        open.set(false);
    };
    let cancel = move |_: leptos::ev::MouseEvent| {
        draft_secret.set(String::new());
        open.set(false);
    };

    let columns = move || {
        let action_view = Arc::new(move |record: DataRecord| {
            let label = record
                .get("action")
                .map(DataValue::display_string)
                .unwrap_or_default();
            let id = record.id.clone();
            let test_id = format!("delivery-oauth-{id}-edit");
            view! {
                <div data-testid=test_id>
                    <Button
                        appearance=ButtonAppearance::Secondary
                        on_click=Callback::new(move |_| open_for.run(id.clone()))
                    >
                        {label}
                    </Button>
                </div>
            }
            .into_any()
        });
        vec![
            DataTableColumnDef::new("provider", "Provider"),
            DataTableColumnDef::new("client_id", "Client ID"),
            DataTableColumnDef::new("secret", "Secret"),
            DataTableColumnDef::new("action", "").with_cell_view(action_view),
        ]
    };

    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Sign-in providers (OAuth)"</Subtitle2>
            </CardHeader>
            <CardContent>
                <div data-testid="delivery-oauth-card">
                    <Flex vertical=true gap=FlexGap::Medium>
                        {move || {
                            let features = if narrow.get() {
                                DataTableFeatures::LIST_VIEW
                            } else {
                                DataTableFeatures::empty()
                            };
                            view! {
                                <DataTable
                                    data_source=DataTableSource::Client(rows)
                                    paging=PagingMode::None
                                    features=features
                                    list_view=ListViewConfig::new("provider")
                                        .with_secondary_fields(vec!["client_id".into(), "secret".into()])
                                    columns=columns()
                                    sortable=false
                                    events=DataTableEvents {
                                        on_row_click: Some(Callback::new(move |(id,): (String,)| open_for.run(id))),
                                        ..Default::default()
                                    }
                                />
                            }
                        }}
                        <Show when=move || narrow.get()>
                            <Caption1>"Tap a provider to set its client ID and secret."</Caption1>
                        </Show>
                        {move || outcome.get().map(|o| o.into_message_bar("delivery-oauth-outcome"))}
                    </Flex>
                </div>
            </CardContent>
        </Card>
        <AdaptiveSheet
            open=open
            title=move || format!("{} sign-in", editing.get().name())
            testid="delivery-oauth-dialog"
            size=DrawerSize::Small
            actions=move || view! {
                <Button appearance=ButtonAppearance::Secondary on_click=Callback::new(cancel)>
                    "Cancel"
                </Button>
                <div data-testid="delivery-oauth-dialog-apply">
                    <Button appearance=ButtonAppearance::Primary on_click=Callback::new(apply)>
                        "Set"
                    </Button>
                </div>
            }
        >
            <Flex vertical=true gap=FlexGap::Medium>
                <Field label="Client ID">
                    <div data-testid="delivery-oauth-client-id">
                        <Input bind=draft_id />
                    </div>
                </Field>
                <Field label="Client secret">
                    <div data-testid="delivery-oauth-client-secret">
                        <Input
                            bind=draft_secret
                            appearance=InputAppearance {
                                input_type: Signal::from(InputType::Password),
                                placeholder: "Leave empty to keep the current secret".into(),
                                ..Default::default()
                            }
                        />
                    </div>
                </Field>
                <Caption1>"Stored in Neutrino. The value is never shown again. Clear the client ID to turn the provider off."</Caption1>
            </Flex>
        </AdaptiveSheet>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_hold_no_secret_value_and_name_the_next_step() {
        let r = record(Provider::GitHub, "", &SecretStatus::default(), false);
        assert_eq!(
            r.get("action").map(DataValue::display_string).as_deref(),
            Some("Set up")
        );
        assert_eq!(
            r.get("secret").map(DataValue::display_string).as_deref(),
            Some("Not set")
        );
        let r = record(
            Provider::Google,
            "1234-abcd.apps",
            &SecretStatus::default(),
            true,
        );
        assert_eq!(
            r.get("secret").map(DataValue::display_string).as_deref(),
            Some("New value ready to save")
        );
    }
}
