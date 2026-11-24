//! Viewing a saved dashboard: the toolbar state kept in the URL, the query behind each
//! panel, and how personal fields are hidden.
//!
//! Everything here is plain data in and out, so the page, the server functions and the tests
//! share one definition. The server functions in `spectra-app` call
//! [`event_aggregate_request`], [`event_rows_query`] and [`metrics_query`] after the per-table
//! Gauge check, then [`mask_rows`] and [`stand_in_labels`] before anything leaves the server.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spectra_core::{
    EventAggregateRequest, EventAggregationSpec, EventExploreView, EventMeasure, EventQuery,
    GridFilterItem, GridFilterModel, GridFilterOperator, GridLogicOperator, GridPaginationModel,
    GridSortDirection, GridSortItem, LabelMatcher, MetricsQuery, SchemaRegistry, PII_MASK,
};

use super::model::{
    ChartKind, Dashboard, DashboardFilter, FilterOp, Panel, PanelSource, Variable, Window,
};
use crate::{encode_ops_path_segment, spectra_metric_explore_path, spectra_schema_explore_path};

/// Default row limit for a table panel saved without [`super::TableOpts`].
pub const DEFAULT_TABLE_PANEL_ROWS: u32 = 20;

/// The dashboard-wide time range picked in the toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RangeChoice {
    /// The last hour.
    #[default]
    LastHour,
    /// The last six hours.
    Last6Hours,
    /// The last 24 hours.
    Last24Hours,
    /// The last seven days.
    Last7Days,
    /// A fixed range.
    Custom {
        /// Start of the range.
        from: DateTime<Utc>,
        /// End of the range.
        to: DateTime<Utc>,
    },
}

impl RangeChoice {
    /// The relative ranges, in toolbar order.
    pub const PRESETS: [Self; 4] = [
        Self::LastHour,
        Self::Last6Hours,
        Self::Last24Hours,
        Self::Last7Days,
    ];

    /// Value written to the `range` query parameter.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::LastHour => "1h",
            Self::Last6Hours => "6h",
            Self::Last24Hours => "24h",
            Self::Last7Days => "7d",
            Self::Custom { .. } => "custom",
        }
    }

    /// Toolbar label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::LastHour => "Last hour",
            Self::Last6Hours => "Last 6 hours",
            Self::Last24Hours => "Last 24 hours",
            Self::Last7Days => "Last 7 days",
            Self::Custom { .. } => "Custom range",
        }
    }

    /// The preset with this `range` key.
    #[must_use]
    pub fn preset(key: &str) -> Option<Self> {
        Self::PRESETS.into_iter().find(|p| p.key() == key)
    }

    /// The window panels query unless they keep their own.
    #[must_use]
    pub const fn window(&self) -> Window {
        let secs = match self {
            Self::LastHour => 3_600,
            Self::Last6Hours => 6 * 3_600,
            Self::Last24Hours => 24 * 3_600,
            Self::Last7Days => 7 * 24 * 3_600,
            Self::Custom { from, to } => {
                return Window::Absolute {
                    from: *from,
                    to: *to,
                }
            }
        };
        Window::Relative { secs }
    }
}

/// Toolbar state for one dashboard view: range, variable values and filters.
///
/// It round-trips through the URL query (`range`, `from`, `to`, `var.<name>`, and a repeated
/// `f=field:op:value`), so a copied link opens the dashboard as it was seen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewState {
    /// Dashboard-wide range.
    pub range: RangeChoice,
    /// Variable values picked in the toolbar, by variable name. A missing name uses the
    /// variable's default; an empty value means all values.
    pub variables: BTreeMap<String, String>,
    /// Filters picked in the toolbar. `None` uses the dashboard's saved filters.
    pub filters: Option<Vec<DashboardFilter>>,
}

