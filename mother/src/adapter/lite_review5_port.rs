//! Faults are test-only; File eligibility still goes through the native checks.
use super::*;
impl GovernedPort {
    pub(crate) fn review5_cell(&self) -> HaltCell {
        self.file_session.get().unwrap().1.clone()
    }

    pub(crate) fn review5_forget_wait(&self, id: MissionId) {
        self.runs.lock().unwrap().waiting.remove(&id);
    }
    pub(crate) fn review5_poison(&self) {
        let _held = self.runs.lock().unwrap();
        panic!("port poison");
    }
    pub(crate) fn review5_clear_poison(&self) {
        self.runs.clear_poison();
    }
    pub(crate) fn review5_reopen(&self) {
        assert!(self.door.file_port().is_err());
    }
    pub(crate) fn review5_cancel(
        &self,
        run: RunId,
    ) -> nexus_governed_control::authority::halt::RecordBatch {
        self.door.cancel_run(run).unwrap()
    }
}
