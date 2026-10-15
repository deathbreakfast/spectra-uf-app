//! Live query helper contracts against mem Spectra backends.

#![allow(missing_docs, clippy::expect_used)]

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use spectra::{MemEventsBackend, MemMetricsBackend, Spectra};
use spectra_backend::{
    count_matching_events, dashboard_catalog_summary, execute_event_query, execute_metrics_query,
    DEFAULT_RECENT_SCHEMA_LIMIT,
};
use spectra_core::{
    EventQuery, EventStorageBackend, GridFilterItem, GridFilterModel, GridFilterOperator,
    GridPaginationModel, MetricsQuery, MetricsStorageBackend, SpectraRouter,
};

const SEED_TABLE: &str = "integ.events";
const SEED_METRIC: &str = "integ.cpu";

fn empty_router() -> Arc<SpectraRouter> {
    let metrics: Arc<dyn MetricsStorageBackend> = Arc::new(MemMetricsBackend::new());
    let events: Arc<dyn EventStorageBackend> = Arc::new(MemEventsBackend::new());
    Spectra::builder()
        .metrics_backend(metrics)
        .events_backend(events)
        .embedded()
        .build()
        .expect("mem spectra")
        .router()
}

async fn seeded_router() -> Arc<SpectraRouter> {
    let metrics: Arc<MemMetricsBackend> = Arc::new(MemMetricsBackend::new());
    let events: Arc<MemEventsBackend> = Arc::new(MemEventsBackend::new());
    let ts = Utc::now() - chrono::Duration::minutes(5);
    events
        .append_row(
            SEED_TABLE,
            &json!({
                "id": "integ-1",
                "message": "integration seed row",
                "severity": "info",
                "value": 10,
            }),
            ts,
            None,
        )
        .await
        .expect("append row 1");
    events
        .append_row(
            SEED_TABLE,
            &json!({
                "id": "integ-2",
                "message": "warn row",
                "severity": "warn",
                "value": 3,
            }),
            ts - chrono::Duration::minutes(1),
            None,
        )
        .await
        .expect("append row 2");
    metrics
        .record_gauge(SEED_METRIC, &json!({"host": "integ"}), 42.0, ts)
        .await
        .expect("record metric");
    Spectra::builder()
        .metrics_backend(Arc::clone(&metrics) as Arc<dyn MetricsStorageBackend>)
        .events_backend(Arc::clone(&events) as Arc<dyn EventStorageBackend>)
        .embedded()
        .build()
        .expect("mem spectra")
        .router()
}

#[tokio::test]
async fn execute_event_query_empty_table_happy_path() {
    let router = empty_router();
    let result = execute_event_query(
        &router,
        &EventQuery {
            table: "e2e.events".into(),
            start: Utc::now() - chrono::Duration::hours(1),
            end: Utc::now(),
            partition: None,
            pagination: GridPaginationModel::default(),
            sort: vec![],
            filter: GridFilterModel::default(),
        },
    )
    .await
    .expect("query");
    assert_eq!(result.row_count, 0);
    assert!(result.rows.is_empty());
}

#[tokio::test]
async fn execute_event_query_seeded_row_happy_path() {
    let router = seeded_router().await;
    let result = execute_event_query(
        &router,
        &EventQuery {
            table: SEED_TABLE.into(),
            start: Utc::now() - chrono::Duration::hours(1),
            end: Utc::now(),
            partition: None,
            pagination: GridPaginationModel::default(),
            sort: vec![],
            filter: GridFilterModel::default(),
        },
    )
    .await
    .expect("query");
    assert!(result.row_count >= 2, "expected seeded rows");
    let body = format!("{result:?}");
    assert!(
        body.contains("integration seed row"),
        "expected seed message in rows: {body}"
    );
}

#[tokio::test]
async fn execute_metrics_query_empty_happy_path() {
    let router = empty_router();
    let result = execute_metrics_query(
        &router,
        &MetricsQuery {
            metric: "e2e.cpu".into(),
            start: Utc::now() - chrono::Duration::hours(1),
            end: Utc::now(),
            step_secs: Some(60),
            label_matchers: vec![],
        },
    )
    .await
    .expect("query");
    assert!(result.headline.is_empty() || !result.series.is_empty());
}

#[tokio::test]
async fn execute_metrics_query_seeded_point_happy_path() {
    let router = seeded_router().await;
    let result = execute_metrics_query(
        &router,
        &MetricsQuery {
            metric: SEED_METRIC.into(),
            start: Utc::now() - chrono::Duration::hours(1),
            end: Utc::now(),
            step_secs: Some(60),
            label_matchers: vec![],
        },
    )
    .await
    .expect("query");
    let snapshot = format!("{result:?}");
    assert!(
        snapshot.contains("42") || !result.series.is_empty(),
        "expected metric series or headline with seeded value: {snapshot}"
    );
}

const PAGING_TABLE: &str = "integ.paging";

/// Seven rows inside the last hour (two `warn`) plus one row a day old.
async fn paging_router() -> Arc<SpectraRouter> {
    let events: Arc<MemEventsBackend> = Arc::new(MemEventsBackend::new());
    let now = Utc::now();
    for i in 0..7i64 {
        let severity = if i < 2 { "warn" } else { "info" };
        events
            .append_row(
                PAGING_TABLE,
                &json!({ "id": format!("p-{i}"), "severity": severity }),
                now - chrono::Duration::minutes(i + 1),
                None,
            )
            .await
            .expect("append paging row");
    }
    events
        .append_row(
            PAGING_TABLE,
            &json!({ "id": "p-old", "severity": "info" }),
            now - chrono::Duration::days(1),
            None,
        )
        .await
        .expect("append old row");
    router_with_events(events)
}

