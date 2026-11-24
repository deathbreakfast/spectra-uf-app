//! Saved-dashboard server functions.
//!
//! Each one resolves the Higgs session, takes the session actor's own Valence and calls
//! [`spectra_backend::dashboards::store`]. Nothing here switches actor; Valence privacy and
//! the dashboard's Gauge permissions decide what the caller may do.
//!
//! A [`DashboardError`] reaches the client inside the `ServerFnError` message. Use
//! [`dashboard_error`] to get it back, for example to show the conflict bar when
//! [`save_dashboard`] returns [`DashboardError::Conflict`].

use leptos::prelude::*;
use leptos::server_fn::codec::Json;
// `DashboardAccess` stays path-qualified: `#[server] fn dashboard_access` generates a
// struct with that name in this module.
use spectra_backend::dashboards::{
    Dashboard, DashboardError, DashboardScope, DashboardSummary, Principal, Version,
};

#[cfg(feature = "ssr")]
use super::require_session;

/// What the client sees in place of a [`DashboardError::Store`] message.
pub const STORE_ERROR_CLIENT_MESSAGE: &str = "something went wrong on the server. Try again.";

/// Turn a [`DashboardError`] into a `ServerFnError` that [`dashboard_error`] can decode.
///
/// A [`DashboardError::Store`] carries raw Valence or Gauge text (table names, engine
/// messages, permission ids). That detail is logged on the server, and the client gets
/// [`STORE_ERROR_CLIENT_MESSAGE`] instead.
#[must_use]
pub fn to_dashboard_server_error(err: &DashboardError) -> ServerFnError {
    let sent = match err {
        DashboardError::Store { message } => {
            log::error!("Spectra dashboard store error: {message}");
            DashboardError::store(STORE_ERROR_CLIENT_MESSAGE)
        }
        other => other.clone(),
    };
    ServerFnError::new(format!("{sent} ({})", sent.encode()))
}

/// The [`DashboardError`] inside a dashboard server function's error, if there is one.
#[must_use]
pub fn dashboard_error(err: &ServerFnError) -> Option<DashboardError> {
    DashboardError::decode(&err.to_string())
}

#[cfg(feature = "ssr")]
pub(super) async fn session_valence() -> Result<valence::Valence, ServerFnError> {
    let ctx = higgs::Higgs::from_request().await?;
    require_session(&ctx)?;
    ctx.valence()
        .map_err(|e| to_dashboard_server_error(&DashboardError::store(e)))
}

#[cfg(feature = "ssr")]
pub(super) fn to_sfe(err: DashboardError) -> ServerFnError {
    to_dashboard_server_error(&err)
}

/// List the dashboards the caller can open: their own, shared with them, or all of them
/// including the built-ins.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn list_dashboards(
    /// Which dashboards to list.
    scope: DashboardScope,
    /// Keep only dashboards whose title or description contains this text.
    q: Option<String>,
) -> Result<Vec<DashboardSummary>, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::list(&v, scope, q.as_deref())
        .await
        .map_err(to_sfe)
}

/// Load a dashboard, or one of its saved versions.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn get_dashboard(
    /// Dashboard id.
    id: String,
    /// Saved version to load; `None` for the newest.
    version: Option<u32>,
) -> Result<Dashboard, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::get(&v, &id, version)
        .await
        .map_err(to_sfe)
}

/// Create a dashboard owned by the caller, empty or copied from another dashboard.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn create_dashboard(
    /// Title.
    title: String,
    /// One-line description.
    description: String,
    /// Dashboard (or built-in) to copy sections from.
    from: Option<String>,
) -> Result<Dashboard, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::create(&v, &title, &description, from.as_deref())
        .await
        .map_err(to_sfe)
}

/// Save a dashboard as a new version.
///
/// Fails with [`DashboardError::Conflict`] when someone saved after `expected_version`.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn save_dashboard(
    /// The edited dashboard.
    dashboard: Dashboard,
    /// Version the edit started from.
    expected_version: u32,
    /// One-line summary for the history; defaults to "Saved changes".
    summary: Option<String>,
) -> Result<Dashboard, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::save(
        &v,
        &dashboard,
        expected_version,
        summary.as_deref(),
        None,
    )
    .await
    .map_err(to_sfe)
}

/// Copy a dashboard the caller can open into a new one they own.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn duplicate_dashboard(
    /// Dashboard to copy.
    id: String,
    /// Title of the copy.
    title: String,
) -> Result<Dashboard, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::duplicate(&v, &id, &title)
        .await
        .map_err(to_sfe)
}

/// Delete a dashboard with its versions and favourites. Owner only.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn delete_dashboard(
    /// Dashboard id.
    id: String,
) -> Result<(), ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::delete(&v, &id)
        .await
        .map_err(to_sfe)
}

/// A dashboard's saved versions, newest first.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn list_versions(
    /// Dashboard id.
    id: String,
) -> Result<Vec<Version>, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::list_versions(&v, &id)
        .await
        .map_err(to_sfe)
}

/// Save an earlier version again as the newest one. Nothing is lost.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn restore_version(
    /// Dashboard id.
    id: String,
    /// Version number to restore.
    n: u32,
) -> Result<Dashboard, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::restore(&v, &id, n, None)
        .await
        .map_err(to_sfe)
}

/// Star or unstar a dashboard for the caller.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn set_favourite(
    /// Dashboard id.
    id: String,
    /// `true` to star, `false` to unstar.
    favourite: bool,
) -> Result<(), ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::store::set_favourite(&v, &id, favourite)
        .await
        .map_err(to_sfe)
}

/// Set who can edit and who can view a dashboard. Owner only.
///
/// Sent as JSON: the default URL encoding drops an empty `editors` or `viewers` list.
#[uf_product_macros::server(permission = "QueryTable")]
#[server(input = Json)]
pub async fn share_dashboard(
    /// Dashboard id.
    id: String,
    /// People and groups who can edit.
    editors: Vec<Principal>,
    /// People and groups who can only view.
    viewers: Vec<Principal>,
) -> Result<spectra_backend::dashboards::DashboardAccess, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::sharing::share(&v, &id, &editors, &viewers)
        .await
        .map_err(to_sfe)
}

/// Who can reach a dashboard, and what the caller may do with it.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn dashboard_access(
    /// Dashboard id.
    id: String,
) -> Result<spectra_backend::dashboards::DashboardAccess, ServerFnError> {
    let v = session_valence().await?;
    spectra_backend::dashboards::sharing::access(&v, &id)
        .await
        .map_err(to_sfe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_round_trips_through_server_fn_error_happy_path() {
        let err = to_dashboard_server_error(&DashboardError::Conflict { current: 3 });
        assert_eq!(
            dashboard_error(&err),
            Some(DashboardError::Conflict { current: 3 })
        );
        assert!(err.to_string().contains("version 3"));
    }

    #[test]
    fn store_detail_stays_on_the_server_sad_path() {
        let err = to_dashboard_server_error(&DashboardError::store(
            "permission rp_perm_spectra_dashboard_x_view: backend timeout",
        ));
        assert!(!err.to_string().contains("rp_perm"));
        assert_eq!(
            dashboard_error(&err),
            Some(DashboardError::store(STORE_ERROR_CLIENT_MESSAGE))
        );
    }

    #[test]
    fn other_server_errors_decode_to_none_sad_path() {
        assert_eq!(dashboard_error(&ServerFnError::new("boom")), None);
    }
}
