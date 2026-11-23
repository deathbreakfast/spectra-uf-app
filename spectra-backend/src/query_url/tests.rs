use chrono::{DateTime, Utc};
use spectra_core::{SchemaDetailDto, SchemaFieldDto};

use super::*;

fn ts(secs: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(secs, 0).unwrap()
}

fn round_trip(link: &QueryLink) {
    let encoded = link.encode();
    let decoded = decode(&encoded).unwrap_or_else(|e| panic!("{encoded}: {e}"));
    assert_eq!(&decoded.link, link, "{encoded}");
    assert_eq!(decoded.ignored_keys, Vec::<String>::new());
}

fn schema(fields: &[&str]) -> SchemaDetailDto {
    SchemaDetailDto {
        table_or_metric: "checkout_event".into(),
        description: None,
        logging_kind: "event".into(),
        version: "1".into(),
        fields: fields
            .iter()
            .map(|name| SchemaFieldDto {
                name: (*name).to_string(),
                rust_type: "String".into(),
                classification: "public".into(),
            })
            .collect(),
        can_query: true,
    }
}

#[test]
fn every_event_view_round_trips_happy_path() {
    for view in QueryView::ALL {
        let mut link = QueryLink::event("checkout_event");
        link.view = Some(view);
        link.measure = Some(QueryMeasure::Count);
        link.group = Some("status".into());
        link.bucket = QueryBucket::from_secs(300);
        link.filters.push(LinkFilter {
            field: "status".into(),
            op: FilterOp::Eq,
            value: "failed".into(),
        });
        round_trip(&link);
    }
}

#[test]
fn full_event_link_round_trips_happy_path() {
    let mut link = QueryLink::event("ops.events");
    link.window = QueryWindow::Range {
        from: ts(1_700_000_000),
        to: ts(1_700_003_600),
    };
    link.view = Some(QueryView::Table);
    link.measure = Some(QueryMeasure::Sum("amount cents".into()));
    link.group = Some("region".into());
    link.split = Some("status".into());
    link.bucket = QueryBucket::from_secs(150);
    link.filters = FilterOp::ALL
        .into_iter()
        .map(|op| LinkFilter {
            field: "note:x".into(),
            op,
            value: "a:b c&d=e+f%".into(),
        })
        .collect();
    link.cols = vec!["ts".into(), "a,b".into(), "message".into()];
    link.sort = Some(QuerySort {
        field: "ts".into(),
        descending: true,
    });
    link.limit = Some(25);
    round_trip(&link);
}

#[test]
fn metric_links_round_trip_happy_path() {
    for preset in WindowPreset::ALL {
        let mut link = QueryLink::metric("api_latency_ms");
        link.window = QueryWindow::Last(preset);
        link.agg = Some("max".into());
        link.bucket = QueryBucket::from_secs(300);
        link.split = Some("route".into());
        round_trip(&link);
    }
    round_trip(&QueryLink::metric("platform_smoke_counter"));
}

#[test]
fn encode_matches_documented_forms_happy_path() {
    let mut link = QueryLink::event("checkout_event");
    link.view = Some(QueryView::Log);
    link.filters.push(LinkFilter {
        field: "status".into(),
        op: FilterOp::Eq,
        value: "failed".into(),
    });
    assert_eq!(
        link.encode(),
        "/spectra/schema/checkout_event/explore?window=1h&view=log&f=status:eq:failed"
    );

    let mut metric = QueryLink::metric("api_latency");
    metric.window = QueryWindow::Last(WindowPreset::Hours6);
    metric.agg = Some("max".into());
    metric.bucket = QueryBucket::from_secs(300);
    metric.split = Some("route".into());
    assert_eq!(
        metric.encode(),
        "/spectra/metric/api_latency/explore?window=6h&agg=max&split=route&bucket=5m"
    );
}

#[test]
fn full_url_on_any_host_decodes_happy_path() {
    let decoded = decode(
        "https://ops.example.com/spectra/schema/checkout_event/explore?window=24h&view=bar&measure=count&group=status#top",
    )
    .unwrap();
    assert_eq!(
        decoded.link.source,
        QuerySource::Event("checkout_event".into())
    );
    assert_eq!(
        decoded.link.window,
        QueryWindow::Last(WindowPreset::Hours24)
    );
    assert_eq!(decoded.link.view, Some(QueryView::Bar));
    assert_eq!(decoded.link.group.as_deref(), Some("status"));
}

