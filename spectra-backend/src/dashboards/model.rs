//! Saved-dashboard types shared by the store, the server functions and the pages.
//!
//! Everything here is plain serde data with no Valence or Gauge dependency, so it compiles
//! into the hydrate (wasm) build. A panel holds only its query, never query results: reading
//! a panel runs that query as the viewer, so a viewer without `spectra.query.{table}` gets a
//! permission error for that one panel (see [`Panel::required_query_permission`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{spectra_query_permission_name, validate_spectra_query_name};

/// Width of the dashboard grid in columns.
pub const DASHBOARD_GRID_COLUMNS: u8 = 12;
/// Longest title accepted on a dashboard, section or panel, in characters.
pub const MAX_DASHBOARD_TITLE_CHARS: usize = 200;
/// Longest dashboard description accepted, in characters.
pub const MAX_DASHBOARD_DESCRIPTION_CHARS: usize = 2_000;
/// Largest row limit a table panel may ask for.
pub const MAX_TABLE_PANEL_ROWS: u32 = 10_000;

/// A saved dashboard, or a built-in one when [`Dashboard::owner`] is `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dashboard {
    /// Stable id used in `/spectra/dashboards/{id}`.
    pub id: String,
    /// Display title.
    pub title: String,
    /// One-line summary shown on the dashboards list.
    pub description: String,
    /// User id of the owner. `None` for built-in dashboards.
    pub owner: Option<String>,
    /// True for built-in dashboards, which nobody can save, share or delete.
    pub read_only: bool,
    /// Sections in display order.
    pub sections: Vec<Section>,
    /// Dashboard-wide variables panels can refer to.
    pub variables: Vec<Variable>,
    /// Filters applied to every panel whose source has the field.
    pub filters: Vec<DashboardFilter>,
    /// Number of the newest saved version. Built-ins stay at 0.
    pub version: u32,
    /// When the newest version was saved.
    pub updated_at: DateTime<Utc>,
}

/// A titled group of panels on a dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    /// Id unique within the dashboard.
    pub id: String,
    /// Section heading; empty for an untitled section.
    pub title: String,
    /// Whether the section starts collapsed.
    pub collapsed: bool,
    /// Panels in display order.
    pub panels: Vec<Panel>,
}

/// One chart, table or text block on a dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Panel {
    /// Id unique within the dashboard.
    pub id: String,
    /// Panel heading.
    pub title: String,
    /// What the panel reads.
    pub source: PanelSource,
    /// How the panel draws its result.
    pub chart: ChartKind,
    /// Aggregate to plot, such as `count` or `sum(duration_ms)`. `None` uses the source default.
    pub measure: Option<String>,
    /// Time bucket for series, such as `1m` or `1h`. `None` picks one from the window.
    pub bucket: Option<String>,
    /// Fields to split the result by.
    pub group_by: Vec<String>,
    /// Filters for this panel only, on top of the dashboard's.
    pub filters: Vec<DashboardFilter>,
    /// Columns, sort and row limit for [`ChartKind::Table`] panels.
    pub table: Option<TableOpts>,
    /// Where the panel sits on the grid.
    pub layout: Layout,
    /// When set, the panel keeps this window instead of following the dashboard's range.
    pub keep_window: Option<Window>,
}

/// What a panel reads.
///
/// Serialized with a `kind` tag. More sources (saved queries, once that design is done) are
/// added as new variants, so match with a wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PanelSource {
    /// A Spectra metric.
    Metric {
        /// Metric name.
        metric: String,
    },
    /// A Spectra event table.
    EventTable {
        /// Event table name.
        table: String,
    },
    /// Markdown text with no query behind it.
    Text {
        /// Markdown body.
        markdown: String,
    },
}

/// How a panel draws its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    /// Line chart over time.
    TimeSeries,
    /// Bar chart.
    Bar,
    /// Pie chart.
    Pie,
    /// Rows in a table.
    Table,
    /// One headline number.
    SingleStat,
}

/// Columns, sort and row limit for a table panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableOpts {
    /// Columns to show, in order. Empty shows the source's default columns.
    pub columns: Vec<String>,
    /// Column to sort by, if any.
    pub sort: Option<TableSort>,
    /// Most rows to show.
    pub limit: u32,
}

/// Sort order for a table panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSort {
    /// Column to sort by.
    pub column: String,
    /// Descending when true.
    pub descending: bool,
}

/// A panel's place on the twelve-column grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    /// First column, from 0.
    pub col: u8,
    /// Columns the panel spans, from 1 to 12.
    pub span: u8,
    /// Row, from 0.
    pub row: u16,
    /// Height in grid rows, at least 1.
    pub height: u16,
}

