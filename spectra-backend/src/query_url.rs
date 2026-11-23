//! Query links: one URL format for every Spectra explore query.
//!
//! A [`QueryLink`] is the whole query an explorer shows: the table or metric, the time
//! window, the view, and the aggregation and filter settings. [`QueryLink::encode`] writes
//! it as an explore path with a query string; [`decode`] reads one back from a path or a
//! full URL on any host. The explorers keep their state in this URL, and dashboards read
//! pasted links with the same decoder.
//!
//! ```text
//! /spectra/schema/{table}/explore?window=1h&view=bar&measure=count&group=status&f=status:eq:failed
//! /spectra/metric/{metric}/explore?window=6h&agg=max&bucket=5m&split=route
//! ```

use std::fmt;

use chrono::{DateTime, SecondsFormat, Utc};
use spectra_core::SchemaDetailDto;

use crate::{encode_ops_path_segment, validate_spectra_query_name, SpectraQueryNameError};

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

    fn from_token(token: &str) -> Option<Self> {
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

    fn from_token(token: &str) -> Option<Self> {
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

    fn token(self) -> String {
        Self::NAMED
            .iter()
            .find(|(secs, _)| *secs == self.0)
            .map_or_else(|| format!("{}s", self.0), |(_, t)| (*t).to_string())
    }

    fn from_token(token: &str) -> Option<Self> {
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

    fn from_token(token: &str) -> Option<Self> {
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

    /// Reads a link from an explore path or a full URL; same as [`decode`].
    ///
    /// # Errors
    ///
    /// See [`decode`].
    pub fn decode(input: &str) -> Result<Decoded, QueryLinkError> {
        decode(input)
    }

    /// Path and query string, such as
    /// `/spectra/schema/checkout_event/explore?window=1h&view=log`.
    ///
    /// Keys always come out in the same order, so equal links encode to equal strings.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = match &self.source {
            QuerySource::Event(table) => {
                format!("/spectra/schema/{}/explore", encode_ops_path_segment(table))
            }
            QuerySource::Metric(metric) => {
                format!(
                    "/spectra/metric/{}/explore",
                    encode_ops_path_segment(metric)
                )
            }
        };
        let mut pairs: Vec<(&str, String)> = Vec::new();
        match self.window {
            QueryWindow::Last(preset) => pairs.push(("window", preset.token().to_string())),
            QueryWindow::Range { from, to } => {
                pairs.push(("from", encode_value(&rfc3339(from))));
                pairs.push(("to", encode_value(&rfc3339(to))));
            }
        }
        if let Some(view) = self.view {
            pairs.push(("view", view.token().to_string()));
        }
        match &self.measure {
            Some(QueryMeasure::Count) => pairs.push(("measure", "count".into())),
            Some(QueryMeasure::Sum(field)) => {
                pairs.push(("measure", format!("sum:{}", encode_value(field))));
            }
            None => {}
        }
        if let Some(agg) = &self.agg {
            pairs.push(("agg", encode_value(agg)));
        }
        if let Some(group) = &self.group {
            pairs.push(("group", encode_value(group)));
        }
        if let Some(split) = &self.split {
            pairs.push(("split", encode_value(split)));
        }
        if let Some(bucket) = self.bucket {
            pairs.push(("bucket", bucket.token()));
        }
        for f in &self.filters {
            pairs.push((
                "f",
                format!(
                    "{}:{}:{}",
                    encode_ops_path_segment(&f.field),
                    f.op.token(),
                    encode_value(&f.value)
                ),
            ));
        }
        if !self.cols.is_empty() {
            let cols: Vec<String> = self
                .cols
                .iter()
                .map(|c| encode_ops_path_segment(c))
                .collect();
            pairs.push(("cols", cols.join(",")));
        }
        if let Some(sort) = &self.sort {
            let dir = if sort.descending { "desc" } else { "asc" };
            pairs.push((
                "sort",
                format!("{}:{dir}", encode_ops_path_segment(&sort.field)),
            ));
        }
        if let Some(limit) = self.limit {
            pairs.push(("limit", limit.to_string()));
        }
        for (i, (key, value)) in pairs.iter().enumerate() {
            out.push(if i == 0 { '?' } else { '&' });
            out.push_str(key);
            out.push('=');
            out.push_str(value);
        }
        out
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

impl Decoded {
    /// Checks the link against `schema` and fills [`Decoded::unknown_fields`].
    #[must_use]
    pub fn resolve(mut self, schema: &SchemaDetailDto) -> Self {
        self.unknown_fields = resolve(&self.link, schema);
        self
    }
}

/// Fields `link` names that `schema` doesn't have, in link order.
///
/// `ts` is always known. The link itself is left as it is: an unknown field stays in the
/// query, so the caller can tell the reader about it instead of guessing.
#[must_use]
pub fn resolve(link: &QueryLink, schema: &SchemaDetailDto) -> Vec<String> {
    link.referenced_fields()
        .into_iter()
        .filter(|field| field != "ts" && !schema.fields.iter().any(|f| &f.name == field))
        .collect()
}

/// Why a string isn't a usable query link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryLinkError {
    /// The path isn't a Spectra explore path.
    NotSpectraLink,
    /// The table or metric name in the path is blank or unsafe.
    InvalidName(SpectraQueryNameError),
    /// A `%` escape is malformed, or the decoded text isn't UTF-8.
    InvalidEncoding,
    /// A key has a value the format doesn't allow.
    InvalidValue {
        /// Query key.
        key: String,
        /// Value as decoded.
        value: String,
    },
    /// A key that may appear once appears more than once.
    DuplicateKey(String),
    /// `from` without `to`, or `to` without `from`.
    IncompleteRange,
    /// Both `window` and `from`/`to` are set.
    ConflictingWindow,
}

impl fmt::Display for QueryLinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSpectraLink => write!(f, "This isn't a link to a Spectra query"),
            Self::InvalidName(err) => write!(f, "{err}"),
            Self::InvalidEncoding => write!(f, "The link has a malformed % escape"),
            Self::InvalidValue { key, value } => {
                write!(f, "The link's {key} value \"{value}\" isn't valid")
            }
            Self::DuplicateKey(key) => write!(f, "The link sets {key} more than once"),
            Self::IncompleteRange => write!(f, "The link needs both from and to"),
            Self::ConflictingWindow => write!(f, "The link sets both window and from/to"),
        }
    }
}

