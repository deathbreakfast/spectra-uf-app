use leptos::prelude::*;
use orbital::primitives::{Button, ButtonAppearance, ButtonSize, Tooltip};

const TOOLTIP: &str =
    "Copy a link to this view. Paste it into a dashboard panel to chart the same query.";

/// Writes the page's address to the clipboard. The explorers keep their whole query in
/// the URL, so the address is the query.
#[cfg(target_arch = "wasm32")]
fn copy_current_url() {
    let window = window();
    if let Ok(href) = window.location().href() {
        // The clipboard promise needs no follow-up; a refused write leaves it unchanged.
        let _ = window.navigator().clipboard().write_text(&href);
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn copy_current_url() {}

/// Small link icon beside an explorer's title that copies the current query link.
#[component]
pub fn CopyQueryLink() -> impl IntoView {
    view! {
        <span data-testid="spectra-copy-query-link">
            <Tooltip content=TOOLTIP>
                <Button
                    appearance=ButtonAppearance::Subtle
                    size=ButtonSize::Small
                    icon=icondata::AiLinkOutlined
                    attr:aria-label="Copy link"
                    on_click=Callback::new(move |_| copy_current_url())
                />
            </Tooltip>
        </span>
    }
}
