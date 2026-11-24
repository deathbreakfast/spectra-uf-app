//! Server functions behind one dashboard's page: what the viewer may see, and each panel's
//! data.
//!
//! Both run as the session actor. [`get_dashboard_view`] works out which panels are locked
//! before any panel fetches, so a locked panel never asks for data. [`query_dashboard_panel`]
//! repeats the per-table Gauge check itself, then hides personal fields before anything is
//! returned: `***` in table cells and stand-in labels on charts grouped by one.

use std::collections::BTreeMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
// `DashboardAccess` stays path-qualified, as in `dashboards.rs`.
use spectra_backend::dashboards::{Dashboard, DashboardFilter, Panel, Variable, Window};
use spectra_core::{SliceDto, StatCardDto, TimeSeriesDto};

/// Everything the dashboard page needs before its panels load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardView {
    /// The dashboard.
    pub dashboard: Dashboard,
    /// What the viewer may do with it.
    pub access: spectra_backend::dashboards::DashboardAccess,
    /// Whether the viewer starred it.
    pub favourite: bool,
    /// `spectra.query.*` permissions the viewer lacks for at least one panel, sorted.
    pub missing_permissions: Vec<String>,
    /// Where Request access goes: Gauge's request form for the first missing permission.
    /// `None` when nothing is missing.
    pub request_access_href: Option<String>,
    /// For each panel id, the variables whose field the panel's source doesn't have.
    pub variable_gaps: BTreeMap<String, Vec<String>>,
}

/// One panel's query, as the page sends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelQuery {
    /// Dashboard id; with the panel id it salts stand-in labels.
    pub dashboard_id: String,
    /// The panel to load.
    pub panel: Panel,
    /// The dashboard's range. A panel with `keep_window` uses its own.
    pub window: Window,
    /// The toolbar's filters (or the dashboard's saved ones).
    pub filters: Vec<DashboardFilter>,
    /// The dashboard's variables.
    pub variables: Vec<Variable>,
    /// Each variable's value in effect, by name.
    pub variable_values: BTreeMap<String, String>,
}

/// A loaded panel, ready to draw.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PanelData {
    /// A time series: a metric, or an event table's count or sum over time.
    Series {
        /// Series to plot.
        series: Vec<TimeSeriesDto>,
        /// Headline numbers.
        headline: Vec<StatCardDto>,
    },
    /// Bar or pie slices.
    Slices {
        /// Slices to plot. Labels are stand-ins when `stand_in_field` is set.
        slices: Vec<SliceDto>,
        /// The personal field the slices are grouped by, when labels were replaced.
        stand_in_field: Option<String>,
    },
    /// Headline numbers only.
    Stat {
        /// Headline numbers.
        headline: Vec<StatCardDto>,
    },
    /// Rows for a table panel, as display text.
    Table {
        /// Column fields, in order.
        columns: Vec<String>,
        /// Cell text per row, in column order. Masked cells read `***`.
        rows: Vec<Vec<String>>,
        /// Columns whose values were masked.
        masked: Vec<String>,
        /// Rows matching the query, of which `rows` are the newest.
        total: u64,
    },
}

#[cfg(feature = "ssr")]
async fn request_access_href(v: &valence::Valence, permission: &str) -> String {
    // Gauge's permission page opens its request form from `?request=1`. It needs the
    // permission's id; fall back to the permissions index when the viewer can't list it.
    match gauge::service::list_permissions(v, None).await {
        Ok(perms) => perms
            .into_iter()
            .find(|p| p.name == permission)
            .map_or_else(
                || uf_product::paths::PERMISSION_PERMISSIONS.to_string(),
                |p| {
                    format!(
                        "{}/{}?request=1",
                        uf_product::paths::PERMISSION_PERMISSIONS,
                        spectra_backend::encode_ops_path_segment(&p.id)
                    )
                },
            ),
        Err(err) => {
            log::warn!("Spectra dashboard: listing permissions for Request access failed: {err}");
            uf_product::paths::PERMISSION_PERMISSIONS.to_string()
        }
    }
}

/// Load a dashboard with the viewer's access, the query permissions they lack, and the
/// variables each panel can't apply.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn get_dashboard_view(
    /// Dashboard id.
    id: String,
) -> Result<DashboardView, ServerFnError> {
    use spectra_backend::dashboards::view::{
        missing_variable_fields, panel_source_name, source_fields,
    };
    use spectra_backend::dashboards::{sharing, store, DashboardError};
    use std::collections::BTreeSet;

    use super::dashboards::{session_valence, to_sfe};

    let v = session_valence().await?;
    let dashboard = store::get(&v, &id, None).await.map_err(to_sfe)?;
    let access = sharing::access(&v, &id).await.map_err(to_sfe)?;
    let favourite = store::is_favourite(&v, &id).await.map_err(to_sfe)?;

    let needed: BTreeSet<String> = dashboard
        .sections
        .iter()
        .flat_map(|s| &s.panels)
        .filter_map(Panel::required_query_permission)
        .collect();
    let mut missing_permissions = Vec::new();
    for permission in needed {
        let allowed = gauge::service::actor_can(&v, &permission)
            .await
            .map_err(|e| to_sfe(DashboardError::store(e)))?;
        if !allowed {
            missing_permissions.push(permission);
        }
    }
    let request_access_href = match missing_permissions.first() {
        Some(first) => Some(request_access_href(&v, first).await),
        None => None,
    };

    let mut variable_gaps = BTreeMap::new();
    for panel in dashboard.sections.iter().flat_map(|s| &s.panels) {
        let Some(source) = panel_source_name(panel) else {
            continue;
        };
        let gaps = missing_variable_fields(
            panel,
            &dashboard.variables,
            source_fields(source).as_deref(),
        );
        if !gaps.is_empty() {
            variable_gaps.insert(panel.id.clone(), gaps);
        }
    }

    Ok(DashboardView {
        dashboard,
        access,
        favourite,
        missing_permissions,
        request_access_href,
        variable_gaps,
    })
}