impl ViewState {
    /// Read the state from decoded query pairs. Unknown keys and malformed values are skipped.
    #[must_use]
    pub fn from_query<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut state = Self::default();
        let mut from = None;
        let mut to = None;
        let mut custom = false;
        for (key, value) in pairs {
            match key {
                "range" if value == "custom" => custom = true,
                "range" => {
                    if let Some(preset) = RangeChoice::preset(value) {
                        state.range = preset;
                    }
                }
                "from" => from = DateTime::parse_from_rfc3339(value).ok(),
                "to" => to = DateTime::parse_from_rfc3339(value).ok(),
                "f" => {
                    let filters = state.filters.get_or_insert_with(Vec::new);
                    if let Some(filter) = parse_filter(value) {
                        filters.push(filter);
                    }
                }
                _ => {
                    if let Some(name) = key.strip_prefix("var.") {
                        state.variables.insert(name.to_string(), value.to_string());
                    }
                }
            }
        }
        if custom {
            if let (Some(from), Some(to)) = (from, to) {
                let (from, to) = (from.with_timezone(&Utc), to.with_timezone(&Utc));
                if from < to {
                    state.range = RangeChoice::Custom { from, to };
                }
            }
        }
        state
    }

    /// The query string for this state, without the leading `?`. The default range is left
    /// out; an empty filter list is written as a bare `f=` so it overrides saved filters.
    #[must_use]
    pub fn to_query(&self) -> String {
        let mut parts = Vec::new();
        match self.range {
            RangeChoice::LastHour => {}
            RangeChoice::Custom { from, to } => {
                parts.push("range=custom".to_string());
                parts.push(format!(
                    "from={}",
                    encode_ops_path_segment(&from.to_rfc3339())
                ));
                parts.push(format!("to={}", encode_ops_path_segment(&to.to_rfc3339())));
            }
            other => parts.push(format!("range={}", other.key())),
        }
        for (name, value) in &self.variables {
            parts.push(format!(
                "var.{}={}",
                encode_ops_path_segment(name),
                encode_ops_path_segment(value)
            ));
        }
        match &self.filters {
            None => {}
            Some(filters) if filters.is_empty() => parts.push("f=".to_string()),
            Some(filters) => {
                for f in filters {
                    parts.push(format!("f={}", encode_ops_path_segment(&format_filter(f))));
                }
            }
        }
        parts.join("&")
    }

    /// The filters in effect: the toolbar's, or the dashboard's saved ones.
    #[must_use]
    pub fn filters_or<'a>(&'a self, saved: &'a [DashboardFilter]) -> &'a [DashboardFilter] {
        self.filters.as_deref().unwrap_or(saved)
    }

    /// Each variable's value in effect, by variable name. Variables set to all values are
    /// left out.
    #[must_use]
    pub fn variable_values(&self, variables: &[Variable]) -> BTreeMap<String, String> {
        variables
            .iter()
            .filter_map(|var| {
                let value = self
                    .variables
                    .get(&var.name)
                    .cloned()
                    .or_else(|| var.default.clone())?;
                (!value.is_empty()).then(|| (var.name.clone(), value))
            })
            .collect()
    }
}

/// Short name of a filter operator in the URL.
#[must_use]
pub const fn filter_op_key(op: FilterOp) -> &'static str {
    match op {
        FilterOp::Eq => "eq",
        FilterOp::Ne => "ne",
        FilterOp::Contains => "contains",
        FilterOp::Gt => "gt",
        FilterOp::Lt => "lt",
    }
}

/// The symbol shown on a filter tag.
#[must_use]
pub const fn filter_op_symbol(op: FilterOp) -> &'static str {
    match op {
        FilterOp::Eq => "=",
        FilterOp::Ne => "≠",
        FilterOp::Contains => "contains",
        FilterOp::Gt => ">",
        FilterOp::Lt => "<",
    }
}

/// Every filter operator, in picker order.
pub const FILTER_OPS: [FilterOp; 5] = [
    FilterOp::Eq,
    FilterOp::Ne,
    FilterOp::Contains,
    FilterOp::Gt,
    FilterOp::Lt,
];

/// `field:op:value`, as written to the `f` query parameter.
#[must_use]
pub fn format_filter(filter: &DashboardFilter) -> String {
    format!(
        "{}:{}:{}",
        filter.field,
        filter_op_key(filter.op),
        filter.value
    )
}

/// Parse `field:op:value`. The value may itself contain `:`.
#[must_use]
pub fn parse_filter(raw: &str) -> Option<DashboardFilter> {
    let mut parts = raw.splitn(3, ':');
    let field = parts.next()?.trim();
    let op = parts.next()?;
    let value = parts.next()?;
    let op = FILTER_OPS.into_iter().find(|o| filter_op_key(*o) == op)?;
    (!field.is_empty()).then(|| DashboardFilter {
        field: field.to_string(),
        op,
        value: value.to_string(),
    })
}

/// Start and end of `window`, ending at `now` for relative windows.
#[must_use]
pub fn window_bounds(window: Window, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    match window {
        Window::Relative { secs } => (
            now - Duration::seconds(i64::try_from(secs).unwrap_or(i64::MAX / 1_000)),
            now,
        ),
        Window::Absolute { from, to } => (from, to),
    }
}

