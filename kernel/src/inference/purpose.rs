//! M3-Cq Cq-0 (rulings CQ-R1, CQ-R3): the purposes a local model may be
//! qualified for, the frozen prompt floor, and the source-frozen
//! qualification records. Unused in Cq-0: no transport reads them yet.
//!
//! - A [`Purpose`] is closed: no default, no wildcard. Its [`Purpose::code`]
//!   is a stable byte string (identity v2 and the worker ledger use it).
//! - A [`Floor`] is positive and made only by [`freeze_floor`]
//!   (`min_prompt_eval_count − 32`, checked).
//! - A [`QualificationRecord`] binds a purpose to the **whole measured
//!   identity** (the identity v2 digest of the pinned model, runtime and
//!   configuration) and its floor. [`QUALIFICATIONS`] is empty: no model is
//!   qualified. A record is added only by a separate evidence commit, as
//!   `freeze_floor(<measured min>)` in a `const` with a compile-time `Some`
//!   assertion. There is no runtime setter, static, environment, file or
//!   configuration input, and these items are the same in every build.

use std::num::NonZeroU32;

/// A purpose a local model may be qualified for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    PageBuilder,
    N1Worker,
    Judge,
}

impl Purpose {
    /// The stable code of this purpose.
    pub const fn code(self) -> &'static str {
        match self {
            Purpose::PageBuilder => "page_builder",
            Purpose::N1Worker => "n1_worker",
            Purpose::Judge => "judge",
        }
    }
}

/// A frozen, positive prompt floor (tokens). Made only by [`freeze_floor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Floor(NonZeroU32);

impl Floor {
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

/// The floor frozen from a qualification's measured minimum prompt count:
/// `min_prompt_eval_count − 32`, `None` unless that is positive (then the
/// qualification is BLOCKED; there is no fallback).
pub const fn freeze_floor(min_prompt_eval_count: u32) -> Option<Floor> {
    match min_prompt_eval_count.checked_sub(32) {
        Some(floor) => match NonZeroU32::new(floor) {
            Some(floor) => Some(Floor(floor)),
            None => None,
        },
        None => None,
    }
}

/// One qualification: `purpose` for exactly the model identity whose v2
/// digest is `identity_sha256`, with its frozen floor and evidence id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QualificationRecord {
    pub purpose: Purpose,
    pub identity_sha256: [u8; 32],
    pub floor: Floor,
    pub evidence: &'static str,
}

/// The qualification records. Empty: no model is qualified for any purpose.
pub const QUALIFICATIONS: &[QualificationRecord] = &[];

const _: () = assert!(
    no_duplicate(QUALIFICATIONS),
    "one record per purpose and identity"
);

/// The frozen floor of `purpose` for the identity whose v2 digest is
/// `identity_sha256`; `None`: not qualified.
pub fn qualified_floor(purpose: Purpose, identity_sha256: &[u8; 32]) -> Option<Floor> {
    find(QUALIFICATIONS, purpose, identity_sha256)
}

/// The floor of the record in `table` naming both `purpose` and the whole
/// identity digest.
pub(super) fn find(
    table: &[QualificationRecord],
    purpose: Purpose,
    identity_sha256: &[u8; 32],
) -> Option<Floor> {
    table
        .iter()
        .find(|r| r.purpose == purpose && r.identity_sha256 == *identity_sha256)
        .map(|r| r.floor)
}

/// Whether no two records of `table` name the same purpose and identity.
pub(super) const fn no_duplicate(table: &[QualificationRecord]) -> bool {
    let mut i = 0;
    while i < table.len() {
        let mut j = i + 1;
        while j < table.len() {
            if table[i].purpose as u8 == table[j].purpose as u8
                && same_digest(&table[i].identity_sha256, &table[j].identity_sha256)
            {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

const fn same_digest(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut k = 0;
    while k < 32 {
        if a[k] != b[k] {
            return false;
        }
        k += 1;
    }
    true
}
