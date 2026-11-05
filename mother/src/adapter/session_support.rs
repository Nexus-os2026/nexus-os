//! Real mandatory session fixture. No raw backend authority is exported.
use super::{create_private, FileJournal, FilePrivate, SessionPort};
use crate::journal::JournalStore;
use nexus_governed_control::authority::approval::*;
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::host_session::{CommandKind, HostSession, SessionSeed};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct Owner {
    pub decline: AtomicBool,
}
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        !self.decline.load(Ordering::SeqCst)
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
}
pub(crate) struct Fixture {
    pub dir: PathBuf,
    pub workspace: PathBuf,
    pub session: HostSession,
    pub port: Arc<SessionPort>,
    pub owner: Arc<Owner>,
    pub clock: Arc<ManualClock>,
    next: AtomicU64,
}
impl Fixture {
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("m2dd-{label}-{}", std::process::id()));
        create_private(&dir).unwrap();
        let broker = dir.join("broker");
        let evidence = nexus_governed_control::evidence_store::PrimaryEvidence::open(
            &broker.join("p3-evidence/evidence.sqlite3"),
            vec![],
        )
        .unwrap();
        let clock = Arc::new(ManualClock::default());
        let owner = Arc::new(Owner::default());
        let session = HostSession::open_with_history(
            SessionSeed::new().unwrap(),
            &broker,
            &dir,
            evidence,
            clock.clone(),
            owner.clone(),
        )
        .unwrap();
        let workspace = dir.join("workspace");
        create_private(&workspace).unwrap();
        let info = session.bootstrap_workspace(&workspace, &[]).unwrap();
        session.finish_bootstrap().unwrap();
        let request = session.ingress().accept(CommandKind::Run, 1).unwrap();
        let port = Arc::new(SessionPort::new(
            session.open_file_door(&request).unwrap(),
            info.name,
        ));
        Self {
            dir,
            workspace,
            session,
            port,
            owner,
            clock,
            next: AtomicU64::new(2),
        }
    }
    pub fn journal(&self) -> FileJournal {
        FileJournal::open(&self.dir.join("mother-journal.jsonl")).unwrap()
    }
    pub fn lines(&self) -> Vec<String> {
        self.journal().load().unwrap()
    }
    pub fn private(&self) -> FilePrivate {
        FilePrivate::open_session(&self.dir.join("private"), self.port.context()).unwrap()
    }
    pub fn approve(
        &self,
    ) -> Result<
        nexus_governed_control::control::EffectOutput,
        nexus_governed_control::host_session::SessionFailure,
    > {
        let sequence = self.next.fetch_add(1, Ordering::SeqCst);
        let request = self
            .session
            .ingress()
            .accept(CommandKind::Approve, sequence)
            .unwrap();
        self.session.approve_file(&request)
    }
}
