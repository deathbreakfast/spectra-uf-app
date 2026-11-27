//! Tests for the link and panel mapping.

use chrono::TimeZone;

use super::*;

fn field(name: &str, numeric: bool) -> SourceField {
    SourceField {
        name: name.into(),
        numeric,
        pii: false,
    }
}

fn checkout_fields() -> Vec<SourceField> {
    vec![
        field("status", false),
        field("amount_cents", true),
        field("region", false),
    ]
}

fn import(link: &str) -> LinkImport {
    let decoded = decode(link).expect("a Spectra link");
    panel_from_link(&decoded.link, Some(&checkout_fields()))
}

#[test]
fn an_event_link_fills_in_every_setting() {
    let got = import(
        "https://spectra.example/spectra/schema/checkout_event/explore?window=6h&view=bar\
         &measure=sum:amount_cents&group=status&bucket=5m&f=region:eq:eu-west",
    );
    assert_eq!(got.unknown, Vec::<UnknownField>::new());
    assert_eq!(got.dropped_filters, Vec::<LinkFilter>::new());
    assert_eq!(
        got.panel.source,
        PanelSource::EventTable {
            table: "checkout_event".into()
        }
    );
    assert_eq!(got.panel.chart, ChartKind::Bar);
    assert_eq!(got.panel.measure.as_deref(), Some("sum(amount_cents)"));
    assert_eq!(got.panel.group_by, vec!["status".to_string()]);
    assert_eq!(got.panel.bucket.as_deref(), Some("5m"));
    assert_eq!(got.panel.filters.len(), 1);
    assert_eq!(got.panel.keep_window, None);
    assert_eq!(got.window, Window::Relative { secs: 6 * 3_600 });
    assert_eq!(
        filled_in_summary(&got.panel),
        "Event table checkout_event, Bar, Sum of amount_cents, grouped by status"
    );
}

#[test]
fn an_unknown_group_field_empties_group_by_and_is_named() {
    let got = import("/spectra/schema/checkout_event/explore?view=bar&group=customer_name");
    assert_eq!(got.panel.group_by, Vec::<String>::new());
    assert_eq!(
        got.unknown[0].message("checkout_event"),
        "`customer_name` isn't a field of checkout_event, so Group by is empty."
    );
}

#[test]
fn an_unknown_sum_field_falls_back_to_count() {
    let got = import("/spectra/schema/checkout_event/explore?measure=sum:refund_cents");
    assert_eq!(got.panel.measure, None);
    assert_eq!(got.unknown[0].emptied, Emptied::Measure);
}

#[test]
fn filters_a_panel_cant_express_are_dropped_and_named() {
    let got = import("/spectra/schema/checkout_event/explore?f=amount_cents:ge:500");
    assert_eq!(got.panel.filters, Vec::<DashboardFilter>::new());
    assert_eq!(
        dropped_filter_message(&got.dropped_filters[0]),
        "`amount_cents ≥ 500` can't be a panel filter yet, so it was left out."
    );
}

#[test]
fn a_metric_link_keeps_its_aggregation_and_split() {
    let decoded =
        decode("/spectra/metric/api_latency_ms/explore?agg=max&bucket=1h&split=route").unwrap();
    let got = panel_from_link(&decoded.link, Some(&[]));
    assert_eq!(got.unknown, Vec::<UnknownField>::new());
    assert_eq!(got.panel.measure.as_deref(), Some("max"));
    assert_eq!(got.panel.group_by, vec!["route".to_string()]);
    assert_eq!(got.panel.bucket.as_deref(), Some("1h"));
    assert_eq!(measure_label(&got.panel), "Max");
}

#[test]
fn a_log_view_becomes_a_table_with_its_columns() {
    let got = import(
        "/spectra/schema/checkout_event/explore?view=log&cols=ts,status,nickname&sort=ts:desc&limit=50",
    );
    let table = got.panel.table.expect("table options");
    assert_eq!(got.panel.chart, ChartKind::Table);
    assert_eq!(table.columns, vec!["ts".to_string(), "status".to_string()]);
    assert_eq!(table.limit, 50);
    assert!(table.sort.expect("sort").descending);
    assert_eq!(got.unknown[0].emptied, Emptied::Column);
}

