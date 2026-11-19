//! M3-Dd composed page fixture (DD-R1, SB3E-R153): the actual
//! PageBuilderLane and PageVerifierLane behind forwarding observers, the
//! real C1 caller, SessionPort, session broker and fake loopback peer (the
//! TH-R1 B home, registered through the public R0). An unqualified fixture
//! route: not Q-M3, not live. Observers only record and pause.
use super::c1b_fake_ollama::{Answer, PATIENCE};
use super::m3cb_support::{outcome, Pause};
use super::session_model_bridge_support::{Mother, SFx};
use crate::coordinator::{Coordinator, Lanes};
use crate::journal::{verify, Event, MissionId};
use crate::lane::*;
use crate::page_builder::lane::PageBuilderLane;
use crate::page_verifier::PageVerifierLane;
use nexus_governed_control::authority::approval::ControlConfirmer;
use nexus_governed_control::inference::{ModelReply, ModelRequest};
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct Seen {
    /// Caller entries (before C1), and each one's result from real C1.
    pub calls: usize,
    pub results: Vec<String>,
    /// Per Builder invocation: its input is the consented snapshot.
    pub bound: Vec<bool>,
    pub verifier: usize,
    pub output: Option<LaneOutput>,
    pub pause_call: Option<(usize, Arc<Pause>)>,
}
struct Caller<'a> {
    real: &'a dyn ModelCaller,
    seen: &'a Mutex<Seen>,
}
impl ModelCaller for Caller<'_> {
    fn call(&self, r: &ModelRequest) -> Result<ModelReply, ModelCallError> {
        let mut seen = self.seen.lock().unwrap();
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
    seen: Arc<Mutex<Seen>>,
}
impl Lane for Observer {
    fn identity(&self) -> LaneIdentity {
        self.real.identity()
    }
    fn max_model_calls(&self) -> u32 {
        self.real.max_model_calls()
    }
    fn invoke(&self, i: &LaneInput) -> LaneOutput {
        self.real.invoke(i)
    }
    fn invoke_in(&self, i: &LaneInput, c: &LaneContext<'_>) -> LaneOutput {
        if i.role == LaneRole::Verifier {
            self.seen.lock().unwrap().verifier += 1;
        }
        if i.role != LaneRole::Builder {
            return self.real.invoke_in(i, c);
        }
        let bound = match (c.contract, i.builder_input.as_ref()) {
            (Some(k), Some(p)) => {
                p.validate(k, i.contract_hash, i.invocation).is_ok()
                    && p.borrowed() == (&k.notes[0], k.page_facts.as_ref().unwrap())
            }
            _ => false,
        };
        self.seen.lock().unwrap().bound.push(bound);
        let caller = c.model.map(|real| Caller {
            real,
            seen: &self.seen,
        });
        let context = LaneContext {
            contract: c.contract,
            model: caller.as_ref().map(|x| x as &dyn ModelCaller),
        };
        let out = self.real.invoke_in(i, &context);
        self.seen.lock().unwrap().output = Some(out.clone());
        out
    }
}
pub(super) struct Page {
    pub f: SFx,
    pub seen: Arc<Mutex<Seen>>,
    pub m: Mother,
    pub id: MissionId,
}
pub(super) fn lanes(seen: &Arc<Mutex<Seen>>) -> Vec<Arc<dyn Lane>> {
    crate::oracles::keyed_lanes()
        .into_iter()
        .map(|lane| {
            let real: Arc<dyn Lane> = match lane.identity().role {
                LaneRole::Builder => Arc::new(PageBuilderLane),
                LaneRole::Verifier => Arc::new(PageVerifierLane),
                _ => lane,
            };
            Arc::new(Observer {
                real,
                seen: seen.clone(),
            }) as Arc<dyn Lane>
        })
        .collect()
}
impl Page {
    /// Consented and at Building: the next advance runs the actual Builder.
    pub fn new(replies: Vec<Answer>) -> Self {
        let f = SFx::new();
        let replies = Mutex::new(VecDeque::from(replies));
        f.fake.route(move |p| {
            if p == "/api/chat" {
                replies.lock().unwrap().pop_front()
            } else {
                None
            }
        });
        let seen = Arc::new(Mutex::new(Seen::default()));
        let m = Arc::new(
            Coordinator::open(
                f.home.journal(),
                f.home.port.clone(),
                Lanes::new(lanes(&seen)).unwrap(),
                f.home.clock.clone(),
                f.home.private(),
            )
            .unwrap()
            .into_file_session()
            .unwrap(),
        );
        let mut c = f.contract();
        let data = crate::page_builder::cb_fixtures::contract();
        c.page_facts = data.page_facts;
        c.pass_criteria = data.pass_criteria;
        c.notes = data.notes;
        let id = f.building(&m, c);
        Self { f, seen, m, id }
    }
    pub fn advance_bg(
        &self,
    ) -> Receiver<Result<crate::coordinator::Advance, crate::coordinator::CoordinatorError>> {
        let (tx, rx) = channel();
        let (m, id) = (self.m.clone(), self.id);
        std::thread::spawn(move || {
            let _ = tx.send(m.advance(id));
        });
        rx
    }
    /// The Owner's File answer through session ingress, from `owner`
    /// (an oracles confirmer: scan.rs keeps every confirmer under src/oracles).
    pub fn approve(&self, owner: &dyn ControlConfirmer) -> bool {
        let kind = nexus_governed_control::host_session::CommandKind::Approve;
        // No pending File: nothing the Owner can approve.
        let Ok(request) = self.f.home.session.ingress().accept(kind, 50) else {
            return false;
        };
        self.f
            .home
            .session
            .approve_file_confirmed(&request, owner)
            .is_ok()
    }
    /// Caller entries, peer chats, `ModelCallCharged`.
    pub fn tally(&self) -> (usize, usize, usize) {
        let calls = self.seen.lock().unwrap().calls;
        (calls, self.f.fake.count("/api/chat"), self.f.charges())
    }
    pub fn results(&self) -> Vec<String> {
        self.seen.lock().unwrap().results.clone()
    }
    pub fn verifier(&self) -> usize {
        self.seen.lock().unwrap().verifier
    }
    pub fn output(&self) -> LaneOutput {
        let seen = self.seen.lock().unwrap();
        seen.output.clone().expect("the actual lane returned")
    }
    /// The broker's own native result record for the model slot.
    pub fn native(&self) -> &'static str {
        self.f
            .home
            .session
            .close_obligations()
            .unwrap()
            .model
            .record
    }
    pub fn index(&self) -> Option<Vec<u8>> {
        std::fs::read(self.f.home.workspace.join("index.html")).ok()
    }
    pub fn count(&self, f: fn(&Event) -> bool) -> usize {
        self.f.events().iter().filter(|e| f(e)).count()
    }
    /// The terminal transition, as `kind:code`.
    pub fn end(&self) -> String {
        use crate::state::Trigger;
        let trigger = verify(&self.f.home.lines())
            .unwrap()
            .into_iter()
            .filter(|e| e.mission == self.id)
            .filter_map(|e| match e.event {
                Event::Transition { trigger } => Some(trigger),
                _ => None,
            })
            .next_back()
            .unwrap();
        match trigger {
            Trigger::Stop { reason } => format!("stop:{reason}"),
            Trigger::Block { reason } => format!("block:{reason}"),
            Trigger::Fail { reason } => format!("fail:{reason}"),
            other => other.name().to_string(),
        }
    }
    /// No accepted Builder output, File dispatch or written page, and the
    /// reply's content is in neither the journal nor the private store.
    pub fn nothing_accepted(&self) {
        assert_eq!(self.f.model_lane_returned(), 0, "no LaneReturned");
        assert_eq!(self.f.dispatched(), 0, "no File dispatch");
        assert!(self.index().is_none(), "no File");
        let journal = self.f.home.lines().concat();
        let private = walk(&self.f.home.dir.join("private"));
        for surface in [journal, private] {
            assert!(
                !surface.contains(CONTENT),
                "the reply's content is withheld"
            );
        }
    }
}
/// A paragraph only the drafted reply carries (`cb_fixtures::DRAFT`).
pub(super) const CONTENT: &str = "Baked with care.";
/// Every file under `dir`, concatenated (lossy).
fn walk(dir: &std::path::Path) -> String {
    let mut text = String::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for path in entries.map(|e| e.unwrap().path()) {
            if path.is_dir() {
                pending.push(path);
            } else {
                text.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
            }
        }
    }
    text
}
/// A bounded receive of a schedule step.
pub(super) fn within<T>(rx: &Receiver<T>) -> T {
    rx.recv_timeout(PATIENCE)
        .expect("the schedule step happens")
}