impl std::error::Error for QueryLinkError {}

/// Reads a query link from an explore path or a full URL.
///
/// Any scheme and host are accepted; only the path and query string matter. A trailing
/// `#fragment` is ignored. Unrecognized keys are listed in [`Decoded::ignored_keys`].
///
/// # Errors
///
/// [`QueryLinkError`] when the path isn't `/spectra/schema/{table}/explore` or
/// `/spectra/metric/{metric}/explore`, when a value is malformed, or when a single-use key
/// repeats.
pub fn decode(input: &str) -> Result<Decoded, QueryLinkError> {
    let input = input.trim();
    let input = input.split_once('#').map_or(input, |(before, _)| before);
    let rest = match input.split_once("://") {
        Some((_, after)) => after.find('/').map_or("", |slash| &after[slash..]),
        None => input,
    };
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut link = QueryLink::new(parse_source(path)?);
    let mut ignored_keys: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut window: Option<WindowPreset> = None;
    let mut from: Option<DateTime<Utc>> = None;
    let mut to: Option<DateTime<Utc>> = None;

    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode(raw_key)?;
        let known = KEYS.iter().find(|k| **k == key);
        let Some(&key) = known else {
            if !ignored_keys.contains(&key) {
                ignored_keys.push(key);
            }
            continue;
        };
        if key != "f" {
            if seen.contains(&key) {
                return Err(QueryLinkError::DuplicateKey(key.to_string()));
            }
            seen.push(key);
        }
        if key == "f" {
            link.filters.push(parse_filter(raw_value)?);
            continue;
        }
        if key == "cols" {
            link.cols = raw_value
                .split(',')
                .map(|c| non_empty(key, percent_decode(c)?))
                .collect::<Result<_, _>>()?;
            continue;
        }
        if key == "sort" {
            link.sort = Some(parse_sort(raw_value)?);
            continue;
        }
        let value = percent_decode(raw_value)?;
        let invalid = || QueryLinkError::InvalidValue {
            key: key.to_string(),
            value: value.clone(),
        };
        match key {
            "window" => window = Some(WindowPreset::from_token(&value).ok_or_else(invalid)?),
            "from" => from = Some(parse_time(&value).ok_or_else(invalid)?),
            "to" => to = Some(parse_time(&value).ok_or_else(invalid)?),
            "view" => link.view = Some(QueryView::from_token(&value).ok_or_else(invalid)?),
            "measure" => {
                link.measure = Some(if value == "count" {
                    QueryMeasure::Count
                } else {
                    let field = value.strip_prefix("sum:").ok_or_else(invalid)?;
                    QueryMeasure::Sum(non_empty(key, field.to_string())?)
                });
            }
            "agg" => link.agg = Some(non_empty(key, value.clone())?),
            "group" => link.group = Some(non_empty(key, value.clone())?),
            "split" => link.split = Some(non_empty(key, value.clone())?),
            "bucket" => link.bucket = Some(QueryBucket::from_token(&value).ok_or_else(invalid)?),
            "limit" => {
                link.limit = Some(value.parse().ok().filter(|n| *n > 0).ok_or_else(invalid)?);
            }
            _ => {}
        }
    }

    link.window = match (window, from, to) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            return Err(QueryLinkError::ConflictingWindow)
        }
        (Some(preset), None, None) => QueryWindow::Last(preset),
        (None, Some(from), Some(to)) => QueryWindow::Range { from, to },
        (None, None, None) => QueryWindow::default(),
        (None, _, _) => return Err(QueryLinkError::IncompleteRange),
    };

    Ok(Decoded {
        link,
        unknown_fields: Vec::new(),
        ignored_keys,
    })
}

