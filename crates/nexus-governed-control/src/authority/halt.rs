//! The door halt's records (Mother Nexus M2c, PR-B): a [`RecordBatch`]
//! holds the end records of the transitions one call made, to be written
//! by that call once it holds no lock. Every record is accounted for
//! exactly once in the broker's halt counters: written (recorded, owed or
//! uncertain, as the store says), or, if the batch is dropped first, lost.
//! Dropping a batch does no I/O, takes no lock and cannot panic.

use super::evidence::{EndStatus, EvidenceRecord, EvidenceSink};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Why a door was halted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaltReason {
    /// The agent's own journal failed: nothing more of it may act.
    JournalFailed,
}

impl HaltReason {
    pub fn as_str(self) -> &'static str {
        match self {
            HaltReason::JournalFailed => "journal_failed",
        }
    }
}

/// The halt counters, as the status shows them. Every record pushed into a
/// batch is `pending` until it is recorded, owed, uncertain or lost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct HaltRecords {
    pub pending: u64,
    pub recorded: u64,
    pub owed: u64,
    pub uncertain: u64,
    pub lost: u64,
}

/// The broker's halt counters (atomics only).
#[derive(Default)]
pub(crate) struct HaltCounters {
    pending: AtomicU64,
    recorded: AtomicU64,
    owed: AtomicU64,
    uncertain: AtomicU64,
    lost: AtomicU64,
}

impl HaltCounters {
    pub(crate) fn snapshot(&self) -> HaltRecords {
        HaltRecords {
            pending: self.pending.load(Ordering::SeqCst),
            recorded: self.recorded.load(Ordering::SeqCst),
            owed: self.owed.load(Ordering::SeqCst),
            uncertain: self.uncertain.load(Ordering::SeqCst),
            lost: self.lost.load(Ordering::SeqCst),
        }
    }

    /// One pending record leaves for `to`.
    fn leave(&self, to: &AtomicU64) {
        self.pending.fetch_sub(1, Ordering::SeqCst);
        to.fetch_add(1, Ordering::SeqCst);
    }
}

/// What writing a batch came to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchOutcome {
    pub recorded: usize,
    pub owed: usize,
    pub uncertain: usize,
}

/// The end records of transitions one call made, still to write. It has no
/// public constructor and is not `Clone`; write it with [`RecordBatch::write`]
/// once no lock is held. Dropped unwritten, its records are counted lost.
#[must_use = "a batch's records are lost unless it is written"]
pub struct RecordBatch {
    records: VecDeque<EvidenceRecord>,
    in_flight: bool,
    sink: Arc<dyn EvidenceSink>,
    counters: Arc<HaltCounters>,
}

impl std::fmt::Debug for RecordBatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordBatch")
            .field("records", &self.records.len())
            .finish_non_exhaustive()
    }
}

impl RecordBatch {
    /// An empty batch, counted from now on.
    pub(crate) fn new(sink: Arc<dyn EvidenceSink>, counters: Arc<HaltCounters>) -> Self {
        Self {
            records: VecDeque::new(),
            in_flight: false,
            sink,
            counters,
        }
    }

    /// One transition's end record.
    pub(crate) fn push(&mut self, record: EvidenceRecord) {
        self.counters.pending.fetch_add(1, Ordering::SeqCst);
        self.records.push_back(record);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The records still to write (tests).
    #[cfg(test)]
    pub(crate) fn records(&self) -> &VecDeque<EvidenceRecord> {
        &self.records
    }

    /// Write every record, in order, as end records (each transition has
    /// already taken effect). A record whose write panics (a projection may
    /// panic after its commit) is counted uncertain, the rest lost.
    pub fn write(mut self) -> BatchOutcome {
        let mut outcome = BatchOutcome::default();
        while let Some(record) = self.records.front() {
            self.in_flight = true;
            let status = self.sink.record_end(record);
            let to = match status {
                EndStatus::Recorded { .. } => {
                    outcome.recorded += 1;
                    &self.counters.recorded
                }
                EndStatus::Owed => {
                    outcome.owed += 1;
                    &self.counters.owed
                }
                EndStatus::Uncertain => {
                    outcome.uncertain += 1;
                    &self.counters.uncertain
                }
            };
            self.counters.leave(to);
            self.records.pop_front();
            self.in_flight = false;
        }
        outcome
    }
}

impl Drop for RecordBatch {
    /// No I/O, no lock, no panic: atomics only.
    fn drop(&mut self) {
        let mut left = self.records.len() as u64;
        if self.in_flight && left > 0 {
            self.counters.leave(&self.counters.uncertain);
            left -= 1;
        }
        self.counters.pending.fetch_sub(left, Ordering::SeqCst);
        self.counters.lost.fetch_add(left, Ordering::SeqCst);
    }
}
