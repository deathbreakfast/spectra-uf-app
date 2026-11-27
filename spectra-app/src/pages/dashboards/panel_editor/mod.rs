//! The panel editor: build a chart or table panel, or start from a pasted Spectra query link.
//!
//! Settings sit on the left and a live preview on the right. The preview runs the unsaved
//! panel through [`query_dashboard_panel`](crate::server::query_dashboard_panel) on the dashboard's range, filters and variables,
//! so it shows what the panel will show. A pasted link is decoded in the browser, then
//! [`panel_source_access`](crate::server::panel_source_access) checks that the viewer can read its table and returns the fields
//! the link is checked against. Nothing is guessed: a field the table doesn't have is named
//! and left out.

mod link;
mod link_box;
mod preview;
mod settings;
mod source_settings;
mod table_filters;

use leptos::prelude::*;
use orbital::components::{
    Caption1Strong, Divider, DrawerBody, DrawerHeader, DrawerHeaderTitle, DrawerHeaderTitleAction,
    DrawerSize, Grid, GridConfig, GridItem, GridItemConfig, OverlayDrawer, SpacingSize,
};
use orbital::primitives::{
    use_breakpoint_up, Breakpoint, Button, ButtonAppearance, Flex, FlexJustify,
};
use spectra_backend::dashboards::view::ViewState;
use spectra_backend::dashboards::{Dashboard, Panel, PanelSource, Window};

pub(super) use link::panel_to_link;

use self::link::{blank_panel, LinkImport};
use self::link_box::LinkBox;
use self::preview::{preview_query, PanelPreview};
use self::settings::Settings;
use self::source_settings::SourceSettings;
use super::edit::{add_panel, PanelEditorStart};
use crate::server::{panel_source_access, PanelSourceDetail, PanelSources, SourceField};

/// The panel editor drawer. `start` says how it opened; the body is built fresh each time
/// the drawer opens.
#[component]
pub(super) fn PanelEditor(
    open: RwSignal<bool>,
    start: RwSignal<Option<PanelEditorStart>>,
    draft: RwSignal<Dashboard>,
    state: Memo<ViewState>,
) -> impl IntoView {
    let heading = move || {
        if matches!(start.get(), Some(PanelEditorStart::Edit(_))) {
            "Edit panel"
        } else {
            "Add panel"
        }
    };
    view! {
        <OverlayDrawer open=open close_on_esc=true size=DrawerSize::Large>
            <DrawerHeader>
                <DrawerHeaderTitle>
                    {heading}
                    <DrawerHeaderTitleAction slot>
                        <Button
                            appearance=ButtonAppearance::Subtle
                            icon=icondata::AiCloseOutlined
                            attr:aria-label="Close"
                            on_click=Callback::new(move |_| open.set(false))
                        />
                    </DrawerHeaderTitleAction>
                </DrawerHeaderTitle>
            </DrawerHeader>
            <DrawerBody>
                <Show when=move || open.get()>
                    {move || start.get_untracked().map(|start| view! {
                        <PanelEditorBody start=start open=open draft=draft state=state />
                    })}
                </Show>
            </DrawerBody>
        </OverlayDrawer>
    }
}
/// The picked source's fields, refetched when the source changes. Stale answers are
/// dropped.
fn source_fields(settings: Settings) -> RwSignal<Option<Vec<SourceField>>> {
    let fields = RwSignal::new(None::<Vec<SourceField>>);
    let seq = StoredValue::new(0u32);
    Effect::new(move |_| {
        let name = settings.source.get();
        fields.set(None);
        if name.is_empty() {
            return;
        }
        let n = seq.get_value() + 1;
        seq.set_value(n);
        leptos::task::spawn_local_scoped(async move {
            let result = panel_source_access(Some(name)).await;
            if seq.get_value() != n {
                return;
            }
            if let Ok(PanelSources {
                detail: Some(PanelSourceDetail::Readable { fields: f }),
                ..
            }) = result
            {
                fields.set(f);
            }
        });
    });
    fields
}