/// Load one panel's data as the session actor, with personal fields hidden.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn query_dashboard_panel(
    /// The panel and the toolbar state it follows.
    query: PanelQuery,
) -> Result<PanelData, ServerFnError> {
    use chrono::Utc;
    use spectra_backend::dashboards::view::{
        effective_filters, event_aggregate_request, event_rows_query, mask_rows, metrics_query,
        pii_fields, source_fields, stand_in_labels,
    };
    use spectra_backend::dashboards::{ChartKind, PanelSource};
    use spectra_backend::{
        execute_event_aggregate, execute_event_query, execute_metrics_query, SpectraOpsError,
    };
    use spectra_core::EventAggregateResult;

    use super::{require_session, require_spectra_query, to_server_fn_error};

    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;

    let PanelQuery {
        dashboard_id,
        panel,
        window,
        filters,
        variables,
        variable_values,
    } = query;
    let (source, is_metric) = match &panel.source {
        PanelSource::Metric { metric } => (metric.clone(), true),
        PanelSource::EventTable { table } => (table.clone(), false),
        _ => {
            return Err(to_server_fn_error(SpectraOpsError::QueryFailed(
                "text panels have no data to load".into(),
            )))
        }
    };
    require_spectra_query(&source)
        .await
        .map_err(to_server_fn_error)?;
    let router = spectra_core::SpectraRouter::try_global()
        .ok_or_else(|| to_server_fn_error(SpectraOpsError::BackendNotInstalled))?;
    let query_failed =
        |e: spectra_core::Error| to_server_fn_error(SpectraOpsError::QueryFailed(e.to_string()));

    let fields = source_fields(&source);
    let filters = effective_filters(
        &panel,
        &variables,
        &filters,
        &variable_values,
        fields.as_deref(),
    );
    let window = panel.keep_window.unwrap_or(window);
    let now = Utc::now();

    if is_metric {
        let result = execute_metrics_query(
            &router,
            &metrics_query(&panel, &source, window, now, &filters),
        )
        .await
        .map_err(query_failed)?;
        return Ok(if panel.chart == ChartKind::SingleStat {
            PanelData::Stat {
                headline: result.headline,
            }
        } else {
            PanelData::Series {
                series: result.series,
                headline: result.headline,
            }
        });
    }

    if panel.chart == ChartKind::Table {
        let result = execute_event_query(
            &router,
            &event_rows_query(&panel, &source, window, now, &filters),
        )
        .await
        .map_err(query_failed)?;
        let columns = match panel.table.as_ref().map(|t| &t.columns) {
            Some(cols) if !cols.is_empty() => cols.clone(),
            _ => result.columns.iter().map(|c| c.field.clone()).collect(),
        };
        let records: Vec<serde_json::Value> = result
            .rows
            .iter()
            .map(|row| {
                let mut record = match &row.fields {
                    serde_json::Value::Object(map) => map.clone(),
                    _ => serde_json::Map::new(),
                };
                record
                    .entry("id")
                    .or_insert_with(|| serde_json::Value::String(row.id.clone()));
                record.insert("ts".into(), serde_json::Value::String(row.ts.to_rfc3339()));
                serde_json::Value::Object(record)
            })
            .collect();
        let (rows, masked) = mask_rows(&columns, &records, &pii_fields(&source));
        return Ok(PanelData::Table {
            columns,
            rows,
            masked,
            total: result.row_count,
        });
    }

    let request = event_aggregate_request(&panel, &source, window, now, &filters);
    let group_by = request.aggregation.group_by_field.clone();
    let result = execute_event_aggregate(&router, &request)
        .await
        .map_err(query_failed)?;
    Ok(match result {
        EventAggregateResult::TimeSeries { headline, .. }
            if panel.chart == ChartKind::SingleStat =>
        {
            PanelData::Stat { headline }
        }
        EventAggregateResult::TimeSeries { series, headline } => {
            PanelData::Series { series, headline }
        }
        EventAggregateResult::Slices { mut slices, .. } => {
            let stand_in_field = group_by.filter(|f| pii_fields(&source).contains(f));
            if let Some(field) = &stand_in_field {
                let labels: Vec<String> = slices.iter().map(|s| s.label.clone()).collect();
                let salt = format!("{dashboard_id}/{}", panel.id);
                for (slice, label) in slices
                    .iter_mut()
                    .zip(stand_in_labels(field, &labels, &salt))
                {
                    slice.label = label;
                }
            }
            PanelData::Slices {
                slices,
                stand_in_field,
            }
        }
        EventAggregateResult::Pivot { headline, .. } => PanelData::Stat { headline },
    })
}
