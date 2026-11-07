//! Spotlight steps for the callsite graph (`/spectra/graph`).

use leptos::prelude::*;
use uf_help_macros::help_spotlight_step;

use super::help_stack;

/// Time window buttons.
#[help_spotlight_step(
    route = "/spectra/graph",
    feature_highlight = "spectra-callsite-window",
    title = "Pick a window",
    spotlight = "spectra-callsite-window",
    position = "bottom",
    order = 10
)]
#[component]
pub fn SpectraCallsiteWindowHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-callsite-window",
        "Choose how far back to look. Only rows written in that window count.",
        None,
        &[],
    )
}

/// The graph itself.
#[help_spotlight_step(
    route = "/spectra/graph",
    feature_highlight = "spectra-callsite-graph",
    title = "Read the graph",
    spotlight = "spectra-callsite-graph",
    position = "top",
    order = 20
)]
#[component]
pub fn SpectraCallsiteGraphHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-callsite-graph",
        "Each row on the left is a source line that logged events. The line on its right points to the table it wrote.",
        Some("The badge is how many rows that line wrote in the window."),
        &["Click a table name to explore its rows"],
    )
}
