//! `/spectra/dashboards/:id`: one dashboard with its toolbar, sections and panels.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_params_map, use_query_map};
use leptos_router::NavigateOptions;
use orbital::components::{
    Body1, Caption1, ContentContainer, EmptyState, EmptyStateCallToAction, Persona, PersonaConfig,
    SpacingSize, Title3,
};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, BadgeSize, Button, ButtonAppearance, Flex, FlexAlign,
    FlexWrap, Link, MessageBar, MessageBarActions, MessageBarBody, MessageBarIntent, Skeleton,
    SkeletonItem,
};
use spectra_backend::dashboards::view::ViewState;
use spectra_backend::dashboards::{Dashboard, DashboardError, DashboardFilter, Variable};
use spectra_backend::spectra_dashboard_path;

use super::section::DashboardSection;
use super::toolbar::DashboardToolbar;
use super::{action_error_text, copy_title, saved_date};
use crate::layout::DashboardCrumb;
use crate::server::{
    dashboard_error, duplicate_dashboard, get_dashboard_view, set_favourite, DashboardView,
    PanelQuery,
};

/// What every panel on the page shares: the toolbar state and who can see what.
#[derive(Clone)]
pub(super) struct PanelEnv {
    /// Dashboard id.
    pub dashboard_id: String,
    /// The dashboard's variables.
    pub variables: Arc<Vec<Variable>>,
    /// The dashboard's saved filters, used until the toolbar changes them.
    pub saved_filters: Arc<Vec<DashboardFilter>>,
    /// Toolbar state from the URL.
    pub state: Memo<ViewState>,
    /// `spectra.query.*` permissions the viewer lacks.
    pub missing: Arc<BTreeSet<String>>,
    /// Variables each panel can't apply, by panel id.
    pub variable_gaps: Arc<BTreeMap<String, Vec<String>>>,
}

impl PanelEnv {
    /// The query for `panel` under the current toolbar state. Reads the state, so a resource
    /// keyed on this refetches only when the range, variables or filters change.
    pub fn query(&self, panel: &spectra_backend::dashboards::Panel) -> PanelQuery {
        self.state.with(|state| PanelQuery {
            dashboard_id: self.dashboard_id.clone(),
            panel: panel.clone(),
            window: state.range.window(),
            filters: state.filters_or(&self.saved_filters).to_vec(),
            variables: self.variables.to_vec(),
            variable_values: state.variable_values(&self.variables),
        })
    }
}

/// "a", "a and b", "a, b and c".
pub(super) fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The locked-panels warning. Request access opens the form for the first permission
/// only, so with several missing it says which one the request covers.
pub(super) fn missing_access_text(missing: &[String]) -> String {
    let locked = format!(
        "Some panels are locked because you don't have {}.",
        join_names(missing)
    );
    match missing {
        [first, _, ..] => format!(
            "{locked} Request access asks for {first}; request the others the same way from Gauge."
        ),
        _ => locked,
    }
}

fn is_not_found(err: &ServerFnError) -> bool {
    matches!(dashboard_error(err), Some(DashboardError::NotFound))
}

/// The dashboard page.
#[component]
pub fn DashboardDetailPage() -> impl IntoView {
    let params = use_params_map();
    let id = Memo::new(move |_| params.with(|p| p.get("id").unwrap_or_default()));
    let refresh = RwSignal::new(0u32);
    // Keyed on the path id and Retry only, never the URL query, so changing the range or a
    // filter refetches panels without remounting the page.
    let view_res = Resource::new(
        move || (id.get(), refresh.get()),
        |(id, _)| get_dashboard_view(id),
    );

    if let Some(crumb) = use_context::<DashboardCrumb>() {
        Effect::new(move |_| {
            let label = match view_res.get() {
                Some(Ok(view)) => Some(view.dashboard.title),
                Some(Err(e)) if is_not_found(&e) => Some("Not found".to_string()),
                _ => None,
            };
            crumb.0.set(label);
        });
        on_cleanup(move || crumb.0.set(None));
    }

    view! {
        <ContentContainer data_testid="spectra-dashboard-page">
            <Transition fallback=HeaderSkeleton>
                {move || view_res.get().map(|result| match result {
                    Ok(view) => view! { <DashboardBody view=view /> }.into_any(),
                    Err(e) if is_not_found(&e) => view! { <DashboardNotFound /> }.into_any(),
                    Err(e) => view! {
                        <div data-testid="spectra-dashboard-error">
                            <MessageBar intent=MessageBarIntent::Error>
                                <MessageBarBody>{format!("Couldn't load this dashboard: {e}")}</MessageBarBody>
                                <MessageBarActions>
                                    <Button on_click=Callback::new(move |_| refresh.update(|n| *n += 1))>
                                        "Retry"
                                    </Button>
                                </MessageBarActions>
                            </MessageBar>
                        </div>
                    }
                    .into_any(),
                })}
            </Transition>
        </ContentContainer>
    }
}