fn router_with_events(events: Arc<dyn EventStorageBackend>) -> Arc<SpectraRouter> {
    Spectra::builder()
        .metrics_backend(Arc::new(MemMetricsBackend::new()) as Arc<dyn MetricsStorageBackend>)
        .events_backend(events)
        .embedded()
        .build()
        .expect("mem spectra")
        .router()
}

fn paging_query(page: u32, page_size: u32, filter: GridFilterModel) -> EventQuery {
    EventQuery {
        table: PAGING_TABLE.into(),
        start: Utc::now() - chrono::Duration::hours(1),
        end: Utc::now(),
        partition: None,
        pagination: GridPaginationModel { page, page_size },
        sort: vec![],
        filter,
    }
}

#[tokio::test]
async fn execute_event_query_row_count_spans_pages_happy_path() {
    let router = paging_router().await;
    let first = execute_event_query(&router, &paging_query(0, 3, GridFilterModel::default()))
        .await
        .expect("page 0");
    assert_eq!(first.rows.len(), 3);
    assert_eq!(
        first.row_count, 7,
        "total covers every page, not just this one"
    );

    let last = execute_event_query(&router, &paging_query(2, 3, GridFilterModel::default()))
        .await
        .expect("page 2");
    assert_eq!(last.rows.len(), 1);
    assert_eq!(last.row_count, 7);
    assert!(
        first.rows.iter().all(|r| r.id != last.rows[0].id),
        "pages must not overlap"
    );
}

#[tokio::test]
async fn execute_event_query_row_count_honors_filter_happy_path() {
    let router = paging_router().await;
    let warn_only = GridFilterModel {
        items: vec![GridFilterItem {
            field: "severity".into(),
            operator: GridFilterOperator::Equals,
            value: json!("warn"),
        }],
        ..Default::default()
    };
    let result = execute_event_query(&router, &paging_query(0, 50, warn_only))
        .await
        .expect("filtered");
    assert_eq!(result.row_count, 2);
    assert_eq!(result.rows.len(), 2);
}

#[tokio::test]
async fn count_matching_events_excludes_rows_outside_range_sad_path() {
    let router = paging_router().await;
    let count = count_matching_events(&router, &paging_query(0, 50, GridFilterModel::default()))
        .await
        .expect("count");
    assert_eq!(count, 7, "the day-old row is outside the 1h window");

    let empty = count_matching_events(
        &router,
        &EventQuery {
            table: "integ.no_rows".into(),
            ..paging_query(0, 50, GridFilterModel::default())
        },
    )
    .await
    .expect("empty count");
    assert_eq!(empty, 0);
}

#[tokio::test]
async fn count_matching_events_rejects_bad_table_sad_path() {
    let router = paging_router().await;
    let err = count_matching_events(
        &router,
        &EventQuery {
            table: "integ; DROP".into(),
            ..paging_query(0, 50, GridFilterModel::default())
        },
    )
    .await
    .expect_err("bad identifier");
    assert!(matches!(err, spectra_core::Error::Config(_)), "{err:?}");
}

/// Rows work but aggregates fail, like an engine without the JSON functions.
struct NoAggregateBackend(MemEventsBackend);

#[async_trait::async_trait]
impl EventStorageBackend for NoAggregateBackend {
    fn engine_type(&self) -> spectra_core::StorageEngineType {
        self.0.engine_type()
    }

    async fn append_row(
        &self,
        table: &str,
        fields: &serde_json::Value,
        ts: chrono::DateTime<Utc>,
        correlation_id: Option<&str>,
    ) -> spectra_core::Result<()> {
        self.0.append_row(table, fields, ts, correlation_id).await
    }

    async fn query_rows(
        &self,
        filter: spectra_core::EventsQueryFilter,
    ) -> spectra_core::Result<Vec<spectra_core::EventRow>> {
        self.0.query_rows(filter).await
    }

    async fn query_aggregate(
        &self,
        _filter: spectra_core::EventsAggregateFilter,
    ) -> spectra_core::Result<spectra_core::EventAggregateResult> {
        Err(spectra_core::Error::storage("aggregate unsupported"))
    }
}

#[tokio::test]
async fn execute_event_query_falls_back_to_rows_seen_when_count_fails_sad_path() {
    let inner = MemEventsBackend::new();
    let now = Utc::now();
    for i in 0..5i64 {
        inner
            .append_row(
                PAGING_TABLE,
                &json!({ "id": format!("f-{i}") }),
                now - chrono::Duration::minutes(i + 1),
                None,
            )
            .await
            .expect("append");
    }
    let router = router_with_events(Arc::new(NoAggregateBackend(inner)));
    let page1 = execute_event_query(&router, &paging_query(1, 2, GridFilterModel::default()))
        .await
        .expect("rows still load");
    assert_eq!(page1.rows.len(), 2);
    assert_eq!(page1.row_count, 4, "offset 2 + 2 rows seen");
}

#[test]
fn dashboard_catalog_summary_counts_happy_path() {
    let summary = dashboard_catalog_summary(DEFAULT_RECENT_SCHEMA_LIMIT);
    assert_eq!(
        summary.schema_count,
        summary.event_table_count
            + summary.metric_count
            + u64::try_from(
                summary
                    .recent_schemas
                    .iter()
                    .filter(|s| s.logging_kind != "event" && s.logging_kind != "metric")
                    .count()
            )
            .unwrap_or(0)
    );
    assert!(summary.recent_schemas.len() <= DEFAULT_RECENT_SCHEMA_LIMIT);
}
