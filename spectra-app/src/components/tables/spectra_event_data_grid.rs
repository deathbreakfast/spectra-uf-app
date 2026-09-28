use leptos::prelude::*;
use orbital_datatable::{
    DataTable, DataTableEmptyView, DataTableEvents, DataTableFeatures, DataTableFilter,
    DataTableFooterSlot, DataTableNoResultsView, PagingMode,
};
use spectra_core::EventQueryResult;

use super::event_grid_mapper::{to_column_defs, to_row_models};

#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn SpectraEventDataGrid(
    /// Event log query result (rows + columns). When `None`, `column_fields` supplies columns.
    #[prop(default = None)]
    result: Option<EventQueryResult>,
    /// Column field/header pairs when `result` is `None`.
    #[prop(default = Vec::new())]
    column_fields: Vec<(String, String)>,
    /// Controlled filter model shared with explore queries.
    filter: RwSignal<Option<DataTableFilter>>,
    /// Fires when the operator changes structured filters (header or Filters panel).
    on_filter_change: Callback<DataTableFilter>,
    /// When true, keep DataTable toolbar only (hide thead/tbody/footer chrome).
    #[prop(default = false)]
    chart_mode: bool,
) -> impl IntoView {
    let columns = result.as_ref().map_or_else(
        || {
            column_fields
                .into_iter()
                .map(|(field, header)| orbital_datatable::DataTableColumnDef::new(field, header))
                .collect()
        },
        |result| to_column_defs(&result.columns),
    );

    let row_models = if chart_mode {
        Vec::new()
    } else if let Some(ref result) = result {
        to_row_models(&result.rows, &result.columns)
    } else {
        Vec::new()
    };
    let items = RwSignal::new(row_models);

    let features = if chart_mode {
        DataTableFeatures::empty()
    } else {
        DataTableFeatures::HEADER_FILTERS
    };

    let events = DataTableEvents {
        on_filter_change: Some(Callback::new(move |f: DataTableFilter| {
            filter.set(Some(f.clone()));
            on_filter_change.run(f);
        })),
        ..Default::default()
    };

    // Prefer Orbital `show_table_grid=false` when hosts path-patch a newer datatable.
    // Until then, hide scroll-host/footer so chart mode keeps search/Filters/Columns/Export only.
    let chart_chrome_css = r#"
[data-testid="spectra-event-data-grid"][data-chart-mode="true"] .orbital-data-table__scroll-host,
[data-testid="spectra-event-data-grid"][data-chart-mode="true"] .orbital-data-table__footer {
    display: none !important;
}
"#;

    if chart_mode {
        view! {
            <style>{chart_chrome_css}</style>
            <div data-testid="spectra-event-data-grid" data-chart-mode="true">
                <DataTable
                    columns=columns
                    items=items
                    sortable=true
                    features=features
                    filter=Signal::derive(move || filter.get())
                    data_table_events=events
                    paging=PagingMode::None
                >
                    <DataTableFooterSlot slot>
                        <span></span>
                    </DataTableFooterSlot>
                    <DataTableEmptyView slot>
                        <div style="display:none" aria-hidden="true"></div>
                    </DataTableEmptyView>
                    <DataTableNoResultsView slot>
                        <div style="display:none" aria-hidden="true"></div>
                    </DataTableNoResultsView>
                </DataTable>
            </div>
        }
        .into_any()
    } else {
        view! {
            <div data-testid="spectra-event-data-grid" data-chart-mode="false">
                <DataTable
                    columns=columns
                    items=items
                    sortable=true
                    features=features
                    filter=Signal::derive(move || filter.get())
                    data_table_events=events
                    paging=PagingMode::Paged
                />
            </div>
        }
        .into_any()
    }
}
