//! `/dev/person-picker`: every `PersonPicker` mode on one page, for Playwright.
//!
//! Lab-only demo. `?exclude=<user id>,…` marks those people excluded in the multi picker.

use lepton_shell::people::{
    PersonPicker, PickedPrincipal, PickerGroup, PickerOption, PrincipalKind,
};
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use uf_integrations::{
    HostAuthMenu, ShellAppBar, ShellAuthMenu, UnifiedFieldAppBar, UnifiedFieldShellLayout,
};
use uf_product::components::{Caption1, ContentContainer};
use uf_product::primitives::{
    Badge, BadgeAppearance, BadgeColor, BadgeSize, Body1, Flex, FlexAlign, FlexGap, Title3,
};

/// The current picks as text, for assertions.
#[component]
fn PickedReadout(value: RwSignal<Vec<PickedPrincipal>>, testid: &'static str) -> impl IntoView {
    view! {
        <span data-testid=testid>
            <Caption1>
                {move || {
                    let picks = value.get();
                    if picks.is_empty() {
                        "Nothing picked".to_string()
                    } else {
                        picks
                            .iter()
                            .map(|p| format!("{} ({})", p.label, p.kind.key()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                }}
            </Caption1>
        </span>
    }
}

/// One labelled demo instance.
#[component]
fn DemoSection(title: &'static str, testid: &'static str, children: Children) -> impl IntoView {
    view! {
        <section data-testid=testid style="width: 100%;">
            <Flex vertical=true gap=FlexGap::Small full_width=true>
                <Body1>{title}</Body1>
                {children()}
            </Flex>
        </section>
    }
}

fn event_fields() -> Vec<PickerGroup> {
    vec![PickerGroup {
        label: "From this event".to_string(),
        options: vec![PickerOption {
            id: "author".to_string(),
            label: "Author".to_string(),
            secondary: Some("trigger.task.author".to_string()),
            chip: ViewFn::from(|| {
                view! {
                    <Badge
                        appearance=BadgeAppearance::Tint
                        color=BadgeColor::Brand
                        size=BadgeSize::Small
                    >
                        "Task › author"
                    </Badge>
                }
            }),
        }],
    }]
}

/// Five pickers: single, multi (with exclusions), teams, service users, and people and
/// teams with source captions and kind badges.
#[component]
pub fn PersonPickerDemoPage() -> impl IntoView {
    let query = use_query_map();
    let exclude = Signal::derive(move || {
        query
            .get()
            .get("exclude")
            .unwrap_or_default()
            .split(',')
            .filter(|id| !id.is_empty())
            .map(|id| PickedPrincipal {
                kind: PrincipalKind::User,
                id: id.to_string(),
                label: String::new(),
                secondary: None,
                image_url: None,
            })
            .collect::<Vec<_>>()
    });

    let single = RwSignal::new(Vec::new());
    let multi = RwSignal::new(Vec::new());
    let teams = RwSignal::new(Vec::new());
    let service_users = RwSignal::new(Vec::new());
    let sources = RwSignal::new(Vec::new());

    view! {
        <UnifiedFieldShellLayout>
            <ShellAppBar slot>
                <UnifiedFieldAppBar app_name="Lepton product e2e".to_string()>
                    <ShellAuthMenu slot:auth_menu>
                        <HostAuthMenu />
                    </ShellAuthMenu>
                </UnifiedFieldAppBar>
            </ShellAppBar>
            <ContentContainer max_width="720px" data_testid="person-picker-demo">
                <Flex vertical=true gap=FlexGap::Large align=FlexAlign::Start full_width=true>
                    <Title3>"Person picker"</Title3>
                    <DemoSection title="Single person" testid="pp-single">
                        <PersonPicker value=single single=true label="Approver" />
                        <PickedReadout value=single testid="pp-single-value" />
                    </DemoSection>
                    <DemoSection title="People, teams and service users" testid="pp-multi">
                        <PersonPicker
                            value=multi
                            teams=true
                            service_users=true
                            exclude=exclude
                            label="Members"
                            placeholder="Add a person, team or service user"
                        />
                        <PickedReadout value=multi testid="pp-multi-value" />
                    </DemoSection>
                    <DemoSection title="Teams only" testid="pp-teams">
                        <PersonPicker value=teams people=false teams=true label="Team" />
                        <PickedReadout value=teams testid="pp-teams-value" />
                    </DemoSection>
                    <DemoSection title="Service users only" testid="pp-service-users">
                        <PersonPicker
                            value=service_users
                            people=false
                            service_users=true
                            label="Service user"
                        />
                        <PickedReadout value=service_users testid="pp-service-users-value" />
                    </DemoSection>
                    <DemoSection title="Owner, with sources and kinds" testid="pp-sources">
                        <PersonPicker
                            value=sources
                            teams=true
                            show_sources=true
                            kind_badges=true
                            extra_groups=Signal::derive(event_fields)
                            label="Owner"
                        />
                        <PickedReadout value=sources testid="pp-sources-value" />
                    </DemoSection>
                </Flex>
            </ContentContainer>
        </UnifiedFieldShellLayout>
    }
}
