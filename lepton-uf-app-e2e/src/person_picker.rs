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

/// Sixty permission groups, standing in for a caller's own source.
const LAB_GROUPS: usize = 60;
/// How many groups the stand-in source returns per query.
const GROUPS_PER_QUERY: usize = 50;

/// The stand-in source: the first fifty groups whose name holds `query`, as a caller
/// would load them for each typed query.
fn groups_matching(query: &str) -> Vec<PickerGroup> {
    let needle = query.trim().to_lowercase();
    let options: Vec<PickerOption> = (1..=LAB_GROUPS)
        .map(|n| format!("Group {n:02}"))
        .filter(|name| name.to_lowercase().contains(&needle))
        .take(GROUPS_PER_QUERY)
        .map(|name| {
            // A caller row shows its chip, not its label, so the chip carries the name.
            let shown = name.clone();
            PickerOption {
                id: name.to_lowercase().replace(' ', "-"),
                label: name,
                secondary: None,
                chip: ViewFn::from(move || view! { <span>{shown.clone()}</span> }),
            }
        })
        .collect();
    if options.is_empty() {
        return Vec::new();
    }
    vec![PickerGroup {
        label: "Permission groups".to_string(),
        options,
    }]
}

/// Six pickers: single, multi (with exclusions), teams, service users, and people and
/// teams with source captions and kind badges, and groups loaded for the typed query.
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
    let groups = RwSignal::new(Vec::new());
    let groups_query = RwSignal::new(String::new());

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
                    <DemoSection title="Groups searched as you type" testid="pp-groups">
                        <PersonPicker
                            value=groups
                            people=false
                            query=groups_query
                            extra_groups=Signal::derive(move || groups_matching(&groups_query.get()))
                            label="Permission group"
                        />
                        <PickedReadout value=groups testid="pp-groups-value" />
                    </DemoSection>
                </Flex>
            </ContentContainer>
        </UnifiedFieldShellLayout>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(groups: &[PickerGroup]) -> Vec<&str> {
        groups
            .iter()
            .flat_map(|g| &g.options)
            .map(|o| o.label.as_str())
            .collect()
    }

    #[test]
    fn typed_query_reaches_groups_past_the_first_fifty_happy() {
        assert_eq!(labels(&groups_matching("")).len(), GROUPS_PER_QUERY);
        assert_eq!(labels(&groups_matching("group 55")), vec!["Group 55"]);
    }

    #[test]
    fn unmatched_query_returns_no_group_sad() {
        assert!(groups_matching("zzz").is_empty());
    }
}
