//! `/spectra/dashboards/:id/edit`: edit a saved dashboard.
//!
//! The page keeps the saved dashboard and a draft. Changes go to the draft only; Save sends it
//! with the version the edit started from, and when someone saved first nothing is
//! overwritten. Owners and editors only: anyone else, and every built-in, gets the not-found
//! page.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use leptos::ev;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};
use leptos_router::NavigateOptions;
use orbital::components::{
    Caption1, ContentContainer, DialogSize, EmptyState, List, ListItem, SpacingSize, Subtitle2,
    Title3,
};
use orbital::primitives::{
    use_breakpoint_up, Body1, Breakpoint, Button, ButtonAppearance, Dialog, DialogActions,
    DialogBody, DialogContent, DialogSurface, DialogTitle, Field, Flex, FlexAlign, FlexJustify,
    FlexWrap, Input, InputBind, MessageBar, MessageBarActions, MessageBarBody, MessageBarIntent,
    MessageBarLayout, MessageBarTitle, Skeleton, SkeletonItem, Textarea, TextareaBind,
};
use spectra_backend::dashboards::view::{missing_variable_fields, panel_source_name};
use spectra_backend::dashboards::{
    ChartKind, Dashboard, DashboardError, Layout, Panel, PanelSource, Section, Version,
};
use spectra_backend::spectra_dashboard_path;
use wasm_bindgen::{closure::Closure, JsCast};

use super::detail::{
    is_not_found, join_names, use_view_state, DashboardNotFound, HeaderSkeleton, PanelEnv,
};
use super::edit_bar::EditBar;
use super::edit_grid::{next_free_row, EditGrid, EditSectionHeader, PhoneSectionList};
use super::section::DashboardSection;
use super::toolbar::DashboardToolbar;
use super::variables::{capitalise, count_word, VariablesDialog};
use super::{action_error_text, copy_title, dashboard_edit_path};
use crate::layout::DashboardCrumb;
use crate::server::{
    dashboard_error, duplicate_dashboard, get_dashboard, get_dashboard_view, list_versions,
    save_dashboard, DashboardView,
};

/// How the panel editor opens: from one of Add panel's choices, or on an existing panel.
///
/// The panel editor itself comes later. Until then only [`PanelEditorStart::Text`] and
/// [`PanelEditorStart::Edit`] on a text panel do anything: they open a Markdown dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PanelEditorStart {
    /// Build a panel from scratch.
    Build,
    /// Start from a pasted Spectra query link.
    FromLink,
    /// A Markdown text panel.
    Text,
    /// Edit the panel with this id.
    Edit(String),
}

/// Who Preview as viewer shows the dashboard to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PreviewAs {
    /// A viewer with every query permission the panels need.
    EveryGrant,
    /// A viewer without this query permission.
    Without(String),
}

/// Whether `next` differs from `base` in anything a save would store.
pub(super) fn has_changes(base: &Dashboard, next: &Dashboard) -> bool {
    base.title != next.title
        || base.description != next.description
        || base.sections != next.sections
        || base.variables != next.variables
        || base.filters != next.filters
}

fn counted(n: usize, singular: &str, plural: &str) -> String {
    format!(
        "{} {}",
        count_word(n),
        if n == 1 { singular } else { plural }
    )
}

/// What changed from `base` to `next`, one short clause each, such as "moved two panels".
pub(super) fn change_parts(base: &Dashboard, next: &Dashboard) -> Vec<String> {
    let index = |d: &Dashboard| -> BTreeMap<String, (String, Panel)> {
        d.sections
            .iter()
            .flat_map(|s| {
                s.panels
                    .iter()
                    .map(|p| (p.id.clone(), (s.id.clone(), p.clone())))
            })
            .collect()
    };
    let (before, after) = (index(base), index(next));
    let (mut moved, mut resized, mut edited, mut removed) = (0, 0, 0, 0);
    for (id, (section, panel)) in &before {
        let Some((next_section, next_panel)) = after.get(id) else {
            removed += 1;
            continue;
        };
        let (a, b) = (panel.layout, next_panel.layout);
        let moved_within = a.col != b.col || a.row != b.row;
        if section != next_section || moved_within {
            moved += 1;
        }
        if (a.span, a.height) != (b.span, b.height) {
            resized += 1;
        }
        let same_content = Panel {
            layout: b,
            ..panel.clone()
        } == *next_panel;
        if !same_content {
            edited += 1;
        }
    }
    let added = after.keys().filter(|id| !before.contains_key(*id)).count();

    let section_ids = |d: &Dashboard| d.sections.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let (old_ids, new_ids) = (section_ids(base), section_ids(next));
    let sections_added = new_ids.iter().filter(|id| !old_ids.contains(id)).count();
    let sections_removed = old_ids.iter().filter(|id| !new_ids.contains(id)).count();
    let renamed = base
        .sections
        .iter()
        .filter(|s| {
            next.sections
                .iter()
                .any(|n| n.id == s.id && n.title != s.title)
        })
        .count();
    let kept_old: Vec<_> = old_ids.iter().filter(|id| new_ids.contains(id)).collect();
    let kept_new: Vec<_> = new_ids.iter().filter(|id| old_ids.contains(id)).collect();

    let mut parts = Vec::new();
    let mut push = |n: usize, verb: &str, singular: &str, plural: &str| {
        if n > 0 {
            parts.push(format!("{verb} {}", counted(n, singular, plural)));
        }
    };
    push(moved, "moved", "panel", "panels");
    push(resized, "resized", "panel", "panels");
    push(edited, "edited", "panel", "panels");
    push(added, "added", "panel", "panels");
    push(removed, "removed", "panel", "panels");
    push(sections_added, "added", "section", "sections");
    push(sections_removed, "removed", "section", "sections");
    push(renamed, "renamed", "section", "sections");
    if kept_old != kept_new {
        parts.push("reordered the sections".to_string());
    }
    if base.variables != next.variables {
        parts.push("changed the variables".to_string());
    }
    if base.filters != next.filters {
        parts.push("changed the saved filters".to_string());
    }
    if base.title != next.title || base.description != next.description {
        parts.push("changed the title or description".to_string());
    }
    parts
}

