use leptos::prelude::*;
use orbital_charts::{BarChart, TooltipConfig};
use spectra_core::SliceDto;

use super::chart_series::chart_from_slices;
use super::chart_surface_material::ChartSurfaceMaterial;

#[component]
pub fn EventBarChart(
    /// Category slices to render.
    slices: Vec<SliceDto>,
) -> impl IntoView {
    let (x_axis, chart_series) = chart_from_slices(&slices, orbital_charts::ChartType::Bar);
    view! {
        <div data-testid="spectra-event-bar-chart" style="width: 100%; height: 100%;">
            <ChartSurfaceMaterial>
                <div style="width: 100%; flex: 1 1 auto; min-height: 320px;">
                    <BarChart
                        x_axis=x_axis
                        series=chart_series
                        responsive=true
                        tooltip=TooltipConfig::axis()
                    />
                </div>
            </ChartSurfaceMaterial>
        </div>
    }
}
