//! Replay harness. Phase 1.1 stub: always returns `Ok(())`. Real
//! byte-identical session replay lands in Phase 1.4 alongside the
//! `vision_judge` specialist (so the `(inputs, output)` capture pattern
//! has something to replay).

use std::path::Path;

/// The replay harness.
pub struct ReplayHarness;

impl ReplayHarness {
    /// Replay a session from its audit log. Phase 1.1 stub.
    pub fn replay(&self, _audit_path: &Path) -> crate::Result<()> {
        Ok(())
    }
}