/// One sentence naming what changed, as the Discard dialog says it.
pub(super) fn change_summary(base: &Dashboard, next: &Dashboard) -> String {
    let parts = change_parts(base, next);
    if parts.is_empty() {
        "You haven't changed anything.".to_string()
    } else {
        format!("You {}.", join_names(&parts))
    }
}

/// The conflict bar's message: who saved which version when, and that nothing was lost.
pub(super) fn conflict_text(saved: Option<&Version>, current: u32, editing: u32) -> String {
    let who = saved.map_or_else(
        || format!("Someone saved version {current}"),
        |v| {
            let author = v.author.as_deref().unwrap_or("Someone");
            let via = v
                .via
                .as_ref()
                .map_or_else(String::new, |via| format!(" through {via}"));
            format!(
                "{author} saved version {current}{via} at {} UTC",
                v.created_at.format("%H:%M")
            )
        },
    );
    format!("{who} while you were editing version {editing}. Your changes are still here.")
}

/// `"{prefix}-{n}"` for the smallest `n` not already taken.
pub(super) fn next_id<'a>(prefix: &str, taken: impl IntoIterator<Item = &'a str>) -> String {
    let taken: BTreeSet<&str> = taken.into_iter().collect();
    // One more candidate than there are taken ids, so one of them is free.
    (1..=taken.len() + 1)
        .map(|n| format!("{prefix}-{n}"))
        .find(|id| !taken.contains(id.as_str()))
        .unwrap_or_else(|| prefix.to_string())
}

fn panel_ids(d: &Dashboard) -> Vec<String> {
    d.sections
        .iter()
        .flat_map(|s| s.panels.iter().map(|p| p.id.clone()))
        .collect()
}

/// Add an empty section at the end and return its id.
pub(super) fn add_section(d: &mut Dashboard, title: &str) -> String {
    let id = next_id("section", d.sections.iter().map(|s| s.id.as_str()));
    d.sections.push(Section {
        id: id.clone(),
        title: title.to_string(),
        collapsed: false,
        panels: Vec::new(),
    });
    id
}

/// Add a text panel below everything in the last section (adding a section if there is
/// none) and return its id.
pub(super) fn add_text_panel(d: &mut Dashboard, title: String, markdown: String) -> String {
    let ids = panel_ids(d);
    let id = next_id("panel", ids.iter().map(String::as_str));
    if d.sections.is_empty() {
        add_section(d, "");
    }
    if let Some(section) = d.sections.last_mut() {
        let row = next_free_row(&section.panels);
        section.panels.push(Panel {
            id: id.clone(),
            title,
            source: PanelSource::Text { markdown },
            chart: ChartKind::Table,
            measure: None,
            bucket: None,
            group_by: Vec::new(),
            filters: Vec::new(),
            table: None,
            layout: Layout {
                col: 0,
                span: 6,
                row,
                height: 2,
            },
            keep_window: None,
        });
    }
    id
}

