use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::primitives::{DateTimeRange, MessageBar, MessageBarIntent};
use orbital_datatable::DataTableFilter;
use serde::{Deserialize, Serialize};
use spectra_core::{
    EventAggregateRequest, EventAggregateResult, EventAggregationSpec, EventExploreView,
    EventQuery, EventQueryResult, GridFilterModel, GridPaginationModel, GridSortDirection,
    GridSortItem,
};

use crate::components::explore::{EventExploreSkeleton, EventExploreViewport};
use crate::components::query::{normalize_view, PermissionDeniedState};
use crate::components::tables::data_table_filter_to_grid;
use crate::explore_time::{default_explore_datetime_range, range_from_datetime_range};
use crate::server::{
    get_schema_metadata, query_event_aggregate, query_events, server_fn_is_permission_denied,
};

use super::event_toolbar::EventToolbar;

#[derive(Clone, Serialize, Deserialize)]
enum ExploreData {
    Rows(EventQueryResult),
    Aggregate(EventAggregateResult),
}

#[component]
pub fn EventExplorePanel(
    /// Reactive signal for the table identifier.
    table: Memo<String>,
    /// Bound datetime range.
    range: RwSignal<Option<DateTimeRange>>,
    /// View Select value (pending until See).
    picker_view: RwSignal<EventExploreView>,
    /// Applied view driving the query + viewport.
    applied_view: RwSignal<EventExploreView>,
    /// Two-way signal holding the aggregation mode to apply.
    aggregation: RwSignal<EventAggregationSpec>,
) -> impl IntoView {
    let (last_refreshed, set_last_refreshed) = signal(None::<DateTime<Utc>>);
    let table_filter = RwSignal::new(None::<DataTableFilter>);
    let grid_filter = RwSignal::new(GridFilterModel::default());

    let schema_res = Resource::new(
        move || table.get(),
        |name| async move { get_schema_metadata(name).await },
    );

    let field_options = Memo::new(move |_| {
        schema_res
            .get()
            .and_then(|r| r.ok().flatten())
            .map(|d| {
                d.fields
                    .into_iter()
                    .map(|f| f.name)
                    .filter(|n| n != "ts")
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });

    let column_fields = Memo::new(move |_| {
        schema_res
            .get()
            .and_then(|r| r.ok().flatten())
            .map(|d| {
                let mut cols = vec![("ts".to_string(), "ts".to_string())];
                cols.extend(d.fields.into_iter().map(|f| (f.name.clone(), f.name)));
                cols
            })
            .unwrap_or_else(|| vec![("ts".to_string(), "ts".to_string())])
    });

    let explore_res = Resource::new(
        move || {
            let range_key = range.get().map(|r| {
                let n = r.normalized();
                (n.start.instant().timestamp(), n.end.instant().timestamp())
            });
            let agg = aggregation.get();
            let filter = grid_filter.get();
            let filter_key = serde_json::to_string(&filter).unwrap_or_default();
            let agg_key = serde_json::to_string(&agg).unwrap_or_default();
            (
                table.get(),
                range_key,
                normalize_view(applied_view.get()),
                agg_key,
                filter_key,
            )
        },
        |(table, range_key, view, agg_key, filter_key)| async move {
            let filter: GridFilterModel = serde_json::from_str(&filter_key).unwrap_or_default();
            let mut agg: EventAggregationSpec =
                serde_json::from_str(&agg_key).unwrap_or(EventAggregationSpec {
                    measure: spectra_core::EventMeasure::Count,
                    measure_field: None,
                    time_bucket_secs: Some(3600),
                    group_by_field: None,
                    row_fields: Vec::new(),
                    pivot_field: None,
                });
            let range = match range_key {
                Some((start_ts, end_ts)) => {
                    use orbital::primitives::{DatetimeTimezone, OrbitalDateTime};
                    // Keep Local tags aligned with DateTimeRangePicker appearance.
                    DateTimeRange::new(
                        OrbitalDateTime::from_instant(
                            DateTime::<Utc>::from_timestamp(start_ts, 0).unwrap_or_else(Utc::now),
                            DatetimeTimezone::Local,
                        ),
                        OrbitalDateTime::from_instant(
                            DateTime::<Utc>::from_timestamp(end_ts, 0).unwrap_or_else(Utc::now),
                            DatetimeTimezone::Local,
                        ),
                    )
                }
                None => default_explore_datetime_range(),
            };
            let (start, end) = range_from_datetime_range(&range);
            let view = normalize_view(view);
            match view {
                EventExploreView::EventLog => {
                    let rows = query_events(EventQuery {
                        table: table.clone(),
                        start,
                        end,
                        partition: None,
                        pagination: GridPaginationModel::default(),
                        sort: vec![GridSortItem {
                            field: "ts".into(),
                            sort: GridSortDirection::Desc,
                        }],
                        filter,
                    })
                    .await?;
                    Ok::<ExploreData, ServerFnError>(ExploreData::Rows(rows))
                }
                other => {
                    if !matches!(
                        other,
                        EventExploreView::PieChart | EventExploreView::BarChart
                    ) {
                        agg.group_by_field = None;
                    }
                    let result = query_event_aggregate(EventAggregateRequest {
                        table: table.clone(),
                        start,
                        end,
                        partition: None,
                        filter,
                        view: other,
                        aggregation: agg,
                    })
                    .await?;
                    Ok::<ExploreData, ServerFnError>(ExploreData::Aggregate(result))
                }
            }
        },
    );

    Effect::new(move |_| {
        if matches!(explore_res.get(), Some(Ok(_))) {
            set_last_refreshed.set(Some(Utc::now()));
        }
    });

    // Selecting Event log applies immediately; chart views wait for See.
    Effect::new(move |_| {
        let picked = normalize_view(picker_view.get());
        if picked == EventExploreView::EventLog {
            applied_view.set(EventExploreView::EventLog);
        }
    });

    let on_filter_change = Callback::new(move |f: DataTableFilter| {
        grid_filter.set(data_table_filter_to_grid(&f));
    });

    view! {
        <div
            data-testid="spectra-event-explore-layout"
            style="display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--orb-space-inline-md); min-height: 0; flex: 1 1 auto; width: 100%;"
        >
            <EventToolbar
                range=range
                picker_view=Signal::derive(move || picker_view.get())
                on_picker_view=Callback::new(move |v| picker_view.set(normalize_view(v)))
                on_see=Callback::new(move |_| {
                    applied_view.set(normalize_view(picker_view.get()));
                })
                aggregation=aggregation
                field_options=Signal::derive(move || field_options.get())
                on_refresh=Callback::new(move |_| explore_res.refetch())
                last_refreshed=Signal::derive(move || last_refreshed.get())
            />
            <div
                data-testid="spectra-event-explore-viewport-pane"
                style="flex: 1 1 28rem; min-width: min(100%, 18rem); min-height: 0; align-self: stretch;"
            >
            <Transition fallback=move || view! { <EventExploreSkeleton view=applied_view.get() /> }>
                {move || match explore_res.get() {
                    Some(Ok(ExploreData::Rows(rows))) => view! {
                        <EventExploreViewport
                            view=EventExploreView::EventLog
                            row_result=Some(rows)
                            aggregate_result=None
                            group_by_field=None
                            row_fields=Vec::new()
                            column_fields=column_fields.get()
                            table_filter=table_filter
                            on_filter_change=on_filter_change
                        />
                    }.into_any(),
                    Some(Ok(ExploreData::Aggregate(agg))) => view! {
                        <EventExploreViewport
                            view=applied_view.get()
                            row_result=None
                            aggregate_result=Some(agg)
                            group_by_field=aggregation.get().group_by_field.clone()
                            row_fields=aggregation.get().row_fields.clone()
                            column_fields=column_fields.get()
                            table_filter=table_filter
                            on_filter_change=on_filter_change
                        />
                    }.into_any(),
                    Some(Err(e)) if server_fn_is_permission_denied(&e) => {
                        view! { <PermissionDeniedState /> }.into_any()
                    }
                    Some(Err(e)) => view! {
                        <MessageBar intent=MessageBarIntent::Error>{e.to_string()}</MessageBar>
                    }.into_any(),
                    None => view! { <EventExploreSkeleton view=applied_view.get() /> }.into_any(),
                }}
            </Transition>
            </div>
        </div>
    }
}
