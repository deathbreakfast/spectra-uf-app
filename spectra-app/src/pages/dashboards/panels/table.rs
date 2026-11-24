//! Table panels: the newest rows of an event table, with personal fields masked.

use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;
use orbital::components::{Caption1, EmptyState, SpacingSize};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, BadgeSize, Flex, FlexAlign, FlexWrap, Link,
};
use orbital_datatable::{
    DataRecord, DataTable, DataTableColumnDef, DataTableFeatures, DataTableRowModel,
    DataTableSource, DataTableToolbarConfig, PagingMode,
};
use spectra_core::PII_MASK;

/// "Newest 20 of 1204 rows."
pub(super) fn rows_summary(shown: usize, total: u64) -> String {
    let noun = if total == 1 { "row" } else { "rows" };
    format!("Newest {shown} of {total} {noun}.")
}

fn masked_column(field: &str) -> DataTableColumnDef {
    DataTableColumnDef::new(field, field).with_cell_view(Arc::new(|_: DataRecord| {
        view! {
            <Flex gap=SpacingSize::Size40.flex_gap() align=FlexAlign::Center>
                <span>{PII_MASK}</span>
                <span data-testid="spectra-dashboard-masked-badge">
                    <Badge appearance=BadgeAppearance::Outline color=BadgeColor::Subtle size=BadgeSize::Small>
                        "Masked"
                    </Badge>
                </span>
            </Flex>
        }
        .into_any()
    }))
}

/// A table panel's body.
#[component]
pub(super) fn TablePanel(
    /// Column fields, in order.
    columns: Vec<String>,
    /// Cell text per row.
    rows: Vec<Vec<String>>,
    /// Columns that were masked.
    masked: Vec<String>,
    /// Rows matching the query.
    total: u64,
    /// The panel's explorer link.
    explore: Memo<Option<String>>,
) -> impl IntoView {
    let shown = rows.len();
    if shown == 0 {
        return view! {
            <div data-testid="spectra-dashboard-table-panel">
                <EmptyState message="No rows" description="Nothing matched in this range." />
            </div>
        }
        .into_any();
    }
    let table_rows: Vec<DataTableRowModel> = rows
        .into_iter()
        .enumerate()
        .map(|(i, cells)| {
            let map: HashMap<String, String> = columns.iter().cloned().zip(cells).collect();
            DataTableRowModel::from_text_cells(i.to_string(), map)
        })
        .collect();
    let defs: Vec<DataTableColumnDef> = columns
        .iter()
        .map(|c| {
            if masked.contains(c) {
                masked_column(c)
            } else {
                DataTableColumnDef::new(c.as_str(), c.as_str())
            }
        })
        .collect();
    let masked_note = (!masked.is_empty()).then(|| {
        format!(
            "{} {} personal data, so {} hidden here. Open the row in explore if you're allowed to see it.",
            super::super::detail::join_names(&masked),
            if masked.len() == 1 { "is" } else { "are" },
            if masked.len() == 1 { "it's" } else { "they're" },
        )
    });
    view! {
        <div data-testid="spectra-dashboard-table-panel">
            <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                {masked_note.map(|note| view! {
                    <span data-testid="spectra-dashboard-masked-note">
                        <Caption1>{note}</Caption1>
                    </span>
                })}
                <DataTable
                    data_source=DataTableSource::Client(RwSignal::new(table_rows))
                    paging=PagingMode::Paged
                    features=DataTableFeatures::empty()
                    toolbar_config=DataTableToolbarConfig {
                        quick_search: false,
                        filter_panel: false,
                        column_picker: false,
                        pivot: false,
                        export_menu: false,
                    }
                    columns=defs
                />
                <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                    <span data-testid="spectra-dashboard-table-summary">
                        <Caption1>{rows_summary(shown, total)}</Caption1>
                    </span>
                    {move || explore.get().map(|href| view! {
                        <Link href=href>"Open in explore"</Link>
                    })}
                </Flex>
            </Flex>
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_rows() {
        assert_eq!(rows_summary(20, 1204), "Newest 20 of 1204 rows.");
        assert_eq!(rows_summary(1, 1), "Newest 1 of 1 row.");
    }
}
