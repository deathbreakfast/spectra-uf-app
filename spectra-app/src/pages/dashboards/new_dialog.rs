//! The New dashboard dialog: a name, a description, and blank or a copy of another
//! dashboard.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use orbital::components::SpacingSize;
use orbital::primitives::{
    Button, ButtonAppearance, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface,
    DialogTitle, Field, Flex, Input, InputAppearance, InputBind, MessageBar, MessageBarIntent,
    Radio, RadioGroup, Select, SelectBind, Textarea, TextareaBind,
};
use spectra_backend::dashboards::DashboardSummary;
use spectra_backend::spectra_dashboard_path;

use super::action_error_text;
use crate::server::create_dashboard;

const START_BLANK: &str = "blank";
const START_COPY: &str = "copy";

/// Opens with New dashboard. Create opens the new dashboard.
#[component]
pub(super) fn NewDashboardDialog(
    /// Whether the dialog is open.
    open: RwSignal<bool>,
    /// Dashboards the viewer can read, to copy from.
    #[prop(into)]
    sources: Signal<Vec<DashboardSummary>>,
) -> impl IntoView {
    let title = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let start = RwSignal::new(Some(START_BLANK.to_string()));
    let copy_from = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let navigate = use_navigate();

    // Default the copy source to the first dashboard once the list is in.
    Effect::new(move |_| {
        let first = sources.with(|s| s.first().map(|d| d.id.clone()));
        if copy_from.with_untracked(String::is_empty) {
            if let Some(first) = first {
                copy_from.set(first);
            }
        }
    });

    let create = Action::new(move |input: &(String, String, Option<String>)| {
        let (title, description, from) = input.clone();
        async move { create_dashboard(title, description, from).await }
    });
    let creating = create.pending();
    Effect::new(move |_| {
        if let Some(result) = create.value().get() {
            match result {
                Ok(dashboard) => {
                    open.set(false);
                    // The edit route lands with edit mode; until then open the dashboard.
                    navigate(
                        &spectra_dashboard_path(&dashboard.id),
                        NavigateOptions::default(),
                    );
                }
                Err(e) => error.set(Some(action_error_text(&e))),
            }
        }
    });

    let on_create = move |_| {
        error.set(None);
        let from = (start.get_untracked().as_deref() == Some(START_COPY))
            .then(|| copy_from.get_untracked())
            .filter(|id| !id.is_empty());
        create.dispatch((title.get_untracked(), description.get_untracked(), from));
    };

    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-new-dashboard-dialog">
                    <DialogBody>
                        <DialogTitle>"New dashboard"</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Field label="Name" required=true>
                                    <div data-testid="spectra-new-dashboard-name">
                                        <Input
                                            bind=InputBind::new(title)
                                            appearance=InputAppearance::with_placeholder("Checkout health")
                                        />
                                    </div>
                                </Field>
                                <Field label="Description">
                                    <div data-testid="spectra-new-dashboard-description">
                                        <Textarea bind=TextareaBind::new(description) />
                                    </div>
                                </Field>
                                <Field label="Start from">
                                    <div data-testid="spectra-new-dashboard-start">
                                        <RadioGroup bind=start>
                                            <Radio value=START_BLANK label="A blank dashboard" />
                                            <Radio value=START_COPY label="A copy of a dashboard I can read" />
                                        </RadioGroup>
                                    </div>
                                </Field>
                                <Show when=move || start.get().as_deref() == Some(START_COPY)>
                                    <Field label="Copy">
                                        <div data-testid="spectra-new-dashboard-copy-from">
                                            <Select bind=SelectBind::new(copy_from)>
                                                {move || sources.get().into_iter().map(|d| view! {
                                                    <option value=d.id>{d.title}</option>
                                                }).collect_view()}
                                            </Select>
                                        </div>
                                    </Field>
                                </Show>
                                {move || error.get().map(|e| view! {
                                    <div data-testid="spectra-new-dashboard-error">
                                        <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
                                    </div>
                                })}
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button on_click=Callback::new(move |_| open.set(false))>"Cancel"</Button>
                            <span data-testid="spectra-new-dashboard-create">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    loading=creating
                                    disabled=Signal::derive(move || title.with(|t| t.trim().is_empty()))
                                    on_click=Callback::new(on_create)
                                >
                                    "Create"
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
    use crate::server::to_dashboard_server_error;
    use spectra_backend::dashboards::DashboardError;

    #[test]
    fn validation_errors_show_their_own_message() {
        let err = to_dashboard_server_error(&DashboardError::validation("Title is required"));
        assert_eq!(action_error_text(&err), "Title is required");
    }
}
