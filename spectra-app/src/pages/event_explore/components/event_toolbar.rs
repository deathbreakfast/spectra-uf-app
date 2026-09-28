use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::components::Caption1;
use orbital::primitives::{
    Button, ButtonAppearance, DateTimeRange, DateTimeRangePicker, Flex, FlexAlign, FlexGap,
};

use spectra_core::{EventAggregationSpec, EventExploreView};

use crate::components::query::{
    normalize_view, EventAggregationBar, EventViewPicker, QueryToolbarMaterial,
};

#[component]
pub fn EventToolbar(
    /// Bound datetime range for explore queries.
    range: RwSignal<Option<DateTimeRange>>,
    /// Picker selection (may differ from applied until See).
    #[prop(into)]
    picker_view: Signal<EventExploreView>,
    /// Callback when the view Select changes.
    on_picker_view: Callback<EventExploreView>,
    /// Apply picker view (See) or refresh Event log.
    on_see: Callback<()>,
    /// Two-way aggregation spec.
    aggregation: RwSignal<EventAggregationSpec>,
    /// Schema field names for group-by / sum Selects.
    #[prop(into)]
    field_options: Signal<Vec<String>>,
    /// Manual refresh of the current applied view.
    on_refresh: Callback<()>,
    /// Last successful refresh time (UTC), when known.
    #[prop(into)]
    last_refreshed: Signal<Option<DateTime<Utc>>>,
) -> impl IntoView {
    view! {
        <div
            data-testid="spectra-event-explore-controls"
            style="flex: 0 1 20rem; min-width: min(100%, 16rem); max-width: 24rem;"
        >
            <QueryToolbarMaterial>
                <Flex vertical=true gap=FlexGap::Small align=FlexAlign::Stretch>
                    <div id="spectra-event-time-range" data-testid="spectra-event-time-range">
                        <DateTimeRangePicker bind=range />
                    </div>
                    <EventViewPicker view=picker_view on_change=on_picker_view />
                    <span data-testid="spectra-event-see">
                        <Button
                            appearance=ButtonAppearance::Primary
                            on:click=move |_| on_see.run(())
                        >
                            {move || {
                                if normalize_view(picker_view.get()) == EventExploreView::EventLog {
                                    "Show rows"
                                } else {
                                    "See"
                                }
                            }}
                        </Button>
                    </span>
                    <EventAggregationBar
                        view=picker_view
                        spec=aggregation
                        field_options=field_options
                    />
                    <span data-testid="spectra-refresh-data">
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on:click=move |_| on_refresh.run(())
                        >
                            "Refresh Data"
                        </Button>
                    </span>
                    <Caption1>
                        {move || {
                            last_refreshed.get().map_or_else(String::new, |ts| {
                                format!("Last refreshed {}", ts.format("%H:%M:%S UTC"))
                            })
                        }}
                    </Caption1>
                </Flex>
            </QueryToolbarMaterial>
        </div>
    }
}