/// The table or metric a panel reads; `None` for text.
#[must_use]
pub fn panel_source_name(panel: &Panel) -> Option<&str> {
    match &panel.source {
        PanelSource::Metric { metric } => Some(metric),
        PanelSource::EventTable { table } => Some(table),
        _ => None,
    }
}

/// Every `spectra.query.*` permission the dashboard's panels need that `can_query` says the
/// viewer lacks. Text panels need none.
pub fn missing_query_permissions(
    dashboard: &Dashboard,
    mut can_query: impl FnMut(&str) -> bool,
) -> BTreeSet<String> {
    let needed: BTreeSet<String> = dashboard
        .sections
        .iter()
        .flat_map(|s| &s.panels)
        .filter_map(Panel::required_query_permission)
        .collect();
    needed.into_iter().filter(|p| !can_query(p)).collect()
}

/// Field names in a registered schema, or `None` when the binary doesn't know the schema.
#[must_use]
pub fn source_fields(source: &str) -> Option<Vec<String>> {
    SchemaRegistry::global()
        .get_schema(source)
        .map(|schema| schema.fields.iter().map(|f| f.name.clone()).collect())
}

/// Personal (PII) fields in a registered schema.
#[must_use]
pub fn pii_fields(source: &str) -> BTreeSet<String> {
    SchemaRegistry::global()
        .get_schema(source)
        .map_or_default(|schema| {
            schema
                .fields
                .iter()
                .filter(|f| f.classification.pii)
                .map(|f| f.name.clone())
                .collect()
        })
}

fn has_field(fields: Option<&[String]>, field: &str) -> bool {
    field == "ts" || fields.is_none_or(|fields| fields.iter().any(|f| f == field))
}

/// Names of the variables whose field the panel's source doesn't have, given the source's
/// fields (`None` when unknown, which counts as having every field). Text panels have none.
#[must_use]
pub fn missing_variable_fields(
    panel: &Panel,
    variables: &[Variable],
    fields: Option<&[String]>,
) -> Vec<String> {
    if panel_source_name(panel).is_none() {
        return Vec::new();
    }
    variables
        .iter()
        .filter(|v| !has_field(fields, &v.field))
        .map(|v| v.name.clone())
        .collect()
}

/// The filters one panel queries with.
///
/// That is the dashboard's filters (or the toolbar's), one equality filter per variable
/// value, then the panel's own. Dashboard filters and variables on a field the source
/// doesn't have are skipped rather than emptying the panel.
#[must_use]
pub fn effective_filters(
    panel: &Panel,
    variables: &[Variable],
    dashboard_filters: &[DashboardFilter],
    variable_values: &BTreeMap<String, String>,
    fields: Option<&[String]>,
) -> Vec<DashboardFilter> {
    let mut out: Vec<DashboardFilter> = dashboard_filters
        .iter()
        .filter(|f| has_field(fields, &f.field))
        .cloned()
        .collect();
    for var in variables {
        if let Some(value) = variable_values.get(&var.name) {
            if has_field(fields, &var.field) {
                out.push(DashboardFilter {
                    field: var.field.clone(),
                    op: FilterOp::Eq,
                    value: value.clone(),
                });
            }
        }
    }
    out.extend(panel.filters.iter().cloned());
    out
}

fn filter_value(op: FilterOp, raw: &str) -> Value {
    if matches!(op, FilterOp::Gt | FilterOp::Lt) {
        if let Ok(n) = raw.parse::<i64>() {
            return Value::from(n);
        }
        if let Ok(n) = raw.parse::<f64>() {
            return Value::from(n);
        }
    }
    Value::String(raw.to_string())
}

/// The explorer's filter model for `filters`, all joined with AND.
#[must_use]
pub fn to_grid_filter(filters: &[DashboardFilter]) -> GridFilterModel {
    GridFilterModel {
        items: filters
            .iter()
            .map(|f| GridFilterItem {
                field: f.field.clone(),
                operator: match f.op {
                    FilterOp::Eq => GridFilterOperator::Equals,
                    FilterOp::Ne => GridFilterOperator::DoesNotEqual,
                    FilterOp::Contains => GridFilterOperator::Contains,
                    FilterOp::Gt => GridFilterOperator::GreaterThan,
                    FilterOp::Lt => GridFilterOperator::LessThan,
                },
                value: filter_value(f.op, &f.value),
            })
            .collect(),
        logic_operator: GridLogicOperator::And,
        quick_filter_values: Vec::new(),
    }
}

