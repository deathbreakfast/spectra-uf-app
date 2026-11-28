//! The live preview: the unsaved panel run on the dashboard's range, filters and variables.

use leptos::prelude::*;
use orbital::components::{Caption1Strong, EmptyState, SpacingSize};
use orbital::primitives::{Card, CardContent, Flex, MessageBar, MessageBarBody, MessageBarIntent};
use spectra_backend::dashboards::view::{RangeChoice, ViewState};
use spectra_backend::dashboards::{Dashboard, DashboardFilter, PanelSource};

use super::super::panel::failure_reason;
use super::super::panels::{PanelBody, PanelBodySkeleton};
use super::super::toolbar::filter_label;
use super::link::{blank_panel, window_text};
use super::settings::Settings;
use crate::server::{query_dashboard_panel, PanelData, PanelQuery};

/// The preview's heading: "Preview · last hour, region = eu-west, $host: all".
pub(super) fn preview_heading(
    state: &ViewState,
    saved: &[DashboardFilter],
    dashboard: &Dashboard,
) -> String {
    let mut parts = vec![match state.range {
        RangeChoice::Custom { .. } => window_text(state.range.window()),
        preset => preset.label().to_lowercase(),
    }];
    parts.extend(state.filters_or(saved).iter().map(filter_label));
    let values = state.variable_values(&dashboard.variables);
    parts.extend(dashboard.variables.iter().map(|v| {
        values.get(&v.name).filter(|x| !x.is_empty()).map_or_else(
            || format!("${}: all", v.name),
            |value| format!("${} = {value}", v.name),
        )
    }));
    format!("Preview · {}", parts.join(", "))
}

/// "No rows in the last hour", or "No rows in this range" for a custom range.
pub(super) const fn no_rows_text(range: RangeChoice) -> &'static str {
    match range {
        RangeChoice::LastHour => "No rows in the last hour",
        RangeChoice::Last6Hours => "No rows in the last 6 hours",
        RangeChoice::Last24Hours => "No rows in the last 24 hours",
        RangeChoice::Last7Days => "No rows in the last 7 days",
        RangeChoice::Custom { .. } => "No rows in this range",
    }
}

/// Whether loaded panel data has nothing to draw.
pub(super) fn is_empty_data(data: &PanelData) -> bool {
    match data {
        PanelData::Series { series, .. } => series.iter().all(|s| s.points.is_empty()),
        PanelData::Slices { slices, .. } => slices.is_empty(),
        PanelData::Stat { headline } => headline.is_empty(),
        PanelData::Table { rows, .. } => rows.is_empty(),
    }
}

/// The query the preview runs, or `None` before there's a source or while `blocked`. The
/// title is left out, so typing one never refetches.
pub(super) fn preview_query(
    settings: Settings,
    blocked: Signal<bool>,
    dashboard: StoredValue<Dashboard>,
    state: Memo<ViewState>,
) -> Memo<Option<PanelQuery>> {
    Memo::new(move |_| {
        if blocked.get() {
            return None;
        }
        let layout = blank_panel(PanelSource::Text {
            markdown: String::new(),
        })
        .layout;
        let mut panel = settings.build("preview", layout)?;
        panel.title = String::new();
        dashboard.with_value(|d| {
            state.with(|s| {
                Some(PanelQuery {
                    dashboard_id: d.id.clone(),
                    panel,
                    window: s.range.window(),
                    filters: s.filters_or(&d.filters).to_vec(),
                    variables: d.variables.clone(),
                    variable_values: s.variable_values(&d.variables),
                })
            })
        })
    })
}

/// The preview pane: a heading naming the range, filters and variables, over a Card with
/// the chart, or the blank, empty or failed state.
#[component]
pub(super) fn PanelPreview(
    query: Memo<Option<PanelQuery>>,
    dashboard: StoredValue<Dashboard>,
    state: Memo<ViewState>,
) -> impl IntoView {
    let preview = LocalResource::new(move || {
        let query = query.get();
        async move {
            match query {
                Some(query) => Some(query_dashboard_panel(query).await),
                None => None,
            }
        }
    });
    let heading =
        move || dashboard.with_value(|d| state.with(|s| preview_heading(s, &d.filters, d)));
    let body = move || {
        let range = state.with(|s| s.range);
        match preview.get() {
            None => view! { <PanelBodySkeleton /> }.into_any(),
            Some(None) => view! {
                <div data-testid="spectra-panel-editor-preview-blank">
                    <EmptyState
                        message="Nothing to preview yet"
                        description="Paste a link or pick a source to see the panel here."
                    />
                </div>
            }
            .into_any(),
            Some(Some(Ok(data))) if is_empty_data(&data) => view! {
                <div data-testid="spectra-panel-editor-preview-empty">
                    <EmptyState message=no_rows_text(range) />
                </div>
            }
            .into_any(),
            Some(Some(Ok(data))) => query
                .get_untracked()
                .map(|q| {
                    let chart = format!("{:?}", q.panel.chart);
                    view! {
                        <div data-testid="spectra-panel-editor-preview-body" data-chart=chart>
                            <PanelBody
                                panel=q.panel
                                data=data
                                as_table=RwSignal::new(false)
                                explore=Memo::new(|_| None)
                            />
                        </div>
                    }
                })
                .into_any(),
            Some(Some(Err(err))) => view! {
                <div data-testid="spectra-panel-editor-preview-failed">
                    <MessageBar intent=MessageBarIntent::Error>
                        <MessageBarBody>
                            {format!(
                                "The preview couldn't run: {}. You can still save the panel.",
                                failure_reason(&err).trim_end_matches('.')
                            )}
                        </MessageBarBody>
                    </MessageBar>
                </div>
            }
            .into_any(),
        }
    };
    view! {
        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
            <span data-testid="spectra-panel-editor-preview-heading">
                <Caption1Strong>{heading}</Caption1Strong>
            </span>
            <div data-testid="spectra-panel-editor-preview">
                <Card>
                    <CardContent>{body}</CardContent>
                </Card>
            </div>
        </Flex>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_results_and_wording() {
        assert!(is_empty_data(&PanelData::Slices {
            slices: Vec::new(),
            stand_in_field: None
        }));
        assert_eq!(
            no_rows_text(RangeChoice::LastHour),
            "No rows in the last hour"
        );
    }
}
