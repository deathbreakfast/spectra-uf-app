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

mod codec;
mod resolve;
#[cfg(test)]
mod tests;
mod types;

pub use codec::{decode, QueryLinkError};
pub use resolve::resolve;
pub use types::{
    Decoded, FilterOp, LinkFilter, QueryBucket, QueryLink, QueryMeasure, QuerySort, QuerySource,
    QueryView, QueryWindow, WindowPreset,
};