/// The measure written as `count` or `sum(field)`. Anything else counts.
#[must_use]
pub fn parse_measure(measure: Option<&str>) -> (EventMeasure, Option<String>) {
    measure
        .map(str::trim)
        .and_then(|m| m.strip_prefix("sum(")?.strip_suffix(')'))
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map_or((EventMeasure::Count, None), |field| {
            (EventMeasure::Sum, Some(field.to_string()))
        })
}

/// Bucket length in seconds: the panel's (`30s`, `5m`, `1h`, `1d`), or one picked from the
/// window's length.
#[must_use]
pub fn bucket_secs(bucket: Option<&str>, window_secs: u64) -> u64 {
    let parsed = bucket.map(str::trim).and_then(|b| {
        let (num, unit) = b.split_at(b.find(|c: char| !c.is_ascii_digit())?);
        let n: u64 = num.parse().ok()?;
        let scale = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3_600,
            "d" => 86_400,
            _ => return None,
        };
        (n > 0).then_some(n * scale)
    });
    parsed.unwrap_or(match window_secs {
        0..=3_600 => 60,
        3_601..=21_600 => 300,
        21_601..=86_400 => 900,
        _ => 3_600,
    })
}

/// The caption under a panel's title: the source and how it's measured, such as
/// `checkout_event · count by status`.
#[must_use]
pub fn panel_caption(panel: &Panel) -> String {
    let Some(source) = panel_source_name(panel) else {
        return "Text".to_string();
    };
    let how = match (&panel.source, panel.chart) {
        (_, ChartKind::Table) => "newest rows".to_string(),
        (PanelSource::Metric { .. }, _) => {
            panel.measure.clone().unwrap_or_else(|| "value".to_string())
        }
        _ => {
            let measure = panel.measure.clone().unwrap_or_else(|| "count".to_string());
            match panel.group_by.first() {
                Some(group) if matches!(panel.chart, ChartKind::Bar | ChartKind::Pie) => {
                    format!("{measure} by {group}")
                }
                _ => measure,
            }
        }
    };
    format!("{source} · {how}")
}

/// The aggregate query behind an event-table chart or single-stat panel.
#[must_use]
pub fn event_aggregate_request(
    panel: &Panel,
    table: &str,
    window: Window,
    now: DateTime<Utc>,
    filters: &[DashboardFilter],
) -> EventAggregateRequest {
    let (start, end) = window_bounds(window, now);
    let window_secs = u64::try_from((end - start).num_seconds()).unwrap_or(0);
    let (measure, measure_field) = parse_measure(panel.measure.as_deref());
    let (view, group_by_field) = match panel.chart {
        ChartKind::Bar => (EventExploreView::BarChart, panel.group_by.first().cloned()),
        ChartKind::Pie => (EventExploreView::PieChart, panel.group_by.first().cloned()),
        _ => (EventExploreView::TimeSeries, None),
    };
    EventAggregateRequest {
        table: table.to_string(),
        start,
        end,
        partition: None,
        filter: to_grid_filter(filters),
        view,
        aggregation: EventAggregationSpec {
            measure,
            measure_field,
            time_bucket_secs: Some(bucket_secs(panel.bucket.as_deref(), window_secs)),
            group_by_field,
            row_fields: Vec::new(),
            pivot_field: None,
        },
    }
}

/// The row query behind an event-table table panel: its sort (newest first by default) and
/// row limit.
#[must_use]
pub fn event_rows_query(
    panel: &Panel,
    table: &str,
    window: Window,
    now: DateTime<Utc>,
    filters: &[DashboardFilter],
) -> EventQuery {
    let (start, end) = window_bounds(window, now);
    let opts = panel.table.as_ref();
    let sort = opts.and_then(|t| t.sort.as_ref()).map_or_else(
        || GridSortItem {
            field: "ts".into(),
            sort: GridSortDirection::Desc,
        },
        |s| GridSortItem {
            field: s.column.clone(),
            sort: if s.descending {
                GridSortDirection::Desc
            } else {
                GridSortDirection::Asc
            },
        },
    );
    EventQuery {
        table: table.to_string(),
        start,
        end,
        partition: None,
        pagination: GridPaginationModel {
            page: 0,
            page_size: opts.map_or(DEFAULT_TABLE_PANEL_ROWS, |t| t.limit),
        },
        sort: vec![sort],
        filter: to_grid_filter(filters),
    }
}

