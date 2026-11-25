//! The edit-mode grid: twelve columns and 80px rows. Each panel drags by the handle in its
//! header, resizes from its bottom-right corner and snaps to whole columns and rows. On a
//! focused handle the arrow keys move the panel and Shift with an arrow key resizes it.
//!
//! A section drags by its handle too: release it over another section to move it there.
//! Moving a panel into another section is not built yet.
//!
//! Below `Md` there is no drag or resize: [`PhoneSectionList`] shows each panel as a List row
//! with Move up, Move down, Edit and Remove.
//!
//! TODO(orbital): Orbital has no dashboard grid yet. Replace [`EditGrid`] with an Orbital
//! `DashboardGrid` (snapped drag and resize, keyboard moves, a read-only mode matching Orbital
//! `Grid`) once it lands.

use leptos::ev;
use leptos::html::Div;
use leptos::prelude::*;
use orbital::components::{
    Caption1, EmptyState, List, ListItem, ListItemTrailing, SpacingSize, Subtitle1, Subtitle2,
};
use orbital::primitives::{
    Body1, Button, ButtonAppearance, ButtonSize, Flex, FlexAlign, FlexJustify, Input, InputBind,
    InputEvents, Menu, MenuItem, MenuTrigger, Tooltip,
};
use spectra_backend::dashboards::view::panel_caption;
use spectra_backend::dashboards::{
    Dashboard, Layout, Panel, PanelSource, Section, DASHBOARD_GRID_COLUMNS,
};
use wasm_bindgen::{closure::Closure, JsCast};

use super::edit::PanelEditorStart;

/// Height of one grid row in pixels.
const ROW_PX: f64 = 80.0;
/// Gap between rows and columns in pixels; matches `--orb-space-inline-sm`.
const GAP_PX: f64 = 8.0;

/// Keep a layout inside the twelve columns, at least one column wide and one row tall.
pub(super) fn clamp_layout(l: Layout) -> Layout {
    let span = l.span.clamp(1, DASHBOARD_GRID_COLUMNS);
    Layout {
        col: l.col.min(DASHBOARD_GRID_COLUMNS - span),
        span,
        row: l.row,
        height: l.height.max(1),
    }
}

fn overlaps(a: Layout, b: Layout) -> bool {
    let (a_start, a_end) = (u16::from(a.col), u16::from(a.col) + u16::from(a.span));
    let (b_start, b_end) = (u16::from(b.col), u16::from(b.col) + u16::from(b.span));
    a_start < b_end && b_start < a_end && a.row < b.row + b.height && b.row < a.row + a.height
}

/// Push panels down until none overlap. The panel with id `fixed` keeps its spot; the rest
/// settle in row-then-column order, each moving below whatever it overlaps.
fn settle(panels: &mut [Panel], fixed: &str) {
    let mut settled: Vec<Layout> = panels
        .iter()
        .filter(|p| p.id == fixed)
        .map(|p| p.layout)
        .collect();
    let mut order: Vec<usize> = (0..panels.len())
        .filter(|&i| panels[i].id != fixed)
        .collect();
    order.sort_by_key(|&i| (panels[i].layout.row, panels[i].layout.col));
    for i in order {
        let mut layout = panels[i].layout;
        while let Some(block) = settled.iter().find(|s| overlaps(**s, layout)) {
            layout.row = block.row + block.height;
        }
        panels[i].layout = layout;
        settled.push(layout);
    }
}

/// Put panel `id` at `target` (clamped to the grid) and push any panel it now covers
/// further down. Returns `false` when there is no such panel.
pub(super) fn place(panels: &mut [Panel], id: &str, target: Layout) -> bool {
    let Some(panel) = panels.iter_mut().find(|p| p.id == id) else {
        return false;
    };
    panel.layout = clamp_layout(target);
    settle(panels, id);
    true
}

/// The layout after pressing `key` on a panel's handle: arrows move by one column or row,
/// and with `resize` (Shift held) they change the width or height instead. `None` for any
/// other key.
pub(super) fn nudge(l: Layout, key: &str, resize: bool) -> Option<Layout> {
    let mut next = l;
    match (key, resize) {
        ("ArrowLeft", false) => next.col = l.col.saturating_sub(1),
        ("ArrowRight", false) => next.col = l.col.saturating_add(1),
        ("ArrowUp", false) => next.row = l.row.saturating_sub(1),
        ("ArrowDown", false) => next.row = l.row.saturating_add(1),
        ("ArrowLeft", true) => next.span = l.span.saturating_sub(1),
        ("ArrowRight", true) => next.span = l.span.saturating_add(1),
        ("ArrowUp", true) => next.height = l.height.saturating_sub(1),
        ("ArrowDown", true) => next.height = l.height.saturating_add(1),
        _ => return None,
    }
    Some(clamp_layout(next))
}

