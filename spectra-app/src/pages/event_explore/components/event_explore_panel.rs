use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::primitives::{MessageBar, MessageBarIntent};
use orbital_datatable::DataTableFilter;
use serde::{Deserialize, Serialize};
use spectra_backend::query_url::resolve;
use spectra_backend::{LinkFilter, QueryLink, QueryView};
use spectra_core::{
    EventAggregateRequest, EventAggregateResult, EventAggregationSpec, EventExploreView,
    EventQueryResult,
};

use crate::components::explore::{EventExploreSkeleton, EventExploreViewport};
use crate::components::query::PermissionDeniedState;
use crate::components::tables::{
    build_event_log_fetcher, data_table_filter_to_grid, EventLogScope,
};
use crate::explore_time::ExploreLinkState;
use crate::server::{
    get_schema_metadata, query_event_aggregate, query_events, server_fn_is_permission_denied,
};

use super::event_toolbar::EventToolbar;
use super::link_filters::{link_filters_to_table, table_filter_to_link};
use super::link_query::{aggregation_for, explore_view};

#[derive(Clone, Serialize, Deserialize)]
enum ExploreData {
    /// First Event log page; the grid fetches later pages for the same table and range.
    Rows {
        table: String,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        first_page: EventQueryResult,
    },
    Aggregate(EventAggregateResult),
}

fn seed_filter(filters: &[LinkFilter]) -> Option<DataTableFilter> {
    (!filters.is_empty()).then(|| link_filters_to_table(filters))
}

