//! Valence store for saved dashboards (`ssr`).
//!
//! Every function takes the caller's own [`Valence`] (the Higgs session actor in a server
//! function) and never switches actor: the privacy rules in [`super::policies`] decide what
//! each caller can read and write. Built-in dashboards from `dashboard_presets` show up in
//! [`list`] and [`get`] and refuse every write with [`DashboardError::ReadOnly`]. Sharing
//! and access reports are in [`super::sharing`].
//!
//! ## Saving and conflicts
//!
//! [`save`] takes the version the editor started from. When the stored version has moved
//! on, it returns [`DashboardError::Conflict`] with the newest number and writes nothing.
//! The check reads and then writes, so two saves that land at the same moment can both
//! pass; the second simply becomes the newer version. Nothing is lost either way, because
//! every save also writes a [`SpectraDashboardVersion`] row.

use std::collections::HashSet;

use chrono::Utc;
use gauge::resource_permissions::{
    delete_resource_permission_bundle, ensure_resource_permission_bundle, ActorId,
    ResourcePermissionSpec,
};
use serde::{Deserialize, Serialize};
use valence::{Model, RecordId, RecordPredicate, Valence};

use super::model::{
    validate_dashboard_text, Dashboard, DashboardError, DashboardFilter, DashboardScope,
    DashboardSummary, Section, Variable, Version,
};
use super::policies::SPECTRA_DASHBOARD;
use crate::builtin_dashboards;
use crate::dashboard_presets::builtin_dashboard;
use crate::generated::{SpectraDashboard, SpectraDashboardFavourite, SpectraDashboardVersion};

const DASHBOARD_TABLE: &str = "spectra_dashboard";
const USER_TABLE: &str = "user";

/// Summary written on a save that doesn't give one.
pub const DEFAULT_SAVE_SUMMARY: &str = "Saved changes";

/// The parts of a dashboard stored as one JSON column.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Body {
    sections: Vec<Section>,
    variables: Vec<Variable>,
    filters: Vec<DashboardFilter>,
}

impl Body {
    fn of(dashboard: &Dashboard) -> Self {
        Self {
            sections: dashboard.sections.clone(),
            variables: dashboard.variables.clone(),
            filters: dashboard.filters.clone(),
        }
    }

