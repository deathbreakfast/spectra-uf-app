//! "Start from a query link": the box, the decode and source check, and the bar that says
//! what was filled in.

use leptos::prelude::*;
use orbital::components::{
    Caption1, Checkbox, FieldValidationState, FormHint, SpacingSize, TextareaAppearance,
    TextareaRule, TextareaRuleTrigger,
};
use orbital::primitives::{
    Field, Flex, Link, MessageBar, MessageBarActions, MessageBarBody, MessageBarIntent,
    MessageBarLayout, MessageBarTitle, Textarea, TextareaBind,
};
use spectra_backend::dashboards::Window;
use spectra_backend::query_url::decode;

use super::super::panel::failure_reason;
use super::link::{
    dropped_filter_message, filled_in_summary, link_error, panel_from_link, window_text, LinkImport,
};
use crate::server::{panel_source_access, PanelSourceDetail, PanelSources};

/// What the link box last did.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LinkState {
    /// Nothing pasted, or text that isn't a link.
    Idle,
    /// Checking the link's source with the server.
    Checking,
    /// Settings were filled in from the link.
    Filled {
        summary: String,
        gaps: Vec<String>,
        window: Window,
    },
    /// The viewer can't read the link's source; nothing was filled in.
    Unreadable {
        source: String,
        permission: String,
        href: String,
    },
    /// The check itself failed.
    Failed(String),
}
/// Focus the first textarea inside `wrapper`.
fn focus_textarea_in(wrapper: &leptos::web_sys::HtmlDivElement) {
    use wasm_bindgen::JsCast;
    if let Ok(Some(el)) = wrapper.query_selector("textarea") {
        if let Ok(el) = el.dyn_into::<leptos::web_sys::HtmlElement>() {
            // A refused focus leaves the box as it is.
            let _ = el.focus();
        }
    }
}
/// What the server said about a decoded link's source, as the next [`LinkState`].
fn checked_state(
    result: Result<PanelSources, ServerFnError>,
    import: impl FnOnce(Option<Vec<crate::server::SourceField>>) -> LinkImport,
    source: String,
) -> (LinkState, Option<LinkImport>) {
    match result {
        Ok(PanelSources {
            detail: Some(PanelSourceDetail::Readable { fields }),
            ..
        }) => {
            let import = import(fields);
            let mut gaps: Vec<String> = import.unknown.iter().map(|u| u.message(&source)).collect();
            gaps.extend(import.dropped_filters.iter().map(dropped_filter_message));
            let state = LinkState::Filled {
                summary: filled_in_summary(&import.panel),
                gaps,
                window: import.window,
            };
            (state, Some(import))
        }
        Ok(PanelSources {
            detail:
                Some(PanelSourceDetail::Unreadable {
                    permission,
                    request_access_href,
                }),
            ..
        }) => (
            LinkState::Unreadable {
                source,
                permission,
                href: request_access_href,
            },
            None,
        ),
        Ok(_) => (
            LinkState::Failed("the server didn't describe the link's source".into()),
            None,
        ),
        Err(err) => (LinkState::Failed(failure_reason(&err)), None),
    }
}

