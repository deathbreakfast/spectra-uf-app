//! Spotlight steps for the schema index (`/spectra/schema`).

use leptos::prelude::*;
use uf_help_macros::help_spotlight_step;

use super::help_stack;

/// Schema search box.
#[help_spotlight_step(
    route = "/spectra/schema",
    feature_highlight = "spectra-schema-search",
    title = "Search schemas",
    spotlight = "spectra-schema-search",
    position = "bottom",
    order = 10
)]
#[component]
pub fn SpectraSchemaSearchHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-schema-search",
        "Type part of a schema name to hide cards that do not match. Clear the box to see everything again.",
        Some("If nothing matches, the page says so—try a shorter fragment."),
        &[],
    )
}

/// Schema card grid.
#[help_spotlight_step(
    route = "/spectra/schema",
    feature_highlight = "spectra-schema-grid",
    title = "Schema cards",
    spotlight = "spectra-schema-grid",
    position = "top",
    order = 20
)]
#[component]
pub fn SpectraSchemaGridHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-schema-grid",
        "Every card is one schema drawer.",
        Some("Scan kinds before you open a card."),
        &[
            "Name: identifier you will type later",
            "Kind: Event log or Metric",
            "Description: optional plain-language note",
        ],
    )
}

/// Details link on the first card.
#[help_spotlight_step(
    route = "/spectra/schema",
    feature_highlight = "spectra-schema-open-details",
    title = "Open Details",
    spotlight = "spectra-schema-open-details",
    position = "top",
    order = 30
)]
#[component]
pub fn SpectraSchemaOpenDetailsHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-schema-open-details",
        "Opens the info page for this schema: kind, description, and a button into Explore.",
        None,
        &[],
    )
}

/// Explore link on the first card.
#[help_spotlight_step(
    route = "/spectra/schema",
    feature_highlight = "spectra-schema-open-explore",
    title = "Open Explore",
    spotlight = "spectra-schema-open-explore",
    position = "top",
    order = 40
)]
#[component]
pub fn SpectraSchemaOpenExploreHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-schema-open-explore",
        "Opens the live data view. Event schemas go to the event explorer; metrics go to the metric explorer.",
        None,
        &[],
    )
}

/// Left navigation on the schema index.
#[help_spotlight_step(
    route = "/spectra/schema",
    feature_highlight = "spectra-schema-nav",
    title = "Finding your way",
    spotlight = "spectra-nav",
    position = "right",
    order = 50
)]
#[component]
pub fn SpectraSchemaNavHelp() -> impl IntoView {
    help_stack(
        "help-step-spectra-schema-nav",
        "Home has the catalog's counts, Schemas lists every table and metric, Dashboards holds saved views, and Callsites shows which code writes each one.",
        Some("Replay anytime: Help → Replay this route."),
        &[],
    )
}
