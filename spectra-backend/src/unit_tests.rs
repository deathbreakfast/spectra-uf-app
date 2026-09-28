use super::{
    empty_event_aggregate_result, empty_event_query_result, empty_metrics_query_result,
    encode_ops_path_segment, schema_metadata_detail, schema_metadata_list,
    spectra_metric_explore_path, spectra_query_permission_name, spectra_schema_explore_path,
    spectra_schema_path, validate_spectra_query_name, SpectraQueryNameError,
    MAX_SPECTRA_QUERY_NAME_CHARS,
};
use spectra_core::EventAggregateResult;

#[test]
fn spectra_query_permission_name_formats_table_happy_path() {
    assert_eq!(
        spectra_query_permission_name("my_events"),
        "spectra.query.my_events"
    );
    assert_eq!(
        spectra_query_permission_name("  metric_a  "),
        "spectra.query.metric_a"
    );
}

#[test]
fn schema_metadata_list_returns_vec_happy_path() {
    let items = schema_metadata_list();
    assert!(items.iter().all(|i| !i.table_or_metric.is_empty()));
}

#[test]
fn schema_metadata_detail_unknown_is_none_sad() {
    assert!(schema_metadata_detail("__spectra_backend_missing_schema__").is_none());
}

#[test]
fn empty_metrics_query_result_happy_path() {
    let r = empty_metrics_query_result();
    assert!(r.series.is_empty());
    assert!(r.headline.is_empty());
}

#[test]
fn empty_event_query_result_unknown_table_has_ts_column_happy_path() {
    let r = empty_event_query_result("__spectra_backend_missing_table__");
    assert!(r.rows.is_empty());
    assert_eq!(r.row_count, 0);
    assert_eq!(r.columns.len(), 1);
    assert_eq!(r.columns[0].field, "ts");
}

#[test]
fn empty_event_aggregate_result_is_empty_timeseries_happy_path() {
    match empty_event_aggregate_result() {
        EventAggregateResult::TimeSeries { series, headline } => {
            assert!(series.is_empty());
            assert!(headline.is_empty());
        }
        EventAggregateResult::Slices { .. } | EventAggregateResult::Pivot { .. } => {
            panic!("expected TimeSeries stub, got non-TimeSeries")
        }
    }
}

#[test]
fn validate_spectra_query_name_rejects_blank_sad() {
    assert_eq!(
        validate_spectra_query_name("").expect_err("blank"),
        SpectraQueryNameError::EmptyTableName
    );
    assert_eq!(
        validate_spectra_query_name("   ").expect_err("whitespace"),
        SpectraQueryNameError::EmptyTableName
    );
    assert!(SpectraQueryNameError::EmptyTableName
        .to_string()
        .contains("required"));
}

#[test]
fn validate_spectra_query_name_rejects_slash_control_dotdot_sad() {
    assert_eq!(
        validate_spectra_query_name("a/b").expect_err("slash"),
        SpectraQueryNameError::UnsafeTableName
    );
    assert_eq!(
        validate_spectra_query_name("a\\b").expect_err("backslash"),
        SpectraQueryNameError::UnsafeTableName
    );
    assert_eq!(
        validate_spectra_query_name("a\nb").expect_err("control"),
        SpectraQueryNameError::UnsafeTableName
    );
    assert_eq!(
        validate_spectra_query_name(".").expect_err("dot"),
        SpectraQueryNameError::UnsafeTableName
    );
    assert_eq!(
        validate_spectra_query_name("..").expect_err("dotdot"),
        SpectraQueryNameError::UnsafeTableName
    );
    assert!(SpectraQueryNameError::UnsafeTableName
        .to_string()
        .contains("unsafe"));
}

#[test]
fn validate_spectra_query_name_rejects_oversized_sad() {
    let oversized: String = "t".repeat(MAX_SPECTRA_QUERY_NAME_CHARS + 1);
    assert_eq!(
        validate_spectra_query_name(&oversized).expect_err("oversized"),
        SpectraQueryNameError::TableNameTooLong
    );
    assert!(SpectraQueryNameError::TableNameTooLong
        .to_string()
        .contains(&MAX_SPECTRA_QUERY_NAME_CHARS.to_string()));
}

#[test]
fn validate_spectra_query_name_accepts_non_empty_happy_path() {
    validate_spectra_query_name("events").expect("non-empty");
    validate_spectra_query_name("  metrics.latency  ").expect("trimmed dotted");
    validate_spectra_query_name("ops.events").expect("dotted table");
}

#[test]
fn encode_ops_path_segment_encodes_slash_and_space_happy_path() {
    assert_eq!(encode_ops_path_segment("orders"), "orders");
    assert_eq!(encode_ops_path_segment("a/b"), "a%2Fb");
    assert_eq!(encode_ops_path_segment("a b"), "a%20b");
    assert_eq!(encode_ops_path_segment("a\\b"), "a%5Cb");
    assert_eq!(encode_ops_path_segment("ops.events"), "ops.events");
}

#[test]
fn spectra_ops_paths_encode_segments_happy_path() {
    assert_eq!(spectra_schema_path("a/b"), "/spectra/schema/a%2Fb");
    assert_eq!(
        spectra_schema_explore_path("a/b"),
        "/spectra/schema/a%2Fb/explore"
    );
    assert_eq!(
        spectra_metric_explore_path("m x"),
        "/spectra/metric/m%20x/explore"
    );
}
