//! M2d-C round 5 (the Architect's amendment H1-H4 with its binding changes
//! C1-C6): a per-mission halt cell. A [`HaltCell`] only ever adds a
//! refusal: an unengaged cell grants nothing, and every other check of the
//! broker still applies. It is deny-only caller material, never authority.
//!
//! The broker owns one [`HaltTable`] (C1), partitioned by agent: a door
//! hands out its agent's [`HaltCells`]. A cell is found by a caller-chosen
//! key, `(journal identity, mission)`, get-or-create, and is the same cell
//! for as long as the broker lives, so a rebuilt caller finds the original.
//! The table is a dedicated mutex (C3), taken even if poisoned (it maps keys
//! to `Arc`s only), held only to insert, remove or clone out, never across
//! I/O, a callback or another lock; its guard is released before any cell is
//! engaged. Its completeness rests on the caller registering a mission's
//! cell before exposing the mission or opening its run.

use super::ids::AgentId;
use super::run::RunRegistry;
use super::AuthorityError;
use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Most cells one agent's partition holds at once.
pub(crate) const MAX_HALT_CELLS: usize = 4096;

/// M2d-C round 6 (R6-3): why a cell was engaged, its first cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaltCause {
    /// The Owner's Stop of the mission (or Stop everything).
    Stop,
    /// The Owner's emergency stop (the broker's ingress).
    Emergency,
    /// The mission's time budget.
    TimeBudget,
    /// A failure fence, or a mismatch that stops surviving work.
    Fence,
}

impl HaltCause {
    fn code(self) -> u8 {
        match self {
            HaltCause::Stop => 1,
            HaltCause::Emergency => 2,
            HaltCause::TimeBudget => 3,
            HaltCause::Fence => 4,
        }
    }

    fn of(code: u8) -> Option<Self> {
        match code {
            1 => Some(HaltCause::Stop),
            2 => Some(HaltCause::Emergency),
            3 => Some(HaltCause::TimeBudget),
            4 => Some(HaltCause::Fence),
            _ => None,
        }
    }
}

/// A mission's halt: engaged once, with its first cause, never cleared or
/// replaced.
#[derive(Clone, Debug)]
pub struct HaltCell(Arc<AtomicU8>);

impl HaltCell {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(0)))
    }

    /// Engage it as the Owner's Stop (see [`Self::engage_with`]).
    pub fn engage(&self) {
        self.engage_with(HaltCause::Stop);
    }

    /// Engage it, for good, keeping the first cause: no lock, no I/O, no
    /// panic.
    pub fn engage_with(&self, cause: HaltCause) {
        let _ = self
            .0
            .compare_exchange(0, cause.code(), Ordering::SeqCst, Ordering::SeqCst);
    }

    /// Whether it is engaged (a read: it grants nothing).
    pub fn is_engaged(&self) -> bool {
        self.0.load(Ordering::SeqCst) != 0
    }

    /// Its first cause, once engaged (a read).
    pub fn cause(&self) -> Option<HaltCause> {
        HaltCause::of(self.0.load(Ordering::SeqCst))
    }

    pub(crate) fn same(&self, other: &HaltCell) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

type Key = (AgentId, String, u64);

/// The broker's table of halt cells (one per `Authority`).
/// X2: what keeps a journal's identity from being reused while its cells
/// live (an opaque handle of the caller's, such as its open file).
type Anchor = Box<dyn Any + Send + Sync>;

#[derive(Default)]
pub(crate) struct HaltTable {
    cells: Mutex<HashMap<Key, HaltCell>>,
    /// X2: one anchor per (agent, journal), dropped with its last cell. A
    /// separate mutex, never taken while `cells` is held.
    anchors: Mutex<HashMap<(AgentId, String), Anchor>>,
}

impl HaltTable {
    fn lock(&self) -> MutexGuard<'_, HashMap<Key, HaltCell>> {
        self.cells.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// R6-3 / X1: the emergency ingress: every cell, cloned out under one
    /// hold, engaged once it is released, with `cause` (a cell engaged
    /// before keeps its first cause).
    pub(crate) fn engage_all(&self, cause: HaltCause) {
        let cells: Vec<HaltCell> = self.lock().values().cloned().collect();
        for cell in cells {
            cell.engage_with(cause);
        }
    }
}

