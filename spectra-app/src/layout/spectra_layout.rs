//! Spectra app shell inside platform layout.

use lepton_shell::AppBarUserMenu;
use leptos::prelude::*;
use leptos_router::{
    components::Outlet,
    hooks::{use_location, use_params_map},
};
use orbital::components::{
    Navigation, NavigationBody, NavigationConfig, NavigationLink, NavigationMaterial,
};
use orbital::primitives::{Breadcrumb, BreadcrumbButton, BreadcrumbItem, Flex, Link};
use uf_integrations::{
    ShellAppBar, ShellAuthMenu, ShellLeftNav, UnifiedFieldAppBar, UnifiedFieldShellLayout,
};
use uf_product::routes::RequireAuthenticated;

use crate::AppMetadata;

#[component]
pub fn SpectraLayout() -> impl IntoView {
    let app_name = AppMetadata::name().to_string();
    let selected_value = RwSignal::new(None::<String>);
    let open_categories = RwSignal::new(Vec::<String>::new());

    view! {
        <div data-testid="spectra-app-root">
            <UnifiedFieldShellLayout>
                <ShellAppBar slot>
                    <UnifiedFieldAppBar
                        app_name=app_name
                        app_id=AppMetadata::id()
                        homepage_url="/".to_string()
                    >
                        <ShellAuthMenu slot:auth_menu>
                            <AppBarUserMenu />
                        </ShellAuthMenu>
                    </UnifiedFieldAppBar>
                </ShellAppBar>
                <ShellLeftNav slot>
                    <Navigation config=NavigationConfig::new().with_selected_value(selected_value).with_open_categories(open_categories)>
                        <NavigationMaterial slot />
                        <NavigationBody slot>
                            <div id="spectra-nav">
                                <NavigationLink path="/spectra" value="/spectra" icon=icondata::AiHomeOutlined test_id="nav-spectra-home">"Home"</NavigationLink>
                                <NavigationLink path="/spectra/schema" value="/spectra/schema" icon=icondata::AiDatabaseOutlined test_id="nav-spectra-schemas">"Schemas"</NavigationLink>
                            </div>
                        </NavigationBody>
                    </Navigation>
                </ShellLeftNav>
                <RequireAuthenticated permission_name="QueryTable">
                    <Flex vertical=true fill=true>
                        <div data-testid="spectra-breadcrumbs">
                            <SpectraBreadcrumbTrail />
                        </div>
                        <Outlet />
                    </Flex>
                </RequireAuthenticated>
            </UnifiedFieldShellLayout>
        </div>
    }
}

#[component]
fn SpectraBreadcrumbTrail() -> impl IntoView {
    let location = use_location();
    let params = use_params_map();
    view! {
        <Breadcrumb>
            <BreadcrumbItem>
                <Link href="/spectra">
                    <BreadcrumbButton>"Spectra"</BreadcrumbButton>
                </Link>
            </BreadcrumbItem>
            {move || {
                let path = location.pathname.get();
                if path == "/spectra" {
                    return None;
                }
                if path == "/spectra/schema" {
                    return Some(view! {
                        <BreadcrumbItem>
                            <BreadcrumbButton current=true>"Schemas"</BreadcrumbButton>
                        </BreadcrumbItem>
                    }.into_any());
                }
                if path.starts_with("/spectra/schema/") {
                    let name = params.with(|p| p.get("name").unwrap_or_default());
                    if path.ends_with("/explore") {
                        return Some(view! {
                            <BreadcrumbItem>
                                <Link href="/spectra/schema">
                                    <BreadcrumbButton>"Schemas"</BreadcrumbButton>
                                </Link>
                            </BreadcrumbItem>
                            <BreadcrumbItem>
                                <Link href=format!("/spectra/schema/{name}")>
                                    <BreadcrumbButton>{name.clone()}</BreadcrumbButton>
                                </Link>
                            </BreadcrumbItem>
                            <BreadcrumbItem>
                                <BreadcrumbButton current=true>"Explore"</BreadcrumbButton>
                            </BreadcrumbItem>
                        }.into_any());
                    }
                    return Some(view! {
                        <BreadcrumbItem>
                            <Link href="/spectra/schema">
                                <BreadcrumbButton>"Schemas"</BreadcrumbButton>
                            </Link>
                        </BreadcrumbItem>
                        <BreadcrumbItem>
                            <BreadcrumbButton current=true>{name}</BreadcrumbButton>
                        </BreadcrumbItem>
                    }.into_any());
                }
                if path.starts_with("/spectra/metric/") && path.ends_with("/explore") {
                    let name = params.with(|p| p.get("name").unwrap_or_default());
                    let detail_href = format!("/spectra/schema/{name}");
                    return Some(view! {
                        <BreadcrumbItem>
                            <Link href="/spectra/schema">
                                <BreadcrumbButton>"Schemas"</BreadcrumbButton>
                            </Link>
                        </BreadcrumbItem>
                        <BreadcrumbItem>
                            <Link href=detail_href>
                                <BreadcrumbButton>{name.clone()}</BreadcrumbButton>
                            </Link>
                        </BreadcrumbItem>
                        <BreadcrumbItem>
                            <BreadcrumbButton current=true>"Explore"</BreadcrumbButton>
                        </BreadcrumbItem>
                    }.into_any());
                }
                None
            }}
        </Breadcrumb>
    }
}
