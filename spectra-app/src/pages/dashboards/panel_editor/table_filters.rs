//! The panel's own filters, and the columns, sort and row limit of a table panel.

use leptos::prelude::*;
use orbital::components::{FormHint, SpacingSize, Tag, TagGroup};
use orbital::primitives::{
    Button, ButtonAppearance, Field, Flex, FlexAlign, FlexWrap, Input, InputBind, Menu, MenuItem,
    MenuTrigger, Select, SelectBind,
};
use spectra_backend::dashboards::view::{
    filter_op_key, filter_op_symbol, parse_filter, FILTER_OPS,
};
use spectra_backend::dashboards::{DashboardFilter, FilterOp};

use super::super::toolbar::filter_label;
use super::settings::{sort_key, Settings, ROW_CHOICES};

/// The panel's own filters as Tags, with Add filter.
#[component]
pub(super) fn PanelFilters(
    filters: RwSignal<Vec<DashboardFilter>>,
    field_names: Signal<Vec<String>>,
) -> impl IntoView {
    let adding = RwSignal::new(false);
    let remove = move |index: String| {
        if let Ok(i) = index.parse::<usize>() {
            filters.update(|f| {
                if i < f.len() {
                    f.remove(i);
                }
            });
        }
    };
    let on_add = Callback::new(move |filter: DashboardFilter| {
        filters.update(|f| f.push(filter));
        adding.set(false);
    });
    view! {
        <Field label="Filters">
            <div data-testid="spectra-panel-editor-filters">
                <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                    <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                        <TagGroup dismissible=true on_dismiss=remove>
                            {move || filters.get().into_iter().enumerate().map(|(i, f)| {
                                let label = filter_label(&f);
                                view! {
                                    <Tag value=i.to_string() dismiss_label=format!("Remove filter {label}")>
                                        {label.clone()}
                                    </Tag>
                                }
                            }).collect_view()}
                        </TagGroup>
                        <span data-testid="spectra-panel-editor-add-filter">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                icon=icondata::AiPlusOutlined
                                on_click=Callback::new(move |_| adding.update(|a| *a = !*a))
                            >
                                "Add filter"
                            </Button>
                        </span>
                    </Flex>
                    <Show when=move || adding.get()>
                        <FilterAddRow field_names=field_names on_add=on_add />
                    </Show>
                </Flex>
            </div>
        </Field>
        <FormHint>"The dashboard's own filters and $host apply on top of these."</FormHint>
    }
}

/// Field, condition and value for a new filter. The field is a Select of the source's
/// fields, or free text when the schema isn't known.
#[component]
fn FilterAddRow(
    field_names: Signal<Vec<String>>,
    on_add: Callback<DashboardFilter>,
) -> impl IntoView {
    let field = RwSignal::new(String::new());
    let op = RwSignal::new(filter_op_key(FilterOp::Eq).to_string());
    let value = RwSignal::new(String::new());
    let parsed = Memo::new(move |_| {
        parse_filter(&format!(
            "{}:{}:{}",
            field.get().trim(),
            op.get(),
            value.get()
        ))
    });
    let add = Callback::new(move |_| {
        if let Some(filter) = parsed.get_untracked() {
            on_add.run(filter);
        }
    });
    let field_control = move || {
        let names = field_names.get();
        if names.is_empty() {
            return view! { <Input bind=InputBind::new(field) /> }.into_any();
        }
        view! {
            <Select bind=SelectBind::new(field)>
                <option value="">"Pick a field"</option>
                {names.into_iter().map(|n| view! { <option value=n>{n.clone()}</option> }).collect_view()}
            </Select>
        }
        .into_any()
    };
    view! {
        <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::End wrap=FlexWrap::Wrap>
            <Field label="Field">
                <div data-testid="spectra-panel-editor-filter-field">{field_control}</div>
            </Field>
            <Field label="Condition">
                <Select bind=SelectBind::new(op)>
                    {FILTER_OPS.iter().map(|o| view! {
                        <option value=filter_op_key(*o)>{filter_op_symbol(*o)}</option>
                    }).collect_view()}
                </Select>
            </Field>
            <Field label="Value">
                <div data-testid="spectra-panel-editor-filter-value">
                    <Input bind=InputBind::new(value) />
                </div>
            </Field>
            <span data-testid="spectra-panel-editor-filter-submit">
                <Button disabled=Signal::derive(move || parsed.get().is_none()) on_click=add>
                    "Add"
                </Button>
            </span>
        </Flex>
    }
}

/// Columns, sort and row limit for a table panel.
#[component]
pub(super) fn TableSettings(settings: Settings, field_names: Signal<Vec<String>>) -> impl IntoView {
    let all_fields = move || {
        let mut names = vec!["ts".to_string()];
        names.extend(field_names.get().into_iter().filter(|n| n != "ts"));
        names
    };
    view! {
        <Field label="Columns">
            <div data-testid="spectra-panel-editor-columns">
                <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                    <TagGroup
                        dismissible=true
                        on_dismiss=move |column: String| {
                            settings.columns.update(|c| c.retain(|x| *x != column));
                        }
                    >
                        {move || settings.columns.get().into_iter().map(|c| view! {
                            <Tag value=c.clone() dismiss_label=format!("Remove column {c}")>{c.clone()}</Tag>
                        }).collect_view()}
                    </TagGroup>
                    <Menu on_select=move |column: String| {
                        settings.columns.update(|c| {
                            if !c.contains(&column) {
                                c.push(column);
                            }
                        });
                    }>
                        <MenuTrigger slot>
                            <Button appearance=ButtonAppearance::Subtle icon=icondata::AiPlusOutlined>
                                "Add column"
                            </Button>
                        </MenuTrigger>
                        {move || all_fields().into_iter().map(|f| view! {
                            <MenuItem value=f.clone()>{f.clone()}</MenuItem>
                        }).collect_view()}
                    </Menu>
                </Flex>
            </div>
        </Field>
        <Field label="Sort">
            <div data-testid="spectra-panel-editor-sort">
                {move || {
                    let mut options = vec![(String::new(), "Newest first".to_string())];
                    for f in all_fields() {
                        options.push((sort_key(&f, true), format!("{f}, highest first")));
                        options.push((sort_key(&f, false), format!("{f}, lowest first")));
                    }
                    let current = settings.sort.get_untracked();
                    if !current.is_empty() && !options.iter().any(|(k, _)| *k == current) {
                        options.push((current.clone(), current));
                    }
                    view! {
                        <Select bind=SelectBind::new(settings.sort)>
                            {options.into_iter().map(|(value, label)| view! {
                                <option value=value>{label}</option>
                            }).collect_view()}
                        </Select>
                    }
                }}
            </div>
        </Field>
        <Field label="Rows">
            <div data-testid="spectra-panel-editor-rows">
                {move || {
                    let current = settings.rows.get_untracked();
                    let mut choices: Vec<String> = ROW_CHOICES.iter().map(u32::to_string).collect();
                    if !choices.contains(&current) {
                        choices.push(current);
                    }
                    view! {
                        <Select bind=SelectBind::new(settings.rows)>
                            {choices.into_iter().map(|n| view! {
                                <option value=n>{n.clone()}</option>
                            }).collect_view()}
                        </Select>
                    }
                }}
            </div>
        </Field>
    }
}
