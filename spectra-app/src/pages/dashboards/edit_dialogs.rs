//! The edit page's dialogs: confirmations and Compare versions.

use leptos::prelude::*;
use orbital::components::{Caption1, DialogSize, List, ListItem, SpacingSize, Subtitle2};
use orbital::primitives::{
    Body1, Button, ButtonAppearance, Dialog, DialogActions, DialogBody, DialogContent,
    DialogSurface, DialogTitle, Flex, MessageBar, MessageBarIntent, Skeleton, SkeletonItem,
};
use spectra_backend::dashboards::Dashboard;

use super::action_error_text;
use super::edit::change_parts;
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
