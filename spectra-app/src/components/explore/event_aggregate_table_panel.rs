use std::collections::HashMap;

use leptos::prelude::*;
use orbital::components::{Caption1, EmptyState};
use orbital_datatable::{DataTable, DataTableColumnDef, DataTableRowModel, PagingMode};
use spectra_core::EventAggregateResult;

use crate::components::charts::EventAggregateStatRow;

/// Renders [`EventAggregateResult::Pivot`] as an Orbital DataTable matrix.
#[component]
pub fn EventAggregateTablePanel(
    /// Aggregate payload (Pivot expected).
    result: EventAggregateResult,
    /// Whether the operator has selected at least one Rows field.
    #[prop(into)]
    row_fields_selected: Signal<bool>,
) -> impl IntoView {
    if !row_fields_selected.get() {
        return view! {
            <div data-testid="spectra-event-empty-state">
                <EmptyState
                    message="Choose row fields"
                    description="Table view needs one or more schema fields as row keys before it can pivot."
                />
            </div>
        }
        .into_any();
    }

    match result {
        EventAggregateResult::Pivot {
            row_fields,
            column_keys,
            rows,
            headline,
        } => {
            if rows.is_empty() {
                return view! {
                    <div data-testid="spectra-event-empty-state">
                        <EmptyState
                            message="No pivot data"
                            description="Nothing to show for these Rows / Columns settings and time range."
                        />
                    </div>
                }
                .into_any();
            }

            let capped = column_keys.iter().any(|k| k == "(other)");
            let show_total = column_keys.len() > 1;

            let mut columns: Vec<DataTableColumnDef> = row_fields
                .iter()
                .map(|f| DataTableColumnDef::new(format!("row_{f}"), f.clone()))
                .collect();
            for key in &column_keys {
                columns.push(DataTableColumnDef::new(format!("cell_{key}"), key.clone()));
            }
            if show_total {
                columns.push(DataTableColumnDef::new("row_total", "Σ"));
            }

            let row_models: Vec<DataTableRowModel> = rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let mut cells = HashMap::new();
                    for (idx, field) in row_fields.iter().enumerate() {
                        let value = row
                            .row_values
                            .get(idx)
                            .cloned()
                            .unwrap_or_else(|| "(blank)".into());
                        cells.insert(format!("row_{field}"), value);
                    }
                    let mut total = 0.0;
                    for (idx, key) in column_keys.iter().enumerate() {
                        let value = row.cells.get(idx).copied().unwrap_or(0.0);
                        total += value;
                        cells.insert(format!("cell_{key}"), format_cell(value));
                    }
                    if show_total {
                        cells.insert("row_total".into(), format_cell(total));
                    }
                    DataTableRowModel::from_text_cells(format!("pivot-{i}"), cells)
                })
                .collect();

            let items = RwSignal::new(row_models);

            view! {
                <div data-testid="spectra-event-aggregate-table-panel">
                    <EventAggregateStatRow headline=headline />
                    {capped.then(|| view! {
                        <Caption1>
                            "Some column values were rolled into (other) (top 50 by measure)."
                        </Caption1>
                    })}
                    <DataTable
                        columns=columns
                        items=items
                        sortable=true
                        paging=PagingMode::Paged
                    />
                </div>
            }
            .into_any()
        }
        _ => view! {
            <div data-testid="spectra-event-empty-state">
                <EmptyState
                    message="No pivot data"
                    description="This view expects a Table pivot result. Click See after setting Rows."
                />
            </div>
        }
        .into_any(),
    }
}

fn format_cell(value: f64) -> String {
    if (value - value.round()).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}
