//! Saved dashboards: the list at `/spectra/dashboards` and one dashboard at
//! `/spectra/dashboards/:id`.
//!
//! Both read the store through the server functions in `crate::server::dashboards` and
//! `crate::server::dashboard_view`, as the session actor.

mod detail;
mod list;
mod new_dialog;
mod panel;
mod panels;
mod section;
mod toolbar;

pub use detail::DashboardDetailPage;
pub use list::DashboardIndexPage;

use chrono::{DateTime, Utc};
use leptos::prelude::ServerFnError;
use spectra_backend::dashboards::DashboardError;

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

/// Title for a copy made with Duplicate.
fn copy_title(title: &str) -> String {
    format!("Copy of {title}")
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
    fn copy_title_names_the_original() {
        assert_eq!(copy_title("Checkout health"), "Copy of Checkout health");
    }
}
