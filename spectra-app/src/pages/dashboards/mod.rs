mod panel;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use orbital::components::{AutoGrid, Caption1, ContentContainer, EmptyState, SpacingSize, Title3};
use orbital::primitives::{Body1, Card, CardContent, CardHeader, Flex, Link};
use spectra_backend::{dashboard_preset, dashboard_presets, spectra_dashboard_path};

use self::panel::DashboardPanelView;

/// Dashboards index: the curated seed dashboards as a card grid.
#[component]
pub fn DashboardIndexPage() -> impl IntoView {
    view! {
        <ContentContainer data_testid="spectra-dashboards-page">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Title3>"Dashboards"</Title3>
                <Caption1>"Curated views of platform metrics"</Caption1>
                <AutoGrid min=Signal::derive(|| "270px".to_string())>
                    {dashboard_presets().iter().map(|preset| {
                        let test_id = format!("spectra-dashboard-card-{}", preset.id);
                        let link_test_id = format!("spectra-dashboard-card-{}-open", preset.id);
                        view! {
                            <div data-testid=test_id>
                                <Card>
                                    <CardHeader>
                                        <Title3>{preset.title}</Title3>
                                    </CardHeader>
                                    <CardContent>
                                        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                                            <Body1>{preset.description}</Body1>
                                            <span data-testid=link_test_id>
                                                <Link href=spectra_dashboard_path(preset.id)>"Open dashboard"</Link>
                                            </span>
                                        </Flex>
                                    </CardContent>
                                </Card>
                            </div>
                        }
                    }).collect_view()}
                </AutoGrid>
            </Flex>
        </ContentContainer>
    }
}

/// One curated dashboard: a metric panel per preset entry.
#[component]
pub fn DashboardDetailPage() -> impl IntoView {
    let params = use_params_map();
    let id = Memo::new(move |_| params.with(|p| p.get("id").unwrap_or_default()));

    view! {
        <ContentContainer data_testid="spectra-dashboard-page">
            {move || match dashboard_preset(&id.get()) {
                Some(preset) => view! {
                    <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                        <Title3>{preset.title}</Title3>
                        <Caption1>{preset.description}</Caption1>
                        <AutoGrid min=Signal::derive(|| "360px".to_string())>
                            {preset.panels.iter().map(|panel| {
                                view! { <DashboardPanelView panel=*panel /> }
                            }).collect_view()}
                        </AutoGrid>
                    </Flex>
                }.into_any(),
                None => view! {
                    <div data-testid="spectra-dashboard-not-found">
                        <EmptyState
                            message="Dashboard not found"
                            description="Pick one from the dashboards list."
                        />
                    </div>
                }.into_any(),
            }}
        </ContentContainer>
    }
}
