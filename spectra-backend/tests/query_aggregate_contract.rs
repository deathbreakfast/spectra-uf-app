//! Live aggregate query contracts against mem Spectra backends.

#![allow(missing_docs, clippy::expect_used)]

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use spectra::{MemEventsBackend, MemMetricsBackend, Spectra};
use spectra_backend::execute_event_aggregate;
use spectra_core::{
    EventAggregateRequest, EventAggregateResult, EventAggregationSpec, EventExploreView,
    EventMeasure, EventStorageBackend, GridFilterModel, MetricsStorageBackend, SpectraRouter,
};

const SEED_TABLE: &str = "agg.events";

async fn seeded_router() -> Arc<SpectraRouter> {
    let metrics: Arc<MemMetricsBackend> = Arc::new(MemMetricsBackend::new());
    let events: Arc<MemEventsBackend> = Arc::new(MemEventsBackend::new());
    let ts = Utc::now() - chrono::Duration::minutes(5);
    for (id, severity, value) in [("a1", "info", 10), ("a2", "info", 5), ("a3", "warn", 3)] {
        events
            .append_row(
                SEED_TABLE,
                &json!({"id": id, "severity": severity, "value": value}),
                ts,
                None,
            )
            .await
            .expect("append");
    }
    Spectra::builder()
        .metrics_backend(Arc::clone(&metrics) as Arc<dyn MetricsStorageBackend>)
        .events_backend(Arc::clone(&events) as Arc<dyn EventStorageBackend>)
        .embedded()
        .build()
        .expect("mem spectra")
        .router()
}

fn aggregate_request(
    view: EventExploreView,
    aggregation: EventAggregationSpec,
) -> EventAggregateRequest {
    EventAggregateRequest {
        table: SEED_TABLE.into(),
        start: Utc::now() - chrono::Duration::hours(1),
        end: Utc::now(),
        partition: None,
        filter: GridFilterModel::default(),
        view,
        aggregation,
    }
}

#[tokio::test]
async fn execute_event_aggregate_count_happy_path() {
    let router = seeded_router().await;
    let result = execute_event_aggregate(
        &router,
        &aggregate_request(
            EventExploreView::TimeSeries,
            EventAggregationSpec {
                measure: EventMeasure::Count,
                measure_field: None,
                time_bucket_secs: Some(3600),
                group_by_field: None,
                row_fields: vec![],
                pivot_field: None,
            },
        ),
    )
    .await
    .expect("aggregate");
    match result {
        EventAggregateResult::TimeSeries { series, .. } => {
            let total: f64 = series
                .iter()
                .flat_map(|s| s.points.iter())
                .map(|p| p.value)
                .sum();
            assert!(
                total >= 3.0,
                "count series should reflect seeded rows: {series:?}"
            );
        }
        EventAggregateResult::Slices { .. } | EventAggregateResult::Pivot { .. } => {
            panic!("expected time series aggregate")
        }
    }
}

#[tokio::test]
async fn execute_event_aggregate_sum_mem_backend_happy_path() {
    let router = seeded_router().await;
    let result = execute_event_aggregate(
        &router,
        &aggregate_request(
            EventExploreView::TimeSeries,
            EventAggregationSpec {
                measure: EventMeasure::Sum,
                measure_field: Some("value".into()),
                time_bucket_secs: Some(3600),
                group_by_field: None,
                row_fields: vec![],
                pivot_field: None,
            },
        ),
    )
    .await
    .expect("aggregate sum");
    match result {
        EventAggregateResult::TimeSeries { series, .. } => {
            let series_total: f64 = series
                .iter()
                .flat_map(|s| s.points.iter())
                .map(|p| p.value)
                .sum();
            assert!(
                (series_total - 18.0).abs() < f64::EPSILON,
                "sum of seeded values should be 18: series={series:?}"
            );
        }
        EventAggregateResult::Slices { .. } | EventAggregateResult::Pivot { .. } => {
            panic!("expected time series aggregate")
        }
    }
}