/// The link box. A pasted link is decoded at once; when the viewer can read its source the
/// import goes to `on_import`, and `blocked` is set while the source is unreadable.
#[component]
pub(super) fn LinkBox(
    /// Opened from Add panel › From a query link: focus the box and show the caption.
    from_link: bool,
    /// True while the link names a source the viewer can't read.
    blocked: RwSignal<bool>,
    /// Gets the settings a readable link describes.
    on_import: Callback<LinkImport>,
    /// Gets the link's window when the checkbox is ticked, `None` when it's cleared.
    on_keep_window: Callback<Option<Window>>,
) -> impl IntoView {
    let link_text = RwSignal::new(String::new());
    let link_state = RwSignal::new(LinkState::Idle);
    let keep = RwSignal::new(false);
    let seq = StoredValue::new(0u32);
    let set_state = move |next: LinkState| {
        blocked.set(matches!(next, LinkState::Unreadable { .. }));
        link_state.set(next);
    };
    let box_ref = NodeRef::<leptos::html::Div>::new();
    if from_link {
        Effect::new(move |_| {
            if let Some(wrapper) = box_ref.get() {
                request_animation_frame(move || focus_textarea_in(&wrapper));
            }
        });
    }
    Effect::new(move |_| {
        let text = link_text.get();
        let text = text.trim();
        if text.is_empty() {
            set_state(LinkState::Idle);
            return;
        }
        // Not a link: the Field shows why, and nothing changes.
        let Ok(decoded) = decode(text) else {
            return;
        };
        let n = seq.get_value() + 1;
        seq.set_value(n);
        set_state(LinkState::Checking);
        let source = decoded.link.source.name().to_string();
        leptos::task::spawn_local_scoped(async move {
            let result = panel_source_access(Some(source.clone())).await;
            if seq.get_value() != n {
                return;
            }
            let link = decoded.link;
            let (next, import) = checked_state(
                result,
                |fields| panel_from_link(&link, fields.as_deref()),
                source,
            );
            if let Some(import) = import {
                keep.set(false);
                on_import.run(import);
            }
            set_state(next);
        });
    });
    let on_keep = Callback::new(move |on: bool| {
        if let LinkState::Filled { window, .. } = link_state.get_untracked() {
            on_keep_window.run(on.then_some(window));
        }
    });
    let rule = TextareaRule::validator(move |value, _| {
        link_error(value).map_or(Ok(()), |message| Err(FieldValidationState::Error(message)))
    })
    .with_trigger(TextareaRuleTrigger::Input);

    view! {
        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
            <Field label="Start from a query link">
                <div data-testid="spectra-panel-editor-link" node_ref=box_ref>
                    <Textarea
                        bind=TextareaBind { value: link_text.into(), rules: vec![rule], ..Default::default() }
                        appearance=TextareaAppearance {
                            placeholder: "Paste a Spectra link".to_string().into(),
                            rows: Signal::from(Some(3)),
                            ..Default::default()
                        }
                    />
                </div>
            </Field>
            <FormHint>
                "Links from the event explorer, the metric explorer and saved queries fill in everything below."
            </FormHint>
            {from_link.then(|| view! {
                <span data-testid="spectra-panel-editor-link-caption">
                    <Caption1>
                        "Copy one with the link icon next to an explorer's title, or take one a teammate sent you."
                    </Caption1>
                </span>
            })}
            <LinkStateBar link_state=link_state keep=keep on_keep=on_keep />
        </Flex>
    }
}

/// The bar under the link box: filled in, filled in with gaps, unreadable, or failed.
#[component]
fn LinkStateBar(
    link_state: RwSignal<LinkState>,
    keep: RwSignal<bool>,
    on_keep: Callback<bool>,
) -> impl IntoView {
    move || match link_state.get() {
        LinkState::Idle | LinkState::Checking => ().into_any(),
        LinkState::Filled {
            summary,
            gaps,
            window,
        } => {
            let partial = !gaps.is_empty();
            view! {
                <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                    <div
                        data-testid="spectra-panel-editor-link-filled"
                        data-partial=partial.to_string()
                    >
                        <MessageBar
                            intent=if partial { MessageBarIntent::Warning } else { MessageBarIntent::Success }
                            layout=MessageBarLayout::Multiline
                        >
                            <MessageBarBody>
                                <MessageBarTitle>"Filled in from the link"</MessageBarTitle>
                                {summary}
                                {gaps.into_iter().map(|gap| view! {
                                    <div data-testid="spectra-panel-editor-link-gap">{gap}</div>
                                }).collect_view()}
                            </MessageBarBody>
                        </MessageBar>
                    </div>
                    <span data-testid="spectra-panel-editor-keep-window">
                        <Checkbox
                            checked=keep
                            label=format!("Keep the link's window instead ({})", window_text(window))
                            on_change=on_keep
                        />
                    </span>
                </Flex>
            }
            .into_any()
        }
        LinkState::Unreadable {
            source,
            permission,
            href,
        } => view! {
            <div data-testid="spectra-panel-editor-link-unreadable">
                <MessageBar intent=MessageBarIntent::Error layout=MessageBarLayout::Multiline>
                    <MessageBarBody>
                        <MessageBarTitle>{format!("You can't read {source}")}</MessageBarTitle>
                        {format!("Reading it needs {permission}. Nothing was filled in.")}
                    </MessageBarBody>
                    <MessageBarActions>
                        <span data-testid="spectra-panel-editor-request-access">
                            <Link href=href>"Request access"</Link>
                        </span>
                    </MessageBarActions>
                </MessageBar>
            </div>
        }
        .into_any(),
        LinkState::Failed(reason) => view! {
            <div data-testid="spectra-panel-editor-link-failed">
                <MessageBar intent=MessageBarIntent::Error>
                    <MessageBarBody>{format!("Couldn't check the link: {reason}")}</MessageBarBody>
                </MessageBar>
            </div>
        }
        .into_any(),
    }
}
