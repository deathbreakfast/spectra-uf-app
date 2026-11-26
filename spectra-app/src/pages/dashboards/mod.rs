//! Saved dashboards: the list at `/spectra/dashboards`, one dashboard at
//! `/spectra/dashboards/:id` and its edit page at `/spectra/dashboards/:id/edit`.
//!
//! Both read the store through the server functions in `crate::server::dashboards` and
//! `crate::server::dashboard_view`, as the session actor. `?version=N` on the dashboard
//! page shows saved version N read-only.

mod delete;
mod detail;
mod duplicate;
mod edit;
mod edit_bar;
mod edit_dialogs;
mod edit_grid;
mod edit_save;
mod history;
mod list;
mod new_dialog;
mod panel;
mod panel_editor;
mod panels;
mod section;
mod share;
mod text_panel;
mod toolbar;
mod variables;

pub use detail::DashboardDetailPage;
pub use edit::DashboardEditPage;
pub use list::DashboardIndexPage;

use chrono::{DateTime, Utc};
use leptos::prelude::ServerFnError;
use spectra_backend::dashboards::{DashboardError, Principal, Version};
use spectra_backend::spectra_dashboard_path;

use crate::server::dashboard_error;

/// The message shown when a dashboard action fails: a validation message as written,
/// otherwise the error's own text.
fn action_error_text(err: &ServerFnError) -> String {
    match dashboard_error(err) {
        Some(DashboardError::Validation { message }) => message,
        Some(other) => other.to_string(),
        None => err.to_string(),
    }
}

/// How the list and the page show a save time: the date, in UTC so server and browser
/// render the same text.
fn saved_date(at: DateTime<Utc>) -> String {
    at.format("%b %-d, %Y").to_string()
}

/// Where a dashboard's edit page lives.
fn dashboard_edit_path(id: &str) -> String {
    format!("{}/edit", spectra_dashboard_path(id))
}

/// The name the Duplicate dialog starts with.
fn duplicate_name(title: &str) -> String {
    format!("{title} (copy)")
}

/// The query key that opens a saved version read-only.
const VERSION_QUERY_KEY: &str = "version";

/// A save time with the minute, in UTC.
fn saved_time(at: DateTime<Utc>) -> String {
    at.format("%b %-d, %Y %H:%M UTC").to_string()
}

/// Who saved a version: the author's user id, or a note once that user is gone.
fn author_name(version: &Version) -> String {
    version
        .author
        .clone()
        .unwrap_or_else(|| "a deleted user".to_string())
}

/// The bar above an older version: "Version 3, saved by admin at Nov 3, 2026 09:12 UTC".
fn version_bar_text(version: &Version) -> String {
    format!(
        "Version {}, saved by {} at {}",
        version.number,
        author_name(version),
        saved_time(version.created_at)
    )
}

/// What Restore asks before it saves `number` again on top of `newest`.
fn restore_text(number: u32, newest: &Version) -> String {
    format!(
        "Version {number} becomes version {}. Version {}, saved by {}, stays in the history.",
        newest.number.saturating_add(1),
        newest.number,
        author_name(newest)
    )
}

/// What Delete says about access, given the names of everyone it was shared with.
fn delete_access_text(names: &[String]) -> String {
    let who = if names.is_empty() {
        "Nobody else has access.".to_string()
    } else {
        format!("{} will lose access.", detail::join_names(names))
    };
    format!("{who} The dashboard and its version history are deleted for good.")
}

/// The TagPicker key for a person or group: `user:<id>` or `group:<id>`.
fn principal_key(principal: &Principal) -> String {
    match principal {
        Principal::User(id) => format!("user:{id}"),
        Principal::Group(id) => format!("group:{id}"),
    }
}

/// The person or group behind a [`principal_key`].
fn parse_principal_key(key: &str) -> Option<Principal> {
    match key.split_once(':')? {
        ("user", id) if !id.is_empty() => Some(Principal::User(id.to_string())),
        ("group", id) if !id.is_empty() => Some(Principal::Group(id.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn saved_date_reads_as_a_date() {
        let at = Utc.with_ymd_and_hms(2026, 11, 3, 23, 59, 0).unwrap();
        assert_eq!(saved_date(at), "Nov 3, 2026");
    }

    #[test]
    fn edit_path_follows_the_dashboard_path() {
        assert_eq!(
            dashboard_edit_path("abc"),
            format!("{}/edit", spectra_dashboard_path("abc"))
        );
        assert!(dashboard_edit_path("abc").ends_with("/abc/edit"));
    }

    #[test]
    fn duplicate_name_marks_the_copy() {
        assert_eq!(duplicate_name("Checkout health"), "Checkout health (copy)");
    }

    fn version(number: u32, author: Option<&str>) -> Version {
        Version {
            number,
            author: author.map(str::to_string),
            via: None,
            created_at: Utc.with_ymd_and_hms(2026, 10, 27, 9, 12, 0).unwrap(),
            summary: "Saved changes".into(),
        }
    }

    #[test]
    fn version_bar_names_who_and_when() {
        assert_eq!(
            version_bar_text(&version(3, Some("admin"))),
            "Version 3, saved by admin at Oct 27, 2026 09:12 UTC"
        );
        assert_eq!(
            version_bar_text(&version(3, None)),
            "Version 3, saved by a deleted user at Oct 27, 2026 09:12 UTC"
        );
    }

    #[test]
    fn restore_text_says_nothing_is_lost() {
        assert_eq!(
            restore_text(8, &version(9, Some("release-bot"))),
            "Version 8 becomes version 10. Version 9, saved by release-bot, stays in the history."
        );
    }

    #[test]
    fn delete_text_names_who_loses_access() {
        assert_eq!(
            delete_access_text(&[]),
            "Nobody else has access. The dashboard and its version history are deleted for good."
        );
        let names = vec!["Checkout team".to_string(), "teammate".to_string()];
        assert!(
            delete_access_text(&names).starts_with("Checkout team and teammate will lose access.")
        );
    }

    #[test]
    fn principal_keys_round_trip() {
        for p in [
            Principal::User("admin".into()),
            Principal::Group("g1".into()),
        ] {
            assert_eq!(parse_principal_key(&principal_key(&p)), Some(p));
        }
        assert_eq!(parse_principal_key("user:"), None);
        assert_eq!(parse_principal_key("robot:x"), None);
        assert_eq!(parse_principal_key("nothing"), None);
    }
}
