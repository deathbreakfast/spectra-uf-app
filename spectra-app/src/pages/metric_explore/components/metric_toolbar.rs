use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::components::{Caption1, Title3};
use orbital::primitives::{Button, ButtonAppearance, Flex};

use crate::components::query::{QueryToolbarMaterial, TimeRangePicker};

#[component]
pub fn MetricToolbar(
    /// Reactive signal for the range secs.
    #[prop(into)]
    range_secs: Signal<i64>,
    /// Callback invoked when range occurs.
    on_range: Callback<i64>,
    /// Callback invoked when the operator requests a refresh.
    on_refresh: Callback<()>,
    /// Last successful refresh time (UTC), when known.
    #[prop(into)]
    last_refreshed: Signal<Option<DateTime<Utc>>>,
) -> impl IntoView {
    view! {
        <QueryToolbarMaterial>
            <Flex>
                <div id="spectra-metric-time-range">
                    <TimeRangePicker selected_secs=range_secs on_change=on_range />
                </div>
                <span data-testid="spectra-refresh-data">
                    <Button appearance=ButtonAppearance::Secondary on:click=move |_| on_refresh.run(())>
                        "Refresh Data"
                    </Button>
                </span>
                <Caption1>
                    {move || match last_refreshed.get() {
                        Some(ts) => format!("Last refreshed {}", ts.format("%H:%M:%S UTC")),
                        None => String::new(),
                    }}
                </Caption1>
            </Flex>
        </QueryToolbarMaterial>
    }
}

/// Title row for metric explore (Refresh stays on the toolbar).
#[component]
pub fn MetricExploreTitle(
    /// Metric name for the heading.
    #[prop(into)]
    metric_name: Signal<String>,
) -> impl IntoView {
    view! {
        <Title3>{move || metric_name.get()}</Title3>
    }
}
