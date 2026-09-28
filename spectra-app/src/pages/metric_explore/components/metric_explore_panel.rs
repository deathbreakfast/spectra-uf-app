use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::components::EmptyState;
use orbital::primitives::{MessageBar, MessageBarIntent};
use spectra_core::MetricsQuery;

use crate::components::charts::{MetricStatCardRow, MetricTimeSeriesChart};
use crate::components::query::{ChartSkeleton, PermissionDeniedState};
use crate::explore_time::range_from_secs;
use crate::server::{query_metrics, server_fn_is_permission_denied};

use super::metric_toolbar::{MetricExploreTitle, MetricToolbar};

#[component]
pub fn MetricExplorePanel(
    /// Reactive signal for the metric name.
    metric_name: Memo<String>,
    /// Reactive signal for the range secs.
    range_secs: ReadSignal<i64>,
    /// Setter used to update the range secs.
    set_range_secs: WriteSignal<i64>,
) -> impl IntoView {
    let (last_refreshed, set_last_refreshed) = signal(None::<DateTime<Utc>>);

    let query_res = Resource::new(
        move || (metric_name.get(), range_secs.get()),
        |(metric, secs)| async move {
            let (start, end) = range_from_secs(secs);
            query_metrics(MetricsQuery {
                metric,
                start,
                end,
                step_secs: Some(60),
                label_matchers: Vec::new(),
            })
            .await
        },
    );

    Effect::new(move |_| {
        if matches!(query_res.get(), Some(Ok(_))) {
            set_last_refreshed.set(Some(Utc::now()));
        }
    });

    view! {
        <div style="display: flex; flex-direction: column; min-height: 0; flex: 1 1 auto; gap: 0;">
            <MetricExploreTitle metric_name=Signal::derive(move || metric_name.get()) />
            <MetricToolbar
                range_secs=Signal::derive(move || range_secs.get())
                on_range=Callback::new(move |s| set_range_secs.set(s))
                on_refresh=Callback::new(move |_| query_res.refetch())
                last_refreshed=Signal::derive(move || last_refreshed.get())
            />
            <Transition fallback=ChartSkeleton>
                {move || match query_res.get() {
                    Some(Ok(data)) => {
                        let empty = data.series.iter().all(|s| s.points.is_empty());
                        if empty {
                            view! {
                                <div id="spectra-metric-results" data-testid="spectra-metric-empty-state">
                                    <EmptyState
                                        message="No series data"
                                        description="Nothing to chart for this metric and time range."
                                    />
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div id="spectra-metric-results">
                                    <MetricStatCardRow headline=data.headline />
                                    <MetricTimeSeriesChart series=data.series />
                                </div>
                            }.into_any()
                        }
                    }
                    Some(Err(e)) if server_fn_is_permission_denied(&e) => {
                        view! { <PermissionDeniedState /> }.into_any()
                    }
                    Some(Err(e)) => view! {
                        <MessageBar intent=MessageBarIntent::Error>{e.to_string()}</MessageBar>
                    }.into_any(),
                    None => view! { <ChartSkeleton /> }.into_any(),
                }}
            </Transition>
        </div>
    }
}