/// A time window for a panel or a dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Window {
    /// The last `secs` seconds up to now.
    Relative {
        /// Length of the window in seconds.
        secs: u64,
    },
    /// A fixed range.
    Absolute {
        /// Start of the range.
        from: DateTime<Utc>,
        /// End of the range.
        to: DateTime<Utc>,
    },
}

/// A dashboard-wide variable, such as `service`, that panels filter on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    /// Variable name.
    pub name: String,
    /// Source field the variable filters.
    pub field: String,
    /// Value used until the viewer picks one.
    pub default: Option<String>,
}

/// One filter on a dashboard or a panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardFilter {
    /// Field to filter on.
    pub field: String,
    /// Comparison.
    pub op: FilterOp,
    /// Value to compare with.
    pub value: String,
}

/// Comparison used by a [`DashboardFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    /// Equal to.
    Eq,
    /// Not equal to.
    Ne,
    /// Contains, for text.
    Contains,
    /// Greater than.
    Gt,
    /// Less than.
    Lt,
}

/// Which dashboards [`list_dashboards`](crate::dashboards) returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardScope {
    /// Dashboards the caller owns.
    Mine,
    /// Dashboards shared with the caller that someone else owns.
    Shared,
    /// Everything the caller can read, built-ins included.
    #[default]
    All,
}

/// One row of the dashboards list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardSummary {
    /// Dashboard id.
    pub id: String,
    /// Display title.
    pub title: String,
    /// One-line summary.
    pub description: String,
    /// Owner user id; `None` for built-ins.
    pub owner: Option<String>,
    /// True for built-ins.
    pub read_only: bool,
    /// Whether the caller marked it as a favourite.
    pub favourite: bool,
    /// Newest version number.
    pub version: u32,
    /// When the newest version was saved.
    pub updated_at: DateTime<Utc>,
    /// People and groups the dashboard is shared with. Filled only on the owner's own
    /// dashboards; empty for everyone else and for built-ins.
    #[serde(default)]
    pub shared_with: Vec<Principal>,
}

/// One saved version of a dashboard. Versions are kept for 180 days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    /// Version number, from 1.
    pub number: u32,
    /// User id of the person who saved it; `None` once that user is deleted.
    pub author: Option<String>,
    /// Agent that saved it for the author, such as `MCP`. `None` for a save from the app.
    pub via: Option<String>,
    /// When it was saved.
    pub created_at: DateTime<Utc>,
    /// One-line summary of the change.
    pub summary: String,
}

/// A person or group a dashboard is shared with.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Principal {
    /// A user, by user id.
    User(String),
    /// A Gauge group, by group id.
    Group(String),
}

/// Who can reach a dashboard, and what the caller may do with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardAccess {
    /// Owner user id; `None` for built-ins.
    pub owner: Option<String>,
    /// People and groups who can edit. The owner sees Gauge's grants; others see the copy
    /// saved the last time the owner shared it.
    pub editors: Vec<Principal>,
    /// People and groups who can only view, filled in the same way as `editors`.
    pub viewers: Vec<Principal>,
    /// Whether the caller can save changes.
    pub can_edit: bool,
    /// Whether the caller can delete the dashboard.
    pub can_delete: bool,
    /// Whether the caller can change who has access.
    pub can_share: bool,
}

/// Why a dashboard operation failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DashboardError {
    /// No dashboard (or version) with that id that the caller can read.
    NotFound,
    /// Someone saved a newer version first; reload version `current` and reapply the change.
    Conflict {
        /// Newest stored version.
        current: u32,
    },
    /// Built-in dashboards can't be saved, shared or deleted.
    ReadOnly,
    /// The caller can read the dashboard but lacks the right to do this.
    Forbidden,
    /// The dashboard failed validation; the message says which field.
    Validation {
        /// What was wrong.
        message: String,
    },
    /// The store or Gauge failed.
    Store {
        /// Underlying error text.
        message: String,
    },
}

/// Marker that starts an encoded [`DashboardError`] inside a server-function error message.
pub const DASHBOARD_ERROR_PREFIX: &str = "DashboardError:";

impl DashboardError {
    /// Shorthand for [`DashboardError::Validation`].
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Shorthand for [`DashboardError::Store`].
    #[must_use]
    pub fn store(message: impl std::fmt::Display) -> Self {
        Self::Store {
            message: message.to_string(),
        }
    }

    /// Encode as `DashboardError:{json}` so the error survives a server-function boundary.
    #[must_use]
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_else(|_| r#"{"kind":"store"}"#.into());
        format!("{DASHBOARD_ERROR_PREFIX}{json}")
    }