#[component]
pub fn EventExplorePanel(
    /// Reactive signal for the table identifier.
    table: Memo<String>,
    /// The explore query in the URL, which every control writes to.
    link: ExploreLinkState,
) -> impl IntoView {
    let (last_refreshed, set_last_refreshed) = signal(None::<DateTime<Utc>>);
    let query = Memo::new(move |_| {
        link.current
            .with(|c| c.as_ref().map(|c| c.link.clone()))
            .unwrap_or_else(|| QueryLink::event(table.get()))
    });
    let applied_view = Memo::new(move |_| query.with(|q| explore_view(q.view)));

    // The grid's filter starts from the link and follows it when the URL changes from outside.
    let table_filter = RwSignal::new(query.with_untracked(|q| seed_filter(&q.filters)));
    Effect::new(move |prev: Option<u64>| {
        let rev = link.seed_rev.get();
        if prev.is_some() {
            table_filter.set(query.with_untracked(|q| seed_filter(&q.filters)));
        }
        rev
    });

    let schema_res = Resource::new(
        move || table.get(),
        |name| async move { get_schema_metadata(name).await },
    );

    let field_options = Memo::new(move |_| {
        schema_res
            .get()
            .and_then(|r| r.ok().flatten())
            .map_or_else(Vec::new, |d| {
                d.fields
                    .into_iter()
                    .map(|f| f.name)
                    .filter(|n| n != "ts")
                    .collect::<Vec<_>>()
            })
    });

    let column_fields = Memo::new(move |_| {
        schema_res.get().and_then(|r| r.ok().flatten()).map_or_else(
            || vec![("ts".to_string(), "ts".to_string())],
            |d| {
                let mut cols = vec![("ts".to_string(), "ts".to_string())];
                cols.extend(d.fields.into_iter().map(|f| (f.name.clone(), f.name)));
                cols
            },
        )
    });

    let explore_res = Resource::new(
        move || {
            let q = query.get();
            let view = explore_view(q.view);
            let agg_key = serde_json::to_string(&aggregation_for(&q, view)).unwrap_or_default();
            // The Event log grid filters on the server itself; only charts refetch here.
            let filters = if view == EventExploreView::EventLog {
                Vec::new()
            } else {
                q.filters
            };
            (table.get(), q.window, view, agg_key, filters)
        },
        |(table, window, view, agg_key, filters)| async move {
            let agg: EventAggregationSpec = serde_json::from_str(&agg_key)
                .unwrap_or_else(|_| aggregation_for(&QueryLink::event(table.clone()), view));
            let (start, end) = window.bounds(Utc::now());
            match view {
                EventExploreView::EventLog => {
                    let scope = EventLogScope {
                        table: table.clone(),
                        start,
                        end,
                    };
                    let first_page = query_events(scope.first_page_query()).await?;
                    Ok::<ExploreData, ServerFnError>(ExploreData::Rows {
                        table,
                        start,
                        end,
                        first_page,
                    })
                }
                other => {
                    let filter = data_table_filter_to_grid(&link_filters_to_table(&filters));
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

    let on_filter_change = Callback::new(move |f: DataTableFilter| {
        let filters = table_filter_to_link(&f);
        link.update(|l| l.filters = filters);
    });

    // The query as opened (or as last changed from outside). Fields picked in the controls
    // come from the field lists, so only fields that arrived with a link are checked.
    let opened = Memo::new(move |_| {
        link.seed_rev.track();
        query.get_untracked()
    });

    // What the link asks for that this explorer can't show as asked.
    let notices = move || {
        let mut out: Vec<String> = Vec::new();
        if let Some(err) = link
            .current
            .with(|c| c.as_ref().and_then(|c| c.error.clone()))
        {
            out.push(format!(
                "{err}, so this opens {} with its default settings.",
                table.get()
            ));
        }
        let q = query.get();
        if q.view == Some(QueryView::Stat) {
            out.push(
                "The event explorer has no Stat view, so this link opens on the Event log.".into(),
            );
        }
        if let Some(Ok(Some(schema))) = schema_res.get() {
            let still_used = q.referenced_fields();
            let unknown: Vec<String> = opened
                .with(|o| resolve(o, &schema))
                .into_iter()
                .filter(|f| still_used.contains(f))
                .collect();
            if !unknown.is_empty() {
                out.push(format!(
                    "This link uses fields {} doesn't have: {}.",
                    schema.table_or_metric,
                    unknown.join(", ")
                ));
            }
        }
        out.into_iter()
            .map(|text| {
                view! {
                    <div data-testid="spectra-query-link-notice">
                        <MessageBar intent=MessageBarIntent::Warning>{text}</MessageBar>
                    </div>
                }
            })
            .collect_view()
    };

    view! {
        <div
            data-testid="spectra-event-explore-layout"
            style="display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--orb-space-inline-md); min-height: 0; flex: 1 1 auto; width: 100%;"
        >
            {move || {
                // Rebuild the controls only when the URL changes from outside.
                link.seed_rev.track();
                let seed = query.get_untracked();
                view! {
                    <EventToolbar
                        link=link
                        seed=seed
                        field_options=Signal::derive(move || field_options.get())
                        on_refresh=Callback::new(move |()| explore_res.refetch())
                        last_refreshed=Signal::derive(move || last_refreshed.get())
                    />
                }
            }}
            <div
                data-testid="spectra-event-explore-viewport-pane"
                style="flex: 1 1 28rem; min-width: min(100%, 18rem); min-height: 0; align-self: stretch;"
            >
            <Transition fallback=|| ()>{notices}</Transition>
            <Transition fallback=move || view! { <EventExploreSkeleton view=applied_view.get() /> }>
                {move || match explore_res.get() {
                    Some(Ok(ExploreData::Rows { table, start, end, first_page })) => {
                        let fetcher = build_event_log_fetcher(
                            EventLogScope { table, start, end },
                            Some(first_page.clone()),
                        );
                        view! {
                            <EventExploreViewport
                                view=EventExploreView::EventLog
                                row_result=Some(first_page)
                                aggregate_result=None
                                group_by_field=None
                                row_fields=Vec::new()
                                column_fields=column_fields.get()
                                table_filter=table_filter
                                on_filter_change=on_filter_change
                                event_log_fetcher=Some(fetcher)
                            />
                        }.into_any()
                    }
                    Some(Ok(ExploreData::Aggregate(agg))) => {
                        // The result already reflects the query; read it untracked so a
                        // query edit waits for its own refetch.
                        let view = applied_view.get_untracked();
                        let spec = query.with_untracked(|q| aggregation_for(q, view));
                        view! {
                            <EventExploreViewport
                                view=view
                                row_result=None
                                aggregate_result=Some(agg)
                                group_by_field=spec.group_by_field
                                row_fields=spec.row_fields
                                column_fields=column_fields.get()
                                table_filter=table_filter
                                on_filter_change=on_filter_change
                            />
                        }.into_any()
                    }
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
