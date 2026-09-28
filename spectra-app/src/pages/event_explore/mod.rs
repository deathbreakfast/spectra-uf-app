mod components;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use orbital::components::Title3;
use spectra_core::{EventAggregationSpec, EventExploreView, EventMeasure};

use self::components::EventExplorePanel;
use crate::explore_time::default_explore_datetime_range;

/// Event explore canvas: query, filter, and visualize logged events for a schema.
///
/// Fills the shell under the breadcrumb — no page gutter or ContentContainer column.
#[component]
pub fn EventExplorePage() -> impl IntoView {
    let params = use_params_map();
    let table = Memo::new(move |_| params.with(|p| p.get("name").unwrap_or_default()));
    let range = RwSignal::new(Some(default_explore_datetime_range()));
    let picker_view = RwSignal::new(EventExploreView::EventLog);
    let applied_view = RwSignal::new(EventExploreView::EventLog);
    let aggregation = RwSignal::new(EventAggregationSpec {
        measure: EventMeasure::Count,
        measure_field: None,
        time_bucket_secs: Some(3600),
        group_by_field: None,
        row_fields: Vec::new(),
        pivot_field: None,
    });

    view! {
        <div data-testid="spectra-event-explore-panel" style="display: flex; flex-direction: column; min-height: 0; flex: 1 1 auto; width: 100%;">
            <Title3>"Explore event rows"</Title3>
            <EventExplorePanel
                table=table
                range=range
                picker_view=picker_view
                applied_view=applied_view
                aggregation=aggregation
            />
        </div>
    }
}
