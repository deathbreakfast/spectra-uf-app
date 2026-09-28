//! Shared time-range helpers for explore pages.

use chrono::{DateTime, Duration, Utc};
use orbital::primitives::{DateTimeRange, DatetimeTimezone, OrbitalDateTime};

/// Inclusive wall-clock window ending at now, spanning `secs` seconds.
pub fn range_from_secs(secs: i64) -> (DateTime<Utc>, DateTime<Utc>) {
    let end = Utc::now();
    let start = end - Duration::seconds(secs);
    (start, end)
}

/// Default explore window: last one hour ending at now.
///
/// Endpoints use [`DatetimeTimezone::Local`] so they match Orbital
/// [`DateTimeRangePicker`](orbital::primitives::DateTimeRangePicker)'s default
/// appearance timezone. Utc-tagged values + Local picker round-trips shift the
/// bound instant and empty recent event queries.
#[must_use]
pub fn default_explore_datetime_range() -> DateTimeRange {
    let end = OrbitalDateTime::utc_now(DatetimeTimezone::Local);
    let start =
        OrbitalDateTime::from_instant(end.instant() - Duration::hours(1), DatetimeTimezone::Local);
    DateTimeRange::new(start, end)
}

/// Convert an Orbital datetime range into UTC chrono bounds (normalized).
#[must_use]
pub fn range_from_datetime_range(range: &DateTimeRange) -> (DateTime<Utc>, DateTime<Utc>) {
    let n = range.clone().normalized();
    (n.start.instant(), n.end.instant())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_from_secs_spans_requested_duration_happy_path() {
        let (start, end) = range_from_secs(3_600);
        let delta = (end - start).num_seconds();
        assert_eq!(delta, 3_600);
        assert!(end >= start);
    }

    #[test]
    fn range_from_secs_zero_collapses_to_now_happy_path() {
        let (start, end) = range_from_secs(0);
        assert_eq!(start, end);
    }

    #[test]
    fn range_from_secs_negative_inverts_window_sad() {
        let (start, end) = range_from_secs(-60);
        assert!(start > end, "negative secs yields inverted window");
        assert_eq!((start - end).num_seconds(), 60);
    }

    #[test]
    fn default_explore_datetime_range_is_one_hour() {
        let range = default_explore_datetime_range();
        let (start, end) = range_from_datetime_range(&range);
        let delta = (end - start).num_seconds();
        assert!((3_590..=3_610).contains(&delta), "delta={delta}");
    }

    #[test]
    fn range_from_datetime_range_normalizes_inverted() {
        let end = Utc::now();
        let start = end - Duration::hours(1);
        let inverted = DateTimeRange::new(
            OrbitalDateTime::from_instant(end, DatetimeTimezone::Local),
            OrbitalDateTime::from_instant(start, DatetimeTimezone::Local),
        );
        let (a, b) = range_from_datetime_range(&inverted);
        assert!(a <= b);
        assert_eq!((b - a).num_seconds(), 3_600);
    }
}
