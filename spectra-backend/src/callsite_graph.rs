//! Callsite graph: which source lines write which event tables and record which metrics.
//!
//! Spectra stamps each event row with a reserved `_callsite` object (`file`, `line`, and
//! `module` when known) and each metric point with a `_callsite` label holding that object's
//! JSON text. This module folds recent rows and points back into one [`EmitSite`] per
//! `(file, line, target)` so the UI can draw emit sites linked to what they write.
//!
//! [`execute_callsite_graph`] also says why a graph is empty ([`EmptyReason`]) and which
//! tables the caller couldn't read ([`CallsiteGraph::hidden_tables`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spectra_core::{
    EventGridRow, EventQuery, GridFilterModel, GridPaginationModel, GridSortDirection,
    GridSortItem, MetricPoint, MetricsQueryRange, SpectraRouter, CALLSITE_FIELD,
    MAX_EVENT_QUERY_LIMIT,
};

use crate::execute_event_query;

/// What an emit site writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    /// Rows into an event table.
    Table,
    /// Points into a metric.
    Metric,
}

/// Why a callsite graph has no emit sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyReason {
    /// No Spectra query backend is installed on this host.
    NoBackend,
    /// The caller can't query any event table or metric.
    NoGrants,
    /// Nothing the caller can read has a callsite in the window.
    NothingLogged,
}

/// One source line that wrote rows into an event table or points into a metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitSite {
    /// Source file as recorded by `#[track_caller]`.
    pub file: String,
    /// One-based line number.
    pub line: u32,
    /// Module path of the line, crate first, when Spectra could derive it.
    pub module: Option<String>,
    /// Whether [`Self::target`] is an event table or a metric.
    pub kind: SiteKind,
    /// Event table or metric the site writes.
    pub target: String,
    /// Rows (tables) or points (metrics) from this site in the scanned window.
    pub count: u64,
    /// Newest row or point timestamp from this site.
    pub last_seen: DateTime<Utc>,
}

impl EmitSite {
    /// The module path without its crate (`fernwood_store::checkout` → `checkout`); a crate
    /// root module keeps its crate name.
    #[must_use]
    pub fn module_in_crate(&self) -> Option<&str> {
        let module = self.module.as_deref()?;
        Some(module.split_once("::").map_or(module, |(_, rest)| rest))
    }
}

/// What the caller may scan: readable tables and metrics, plus the tables left out for lack
/// of a `spectra.query.{table}` grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallsiteScope {
    /// Event tables the caller may query.
    pub tables: Vec<String>,
    /// Metrics the caller may query.
    pub metrics: Vec<String>,
    /// Event tables the caller may not query; reported, never scanned.
    pub hidden_tables: Vec<String>,
}

/// Emit sites and what they write, built from a bounded scan of recent rows and points.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallsiteGraph {
    /// Emit sites, sorted by file, line, kind, then target.
    pub sites: Vec<EmitSite>,
    /// Tables that have at least one emit site, sorted by name.
    pub tables: Vec<String>,
    /// Metrics that have at least one emit site, sorted by name.
    pub metrics: Vec<String>,
    /// Event tables left out because the caller can't query them, sorted by name.
    pub hidden_tables: Vec<String>,
    /// Event rows scanned across all tables.
    pub scanned: u64,
    /// Scanned rows with no usable `_callsite`.
    pub untracked: u64,
    /// `true` when a table had more matching rows than the scan covered.
    pub truncated: bool,
    /// Why there are no sites; `None` whenever [`Self::sites`] is non-empty.
    pub empty: Option<EmptyReason>,
}

/// A parsed `_callsite`: file, line and optional module.
struct Callsite {
    file: String,
    line: u32,
    module: Option<String>,
}

/// Parses a `_callsite` object into its parts.
fn parse_callsite_object(site: &Value) -> Option<Callsite> {
    let file = site.get("file")?.as_str()?;
    let line = u32::try_from(site.get("line")?.as_u64()?).ok()?;
    if file.is_empty() {
        return None;
    }
    let module = site
        .get("module")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
        .map(str::to_owned);
    Some(Callsite {
        file: file.to_owned(),
        line,
        module,
    })
}

