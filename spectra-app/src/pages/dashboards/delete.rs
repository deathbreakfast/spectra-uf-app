//! The Delete dialog: says who loses access, and stays disabled until the name is typed.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use orbital::components::SpacingSize;
use orbital::primitives::{
    Body1, Button, ButtonAppearance, Dialog, DialogActions, DialogBody, DialogContent,
    DialogSurface, DialogTitle, Field, Flex, Input, InputBind, MessageBar, MessageBarIntent,
};
use spectra_backend::dashboards::DashboardAccess;

use super::share::{label_map, principal_label, share_directory};
use super::{action_error_text, delete_access_text};
use crate::server::delete_dashboard;

/// Deletes a dashboard for good. Shown only to the owner.
#[component]
pub(super) fn DeleteDialog(
    /// Whether the dialog is open.
    open: RwSignal<bool>,
    /// Dashboard id.
    dashboard_id: String,
    /// Its title, which must be typed to confirm.
    title: String,
    /// Who has access now.
    access: RwSignal<DashboardAccess>,
) -> impl IntoView {
    let typed = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if open.get() {
            typed.set(String::new());
            error.set(None);
        }
    });

    // Names for the people and groups who lose access.
    let directory = LocalResource::new(move || {
        let load = open.get() && access.with(|a| !a.editors.is_empty() || !a.viewers.is_empty());
        async move {
            if load {
                share_directory(None).await.unwrap_or_default()
            } else {
                Vec::new()
            }
        }
    });
    let access_text = move || {
        let labels = directory
            .get()
            .map_or_else(std::collections::HashMap::new, |d| label_map(&d));
        let names: Vec<String> = access.with(|a| {
            a.editors
                .iter()
                .chain(&a.viewers)
                .map(|p| principal_label(p, &labels))
                .collect()
        });
        delete_access_text(&names)
    };

    let navigate = use_navigate();
    let delete = Action::new(move |id: &String| {
        let id = id.clone();
        async move { delete_dashboard(id).await }
    });
    let pending = delete.pending();
    Effect::new(move |_| {
        if let Some(result) = delete.value().get() {
            match result {
                Ok(()) => {
                    open.set(false);
                    navigate("/spectra/dashboards", NavigateOptions::default());
                }
                Err(e) => error.set(Some(format!(
                    "Couldn't delete this dashboard: {}",
                    action_error_text(&e)
                ))),
            }
        }
    });

    let matches = {
        let title = title.clone();
        Signal::derive(move || typed.with(|t| *t == title))
    };
    let on_delete = Callback::new(move |_| {
        if matches.get_untracked() {
            error.set(None);
            delete.dispatch(dashboard_id.clone());
        }
    });

    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-delete-dialog">
                    <DialogBody>
                        <DialogTitle>{format!("Delete {title}?")}</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <span data-testid="spectra-delete-access">
                                    <Body1>{access_text}</Body1>
                                </span>
                                <Field label="Type the dashboard's name to confirm" required=true>
                                    <div data-testid="spectra-delete-name">
                                        <Input bind=InputBind::new(typed) />
                                    </div>
                                </Field>
                                {move || error.get().map(|message| view! {
                                    <div data-testid="spectra-delete-error">
                                        <MessageBar intent=MessageBarIntent::Error>{message}</MessageBar>
                                    </div>
                                })}
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button on_click=Callback::new(move |_| open.set(false))>"Cancel"</Button>
                            <span data-testid="spectra-delete-submit">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    loading=pending
                                    disabled=Signal::derive(move || !matches.get())
                                    on_click=on_delete
                                >
                                    "Delete dashboard"
                                </Button>
                            </span>
                        </DialogActions>
                    </DialogBody>
                </div>
            </DialogSurface>
        </Dialog>
    }
}
