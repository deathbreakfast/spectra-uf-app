use chrono::{DateTime, Utc};
use leptos::prelude::*;
use orbital::components::Caption1;
use orbital::primitives::{
    Button, ButtonAppearance, DateTimeRangePicker, Flex, FlexAlign, FlexGap,
};
use spectra_backend::{QueryLink, QueryView};
use spectra_core::EventExploreView;

use super::link_query::{aggregation_seed, apply_aggregation, explore_view, link_view};
use crate::components::query::{
    normalize_view, EventAggregationBar, EventViewPicker, QueryToolbarMaterial,
};
use crate::explore_time::{
    same_picked_range, window_datetime_range, window_from_datetime_range, ExploreLinkState,
};

/// Explore controls. They start from `seed` and write every change to the URL; the
/// explore query reads the URL, never these controls.
#[component]
pub fn EventToolbar(
    /// URL state the controls write to.
    link: ExploreLinkState,
    /// Query the controls start from.
    seed: QueryLink,
    /// Schema field names for group-by / sum Selects.
    #[prop(into)]
    field_options: Signal<Vec<String>>,
    /// Manual refresh of the current applied view.
    on_refresh: Callback<()>,
    /// Last successful refresh time (UTC), when known.
    #[prop(into)]
    last_refreshed: Signal<Option<DateTime<Utc>>>,
) -> impl IntoView {
    let seeded_range = window_datetime_range(seed.window);
    let range = RwSignal::new(Some(seeded_range.clone()));
    let synced_range = StoredValue::new(seeded_range);
    Effect::new(move |_| {
        let Some(picked) = range.get() else {
            return;
        };
        if synced_range.with_value(|synced| same_picked_range(synced, &picked)) {
            return;
        }
        let window = window_from_datetime_range(&picked);
        synced_range.set_value(picked);
        link.update(|l| l.window = window);
    });

    // The view Select is a pending choice; chart views apply on See.
    let picker_view = RwSignal::new(explore_view(seed.view));
    // Selecting Event log applies immediately.
    Effect::new(move |_| {
        if normalize_view(picker_view.get()) == EventExploreView::EventLog {
            link.update(|l| {
                if explore_view(l.view) != EventExploreView::EventLog {
                    l.view = Some(QueryView::Log);
                }
            });
        }
    });

    let aggregation = RwSignal::new(aggregation_seed(&seed));
    Effect::new(move |_| {
        let spec = aggregation.get();
        link.update(|l| apply_aggregation(l, &spec));
    });

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
                    <EventViewPicker
                        view=Signal::derive(move || picker_view.get())
                        on_change=Callback::new(move |v| picker_view.set(normalize_view(v)))
                    />
                    <span data-testid="spectra-event-see">
                        <Button
                            appearance=ButtonAppearance::Primary
                            on:click=move |_| {
                                let view = link_view(picker_view.get_untracked());
                                link.update(|l| l.view = Some(view));
                            }
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
                        view=Signal::derive(move || picker_view.get())
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