    /// Find and decode an error written by [`DashboardError::encode`] anywhere in `message`.
    ///
    /// Leptos may wrap server-function messages in extra text, so this looks for the marker
    /// rather than requiring it at the start. `None` when there is no encoded error.
    #[must_use]
    pub fn decode(message: &str) -> Option<Self> {
        let start = message.find(DASHBOARD_ERROR_PREFIX)? + DASHBOARD_ERROR_PREFIX.len();
        let mut stream = serde_json::Deserializer::from_str(&message[start..]).into_iter::<Self>();
        stream.next()?.ok()
    }
}

impl std::fmt::Display for DashboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => f.write_str("Dashboard not found"),
            Self::Conflict { current } => write!(
                f,
                "Someone else saved this dashboard first (version {current}). Reload it and make your change again."
            ),
            Self::ReadOnly => f.write_str("Built-in dashboards can't be changed. Duplicate it to make your own copy."),
            Self::Forbidden => f.write_str("You don't have access to do that on this dashboard"),
            Self::Validation { message } => f.write_str(message),
            Self::Store { message } => write!(f, "Dashboard store error: {message}"),
        }
    }
}

impl std::error::Error for DashboardError {}

impl Panel {
    /// Gauge permission a viewer needs to read this panel's data, or `None` for a text panel.
    #[must_use]
    pub fn required_query_permission(&self) -> Option<String> {
        match &self.source {
            PanelSource::Metric { metric } => Some(spectra_query_permission_name(metric)),
            PanelSource::EventTable { table } => Some(spectra_query_permission_name(table)),
            PanelSource::Text { .. } => None,
        }
    }

    fn validate(&self) -> Result<(), DashboardError> {
        check_title("Panel title", &self.title, true)?;
        match &self.source {
            PanelSource::Metric { metric: name } | PanelSource::EventTable { table: name } => {
                validate_spectra_query_name(name).map_err(|e| {
                    DashboardError::validation(format!("Panel \"{}\": {e}", self.title))
                })?;
            }
            PanelSource::Text { .. } => {}
        }
        let Layout {
            col,
            span,
            row: _,
            height,
        } = self.layout;
        if span == 0
            || height == 0
            || u16::from(col) + u16::from(span) > u16::from(DASHBOARD_GRID_COLUMNS)
        {
            return Err(DashboardError::validation(format!(
                "Panel \"{}\" doesn't fit the {DASHBOARD_GRID_COLUMNS}-column grid",
                self.title
            )));
        }
        if let Some(table) = &self.table {
            if table.limit == 0 || table.limit > MAX_TABLE_PANEL_ROWS {
                return Err(DashboardError::validation(format!(
                    "Panel \"{}\" row limit must be between 1 and {MAX_TABLE_PANEL_ROWS}",
                    self.title
                )));
            }
        }
        Ok(())
    }
}

fn check_title(label: &str, value: &str, allow_empty: bool) -> Result<(), DashboardError> {
    if !allow_empty && value.trim().is_empty() {
        return Err(DashboardError::validation(format!("{label} is required")));
    }
    if value.chars().count() > MAX_DASHBOARD_TITLE_CHARS {
        return Err(DashboardError::validation(format!(
            "{label} is longer than {MAX_DASHBOARD_TITLE_CHARS} characters"
        )));
    }
    Ok(())
}

/// Check a new dashboard's title and description.
///
/// # Errors
///
/// [`DashboardError::Validation`] for a blank or overlong title, or an overlong description.
pub fn validate_dashboard_text(title: &str, description: &str) -> Result<(), DashboardError> {
    check_title("Dashboard title", title, false)?;
    if description.chars().count() > MAX_DASHBOARD_DESCRIPTION_CHARS {
        return Err(DashboardError::validation(format!(
            "Description is longer than {MAX_DASHBOARD_DESCRIPTION_CHARS} characters"
        )));
    }
    Ok(())
}

impl Dashboard {
    /// Check everything a save would store: title, description, section and panel ids,
    /// panel sources and layouts.
    ///
    /// # Errors
    ///
    /// [`DashboardError::Validation`] naming the first problem found.
    pub fn validate(&self) -> Result<(), DashboardError> {
        validate_dashboard_text(&self.title, &self.description)?;
        let mut section_ids = std::collections::HashSet::new();
        let mut panel_ids = std::collections::HashSet::new();
        for section in &self.sections {
            check_title("Section title", &section.title, true)?;
            if section.id.trim().is_empty() || !section_ids.insert(section.id.as_str()) {
                return Err(DashboardError::validation(format!(
                    "Section id \"{}\" is blank or used twice",
                    section.id
                )));
            }
            for panel in &section.panels {
                if panel.id.trim().is_empty() || !panel_ids.insert(panel.id.as_str()) {
                    return Err(DashboardError::validation(format!(
                        "Panel id \"{}\" is blank or used twice",
                        panel.id
                    )));
                }
                panel.validate()?;
            }
        }
        Ok(())
    }