/// The layout after dragging by `cols` columns and `rows` rows from `start`: a move, or with
/// `resize` a change of width and height.
pub(super) fn drag_target(start: Layout, cols: i32, rows: i32, resize: bool) -> Layout {
    let shift_u8 = |v: u8, by: i32| u8::try_from((i32::from(v) + by).max(0)).unwrap_or(u8::MAX);
    let shift_u16 = |v: u16, by: i32| u16::try_from((i32::from(v) + by).max(0)).unwrap_or(u16::MAX);
    let next = if resize {
        Layout {
            span: shift_u8(start.span, cols),
            height: shift_u16(start.height, rows),
            ..start
        }
    } else {
        Layout {
            col: shift_u8(start.col, cols),
            row: shift_u16(start.row, rows),
            ..start
        }
    };
    clamp_layout(next)
}

/// Swap panel `id` with the panel before it (`up`) or after it in reading order, keeping
/// both sizes. Returns `false` when it is already first or last.
pub(super) fn swap_with_neighbour(panels: &mut [Panel], id: &str, up: bool) -> bool {
    let mut order: Vec<usize> = (0..panels.len()).collect();
    order.sort_by_key(|&i| (panels[i].layout.row, panels[i].layout.col));
    let Some(pos) = order.iter().position(|&i| panels[i].id == id) else {
        return false;
    };
    let other = if up {
        pos.checked_sub(1)
    } else {
        Some(pos + 1).filter(|&n| n < order.len())
    };
    let Some(other) = other else {
        return false;
    };
    let (a, b) = (order[pos], order[other]);
    let (la, lb) = (panels[a].layout, panels[b].layout);
    if la.row == lb.row {
        // Side by side: the panel that now comes first takes the leftmost start and the
        // other follows it.
        let (first, second) = if up { (a, b) } else { (b, a) };
        let start = la.col.min(lb.col);
        let first_layout = clamp_layout(Layout {
            col: start,
            ..panels[first].layout
        });
        panels[first].layout = first_layout;
        panels[second].layout = clamp_layout(Layout {
            col: start.saturating_add(first_layout.span),
            ..panels[second].layout
        });
        let fixed = panels[first].id.clone();
        settle(panels, &fixed);
        return true;
    }
    panels[a].layout = clamp_layout(Layout {
        col: lb.col,
        row: lb.row,
        ..la
    });
    panels[b].layout = clamp_layout(Layout {
        col: la.col,
        row: la.row,
        ..lb
    });
    let moved = panels[a].id.clone();
    settle(panels, &moved);
    true
}

/// Move section `id` one place up or down. Returns `false` at either end.
pub(super) fn move_section(sections: &mut [Section], id: &str, up: bool) -> bool {
    let Some(pos) = sections.iter().position(|s| s.id == id) else {
        return false;
    };
    let other = if up {
        pos.checked_sub(1)
    } else {
        Some(pos + 1).filter(|&n| n < sections.len())
    };
    other.is_some_and(|other| {
        sections.swap(pos, other);
        true
    })
}

/// Move section `id` so that `index` other sections come before it. Returns `false` when
/// nothing moves: unknown id, or it is already there.
pub(super) fn move_section_to(sections: &mut Vec<Section>, id: &str, index: usize) -> bool {
    let Some(pos) = sections.iter().position(|s| s.id == id) else {
        return false;
    };
    let index = index.min(sections.len() - 1);
    if pos == index {
        return false;
    }
    let section = sections.remove(pos);
    sections.insert(index, section);
    true
}

/// How many other sections' headers sit above pointer height `y`: the index a dragged
/// section lands at.
fn drop_index(section_id: &str, y: f64) -> usize {
    let Ok(headers) = document().query_selector_all("[data-section-id]") else {
        return 0;
    };
    (0..headers.length())
        .filter_map(|i| headers.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::Element>().ok())
        .filter(|el| el.get_attribute("data-section-id").as_deref() != Some(section_id))
        .filter(|el| {
            let r = el.get_bounding_client_rect();
            r.top() + r.height() / 2.0 < y
        })
        .count()
}

/// Swallow the click that follows a drag release, so it doesn't open the handle's menu.
fn swallow_next_click() {
    let doc = document();
    let on_click = Closure::<dyn Fn(web_sys::Event)>::new(|e: web_sys::Event| {
        e.stop_propagation();
        e.prevent_default();
    });
    let listener: web_sys::js_sys::Function = on_click.into_js_value().unchecked_into();
    _ = doc.add_event_listener_with_callback_and_bool("click", &listener, true);
    // No click follows a release away from the handle; don't hold the next real one.
    set_timeout(
        move || {
            _ = document().remove_event_listener_with_callback_and_bool("click", &listener, true);
        },
        std::time::Duration::from_millis(50),
    );
}

/// The first row below every panel in `panels`.
pub(super) fn next_free_row(panels: &[Panel]) -> u16 {
    panels
        .iter()
        .map(|p| p.layout.row + p.layout.height)
        .max()
        .unwrap_or(0)
}

