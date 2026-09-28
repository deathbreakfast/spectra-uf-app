use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};
use orbital::components::{Caption1, ContentContainer, EmptyState, SpacingSize, StatCard, Title3};
use orbital::primitives::{
    Body1, Button, ButtonAppearance, Card, CardContent, CardHeader, Flex, FlexWrap, Link,
    MessageBar, MessageBarIntent, SearchBox, SearchBoxAppearance, SearchBoxBind,
};
use uf_product::services::report_server_fn_error;

use crate::components::schema::SchemaCard;
use crate::server::{get_spectra_dashboard_summary, SpectraDashboardSummary};
use spectra_backend::{
    spectra_schema_explore_path, spectra_schema_path, validate_spectra_query_name,
};

/// Spectra home dashboard: catalog stats, recent schemas, and quick open.
#[component]
pub fn SpectraHomePage() -> impl IntoView {
    let summary_res = Resource::new(|| (), |_| get_spectra_dashboard_summary());

    view! {
        <ContentContainer data_testid="spectra-home-page">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Title3>"Spectra"</Title3>
                <Caption1>"Inspect schemas, event logs, and metrics"</Caption1>
                <Suspense fallback=|| view! { <Body1>"Loading dashboard…"</Body1> }>
                    {move || match summary_res.get() {
                        Some(Ok(summary)) if summary.schema_count == 0 => view! {
                            <EmptyState
                                message="No schemas registered"
                                description="Register Spectra schemas in the host to browse them here."
                            />
                        }.into_any(),
                        Some(Ok(summary)) => view! { <DashboardBody summary=summary /> }.into_any(),
                        Some(Err(e)) => {
                            let soft = report_server_fn_error(&e);
                            if soft {
                                view! {
                                    <EmptyState message="Couldn't load dashboard" />
                                }
                                .into_any()
                            } else {
                                view! {
                                    <MessageBar intent=MessageBarIntent::Error>
                                        {format!("Failed to load dashboard: {e}")}
                                    </MessageBar>
                                }
                                .into_any()
                            }
                        }
                        None => view! { <Body1>"Loading…"</Body1> }.into_any(),
                    }}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

#[component]
fn DashboardBody(
    /// Dashboard summary payload.
    summary: SpectraDashboardSummary,
) -> impl IntoView {
    let activity = summary.activity_24h_event_rows;
    view! {
        <div id="spectra-dashboard-stats">
            <Flex gap=SpacingSize::Size160.flex_gap() wrap=FlexWrap::Wrap>
                <div data-testid="spectra-stat-schemas">
                    <StatCard label="Schemas" value=Signal::derive(move || summary.schema_count.to_string()) />
                </div>
                <div data-testid="spectra-stat-event-tables">
                    <StatCard label="Event tables" value=Signal::derive(move || summary.event_table_count.to_string()) />
                </div>
                <div data-testid="spectra-stat-metrics">
                    <StatCard label="Metrics" value=Signal::derive(move || summary.metric_count.to_string()) />
                </div>
                {activity.map(|n| {
                    let value = Signal::derive(move || n.to_string());
                    view! {
                        <div data-testid="spectra-stat-24h-events">
                            <StatCard label="24h event rows" value=value />
                        </div>
                    }
                })}
            </Flex>
        </div>
        <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
            <div id="spectra-home-recent">
                <Card>
                    <CardHeader>
                        <Title3>"Recent schemas"</Title3>
                    </CardHeader>
                    <CardContent>
                        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                            {summary.recent_schemas.into_iter().map(|item| view! {
                                <SchemaCard item=item />
                            }).collect_view()}
                            <div id="spectra-home-view-all">
                                <Link href="/spectra/schema">"View all schemas →"</Link>
                            </div>
                        </Flex>
                    </CardContent>
                </Card>
            </div>
            <QuickOpenCard />
        </Flex>
    }
}

#[component]
fn QuickOpenCard() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let navigate = use_navigate();
    let open_detail = {
        let navigate = navigate.clone();
        move |_| {
            let name = query.get().trim().to_string();
            if validate_spectra_query_name(&name).is_ok() {
                let path = spectra_schema_path(&name);
                navigate(&path, NavigateOptions::default());
            }
        }
    };
    let open_explore = move |_| {
        let name = query.get().trim().to_string();
        if validate_spectra_query_name(&name).is_ok() {
            let path = spectra_schema_explore_path(&name);
            navigate(&path, NavigateOptions::default());
        }
    };
    view! {
        <div data-testid="spectra-home-quick-open">
            <Card>
            <CardHeader>
                <Title3>"Quick open"</Title3>
            </CardHeader>
            <CardContent>
                <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                    <div id="spectra-quick-open-search" data-testid="spectra-quick-open-search">
                        <SearchBox
                            bind=SearchBoxBind::from(query)
                            appearance=SearchBoxAppearance::with_placeholder("Search schema name…")
                        />
                    </div>
                    <Flex gap=SpacingSize::Size80.flex_gap()>
                        <span id="spectra-quick-open-detail" data-testid="spectra-quick-open-detail">
                            <Button appearance=ButtonAppearance::Secondary on:click=open_detail>
                                "Open detail"
                            </Button>
                        </span>
                        <span id="spectra-quick-open-explore" data-testid="spectra-quick-open-explore">
                            <Button appearance=ButtonAppearance::Primary on:click=open_explore>
                                "Open explore"
                            </Button>
                        </span>
                    </Flex>
                </Flex>
            </CardContent>
            </Card>
        </div>
    }
}