const KEYS: [&str; 13] = [
    "window", "from", "to", "view", "measure", "agg", "group", "split", "bucket", "f", "cols",
    "sort", "limit",
];

fn parse_source(path: &str) -> Result<QuerySource, QueryLinkError> {
    let path = path.strip_suffix('/').unwrap_or(path);
    let mut segments = path.split('/');
    let shape = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    );
    let (Some(""), Some("spectra"), Some(kind), Some(raw_name), Some("explore"), None) = shape
    else {
        return Err(QueryLinkError::NotSpectraLink);
    };
    let name = percent_decode(raw_name)?;
    validate_spectra_query_name(&name).map_err(QueryLinkError::InvalidName)?;
    match kind {
        "schema" => Ok(QuerySource::Event(name)),
        "metric" => Ok(QuerySource::Metric(name)),
        _ => Err(QueryLinkError::NotSpectraLink),
    }
}

fn parse_filter(raw: &str) -> Result<LinkFilter, QueryLinkError> {
    let invalid = || QueryLinkError::InvalidValue {
        key: "f".into(),
        value: percent_decode(raw).unwrap_or_else(|_| raw.to_string()),
    };
    let mut parts = raw.splitn(3, ':');
    let (Some(field), Some(op), Some(value)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(invalid());
    };
    let field = percent_decode(field)?;
    if field.is_empty() {
        return Err(invalid());
    }
    let op = FilterOp::from_token(&percent_decode(op)?).ok_or_else(invalid)?;
    Ok(LinkFilter {
        field,
        op,
        value: percent_decode(value)?,
    })
}

