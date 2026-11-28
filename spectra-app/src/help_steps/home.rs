//! Spotlight steps for the Spectra home (`/spectra`).

use leptos::prelude::*;
use uf_help_macros::help_spotlight_step;

use super::help_stack;

/// Centered intro: filing-room metaphor and Schema / Event log / Metric vocabulary.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-intro",
    title = "Welcome to Spectra",
    order = 10
)]
#[component]
pub fn SpectraIntroHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-intro",
        "Spectra is where you look at recorded events and metrics. Think of it like a filing room for data the system already collected.",
        Some("We will walk the screens one piece at a time."),
        &[
            "Schema: the label on a drawer (what kind of data this is)",
            "Event log: a diary of things that happened",
            "Metric: a number that changes over time",
        ],
    )
}

/// KPI cards on the home dashboard.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-dashboard-stats",
    title = "At a glance",
    spotlight = "spectra-dashboard-stats",
    position = "bottom",
    order = 20
)]
#[component]
pub fn SpectraDashboardStatsHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-dashboard-stats",
        "These numbers are the catalog's pulse.",
        Some("Come back here for a quick health check."),
        &[
            "Schemas: how many drawers exist",
            "Event tables: how many event diaries",
            "Metrics: how many number series",
            "24h event rows: recent diary activity (when the host reports it)",
        ],
    )
}

/// Recent schema cards.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-home-recent",
    title = "Recent schemas",
    spotlight = "spectra-home-recent",
    position = "top",
    order = 30
)]
#[component]
pub fn SpectraHomeRecentHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-home-recent",
        "Each card is one schema: its name, whether it is an Event or Metric, and a short description when one exists.",
        Some("Use these cards to recognize what data is available before you open Details or Explore from the catalog."),
        &[],
    )
}

/// Link to the full schema catalog.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-home-view-all",
    title = "View all schemas",
    spotlight = "spectra-home-view-all",
    position = "top",
    order = 40
)]
#[component]
pub fn SpectraHomeViewAllHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-home-view-all",
        "Opens the full Schemas list so you can search and open any registered schema.",
        Some("You can click now, or keep touring and use the left menu later."),
        &[],
    )
}

/// Quick-open search box.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-quick-open-search",
    title = "Find by name",
    spotlight = "spectra-quick-open-search",
    position = "top",
    order = 50
)]
#[component]
pub fn SpectraQuickOpenSearchHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-quick-open-search",
        "Type the exact schema name when you already know it. This skips scrolling the catalog.",
        Some("Tip: names must match a registered schema."),
        &[],
    )
}

/// Open detail from quick open.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-quick-open-detail",
    title = "Open detail",
    spotlight = "spectra-quick-open-detail",
    position = "top",
    order = 60
)]
#[component]
pub fn SpectraQuickOpenDetailHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-quick-open-detail",
        "Opens that schema's info page: description and kind, plus a path into Explore.",
        None,
        &[],
    )
}

/// Open explore from quick open.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-quick-open-explore",
    title = "Open explore",
    spotlight = "spectra-quick-open-explore",
    position = "top",
    order = 70
)]
#[component]
pub fn SpectraQuickOpenExploreHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-quick-open-explore",
        "Jumps straight into exploring that schema's data. From home, this uses the event-style explore path for the name you typed.",
        None,
        &[],
    )
}

/// Left navigation.
#[help_spotlight_step(
    route = "/spectra",
    feature_highlight = "spectra-nav",
    title = "Finding your way",
    spotlight = "spectra-nav",
    position = "right",
    order = 80
)]
#[component]
pub fn SpectraNavHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-nav",
        "Home has the catalog's counts, Schemas lists every table and metric, Dashboards holds saved views, and Callsites shows which code writes each one.",
        Some("Replay anytime: Help → Replay this route."),
        &[],
    )
}
