use leptos::prelude::*;
use orbital::components::{Card, CardContent};
use turf::inline_style_sheet_values;

#[component]
pub fn ChartSurfaceMaterial(
    /// Child content rendered inside the component.
    children: Children,
) -> impl IntoView {
    // `Card`'s own layout CSS documents this contract (orbital-core-components card/styles.rs
    // `card_layout_styles`): its inner Flex already stretches to fill via `flex: 1 1 auto;
    // height: 100%` once the card root itself is given `height: 100%` — Material's docs call
    // for a Turf class here rather than inline `style` for one-off sizing on Orbital surfaces.
    // `CardContent` has no such default (a plain padded div), so it needs the same treatment
    // to pass the remaining height down to the chart.
    let (style_sheet, class_names) = inline_style_sheet_values! {
        .ChartSurfaceFill {
            height: 100%;
        }

        .ChartSurfaceContentFill {
            flex: 1 1 auto;
            min-height: 0;
            display: flex;
            flex-direction: column;
        }
    };

    view! {
        <style>{style_sheet}</style>
        <Card class=class_names.chart_surface_fill>
            <CardContent class=class_names.chart_surface_content_fill>
                {children()}
            </CardContent>
        </Card>
    }
}
