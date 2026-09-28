//! Spectra home dashboard server function.

use chrono::{Duration, Utc};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use spectra_backend::{
    dashboard_catalog_summary, execute_event_aggregate, DEFAULT_RECENT_SCHEMA_LIMIT,
};
use spectra_core::{
    EventAggregateRequest, EventAggregationSpec, EventExploreView, EventMeasure, GridFilterModel,
    SchemaListItem,
};

#[cfg(feature = "ssr")]
use super::require_session;
use super::{require_spectra_query, to_server_fn_error};
use spectra_backend::SpectraOpsError;

/// Home dashboard payload for `/spectra`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectraDashboardSummary {
    /// Total registered schemas.
    pub schema_count: u64,
    /// Event table schemas.
    pub event_table_count: u64,
    /// Metric schemas.
    pub metric_count: u64,
    /// Recent schemas for the home list.
    pub recent_schemas: Vec<SchemaListItem>,
    /// Sum of 24h event row counts when router + permissions allow.
    ///
    /// `None` when no Spectra query backend is installed. `Some(0)` when the backend
    /// is present but every recent event table was skipped (permission denied or query
    /// failure). Individual table errors are omitted from the aggregate by design.
    pub activity_24h_event_rows: Option<u64>,
}

/// Loads catalog stats and optional 24h activity for the home dashboard.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn get_spectra_dashboard_summary() -> Result<SpectraDashboardSummary, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;

    let catalog = dashboard_catalog_summary(DEFAULT_RECENT_SCHEMA_LIMIT);
    let activity_24h_event_rows = activity_24h_rows(&catalog.recent_schemas).await;

    Ok(SpectraDashboardSummary {
        schema_count: catalog.schema_count,
        event_table_count: catalog.event_table_count,
        metric_count: catalog.metric_count,
        recent_schemas: catalog.recent_schemas,
        activity_24h_event_rows,
    })
}

async fn activity_24h_rows(schemas: &[SchemaListItem]) -> Option<u64> {
    let router = spectra_core::SpectraRouter::try_global()?;
    let end = Utc::now();
    let start = end - Duration::hours(24);
    let mut total = 0u64;

    for item in schemas {
        if item.logging_kind != "event" {
            continue;
        }
        if require_spectra_query(&item.table_or_metric).await.is_err() {
            continue;
        }
        let request = EventAggregateRequest {
            table: item.table_or_metric.clone(),
            start,
            end,
            partition: None,
            filter: GridFilterModel::default(),
            view: EventExploreView::TimeSeries,
            aggregation: EventAggregationSpec {
                measure: EventMeasure::Count,
                measure_field: None,
                time_bucket_secs: Some(3600),
                group_by_field: None,
                row_fields: Vec::new(),
                pivot_field: None,
            },
        };
        let Ok(result) = execute_event_aggregate(&router, &request).await else {
            continue;
        };
        if let spectra_core::EventAggregateResult::TimeSeries { headline, .. } = result {
            for card in headline {
                if card.label.eq_ignore_ascii_case("count") {
                    if let Ok(n) = card.value.replace(',', "").parse::<u64>() {
                        total = total.saturating_add(n);
                    }
                }
            }
        }
    }

    Some(total)
}
