use std::collections::HashMap;

use orbital_datatable::{DataTableColumnDef, DataTableRowModel};
use serde_json::Value;
use spectra_core::{EventGridRow, GridColumnDto};

pub fn to_column_defs(columns: &[GridColumnDto]) -> Vec<DataTableColumnDef> {
    columns
        .iter()
        .map(|c| DataTableColumnDef::new(&c.field, &c.header_name))
        .collect()
}

pub fn to_row_models(rows: &[EventGridRow], columns: &[GridColumnDto]) -> Vec<DataTableRowModel> {
    rows.iter()
        .map(|r| {
            let cells = columns
                .iter()
                .map(|col| {
                    let value = if col.field == "ts" {
                        r.ts.to_rfc3339()
                    } else {
                        display_field_value(&r.fields, &col.field)
                    };
                    (col.field.clone(), value)
                })
                .collect::<HashMap<_, _>>();
            DataTableRowModel::from_text_cells(r.id.clone(), cells)
        })
        .collect()
}

pub fn display_field_value(fields: &Value, field: &str) -> String {
    if field == "ts" {
        return String::new();
    }
    fields.get(field).map_or_else(String::new, |v| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}