/// Query permissions the dashboard's panels need, sorted.
fn needed_grants(d: &Dashboard) -> Vec<String> {
    d.sections
        .iter()
        .flat_map(|s| &s.panels)
        .filter_map(Panel::required_query_permission)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Variables each panel can't apply, by panel id, from the known source fields.
fn variable_gaps(
    d: &Dashboard,
    fields: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    d.sections
        .iter()
        .flat_map(|s| &s.panels)
        .filter_map(|panel| {
            let source = panel_source_name(panel)?;
            let gaps =
                missing_variable_fields(panel, &d.variables, fields.get(source).map(Vec::as_slice));
            (!gaps.is_empty()).then(|| (panel.id.clone(), gaps))
        })
        .collect()
}

/// While `dirty`, ask before the tab closes or reloads, and hold in-app link clicks in
/// `pending` so the page can ask first. Leptos's router follows links from a click listener
/// on the window, so a listener on the document runs first and can stop it. Back and
/// forward can't be held this way.
fn guard_leaving(dirty: Memo<bool>, pending: RwSignal<Option<String>>) {
    Effect::new(move |_| {
        let unload = window_event_listener(ev::beforeunload, move |e| {
            if dirty.try_get_untracked().unwrap_or(false) {
                e.prevent_default();
                e.set_return_value("");
            }
        });
        let on_click =
            Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                if !dirty.try_get_untracked().unwrap_or(false)
                    || e.default_prevented()
                    || e.button() != 0
                    || e.meta_key()
                    || e.ctrl_key()
                    || e.shift_key()
                    || e.alt_key()
                {
                    return;
                }
                let Some(target) = e
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                else {
                    return;
                };
                let Ok(Some(anchor)) = target.closest("a[href]") else {
                    return;
                };
                let Some(href) = anchor.get_attribute("href") else {
                    return;
                };
                let new_tab = anchor
                    .get_attribute("target")
                    .is_some_and(|t| t == "_blank");
                if new_tab || anchor.has_attribute("download") {
                    return;
                }
                if !href.starts_with('/') || href.starts_with("//") {
                    return;
                }
                let here = window().location().pathname().unwrap_or_default();
                if href == here {
                    return;
                }
                e.prevent_default();
                pending.set(Some(href));
            });
        let listener: web_sys::js_sys::Function = on_click.into_js_value().unchecked_into();
        _ = document().add_event_listener_with_callback("click", &listener);
        let listener = StoredValue::new_local(listener);
        on_cleanup(move || {
            unload.remove();
            listener.try_with_value(|f| {
                _ = document().remove_event_listener_with_callback("click", f);
            });
        });
    });
}

