//! Valence privacy rules for the saved-dashboard tables.
//!
//! Access comes from the dashboard's own Gauge resource bundle (see
//! [`SPECTRA_DASHBOARD`]). The owner holds every action through the bundle's owners group.
//! Sharing grants other people or groups `Edit` (editors) or `View` (viewers). `Delete` is
//! never shared, so only the owner can delete. Every check runs as the caller's own actor;
//! System passes so user-deletion cascades can clear rows.

use std::any::Any;

use async_trait::async_trait;
use gauge::resource_permissions::{
    permission_name, KindDefaultGroups, ResourceAction, ResourceKindDescriptor, UmbrellaPolicy,
};
use valence::privacy_policies::owner::check_owner_by_field;
use valence::{
    Actor, ActorContext, Error, PolicyEvaluator, PrivacyOperation, PrivacyRule, Valence,
};

/// Gauge resource kind for saved dashboards.
///
/// No umbrella groups: a dashboard is reachable only by its owner and the people and groups
/// it is shared with. The default-group and create-permission names exist because the
/// descriptor requires them; Spectra doesn't seed or grant them.
pub const SPECTRA_DASHBOARD: ResourceKindDescriptor = ResourceKindDescriptor {
    prefix: "spectra_dashboard",
    display_label: "Spectra dashboard",
    actions: ResourceAction::STANDARD,
    umbrella: UmbrellaPolicy::None,
    groups: KindDefaultGroups {
        creators: "spectra.dashboard.creators",
        viewers: "spectra.dashboard.viewers",
        editors: "spectra.dashboard.editors",
        operators: "spectra.dashboard.operators",
    },
    create_permission: "CreateSpectraDashboards",
};

/// Gauge permission name for `action` on dashboard `dashboard_id`.
#[must_use]
pub fn dashboard_permission_name(dashboard_id: &str, action: ResourceAction) -> String {
    permission_name(SPECTRA_DASHBOARD, dashboard_id, action)
}

/// Gauge-backed gate on one dashboard, keyed by a record field holding the dashboard id.
///
/// Read needs `View`, update needs `Edit`, delete needs `Delete`. Create needs `Edit` when
/// [`DashboardPermissionPolicy::gate_create`] is set (child rows such as versions) and is
/// refused otherwise (the dashboard row itself, whose create goes through [`DASHBOARD_OWNER`]).
#[derive(Debug, Clone, Copy)]
pub struct DashboardPermissionPolicy {
    /// Namespaced rule name.
    pub rule_name: &'static str,
    /// Record field holding the dashboard id (a string or a record id).
    pub id_field: &'static str,
    /// Whether create is checked against `Edit` instead of refused.
    pub gate_create: bool,
}

fn actor_from_context(actor: &dyn ActorContext) -> valence::Result<Actor> {
    serde_json::from_value(actor.actor_json().clone())
        .map_err(|e| Error::Internal(format!("invalid actor context: {e}")))
}

/// Bare dashboard id from a field that may hold `id`, `table:id` or `{ table, id }`.
pub(crate) fn bare_id(value: &serde_json::Value) -> Option<String> {
    let raw = valence::extract_id_from_select_value(value).ok()?;
    let bare = raw
        .rsplit_once(':')
        .map_or(raw.as_str(), |(_, id)| id)
        .trim();
    (!bare.is_empty()).then(|| bare.to_string())
}

#[async_trait]
impl PolicyEvaluator for DashboardPermissionPolicy {
    fn name(&self) -> &'static str {
        self.rule_name
    }

    fn description(&self) -> Option<&'static str> {
        Some("Spectra dashboard Gauge resource permission (View / Edit / Delete)")
    }

    async fn evaluate(
        &self,
        op: PrivacyOperation,
        record: &serde_json::Value,
        actor: &dyn ActorContext,
        v: &Valence,
    ) -> valence::Result<bool> {
        if actor_from_context(actor)?.is_system() {
            return Ok(true);
        }
        let action = match op {
            PrivacyOperation::Read => ResourceAction::View,
            PrivacyOperation::Update => ResourceAction::Edit,
            PrivacyOperation::Delete => ResourceAction::Delete,
            PrivacyOperation::Create if self.gate_create => ResourceAction::Edit,
            PrivacyOperation::Create => return Ok(false),
        };
        let Some(dashboard_id) = record.get(self.id_field).and_then(bare_id) else {
            return Ok(false);
        };
        let name = dashboard_permission_name(&dashboard_id, action);
        // `actor_can_raw`: Gauge's `actor_can` must not be called from inside a privacy check.
        gauge::service::actor_can_raw(v, &name).await.map_err(|e| {
            Error::Privacy(format!(
                "Dashboard permission check failed for '{name}': {e}"
            ))
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Dashboard rows: read, update and delete through the dashboard's own bundle.
pub const DASHBOARD_RESOURCE: DashboardPermissionPolicy = DashboardPermissionPolicy {
    rule_name: "spectra::DASHBOARD_RESOURCE",
    id_field: "id",
    gate_create: false,
};

/// Version and favourite rows: checked against the parent dashboard's bundle.
pub const DASHBOARD_CHILD_RESOURCE: DashboardPermissionPolicy = DashboardPermissionPolicy {
    rule_name: "spectra::DASHBOARD_CHILD_RESOURCE",
    id_field: "dashboard",
    gate_create: true,
};

fn dashboard_owner_check(record: &serde_json::Value, viewer: &Actor) -> bool {
    !viewer.is_service_user() && check_owner_by_field(record, viewer, "owner")
}

fn favourite_self_check(record: &serde_json::Value, viewer: &Actor) -> bool {
    !viewer.is_service_user() && check_owner_by_field(record, viewer, "user")
}

/// The row's `owner` is the caller, so a signed-in user creates only dashboards they own.
pub const DASHBOARD_OWNER: PrivacyRule = PrivacyRule {
    name: "spectra::DASHBOARD_OWNER",
    description: Some("Allow when the dashboard's owner field is the caller"),
    check: dashboard_owner_check,
};

/// The favourite's `user` is the caller.
pub const FAVOURITE_SELF: PrivacyRule = PrivacyRule {
    name: "spectra::FAVOURITE_SELF",
    description: Some("Allow when the favourite belongs to the caller"),
    check: favourite_self_check,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_id_strips_table_prefix_happy_path() {
        assert_eq!(bare_id(&serde_json::json!("abc")).as_deref(), Some("abc"));
        assert_eq!(
            bare_id(&serde_json::json!("spectra_dashboard:abc")).as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn bare_id_blank_is_none_sad_path() {
        assert_eq!(bare_id(&serde_json::json!("")), None);
        assert_eq!(bare_id(&serde_json::json!(null)), None);
    }

    #[test]
    fn owner_rule_matches_only_the_owner_happy_path() {
        let row = serde_json::json!({ "owner": "user:alice" });
        let alice = Actor::User {
            user_id: "alice".into(),
        };
        let bob = Actor::User {
            user_id: "bob".into(),
        };
        assert!(dashboard_owner_check(&row, &alice));
        assert!(!dashboard_owner_check(&row, &bob));
    }

    #[test]
    fn permission_names_are_per_dashboard_happy_path() {
        let edit = dashboard_permission_name("d1", ResourceAction::Edit);
        let view = dashboard_permission_name("d1", ResourceAction::View);
        assert_ne!(edit, view);
        assert_ne!(edit, dashboard_permission_name("d2", ResourceAction::Edit));
    }
}
