//! Text panels: Markdown shown with Orbital's MarkdownView.

use leptos::prelude::*;
use orbital::components::MarkdownView;

/// A text panel's body.
#[component]
pub(in crate::pages::dashboards) fn TextPanel(markdown: String) -> impl IntoView {
    view! {
        <MarkdownView
            markdown=Signal::stored(markdown)
            testid="spectra-dashboard-panel-markdown".to_string()
        />
    }
}
