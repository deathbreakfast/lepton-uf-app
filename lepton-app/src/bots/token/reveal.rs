//! The one-time token reveal, with copy buttons and setup commands.
//!
//! The secret lives only in a signal owned by the page that minted it. "Done"
//! returns to the Bots page, which drops that page and the signal with it.
//! Nothing stores the secret, so a reload or the back button never shows it
//! again.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use uf_product::components::{Body1, Body1Strong, Subtitle2};
use uf_product::primitives::*;

use crate::bots::types::MintedTokenView;
use crate::clipboard::copy_text;
use crate::paths;

/// How long a copy button says "Copied".
const COPIED_MS: u64 = 2000;

/// A copy button that says "Copied" for a moment after a click.
#[component]
fn CopyButton(text: String, label: &'static str, test_id: &'static str) -> impl IntoView {
    let copied = RwSignal::new(false);
    view! {
        <div data-testid=test_id aria-live="polite">
            <Button
                appearance=ButtonAppearance::Secondary
                icon=icondata::AiCopyOutlined
                on_click=Callback::new(move |_| {
                    copy_text(&text);
                    copied.set(true);
                    set_timeout(
                        move || {
                            // The reveal may be gone by now (Done was pressed).
                            let _ = copied.try_set(false);
                        },
                        std::time::Duration::from_millis(COPIED_MS),
                    );
                })
            >
                {move || if copied.get() { "Copied" } else { label }}
            </Button>
        </div>
    }
}

/// Show a freshly minted token once.
#[component]
pub fn TokenReveal(handle: String, token: MintedTokenView) -> impl IntoView {
    let navigate = use_navigate();
    let secret = token.secret.expose().to_string();
    let summary = format!(
        "@{handle} can {}. This token expires on {}.",
        token.can_do, token.expires_on
    );

    // Put focus on the copy button so keyboard users land on the next action.
    #[cfg(feature = "hydrate")]
    request_animation_frame(|| {
        use wasm_bindgen::JsCast;
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| {
                d.query_selector("[data-testid=bot-token-copy] button")
                    .ok()
                    .flatten()
            })
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    });

    let setup = token
        .setup
        .into_iter()
        .map(|step| {
            view! {
                <Flex vertical=true gap=FlexGap::Small>
                    <Body1>{step.title}</Body1>
                    <div data-testid="bot-token-setup-code"><Code text=step.code.clone() /></div>
                    <Flex justify=FlexJustify::End>
                        <CopyButton text=step.code label="Copy commands" test_id="bot-token-setup-copy" />
                    </Flex>
                </Flex>
            }
        })
        .collect_view();

    view! {
        <div data-testid="bot-token-reveal">
            <Card elevation=MaterialElevation::Raised>
                <CardHeader><Subtitle2>{format!("Token for @{handle}")}</Subtitle2></CardHeader>
                <CardContent>
                    <Flex vertical=true gap=FlexGap::Medium>
                        <MessageBar intent=MessageBarIntent::Warning>
                            "Copy this token now. You won't see it again."
                        </MessageBar>
                        <Flex vertical=true gap=FlexGap::Small>
                            <div data-testid="bot-token-secret"><Code text=secret.clone() /></div>
                            <Flex justify=FlexJustify::End>
                                <CopyButton text=secret label="Copy token" test_id="bot-token-copy" />
                            </Flex>
                        </Flex>
                        <Divider />
                        <Subtitle2>"Add it to Claude Code"</Subtitle2>
                        {setup}
                        <div data-testid="bot-token-summary"><Body1Strong>{summary}</Body1Strong></div>
                    </Flex>
                </CardContent>
                <CardFooter>
                    <div data-testid="bot-token-done">
                        <Button
                            appearance=ButtonAppearance::Primary
                            on_click=Callback::new(move |_| navigate(paths::BOTS, Default::default()))
                        >
                            "Done"
                        </Button>
                    </div>
                </CardFooter>
            </Card>
        </div>
    }
}
