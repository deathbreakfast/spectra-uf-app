//! One dashboard section: a heading with a collapse button, and its panels on a
//! twelve-column grid that becomes one column below `Md`.

use leptos::prelude::*;
use orbital::components::{SpacingSize, Subtitle1};
use orbital::primitives::{
    use_breakpoint_up, Breakpoint, Button, ButtonAppearance, Flex, FlexAlign, Grid, GridConfig,
    GridItem, GridItemConfig,
};
use spectra_backend::dashboards::{Layout, Section, DASHBOARD_GRID_COLUMNS};

use super::detail::PanelEnv;
use super::panel::{DashboardPanel, PanelSlot};

const GRID_GAP: u16 = 16;

/// Each panel's offset from the end of the panel before it on the same row, for panels
/// sorted by row then column. The first panel on a row is offset from the left edge.
pub(super) fn grid_offsets(layouts: &[Layout]) -> Vec<u16> {
    let mut row = None;
    let mut end = 0u16;
    layouts
        .iter()
        .map(|l| {
            if row != Some(l.row) {
                row = Some(l.row);
                end = 0;
            }
            let col = u16::from(l.col);
            let offset = col.saturating_sub(end);
            end = col.max(end) + u16::from(l.span);
            offset
        })
        .collect()
}

/// A section of the dashboard page.
#[component]
pub(super) fn DashboardSection(section: Section) -> impl IntoView {
    let env = expect_context::<PanelEnv>();
    let collapsed = RwSignal::new(section.collapsed);
    let wide = use_breakpoint_up(Breakpoint::Md);

    let mut panels = section.panels.clone();
    panels.sort_by_key(|p| (p.layout.row, p.layout.col));
    let offsets = grid_offsets(&panels.iter().map(|p| p.layout).collect::<Vec<_>>());
    // Panel resources live here, outside the grid, so rebuilding the grid when the
    // breakpoint changes (or the section opens again) never refetches a panel.
    let slots: Vec<(PanelSlot, u16)> = panels
        .into_iter()
        .zip(offsets)
        .map(|(panel, offset)| (PanelSlot::new(panel, &env), offset))
        .collect();
    let slots = StoredValue::new(slots);

    let title = section.title.clone();
    let test_id = format!("spectra-dashboard-section-{}", section.id);
    let toggle_label = {
        let title = title.clone();
        move || {
            if collapsed.get() {
                format!("Show {title}")
            } else {
                format!("Hide {title}")
            }
        }
    };

    view! {
        <div data-testid=test_id data-collapsed=move || collapsed.get().to_string()>
            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                {(!title.is_empty()).then(|| view! {
                    <Flex gap=SpacingSize::Size40.flex_gap() align=FlexAlign::Center>
                        <span data-testid="spectra-dashboard-section-toggle">
                            <Button
                                appearance=ButtonAppearance::Subtle
                                icon=Signal::derive(move || Some(if collapsed.get() {
                                    icondata::AiRightOutlined
                                } else {
                                    icondata::AiDownOutlined
                                }))
                                attr:aria-label=toggle_label.clone()
                                attr:aria-expanded=move || (!collapsed.get()).to_string()
                                on_click=Callback::new(move |_| collapsed.update(|c| *c = !*c))
                            />
                        </span>
                        <Subtitle1>{title.clone()}</Subtitle1>
                    </Flex>
                })}
                <Show when=move || !collapsed.get()>
                    {move || {
                        let wide = wide.get();
                        let cols = if wide { u16::from(DASHBOARD_GRID_COLUMNS) } else { 1 };
                        view! {
                            <div data-testid="spectra-dashboard-grid" data-columns=cols.to_string()>
                                <Grid config=GridConfig::with_gaps(cols, GRID_GAP, GRID_GAP)>
                                    {slots.get_value().into_iter().map(|(slot, offset)| {
                                        let config = if wide {
                                            GridItemConfig::with_offset(u16::from(slot.panel.layout.span), offset)
                                        } else {
                                            GridItemConfig::span(1)
                                        };
                                        view! {
                                            <GridItem config=config>
                                                <DashboardPanel entry=slot />
                                            </GridItem>
                                        }
                                    }).collect_view()}
                                </Grid>
                            </div>
                        }
                    }}
                </Show>
            </Flex>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(row: u16, col: u8, span: u8) -> Layout {
        Layout {
            col,
            span,
            row,
            height: 4,
        }
    }

    #[test]
    fn offsets_fill_gaps_within_a_row() {
        let layouts = [at(0, 0, 4), at(0, 6, 6), at(1, 2, 4), at(1, 6, 6)];
        assert_eq!(grid_offsets(&layouts), vec![0, 2, 2, 0]);
    }

    #[test]
    fn packed_rows_have_no_offsets() {
        let layouts = [at(0, 0, 6), at(0, 6, 6), at(1, 0, 12)];
        assert_eq!(grid_offsets(&layouts), vec![0, 0, 0]);
    }
}
