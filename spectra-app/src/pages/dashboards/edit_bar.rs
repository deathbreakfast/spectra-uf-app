//! The floating save bar on the edit page: add actions on the left, the unsaved state with
//! Discard and Save on the right.

use leptos::prelude::*;
use orbital::components::{Caption1, SpacingSize};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, BadgeSize, Button, ButtonAppearance, Card, CardContent,
    Flex, FlexAlign, FlexJustify, FlexWrap, MaterialElevation, Menu, MenuItem, MenuTrigger,
    Spinner, SpinnerSize,
};

use super::edit::PanelEditorStart;

/// The save bar.
#[component]
pub(super) fn EditBar(
    /// Whether the draft differs from the saved version.
    dirty: Signal<bool>,
    /// Whether a save is in flight.
    saving: Signal<bool>,
    /// Add panel's menu choices.
    on_open_panel_editor: Callback<PanelEditorStart>,
    /// Add an empty section at the end.
    on_add_section: Callback<()>,
    /// Open the variables dialog.
    on_variables: Callback<()>,
    /// Ask before throwing the changes away.
    on_discard: Callback<()>,
    /// Save the draft.
    on_save: Callback<()>,
    /// Leave edit mode once everything is saved.
    on_done: Callback<()>,
) -> impl IntoView {
    view! {
        <div
            data-testid="spectra-edit-bar"
            style="position: sticky; top: var(--orb-space-block-sm); z-index: 2;"
        >
            <Card elevation=MaterialElevation::Floating>
                <CardContent>
                    <Flex
                        gap=SpacingSize::Size120.flex_gap()
                        align=FlexAlign::Center
                        justify=FlexJustify::SpaceBetween
                        wrap=FlexWrap::Wrap
                    >
                        <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center wrap=FlexWrap::Wrap>
                            <Menu
                                data_testid="spectra-edit-add-panel"
                                on_select=move |choice: &str| match choice {
                                    "build" => on_open_panel_editor.run(PanelEditorStart::Build),
                                    "link" => on_open_panel_editor.run(PanelEditorStart::FromLink),
                                    "text" => on_open_panel_editor.run(PanelEditorStart::Text),
                                    _ => {}
                                }
                            >
                                <MenuTrigger slot>
                                    <Button appearance=ButtonAppearance::Primary icon=icondata::AiDownOutlined>
                                        "Add panel"
                                    </Button>
                                </MenuTrigger>
                                <MenuItem value="build">
                                    <span data-testid="spectra-edit-add-build">"Build a panel"</span>
                                </MenuItem>
                                <MenuItem value="link">
                                    <span data-testid="spectra-edit-add-link">"From a query link"</span>
                                </MenuItem>
                                <MenuItem value="text">
                                    <span data-testid="spectra-edit-add-text-item">"Text"</span>
                                </MenuItem>
                            </Menu>
                            <span data-testid="spectra-edit-add-section">
                                <Button
                                    icon=icondata::AiPlusOutlined
                                    on_click=Callback::new(move |_| on_add_section.run(()))
                                >
                                    "Add section"
                                </Button>
                            </span>
                            <span data-testid="spectra-edit-add-text">
                                <Button
                                    icon=icondata::AiEditOutlined
                                    on_click=Callback::new(move |_| on_open_panel_editor.run(PanelEditorStart::Text))
                                >
                                    "Add text"
                                </Button>
                            </span>
                            <span data-testid="spectra-edit-variables">
                                <Button
                                    appearance=ButtonAppearance::Subtle
                                    icon=icondata::AiSettingOutlined
                                    on_click=Callback::new(move |_| on_variables.run(()))
                                >
                                    "Variables"
                                </Button>
                            </span>
                        </Flex>
                        <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center>
                            {move || {
                                if saving.get() {
                                    view! {
                                        <span data-testid="spectra-edit-saving">
                                            <Flex gap=SpacingSize::Size80.flex_gap() align=FlexAlign::Center>
                                                <Spinner size=SpinnerSize::Small />
                                                <Caption1>"Saving…"</Caption1>
                                            </Flex>
                                        </span>
                                    }
                                    .into_any()
                                } else if dirty.get() {
                                    view! {
                                        <span data-testid="spectra-edit-unsaved">
                                            <Badge
                                                appearance=BadgeAppearance::Tint
                                                color=BadgeColor::Warning
                                                size=BadgeSize::Small
                                            >
                                                "Unsaved changes"
                                            </Badge>
                                        </span>
                                        <span data-testid="spectra-edit-discard">
                                            <Button
                                                appearance=ButtonAppearance::Subtle
                                                on_click=Callback::new(move |_| on_discard.run(()))
                                            >
                                                "Discard"
                                            </Button>
                                        </span>
                                        <span data-testid="spectra-edit-save">
                                            <Button
                                                appearance=ButtonAppearance::Primary
                                                on_click=Callback::new(move |_| on_save.run(()))
                                            >
                                                "Save"
                                            </Button>
                                        </span>
                                    }
                                    .into_any()
                                } else {
                                    view! {
                                        <span data-testid="spectra-edit-saved">
                                            <Caption1>"No unsaved changes"</Caption1>
                                        </span>
                                        <span data-testid="spectra-edit-done">
                                            <Button
                                                appearance=ButtonAppearance::Primary
                                                on_click=Callback::new(move |_| on_done.run(()))
                                            >
                                                "Done"
                                            </Button>
                                        </span>
                                    }
                                    .into_any()
                                }
                            }}
                        </Flex>
                    </Flex>
                </CardContent>
            </Card>
        </div>
    }
}
