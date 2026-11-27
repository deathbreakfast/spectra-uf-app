//! Query links and panels: turning a pasted link into panel settings and a panel back into
//! a link, plus the wording the editor shows about it.

use chrono::{DateTime, Utc};
use spectra_backend::dashboards::view::{parse_measure, RangeChoice};
use spectra_backend::dashboards::{
    ChartKind, DashboardFilter, FilterOp, Layout, Panel, PanelSource, TableOpts, TableSort, Window,
};
use spectra_backend::query_url::{
    decode, FilterOp as LinkOp, LinkFilter, QueryBucket, QueryLink, QueryLinkError, QueryMeasure,
    QuerySort, QuerySource, QueryView, QueryWindow, WindowPreset,
};
use spectra_core::EventMeasure;

use crate::server::SourceField;

/// The Field error for text that isn't a Spectra query link.
pub(super) const NOT_A_LINK: &str = "That isn't a Spectra query link. Paste one from the event \
                                     explorer, the metric explorer or a saved query.";
/// Row limit for a new table panel, and for a link that sets none.
pub(super) const DEFAULT_ROWS: u32 = 25;
/// Metric aggregations, with their labels.
pub(super) const METRIC_MEASURES: [(&str, &str); 5] = [
    ("avg", "Average"),
    ("max", "Max"),
    ("min", "Min"),
    ("sum", "Sum"),
    ("last", "Last"),
];
/// Chart choices, with their labels.
pub(super) const CHARTS: [(ChartKind, &str); 5] = [
    (ChartKind::TimeSeries, "Time series"),
    (ChartKind::Bar, "Bar"),
    (ChartKind::Pie, "Pie"),
    (ChartKind::Table, "Table"),
    (ChartKind::SingleStat, "Single stat"),
];

/// The setting a link field fed, emptied because the source has no such field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Emptied {
    Measure,
    GroupBy,
    Filter,
    Column,
    Sort,
}

/// A field a link names that the source doesn't have, and what was left out because of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UnknownField {
    pub field: String,
    pub emptied: Emptied,
}

impl UnknownField {
    /// "`customer_name` isn't a field of checkout_event, so Group by is empty."
    pub fn message(&self, source: &str) -> String {
        let consequence = match self.emptied {
            Emptied::Measure => "so Measure is Count",
            Emptied::GroupBy => "so Group by is empty",
            Emptied::Filter => "so its filter was left out",
            Emptied::Column => "so it isn't one of the columns",
            Emptied::Sort => "so the rows use the default sort",
        };
        format!("`{}` isn't a field of {source}, {consequence}.", self.field)
    }
}

/// A pasted link turned into panel settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkImport {
    /// The panel the link describes, without an id, title or place on the grid.
    pub panel: Panel,
    /// Fields the source doesn't have, each once per setting it fed.
    pub unknown: Vec<UnknownField>,
    /// Filters whose comparison a panel filter can't express.
    pub dropped_filters: Vec<LinkFilter>,
    /// The link's window. The panel follows the dashboard unless the editor keeps this.
    pub window: Window,
}

