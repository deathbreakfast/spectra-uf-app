use leptos::prelude::*;
use orbital_datatable::DataTableFilter;
use spectra_core::EventQueryResult;

use crate::components::tables::SpectraEventDataGrid;

#[component]
pub fn EventLogPanel(
    /// Result data to render.
    result: EventQueryResult,
    /// Controlled filter model shared with explore queries.
    #[prop(optional)]
    filter: Option<RwSignal<Option<DataTableFilter>>>,
    /// Bridge filter changes into the explore query model.
    #[prop(optional)]
    on_filter_change: Option<Callback<DataTableFilter>>,
) -> impl IntoView {
    let filter = filter.unwrap_or_else(|| RwSignal::new(None));
    let on_filter_change = on_filter_change.unwrap_or_else(|| Callback::new(|_| {}));
    view! {
                <SpectraEventDataGrid
                    result=Some(result)
                    filter=filter
                    on_filter_change=on_filter_change
                    chart_mode=false
                />
    }
}
