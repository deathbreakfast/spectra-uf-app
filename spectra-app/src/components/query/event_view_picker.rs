use leptos::prelude::*;
use orbital::components::Caption1;
use orbital::primitives::{Flex, Select};
use spectra_core::EventExploreView;

const VIEWS: &[(EventExploreView, &str, &str, &str)] = &[
    (
        EventExploreView::EventLog,
        "Event log",
        "event_log",
        "spectra-event-view-event-log",
    ),
    (
        EventExploreView::TimeSeries,
        "Time series",
        "time_series",
        "spectra-event-view-time-series",
    ),
    (
        EventExploreView::BarChart,
        "Bar chart",
        "bar_chart",
        "spectra-event-view-bar-chart",
    ),
    (
        EventExploreView::PieChart,
        "Pie chart",
        "pie_chart",
        "spectra-event-view-pie-chart",
    ),
    (
        EventExploreView::Table,
        "Table",
        "table",
        "spectra-event-view-table",
    ),
];

fn view_value(v: EventExploreView) -> &'static str {
    let v = normalize_view(v);
    VIEWS
        .iter()
        .find(|(view, _, _, _)| *view == v)
        .map(|(_, _, value, _)| *value)
        .unwrap_or("event_log")
}

fn parse_view(value: &str) -> Option<EventExploreView> {
    VIEWS
        .iter()
        .find(|(_, _, v, _)| *v == value)
        .map(|(view, _, _, _)| *view)
}

/// Line chart is an alias of Time series (removed from the picker).
#[must_use]
pub fn normalize_view(view: EventExploreView) -> EventExploreView {
    match view {
        EventExploreView::LineChart => EventExploreView::TimeSeries,
        other => other,
    }
}

#[component]
pub fn EventViewPicker(
    /// Reactive signal for the current view selection.
    #[prop(into)]
    view: Signal<EventExploreView>,
    /// Callback invoked when the value changes.
    on_change: Callback<EventExploreView>,
) -> impl IntoView {
    let selected = RwSignal::new(view_value(view.get_untracked()).to_string());

    Effect::new(move |_| {
        selected.set(view_value(view.get()).to_string());
    });

    Effect::new(move |_| {
        let value = selected.get();
        if let Some(parsed) = parse_view(&value) {
            if parsed != normalize_view(view.get_untracked()) {
                on_change.run(parsed);
            }
        }
    });

    view! {
        <div id="spectra-event-view-picker" data-testid="spectra-event-view-picker">
            <Flex vertical=true>
                <Caption1>"View"</Caption1>
                <Select bind=selected>
                    {VIEWS.iter().map(|(_, label, value, test_id)| {
                        let label = *label;
                        let value = *value;
                        let test_id = *test_id;
                        view! {
                            <option value=value data-testid=test_id>{label}</option>
                        }
                    }).collect_view()}
                </Select>
            </Flex>
        </div>
    }
}