/// The edit page.
#[component]
pub fn DashboardEditPage() -> impl IntoView {
    let params = use_params_map();
    let id = Memo::new(move |_| params.with(|p| p.get("id").unwrap_or_default()));
    let refresh = RwSignal::new(0u32);
    // Keyed on the path id and Retry only: editing the draft never refetches the page.
    let view_res = Resource::new(
        move || (id.get(), refresh.get()),
        |(id, _)| get_dashboard_view(id),
    );

    if let Some(crumb) = use_context::<DashboardCrumb>() {
        Effect::new(move |_| {
            let label = match view_res.get() {
                Some(Ok(view)) if view.access.can_edit && !view.dashboard.read_only => {
                    Some(view.dashboard.title)
                }
                Some(Ok(_)) => Some("Not found".to_string()),
                Some(Err(e)) if is_not_found(&e) => Some("Not found".to_string()),
                _ => None,
            };
            crumb.0.set(label);
        });
        on_cleanup(move || crumb.0.set(None));
    }

    view! {
        <ContentContainer data_testid="spectra-dashboard-edit-page">
            <Transition fallback=HeaderSkeleton>
                {move || view_res.get().map(|result| match result {
                    Ok(view) if view.access.can_edit && !view.dashboard.read_only => {
                        view! { <DashboardEditor view=view /> }.into_any()
                    }
                    Ok(_) => view! { <DashboardNotFound /> }.into_any(),
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

/// A save that lost to a newer version.
#[derive(Debug, Clone)]
struct Conflict {
    current: u32,
    editing: u32,
    message: String,
}

#[component]
fn DashboardEditor(view: DashboardView) -> impl IntoView {
    let DashboardView {
        dashboard,
        access,
        missing_permissions,
        source_fields,
        ..
    } = view;
    let dashboard_id = StoredValue::new(dashboard.id.clone());
    let base = RwSignal::new(dashboard.clone());
    let draft = RwSignal::new(dashboard);
    let dirty = Memo::new(move |_| base.with(|b| draft.with(|d| has_changes(b, d))));
    let source_fields = StoredValue::new(source_fields);
    let own_missing = StoredValue::new(missing_permissions);
    let (state, on_state) = use_view_state();
    let wide = use_breakpoint_up(Breakpoint::Md);
    let navigate = use_navigate();
    let dashboard_path = move || spectra_dashboard_path(&dashboard_id.get_value());

    let saving = RwSignal::new(false);
    let save_error = RwSignal::new(None::<String>);
    let conflict = RwSignal::new(None::<Conflict>);
    let copying = RwSignal::new(false);
    let renaming = RwSignal::new(None::<String>);
    let preview = RwSignal::new(None::<PreviewAs>);
    let variables_open = RwSignal::new(false);
    let text_open = RwSignal::new(false);
    let text_target = RwSignal::new(None::<String>);
    let discard_open = RwSignal::new(false);
    let compare_open = RwSignal::new(false);
    let leave_href = RwSignal::new(None::<String>);
    let leave_open = RwSignal::new(false);
    Effect::new(move |_| {
        if leave_href.with(Option::is_some) {
            leave_open.set(true);
        }
    });
    guard_leaving(dirty, leave_href);

    let on_open_panel_editor = Callback::new(move |start: PanelEditorStart| match start {
        PanelEditorStart::Text => {
            text_target.set(None);
            text_open.set(true);
        }
        PanelEditorStart::Edit(id) => {
            let is_text = draft.with_untracked(|d| {
                d.sections
                    .iter()
                    .flat_map(|s| &s.panels)
                    .any(|p| p.id == id && matches!(p.source, PanelSource::Text { .. }))
            });
            if is_text {
                text_target.set(Some(id));
                text_open.set(true);
            }
        }
        // The panel editor lands in a later change; the menu keeps these disabled.
        PanelEditorStart::Build | PanelEditorStart::FromLink => {}
    });
    let on_add_section = Callback::new(move |()| {
        let mut id = String::new();
        draft.update(|d| id = add_section(d, "New section"));
        renaming.set(Some(id));
    });

    let on_save = Callback::new(move |()| {
        if saving.get_untracked() {
            return;
        }
        let next = draft.get_untracked();
        let editing = base.with_untracked(|b| b.version);
        saving.set(true);
        save_error.set(None);
        leptos::task::spawn_local_scoped(async move {
            match save_dashboard(next.clone(), editing, None).await {
                Ok(saved) => {
                    base.set(saved.clone());
                    draft.set(saved);
                    conflict.set(None);
                }
                Err(e) => match dashboard_error(&e) {
                    Some(DashboardError::Conflict { current }) => {
                        let versions = list_versions(next.id.clone()).await.unwrap_or_default();
                        let saved = versions.iter().find(|v| v.number == current);
                        conflict.set(Some(Conflict {
                            current,
                            editing,
                            message: conflict_text(saved, current, editing),
                        }));
                    }
                    _ => save_error.set(Some(format!(
                        "Couldn't save your changes: {}",
                        action_error_text(&e)
                    ))),
                },
            }
            saving.set(false);
        });
    });

    let save_copy = Callback::new({
        let navigate = navigate.clone();
        move |()| {
            if copying.get_untracked() {
                return;
            }
            let next = draft.get_untracked();
            let navigate = navigate.clone();
            copying.set(true);
            leptos::task::spawn_local_scoped(async move {
                let result = async {
                    let copy =
                        duplicate_dashboard(next.id.clone(), copy_title(&next.title)).await?;
                    let version = copy.version;
                    let edited = Dashboard {
                        id: copy.id,
                        title: copy.title,
                        owner: copy.owner,
                        read_only: false,
                        version,
                        updated_at: copy.updated_at,
                        ..next
                    };
                    save_dashboard(edited, version, None).await
                }
                .await;
                match result {
                    Ok(saved) => {
                        // The original's draft is now saved in the copy.
                        draft.set(base.get_untracked());
                        navigate(&dashboard_edit_path(&saved.id), NavigateOptions::default());
                    }
                    Err(e) => save_error.set(Some(format!(
                        "Couldn't save a copy: {}",
                        action_error_text(&e)
                    ))),
                }
                copying.set(false);
            });
        }
    });

    let on_done = Callback::new({
        let navigate = navigate.clone();
        move |()| navigate(&dashboard_path(), NavigateOptions::default())
    });
    let confirm_discard = Callback::new({
        let navigate = navigate.clone();
        move |()| {
            draft.set(base.get_untracked());
            discard_open.set(false);
            navigate(&dashboard_path(), NavigateOptions::default());
        }
    });
    let confirm_leave = Callback::new({
        let navigate = navigate.clone();
        move |()| {
            let href = leave_href.get_untracked();
            draft.set(base.get_untracked());
            leave_open.set(false);
            leave_href.set(None);
            if let Some(href) = href {
                navigate(&href, NavigateOptions::default());
            }
        }
    });
    let keep_editing = Callback::new(move |()| {
        leave_open.set(false);
        leave_href.set(None);
    });

    // The toolbar is rebuilt only when what it shows from the draft changes.
    let toolbar_input = Memo::new(move |_| {
        draft.with(|d| (d.variables.clone(), d.filters.clone(), needed_grants(d)))
    });
    let access = StoredValue::new(access);
    let on_preview = Callback::new(move |choice: PreviewAs| preview.set(Some(choice)));

    let title = Memo::new(move |_| base.with(|b| b.title.clone()));
    let version = Memo::new(move |_| base.with(|b| b.version));
    let section_ids = Memo::new(move |_| {
        draft.with(|d| d.sections.iter().map(|s| s.id.clone()).collect::<Vec<_>>())
    });

    view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center wrap=FlexWrap::Wrap>
                <span data-testid="spectra-edit-title">
                    <Title3>{move || format!("Editing {}", title.get())}</Title3>
                </span>
                <span data-testid="spectra-edit-version" data-version=move || version.get().to_string()>
                    <Caption1>{move || format!("Version {}", version.get())}</Caption1>
                </span>
            </Flex>
            <EditBar
                dirty=dirty.into()
                saving=saving.into()
                on_open_panel_editor=on_open_panel_editor
                on_add_section=on_add_section
                on_variables=Callback::new(move |()| variables_open.set(true))
                on_discard=Callback::new(move |()| discard_open.set(true))
                on_save=on_save
                on_done=on_done
            />
            {move || conflict.get().map(|c| view! {
                <div data-testid="spectra-edit-conflict" data-current=c.current.to_string()>
                    <MessageBar intent=MessageBarIntent::Error layout=MessageBarLayout::Multiline>
                        <MessageBarBody>
                            <MessageBarTitle>"Someone saved a newer version"</MessageBarTitle>
                            {c.message.clone()}
                        </MessageBarBody>
                        <MessageBarActions>
                            <span data-testid="spectra-edit-conflict-compare">
                                <Button on_click=Callback::new(move |_| compare_open.set(true))>
                                    "Compare versions"
                                </Button>
                            </span>
                            <span data-testid="spectra-edit-conflict-copy">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    loading=copying
                                    on_click=Callback::new(move |_| save_copy.run(()))
                                >
                                    "Save as a copy"
                                </Button>
                            </span>
                        </MessageBarActions>
                    </MessageBar>
                </div>
            })}
            {move || save_error.get().map(|message| view! {
                <div data-testid="spectra-edit-save-error">
                    <MessageBar intent=MessageBarIntent::Error>{message}</MessageBar>
                </div>
            })}
            {move || {
                let (_, _, grants) = toolbar_input.get();
                let dashboard = draft.get_untracked();
                view! {
                    <DashboardToolbar
                        dashboard=dashboard
                        access=access.get_value()
                        state=state
                        on_change=on_state
                        on_duplicate=Callback::new(|()| {})
                        on_preview=on_preview
                        preview_grants=grants
                    />
                }
            }}
            {move || match preview.get() {
                Some(viewer) => view! {
                    <PreviewPane
                        dashboard=draft.get_untracked()
                        viewer=viewer
                        own_missing=own_missing.get_value()
                        source_fields=source_fields.get_value()
                        state=state
                        on_back=Callback::new(move |()| preview.set(None))
                    />
                }
                .into_any(),
                None if wide.get() => view! {
                    <Show
                        when=move || section_ids.with(|s| !s.is_empty())
                        fallback=|| view! { <EmptyDraft /> }
                    >
                        <For each=move || section_ids.get() key=|id| id.clone() let:section_id>
                            <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                                <EditSectionHeader draft=draft section_id=section_id.clone() renaming=renaming />
                                <EditGrid
                                    draft=draft
                                    section_id=section_id
                                    on_open_panel_editor=on_open_panel_editor
                                />
                            </Flex>
                        </For>
                    </Show>
                }
                .into_any(),
                None => view! {
                    <div data-testid="spectra-edit-phone-note">
                        <MessageBar intent=MessageBarIntent::Info layout=MessageBarLayout::Multiline>
                            "Drag and resize need a wider screen. Here you can reorder, edit, add and remove panels."
                        </MessageBar>
                    </div>
                    <Show
                        when=move || section_ids.with(|s| !s.is_empty())
                        fallback=|| view! { <EmptyDraft /> }
                    >
                        <For each=move || section_ids.get() key=|id| id.clone() let:section_id>
                            <PhoneSection draft=draft section_id=section_id on_open_panel_editor=on_open_panel_editor />
                        </For>
                    </Show>
                }
                .into_any(),
            }}
        </Flex>
        <VariablesDialog open=variables_open draft=draft source_fields=source_fields />
        <TextPanelDialog open=text_open target=text_target draft=draft />
        <ConfirmDialog
            open=discard_open
            test_id="spectra-edit-discard-dialog"
            title="Discard your changes?"
            body=Signal::derive(move || {
                base.with(|b| draft.with(|d| format!(
                    "{} The dashboard goes back to version {}.",
                    change_summary(b, d),
                    b.version
                )))
            })
            confirm="Discard changes"
            on_confirm=confirm_discard
            on_cancel=Callback::new(move |()| discard_open.set(false))
        />
        <ConfirmDialog
            open=leave_open
            test_id="spectra-edit-leave-dialog"
            title="Leave without saving?"
            body=Signal::derive(move || {
                base.with(|b| draft.with(|d| format!(
                    "{} Leaving now throws those changes away.",
                    change_summary(b, d)
                )))
            })
            confirm="Leave without saving"
            on_confirm=confirm_leave
            on_cancel=keep_editing
        />
        <CompareDialog open=compare_open base=base draft=draft conflict=conflict />
    }
}

#[component]
fn EmptyDraft() -> impl IntoView {
    view! {
        <div data-testid="spectra-edit-empty">
            <EmptyState
                message="No sections yet"
                description="Add a section or a text panel to start."
            />
        </div>
    }
}

#[component]
fn PhoneSection(
    draft: RwSignal<Dashboard>,
    section_id: String,
    on_open_panel_editor: Callback<PanelEditorStart>,
) -> impl IntoView {
    let sid = StoredValue::new(section_id.clone());
    let title = move || {
        draft.with(|d| {
            d.sections
                .iter()
                .find(|s| s.id == sid.get_value())
                .map(|s| s.title.clone())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "Untitled section".to_string())
        })
    };
    view! {
        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
            <Subtitle2>{title}</Subtitle2>
            <PhoneSectionList draft=draft section_id=section_id on_open_panel_editor=on_open_panel_editor />
        </Flex>
    }
}

/// The draft as a viewer would see it, including unsaved changes.
#[component]
fn PreviewPane(
    dashboard: Dashboard,
    viewer: PreviewAs,
    own_missing: Vec<String>,
    source_fields: BTreeMap<String, Vec<String>>,
    state: Memo<spectra_backend::dashboards::view::ViewState>,
    on_back: Callback<()>,
) -> impl IntoView {
    let mut missing: BTreeSet<String> = own_missing.into_iter().collect();
    let note = match &viewer {
        PreviewAs::EveryGrant => {
            "This is how a viewer with every grant sees the dashboard, unsaved changes included."
                .to_string()
        }
        PreviewAs::Without(grant) => {
            missing.insert(grant.clone());
            format!(
                "This is how a viewer without {grant} sees the dashboard, unsaved changes included."
            )
        }
    };
    provide_context(PanelEnv {
        dashboard_id: dashboard.id.clone(),
        variables: Arc::new(dashboard.variables.clone()),
        saved_filters: Arc::new(dashboard.filters.clone()),
        state,
        missing: Arc::new(missing),
        variable_gaps: Arc::new(variable_gaps(&dashboard, &source_fields)),
    });
    view! {
        <div data-testid="spectra-edit-preview">
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                <MessageBar intent=MessageBarIntent::Info layout=MessageBarLayout::Multiline>
                    <MessageBarBody>{note}</MessageBarBody>
                    <MessageBarActions>
                        <span data-testid="spectra-edit-preview-back">
                            <Button on_click=Callback::new(move |_| on_back.run(()))>"Back to editing"</Button>
                        </span>
                    </MessageBarActions>
                </MessageBar>
                {dashboard.sections.into_iter().map(|section| view! {
                    <DashboardSection section=section />
                }).collect_view()}
            </Flex>
        </div>
    }
}

/// A small confirmation Dialog with Keep editing and one confirm button.
#[component]
fn ConfirmDialog(
    open: RwSignal<bool>,
    test_id: &'static str,
    title: &'static str,
    body: Signal<String>,
    confirm: &'static str,
    on_confirm: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    view! {
        <Dialog open=open>
            <DialogSurface size=DialogSize::Small>
                <div data-testid=test_id>
                    <DialogBody>
                        <DialogTitle>{title}</DialogTitle>
                        <DialogContent>
                            <Body1>{move || body.get()}</Body1>
                        </DialogContent>
                        <DialogActions>
                            <span data-testid="spectra-edit-keep-editing">
                                <Button appearance=ButtonAppearance::Subtle on_click=Callback::new(move |_| on_cancel.run(()))>
                                    "Keep editing"
                                </Button>
                            </span>
                            <span data-testid="spectra-edit-confirm">
                                <Button appearance=ButtonAppearance::Primary on_click=Callback::new(move |_| on_confirm.run(()))>
                                    {confirm}
                                </Button>
                            </span>
                        </DialogActions>
                    </DialogBody>
                </div>
            </DialogSurface>
        </Dialog>
    }
}

/// Compare versions: what you changed since the version you started from, and what the
/// newer saved version changed.
#[component]
fn CompareDialog(
    open: RwSignal<bool>,
    base: RwSignal<Dashboard>,
    draft: RwSignal<Dashboard>,
    conflict: RwSignal<Option<Conflict>>,
) -> impl IntoView {
    let theirs = RwSignal::new(None::<Result<Dashboard, String>>);
    Effect::new(move |_| {
        if !open.get() {
            return;
        }
        let Some(c) = conflict.get_untracked() else {
            return;
        };
        theirs.set(None);
        let id = base.with_untracked(|b| b.id.clone());
        leptos::task::spawn_local_scoped(async move {
            let result = get_dashboard(id, Some(c.current))
                .await
                .map_err(|e| action_error_text(&e));
            theirs.set(Some(result));
        });
    });
    let lines = |parts: Vec<String>| {
        if parts.is_empty() {
            view! { <Caption1>"Nothing."</Caption1> }.into_any()
        } else {
            view! {
                <List>
                    {parts.into_iter().map(|p| view! { <ListItem>{capitalise(&p)}</ListItem> }).collect_view()}
                </List>
            }
            .into_any()
        }
    };
    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-edit-compare-dialog">
                    <DialogBody>
                        <DialogTitle>"Compare versions"</DialogTitle>
                        <DialogContent>
                            {move || {
                                let Some(c) = conflict.get() else {
                                    return ().into_any();
                                };
                                let mine = base.with(|b| draft.with(|d| change_parts(b, d)));
                                view! {
                                    <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                        <div data-testid="spectra-edit-compare-mine">
                                            <Subtitle2>{format!("Your changes since version {}", c.editing)}</Subtitle2>
                                            {lines(mine)}
                                        </div>
                                        <div data-testid="spectra-edit-compare-theirs">
                                            <Subtitle2>{format!("What version {} changed", c.current)}</Subtitle2>
                                            {move || match theirs.get() {
                                                None => view! {
                                                    <Skeleton>
                                                        <SkeletonItem width="16rem".to_string() height="1rem".to_string() />
                                                    </Skeleton>
                                                }
                                                .into_any(),
                                                Some(Ok(newer)) => lines(base.with(|b| change_parts(b, &newer))),
                                                Some(Err(message)) => view! {
                                                    <MessageBar intent=MessageBarIntent::Error>
                                                        {format!("Couldn't load version {}: {message}", c.current)}
                                                    </MessageBar>
                                                }
                                                .into_any(),
                                            }}
                                        </div>
                                    </Flex>
                                }
                                .into_any()
                            }}
                        </DialogContent>
                        <DialogActions>
                            <Button on_click=Callback::new(move |_| open.set(false))>"Close"</Button>
                        </DialogActions>
                    </DialogBody>
                </div>
            </DialogSurface>
        </Dialog>
    }
}

/// Title and Markdown for a text panel. With `target` set it edits that panel, otherwise
/// Apply adds a new one.
#[component]
fn TextPanelDialog(
    open: RwSignal<bool>,
    target: RwSignal<Option<String>>,
    draft: RwSignal<Dashboard>,
) -> impl IntoView {
    let title = RwSignal::new(String::new());
    let markdown = RwSignal::new(String::new());
    Effect::new(move |_| {
        if !open.get() {
            return;
        }
        let existing = target.get_untracked().and_then(|id| {
            draft.with_untracked(|d| {
                d.sections
                    .iter()
                    .flat_map(|s| &s.panels)
                    .find(|p| p.id == id)
                    .and_then(|p| match &p.source {
                        PanelSource::Text { markdown } => Some((p.title.clone(), markdown.clone())),
                        _ => None,
                    })
            })
        });
        let (t, m) = existing.unwrap_or_default();
        title.set(t);
        markdown.set(m);
    });
    let heading = move || {
        if target.with(Option::is_some) {
            "Edit text"
        } else {
            "Add text"
        }
    };
    let apply = Callback::new(move |_| {
        let (t, m) = (
            title.get_untracked().trim().to_string(),
            markdown.get_untracked(),
        );
        if m.trim().is_empty() {
            return;
        }
        match target.get_untracked() {
            Some(id) => draft.update(|d| {
                if let Some(panel) = d
                    .sections
                    .iter_mut()
                    .flat_map(|s| s.panels.iter_mut())
                    .find(|p| p.id == id)
                {
                    panel.title = t;
                    panel.source = PanelSource::Text { markdown: m };
                }
            }),
            None => draft.update(|d| {
                add_text_panel(d, t, m);
            }),
        }
        open.set(false);
    });
    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-edit-text-dialog">
                    <DialogBody>
                        <DialogTitle>{heading}</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Field label="Title">
                                    <div data-testid="spectra-edit-text-title">
                                        <Input bind=InputBind::new(title) />
                                    </div>
                                </Field>
                                <Field label="Text (Markdown)" required=true>
                                    <div data-testid="spectra-edit-text-markdown">
                                        <Textarea bind=TextareaBind::new(markdown) />
                                    </div>
                                </Field>
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button appearance=ButtonAppearance::Subtle on_click=Callback::new(move |_| open.set(false))>
                                "Cancel"
                            </Button>
                            <span data-testid="spectra-edit-text-apply">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    disabled=Signal::derive(move || markdown.with(|m| m.trim().is_empty()))
                                    on_click=apply
                                >
                                    "Apply"
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
    use chrono::{TimeZone, Utc};

    use super::*;

    fn dashboard(sections: Vec<Section>) -> Dashboard {
        Dashboard {
            id: "d".into(),
            title: "Checkout health".into(),
            description: String::new(),
            owner: Some("ines".into()),
            read_only: false,
            sections,
            variables: Vec::new(),
            filters: Vec::new(),
            version: 8,
            updated_at: Utc.with_ymd_and_hms(2026, 11, 3, 9, 0, 0).unwrap(),
        }
    }

    fn section(id: &str, panels: Vec<Panel>) -> Section {
        Section {
            id: id.into(),
            title: id.into(),
            collapsed: false,
            panels,
        }
    }

    fn text(id: &str, col: u8, row: u16) -> Panel {
        Panel {
            id: id.into(),
            title: id.into(),
            source: PanelSource::Text {
                markdown: "hi".into(),
            },
            chart: ChartKind::Table,
            measure: None,
            bucket: None,
            group_by: Vec::new(),
            filters: Vec::new(),
            table: None,
            layout: Layout {
                col,
                span: 4,
                row,
                height: 2,
            },
            keep_window: None,
        }
    }

    #[test]
    fn summary_names_moves_and_additions() {
        let base = dashboard(vec![section("a", vec![text("p1", 0, 0), text("p2", 4, 0)])]);
        let mut next = base.clone();
        next.sections[0].panels[0].layout.col = 8;
        next.sections[0].panels[1].layout.row = 2;
        next.sections[0].panels.push(text("p3", 0, 4));
        assert_eq!(
            change_summary(&base, &next),
            "You moved two panels and added one panel."
        );
        assert!(has_changes(&base, &next));
    }

    #[test]
    fn summary_names_section_changes() {
        let base = dashboard(vec![section("a", Vec::new()), section("b", Vec::new())]);
        let mut next = base.clone();
        next.sections.swap(0, 1);
        next.sections[0].title = "Renamed".into();
        add_section(&mut next, "New");
        assert_eq!(
            change_summary(&base, &next),
            "You added one section, renamed one section and reordered the sections."
        );
    }

    #[test]
    fn summary_without_changes_says_so() {
        let base = dashboard(Vec::new());
        assert_eq!(
            change_summary(&base, &base),
            "You haven't changed anything."
        );
        assert!(!has_changes(&base, &base));
    }

    #[test]
    fn saved_version_number_alone_is_not_a_change() {
        let base = dashboard(Vec::new());
        let saved = Dashboard {
            version: 9,
            ..base.clone()
        };
        assert!(!has_changes(&base, &saved));
    }

    #[test]
    fn conflict_text_names_the_agent_and_time() {
        let v = Version {
            number: 9,
            author: Some("release-bot".into()),
            via: Some("MCP".into()),
            created_at: Utc.with_ymd_and_hms(2026, 11, 3, 9, 30, 0).unwrap(),
            summary: String::new(),
        };
        assert_eq!(
            conflict_text(Some(&v), 9, 8),
            "release-bot saved version 9 through MCP at 09:30 UTC while you were editing version 8. Your changes are still here."
        );
        let person = Version {
            via: None,
            author: Some("rosa".into()),
            ..v
        };
        assert!(conflict_text(Some(&person), 9, 8).starts_with("rosa saved version 9 at 09:30 UTC"));
        assert_eq!(
            conflict_text(None, 9, 8),
            "Someone saved version 9 while you were editing version 8. Your changes are still here."
        );
    }

    #[test]
    fn next_id_skips_taken_ids() {
        assert_eq!(next_id("panel", ["panel-1", "panel-2", "x"]), "panel-3");
        assert_eq!(next_id("section", []), "section-1");
    }

    #[test]
    fn text_panels_go_below_the_last_section() {
        let mut d = dashboard(vec![section("a", vec![text("p1", 0, 0)])]);
        let id = add_text_panel(&mut d, "Notes".into(), "**hi**".into());
        let panel = d.sections[0].panels.iter().find(|p| p.id == id).unwrap();
        assert_eq!(panel.layout.row, 2);
        assert!(d.validate().is_ok());

        let mut empty = dashboard(Vec::new());
        add_text_panel(&mut empty, String::new(), "x".into());
        assert_eq!(empty.sections.len(), 1);
        assert!(empty.validate().is_ok());
    }

    #[test]
    fn preview_gaps_follow_the_known_fields() {
        let mut d = dashboard(vec![section(
            "a",
            vec![Panel {
                source: PanelSource::EventTable {
                    table: "refunds".into(),
                },
                ..text("p1", 0, 0)
            }],
        )]);
        d.variables = vec![spectra_backend::dashboards::Variable {
            name: "host".into(),
            field: "host".into(),
            default: None,
        }];
        let fields = BTreeMap::from([("refunds".to_string(), vec!["reason".to_string()])]);
        assert_eq!(
            variable_gaps(&d, &fields).get("p1"),
            Some(&vec!["host".to_string()])
        );
        assert!(variable_gaps(&d, &BTreeMap::new()).is_empty());
        assert_eq!(needed_grants(&d).len(), 1);
    }
}