    /// The list row for this dashboard.
    #[must_use]
    pub fn summary(&self, favourite: bool) -> DashboardSummary {
        DashboardSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            owner: self.owner.clone(),
            read_only: self.read_only,
            favourite,
            version: self.version,
            updated_at: self.updated_at,
            shared_with: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(id: &str, source: PanelSource) -> Panel {
        Panel {
            id: id.into(),
            title: "Requests".into(),
            source,
            chart: ChartKind::Table,
            measure: Some("count".into()),
            bucket: None,
            group_by: vec!["route".into()],
            filters: vec![DashboardFilter {
                field: "status".into(),
                op: FilterOp::Gt,
                value: "499".into(),
            }],
            table: Some(TableOpts {
                columns: vec!["ts".into(), "route".into()],
                sort: Some(TableSort {
                    column: "ts".into(),
                    descending: true,
                }),
                limit: 50,
            }),
            layout: Layout {
                col: 0,
                span: 6,
                row: 0,
                height: 4,
            },
            keep_window: Some(Window::Relative { secs: 3_600 }),
        }
    }

    fn dashboard(panels: Vec<Panel>) -> Dashboard {
        Dashboard {
            id: "d1".into(),
            title: "Checkout health".into(),
            description: String::new(),
            owner: Some("alice".into()),
            read_only: false,
            sections: vec![Section {
                id: "s1".into(),
                title: String::new(),
                collapsed: false,
                panels,
            }],
            variables: vec![Variable {
                name: "service".into(),
                field: "service".into(),
                default: None,
            }],
            filters: Vec::new(),
            version: 1,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn event_table_panel_round_trips_through_json_happy_path() {
        let p = panel(
            "p1",
            PanelSource::EventTable {
                table: "checkout.events".into(),
            },
        );
        let json = serde_json::to_value(&p).expect("serialize");
        assert_eq!(json["source"]["kind"], "event_table");
        assert_eq!(json["chart"], "table");
        let back: Panel = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, p);
    }

    #[test]
    fn unknown_panel_source_kind_is_rejected_sad_path() {
        let json = serde_json::json!({ "kind": "saved_query", "name": "x" });
        assert!(serde_json::from_value::<PanelSource>(json).is_err());
    }

    #[test]
    fn panel_permission_follows_its_source_happy_path() {
        let metric = panel(
            "p",
            PanelSource::Metric {
                metric: "photon_publishes".into(),
            },
        );
        assert_eq!(
            metric.required_query_permission().as_deref(),
            Some("spectra.query.photon_publishes")
        );
        let text = panel(
            "t",
            PanelSource::Text {
                markdown: "# Notes".into(),
            },
        );
        assert_eq!(text.required_query_permission(), None);
    }

    #[test]
    fn valid_dashboard_passes_validation_happy_path() {
        let d = dashboard(vec![panel(
            "p1",
            PanelSource::EventTable {
                table: "checkout.events".into(),
            },
        )]);
        assert_eq!(d.validate(), Ok(()));
    }

    #[test]
    fn duplicate_panel_ids_fail_validation_sad_path() {
        let source = PanelSource::Metric { metric: "m".into() };
        let d = dashboard(vec![panel("p1", source.clone()), panel("p1", source)]);
        assert!(matches!(
            d.validate(),
            Err(DashboardError::Validation { .. })
        ));
    }

    #[test]
    fn panel_past_the_grid_edge_fails_validation_sad_path() {
        let mut p = panel("p1", PanelSource::Metric { metric: "m".into() });
        p.layout.col = 8;
        p.layout.span = 6;
        assert!(matches!(
            dashboard(vec![p]).validate(),
            Err(DashboardError::Validation { .. })
        ));
    }

    #[test]
    fn unsafe_table_name_fails_validation_sad_path() {
        let p = panel(
            "p1",
            PanelSource::EventTable {
                table: "../etc".into(),
            },
        );
        assert!(matches!(
            dashboard(vec![p]).validate(),
            Err(DashboardError::Validation { .. })
        ));
    }

    #[test]
    fn blank_title_fails_validation_sad_path() {
        assert!(validate_dashboard_text("  ", "").is_err());
        assert!(validate_dashboard_text("Ok", "").is_ok());
    }

    #[test]
    fn error_survives_encode_inside_wrapped_message_happy_path() {
        let err = DashboardError::Conflict { current: 7 };
        let wrapped = format!("error running server function: {}", err.encode());
        assert_eq!(DashboardError::decode(&wrapped), Some(err));
    }

    #[test]
    fn plain_message_decodes_to_none_sad_path() {
        assert_eq!(DashboardError::decode("Permission denied: x"), None);
        assert_eq!(DashboardError::decode("DashboardError:not json"), None);
    }
}
