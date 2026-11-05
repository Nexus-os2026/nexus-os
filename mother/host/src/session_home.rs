//! Hands owns only this closed session and report/store services. Legacy Home
//! construction and the walk terminal are separate call graphs.
use crate::{Confirmer, HomeError};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::{
    EvidenceProjection, EvidenceRecord, MemoryEvidence,
};
use nexus_governed_control::evidence_store::{EvidenceHealth, PrimaryEvidence};
use nexus_governed_control::host_session::{HostSession, SessionContext, SessionSeed};
use nexus_mother::adapter::{create_private, FileJournal, FilePrivate};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// Display data only; carries no backend identity, handle, or authority.
pub(crate) type DoorPrompt = nexus_governed_control::authority::approval::AgentDoorConfirmation;

pub(crate) struct SessionServices {
    dir: PathBuf,
    pub(crate) session: Arc<HostSession>,
    recent: Arc<MemoryEvidence>,
    evidence: Arc<PrimaryEvidence>,
}
impl SessionServices {
    pub(crate) fn open(
        seed: SessionSeed,
        dir: &Path,
        confirmer: Confirmer,
    ) -> Result<Self, HomeError> {
        Self::open_projected(seed, dir, confirmer, vec![])
    }
    pub(crate) fn open_projected(
        seed: SessionSeed,
        dir: &Path,
        confirmer: Confirmer,
        mut projections: Vec<Arc<dyn EvidenceProjection>>,
    ) -> Result<Self, HomeError> {
        let ingress = seed.ingress();
        let result = (|| {
            if !dir.is_absolute() {
                return Err(HomeError("absolute_path_required".into()));
            }
            create_private(dir).map_err(|_| HomeError("home_unavailable".into()))?;
            let recent = Arc::new(MemoryEvidence::new(256));
            projections.push(recent.clone());
            let evidence = PrimaryEvidence::open(
                &dir.join("broker/p3-evidence/evidence.sqlite3"),
                projections,
            )
            .map_err(|_| HomeError("evidence_unavailable".into()))?;
            let mut session = HostSession::open_with_history(
                seed,
                &dir.join("broker"),
                dir,
                evidence.clone(),
                Arc::new(SystemClock::default()),
                confirmer,
            )
            .map_err(|_| HomeError("session_unavailable".into()))?;
            session
                .bind_history_inspector(Arc::new(nexus_mother::adapter::SessionHistoryInspector))
                .map_err(|_| HomeError("mother_history".into()))?;
            Ok(Self {
                dir: dir.into(),
                session: Arc::new(session),
                recent,
                evidence,
            })
        })();
        if result.is_err() {
            ingress.fail();
        }
        result
    }
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }
    pub(crate) fn journal(&self) -> Result<FileJournal, nexus_mother::journal::JournalError> {
        FileJournal::open(&self.dir.join("mother-journal.jsonl"))
    }
    pub(crate) fn private(
        &self,
        context: SessionContext,
    ) -> Result<FilePrivate, nexus_mother::private::PrivateError> {
        FilePrivate::open_session(&self.dir.join("private"), context)
    }
    pub(crate) fn recent_evidence(&self) -> Vec<EvidenceRecord> {
        self.recent.records()
    }
    pub(crate) fn evidence_health(&self) -> EvidenceHealth {
        self.evidence.health()
    }
}