#[test]
fn bare_path_defaults_to_last_hour_happy_path() {
    let decoded = decode("/spectra/metric/platform_smoke_counter/explore").unwrap();
    assert_eq!(decoded.link, QueryLink::metric("platform_smoke_counter"));
}

#[test]
fn unknown_keys_are_listed_sad_path() {
    let decoded =
        decode("/spectra/schema/t/explore?utm_source=mail&view=pie&utm_source=x&zoom=2").unwrap();
    assert_eq!(decoded.ignored_keys, vec!["utm_source", "zoom"]);
    assert_eq!(decoded.link.view, Some(QueryView::Pie));
}

#[test]
fn unknown_field_is_reported_sad_path() {
    let mut link = QueryLink::event("checkout_event");
    link.group = Some("customer_name".into());
    link.filters.push(LinkFilter {
        field: "status".into(),
        op: FilterOp::Eq,
        value: "failed".into(),
    });
    link.sort = Some(QuerySort {
        field: "ts".into(),
        descending: true,
    });
    let decoded = decode(&link.encode())
        .unwrap()
        .resolve(&schema(&["status", "region"]));
    assert_eq!(decoded.unknown_fields, vec!["customer_name"]);
    assert_eq!(decoded.link, link, "unknown fields stay in the query");
}

#[test]
fn non_spectra_urls_are_errors_sad_path() {
    for input in [
        "https://example.com/",
        "/spectra/schema/t",
        "/spectra/dashboards/abc/explore",
        "/other/schema/t/explore",
        "/spectra/schema/t/explore/extra",
        "",
    ] {
        assert_eq!(
            decode(input).unwrap_err(),
            QueryLinkError::NotSpectraLink,
            "{input}"
        );
    }
    assert!(matches!(
        decode("/spectra/schema/%2E%2E/explore").unwrap_err(),
        QueryLinkError::InvalidName(_)
    ));
}

#[test]
fn malformed_values_are_errors_sad_path() {
    let bad = [
        "window=2h",
        "view=line",
        "measure=avg",
        "measure=sum:",
        "bucket=0s",
        "bucket=5x",
        "f=status:like:x",
        "f=status",
        "sort=ts:sideways",
        "limit=0",
        "from=yesterday&to=2026-01-01T00:00:00Z",
        "group=",
    ];
    for query in bad {
        let err = decode(&format!("/spectra/schema/t/explore?{query}")).unwrap_err();
        assert!(
            matches!(err, QueryLinkError::InvalidValue { .. }),
            "{query}: {err:?}"
        );
    }
    let err = |q: &str| decode(&format!("/spectra/schema/t/explore?{q}")).unwrap_err();
    assert_eq!(
        err("view=log&view=bar"),
        QueryLinkError::DuplicateKey("view".into())
    );
    assert_eq!(
        err("from=2026-01-01T00:00:00Z"),
        QueryLinkError::IncompleteRange
    );
    assert_eq!(
        err("window=1h&from=2026-01-01T00:00:00Z&to=2026-01-02T00:00:00Z"),
        QueryLinkError::ConflictingWindow
    );
    assert_eq!(err("group=%zz"), QueryLinkError::InvalidEncoding);
}

#[test]
fn bucket_tokens_happy_path() {
    assert_eq!(QueryBucket::from_token("15m"), QueryBucket::from_secs(900));
    assert_eq!(
        QueryBucket::from_token("1d"),
        QueryBucket::from_secs(86_400)
    );
    assert_eq!(QueryBucket::from_token("150s"), QueryBucket::from_secs(150));
    assert_eq!(
        QueryBucket::from_token("60s")
            .map(QueryBucket::token)
            .as_deref(),
        Some("1m")
    );
}

#[test]
fn window_bounds_measure_back_from_now_happy_path() {
    let now = ts(1_700_000_000);
    assert_eq!(
        QueryWindow::Last(WindowPreset::Hours6).bounds(now),
        (ts(1_700_000_000 - 6 * 3_600), now)
    );
    let range = QueryWindow::Range {
        from: ts(1),
        to: ts(2),
    };
    assert_eq!(range.bounds(now), (ts(1), ts(2)));
}
