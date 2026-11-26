//! The pieces of a query link and their URL tokens.

use chrono::{DateTime, Utc};

/// What a link queries: an event table or a metric family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuerySource {
    /// Event table, explored at `/spectra/schema/{table}/explore`.
    Event(String),
    /// Metric family, explored at `/spectra/metric/{metric}/explore`.
    Metric(String),
}

impl QuerySource {
    /// Table or metric name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Event(name) | Self::Metric(name) => name,
        }
    }
}

/// Relative window presets: the last 1h, 6h, 24h or 7d.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPreset {
    /// Last hour.
    Hour1,
    /// Last six hours.
    Hours6,
    /// Last 24 hours.
    Hours24,
    /// Last seven days.
    Days7,
}

impl WindowPreset {
    /// Every preset, shortest first.
    pub const ALL: [Self; 4] = [Self::Hour1, Self::Hours6, Self::Hours24, Self::Days7];

    /// URL token (`1h`, `6h`, `24h`, `7d`).
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Hour1 => "1h",
            Self::Hours6 => "6h",
            Self::Hours24 => "24h",
            Self::Days7 => "7d",
        }
    }

    /// Window length in seconds.
    #[must_use]
    pub const fn secs(self) -> i64 {
        match self {
            Self::Hour1 => 3_600,
            Self::Hours6 => 6 * 3_600,
            Self::Hours24 => 24 * 3_600,
            Self::Days7 => 7 * 24 * 3_600,
        }
    }

    /// Preset whose length is exactly `secs`.
    #[must_use]
    pub fn from_secs(secs: i64) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.secs() == secs)
    }

    pub(super) fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.token() == token)
    }
}

/// Time window of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryWindow {
    /// A window that ends now (`window=1h`).
    Last(WindowPreset),
    /// A fixed range (`from=…&to=…`, RFC 3339, whole seconds).
    Range {
        /// Range start.
        from: DateTime<Utc>,
        /// Range end.
        to: DateTime<Utc>,
    },
}

impl Default for QueryWindow {
    fn default() -> Self {
        Self::Last(WindowPreset::Hour1)
    }
}

impl QueryWindow {
    /// Start and end of the window, measuring a relative window back from `now`.
    #[must_use]
    pub fn bounds(self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        match self {
            Self::Last(preset) => (now - chrono::Duration::seconds(preset.secs()), now),
            Self::Range { from, to } => (from, to),
        }
    }
}

/// How a query is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryView {
    /// Event log rows (`log`).
    Log,
    /// Time series (`series`).
    Series,
    /// Bar chart (`bar`).
    Bar,
    /// Pie chart (`pie`).
    Pie,
    /// Pivot table (`table`).
    Table,
    /// Single headline value (`stat`).
    Stat,
}

impl QueryView {
    /// Every view.
    pub const ALL: [Self; 6] = [
        Self::Log,
        Self::Series,
        Self::Bar,
        Self::Pie,
        Self::Table,
        Self::Stat,
    ];

    /// URL token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Series => "series",
            Self::Bar => "bar",
            Self::Pie => "pie",
            Self::Table => "table",
            Self::Stat => "stat",
        }
    }

    pub(super) fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.token() == token)
    }
}

/// What a chart counts: rows, or the sum of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryMeasure {
    /// Row count (`measure=count`).
    Count,
    /// Sum of a numeric field (`measure=sum:<field>`).
    Sum(String),
}

/// Time bucket width, in seconds.
///
/// `1m`, `5m`, `15m`, `1h` and `1d` have their own tokens; any other width is written as
/// `<n>s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryBucket(u64);

impl QueryBucket {
    const NAMED: [(u64, &'static str); 5] = [
        (60, "1m"),
        (300, "5m"),
        (900, "15m"),
        (3_600, "1h"),
        (86_400, "1d"),
    ];

    /// Bucket of `secs` seconds; `None` for zero.
    #[must_use]
    pub const fn from_secs(secs: u64) -> Option<Self> {
        if secs == 0 {
            None
        } else {
            Some(Self(secs))
        }
    }

    /// Width in seconds.
    #[must_use]
    pub const fn secs(self) -> u64 {
        self.0
    }

    pub(super) fn token(self) -> String {
        Self::NAMED
            .iter()
            .find(|(secs, _)| *secs == self.0)
            .map_or_else(|| format!("{}s", self.0), |(_, t)| (*t).to_string())
    }

    pub(super) fn from_token(token: &str) -> Option<Self> {
        if let Some((secs, _)) = Self::NAMED.iter().find(|(_, t)| *t == token) {
            return Some(Self(*secs));
        }
        let digits = token.strip_suffix('s')?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok().and_then(Self::from_secs)
    }
}

/// Comparison in a link filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    /// Equals (`eq`).
    Eq,
    /// Does not equal (`ne`).
    Ne,
    /// Contains (`contains`).
    Contains,
    /// Greater than (`gt`).
    Gt,
    /// Less than (`lt`).
    Lt,
    /// Greater than or equal (`ge`).
    Ge,
    /// Less than or equal (`le`).
    Le,
    /// Starts with (`starts`).
    Starts,
    /// Ends with (`ends`).
    Ends,
    /// Is empty (`empty`); the value is ignored.
    Empty,
    /// Is not empty (`notempty`); the value is ignored.
    NotEmpty,
}

