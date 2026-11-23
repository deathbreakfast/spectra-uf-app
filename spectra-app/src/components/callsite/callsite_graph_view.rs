use std::collections::HashSet;

use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};
use orbital::components::{Body1, Caption1, SpacingSize, Title3};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, Button, ButtonAppearance, ButtonGroup, ButtonSize, Card,
    CardContent, CardHeader, Flex, FlexWrap, Link, MessageBar, MessageBarActions, MessageBarBody,
    MessageBarIntent, MessageBarLayout, MessageBarTitle,
};
use orbital_graph::{
    Graph, GraphEdge, GraphFill, GraphInput, GraphLegend, GraphLegendItem, GraphLegendLine,
    GraphNode, GraphNodeView,
};
use spectra_backend::{
    spectra_metric_explore_path, spectra_schema_explore_path, CallsiteGraph, EmitSite, SiteKind,
};
use uf_product::paths::PERMISSION_PERMISSIONS;

/// Legend set of event-table sites and nodes.
pub const TABLES_SET: &str = "tables";
/// Legend set of metric sites and nodes.
pub const METRICS_SET: &str = "metrics";

/// Smallest and largest graph heights, in px.
const MIN_GRAPH_PX: usize = 320;
const MAX_GRAPH_PX: usize = 640;
/// Row pitch of Orbital Graph's default layout, in px.
const ROW_PX: usize = 88;

const fn set_of(kind: SiteKind) -> &'static str {
    match kind {
        SiteKind::Table => TABLES_SET,
        SiteKind::Metric => METRICS_SET,
    }
}

fn site_label(site: &EmitSite) -> String {
    format!("{}:{}", site.file, site.line)
}

/// `abandon.rs:42`: the file name without its directories.
fn short_site_label(site: &EmitSite) -> String {
    let name = site.file.rsplit(['/', '\\']).next().unwrap_or(&site.file);
    format!("{name}:{}", site.line)
}

fn count_label(site: &EmitSite) -> String {
    let unit = match (site.kind, site.count) {
        (SiteKind::Table, 1) => "row",
        (SiteKind::Table, _) => "rows",
        (SiteKind::Metric, 1) => "point",
        (SiteKind::Metric, _) => "points",
    };
    format!("{} {unit}", site.count)
}

fn last_seen_label(site: &EmitSite) -> String {
    site.last_seen.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Graph node id of an emit site; one node per line and kind.
fn site_node_id(site: &EmitSite) -> String {
    let kind = match site.kind {
        SiteKind::Table => "table",
        SiteKind::Metric => "metric",
    };
    format!("site:{kind}:{}", site_label(site))
}

/// Graph node id of a table or metric.
fn target_node_id(kind: SiteKind, name: &str) -> String {
    match kind {
        SiteKind::Table => format!("table:{name}"),
        SiteKind::Metric => format!("metric:{name}"),
    }
}

fn target_href(kind: SiteKind, name: &str) -> String {
    match kind {
        SiteKind::Table => spectra_schema_explore_path(name),
        SiteKind::Metric => spectra_metric_explore_path(name),
    }
}

/// Graph input and node views: sites on the left, tables and metrics on the right.
fn graph_model(graph: &CallsiteGraph) -> (GraphInput, Vec<GraphNodeView>) {
    let mut input = GraphInput::default();
    let mut views = Vec::new();
    let mut seen_sites = HashSet::new();
    for site in &graph.sites {
        let id = site_node_id(site);
        if seen_sites.insert(id.clone()) {
            let count = graph
                .sites
                .iter()
                .filter(|s| site_node_id(s) == id)
                .fold(0u64, |sum, s| sum.saturating_add(s.count));
            let shown = EmitSite {
                count,
                ..site.clone()
            };
            let mut lines = vec![short_site_label(site)];
            lines.extend(site.module_in_crate().map(str::to_owned));
            lines.push(count_label(&shown));
            let aria = format!(
                "{}, {}{}",
                site_label(site),
                site.module
                    .as_deref()
                    .map_or_else(String::new, |m| format!("{m}, ")),
                count_label(&shown)
            );
            input.nodes.push(GraphNode {
                id: id.clone(),
                ..GraphNode::default()
            });
            views.push(GraphNodeView {
                id: id.clone(),
                fill: GraphFill::Neutral,
                lines,
                aria_label: Some(aria),
                set: Some(set_of(site.kind).to_owned()),
            });
        }
        input.edges.push(GraphEdge {
            from: id,
            to: target_node_id(site.kind, &site.target),
            ..GraphEdge::default()
        });
    }
    let targets = graph
        .tables
        .iter()
        .map(|t| (SiteKind::Table, t))
        .chain(graph.metrics.iter().map(|m| (SiteKind::Metric, m)));
    for (kind, name) in targets {
        let id = target_node_id(kind, name);
        input.nodes.push(GraphNode {
            id: id.clone(),
            ..GraphNode::default()
        });
        let (fill, caption) = match kind {
            SiteKind::Table => (GraphFill::Yellow, "Event table"),
            SiteKind::Metric => (GraphFill::Brand, "Metric"),
        };
        views.push(GraphNodeView {
            id,
            fill,
            lines: vec![name.clone(), caption.to_owned()],
            aria_label: None,
            set: Some(set_of(kind).to_owned()),
        });
    }
    (input, views)
}

fn graph_height(graph: &CallsiteGraph) -> String {
    let site_nodes = graph
        .sites
        .iter()
        .map(site_node_id)
        .collect::<HashSet<_>>()
        .len();
    let rows = site_nodes.max(graph.tables.len() + graph.metrics.len());
    let px = (rows * ROW_PX + ROW_PX).clamp(MIN_GRAPH_PX, MAX_GRAPH_PX);
    format!("{px}px")
}

/// Which writers the graph shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShowChoice {
    Tables,
    Metrics,
    Both,
}

