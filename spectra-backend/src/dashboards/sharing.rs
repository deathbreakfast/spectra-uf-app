//! Sharing a saved dashboard and reporting who can reach it (`ssr`).
//!
//! A dashboard's access lives in its Gauge resource bundle (see
//! [`super::policies::SPECTRA_DASHBOARD`]). [`share`] grants the bundle's `Edit` and `View`
//! permissions to people and groups; [`access`] reads them back with the caller's own rights.

use std::collections::BTreeSet;

use gauge::resource_permissions::{owners_group_id, permission_record_id, ResourceAction};
use gauge::types::PrincipalKind;
use valence::Valence;

use super::model::{DashboardAccess, DashboardError, Principal};
use super::policies::{dashboard_permission_name, SPECTRA_DASHBOARD};
use super::store::{caller, load_row, record_bare_id};
use crate::dashboard_presets::builtin_dashboard;

fn principals_from(
    list: Vec<gauge::types::PrincipalRefDto>,
    owners_group: &str,
) -> BTreeSet<Principal> {
    list.into_iter()
        .filter_map(|p| match p.kind {
            PrincipalKind::User => Some(Principal::User(p.id)),
            PrincipalKind::Group if p.id != owners_group => Some(Principal::Group(p.id)),
            _ => None,
        })
        .collect()
}

pub(super) async fn granted(
    v: &Valence,
    id: &str,
    action: ResourceAction,
) -> Result<BTreeSet<Principal>, DashboardError> {
    let detail = gauge::service::get_permission_detail(
        &permission_record_id(SPECTRA_DASHBOARD, id, action),
        v,
    )
    .await
    .map_err(DashboardError::store)?
    .ok_or(DashboardError::NotFound)?;
    Ok(principals_from(
        detail.allow_list,
        &owners_group_id(SPECTRA_DASHBOARD, id),
    ))
}

async fn set_grant(
    v: &Valence,
    id: &str,
    action: ResourceAction,
    principal: &Principal,
    grant: bool,
) -> Result<(), DashboardError> {
    let permission = permission_record_id(SPECTRA_DASHBOARD, id, action);
    let result = match (principal, grant) {
        (Principal::User(user), true) => {
            gauge::service::grant_permission_to_user(&permission, user, v).await
        }
        (Principal::User(user), false) => {
            gauge::service::revoke_permission_from_user(&permission, user, v).await
        }
        (Principal::Group(group), true) => {
            gauge::service::grant_permission_to_group(&permission, group, v).await
        }
        (Principal::Group(group), false) => {
            gauge::service::revoke_permission_from_group(&permission, group, v).await
        }
    };
    result.map_err(DashboardError::store)
}

/// Make `editors` the people and groups who can edit, and `viewers` those who can only view.
/// Owner only.
///
/// Editors are granted the dashboard's `Edit` and `View` permissions, viewers only `View`;
/// anyone left off both lists loses both. Someone on both lists is an editor.
///
/// # Errors
///
/// [`DashboardError::ReadOnly`] for a built-in, [`DashboardError::Forbidden`] for anyone
/// but the owner, [`DashboardError::Store`] when Gauge refuses a grant (an unknown group).
pub async fn share(
    v: &Valence,
    id: &str,
    editors: &[Principal],
    viewers: &[Principal],
) -> Result<DashboardAccess, DashboardError> {
    if builtin_dashboard(id).is_some() {
        return Err(DashboardError::ReadOnly);
    }
    let me = caller(v)?;
    let row = load_row(v, id).await?;
    if record_bare_id(row.owner()) != me {
        return Err(DashboardError::Forbidden);
    }
    let owner = Principal::User(me);
    let want_edit: BTreeSet<Principal> = editors.iter().filter(|p| **p != owner).cloned().collect();
    let want_view: BTreeSet<Principal> = viewers
        .iter()
        .filter(|p| **p != owner)
        .cloned()
        .chain(want_edit.iter().cloned())
        .collect();
    for (action, want) in [
        (ResourceAction::Edit, &want_edit),
        (ResourceAction::View, &want_view),
    ] {
        let have = granted(v, id, action).await?;
        for p in want.difference(&have) {
            set_grant(v, id, action, p, true).await?;
        }
        for p in have.difference(want) {
            set_grant(v, id, action, p, false).await?;
        }
    }
    access(v, id).await
}

/// Who can reach a dashboard, and what the caller may do with it.
///
/// Editors and viewers are listed only for the owner; other callers get empty lists.
///
/// # Errors
///
/// [`DashboardError::NotFound`] when the caller can't open the dashboard.
pub async fn access(v: &Valence, id: &str) -> Result<DashboardAccess, DashboardError> {
    if builtin_dashboard(id).is_some() {
        return Ok(DashboardAccess {
            owner: None,
            editors: Vec::new(),
            viewers: Vec::new(),
            can_edit: false,
            can_delete: false,
            can_share: false,
        });
    }
    let me = caller(v)?;
    let row = load_row(v, id).await?;
    let owner = record_bare_id(row.owner());
    let is_owner = owner == me;
    let can = |action| {
        let name = dashboard_permission_name(id, action);
        async move {
            gauge::service::actor_can(v, &name)
                .await
                .map_err(DashboardError::store)
        }
    };
    let can_edit = can(ResourceAction::Edit).await?;
    let can_delete = is_owner && can(ResourceAction::Delete).await?;
    let (editors, viewers) = if is_owner {
        let editors = granted(v, id, ResourceAction::Edit).await?;
        let viewers = granted(v, id, ResourceAction::View).await?;
        let viewers = viewers.difference(&editors).cloned().collect();
        (editors.into_iter().collect(), viewers)
    } else {
        (Vec::new(), Vec::new())
    };
    Ok(DashboardAccess {
        owner: Some(owner),
        editors,
        viewers,
        can_edit,
        can_delete,
        can_share: is_owner,
    })
}
