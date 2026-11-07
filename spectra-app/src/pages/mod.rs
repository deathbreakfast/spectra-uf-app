pub mod callsite_graph;
pub mod dashboards;
pub mod event_explore;
pub mod home;
pub mod metric_explore;
pub mod schema_detail;
pub mod schema_index;

pub use callsite_graph::CallsiteGraphPage;
pub use dashboards::{DashboardDetailPage, DashboardIndexPage};
pub use event_explore::EventExplorePage;
pub use home::SpectraHomePage;
pub use metric_explore::MetricExplorePage;
pub use schema_detail::SchemaDetailPage;
pub use schema_index::SchemaIndexPage;
