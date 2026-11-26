//! Map query-link filters onto the event grid's `DataTable` filter and back.

use orbital_datatable::{DataTableFilter, DataValue, FilterLogic, FilterOperator, FilterRule};
use spectra_backend::{FilterOp, LinkFilter};

/// Grid filter for a link's filters (all must match).
#[must_use]
pub fn link_filters_to_table(filters: &[LinkFilter]) -> DataTableFilter {
    DataTableFilter {
        items: filters
            .iter()
            .map(|f| FilterRule {
                field: f.field.clone(),
                operator: match f.op {
                    FilterOp::Eq => FilterOperator::Equals,
                    FilterOp::Ne => FilterOperator::NotEquals,
                    FilterOp::Contains => FilterOperator::Contains,
                    FilterOp::Gt => FilterOperator::GreaterThan,
                    FilterOp::Lt => FilterOperator::LessThan,
                    FilterOp::Ge => FilterOperator::GreaterThanOrEqual,
                    FilterOp::Le => FilterOperator::LessThanOrEqual,
                    FilterOp::Starts => FilterOperator::StartsWith,
                    FilterOp::Ends => FilterOperator::EndsWith,
                    FilterOp::Empty => FilterOperator::IsEmpty,
                    FilterOp::NotEmpty => FilterOperator::IsNotEmpty,
                },
                value: DataValue::Text(f.value.clone()),
            })
            .collect(),
        logic: FilterLogic::And,
    }
}

/// Link filters for a grid filter.
///
/// "Does not contain" has no link form, and Spectra's event query can't run it either, so
/// those rules are left out of both.
#[must_use]
pub fn table_filter_to_link(filter: &DataTableFilter) -> Vec<LinkFilter> {
    filter
        .items
        .iter()
        .filter_map(|rule| {
            let op = match rule.operator {
                FilterOperator::Equals | FilterOperator::Is => FilterOp::Eq,
                FilterOperator::NotEquals | FilterOperator::IsNot => FilterOp::Ne,
                FilterOperator::Contains => FilterOp::Contains,
                FilterOperator::GreaterThan => FilterOp::Gt,
                FilterOperator::LessThan => FilterOp::Lt,
                FilterOperator::GreaterThanOrEqual => FilterOp::Ge,
                FilterOperator::LessThanOrEqual => FilterOp::Le,
                FilterOperator::StartsWith => FilterOp::Starts,
                FilterOperator::EndsWith => FilterOp::Ends,
                FilterOperator::IsEmpty => FilterOp::Empty,
                FilterOperator::IsNotEmpty => FilterOp::NotEmpty,
                FilterOperator::NotContains => return None,
            };
            let value = match op {
                FilterOp::Empty | FilterOp::NotEmpty => String::new(),
                _ => data_value_text(&rule.value),
            };
            Some(LinkFilter {
                field: rule.field.clone(),
                op,
                value,
            })
        })
        .collect()
}

fn data_value_text(value: &DataValue) -> String {
    match value {
        DataValue::Text(s) | DataValue::Category(s) => s.clone(),
        DataValue::Number(n) => n.to_string(),
        DataValue::Bool(b) => b.to_string(),
        DataValue::Date(d) => d.to_string(),
        DataValue::Null => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(field: &str, op: FilterOp, value: &str) -> LinkFilter {
        LinkFilter {
            field: field.into(),
            op,
            value: value.into(),
        }
    }

    #[test]
    fn link_filters_round_trip_through_grid_happy_path() {
        let filters: Vec<LinkFilter> = FilterOp::ALL
            .into_iter()
            .map(|op| match op {
                FilterOp::Empty | FilterOp::NotEmpty => filter("message", op, ""),
                _ => filter("status", op, "failed:x"),
            })
            .collect();
        let grid = link_filters_to_table(&filters);
        assert!(matches!(grid.logic, FilterLogic::And));
        assert_eq!(table_filter_to_link(&grid), filters);
    }

    #[test]
    fn grid_only_operators_map_or_drop_sad_path() {
        let grid = DataTableFilter {
            items: vec![
                FilterRule {
                    field: "status".into(),
                    operator: FilterOperator::Is,
                    value: DataValue::Category("failed".into()),
                },
                FilterRule {
                    field: "message".into(),
                    operator: FilterOperator::NotContains,
                    value: DataValue::Text("ok".into()),
                },
                FilterRule {
                    field: "value".into(),
                    operator: FilterOperator::GreaterThan,
                    value: DataValue::Number(3.0),
                },
            ],
            logic: FilterLogic::And,
        };
        assert_eq!(
            table_filter_to_link(&grid),
            vec![
                filter("status", FilterOp::Eq, "failed"),
                filter("value", FilterOp::Gt, "3"),
            ]
        );
    }
}
