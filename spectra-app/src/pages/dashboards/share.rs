//! The Share dialog: who owns a dashboard, who can edit and view it, and a link to it.
//!
//! The owner picks editors and viewers from Gauge's people and groups and saves them with
//! `share_dashboard`. Everyone else sees the owner, their own access, who else has access
//! (read-only) and Copy link.

use std::collections::HashMap;

use leptos::prelude::*;
use orbital::components::{
    Caption1, Persona, PersonaConfig, SpacingSize, Tag, TagGroup, TagPicker, TagPickerBind,
    TagPickerControl, TagPickerGroup, TagPickerInput, TagPickerOption,
};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, Button, ButtonAppearance, CopyButton, Dialog,
    DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Field, Flex, FlexAlign,
    MessageBar, MessageBarBody, MessageBarIntent,
};
use serde::{Deserialize, Serialize};
use spectra_backend::dashboards::{DashboardAccess, Principal};
use spectra_backend::spectra_dashboard_path;

use super::{action_error_text, parse_principal_key, principal_key};
use crate::server::share_dashboard;

/// Gauge's search source for people.
const USER_SOURCE: &str = "user_search_source";
/// Gauge's search source for permission groups.
const GROUP_SOURCE: &str = "permission_group_search_source";
/// Gauge names each resource's owners group with this prefix; they aren't share targets.
const OWNERS_GROUP_PREFIX: &str = "rp_owners_";
/// Most people, and most groups, one directory load returns.
const DIRECTORY_LIMIT: u32 = 50;

/// A person or group the owner can share with, with the name to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareCandidate {
    /// Who.
    pub principal: Principal,
    /// Display name.
    pub label: String,
}

/// A search hit from Gauge's people or groups source as a share target, or `None` for
/// anything else, such as a resource's owners group or the super-user group.
fn candidate(source_id: &str, id: String, title: String) -> Option<ShareCandidate> {
    if id.is_empty() {
        return None;
    }
    let principal = match source_id {
        USER_SOURCE => Principal::User(id),
        GROUP_SOURCE if !id.starts_with(OWNERS_GROUP_PREFIX) && id != "super_user_group" => {
            Principal::Group(id)
        }
        _ => return None,
    };
    Some(ShareCandidate {
        principal,
        label: title,
    })
}

/// People and groups the caller can see in Gauge, searched by `q`.
///
/// Runs Gauge's own principal search sources as the session actor, so the list holds only
/// what Gauge shows this person.
#[uf_product_macros::server(permission = "QueryTable")]
pub async fn share_directory(
    /// Text to match in a name or id; empty for everyone.
    q: Option<String>,
) -> Result<Vec<ShareCandidate>, ServerFnError> {
    use spectra_backend::dashboards::DashboardError;
    use spectra_backend::SpectraOpsError;
    use uf_product::search_sources::{SearchSourceKey, SearchSourceRegistry};

    use crate::server::{to_dashboard_server_error, to_server_fn_error};

    let ctx = higgs::Higgs::from_request().await?;
    if ctx.session_user_id().is_none() {
        return Err(to_server_fn_error(SpectraOpsError::AuthRequired));
    }
    let v = ctx
        .valence()
        .map_err(|e| to_dashboard_server_error(&DashboardError::store(e)))?;
    let registry = SearchSourceRegistry::auto_discover();
    let q = q.unwrap_or_default();
    let mut out = Vec::new();
    // One source at a time, so a long list of people never crowds out the groups.
    for (id, label) in [(USER_SOURCE, "People"), (GROUP_SOURCE, "Groups")] {
        let hits = registry
            .query_many(
                &[SearchSourceKey::new(id, label)],
                &v,
                q.trim(),
                DIRECTORY_LIMIT,
            )
            .await
            .map_err(|e| to_dashboard_server_error(&DashboardError::store(e)))?;
        out.extend(
            hits.into_iter()
                .filter_map(|hit| candidate(&hit.source_id, hit.id, hit.title)),
        );
    }
    Ok(out)
}