    fn to_json(&self) -> Result<serde_json::Value, DashboardError> {
        serde_json::to_value(self).map_err(DashboardError::store)
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, DashboardError> {
        serde_json::from_value(value.clone()).map_err(DashboardError::store)
    }
}

fn map_valence(err: valence::Error) -> DashboardError {
    match err {
        valence::Error::Privacy(_) => DashboardError::Forbidden,
        other => DashboardError::store(other),
    }
}

pub(super) fn caller(v: &Valence) -> Result<String, DashboardError> {
    v.actor()
        .user_id()
        .map(|id| id.rsplit_once(':').map_or(id, |(_, bare)| bare).to_string())
        .filter(|id| !id.is_empty())
        .ok_or(DashboardError::Forbidden)
}

pub(super) fn record_bare_id(rid: &RecordId) -> String {
    valence::extract_id_from_record(rid).unwrap_or_default()
}

fn version_row_id(dashboard_id: &str, number: u32) -> String {
    format!("{dashboard_id}-v{number}")
}

fn to_u32(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

fn row_to_dashboard(row: &SpectraDashboard) -> Result<Dashboard, DashboardError> {
    let body = Body::from_json(row.body())?;
    Ok(Dashboard {
        id: row.id().map_or_default(record_bare_id),
        title: row.title().clone(),
        description: row.description().cloned().unwrap_or_default(),
        owner: Some(record_bare_id(row.owner())),
        read_only: false,
        sections: body.sections,
        variables: body.variables,
        filters: body.filters,
        version: to_u32(*row.version()),
        updated_at: *row.updated_at(),
    })
}

fn version_to_dashboard(
    row: &SpectraDashboard,
    snapshot: &SpectraDashboardVersion,
) -> Result<Dashboard, DashboardError> {
    let body = Body::from_json(snapshot.body())?;
    Ok(Dashboard {
        id: row.id().map_or_default(record_bare_id),
        title: snapshot.title().clone(),
        description: snapshot.description().cloned().unwrap_or_default(),
        owner: Some(record_bare_id(row.owner())),
        read_only: false,
        sections: body.sections,
        variables: body.variables,
        filters: body.filters,
        version: to_u32(*snapshot.number()),
        updated_at: *snapshot.created_at(),
    })
}

fn version_row_to_version(row: &SpectraDashboardVersion) -> Version {
    Version {
        number: to_u32(*row.number()),
        author: row.author().map(record_bare_id),
        via: row.via().cloned(),
        created_at: *row.created_at(),
        summary: row.summary().clone(),
    }
}

pub(super) async fn load_row(v: &Valence, id: &str) -> Result<SpectraDashboard, DashboardError> {
    SpectraDashboard::get(id, v, valence::use_!(r"When you open, save, share or delete a **Spectra dashboard**, we **load the dashboard** by its id: its title, owner and panel queries. Opening shows it to you; the other actions use it to check the version you edited and who owns it."))
        .await
        .map_err(map_valence)?
        .ok_or(DashboardError::NotFound)
}

async fn favourite_ids(v: &Valence, user_id: &str) -> Result<HashSet<String>, DashboardError> {
    let rows = SpectraDashboardFavourite::query(v, valence::use_!(r"On the **Spectra dashboards** list, we **load the dashboards you starred** so they show with a star and sort to the top. Only you see your own stars."))
        .where_user(RecordPredicate::Equals(RecordId::new(USER_TABLE, user_id)))
        .await
        .map_err(map_valence)?;
    Ok(rows.iter().map(|f| record_bare_id(f.dashboard())).collect())
}

/// List the dashboards the caller can open.
///
/// `Mine` is what the caller owns, `Shared` is what others shared with them, and `All` is
/// both plus the built-ins. `q` keeps rows whose title or description contains it, ignoring
/// case. Favourites sort first, then titles A to Z.
///
/// # Errors
///
/// [`DashboardError::Forbidden`] for a caller who isn't a signed-in user;
/// [`DashboardError::Store`] when Valence fails.
pub async fn list(
    v: &Valence,
    scope: DashboardScope,
    q: Option<&str>,
) -> Result<Vec<DashboardSummary>, DashboardError> {
    let me = caller(v)?;
    let favourites = favourite_ids(v, &me).await?;
    let mut query = SpectraDashboard::query(
        v,
        valence::use_!(
            r"On the **Spectra dashboards** list, we **load every dashboard you can open**: ones you own and ones shared with you. The list shows each one's title, description, owner and when it was last saved."
        ),
    );
    if scope == DashboardScope::Mine {
        query = query.where_owner(RecordPredicate::Equals(RecordId::new(USER_TABLE, &me)));
    }
    let rows = query.await.map_err(map_valence)?;

    let mut out = Vec::new();
    if scope == DashboardScope::All {
        out.extend(
            builtin_dashboards()
                .iter()
                .map(|d| d.summary(favourites.contains(&d.id))),
        );
    }
    for row in &rows {
        let owner = record_bare_id(row.owner());
        // Mine is filtered in the query; Shared (readable, not owned) still needs this pass.
        if scope != DashboardScope::Shared || owner != me {
            let dashboard = row_to_dashboard(row)?;
            out.push(dashboard.summary(favourites.contains(&dashboard.id)));
        }
    }
    if let Some(needle) = q.map(str::trim).filter(|s| !s.is_empty()) {
        let needle = needle.to_lowercase();
        out.retain(|d| {
            d.title.to_lowercase().contains(&needle)
                || d.description.to_lowercase().contains(&needle)
        });
    }
    out.sort_by(|a, b| {
        b.favourite
            .cmp(&a.favourite)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    Ok(out)
}

/// Load one dashboard, or one of its saved versions when `version` is set.
///
/// # Errors
///
/// [`DashboardError::NotFound`] when the dashboard or version doesn't exist or the caller
/// can't open it.
pub async fn get(v: &Valence, id: &str, version: Option<u32>) -> Result<Dashboard, DashboardError> {
    if let Some(builtin) = builtin_dashboard(id) {
        return match version {
            None | Some(0) => Ok(builtin),
            Some(_) => Err(DashboardError::NotFound),
        };
    }
    let row = load_row(v, id).await?;
    match version {
        None => row_to_dashboard(&row),
        Some(n) if n == to_u32(*row.version()) => row_to_dashboard(&row),
        Some(n) => {
            let snapshot = SpectraDashboardVersion::get(&version_row_id(id, n), v, valence::use_!(r"When you open an **older version of a Spectra dashboard** from its history, we **load that saved version**: its title and panel queries as they were then. The page shows it to you read-only."))
                .await
                .map_err(map_valence)?
                .ok_or(DashboardError::NotFound)?;
            version_to_dashboard(&row, &snapshot)
        }
    }
}

async fn write_version(
    v: &Valence,
    dashboard: &Dashboard,
    number: u32,
    summary: &str,
    via: Option<String>,
) -> Result<(), DashboardError> {
    let author = caller(v)?;
    let row = SpectraDashboardVersion::new(
        RecordId::new(DASHBOARD_TABLE, &dashboard.id),
        i64::from(number),
        Some(RecordId::new(USER_TABLE, &author)),
        via,
        summary.to_string(),
        dashboard.title.clone(),
        Some(dashboard.description.clone()).filter(|d| !d.is_empty()),
        Body::of(dashboard).to_json()?,
        Utc::now(),
    )
    .map_err(map_valence)?;
    SpectraDashboardVersion::upsert(&version_row_id(&dashboard.id, number), row, v, valence::use_!(r"Each time you save a **Spectra dashboard**, we **keep a copy of that version**: its title, panel queries, who saved it and a one-line summary. Anyone who can open the dashboard sees these in its history and can restore one. Versions are deleted after 180 days."))
        .await
        .map_err(map_valence)?;
    Ok(())
}

/// Create a dashboard owned by the caller, empty or copied from `from`.
///
/// `from` may name a built-in or any dashboard the caller can open; its sections,
/// variables and filters are copied. The new dashboard gets its own Gauge permission bundle
/// with the caller as owner, and starts at version 1.
///
/// # Errors
///
/// [`DashboardError::Validation`] for a bad title or description;
/// [`DashboardError::NotFound`] when `from` can't be opened; [`DashboardError::Store`] when
/// Gauge or Valence fails.
pub async fn create(
    v: &Valence,
    title: &str,
    description: &str,
    from: Option<&str>,
) -> Result<Dashboard, DashboardError> {
    let title = title.trim();
    let description = description.trim();
    validate_dashboard_text(title, description)?;
    let me = caller(v)?;
    let (body, summary) = match from {
        Some(source_id) => {
            let source = get(v, source_id, None).await?;
            (
                Body::of(&source),
                format!("Copied from \"{}\"", source.title),
            )
        }
        None => (Body::default(), "Created".to_string()),
    };

    let id = uuid::Uuid::new_v4().simple().to_string();
    let actor = ActorId::from_valence(v).ok_or(DashboardError::Forbidden)?;
    ensure_resource_permission_bundle(
        v,
        ResourcePermissionSpec {
            kind: SPECTRA_DASHBOARD,
            resource_id: id.clone(),
            // Gauge names it "Spectra dashboard {id}". Not the title: that's user content
            // and would go stale on rename.
            display_name: String::new(),
            actions: Vec::new(),
            actor,
        },
    )
    .await
    .map_err(DashboardError::store)?;

    let now = Utc::now();
    let row = SpectraDashboard::new(
        title.to_string(),
        Some(description.to_string()).filter(|d| !d.is_empty()),
        RecordId::new(USER_TABLE, &me),
        body.to_json()?,
        1,
        now,
        now,
    )
    .map_err(map_valence)?;
    let created = SpectraDashboard::upsert(&id, row, v, valence::use_!(r"When you create or duplicate a **Spectra dashboard**, we **save the new dashboard** with you as its owner: its title, description and panel queries. It then shows in your dashboards list and opens for you and anyone you share it with."))
        .await
        .map_err(map_valence);
    let created = match created {
        Ok(row) => row,
        Err(err) => {
            // Don't leave an orphan bundle behind; the create error is the one to report.
            let _ = delete_resource_permission_bundle(v, SPECTRA_DASHBOARD, &id).await;
            return Err(err);
        }
    };
    let dashboard = row_to_dashboard(&created)?;
    write_version(v, &dashboard, 1, &summary, None).await?;
    Ok(dashboard)
}

/// Save `dashboard` as a new version, if nobody saved since `expected_version`.
///
/// `summary` defaults to [`DEFAULT_SAVE_SUMMARY`]. `via` names an agent saving for the
/// caller (such as `MCP`) and is recorded on the version.
///
/// # Errors
///
/// [`DashboardError::ReadOnly`] for a built-in, [`DashboardError::Conflict`] when the stored
/// version isn't `expected_version`, [`DashboardError::Forbidden`] when the caller can open
/// but not edit it, [`DashboardError::Validation`] for bad content.
pub async fn save(
    v: &Valence,
    dashboard: &Dashboard,
    expected_version: u32,
    summary: Option<&str>,
    via: Option<String>,
) -> Result<Dashboard, DashboardError> {
    if builtin_dashboard(&dashboard.id).is_some() {
        return Err(DashboardError::ReadOnly);
    }
    let mut next = dashboard.clone();
    next.title = next.title.trim().to_string();
    next.description = next.description.trim().to_string();
    next.validate()?;
    let row = load_row(v, &dashboard.id).await?;
    let current = to_u32(*row.version());
    if current != expected_version {
        return Err(DashboardError::Conflict { current });
    }
    let number = current.saturating_add(1);
    let body = Body::of(&next).to_json()?;
    let updated = row
        .get_mutable(v, valence::use_!(r"When you save a **Spectra dashboard**, we **write your changes** to its title, description and panel queries and move it to the next version number. Everyone who can open the dashboard sees the new version."))
        .set_title(next.title.clone())
        .and_then(|m| m.set_description(next.description.clone()))
        .and_then(|m| m.set_body(body))
        .and_then(|m| m.set_version(i64::from(number)))
        .and_then(|m| m.set_updated_at(Utc::now()))
        .map_err(map_valence)?
        .commit()
        .await
        .map_err(map_valence)?;
    let saved = row_to_dashboard(&updated)?;
    write_version(
        v,
        &saved,
        number,
        summary
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_SAVE_SUMMARY),
        via,
    )
    .await?;
    Ok(saved)
}

/// Copy a dashboard the caller can open into a new one they own, titled `title`.
///
/// # Errors
///
/// As [`create`].
pub async fn duplicate(v: &Valence, id: &str, title: &str) -> Result<Dashboard, DashboardError> {
    let source = get(v, id, None).await?;
    create(v, title, &source.description, Some(id)).await
}

/// Delete a dashboard, its versions and everyone's favourites of it. Owner only.
///
/// # Errors
///
/// [`DashboardError::ReadOnly`] for a built-in, [`DashboardError::Forbidden`] for anyone
/// but the owner, [`DashboardError::NotFound`] when the caller can't open it.
pub async fn delete(v: &Valence, id: &str) -> Result<(), DashboardError> {
    if builtin_dashboard(id).is_some() {
        return Err(DashboardError::ReadOnly);
    }
    // Only the owner holds the dashboard's Gauge `Delete` permission, so the delete policy
    // refuses everyone else and `map_valence` turns that into `Forbidden`.
    load_row(v, id).await?;
    SpectraDashboard::delete_now(id, v, valence::use_!(r"When the owner **deletes a Spectra dashboard**, we **remove the dashboard** right away, together with its saved versions and every person's star on it. Nobody can open it afterwards."))
        .await
        .map_err(map_valence)?;
    delete_resource_permission_bundle(v, SPECTRA_DASHBOARD, id)
        .await
        .map_err(DashboardError::store)
}

/// Saved versions of a dashboard, newest first. Empty for a built-in.
///
/// # Errors
///
/// [`DashboardError::NotFound`] when the caller can't open the dashboard.
pub async fn list_versions(v: &Valence, id: &str) -> Result<Vec<Version>, DashboardError> {
    if builtin_dashboard(id).is_some() {
        return Ok(Vec::new());
    }
    load_row(v, id).await?;
    let rows = SpectraDashboardVersion::query(v, valence::use_!(r"In a **Spectra dashboard's history**, we **list its saved versions**: number, who saved it, through which agent, when, and the one-line summary. Anyone who can open the dashboard sees this list."))
        .where_dashboard(RecordPredicate::Equals(RecordId::new(DASHBOARD_TABLE, id)))
        .await
        .map_err(map_valence)?;
    let mut versions: Vec<Version> = rows.iter().map(version_row_to_version).collect();
    versions.sort_by_key(|a| std::cmp::Reverse(a.number));
    Ok(versions)
}

/// Save version `number` again as the newest version. Earlier versions stay.
///
/// # Errors
///
/// [`DashboardError::NotFound`] when the version doesn't exist (or has expired), otherwise
/// as [`save`].
pub async fn restore(
    v: &Valence,
    id: &str,
    number: u32,
    via: Option<String>,
) -> Result<Dashboard, DashboardError> {
    if builtin_dashboard(id).is_some() {
        return Err(DashboardError::ReadOnly);
    }
    let current = get(v, id, None).await?;
    let old = get(v, id, Some(number)).await?;
    save(
        v,
        &old,
        current.version,
        Some(&format!("Restored version {number}")),
        via,
    )
    .await
}

/// Star or unstar a dashboard for the caller.
///
/// # Errors
///
/// [`DashboardError::NotFound`] when the caller can't open the dashboard.
pub async fn set_favourite(v: &Valence, id: &str, favourite: bool) -> Result<(), DashboardError> {
    let me = caller(v)?;
    get(v, id, None).await?;
    let existing = SpectraDashboardFavourite::query(v, valence::use_!(r"When you **star or unstar a Spectra dashboard**, we **look up your existing star** on it, so starring twice keeps one star and unstarring removes it. Only you see your stars."))
        .where_user(RecordPredicate::Equals(RecordId::new(USER_TABLE, &me)))
        .where_dashboard(RecordPredicate::Equals(RecordId::new(DASHBOARD_TABLE, id)))
        .await
        .map_err(map_valence)?;
    if favourite {
        if existing.is_empty() {
            let row = SpectraDashboardFavourite::new(
                RecordId::new(USER_TABLE, &me),
                RecordId::new(DASHBOARD_TABLE, id),
                Utc::now(),
            )
            .map_err(map_valence)?;
            SpectraDashboardFavourite::create(row, v, valence::use_!(r"When you **star a Spectra dashboard**, we **save the star** with your user id and the dashboard, so it sorts to the top of your dashboards list. Only you see your stars."))
                .await
                .map_err(map_valence)?;
        }
    } else {
        for row in existing {
            if let Some(rid) = row.id() {
                SpectraDashboardFavourite::delete_now(&record_bare_id(rid), v, valence::use_!(r"When you **unstar a Spectra dashboard**, we **remove your star** on it, so it no longer sorts to the top of your dashboards list."))
                    .await
                    .map_err(map_valence)?;
            }
        }
    }
    Ok(())
}
