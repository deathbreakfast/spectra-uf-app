mod components;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use orbital::components::Title3;
use orbital::primitives::{Flex, FlexAlign, FlexGap};

use self::components::EventExplorePanel;
use crate::components::query::CopyQueryLink;
use crate::explore_time::use_explore_link;

/// Event explore canvas: query, filter, and visualize logged events for a schema.
///
/// The whole query lives in the URL, so the address bar is always a link to this view.
/// Fills the shell under the breadcrumb — no page gutter or ContentContainer column.
#[component]
pub fn EventExplorePage() -> impl IntoView {
    let params = use_params_map();
    let table = Memo::new(move |_| params.with(|p| p.get("name").unwrap_or_default()));
    let link = use_explore_link();

    view! {
        <div data-testid="spectra-event-explore-panel" style="display: flex; flex-direction: column; min-height: 0; flex: 1 1 auto; width: 100%;">
            <Flex align=FlexAlign::Center gap=FlexGap::Small>
                <Title3>"Explore event rows"</Title3>
                <CopyQueryLink />
            </Flex>
            <EventExplorePanel table=table link=link />
        </div>
    }
}
