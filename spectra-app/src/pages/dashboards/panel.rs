//! One panel's Card: title, what it reads, the panel menu, and its body.

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use orbital::components::{Caption1, SpacingSize, Subtitle1};
use orbital::primitives::{
    Button, ButtonAppearance, Card, CardContent, CardHeader, CardHeaderAction,
    CardHeaderDescription, CopyButton, Flex, FlexAlign, Menu, MenuItem, MenuTrigger, MessageBar,
    MessageBarActions, MessageBarBody, MessageBarIntent,
};
use spectra_backend::dashboards::view::{explore_href, panel_caption, panel_source_name};
use spectra_backend::dashboards::{ChartKind, Panel, PanelSource};

use super::detail::PanelEnv;
use super::panels::{LockedPanel, PanelBody, PanelBodySkeleton, TextPanel};
use crate::server::{query_dashboard_panel, PanelData};

/// A panel and, unless it's text or locked, the resource that loads it.
#[derive(Clone)]
pub(super) struct PanelSlot {
    pub panel: Panel,
    data: Option<Resource<Result<PanelData, ServerFnError>>>,
    retry: RwSignal<u32>,
    locked: bool,
    gaps: Vec<String>,
    env: PanelEnv,
}

impl PanelSlot {
    /// A locked panel gets no resource, so it never asks for data.
    pub fn new(panel: Panel, env: &PanelEnv) -> Self {
        let locked = panel
            .required_query_permission()
            .is_some_and(|p| env.missing.contains(&p));
        let retry = RwSignal::new(0u32);
        let data = (!locked && panel_source_name(&panel).is_some()).then(|| {
            let (env, panel) = (env.clone(), panel.clone());
            Resource::new(
                move || (env.query(&panel), retry.get()),
                |(query, _)| query_dashboard_panel(query),
            )
        });
        Self {
            gaps: env
                .variable_gaps
                .get(&panel.id)
                .cloned()
                .unwrap_or_default(),
            panel,
            data,
            retry,
            locked,
            env: env.clone(),
        }
    }
}

/// The reason a panel failed, without the server-function framing.
fn failure_reason(err: &ServerFnError) -> String {
    match err {
        ServerFnError::ServerError(message) => message.clone(),
        other => other.to_string(),
    }
}

/// `"$host"`, `"$host and $region"`.
fn variable_list(names: &[String]) -> String {
    let names: Vec<String> = names.iter().map(|n| format!("${n}")).collect();
    super::detail::join_names(&names)
}

const fn chart_key(panel: &Panel) -> &'static str {
    match (&panel.source, panel.chart) {
        (PanelSource::Text { .. }, _) => "text",
        (_, ChartKind::TimeSeries) => "time_series",
        (_, ChartKind::Bar) => "bar",
        (_, ChartKind::Pie) => "pie",
        (_, ChartKind::Table) => "table",
        (_, ChartKind::SingleStat) => "single_stat",
    }
}

