//! `/spectra/dashboards`: every dashboard the viewer can open, in three tabs.

use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;
use orbital::components::{
    Caption1, ContentContainer, EmptyState, EmptyStateCallToAction, Persona, PersonaConfig,
    SpacingSize, Tag, TagGroup, Title3,
};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, BadgeSize, Button, ButtonAppearance, Flex, FlexAlign,
    FlexJustify, Link, MessageBar, MessageBarActions, MessageBarBody, MessageBarIntent, SearchBox,
    SearchBoxAppearance, SearchBoxBind, Tab, TabList,
};
use orbital_datatable::{
    ColumnWidth, DataRecord, DataTable, DataTableColumnDef, DataTableEmptyView, DataTableFeatures,
    DataTableLoadingStyle, DataTableRowModel, DataTableSource, DataTableToolbarConfig, DataValue,
    PagingMode,
};
use spectra_backend::dashboards::{DashboardScope, DashboardSummary, Principal};
use spectra_backend::spectra_dashboard_path;

use super::new_dialog::NewDashboardDialog;
use super::saved_date;
use crate::server::{list_dashboards, set_favourite};

const TAB_MINE: &str = "mine";
const TAB_SHARED: &str = "shared";
const TAB_ALL: &str = "all";

/// The three lists behind the tabs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DashboardLists {
    pub mine: Vec<DashboardSummary>,
    pub shared: Vec<DashboardSummary>,
    pub all: Vec<DashboardSummary>,
}

impl DashboardLists {
    fn tab(&self, tab: &str) -> &[DashboardSummary] {
        match tab {
            TAB_MINE => &self.mine,
            TAB_SHARED => &self.shared,
            _ => &self.all,
        }
    }
}

async fn load_lists() -> Result<DashboardLists, String> {
    let load = |scope| async move {
        list_dashboards(scope, None)
            .await
            .map_err(|e| e.to_string())
    };
    Ok(DashboardLists {
        mine: load(DashboardScope::Mine).await?,
        shared: load(DashboardScope::Shared).await?,
        all: load(DashboardScope::All).await?,
    })
}

/// Rows whose title or description contains `search`, ignoring case.
pub(super) fn matching<'a>(
    rows: &'a [DashboardSummary],
    search: &str,
) -> Vec<&'a DashboardSummary> {
    let needle = search.trim().to_lowercase();
    rows.iter()
        .filter(|d| {
            needle.is_empty()
                || d.title.to_lowercase().contains(&needle)
                || d.description.to_lowercase().contains(&needle)
        })
        .collect()
}

fn principal_label(p: &Principal) -> String {
    match p {
        Principal::User(id) | Principal::Group(id) => id.clone(),
    }
}

