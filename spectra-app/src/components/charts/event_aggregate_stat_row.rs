use leptos::prelude::*;
use orbital::components::{SpacingSize, StatCard};
use orbital::primitives::{Flex, FlexWrap};
use spectra_core::StatCardDto;

#[component]
pub fn EventAggregateStatRow(
    /// Headline text.
    headline: Vec<StatCardDto>,
) -> impl IntoView {
    view! {
        <div data-testid="spectra-event-headline-stats">
            <Flex gap=SpacingSize::Size160.flex_gap() wrap=FlexWrap::Wrap>
                {headline.into_iter().map(|c| {
                    let label: &'static str =
                        Box::leak(c.label.clone().into_boxed_str());
                    let value = Signal::derive(move || c.value.clone());
                    view! { <StatCard label=label value=value /> }
                }).collect_view()}
            </Flex>
        </div>
    }
}
