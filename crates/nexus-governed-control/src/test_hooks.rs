//! Named test hooks (M2c-B, tests only): a broker call stops at a named
//! place while a test holds it there, so a race is a forced order, never a
//! stress loop or a sleep. A hook armed once runs on the next call that
//! reaches its place; a hook armed for every call runs on each. A [`Gate`]
//! is a test-held barrier (the evidence store's `Fault::Stall` waits at one
//! inside its mutex). Every wait is bounded.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// The places a hook may stop at.
pub(crate) mod at {
    pub(crate) const COMPLETE_BEFORE_CHECK: &str = "complete.before_check";
    pub(crate) const COMPLETE_CHECKED: &str = "complete.checked";
    pub(crate) const COMPLETE_FINALIZED: &str = "complete.finalized";
    pub(crate) const OPEN_SETTLED: &str = "open.settled";
    pub(crate) const OPEN_INSERTED: &str = "open.inserted";
    pub(crate) const OPEN_CHECKED: &str = "open.checked";
    pub(crate) const PREPARE_BEFORE_INSERT: &str = "prepare.before_insert";
    pub(crate) const AUTHORIZE_RECORDED: &str = "authorize.recorded";
    pub(crate) const PLACE_BEFORE_FILL: &str = "place.before_fill";
    pub(crate) const LEASE_RECORDED: &str = "lease.recorded";
    pub(crate) const RELEASE_RECORDED: &str = "release.recorded";
    pub(crate) const RELEASE_VAULT_READ: &str = "release.vault_read";
    pub(crate) const HALT_AFTER_RUNS: &str = "halt.after_runs";
    pub(crate) const HALT_ENTRIES_TAKEN: &str = "halt.entries_taken";
    pub(crate) const HALT_AFTER_ENTRIES: &str = "halt.after_entries";
    pub(crate) const HALT_AFTER_LEASES: &str = "halt.after_leases";
    pub(crate) const REVOKE_WHERE_FIRST: &str = "revoke_where.first";
    pub(crate) const END_UNCONSUMED_RECORDED: &str = "end_unconsumed.recorded";
    pub(crate) const DOOR_MEMBER_CHECKED: &str = "door.member_checked";
    pub(crate) const HELD_RECORDED: &str = "door.held_recorded";
    pub(crate) const MISSION_RECORDED: &str = "door.mission_recorded";
    pub(crate) const ISSUE_ANSWERED: &str = "issue.answered";
    pub(crate) const ISSUE_RECORDED: &str = "issue.recorded";
    pub(crate) const CONSENT_BEFORE_FILL: &str = "door.consent_before_fill";
    pub(crate) const ADMIT_BEFORE_BIND: &str = "door.admit_before_bind";
    /// M2d-B2: an observed Files apply is at its final check (the
    /// candidate written and synced; the guard is asked next).
    pub(crate) const FILE_BEFORE_FINAL: &str = "files.before_final";
    /// M2d-B2: the final check passed; the one publication is next.
    pub(crate) const FILE_AFTER_FINAL: &str = "files.after_final";
    /// M2d-B2: an undo is confirmed and its write grant issued; the
    /// kernel's restore (its U1 check) is next.
    pub(crate) const FILE_UNDO_APPROVED: &str = "files.undo_approved";
    /// Codex review 1 of PR #64: a Files cleanup of a handed-over run is
    /// next (H4's held cleanup).
    pub(crate) const FILE_CLEANUP: &str = "files.cleanup";
    /// H1 decision (row 3): the observer refuses the publication's handoff
    /// (its final check found a cause; nothing is published).
    pub(crate) const FILE_HANDOFF_REFUSED: &str = "files.handoff_refused";
    /// H1 decision (rows 4-5): an actual staging removal is about to be
    /// attempted.
    pub(crate) const FILE_DISCARD: &str = "files.discard";
}

type Once = Box<dyn FnOnce() + Send>;
type Every = Arc<dyn Fn() + Send + Sync>;

/// The hooks of one broker.
#[derive(Default)]
pub(crate) struct Hooks {
    once: Mutex<HashMap<&'static str, Once>>,
    every: Mutex<HashMap<&'static str, Every>>,
}

impl Hooks {
    /// Run `hook` once, on the next call that reaches `at`.
    pub(crate) fn arm(&self, at: &'static str, hook: impl FnOnce() + Send + 'static) {
        self.once.lock().unwrap().insert(at, Box::new(hook));
    }

    /// Run `hook` on every call that reaches `at`.
    pub(crate) fn every(&self, at: &'static str, hook: impl Fn() + Send + Sync + 'static) {
        self.every.lock().unwrap().insert(at, Arc::new(hook));
    }

    /// A call reached `at`. The hook runs with no lock of this registry held.
    pub(crate) fn fire(&self, at: &'static str) {
        let once = self
            .once
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(at);
        if let Some(hook) = once {
            hook();
        }
        let every = self
            .every
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(at)
            .cloned();
        if let Some(hook) = every {
            hook();
        }
    }
}

/// The longest a gate holds (a safety bound, longer than a bounded join so
/// that a wait on a held call shows; tests open it, never this).
const HOLD: Duration = Duration::from_secs(30);
/// The longest a test waits for a holder to arrive.
const REACH: Duration = Duration::from_secs(10);

#[derive(Default)]
struct GateState {
    entered: bool,
    open: bool,
}

/// A barrier a test holds a call at, and opens.
#[derive(Clone, Default)]
pub(crate) struct Gate(Arc<(Mutex<GateState>, Condvar)>);

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Gate")
    }
}

impl Gate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The held call: say so, and wait until the gate opens (bounded).
    pub(crate) fn hold(&self) {
        let (state, changed) = &*self.0;
        let mut state = state.lock().unwrap();
        state.entered = true;
        changed.notify_all();
        let _ = changed
            .wait_timeout_while(state, HOLD, |s| !s.open)
            .unwrap();
    }

    /// The test: wait until a call is held here.
    pub(crate) fn reached(&self) {
        let (state, changed) = &*self.0;
        let state = state.lock().unwrap();
        let (state, _) = changed
            .wait_timeout_while(state, REACH, |s| !s.entered)
            .unwrap();
        assert!(state.entered, "no call reached the gate");
    }

    /// Release every held call (and every later one).
    pub(crate) fn open(&self) {
        let (state, changed) = &*self.0;
        state.lock().unwrap().open = true;
        changed.notify_all();
    }
}
