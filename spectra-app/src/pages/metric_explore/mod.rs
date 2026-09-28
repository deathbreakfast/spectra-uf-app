mod components;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use self::components::MetricExplorePanel;

/// Metric explore canvas: query and chart a single metric over a selectable time range.
///
/// Fills the shell under the breadcrumb — no page gutter or ContentContainer column.
#[component]
pub fn MetricExplorePage() -> impl IntoView {
    let params = use_params_map();
    let metric_name = Memo::new(move |_| params.with(|p| p.get("name").unwrap_or_default()));
    let (range_secs, set_range_secs) = signal(3600i64);

    view! {
        <div data-testid="spectra-metric-explore-panel" style="display: flex; flex-direction: column; min-height: 0; flex: 1 1 auto; width: 100%;">
            <MetricExplorePanel
                metric_name=metric_name
                range_secs=range_secs
                set_range_secs=set_range_secs
            />
        </div>
    }
}