/// Panels in reading order: row, then column.
fn reading_order(panels: &[Panel]) -> Vec<Panel> {
    let mut panels = panels.to_vec();
    panels.sort_by_key(|p| (p.layout.row, p.layout.col));
    panels
}

fn find_section<'a>(d: &'a Dashboard, section_id: &str) -> Option<&'a Section> {
    d.sections.iter().find(|s| s.id == section_id)
}

fn find_panel<'a>(d: &'a Dashboard, panel_id: &str) -> Option<&'a Panel> {
    d.sections
        .iter()
        .flat_map(|s| &s.panels)
        .find(|p| p.id == panel_id)
}

fn with_section(draft: RwSignal<Dashboard>, section_id: &str, f: impl FnOnce(&mut Section)) {
    draft.update(|d| {
        if let Some(section) = d.sections.iter_mut().find(|s| s.id == section_id) {
            f(section);
        }
    });
}

const fn is_text(panel: &Panel) -> bool {
    matches!(panel.source, PanelSource::Text { .. })
}

/// Edit for a text panel opens the text dialog; other panels wait for the panel editor.
#[component]
fn PanelEditButton(
    panel_id: String,
    title: Signal<String>,
    text: bool,
    on_open_panel_editor: Callback<PanelEditorStart>,
) -> impl IntoView {
    let label = move || format!("Edit {}", title.get());
    if text {
        view! {
            <span data-testid="spectra-edit-panel-edit">
                <Button
                    appearance=ButtonAppearance::Subtle
                    size=ButtonSize::Small
                    icon=icondata::AiEditOutlined
                    attr:aria-label=label
                    on_click=Callback::new(move |_| {
                        on_open_panel_editor.run(PanelEditorStart::Edit(panel_id.clone()));
                    })
                />
            </span>
        }
        .into_any()
    } else {
        view! {
            <Tooltip content="Coming soon".to_string()>
                <span data-testid="spectra-edit-panel-edit">
                    <Button
                        appearance=ButtonAppearance::Subtle
                        size=ButtonSize::Small
                        icon=icondata::AiEditOutlined
                        disabled_focusable=true
                        attr:aria-label=label
                    />
                </span>
            </Tooltip>
        }
        .into_any()
    }
}

/// A drag or resize in progress.
#[derive(Clone, Copy)]
struct Drag {
    x: f64,
    y: f64,
    start: Layout,
    resize: bool,
    col_px: f64,
}

/// One section's panels on the twelve-column grid.
#[component]
pub(super) fn EditGrid(
    /// The dashboard being edited.
    draft: RwSignal<Dashboard>,
    /// The section shown.
    section_id: String,
    /// Opens the panel editor (or, for now, the text dialog).
    on_open_panel_editor: Callback<PanelEditorStart>,
) -> impl IntoView {
    let grid_ref = NodeRef::<Div>::new();
    let drag = RwSignal::new(None::<(String, Drag)>);
    let preview = RwSignal::new(None::<(String, Layout)>);

    let sid = StoredValue::new(section_id);
    let ids = Memo::new(move |_| {
        draft.with(|d| {
            find_section(d, &sid.get_value()).map_or_else(Vec::new, |s| {
                s.panels.iter().map(|p| p.id.clone()).collect::<Vec<_>>()
            })
        })
    });

    let commit = move |id: &str, target: Layout| {
        with_section(draft, &sid.get_value(), |s| {
            let current = s.panels.iter().find(|p| p.id == id).map(|p| p.layout);
            if current != Some(clamp_layout(target)) {
                place(&mut s.panels, id, target);
            }
        });
    };

    // Moves and releases are tracked on the window so a fast drag that leaves the handle
    // still follows the pointer.
    let target_of = move |ev: &ev::PointerEvent| {
        drag.with_untracked(|d| {
            d.as_ref().map(|(id, d)| {
                let cols = ((f64::from(ev.client_x()) - d.x) / d.col_px).round();
                let rows = ((f64::from(ev.client_y()) - d.y) / (ROW_PX + GAP_PX)).round();
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "a drag spans far fewer than 2^31 cells"
                )]
                let target = drag_target(d.start, cols as i32, rows as i32, d.resize);
                (id.clone(), target)
            })
        })
    };
    let on_move = window_event_listener(ev::pointermove, move |ev| {
        if let Some(next) = target_of(&ev) {
            if preview.get_untracked().as_ref() != Some(&next) {
                preview.set(Some(next));
            }
        }
    });
    let on_up = window_event_listener(ev::pointerup, move |ev| {
        if let Some((id, target)) = target_of(&ev) {
            commit(&id, target);
        }
        drag.set(None);
        preview.set(None);
    });
    on_cleanup(move || {
        on_move.remove();
        on_up.remove();
    });

    let start_drag = move |ev: &ev::PointerEvent, id: &str, resize: bool| {
        if ev.button() != 0 {
            return;
        }
        let Some(start) = draft.with_untracked(|d| find_panel(d, id).map(|p| p.layout)) else {
            return;
        };
        let width = grid_ref
            .get_untracked()
            .map_or(0.0, |el| f64::from(el.client_width()));
        if width <= 0.0 {
            return;
        }
        ev.prevent_default();
        let col_px = (width + GAP_PX) / f64::from(DASHBOARD_GRID_COLUMNS);
        drag.set(Some((
            id.to_string(),
            Drag {
                x: f64::from(ev.client_x()),
                y: f64::from(ev.client_y()),
                start,
                resize,
                col_px,
            },
        )));
        preview.set(Some((id.to_string(), start)));
    };

    view! {
        <div
            node_ref=grid_ref
            data-testid="spectra-edit-grid"
            data-columns=DASHBOARD_GRID_COLUMNS.to_string()
            style=format!(
                "display: grid; grid-template-columns: repeat({DASHBOARD_GRID_COLUMNS}, minmax(0, 1fr)); \
                 grid-auto-rows: {ROW_PX}px; gap: {GAP_PX}px; min-height: {ROW_PX}px; \
                 padding: var(--orb-space-block-xs) 0; \
                 background-image: linear-gradient(to right, var(--orb-color-border-subtle) 1px, transparent 1px); \
                 background-size: calc((100% + {GAP_PX}px) / {DASHBOARD_GRID_COLUMNS}) 100%;"
            )
        >
            <For each=move || ids.get() key=|id| id.clone() let:panel_id>
                <EditCell
                    draft=draft
                    panel_id=panel_id
                    preview=preview
                    drag=drag
                    on_start=Callback::new(move |(ev, id, resize): (ev::PointerEvent, String, bool)| {
                        start_drag(&ev, &id, resize);
                    })
                    on_key=Callback::new(move |(id, target): (String, Layout)| commit(&id, target))
                    on_remove=Callback::new(move |id: String| {
                        with_section(draft, &sid.get_value(), |s| s.panels.retain(|p| p.id != id));
                    })
                    on_open_panel_editor=on_open_panel_editor
                />
            </For>
        </div>
    }
}