/// Adds one row or point to the matching site, or starts a new one.
fn add_to_sites(
    sites: &mut Vec<EmitSite>,
    kind: SiteKind,
    target: &str,
    callsite: Callsite,
    ts: DateTime<Utc>,
) {
    if let Some(site) = sites
        .iter_mut()
        .find(|s| s.file == callsite.file && s.line == callsite.line)
    {
        site.count = site.count.saturating_add(1);
        site.last_seen = site.last_seen.max(ts);
        if site.module.is_none() {
            site.module = callsite.module;
        }
    } else {
        sites.push(EmitSite {
            file: callsite.file,
            line: callsite.line,
            module: callsite.module,
            kind,
            target: target.to_owned(),
            count: 1,
            last_seen: ts,
        });
    }
}

/// Folds one table's rows into emit sites, returning them with the count of rows that
/// had no usable `_callsite`.
#[must_use]
pub fn fold_callsites(table: &str, rows: &[EventGridRow]) -> (Vec<EmitSite>, u64) {
    let mut sites: Vec<EmitSite> = Vec::new();
    let mut untracked = 0u64;
    for row in rows {
        match row
            .fields
            .get(CALLSITE_FIELD)
            .and_then(parse_callsite_object)
        {
            Some(callsite) => add_to_sites(&mut sites, SiteKind::Table, table, callsite, row.ts),
            None => untracked = untracked.saturating_add(1),
        }
    }
    (sites, untracked)
}

/// Folds one metric's points into emit sites. Points without a `_callsite` label (gauges,
/// direct `try_record_*` calls) are skipped.
#[must_use]
pub fn fold_metric_callsites(metric: &str, points: &[MetricPoint]) -> Vec<EmitSite> {
    let mut sites: Vec<EmitSite> = Vec::new();
    for point in points {
        let callsite = point
            .labels
            .get(CALLSITE_FIELD)
            .and_then(Value::as_str)
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .as_ref()
            .and_then(parse_callsite_object);
        if let Some(callsite) = callsite {
            add_to_sites(&mut sites, SiteKind::Metric, metric, callsite, point.ts);
        }
    }
    sites
}

/// Builds the callsite graph for `scope` over `[start, end]`.
///
/// Scans the newest [`MAX_EVENT_QUERY_LIMIT`] rows per table and every point per metric in
/// the window. A table or metric whose query fails is skipped with a warning so one broken
/// store does not blank the graph. With no `router` the graph is empty with
/// [`EmptyReason::NoBackend`]; with nothing readable, [`EmptyReason::NoGrants`].
pub async fn execute_callsite_graph(
    router: Option<&SpectraRouter>,
    scope: &CallsiteScope,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> CallsiteGraph {
    let mut hidden_tables = scope.hidden_tables.clone();
    hidden_tables.sort();
    hidden_tables.dedup();
    let mut graph = CallsiteGraph {
        hidden_tables,
        ..CallsiteGraph::default()
    };
    let Some(router) = router else {
        graph.empty = Some(EmptyReason::NoBackend);
        return graph;
    };
    if scope.tables.is_empty() && scope.metrics.is_empty() {
        graph.empty = Some(EmptyReason::NoGrants);
        return graph;
    }
    for table in &scope.tables {
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
    for metric in &scope.metrics {
        let range = MetricsQueryRange {
            metric_name: metric.clone(),
            start,
            end,
            label_matchers: Vec::new(),
        };
        match router.query_metrics(range).await {
            Ok(points) => graph.sites.extend(fold_metric_callsites(metric, &points)),
            Err(err) => log::warn!("callsite graph skipped metric (metric={metric}, error={err})"),
        }
    }
    graph.sites.sort_by(|a, b| {
        (&a.file, a.line, a.kind, &a.target).cmp(&(&b.file, b.line, b.kind, &b.target))
    });
    graph.tables = targets_of(&graph.sites, SiteKind::Table);
    graph.metrics = targets_of(&graph.sites, SiteKind::Metric);
    if graph.sites.is_empty() {
        graph.empty = Some(EmptyReason::NothingLogged);
    }
    graph
}

/// Sorted, de-duplicated targets of one kind.
fn targets_of(sites: &[EmitSite], kind: SiteKind) -> Vec<String> {
    let mut targets: Vec<String> = sites
        .iter()
        .filter(|s| s.kind == kind)
        .map(|s| s.target.clone())
        .collect();
    targets.sort();
    targets.dedup();
    targets
}