impl ShowChoice {
    const ALL: [Self; 3] = [Self::Tables, Self::Metrics, Self::Both];

    const fn label(self) -> &'static str {
        match self {
            Self::Tables => "Tables",
            Self::Metrics => "Metrics",
            Self::Both => "Both",
        }
    }

    const fn test_id(self) -> &'static str {
        match self {
            Self::Tables => "callsite-show-tables",
            Self::Metrics => "callsite-show-metrics",
            Self::Both => "callsite-show-both",
        }
    }

    /// Legend sets hidden by this choice.
    fn hidden(self) -> HashSet<String> {
        match self {
            Self::Tables => HashSet::from([METRICS_SET.to_owned()]),
            Self::Metrics => HashSet::from([TABLES_SET.to_owned()]),
            Self::Both => HashSet::new(),
        }
    }
}

/// Show: Tables, Metrics or Both. Writes the legend sets the graph hides, so it never
/// refetches the graph.
#[component]
pub fn CallsiteShowToggle(
    /// Legend sets hidden in the graph, shared with [`CallsiteGraphView`].
    hidden_sets: RwSignal<HashSet<String>>,
) -> impl IntoView {
    view! {
        <div id="spectra-callsite-show" data-testid="callsite-show">
            <Flex gap=SpacingSize::Size80.flex_gap()>
                <Caption1>"Show"</Caption1>
                <ButtonGroup>
                    {ShowChoice::ALL.into_iter().map(|choice| {
                        let pressed = move || hidden_sets.with(|h| *h == choice.hidden());
                        view! {
                            <span data-testid=choice.test_id()>
                                <Button
                                    size=ButtonSize::Small
                                    appearance=Signal::derive(move || {
                                        if pressed() { ButtonAppearance::Secondary } else { ButtonAppearance::Subtle }
                                    })
                                    aria_pressed=Signal::derive(move || Some(pressed().to_string()))
                                    on_click=Callback::new(move |_| hidden_sets.set(choice.hidden()))
                                >
                                    {choice.label()}
                                </Button>
                            </span>
                        }
                    }).collect_view()}
                </ButtonGroup>
            </Flex>
        </div>
    }
}

/// Request access: opens the permission request page.
#[component]
pub fn RequestAccessButton(
    /// Button style; primary in an empty state, default in a message bar.
    #[prop(optional)]
    primary: bool,
) -> impl IntoView {
    let navigate = use_navigate();
    let appearance = if primary {
        ButtonAppearance::Primary
    } else {
        ButtonAppearance::Secondary
    };
    view! {
        <span data-testid="callsite-request-access">
            <Button
                appearance=appearance
                on_click=Callback::new(move |_| {
                    navigate(PERMISSION_PERMISSIONS, NavigateOptions::default());
                })
            >
                "Request access"
            </Button>
        </span>
    }
}

fn hidden_tables_body(tables: &[String]) -> String {
    match tables {
        [table] => format!(
            "You can't read {table}, so its lines aren't shown. Request spectra.query.{table} to see them."
        ),
        _ => format!(
            "You can't read {}, so their lines aren't shown. Request spectra.query for each to see them.",
            tables.join(", ")
        ),
    }
}

