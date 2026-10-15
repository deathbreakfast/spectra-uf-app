//! Spectra home dashboard server function.

use chrono::{Duration, Utc};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use spectra_backend::{
    count_matching_events, dashboard_catalog_summary, schema_metadata_list,
    DEFAULT_RECENT_SCHEMA_LIMIT,
};
use spectra_core::{EventQuery, GridFilterModel, GridPaginationModel, SchemaListItem};

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
    /// Event rows written in the last 24 hours, summed across every registered event table
    /// the caller may query.
    ///
    /// `None` when no Spectra query backend is installed. Tables the caller lacks permission
    /// for, or whose count query fails, add nothing, so `Some(0)` can also mean every table
    /// was skipped.
    pub activity_24h_event_rows: Option<u64>,
}

/// Loads catalog stats and optional 24h activity for the home dashboard.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn get_spectra_dashboard_summary() -> Result<SpectraDashboardSummary, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;

    let catalog = dashboard_catalog_summary(DEFAULT_RECENT_SCHEMA_LIMIT);
    let activity_24h_event_rows = activity_24h_rows().await;

    Ok(SpectraDashboardSummary {
        schema_count: catalog.schema_count,
        event_table_count: catalog.event_table_count,
        metric_count: catalog.metric_count,
        recent_schemas: catalog.recent_schemas,
        activity_24h_event_rows,
    })
}

async fn activity_24h_rows() -> Option<u64> {
    let router = spectra_core::SpectraRouter::try_global()?;
    let end = Utc::now();
    let start = end - Duration::hours(24);
    let mut total = 0u64;

    for item in schema_metadata_list() {
        if item.logging_kind != "event" {
            continue;
        }
        if require_spectra_query(&item.table_or_metric).await.is_err() {
            continue;
        }
        let query = EventQuery {
            table: item.table_or_metric,
            start,
            end,
            partition: None,
            pagination: GridPaginationModel::default(),
            sort: Vec::new(),
            filter: GridFilterModel::default(),
        };
        if let Ok(n) = count_matching_events(&router, &query).await {
            total = total.saturating_add(n);
        }
    }

    Some(total)
}
