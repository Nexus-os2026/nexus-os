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
use super::AuthorityError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Most cells one agent's partition holds at once.
pub(crate) const MAX_HALT_CELLS: usize = 4096;

/// A mission's halt: engaged once, never cleared.
#[derive(Clone, Debug)]
pub struct HaltCell(Arc<AtomicBool>);

impl HaltCell {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Engage it, for good: no lock, no I/O, no panic.
    pub fn engage(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether it is engaged (a read: it grants nothing).
    pub fn is_engaged(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// The flag a run's cancellation token reads.
    pub(crate) fn flag(&self) -> Arc<AtomicBool> {
        self.0.clone()
    }
}

type Key = (AgentId, String, u64);

/// The broker's table of halt cells (one per `Authority`).
#[derive(Default)]
pub(crate) struct HaltTable {
    cells: Mutex<HashMap<Key, HaltCell>>,
}

impl HaltTable {
    fn lock(&self) -> MutexGuard<'_, HashMap<Key, HaltCell>> {
        self.cells.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One agent's partition of the broker's halt table.
#[derive(Clone)]
pub struct HaltCells {
    table: Arc<HaltTable>,
    agent: AgentId,
}

impl HaltCells {
    pub(crate) fn of(table: Arc<HaltTable>, agent: AgentId) -> Self {
        Self { table, agent }
    }

    /// A table of its own, owned by no broker (a caller's tests): its cells
    /// only ever add refusals.
    pub fn detached(agent: AgentId) -> Self {
        Self::of(Arc::default(), agent)
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
        self.table.lock().remove(&key);
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