/// A panel reading `source` with every setting at its default.
pub(super) const fn blank_panel(source: PanelSource) -> Panel {
    Panel {
        id: String::new(),
        title: String::new(),
        source,
        chart: ChartKind::TimeSeries,
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

/// Bucket token for a width: `1m`, `5m`, `15m`, `1h`, `1d`, else `<n>s`.
pub(super) fn bucket_token(secs: u64) -> String {
    match secs {
        60 => "1m".into(),
        300 => "5m".into(),
        900 => "15m".into(),
        3_600 => "1h".into(),
        86_400 => "1d".into(),
        n => format!("{n}s"),
    }
}

/// Width of a panel bucket such as `5m` or `90s`.
pub(super) fn bucket_width(token: &str) -> Option<u64> {
    let token = token.trim();
    let split = token.find(|c: char| !c.is_ascii_digit())?;
    let (num, unit) = token.split_at(split);
    let n: u64 = num.parse().ok()?;
    let scale = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return None,
    };
    (n > 0).then_some(n * scale)
}

pub(super) const fn panel_op(op: LinkOp) -> Option<FilterOp> {
    match op {
        LinkOp::Eq => Some(FilterOp::Eq),
        LinkOp::Ne => Some(FilterOp::Ne),
        LinkOp::Contains => Some(FilterOp::Contains),
        LinkOp::Gt => Some(FilterOp::Gt),
        LinkOp::Lt => Some(FilterOp::Lt),
        _ => None,
    }
}

pub(super) const fn link_op(op: FilterOp) -> LinkOp {
    match op {
        FilterOp::Eq => LinkOp::Eq,
        FilterOp::Ne => LinkOp::Ne,
        FilterOp::Contains => LinkOp::Contains,
        FilterOp::Gt => LinkOp::Gt,
        FilterOp::Lt => LinkOp::Lt,
    }
}

/// "`amount ≥ 5` can't be a panel filter yet, so it was left out."
pub(super) fn dropped_filter_message(filter: &LinkFilter) -> String {
    let text = match filter.op {
        LinkOp::Empty => format!("{} is empty", filter.field),
        LinkOp::NotEmpty => format!("{} isn't empty", filter.field),
        op => {
            let symbol = match op {
                LinkOp::Ge => "≥",
                LinkOp::Le => "≤",
                LinkOp::Starts => "starts with",
                LinkOp::Ends => "ends with",
                other => other.token(),
            };
            format!("{} {symbol} {}", filter.field, filter.value)
        }
    };
    format!("`{text}` can't be a panel filter yet, so it was left out.")
}

pub(super) const fn window_from_link(window: QueryWindow) -> Window {
    match window {
        QueryWindow::Last(preset) => Window::Relative {
            secs: preset.secs().unsigned_abs(),
        },
        QueryWindow::Range { from, to } => Window::Absolute { from, to },
    }
}

pub(super) fn window_to_link(window: Window, now: DateTime<Utc>) -> QueryWindow {
    match window {
        Window::Relative { secs } => i64::try_from(secs)
            .ok()
            .and_then(WindowPreset::from_secs)
            .map_or_else(
                || QueryWindow::Range {
                    from: now - chrono::Duration::seconds(i64::try_from(secs).unwrap_or(0)),
                    to: now,
                },
                QueryWindow::Last,
            ),
        Window::Absolute { from, to } => QueryWindow::Range { from, to },
    }
}

/// Panel settings from a decoded link, checked against the source's `fields`.
///
/// `fields` is `None` when the schema isn't registered, and metric labels aren't schema
/// fields; in both cases nothing is checked. Otherwise each field the source lacks is left
/// out of the setting it fed and reported in [`LinkImport::unknown`].
pub(super) fn panel_from_link(link: &QueryLink, fields: Option<&[SourceField]>) -> LinkImport {
    let is_metric = matches!(link.source, QuerySource::Metric(_));
    let source = match &link.source {
        QuerySource::Metric(metric) => PanelSource::Metric {
            metric: metric.clone(),
        },
        QuerySource::Event(table) => PanelSource::EventTable {
            table: table.clone(),
        },
    };
    let known = |field: &str| {
        field == "ts" || is_metric || fields.is_none_or(|fs| fs.iter().any(|f| f.name == field))
    };
    let mut unknown: Vec<UnknownField> = Vec::new();
    let mut flag = |field: &str, emptied: Emptied| {
        if !unknown
            .iter()
            .any(|u| u.field == field && u.emptied == emptied)
        {
            unknown.push(UnknownField {
                field: field.to_string(),
                emptied,
            });
        }
    };

    let mut panel = blank_panel(source);
    panel.chart = match link.view {
        Some(QueryView::Series) | None => ChartKind::TimeSeries,
        Some(QueryView::Bar) => ChartKind::Bar,
        Some(QueryView::Pie) => ChartKind::Pie,
        Some(QueryView::Table | QueryView::Log) => ChartKind::Table,
        Some(QueryView::Stat) => ChartKind::SingleStat,
    };
    panel.measure = if is_metric {
        link.agg.clone()
    } else {
        match &link.measure {
            None => None,
            Some(QueryMeasure::Count) => Some("count".into()),
            Some(QueryMeasure::Sum(field)) if known(field) => Some(format!("sum({field})")),
            Some(QueryMeasure::Sum(field)) => {
                flag(field, Emptied::Measure);
                None
            }
        }
    };
    let group = if is_metric {
        link.split.as_ref().or(link.group.as_ref())
    } else {
        link.group.as_ref().or(link.split.as_ref())
    };
    if let Some(group) = group {
        if known(group) {
            panel.group_by = vec![group.clone()];
        } else {
            flag(group, Emptied::GroupBy);
        }
    }
    panel.bucket = link.bucket.map(|b| bucket_token(b.secs()));

    let mut dropped_filters = Vec::new();
    for filter in &link.filters {
        if !known(&filter.field) {
            flag(&filter.field, Emptied::Filter);
            continue;
        }
        match panel_op(filter.op) {
            Some(op) => panel.filters.push(DashboardFilter {
                field: filter.field.clone(),
                op,
                value: filter.value.clone(),
            }),
            None => dropped_filters.push(filter.clone()),
        }
    }

    if panel.chart == ChartKind::Table {
        let mut columns = Vec::new();
        for col in &link.cols {
            if known(col) {
                columns.push(col.clone());
            } else {
                flag(col, Emptied::Column);
            }
        }
        let sort = link.sort.as_ref().and_then(|s| {
            if known(&s.field) {
                Some(TableSort {
                    column: s.field.clone(),
                    descending: s.descending,
                })
            } else {
                flag(&s.field, Emptied::Sort);
                None
            }
        });
        panel.table = Some(TableOpts {
            columns,
            sort,
            limit: link.limit.unwrap_or(DEFAULT_ROWS),
        });
    }

    LinkImport {
        panel,
        unknown,
        dropped_filters,
        window: window_from_link(link.window),
    }
}

/// The query link for a panel: its source and settings, over its kept window or `window`.
/// Text panels have none.
pub(in crate::pages::dashboards) fn panel_to_link(
    panel: &Panel,
    window: Window,
    now: DateTime<Utc>,
) -> Option<QueryLink> {
    let mut link = match &panel.source {
        PanelSource::Metric { metric } => QueryLink::metric(metric.clone()),
        PanelSource::EventTable { table } => QueryLink::event(table.clone()),
        _ => return None,
    };
    let is_metric = matches!(panel.source, PanelSource::Metric { .. });
    link.window = window_to_link(panel.keep_window.unwrap_or(window), now);
    link.view = Some(match panel.chart {
        ChartKind::TimeSeries => QueryView::Series,
        ChartKind::Bar => QueryView::Bar,
        ChartKind::Pie => QueryView::Pie,
        ChartKind::Table => QueryView::Table,
        ChartKind::SingleStat => QueryView::Stat,
    });
    if is_metric {
        link.agg.clone_from(&panel.measure);
        link.split = panel.group_by.first().cloned();
    } else {
        link.measure = panel
            .measure
            .as_deref()
            .map(|m| match parse_measure(Some(m)) {
                (EventMeasure::Sum, Some(field)) => QueryMeasure::Sum(field),
                _ => QueryMeasure::Count,
            });
        link.group = panel.group_by.first().cloned();
    }
    link.bucket = panel
        .bucket
        .as_deref()
        .and_then(bucket_width)
        .and_then(QueryBucket::from_secs);
    link.filters = panel
        .filters
        .iter()
        .map(|f| LinkFilter {
            field: f.field.clone(),
            op: link_op(f.op),
            value: f.value.clone(),
        })
        .collect();
    if let Some(table) = &panel.table {
        link.cols.clone_from(&table.columns);
        link.sort = table.sort.as_ref().map(|s| QuerySort {
            field: s.column.clone(),
            descending: s.descending,
        });
        link.limit = Some(table.limit);
    }
    Some(link)
}

pub(super) fn chart_label(chart: ChartKind) -> &'static str {
    CHARTS
        .iter()
        .find(|(c, _)| *c == chart)
        .map_or("Time series", |(_, label)| label)
}

