//! Live Spectra query execution against an installed [`SpectraRouter`].
//!
//! Maps UI DTOs to storage filters, runs router queries, and maps results back.
//! Used by `spectra-app` server functions when a host registers a global router.

use spectra_core::{
    aggregate_request_to_filter, event_query_to_filter, metrics_query_to_range,
    points_to_metrics_result, rows_to_event_result, EventAggregateRequest, EventAggregateResult,
    EventAggregationSpec, EventExploreView, EventMeasure, EventQuery, EventQueryResult,
    MetricsQuery, MetricsQueryResult, SpectraRouter,
};

/// Runs a metric explore query through the router.
///
/// # Errors
///
/// Returns [`spectra_core::Error`] when identifier validation or backend query fails.
pub async fn execute_metrics_query(
    router: &SpectraRouter,
    query: &MetricsQuery,
) -> Result<MetricsQueryResult, spectra_core::Error> {
    let range = metrics_query_to_range(query);
    let points = router.query_metrics(range).await?;
    Ok(points_to_metrics_result(points))
}

/// Runs one page of an event log query through the router.
///
/// `row_count` is the number of rows matching the table, time range, partition, and filter
/// across all pages, from [`count_matching_events`]. When that count fails (a backend that
/// cannot aggregate), `row_count` falls back to the rows seen through this page so the log
/// still renders.
///
/// # Errors
///
/// Returns [`spectra_core::Error`] when identifier validation or the row query fails.
pub async fn execute_event_query(
    router: &SpectraRouter,
    query: &EventQuery,
) -> Result<EventQueryResult, spectra_core::Error> {
    let filter = event_query_to_filter(query);
    let offset = u64::from(filter.offset.unwrap_or(0));
    let rows = router.query_events(filter).await?;
    let seen = offset.saturating_add(u64::try_from(rows.len()).unwrap_or(u64::MAX));
    let row_count = match count_matching_events(router, query).await {
        Ok(total) => total.max(seen),
        Err(err) => {
            log::warn!(
                "spectra event count failed; using rows seen (table={}, error={err})",
                query.table
            );
            seen
        }
    };
    Ok(rows_to_event_result(&query.table, rows, row_count))
}

/// Counts events matching `query`'s table, time range, partition, and filter.
///
/// Paging and sort are ignored. Runs a Count aggregate whose bucket spans the whole range,
/// so the backend returns one or two buckets instead of rows.
///
/// # Errors
///
/// Returns [`spectra_core::Error`] when identifier validation or the aggregate query fails.
pub async fn count_matching_events(
    router: &SpectraRouter,
    query: &EventQuery,
) -> Result<u64, spectra_core::Error> {
    let span_secs = u64::try_from((query.end - query.start).num_seconds())
        .unwrap_or(1)
        .max(1);
    let request = EventAggregateRequest {
        table: query.table.clone(),
        start: query.start,
        end: query.end,
        partition: query.partition,
        filter: query.filter.clone(),
        view: EventExploreView::TimeSeries,
        aggregation: EventAggregationSpec {
            measure: EventMeasure::Count,
            time_bucket_secs: Some(span_secs),
            ..Default::default()
        },
    };
    let result = execute_event_aggregate(router, &request).await?;
    Ok(time_series_count(&result))
}

/// Runs an event aggregate (chart) query through the router.
///
/// # Errors
///
/// Returns [`spectra_core::Error`] when identifier validation or backend query fails.
pub async fn execute_event_aggregate(
    router: &SpectraRouter,
    request: &EventAggregateRequest,
) -> Result<EventAggregateResult, spectra_core::Error> {
    let filter = aggregate_request_to_filter(request);
    router.query_event_aggregate(filter).await
}

fn time_series_count(result: &EventAggregateResult) -> u64 {
    let EventAggregateResult::TimeSeries { series, .. } = result else {
        return 0;
    };
    series
        .iter()
        .flat_map(|s| s.points.iter())
        .map(|p| count_from_f64(p.value))
        .fold(0, u64::saturating_add)
}

/// Converts a bucket count (`f64` on the wire) to `u64`; NaN and negatives become zero.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "value is positive and finite; float-to-int `as` saturates at u64::MAX"
)]
fn count_from_f64(value: f64) -> u64 {
    if value.is_finite() && value > 0.0 {
        value.round() as u64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectra_core::{MetricPointDto, TimeSeriesDto};

    #[test]
    fn time_series_count_sums_buckets_and_ignores_bad_values() {
        let point = |value| MetricPointDto {
            ts: chrono::Utc::now(),
            value,
        };
        let result = EventAggregateResult::TimeSeries {
            series: vec![TimeSeriesDto {
                labels: serde_json::json!({}),
                points: vec![point(3.0), point(2.0), point(-1.0), point(f64::NAN)],
            }],
            headline: Vec::new(),
        };
        assert_eq!(time_series_count(&result), 5);
    }

    #[test]
    fn time_series_count_is_zero_for_other_shapes() {
        let result = EventAggregateResult::Slices {
            slices: Vec::new(),
            headline: Vec::new(),
        };
        assert_eq!(time_series_count(&result), 0);
    }
}