/// One agent's partition of the broker's halt table.
#[derive(Clone)]
pub struct HaltCells {
    table: Arc<HaltTable>,
    agent: AgentId,
    /// X2: the runs registry the partition's emptiness reads.
    runs: Arc<RunRegistry>,
}

impl HaltCells {
    pub(crate) fn of(table: Arc<HaltTable>, agent: AgentId, runs: Arc<RunRegistry>) -> Self {
        Self { table, agent, runs }
    }

    /// A table of its own, owned by no broker (a caller's tests): its cells
    /// only ever add refusals.
    pub fn detached(agent: AgentId) -> Self {
        Self::of(Arc::default(), agent, Arc::default())
    }

    /// X2: keep `anchor` while any cell of `journal` lives (the first one
    /// given stays; it is dropped with the journal's last cell).
    pub fn anchor(&self, journal: &str, anchor: Anchor) {
        let held = self
            .table
            .lock()
            .keys()
            .any(|(a, j, _)| *a == self.agent && j == journal);
        if held {
            self.table
                .anchors
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry((self.agent.clone(), journal.to_string()))
                .or_insert(anchor);
        }
    }

    /// X2: whether the agent's whole partition is empty: no cell and no live
    /// agent run of the agent, decided in one hold of the runs lock (the
    /// table read inside it). An unreadable registry is an error.
    pub fn partition_empty(&self) -> Result<bool, AuthorityError> {
        self.runs.while_held(|live_run_of| {
            let runs = live_run_of(&self.agent);
            let cells = self.table.lock().keys().any(|(a, _, _)| *a == self.agent);
            !runs && !cells
        })
    }

    fn key(&self, journal: &str, mission: u64) -> Key {
        (self.agent.clone(), journal.to_string(), mission)
    }

    /// The cell of `(journal, mission)`, created if it has none: the same
    /// cell for as long as the table lives. `Capacity` once the agent holds
    /// [`MAX_HALT_CELLS`].
    pub fn cell(&self, journal: &str, mission: u64) -> Result<HaltCell, AuthorityError> {
        let key = self.key(journal, mission);
        let mut cells = self.table.lock();
        if let Some(cell) = cells.get(&key) {
            return Ok(cell.clone());
        }
        let held = cells.keys().filter(|(a, _, _)| *a == self.agent).count();
        if held >= MAX_HALT_CELLS {
            return Err(AuthorityError::Capacity);
        }
        let cell = HaltCell::new();
        cells.insert(key, cell.clone());
        Ok(cell)
    }

    /// The cell of `(journal, mission)`, if it has one.
    pub fn get(&self, journal: &str, mission: u64) -> Option<HaltCell> {
        self.table.lock().get(&self.key(journal, mission)).cloned()
    }

    /// One snapshot of every cell of `journal` (C2's cohort), cloned out
    /// under one hold; the caller engages them after it is released.
    pub fn cohort(&self, journal: &str) -> Vec<(u64, HaltCell)> {
        let cells = self.table.lock();
        let mut cohort: Vec<(u64, HaltCell)> = cells
            .iter()
            .filter(|((a, j, _), _)| *a == self.agent && j == journal)
            .map(|((_, _, m), cell)| (*m, cell.clone()))
            .collect();
        drop(cells);
        cohort.sort_by_key(|(m, _)| *m);
        cohort
    }

    /// Retire `(journal, mission)`'s cell once its mission is terminal: it
    /// is engaged first, then its entry dropped.
    pub fn retire(&self, journal: &str, mission: u64) {
        let key = self.key(journal, mission);
        if let Some(cell) = self.get(journal, mission) {
            cell.engage();
        }
        let left = {
            let mut cells = self.table.lock();
            cells.remove(&key);
            cells
                .keys()
                .any(|(a, j, _)| *a == self.agent && j == journal)
        };
        if !left {
            let anchor = self
                .table
                .anchors
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&(self.agent.clone(), journal.to_string()));
            drop(anchor);
        }
    }

    /// Tests: poison the table's lock.
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        std::thread::scope(|scope| {
            let _ = scope
                .spawn(|| {
                    let _held = self.table.cells.lock();
                    panic!("poisoning the halt table (a test)");
                })
                .join();
        });
    }
}