fn parse_sort(raw: &str) -> Result<QuerySort, QueryLinkError> {
    let (field, dir) = raw.rsplit_once(':').unwrap_or((raw, "asc"));
    let field = percent_decode(field)?;
    let descending = match dir {
        "asc" => false,
        "desc" => true,
        _ => {
            return Err(QueryLinkError::InvalidValue {
                key: "sort".into(),
                value: percent_decode(raw)?,
            })
        }
    };
    Ok(QuerySort {
        field: non_empty("sort", field)?,
        descending,
    })
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

fn rfc3339(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn non_empty(key: &str, value: String) -> Result<String, QueryLinkError> {
    if value.trim().is_empty() {
        Err(QueryLinkError::InvalidValue {
            key: key.to_string(),
            value,
        })
    } else {
        Ok(value)
    }
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Like [`encode_ops_path_segment`], but keeps `:` readable (it's legal in a query).
fn encode_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for &b in raw.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b':' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

/// Decodes `%XX` escapes, and `+` as a space (form encoding).
fn percent_decode(raw: &str) -> Result<String, QueryLinkError> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = bytes
                    .get(i + 1..i + 3)
                    .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
                    .ok_or(QueryLinkError::InvalidEncoding)?;
                let hex = std::str::from_utf8(hex).map_err(|_| QueryLinkError::InvalidEncoding)?;
                out.push(u8::from_str_radix(hex, 16).map_err(|_| QueryLinkError::InvalidEncoding)?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| QueryLinkError::InvalidEncoding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectra_core::SchemaFieldDto;

    fn ts(secs: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(secs, 0).unwrap()
    }

    fn round_trip(link: &QueryLink) {
        let encoded = link.encode();
        let decoded = decode(&encoded).unwrap_or_else(|e| panic!("{encoded}: {e}"));
        assert_eq!(&decoded.link, link, "{encoded}");
        assert_eq!(decoded.ignored_keys, Vec::<String>::new());
    }

    fn schema(fields: &[&str]) -> SchemaDetailDto {
        SchemaDetailDto {
            table_or_metric: "checkout_event".into(),
            description: None,
            logging_kind: "event".into(),
            version: "1".into(),
            fields: fields
                .iter()
                .map(|name| SchemaFieldDto {
                    name: (*name).to_string(),
                    rust_type: "String".into(),
                    classification: "public".into(),
                })
                .collect(),
            can_query: true,
        }
    }

    #[test]
    fn every_event_view_round_trips_happy_path() {
        for view in QueryView::ALL {
            let mut link = QueryLink::event("checkout_event");
            link.view = Some(view);
            link.measure = Some(QueryMeasure::Count);
            link.group = Some("status".into());
            link.bucket = QueryBucket::from_secs(300);
            link.filters.push(LinkFilter {
                field: "status".into(),
                op: FilterOp::Eq,
                value: "failed".into(),
            });
            round_trip(&link);
        }
    }

    #[test]
    fn full_event_link_round_trips_happy_path() {
        let mut link = QueryLink::event("ops.events");
        link.window = QueryWindow::Range {
            from: ts(1_700_000_000),
            to: ts(1_700_003_600),
        };
        link.view = Some(QueryView::Table);
        link.measure = Some(QueryMeasure::Sum("amount cents".into()));
        link.group = Some("region".into());
        link.split = Some("status".into());
        link.bucket = QueryBucket::from_secs(150);
        link.filters = FilterOp::ALL
            .into_iter()
            .map(|op| LinkFilter {
                field: "note:x".into(),
                op,
                value: "a:b c&d=e+f%".into(),
            })
            .collect();
        link.cols = vec!["ts".into(), "a,b".into(), "message".into()];
        link.sort = Some(QuerySort {
            field: "ts".into(),
            descending: true,
        });
        link.limit = Some(25);
        round_trip(&link);
    }

    #[test]
    fn metric_links_round_trip_happy_path() {
        for preset in WindowPreset::ALL {
            let mut link = QueryLink::metric("api_latency_ms");
            link.window = QueryWindow::Last(preset);
            link.agg = Some("max".into());
            link.bucket = QueryBucket::from_secs(300);
            link.split = Some("route".into());
            round_trip(&link);
        }
        round_trip(&QueryLink::metric("platform_smoke_counter"));
    }

    #[test]
    fn encode_matches_documented_forms_happy_path() {
        let mut link = QueryLink::event("checkout_event");
        link.view = Some(QueryView::Log);
        link.filters.push(LinkFilter {
            field: "status".into(),
            op: FilterOp::Eq,
            value: "failed".into(),
        });
        assert_eq!(
            link.encode(),
            "/spectra/schema/checkout_event/explore?window=1h&view=log&f=status:eq:failed"
        );

        let mut metric = QueryLink::metric("api_latency");
        metric.window = QueryWindow::Last(WindowPreset::Hours6);
        metric.agg = Some("max".into());
        metric.bucket = QueryBucket::from_secs(300);
        metric.split = Some("route".into());
        assert_eq!(
            metric.encode(),
            "/spectra/metric/api_latency/explore?window=6h&agg=max&split=route&bucket=5m"
        );
    }

    #[test]
    fn full_url_on_any_host_decodes_happy_path() {
        let decoded = decode(
            "https://ops.example.com/spectra/schema/checkout_event/explore?window=24h&view=bar&measure=count&group=status#top",
        )
        .unwrap();
        assert_eq!(
            decoded.link.source,
            QuerySource::Event("checkout_event".into())
        );
        assert_eq!(
            decoded.link.window,
            QueryWindow::Last(WindowPreset::Hours24)
        );
        assert_eq!(decoded.link.view, Some(QueryView::Bar));
        assert_eq!(decoded.link.group.as_deref(), Some("status"));
    }

    #[test]
    fn bare_path_defaults_to_last_hour_happy_path() {
        let decoded = decode("/spectra/metric/platform_smoke_counter/explore").unwrap();
        assert_eq!(decoded.link, QueryLink::metric("platform_smoke_counter"));
    }

    #[test]
    fn unknown_keys_are_listed_sad_path() {
        let decoded =
            decode("/spectra/schema/t/explore?utm_source=mail&view=pie&utm_source=x&zoom=2")
                .unwrap();
        assert_eq!(decoded.ignored_keys, vec!["utm_source", "zoom"]);
        assert_eq!(decoded.link.view, Some(QueryView::Pie));
    }

    #[test]
    fn unknown_field_is_reported_sad_path() {
        let mut link = QueryLink::event("checkout_event");
        link.group = Some("customer_name".into());
        link.filters.push(LinkFilter {
            field: "status".into(),
            op: FilterOp::Eq,
            value: "failed".into(),
        });
        link.sort = Some(QuerySort {
            field: "ts".into(),
            descending: true,
        });
        let decoded = decode(&link.encode())
            .unwrap()
            .resolve(&schema(&["status", "region"]));
        assert_eq!(decoded.unknown_fields, vec!["customer_name"]);
        assert_eq!(decoded.link, link, "unknown fields stay in the query");
    }

    #[test]
    fn non_spectra_urls_are_errors_sad_path() {
        for input in [
            "https://example.com/",
            "/spectra/schema/t",
            "/spectra/dashboards/abc/explore",
            "/other/schema/t/explore",
            "/spectra/schema/t/explore/extra",
            "",
        ] {
            assert_eq!(
                decode(input).unwrap_err(),
                QueryLinkError::NotSpectraLink,
                "{input}"
            );
        }
        assert!(matches!(
            decode("/spectra/schema/%2E%2E/explore").unwrap_err(),
            QueryLinkError::InvalidName(_)
        ));
    }

    #[test]
    fn malformed_values_are_errors_sad_path() {
        let bad = [
            "window=2h",
            "view=line",
            "measure=avg",
            "measure=sum:",
            "bucket=0s",
            "bucket=5x",
            "f=status:like:x",
            "f=status",
            "sort=ts:sideways",
            "limit=0",
            "from=yesterday&to=2026-01-01T00:00:00Z",
            "group=",
        ];
        for query in bad {
            let err = decode(&format!("/spectra/schema/t/explore?{query}")).unwrap_err();
            assert!(
                matches!(err, QueryLinkError::InvalidValue { .. }),
                "{query}: {err:?}"
            );
        }
        let err = |q: &str| decode(&format!("/spectra/schema/t/explore?{q}")).unwrap_err();
        assert_eq!(
            err("view=log&view=bar"),
            QueryLinkError::DuplicateKey("view".into())
        );
        assert_eq!(
            err("from=2026-01-01T00:00:00Z"),
            QueryLinkError::IncompleteRange
        );
        assert_eq!(
            err("window=1h&from=2026-01-01T00:00:00Z&to=2026-01-02T00:00:00Z"),
            QueryLinkError::ConflictingWindow
        );
        assert_eq!(err("group=%zz"), QueryLinkError::InvalidEncoding);
    }

    #[test]
    fn bucket_tokens_happy_path() {
        assert_eq!(QueryBucket::from_token("15m"), QueryBucket::from_secs(900));
        assert_eq!(
            QueryBucket::from_token("1d"),
            QueryBucket::from_secs(86_400)
        );
        assert_eq!(QueryBucket::from_token("150s"), QueryBucket::from_secs(150));
        assert_eq!(
            QueryBucket::from_token("60s")
                .map(QueryBucket::token)
                .as_deref(),
            Some("1m")
        );
    }

    #[test]
    fn window_bounds_measure_back_from_now_happy_path() {
        let now = ts(1_700_000_000);
        assert_eq!(
            QueryWindow::Last(WindowPreset::Hours6).bounds(now),
            (ts(1_700_000_000 - 6 * 3_600), now)
        );
        let range = QueryWindow::Range {
            from: ts(1),
            to: ts(2),
        };
        assert_eq!(range.bounds(now), (ts(1), ts(2)));
    }
}
