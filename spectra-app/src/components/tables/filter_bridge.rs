//! Map Orbital DataTable filter models onto Spectra [`GridFilterModel`].

use orbital_datatable::{DataTableFilter, DataValue, FilterLogic, FilterOperator, FilterRule};
use orbital_paging::{FilterLogicWire, FilterQuery};
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

/// Convert a server-paging [`FilterQuery`] into a Spectra grid filter.
///
/// Rules with an unknown wire operator, or one Spectra cannot express, are dropped.
#[must_use]
pub fn filter_query_to_grid(query: &FilterQuery) -> GridFilterModel {
    GridFilterModel {
        items: query
            .items
            .iter()
            .filter_map(|rule| {
                Some(GridFilterItem {
                    field: rule.field.clone(),
                    operator: map_operator(operator_from_wire(&rule.operator)?)?,
                    value: data_value_to_json(&rule.value),
                })
            })
            .collect(),
        logic_operator: match query.logic {
            FilterLogicWire::And => GridLogicOperator::And,
            FilterLogicWire::Or => GridLogicOperator::Or,
        },
        quick_filter_values: Vec::new(),
    }
}

/// Wire names match `orbital_datatable`'s server query encoding, which is crate-private.
fn operator_from_wire(value: &str) -> Option<FilterOperator> {
    Some(match value {
        "is_empty" => FilterOperator::IsEmpty,
        "is_not_empty" => FilterOperator::IsNotEmpty,
        "contains" => FilterOperator::Contains,
        "not_contains" => FilterOperator::NotContains,
        "equals" => FilterOperator::Equals,
        "not_equals" => FilterOperator::NotEquals,
        "starts_with" => FilterOperator::StartsWith,
        "ends_with" => FilterOperator::EndsWith,
        "greater_than" => FilterOperator::GreaterThan,
        "greater_than_or_equal" => FilterOperator::GreaterThanOrEqual,
        "less_than" => FilterOperator::LessThan,
        "less_than_or_equal" => FilterOperator::LessThanOrEqual,
        "is" => FilterOperator::Is,
        "is_not" => FilterOperator::IsNot,
        _ => return None,
    })
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
    fn filter_query_maps_wire_operators_happy_path() {
        let query = FilterQuery {
            items: vec![orbital_paging::FilterRuleParam {
                field: "severity".into(),
                operator: "equals".into(),
                value: DataValue::Text("warn".into()),
            }],
            logic: FilterLogicWire::Or,
        };
        let grid = filter_query_to_grid(&query);
        assert_eq!(grid.items.len(), 1);
        assert!(matches!(grid.items[0].operator, GridFilterOperator::Equals));
        assert_eq!(grid.items[0].value, json!("warn"));
        assert!(matches!(grid.logic_operator, GridLogicOperator::Or));
    }

    #[test]
    fn filter_query_drops_unknown_and_unsupported_operators_sad_path() {
        let rule = |operator: &str| orbital_paging::FilterRuleParam {
            field: "message".into(),
            operator: operator.into(),
            value: DataValue::Text("x".into()),
        };
        let query = FilterQuery {
            items: vec![rule("regex"), rule("not_contains")],
            logic: FilterLogicWire::And,
        };
        assert_eq!(filter_query_to_grid(&query).items.len(), 0);
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