/// The panel Card.
#[component]
pub(super) fn DashboardPanel(entry: PanelSlot) -> impl IntoView {
    let PanelSlot {
        panel,
        data,
        retry,
        locked,
        gaps,
        env,
    } = entry;
    let as_table = RwSignal::new(false);
    let link_open = RwSignal::new(false);
    let location = use_location();
    let navigate = use_navigate();

    let explore = Memo::new({
        let panel = panel.clone();
        move |_| {
            let query = env.query(&panel);
            let window = panel.keep_window.unwrap_or(query.window);
            explore_href(&panel, window, &query.filters)
        }
    });
    let panel_id = panel.id.clone();
    let panel_link = Memo::new(move |_| {
        let search = location.search.get();
        let query = if search.is_empty() {
            String::new()
        } else {
            format!("?{}", search.trim_start_matches('?'))
        };
        let origin = window().location().origin().unwrap_or_default();
        format!(
            "{origin}{}{query}#panel-{panel_id}",
            location.pathname.get()
        )
    });

    let source = panel_source_name(&panel).map(str::to_string);
    let is_text = source.is_none();
    let test_id = format!("spectra-dashboard-panel-{}", panel.id);
    let anchor = format!("panel-{}", panel.id);
    let caption = panel_caption(&panel);
    let title = panel.title.clone();
    let gap_note = (!gaps.is_empty()).then(|| {
        format!(
            "{} not applied: {} has no field for it.",
            variable_list(&gaps),
            source.clone().unwrap_or_default()
        )
    });

    let body = {
        let panel = panel.clone();
        let source = source.unwrap_or_default();
        move || match (&panel.source, data) {
            (PanelSource::Text { markdown }, _) => {
                view! { <TextPanel markdown=markdown.clone() /> }.into_any()
            }
            _ if locked => view! { <LockedPanel source=source.clone() /> }.into_any(),
            (_, Some(data)) => {
                let panel = panel.clone();
                view! {
                    <Transition fallback=PanelBodySkeleton>
                        {move || data.get().map(|result| match result {
                            Ok(loaded) => view! {
                                <PanelBody
                                    panel=panel.clone()
                                    data=loaded
                                    as_table=as_table
                                    explore=explore
                                />
                            }
                            .into_any(),
                            Err(e) => view! {
                                <div data-testid="spectra-dashboard-panel-failed">
                                    <MessageBar intent=MessageBarIntent::Error>
                                        <MessageBarBody>
                                            {format!("Couldn't load this panel: {}", failure_reason(&e))}
                                        </MessageBarBody>
                                        <MessageBarActions>
                                            <Button on_click=Callback::new(move |_| retry.update(|n| *n += 1))>
                                                "Retry"
                                            </Button>
                                        </MessageBarActions>
                                    </MessageBar>
                                </div>
                            }
                            .into_any(),
                        })}
                    </Transition>
                }
                .into_any()
            }
            (_, None) => view! { <PanelBodySkeleton /> }.into_any(),
        }
    };

    let show_menu = !is_text && !locked;
    let navigate_explore = navigate.clone();
    view! {
        <div id=anchor data-testid=test_id data-chart=chart_key(&panel) data-locked=locked.to_string()>
            <Card>
                <CardHeader>
                    <span data-testid="spectra-dashboard-panel-title">
                        <Subtitle1>{title}</Subtitle1>
                    </span>
                    <CardHeaderDescription slot>
                        <Flex vertical=true gap=SpacingSize::Size20.flex_gap()>
                            <span data-testid="spectra-dashboard-panel-caption">
                                <Caption1>{caption}</Caption1>
                            </span>
                            {gap_note.map(|note| view! {
                                <span data-testid="spectra-dashboard-panel-variable-note">
                                    <Caption1>{note}</Caption1>
                                </span>
                            })}
                        </Flex>
                    </CardHeaderDescription>
                    <CardHeaderAction slot>
                        {show_menu.then(|| view! {
                            <Menu
                                data_testid="spectra-dashboard-panel-menu"
                                on_select=move |action: &str| match action {
                                    "explore" => {
                                        if let Some(href) = explore.get_untracked() {
                                            navigate_explore(&href, NavigateOptions::default());
                                        }
                                    }
                                    "table" => as_table.update(|t| *t = !*t),
                                    "link" => link_open.set(true),
                                    _ => {}
                                }
                            >
                                <MenuTrigger slot>
                                    <Button
                                        appearance=ButtonAppearance::Subtle
                                        icon=icondata::AiMoreOutlined
                                        attr:aria-label="Panel actions"
                                    />
                                </MenuTrigger>
                                <MenuItem value="explore">"Open in explore"</MenuItem>
                                <MenuItem value="table">
                                    {move || if as_table.get() { "View as chart" } else { "View as table" }}
                                </MenuItem>
                                <MenuItem value="link">"Copy link to panel"</MenuItem>
                            </Menu>
                        })}
                    </CardHeaderAction>
                </CardHeader>
                <CardContent>
                    <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                        <Show when=move || link_open.get()>
                            <div data-testid="spectra-dashboard-panel-link">
                                <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center>
                                    <Caption1>{move || panel_link.get()}</Caption1>
                                    <CopyButton
                                        text=Signal::derive(move || panel_link.get())
                                        label="Copy link".to_string()
                                    />
                                    <Button
                                        appearance=ButtonAppearance::Subtle
                                        on_click=Callback::new(move |_| link_open.set(false))
                                    >
                                        "Done"
                                    </Button>
                                </Flex>
                            </div>
                        </Show>
                        {body}
                    </Flex>
                </CardContent>
            </Card>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_list_prefixes_each_name() {
        assert_eq!(variable_list(&["host".into()]), "$host");
        assert_eq!(
            variable_list(&["host".into(), "region".into()]),
            "$host and $region"
        );
    }

    #[test]
    fn server_errors_lose_their_framing() {
        let err = ServerFnError::ServerError("query failed: timeout".into());
        assert_eq!(failure_reason(&err), "query failed: timeout");
    }
}
