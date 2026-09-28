//! Map Spectra query DTOs to orbital-charts series and axis definitions.

use orbital_charts::{AxisDef, AxisPosition, ChartType, ScaleType, SeriesDef};
use spectra_core::{MetricPointDto, SliceDto, TimeSeriesDto};

/// Display width used when the chart host has not measured yet (full-bleed canvas).
const DISPLAY_SAMPLE_WIDTH_PX: f64 = 1200.0;

/// Cap drawn points for a CSS width (same policy as `orbital_charts::sample_series_for_width`).
///
/// Kept local so hosts can path-patch `spectra-app` against git Orbital that does not
/// yet export that helper.
fn sample_series_for_width<T: Clone>(points: &[T], width_px: f64) -> Vec<T> {
    if points.is_empty() {
        return Vec::new();
    }
    if !width_px.is_finite() || width_px <= 0.0 {
        return Vec::new();
    }
    if points.len() <= 2 {
        return points.to_vec();
    }

    let budget = (width_px / 2.0).ceil() as usize + 2;
    if points.len() <= budget {
        return points.to_vec();
    }

    let mut out = Vec::with_capacity(budget);
    out.push(points[0].clone());
    let inner = budget - 2;
    let last = points.len() - 1;
    for i in 1..=inner {
        let idx = (i * last) / (inner + 1);
        out.push(points[idx].clone());
    }
    out.push(points[last].clone());
    out
}

/// Sort points by timestamp, then sample for display width.
fn sorted_sampled_points(points: &[MetricPointDto]) -> Vec<MetricPointDto> {
    let mut sorted = points.to_vec();
    sorted.sort_by_key(|p| p.ts);
    sample_series_for_width(&sorted, DISPLAY_SAMPLE_WIDTH_PX)
}

/// Unique band category for Orbital [`BandScale`] (first-match lookup).
///
/// Bare `%H:%M` collides across minutes and days and folds the polyline left.
fn unique_band_category(point: &MetricPointDto) -> String {
    point.ts.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Short display label for a bucket, distinct from its unique scale key
/// ([`unique_band_category`]) so dense axes (many buckets) don't render the full timestamp —
/// see [`AxisDef::tick_labels`]. `%H:%M` when the bucket falls on the same calendar day as the
/// previous bucket (the common case for hourly/minute buckets within one day); `%b %d` when it
/// crosses a day boundary from the previous bucket, or is the first bucket, so a multi-day
/// range still shows a date instead of a bare time that could belong to any day.
fn short_display_label(point: &MetricPointDto, previous: Option<&MetricPointDto>) -> String {
    let same_day = previous.is_some_and(|p| p.ts.date_naive() == point.ts.date_naive());
    if same_day {
        point.ts.format("%H:%M").to_string()
    } else {
        point.ts.format("%b %d").to_string()
    }
}

/// Short display labels for a sorted run of points — see [`short_display_label`].
fn short_display_labels(points: &[MetricPointDto]) -> Vec<String> {
    let mut labels = Vec::with_capacity(points.len());
    let mut previous: Option<&MetricPointDto> = None;
    for point in points {
        labels.push(short_display_label(point, previous));
        previous = Some(point);
    }
    labels
}

/// Builds x-axis categories and line/bar series from time-series DTOs.
///
/// Dense series are capped with [`sample_series_for_width`] (local helper) so drawing
/// stays within about one point per two CSS pixels for [`DISPLAY_SAMPLE_WIDTH_PX`].
#[must_use]
pub fn chart_from_time_series(series: &[TimeSeriesDto]) -> (Vec<AxisDef>, Vec<SeriesDef>) {
    if series.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let sampled_primary = sorted_sampled_points(&series[0].points);
    let categories: Vec<String> = sampled_primary.iter().map(unique_band_category).collect();
    let tick_labels = short_display_labels(&sampled_primary);

    let x_axis = vec![AxisDef {
        id: "x".to_string(),
        scale_type: ScaleType::Band,
        data: Some(categories),
        tick_labels: Some(tick_labels),
        position: AxisPosition::Bottom,
        ..Default::default()
    }];

    let chart_series: Vec<SeriesDef> = series
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let label = match &s.labels {
                serde_json::Value::String(name) if !name.is_empty() => name.clone(),
                _ => format!("series-{i}"),
            };
            let points = if i == 0 {
                sampled_primary.clone()
            } else {
                sorted_sampled_points(&s.points)
            };
            SeriesDef {
                id: format!("series-{i}"),
                label: Some(label),
                chart_type: Some(ChartType::Line),
                data: Some(points.iter().map(|p| p.value).collect()),
                ..Default::default()
            }
        })
        .collect();

    (x_axis, chart_series)
}

