//! The toolbar above a dashboard's sections: range, variables, filters and actions.
//!
//! Every change goes through `on_change` as a new [`ViewState`], which the page writes to the
//! URL. Panels refetch from the URL; the page itself never remounts.

use leptos::prelude::*;
use orbital::components::{SpacingSize, Tag, TagGroup};
use orbital::primitives::{
    Button, ButtonAppearance, Card, CardContent, DateTimeRange, DateTimeRangePicker,
    DatetimeTimezone, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle,
    Field, Flex, FlexAlign, FlexWrap, Input, InputBind, Menu, MenuItem, MenuTrigger,
    OrbitalDateTime, Select, SelectBind, Tooltip,
};
use spectra_backend::dashboards::view::{
    filter_op_key, filter_op_symbol, parse_filter, RangeChoice, ViewState, FILTER_OPS,
};
use spectra_backend::dashboards::{Dashboard, DashboardAccess, DashboardFilter, FilterOp};

use crate::explore_time::{default_explore_datetime_range, range_from_datetime_range};

const CUSTOM: &str = "custom";

/// The text on a filter tag, such as `status = 500`.
pub(super) fn filter_label(f: &DashboardFilter) -> String {
    format!("{} {} {}", f.field, filter_op_symbol(f.op), f.value)
}

fn picker_range(state: &ViewState) -> DateTimeRange {
    match state.range {
        RangeChoice::Custom { from, to } => DateTimeRange::new(
            OrbitalDateTime::from_instant(from, DatetimeTimezone::Local),
            OrbitalDateTime::from_instant(to, DatetimeTimezone::Local),
        ),
        _ => default_explore_datetime_range(),
    }
}

/// The toolbar Card.
#[component]
pub(super) fn DashboardToolbar(
    /// The dashboard shown.
    dashboard: Dashboard,
    /// What the viewer may do with it.
    access: DashboardAccess,
    /// Toolbar state from the URL.
    state: Memo<ViewState>,
    /// Called with the next state; the page writes it to the URL.
    on_change: Callback<ViewState>,
    /// Open the Duplicate dialog.
    on_duplicate: Callback<()>,
    /// Open the Share dialog.
    on_share: Callback<()>,
    /// Open the version history.
    on_history: Callback<()>,
    /// Open the Delete dialog.
    on_delete: Callback<()>,
    /// True while an older version is shown, which can't be edited, shared or deleted.
    #[prop(optional)]
    old_version: bool,
) -> impl IntoView {
    let saved_filters = dashboard.filters.clone();
    let update = move |f: &dyn Fn(&mut ViewState)| {
        let mut next = state.get_untracked();
        f(&mut next);
        if next != state.get_untracked() {
            on_change.run(next);
        }
    };

    // Range: a preset applies at once; Custom range shows the picker.
    let range_key = RwSignal::new(state.get_untracked().range.key().to_string());
    let custom = RwSignal::new(Some(picker_range(&state.get_untracked())));
    Effect::new(move |_| {
        let key = range_key.get();
        if let Some(preset) = RangeChoice::preset(&key) {
            update(&|s| s.range = preset);
        }
    });
    Effect::new(move |_| {
        if range_key.get() != CUSTOM {
            return;
        }
        if let Some(range) = custom.get() {
            let (from, to) = range_from_datetime_range(&range);
            if from < to {
                update(&|s| s.range = RangeChoice::Custom { from, to });
            }
        }
    });

    let filters = Memo::new({
        let saved = saved_filters;
        move |_| state.with(|s| s.filters_or(&saved).to_vec())
    });
    let remove_filter = Callback::new(move |index: String| {
        let Ok(index) = index.parse::<usize>() else {
            return;
        };
        let mut next = filters.get_untracked();
        if index < next.len() {
            next.remove(index);
            update(&|s| s.filters = Some(next.clone()));
        }
    });
    let add_open = RwSignal::new(false);
    let add_filter = Callback::new(move |filter: DashboardFilter| {
        let mut next = filters.get_untracked();
        next.push(filter);
        update(&|s| s.filters = Some(next.clone()));
    });

    let variables = dashboard.variables;
    let builtin = dashboard.read_only;
    let can_edit = access.can_edit && !old_version;
    let can_share = !builtin && !old_version;
    let can_delete = access.can_delete && !old_version;

    view! {
        <div data-testid="spectra-dashboard-toolbar">
            <Card>
                <CardContent>
                    <Flex gap=SpacingSize::Size120.flex_gap() align=FlexAlign::End wrap=FlexWrap::Wrap>
                        <Field label="Time range">
                            <div data-testid="spectra-dashboard-range">
                                <Select bind=SelectBind::new(range_key)>
                                    {RangeChoice::PRESETS.iter().map(|p| view! {
                                        <option value=p.key()>{p.label()}</option>
                                    }).collect_view()}
                                    <option value=CUSTOM>"Custom range"</option>
                                </Select>
                            </div>
                        </Field>
                        <Show when=move || range_key.get() == CUSTOM>
                            <div data-testid="spectra-dashboard-custom-range">
                                <DateTimeRangePicker bind=custom />
                            </div>
                        </Show>
                        {variables.into_iter().map(|var| {
                            let name = var.name.clone();
                            let current = state.with_untracked(|s| s.variables.get(&name).cloned())
                                .or_else(|| var.default.clone())
                                .unwrap_or_default();
                            let selected = RwSignal::new(current.clone());
                            let (effect_name, default) = (name.clone(), var.default.clone());
                            Effect::new(move |_| {
                                let value = selected.get();
                                let name = effect_name.clone();
                                let in_effect = state
                                    .with_untracked(|s| s.variables.get(&name).cloned())
                                    .or_else(|| default.clone())
                                    .unwrap_or_default();
                                if value != in_effect {
                                    update(&|s| {
                                        s.variables.insert(name.clone(), value.clone());
                                    });
                                }
                            });
                            let mut options = vec![(String::new(), "All".to_string())];
                            for value in [var.default, Some(current)].into_iter().flatten() {
                                if !value.is_empty() && !options.iter().any(|(v, _)| *v == value) {
                                    options.push((value.clone(), value));
                                }
                            }
                            let test_id = format!("spectra-dashboard-variable-{name}");
                            view! {
                                <Field label=format!("${name}")>
                                    <div data-testid=test_id>
                                        <Select bind=SelectBind::new(selected)>
                                            {options.into_iter().map(|(value, label)| view! {
                                                <option value=value>{label}</option>
                                            }).collect_view()}
                                        </Select>
                                    </div>
                                </Field>
                            }
                        }).collect_view()}
                        <div data-testid="spectra-dashboard-filters">
                            <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                                <TagGroup dismissible=true on_dismiss=move |index: String| remove_filter.run(index)>
                                    {move || filters.get().into_iter().enumerate().map(|(i, f)| {
                                        let label = filter_label(&f);
                                        view! {
                                            <Tag value=i.to_string() dismiss_label=format!("Remove filter {label}")>
                                                {label.clone()}
                                            </Tag>
                                        }
                                    }).collect_view()}
                                </TagGroup>
                                <span data-testid="spectra-dashboard-add-filter">
                                    <Button
                                        appearance=ButtonAppearance::Subtle
                                        icon=icondata::AiPlusOutlined
                                        on_click=Callback::new(move |_| add_open.set(true))
                                    >
                                        "Add filter"
                                    </Button>
                                </span>
                            </Flex>
                        </div>
                        <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center>
                            {can_edit.then(|| view! {
                                <Tooltip content="Editing is coming soon".to_string()>
                                    <span data-testid="spectra-dashboard-edit">
                                        <Button disabled_focusable=true>"Edit"</Button>
                                    </span>
                                </Tooltip>
                            })}
                            {can_share.then(|| view! {
                                <span data-testid="spectra-dashboard-share">
                                    <Button
                                        icon=icondata::AiShareAltOutlined
                                        on_click=Callback::new(move |_| on_share.run(()))
                                    >
                                        "Share"
                                    </Button>
                                </span>
                            })}
                            {(!can_edit).then(|| view! {
                                <span data-testid="spectra-dashboard-duplicate">
                                    <Button on_click=Callback::new(move |_| on_duplicate.run(()))>
                                        "Duplicate"
                                    </Button>
                                </span>
                            })}
                            <Menu
                                data_testid="spectra-dashboard-more"
                                on_select=move |action: &str| match action {
                                    "duplicate" => on_duplicate.run(()),
                                    "history" => on_history.run(()),
                                    "delete" => on_delete.run(()),
                                    _ => {}
                                }
                            >
                                <MenuTrigger slot>
                                    <Button
                                        appearance=ButtonAppearance::Subtle
                                        icon=icondata::AiMoreOutlined
                                        attr:aria-label="More actions"
                                    />
                                </MenuTrigger>
                                <MenuItem value="duplicate">"Duplicate"</MenuItem>
                                {(!builtin).then(|| view! {
                                    <MenuItem value="history">"Version history"</MenuItem>
                                })}
                                {can_delete.then(|| view! {
                                    <MenuItem value="delete">"Delete"</MenuItem>
                                })}
                            </Menu>
                        </Flex>
                    </Flex>
                </CardContent>
            </Card>
            <AddFilterDialog open=add_open on_add=add_filter />
        </div>
    }
}

