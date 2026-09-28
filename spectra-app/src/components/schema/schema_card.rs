use leptos::prelude::*;
use orbital::components::{Body1, SpacingSize, Title3};
use orbital::primitives::{Card, CardContent, CardHeader, Flex, Link};
use spectra_backend::{
    spectra_metric_explore_path, spectra_schema_explore_path, spectra_schema_path,
};
use spectra_core::SchemaListItem;

use super::KindBadge;

#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn SchemaCard(
    /// Item to render.
    item: SchemaListItem,
    /// Spotlight DOM id for the Details link (empty = omit).
    #[prop(optional)]
    details_spotlight_id: &'static str,
    /// Spotlight DOM id for the Explore link (empty = omit).
    #[prop(optional)]
    explore_spotlight_id: &'static str,
) -> impl IntoView {
    let name = item.table_or_metric.clone();
    let kind = item.logging_kind.clone();
    let description = item.description.clone().unwrap_or_default();
    let href = if kind == "metric" {
        spectra_metric_explore_path(&name)
    } else {
        spectra_schema_explore_path(&name)
    };
    let detail_href = spectra_schema_path(&name);
    let test_id = format!("spectra-schema-card-{name}");
    let detail_test_id = format!("spectra-schema-card-{name}-details");
    let explore_test_id = format!("spectra-schema-card-{name}-explore");
    let details_id = (!details_spotlight_id.is_empty()).then_some(details_spotlight_id);
    let explore_id = (!explore_spotlight_id.is_empty()).then_some(explore_spotlight_id);
    view! {
        <div data-testid=test_id>
            <Card>
                <CardHeader>
                    <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                        <Title3>{name.clone()}</Title3>
                        <KindBadge kind=kind />
                    </Flex>
                </CardHeader>
                <CardContent>
                    <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                        {(!description.is_empty()).then(|| view! { <Body1>{description}</Body1> })}
                        <span id=details_id data-testid=detail_test_id>
                            <Link href=detail_href>"Details"</Link>
                        </span>
                        <span id=explore_id data-testid=explore_test_id>
                            <Link href=href>"Explore"</Link>
                        </span>
                    </Flex>
                </CardContent>
            </Card>
        </div>
    }
}
