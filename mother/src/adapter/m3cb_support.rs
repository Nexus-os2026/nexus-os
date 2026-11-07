//! Forwarding observers: all page behavior comes from the actual lane.
use super::{
    c1b_coordinator_oracles::{within, Fx, ProbeJournal},
    c1b_fake_ollama::{Answer, PATIENCE},
    GovernedPort,
};
use crate::page_builder::{lane::PageBuilderLane, take_file_constructions_for_test};
use crate::{
    coordinator::{Advance, Coordinator, CoordinatorError, FileSession, Lanes},
    journal::MissionId,
    lane::*,
};
use std::{
    collections::VecDeque,
    sync::mpsc::{channel, sync_channel, Receiver, SyncSender},
    sync::{Arc, Mutex},
};
/// A lane-thread pause: the test sees `reached`, acts, then releases (bounded).
pub(super) struct Pause(SyncSender<()>, Mutex<Receiver<()>>);
impl Pause {
    pub fn hold(&self) {
        let _ = self.0.send(());
        let _ = self.1.lock().unwrap().recv_timeout(PATIENCE);
    }
}
pub(super) struct Held(Receiver<()>, SyncSender<()>);
impl Held {
    pub fn reached(&self) {
        within(&self.0)
    }
    pub fn release(&self) {
        self.1.send(()).unwrap()
    }
}
pub(super) fn pause() -> (Arc<Pause>, Held) {
    let ((a, b), (c, d)) = (sync_channel(1), sync_channel(1));
    (Arc::new(Pause(a, Mutex::new(d))), Held(b, c))
}
pub(super) fn outcome<T>(r: &Result<T, ModelCallError>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(ModelCallError::Invalidated) => "invalidated".into(),
        Err(ModelCallError::RecordUnavailable) => "record_unavailable".into(),
        Err(ModelCallError::Refused(c)) => format!("refused:{c}"),
        Err(ModelCallError::Failed(c)) => format!("failed:{c}"),
    }
}
#[derive(Default)]
pub(super) struct Observed {
    pub calls: usize,
    pub constructions: Vec<usize>,
    pub output: Option<LaneOutput>,
    pub contexts: Vec<(LaneRole, u32, Option<crate::contract::MissionContract>)>,
    /// Each caller entry's result, as returned by real C1.
    pub results: Vec<String>,
    /// Pause at caller entry `n` (0-based), before delegating to C1.
    pub pause_call: Option<(usize, Arc<Pause>)>,
    /// Pause after the actual lane returned, before the coordinator sees it.
    pub pause_answer: Option<Arc<Pause>>,
}
struct Caller<'a> {
    real: &'a dyn ModelCaller,
    seen: &'a Mutex<Observed>,
}
impl ModelCaller for Caller<'_> {
    fn call(
        &self,
        r: &nexus_governed_control::inference::ModelRequest,
    ) -> Result<nexus_governed_control::inference::ModelReply, ModelCallError> {
        let mut seen = self.seen.lock().unwrap();
        if seen.calls == 1 {
            seen.constructions.push(take_file_constructions_for_test());
        }
        let at = seen.calls;
        seen.calls += 1;
        let pause = seen.pause_call.clone().filter(|(n, _)| *n == at);
        drop(seen);
        if let Some((_, p)) = pause {
            p.hold();
        }
        let result = self.real.call(r);
        self.seen.lock().unwrap().results.push(outcome(&result));
        result
    }
}
struct Observer {
    real: Arc<dyn Lane>,
    seen: Arc<Mutex<Observed>>,
    cap: Option<u32>,
}
impl Lane for Observer {
    fn identity(&self) -> LaneIdentity {
        self.real.identity()
    }
    fn max_model_calls(&self) -> u32 {
        self.cap.unwrap_or_else(|| self.real.max_model_calls())
    }
    fn invoke(&self, i: &LaneInput) -> LaneOutput {
        self.real.invoke(i)
    }
    fn invoke_in(&self, i: &LaneInput, c: &LaneContext<'_>) -> LaneOutput {
        self.seen.lock().unwrap().contexts.push((
            i.role,
            self.max_model_calls(),
            c.contract.cloned(),
        ));
        if i.role != LaneRole::Builder {
            return self.real.invoke_in(i, c);
        }
        take_file_constructions_for_test();
        let caller = c.model.map(|real| Caller {
            real,
            seen: &self.seen,
        });
        let context = LaneContext {
            contract: c.contract,
            model: caller.as_ref().map(|x| x as &dyn ModelCaller),
        };
        let out = self.real.invoke_in(i, &context);
        let mut seen = self.seen.lock().unwrap();
        seen.constructions.push(take_file_constructions_for_test());
        seen.output = Some(out.clone());
        let pause = seen.pause_answer.clone();
        drop(seen);
        if let Some(p) = pause {
            p.hold();
        }
        out
    }
}
pub(super) struct Scenario {
    pub fx: Fx,
    pub seen: Arc<Mutex<Observed>>,
    pub m: Arc<FileSession<GovernedPort>>,
    pub c: crate::contract::MissionContract,
    pub id: MissionId,
    pub asked: nexus_governed_control::authority::approval::MissionConfirmation,
}
pub(super) fn mother(
    fx: &Fx,
    seen: &Arc<Mutex<Observed>>,
    page: bool,
) -> Arc<FileSession<GovernedPort>> {
    mother_with(fx, seen, page, None)
}
/// `verifier_cap` overrides only the observed Verifier's declared cap.
pub(super) fn mother_with(
    fx: &Fx,
    seen: &Arc<Mutex<Observed>>,
    page: bool,
    verifier_cap: Option<u32>,
) -> Arc<FileSession<GovernedPort>> {
    let lanes = crate::oracles::keyed_lanes()
        .into_iter()
        .map(|lane| {
            let real = if page && lane.identity().role == LaneRole::Builder {
                Arc::new(PageBuilderLane) as Arc<dyn Lane>
            } else {
                lane
            };
            let cap = verifier_cap.filter(|_| real.identity().role == LaneRole::Verifier);
            Arc::new(Observer {
                real,
                seen: seen.clone(),
                cap,
            }) as Arc<dyn Lane>
        })
        .collect();
    Arc::new(
        Coordinator::open(
            ProbeJournal {
                journal: fx.journal.clone(),
                probe: fx.probe.clone(),
            },
            fx.port.clone(),
            Lanes::new(lanes).unwrap(),
            fx.clock.clone(),
            fx.home.private(),
        )
        .unwrap()
        .into_file_session()
        .unwrap(),
    )
}
impl Scenario {
    pub fn new(replies: Vec<Answer>, note: &str) -> Self {
        Self::with(replies, |c| c.notes[0].text = note.into(), None)
    }
    pub fn with(
        replies: Vec<Answer>,
        edit: impl FnOnce(&mut crate::contract::MissionContract),
        verifier_cap: Option<u32>,
    ) -> Self {
        let fx = Fx::new();
        let replies = Mutex::new(VecDeque::from(replies));
        fx.fake.route(move |p| {
            if p == "/api/chat" {
                replies.lock().unwrap().pop_front()
            } else {
                None
            }
        });
        let seen = Arc::new(Mutex::new(Observed::default()));
        let m = mother_with(&fx, &seen, true, verifier_cap);
        let mut c = fx.contract();
        let data = crate::page_builder::cb_fixtures::contract();
        c.page_facts = data.page_facts;
        c.pass_criteria = data.pass_criteria;
        c.notes = data.notes;
        edit(&mut c);
        let (id, asked) = fx.building(&m, c.clone());
        Self {
            fx,
            seen,
            m,
            c,
            id,
            asked,
        }
    }
    pub fn advance(&self) {
        self.m.advance(self.id).unwrap();
        self.contexts();
    }
    /// The advance on another thread; receive it with a bounded `within`.
    pub fn advance_bg(&self) -> Receiver<Result<Advance, CoordinatorError>> {
        let (tx, rx) = channel();
        let (m, id) = (self.m.clone(), self.id);
        std::thread::spawn(move || {
            let _ = tx.send(m.advance(id));
        });
        rx
    }
    pub fn contexts(&self) {
        let seen = self.seen.lock().unwrap();
        for (role, cap, c) in &seen.contexts {
            if *role == LaneRole::Builder && *cap > 0 {
                assert_eq!(c.as_ref(), Some(&self.c));
            } else {
                assert!(c.is_none(), "contract leaked to {role:?}/{cap}");
            }
        }
    }
    pub fn counts(&self, n: usize) {
        assert_eq!(
            (
                self.seen.lock().unwrap().calls,
                self.fx.fake.count("/api/chat"),
                self.fx.charges(),
                self.fx.attempts()
            ),
            (n, n, n, n)
        );
        assert_eq!(self.fx.model_invocations(), 1);
    }
    pub fn output(&self) -> LaneOutput {
        self.seen
            .lock()
            .unwrap()
            .output
            .clone()
            .expect("actual lane returned")
    }
    pub fn bodies(&self) -> Vec<Vec<u8>> {
        self.fx.fake.bodies("/api/chat")
    }
}
