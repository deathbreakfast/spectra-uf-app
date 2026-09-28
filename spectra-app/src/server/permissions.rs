//! Spectra query permission gate and Gauge sync helpers.

pub use spectra_backend::{
    spectra_query_permission_name, validate_spectra_query_name, SpectraOpsError,
};

/// Ensure Gauge permissions exist for every registered Spectra schema.
///
/// Creates missing `spectra.query.{table}` rows under the `spectra` permission
/// domain. Does **not** grant them to any user — hosts or operators grant via
/// Gauge after sync.
///
/// Call from host SSR boot (after schemas are registered) with a Valence whose
/// actor can create permissions in the Spectra domain, for example:
///
/// ```rust,ignore
/// spectra_app::ensure_table_query_permissions(&admin_valence).await?;
/// ```
///
/// Returns the number of permissions newly created.
#[cfg(feature = "ssr")]
pub async fn ensure_table_query_permissions(valence: &valence::Valence) -> Result<usize, String> {
    use gauge::service;
    use gauge::types::PermissionCreateInput;
    use spectra_backend::schema_metadata_list;

    let schemas = schema_metadata_list();
    let existing = service::list_permissions(valence, None)
        .await
        .map_err(|e| e.to_string())?;
    let existing_names: std::collections::HashSet<String> =
        existing.into_iter().map(|p| p.name).collect();

    let mut created = 0usize;
    for item in schemas {
        let table = item.table_or_metric.trim();
        if table.is_empty() {
            continue;
        }
        let perm_name = spectra_query_permission_name(table);
        if existing_names.contains(&perm_name) {
            continue;
        }
        service::create_permission(
            PermissionCreateInput {
                name: perm_name,
                description: format!("Query Spectra table or metric `{table}`"),
                owners_group_id: String::new(),
                domain_id: "spectra".into(),
            },
            valence,
        )
        .await
        .map_err(|e| e.to_string())?;
        created += 1;
    }
    Ok(created)
}

/// Table/metric query permission check via Gauge `spectra.query.{table}`.
///
/// Validates the table/metric name (blank / oversized / path-unsafe rejected),
/// resolves Higgs request context, and calls [`gauge::service::actor_can`] for
/// the per-table permission.
///
/// Invalid names surface [`spectra_backend::SpectraQueryNameError`] via
/// [`SpectraOpsError::Validation`]; Gauge denials use [`SpectraOpsError::PermissionDenied`].
///
/// ## Non-SSR builds
///
/// When the `ssr` feature is disabled (WASM client compile), this function is a
/// no-op that returns `Ok(())`. Permission enforcement runs only on the server
/// inside `#[server]` functions — never rely on this stub for authz in client code.
pub async fn require_spectra_query(table: &str) -> Result<(), SpectraOpsError> {
    #[cfg(feature = "ssr")]
    {
        validate_spectra_query_name(table)?;

        let ctx = higgs::Higgs::from_request().await.map_err(|e| {
            SpectraOpsError::ContextResolution(format!("Failed to resolve request context: {e}"))
        })?;

        let valence = ctx.valence().map_err(|e| {
            SpectraOpsError::ContextResolution(format!("Failed to resolve valence: {e}"))
        })?;

        let permission = spectra_query_permission_name(table);
        let allowed = gauge::service::actor_can(&valence, &permission)
            .await
            .map_err(|e| {
                SpectraOpsError::ContextResolution(format!(
                    "Permission check failed for `{permission}`: {e}"
                ))
            })?;

        if !allowed {
            return Err(SpectraOpsError::PermissionDenied { permission });
        }
        Ok(())
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = table;
        Ok(())
    }
}
