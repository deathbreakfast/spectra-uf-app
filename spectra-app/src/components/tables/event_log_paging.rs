//! Server-paged event log: maps `DataTable` page requests onto Spectra event queries.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use leptos::prelude::ServerFnError;
use orbital_datatable::{DataRecord, DataValue, PageFetcher};
use orbital_paging::{Page, PageRequest, SortDirectionWire};
use spectra_core::{
    EventQuery, EventQueryResult, GridFilterModel, GridPaginationModel, GridSortDirection,
    GridSortItem,
};

use super::event_grid_mapper::display_field_value;
use super::filter_bridge::filter_query_to_grid;
use crate::server::query_events;

/// Rows per event log page.
pub const EVENT_LOG_PAGE_SIZE: u32 = 50;

/// Table and time range every page of one event log shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLogScope {
    /// Event table name.
    pub table: String,
    /// Inclusive range start.
    pub start: DateTime<Utc>,
    /// Range end.
    pub end: DateTime<Utc>,
}

impl EventLogScope {
    /// Query for the unfiltered first page, newest rows first.
    #[must_use]
    pub fn first_page_query(&self) -> EventQuery {
        page_request_to_event_query(self, &first_page_request())
    }
}

const fn first_page_request() -> PageRequest {
    PageRequest {
        offset: 0,
        limit: EVENT_LOG_PAGE_SIZE,
        sort: None,
        filter: None,
        quick_search: None,
    }
}

fn is_first_page_request(request: &PageRequest) -> bool {
    request.offset == 0
        && request.limit == EVENT_LOG_PAGE_SIZE
        && request.sort.as_ref().is_none_or(Vec::is_empty)
        && request.filter.as_ref().is_none_or(|f| f.items.is_empty())
        && request
            .quick_search
            .as_deref()
            .is_none_or(|q| q.trim().is_empty())
}

/// Builds the Spectra query for one `DataTable` page.
///
/// Paged mode always requests offsets that are multiples of `limit`, so the page index is
/// `offset / limit`. With no sort columns the log is newest first.
#[must_use]
pub fn page_request_to_event_query(scope: &EventLogScope, request: &PageRequest) -> EventQuery {
    let page_size = request.limit.max(1);
    let mut sort: Vec<GridSortItem> = request
        .sort
        .iter()
        .flatten()
        .map(|s| GridSortItem {
            field: s.field.clone(),
            sort: match s.direction {
                SortDirectionWire::Asc => GridSortDirection::Asc,
                SortDirectionWire::Desc => GridSortDirection::Desc,
            },
        })
        .collect();
    if sort.is_empty() {
        sort.push(GridSortItem {
            field: "ts".into(),
            sort: GridSortDirection::Desc,
        });
    }
    let mut filter = request
        .filter
        .as_ref()
        .map_or_else(GridFilterModel::default, filter_query_to_grid);
    filter.quick_filter_values = request
        .quick_search
        .as_deref()
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    EventQuery {
        table: scope.table.clone(),
        start: scope.start,
        end: scope.end,
        partition: None,
        pagination: GridPaginationModel {
            page: request.offset / page_size,
            page_size,
        },
        sort,
        filter,
    }
}

/// Converts one page of event rows into `DataTable` records.
///
/// `result.row_count` is the total across pages, so it becomes the footer total.
#[must_use]
pub fn event_result_to_page(result: EventQueryResult, offset: u32) -> Page<DataRecord> {
    let seen = u64::from(offset).saturating_add(u64::try_from(result.rows.len()).unwrap_or(0));
    let columns = result.columns;
    let items = result
        .rows
        .into_iter()
        .map(|row| {
            let values = columns
                .iter()
                .map(|col| {
                    let text = if col.field == "ts" {
                        row.ts.to_rfc3339()
                    } else {
                        display_field_value(&row.fields, &col.field)
                    };
                    (col.field.clone(), DataValue::Text(text))
                })
                .collect::<HashMap<_, _>>();
            DataRecord::new(row.id, values)
        })
        .collect();
    Page {
        items,
        has_more: seen < result.row_count,
        total_count: Some(result.row_count),
        next_request_offset: None,
    }
}