/// The name to show for `principal`: its directory label, else its id.
pub(super) fn principal_label(principal: &Principal, labels: &HashMap<String, String>) -> String {
    labels
        .get(&principal_key(principal))
        .cloned()
        .unwrap_or_else(|| match principal {
            Principal::User(id) | Principal::Group(id) => id.clone(),
        })
}

/// Directory labels by [`principal_key`].
pub(super) fn label_map(candidates: &[ShareCandidate]) -> HashMap<String, String> {
    candidates
        .iter()
        .map(|c| (principal_key(&c.principal), c.label.clone()))
        .collect()
}

/// The owner's picks and the Save action.
#[derive(Clone, Copy)]
struct ShareState {
    editors: RwSignal<Vec<String>>,
    viewers: RwSignal<Vec<String>>,
    error: RwSignal<bool>,
    saving: RwSignal<bool>,
    save: Callback<()>,
}

impl ShareState {
    fn new(open: RwSignal<bool>, dashboard_id: String, access: RwSignal<DashboardAccess>) -> Self {
        let keys = |list: &[Principal]| list.iter().map(principal_key).collect::<Vec<_>>();
        let editors = RwSignal::new(access.with_untracked(|a| keys(&a.editors)));
        let viewers = RwSignal::new(access.with_untracked(|a| keys(&a.viewers)));
        let error = RwSignal::new(false);
        let saving = RwSignal::new(false);
        // Each opening starts from who has access now.
        Effect::new(move |_| {
            if open.get() {
                access.with_untracked(|a| {
                    editors.set(keys(&a.editors));
                    viewers.set(keys(&a.viewers));
                });
                error.set(false);
            }
        });
        let save = Callback::new(move |()| {
            let parse = |list: Vec<String>| {
                list.iter()
                    .filter_map(|k| parse_principal_key(k))
                    .collect::<Vec<_>>()
            };
            let (id, eds, vws) = (
                dashboard_id.clone(),
                parse(editors.get_untracked()),
                parse(viewers.get_untracked()),
            );
            saving.set(true);
            error.set(false);
            leptos::task::spawn_local_scoped(async move {
                match share_dashboard(id, eds, vws).await {
                    Ok(next) => {
                        access.set(next);
                        open.set(false);
                    }
                    Err(e) => {
                        log::warn!("Spectra dashboard: saving who has access failed: {e}");
                        error.set(true);
                    }
                }
                saving.set(false);
            });
        });
        Self {
            editors,
            viewers,
            error,
            saving,
            save,
        }
    }
}

/// The Share dialog.
#[component]
pub(super) fn ShareDialog(
    /// Whether the dialog is open.
    open: RwSignal<bool>,
    /// Dashboard id.
    dashboard_id: String,
    /// Owner user id.
    owner: String,
    /// Who has access; updated after a save.
    access: RwSignal<DashboardAccess>,
) -> impl IntoView {
    let can_share = access.with_untracked(|a| a.can_share);
    let can_edit = access.with_untracked(|a| a.can_edit);
    let state = can_share.then(|| ShareState::new(open, dashboard_id.clone(), access));

    let path = spectra_dashboard_path(&dashboard_id);
    let link = RwSignal::new(path.clone());
    Effect::new(move |_| {
        if let Ok(origin) = window().location().origin() {
            link.set(format!("{origin}{path}"));
        }
    });

    view! {
        <Dialog open=open>
            <DialogSurface>
                <div data-testid="spectra-share-dialog">
                    <DialogBody>
                        <DialogTitle>"Share dashboard"</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                                <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                                    <Caption1>"Owner"</Caption1>
                                    <span data-testid="spectra-share-owner">
                                        <Persona config=PersonaConfig::named(owner) />
                                    </span>
                                </Flex>
                                {state.map_or_else(
                                    || view! { <ReadOnlySharing open=open access=access can_edit=can_edit /> }.into_any(),
                                    |state| view! { <OwnerSharing open=open state=state /> }.into_any(),
                                )}
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <span data-testid="spectra-share-copy-link">
                                <CopyButton text=link label="Copy link" copied_label="Link copied" />
                            </span>
                            <span data-testid="spectra-share-close">
                                <Button on_click=Callback::new(move |_| open.set(false))>
                                    {if can_share { "Cancel" } else { "Close" }}
                                </Button>
                            </span>
                            {state.map(|state| view! {
                                <span data-testid="spectra-share-save">
                                    <Button
                                        appearance=ButtonAppearance::Primary
                                        loading=state.saving
                                        on_click=Callback::new(move |_| state.save.run(()))
                                    >
                                        "Save"
                                    </Button>
                                </span>
                            })}
                        </DialogActions>
                    </DialogBody>
                </div>
            </DialogSurface>
        </Dialog>
    }
}

