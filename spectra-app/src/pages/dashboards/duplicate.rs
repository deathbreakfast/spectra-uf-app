//! The Duplicate dialog: name the copy, then open it.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use orbital::components::SpacingSize;
use orbital::primitives::{
    Body1, Button, ButtonAppearance, Dialog, DialogActions, DialogBody, DialogContent,
    DialogSurface, DialogTitle, Field, Flex, Input, InputBind, MessageBar, MessageBarIntent,
};
use spectra_backend::spectra_dashboard_path;

use super::{action_error_text, duplicate_name};
use crate::server::duplicate_dashboard;

/// Copies a dashboard into a new one the viewer owns and opens it.
#[component]
pub(super) fn DuplicateDialog(
    /// Whether the dialog is open.
    open: RwSignal<bool>,
    /// Dashboard to copy.
    dashboard_id: String,
    /// Its title, which the copy's name starts from.
    title: String,
) -> impl IntoView {
    let name = RwSignal::new(duplicate_name(&title));
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if open.get() {
            name.set(duplicate_name(&title));
            error.set(None);
        }
    });

    let navigate = use_navigate();
    let duplicate = Action::new(move |input: &(String, String)| {
        let (id, title) = input.clone();
        async move { duplicate_dashboard(id, title).await }
    });
    let pending = duplicate.pending();
    Effect::new(move |_| {
        if let Some(result) = duplicate.value().get() {
            match result {
                Ok(copy) => {
                    open.set(false);
                    navigate(
                        &spectra_dashboard_path(&copy.id),
                        NavigateOptions::default(),
                    );
                }
                Err(e) => error.set(Some(format!(
                    "Couldn't duplicate this dashboard: {}",
                    action_error_text(&e)
                ))),
            }
        }
    });
    let on_submit = Callback::new(move |_| {
        error.set(None);
        duplicate.dispatch((
            dashboard_id.clone(),
            name.get_untracked().trim().to_string(),
        ));
    });

    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-duplicate-dialog">
                    <DialogBody>
                        <DialogTitle>"Duplicate dashboard"</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Field label="Name" required=true>
                                    <div data-testid="spectra-duplicate-name">
                                        <Input bind=InputBind::new(name) />
                                    </div>
                                </Field>
                                <span data-testid="spectra-duplicate-note">
                                    <Body1>"The copy is yours and isn't shared with anyone yet."</Body1>
                                </span>
                                {move || error.get().map(|message| view! {
                                    <div data-testid="spectra-duplicate-error">
                                        <MessageBar intent=MessageBarIntent::Error>{message}</MessageBar>
                                    </div>
                                })}
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button on_click=Callback::new(move |_| open.set(false))>"Cancel"</Button>
                            <span data-testid="spectra-duplicate-submit">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    loading=pending
                                    disabled=Signal::derive(move || name.with(|n| n.trim().is_empty()))
                                    on_click=on_submit
                                >
                                    "Duplicate"
                                </Button>
                            </span>
                        </DialogActions>
                    </DialogBody>
                </div>
            </DialogSurface>
        </Dialog>
    }
}
