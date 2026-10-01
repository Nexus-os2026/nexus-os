//! Deterministic faults at the real ownership points of one verifier
//! execution: the live panic controls (P2-R1).
//!
//! Production never injects a fault: [`crate::execution::run`] passes none.
//! Only builds with the `live-sandbox-harness` feature (this crate's live
//! sandbox suite, through its own dev-dependency) can name a fault at all.
// Without the harness no fault is ever constructed.
#![cfg_attr(not(any(test, feature = "live-sandbox-harness")), allow(dead_code))]

use std::time::Duration;

/// Where a fault is injected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    /// The helper exists and its output is being drained; no scope yet.
    AfterSpawn,
    /// Inside scope creation: the new unit holds the helper but is not yet
    /// proven.
    ScopeProof,
    /// The proven scope holds the helper; no launch has been sent.
    AfterScope,
    /// The helper accepted the launch: the verifier is running.
    AfterLaunch,
    /// The verifier has been running for [`RUNNING_FAULT_AFTER`].
    Running,
    /// The verifier's output is being drained.
    Draining,
    /// Inside an output thread, after it has read the verifier's output.
    DrainThread,
    /// The wait has ended; ordinary finalization is about to begin.
    BeforeFinalize,
    /// Finalization has read the scope's counters and is about to end
    /// everything in the scope.
    Finalizing,
}

/// A fault to inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Panic at the point.
    Panic(FaultPoint),
    /// At [`FaultPoint::Finalizing`] only: the step fails without a panic,
    /// as when `cgroup.kill` cannot be written.
    Fail(FaultPoint),
}

/// How long the verifier runs before a [`FaultPoint::Running`] fault.
pub const RUNNING_FAULT_AFTER: Duration = Duration::from_secs(2);

/// Apply `fault` at `point`: panic there, or answer whether the step at
/// `point` fails.
pub(crate) fn at(fault: Option<Fault>, point: FaultPoint) -> bool {
    match fault {
        Some(Fault::Panic(at)) if at == point => panic!("injected verifier fault at {point:?}"),
        Some(Fault::Fail(at)) => at == point,
        _ => false,
    }
}
