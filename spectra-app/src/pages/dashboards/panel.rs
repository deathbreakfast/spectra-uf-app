use leptos::prelude::*;
use orbital::components::{EmptyState, SpacingSize, Title3};
use orbital::primitives::{
    Card, CardContent, CardHeader, Flex, Link, MessageBar, MessageBarIntent,
};
use spectra_backend::DashboardPanel;
use spectra_core::MetricsQuery;

use crate::components::charts::{MetricStatCardRow, MetricTimeSeriesChart};
use crate::components::query::{ChartSkeleton, PermissionDeniedState};
use crate::explore_time::range_from_secs;
use crate::server::{query_metrics, server_fn_is_permission_denied};

/// One metric chart card with a link into metric explore.
#[component]
pub fn DashboardPanelView(
    /// Preset panel to render.
    panel: DashboardPanel,
) -> impl IntoView {
    let query_res = Resource::new(
        move || (panel.metric, panel.range_secs),
        |(metric, secs)| async move {
            let (start, end) = range_from_secs(secs);
            query_metrics(MetricsQuery {
                metric: metric.to_string(),
                start,
                end,
                step_secs: Some(60),
                label_matchers: Vec::new(),
            })
            .await
        },
    );
    let test_id = format!("spectra-dashboard-panel-{}", panel.metric);
    let explore_test_id = format!("spectra-dashboard-panel-{}-explore", panel.metric);

    view! {
        <div data-testid=test_id>
            <Card>
                <CardHeader>
                    <Title3>{panel.title}</Title3>
                </CardHeader>
                <CardContent>
                    <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                        <Transition fallback=ChartSkeleton>
                            {move || match query_res.get() {
                                Some(Ok(data)) => {
                                    if data.series.iter().all(|s| s.points.is_empty()) {
                                        view! {
                                            <EmptyState
                                                message="No series data"
                                                description="Nothing recorded for this metric in the last hour."
                                            />
                                        }.into_any()
                                    } else {
                                        view! {
                                            <MetricStatCardRow headline=data.headline />
                                            <MetricTimeSeriesChart series=data.series />
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
                        <span data-testid=explore_test_id>
                            <Link href=panel.explore_path()>"Open in explore"</Link>
                        </span>
                    </Flex>
                </CardContent>
            </Card>
        </div>
    }
}
