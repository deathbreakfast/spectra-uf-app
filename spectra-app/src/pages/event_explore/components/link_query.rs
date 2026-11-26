//! Map a query link onto the event explorer's view and aggregation settings and back.

use spectra_backend::{QueryBucket, QueryLink, QueryMeasure, QueryView};
use spectra_core::{EventAggregationSpec, EventExploreView, EventMeasure};

use crate::components::query::normalize_view;

/// Default time bucket when a link has none: one hour.
pub const DEFAULT_BUCKET_SECS: u64 = 3_600;

/// Explorer view for a link view. No view, and `stat` (which the event explorer doesn't
/// draw), open on the Event log.
#[must_use]
pub const fn explore_view(view: Option<QueryView>) -> EventExploreView {
    match view {
        Some(QueryView::Series) => EventExploreView::TimeSeries,
        Some(QueryView::Bar) => EventExploreView::BarChart,
        Some(QueryView::Pie) => EventExploreView::PieChart,
        Some(QueryView::Table) => EventExploreView::Table,
        Some(QueryView::Log | QueryView::Stat) | None => EventExploreView::EventLog,
    }
}

/// Link view for an explorer view.
#[must_use]
pub const fn link_view(view: EventExploreView) -> QueryView {
    match normalize_view(view) {
        EventExploreView::TimeSeries | EventExploreView::LineChart => QueryView::Series,
        EventExploreView::BarChart => QueryView::Bar,
        EventExploreView::PieChart => QueryView::Pie,
        EventExploreView::Table => QueryView::Table,
        EventExploreView::EventLog => QueryView::Log,
    }
}

fn link_measure(link: &QueryLink) -> (EventMeasure, Option<String>) {
    match &link.measure {
        Some(QueryMeasure::Sum(field)) => (EventMeasure::Sum, Some(field.clone())),
        Some(QueryMeasure::Count) | None => (EventMeasure::Count, None),
    }
}

/// Aggregation request for a link: grouping only where `view` uses it.
#[must_use]
pub fn aggregation_for(link: &QueryLink, view: EventExploreView) -> EventAggregationSpec {
    let (measure, measure_field) = link_measure(link);
    let view = normalize_view(view);
    let slices = matches!(
        view,
        EventExploreView::BarChart | EventExploreView::PieChart
    );
    let table = view == EventExploreView::Table;
    EventAggregationSpec {
        measure,
        measure_field,
        time_bucket_secs: Some(link.bucket.map_or(DEFAULT_BUCKET_SECS, QueryBucket::secs)),
        group_by_field: link.group.clone().filter(|_| slices),
        row_fields: link.group.iter().filter(|_| table).cloned().collect(),
        pivot_field: link.split.clone().filter(|_| table),
    }
}

/// Starting values for the aggregation controls. The group field seeds both Group by and
/// Rows; the controls keep whichever the picked view shows.
#[must_use]
pub fn aggregation_seed(link: &QueryLink) -> EventAggregationSpec {
    let (measure, measure_field) = link_measure(link);
    EventAggregationSpec {
        measure,
        measure_field,
        time_bucket_secs: Some(link.bucket.map_or(DEFAULT_BUCKET_SECS, QueryBucket::secs)),
        group_by_field: link.group.clone(),
        row_fields: link.group.iter().cloned().collect(),
        pivot_field: link.split.clone(),
    }
}

/// Copies the controls' aggregation into `link`, leaving a key unset while the controls
/// still match its default.
pub fn apply_aggregation(link: &mut QueryLink, spec: &EventAggregationSpec) {
    let measure = match (spec.measure, &spec.measure_field) {
        (EventMeasure::Sum, Some(field)) => QueryMeasure::Sum(field.clone()),
        _ => QueryMeasure::Count,
    };
    if link.measure.clone().unwrap_or(QueryMeasure::Count) != measure {
        link.measure = Some(measure);
    }
    let bucket = spec.time_bucket_secs.unwrap_or(DEFAULT_BUCKET_SECS);
    if link.bucket.map_or(DEFAULT_BUCKET_SECS, QueryBucket::secs) != bucket {
        link.bucket = QueryBucket::from_secs(bucket);
    }
    let group = spec
        .group_by_field
        .clone()
        .or_else(|| spec.row_fields.first().cloned());
    if link.group != group {
        link.group = group;
    }
    if link.split != spec.pivot_field {
        link.split.clone_from(&spec.pivot_field);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_round_trip_happy_path() {
        for view in [
            EventExploreView::EventLog,
            EventExploreView::TimeSeries,
            EventExploreView::BarChart,
            EventExploreView::PieChart,
            EventExploreView::Table,
        ] {
            assert_eq!(explore_view(Some(link_view(view))), view);
        }
        assert_eq!(explore_view(None), EventExploreView::EventLog);
    }

    #[test]
    fn stat_view_opens_event_log_sad_path() {
        assert_eq!(
            explore_view(Some(QueryView::Stat)),
            EventExploreView::EventLog
        );
    }

    #[test]
    fn aggregation_groups_only_where_view_uses_it_happy_path() {
        let mut link = QueryLink::event("t");
        link.group = Some("status".into());
        link.split = Some("region".into());
        link.measure = Some(QueryMeasure::Sum("value".into()));
        link.bucket = QueryBucket::from_secs(150);

        let bar = aggregation_for(&link, EventExploreView::BarChart);
        assert_eq!(bar.group_by_field.as_deref(), Some("status"));
        assert!(bar.row_fields.is_empty() && bar.pivot_field.is_none());
        assert_eq!(bar.measure, EventMeasure::Sum);
        assert_eq!(bar.measure_field.as_deref(), Some("value"));
        assert_eq!(bar.time_bucket_secs, Some(150));

        let table = aggregation_for(&link, EventExploreView::Table);
        assert_eq!(table.row_fields, vec!["status".to_string()]);
        assert_eq!(table.pivot_field.as_deref(), Some("region"));
        assert!(table.group_by_field.is_none());

        let series = aggregation_for(&link, EventExploreView::TimeSeries);
        assert!(series.group_by_field.is_none() && series.row_fields.is_empty());
    }

    #[test]
    fn unchanged_controls_leave_link_alone_happy_path() {
        let bare = QueryLink::event("t");
        let mut link = bare.clone();
        apply_aggregation(&mut link, &aggregation_seed(&bare));
        assert_eq!(link, bare, "a bare link stays bare");

        let mut changed = bare.clone();
        let mut spec = aggregation_seed(&bare);
        spec.measure = EventMeasure::Sum;
        spec.measure_field = Some("value".into());
        spec.time_bucket_secs = Some(300);
        spec.group_by_field = Some("status".into());
        apply_aggregation(&mut changed, &spec);
        assert_eq!(changed.measure, Some(QueryMeasure::Sum("value".into())));
        assert_eq!(changed.bucket, QueryBucket::from_secs(300));
        assert_eq!(changed.group.as_deref(), Some("status"));
    }
}