/// One panel on the edit grid: its header with the drag handle, and the resize corner.
#[component]
fn EditCell(
    draft: RwSignal<Dashboard>,
    panel_id: String,
    preview: RwSignal<Option<(String, Layout)>>,
    drag: RwSignal<Option<(String, Drag)>>,
    on_start: Callback<(ev::PointerEvent, String, bool)>,
    on_key: Callback<(String, Layout)>,
    on_remove: Callback<String>,
    on_open_panel_editor: Callback<PanelEditorStart>,
) -> impl IntoView {
    let id = StoredValue::new(panel_id.clone());
    let panel = Memo::new(move |_| draft.with(|d| find_panel(d, &id.get_value()).cloned()));
    let layout = Memo::new(move |_| {
        preview
            .with(|p| {
                p.as_ref()
                    .filter(|(pid, _)| *pid == id.get_value())
                    .map(|(_, l)| *l)
            })
            .or_else(|| panel.with(|p| p.as_ref().map(|p| p.layout)))
            .unwrap_or(Layout {
                col: 0,
                span: 1,
                row: 0,
                height: 1,
            })
    });
    let dragging = Memo::new(move |_| {
        drag.with(|d| d.as_ref().is_some_and(|(pid, _)| *pid == id.get_value()))
    });
    let title = Signal::derive(move || {
        panel.with(|p| {
            p.as_ref().map_or_else(String::new, |p| {
                if p.title.is_empty() {
                    "Untitled panel".to_string()
                } else {
                    p.title.clone()
                }
            })
        })
    });
    let caption = move || panel.with(|p| p.as_ref().map_or_else(String::new, panel_caption));
    let text = panel.with_untracked(|p| p.as_ref().is_some_and(is_text));
    let style = move || {
        let l = layout.get();
        let (border, background) = if dragging.get() {
            (
                "2px dashed var(--orb-color-brand-stroke)",
                "var(--orb-color-brand-bg-subtle)",
            )
        } else {
            (
                "1px solid var(--orb-color-border-subtle)",
                "var(--orb-color-surface-raised)",
            )
        };
        format!(
            "grid-column: {} / span {}; grid-row: {} / span {}; border: {border}; \
             background: {background}; border-radius: var(--orb-radius-md); \
             padding: var(--orb-space-block-xs) var(--orb-space-inline-sm); position: relative; \
             min-width: 0; overflow: hidden;",
            l.col + 1,
            l.span,
            l.row + 1,
            l.height
        )
    };
    let handle_label = move || {
        format!(
            "Move {}. Arrow keys move it; Shift and an arrow key resize it.",
            title.get()
        )
    };
    let on_keydown = move |ev: ev::KeyboardEvent| {
        let Some(current) = panel.with_untracked(|p| p.as_ref().map(|p| p.layout)) else {
            return;
        };
        if let Some(next) = nudge(current, &ev.key(), ev.shift_key()) {
            ev.prevent_default();
            on_key.run((id.get_value(), next));
        }
    };
    let test_id = format!("spectra-edit-panel-{panel_id}");

    view! {
        <div
            data-testid=test_id
            data-col=move || layout.get().col.to_string()
            data-row=move || layout.get().row.to_string()
            data-span=move || layout.get().span.to_string()
            data-height=move || layout.get().height.to_string()
            data-dragging=move || dragging.get().to_string()
            style=style
        >
            <Flex gap=SpacingSize::Size40.flex_gap() align=FlexAlign::Start justify=FlexJustify::SpaceBetween>
                <Flex gap=SpacingSize::Size40.flex_gap() align=FlexAlign::Center>
                    <span
                        data-testid="spectra-edit-panel-handle"
                        style="touch-action: none; cursor: grab;"
                        on:pointerdown=move |ev| on_start.run((ev, id.get_value(), false))
                        on:keydown=on_keydown
                    >
                        <Button
                            appearance=ButtonAppearance::Transparent
                            size=ButtonSize::Small
                            icon=icondata::AiHolderOutlined
                            attr:aria-label=handle_label
                        />
                    </span>
                    <Flex vertical=true gap=SpacingSize::Size20.flex_gap()>
                        <Subtitle2>{move || title.get()}</Subtitle2>
                        <Caption1>{caption}</Caption1>
                    </Flex>
                </Flex>
                <Flex gap=SpacingSize::Size20.flex_gap() align=FlexAlign::Center>
                    <PanelEditButton
                        panel_id=panel_id.clone()
                        title=title
                        text=text
                        on_open_panel_editor=on_open_panel_editor
                    />
                    <span data-testid="spectra-edit-panel-remove">
                        <Button
                            appearance=ButtonAppearance::Subtle
                            size=ButtonSize::Small
                            icon=icondata::AiDeleteOutlined
                            attr:aria-label=move || format!("Remove {}", title.get())
                            on_click=Callback::new(move |_| on_remove.run(id.get_value()))
                        />
                    </span>
                </Flex>
            </Flex>
            <div
                data-testid="spectra-edit-panel-resize"
                aria-hidden="true"
                style="position: absolute; right: 4px; bottom: 4px; width: 12px; height: 12px; \
                       border-right: 2px solid var(--orb-color-border-default); \
                       border-bottom: 2px solid var(--orb-color-border-default); \
                       cursor: nwse-resize; touch-action: none;"
                on:pointerdown=move |ev| on_start.run((ev, id.get_value(), true))
            ></div>
        </div>
    }
}