#[test]
fn a_range_window_is_kept_as_a_range() {
    let got = import(
        "/spectra/schema/checkout_event/explore?from=2026-11-01T00:00:00Z&to=2026-11-02T00:00:00Z",
    );
    assert!(matches!(got.window, Window::Absolute { .. }));
    assert!(window_text(got.window).starts_with("2026-11-01 00:00 to"));
}

#[test]
fn unregistered_sources_are_not_checked() {
    let decoded = decode("/spectra/schema/e2e.empty_events/explore?group=anything").unwrap();
    let got = panel_from_link(&decoded.link, None);
    assert_eq!(got.unknown, Vec::<UnknownField>::new());
    assert_eq!(got.panel.group_by, vec!["anything".to_string()]);
}

#[test]
fn panel_links_round_trip_for_every_chart() {
    let now = Utc.with_ymd_and_hms(2026, 11, 26, 12, 0, 0).unwrap();
    for (chart, _) in CHARTS {
        let mut panel = blank_panel(PanelSource::EventTable {
            table: "checkout_event".into(),
        });
        panel.chart = chart;
        panel.measure = Some("sum(amount_cents)".into());
        panel.bucket = Some("15m".into());
        panel.group_by = vec!["status".into()];
        panel.filters = vec![DashboardFilter {
            field: "region".into(),
            op: FilterOp::Ne,
            value: "us-east".into(),
        }];
        if chart == ChartKind::Table {
            panel.table = Some(TableOpts {
                columns: vec!["ts".into(), "status".into()],
                sort: Some(TableSort {
                    column: "amount_cents".into(),
                    descending: true,
                }),
                limit: 100,
            });
        }
        let link = panel_to_link(&panel, Window::Relative { secs: 3_600 }, now)
            .expect("a link")
            .encode();
        let got = import(&link);
        assert!(got.unknown.is_empty(), "{chart:?}: {:?}", got.unknown);
        assert_eq!(got.panel, panel, "{chart:?} via {link}");
    }
}

#[test]
fn metric_panels_round_trip() {
    let now = Utc::now();
    let mut panel = blank_panel(PanelSource::Metric {
        metric: "api_latency_ms".into(),
    });
    panel.measure = Some("avg".into());
    panel.group_by = vec!["route".into()];
    let link = panel_to_link(&panel, Window::Relative { secs: 6 * 3_600 }, now).unwrap();
    let got = panel_from_link(&decode(&link.encode()).unwrap().link, Some(&[]));
    assert_eq!(got.panel, panel);
}

#[test]
fn a_kept_window_goes_into_the_link() {
    let mut panel = blank_panel(PanelSource::EventTable {
        table: "checkout_event".into(),
    });
    panel.keep_window = Some(Window::Relative { secs: 7 * 86_400 });
    let link = panel_to_link(&panel, Window::Relative { secs: 3_600 }, Utc::now()).unwrap();
    assert_eq!(link.window, QueryWindow::Last(WindowPreset::Days7));
}

#[test]
fn text_panels_have_no_query_link() {
    let panel = blank_panel(PanelSource::Text {
        markdown: "hi".into(),
    });
    assert!(panel_to_link(&panel, Window::Relative { secs: 3_600 }, Utc::now()).is_none());
}

#[test]
fn pasted_text_that_isnt_a_link_gets_the_field_error() {
    assert_eq!(
        link_error("https://wiki.example.com/Checkout_runbook").as_deref(),
        Some(NOT_A_LINK)
    );
    assert_eq!(link_error("  "), None);
    assert_eq!(link_error("/spectra/schema/checkout_event/explore"), None);
}

#[test]
fn window_and_bucket_wording() {
    assert_eq!(window_text(Window::Relative { secs: 3_600 }), "last hour");
    assert_eq!(bucket_width("15m"), Some(900));
    assert_eq!(bucket_token(90), "90s");
}
