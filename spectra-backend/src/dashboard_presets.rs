//! Curated seed dashboards for the Spectra app.
//!
//! Each preset is a fixed list of metric panels. The catalog needs no router IO: panels
//! name a metric, and the app queries each one with the caller's own permissions, so a
//! metric the host never registered shows an empty panel instead of failing the page.

use crate::spectra_metric_explore_path;

/// One metric chart on a dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardPanel {
    /// Panel heading.
    pub title: &'static str,
    /// Metric name queried for the panel.
    pub metric: &'static str,
    /// Look-back window in seconds.
    pub range_secs: i64,
}

impl DashboardPanel {
    /// Metric explore href for this panel.
    #[must_use]
    pub fn explore_path(&self) -> String {
        spectra_metric_explore_path(self.metric)
    }
}

/// A named, code-defined dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardPreset {
    /// Stable id used in `/spectra/dashboards/{id}`.
    pub id: &'static str,
    /// Display title.
    pub title: &'static str,
    /// One-line summary shown on the dashboards index.
    pub description: &'static str,
    /// Panels in display order.
    pub panels: &'static [DashboardPanel],
}

const ONE_HOUR_SECS: i64 = 3_600;

static PRESETS: &[DashboardPreset] = &[
    DashboardPreset {
        id: "photon-throughput",
        title: "Photon throughput",
        description: "Publishes, drains, backlog, and failures on the Photon event log",
        panels: &[
            DashboardPanel {
                title: "Publishes",
                metric: "photon_publishes",
                range_secs: ONE_HOUR_SECS,
            },
            DashboardPanel {
                title: "Drains",
                metric: "photon_drains",
                range_secs: ONE_HOUR_SECS,
            },
            DashboardPanel {
                title: "Backlog",
                metric: "photon_backlog",
                range_secs: ONE_HOUR_SECS,
            },
            DashboardPanel {
                title: "Publish errors",
                metric: "photon_publish_errors",
                range_secs: ONE_HOUR_SECS,
            },
        ],
    },
    DashboardPreset {
        id: "valence-db-activity",
        title: "Valence DB activity",
        description: "Reads, writes, errors, and wall time on Valence database calls",
        panels: &[
            DashboardPanel {
                title: "Reads",
                metric: "valence_db_reads",
                range_secs: ONE_HOUR_SECS,
            },
            DashboardPanel {
                title: "Writes",
                metric: "valence_db_writes",
                range_secs: ONE_HOUR_SECS,
            },
            DashboardPanel {
                title: "Errors",
                metric: "valence_db_errors",
                range_secs: ONE_HOUR_SECS,
            },
            DashboardPanel {
                title: "Wall time (ms)",
                metric: "valence_db_wall_ms",
                range_secs: ONE_HOUR_SECS,
            },
        ],
    },
];

/// Every curated dashboard, in index order.
#[must_use]
pub fn dashboard_presets() -> &'static [DashboardPreset] {
    PRESETS
}

/// Looks up a preset by id; `None` for an unknown id.
#[must_use]
pub fn dashboard_preset(id: &str) -> Option<&'static DashboardPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// `/spectra/dashboards/{id}` href.
#[must_use]
pub fn spectra_dashboard_path(id: &str) -> String {
    format!("/spectra/dashboards/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_spectra_query_name;
    use std::collections::HashSet;

    #[test]
    fn preset_ids_are_unique_happy_path() {
        let ids: HashSet<_> = dashboard_presets().iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), dashboard_presets().len());
    }

    #[test]
    fn panel_metrics_pass_name_validation_happy_path() {
        for preset in dashboard_presets() {
            assert!(!preset.panels.is_empty(), "{} has no panels", preset.id);
            for panel in preset.panels {
                assert!(validate_spectra_query_name(panel.metric).is_ok());
                assert!(panel.range_secs > 0);
            }
        }
    }

    #[test]
    fn preset_lookup_finds_known_id_happy_path() {
        let preset = dashboard_preset("photon-throughput").expect("photon preset");
        assert_eq!(preset.title, "Photon throughput");
        assert_eq!(
            preset.panels[0].explore_path(),
            "/spectra/metric/photon_publishes/explore"
        );
    }

    #[test]
    fn preset_lookup_unknown_id_sad_path() {
        assert!(dashboard_preset("nope").is_none());
        assert!(dashboard_preset("").is_none());
    }
}