#[component]
fn HeaderSkeleton() -> impl IntoView {
    view! {
        <div data-testid="spectra-dashboard-loading">
            <Skeleton>
                <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                    <SkeletonItem width="16rem".to_string() height="1.75rem".to_string() />
                    <SkeletonItem width="24rem".to_string() height="1rem".to_string() />
                </Flex>
            </Skeleton>
        </div>
    }
}

/// Deleted, mistyped and not shared all look the same.
#[component]
fn DashboardNotFound() -> impl IntoView {
    view! {
        <div data-testid="spectra-dashboard-not-found">
            <EmptyState
                message="Dashboard not found"
                description="It may have been deleted, the link may be wrong, or it hasn't been shared with you."
            >
                <EmptyStateCallToAction slot:call_to_action>
                    <span data-testid="spectra-dashboard-not-found-back">
                        <Link href="/spectra/dashboards">"Back to dashboards"</Link>
                    </span>
                </EmptyStateCallToAction>
            </EmptyState>
        </div>
    }
}

#[component]
fn DashboardBody(view: DashboardView) -> impl IntoView {
    let DashboardView {
        dashboard,
        access,
        favourite,
        missing_permissions,
        request_access_href,
        variable_gaps,
    } = view;

    let query = use_query_map();
    let state = Memo::new(move |_| {
        query.with(|q| ViewState::from_query(q.into_iter().map(|(k, v)| (k.as_ref(), v))))
    });
    let location = use_location();
    let navigate = use_navigate();
    let on_state = Callback::new(move |next: ViewState| {
        let path = location.pathname.get_untracked();
        let query = next.to_query();
        let href = if query.is_empty() {
            path
        } else {
            format!("{path}?{query}")
        };
        navigate(
            &href,
            NavigateOptions {
                replace: true,
                scroll: false,
                ..Default::default()
            },
        );
    });

    let id = dashboard.id.clone();
    let title = dashboard.title.clone();
    let navigate = use_navigate();
    let duplicating = RwSignal::new(false);
    let duplicate_error = RwSignal::new(None::<String>);
    let on_duplicate = Callback::new(move |()| {
        let (id, title, navigate) = (id.clone(), copy_title(&title), navigate.clone());
        duplicating.set(true);
        duplicate_error.set(None);
        leptos::task::spawn_local_scoped(async move {
            match duplicate_dashboard(id, title).await {
                Ok(copy) => navigate(
                    &spectra_dashboard_path(&copy.id),
                    NavigateOptions::default(),
                ),
                Err(e) => duplicate_error.set(Some(format!(
                    "Couldn't duplicate this dashboard: {}",
                    action_error_text(&e)
                ))),
            }
            duplicating.set(false);
        });
    });

    provide_context(PanelEnv {
        dashboard_id: dashboard.id.clone(),
        variables: Arc::new(dashboard.variables.clone()),
        saved_filters: Arc::new(dashboard.filters.clone()),
        state,
        missing: Arc::new(missing_permissions.iter().cloned().collect()),
        variable_gaps: Arc::new(variable_gaps),
    });

    let builtin = dashboard.read_only;
    let sections = dashboard.sections.clone();

    view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            <DashboardHeader dashboard=dashboard.clone() favourite=favourite />
            {move || duplicate_error.get().map(|message| view! {
                <div data-testid="spectra-dashboard-duplicate-error">
                    <MessageBar intent=MessageBarIntent::Error>{message}</MessageBar>
                </div>
            })}
            {builtin.then(|| view! {
                <div data-testid="spectra-dashboard-builtin-bar">
                    <MessageBar intent=MessageBarIntent::Info>
                        <MessageBarBody>
                            "Built-in dashboards can't be changed. Duplicate it to make your own copy."
                        </MessageBarBody>
                        <MessageBarActions>
                            <span data-testid="spectra-dashboard-builtin-duplicate">
                                <Button
                                    loading=duplicating
                                    on_click=Callback::new(move |_| on_duplicate.run(()))
                                >
                                    "Duplicate"
                                </Button>
                            </span>
                        </MessageBarActions>
                    </MessageBar>
                </div>
            })}
            {(!missing_permissions.is_empty()).then(|| view! {
                <div data-testid="spectra-dashboard-missing-access">
                    <MessageBar intent=MessageBarIntent::Warning>
                        <MessageBarBody>
                            {missing_access_text(&missing_permissions)}
                        </MessageBarBody>
                        <MessageBarActions>
                            <span data-testid="spectra-dashboard-request-access">
                                <Link href=request_access_href.clone().unwrap_or_default()>
                                    "Request access"
                                </Link>
                            </span>
                        </MessageBarActions>
                    </MessageBar>
                </div>
            })}
            <DashboardToolbar
                dashboard=dashboard.clone()
                access=access
                state=state
                on_change=on_state
                on_duplicate=on_duplicate
            />
            {sections.into_iter().map(|section| view! {
                <DashboardSection section=section />
            }).collect_view()}
        </Flex>
    }
}