/// Put `panel` in the draft: in place of `old` when editing, otherwise as a new panel.
fn apply_to_draft(draft: RwSignal<Dashboard>, old: Option<&Panel>, panel: Panel) {
    match old {
        Some(old) => draft.update(|d| {
            if let Some(slot) = d
                .sections
                .iter_mut()
                .flat_map(|s| s.panels.iter_mut())
                .find(|p| p.id == old.id)
            {
                *slot = panel;
            }
        }),
        None => draft.update(|d| {
            add_panel(d, panel);
        }),
    }
}

#[component]
fn PanelEditorBody(
    start: PanelEditorStart,
    open: RwSignal<bool>,
    draft: RwSignal<Dashboard>,
    state: Memo<ViewState>,
) -> impl IntoView {
    let existing = match &start {
        PanelEditorStart::Edit(id) => draft.with_untracked(|d| {
            d.sections
                .iter()
                .flat_map(|s| &s.panels)
                .find(|p| &p.id == id)
                .cloned()
        }),
        _ => None,
    };
    let base = StoredValue::new(existing.clone());
    let dashboard = StoredValue::new(draft.get_untracked());
    let settings = Settings::new();
    if let Some(panel) = &existing {
        settings.load(panel);
    }
    settings.follow_source_picks();
    let wide = use_breakpoint_up(Breakpoint::Md);

    // Readable sources for the picker, loaded once per opening.
    let sources = LocalResource::new(move || async move {
        panel_source_access(None)
            .await
            .map(|a| a.sources)
            .map_err(|e| super::panel::failure_reason(&e))
    });
    let fields = source_fields(settings);

    let blocked = RwSignal::new(false);
    let on_import = Callback::new(move |import: LinkImport| {
        let mut panel = import.panel;
        panel.title = settings.title.get_untracked();
        settings.load(&panel);
    });
    let on_keep_window =
        Callback::new(move |window: Option<Window>| settings.keep_window.set(window));
    let query = preview_query(settings, blocked.into(), dashboard, state);

    let can_apply = Memo::new(move |_| !blocked.get() && !settings.source.with(String::is_empty));
    let apply = Callback::new(move |_| {
        let old = base.get_value();
        let (id, layout) = old.as_ref().map_or_else(
            || {
                (
                    String::new(),
                    blank_panel(PanelSource::Text {
                        markdown: String::new(),
                    })
                    .layout,
                )
            },
            |p| (p.id.clone(), p.layout),
        );
        let Some(mut panel) = settings.build(&id, layout) else {
            return;
        };
        if blocked.get_untracked() {
            return;
        }
        if panel.title.is_empty() {
            panel.title = settings.source.get_untracked();
        }
        apply_to_draft(draft, old.as_ref(), panel);
        open.set(false);
    });

    let settings_pane = view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            <LinkBox
                from_link=start == PanelEditorStart::FromLink
                blocked=blocked
                on_import=on_import
                on_keep_window=on_keep_window
            />
            <Divider />
            <Caption1Strong>"Or set it up yourself"</Caption1Strong>
            <SourceSettings settings=settings sources=sources fields=fields />
        </Flex>
    };
    let preview_pane = view! { <PanelPreview query=query dashboard=dashboard state=state /> };
    // Settings and preview side by side from Md up; stacked on a phone.
    let body = if wide.get_untracked() {
        view! {
            <Grid config=GridConfig { cols: 12, x_gap: 24, y_gap: 24 }>
                <GridItem config=GridItemConfig::span(5)>{settings_pane}</GridItem>
                <GridItem config=GridItemConfig::span(7)>{preview_pane}</GridItem>
            </Grid>
        }
        .into_any()
    } else {
        view! {
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                {settings_pane}
                {preview_pane}
            </Flex>
        }
        .into_any()
    };

    view! {
        <div data-testid="spectra-panel-editor">
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                {body}
                <Flex justify=FlexJustify::End gap=SpacingSize::Size80.flex_gap()>
                    <Button appearance=ButtonAppearance::Subtle on_click=Callback::new(move |_| open.set(false))>
                        "Cancel"
                    </Button>
                    <span data-testid="spectra-panel-editor-apply">
                        <Button
                            appearance=ButtonAppearance::Primary
                            disabled=Signal::derive(move || !can_apply.get())
                            on_click=apply
                        >
                            "Apply"
                        </Button>
                    </span>
                </Flex>
            </Flex>
        </div>
    }
}
