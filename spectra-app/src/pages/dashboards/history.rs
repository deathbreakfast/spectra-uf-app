//! Version history: the drawer listing every saved version, and the Restore confirmation.

use std::collections::HashMap;

use leptos::prelude::*;
use orbital::components::{Body1, Body1Strong, Caption1, SpacingSize};
use orbital::primitives::{
    change_view_fn, Badge, BadgeAppearance, BadgeColor, BadgeSize, Button, ButtonAppearance,
    ButtonSize, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle,
    DrawerBody, DrawerHeader, DrawerHeaderTitle, DrawerHeaderTitleAction, DrawerSize, Flex,
    FlexAlign, FlexWrap, HistoryActor, HistoryChange, HistoryChangeSlot, HistoryEntry,
    HistorySource, HistoryTimeline, MessageBar, MessageBarBody, MessageBarIntent, OverlayDrawer,
};
use spectra_backend::dashboards::Version;

use super::{action_error_text, author_name, restore_text};
use crate::server::{list_versions, restore_version};

/// A saved version as a timeline entry. The entry id is the version number.
fn history_entry(version: &Version) -> HistoryEntry {
    let name = author_name(version);
    HistoryEntry {
        id: version.number.to_string(),
        kind: "version".into(),
        changed_at: version.created_at,
        actor: HistoryActor::User {
            id: version.author.clone().unwrap_or_default(),
            display_name: name,
            href: None,
        },
        change: HistoryChange::Custom {
            summary: version.summary.clone(),
        },
    }
}

/// The history drawer.
#[component]
pub(super) fn HistoryDrawer(
    /// Whether the drawer is open.
    open: RwSignal<bool>,
    /// Dashboard id.
    dashboard_id: String,
    /// Whether the viewer can restore (edit) the dashboard.
    can_restore: bool,
    /// Open version N read-only.
    on_view: Callback<u32>,
    /// Ask to restore version N.
    on_restore: Callback<u32>,
) -> impl IntoView {
    // Loads each time the drawer opens; closing it or opening a dialog never touches the URL.
    let versions = LocalResource::new(move || {
        let (load, id) = (open.get(), dashboard_id.clone());
        async move {
            if load {
                list_versions(id).await.map_err(|e| action_error_text(&e))
            } else {
                Ok(Vec::new())
            }
        }
    });
    let entries = RwSignal::new(Vec::<HistoryEntry>::new());
    let by_number = StoredValue::new(HashMap::<u32, Version>::new());
    let newest = StoredValue::new(0u32);
    Effect::new(move |_| {
        if let Some(Ok(list)) = versions.get() {
            newest.set_value(list.iter().map(|v| v.number).max().unwrap_or(0));
            by_number.set_value(list.iter().map(|v| (v.number, v.clone())).collect());
            entries.set(list.iter().map(history_entry).collect());
        }
    });
    let loading = Signal::derive(move || versions.get().is_none());
    let error = move || versions.get().and_then(Result::err);

    let render = change_view_fn(move |ctx| {
        let number = ctx.entry.id.parse::<u32>().ok()?;
        let version = by_number.with_value(|m| m.get(&number).cloned())?;
        let current = number == newest.get_value();
        let via = version.via.clone();
        Some(
            view! {
                <div data-testid=format!("spectra-history-version-{number}")>
                    <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                        <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                            <Body1Strong>{format!("Version {number}")}</Body1Strong>
                            {current.then(|| view! {
                                <span data-testid="spectra-history-current">
                                    <Badge appearance=BadgeAppearance::Filled color=BadgeColor::Brand size=BadgeSize::Small>
                                        "Current"
                                    </Badge>
                                </span>
                            })}
                            {via.map(|via| view! {
                                <span data-testid="spectra-history-via">
                                    <Badge appearance=BadgeAppearance::Tint color=BadgeColor::Informative size=BadgeSize::Small>
                                        {format!("via {via}")}
                                    </Badge>
                                </span>
                            })}
                        </Flex>
                        <Body1>{version.summary.clone()}</Body1>
                        <Flex gap=SpacingSize::Size80.flex_gap()>
                            <span data-testid="spectra-history-view">
                                <Button
                                    size=ButtonSize::Small
                                    on_click=Callback::new(move |_| on_view.run(number))
                                >
                                    "View"
                                </Button>
                            </span>
                            {(can_restore && !current).then(|| view! {
                                <span data-testid="spectra-history-restore">
                                    <Button
                                        size=ButtonSize::Small
                                        on_click=Callback::new(move |_| on_restore.run(number))
                                    >
                                        "Restore"
                                    </Button>
                                </span>
                            })}
                        </Flex>
                    </Flex>
                </div>
            }
            .into_any(),
        )
    });

    let render = StoredValue::new(render);

    view! {
        <OverlayDrawer open=open close_on_esc=true size=DrawerSize::Medium>
            <DrawerHeader>
                <DrawerHeaderTitle>
                    "Version history"
                    <DrawerHeaderTitleAction slot>
                        <span data-testid="spectra-history-close">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                icon=icondata::AiCloseOutlined
                                attr:aria-label="Close version history"
                                on_click=Callback::new(move |_| open.set(false))
                            />
                        </span>
                    </DrawerHeaderTitleAction>
                </DrawerHeaderTitle>
            </DrawerHeader>
            <DrawerBody>
                <Show when=move || open.get()>
                    <div data-testid="spectra-history-drawer">
                        <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                            <Caption1>
                                "Every save is kept for 180 days. Agents' saves show who they acted for."
                            </Caption1>
                            {move || error().map(|message| view! {
                                <div data-testid="spectra-history-error">
                                    <MessageBar intent=MessageBarIntent::Error>
                                        <MessageBarBody>{format!("Couldn't load the history: {message}")}</MessageBarBody>
                                    </MessageBar>
                                </div>
                            })}
                            <HistoryTimeline
                                data_source=HistorySource::Client(entries)
                                loading=loading
                                max_height="70vh".to_string()
                            >
                                <HistoryChangeSlot slot render=render.get_value() />
                            </HistoryTimeline>
                        </Flex>
                    </div>
                </Show>
            </DrawerBody>
        </OverlayDrawer>
    }
}

