//! M1's pre-registered oracles (`MOTHER.md` §3.1): committed before the
//! implementation they judge, and not edited after a result is seen.
//! `README.md` names the invariant each test pins (`m1_i<n>_...`).

pub mod fake;

mod consent;
mod contract;
mod expiry;
mod stop;
mod structure;

use crate::contract::{MissionContract, Step};
use crate::coordinator::{ConsentRequest, Coordinator, MissionId, OwnerConsent};
use fake::FakeBroker;
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::browser::BrowserIntent;
use nexus_governed_control::connector::ConnectorIntent;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::governed::Intent;
use nexus_governed_control::tool::ToolIntent;
use std::sync::{Arc, Mutex};

pub const TTL_MS: u64 = 60_000;

pub fn request(url: &str) -> Intent {
    Intent::Request(EgressIntent {
        method: "GET".into(),
        url: url.into(),
        headers: vec![],
        body: None,
    })
}

pub fn tool(name: &str) -> Intent {
    Intent::Tool(ToolIntent {
        tool: name.into(),
        input: serde_json::json!({}),
    })
}

pub fn browse(url: &str) -> Intent {
    Intent::Browse(BrowserIntent {
        start_url: url.into(),
        steps: vec![],
    })
}

pub fn connector(operation: &str) -> Intent {
    Intent::Connector(ConnectorIntent {
        operation: operation.into(),
        input: serde_json::json!({}),
    })
}

pub fn step(intent: Intent) -> Step {
    Step {
        intent,
        declared_cost_minor: 0,
    }
}

pub fn contract(steps: Vec<Step>) -> MissionContract {
    MissionContract {
        goal: "Read the example page and summarise it".into(),
        steps,
        consent_ttl_ms: TTL_MS,
    }
}

/// One network step.
pub fn one_step() -> MissionContract {
    contract(vec![step(request("https://example.org/a"))])
}

/// Two network steps.
pub fn two_steps() -> MissionContract {
    contract(vec![
        step(request("https://example.org/a")),
        step(request("https://example.org/b")),
    ])
}

/// Says yes.
pub struct Yes;
impl OwnerConsent for Yes {
    fn confirm_mission(&self, _: &ConsentRequest) -> bool {
        true
    }
}

/// Says no.
pub struct No;
impl OwnerConsent for No {
    fn confirm_mission(&self, _: &ConsentRequest) -> bool {
        false
    }
}

/// Says yes and keeps what it was shown.
#[derive(Default)]
pub struct Recording(pub Mutex<Vec<ConsentRequest>>);
impl OwnerConsent for Recording {
    fn confirm_mission(&self, request: &ConsentRequest) -> bool {
        self.0.lock().unwrap().push(request.clone());
        true
    }
}

pub struct Setup {
    pub broker: Arc<FakeBroker>,
    pub clock: Arc<ManualClock>,
    pub mother: Arc<Coordinator<FakeBroker>>,
}

pub fn setup() -> Setup {
    let broker = FakeBroker::new();
    let clock = Arc::new(ManualClock::default());
    let mother = Arc::new(Coordinator::new(broker.clone(), clock.clone()));
    Setup {
        broker,
        clock,
        mother,
    }
}

impl Setup {
    /// Draft and consent.
    pub fn approved(&self, contract: MissionContract) -> MissionId {
        let id = self.mother.draft(contract).unwrap();
        self.mother.request_consent(id, &Yes).unwrap();
        id
    }
}
