//! The panel editor: build a chart or table panel, or start from a pasted Spectra query link.
//!
//! Settings sit on the left and a live preview on the right. The preview runs the unsaved
//! panel through [`query_dashboard_panel`] on the dashboard's range, filters and variables,
//! so it shows what the panel will show. A pasted link is decoded in the browser, then
//! [`panel_source_access`] checks that the viewer can read its table and returns the fields
//! the link is checked against. Nothing is guessed: a field the table doesn't have is named
//! and left out.

use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::components::{
    Caption1, Caption1Strong, Checkbox, Combobox, ComboboxBind, ComboboxOption, Divider,
    DrawerBody, DrawerHeader, DrawerHeaderTitle, DrawerHeaderTitleAction, DrawerSize, EmptyState,
    FieldValidationState, FormHint, Grid, GridConfig, GridItem, GridItemConfig, OverlayDrawer,
    SpacingSize, Tab, TabList, Tag, TagGroup, TextareaAppearance, TextareaRule,
    TextareaRuleTrigger, ToggleButton,
};
use orbital::primitives::{
    use_breakpoint_up, Breakpoint, Button, ButtonAppearance, Card, CardContent, Field, Flex,
    FlexAlign, FlexJustify, FlexWrap, Input, InputBind, Link, Menu, MenuItem, MenuTrigger,
    MessageBar, MessageBarActions, MessageBarBody, MessageBarIntent, MessageBarLayout,
    MessageBarTitle, Select, SelectBind, Textarea, TextareaBind,
};
use spectra_backend::dashboards::view::{
    filter_op_key, filter_op_symbol, parse_filter, parse_measure, RangeChoice, ViewState,
    FILTER_OPS,
};
use spectra_backend::dashboards::{
    ChartKind, Dashboard, DashboardFilter, FilterOp, Layout, Panel, PanelSource, TableOpts,
    TableSort, Window,
};
use spectra_backend::query_url::{
    decode, FilterOp as LinkOp, LinkFilter, QueryBucket, QueryLink, QueryLinkError, QueryMeasure,
    QuerySort, QuerySource, QueryView, QueryWindow, WindowPreset,
};
use spectra_core::EventMeasure;

use super::edit::{add_panel, PanelEditorStart};
use super::panel::failure_reason;
use super::panels::{PanelBody, PanelBodySkeleton};
use super::toolbar::filter_label;
use crate::server::{
    panel_source_access, query_dashboard_panel, PanelData, PanelQuery, PanelSourceDetail,
    PanelSources, SourceField,
};

/// The Field error for text that isn't a Spectra query link.
pub(super) const NOT_A_LINK: &str = "That isn't a Spectra query link. Paste one from the event \
                                     explorer, the metric explorer or a saved query.";
/// Row limits a table panel offers.
const ROW_CHOICES: [u32; 4] = [10, 25, 50, 100];
/// Row limit for a new table panel, and for a link that sets none.
const DEFAULT_ROWS: u32 = 25;
/// Bucket widths, with their labels (G16: a fixed list, not free text).
const BUCKETS: [(&str, &str); 5] = [
    ("1m", "1 minute"),
    ("5m", "5 minutes"),
    ("15m", "15 minutes"),
    ("1h", "1 hour"),
    ("1d", "1 day"),
];
/// Metric aggregations, with their labels.
const METRIC_MEASURES: [(&str, &str); 5] = [
    ("avg", "Average"),
    ("max", "Max"),
    ("min", "Min"),
    ("sum", "Sum"),
    ("last", "Last"),
];
/// Chart choices, with their labels.
const CHARTS: [(ChartKind, &str); 5] = [
    (ChartKind::TimeSeries, "Time series"),
    (ChartKind::Bar, "Bar"),
    (ChartKind::Pie, "Pie"),
    (ChartKind::Table, "Table"),
    (ChartKind::SingleStat, "Single stat"),
];
const TAB_METRIC: &str = "metric";
const TAB_EVENT: &str = "event";

