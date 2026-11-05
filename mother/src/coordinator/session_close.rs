//! The Mother end is prepared before the broker seals. Its sole post-seal
//! operation is the reserved journal append; no port or recovery is retained.
use super::lite::FileSession;
use super::*;
use nexus_governed_control::host_session::{close::CloseSeal, SessionIdentity};
use std::sync::atomic::Ordering;

pub struct PreparedSessionEnd {
    store: Box<dyn JournalStore>,
    line: String,
    session: SessionIdentity,
}
impl PreparedSessionEnd {
    pub fn record(mut self, seal: &CloseSeal) -> Result<(), CoordinatorError> {
        seal.begin_mother_end(self.session)
            .map_err(|_| CoordinatorError::Stopped)?;
        self.store
            .append(&self.line)
            .map_err(CoordinatorError::Journal)
    }
    /// An idle Home has no operational coordinator, door or File attachment.
    /// This opens its administrative journal session and reserves only its end.
    pub fn idle(
        mut store: crate::adapter::FileJournal,
        session: SessionIdentity,
    ) -> Result<Self, CoordinatorError> {
        let lines = store.load().map_err(CoordinatorError::Journal)?;
        let entries = crate::journal::verify(&lines).map_err(CoordinatorError::Journal)?;
        if entries
            .last()
            .is_some_and(|entry| !matches!(entry.event, Event::SessionClosed))
        {
            return Err(CoordinatorError::FileUnfinished);
        }
        let head = entries.last().map_or(GENESIS, |entry| entry.hash.as_str());
        let opening = Entry::chained(entries.len() as u64, head, SESSION, Event::SessionOpened);
        store
            .append(&opening.to_line())
            .map_err(CoordinatorError::Journal)?;
        Ok(Self {
            line: Entry::chained(
                opening.seq + 1,
                &opening.hash,
                SESSION,
                Event::SessionClosed,
            )
            .to_line(),
            store: Box::new(store),
            session,
        })
    }
}
impl FileSession<crate::adapter::SessionPort> {
    /// Exclusive transfer removes all operational controller routes. Terminal
    /// settlement and the exact acknowledgements happen before this returns.
    pub fn prepare_session_end(self) -> Result<PreparedSessionEnd, CoordinatorError> {
        let id = self.life().mission.load(Ordering::SeqCst);
        if id != 0 {
            self.call(|c| {
                let terminal = c.peek(|inner| {
                    inner
                        .missions
                        .get(&MissionId(id))
                        .is_some_and(|m| m.state.is_terminal())
                });
                if !terminal {
                    c.file_stop(MissionId(id), SettleCause::Stop(StopReason::Owner))?;
                }
                c.file_ack_all(MissionId(id))
            })?;
        }
        let mut inner = self
            .coordinator
            .inner
            .into_inner()
            .map_err(|_| CoordinatorError::Poisoned)?;
        inner.usable()?;
        if !inner.owed.is_empty()
            || inner.missions.values().any(|m| {
                !m.state.is_terminal()
                    || m.terminal_uncertain
                    || m.busy
                    || !m.acks.is_empty()
                    || m.open_action.is_some()
                    || m.unsettled.is_some()
                    || m.unresolved_file.is_some()
            })
        {
            return Err(CoordinatorError::FileUnfinished);
        }
        inner.open_session()?;
        let lines = inner.store.load().map_err(CoordinatorError::Journal)?;
        let entries = crate::journal::verify(&lines).map_err(CoordinatorError::Journal)?;
        if entries.len() as u64 != inner.next_seq
            || entries.last().map_or(GENESIS, |entry| entry.hash.as_str()) != inner.head
        {
            return Err(CoordinatorError::Journal(JournalError::Store(
                StoreFailure::ChangedUnderCoordinator,
            )));
        }
        let context = inner
            .session_context
            .as_ref()
            .ok_or(CoordinatorError::Stopped)?;
        Ok(PreparedSessionEnd {
            line: Entry::chained(inner.next_seq, &inner.head, SESSION, Event::SessionClosed)
                .to_line(),
            session: context.session(),
            store: inner.store,
        })
    }
}
