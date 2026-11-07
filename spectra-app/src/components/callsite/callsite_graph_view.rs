use leptos::prelude::*;
use orbital::components::{Body1, Caption1, SpacingSize, Title3};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, Card, CardContent, CardHeader, Flex, FlexWrap, Link,
    MessageBar, MessageBarIntent,
};
use spectra_backend::{spectra_schema_explore_path, CallsiteGraph, EmitSite};

use super::layout::{layout_graph, GraphLayout, ROW_H};

const EDGE_VIEW_WIDTH: f64 = 100.0;

fn site_label(site: &EmitSite) -> String {
    format!("{}:{}", site.file, site.line)
}

fn last_seen_label(site: &EmitSite) -> String {
    site.last_seen.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Emit sites linked to the event tables they write, plus a full edge list.
#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn CallsiteGraphView(
    /// Graph payload from `get_callsite_graph`.
    graph: CallsiteGraph,
) -> impl IntoView {
    let layout = layout_graph(&graph);
    let notices = view! {
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
                        "{} of {} scanned rows have no callsite. They were written without a generated logger.",
                        graph.untracked, graph.scanned
                    )}
                </Caption1>
            </div>
        })}
    };
    view! {
        <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
            {notices}
            <GraphColumns layout=layout.clone() />
            <EdgeList layout=layout />
        </Flex>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)]
fn GraphColumns(
    /// Computed row geometry.
    layout: GraphLayout,
) -> impl IntoView {
    let height = layout.height;
    let row_style = format!("height: {ROW_H}px; min-width: 0;");
    let column_style =
        format!("flex: 1 1 0; min-width: 0; height: {height}px; position: relative;");
    let edges_style = format!("flex: 0 0 3rem; height: {height}px;");
    let paths = layout
        .edges
        .iter()
        .map(|e| {
            let mid = EDGE_VIEW_WIDTH / 2.0;
            format!(
                "M0,{y1} C{mid},{y1} {mid},{y2} {w},{y2}",
                y1 = e.site_y,
                y2 = e.table_y,
                w = EDGE_VIEW_WIDTH
            )
        })
        .collect::<Vec<_>>();
    let view_box = format!("0 0 {EDGE_VIEW_WIDTH} {height}");
    view! {
        <div id="spectra-callsite-graph" data-testid="callsite-graph" aria-label="Emit sites linked to the tables they write">
            <Flex>
                <div style=column_style.clone()>
                    {layout.sites.iter().map(|site| {
                        let test_id = format!("callsite-site-{}", site_label(site));
                        let label = site_label(site);
                        let title = label.clone();
                        let count = site.count.to_string();
                        let style = row_style.clone();
                        view! {
                            <div style=style data-testid=test_id title=title>
                                <Card>
                                    <CardContent>
                                        <Flex>
                                            <span style="flex: 1 1 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                                                <Caption1>{label}</Caption1>
                                            </span>
                                            <Badge appearance=BadgeAppearance::Tint color=BadgeColor::Brand>
                                                {count}
                                            </Badge>
                                        </Flex>
                                    </CardContent>
                                </Card>
                            </div>
                        }
                    }).collect_view()}
                </div>
                <svg
                    style=edges_style
                    viewBox=view_box
                    preserveAspectRatio="none"
                    aria-hidden="true"
                    data-testid="callsite-graph-edges"
                >
                    {paths.into_iter().map(|d| view! {
                        <path d=d fill="none" stroke="currentColor" stroke-opacity="0.45"
                            stroke-width="1.5" vector-effect="non-scaling-stroke" />
                    }).collect_view()}
                </svg>
                <div style=column_style>
                    {layout.blocks.iter().map(|block| {
                        let test_id = format!("callsite-table-{}", block.name);
                        let href = spectra_schema_explore_path(&block.name);
                        let block_style = format!(
                            "position: absolute; top: {top}px; height: {h}px; left: 0; right: 0;",
                            top = block_top(block.first_row),
                            h = block_height(block.rows),
                        );
                        let name = block.name.clone();
                        view! {
                            <div style=block_style data-testid=test_id>
                                <Card>
                                    <CardHeader>
                                        <Link href=href>{name}</Link>
                                    </CardHeader>
                                </Card>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </Flex>
        </div>
    }
}

#[allow(clippy::cast_precision_loss, reason = "row counts are far below 2^52")]
fn block_top(first_row: usize) -> f64 {
    first_row as f64 * ROW_H
}

#[allow(clippy::cast_precision_loss, reason = "row counts are far below 2^52")]
fn block_height(rows: usize) -> f64 {
    rows as f64 * ROW_H
}

#[component]
#[allow(clippy::needless_pass_by_value)]
fn EdgeList(
    /// Computed row geometry; only the ordered sites are used.
    layout: GraphLayout,
) -> impl IntoView {
    view! {
        <div data-testid="callsite-edge-list">
            <Card>
                <CardHeader>
                    <Title3>"All emit sites"</Title3>
                </CardHeader>
                <CardContent>
                    <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                        {layout.sites.into_iter().map(|site| {
                            let label = site_label(&site);
                            let test_id = format!("callsite-edge-{label}");
                            let href = spectra_schema_explore_path(&site.table);
                            let detail = format!(
                                "{} rows, last {}",
                                site.count,
                                last_seen_label(&site)
                            );
                            view! {
                                <div data-testid=test_id>
                                    <Flex vertical=true gap=SpacingSize::Size40.flex_gap() wrap=FlexWrap::Wrap>
                                        <Body1>{label}</Body1>
                                        <Caption1>"writes "<Link href=href>{site.table}</Link>" · "{detail}</Caption1>
                                    </Flex>
                                </div>
                            }
                        }).collect_view()}
                    </Flex>
                </CardContent>
            </Card>
        </div>
    }
}
