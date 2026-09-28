use leptos::prelude::*;
use orbital_charts::PieChart;
use spectra_core::SliceDto;

use super::chart_series::chart_from_slices;
use super::chart_surface_material::ChartSurfaceMaterial;

#[component]
pub fn EventPieChart(
    /// Category slices to render.
    slices: Vec<SliceDto>,
) -> impl IntoView {
    let (x_axis, chart_series) = chart_from_slices(&slices, orbital_charts::ChartType::Pie);
    view! {
        <div data-testid="spectra-event-pie-chart" style="width: 100%; height: 100%;">
            <ChartSurfaceMaterial>
                <div style="width: 100%; flex: 1 1 auto; min-height: 320px;">
                    <PieChart x_axis=x_axis series=chart_series responsive=true />
                </div>
            </ChartSurfaceMaterial>
        </div>
    }
}