/// The metric query behind a metric panel. Equality filters become label matchers; other
/// operators have no metric equivalent and are skipped.
#[must_use]
pub fn metrics_query(
    panel: &Panel,
    metric: &str,
    window: Window,
    now: DateTime<Utc>,
    filters: &[DashboardFilter],
) -> MetricsQuery {
    let (start, end) = window_bounds(window, now);
    let window_secs = u64::try_from((end - start).num_seconds()).unwrap_or(0);
    MetricsQuery {
        metric: metric.to_string(),
        start,
        end,
        step_secs: Some(bucket_secs(panel.bucket.as_deref(), window_secs)),
        label_matchers: filters
            .iter()
            .filter(|f| f.op == FilterOp::Eq)
            .map(|f| LabelMatcher {
                key: f.field.clone(),
                value: f.value.clone(),
            })
            .collect(),
    }
}

/// Link into the explorer for a panel's source, carrying the window and filters.
#[must_use]
pub fn explore_href(panel: &Panel, window: Window, filters: &[DashboardFilter]) -> Option<String> {
    let base = match &panel.source {
        PanelSource::Metric { metric } => spectra_metric_explore_path(metric),
        PanelSource::EventTable { table } => spectra_schema_explore_path(table),
        _ => return None,
    };
    let mut parts = match window {
        Window::Relative { secs } => vec![format!("window={secs}")],
        Window::Absolute { from, to } => vec![
            format!("from={}", encode_ops_path_segment(&from.to_rfc3339())),
            format!("to={}", encode_ops_path_segment(&to.to_rfc3339())),
        ],
    };
    parts.extend(
        filters
            .iter()
            .map(|f| format!("f={}", encode_ops_path_segment(&format_filter(f)))),
    );
    Some(format!("{base}?{}", parts.join("&")))
}

/// The noun a stand-in label uses for a personal field: `customer_id` reads as "Customer".
#[must_use]
pub fn stand_in_noun(field: &str) -> String {
    let base = ["_id", "_email", "_name", "_ip", "_phone"]
        .iter()
        .find_map(|suffix| field.strip_suffix(suffix))
        .unwrap_or(field);
    let words = base.replace(['_', '-'], " ");
    let mut chars = words.trim().chars();
    chars.next().map_or_else(
        || "Value".to_string(),
        |first| first.to_uppercase().chain(chars).collect(),
    )
}

