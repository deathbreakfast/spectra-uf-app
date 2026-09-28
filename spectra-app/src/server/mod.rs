//! Spectra app server functions.
//!
//! Leptos `#[server]` entrypoints resolve Higgs request context, then delegate to
//! [`spectra_backend`] helpers (and [`permissions`]) so catalog/query contracts can
//! be verified without a full host.
//!
//! Every server fn requires an authenticated session and `QueryTable` (via
//! `#[uf_product_macros::server(permission = "...")]`). Explore queries also call
//! [`require_spectra_query`] for per-table Gauge `spectra.query.{table}` checks.
//!
//! ## Errors
//!
//! Fallible ops return [`ServerFnError`] (Leptos boundary). Blank, oversized, or
//! path-unsafe table/metric names are rejected by
//! [`spectra_backend::validate_spectra_query_name`] as
//! [`spectra_backend::SpectraOpsError::Validation`]. Missing session,
//! Higgs/valence resolution failures, and Gauge denials map from
//! [`spectra_backend::SpectraOpsError`] via [`to_server_fn_error`].
//!
//! ## Security note
//!
//! Authz is session + `QueryTable` on every ops server fn, plus per-table Gauge
//! `spectra.query.{table}` on explore queries. Detail/explore hrefs must use
//! [`spectra_backend`] path helpers so registry names cannot smuggle `/` path
//! segments. Event/metric row payloads may contain PII when a live backend is
//! wired — grant `QueryTable` / per-table query permissions narrowly.

mod permissions;

use leptos::prelude::*;
#[cfg(feature = "ssr")]
pub use permissions::ensure_table_query_permissions;
pub use permissions::require_spectra_query;
pub use spectra_backend::{
    encode_ops_path_segment, execute_event_aggregate, execute_event_query, execute_metrics_query,
    schema_metadata_detail, schema_metadata_list, spectra_metric_explore_path,
    spectra_query_permission_name, spectra_schema_explore_path, spectra_schema_path,
    validate_spectra_query_name, SpectraOpsError, MAX_SPECTRA_QUERY_NAME_CHARS,
};

mod dashboard;
mod error;
pub use dashboard::{get_spectra_dashboard_summary, SpectraDashboardSummary};
pub use error::{server_fn_is_permission_denied, to_server_fn_error};
use spectra_core::{
    EventAggregateRequest, EventAggregateResult, EventQuery, EventQueryResult, MetricsQuery,
    MetricsQueryResult, SchemaDetailDto, SchemaListItem,
};

/// Permission name required for Spectra catalog and explore reads
/// (manifest: [`crate::permissions::SpectraPermission::QueryTable`]).
pub const SPECTRA_QUERY_PERMISSION: &str = "QueryTable";

/// Require an authenticated session (`session_user_id`).
#[cfg(feature = "ssr")]
fn require_session(ctx: &higgs::Higgs) -> Result<(), ServerFnError> {
    if ctx.session_user_id().is_some() {
        Ok(())
    } else {
        Err(to_server_fn_error(SpectraOpsError::AuthRequired))
    }
}

/// List summary metadata for every registered schema.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn list_schema_metadata() -> Result<Vec<SchemaListItem>, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;
    Ok(schema_metadata_list())
}

/// Fetch full detail for a single schema by name, if it exists.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn get_schema_metadata(
    /// Name of the schema to fetch detail for.
    name: String,
) -> Result<Option<SchemaDetailDto>, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;
    validate_spectra_query_name(&name).map_err(|e| to_server_fn_error(e.into()))?;
    Ok(schema_metadata_detail(&name))
}

/// Run a metric query and return the resulting time series and headline values.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn query_metrics(
    /// Metric query describing the metric, time range, and aggregation to run.
    query: MetricsQuery,
) -> Result<MetricsQueryResult, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;
    require_spectra_query(&query.metric)
        .await
        .map_err(to_server_fn_error)?;
    let router = spectra_core::SpectraRouter::try_global()
        .ok_or_else(|| to_server_fn_error(SpectraOpsError::BackendNotInstalled))?;
    execute_metrics_query(&router, &query)
        .await
        .map_err(|e| to_server_fn_error(SpectraOpsError::QueryFailed(e.to_string())))
}

/// Run an event query against a table and return matching rows.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn query_events(
    /// Event query describing the table, filters, and paging to run.
    query: EventQuery,
) -> Result<EventQueryResult, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;
    require_spectra_query(&query.table)
        .await
        .map_err(to_server_fn_error)?;
    let router = spectra_core::SpectraRouter::try_global()
        .ok_or_else(|| to_server_fn_error(SpectraOpsError::BackendNotInstalled))?;
    execute_event_query(&router, &query)
        .await
        .map_err(|e| to_server_fn_error(SpectraOpsError::QueryFailed(e.to_string())))
}

/// Run an aggregate query (time series or headline) over events in a table.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn query_event_aggregate(
    /// Aggregate query describing the table, grouping, and time range to run.
    request: EventAggregateRequest,
) -> Result<EventAggregateResult, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;
    require_spectra_query(&request.table)
        .await
        .map_err(to_server_fn_error)?;
    let router = spectra_core::SpectraRouter::try_global()
        .ok_or_else(|| to_server_fn_error(SpectraOpsError::BackendNotInstalled))?;
    execute_event_aggregate(&router, &request)
        .await
        .map_err(|e| to_server_fn_error(SpectraOpsError::QueryFailed(e.to_string())))
}
