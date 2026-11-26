//! Shared time-range and query-link helpers for explore pages.
//!
//! Each explorer keeps its query in the URL. [`use_explore_link`] reads it and writes
//! changes back by replacing the URL, so every control change leaves no history entry.

use chrono::{DateTime, Duration, Utc};
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use orbital::primitives::{DateTimeRange, DatetimeTimezone, OrbitalDateTime};
use spectra_backend::{QueryLink, QueryWindow};

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

/// Orbital datetime range covering `window`, a relative window ending now.
///
/// Endpoints use [`DatetimeTimezone::Local`], as in [`default_explore_datetime_range`].
#[must_use]
pub fn window_datetime_range(window: QueryWindow) -> DateTimeRange {
    let (start, end) = window.bounds(Utc::now());
    DateTimeRange::new(
        OrbitalDateTime::from_instant(start, DatetimeTimezone::Local),
        OrbitalDateTime::from_instant(end, DatetimeTimezone::Local),
    )
}

/// Fixed window for a picked range, in whole seconds.
#[must_use]
pub fn window_from_datetime_range(range: &DateTimeRange) -> QueryWindow {
    let (start, end) = range_from_datetime_range(range);
    let whole = |t: DateTime<Utc>| DateTime::<Utc>::from_timestamp(t.timestamp(), 0).unwrap_or(t);
    QueryWindow::Range {
        from: whole(start),
        to: whole(end),
    }
}

/// Whether two ranges start and end within a minute of each other.
///
/// The range picker works in minutes, so a smaller difference is the same pick, not a
/// new one.
#[must_use]
pub fn same_picked_range(a: &DateTimeRange, b: &DateTimeRange) -> bool {
    let (a_start, a_end) = range_from_datetime_range(a);
    let (b_start, b_end) = range_from_datetime_range(b);
    (a_start - b_start).num_seconds().abs() < 60 && (a_end - b_end).num_seconds().abs() < 60
}

/// The explore query in the current URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreLink {
    /// The query.
    pub link: QueryLink,
    /// Query keys the explorer doesn't use.
    pub ignored_keys: Vec<String>,
    /// Why the query string couldn't be read; `link` then holds only what the path says.
    pub error: Option<String>,
}

/// Reads the explore query from a URL path and query string (with or without `?`).
///
/// A bad query string falls back to the path alone and reports why in
/// [`ExploreLink::error`]. Returns `None` when the path isn't an explore path.
#[must_use]
pub fn explore_link_from_url(path: &str, search: &str) -> Option<ExploreLink> {
    let search = search.strip_prefix('?').unwrap_or(search);
    match QueryLink::decode(&format!("{path}?{search}")) {
        Ok(decoded) => Some(ExploreLink {
            link: decoded.link,
            ignored_keys: decoded.ignored_keys,
            error: None,
        }),
        Err(err) => QueryLink::decode(path).ok().map(|decoded| ExploreLink {
            link: decoded.link,
            ignored_keys: Vec::new(),
            error: Some(err.to_string()),
        }),
    }
}

/// How many of our own writes to remember while the router catches up.
const PENDING_WRITES: usize = 16;

/// Reads and writes the explore query in the URL.
#[derive(Clone, Copy)]
pub struct ExploreLinkState {
    /// Query in the address bar.
    pub current: Memo<Option<ExploreLink>>,
    /// Goes up when the URL changes for a reason other than [`ExploreLinkState::update`]
    /// (a pasted link, a client-side navigation), so controls can reseed from it.
    pub seed_rev: ReadSignal<u64>,
    written: StoredValue<Vec<QueryLink>>,
    navigate: Callback<String>,
}

impl ExploreLinkState {
    /// Latest query: the last one written, or the URL's when nothing is pending.
    #[must_use]
    pub fn latest(&self) -> Option<QueryLink> {
        self.written.with_value(|w| w.last().cloned()).or_else(|| {
            self.current
                .with_untracked(|c| c.as_ref().map(|c| c.link.clone()))
        })
    }

