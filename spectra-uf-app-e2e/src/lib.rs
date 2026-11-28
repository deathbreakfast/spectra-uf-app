//! Spectra ops UI Playwright host.
#![allow(missing_docs)]
// Lab bootstrap panics on setup failure — same posture as chronon-uf-app-e2e.
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod app;
#[cfg(feature = "ssr")]
mod e2e_schemas;
#[cfg(feature = "ssr")]
mod e2e_spectra;
#[cfg(feature = "ssr")]
mod e2e_valence;
mod gate_demos;
mod harness_auth_menu;
#[cfg(feature = "ssr")]
pub mod seed;
mod spectra_routes_eager;

pub use app::{shell, wire_gauge_permissions_bridge, App};
#[cfg(feature = "ssr")]
pub use e2e_spectra::e2e_spectra;
#[cfg(feature = "ssr")]
pub use e2e_valence::{
    e2e_admin_valence, e2e_fixtures, e2e_higgs_config, e2e_router, e2e_system_valence,
    init_e2e_valence,
};
#[cfg(feature = "ssr")]
pub use gate_demos::inject_e2e_session_snapshot;
