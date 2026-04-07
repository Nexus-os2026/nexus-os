//! The five v1.1 invariants. See v1.1 §3 (and §3.1 for I-2 layers).
//!
//! Phase 1.1 ships the enum, the trait, and a registry stub. Real
//! per-invariant checks land in Phase 1.2 alongside the first specialist
//! integration.

/// The five invariants from v1.1 §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invariant {
    /// I-1 — Kernel allowlist (path allowlist; the driver has no
    /// `fs::write` capability anyway, so this is belt + suspenders).
    I1KernelAllowlist,
    /// I-2 — Read-only-by-construction. The scout has no capability to
    /// modify state outside its own report and audit directories. This
    /// holds across three layers: filesystem ACL, per-app input
    /// governance, and OS-level isolation. See §3.1.
    I2ReadOnlyFilesystem,
    /// I-3 — Every fix is HITL by definition. The scout reports; the
    /// human repairs. There is no autonomous repair path, so no HITL
    /// gating logic is needed.
    I3HitlByDefinition,
    /// I-4 — Immutable provider routing. The autonomous routing table
    /// contains only `codex_cli`, `ollama`, and a small Anthropic API
    /// allowance for vision ambiguity. `claude_cli` and
    /// `claude_ai_credits` are explicitly forbidden.
    I4ImmutableProviderRouting,
    /// I-5 — Replayable sessions. Append-only signed log; replay harness
    /// in CI. Every specialist call records `(inputs, output)` so replay
    /// is byte-identical despite non-deterministic LLM calls.
    I5ReplayableSessions,
}

/// Trait for any object capable of self-checking against an invariant.
pub trait InvariantCheck {
    /// Returns `Ok(())` if the invariant holds, otherwise an
    /// [`Error::InvariantViolation`](crate::Error::InvariantViolation).
    fn check(&self) -> crate::Result<()>;
}

/// Registry of all invariant checks the driver runs at every state
/// transition. Phase 1.1 carries no checks; Phase 1.2 fills it in.
#[derive(Debug, Default)]
pub struct InvariantRegistry {
    // Empty for Phase 1.1. Phase 1.2 adds boxed checks.
}

impl InvariantRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run every registered invariant check. Phase 1.1 returns `Ok(())`
    /// because no checks are wired yet.
    pub fn check_all(&self) -> crate::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_passes() {
        let r = InvariantRegistry::new();
        assert!(r.check_all().is_ok());
    }
}