/// Emit sites linked to the event tables they write and the metrics they record, plus a
/// full edge list.
#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn CallsiteGraphView(
    /// Graph payload from `get_callsite_graph`.
    graph: CallsiteGraph,
    /// Legend sets hidden in the graph and the edge list, shared with [`CallsiteShowToggle`].
    hidden_sets: RwSignal<HashSet<String>>,
) -> impl IntoView {
    let (input, views) = graph_model(&graph);
    let height = graph_height(&graph);
    let legend_items = vec![
        GraphLegendItem {
            set: TABLES_SET.to_owned(),
            label: format!("Event tables ({})", graph.tables.len()),
            fill: GraphFill::Yellow,
            count: u32::try_from(graph.tables.len()).ok(),
        },
        GraphLegendItem {
            set: METRICS_SET.to_owned(),
            label: format!("Metrics ({})", graph.metrics.len()),
            fill: GraphFill::Brand,
            count: u32::try_from(graph.metrics.len()).ok(),
        },
    ];
    let hidden_count = graph.hidden_tables.len();
    let notices = view! {
        {(!graph.hidden_tables.is_empty()).then(|| view! {
            <div data-testid="callsite-graph-hidden">
                <MessageBar intent=MessageBarIntent::Info layout=MessageBarLayout::Multiline>
                    <MessageBarBody>
                        <MessageBarTitle>
                            {if hidden_count == 1 { "1 table hidden".to_owned() } else { format!("{hidden_count} tables hidden") }}
                        </MessageBarTitle>
                        {hidden_tables_body(&graph.hidden_tables)}
                    </MessageBarBody>
                    <MessageBarActions>
                        <RequestAccessButton />
                    </MessageBarActions>
                </MessageBar>
            </div>
        })}
        {graph.truncated.then(|| view! {
            <div data-testid="callsite-graph-truncated">
                <MessageBar intent=MessageBarIntent::Info>
                    "Showing the newest 1000 rows per table. Busier tables may have more emit sites."
                </MessageBar>
            </div>
        })}
        {(graph.untracked > 0).then(|| view! {
            <div data-testid="callsite-graph-untracked">
                <Caption1>
                    {format!(
                        "{} of {} scanned rows have no callsite, so they aren't drawn.",
                        graph.untracked, graph.scanned
                    )}
                </Caption1>
            </div>
        })}
    };
    view! {
        <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
            {notices}
            <div id="spectra-callsite-graph">
                <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                    <GraphLegend
                        items=legend_items
                        hidden_sets=hidden_sets
                        lines=vec![GraphLegendLine::new("writes or records").with_symbol("─▶")]
                        data_testid="callsite-graph-legend"
                    />
                    <Graph
                        input=input
                        nodes=views
                        hidden_sets=hidden_sets
                        label="Lines of code linked to the tables they write and the metrics they record"
                        height=height
                        data_testid="callsite-graph"
                    />
                </Flex>
            </div>
            <EdgeList sites=graph.sites hidden_sets=hidden_sets />
        </Flex>
    }
}

/// One "All emit sites" entry: the line, its kind, module, and what it writes.
fn edge_row(site: &EmitSite) -> impl IntoView {
    let label = site_label(site);
    let test_id = format!("callsite-edge-{label}");
    let href = target_href(site.kind, &site.target);
    let (verb, kind_label) = match site.kind {
        SiteKind::Table => ("writes ", "Event"),
        SiteKind::Metric => ("records ", "Metric"),
    };
    let detail = format!(" · {}, last {}", count_label(site), last_seen_label(site));
    let module = site.module.clone();
    let target = site.target.clone();
    view! {
        <div data-testid=test_id data-kind=set_of(site.kind)>
            <Flex vertical=true gap=SpacingSize::Size40.flex_gap() wrap=FlexWrap::Wrap>
                <Flex gap=SpacingSize::Size80.flex_gap() wrap=FlexWrap::Wrap>
                    <Body1>{label}</Body1>
                    <Badge appearance=BadgeAppearance::Tint color=BadgeColor::Informative>
                        {kind_label}
                    </Badge>
                </Flex>
                {module.map(|m| view! { <Caption1>{m}</Caption1> })}
                <Caption1>
                    {verb}
                    <Link href=href>{target}</Link>
                    {detail}
                </Caption1>
            </Flex>
        </div>
    }
}

#[component]
fn EdgeList(
    /// Every emit site, in graph order.
    sites: Vec<EmitSite>,
    /// Legend sets to leave out.
    hidden_sets: RwSignal<HashSet<String>>,
) -> impl IntoView {
    let rows = sites
        .into_iter()
        .map(|site| {
            let set = set_of(site.kind);
            let visible = move || !hidden_sets.with(|h| h.contains(set));
            view! { <Show when=visible>{edge_row(&site)}</Show> }
        })
        .collect_view();
    view! {
        <div data-testid="callsite-edge-list">
            <Card>
                <CardHeader>
                    <Title3>"All emit sites"</Title3>
                </CardHeader>
                <CardContent>
                    <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                        {rows}
                    </Flex>
                </CardContent>
            </Card>
        </div>
    }
}