/// A section's header in edit mode: its handle, title (renamed in place) and actions.
#[component]
pub(super) fn EditSectionHeader(
    /// The dashboard being edited.
    draft: RwSignal<Dashboard>,
    /// The section.
    section_id: String,
    /// The section whose title is being renamed, if any.
    renaming: RwSignal<Option<String>>,
) -> impl IntoView {
    let sid = StoredValue::new(section_id.clone());
    let title = Memo::new(move |_| {
        draft.with(|d| {
            find_section(d, &sid.get_value()).map_or_else(String::new, |s| s.title.clone())
        })
    });
    let shown_title = move || {
        let t = title.get();
        if t.is_empty() {
            "Untitled section".to_string()
        } else {
            t
        }
    };
    let position = Memo::new(move |_| {
        draft.with(|d| {
            (
                d.sections.iter().position(|s| s.id == sid.get_value()),
                d.sections.len(),
            )
        })
    });
    let reorder = move |up: bool| {
        draft.update(|d| {
            move_section(&mut d.sections, &sid.get_value(), up);
        });
    };
    let is_renaming =
        Memo::new(move |_| renaming.with(|r| r.as_deref() == Some(sid.get_value().as_str())));
    let name = RwSignal::new(String::new());
    Effect::new(move |_| {
        if is_renaming.get() {
            name.set(title.get_untracked());
        }
    });
    let commit = move || {
        if !is_renaming.get_untracked() {
            return;
        }
        let next = name.get_untracked().trim().to_string();
        with_section(draft, &sid.get_value(), |s| {
            if s.title != next {
                s.title = next;
            }
        });
        renaming.set(None);
    };
    let on_keydown = move |ev: ev::KeyboardEvent| match ev.key().as_str() {
        "Enter" => {
            ev.prevent_default();
            commit();
        }
        "Escape" => renaming.set(None),
        _ => {}
    };
    let on_handle_key = move |ev: ev::KeyboardEvent| match ev.key().as_str() {
        "ArrowUp" => {
            ev.prevent_default();
            reorder(true);
        }
        "ArrowDown" => {
            ev.prevent_default();
            reorder(false);
        }
        _ => {}
    };
    let test_id = format!("spectra-edit-section-{section_id}");

    // A drag starts after the pointer moves a few pixels, so a plain click still opens the menu.
    let press = StoredValue::new(None::<f64>);
    let dragging = RwSignal::new(false);
    let drop_at = RwSignal::new(0usize);
    let on_move = window_event_listener(ev::pointermove, move |ev| {
        let Some(start_y) = press.try_get_value().flatten() else {
            return;
        };
        let y = f64::from(ev.client_y());
        if !dragging.get_untracked() && (y - start_y).abs() > 4.0 {
            dragging.set(true);
        }
        if dragging.get_untracked() {
            drop_at.set(drop_index(&sid.get_value(), y));
        }
    });
    let on_up = window_event_listener(ev::pointerup, move |ev| {
        if press.try_get_value().flatten().is_none() {
            return;
        }
        press.set_value(None);
        if dragging.get_untracked() {
            dragging.set(false);
            let to = drop_index(&sid.get_value(), f64::from(ev.client_y()));
            draft.update(|d| {
                move_section_to(&mut d.sections, &sid.get_value(), to);
            });
            swallow_next_click();
        }
    });
    on_cleanup(move || {
        on_move.remove();
        on_up.remove();
    });

    view! {
        <div
            data-testid=test_id
            data-section-id=section_id
            data-dragging=move || dragging.get().to_string()
            style=move || if dragging.get() {
                "outline: 2px dashed var(--orb-color-brand-stroke);"
            } else {
                ""
            }
        >
            <Flex gap=SpacingSize::Size40.flex_gap() align=FlexAlign::Center>
                <span
                    data-testid="spectra-edit-section-handle"
                    style="touch-action: none; cursor: grab;"
                    on:keydown=on_handle_key
                    on:pointerdown=move |ev| {
                        if ev.button() == 0 {
                            press.set_value(Some(f64::from(ev.client_y())));
                        }
                    }
                >
                    <Menu
                        on_select=move |action: &str| match action {
                            "up" => reorder(true),
                            "down" => reorder(false),
                            "remove" => draft.update(|d| d.sections.retain(|s| s.id != sid.get_value())),
                            _ => {}
                        }
                    >
                        <MenuTrigger slot>
                            <Button
                                appearance=ButtonAppearance::Transparent
                                size=ButtonSize::Small
                                icon=icondata::AiHolderOutlined
                                attr:aria-label=move || format!(
                                    "Reorder {}. Drag it, or press arrow up or down.",
                                    shown_title()
                                )
                            />
                        </MenuTrigger>
                        <MenuItem value="up" disabled=Signal::derive(move || position.get().0 == Some(0))>
                            "Move up"
                        </MenuItem>
                        <MenuItem
                            value="down"
                            disabled=Signal::derive(move || {
                                let (pos, len) = position.get();
                                pos.is_none_or(|p| p + 1 >= len)
                            })
                        >
                            "Move down"
                        </MenuItem>
                        <MenuItem value="remove">"Remove section"</MenuItem>
                    </Menu>
                </span>
                <Show
                    when=move || is_renaming.get()
                    fallback=move || view! {
                        <span data-testid="spectra-edit-section-title">
                            <Subtitle1>{shown_title}</Subtitle1>
                        </span>
                        <span data-testid="spectra-edit-section-rename">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Small
                                icon=icondata::AiEditOutlined
                                on_click=Callback::new(move |_| renaming.set(Some(sid.get_value())))
                            >
                                "Rename"
                            </Button>
                        </span>
                        <Show when=move || dragging.get()>
                            <Caption1>
                                {move || format!("Drop to move to position {}", drop_at.get() + 1)}
                            </Caption1>
                        </Show>
                    }
                >
                    <span data-testid="spectra-edit-section-name" on:keydown=on_keydown>
                        <Input
                            bind=InputBind::new(name)
                            events=InputEvents {
                                on_blur: Some(Box::new(move |_| commit()).into()),
                                ..Default::default()
                            }
                            attr:aria-label="Section title"
                        />
                    </span>
                </Show>
            </Flex>
        </div>
    }
}