impl FilterOp {
    /// Every operator.
    pub const ALL: [Self; 11] = [
        Self::Eq,
        Self::Ne,
        Self::Contains,
        Self::Gt,
        Self::Lt,
        Self::Ge,
        Self::Le,
        Self::Starts,
        Self::Ends,
        Self::Empty,
        Self::NotEmpty,
    ];

    /// URL token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Contains => "contains",
            Self::Gt => "gt",
            Self::Lt => "lt",
            Self::Ge => "ge",
            Self::Le => "le",
            Self::Starts => "starts",
            Self::Ends => "ends",
            Self::Empty => "empty",
            Self::NotEmpty => "notempty",
        }
    }

    pub(super) fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.token() == token)
    }
}

/// One `f=field:op:value` filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkFilter {
    /// Field the filter tests.
    pub field: String,
    /// Comparison.
    pub op: FilterOp,
    /// Value compared against; may contain `:`.
    pub value: String,
}

/// Sort order (`sort=field:asc` or `sort=field:desc`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySort {
    /// Sorted field.
    pub field: String,
    /// Newest or largest first.
    pub descending: bool,
}

/// A whole explore query, as carried in its URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryLink {
    /// Table or metric.
    pub source: QuerySource,
    /// Time window; a link without one means the last hour.
    pub window: QueryWindow,
    /// View; `None` leaves the explorer's default.
    pub view: Option<QueryView>,
    /// Event chart measure.
    pub measure: Option<QueryMeasure>,
    /// Metric aggregation, such as `max` or `p95`.
    pub agg: Option<String>,
    /// Group-by field (Bar and Pie), or the Rows field of the Table view.
    pub group: Option<String>,
    /// Metric split label, or the Pivot field of the Table view.
    pub split: Option<String>,
    /// Time bucket width.
    pub bucket: Option<QueryBucket>,
    /// Filters, all of which must match.
    pub filters: Vec<LinkFilter>,
    /// Visible columns, in order.
    pub cols: Vec<String>,
    /// Sort order.
    pub sort: Option<QuerySort>,
    /// Row limit.
    pub limit: Option<u32>,
}

impl QueryLink {
    /// Link to `source` over the last hour with nothing else set.
    #[must_use]
    pub fn new(source: QuerySource) -> Self {
        Self {
            source,
            window: QueryWindow::default(),
            view: None,
            measure: None,
            agg: None,
            group: None,
            split: None,
            bucket: None,
            filters: Vec::new(),
            cols: Vec::new(),
            sort: None,
            limit: None,
        }
    }

    /// Link to an event table over the last hour.
    #[must_use]
    pub fn event(table: impl Into<String>) -> Self {
        Self::new(QuerySource::Event(table.into()))
    }

    /// Link to a metric over the last hour.
    #[must_use]
    pub fn metric(metric: impl Into<String>) -> Self {
        Self::new(QuerySource::Metric(metric.into()))
    }

    /// Field names the link refers to, in link order, without repeats.
    #[must_use]
    pub fn referenced_fields(&self) -> Vec<String> {
        let mut fields: Vec<&str> = Vec::new();
        if let Some(QueryMeasure::Sum(field)) = &self.measure {
            fields.push(field);
        }
        fields.extend(self.group.as_deref());
        fields.extend(self.split.as_deref());
        fields.extend(self.filters.iter().map(|f| f.field.as_str()));
        fields.extend(self.cols.iter().map(String::as_str));
        fields.extend(self.sort.as_ref().map(|s| s.field.as_str()));
        let mut out: Vec<String> = Vec::new();
        for field in fields {
            if !out.iter().any(|f| f == field) {
                out.push(field.to_string());
            }
        }
        out
    }
}

/// A decoded link and what the decoder couldn't use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The query the link describes.
    pub link: QueryLink,
    /// Fields the link names that the schema doesn't have. Empty until [`Decoded::resolve`]
    /// checks the link against a schema.
    pub unknown_fields: Vec<String>,
    /// Query keys the decoder didn't recognize; they're left out of `link`.
    pub ignored_keys: Vec<String>,
}
