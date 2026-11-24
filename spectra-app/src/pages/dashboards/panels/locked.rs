//! A panel the viewer can't query. It never fetches; the page's warning bar has the one
//! Request access button.

use leptos::prelude::*;
use orbital::components::{Caption1, EmptyState};
use spectra_backend::spectra_query_permission_name;

/// What the panel says instead of a chart.
pub(super) fn locked_detail(source: &str) -> String {
    format!(
        "It reads {source}, which needs {}.",
        spectra_query_permission_name(source)
    )
}

/// A locked panel's body.
#[component]
pub(in crate::pages::dashboards) fn LockedPanel(source: String) -> impl IntoView {
    view! {
        <div data-testid="spectra-dashboard-panel-locked">
            <EmptyState
                message="You don't have access to this panel"
                icon=icondata::AiLockOutlined
            />
            <Caption1>{locked_detail(&source)}</Caption1>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_table_and_permission() {
        assert_eq!(
            locked_detail("checkout_event"),
            "It reads checkout_event, which needs spectra.query.checkout_event."
        );
    }
}
