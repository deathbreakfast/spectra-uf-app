//! Eager `/spectra` routes for the Playwright host.
//!
//! Production [`spectra_app::SpectraRoutes`] wraps leaf pages in `Lazy` for
//! wasm-split. Nested `Lazy` under `ParentRoute` still panics on
//! `hydrate_body` in this Leptos pin, so the lab host mounts the same page
//! components without `Lazy`.

use leptos::prelude::*;
use leptos_router::{
    components::{ParentRoute, Route},
    path,
};
use spectra_app::{
    DashboardDetailPage, DashboardIndexPage, EventExplorePage, MetricExplorePage, SchemaDetailPage,
    SchemaIndexPage, SpectraHomePage, SpectraLayout,
};

/// Same paths as [`spectra_app::SpectraRoutes`], without Lazy route views.
#[component(transparent)]
pub fn SpectraRoutesEager() -> impl leptos_router::MatchNestedRoutes + Clone {
    spectra_app::ensure_help_steps_linked();
    view! {
        <ParentRoute path=path!("spectra") view=SpectraLayout>
            <Route path=path!("") view=SpectraHomePage />
            <Route path=path!("dashboards") view=DashboardIndexPage />
            <Route path=path!("dashboards/:id") view=DashboardDetailPage />
            <Route path=path!("schema") view=SchemaIndexPage />
            <Route path=path!("schema/:name") view=SchemaDetailPage />
            <Route path=path!("metric/:name/explore") view=MetricExplorePage />
            <Route path=path!("schema/:name/explore") view=EventExplorePage />
        </ParentRoute>
    }
    .into_inner()
}
