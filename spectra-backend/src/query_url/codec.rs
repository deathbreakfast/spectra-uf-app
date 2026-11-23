//! Encoding a query link to a URL and decoding one back.

use std::fmt;

use chrono::{DateTime, SecondsFormat, Utc};

use super::types::{
    Decoded, FilterOp, LinkFilter, QueryBucket, QueryLink, QueryMeasure, QuerySort, QuerySource,
    QueryView, QueryWindow, WindowPreset,
};
use crate::{encode_ops_path_segment, validate_spectra_query_name, SpectraQueryNameError};

impl QueryLink {
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
