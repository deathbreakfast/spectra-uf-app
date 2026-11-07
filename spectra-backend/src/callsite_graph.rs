//! Callsite graph: which source lines emit rows into which event tables.
//!
//! Generated `*Logger` helpers stamp each event row with a reserved `_callsite` object
//! (`file`, `line`). This module folds recent rows back into one [`EmitSite`] per
//! `(file, line, table)` so the UI can draw emit sites linked to the tables they write.
//! Metrics carry no callsite, so they are not part of the graph.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spectra_core::{
    EventGridRow, EventQuery, GridFilterModel, GridPaginationModel, GridSortDirection,
    GridSortItem, SpectraRouter, CALLSITE_FIELD, MAX_EVENT_QUERY_LIMIT,
};

use crate::execute_event_query;

/// One source line that wrote rows into an event table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitSite {
    /// Source file as recorded by `#[track_caller]`.
    pub file: String,
    /// One-based line number.
    pub line: u32,
    /// Event table the site writes.
    pub table: String,
    /// Rows from this site in the scanned window.
    pub count: u64,
    /// Newest row timestamp from this site.
    pub last_seen: DateTime<Utc>,
}

/// Emit sites and the tables they write, built from a bounded scan of recent rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallsiteGraph {
    /// Emit sites, sorted by file, line, then table.
    pub sites: Vec<EmitSite>,
    /// Tables that have at least one emit site, sorted by name.
    pub tables: Vec<String>,
    /// Rows scanned across all tables.
    pub scanned: u64,
    /// Scanned rows with no usable `_callsite` (rows written without a generated logger).
    pub untracked: u64,
    /// `true` when a table had more matching rows than the scan covered.
    pub truncated: bool,
}

/// Parses the reserved `_callsite` object into `(file, line)`.
fn parse_callsite(fields: &Value) -> Option<(String, u32)> {
    let site = fields.get(CALLSITE_FIELD)?;
    let file = site.get("file")?.as_str()?;
    let line = u32::try_from(site.get("line")?.as_u64()?).ok()?;
    if file.is_empty() {
        return None;
    }
    Some((file.to_owned(), line))
}

/// Folds one table's rows into emit sites, returning them with the count of rows that
/// had no usable `_callsite`.
#[must_use]
pub fn fold_callsites(table: &str, rows: &[EventGridRow]) -> (Vec<EmitSite>, u64) {
    let mut sites: Vec<EmitSite> = Vec::new();
    let mut untracked = 0u64;
    for row in rows {
        let Some((file, line)) = parse_callsite(&row.fields) else {
            untracked = untracked.saturating_add(1);
            continue;
        };
        if let Some(site) = sites.iter_mut().find(|s| s.file == file && s.line == line) {
            site.count = site.count.saturating_add(1);
            site.last_seen = site.last_seen.max(row.ts);
        } else {
            sites.push(EmitSite {
                file,
                line,
                table: table.to_owned(),
                count: 1,
                last_seen: row.ts,
            });
        }
    }
    (sites, untracked)
}

/// Builds the callsite graph for `tables` over `[start, end]`.
///
/// Scans the newest [`MAX_EVENT_QUERY_LIMIT`] rows per table. A table whose query fails is
/// skipped with a warning so one broken table does not blank the graph.
pub async fn execute_callsite_graph(
    router: &SpectraRouter,
    tables: &[String],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> CallsiteGraph {
    let mut graph = CallsiteGraph::default();
    for table in tables {
        let query = EventQuery {
            table: table.clone(),
            start,
            end,
            partition: None,
            pagination: GridPaginationModel {
                page: 0,
                page_size: MAX_EVENT_QUERY_LIMIT,
            },
            sort: vec![GridSortItem {
                field: "ts".into(),
                sort: GridSortDirection::Desc,
            }],
            filter: GridFilterModel::default(),
        };
        let result = match execute_event_query(router, &query).await {
            Ok(result) => result,
            Err(err) => {
                log::warn!("callsite graph skipped table (table={table}, error={err})");
                continue;
            }
        };
        let scanned = u64::try_from(result.rows.len()).unwrap_or(u64::MAX);
        graph.scanned = graph.scanned.saturating_add(scanned);
        graph.truncated |= result.row_count > scanned;
        let (sites, untracked) = fold_callsites(table, &result.rows);
        graph.untracked = graph.untracked.saturating_add(untracked);
        graph.sites.extend(sites);
    }
    graph
        .sites
        .sort_by(|a, b| (&a.file, a.line, &a.table).cmp(&(&b.file, b.line, &b.table)));
    let mut tables: Vec<String> = graph.sites.iter().map(|s| s.table.clone()).collect();
    tables.sort();
    tables.dedup();
    graph.tables = tables;
    graph
}