/// One section at phone width: a List row per panel with Move up, Move down, Edit and Remove.
#[component]
pub(super) fn PhoneSectionList(
    /// The dashboard being edited.
    draft: RwSignal<Dashboard>,
    /// The section shown.
    section_id: String,
    /// Opens the panel editor (or, for now, the text dialog).
    on_open_panel_editor: Callback<PanelEditorStart>,
) -> impl IntoView {
    let sid = StoredValue::new(section_id.clone());
    let panels = Memo::new(move |_| {
        draft.with(|d| {
            find_section(d, &sid.get_value()).map_or_else(Vec::new, |s| reading_order(&s.panels))
        })
    });
    let swap = move |id: String, up: bool| {
        with_section(draft, &sid.get_value(), |s| {
            swap_with_neighbour(&mut s.panels, &id, up);
        });
    };
    let test_id = format!("spectra-edit-list-{section_id}");

    view! {
        <div data-testid=test_id>
            <Show
                when=move || panels.with(|p| !p.is_empty())
                fallback=|| view! {
                    <EmptyState
                        message="No panels in this section"
                        description="Add a panel or move one here on a wider screen."
                    />
                }
            >
                <List>
                    {move || {
                        let rows = panels.get();
                        let last = rows.len().saturating_sub(1);
                        rows.into_iter().enumerate().map(|(i, panel)| view! {
                            <PhoneRow
                                panel=panel
                                index=i
                                last=last
                                on_swap=Callback::new(move |(id, up): (String, bool)| swap(id, up))
                                on_remove=Callback::new(move |id: String| {
                                    with_section(draft, &sid.get_value(), |s| s.panels.retain(|p| p.id != id));
                                })
                                on_open_panel_editor=on_open_panel_editor
                            />
                        }).collect_view()
                    }}
                </List>
            </Show>
        </div>
    }
}

