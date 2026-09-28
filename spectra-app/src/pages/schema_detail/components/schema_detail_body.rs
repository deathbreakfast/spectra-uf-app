use leptos::prelude::*;
use orbital::components::{
    Body1, Card, CardContent, CardHeader, EmptyState, Skeleton, SkeletonItem, Title3,
};
use orbital::primitives::{Flex, MessageBar, MessageBarIntent};
use orbital_datatable::{DataTable, DataTableColumnDef, DataTableRowModel};
use spectra_core::SchemaFieldDto;
use std::collections::HashMap;

use crate::server::get_schema_metadata;

use super::quick_actions_card::QuickActionsCard;

/// Maps schema field DTOs into DataTable rows (Field / Type / Classification).
#[must_use]
pub fn schema_fields_to_rows(fields: &[SchemaFieldDto]) -> Vec<DataTableRowModel> {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut cells = HashMap::new();
            cells.insert("name".to_string(), f.name.clone());
            cells.insert("rust_type".to_string(), f.rust_type.clone());
            cells.insert("classification".to_string(), f.classification.clone());
            DataTableRowModel::from_text_cells(format!("field-{i}-{}", f.name), cells)
        })
        .collect()
}

fn field_columns() -> Vec<DataTableColumnDef> {
    vec![
        DataTableColumnDef::new("name", "Field"),
        DataTableColumnDef::new("rust_type", "Type"),
        DataTableColumnDef::new("classification", "Classification"),
    ]
}

#[component]
pub fn SchemaDetailBody(
    /// Reactive signal for the display name.
    #[prop(into)]
    name: Memo<String>,
) -> impl IntoView {
    let detail_res = Resource::new(
        move || name.get(),
        |n| async move { get_schema_metadata(n).await },
    );

    view! {
        <Suspense fallback=|| view! {
            <Card>
                <CardContent>
                    <Skeleton>
                        <SkeletonItem />
                        <SkeletonItem />
                    </Skeleton>
                </CardContent>
            </Card>
        }>
            {move || match detail_res.get() {
                Some(Ok(Some(d))) => {
                    let table_or_metric = d.table_or_metric.clone();
                    let logging_kind = d.logging_kind.clone();
                    let logging_kind_display = logging_kind.clone();
                    let fields = d.fields.clone();
                    let field_rows = RwSignal::new(schema_fields_to_rows(&fields));
                    let columns = field_columns();
                    view! {
                        <Flex vertical=true>
                            <div id="spectra-detail-meta">
                                <Card>
                                    <CardContent>
                                        <Flex vertical=true>
                                            <Body1>{d.description.unwrap_or_else(|| "No description".into())}</Body1>
                                            <Body1>{format!("Kind: {logging_kind_display}")}</Body1>
                                        </Flex>
                                    </CardContent>
                                </Card>
                            </div>
                            <Card>
                                <CardHeader>
                                    <Title3>"Fields"</Title3>
                                </CardHeader>
                                <CardContent>
                                    <div data-testid="spectra-schema-fields-table">
                                        {if fields.is_empty() {
                                            view! {
                                                <EmptyState
                                                    message="No field metadata on this schema."
                                                    description="The registry did not publish field rows for this table or metric."
                                                />
                                            }.into_any()
                                        } else {
                                            view! {
                                                <DataTable columns=columns items=field_rows sortable=true />
                                            }.into_any()
                                        }}
                                    </div>
                                </CardContent>
                            </Card>
                            <QuickActionsCard name=table_or_metric kind=logging_kind />
                        </Flex>
                    }.into_any()
                }
                Some(Ok(None)) => view! {
                    <EmptyState
                        message="Schema not found"
                        description="No schema is registered with that name."
                    />
                }.into_any(),
                Some(Err(_)) => view! {
                    <MessageBar intent=MessageBarIntent::Error>"Error loading schema."</MessageBar>
                }.into_any(),
                None => view! {
                    <Card>
                        <CardContent>
                            <Skeleton>
                                <SkeletonItem />
                            </Skeleton>
                        </CardContent>
                    </Card>
                }.into_any(),
            }}
        </Suspense>
    }
}

#[cfg(test)]
mod tests {
    use super::schema_fields_to_rows;
    use spectra_core::SchemaFieldDto;

    #[test]
    fn schema_detail_maps_fields_happy() {
        let fields = vec![SchemaFieldDto {
            name: "message".into(),
            rust_type: "String".into(),
            classification: "payload".into(),
        }];
        let rows = schema_fields_to_rows(&fields);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn schema_detail_empty_fields_shows_empty_state() {
        assert!(schema_fields_to_rows(&[]).is_empty());
    }
}
