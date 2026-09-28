use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use orbital::components::{Card, CardContent, CardHeader, Title3};
use orbital::primitives::{Button, ButtonAppearance};
use spectra_backend::{spectra_metric_explore_path, spectra_schema_explore_path};

#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn QuickActionsCard(
    /// Display name.
    name: String,
    /// Kind or category.
    kind: String,
) -> impl IntoView {
    let href = if kind == "metric" {
        spectra_metric_explore_path(&name)
    } else {
        spectra_schema_explore_path(&name)
    };
    let navigate = use_navigate();
    view! {
        <Card>
            <CardHeader>
                <Title3>"Explore"</Title3>
            </CardHeader>
            <CardContent>
                <span id="spectra-detail-open-explore" data-testid="spectra-detail-open-explore">
                    <Button
                        appearance=ButtonAppearance::Primary
                        on:click=move |_| {
                            navigate(&href, Default::default());
                        }
                    >
                        "Open explore"
                    </Button>
                </span>
            </CardContent>
        </Card>
    }
}
