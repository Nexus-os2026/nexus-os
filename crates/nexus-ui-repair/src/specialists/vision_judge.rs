//! Before/after screen comparison. Phase 1.1 stub: always returns
//! `Unchanged`. Real implementation (Codex CLI default, Anthropic API
//! escalation) lands in Phase 1.4.

/// Outcome of a single before/after vision comparison.
pub enum VisionVerdict {
    Changed,
    Unchanged,
    Ambiguous { similarity: f64 },
}

/// The vision judge specialist.
pub struct VisionJudge;

impl VisionJudge {
    /// Judge whether two screenshots represent a meaningful change.
    /// Phase 1.1 stub returns `Unchanged` for any input.
    pub fn judge(&self, _before: &[u8], _after: &[u8]) -> crate::Result<VisionVerdict> {
        Ok(VisionVerdict::Unchanged)
    }
}
