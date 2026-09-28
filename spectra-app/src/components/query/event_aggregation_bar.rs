use leptos::prelude::*;
use orbital::components::Caption1;
use orbital::primitives::{Flex, FlexGap, Input, Select};
use spectra_core::{EventAggregationSpec, EventExploreView, EventMeasure};

use super::event_view_picker::normalize_view;

#[component]
pub fn EventAggregationBar(
    /// Current view selection.
    #[prop(into)]
    view: Signal<EventExploreView>,
    /// Two-way signal holding the spec describing what to render.
    spec: RwSignal<EventAggregationSpec>,
    /// Schema / event field names for group-by and sum-field Selects.
    #[prop(into)]
    field_options: Signal<Vec<String>>,
) -> impl IntoView {
    let bucket = RwSignal::new(
        spec.get_untracked()
            .time_bucket_secs
            .unwrap_or(3600)
            .to_string(),
    );
    let group_by = RwSignal::new(spec.get_untracked().group_by_field.unwrap_or_default());
    let row_field = RwSignal::new(
        spec.get_untracked()
            .row_fields
            .first()
            .cloned()
            .unwrap_or_default(),
    );
    let pivot_field = RwSignal::new(spec.get_untracked().pivot_field.unwrap_or_default());
    let measure_str = RwSignal::new(if spec.get_untracked().measure == EventMeasure::Count {
        "count".to_string()
    } else {
        "sum".to_string()
    });
    let measure_field = RwSignal::new(
        spec.get_untracked()
            .measure_field
            .unwrap_or_else(|| "value".to_string()),
    );

    Effect::new(move |_| {
        let current = normalize_view(view.get());
        if let Ok(v) = bucket.get().parse::<u64>() {
            spec.update(|s| s.time_bucket_secs = Some(v));
        }
        let m = measure_str.get();
        let mf = measure_field.get();
        spec.update(|s| {
            if m == "sum" {
                s.measure = EventMeasure::Sum;
                s.measure_field = if mf.trim().is_empty() {
                    Some("value".into())
                } else {
                    Some(mf)
                };
            } else {
                s.measure = EventMeasure::Count;
                s.measure_field = None;
            }
        });

        // Host SQLite aggregate infers Slices whenever group_by is set — clear it for
        // Time series / Event log so leftover Bar/Pie group-by cannot empty the series panel.
        let slice_view = matches!(
            current,
            EventExploreView::PieChart | EventExploreView::BarChart
        );
        if slice_view {
            let gb = group_by.get();
            spec.update(|s| {
                s.group_by_field = if gb.is_empty() { None } else { Some(gb) };
            });
        } else if !group_by.get_untracked().is_empty() {
            group_by.set(String::new());
            spec.update(|s| s.group_by_field = None);
        } else {
            spec.update(|s| s.group_by_field = None);
        }

        // Same reset-when-leaving-view convention as group_by_field above, for Table's
        // row_fields / pivot_field.
        let table_view = current == EventExploreView::Table;
        if table_view {
            let rf = row_field.get();
            let pf = pivot_field.get();
            spec.update(|s| {
                s.row_fields = if rf.is_empty() { Vec::new() } else { vec![rf] };
                s.pivot_field = if pf.is_empty() { None } else { Some(pf) };
            });
        } else if !row_field.get_untracked().is_empty() || !pivot_field.get_untracked().is_empty() {
            row_field.set(String::new());
            pivot_field.set(String::new());
            spec.update(|s| {
                s.row_fields = Vec::new();
                s.pivot_field = None;
            });
        } else {
            spec.update(|s| {
                s.row_fields = Vec::new();
                s.pivot_field = None;
            });
        }
    });

    view! {
        {move || {
            if normalize_view(view.get()) == EventExploreView::EventLog {
                return view! {
                    <>
                        <div id="spectra-aggregation-measure" data-testid="spectra-aggregation-measure"></div>
                        <div id="spectra-aggregation-bucket" data-testid="spectra-aggregation-bucket"></div>
                        <div id="spectra-aggregation-group-by" data-testid="spectra-aggregation-group-by"></div>
                    </>
                }
                .into_any();
            }
            let current = normalize_view(view.get());
            let is_sum = measure_str.get() == "sum";
            let mut fields = field_options.get();
            // Payload keys used by e2e / ops seeds may outlive schema metadata.
            for extra in ["severity", "value", "id"] {
                if !fields.iter().any(|f| f == extra) {
                    fields.push((*extra).to_string());
                }
            }
            view! {
                <Flex vertical=true gap=FlexGap::Small>
                    <Flex vertical=true>
                        <Caption1>"Measure"</Caption1>
                        <div id="spectra-aggregation-measure" data-testid="spectra-aggregation-measure">
                            <Select bind=measure_str>
                                <option value="count">"Count"</option>
                                <option value="sum">"Sum"</option>
                            </Select>
                        </div>
                    </Flex>
                    {is_sum.then(|| {
                        let fields = fields.clone();
                        view! {
                            <Flex vertical=true>
                                <Caption1>"Sum field"</Caption1>
                                <div data-testid="spectra-aggregation-measure-field">
                                    <Select bind=measure_field>
                                        {fields.iter().map(|f| {
                                            let opt = f.clone();
                                            view! { <option value=opt.clone()>{opt.clone()}</option> }
                                        }).collect_view()}
                                    </Select>
                                </div>
                            </Flex>
                        }
                    })}
                    {matches!(current, EventExploreView::TimeSeries).then(|| view! {
                        <Flex vertical=true>
                            <Caption1>"Time bucket (seconds)"</Caption1>
                            <div id="spectra-aggregation-bucket" data-testid="spectra-aggregation-bucket">
                                <Input bind=bucket />
                            </div>
                        </Flex>
                    })}
                    {matches!(current, EventExploreView::PieChart | EventExploreView::BarChart).then(|| {
                        let fields = fields.clone();
                        view! {
                            <Flex vertical=true>
                                <Caption1>"Group by field"</Caption1>
                                <div id="spectra-aggregation-group-by" data-testid="spectra-aggregation-group-by">
                                    <Select bind=group_by>
                                        <option value="">"(select field)"</option>
                                        {fields.iter().map(|f| {
                                            let opt = f.clone();
                                            view! { <option value=opt.clone()>{opt.clone()}</option> }
                                        }).collect_view()}
                                    </Select>
                                </div>
                            </Flex>
                        }
                    })}
                    {matches!(current, EventExploreView::Table).then(|| {
                        let row_fields_options = fields.clone();
                        let pivot_field_options = fields.clone();
                        view! {
                            <>
                                <Flex vertical=true>
                                    <Caption1>"Rows"</Caption1>
                                    <div id="spectra-aggregation-rows" data-testid="spectra-aggregation-rows">
                                        <Select bind=row_field>
                                            <option value="">"(select field)"</option>
                                            {row_fields_options.iter().map(|f| {
                                                let opt = f.clone();
                                                view! { <option value=opt.clone()>{opt.clone()}</option> }
                                            }).collect_view()}
                                        </Select>
                                    </div>
                                </Flex>
                                <Flex vertical=true>
                                    <Caption1>"Pivot field (optional)"</Caption1>
                                    <div id="spectra-aggregation-pivot-field" data-testid="spectra-aggregation-pivot-field">
                                        <Select bind=pivot_field>
                                            <option value="">"(none)"</option>
                                            {pivot_field_options.iter().map(|f| {
                                                let opt = f.clone();
                                                view! { <option value=opt.clone()>{opt.clone()}</option> }
                                            }).collect_view()}
                                        </Select>
                                    </div>
                                </Flex>
                            </>
                        }
                    })}
                </Flex>
            }
            .into_any()
        }}
    }
}
