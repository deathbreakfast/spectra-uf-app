//! "Or set it up yourself": title, source, chart, measure, bucket and group by, with the
//! filters and table options below them.

use leptos::prelude::*;
use orbital::components::{
    Caption1, Combobox, ComboboxBind, ComboboxOption, FormHint, SpacingSize, Tab, TabList,
    ToggleButton,
};
use orbital::primitives::{Field, Flex, FlexWrap, Input, InputBind, Select, SelectBind};
use spectra_backend::dashboards::{ChartKind, Panel, PanelSource};

use super::link::{blank_panel, measure_label, CHARTS, METRIC_MEASURES};
use super::settings::{Settings, BUCKETS, TAB_EVENT, TAB_METRIC};
use super::table_filters::{PanelFilters, TableSettings};
use crate::server::{PanelSourceOption, SourceField};

/// Readable sources for the picker, or why they couldn't be loaded.
pub(super) type SourceList = LocalResource<Result<Vec<PanelSourceOption>, String>>;

/// `(value, label)` options with `current` added when the list doesn't have it, so a value
/// from a link or a saved panel stays visible.
fn with_current(
    mut options: Vec<(String, String)>,
    current: String,
    label: String,
) -> Vec<(String, String)> {
    if !current.is_empty() && !options.iter().any(|(k, _)| *k == current) {
        options.push((current, label));
    }
    options
}

fn select_options(options: Vec<(String, String)>) -> impl IntoView {
    options
        .into_iter()
        .map(|(value, label)| view! { <option value=value>{label}</option> })
        .collect_view()
}

#[component]
pub(super) fn SourceSettings(
    settings: Settings,
    sources: SourceList,
    fields: RwSignal<Option<Vec<SourceField>>>,
) -> impl IntoView {
    let field_names = Signal::derive(move || {
        fields.with(|f| {
            f.as_ref().map_or_else(Default::default, |f| {
                f.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
            })
        })
    });
    view! {
        <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
            <Field label="Title">
                <div data-testid="spectra-panel-editor-title">
                    <Input bind=InputBind::new(settings.title) />
                </div>
            </Field>
            <SourcePicker settings=settings sources=sources />
            <ChartPicker settings=settings />
            <MeasureSelect settings=settings fields=fields />
            <Show when=move || settings.chart.get() != ChartKind::Table>
                <Field label="Bucket">
                    <div data-testid="spectra-panel-editor-bucket">
                        <Select bind=SelectBind::new(settings.bucket)>
                            <option value="">"Pick from the range"</option>
                            {BUCKETS.iter().map(|(value, label)| view! {
                                <option value=*value>{*label}</option>
                            }).collect_view()}
                        </Select>
                    </div>
                </Field>
            </Show>
            <GroupBySelect settings=settings fields=fields />
            <PanelFilters filters=settings.filters field_names=field_names />
            <Show when=move || settings.chart.get() == ChartKind::Table>
                <TableSettings settings=settings field_names=field_names />
            </Show>
        </Flex>
    }
}

/// Metric or Event table, then a Combobox of the readable sources of that kind.
#[component]
fn SourcePicker(settings: Settings, sources: SourceList) -> impl IntoView {
    let label = Signal::derive(move || {
        if settings.is_metric() {
            "Metric"
        } else {
            "Event table"
        }
        .to_string()
    });
    let combobox = move || {
        let metric = settings.is_metric();
        let options: Vec<String> = sources
            .get()
            .and_then(Result::ok)
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.metric == metric)
            .map(|s| s.name)
            .collect();
        view! {
            <Combobox bind=ComboboxBind::new(settings.source_text, settings.source_selected)>
                {options.into_iter().map(|name| view! { <ComboboxOption text=name /> }).collect_view()}
            </Combobox>
        }
    };
    let load_error = move || sources.get().and_then(Result::err);
    view! {
        <Field label="Source">
            <div data-testid="spectra-panel-editor-source-kind">
                <TabList selected_value=settings.tab>
                    <Tab value=TAB_METRIC>"Metric"</Tab>
                    <Tab value=TAB_EVENT>"Event table"</Tab>
                </TabList>
            </div>
        </Field>
        <Field label=label>
            <div data-testid="spectra-panel-editor-source">{combobox}</div>
        </Field>
        {move || load_error().map_or_else(
            || view! {
                <FormHint>
                    {move || if settings.is_metric() {
                        "Metrics you can't read aren't listed."
                    } else {
                        "Tables you can't read aren't listed."
                    }}
                </FormHint>
            }
            .into_any(),
            |reason| view! {
                <span data-testid="spectra-panel-editor-sources-failed">
                    <Caption1>{format!("Couldn't load the sources you can read: {reason}")}</Caption1>
                </span>
            }
            .into_any(),
        )}
    }
}

