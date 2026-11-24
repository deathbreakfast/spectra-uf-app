//! Panel bodies: charts from the explorer, table panels, text and locked panels.

mod locked;
mod table;
mod text;

pub(super) use locked::LockedPanel;
pub(super) use text::TextPanel;

use std::collections::HashMap;

use leptos::prelude::*;
use orbital::components::{Caption1, EmptyState, SpacingSize};
use orbital::primitives::{Flex, Skeleton, SkeletonItem};
use orbital_datatable::{
    DataTable, DataTableColumnDef, DataTableFeatures, DataTableRowModel, DataTableSource,
    DataTableToolbarConfig, PagingMode,
};
use spectra_backend::dashboards::view::stand_in_noun;
use spectra_backend::dashboards::{ChartKind, Panel, PanelSource};
use spectra_core::{SliceDto, StatCardDto, TimeSeriesDto};

use self::table::TablePanel;
use crate::components::charts::{
    EventAggregateStatRow, EventBarChart, EventPieChart, EventTimeSeriesChart, MetricStatCardRow,
    MetricTimeSeriesChart,
};
use crate::server::PanelData;

/// Placeholder while a panel body loads; the Card and its header are already drawn.
#[component]
pub(in crate::pages::dashboards) fn PanelBodySkeleton() -> impl IntoView {
    view! {
        <div data-testid="spectra-dashboard-panel-loading">
            <Skeleton>
                <SkeletonItem width="100%".to_string() height="12rem".to_string() />
            </Skeleton>
        </div>
    }
}

