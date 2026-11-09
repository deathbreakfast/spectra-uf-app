//! Harness-only seed endpoint for Playwright.

use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use gauge::service;
use serde::Deserialize;
use serde_json::json;
use spectra::{try_log_event_at, try_record_gauge_at};
use spectra_backend::{dashboard_presets, spectra_query_permission_name};

use crate::e2e_spectra::e2e_spectra;
use crate::e2e_valence::{
    e2e_admin_valence, e2e_fixtures, E2E_EMPTY_EVENT_TABLE, E2E_EMPTY_METRIC_NAME, E2E_EVENT_TABLE,
    E2E_METRIC_NAME,
};
use crate::gate_demos::{write_e2e_auth_kind, E2eAuthKind};

#[derive(Debug, Deserialize)]
pub struct SeedRequest {
    /// `anonymous` | `admin` | `admin_noperms` | `outsider` | `unverified`
    #[serde(default = "default_auth")]
    pub auth: String,
    /// When `true`, skip writing Spectra rows (auth/session still applied).
    #[serde(default)]
    pub skip_data: bool,
    /// Extra event rows written to the event table, older than the fixed seed rows but inside
    /// the default one-hour explore window. Capped at [`MAX_BULK_EVENTS`].
    #[serde(default)]
    pub bulk_events: u32,
}

/// Upper bound on [`SeedRequest::bulk_events`].
pub const MAX_BULK_EVENTS: u32 = 500;

fn default_auth() -> String {
    E2eAuthKind::Anonymous.as_str().to_string()
}

