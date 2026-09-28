use leptos::prelude::*;
use orbital::components::EmptyState;
use orbital::primitives::{Flex, FlexGap};
use spectra_core::{EventAggregateResult, EventExploreView};

use crate::components::charts::{
    EventAggregateStatRow, EventBarChart, EventPieChart, EventTimeSeriesChart,
};

#[component]
pub fn EventSeriesPanel(
    /// Result data to render.
    result: EventAggregateResult,
) -> impl IntoView {
    match result {
        EventAggregateResult::TimeSeries { series, headline } => view! {
            <Flex vertical=true gap=FlexGap::Medium full_width=true fill=true>
                <EventAggregateStatRow headline=headline />
                <div style="flex: 1 1 auto; min-height: 0; display: flex; flex-direction: column;">
                    <EventTimeSeriesChart series=series />
                </div>
            </Flex>
        }
        .into_any(),
        EventAggregateResult::Slices { .. } | EventAggregateResult::Pivot { .. } => view! {
            <div data-testid="spectra-event-empty-state">
                <EmptyState
                    message="No series data"
                    description="This view expects a time series. Switch to Bar, Pie, or Table for grouped results."
                />
            </div>
        }
        .into_any(),
    }
}

#[component]
pub fn EventPiePanel(
    /// Current chart view.
    view: EventExploreView,
    /// Result data to render.
    result: EventAggregateResult,
    /// Aggregation group-by field from the toolbar (empty means user must choose).
    group_by_field: Option<String>,
) -> impl IntoView {
    let group_by_missing = group_by_field
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if group_by_missing {
        return view! {
            <div data-testid="spectra-event-empty-state">
                <EmptyState
                    message="Choose a group-by field"
                    description="Bar and pie charts need a field name to group event rows."
                />
            </div>
        }
        .into_any();
    }

    match result {
        EventAggregateResult::Slices { slices, headline } => {
            if slices.is_empty() {
                return view! {
                    <div data-testid="spectra-event-empty-state">
                        <EmptyState
                            message="No slice data"
                            description="Nothing to chart for this group-by field and time range."
                        />
                    </div>
                }
                .into_any();
            }
            view! {
                <Flex vertical=true gap=FlexGap::Medium full_width=true fill=true>
                    <EventAggregateStatRow headline=headline />
                    <div style="flex: 1 1 auto; min-height: 0; display: flex; flex-direction: column;">
                        {match view {
                            EventExploreView::BarChart => {
                                view! { <EventBarChart slices=slices.clone() /> }.into_any()
                            }
                            _ => view! { <EventPieChart slices=slices /> }.into_any(),
                        }}
                    </div>
                </Flex>
            }
            .into_any()
        }
        EventAggregateResult::TimeSeries { .. } | EventAggregateResult::Pivot { .. } => view! {
            <div data-testid="spectra-event-empty-state">
                <EmptyState
                    message="No slice data"
                    description="This view expects grouped slices. Check the group-by field and try again."
                />
            </div>
        }
        .into_any(),
    }
}
