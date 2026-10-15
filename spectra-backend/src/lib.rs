//! Pure backend contracts for the Spectra ops UI server surface.
//!
//! Validation, catalog helpers, and empty explore-query payloads that `spectra-app`
//! `#[server]` functions call after resolving Higgs request context. Keeps schema list/detail
//! and explore result shapes unit-testable without a Leptos host or UI graph.
//!
//! ## Features
//!
//! - **Query name validation** — Reject blank, oversized, or path-unsafe table and metric
//!   names before Gauge permission checks or explore stubs.
//!   [Get started](#validate-query-names)
//! - **Catalog and explore stubs** — Schema list/detail helpers and empty explore payloads
//!   returned until the host injects live query backends.
//!   [Get started](#catalog-and-stubs)
//! - **Gauge permission names** — Format per-table `spectra.query.{table}` strings via
//!   [`spectra_query_permission_name`].
//! - **Ops path encoding** — Percent-encode path segments for `/spectra` hrefs via
//!   [`encode_ops_path_segment`], [`spectra_schema_path`], [`spectra_schema_explore_path`],
//!   and [`spectra_metric_explore_path`].
//!
//! ## Validate query names
//!
//! Explore and schema-detail lookups reject names that would break routing or smuggle path
//! segments into Gauge checks. [`validate_spectra_query_name`] runs before `spectra-app`
//! server functions call catalog or explore helpers — call it in custom wrappers when you add
//! new read paths that accept table or metric parameters.
//!
//! **Prerequisites:** None beyond importing this crate; validators are synchronous and return
//! [`SpectraQueryNameError`] on failure.
//!
//! ```rust,ignore
//! use spectra_backend::{
//!     validate_spectra_query_name, SpectraQueryNameError, MAX_SPECTRA_QUERY_NAME_CHARS,
//! };
//!
//! validate_spectra_query_name("ops.events").expect("valid table");
//! assert_eq!(
//!     validate_spectra_query_name("").unwrap_err(),
//!     SpectraQueryNameError::EmptyTableName
//! );
//! assert_eq!(MAX_SPECTRA_QUERY_NAME_CHARS, 256);
//! ```
//!
//! On success validators return `Ok(())` and the trimmed name is safe for permission checks.
//! Blank, oversized, control-character, slash, backslash, or `.` / `..` names map to typed
//! [`SpectraQueryNameError`] variants with operator-facing messages.
//!
//! ## Catalog and stubs
//!
//! Catalog helpers list registered schemas and resolve detail metadata; explore stubs return
//! empty result shapes the UI can render before a host wires live Spectra query backends.
//! [`schema_metadata_list`] and [`schema_metadata_detail`] back schema index/detail pages;
//! [`empty_event_query_result`], [`empty_event_aggregate_result`], and
//! [`empty_metrics_query_result`] back explore server functions.
//!
//! **Prerequisites:** Catalog helpers delegate to `spectra-core` registry functions — they do
//! not perform network IO. Explore stubs are synchronous and ignore query parameters except
//! where noted.
//!
//! ```rust,ignore
//! use spectra_backend::{
//!     schema_metadata_list, schema_metadata_detail,
//!     empty_event_query_result, empty_metrics_query_result,
//! };
//!
//! let schemas = schema_metadata_list();
//! let first_name = schemas.first().map(|s| s.table_or_metric.as_str());
//! assert_eq!(first_name, Some("ops.events"));
//!
//! let detail = schema_metadata_detail("ops.events");
//! assert_eq!(detail.as_ref().map(|d| d.table_or_metric.as_str()), Some("ops.events"));
//!
//! let events = empty_event_query_result("ops.events");
//! assert_eq!(events.row_count, 0);
//!
//! let metrics = empty_metrics_query_result();
//! assert_eq!(metrics.series.len(), 0);
//! ```
//!
//! On success catalog helpers return registered schema rows or `None` when a name is unknown;
//! explore stubs return empty vectors with stable column metadata so pages load without error.
//!
//! ## Examples
//!
//! Start with [Validate query names](#validate-query-names). This crate's unit and integ suites
//! are listed in `docs/VERIFICATION.md`. Runnable host: `examples/protected-spectra-host`
//! (inventory `spectra` / `/spectra`).

mod dashboard;
mod query_live;

pub use dashboard::{
    dashboard_catalog_summary, DashboardCatalogSummary, DEFAULT_RECENT_SCHEMA_LIMIT,
};
mod ops_error;
pub use ops_error::{
    is_permission_denied_message, SpectraOpsError, AUTH_REQUIRED_MESSAGE,
    BACKEND_NOT_INSTALLED_MESSAGE, PERMISSION_DENIED_PREFIX,
};
pub use query_live::{
    count_matching_events, execute_event_aggregate, execute_event_query, execute_metrics_query,
};

use spectra_core::{
    list_schemas, rows_to_event_result, schema_detail, EventAggregateResult, EventQueryResult,
    MetricsQueryResult, SchemaDetailDto, SchemaListItem,
};

/// Maximum Unicode scalar count for table/metric names accepted by explore and
/// schema detail lookups.
pub const MAX_SPECTRA_QUERY_NAME_CHARS: usize = 256;

