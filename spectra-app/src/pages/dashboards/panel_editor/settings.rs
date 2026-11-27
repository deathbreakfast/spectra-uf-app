//! The editor's settings: one signal per control, loaded from a panel and built back into
//! one.

use leptos::prelude::*;
use spectra_backend::dashboards::{
    ChartKind, DashboardFilter, Layout, Panel, PanelSource, TableOpts, TableSort, Window,
};

use super::link::DEFAULT_ROWS;

/// Row limits a table panel offers.
pub(super) const ROW_CHOICES: [u32; 4] = [10, 25, 50, 100];
/// Bucket widths, with their labels (G16: a fixed list, not free text).
pub(super) const BUCKETS: [(&str, &str); 5] = [
    ("1m", "1 minute"),
    ("5m", "5 minutes"),
    ("15m", "15 minutes"),
    ("1h", "1 hour"),
    ("1d", "1 day"),
];
pub(super) const TAB_METRIC: &str = "metric";
pub(super) const TAB_EVENT: &str = "event";

/// One signal per setting. [`Settings::build`] turns them into a panel.
#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub(super) title: RwSignal<String>,
    pub(super) tab: RwSignal<String>,
    pub(super) source: RwSignal<String>,
    pub(super) source_metric: RwSignal<bool>,
    pub(super) source_text: RwSignal<String>,
    pub(super) source_selected: RwSignal<Vec<String>>,
    pub(super) chart: RwSignal<ChartKind>,
    pub(super) measure: RwSignal<String>,
    pub(super) bucket: RwSignal<String>,
    pub(super) group: RwSignal<String>,
    pub(super) filters: RwSignal<Vec<DashboardFilter>>,
    pub(super) columns: RwSignal<Vec<String>>,
    pub(super) sort: RwSignal<String>,
    pub(super) rows: RwSignal<String>,
    pub(super) keep_window: RwSignal<Option<Window>>,
}

impl Settings {
    pub(super) fn new() -> Self {
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

    pub(super) fn is_metric(self) -> bool {
        self.tab.with(|t| t == TAB_METRIC)
    }

    /// Point the settings at `name`, clearing what belonged to the old source.
    pub(super) fn set_source(self, name: &str, metric: bool) {
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

    pub(super) fn load(self, panel: &Panel) {
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
    pub(super) fn build(self, id: &str, layout: Layout) -> Option<Panel> {
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

pub(super) fn sort_key(column: &str, descending: bool) -> String {
    format!("{column}:{}", if descending { "desc" } else { "asc" })
}

pub(super) fn parse_sort_key(key: &str) -> Option<TableSort> {
    let (column, dir) = key.rsplit_once(':')?;
    (!column.is_empty()).then(|| TableSort {
        column: column.to_string(),
        descending: dir == "desc",
    })
}

impl Settings {
    /// Keep the source in step with the picker: a picked option becomes the source, and
    /// switching between Metric and Event table drops a source of the other kind.
    ///
    /// Orbital's Combobox and TabList only take signals, with no selection callback, so
    /// these two react to the signals instead.
    pub(super) fn follow_source_picks(self) {
        Effect::new(move |_| {
            let picked = self.source_selected.get().first().cloned();
            if let Some(name) = picked {
                if self.source.get_untracked() != name {
                    self.set_source(&name, self.is_metric());
                }
            }
        });
        Effect::new(move |_| {
            let metric = self.is_metric();
            if !self.source.get_untracked().is_empty()
                && self.source_metric.get_untracked() != metric
            {
                self.set_source("", metric);
            }
        });
    }
}
