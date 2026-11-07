//! Lazy-loaded route views for WASM code-splitting (`cargo leptos --split`).

use leptos::prelude::*;
use leptos_router::{lazy_route, LazyRoute};

use crate::{
    CallsiteGraphPage, DashboardDetailPage, DashboardIndexPage, EventExplorePage,
    MetricExplorePage, SchemaDetailPage, SchemaIndexPage, SpectraHomePage, SpectraLayout,
};

/// Prefetch the spectra family WASM chunk (leaf pages share split modules).
pub async fn prefetch_family() {
    SpectraHomeRoute::preload().await;
}

/// Eager layout shell for `/spectra/*` ParentRoute (auth gate lives inside [`SpectraLayout`]).
#[component]
pub fn SpectraLayoutRouteView() -> impl IntoView {
    view! { <SpectraLayout /> }
}

/// Lazy `/spectra` home.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpectraHomeRoute;

#[lazy_route]
impl LazyRoute for SpectraHomeRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <SpectraHomePage /> }.into_any()
    }
}

/// Lazy `/spectra/schema`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaIndexRoute;

#[lazy_route]
impl LazyRoute for SchemaIndexRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <SchemaIndexPage /> }.into_any()
    }
}

/// Lazy `/spectra/schema/:name`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaDetailRoute;

#[lazy_route]
impl LazyRoute for SchemaDetailRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <SchemaDetailPage /> }.into_any()
    }
}

/// Lazy `/spectra/metric/:name/explore`.
#[derive(Clone, Copy, Debug, Default)]
pub struct MetricExploreRoute;

#[lazy_route]
impl LazyRoute for MetricExploreRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <MetricExplorePage /> }.into_any()
    }
}

/// Lazy `/spectra/schema/:name/explore`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EventExploreRoute;

#[lazy_route]
impl LazyRoute for EventExploreRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <EventExplorePage /> }.into_any()
    }
}

/// Lazy `/spectra/dashboards`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DashboardIndexRoute;

#[lazy_route]
impl LazyRoute for DashboardIndexRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <DashboardIndexPage /> }.into_any()
    }
}

/// Lazy `/spectra/dashboards/:id`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DashboardDetailRoute;

#[lazy_route]
impl LazyRoute for DashboardDetailRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <DashboardDetailPage /> }.into_any()
    }
}

/// Lazy `/spectra/graph`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CallsiteGraphRoute;

#[lazy_route]
impl LazyRoute for CallsiteGraphRoute {
    fn data() -> Self {
        Self
    }

    fn view(_this: Self) -> AnyView {
        view! { <CallsiteGraphPage /> }.into_any()
    }
}
