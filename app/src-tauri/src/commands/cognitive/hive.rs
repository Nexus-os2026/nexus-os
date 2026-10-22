//! The HiveMind sessions under way: at most `MAX_SESSIONS` at a time, each
//! with its own identity and cancellation. A session's place is held by
//! the `HiveSession` its thread owns and given back when that ends, however
//! it ends (a panic included). Once the desktop is quitting no session
//! starts, and every one under way is told to stop.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// How many HiveMind sessions may run at once.
pub(crate) const MAX_SESSIONS: usize = 2;

/// What a sub-task of a cancelled session is refused (or ended) with.
pub(crate) const SESSION_CANCELLED: &str = "sub-task refused: the HiveMind session was cancelled";

#[derive(Default)]
pub(crate) struct HiveSessions {
    table: Mutex<Table>,
}

#[derive(Default)]
struct Table {
    /// Each session under way, by its identity: its cancellation.
    live: HashMap<String, Arc<AtomicBool>>,
    /// The desktop is quitting: no session starts any more.
    quitting: bool,
}

/// One admitted session: its identity and its cancellation. Dropping it
/// gives its place back.
pub(crate) struct HiveSession {
    sessions: Arc<HiveSessions>,
    id: String,
    cancelled: Arc<AtomicBool>,
}

impl HiveSessions {
    fn table(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Admit a session, its place taken at once, or refuse it: the desktop
    /// is quitting, or `MAX_SESSIONS` run already.
    pub(crate) fn admit(self: &Arc<Self>) -> Result<HiveSession, String> {
        let mut table = self.table();
        if table.quitting {
            return Err("hivemind: the desktop is quitting".to_string());
        }
        if table.live.len() >= MAX_SESSIONS {
            return Err(format!(
                "hivemind: at most {MAX_SESSIONS} sessions run at once"
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let cancelled = Arc::new(AtomicBool::new(false));
        table.live.insert(id.clone(), cancelled.clone());
        Ok(HiveSession {
            sessions: self.clone(),
            id,
            cancelled,
        })
    }

    /// The owner cancels one session: whether it was under way.
    pub(crate) fn cancel(&self, id: &str) -> bool {
        self.table()
            .live
            .get(id)
            .map(|cancelled| cancelled.store(true, Ordering::SeqCst))
            .is_some()
    }

    /// Every session under way is told to stop (the emergency stop): how
    /// many.
    pub(crate) fn cancel_all(&self) -> usize {
        let table = self.table();
        for cancelled in table.live.values() {
            cancelled.store(true, Ordering::SeqCst);
        }
        table.live.len()
    }

    /// The desktop is quitting: no session starts any more, and every one
    /// under way is told to stop. How many.
    pub(crate) fn close(&self) -> usize {
        let mut table = self.table();
        table.quitting = true;
        for cancelled in table.live.values() {
            cancelled.store(true, Ordering::SeqCst);
        }
        table.live.len()
    }

    /// How many sessions are under way (tests).
    #[cfg(test)]
    pub(crate) fn live(&self) -> usize {
        self.table().live.len()
    }
}

impl HiveSession {
    /// The session's identity (the owner cancels it by this).
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Whether the session was told to stop: it assigns no further
    /// sub-task, and the one it waits on ends.
    pub(crate) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Drop for HiveSession {
    fn drop(&mut self) {
        self.sessions.table().live.remove(&self.id);
    }
}