/// Fetcher for a server-paged event log.
///
/// `first_page` is the result the explore panel already loaded with
/// [`EventLogScope::first_page_query`]. The table's first request reuses it when that request
/// is the same unfiltered first page; every other request goes to the server.
#[must_use]
pub fn build_event_log_fetcher(
    scope: EventLogScope,
    first_page: Option<EventQueryResult>,
) -> PageFetcher {
    let first_page = Arc::new(Mutex::new(first_page));
    Arc::new(move |request: PageRequest| {
        let scope = scope.clone();
        let first_page = Arc::clone(&first_page);
        Box::pin(async move {
            let preloaded = first_page
                .lock()
                .ok()
                .and_then(|mut slot| slot.take())
                .filter(|_| is_first_page_request(&request));
            let result = match preloaded {
                Some(result) => result,
                None => query_events(page_request_to_event_query(&scope, &request)).await?,
            };
            Ok(event_result_to_page(result, request.offset))
        }) as Pin<Box<dyn Future<Output = Result<Page<DataRecord>, ServerFnError>> + Send>>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_paging::{FilterLogicWire, FilterQuery, FilterRuleParam, SortParam};
    use serde_json::json;
    use spectra_core::{EventGridRow, GridColumnDto, GridFilterOperator};

    fn scope() -> EventLogScope {
        EventLogScope {
            table: "platform_smoke_event".into(),
            start: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap_or_default(),
            end: DateTime::<Utc>::from_timestamp(1_700_003_600, 0).unwrap_or_default(),
        }
    }

    #[test]
    fn page_request_maps_offset_sort_filter_and_search_happy_path() {
        let request = PageRequest {
            offset: 100,
            limit: 50,
            sort: Some(vec![SortParam {
                field: "severity".into(),
                direction: SortDirectionWire::Asc,
            }]),
            filter: Some(FilterQuery {
                items: vec![FilterRuleParam {
                    field: "severity".into(),
                    operator: "equals".into(),
                    value: DataValue::Text("warn".into()),
                }],
                logic: FilterLogicWire::And,
            }),
            quick_search: Some("  disk  full ".into()),
        };
        let query = page_request_to_event_query(&scope(), &request);
        assert_eq!(query.table, "platform_smoke_event");
        assert_eq!(query.pagination.page, 2);
        assert_eq!(query.pagination.page_size, 50);
        assert_eq!(query.sort.len(), 1);
        assert_eq!(query.sort[0].field, "severity");
        assert_eq!(query.sort[0].sort, GridSortDirection::Asc);
        assert_eq!(query.filter.items.len(), 1);
        assert!(matches!(
            query.filter.items[0].operator,
            GridFilterOperator::Equals
        ));
        assert_eq!(query.filter.quick_filter_values, vec!["disk", "full"]);
    }

    #[test]
    fn page_request_defaults_to_newest_first_sad_path_zero_limit() {
        let request = PageRequest {
            offset: 0,
            limit: 0,
            sort: Some(Vec::new()),
            filter: None,
            quick_search: None,
        };
        let query = page_request_to_event_query(&scope(), &request);
        assert_eq!(query.pagination.page, 0);
        assert_eq!(query.pagination.page_size, 1);
        assert_eq!(query.sort[0].field, "ts");
        assert_eq!(query.sort[0].sort, GridSortDirection::Desc);
        assert_eq!(query.filter.items.len(), 0);
        assert_eq!(query.filter.quick_filter_values.len(), 0);
    }

    #[test]
    fn first_page_request_detection() {
        assert!(is_first_page_request(&first_page_request()));
        let mut later = first_page_request();
        later.offset = EVENT_LOG_PAGE_SIZE;
        assert!(!is_first_page_request(&later));
        let mut searched = first_page_request();
        searched.quick_search = Some("x".into());
        assert!(!is_first_page_request(&searched));
    }

    fn result(rows: usize, row_count: u64) -> EventQueryResult {
        let ts = DateTime::<Utc>::from_timestamp(1_700_000_100, 0).unwrap_or_default();
        EventQueryResult {
            rows: (0..rows)
                .map(|i| EventGridRow {
                    id: format!("r-{i}"),
                    ts,
                    fields: json!({ "severity": "info", "bytes": i }),
                })
                .collect(),
            columns: vec![
                GridColumnDto {
                    field: "ts".into(),
                    header_name: "ts".into(),
                },
                GridColumnDto {
                    field: "bytes".into(),
                    header_name: "bytes".into(),
                },
            ],
            row_count,
        }
    }

    #[test]
    fn result_to_page_reports_total_and_has_more_happy_path() {
        let page = event_result_to_page(result(50, 120), 50);
        assert_eq!(page.items.len(), 50);
        assert_eq!(page.total_count, Some(120));
        assert!(page.has_more);
        assert_eq!(
            page.items[1].get("bytes").map(DataValue::display_string),
            Some("1".into())
        );
    }

    #[test]
    fn result_to_page_last_page_sad_path() {
        let page = event_result_to_page(result(20, 120), 100);
        assert!(!page.has_more);
        let empty = event_result_to_page(result(0, 0), 0);
        assert_eq!(empty.items.len(), 0);
        assert!(!empty.has_more);
        assert_eq!(empty.total_count, Some(0));
    }
}
