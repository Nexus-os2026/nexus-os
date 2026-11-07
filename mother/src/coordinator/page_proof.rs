//! M3-Db: the accepted page Builder's retained deliverable and the broker's
//! receipt for its one publication. Records carry references, digests and
//! sizes only. Oracle stub: nothing is retained or recorded yet.
use super::*;

/// What the journal says of a page mission's deliverable and receipt.
#[derive(Clone, Debug, Default)]
pub(super) struct Proof {
    pub(super) retained: Option<Retained>,
    pub(super) receipt: Option<u64>,
}

/// A retained deliverable, by reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Retained {
    pub(super) invocation: InvocationId,
    pub(super) effect: u64,
    pub(super) content: crate::private::ContentId,
    pub(super) sha256: String,
    pub(super) bytes: u64,
}

#[path = "../oracles/m3d_publication.rs"]
#[cfg(target_os = "linux")]
#[cfg(test)]
mod oracles;