/// Blank, oversized, or path-unsafe table/metric name rejected before Gauge
/// permission checks or explore stubs.
///
/// Callers map this into Leptos `ServerFnError` (or equivalent) at the `#[server]`
/// boundary; the Display text stays stable for UI and contract tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpectraQueryNameError {
    /// Table or metric name was empty or whitespace-only.
    EmptyTableName,
    /// Table or metric name exceeded [`MAX_SPECTRA_QUERY_NAME_CHARS`].
    TableNameTooLong,
    /// Table or metric name contained `/`, `\`, ASCII controls, or was `.` / `..`.
    UnsafeTableName,
}

impl std::fmt::Display for SpectraQueryNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTableName => write!(f, "Spectra query table name is required"),
            Self::TableNameTooLong => write!(
                f,
                "Spectra query table name exceeds {MAX_SPECTRA_QUERY_NAME_CHARS} characters"
            ),
            Self::UnsafeTableName => write!(
                f,
                "Spectra query table name contains unsafe path characters"
            ),
        }
    }
}

impl std::error::Error for SpectraQueryNameError {}

/// ASCII controls (C0 + DEL) plus path separators. Avoids `char::is_control`,
/// which is not `const` on the leptos-lints pinned nightly.
const fn is_unsafe_ops_id_char(c: char) -> bool {
    c <= '\u{1f}' || c == '\u{7f}' || c == '/' || c == '\\'
}

/// Rejects blank, oversized, path-separating, control, or `.` / `..` table/metric
/// names before host permission checks run.
///
/// Dotted names such as `ops.events` remain valid; only the exact segments `.`
/// and `..` are rejected.
///
/// # Errors
///
/// Returns a [`SpectraQueryNameError`] variant when `table` is empty/whitespace-only,
/// longer than [`MAX_SPECTRA_QUERY_NAME_CHARS`], contains `/` `\` or ASCII controls,
/// or is exactly `.` / `..`.
pub fn validate_spectra_query_name(table: &str) -> Result<(), SpectraQueryNameError> {
    let trimmed = table.trim();
    if trimmed.is_empty() {
        return Err(SpectraQueryNameError::EmptyTableName);
    }
    if trimmed.chars().count() > MAX_SPECTRA_QUERY_NAME_CHARS {
        return Err(SpectraQueryNameError::TableNameTooLong);
    }
    if trimmed == "." || trimmed == ".." {
        return Err(SpectraQueryNameError::UnsafeTableName);
    }
    if trimmed.chars().any(is_unsafe_ops_id_char) {
        return Err(SpectraQueryNameError::UnsafeTableName);
    }
    Ok(())
}

/// Gauge permission name for querying a Spectra table or metric.
#[must_use]
pub fn spectra_query_permission_name(table: &str) -> String {
    format!("spectra.query.{}", table.trim())
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Percent-encode a single path segment for `/spectra/...` hrefs.
///
/// Leaves RFC 3986 unreserved characters alone (`ALPHA` / `DIGIT` / `-` `.` `_`
/// `~`). Encodes `/`, `\`, controls, spaces, and other bytes so raw `format!`
/// hrefs cannot smuggle extra path segments.
#[must_use]
pub fn encode_ops_path_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for &b in raw.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

/// `/spectra/schema/{encoded}` detail href.
#[must_use]
pub fn spectra_schema_path(name: &str) -> String {
    format!("/spectra/schema/{}", encode_ops_path_segment(name))
}

/// `/spectra/schema/{encoded}/explore` event explore href.
#[must_use]
pub fn spectra_schema_explore_path(name: &str) -> String {
    format!("/spectra/schema/{}/explore", encode_ops_path_segment(name))
}

/// `/spectra/metric/{encoded}/explore` metric explore href.
#[must_use]
pub fn spectra_metric_explore_path(name: &str) -> String {
    format!("/spectra/metric/{}/explore", encode_ops_path_segment(name))
}

/// Catalog listing used by `list_schema_metadata` after request context resolves.
#[must_use]
pub fn schema_metadata_list() -> Vec<SchemaListItem> {
    list_schemas()
}

/// Schema detail used by `get_schema_metadata` after request context resolves.
#[must_use]
pub fn schema_metadata_detail(name: &str) -> Option<SchemaDetailDto> {
    schema_detail(name)
}

/// Empty metrics explore payload returned until the host injects a live backend.
#[must_use]
pub const fn empty_metrics_query_result() -> MetricsQueryResult {
    MetricsQueryResult {
        series: Vec::new(),
        headline: Vec::new(),
    }
}

/// Empty event-log payload for a table (unknown schema → default `ts` column).
#[must_use]
pub fn empty_event_query_result(table: &str) -> EventQueryResult {
    rows_to_event_result(table, Vec::new(), 0)
}

/// Empty time-series aggregate payload returned until the host injects a live backend.
#[must_use]
pub const fn empty_event_aggregate_result() -> EventAggregateResult {
    EventAggregateResult::TimeSeries {
        series: Vec::new(),
        headline: Vec::new(),
    }
}

#[cfg(test)]
mod unit_tests;
