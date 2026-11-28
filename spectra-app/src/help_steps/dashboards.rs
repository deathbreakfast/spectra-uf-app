//! Spotlight steps for dashboards (`/spectra/dashboards`, `/spectra/dashboards/:id` and
//! `/spectra/dashboards/:id/edit`).

use leptos::prelude::*;
use uf_help_macros::help_spotlight_step;

use super::help_stack;

/// Mine, Shared with me and All I can read tabs on the list.
#[help_spotlight_step(
    route = "/spectra/dashboards",
    feature_highlight = "spectra-dashboards-tabs",
    title = "Your dashboards",
    spotlight = "spectra-dashboards-tabs",
    position = "bottom",
    order = 10
)]
#[component]
pub fn SpectraDashboardsTabsHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboards-tabs",
        "Mine are the dashboards you own. Shared with me lists the ones someone shared with you or a group you're in.",
        Some("Built-in dashboards appear under All I can read and can be duplicated, not changed."),
        &[],
    )
}

/// New dashboard button.
#[help_spotlight_step(
    route = "/spectra/dashboards",
    feature_highlight = "spectra-dashboards-new",
    title = "Start a dashboard",
    spotlight = "spectra-dashboards-new",
    position = "bottom",
    order = 20
)]
#[component]
pub fn SpectraDashboardsNewHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboards-new",
        "Start blank or from a copy. A new dashboard is private until you share it.",
        None,
        &[],
    )
}

/// Left navigation on the dashboard list.
#[help_spotlight_step(
    route = "/spectra/dashboards",
    feature_highlight = "spectra-dashboards-nav",
    title = "Finding your way",
    spotlight = "spectra-nav",
    position = "right",
    order = 30
)]
#[component]
pub fn SpectraDashboardsNavHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboards-nav",
        "Home has the catalog's counts, Schemas lists every table and metric, Dashboards holds saved views, and Callsites shows which code writes each one.",
        Some("Replay anytime: Help → Replay this route."),
        &[],
    )
}

/// Dashboard-wide range, variables and filters.
#[help_spotlight_step(
    route = "/spectra/dashboards/:id",
    feature_highlight = "spectra-dashboard-toolbar",
    title = "One time range for every panel",
    spotlight = "spectra-dashboard-toolbar",
    position = "bottom",
    order = 10
)]
#[component]
pub fn SpectraDashboardToolbarHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboard-toolbar",
        "The range, variables and filters here apply to every panel at once.",
        Some("A panel whose source has no matching field ignores a variable and says so."),
        &["$host: a variable", "region = eu-west: a filter"],
    )
}

/// Panels, including locked ones.
#[help_spotlight_step(
    route = "/spectra/dashboards/:id",
    feature_highlight = "spectra-dashboard-panels",
    title = "Locked panels",
    spotlight = "spectra-dashboard-panels",
    position = "top",
    order = 20
)]
#[component]
pub fn SpectraDashboardPanelsHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboard-panels",
        "A panel on a table you can't read stays locked and never shows its data.",
        Some("Request access asks for the exact permission the panel needs."),
        &[],
    )
}

/// The edit grid.
#[help_spotlight_step(
    route = "/spectra/dashboards/:id/edit",
    feature_highlight = "spectra-dashboard-grid",
    title = "Arrange panels",
    spotlight = "spectra-dashboard-grid",
    position = "top",
    order = 10
)]
#[component]
pub fn SpectraDashboardGridHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboard-grid",
        "Drag a panel by its handle and resize it from the corner. Panels snap to twelve columns.",
        Some("On a phone you can reorder panels but not resize them."),
        &[],
    )
}

/// Add panel menu in the edit bar.
#[help_spotlight_step(
    route = "/spectra/dashboards/:id/edit",
    feature_highlight = "spectra-dashboard-add-panel",
    title = "Add a panel",
    spotlight = "spectra-dashboard-add-panel",
    position = "bottom",
    order = 20
)]
#[component]
pub fn SpectraDashboardAddPanelHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboard-add-panel",
        "Pick a metric or an event table, or paste a query link, then a chart. The preview runs as you change it.",
        Some("Saving keeps the old version in Version history."),
        &[],
    )
}
