use leptos::prelude::*;
use orbital::components::{Caption1, EmptyState};
use orbital_charts::{LineChart, TooltipConfig};
use spectra_core::TimeSeriesDto;

use super::chart_series::chart_from_time_series;
use super::chart_surface_material::ChartSurfaceMaterial;

#[component]
pub fn MetricTimeSeriesChart(
    /// Metric time series to render.
    series: Vec<TimeSeriesDto>,
) -> impl IntoView {
    if series.iter().all(|s| s.points.is_empty()) {
        return view! {
            <div data-testid="spectra-metric-time-series-chart">
                <EmptyState
                    message="No series data"
                    description="Nothing to chart for this metric and time range."
                />
            </div>
        }
        .into_any();
    }

    let (x_axis, chart_series) = chart_from_time_series(&series);
    view! {
        <div data-testid="spectra-metric-time-series-chart" style="width: 100%; height: 100%;">
            <ChartSurfaceMaterial>
                <Caption1>"Time series"</Caption1>
                <div style="width: 100%; flex: 1 1 auto; min-height: 320px;">
                    <LineChart
                        x_axis=x_axis
                        series=chart_series
                        responsive=true
                        tooltip=TooltipConfig::axis()
                    />
                </div>
            </ChartSurfaceMaterial>
        </div>
    }
    .into_any()
}