/// Builds pie/bar chart series from slice DTOs.
#[must_use]
pub fn chart_from_slices(
    slices: &[SliceDto],
    chart_type: ChartType,
) -> (Vec<AxisDef>, Vec<SeriesDef>) {
    if slices.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let labels: Vec<String> = slices.iter().map(|s| s.label.clone()).collect();
    let values: Vec<f64> = slices.iter().map(|s| s.value).collect();

    let x_axis = vec![AxisDef {
        id: "x".to_string(),
        scale_type: ScaleType::Band,
        data: Some(labels),
        position: AxisPosition::Bottom,
        ..Default::default()
    }];

    let series = vec![SeriesDef {
        id: "slices".to_string(),
        label: Some("Value".to_string()),
        chart_type: Some(chart_type),
        data: Some(values),
        ..Default::default()
    }];

    (x_axis, series)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use spectra_core::MetricPointDto;

    #[test]
    fn chart_from_time_series_maps_points_happy_path() {
        let now = Utc::now();
        let series = vec![TimeSeriesDto {
            labels: serde_json::json!({"name": "cpu"}),
            points: vec![
                MetricPointDto {
                    ts: now,
                    value: 1.0,
                },
                MetricPointDto {
                    ts: now + chrono::Duration::minutes(1),
                    value: 2.0,
                },
            ],
        }];
        let (x_axis, chart_series) = chart_from_time_series(&series);
        assert_eq!(x_axis.len(), 1);
        assert_eq!(x_axis[0].data.as_ref().map(|d| d.len()), Some(2));
        assert_eq!(chart_series.len(), 1);
        assert_eq!(chart_series[0].data.as_ref().map(|d| d.len()), Some(2));
        let cats = x_axis[0].data.as_ref().expect("categories");
        assert_ne!(cats[0], cats[1]);
    }

    #[test]
    fn chart_from_time_series_unique_categories_same_minute() {
        let now = Utc::now();
        let series = vec![TimeSeriesDto {
            labels: serde_json::json!("dup-minute"),
            points: vec![
                MetricPointDto {
                    ts: now,
                    value: 1.0,
                },
                MetricPointDto {
                    ts: now + chrono::Duration::seconds(30),
                    value: 2.0,
                },
                MetricPointDto {
                    ts: now + chrono::Duration::seconds(45),
                    value: 3.0,
                },
            ],
        }];
        let (x_axis, _) = chart_from_time_series(&series);
        let cats = x_axis[0].data.as_ref().expect("categories");
        assert_eq!(cats.len(), 3);
        let unique: std::collections::HashSet<_> = cats.iter().collect();
        assert_eq!(
            unique.len(),
            3,
            "Band labels must be unique so the polyline stays L→R"
        );
    }

    #[test]
    fn chart_from_time_series_tick_labels_are_shorter_than_keys_same_day() {
        // Fixed timestamps (not Utc::now()) so this stays deterministic regardless of when the
        // suite runs — a real-clock version would be day-boundary flaky by design here.
        let base = Utc.with_ymd_and_hms(2026, 1, 5, 9, 0, 0).unwrap();
        let series = vec![TimeSeriesDto {
            labels: serde_json::json!("same-day"),
            points: vec![
                MetricPointDto {
                    ts: base,
                    value: 1.0,
                },
                MetricPointDto {
                    ts: base + chrono::Duration::hours(1),
                    value: 2.0,
                },
                MetricPointDto {
                    ts: base + chrono::Duration::hours(2),
                    value: 3.0,
                },
            ],
        }];
        let (x_axis, _) = chart_from_time_series(&series);
        let keys = x_axis[0].data.as_ref().expect("categories");
        let labels = x_axis[0].tick_labels.as_ref().expect("tick_labels");
        assert_eq!(labels.len(), keys.len());
        for (key, label) in keys.iter().zip(labels.iter()) {
            assert!(
                label.len() < key.len(),
                "display label {label:?} should be shorter than scale key {key:?}"
            );
        }
        // First bucket has no previous point -> date anchor; the rest share its day -> time only.
        assert_eq!(labels[0], "Jan 05");
        assert_eq!(labels[1], "10:00");
        assert_eq!(labels[2], "11:00");
    }

    #[test]
    fn chart_from_time_series_tick_labels_show_date_across_day_boundary() {
        let base = Utc.with_ymd_and_hms(2026, 1, 5, 23, 0, 0).unwrap();
        let series = vec![TimeSeriesDto {
            labels: serde_json::json!("crosses-midnight"),
            points: vec![
                MetricPointDto {
                    ts: base,
                    value: 1.0,
                },
                // Next calendar day -> falls back to a date label instead of a bare time that
                // could be mistaken for the previous day's 00:00.
                MetricPointDto {
                    ts: base + chrono::Duration::hours(2),
                    value: 2.0,
                },
            ],
        }];
        let (x_axis, _) = chart_from_time_series(&series);
        let labels = x_axis[0].tick_labels.as_ref().expect("tick_labels");
        assert_eq!(
            labels[0], "Jan 05",
            "first bucket has no previous point -> date label"
        );
        assert_eq!(
            labels[1], "Jan 06",
            "crossed midnight from the previous bucket -> date label"
        );
    }

    #[test]
    fn chart_from_time_series_sorts_out_of_order_points() {
        let now = Utc::now();
        let series = vec![TimeSeriesDto {
            labels: serde_json::json!("unsorted"),
            points: vec![
                MetricPointDto {
                    ts: now + chrono::Duration::minutes(2),
                    value: 3.0,
                },
                MetricPointDto {
                    ts: now,
                    value: 1.0,
                },
                MetricPointDto {
                    ts: now + chrono::Duration::minutes(1),
                    value: 2.0,
                },
            ],
        }];
        let (_, chart_series) = chart_from_time_series(&series);
        assert_eq!(
            chart_series[0].data.as_ref().map(Vec::as_slice),
            Some([1.0, 2.0, 3.0].as_slice())
        );
    }

    #[test]
    fn chart_from_time_series_samples_dense_series() {
        let now = Utc::now();
        let points: Vec<MetricPointDto> = (0..10_000)
            .map(|i| MetricPointDto {
                ts: now + chrono::Duration::seconds(i),
                value: i as f64,
            })
            .collect();
        let series = vec![TimeSeriesDto {
            labels: serde_json::json!("dense"),
            points,
        }];
        let (_, chart_series) = chart_from_time_series(&series);
        let len = chart_series[0].data.as_ref().map(|d| d.len()).unwrap_or(0);
        assert!(len <= (DISPLAY_SAMPLE_WIDTH_PX / 2.0).ceil() as usize + 2);
    }

    #[test]
    fn chart_from_slices_maps_labels_happy_path() {
        let slices = vec![
            SliceDto {
                label: "a".into(),
                value: 3.0,
            },
            SliceDto {
                label: "b".into(),
                value: 7.0,
            },
        ];
        let (_, series) = chart_from_slices(&slices, ChartType::Pie);
        assert_eq!(series[0].data.as_ref().map(|d| d.len()), Some(2));
    }
}
