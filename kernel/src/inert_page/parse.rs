//! S3: the strict parse of a candidate that passed S1 and S2.

use super::tree::Page;
use super::RefusalCode;

/// A refusal code and the byte offset where it applies.
pub(super) type Fail = (RefusalCode, usize);

pub(super) fn parse_document(text: &str) -> Result<Page, Fail> {
    let _ = text;
    Err((RefusalCode::NotCanonical, 0))
}
