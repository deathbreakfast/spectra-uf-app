//! Map Orbital DataTable filter models onto Spectra [`GridFilterModel`].

use orbital_datatable::{DataTableFilter, DataValue, FilterLogic, FilterOperator, FilterRule};
use serde_json::json;
use spectra_core::{GridFilterItem, GridFilterModel, GridFilterOperator, GridLogicOperator};

/// Convert a DataTable filter into a Spectra grid filter (structured items only).
#[must_use]
pub fn data_table_filter_to_grid(filter: &DataTableFilter) -> GridFilterModel {
    GridFilterModel {
        items: filter
            .items
            .iter()
            .filter_map(filter_rule_to_grid_item)
            .collect(),
        logic_operator: match filter.logic {
            FilterLogic::And => GridLogicOperator::And,
            FilterLogic::Or => GridLogicOperator::Or,
        },
        quick_filter_values: Vec::new(),
    }
}

fn filter_rule_to_grid_item(rule: &FilterRule) -> Option<GridFilterItem> {
    let operator = map_operator(rule.operator)?;
    let value = data_value_to_json(&rule.value);
    Some(GridFilterItem {
        field: rule.field.clone(),
        operator,
        value,
    })
}

fn map_operator(op: FilterOperator) -> Option<GridFilterOperator> {
    Some(match op {
        FilterOperator::Equals | FilterOperator::Is => GridFilterOperator::Equals,
        FilterOperator::NotEquals | FilterOperator::IsNot => GridFilterOperator::DoesNotEqual,
        // Spectra grid model has no NotContains; skip until core grows the op.
        FilterOperator::NotContains => return None,
        FilterOperator::Contains => GridFilterOperator::Contains,
        FilterOperator::StartsWith => GridFilterOperator::StartsWith,
        FilterOperator::EndsWith => GridFilterOperator::EndsWith,
        FilterOperator::IsEmpty => GridFilterOperator::IsEmpty,
        FilterOperator::IsNotEmpty => GridFilterOperator::IsNotEmpty,
        FilterOperator::GreaterThan => GridFilterOperator::GreaterThan,
        FilterOperator::GreaterThanOrEqual => GridFilterOperator::GreaterThanOrEqual,
        FilterOperator::LessThan => GridFilterOperator::LessThan,
        FilterOperator::LessThanOrEqual => GridFilterOperator::LessThanOrEqual,
    })
}

fn data_value_to_json(value: &DataValue) -> serde_json::Value {
    match value {
        DataValue::Text(s) | DataValue::Category(s) => json!(s),
        DataValue::Number(n) => json!(n),
        DataValue::Bool(b) => json!(b),
        DataValue::Date(d) => json!(d.to_string()),
        DataValue::Null => json!(null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_contains_rule_happy_path() {
        let filter = DataTableFilter {
            items: vec![FilterRule {
                field: "permission_name".into(),
                operator: FilterOperator::Contains,
                value: DataValue::Text("read".into()),
            }],
            logic: FilterLogic::And,
        };
        let grid = data_table_filter_to_grid(&filter);
        assert_eq!(grid.items.len(), 1);
        assert_eq!(grid.items[0].field, "permission_name");
        assert!(matches!(
            grid.items[0].operator,
            GridFilterOperator::Contains
        ));
        assert_eq!(grid.items[0].value, json!("read"));
    }

    #[test]
    fn maps_or_logic_sad_path_empty_items() {
        let filter = DataTableFilter {
            items: vec![],
            logic: FilterLogic::Or,
        };
        let grid = data_table_filter_to_grid(&filter);
        assert!(grid.items.is_empty());
        assert!(matches!(grid.logic_operator, GridLogicOperator::Or));
    }
}