/// A series' label set as text, such as `status=500`, or `value` when it has none.
fn series_label(series: &TimeSeriesDto) -> String {
    match &series.labels {
        serde_json::Value::Object(map) if !map.is_empty() => map
            .iter()
            .map(|(k, v)| match v {
                serde_json::Value::String(s) => format!("{k}={s}"),
                other => format!("{k}={other}"),
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => "value".to_string(),
    }
}

/// Columns and rows for View as table.
pub(super) fn data_as_rows(
    data: &PanelData,
) -> (Vec<(&'static str, &'static str)>, Vec<Vec<String>>) {
    let stats = |headline: &[StatCardDto]| {
        headline
            .iter()
            .map(|c| vec![c.label.clone(), c.value.clone()])
            .collect()
    };
    match data {
        PanelData::Series { series, .. } => (
            vec![("time", "Time"), ("series", "Series"), ("value", "Value")],
            series
                .iter()
                .flat_map(|s| {
                    let label = series_label(s);
                    s.points.iter().map(move |p| {
                        vec![
                            p.ts.format("%Y-%m-%d %H:%M").to_string(),
                            label.clone(),
                            p.value.to_string(),
                        ]
                    })
                })
                .collect(),
        ),
        PanelData::Slices { slices, .. } => (
            vec![("label", "Label"), ("value", "Value")],
            slices
                .iter()
                .map(|s| vec![s.label.clone(), s.value.to_string()])
                .collect(),
        ),
        PanelData::Stat { headline } => {
            (vec![("label", "Stat"), ("value", "Value")], stats(headline))
        }
        PanelData::Table { .. } => (Vec::new(), Vec::new()),
    }
}

#[component]
fn DataAsTable(data: PanelData) -> impl IntoView {
    let (columns, rows) = data_as_rows(&data);
    let rows: Vec<DataTableRowModel> = rows
        .into_iter()
        .enumerate()
        .map(|(i, cells)| {
            let map: HashMap<String, String> = columns
                .iter()
                .map(|(field, _)| (*field).to_string())
                .zip(cells)
                .collect();
            DataTableRowModel::from_text_cells(i.to_string(), map)
        })
        .collect();
    view! {
        <div data-testid="spectra-dashboard-panel-as-table">
            <DataTable
                data_source=DataTableSource::Client(RwSignal::new(rows))
                paging=PagingMode::Paged
                features=DataTableFeatures::empty()
                toolbar_config=DataTableToolbarConfig {
                    quick_search: false,
                    filter_panel: false,
                    column_picker: false,
                    pivot: false,
                    export_menu: false,
                }
                columns=columns.into_iter().map(|(f, h)| DataTableColumnDef::new(f, h)).collect()
            />
        </div>
    }
}

fn slices_view(chart: ChartKind, slices: Vec<SliceDto>, stand_in_field: Option<String>) -> AnyView {
    if slices.is_empty() {
        return view! {
            <EmptyState message="Nothing to chart" description="No events in this range." />
        }
        .into_any();
    }
    let note = stand_in_field.map(|field| {
        format!(
            "{field} is personal data, so each value shows as a stand-in such as {} 1.",
            stand_in_noun(&field)
        )
    });
    view! {
        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
            {if chart == ChartKind::Pie {
                view! { <EventPieChart slices=slices /> }.into_any()
            } else {
                view! { <EventBarChart slices=slices /> }.into_any()
            }}
            {note.map(|note| view! {
                <span data-testid="spectra-dashboard-panel-stand-ins">
                    <Caption1>{note}</Caption1>
                </span>
            })}
        </Flex>
    }
    .into_any()
}

/// A loaded panel's body, as a chart or as View as table.
#[component]
pub(super) fn PanelBody(
    panel: Panel,
    data: PanelData,
    as_table: RwSignal<bool>,
    explore: Memo<Option<String>>,
) -> impl IntoView {
    let is_metric = matches!(panel.source, PanelSource::Metric { .. });
    let chart = panel.chart;
    move || {
        if as_table.get() && !matches!(data, PanelData::Table { .. }) {
            return view! { <DataAsTable data=data.clone() /> }.into_any();
        }
        match data.clone() {
            PanelData::Series { series, .. } if is_metric => {
                view! { <MetricTimeSeriesChart series=series /> }.into_any()
            }
            PanelData::Series { series, .. } => {
                view! { <EventTimeSeriesChart series=series /> }.into_any()
            }
            PanelData::Slices {
                slices,
                stand_in_field,
            } => slices_view(chart, slices, stand_in_field),
            PanelData::Stat { headline } if is_metric => {
                view! { <MetricStatCardRow headline=headline /> }.into_any()
            }
            PanelData::Stat { headline } => {
                view! { <EventAggregateStatRow headline=headline /> }.into_any()
            }
            PanelData::Table {
                columns,
                rows,
                masked,
                total,
            } => view! {
                <TablePanel columns=columns rows=rows masked=masked total=total explore=explore />
            }
            .into_any(),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use spectra_core::MetricPointDto;

    use super::*;

    #[test]
    fn series_rows_name_each_series() {
        let data = PanelData::Series {
            series: vec![TimeSeriesDto {
                labels: json!({"status": "500"}),
                points: vec![MetricPointDto {
                    ts: Utc::now(),
                    value: 3.0,
                }],
            }],
            headline: Vec::new(),
        };
        let (columns, rows) = data_as_rows(&data);
        assert_eq!(columns.len(), 3);
        assert_eq!(rows[0][1], "status=500");
        assert_eq!(rows[0][2], "3");
    }

    #[test]
    fn unlabelled_series_reads_as_value() {
        let s = TimeSeriesDto {
            labels: json!({}),
            points: Vec::new(),
        };
        assert_eq!(series_label(&s), "value");
    }

    #[test]
    fn slices_and_stats_become_label_value_rows() {
        let slices = PanelData::Slices {
            slices: vec![SliceDto {
                label: "info".into(),
                value: 2.0,
            }],
            stand_in_field: None,
        };
        assert_eq!(
            data_as_rows(&slices).1,
            vec![vec!["info".to_string(), "2".to_string()]]
        );
        let stat = PanelData::Stat {
            headline: vec![StatCardDto {
                label: "Total".into(),
                value: "7".into(),
            }],
        };
        assert_eq!(data_as_rows(&stat).0[0].1, "Stat");
    }
}
