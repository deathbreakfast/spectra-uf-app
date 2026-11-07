use leptos::prelude::*;
use orbital::components::{Caption1, ContentContainer, EmptyState, SpacingSize, Title3};
use orbital::primitives::{Body1, Flex, MessageBar, MessageBarIntent};
use uf_product::services::report_server_fn_error;

use crate::components::callsite::CallsiteGraphView;
use crate::components::query::{PermissionDeniedState, TimeRangePicker};
use crate::server::{get_callsite_graph, server_fn_is_permission_denied};

const DEFAULT_WINDOW_SECS: i64 = 24 * 3600;

/// Callsite graph: which source lines emit rows into which event tables.
#[component]
pub fn CallsiteGraphPage() -> impl IntoView {
    let window_secs = RwSignal::new(DEFAULT_WINDOW_SECS);
    let graph_res = Resource::new(
        move || window_secs.get(),
        |secs| async move { get_callsite_graph(u32::try_from(secs / 3600).unwrap_or(24)).await },
    );

    view! {
        <ContentContainer data_testid="callsite-graph-page">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Title3>"Callsite graph"</Title3>
                <Caption1>"Source lines that wrote event rows, linked to the tables they write"</Caption1>
                <div id="spectra-callsite-window">
                    <TimeRangePicker
                        selected_secs=window_secs
                        on_change=Callback::new(move |secs| window_secs.set(secs))
                    />
                </div>
                <Suspense fallback=|| view! { <Body1>"Loading callsites…"</Body1> }>
                    {move || match graph_res.get() {
                        Some(Ok(graph)) if graph.sites.is_empty() => view! {
                            <div data-testid="callsite-graph-empty">
                                <EmptyState
                                    message="No callsites in this window"
                                    description="Rows logged through generated loggers record the file and line that wrote them. Metrics do not carry a callsite."
                                />
                            </div>
                        }.into_any(),
                        Some(Ok(graph)) => view! { <CallsiteGraphView graph=graph /> }.into_any(),
                        Some(Err(e)) if server_fn_is_permission_denied(&e) => {
                            view! { <PermissionDeniedState /> }.into_any()
                        }
                        Some(Err(e)) => {
                            if report_server_fn_error(&e) {
                                view! { <EmptyState message="Couldn't load callsites" /> }.into_any()
                            } else {
                                view! {
                                    <MessageBar intent=MessageBarIntent::Error>
                                        {format!("Failed to load callsites: {e}")}
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
