//! SB-3 C1: the session model-bridge fixture. A direct session (the TH-R1 B
//! fresh, owned home) registered on a fake loopback peer during Bootstrap,
//! a real coordinator over its SessionPort, and the C1b model lane.
use crate::adapter::c1b_coordinator_oracles::{identity_hex, ModelLane};
use crate::adapter::c1b_fake_ollama::{spec, FakeOllama, PATIENCE};
use crate::adapter::session_support::Fixture;
use crate::adapter::SessionPort;
use crate::contract::{MissionContract, ModelChoice};
use crate::coordinator::{Coordinator, CoordinatorError, FileSession, Lanes};
use crate::journal::{verify, Event, MissionId};
use crate::lane::{Lane, LaneContext, LaneOutput, LaneRole};
use crate::oracles::lite_support::files_lanes;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::state::MissionState;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;

pub(crate) type Mother = Arc<FileSession<SessionPort>>;

pub(crate) struct SFx {
    pub(crate) home: Fixture,
    pub(crate) fake: Arc<FakeOllama>,
    pub(crate) invoked: Arc<AtomicUsize>,
}
impl SFx {
    /// Registered through the public R0 before `finish_bootstrap`.
    pub(crate) fn new() -> Self {
        let fake = Arc::new(FakeOllama::start());
        let home = Fixture::with_model("sb3c", &fake.endpoint(), spec());
        Self::of(home, fake)
    }
    /// Unconfigured: no registration (the fake peer stays unused).
    pub(crate) fn unregistered() -> Self {
        Self::of(Fixture::new("sb3c-none"), Arc::new(FakeOllama::start()))
    }
    fn of(home: Fixture, fake: Arc<FakeOllama>) -> Self {
        assert!(nexus_governed_control::files::make_non_dumpable());
        Self {
            home,
            fake,
            invoked: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn open(&self, lanes: Vec<Arc<dyn Lane>>) -> Mother {
        Arc::new(
            Coordinator::open(
                self.home.journal(),
                self.home.port.clone(),
                Lanes::new(lanes).unwrap(),
                self.home.clock.clone(),
                self.home.private(),
            )
            .unwrap()
            .into_file_session()
            .unwrap(),
        )
    }
    /// The C1b model Builder with `cap` and `script`.
    pub(crate) fn mother(
        &self,
        cap: u32,
        script: impl Fn(&LaneContext<'_>) -> LaneOutput + Send + Sync + 'static,
    ) -> Mother {
        let mut lanes: Vec<Arc<dyn Lane>> = keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::Builder)
            .collect();
        lanes.push(Arc::new(ModelLane {
            cap,
            script: Box::new(script),
            invoked: self.invoked.clone(),
        }));
        self.open(lanes)
    }
    /// The File lanes only (MKFILE): a run without any model lane.
    pub(crate) fn file_mother(&self) -> Mother {
        self.open(files_lanes())
    }
    fn folder(&self) -> String {
        self.home.session.workspace().unwrap().name
    }
    pub(crate) fn contract(&self) -> MissionContract {
        let mut c = contract_with("");
        c.requested_folder = Some(self.folder());
        c.budget.max_duration_ms = 60_000;
        c.model = Some(ModelChoice {
            name: crate::adapter::c1b_fake_ollama::NAME.into(),
            digest: crate::adapter::c1b_fake_ollama::DIGEST.into(),
            identity: identity_hex(),
        });
        c.deliverable = Some("index.html".into());
        c
    }
    pub(crate) fn file_contract(&self) -> MissionContract {
        let mut c = contract_with("MKFILE:session.txt");
        c.requested_folder = Some(self.folder());
        c.budget.max_duration_ms = 120_000;
        c
    }
    /// Consented and advanced to Building: the next advance invokes the
    /// model lane.
    pub(crate) fn building(&self, m: &Mother, c: MissionContract) -> MissionId {
        let id = m.draft(c).unwrap();
        m.submit(id).unwrap();
        answering(&Yes, || m.request_consent(id)).unwrap();
        m.advance(id).unwrap();
        m.advance(id).unwrap();
        assert_eq!(m.state(id).unwrap(), Some(MissionState::Building));
        id
    }
    /// An MKFILE mission advanced to AwaitingApproval: its File run exists.
    pub(crate) fn awaiting(&self, m: &Mother) -> MissionId {
        let id = m.draft(self.file_contract()).unwrap();
        m.submit(id).unwrap();
        answering(&Yes, || m.request_consent(id)).unwrap();
        for _ in 0..8 {
            if m.state(id).unwrap() == Some(MissionState::AwaitingApproval) {
                return id;
            }
            m.advance(id).unwrap();
        }
        panic!("the File mission reaches AwaitingApproval");
    }
    pub(crate) fn events(&self) -> Vec<Event> {
        verify(&self.home.lines())
            .unwrap()
            .into_iter()
            .map(|e| e.event)
            .collect()
    }
    pub(crate) fn charges(&self) -> usize {
        let charged = |e: &Event| matches!(e, Event::ModelCallCharged { .. });
        self.events().iter().filter(|e| charged(e)).count()
    }
}

/// `advance` on another thread, its result returned at join.
pub(crate) fn advance_in(
    m: &Mother,
    id: MissionId,
) -> std::thread::JoinHandle<Result<crate::coordinator::Advance, CoordinatorError>> {
    let m = m.clone();
    std::thread::spawn(move || m.advance(id))
}
pub(crate) fn within<T>(rx: &Receiver<T>) -> T {
    rx.recv_timeout(PATIENCE)
        .expect("the schedule step happens")
}
/// A one-shot signal pair.
pub(crate) fn signal() -> (std::sync::mpsc::SyncSender<()>, Receiver<()>) {
    sync_channel(1)
}
