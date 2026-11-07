//! Forwarding observers: all page behavior comes from the actual lane.
use super::{
    c1b_coordinator_oracles::{Fx, ProbeJournal},
    c1b_fake_ollama::Answer,
    GovernedPort,
};
use crate::page_builder::{lane::PageBuilderLane, take_file_constructions_for_test};
use crate::{
    coordinator::{Coordinator, FileSession, Lanes},
    journal::MissionId,
    lane::*,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
#[derive(Default)]
pub(super) struct Observed {
    pub calls: usize,
    pub constructions: Vec<usize>,
    pub output: Option<LaneOutput>,
    pub contexts: Vec<(LaneRole, u32, Option<crate::contract::MissionContract>)>,
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
        seen.calls += 1;
        drop(seen);
        self.real.call(r)
    }
}
struct Observer {
    real: Arc<dyn Lane>,
    seen: Arc<Mutex<Observed>>,
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
        out
    }
}
pub(super) struct Scenario {
    pub fx: Fx,
    pub seen: Arc<Mutex<Observed>>,
    pub m: Arc<FileSession<GovernedPort>>,
    pub c: crate::contract::MissionContract,
    pub id: MissionId,
}
pub(super) fn mother(
    fx: &Fx,
    seen: &Arc<Mutex<Observed>>,
    page: bool,
) -> Arc<FileSession<GovernedPort>> {
    let lanes = crate::oracles::keyed_lanes()
        .into_iter()
        .map(|lane| {
            let real = if page && lane.identity().role == LaneRole::Builder {
                Arc::new(PageBuilderLane) as Arc<dyn Lane>
            } else {
                lane
            };
            Arc::new(Observer {
                real,
                seen: seen.clone(),
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
        let m = mother(&fx, &seen, true);
        let mut c = fx.contract();
        let data = crate::page_builder::cb_fixtures::contract();
        c.page_facts = data.page_facts;
        c.pass_criteria = data.pass_criteria;
        c.notes = data.notes;
        c.notes[0].text = note.into();
        let (id, _) = fx.building(&m, c.clone());
        Self { fx, seen, m, c, id }
    }
    pub fn advance(&self) {
        self.m.advance(self.id).unwrap();
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
