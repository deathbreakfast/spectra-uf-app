use std::collections::HashSet;

use leptos::prelude::*;
use orbital::components::{
    Caption1, ContentContainer, EmptyState, EmptyStateCallToAction, SpacingSize, Title3,
};
use orbital::primitives::{
    Body1, Button, ButtonAppearance, Flex, FlexWrap, MessageBar, MessageBarIntent,
};
use spectra_backend::EmptyReason;
use uf_product::services::report_server_fn_error;

use crate::components::callsite::{CallsiteGraphView, CallsiteShowToggle, RequestAccessButton};
use crate::components::query::{PermissionDeniedState, TimeRangePicker};
use crate::server::{get_callsite_graph, server_fn_is_permission_denied};

const DEFAULT_WINDOW_SECS: i64 = 24 * 3600;
const WEEK_SECS: i64 = 7 * 24 * 3600;

/// "Nothing logged" headline for the window buttons' presets.
const fn nothing_logged_message(window_secs: i64) -> &'static str {
    match window_secs {
        s if s <= 3600 => "Nothing logged in the last hour",
        s if s <= 6 * 3600 => "Nothing logged in the last 6 hours",
        s if s <= DEFAULT_WINDOW_SECS => "Nothing logged in the last 24 hours",
        _ => "Nothing logged in the last 7 days",
    }
}

/// Callsite graph: which source lines write which event tables and record which metrics.
#[component]
pub fn CallsiteGraphPage() -> impl IntoView {
    let window_secs = RwSignal::new(DEFAULT_WINDOW_SECS);
    // Show and the legend chips only hide sets; they never refetch the graph.
    let hidden_sets = RwSignal::new(HashSet::<String>::new());
    let graph_res = Resource::new(
        move || window_secs.get(),
        |secs| async move { get_callsite_graph(u32::try_from(secs / 3600).unwrap_or(24)).await },
    );

    view! {
        <ContentContainer data_testid="callsite-graph-page">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Title3>"Callsite graph"</Title3>
                <Caption1>"Lines of code linked to the tables they write and the metrics they record"</Caption1>
                <Flex gap=SpacingSize::Size240.flex_gap() wrap=FlexWrap::Wrap>
                    <div id="spectra-callsite-window">
                        <TimeRangePicker
                            selected_secs=window_secs
                            on_change=Callback::new(move |secs| window_secs.set(secs))
                        />
                    </div>
                    <CallsiteShowToggle hidden_sets=hidden_sets />
                </Flex>
                <Suspense fallback=|| view! { <Body1>"Loading callsites…"</Body1> }>
                    {move || match graph_res.get() {
                        Some(Ok(graph)) => match graph.empty {
                            Some(EmptyReason::NoBackend) => view! {
                                <div data-testid="callsite-graph-empty" data-reason="no_backend">
                                    <EmptyState
                                        message="Callsites can't be read here"
                                        description="This host has no Spectra query backend, so there are no rows or points to read."
                                    />
                                </div>
                            }.into_any(),
                            Some(EmptyReason::NoGrants) => view! {
                                <div data-testid="callsite-graph-empty" data-reason="no_grants">
                                    <EmptyState
                                        message="You can't read any tables or metrics yet"
                                        description="Callsites only shows what you can query. Ask for spectra.query on the tables you need."
                                    >
                                        <EmptyStateCallToAction slot:call_to_action>
                                            <RequestAccessButton primary=true />
                                        </EmptyStateCallToAction>
                                    </EmptyState>
                                </div>
                            }.into_any(),
                            Some(EmptyReason::NothingLogged) => {
                                let secs = window_secs.get_untracked();
                                view! {
                                    <div data-testid="callsite-graph-empty" data-reason="nothing_logged">
                                        <EmptyState
                                            message=nothing_logged_message(secs)
                                            description="No row or metric point you can read has a callsite in this window."
                                        >
                                            <EmptyStateCallToAction slot:call_to_action>
                                                {(secs < WEEK_SECS).then(|| view! {
                                                    <span data-testid="callsite-graph-show-week">
                                                        <Button
                                                            appearance=ButtonAppearance::Primary
                                                            on_click=Callback::new(move |_| window_secs.set(WEEK_SECS))
                                                        >
                                                            "Show 7 days"
                                                        </Button>
                                                    </span>
                                                })}
                                            </EmptyStateCallToAction>
                                        </EmptyState>
                                    </div>
                                }.into_any()
                            }
                            None => view! {
                                <CallsiteGraphView graph=graph hidden_sets=hidden_sets window_secs=window_secs.get_untracked() />
                            }.into_any(),
                        },
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