/// Create `spectra.query.{table}` if missing (for lab-only names not in inventory).
async fn ensure_named_query_perm(admin: &valence::Valence, table: &str) -> Result<(), StatusCode> {
    use gauge::types::PermissionCreateInput;

    let perm_name = spectra_query_permission_name(table);
    let perms = service::list_permissions(admin, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if perms.iter().any(|p| p.name == perm_name) {
        return Ok(());
    }
    service::create_permission(
        PermissionCreateInput {
            name: perm_name,
            description: format!("Query Spectra table or metric `{table}`"),
            owners_group_id: String::new(),
            domain_id: "spectra".into(),
        },
        admin,
    )
    .await
    .map_err(|err| {
        log::error!("e2e seed: create_permission for {table} failed: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(())
}

async fn grant_table_query_perm(
    admin: &valence::Valence,
    user_id: &str,
    table: &str,
) -> Result<(), StatusCode> {
    let perm_name = spectra_query_permission_name(table);
    let perms = service::list_permissions(admin, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let perm_id = perms
        .into_iter()
        .find(|p| p.name == perm_name)
        .map(|p| p.id)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    service::grant_permission_to_user(&perm_id, user_id, admin)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

async fn revoke_table_query_perm(
    admin: &valence::Valence,
    user_id: &str,
    table: &str,
) -> Result<(), StatusCode> {
    let perm_name = spectra_query_permission_name(table);
    let perms = service::list_permissions(admin, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(perm) = perms.into_iter().find(|p| p.name == perm_name) else {
        return Ok(());
    };
    service::revoke_permission_from_user(&perm.id, user_id, admin)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

/// Preset metrics left unseeded so dashboards show an empty panel beside charted ones.
const SEED_SKIPPED_PRESET_METRICS: [&str; 2] = ["photon_backlog", "valence_db_wall_ms"];

/// Every metric named by a curated dashboard panel.
fn preset_metrics() -> impl Iterator<Item = &'static str> {
    dashboard_presets()
        .iter()
        .flat_map(|preset| preset.panels.iter().map(|panel| panel.metric))
}

/// Create the Gauge `spectra.query.*` rows the seed grants and revokes.
///
/// Runs once at init while `admin` is still a super user: Valence now enforces write privacy, so
/// the demoted admin can no longer create the default owner group these rows need.
pub(crate) async fn ensure_seed_query_permissions(admin: &valence::Valence) -> Result<(), String> {
    // Create missing spectra.query.* Gauge rows for registered schemas.
    spectra_app::ensure_table_query_permissions(admin).await?;
    // Lab-only empty fixtures are not in Spectra inventory — create Gauge rows explicitly.
    ensure_named_query_perm(admin, E2E_EMPTY_METRIC_NAME)
        .await
        .map_err(|code| format!("{E2E_EMPTY_METRIC_NAME}: {code}"))?;
    ensure_named_query_perm(admin, E2E_EMPTY_EVENT_TABLE)
        .await
        .map_err(|code| format!("{E2E_EMPTY_EVENT_TABLE}: {code}"))?;
    // Dashboard preset metrics come from platform kits this host does not link.
    for metric in preset_metrics() {
        ensure_named_query_perm(admin, metric)
            .await
            .map_err(|code| format!("{metric}: {code}"))?;
    }
    Ok(())
}

async fn seed_spectra_rows(
    kind: E2eAuthKind,
    skip_data: bool,
    bulk_events: u32,
) -> Result<(), StatusCode> {
    if matches!(kind, E2eAuthKind::Admin | E2eAuthKind::AdminNoPerms) {
        let admin = e2e_admin_valence();
        if matches!(kind, E2eAuthKind::Admin) {
            grant_table_query_perm(&admin, "admin", E2E_EVENT_TABLE).await?;
            grant_table_query_perm(&admin, "admin", E2E_METRIC_NAME).await?;
            grant_table_query_perm(&admin, "admin", E2E_EMPTY_METRIC_NAME).await?;
            grant_table_query_perm(&admin, "admin", E2E_EMPTY_EVENT_TABLE).await?;
            for metric in preset_metrics() {
                grant_table_query_perm(&admin, "admin", metric).await?;
            }
        } else if matches!(kind, E2eAuthKind::AdminNoPerms) {
            revoke_table_query_perm(&admin, "admin", E2E_EVENT_TABLE).await?;
            revoke_table_query_perm(&admin, "admin", E2E_METRIC_NAME).await?;
            revoke_table_query_perm(&admin, "admin", E2E_EMPTY_METRIC_NAME).await?;
            revoke_table_query_perm(&admin, "admin", E2E_EMPTY_EVENT_TABLE).await?;
            for metric in preset_metrics() {
                revoke_table_query_perm(&admin, "admin", metric).await?;
            }
        }
    }

    if skip_data || !matches!(kind, E2eAuthKind::Admin | E2eAuthKind::AdminNoPerms) {
        return Ok(());
    }

    let _ = e2e_spectra();
    let now = Utc::now();
    let recent = now - chrono::Duration::minutes(5);
    let stale = now - chrono::Duration::days(8);

    try_log_event_at(
        E2E_EVENT_TABLE,
        &json!({
            "id": "e2e-event-1",
            "message": "Playwright seed row",
            "severity": "info",
            "value": 10,
        }),
        recent,
    );
    try_log_event_at(
        E2E_EVENT_TABLE,
        &json!({
            "id": "e2e-event-2",
            "message": "Second info row",
            "severity": "info",
            "value": 5,
        }),
        recent - chrono::Duration::minutes(2),
    );
    try_log_event_at(
        E2E_EVENT_TABLE,
        &json!({
            "id": "e2e-event-warn",
            "message": "Warn severity row",
            "severity": "warn",
            "value": 3,
        }),
        recent - chrono::Duration::minutes(1),
    );
    try_log_event_at(
        E2E_EVENT_TABLE,
        &json!({
            "id": "e2e-event-stale",
            "message": "Stale row outside 1h window",
            "severity": "info",
            "value": 1,
        }),
        stale,
    );
    // Rows stamped like a generated `*Logger` call: two lines in one file, one in another, and
    // one older than the graph's default 24h window.
    for (file, line, ts) in [
        ("e2e/checkout.rs", 12, recent - chrono::Duration::minutes(3)),
        ("e2e/checkout.rs", 12, recent - chrono::Duration::minutes(4)),
        ("e2e/checkout.rs", 48, recent - chrono::Duration::minutes(5)),
        ("e2e/worker.rs", 7, recent - chrono::Duration::minutes(6)),
        ("e2e/legacy.rs", 3, now - chrono::Duration::days(2)),
    ] {
        try_log_event_at(
            E2E_EVENT_TABLE,
            &json!({
                "message": format!("Callsite row from {file}:{line}"),
                "_callsite": { "file": file, "line": line },
            }),
            ts,
        );
    }
    let bulk_start = now - chrono::Duration::minutes(10);
    for i in 0..bulk_events.min(MAX_BULK_EVENTS) {
        try_log_event_at(
            E2E_EVENT_TABLE,
            &json!({
                "id": format!("e2e-bulk-{i}"),
                "message": format!("Bulk paging row {i}"),
                "severity": "info",
                "value": 1,
            }),
            bulk_start - chrono::Duration::seconds(i64::from(i)),
        );
    }
    try_record_gauge_at(E2E_METRIC_NAME, &[("host", "e2e")], 42.0, recent);
    // Seed every dashboard metric except the Photon backlog and Valence wall time, so the
    // specs can see both a charted panel and an empty one.
    for metric in preset_metrics() {
        if SEED_SKIPPED_PRESET_METRICS.contains(&metric) {
            continue;
        }
        try_record_gauge_at(metric, &[("host", "e2e")], 7.0, recent);
    }

    e2e_spectra().flush_persist().await.map_err(|err| {
        log::error!("e2e seed: spectra flush_persist failed: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(())
}

pub async fn seed_data(
    session: tower_sessions::Session,
    Json(body): Json<SeedRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let kind = E2eAuthKind::parse(&body.auth);
    write_e2e_auth_kind(&session, kind)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    seed_spectra_rows(kind, body.skip_data, body.bulk_events).await?;

    let fixtures = e2e_fixtures();
    Ok(Json(serde_json::json!({
        "ok": true,
        "auth": kind.as_str(),
        "fixtures": {
            "event_table": fixtures.event_table,
            "empty_event_table": fixtures.empty_event_table,
            "metric_name": fixtures.metric_name,
            "empty_metric_name": fixtures.empty_metric_name,
            "seeded_event_count": fixtures.seeded_event_count,
            "seed_message": "Playwright seed row",
            "seed_metric_value": 42.0,
        }
    })))
}
