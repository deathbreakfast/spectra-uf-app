//! Callsite graph server function.

use chrono::{Duration, Utc};
use leptos::prelude::*;
use spectra_backend::{execute_callsite_graph, schema_metadata_list, CallsiteGraph, CallsiteScope};

#[cfg(feature = "ssr")]
use super::require_session;
use super::require_spectra_query;

/// Longest window the graph accepts, in hours (seven days).
pub const MAX_CALLSITE_WINDOW_HOURS: u32 = 168;

/// Loads emit sites for every event table and metric the caller may query.
///
/// Scans the newest rows of each table and the points of each metric inside the last
/// `window_hours` (clamped to 1..=[`MAX_CALLSITE_WINDOW_HOURS`]). Tables the caller lacks
/// `spectra.query.{table}` for are not scanned and come back in
/// [`CallsiteGraph::hidden_tables`]; metrics without the grant are left out. An empty graph
/// carries its [`CallsiteGraph::empty`] reason: no backend, no grants, or nothing logged.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn get_callsite_graph(
    /// Look-back window in hours.
    window_hours: u32,
) -> Result<CallsiteGraph, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;

    let router = spectra_core::SpectraRouter::try_global();
    let mut scope = CallsiteScope::default();
    if router.is_some() {
        for item in schema_metadata_list() {
            let readable = require_spectra_query(&item.table_or_metric).await.is_ok();
            match (item.logging_kind.as_str(), readable) {
                ("event", true) => scope.tables.push(item.table_or_metric),
                ("event", false) => scope.hidden_tables.push(item.table_or_metric),
                ("metric", true) => scope.metrics.push(item.table_or_metric),
                _ => {}
            }
        }
    }
    let end = Utc::now();
    let start = end - Duration::hours(i64::from(window_hours.clamp(1, MAX_CALLSITE_WINDOW_HOURS)));
    Ok(execute_callsite_graph(router.as_deref(), &scope, start, end).await)
}