#[component]
fn DashboardHeader(dashboard: Dashboard, favourite: bool) -> impl IntoView {
    let fav = RwSignal::new(favourite);
    let id = dashboard.id.clone();
    let title = dashboard.title.clone();
    let on_star = Callback::new(move |_| {
        let (id, next) = (id.clone(), !fav.get_untracked());
        leptos::task::spawn_local_scoped(async move {
            if set_favourite(id, next).await.is_ok() {
                fav.set(next);
            }
        });
    });
    let star_label = move || {
        if fav.get() {
            format!("Remove {title} from favourites")
        } else {
            format!("Add {title} to favourites")
        }
    };
    let owner = dashboard.owner.clone().map_or_else(
        || {
            view! {
                <Badge appearance=BadgeAppearance::Tint color=BadgeColor::Informative size=BadgeSize::Small>
                    "Built-in"
                </Badge>
            }
            .into_any()
        },
        |owner| view! { <Persona config=PersonaConfig::named(owner) /> }.into_any(),
    );
    let saved = format!(
        "Saved {} · version {}",
        saved_date(dashboard.updated_at),
        dashboard.version
    );
    view! {
        <div data-testid="spectra-dashboard-header">
            <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center>
                    <span data-testid="spectra-dashboard-title">
                        <Title3>{dashboard.title.clone()}</Title3>
                    </span>
                    <span data-testid="spectra-dashboard-favourite" data-on=move || fav.get().to_string()>
                        <Button
                            appearance=ButtonAppearance::Subtle
                            icon=Signal::derive(move || Some(if fav.get() {
                                icondata::AiStarFilled
                            } else {
                                icondata::AiStarOutlined
                            }))
                            attr:aria-label=star_label
                            attr:aria-pressed=move || fav.get().to_string()
                            on_click=on_star
                        />
                    </span>
                </Flex>
                {(!dashboard.description.is_empty()).then(|| view! {
                    <Body1>{dashboard.description.clone()}</Body1>
                })}
                <Flex gap=SpacingSize::Size120.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                    {owner}
                    <Caption1>{saved}</Caption1>
                </Flex>
            </Flex>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_access_names_what_the_request_covers() {
        let one = vec!["spectra.query.a".to_string()];
        assert_eq!(
            missing_access_text(&one),
            "Some panels are locked because you don't have spectra.query.a."
        );
        let two = vec!["spectra.query.a".to_string(), "spectra.query.b".to_string()];
        assert!(missing_access_text(&two).ends_with(
            "Request access asks for spectra.query.a; request the others the same way from Gauge."
        ));
    }

    #[test]
    fn join_names_reads_as_a_sentence() {
        let names = |n: &[&str]| n.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(join_names(&names(&[])), "");
        assert_eq!(join_names(&names(&["a"])), "a");
        assert_eq!(join_names(&names(&["a", "b"])), "a and b");
        assert_eq!(join_names(&names(&["a", "b", "c"])), "a, b and c");
    }
}