fn salted_hash(salt: &str, value: &str) -> u64 {
    // FNV-1a: fixed across builds and processes, unlike `DefaultHasher`.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in salt.bytes().chain([0]).chain(value.bytes()) {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Stand-in labels for the values of a personal field in one panel: "Customer 1",
/// "Customer 2" and so on, one per distinct value, returned in the order of `values`.
///
/// Numbers come from a hash salted with the panel id, so the same value gets the same label
/// every time the panel loads, two panels number the same values differently, and the
/// numbering says nothing about the real values' order.
///
/// TODO(spc2-03): use spectra-core's stand-in labels once they land.
#[must_use]
pub fn stand_in_labels(field: &str, values: &[String], panel_salt: &str) -> Vec<String> {
    let noun = stand_in_noun(field);
    let mut distinct: Vec<&String> = values.iter().collect::<BTreeSet<_>>().into_iter().collect();
    distinct.sort_by_key(|v| (salted_hash(panel_salt, v), *v));
    let numbers: BTreeMap<&String, usize> = distinct
        .into_iter()
        .enumerate()
        .map(|(i, v)| (v, i + 1))
        .collect();
    values
        .iter()
        .map(|v| format!("{noun} {}", numbers[v]))
        .collect()
}

/// A table panel's rows as display text, with personal fields replaced by
/// [`PII_MASK`]. Returns the rows and the columns that were masked.
#[must_use]
pub fn mask_rows(
    columns: &[String],
    rows: &[Value],
    pii: &BTreeSet<String>,
) -> (Vec<Vec<String>>, Vec<String>) {
    let masked: Vec<String> = columns
        .iter()
        .filter(|c| pii.contains(*c))
        .cloned()
        .collect();
    let text = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|c| {
                    if pii.contains(c) {
                        return PII_MASK.to_string();
                    }
                    match row.get(c) {
                        None | Some(Value::Null) => String::new(),
                        Some(Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                    }
                })
                .collect()
        })
        .collect();
    (text, masked)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::*;
    use crate::dashboards::{Layout, Section, TableOpts, TableSort};

    fn panel(source: PanelSource, chart: ChartKind) -> Panel {
        Panel {
            id: "p1".into(),
            title: "Checkouts".into(),
            source,
            chart,
            measure: None,
            bucket: None,
            group_by: Vec::new(),
            filters: Vec::new(),
            table: None,
            layout: Layout {
                col: 0,
                span: 6,
                row: 0,
                height: 4,
            },
            keep_window: None,
        }
    }

    fn event(table: &str, chart: ChartKind) -> Panel {
        panel(
            PanelSource::EventTable {
                table: table.into(),
            },
            chart,
        )
    }

    fn dashboard(panels: Vec<Panel>) -> Dashboard {
        Dashboard {
            id: "d1".into(),
            title: "Checkout health".into(),
            description: String::new(),
            owner: Some("u1".into()),
            read_only: false,
            sections: vec![Section {
                id: "s1".into(),
                title: String::new(),
                collapsed: false,
                panels,
            }],
            variables: vec![Variable {
                name: "host".into(),
                field: "host".into(),
                default: None,
            }],
            filters: vec![DashboardFilter {
                field: "status".into(),
                op: FilterOp::Eq,
                value: "500".into(),
            }],
            version: 3,
            updated_at: Utc::now(),
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 23, 12, 0, 0).unwrap()
    }

    #[test]
    fn view_state_round_trips_through_the_query() {
        let state = ViewState {
            range: RangeChoice::Last6Hours,
            variables: BTreeMap::from([("host".into(), "web 1".into())]),
            filters: Some(vec![DashboardFilter {
                field: "path".into(),
                op: FilterOp::Contains,
                value: "a:b/c".into(),
            }]),
        };
        let query = state.to_query();
        assert_eq!(
            query,
            "range=6h&var.host=web%201&f=path%3Acontains%3Aa%3Ab%2Fc"
        );
        let decoded = [
            ("range", "6h"),
            ("var.host", "web 1"),
            ("f", "path:contains:a:b/c"),
        ];
        assert_eq!(ViewState::from_query(decoded), state);
    }

    #[test]
    fn custom_range_needs_both_ends_in_order() {
        let from = "2026-11-20T00:00:00+00:00";
        let to = "2026-11-21T00:00:00+00:00";
        let state = ViewState::from_query([("range", "custom"), ("from", from), ("to", to)]);
        assert!(matches!(state.range, RangeChoice::Custom { .. }));
        let back = ViewState::from_query([("range", "custom"), ("from", to), ("to", from)]);
        assert_eq!(back.range, RangeChoice::LastHour);
        let half = ViewState::from_query([("range", "custom"), ("from", from)]);
        assert_eq!(half.range, RangeChoice::LastHour);
    }

    #[test]
    fn bare_f_clears_saved_filters_and_no_f_keeps_them() {
        let d = dashboard(Vec::new());
        assert_eq!(
            ViewState::default().filters_or(&d.filters),
            d.filters.as_slice()
        );
        let cleared = ViewState::from_query([("f", "")]);
        assert_eq!(cleared.filters_or(&d.filters), []);
        assert_eq!(cleared.to_query(), "f=");
    }

    #[test]
    fn malformed_query_values_are_skipped() {
        let state = ViewState::from_query([
            ("range", "3w"),
            ("f", "status:like:5"),
            ("f", ":eq:1"),
            ("other", "x"),
        ]);
        assert_eq!(state.range, RangeChoice::LastHour);
        assert_eq!(state.filters, Some(Vec::new()));
    }

    #[test]
    fn variable_values_fall_back_to_defaults_and_drop_all() {
        let mut d = dashboard(Vec::new());
        d.variables[0].default = Some("web-1".into());
        assert_eq!(
            ViewState::default().variable_values(&d.variables),
            BTreeMap::from([("host".into(), "web-1".into())])
        );
        let all = ViewState::from_query([("var.host", "")]);
        assert!(all.variable_values(&d.variables).is_empty());
    }

    #[test]
    fn missing_permissions_skip_text_and_granted_sources() {
        let d = dashboard(vec![
            event("checkout_event", ChartKind::Bar),
            event("checkout_event", ChartKind::Pie),
            panel(
                PanelSource::Metric {
                    metric: "checkout_total".into(),
                },
                ChartKind::TimeSeries,
            ),
            panel(
                PanelSource::Text {
                    markdown: "hi".into(),
                },
                ChartKind::Table,
            ),
        ]);
        let missing = missing_query_permissions(&d, |p| p == "spectra.query.checkout_total");
        assert_eq!(
            missing.into_iter().collect::<Vec<_>>(),
            vec!["spectra.query.checkout_event".to_string()]
        );
        assert!(missing_query_permissions(&d, |_| true).is_empty());
    }

    #[test]
    fn effective_filters_skip_fields_the_source_lacks() {
        let p = Panel {
            filters: vec![DashboardFilter {
                field: "severity".into(),
                op: FilterOp::Ne,
                value: "debug".into(),
            }],
            ..event("checkout_event", ChartKind::Bar)
        };
        let d = dashboard(vec![p.clone()]);
        let vars = BTreeMap::from([("host".to_string(), "web-1".to_string())]);
        let fields = vec!["severity".to_string(), "host".to_string()];
        let out = effective_filters(&p, &d.variables, &d.filters, &vars, Some(&fields));
        let names: Vec<&str> = out.iter().map(|f| f.field.as_str()).collect();
        assert_eq!(names, ["host", "severity"]);
        assert_eq!(
            missing_variable_fields(&p, &d.variables, Some(&["severity".to_string()])),
            ["host"]
        );
        let unknown = effective_filters(&p, &d.variables, &d.filters, &vars, None);
        assert_eq!(unknown.len(), 3);
        assert_eq!(
            missing_variable_fields(&p, &d.variables, None),
            Vec::<String>::new()
        );
    }

    #[test]
    fn grid_filter_numbers_only_for_comparisons() {
        let filters = [
            DashboardFilter {
                field: "status".into(),
                op: FilterOp::Eq,
                value: "500".into(),
            },
            DashboardFilter {
                field: "ms".into(),
                op: FilterOp::Gt,
                value: "250".into(),
            },
        ];
        let grid = to_grid_filter(&filters);
        assert_eq!(grid.items[0].value, json!("500"));
        assert_eq!(grid.items[1].value, json!(250));
        assert!(matches!(
            grid.items[1].operator,
            GridFilterOperator::GreaterThan
        ));
    }

    #[test]
    fn measure_and_bucket_parse() {
        assert_eq!(parse_measure(None), (EventMeasure::Count, None));
        assert_eq!(parse_measure(Some("count")), (EventMeasure::Count, None));
        assert_eq!(
            parse_measure(Some("sum(duration_ms)")),
            (EventMeasure::Sum, Some("duration_ms".into()))
        );
        assert_eq!(parse_measure(Some("sum()")), (EventMeasure::Count, None));
        assert_eq!(bucket_secs(Some("5m"), 3_600), 300);
        assert_eq!(bucket_secs(Some("1d"), 3_600), 86_400);
        assert_eq!(bucket_secs(Some("0m"), 3_600), 60);
        assert_eq!(bucket_secs(Some("soon"), 7 * 86_400), 3_600);
        assert_eq!(bucket_secs(None, 6 * 3_600), 300);
    }

    #[test]
    fn captions_name_source_and_measure() {
        let bar = Panel {
            group_by: vec!["status".into()],
            ..event("checkout_event", ChartKind::Bar)
        };
        assert_eq!(panel_caption(&bar), "checkout_event · count by status");
        let series = Panel {
            group_by: vec!["status".into()],
            measure: Some("sum(total)".into()),
            ..event("checkout_event", ChartKind::TimeSeries)
        };
        assert_eq!(panel_caption(&series), "checkout_event · sum(total)");
        assert_eq!(
            panel_caption(&event("checkout_event", ChartKind::Table)),
            "checkout_event · newest rows"
        );
        let text = panel(
            PanelSource::Text {
                markdown: String::new(),
            },
            ChartKind::Table,
        );
        assert_eq!(panel_caption(&text), "Text");
    }

    #[test]
    fn aggregate_request_follows_chart_kind() {
        let bar = Panel {
            group_by: vec!["status".into()],
            ..event("checkout_event", ChartKind::Bar)
        };
        let req = event_aggregate_request(
            &bar,
            "checkout_event",
            Window::Relative { secs: 3_600 },
            now(),
            &[],
        );
        assert_eq!(req.view, EventExploreView::BarChart);
        assert_eq!(req.aggregation.group_by_field.as_deref(), Some("status"));
        assert_eq!(req.end - req.start, Duration::hours(1));
        let series = Panel {
            chart: ChartKind::TimeSeries,
            ..bar
        };
        let req = event_aggregate_request(
            &series,
            "checkout_event",
            Window::Relative { secs: 3_600 },
            now(),
            &[],
        );
        assert_eq!(req.view, EventExploreView::TimeSeries);
        assert_eq!(req.aggregation.group_by_field, None);
    }

    #[test]
    fn rows_query_uses_sort_and_limit() {
        let p = Panel {
            table: Some(TableOpts {
                columns: vec!["id".into()],
                sort: Some(TableSort {
                    column: "value".into(),
                    descending: false,
                }),
                limit: 5,
            }),
            ..event("checkout_event", ChartKind::Table)
        };
        let q = event_rows_query(
            &p,
            "checkout_event",
            Window::Relative { secs: 60 },
            now(),
            &[],
        );
        assert_eq!(q.pagination.page_size, 5);
        assert_eq!(q.sort[0].field, "value");
        assert_eq!(q.sort[0].sort, GridSortDirection::Asc);
        let plain = event_rows_query(
            &event("t", ChartKind::Table),
            "t",
            Window::Relative { secs: 60 },
            now(),
            &[],
        );
        assert_eq!(plain.pagination.page_size, DEFAULT_TABLE_PANEL_ROWS);
        assert_eq!(plain.sort[0].sort, GridSortDirection::Desc);
    }

    #[test]
    fn metric_query_keeps_only_equality_filters() {
        let p = panel(
            PanelSource::Metric { metric: "m".into() },
            ChartKind::TimeSeries,
        );
        let filters = [
            DashboardFilter {
                field: "host".into(),
                op: FilterOp::Eq,
                value: "a".into(),
            },
            DashboardFilter {
                field: "ms".into(),
                op: FilterOp::Gt,
                value: "1".into(),
            },
        ];
        let q = metrics_query(&p, "m", Window::Relative { secs: 3_600 }, now(), &filters);
        assert_eq!(q.label_matchers.len(), 1);
        assert_eq!(q.step_secs, Some(60));
    }

    #[test]
    fn explore_href_carries_window_and_filters() {
        let p = event("checkout_event", ChartKind::Bar);
        let f = [DashboardFilter {
            field: "status".into(),
            op: FilterOp::Eq,
            value: "500".into(),
        }];
        assert_eq!(
            explore_href(&p, Window::Relative { secs: 3_600 }, &f).as_deref(),
            Some("/spectra/schema/checkout_event/explore?window=3600&f=status%3Aeq%3A500")
        );
        let text = panel(
            PanelSource::Text {
                markdown: String::new(),
            },
            ChartKind::Table,
        );
        assert_eq!(explore_href(&text, Window::Relative { secs: 1 }, &[]), None);
    }

    #[test]
    fn stand_in_labels_are_stable_distinct_and_salted() {
        let values: Vec<String> = ["alice", "bob", "alice", "carol"]
            .map(String::from)
            .to_vec();
        let a = stand_in_labels("customer_id", &values, "p1");
        assert_eq!(a, stand_in_labels("customer_id", &values, "p1"));
        assert_eq!(a[0], a[2]);
        let distinct: BTreeSet<&String> = a.iter().collect();
        assert_eq!(distinct.len(), 3);
        assert!(a.iter().all(|l| l.starts_with("Customer ")));
        assert!(!a.iter().any(|l| l.contains("alice")));
        let other_salts: Vec<Vec<String>> = (0..8)
            .map(|i| stand_in_labels("customer_id", &values, &format!("panel-{i}")))
            .collect();
        assert!(other_salts.iter().any(|labels| *labels != a));
    }

    #[test]
    fn stand_in_noun_reads_like_a_word() {
        assert_eq!(stand_in_noun("customer_id"), "Customer");
        assert_eq!(stand_in_noun("user_email"), "User");
        assert_eq!(stand_in_noun("shipping_address"), "Shipping address");
        assert_eq!(stand_in_noun("_id"), "Value");
    }

    #[test]
    fn mask_rows_hides_personal_columns() {
        let columns = vec!["id".to_string(), "email".to_string(), "n".to_string()];
        let rows = vec![json!({"id": "r1", "email": "a@b.c", "n": 3})];
        let pii = BTreeSet::from(["email".to_string()]);
        let (text, masked) = mask_rows(&columns, &rows, &pii);
        assert_eq!(
            text,
            vec![vec![
                "r1".to_string(),
                PII_MASK.to_string(),
                "3".to_string()
            ]]
        );
        assert_eq!(masked, ["email"]);
    }
}
