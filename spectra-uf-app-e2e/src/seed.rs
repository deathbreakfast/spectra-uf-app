//! Harness-only seed endpoint for Playwright.

use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use gauge::service;
use serde::Deserialize;
use serde_json::json;
use spectra::{try_log_event_at, try_record_gauge_at};
use spectra_backend::spectra_query_permission_name;

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
}

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

async fn seed_spectra_rows(kind: E2eAuthKind, skip_data: bool) -> Result<(), StatusCode> {
    if matches!(kind, E2eAuthKind::Admin | E2eAuthKind::AdminNoPerms) {
        let admin = e2e_admin_valence();
        // Create missing spectra.query.* Gauge rows for registered schemas.
        spectra_app::ensure_table_query_permissions(&admin)
            .await
            .map_err(|err| {
                log::error!("e2e seed: ensure_table_query_permissions failed: {err}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        // Lab-only empty fixtures are not in Spectra inventory — create Gauge rows explicitly.
        ensure_named_query_perm(&admin, E2E_EMPTY_METRIC_NAME).await?;
        ensure_named_query_perm(&admin, E2E_EMPTY_EVENT_TABLE).await?;
        if matches!(kind, E2eAuthKind::Admin) {
            grant_table_query_perm(&admin, "admin", E2E_EVENT_TABLE).await?;
            grant_table_query_perm(&admin, "admin", E2E_METRIC_NAME).await?;
            grant_table_query_perm(&admin, "admin", E2E_EMPTY_METRIC_NAME).await?;
            grant_table_query_perm(&admin, "admin", E2E_EMPTY_EVENT_TABLE).await?;
        } else if matches!(kind, E2eAuthKind::AdminNoPerms) {
            revoke_table_query_perm(&admin, "admin", E2E_EVENT_TABLE).await?;
            revoke_table_query_perm(&admin, "admin", E2E_METRIC_NAME).await?;
            revoke_table_query_perm(&admin, "admin", E2E_EMPTY_METRIC_NAME).await?;
            revoke_table_query_perm(&admin, "admin", E2E_EMPTY_EVENT_TABLE).await?;
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
    try_record_gauge_at(E2E_METRIC_NAME, &[("host", "e2e")], 42.0, recent);

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

    seed_spectra_rows(kind, body.skip_data).await?;

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