/// Field, condition and value for a new filter.
#[component]
fn AddFilterDialog(open: RwSignal<bool>, on_add: Callback<DashboardFilter>) -> impl IntoView {
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
    let on_submit = Callback::new(move |_| {
        if let Some(filter) = parsed.get_untracked() {
            on_add.run(filter);
            field.set(String::new());
            value.set(String::new());
            open.set(false);
        }
    });
    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-dashboard-add-filter-dialog">
                    <DialogBody>
                        <DialogTitle>"Add filter"</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Field label="Field" required=true>
                                    <div data-testid="spectra-add-filter-field">
                                        <Input bind=InputBind::new(field) />
                                    </div>
                                </Field>
                                <Field label="Condition">
                                    <div data-testid="spectra-add-filter-op">
                                        <Select bind=SelectBind::new(op)>
                                            {FILTER_OPS.iter().map(|o| view! {
                                                <option value=filter_op_key(*o)>{filter_op_symbol(*o)}</option>
                                            }).collect_view()}
                                        </Select>
                                    </div>
                                </Field>
                                <Field label="Value">
                                    <div data-testid="spectra-add-filter-value">
                                        <Input bind=InputBind::new(value) />
                                    </div>
                                </Field>
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button on_click=Callback::new(move |_| open.set(false))>"Cancel"</Button>
                            <span data-testid="spectra-add-filter-submit">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    disabled=Signal::derive(move || parsed.get().is_none())
                                    on_click=on_submit
                                >
                                    "Add"
                                </Button>
                            </span>
                        </DialogActions>
                    </DialogBody>
                </div>
            </DialogSurface>
        </Dialog>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_label_uses_the_symbol() {
        let f = DashboardFilter {
            field: "status".into(),
            op: FilterOp::Ne,
            value: "500".into(),
        };
        assert_eq!(filter_label(&f), "status ≠ 500");
    }
}
