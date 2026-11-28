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

/// Show toggle: tables, metrics or both.
#[help_spotlight_step(
    route = "/spectra/graph",
    feature_highlight = "spectra-callsite-show",
    title = "Tables and metrics",
    spotlight = "spectra-callsite-show",
    position = "bottom",
    order = 15
)]
#[component]
pub fn SpectraCallsiteShowHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-callsite-show",
        "Show narrows the graph to event tables or metrics when you're looking for one kind of writer.",
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
        "Each box on the left is a line of code. Its arrow points to the table it writes or the metric it records.",
        Some("The count is rows for a table and points for a metric, within the window."),
        &[
            "Yellow: event table",
            "Blue: metric",
            "Open a table or metric from All emit sites",
        ],
    )
}