/// The owner's Editors and Viewers pickers.
#[component]
fn OwnerSharing(open: RwSignal<bool>, state: ShareState) -> impl IntoView {
    let directory = LocalResource::new(move || {
        let load = open.get();
        async move {
            if load {
                share_directory(None).await.map_err(|e| {
                    log::warn!("Spectra dashboard: loading people and groups failed: {e}");
                    action_error_text(&e)
                })
            } else {
                Ok(Vec::new())
            }
        }
    });
    let candidates =
        Signal::derive(move || directory.get().and_then(Result::ok).unwrap_or_default());
    let directory_error = move || directory.get().and_then(Result::err);
    let labels = Memo::new(move |_| candidates.with(|c| label_map(c)));
    let error = state.error;

    view! {
        <Field label="Editors">
            <div data-testid="spectra-share-editors">
                <PrincipalPicker selected=state.editors candidates=candidates labels=labels />
            </div>
        </Field>
        <Field label="Viewers">
            <div data-testid="spectra-share-viewers">
                <PrincipalPicker selected=state.viewers candidates=candidates labels=labels />
            </div>
        </Field>
        {move || directory_error().map(|message| view! {
            <div data-testid="spectra-share-directory-error">
                <MessageBar intent=MessageBarIntent::Warning>
                    <MessageBarBody>
                        {format!("Couldn't load people and groups: {message} Close Share and open it again to retry.")}
                    </MessageBarBody>
                </MessageBar>
            </div>
        })}
        <div data-testid="spectra-share-info">
            <MessageBar intent=MessageBarIntent::Info>
                <MessageBarBody>
                    "Sharing opens the dashboard, not its tables. Someone without a table's grant sees that panel locked."
                </MessageBarBody>
            </MessageBar>
        </div>
        <Show when=move || error.get()>
            <div data-testid="spectra-share-error">
                <MessageBar intent=MessageBarIntent::Error>
                    <MessageBarBody>
                        "Couldn't save who has access. Nothing changed; try again."
                    </MessageBarBody>
                </MessageBar>
            </div>
        </Show>
    }
}

/// A TagPicker over people and groups, filtered on the client as the user types.
#[component]
fn PrincipalPicker(
    selected: RwSignal<Vec<String>>,
    candidates: Signal<Vec<ShareCandidate>>,
    labels: Memo<HashMap<String, String>>,
) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let label_of = move |key: &str| {
        labels.with(|l| l.get(key).cloned()).unwrap_or_else(|| {
            parse_principal_key(key)
                .map_or_else(|| key.to_string(), |p| principal_label(&p, &HashMap::new()))
        })
    };
    view! {
        <TagPicker bind=TagPickerBind::new(selected)>
            <TagPickerControl slot>
                <TagPickerGroup>
                    {move || selected.get().into_iter().map(|key| {
                        let label = label_of(&key);
                        view! { <Tag value=key>{label}</Tag> }
                    }).collect_view()}
                    <TagPickerInput on_search=Callback::new(move |q: String| query.set(q)) />
                </TagPickerGroup>
            </TagPickerControl>
            {move || {
                let q = query.get().to_lowercase();
                candidates
                    .get()
                    .into_iter()
                    .filter(|c| q.is_empty() || c.label.to_lowercase().contains(&q))
                    .map(|c| {
                        view! { <TagPickerOption value=principal_key(&c.principal) text=c.label /> }
                    })
                    .collect_view()
            }}
        </TagPicker>
    }
}

