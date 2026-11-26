//! Checking a query link against a schema.

use spectra_core::SchemaDetailDto;

use super::types::{Decoded, QueryLink};

impl Decoded {
    /// Checks the link against `schema` and fills [`Decoded::unknown_fields`].
    ///
    /// This is [`resolve`] with its result stored on the decoded link; the check itself lives
    /// only there.
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
/// [`Decoded::resolve`] calls this and stores the result in `unknown_fields`.
#[must_use]
pub fn resolve(link: &QueryLink, schema: &SchemaDetailDto) -> Vec<String> {
    link.referenced_fields()
        .into_iter()
        .filter(|field| field != "ts" && !schema.fields.iter().any(|f| &f.name == field))
        .collect()
}
