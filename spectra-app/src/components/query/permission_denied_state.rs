use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};
use orbital::primitives::{Button, ButtonAppearance, Flex, MessageBar, MessageBarIntent};
use uf_product::paths::PERMISSION_PERMISSIONS;

#[component]
pub fn PermissionDeniedState() -> impl IntoView {
    let navigate = use_navigate();
    view! {
        <div data-testid="spectra-permission-denied">
            <Flex vertical=true>
                <MessageBar intent=MessageBarIntent::Warning>
                    "You do not have permission to query this table."
                </MessageBar>
                <Button
                    appearance=ButtonAppearance::Primary
                    on_click=Callback::new(move |_| {
                        navigate(PERMISSION_PERMISSIONS, NavigateOptions::default());
                    })
                >
                    "Request Permission"
                </Button>
            </Flex>
        </div>
    }
}