/// Read-only tags for one list of people and groups.
fn principal_tags(principals: &[Principal], labels: &HashMap<String, String>) -> impl IntoView {
    let names: Vec<String> = principals
        .iter()
        .map(|p| principal_label(p, labels))
        .collect();
    view! {
        <TagGroup>
            {names.into_iter().map(|n| view! { <Tag>{n}</Tag> }).collect_view()}
        </TagGroup>
    }
}

/// What someone who can't change sharing sees.
#[component]
fn ReadOnlySharing(
    open: RwSignal<bool>,
    access: RwSignal<DashboardAccess>,
    can_edit: bool,
) -> impl IntoView {
    // Names for the tags; a person or group the caller can't look up shows its id.
    let directory = LocalResource::new(move || {
        let load = open.get();
        async move {
            if load {
                share_directory(None).await.unwrap_or_else(|e| {
                    log::warn!("Spectra dashboard: loading people and groups failed: {e}");
                    Vec::new()
                })
            } else {
                Vec::new()
            }
        }
    });
    let labels = Memo::new(move |_| directory.get().map_or_default(|c| label_map(&c)));
    let list = move |pick: fn(&DashboardAccess) -> &Vec<Principal>,
                     caption: &'static str,
                     testid: &'static str| {
        move || {
            access.with(|a| {
                let principals = pick(a);
                (!principals.is_empty()).then(|| {
                    let tags = labels.with(|l| principal_tags(principals, l));
                    view! {
                        <Caption1>{caption}</Caption1>
                        <div data-testid=testid>{tags}</div>
                    }
                })
            })
        }
    };
    let nobody = move || access.with(|a| a.editors.is_empty() && a.viewers.is_empty());
    view! {
        <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
            <Caption1>"Your access"</Caption1>
            <Flex align=FlexAlign::Center>
                <span data-testid="spectra-share-your-access">
                    <Badge appearance=BadgeAppearance::Tint color=BadgeColor::Informative>
                        {if can_edit { "Can edit" } else { "Can view" }}
                    </Badge>
                </span>
            </Flex>
            {list(|a| &a.editors, "Editors", "spectra-share-readonly-editors")}
            {list(|a| &a.viewers, "Viewers", "spectra-share-readonly-viewers")}
            <Show when=nobody>
                <span data-testid="spectra-share-readonly-empty">
                    <Caption1>"Not shared with anyone else yet."</Caption1>
                </span>
            </Show>
        </Flex>
        <div data-testid="spectra-share-readonly">
            <MessageBar intent=MessageBarIntent::Info>
                <MessageBarBody>"Only the owner can change who has access."</MessageBarBody>
            </MessageBar>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_and_named_groups_are_candidates() {
        assert_eq!(
            candidate(USER_SOURCE, "teammate".into(), "Teammate".into()).map(|c| c.principal),
            Some(Principal::User("teammate".into()))
        );
        assert_eq!(
            candidate(GROUP_SOURCE, "g1".into(), "Checkout team".into()).map(|c| c.principal),
            Some(Principal::Group("g1".into()))
        );
    }

    #[test]
    fn owners_groups_and_other_sources_are_not_sad_path() {
        assert_eq!(
            candidate(
                GROUP_SOURCE,
                "rp_owners_spectra_dashboard_x".into(),
                "x".into()
            ),
            None
        );
        assert_eq!(
            candidate(GROUP_SOURCE, "super_user_group".into(), "Super".into()),
            None
        );
        assert_eq!(candidate("apps", "a".into(), "App".into()), None);
        assert_eq!(candidate(USER_SOURCE, String::new(), "Nobody".into()), None);
    }

    #[test]
    fn labels_fall_back_to_the_id() {
        let labels = label_map(&[ShareCandidate {
            principal: Principal::Group("g1".into()),
            label: "Checkout team".into(),
        }]);
        assert_eq!(
            principal_label(&Principal::Group("g1".into()), &labels),
            "Checkout team"
        );
        assert_eq!(
            principal_label(&Principal::User("bo".into()), &labels),
            "bo"
        );
    }
}