/// Asks before saving an older version again as the newest one.
#[component]
pub(super) fn RestoreDialog(
    /// The version to restore; `Some` opens the dialog.
    target: RwSignal<Option<u32>>,
    /// Dashboard id.
    dashboard_id: String,
    /// Runs after the restore is saved.
    on_restored: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    Effect::new(move |_| {
        let wanted = target.get().is_some();
        if open.get_untracked() != wanted {
            open.set(wanted);
        }
    });
    // Closing with Cancel, Escape or the scrim clears the target.
    Effect::new(move |_| {
        if !open.get() && target.get_untracked().is_some() {
            target.set(None);
        }
    });
    let error = RwSignal::new(None::<String>);

    // The newest version, for the sentence that says what stays.
    let newest = LocalResource::new({
        let id = dashboard_id.clone();
        move || {
            let (wanted, id) = (target.get().is_some(), id.clone());
            async move {
                if wanted {
                    list_versions(id)
                        .await
                        .map(|v| v.into_iter().next())
                        .map_err(|e| {
                            log::warn!(
                                "Spectra dashboard: loading versions for Restore failed: {e}"
                            );
                            action_error_text(&e)
                        })
                } else {
                    Ok(None)
                }
            }
        }
    });
    let text = move || {
        let n = target.get()?;
        let newest = newest.get()?.ok().flatten()?;
        Some(restore_text(n, &newest))
    };
    let load_error = move || newest.get().and_then(Result::err);

    let restore = Action::new(move |input: &(String, u32)| {
        let (id, n) = input.clone();
        async move { restore_version(id, n).await }
    });
    let pending = restore.pending();
    Effect::new(move |_| {
        if let Some(result) = restore.value().get() {
            match result {
                Ok(_) => {
                    open.set(false);
                    on_restored.run(());
                }
                Err(e) => error.set(Some(format!(
                    "Couldn't restore this version: {}",
                    action_error_text(&e)
                ))),
            }
        }
    });
    let on_confirm = Callback::new(move |_| {
        if let Some(n) = target.get_untracked() {
            error.set(None);
            restore.dispatch((dashboard_id.clone(), n));
        }
    });

    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-restore-dialog">
                    <DialogBody>
                        <DialogTitle>
                            {move || target.get().map_or_else(String::new, |n| format!("Restore version {n}?"))}
                        </DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <span data-testid="spectra-restore-text">
                                    <Body1>{text}</Body1>
                                </span>
                                {move || load_error().map(|message| view! {
                                    <div data-testid="spectra-restore-load-error">
                                        <MessageBar intent=MessageBarIntent::Warning>
                                            <MessageBarBody>{format!("Couldn't load the history: {message}")}</MessageBarBody>
                                        </MessageBar>
                                    </div>
                                })}
                                {move || error.get().map(|message| view! {
                                    <div data-testid="spectra-restore-error">
                                        <MessageBar intent=MessageBarIntent::Error>{message}</MessageBar>
                                    </div>
                                })}
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button on_click=Callback::new(move |_| open.set(false))>"Cancel"</Button>
                            <span data-testid="spectra-restore-confirm">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    loading=pending
                                    on_click=on_confirm
                                >
                                    "Restore"
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

    #[test]
    fn entry_carries_number_author_and_summary() {
        let version = Version {
            number: 4,
            author: Some("admin".into()),
            via: Some("MCP".into()),
            created_at: Utc.with_ymd_and_hms(2026, 11, 3, 9, 0, 0).unwrap(),
            summary: "Added a panel".into(),
        };
        let entry = history_entry(&version);
        assert_eq!(entry.id, "4");
        assert!(
            matches!(entry.actor, HistoryActor::User { ref display_name, .. } if display_name == "admin")
        );
        assert_eq!(
            entry.change,
            HistoryChange::Custom {
                summary: "Added a panel".into()
            }
        );
    }
}