/// "Count", "Sum of amount_cents", "Average".
pub(super) fn measure_label(panel: &Panel) -> String {
    let is_metric = matches!(panel.source, PanelSource::Metric { .. });
    if is_metric && panel.measure.is_none() {
        return "Value".to_string();
    }
    let measure = panel.measure.as_deref().unwrap_or("count");
    if let Some((_, label)) = METRIC_MEASURES.iter().find(|(key, _)| *key == measure) {
        return (*label).to_string();
    }
    match parse_measure(Some(measure)) {
        (EventMeasure::Sum, Some(field)) => format!("Sum of {field}"),
        _ => "Count".to_string(),
    }
}

/// What a link filled in: "Event table checkout_event, Bar, Count, grouped by status".
pub(super) fn filled_in_summary(panel: &Panel) -> String {
    let (kind, name) = match &panel.source {
        PanelSource::Metric { metric } => ("Metric", metric.as_str()),
        PanelSource::EventTable { table } => ("Event table", table.as_str()),
        _ => ("Text", ""),
    };
    let group = panel
        .group_by
        .first()
        .map_or_else(Default::default, |g| format!(", grouped by {g}"));
    format!(
        "{kind} {name}, {}, {}{group}",
        chart_label(panel.chart),
        measure_label(panel)
    )
}

/// "last hour", "last 6 hours", or a range's two ends.
pub(super) fn window_text(window: Window) -> String {
    match window {
        Window::Relative { secs } => RangeChoice::PRESETS
            .iter()
            .find(|p| p.window() == window)
            .map_or_else(
                || format!("last {} minutes", secs / 60),
                |p| p.label().to_lowercase(),
            ),
        Window::Absolute { from, to } => format!(
            "{} to {}",
            from.format("%Y-%m-%d %H:%M"),
            to.format("%Y-%m-%d %H:%M")
        ),
    }
}

/// The Field error for pasted text, or `None` when it reads as a link (or is empty).
pub(super) fn link_error(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match decode(text) {
        Ok(_) => None,
        Err(QueryLinkError::NotSpectraLink) => Some(NOT_A_LINK.to_string()),
        Err(err) => Some(format!("That link can't be used: {err}.")),
    }
}

#[cfg(test)]
mod tests;