// ---------------------------------------------------------------------------------------
// Links and panels
// ---------------------------------------------------------------------------------------

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
const fn blank_panel(source: PanelSource) -> Panel {
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
fn bucket_token(secs: u64) -> String {
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
fn bucket_width(token: &str) -> Option<u64> {
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

const fn panel_op(op: LinkOp) -> Option<FilterOp> {
    match op {
        LinkOp::Eq => Some(FilterOp::Eq),
        LinkOp::Ne => Some(FilterOp::Ne),
        LinkOp::Contains => Some(FilterOp::Contains),
        LinkOp::Gt => Some(FilterOp::Gt),
        LinkOp::Lt => Some(FilterOp::Lt),
        _ => None,
    }
}

const fn link_op(op: FilterOp) -> LinkOp {
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

const fn window_from_link(window: QueryWindow) -> Window {
    match window {
        QueryWindow::Last(preset) => Window::Relative {
            secs: preset.secs().unsigned_abs(),
        },
        QueryWindow::Range { from, to } => Window::Absolute { from, to },
    }
}

fn window_to_link(window: Window, now: DateTime<Utc>) -> QueryWindow {
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
pub(super) fn panel_to_link(
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

// ---------------------------------------------------------------------------------------
// Wording
// ---------------------------------------------------------------------------------------

fn chart_label(chart: ChartKind) -> &'static str {
    CHARTS
        .iter()
        .find(|(c, _)| *c == chart)
        .map_or("Time series", |(_, label)| label)
}

/// "Count", "Sum of amount_cents", "Average".
fn measure_label(panel: &Panel) -> String {
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

/// The preview's heading: "Preview · last hour, region = eu-west, $host: all".
fn preview_heading(state: &ViewState, saved: &[DashboardFilter], dashboard: &Dashboard) -> String {
    let mut parts = vec![match state.range {
        RangeChoice::Custom { .. } => window_text(state.range.window()),
        preset => preset.label().to_lowercase(),
    }];
    parts.extend(state.filters_or(saved).iter().map(filter_label));
    let values = state.variable_values(&dashboard.variables);
    parts.extend(dashboard.variables.iter().map(|v| {
        values.get(&v.name).filter(|x| !x.is_empty()).map_or_else(
            || format!("${}: all", v.name),
            |value| format!("${} = {value}", v.name),
        )
    }));
    format!("Preview · {}", parts.join(", "))
}

/// "No rows in the last hour", or "No rows in this range" for a custom range.
const fn no_rows_text(range: RangeChoice) -> &'static str {
    match range {
        RangeChoice::LastHour => "No rows in the last hour",
        RangeChoice::Last6Hours => "No rows in the last 6 hours",
        RangeChoice::Last24Hours => "No rows in the last 24 hours",
        RangeChoice::Last7Days => "No rows in the last 7 days",
        RangeChoice::Custom { .. } => "No rows in this range",
    }
}

/// Whether loaded panel data has nothing to draw.
fn is_empty_data(data: &PanelData) -> bool {
    match data {
        PanelData::Series { series, .. } => series.iter().all(|s| s.points.is_empty()),
        PanelData::Slices { slices, .. } => slices.is_empty(),
        PanelData::Stat { headline } => headline.is_empty(),
        PanelData::Table { rows, .. } => rows.is_empty(),
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

// ---------------------------------------------------------------------------------------
// Editor state
// ---------------------------------------------------------------------------------------

/// What the link box last did.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LinkState {
    /// Nothing pasted, or text that isn't a link.
    Idle,
    /// Checking the link's source with the server.
    Checking,
    /// Settings were filled in from the link.
    Filled {
        summary: String,
        gaps: Vec<String>,
        window: Window,
    },
    /// The viewer can't read the link's source; nothing was filled in.
    Unreadable {
        source: String,
        permission: String,
        href: String,
    },
    /// The check itself failed.
    Failed(String),
}

/// One signal per setting. [`Settings::build`] turns them into a panel.
#[derive(Clone, Copy)]
struct Settings {
    title: RwSignal<String>,
    tab: RwSignal<String>,
    source: RwSignal<String>,
    source_metric: RwSignal<bool>,
    source_text: RwSignal<String>,
    source_selected: RwSignal<Vec<String>>,
    chart: RwSignal<ChartKind>,
    measure: RwSignal<String>,
    bucket: RwSignal<String>,
    group: RwSignal<String>,
    filters: RwSignal<Vec<DashboardFilter>>,
    columns: RwSignal<Vec<String>>,
    sort: RwSignal<String>,
    rows: RwSignal<String>,
    keep_window: RwSignal<Option<Window>>,
}

impl Settings {
    fn new() -> Self {
        Self {
            title: RwSignal::new(String::new()),
            tab: RwSignal::new(TAB_EVENT.to_string()),
            source: RwSignal::new(String::new()),
            source_metric: RwSignal::new(false),
            source_text: RwSignal::new(String::new()),
            source_selected: RwSignal::new(Vec::new()),
            chart: RwSignal::new(ChartKind::TimeSeries),
            measure: RwSignal::new(String::new()),
            bucket: RwSignal::new(String::new()),
            group: RwSignal::new(String::new()),
            filters: RwSignal::new(Vec::new()),
            columns: RwSignal::new(Vec::new()),
            sort: RwSignal::new(String::new()),
            rows: RwSignal::new(DEFAULT_ROWS.to_string()),
            keep_window: RwSignal::new(None),
        }
    }

    fn is_metric(self) -> bool {
        self.tab.with(|t| t == TAB_METRIC)
    }

    /// Point the settings at `name`, clearing what belonged to the old source.
    fn set_source(self, name: &str, metric: bool) {
        self.source.set(name.to_string());
        self.source_metric.set(metric);
        self.source_text.set(name.to_string());
        self.source_selected.set(if name.is_empty() {
            Vec::new()
        } else {
            vec![name.to_string()]
        });
        self.measure.set(String::new());
        self.group.set(String::new());
        self.columns.set(Vec::new());
        self.sort.set(String::new());
    }

    fn load(self, panel: &Panel) {
        let (name, metric) = match &panel.source {
            PanelSource::Metric { metric } => (metric.clone(), true),
            PanelSource::EventTable { table } => (table.clone(), false),
            _ => (String::new(), false),
        };
        self.title.set(panel.title.clone());
        self.tab
            .set(if metric { TAB_METRIC } else { TAB_EVENT }.to_string());
        self.set_source(&name, metric);
        self.chart.set(panel.chart);
        self.measure.set(panel.measure.clone().unwrap_or_default());
        self.bucket.set(panel.bucket.clone().unwrap_or_default());
        self.group
            .set(panel.group_by.first().cloned().unwrap_or_default());
        self.filters.set(panel.filters.clone());
        let table = panel.table.clone();
        self.columns.set(
            table
                .as_ref()
                .map_or_else(Default::default, |t| t.columns.clone()),
        );
        self.sort.set(
            table
                .as_ref()
                .and_then(|t| t.sort.as_ref())
                .map_or_else(Default::default, |s| sort_key(&s.column, s.descending)),
        );
        self.rows
            .set(table.as_ref().map_or(DEFAULT_ROWS, |t| t.limit).to_string());
        self.keep_window.set(panel.keep_window);
    }

    /// The panel the settings describe, or `None` before a source is picked.
    fn build(self, id: &str, layout: Layout) -> Option<Panel> {
        let name = self.source.get();
        if name.is_empty() {
            return None;
        }
        let source = if self.is_metric() {
            PanelSource::Metric { metric: name }
        } else {
            PanelSource::EventTable { table: name }
        };
        let chart = self.chart.get();
        let measure = self.measure.get();
        let bucket = self.bucket.get();
        let group = self.group.get();
        Some(Panel {
            id: id.to_string(),
            title: self.title.get().trim().to_string(),
            source,
            chart,
            measure: (!measure.is_empty()).then_some(measure),
            bucket: (!bucket.is_empty()).then_some(bucket),
            group_by: if group.is_empty() {
                Vec::new()
            } else {
                vec![group]
            },
            filters: self.filters.get(),
            table: (chart == ChartKind::Table).then(|| TableOpts {
                columns: self.columns.get(),
                sort: parse_sort_key(&self.sort.get()),
                limit: self.rows.get().parse().unwrap_or(DEFAULT_ROWS),
            }),
            layout,
            keep_window: self.keep_window.get(),
        })
    }
}

fn sort_key(column: &str, descending: bool) -> String {
    format!("{column}:{}", if descending { "desc" } else { "asc" })
}

fn parse_sort_key(key: &str) -> Option<TableSort> {
    let (column, dir) = key.rsplit_once(':')?;
    (!column.is_empty()).then(|| TableSort {
        column: column.to_string(),
        descending: dir == "desc",
    })
}

// ---------------------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------------------

/// The panel editor drawer. `start` says how it opened; the body is built fresh each time
/// the drawer opens.
#[component]
pub(super) fn PanelEditor(
    open: RwSignal<bool>,
    start: RwSignal<Option<PanelEditorStart>>,
    draft: RwSignal<Dashboard>,
    state: Memo<ViewState>,
) -> impl IntoView {
    let heading = move || {
        if matches!(start.get(), Some(PanelEditorStart::Edit(_))) {
            "Edit panel"
        } else {
            "Add panel"
        }
    };
    view! {
        <OverlayDrawer open=open close_on_esc=true size=DrawerSize::Large>
            <DrawerHeader>
                <DrawerHeaderTitle>
                    {heading}
                    <DrawerHeaderTitleAction slot>
                        <Button
                            appearance=ButtonAppearance::Subtle
                            icon=icondata::AiCloseOutlined
                            attr:aria-label="Close"
                            on_click=Callback::new(move |_| open.set(false))
                        />
                    </DrawerHeaderTitleAction>
                </DrawerHeaderTitle>
            </DrawerHeader>
            <DrawerBody>
                <Show when=move || open.get()>
                    {move || start.get_untracked().map(|start| view! {
                        <PanelEditorBody start=start open=open draft=draft state=state />
                    })}
                </Show>
            </DrawerBody>
        </OverlayDrawer>
    }
}

/// Focus the first textarea inside `wrapper`.
fn focus_textarea_in(wrapper: &leptos::web_sys::HtmlDivElement) {
    use wasm_bindgen::JsCast;
    if let Ok(Some(el)) = wrapper.query_selector("textarea") {
        if let Ok(el) = el.dyn_into::<leptos::web_sys::HtmlElement>() {
            // A refused focus leaves the box as it is.
            let _ = el.focus();
        }
    }
}

#[component]
fn PanelEditorBody(
    start: PanelEditorStart,
    open: RwSignal<bool>,
    draft: RwSignal<Dashboard>,
    state: Memo<ViewState>,
) -> impl IntoView {
    let existing = match &start {
        PanelEditorStart::Edit(id) => draft.with_untracked(|d| {
            d.sections
                .iter()
                .flat_map(|s| &s.panels)
                .find(|p| &p.id == id)
                .cloned()
        }),
        _ => None,
    };
    let from_link = start == PanelEditorStart::FromLink;
    let base = StoredValue::new(existing.clone());
    let dashboard = StoredValue::new(draft.get_untracked());
    let settings = Settings::new();
    if let Some(panel) = &existing {
        settings.load(panel);
    }
    let wide = use_breakpoint_up(Breakpoint::Md);

    // Readable sources for the picker, loaded once per opening.
    let sources = LocalResource::new(move || async move {
        panel_source_access(None)
            .await
            .map_or_else(|_| Vec::new(), |a| a.sources)
    });
    // Fields of the picked source; stale answers are dropped.
    let fields = RwSignal::new(None::<Vec<SourceField>>);
    let fields_seq = StoredValue::new(0u32);
    Effect::new(move |_| {
        let name = settings.source.get();
        fields.set(None);
        if name.is_empty() {
            return;
        }
        let n = fields_seq.get_value() + 1;
        fields_seq.set_value(n);
        leptos::task::spawn_local_scoped(async move {
            let result = panel_source_access(Some(name)).await;
            if fields_seq.get_value() != n {
                return;
            }
            if let Ok(PanelSources {
                detail: Some(PanelSourceDetail::Readable { fields: f }),
                ..
            }) = result
            {
                fields.set(f);
            }
        });
    });
    // A picked option becomes the source.
    Effect::new(move |_| {
        let picked = settings.source_selected.get().first().cloned();
        if let Some(name) = picked {
            if settings.source.get_untracked() != name {
                settings.set_source(&name, settings.is_metric());
            }
        }
    });
    // Switching between Metric and Event table drops a source of the other kind.
    Effect::new(move |_| {
        let metric = settings.is_metric();
        if !settings.source.get_untracked().is_empty()
            && settings.source_metric.get_untracked() != metric
        {
            settings.set_source("", metric);
        }
    });

    // The link box.
    let link_text = RwSignal::new(String::new());
    let link_state = RwSignal::new(LinkState::Idle);
    let keep_link_window = RwSignal::new(false);
    let link_seq = StoredValue::new(0u32);
    let link_box_ref = NodeRef::<leptos::html::Div>::new();
    if from_link {
        Effect::new(move |_| {
            if let Some(wrapper) = link_box_ref.get() {
                request_animation_frame(move || focus_textarea_in(&wrapper));
            }
        });
    }
    Effect::new(move |_| {
        let text = link_text.get();
        let text = text.trim();
        if text.is_empty() {
            link_state.set(LinkState::Idle);
            return;
        }
        // Not a link: the Field shows why, and nothing changes.
        let Ok(decoded) = decode(text) else {
            return;
        };
        let n = link_seq.get_value() + 1;
        link_seq.set_value(n);
        link_state.set(LinkState::Checking);
        let source = decoded.link.source.name().to_string();
        leptos::task::spawn_local_scoped(async move {
            let result = panel_source_access(Some(source.clone())).await;
            if link_seq.get_value() != n {
                return;
            }
            let next = match result {
                Ok(PanelSources {
                    detail: Some(PanelSourceDetail::Readable { fields: f }),
                    ..
                }) => {
                    let import = panel_from_link(&decoded.link, f.as_deref());
                    let mut panel = import.panel;
                    panel.title = settings.title.get_untracked();
                    let mut gaps: Vec<String> =
                        import.unknown.iter().map(|u| u.message(&source)).collect();
                    gaps.extend(import.dropped_filters.iter().map(dropped_filter_message));
                    settings.load(&panel);
                    keep_link_window.set(false);
                    LinkState::Filled {
                        summary: filled_in_summary(&panel),
                        gaps,
                        window: import.window,
                    }
                }
                Ok(PanelSources {
                    detail:
                        Some(PanelSourceDetail::Unreadable {
                            permission,
                            request_access_href,
                        }),
                    ..
                }) => LinkState::Unreadable {
                    source,
                    permission,
                    href: request_access_href,
                },
                Ok(_) => LinkState::Failed("the server didn't describe the link's source".into()),
                Err(err) => LinkState::Failed(failure_reason(&err)),
            };
            link_state.set(next);
        });
    });
    let on_keep_window = Callback::new(move |keep: bool| {
        if let LinkState::Filled { window, .. } = link_state.get_untracked() {
            settings.keep_window.set(keep.then_some(window));
        }
    });

    // The preview runs the unsaved panel; its title doesn't matter, so typing one never
    // refetches.
    let preview_query = Memo::new(move |_| {
        if matches!(link_state.get(), LinkState::Unreadable { .. }) {
            return None;
        }
        let mut panel = settings.build(
            "preview",
            blank_panel(PanelSource::Text {
                markdown: String::new(),
            })
            .layout,
        )?;
        panel.title = String::new();
        dashboard.with_value(|d| {
            state.with(|s| {
                Some(PanelQuery {
                    dashboard_id: d.id.clone(),
                    panel,
                    window: s.range.window(),
                    filters: s.filters_or(&d.filters).to_vec(),
                    variables: d.variables.clone(),
                    variable_values: s.variable_values(&d.variables),
                })
            })
        })
    });
    let preview = LocalResource::new(move || {
        let query = preview_query.get();
        async move {
            match query {
                Some(query) => Some(query_dashboard_panel(query).await),
                None => None,
            }
        }
    });

    let can_apply = Memo::new(move |_| {
        !matches!(link_state.get(), LinkState::Unreadable { .. })
            && !settings.source.with(String::is_empty)
    });
    let apply = Callback::new(move |_| {
        let old = base.get_value();
        let (id, layout) = old.as_ref().map_or(
            (
                String::new(),
                blank_panel(PanelSource::Text {
                    markdown: String::new(),
                })
                .layout,
            ),
            |p| (p.id.clone(), p.layout),
        );
        let Some(mut panel) = settings.build(&id, layout) else {
            return;
        };
        if matches!(link_state.get_untracked(), LinkState::Unreadable { .. }) {
            return;
        }
        if panel.title.is_empty() {
            panel.title = settings.source.get_untracked();
        }
        match old {
            Some(old) => draft.update(|d| {
                if let Some(slot) = d
                    .sections
                    .iter_mut()
                    .flat_map(|s| s.panels.iter_mut())
                    .find(|p| p.id == old.id)
                {
                    *slot = panel;
                }
            }),
            None => draft.update(|d| {
                add_panel(d, panel);
            }),
        }
        open.set(false);
    });

    let link_rule = TextareaRule::validator(move |value, _| {
        link_error(value).map_or(Ok(()), |message| Err(FieldValidationState::Error(message)))
    })
    .with_trigger(TextareaRuleTrigger::Input);

    let link_box = view! {
        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
            <Field label="Start from a query link">
                <div data-testid="spectra-panel-editor-link" node_ref=link_box_ref>
                    <Textarea
                        bind=TextareaBind { value: link_text.into(), rules: vec![link_rule], ..Default::default() }
                        appearance=TextareaAppearance {
                            placeholder: "Paste a Spectra link".to_string().into(),
                            rows: Signal::from(Some(3)),
                            ..Default::default()
                        }
                    />
                </div>
            </Field>
            <FormHint>
                "Links from the event explorer, the metric explorer and saved queries fill in everything below."
            </FormHint>
            {from_link.then(|| view! {
                <span data-testid="spectra-panel-editor-link-caption">
                    <Caption1>
                        "Copy one with the link icon next to an explorer's title, or take one a teammate sent you."
                    </Caption1>
                </span>
            })}
            <LinkStateBar link_state=link_state keep=keep_link_window on_keep=on_keep_window />
        </Flex>
    };

    let settings_pane = view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            {link_box}
            <Divider />
            <Caption1Strong>"Or set it up yourself"</Caption1Strong>
            <SourceSettings settings=settings sources=sources fields=fields />
        </Flex>
    };

    let preview_pane = view! {
        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
            <span data-testid="spectra-panel-editor-preview-heading">
                <Caption1Strong>
                    {move || dashboard.with_value(|d| state.with(|s| preview_heading(s, &d.filters, d)))}
                </Caption1Strong>
            </span>
            <div data-testid="spectra-panel-editor-preview">
                <Card>
                    <CardContent>
                        {move || {
                            let range = state.with(|s| s.range);
                            match preview.get() {
                                None => view! { <PanelBodySkeleton /> }.into_any(),
                                Some(None) => view! {
                                    <div data-testid="spectra-panel-editor-preview-blank">
                                        <EmptyState
                                            message="Nothing to preview yet"
                                            description="Paste a link or pick a source to see the panel here."
                                        />
                                    </div>
                                }
                                .into_any(),
                                Some(Some(Ok(data))) if is_empty_data(&data) => view! {
                                    <div data-testid="spectra-panel-editor-preview-empty">
                                        <EmptyState message=no_rows_text(range) />
                                    </div>
                                }
                                .into_any(),
                                Some(Some(Ok(data))) => {
                                    let panel = preview_query
                                        .get_untracked()
                                        .map(|q| q.panel);
                                    panel.map(|panel| {
                                        let chart = format!("{:?}", panel.chart);
                                        view! {
                                        <div data-testid="spectra-panel-editor-preview-body" data-chart=chart>
                                            <PanelBody
                                                panel=panel
                                                data=data
                                                as_table=RwSignal::new(false)
                                                explore=Memo::new(|_| None)
                                            />
                                        </div>
                                        }
                                    })
                                    .into_any()
                                }
                                Some(Some(Err(err))) => view! {
                                    <div data-testid="spectra-panel-editor-preview-failed">
                                        <MessageBar intent=MessageBarIntent::Error>
                                            <MessageBarBody>
                                                {format!(
                                                    "The preview couldn't run: {}. You can still save the panel.",
                                                    failure_reason(&err).trim_end_matches('.')
                                                )}
                                            </MessageBarBody>
                                        </MessageBar>
                                    </div>
                                }
                                .into_any(),
                            }
                        }}
                    </CardContent>
                </Card>
            </div>
        </Flex>
    };

    let actions = view! {
        <Flex justify=FlexJustify::End gap=SpacingSize::Size80.flex_gap()>
            <Button appearance=ButtonAppearance::Subtle on_click=Callback::new(move |_| open.set(false))>
                "Cancel"
            </Button>
            <span data-testid="spectra-panel-editor-apply">
                <Button
                    appearance=ButtonAppearance::Primary
                    disabled=Signal::derive(move || !can_apply.get())
                    on_click=apply
                >
                    "Apply"
                </Button>
            </span>
        </Flex>
    };

    // Settings and preview side by side from Md up; stacked on a phone.
    let body = if wide.get_untracked() {
        view! {
            <Grid config=GridConfig { cols: 12, x_gap: 24, y_gap: 24 }>
                <GridItem config=GridItemConfig::span(5)>{settings_pane}</GridItem>
                <GridItem config=GridItemConfig::span(7)>{preview_pane}</GridItem>
            </Grid>
        }
        .into_any()
    } else {
        view! {
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                {settings_pane}
                {preview_pane}
            </Flex>
        }
        .into_any()
    };

    view! {
        <div data-testid="spectra-panel-editor">
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                {body}
                {actions}
            </Flex>
        </div>
    }
}

/// The bar under the link box: filled in, filled in with gaps, unreadable, or failed.
#[component]
fn LinkStateBar(
    link_state: RwSignal<LinkState>,
    keep: RwSignal<bool>,
    on_keep: Callback<bool>,
) -> impl IntoView {
    move || match link_state.get() {
        LinkState::Idle | LinkState::Checking => ().into_any(),
        LinkState::Filled {
            summary,
            gaps,
            window,
        } => {
            let partial = !gaps.is_empty();
            view! {
                <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                    <div
                        data-testid="spectra-panel-editor-link-filled"
                        data-partial=partial.to_string()
                    >
                        <MessageBar
                            intent=if partial { MessageBarIntent::Warning } else { MessageBarIntent::Success }
                            layout=MessageBarLayout::Multiline
                        >
                            <MessageBarBody>
                                <MessageBarTitle>"Filled in from the link"</MessageBarTitle>
                                {summary}
                                {gaps.into_iter().map(|gap| view! {
                                    <div data-testid="spectra-panel-editor-link-gap">{gap}</div>
                                }).collect_view()}
                            </MessageBarBody>
                        </MessageBar>
                    </div>
                    <span data-testid="spectra-panel-editor-keep-window">
                        <Checkbox
                            checked=keep
                            label=format!("Keep the link's window instead ({})", window_text(window))
                            on_change=on_keep
                        />
                    </span>
                </Flex>
            }
            .into_any()
        }
        LinkState::Unreadable {
            source,
            permission,
            href,
        } => view! {
            <div data-testid="spectra-panel-editor-link-unreadable">
                <MessageBar intent=MessageBarIntent::Error layout=MessageBarLayout::Multiline>
                    <MessageBarBody>
                        <MessageBarTitle>{format!("You can't read {source}")}</MessageBarTitle>
                        {format!("Reading it needs {permission}. Nothing was filled in.")}
                    </MessageBarBody>
                    <MessageBarActions>
                        <span data-testid="spectra-panel-editor-request-access">
                            <Link href=href>"Request access"</Link>
                        </span>
                    </MessageBarActions>
                </MessageBar>
            </div>
        }
        .into_any(),
        LinkState::Failed(reason) => view! {
            <div data-testid="spectra-panel-editor-link-failed">
                <MessageBar intent=MessageBarIntent::Error>
                    <MessageBarBody>{format!("Couldn't check the link: {reason}")}</MessageBarBody>
                </MessageBar>
            </div>
        }
        .into_any(),
    }
}

/// "Or set it up yourself": title, source, chart, measure, bucket, group by, filters and the
/// table options.
#[component]
fn SourceSettings(
    settings: Settings,
    sources: LocalResource<Vec<crate::server::PanelSourceOption>>,
    fields: RwSignal<Option<Vec<SourceField>>>,
) -> impl IntoView {
    let field_names = move || {
        fields.with(|f| {
            f.as_ref().map_or_else(Default::default, |f| {
                f.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
            })
        })
    };
    let chart_buttons = CHARTS
        .iter()
        .map(|(kind, label)| {
            let kind = *kind;
            let pressed = RwSignal::new(settings.chart.get_untracked() == kind);
            Effect::new(move |_| {
                let on = settings.chart.get() == kind;
                if pressed.get_untracked() != on {
                    pressed.set(on);
                }
            });
            let shown = move || kind == ChartKind::TimeSeries
                || kind == ChartKind::SingleStat
                || !settings.is_metric();
            view! {
                <Show when=shown>
                    <span data-testid=format!("spectra-panel-editor-chart-{}", label.to_lowercase().replace(' ', "-"))>
                        <ToggleButton
                            pressed=pressed
                            on_click=move |_: bool| {
                                pressed.set(true);
                                settings.chart.set(kind);
                            }
                        >
                            {*label}
                        </ToggleButton>
                    </span>
                </Show>
            }
        })
        .collect_view();

    view! {
        <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
            <Field label="Title">
                <div data-testid="spectra-panel-editor-title">
                    <Input bind=InputBind::new(settings.title) />
                </div>
            </Field>
            <Field label="Source">
                <div data-testid="spectra-panel-editor-source-kind">
                    <TabList selected_value=settings.tab>
                        <Tab value=TAB_METRIC>"Metric"</Tab>
                        <Tab value=TAB_EVENT>"Event table"</Tab>
                    </TabList>
                </div>
            </Field>
            <Field label=Signal::derive(move || if settings.is_metric() { "Metric".to_string() } else { "Event table".to_string() })>
                <div data-testid="spectra-panel-editor-source">
                    {move || {
                        let metric = settings.is_metric();
                        let options: Vec<String> = sources
                            .get()
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|s| s.metric == metric)
                            .map(|s| s.name)
                            .collect();
                        view! {
                            <Combobox bind=ComboboxBind::new(settings.source_text, settings.source_selected)>
                                {options.into_iter().map(|name| view! {
                                    <ComboboxOption text=name />
                                }).collect_view()}
                            </Combobox>
                        }
                    }}
                </div>
            </Field>
            <FormHint>
                {move || if settings.is_metric() {
                    "Metrics you can't read aren't listed."
                } else {
                    "Tables you can't read aren't listed."
                }}
            </FormHint>
            <Field label="Chart">
                <Flex gap=SpacingSize::Size80.flex_gap() wrap=FlexWrap::Wrap>
                    {chart_buttons}
                </Flex>
            </Field>
            <Field label="Measure">
                <div data-testid="spectra-panel-editor-measure">
                    {move || {
                        let options: Vec<(String, String)> = if settings.is_metric() {
                            std::iter::once((String::new(), "Default".to_string()))
                                .chain(METRIC_MEASURES.iter().map(|(k, l)| ((*k).to_string(), (*l).to_string())))
                                .collect()
                        } else {
                            let numeric: Vec<String> = fields.with(|f| {
                                f.iter()
                                    .flatten()
                                    .filter(|x| x.numeric)
                                    .map(|x| x.name.clone())
                                    .collect()
                            });
                            std::iter::once((String::new(), "Count".to_string()))
                                .chain(numeric.into_iter().map(|n| (format!("sum({n})"), format!("Sum of {n}"))))
                                .collect()
                        };
                        let mut options = options;
                        let current = settings.measure.get_untracked();
                        if !current.is_empty() && !options.iter().any(|(k, _)| *k == current) {
                            let label = measure_label(&Panel {
                                measure: Some(current.clone()),
                                ..blank_panel(PanelSource::Text { markdown: String::new() })
                            });
                            options.push((current, label));
                        }
                        view! {
                            <Select bind=SelectBind::new(settings.measure)>
                                {options.into_iter().map(|(value, label)| view! {
                                    <option value=value>{label}</option>
                                }).collect_view()}
                            </Select>
                        }
                    }}
                </div>
            </Field>
            <Show when=move || settings.chart.get() != ChartKind::Table>
                <Field label="Bucket">
                    <div data-testid="spectra-panel-editor-bucket">
                        <Select bind=SelectBind::new(settings.bucket)>
                            <option value="">"Pick from the range"</option>
                            {BUCKETS.iter().map(|(value, label)| view! {
                                <option value=*value>{*label}</option>
                            }).collect_view()}
                        </Select>
                    </div>
                </Field>
            </Show>
            <Show when=move || matches!(settings.chart.get(), ChartKind::Bar | ChartKind::Pie) || settings.is_metric()>
                <Field label="Group by">
                    <div data-testid="spectra-panel-editor-group-by">
                        {move || {
                            let mut options: Vec<(String, String)> = vec![(String::new(), "(none)".to_string())];
                            fields.with(|f| {
                                for x in f.iter().flatten() {
                                    let label = if x.pii { format!("{} (masked)", x.name) } else { x.name.clone() };
                                    options.push((x.name.clone(), label));
                                }
                            });
                            let current = settings.group.get_untracked();
                            if !current.is_empty() && !options.iter().any(|(k, _)| *k == current) {
                                options.push((current.clone(), current));
                            }
                            view! {
                                <Select bind=SelectBind::new(settings.group)>
                                    {options.into_iter().map(|(value, label)| view! {
                                        <option value=value>{label}</option>
                                    }).collect_view()}
                                </Select>
                            }
                        }}
                    </div>
                </Field>
                <Caption1>
                    "Fields marked masked are personal data. Charts show stand-in labels for them, never the stored values."
                </Caption1>
            </Show>
            <PanelFilters filters=settings.filters field_names=Signal::derive(field_names) />
            <Show when=move || settings.chart.get() == ChartKind::Table>
                <TableSettings settings=settings field_names=Signal::derive(field_names) />
            </Show>
        </Flex>
    }
}

/// The panel's own filters as Tags, with Add filter.
#[component]
fn PanelFilters(
    filters: RwSignal<Vec<DashboardFilter>>,
    field_names: Signal<Vec<String>>,
) -> impl IntoView {
    let adding = RwSignal::new(false);
    let field = RwSignal::new(String::new());
    let op = RwSignal::new(filter_op_key(FilterOp::Eq).to_string());
    let value = RwSignal::new(String::new());
    let parsed = Memo::new(move |_| {
        parse_filter(&format!(
            "{}:{}:{}",
            field.get().trim(),
            op.get(),
            value.get()
        ))
    });
    let add = Callback::new(move |_| {
        if let Some(filter) = parsed.get_untracked() {
            filters.update(|f| f.push(filter));
            field.set(String::new());
            value.set(String::new());
            adding.set(false);
        }
    });
    view! {
        <Field label="Filters">
            <div data-testid="spectra-panel-editor-filters">
                <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                    <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                        <TagGroup
                            dismissible=true
                            on_dismiss=move |index: String| {
                                if let Ok(i) = index.parse::<usize>() {
                                    filters.update(|f| {
                                        if i < f.len() {
                                            f.remove(i);
                                        }
                                    });
                                }
                            }
                        >
                            {move || filters.get().into_iter().enumerate().map(|(i, f)| {
                                let label = filter_label(&f);
                                view! {
                                    <Tag value=i.to_string() dismiss_label=format!("Remove filter {label}")>
                                        {label.clone()}
                                    </Tag>
                                }
                            }).collect_view()}
                        </TagGroup>
                        <span data-testid="spectra-panel-editor-add-filter">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                icon=icondata::AiPlusOutlined
                                on_click=Callback::new(move |_| adding.update(|a| *a = !*a))
                            >
                                "Add filter"
                            </Button>
                        </span>
                    </Flex>
                    <Show when=move || adding.get()>
                        <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::End wrap=FlexWrap::Wrap>
                            <Field label="Field">
                                <div data-testid="spectra-panel-editor-filter-field">
                                    {move || {
                                        let names = field_names.get();
                                        if names.is_empty() {
                                            view! { <Input bind=InputBind::new(field) /> }.into_any()
                                        } else {
                                            view! {
                                                <Select bind=SelectBind::new(field)>
                                                    <option value="">"Pick a field"</option>
                                                    {names.into_iter().map(|n| view! {
                                                        <option value=n>{n.clone()}</option>
                                                    }).collect_view()}
                                                </Select>
                                            }
                                            .into_any()
                                        }
                                    }}
                                </div>
                            </Field>
                            <Field label="Condition">
                                <Select bind=SelectBind::new(op)>
                                    {FILTER_OPS.iter().map(|o| view! {
                                        <option value=filter_op_key(*o)>{filter_op_symbol(*o)}</option>
                                    }).collect_view()}
                                </Select>
                            </Field>
                            <Field label="Value">
                                <div data-testid="spectra-panel-editor-filter-value">
                                    <Input bind=InputBind::new(value) />
                                </div>
                            </Field>
                            <span data-testid="spectra-panel-editor-filter-submit">
                                <Button
                                    disabled=Signal::derive(move || parsed.get().is_none())
                                    on_click=add
                                >
                                    "Add"
                                </Button>
                            </span>
                        </Flex>
                    </Show>
                </Flex>
            </div>
        </Field>
        <FormHint>"The dashboard's own filters and $host apply on top of these."</FormHint>
    }
}

/// Columns, sort and row limit for a table panel.
#[component]
fn TableSettings(settings: Settings, field_names: Signal<Vec<String>>) -> impl IntoView {
    let all_fields = move || {
        let mut names = vec!["ts".to_string()];
        names.extend(field_names.get().into_iter().filter(|n| n != "ts"));
        names
    };
    view! {
        <Field label="Columns">
            <div data-testid="spectra-panel-editor-columns">
                <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                    <TagGroup
                        dismissible=true
                        on_dismiss=move |column: String| {
                            settings.columns.update(|c| c.retain(|x| *x != column));
                        }
                    >
                        {move || settings.columns.get().into_iter().map(|c| view! {
                            <Tag value=c.clone() dismiss_label=format!("Remove column {c}")>{c.clone()}</Tag>
                        }).collect_view()}
                    </TagGroup>
                    <Menu on_select=move |column: String| {
                        settings.columns.update(|c| {
                            if !c.contains(&column) {
                                c.push(column);
                            }
                        });
                    }>
                        <MenuTrigger slot>
                            <Button appearance=ButtonAppearance::Subtle icon=icondata::AiPlusOutlined>
                                "Add column"
                            </Button>
                        </MenuTrigger>
                        {move || all_fields().into_iter().map(|f| view! {
                            <MenuItem value=f.clone()>{f.clone()}</MenuItem>
                        }).collect_view()}
                    </Menu>
                </Flex>
            </div>
        </Field>
        <Field label="Sort">
            <div data-testid="spectra-panel-editor-sort">
                {move || {
                    let mut options = vec![(String::new(), "Newest first".to_string())];
                    for f in all_fields() {
                        options.push((sort_key(&f, true), format!("{f}, highest first")));
                        options.push((sort_key(&f, false), format!("{f}, lowest first")));
                    }
                    let current = settings.sort.get_untracked();
                    if !current.is_empty() && !options.iter().any(|(k, _)| *k == current) {
                        options.push((current.clone(), current));
                    }
                    view! {
                        <Select bind=SelectBind::new(settings.sort)>
                            {options.into_iter().map(|(value, label)| view! {
                                <option value=value>{label}</option>
                            }).collect_view()}
                        </Select>
                    }
                }}
            </div>
        </Field>
        <Field label="Rows">
            <div data-testid="spectra-panel-editor-rows">
                {move || {
                    let current = settings.rows.get_untracked();
                    let mut choices: Vec<String> = ROW_CHOICES.iter().map(u32::to_string).collect();
                    if !choices.contains(&current) {
                        choices.push(current);
                    }
                    view! {
                        <Select bind=SelectBind::new(settings.rows)>
                            {choices.into_iter().map(|n| view! {
                                <option value=n>{n.clone()}</option>
                            }).collect_view()}
                        </Select>
                    }
                }}
            </div>
        </Field>
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn field(name: &str, numeric: bool) -> SourceField {
        SourceField {
            name: name.into(),
            numeric,
            pii: false,
        }
    }

    fn checkout_fields() -> Vec<SourceField> {
        vec![
            field("status", false),
            field("amount_cents", true),
            field("region", false),
        ]
    }

    fn import(link: &str) -> LinkImport {
        let decoded = decode(link).expect("a Spectra link");
        panel_from_link(&decoded.link, Some(&checkout_fields()))
    }

    #[test]
    fn an_event_link_fills_in_every_setting() {
        let got = import(
            "https://spectra.example/spectra/schema/checkout_event/explore?window=6h&view=bar\
             &measure=sum:amount_cents&group=status&bucket=5m&f=region:eq:eu-west",
        );
        assert_eq!(got.unknown, Vec::<UnknownField>::new());
        assert_eq!(got.dropped_filters, Vec::<LinkFilter>::new());
        assert_eq!(
            got.panel.source,
            PanelSource::EventTable {
                table: "checkout_event".into()
            }
        );
        assert_eq!(got.panel.chart, ChartKind::Bar);
        assert_eq!(got.panel.measure.as_deref(), Some("sum(amount_cents)"));
        assert_eq!(got.panel.group_by, vec!["status".to_string()]);
        assert_eq!(got.panel.bucket.as_deref(), Some("5m"));
        assert_eq!(got.panel.filters.len(), 1);
        assert_eq!(got.panel.keep_window, None);
        assert_eq!(got.window, Window::Relative { secs: 6 * 3_600 });
        assert_eq!(
            filled_in_summary(&got.panel),
            "Event table checkout_event, Bar, Sum of amount_cents, grouped by status"
        );
    }

    #[test]
    fn an_unknown_group_field_empties_group_by_and_is_named() {
        let got = import("/spectra/schema/checkout_event/explore?view=bar&group=customer_name");
        assert_eq!(got.panel.group_by, Vec::<String>::new());
        assert_eq!(
            got.unknown[0].message("checkout_event"),
            "`customer_name` isn't a field of checkout_event, so Group by is empty."
        );
    }

    #[test]
    fn an_unknown_sum_field_falls_back_to_count() {
        let got = import("/spectra/schema/checkout_event/explore?measure=sum:refund_cents");
        assert_eq!(got.panel.measure, None);
        assert_eq!(got.unknown[0].emptied, Emptied::Measure);
    }

    #[test]
    fn filters_a_panel_cant_express_are_dropped_and_named() {
        let got = import("/spectra/schema/checkout_event/explore?f=amount_cents:ge:500");
        assert_eq!(got.panel.filters, Vec::<DashboardFilter>::new());
        assert_eq!(
            dropped_filter_message(&got.dropped_filters[0]),
            "`amount_cents ≥ 500` can't be a panel filter yet, so it was left out."
        );
    }

    #[test]
    fn a_metric_link_keeps_its_aggregation_and_split() {
        let decoded =
            decode("/spectra/metric/api_latency_ms/explore?agg=max&bucket=1h&split=route").unwrap();
        let got = panel_from_link(&decoded.link, Some(&[]));
        assert_eq!(got.unknown, Vec::<UnknownField>::new());
        assert_eq!(got.panel.measure.as_deref(), Some("max"));
        assert_eq!(got.panel.group_by, vec!["route".to_string()]);
        assert_eq!(got.panel.bucket.as_deref(), Some("1h"));
        assert_eq!(measure_label(&got.panel), "Max");
    }

    #[test]
    fn a_log_view_becomes_a_table_with_its_columns() {
        let got = import(
            "/spectra/schema/checkout_event/explore?view=log&cols=ts,status,nickname&sort=ts:desc&limit=50",
        );
        let table = got.panel.table.expect("table options");
        assert_eq!(got.panel.chart, ChartKind::Table);
        assert_eq!(table.columns, vec!["ts".to_string(), "status".to_string()]);
        assert_eq!(table.limit, 50);
        assert!(table.sort.expect("sort").descending);
        assert_eq!(got.unknown[0].emptied, Emptied::Column);
    }

    #[test]
    fn a_range_window_is_kept_as_a_range() {
        let got = import(
            "/spectra/schema/checkout_event/explore?from=2026-11-01T00:00:00Z&to=2026-11-02T00:00:00Z",
        );
        assert!(matches!(got.window, Window::Absolute { .. }));
        assert!(window_text(got.window).starts_with("2026-11-01 00:00 to"));
    }

    #[test]
    fn unregistered_sources_are_not_checked() {
        let decoded = decode("/spectra/schema/e2e.empty_events/explore?group=anything").unwrap();
        let got = panel_from_link(&decoded.link, None);
        assert_eq!(got.unknown, Vec::<UnknownField>::new());
        assert_eq!(got.panel.group_by, vec!["anything".to_string()]);
    }

    #[test]
    fn panel_links_round_trip_for_every_chart() {
        let now = Utc.with_ymd_and_hms(2026, 11, 26, 12, 0, 0).unwrap();
        for (chart, _) in CHARTS {
            let mut panel = blank_panel(PanelSource::EventTable {
                table: "checkout_event".into(),
            });
            panel.chart = chart;
            panel.measure = Some("sum(amount_cents)".into());
            panel.bucket = Some("15m".into());
            panel.group_by = vec!["status".into()];
            panel.filters = vec![DashboardFilter {
                field: "region".into(),
                op: FilterOp::Ne,
                value: "us-east".into(),
            }];
            if chart == ChartKind::Table {
                panel.table = Some(TableOpts {
                    columns: vec!["ts".into(), "status".into()],
                    sort: Some(TableSort {
                        column: "amount_cents".into(),
                        descending: true,
                    }),
                    limit: 100,
                });
            }
            let link = panel_to_link(&panel, Window::Relative { secs: 3_600 }, now)
                .expect("a link")
                .encode();
            let got = import(&link);
            assert!(got.unknown.is_empty(), "{chart:?}: {:?}", got.unknown);
            assert_eq!(got.panel, panel, "{chart:?} via {link}");
        }
    }

    #[test]
    fn metric_panels_round_trip() {
        let now = Utc::now();
        let mut panel = blank_panel(PanelSource::Metric {
            metric: "api_latency_ms".into(),
        });
        panel.measure = Some("avg".into());
        panel.group_by = vec!["route".into()];
        let link = panel_to_link(&panel, Window::Relative { secs: 6 * 3_600 }, now).unwrap();
        let got = panel_from_link(&decode(&link.encode()).unwrap().link, Some(&[]));
        assert_eq!(got.panel, panel);
    }

    #[test]
    fn a_kept_window_goes_into_the_link() {
        let mut panel = blank_panel(PanelSource::EventTable {
            table: "checkout_event".into(),
        });
        panel.keep_window = Some(Window::Relative { secs: 7 * 86_400 });
        let link = panel_to_link(&panel, Window::Relative { secs: 3_600 }, Utc::now()).unwrap();
        assert_eq!(link.window, QueryWindow::Last(WindowPreset::Days7));
    }

    #[test]
    fn text_panels_have_no_query_link() {
        let panel = blank_panel(PanelSource::Text {
            markdown: "hi".into(),
        });
        assert!(panel_to_link(&panel, Window::Relative { secs: 3_600 }, Utc::now()).is_none());
    }

    #[test]
    fn pasted_text_that_isnt_a_link_gets_the_field_error() {
        assert_eq!(
            link_error("https://wiki.example.com/Checkout_runbook").as_deref(),
            Some(NOT_A_LINK)
        );
        assert_eq!(link_error("  "), None);
        assert_eq!(link_error("/spectra/schema/checkout_event/explore"), None);
    }

    #[test]
    fn empty_results_and_wording() {
        assert!(is_empty_data(&PanelData::Slices {
            slices: Vec::new(),
            stand_in_field: None
        }));
        assert_eq!(
            no_rows_text(RangeChoice::LastHour),
            "No rows in the last hour"
        );
        assert_eq!(window_text(Window::Relative { secs: 3_600 }), "last hour");
        assert_eq!(bucket_width("15m"), Some(900));
        assert_eq!(bucket_token(90), "90s");
    }
}