/// One panel's row in the phone list.
#[component]
fn PhoneRow(
    panel: Panel,
    index: usize,
    last: usize,
    on_swap: Callback<(String, bool)>,
    on_remove: Callback<String>,
    on_open_panel_editor: Callback<PanelEditorStart>,
) -> impl IntoView {
    let id = StoredValue::new(panel.id.clone());
    let title = if panel.title.is_empty() {
        "Untitled panel".to_string()
    } else {
        panel.title.clone()
    };
    let label = StoredValue::new(title.clone());
    let text = is_text(&panel);
    let row_id = format!("spectra-edit-row-{}", panel.id);
    view! {
        <div data-testid=row_id data-index=index.to_string()>
            <ListItem>
                <Body1>{title}</Body1>
                <ListItemTrailing slot>
                    <Flex gap=SpacingSize::Size20.flex_gap()>
                        <span data-testid="spectra-edit-row-up">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Small
                                icon=icondata::AiUpOutlined
                                disabled=index == 0
                                attr:aria-label=move || format!("Move {} up", label.get_value())
                                on_click=Callback::new(move |_| on_swap.run((id.get_value(), true)))
                            />
                        </span>
                        <span data-testid="spectra-edit-row-down">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Small
                                icon=icondata::AiDownOutlined
                                disabled=index == last
                                attr:aria-label=move || format!("Move {} down", label.get_value())
                                on_click=Callback::new(move |_| on_swap.run((id.get_value(), false)))
                            />
                        </span>
                        <PanelEditButton
                            panel_id=id.get_value()
                            title=Signal::derive(move || label.get_value())
                            text=text
                            on_open_panel_editor=on_open_panel_editor
                        />
                        <span data-testid="spectra-edit-panel-remove">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                size=ButtonSize::Small
                                icon=icondata::AiDeleteOutlined
                                attr:aria-label=move || format!("Remove {}", label.get_value())
                                on_click=Callback::new(move |_| on_remove.run(id.get_value()))
                            />
                        </span>
                    </Flex>
                </ListItemTrailing>
            </ListItem>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use spectra_backend::dashboards::ChartKind;

    use super::*;

    fn at(row: u16, col: u8, span: u8, height: u16) -> Layout {
        Layout {
            col,
            span,
            row,
            height,
        }
    }

    fn panel(id: &str, layout: Layout) -> Panel {
        Panel {
            id: id.into(),
            title: id.into(),
            source: PanelSource::Text {
                markdown: String::new(),
            },
            chart: ChartKind::Table,
            measure: None,
            bucket: None,
            group_by: Vec::new(),
            filters: Vec::new(),
            table: None,
            layout,
            keep_window: None,
        }
    }

    fn layout_of(panels: &[Panel], id: &str) -> Layout {
        panels.iter().find(|p| p.id == id).unwrap().layout
    }

    #[test]
    fn clamp_keeps_panels_inside_twelve_columns() {
        assert_eq!(clamp_layout(at(0, 10, 6, 0)), at(0, 6, 6, 1));
        assert_eq!(clamp_layout(at(2, 0, 0, 3)), at(2, 0, 1, 3));
        assert_eq!(clamp_layout(at(0, 3, 20, 2)), at(0, 0, 12, 2));
    }

    #[test]
    fn place_pushes_covered_panels_down_in_a_cascade() {
        let mut panels = vec![
            panel("a", at(0, 0, 6, 2)),
            panel("b", at(0, 6, 6, 2)),
            panel("c", at(2, 6, 6, 2)),
        ];
        assert!(place(&mut panels, "a", at(0, 6, 6, 2)));
        assert_eq!(layout_of(&panels, "a"), at(0, 6, 6, 2));
        assert_eq!(layout_of(&panels, "b"), at(2, 6, 6, 2));
        assert_eq!(layout_of(&panels, "c"), at(4, 6, 6, 2));
    }

    #[test]
    fn place_leaves_panels_it_does_not_touch() {
        let mut panels = vec![panel("a", at(0, 0, 4, 2)), panel("b", at(0, 6, 6, 2))];
        assert!(place(&mut panels, "a", at(0, 1, 4, 2)));
        assert_eq!(layout_of(&panels, "b"), at(0, 6, 6, 2));
    }

    #[test]
    fn place_unknown_panel_changes_nothing() {
        let mut panels = vec![panel("a", at(0, 0, 4, 2))];
        assert!(!place(&mut panels, "zz", at(3, 3, 3, 3)));
        assert_eq!(layout_of(&panels, "a"), at(0, 0, 4, 2));
    }

    #[test]
    fn arrows_move_and_shift_arrows_resize() {
        let l = at(1, 2, 4, 2);
        assert_eq!(nudge(l, "ArrowRight", false), Some(at(1, 3, 4, 2)));
        assert_eq!(nudge(l, "ArrowLeft", false), Some(at(1, 1, 4, 2)));
        assert_eq!(nudge(l, "ArrowDown", false), Some(at(2, 2, 4, 2)));
        assert_eq!(nudge(l, "ArrowUp", false), Some(at(0, 2, 4, 2)));
        assert_eq!(nudge(l, "ArrowRight", true), Some(at(1, 2, 5, 2)));
        assert_eq!(nudge(l, "ArrowUp", true), Some(at(1, 2, 4, 1)));
        assert_eq!(nudge(l, "Enter", false), None);
    }

    #[test]
    fn arrows_stop_at_the_grid_edges() {
        assert_eq!(
            nudge(at(0, 0, 4, 1), "ArrowLeft", false),
            Some(at(0, 0, 4, 1))
        );
        assert_eq!(
            nudge(at(0, 0, 4, 1), "ArrowUp", false),
            Some(at(0, 0, 4, 1))
        );
        assert_eq!(
            nudge(at(0, 8, 4, 1), "ArrowRight", false),
            Some(at(0, 8, 4, 1))
        );
        assert_eq!(
            nudge(at(0, 0, 1, 1), "ArrowLeft", true),
            Some(at(0, 0, 1, 1))
        );
        assert_eq!(nudge(at(0, 0, 4, 1), "ArrowUp", true), Some(at(0, 0, 4, 1)));
    }

    #[test]
    fn drag_snaps_to_whole_cells() {
        let start = at(1, 2, 4, 2);
        assert_eq!(drag_target(start, 3, 1, false), at(2, 5, 4, 2));
        assert_eq!(drag_target(start, -5, -4, false), at(0, 0, 4, 2));
        assert_eq!(drag_target(start, 2, 2, true), at(1, 2, 6, 4));
        assert_eq!(drag_target(start, 20, 0, true), at(1, 0, 12, 2));
    }

    #[test]
    fn swapping_neighbours_keeps_sizes() {
        let mut panels = vec![panel("a", at(0, 0, 4, 2)), panel("b", at(0, 4, 8, 2))];
        assert!(swap_with_neighbour(&mut panels, "b", true));
        assert_eq!(layout_of(&panels, "b"), at(0, 0, 8, 2));
        assert_eq!(layout_of(&panels, "a"), at(0, 8, 4, 2));
        assert!(!swap_with_neighbour(&mut panels, "b", true));
        assert!(!swap_with_neighbour(&mut panels, "a", false));
    }

    #[test]
    fn swapping_settles_overlaps() {
        let mut panels = vec![panel("a", at(0, 0, 12, 2)), panel("b", at(2, 6, 6, 2))];
        assert!(swap_with_neighbour(&mut panels, "b", true));
        assert_eq!(layout_of(&panels, "b"), at(0, 0, 6, 2));
        assert_eq!(layout_of(&panels, "a"), at(2, 0, 12, 2));
    }

    #[test]
    fn sections_move_up_and_down() {
        let section = |id: &str| Section {
            id: id.into(),
            title: id.into(),
            collapsed: false,
            panels: Vec::new(),
        };
        let mut sections = vec![section("a"), section("b"), section("c")];
        assert!(move_section(&mut sections, "c", true));
        assert!(!move_section(&mut sections, "a", true));
        let order: Vec<_> = sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(order, ["a", "c", "b"]);
    }

    #[test]
    fn sections_move_to_an_index() {
        let section = |id: &str| Section {
            id: id.into(),
            title: id.into(),
            collapsed: false,
            panels: Vec::new(),
        };
        let order = |s: &[Section]| s.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        let mut sections = vec![section("a"), section("b"), section("c")];
        assert!(move_section_to(&mut sections, "a", 2));
        assert_eq!(order(&sections), ["b", "c", "a"]);
        assert!(move_section_to(&mut sections, "a", 0));
        assert_eq!(order(&sections), ["a", "b", "c"]);
        assert!(!move_section_to(&mut sections, "b", 1));
        assert!(!move_section_to(&mut sections, "zz", 0));
        assert!(move_section_to(&mut sections, "a", 99));
        assert_eq!(order(&sections), ["b", "c", "a"]);
    }

    #[test]
    fn next_free_row_is_below_everything() {
        let panels = vec![panel("a", at(0, 0, 4, 2)), panel("b", at(1, 4, 4, 4))];
        assert_eq!(next_free_row(&panels), 5);
        assert_eq!(next_free_row(&[]), 0);
    }
}
