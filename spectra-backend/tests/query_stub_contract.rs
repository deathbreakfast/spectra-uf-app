//! Integration contracts for stubbed explore query result shapes
//! (`query_events` / `query_event_aggregate` / `query_metrics`).

#![allow(missing_docs)]

use spectra_backend::{
    empty_event_aggregate_result, empty_event_query_result, empty_metrics_query_result,
    spectra_query_permission_name, validate_spectra_query_name, SpectraQueryNameError,
    MAX_SPECTRA_QUERY_NAME_CHARS,
};
use spectra_core::EventAggregateResult;

#[test]
fn empty_metrics_query_result_shape_happy_path() {
    let r = empty_metrics_query_result();
    assert!(r.series.is_empty());
    assert!(r.headline.is_empty());
}

#[test]
fn empty_event_query_result_unknown_table_happy_path() {
    let r = empty_event_query_result("__spectra_uf_app_no_such_table__");
    assert!(r.rows.is_empty());
    assert_eq!(r.row_count, 0);
    assert_eq!(r.columns.len(), 1);
    assert_eq!(r.columns[0].field, "ts");
    assert_eq!(r.columns[0].header_name, "Timestamp");
}

#[test]
fn empty_event_aggregate_result_timeseries_stub_happy_path() {
    match empty_event_aggregate_result() {
        EventAggregateResult::TimeSeries { series, headline } => {
            assert!(series.is_empty());
            assert!(headline.is_empty());
        }
        EventAggregateResult::Slices { .. } | EventAggregateResult::Pivot { .. } => {
            panic!("expected empty TimeSeries stub, got non-TimeSeries")
        }
    }
}

#[test]
fn validate_spectra_query_name_rejects_blank_sad() {
    assert_eq!(
        validate_spectra_query_name("").expect_err("blank name"),
        SpectraQueryNameError::EmptyTableName
    );
    assert_eq!(
        validate_spectra_query_name("   ").expect_err("whitespace name"),
        SpectraQueryNameError::EmptyTableName
    );
}

#[test]
fn validate_spectra_query_name_rejects_slash_and_oversized_sad() {
    assert_eq!(
        validate_spectra_query_name("a/b").expect_err("slash"),
        SpectraQueryNameError::UnsafeTableName
    );
    let oversized = "t".repeat(MAX_SPECTRA_QUERY_NAME_CHARS + 1);
    assert_eq!(
        validate_spectra_query_name(&oversized).expect_err("oversized"),
        SpectraQueryNameError::TableNameTooLong
    );
}

#[test]
fn validate_spectra_query_name_accepts_table_happy_path() {
    validate_spectra_query_name("ops.events").expect("non-empty table");
}

#[test]
fn spectra_query_permission_name_formats_table_happy_path() {
    assert_eq!(
        spectra_query_permission_name("ops.events"),
        "spectra.query.ops.events"
    );
}
