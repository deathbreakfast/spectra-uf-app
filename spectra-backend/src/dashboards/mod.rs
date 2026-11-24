//! Saved, editable dashboards.
//!
//! The types ([`Dashboard`], [`Panel`] and the rest) are in every build. With the `ssr`
//! feature, the Valence store lives in `store`: three tables (dashboards, their saved
//! versions, and per-user favourites), with access through each dashboard's Gauge resource
//! bundle (`policies::SPECTRA_DASHBOARD`). The code presets in `dashboard_presets` appear in the
//! store's list and get as read-only built-ins.

mod model;

pub use model::{
    validate_dashboard_text, ChartKind, Dashboard, DashboardAccess, DashboardError,
    DashboardFilter, DashboardScope, DashboardSummary, FilterOp, Layout, Panel, PanelSource,
    Principal, Section, TableOpts, TableSort, Variable, Version, Window, DASHBOARD_ERROR_PREFIX,
    DASHBOARD_GRID_COLUMNS, MAX_DASHBOARD_DESCRIPTION_CHARS, MAX_DASHBOARD_TITLE_CHARS,
    MAX_TABLE_PANEL_ROWS,
};

#[cfg(feature = "ssr")]
pub mod policies;
#[cfg(feature = "ssr")]
pub mod sharing;
#[cfg(feature = "ssr")]
pub mod store;

/// Valence storage for the dashboard tables.
///
/// The same logical database as Gauge's tables, so any host that routes Gauge (which
/// dashboard sharing needs anyway) also routes dashboards.
#[cfg(feature = "ssr")]
pub const DASHBOARD_STORAGE: valence::DatabaseFromEngine = gauge::embedded_surreal::DEFAULT_STORAGE;