/// Chart toggles. Metrics offer Time series and Single stat only.
#[component]
fn ChartPicker(settings: Settings) -> impl IntoView {
    let buttons = CHARTS
        .iter()
        .map(|(kind, label)| {
            let kind = *kind;
            let pressed = RwSignal::new(settings.chart.get_untracked() == kind);
            // A toggle un-presses itself on click; keep exactly the chosen chart pressed.
            Effect::new(move |_| {
                let on = settings.chart.get() == kind;
                if pressed.get_untracked() != on {
                    pressed.set(on);
                }
            });
            let shown = move || {
                kind == ChartKind::TimeSeries
                    || kind == ChartKind::SingleStat
                    || !settings.is_metric()
            };
            let test_id = format!(
                "spectra-panel-editor-chart-{}",
                label.to_lowercase().replace(' ', "-")
            );
            view! {
                <Show when=shown>
                    <span data-testid=test_id.clone()>
                        <ToggleButton
                            pressed=pressed
                            on_click=move |_: bool| {
                                pressed.set(true);
                                settings.chart.set(kind);
                            }
                        >
                            {*label}
                        </ToggleButton>
                    </span>
                </Show>
            }
        })
        .collect_view();
    view! {
        <Field label="Chart">
            <Flex gap=SpacingSize::Size80.flex_gap() wrap=FlexWrap::Wrap>{buttons}</Flex>
        </Field>
    }
}

/// Count or Sum of a numeric field; a metric's aggregation.
#[component]
fn MeasureSelect(settings: Settings, fields: RwSignal<Option<Vec<SourceField>>>) -> impl IntoView {
    let select = move || {
        let options: Vec<(String, String)> = if settings.is_metric() {
            std::iter::once((String::new(), "Default".to_string()))
                .chain(
                    METRIC_MEASURES
                        .iter()
                        .map(|(k, l)| ((*k).to_string(), (*l).to_string())),
                )
                .collect()
        } else {
            let numeric: Vec<String> = fields.with(|f| {
                f.iter()
                    .flatten()
                    .filter(|x| x.numeric)
                    .map(|x| x.name.clone())
                    .collect()
            });
            std::iter::once((String::new(), "Count".to_string()))
                .chain(
                    numeric
                        .into_iter()
                        .map(|n| (format!("sum({n})"), format!("Sum of {n}"))),
                )
                .collect()
        };
        let current = settings.measure.get_untracked();
        let label = measure_label(&Panel {
            measure: Some(current.clone()),
            ..blank_panel(PanelSource::Text {
                markdown: String::new(),
            })
        });
        view! {
            <Select bind=SelectBind::new(settings.measure)>
                {select_options(with_current(options, current, label))}
            </Select>
        }
    };
    view! {
        <Field label="Measure">
            <div data-testid="spectra-panel-editor-measure">{select}</div>
        </Field>
    }
}

/// Group by, from the source's own fields (G18), personal ones marked masked.
#[component]
fn GroupBySelect(settings: Settings, fields: RwSignal<Option<Vec<SourceField>>>) -> impl IntoView {
    let shown = move || {
        matches!(settings.chart.get(), ChartKind::Bar | ChartKind::Pie) || settings.is_metric()
    };
    let select = move || {
        let mut options = vec![(String::new(), "(none)".to_string())];
        fields.with(|f| {
            for x in f.iter().flatten() {
                let label = if x.pii {
                    format!("{} (masked)", x.name)
                } else {
                    x.name.clone()
                };
                options.push((x.name.clone(), label));
            }
        });
        let current = settings.group.get_untracked();
        view! {
            <Select bind=SelectBind::new(settings.group)>
                {select_options(with_current(options, current.clone(), current))}
            </Select>
        }
    };
    view! {
        <Show when=shown>
            <Field label="Group by">
                <div data-testid="spectra-panel-editor-group-by">{select}</div>
            </Field>
            <Caption1>
                "Fields marked masked are personal data. Charts show stand-in labels for them, never the stored values."
            </Caption1>
        </Show>
    }
}