fn to_row(d: &DashboardSummary) -> DataTableRowModel {
    let flag = |on: bool| if on { "true" } else { "" }.to_string();
    let mut cells = HashMap::new();
    cells.insert("name".into(), d.title.clone());
    cells.insert("description".into(), d.description.clone());
    cells.insert("favourite".into(), flag(d.favourite));
    cells.insert("owner".into(), d.owner.clone().unwrap_or_default());
    cells.insert("read_only".into(), flag(d.read_only));
    cells.insert(
        "shared".into(),
        d.shared_with
            .iter()
            .map(principal_label)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    cells.insert("updated".into(), saved_date(d.updated_at));
    DataTableRowModel::from_text_cells(d.id.clone(), cells)
}

fn text(r: &DataRecord, field: &str) -> String {
    match r.get(field) {
        Some(DataValue::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

fn name_column(on_favourite: Callback<(String, bool)>) -> DataTableColumnDef {
    DataTableColumnDef::new("name", "Name")
        .with_width(ColumnWidth::Flex(3.0))
        .with_min_width(260.0)
        .with_cell_view(Arc::new(move |r: DataRecord| {
            let id = r.id.clone();
            let favourite = !text(&r, "favourite").is_empty();
            let title = text(&r, "name");
            let description = text(&r, "description");
            let star_label = if favourite {
                format!("Remove {title} from favourites")
            } else {
                format!("Add {title} to favourites")
            };
            let star_icon = if favourite {
                icondata::AiStarFilled
            } else {
                icondata::AiStarOutlined
            };
            let test_id = format!("spectra-dashboard-row-{id}");
            let href = spectra_dashboard_path(&id);
            view! {
                <div data-testid=test_id>
                    <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center>
                        <span data-testid="spectra-dashboard-favourite" data-on=favourite.to_string()>
                            <Button
                                appearance=ButtonAppearance::Subtle
                                icon=star_icon
                                attr:aria-label=star_label
                                attr:aria-pressed=favourite.to_string()
                                on_click=Callback::new(move |ev: leptos::ev::MouseEvent| {
                                    ev.stop_propagation();
                                    on_favourite.run((id.clone(), !favourite));
                                })
                            />
                        </span>
                        <Flex vertical=true gap=SpacingSize::Size20.flex_gap()>
                            <span data-testid="spectra-dashboard-row-link">
                                <Link href=href>{title}</Link>
                            </span>
                            <Caption1>{description}</Caption1>
                        </Flex>
                    </Flex>
                </div>
            }
            .into_any()
        }))
}

fn owner_column() -> DataTableColumnDef {
    DataTableColumnDef::new("owner", "Owner")
        .with_width(ColumnWidth::Flex(1.0))
        .with_min_width(150.0)
        .with_cell_view(Arc::new(|r: DataRecord| {
            if text(&r, "read_only").is_empty() {
                view! { <Persona config=PersonaConfig::named(text(&r, "owner")) /> }.into_any()
            } else {
                view! {
                    <span data-testid="spectra-dashboard-builtin-badge">
                        <Badge
                            appearance=BadgeAppearance::Tint
                            color=BadgeColor::Informative
                            size=BadgeSize::Small
                        >
                            "Built-in"
                        </Badge>
                    </span>
                }
                .into_any()
            }
        }))
}

fn shared_column() -> DataTableColumnDef {
    DataTableColumnDef::new("shared", "Shared with")
        .with_width(ColumnWidth::Flex(1.0))
        .with_min_width(150.0)
        .with_cell_view(Arc::new(|r: DataRecord| {
            let names: Vec<String> = text(&r, "shared")
                .lines()
                .map(str::to_string)
                .filter(|s| !s.is_empty())
                .collect();
            view! {
                <TagGroup>
                    {names.into_iter().map(|n| view! { <Tag>{n}</Tag> }).collect_view()}
                </TagGroup>
            }
            .into_any()
        }))
}

/// The list page.
#[component]
pub fn DashboardIndexPage() -> impl IntoView {
    let tab = RwSignal::new(TAB_MINE.to_string());
    let search = RwSignal::new(String::new());
    let refresh = RwSignal::new(0u32);
    let new_open = RwSignal::new(false);

    let lists_res = LocalResource::new(move || {
        refresh.track();
        load_lists()
    });
    let lists = RwSignal::new(None::<Result<DashboardLists, String>>);
    Effect::new(move |_| {
        if let Some(result) = lists_res.get() {
            lists.set(Some(result));
        }
    });

    let loaded = Memo::new(move |_| match lists.get() {
        Some(Ok(l)) => Some(l),
        _ => None,
    });
    let table_rows = RwSignal::new(Vec::<DataTableRowModel>::new());
    let loading = RwSignal::new(true);
    Effect::new(move |_| {
        let Some(l) = loaded.get() else {
            return;
        };
        let rows = search.with(|q| {
            matching(l.tab(&tab.get()), q)
                .into_iter()
                .map(to_row)
                .collect()
        });
        table_rows.set(rows);
        loading.set(false);
    });

    let count = move |which: &'static str| {
        loaded.with(|l| {
            l.as_ref()
                .map(|l| search.with(|q| matching(l.tab(which), q).len()))
        })
    };
    let tab_label = move |label: &'static str, which: &'static str| {
        count(which).map_or_else(|| label.to_string(), |n| format!("{label} ({n})"))
    };

    let on_favourite = Callback::new(move |(id, on): (String, bool)| {
        leptos::task::spawn_local(async move {
            if set_favourite(id, on).await.is_ok() {
                refresh.update(|n| *n += 1);
            }
        });
    });
    let all_dashboards = Signal::derive(move || loaded.get().map_or_default(|l| l.all));

    view! {
        <ContentContainer data_testid="spectra-dashboards-page">
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center>
                    <Title3>"Dashboards"</Title3>
                    <span data-testid="spectra-dashboards-new">
                        <Button
                            appearance=ButtonAppearance::Primary
                            on_click=Callback::new(move |_| new_open.set(true))
                        >
                            "New dashboard"
                        </Button>
                    </span>
                </Flex>
                <div data-testid="spectra-dashboards-tabs">
                    <TabList selected_value=tab>
                        <Tab value=TAB_MINE>
                            <span data-testid="spectra-dashboards-tab-mine">
                                {move || tab_label("Mine", TAB_MINE)}
                            </span>
                        </Tab>
                        <Tab value=TAB_SHARED>
                            <span data-testid="spectra-dashboards-tab-shared">
                                {move || tab_label("Shared with me", TAB_SHARED)}
                            </span>
                        </Tab>
                        <Tab value=TAB_ALL>
                            <span data-testid="spectra-dashboards-tab-all">
                                {move || tab_label("All I can read", TAB_ALL)}
                            </span>
                        </Tab>
                    </TabList>
                </div>
                <div data-testid="spectra-dashboards-search">
                    <SearchBox
                        bind=SearchBoxBind::new(search)
                        appearance=SearchBoxAppearance::with_placeholder("Search dashboards")
                    />
                </div>
                {move || match lists.get() {
                    Some(Err(e)) => Some(view! {
                        <div data-testid="spectra-dashboards-error">
                            <MessageBar intent=MessageBarIntent::Error>
                                <MessageBarBody>{format!("Couldn't load dashboards: {e}")}</MessageBarBody>
                                <MessageBarActions>
                                    <Button on_click=Callback::new(move |_| refresh.update(|n| *n += 1))>
                                        "Retry"
                                    </Button>
                                </MessageBarActions>
                            </MessageBar>
                        </div>
                    }),
                    _ => None,
                }}
                <div data-testid="spectra-dashboards-table">
                    <DataTable
                        data_source=DataTableSource::Client(table_rows)
                        paging=PagingMode::Paged
                        features=DataTableFeatures::empty()
                        toolbar_config=DataTableToolbarConfig {
                            quick_search: false,
                            filter_panel: false,
                            column_picker: false,
                            pivot: false,
                            export_menu: false,
                        }
                        loading=loading
                        loading_style=DataTableLoadingStyle::SkeletonRows(4)
                        auto_row_height=true
                        columns=vec![
                            name_column(on_favourite),
                            owner_column(),
                            shared_column(),
                            DataTableColumnDef::new("updated", "Updated")
                                .with_width(ColumnWidth::Fixed(130.0)),
                        ]
                    >
                        <DataTableEmptyView slot>
                            <ListEmptyState tab=tab search=search new_open=new_open />
                        </DataTableEmptyView>
                    </DataTable>
                </div>
            </Flex>
            <NewDashboardDialog open=new_open sources=all_dashboards />
        </ContentContainer>
    }
}

/// What an empty tab says: no search match, nothing of my own, or nothing shared.
#[component]
fn ListEmptyState(
    tab: RwSignal<String>,
    search: RwSignal<String>,
    new_open: RwSignal<bool>,
) -> impl IntoView {
    move || {
        if !search.with(|q| q.trim().is_empty()) {
            return view! {
                <div data-testid="spectra-dashboards-empty-search">
                    <EmptyState
                        message="No dashboards match your search"
                        description="Try another word, or clear the search to see them all."
                    >
                        <EmptyStateCallToAction slot:call_to_action>
                            <Button on_click=Callback::new(move |_| search.set(String::new()))>
                                "Clear search"
                            </Button>
                        </EmptyStateCallToAction>
                    </EmptyState>
                </div>
            }
            .into_any();
        }
        if tab.get() == TAB_SHARED {
            return view! {
                <div data-testid="spectra-dashboards-empty-shared">
                    <EmptyState
                        message="Nothing shared with you yet"
                        description="When someone shares a dashboard with you, it shows up here."
                    />
                </div>
            }
            .into_any();
        }
        view! {
            <div data-testid="spectra-dashboards-empty-mine">
                <EmptyState
                    message="You haven't made a dashboard yet"
                    description="Start a blank one, or copy a built-in from All I can read and change it."
                >
                    <EmptyStateCallToAction slot:call_to_action>
                        <Button
                            appearance=ButtonAppearance::Primary
                            on_click=Callback::new(move |_| new_open.set(true))
                        >
                            "New dashboard"
                        </Button>
                        <Button on_click=Callback::new(move |_| tab.set(TAB_ALL.to_string()))>
                            "Show built-ins"
                        </Button>
                    </EmptyStateCallToAction>
                </EmptyState>
            </div>
        }
        .into_any()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn summary(title: &str, description: &str) -> DashboardSummary {
        DashboardSummary {
            id: title.to_lowercase(),
            title: title.into(),
            description: description.into(),
            owner: Some("u1".into()),
            read_only: false,
            favourite: false,
            version: 1,
            updated_at: Utc::now(),
            shared_with: vec![Principal::User("u2".into()), Principal::Group("ops".into())],
        }
    }

    #[test]
    fn search_matches_title_or_description_ignoring_case() {
        let rows = vec![
            summary("Checkout", "orders"),
            summary("Photon", "queue depth"),
        ];
        assert_eq!(matching(&rows, "  CHECK ").len(), 1);
        assert_eq!(matching(&rows, "queue")[0].title, "Photon");
        assert_eq!(matching(&rows, "").len(), 2);
        assert_eq!(matching(&rows, "nothing"), Vec::<&DashboardSummary>::new());
    }

    #[test]
    fn rows_carry_every_column() {
        let row = to_row(&summary("Checkout", "orders"));
        for key in [
            "name",
            "description",
            "favourite",
            "owner",
            "read_only",
            "shared",
            "updated",
        ] {
            assert!(row.record.get(key).is_some(), "{key}");
        }
        assert_eq!(text(&row.record, "shared"), "u2\nops");
        assert_eq!(row.record.id, "checkout");
    }
}