#[tokio::test]
async fn execute_event_aggregate_group_by_slices_happy_path() {
    let router = seeded_router().await;
    let result = execute_event_aggregate(
        &router,
        &aggregate_request(
            EventExploreView::PieChart,
            EventAggregationSpec {
                measure: EventMeasure::Count,
                measure_field: None,
                time_bucket_secs: None,
                group_by_field: Some("severity".into()),
                row_fields: vec![],
                pivot_field: None,
            },
        ),
    )
    .await
    .expect("aggregate slices");
    match result {
        EventAggregateResult::Slices { slices, .. } => {
            assert!(
                slices.len() >= 2,
                "expected info and warn slices: {slices:?}"
            );
            let labels: Vec<_> = slices.iter().map(|s| s.label.as_str()).collect();
            assert!(labels.contains(&"info"), "{labels:?}");
            assert!(labels.contains(&"warn"), "{labels:?}");
        }
        EventAggregateResult::TimeSeries { series, .. } => {
            panic!("expected Slices for PieChart group_by, got TimeSeries: {series:?}");
        }
        EventAggregateResult::Pivot { .. } => panic!("expected Slices, got Pivot"),
    }
}

#[tokio::test]
async fn execute_event_aggregate_table_pivot_happy_path() {
    let router = seeded_router().await;
    let result = execute_event_aggregate(
        &router,
        &aggregate_request(
            EventExploreView::Table,
            EventAggregationSpec {
                measure: EventMeasure::Count,
                measure_field: None,
                time_bucket_secs: None,
                group_by_field: None,
                row_fields: vec!["severity".into()],
                pivot_field: None,
            },
        ),
    )
    .await
    .expect("aggregate table");
    match result {
        EventAggregateResult::Pivot {
            row_fields,
            column_keys,
            rows,
            ..
        } => {
            assert_eq!(row_fields, vec!["severity".to_string()]);
            assert_eq!(column_keys.len(), 1);
            assert!(rows.len() >= 2, "expected severity groups: {rows:?}");
        }
        other => panic!("expected Pivot, got {other:?}"),
    }
}

#[tokio::test]
async fn execute_event_aggregate_empty_table_count_sad_path() {
    let metrics: Arc<MemMetricsBackend> = Arc::new(MemMetricsBackend::new());
    let events: Arc<MemEventsBackend> = Arc::new(MemEventsBackend::new());
    let router = Spectra::builder()
        .metrics_backend(Arc::clone(&metrics) as Arc<dyn MetricsStorageBackend>)
        .events_backend(Arc::clone(&events) as Arc<dyn EventStorageBackend>)
        .embedded()
        .build()
        .expect("mem spectra")
        .router();
    let result = execute_event_aggregate(
        &router,
        &aggregate_request(
            EventExploreView::TimeSeries,
            EventAggregationSpec {
                measure: EventMeasure::Count,
                measure_field: None,
                time_bucket_secs: Some(3600),
                group_by_field: None,
                row_fields: vec![],
                pivot_field: None,
            },
        ),
    )
    .await
    .expect("empty aggregate");
    match result {
        EventAggregateResult::TimeSeries { headline, series } => {
            let total: f64 = series
                .iter()
                .flat_map(|s| s.points.iter())
                .map(|p| p.value)
                .sum();
            assert!(
                total == 0.0
                    && (series.is_empty()
                        || series.iter().all(|s| s.points.is_empty())
                        || headline.iter().any(|c| c.label == "Total" && c.value == "0")),
                "empty table should yield zero series values: series={series:?} headline={headline:?}"
            );
        }
        EventAggregateResult::Slices { slices, .. } => assert!(slices.is_empty()),
        EventAggregateResult::Pivot { rows, .. } => assert!(rows.is_empty()),
    }
}
