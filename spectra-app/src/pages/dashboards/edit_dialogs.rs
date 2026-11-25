//! The edit page's dialogs: confirmations, Compare versions and the text panel editor.

use leptos::prelude::*;
use orbital::components::{Caption1, DialogSize, List, ListItem, SpacingSize, Subtitle2};
use orbital::primitives::{
    Body1, Button, ButtonAppearance, Dialog, DialogActions, DialogBody, DialogContent,
    DialogSurface, DialogTitle, Field, Flex, Input, InputBind, MessageBar, MessageBarIntent,
    Skeleton, SkeletonItem, Textarea, TextareaBind,
};
use spectra_backend::dashboards::{Dashboard, PanelSource};

use super::action_error_text;
use super::edit::{add_text_panel, change_parts};
use super::edit_save::Conflict;
use super::variables::capitalise;
use crate::server::get_dashboard;

/// A small confirmation Dialog with Keep editing and one confirm button.
#[component]
pub(super) fn ConfirmDialog(
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
pub(super) fn CompareDialog(
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
pub(super) fn TextPanelDialog(
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
