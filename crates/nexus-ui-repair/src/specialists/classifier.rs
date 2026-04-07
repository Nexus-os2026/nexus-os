//! Outcome classifier. Phase 1.1 stub: always returns `Pass`. Real
//! deterministic-rules-plus-LLM logic lands in Phase 1.4.

use crate::specialists::vision_judge::VisionVerdict;

/// One of the v1.1 classification labels.
pub enum Classification {
    Pass,
    Dead,
    Error,
    Hang,
    Ambiguous,
}

/// The classifier specialist.
pub struct Classifier;

impl Classifier {
    /// Map a vision verdict to a classification. Phase 1.1 stub returns
    /// `Pass` regardless.
    pub fn classify(&self, _verdict: &VisionVerdict) -> Classification {
        Classification::Pass
    }
}
