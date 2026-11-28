//! The text panel drawer: a title and Markdown, with the panel as it will look beside them.

use leptos::prelude::*;
use orbital::components::{
    Caption1Strong, DrawerBody, DrawerHeader, DrawerHeaderTitle, DrawerHeaderTitleAction,
    DrawerSize, Grid, GridConfig, GridItem, GridItemConfig, MarkdownEditor, OverlayDrawer,
    SpacingSize,
};
use orbital::primitives::{
    use_breakpoint_up, Breakpoint, Button, ButtonAppearance, Card, CardContent, Field, Flex,
    FlexJustify, Input, InputBind,
};
use spectra_backend::dashboards::{Dashboard, PanelSource};

use super::edit::add_text_panel;
use super::panels::TextPanel;

/// Title and Markdown for a text panel. With `target` set it edits that panel, otherwise
/// Apply adds a new one.
#[component]
pub(super) fn TextPanelDrawer(
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
    let wide = use_breakpoint_up(Breakpoint::Md);

    let editor = move || {
        view! {
            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                <Field label="Title">
                    <div data-testid="spectra-edit-text-title">
                        <Input bind=InputBind::new(title) />
                    </div>
                </Field>
                <Field label="Text (Markdown)" required=true>
                    <div data-testid="spectra-edit-text-markdown">
                        <MarkdownEditor value=markdown aria_label="Text (Markdown)".to_string() />
                    </div>
                </Field>
            </Flex>
        }
    };
    let rendered = move || {
        view! {
            <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                <Caption1Strong>"How it will look"</Caption1Strong>
                <div data-testid="spectra-edit-text-preview">
                    <Card>
                        <CardContent>
                            <Caption1Strong>{move || title.get()}</Caption1Strong>
                            {move || view! { <TextPanel markdown=markdown.get() /> }}
                        </CardContent>
                    </Card>
                </div>
            </Flex>
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
                    <div data-testid="spectra-edit-text-dialog">
                        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                            {move || if wide.get() {
                                view! {
                                    <Grid config=GridConfig { cols: 12, x_gap: 24, y_gap: 24 }>
                                        <GridItem config=GridItemConfig::span(6)>{editor()}</GridItem>
                                        <GridItem config=GridItemConfig::span(6)>{rendered()}</GridItem>
                                    </Grid>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                                        {editor()}
                                        {rendered()}
                                    </Flex>
                                }
                                .into_any()
                            }}
                            <Flex justify=FlexJustify::End gap=SpacingSize::Size80.flex_gap()>
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
                            </Flex>
                        </Flex>
                    </div>
                </Show>
            </DrawerBody>
        </OverlayDrawer>
    }
}
