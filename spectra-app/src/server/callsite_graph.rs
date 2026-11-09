//! Callsite graph server function.

use chrono::{Duration, Utc};
use leptos::prelude::*;
use spectra_backend::{execute_callsite_graph, schema_metadata_list, CallsiteGraph};

#[cfg(feature = "ssr")]
use super::require_session;
use super::require_spectra_query;

/// Longest window the graph accepts, in hours (seven days).
pub const MAX_CALLSITE_WINDOW_HOURS: u32 = 168;

/// Loads emit sites for every event table the caller may query.
///
/// Scans the newest rows of each table inside the last `window_hours` (clamped to
/// 1..=[`MAX_CALLSITE_WINDOW_HOURS`]). Tables the caller lacks `spectra.query.{table}` for are
/// left out. Returns an empty graph when no Spectra query backend is installed.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn get_callsite_graph(
    /// Look-back window in hours.
    window_hours: u32,
) -> Result<CallsiteGraph, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;

    let Some(router) = spectra_core::SpectraRouter::try_global() else {
        return Ok(CallsiteGraph::default());
    };
    let mut tables = Vec::new();
    for item in schema_metadata_list() {
        if item.logging_kind == "event"
            && require_spectra_query(&item.table_or_metric).await.is_ok()
        {
            tables.push(item.table_or_metric);
        }
    }
    let end = Utc::now();
    let start = end - Duration::hours(i64::from(window_hours.clamp(1, MAX_CALLSITE_WINDOW_HOURS)));
    Ok(execute_callsite_graph(&router, &tables, start, end).await)
}
