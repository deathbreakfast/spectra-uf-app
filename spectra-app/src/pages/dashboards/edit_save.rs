//! Saving the edit page's draft: Save with a version check, and Save as a copy after a
//! conflict.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use spectra_backend::dashboards::{Dashboard, DashboardError, Version};

use super::{action_error_text, copy_title, dashboard_edit_path};
use crate::server::{dashboard_error, duplicate_dashboard, list_versions, save_dashboard};

/// A save that lost to a newer version.
#[derive(Debug, Clone)]
pub(super) struct Conflict {
    /// The newest stored version.
    pub current: u32,
    /// The version the edit started from.
    pub editing: u32,
    /// What the conflict bar says.
    pub message: String,
}

/// The conflict bar's message: who saved which version when, and that nothing was lost.
pub(super) fn conflict_text(saved: Option<&Version>, current: u32, editing: u32) -> String {
    let who = saved.map_or_else(
        || format!("Someone saved version {current}"),
        |v| {
            let author = v.author.as_deref().unwrap_or("Someone");
            let via = v
                .via
                .as_ref()
                .map_or_else(String::new, |via| format!(" through {via}"));
            format!(
                "{author} saved version {current}{via} at {} UTC",
                v.created_at.format("%H:%M")
            )
        },
    );
    format!("{who} while you were editing version {editing}. Your changes are still here.")
}

/// The conflict bar's message from the version list, or, when the list didn't load, the
/// plain message and a line saying who saved couldn't be shown.
pub(super) fn conflict_message(
    versions: Result<&[Version], &str>,
    current: u32,
    editing: u32,
) -> String {
    versions.map_or_else(
        |_| {
            format!(
                "{} Who saved it and when couldn't be loaded.",
                conflict_text(None, current, editing)
            )
        },
        |versions| {
            conflict_text(
                versions.iter().find(|v| v.number == current),
                current,
                editing,
            )
        },
    )
}

/// The save state and actions [`use_dashboard_save`] gives the edit page.
pub(super) struct DashboardSave {
    /// A save is in flight.
    pub saving: RwSignal<bool>,
    /// Why the last save or copy failed, other than a conflict.
    pub save_error: RwSignal<Option<String>>,
    /// The last save lost to a newer version.
    pub conflict: RwSignal<Option<Conflict>>,
    /// Save as a copy is in flight.
    pub copying: RwSignal<bool>,
    /// Save the draft against the version the edit started from.
    pub on_save: Callback<()>,
    /// Save the draft as a new dashboard and open its edit page.
    pub save_copy: Callback<()>,
}

/// Save and Save as a copy for the draft. A successful save makes it the new `base`.
pub(super) fn use_dashboard_save(
    base: RwSignal<Dashboard>,
    draft: RwSignal<Dashboard>,
) -> DashboardSave {
    let saving = RwSignal::new(false);
    let save_error = RwSignal::new(None::<String>);
    let conflict = RwSignal::new(None::<Conflict>);
    let copying = RwSignal::new(false);
    let navigate = use_navigate();

    let on_save = Callback::new(move |()| {
        if saving.get_untracked() {
            return;
        }
        let next = draft.get_untracked();
        let editing = base.with_untracked(|b| b.version);
        saving.set(true);
        save_error.set(None);
        leptos::task::spawn_local_scoped(async move {
            match save_dashboard(next.clone(), editing, None).await {
                Ok(saved) => {
                    base.set(saved.clone());
                    draft.set(saved);
                    conflict.set(None);
                }
                Err(e) => match dashboard_error(&e) {
                    Some(DashboardError::Conflict { current }) => {
                        let versions = list_versions(next.id.clone()).await.map_err(|e| {
                            let reason = action_error_text(&e);
                            log::warn!(
                                "Spectra dashboard {}: couldn't load versions for the conflict bar: {reason}",
                                next.id
                            );
                            reason
                        });
                        let message = conflict_message(
                            versions.as_deref().map_err(String::as_str),
                            current,
                            editing,
                        );
                        conflict.set(Some(Conflict {
                            current,
                            editing,
                            message,
                        }));
                    }
                    _ => save_error.set(Some(format!(
                        "Couldn't save your changes: {}",
                        action_error_text(&e)
                    ))),
                },
            }
            saving.set(false);
        });
    });

    let save_copy = Callback::new(move |()| {
        if copying.get_untracked() {
            return;
        }
        let next = draft.get_untracked();
        let navigate = navigate.clone();
        copying.set(true);
        leptos::task::spawn_local_scoped(async move {
            let result = async {
                let copy = duplicate_dashboard(next.id.clone(), copy_title(&next.title)).await?;
                let version = copy.version;
                let edited = Dashboard {
                    id: copy.id,
                    title: copy.title,
                    owner: copy.owner,
                    read_only: false,
                    version,
                    updated_at: copy.updated_at,
                    ..next
                };
                save_dashboard(edited, version, None).await
            }
            .await;
            match result {
                Ok(saved) => {
                    // The original's draft is now saved in the copy.
                    draft.set(base.get_untracked());
                    navigate(&dashboard_edit_path(&saved.id), NavigateOptions::default());
                }
                Err(e) => save_error.set(Some(format!(
                    "Couldn't save a copy: {}",
                    action_error_text(&e)
                ))),
            }
            copying.set(false);
        });
    });

    DashboardSave {
        saving,
        save_error,
        conflict,
        copying,
        on_save,
        save_copy,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    #[test]
    fn conflict_text_names_the_agent_and_time() {
        let v = Version {
            number: 9,
            author: Some("release-bot".into()),
            via: Some("MCP".into()),
            created_at: Utc.with_ymd_and_hms(2026, 11, 3, 9, 30, 0).unwrap(),
            summary: String::new(),
        };
        assert_eq!(
            conflict_text(Some(&v), 9, 8),
            "release-bot saved version 9 through MCP at 09:30 UTC while you were editing version 8. Your changes are still here."
        );
        let person = Version {
            via: None,
            author: Some("rosa".into()),
            ..v
        };
        assert!(conflict_text(Some(&person), 9, 8).starts_with("rosa saved version 9 at 09:30 UTC"));
        assert_eq!(
            conflict_text(None, 9, 8),
            "Someone saved version 9 while you were editing version 8. Your changes are still here."
        );
    }

    #[test]
    fn conflict_message_says_when_the_details_did_not_load() {
        let v = Version {
            number: 9,
            author: Some("rosa".into()),
            via: None,
            created_at: Utc.with_ymd_and_hms(2026, 11, 3, 9, 30, 0).unwrap(),
            summary: String::new(),
        };
        assert!(conflict_message(Ok(std::slice::from_ref(&v)), 9, 8).starts_with("rosa saved"));
        assert_eq!(
            conflict_message(Err("timeout"), 9, 8),
            "Someone saved version 9 while you were editing version 8. Your changes are still here. Who saved it and when couldn't be loaded."
        );
    }
}
