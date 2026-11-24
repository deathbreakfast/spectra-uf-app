//! Curated seed dashboards for the Spectra app.
//!
//! Each preset is a fixed list of metric panels. The catalog needs no router IO: panels
//! name a metric, and the app queries each one with the caller's own permissions, so a
//! metric the host never registered shows an empty panel instead of failing the page.

use chrono::{DateTime, Utc};

use crate::dashboards::{ChartKind, Dashboard, Layout, Panel, PanelSource, Section, Window};
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

impl DashboardPreset {
    /// This preset as a built-in [`Dashboard`]: no owner, read-only, version 0, same id.
    ///
    /// Panels are time-series metric charts laid out two to a row. Each keeps the preset's
    /// look-back window.
    #[must_use]
    pub fn to_dashboard(&self) -> Dashboard {
        let panels = self
            .panels
            .iter()
            .enumerate()
            .map(|(i, panel)| Panel {
                id: format!("p{}", i + 1),
                title: panel.title.to_string(),
                source: PanelSource::Metric {
                    metric: panel.metric.to_string(),
                },
                chart: ChartKind::TimeSeries,
                measure: None,
                bucket: None,
                group_by: Vec::new(),
                filters: Vec::new(),
                table: None,
                layout: Layout {
                    col: if i % 2 == 0 { 0 } else { 6 },
                    span: 6,
                    row: u16::try_from(i / 2).unwrap_or(u16::MAX),
                    height: 4,
                },
                keep_window: Some(Window::Relative {
                    secs: panel.range_secs.unsigned_abs(),
                }),
            })
            .collect();
        Dashboard {
            id: self.id.to_string(),
            title: self.title.to_string(),
            description: self.description.to_string(),
            owner: None,
            read_only: true,
            sections: vec![Section {
                id: "main".into(),
                title: String::new(),
                collapsed: false,
                panels,
            }],
            variables: Vec::new(),
            filters: Vec::new(),
            version: 0,
            updated_at: DateTime::<Utc>::UNIX_EPOCH,
        }
    }
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

/// Every built-in dashboard as a read-only [`Dashboard`], in index order.
#[must_use]
pub fn builtin_dashboards() -> Vec<Dashboard> {
    PRESETS.iter().map(DashboardPreset::to_dashboard).collect()
}

/// The built-in dashboard with this id, if any.
#[must_use]
pub fn builtin_dashboard(id: &str) -> Option<Dashboard> {
    dashboard_preset(id).map(DashboardPreset::to_dashboard)
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
    fn builtins_keep_preset_ids_and_are_read_only_happy_path() {
        let builtins = builtin_dashboards();
        assert_eq!(builtins.len(), dashboard_presets().len());
        for (dashboard, preset) in builtins.iter().zip(dashboard_presets()) {
            assert_eq!(dashboard.id, preset.id);
            assert_eq!(dashboard.owner, None);
            assert!(dashboard.read_only);
            assert_eq!(dashboard.version, 0);
            assert_eq!(dashboard.validate(), Ok(()));
            let panels = &dashboard.sections[0].panels;
            assert_eq!(panels.len(), preset.panels.len());
            assert_eq!(
                panels[0].source,
                PanelSource::Metric {
                    metric: preset.panels[0].metric.to_string()
                }
            );
        }
    }

    #[test]
    fn builtin_lookup_unknown_id_sad_path() {
        assert!(builtin_dashboard("photon-throughput").is_some());
        assert!(builtin_dashboard("nope").is_none());
    }

    #[test]
    fn preset_lookup_unknown_id_sad_path() {
        assert!(dashboard_preset("nope").is_none());
        assert!(dashboard_preset("").is_none());
    }
}
