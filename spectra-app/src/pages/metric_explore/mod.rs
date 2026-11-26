mod components;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use self::components::MetricExplorePanel;
use crate::explore_time::use_explore_link;

/// Metric explore canvas: query and chart a single metric over a selectable time range.
///
/// The whole query lives in the URL, so the address bar is always a link to this view.
/// Fills the shell under the breadcrumb — no page gutter or ContentContainer column.
#[component]
pub fn MetricExplorePage() -> impl IntoView {
    let params = use_params_map();
    let metric_name = Memo::new(move |_| params.with(|p| p.get("name").unwrap_or_default()));
    let link = use_explore_link();

    view! {
        <div data-testid="spectra-metric-explore-panel" style="display: flex; flex-direction: column; min-height: 0; flex: 1 1 auto; width: 100%;">
            <MetricExplorePanel metric_name=metric_name link=link />
        </div>
    }
}