    /// Applies `edit` to the latest query and replaces the URL with the result.
    ///
    /// Does nothing when `edit` leaves the query unchanged.
    pub fn update(&self, edit: impl FnOnce(&mut QueryLink)) {
        let Some(before) = self.latest() else {
            return;
        };
        let mut link = before.clone();
        edit(&mut link);
        if link == before {
            return;
        }
        let href = link.encode();
        self.written.update_value(|w| {
            w.push(link);
            if w.len() > PENDING_WRITES {
                w.remove(0);
            }
        });
        self.navigate.run(href);
    }
}

/// Explore query state for the current route.
pub fn use_explore_link() -> ExploreLinkState {
    let location = use_location();
    let current = Memo::new(move |_| {
        explore_link_from_url(&location.pathname.read(), &location.search.read())
    });
    let (seed_rev, set_seed_rev) = signal(0u64);
    let written = StoredValue::new(Vec::<QueryLink>::new());
    let navigate = use_navigate();
    let navigate = Callback::new(move |href: String| {
        navigate(
            &href,
            NavigateOptions {
                replace: true,
                scroll: false,
                ..NavigateOptions::default()
            },
        );
    });

    Effect::new(move |prev: Option<()>| {
        let link = current.with(|c| c.as_ref().map(|c| c.link.clone()));
        // The first run is the URL the page opened on; controls already start from it.
        if prev.is_none() {
            return;
        }
        let Some(link) = link else {
            return;
        };
        if let Some(i) = written.with_value(|w| w.iter().position(|l| *l == link)) {
            // One of our writes landed: forget the ones before it.
            written.update_value(|w| {
                w.drain(..i);
            });
        } else {
            written.update_value(Vec::clear);
            set_seed_rev.update(|n| *n += 1);
        }
    });

    ExploreLinkState {
        current,
        seed_rev,
        written,
        navigate,
    }
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

    #[test]
    fn explore_link_reads_path_and_query_happy_path() {
        let link = explore_link_from_url(
            "/spectra/schema/checkout_event/explore",
            "?window=6h&view=bar&utm_source=chat",
        )
        .unwrap();
        assert_eq!(
            link.link.window,
            QueryWindow::Last(spectra_backend::WindowPreset::Hours6)
        );
        assert_eq!(link.link.view, Some(spectra_backend::QueryView::Bar));
        assert_eq!(link.ignored_keys, vec!["utm_source"]);
        assert_eq!(link.error, None);
    }

    #[test]
    fn explore_link_bad_query_falls_back_to_path_sad_path() {
        let link =
            explore_link_from_url("/spectra/metric/api_latency/explore", "window=2h").unwrap();
        assert_eq!(link.link, QueryLink::metric("api_latency"));
        assert!(link.error.is_some_and(|e| e.contains("window")));
        assert_eq!(explore_link_from_url("/spectra/schema", ""), None);
    }

    #[test]
    fn picked_range_becomes_whole_second_window_happy_path() {
        let end = DateTime::<Utc>::from_timestamp(1_700_003_600, 500_000_000).unwrap();
        let start = end - Duration::hours(2);
        let range = DateTimeRange::new(
            OrbitalDateTime::from_instant(start, DatetimeTimezone::Local),
            OrbitalDateTime::from_instant(end, DatetimeTimezone::Local),
        );
        let window = window_from_datetime_range(&range);
        assert_eq!(
            window,
            QueryWindow::Range {
                from: DateTime::<Utc>::from_timestamp(1_700_003_600 - 7_200, 0).unwrap(),
                to: DateTime::<Utc>::from_timestamp(1_700_003_600, 0).unwrap(),
            }
        );
        assert!(same_picked_range(&range, &window_datetime_range(window)));
    }

    #[test]
    fn ranges_a_minute_apart_are_different_picks_sad_path() {
        let a = window_datetime_range(QueryWindow::Last(spectra_backend::WindowPreset::Hour1));
        let (start, end) = range_from_datetime_range(&a);
        let b = DateTimeRange::new(
            OrbitalDateTime::from_instant(start - Duration::minutes(5), DatetimeTimezone::Local),
            OrbitalDateTime::from_instant(end, DatetimeTimezone::Local),
        );
        assert!(!same_picked_range(&a, &b));
    }
}
