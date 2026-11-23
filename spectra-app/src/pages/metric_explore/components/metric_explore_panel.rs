use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::components::EmptyState;
use orbital::primitives::{MessageBar, MessageBarIntent};
use spectra_backend::query_url::resolve;
use spectra_backend::{QueryBucket, QueryLink, QueryWindow, WindowPreset};
use spectra_core::MetricsQuery;

use crate::components::charts::{MetricStatCardRow, MetricTimeSeriesChart};
use crate::components::query::{ChartSkeleton, PermissionDeniedState};
use crate::explore_time::ExploreLinkState;
use crate::server::{get_schema_metadata, query_metrics, server_fn_is_permission_denied};

use super::metric_toolbar::{MetricExploreTitle, MetricToolbar};

/// Chart step when a link has no bucket: one minute.
const DEFAULT_STEP_SECS: u64 = 60;

#[component]
pub fn MetricExplorePanel(
    /// Reactive signal for the metric name.
    metric_name: Memo<String>,
    /// The explore query in the URL, which the time range writes to.
    link: ExploreLinkState,
) -> impl IntoView {
    let (last_refreshed, set_last_refreshed) = signal(None::<DateTime<Utc>>);
    let query = Memo::new(move |_| {
        link.current
            .with(|c| c.as_ref().map(|c| c.link.clone()))
            .unwrap_or_else(|| QueryLink::metric(metric_name.get()))
    });
    let window = Memo::new(move |_| query.with(|q| q.window));
    let step_secs =
        Memo::new(move |_| query.with(|q| q.bucket.map_or(DEFAULT_STEP_SECS, QueryBucket::secs)));

    let query_res = Resource::new(
        move || (metric_name.get(), window.get(), step_secs.get()),
        |(metric, window, step)| async move {
            let (start, end) = window.bounds(Utc::now());
            query_metrics(MetricsQuery {
                metric,
                start,
                end,
                step_secs: Some(step),
                label_matchers: Vec::new(),
            })
            .await
        },
    );

    // Only fetched when the link names a field to check.
    let names_fields = Memo::new(move |_| query.with(|q| !q.referenced_fields().is_empty()));
    let schema_res = Resource::new(
        move || (metric_name.get(), names_fields.get()),
        |(name, names_fields)| async move {
            if names_fields {
                get_schema_metadata(name).await
            } else {
                Ok(None)
            }
        },
    );

    // What the link asks for that this explorer can't show as asked.
    let notices = move || {
        let mut out: Vec<String> = Vec::new();
        if let Some(err) = link
            .current
            .with(|c| c.as_ref().and_then(|c| c.error.clone()))
        {
            out.push(format!(
                "{err}, so this opens {} with its default settings.",
                metric_name.get()
            ));
        }
        let q = query.get();
        if q.agg.is_some() || q.split.is_some() {
            out.push(
                "This explorer doesn't apply the link's aggregation or split yet. They stay in the link."
                    .into(),
            );
        }
        if let Some(Ok(Some(schema))) = schema_res.get() {
            let unknown = resolve(&q, &schema);
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

    Effect::new(move |_| {
        if matches!(query_res.get(), Some(Ok(_))) {
            set_last_refreshed.set(Some(Utc::now()));
        }
    });

    view! {
        <div style="display: flex; flex-direction: column; min-height: 0; flex: 1 1 auto; gap: 0;">
            <MetricExploreTitle metric_name=Signal::derive(move || metric_name.get()) />
            <MetricToolbar
                // A fixed from/to range matches no preset, so none is highlighted.
                range_secs=Signal::derive(move || match window.get() {
                    QueryWindow::Last(preset) => preset.secs(),
                    QueryWindow::Range { .. } => 0,
                })
                on_range=Callback::new(move |secs| {
                    if let Some(preset) = WindowPreset::from_secs(secs) {
                        link.update(|l| l.window = QueryWindow::Last(preset));
                    }
                })
                on_refresh=Callback::new(move |()| query_res.refetch())
                last_refreshed=Signal::derive(move || last_refreshed.get())
            />
            <Transition fallback=|| ()>{notices}</Transition>
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
