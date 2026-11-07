//! M3-Db real-broker controls, frozen before implementation: the accepted
//! page Builder's retained deliverable and the broker's receipt of its one
//! publication. Every scenario owns one fresh broker in its own process.
use super::lite_port::RECEIPT_SEAM;
use super::m2c_support::{port_for, Home};
use super::{create_private, mother_agent, GovernedPort};
use crate::contract::{MissionContract, Note};
use crate::coordinator::{Coordinator, FileSession, Lanes};
use crate::effect::{EffectPort, EffectRequest};
use crate::journal::{verify, Event, JournalError, JournalStore, MemoryJournal, StoreFailure};
use crate::journal::{Entry, MissionId};
use crate::lane::{Lane, LaneRole, ScriptedLane, Verdict};
use crate::oracles::lite_support::{file, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::page_inputs::{page_criteria, PageFacts};
use crate::private::{ContentKind, PrivateStore};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::files::FileReceipt;
use nexus_governed_control::governed::GrantRequest;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

pub(crate) const BAKERY: &str = include_str!("../oracles/m3a_bakery.html");
/// Same size as the bakery page, other bytes.
pub(crate) fn other_page() -> String {
    BAKERY.replace("Lemon drizzle", "Apple crumble")
}
/// RULING 7 (and 6): what the Owner is told of a written, unproven page.
pub(crate) const WRITTEN: &str = "written; proof not recorded";
const CASE: &str = "NEXUS_M3D_DB_CASE";

pub(crate) fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Run `case` of the child test at `path` in a fresh process.
pub(crate) fn run_child(path: &str, case: &str) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", path, "--nocapture", "--test-threads=1"])
        .env(CASE, case)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{case}: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(&format!("m3d-db-receipt:{case}")),
        "{case} ran"
    );
}

pub(crate) fn child_case() -> Option<String> {
    std::env::var(CASE).ok()
}

/// Appends fail on every line containing `needle`.
pub(crate) struct FailOn {
    pub memory: MemoryJournal,
    pub needle: &'static str,
}
impl JournalStore for FailOn {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.memory.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        if line.contains(self.needle) {
            return Err(JournalError::Store(StoreFailure::Injected));
        }
        self.memory.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.memory.poison()
    }
}

pub(crate) struct PageFx {
    pub home: Home,
    pub folder: String,
    pub ws: std::path::PathBuf,
    pub port: Arc<GovernedPort>,
    pub journal: MemoryJournal,
    owner: FilesOwner,
}

impl PageFx {
    pub fn new() -> Self {
        assert!(nexus_governed_control::files::make_non_dumpable());
        let home = Home::fresh_temp("m3db").unwrap();
        let ws = home.dir.join("workspace");
        create_private(&ws).unwrap();
        let info = home.control.register_workspace(&ws, &[]).unwrap();
        let owner = FilesOwner::default();
        home.control
            .request_grant(
                &GrantRequest::Files { workspace: info.id },
                Duration::from_secs(86400),
                &owner,
            )
            .unwrap();
        let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
        Self {
            home,
            folder: info.name,
            ws,
            port,
            journal: MemoryJournal::default(),
            owner,
        }
    }
    /// A File session whose Builder asks for exactly `effects`.
    pub fn session(
        &self,
        effects: Vec<EffectRequest>,
        store: impl JournalStore + 'static,
    ) -> FileSession<GovernedPort> {
        let mut lanes: Vec<Arc<dyn Lane>> = keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::Builder)
            .collect();
        let builder = ScriptedLane::new(LaneRole::Builder, "built", Verdict::Continue);
        lanes.push(Arc::new(builder.asking(effects)));
        Coordinator::open(
            store,
            self.port.clone(),
            Lanes::new(lanes).unwrap(),
            Arc::new(SystemClock::default()),
            self.home.private(),
        )
        .unwrap()
        .into_file_session()
        .unwrap()
    }
    pub fn contract(&self) -> MissionContract {
        let mut c = contract_with("");
        c.requested_folder = Some(self.folder.clone());
        c.deliverable = Some("index.html".into());
        c.notes = vec![Note {
            label: "notes".into(),
            text: "Fresh bread every morning.".into(),
        }];
        c.page_facts = Some(PageFacts {
            name: "Rosa's Bakery".into(),
            hours: vec![
                "Tuesday to Sunday, 7am to 3pm".into(),
                "Closed on Mondays".into(),
            ],
            phone: "555-0142".into(),
        });
        c.pass_criteria = page_criteria();
        c
    }
    /// Drafted, consented and advanced `steps` times.
    pub fn started(&self, m: &FileSession<GovernedPort>, steps: usize) -> MissionId {
        let id = m.draft(self.contract()).unwrap();
        m.submit(id).unwrap();
        assert_eq!(
            answering(&Yes, || m.request_consent(id)),
            Ok(MissionState::Approved)
        );
        for _ in 0..steps {
            m.advance(id).unwrap();
        }
        id
    }
    /// Waiting on the Builder's File at the broker.
    pub fn waiting(&self, m: &FileSession<GovernedPort>) -> MissionId {
        let id = self.started(m, 3);
        assert_eq!(m.state(id), Ok(Some(MissionState::AwaitingApproval)));
        id
    }
    fn ids(&self, id: MissionId) -> (RunId, CommitmentId) {
        let p = self.port.pending(id).unwrap();
        (
            RunId::parse(&p.run).unwrap(),
            CommitmentId::parse(&p.commitment).unwrap(),
        )
    }
    /// The Owner approves the File at the broker, and it runs.
    pub fn publish(&self, id: MissionId) {
        let (run, c) = self.ids(id);
        let control = &self.home.control;
        control
            .authorize(c, &mother_agent(), run, &self.owner)
            .unwrap();
        control.execute(c, &mother_agent(), run).unwrap();
    }
    pub fn deny(&self, id: MissionId) {
        let (run, c) = self.ids(id);
        self.home.control.deny(c, &mother_agent(), run).unwrap();
    }
    pub fn entries(&self) -> Vec<Entry> {
        verify(&self.journal.lines()).unwrap()
    }
    pub fn events(&self) -> Vec<Event> {
        self.entries().into_iter().map(|e| e.event).collect()
    }
    /// Remove or rewrite a retained blob behind the store's back.
    pub fn alter_blob(&self, content: &crate::private::ContentId, bytes: Option<&[u8]>) {
        let blob = self
            .home
            .dir
            .join("private/deliverable")
            .join(content.to_hex());
        match bytes {
            Some(bytes) => std::fs::write(blob, bytes).unwrap(),
            None => std::fs::remove_file(blob).unwrap(),
        }
    }
    pub fn written(&self) -> Option<Vec<u8>> {
        std::fs::read(self.ws.join("index.html")).ok()
    }
}

pub(crate) fn count(events: &[Event], f: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| f(e)).count()
}
pub(crate) fn at(events: &[Event], f: impl Fn(&Event) -> bool) -> Option<usize> {
    events.iter().position(f)
}
pub(crate) fn is_receipt(e: &Event) -> bool {
    matches!(e, Event::PublicationReceipt { .. })
}
pub(crate) fn is_retained(e: &Event) -> bool {
    matches!(e, Event::DeliverableRetained { .. })
}
pub(crate) fn is_dispatch(e: &Event) -> bool {
    matches!(e, Event::EffectDispatched { .. })
}
pub(crate) fn is_approved(e: &Event) -> bool {
    matches!(
        e,
        Event::Transition {
            trigger: Trigger::ActionApproved { .. }
        }
    )
}
/// Lane invocations after `from`, of any role.
pub(crate) fn lanes_after(events: &[Event], from: usize) -> usize {
    count(&events[from..], |e| matches!(e, Event::LaneInvoked { .. }))
}

fn seam(f: fn(Option<FileReceipt>) -> Option<FileReceipt>) {
    RECEIPT_SEAM.with(|s| s.set(Some(f)));
}

const CHILD: &str = "adapter::m3d_publication_oracles::m3d_db_broker_child";

#[test]
fn m3d_deliverable_is_private_before_dispatch() {
    for case in ["o33a-one", "o33a-name", "o33a-two", "o33a-mixed"] {
        run_child(CHILD, case);
    }
}

#[test]
fn m3d_receipt_is_broker_derived_and_action_bound() {
    for case in ["dp1-clean", "dp1-other", "dp1-denied", "dp1-stopped"] {
        run_child(CHILD, case);
    }
}

#[test]
fn m3d_missing_page_receipt_never_claims_nonpublication() {
    for case in ["dp2-receipt", "dp2-policy"] {
        run_child(CHILD, case);
    }
}

#[test]
fn m3d_proof_append_failure_prevents_success() {
    for case in ["dp3-retention", "dp3-approved", "dp3-receipt"] {
        run_child(CHILD, case);
    }
}

#[test]
fn m3d_overtaken_publication_keeps_marked_settlement_evidence() {
    run_child(CHILD, "overtaken-stop");
    run_child(CHILD, "overtaken-nopolicy");
}

#[test]
fn m3d_db_broker_child() {
    let Some(case) = child_case() else {
        return;
    };
    let f = PageFx::new();
    match case.as_str() {
        "o33a-one" => {
            let m = f.session(vec![file("index.html", BAKERY)], f.journal.clone());
            let id = f.waiting(&m);
            let events = f.events();
            assert_eq!(count(&events, is_retained), 1);
            let r = at(&events, is_retained).unwrap();
            let Event::DeliverableRetained {
                invocation,
                effect,
                content,
                sha256,
                bytes,
            } = events[r].clone()
            else {
                unreachable!()
            };
            assert!(
                matches!(&events[r - 1], Event::LaneReturned { invocation: i, .. } if *i == invocation)
            );
            assert!(
                matches!(&events[r + 1], Event::EffectDispatched { effect: e, .. } if *e == effect)
            );
            assert_eq!(count(&events, is_dispatch), 1);
            assert_eq!(invocation.mission, id);
            assert_eq!(sha256, sha(BAKERY.as_bytes()));
            assert_eq!(bytes, BAKERY.len() as u64);
            let kept = f.home.private().get(ContentKind::Deliverable, &content);
            assert_eq!(kept.unwrap(), BAKERY.as_bytes());
            let lines = f.journal.lines().join("\n");
            for canary in ["Lemon drizzle", "555-0142", "Rosa's Bakery"] {
                assert!(!lines.contains(canary), "{canary}");
            }
            assert_eq!(f.written(), None, "nothing published before approval");
        }
        "o33a-name" | "o33a-two" | "o33a-mixed" => {
            let effects = match case.as_str() {
                "o33a-name" => vec![file("page.html", BAKERY)],
                "o33a-two" => vec![file("index.html", BAKERY); 2],
                _ => vec![file("index.html", BAKERY), file("notes.txt", "notes\n")],
            };
            let m = f.session(effects, f.journal.clone());
            let id = f.started(&m, 3);
            assert_eq!(m.state(id), Ok(Some(MissionState::Failed)));
            assert_eq!(m.reason(id), Ok(Some("deliverable_invalid".into())));
            let events = f.events();
            assert_eq!(count(&events, is_retained), 0);
            assert_eq!(count(&events, is_dispatch), 0);
            assert_eq!(f.written(), None);
            assert!(!f.ws.join("page.html").exists());
        }
        "dp1-clean" | "dp1-other" => {
            let other = case == "dp1-other";
            let m = f.session(vec![file("index.html", BAKERY)], f.journal.clone());
            let id = f.waiting(&m);
            f.publish(id);
            if other {
                seam(|r| {
                    r.map(|mut r| {
                        r.sha256 = Sha256::digest(other_page().as_bytes()).into();
                        r
                    })
                });
            }
            assert_eq!(m.advance(id).unwrap().to, MissionState::Verifying);
            let disk = f.written().unwrap();
            assert_eq!(disk, BAKERY.as_bytes());
            let events = f.events();
            assert_eq!(count(&events, is_receipt), 1);
            let p = at(&events, is_receipt).unwrap();
            let Event::PublicationReceipt {
                effect,
                action,
                sha256,
                bytes,
                policy_hash,
                overtaken,
            } = events[p].clone()
            else {
                unreachable!()
            };
            let policy = nexus_governed_control::inert_page::policy_hash();
            let broker = if other {
                other_page().into_bytes()
            } else {
                disk
            };
            assert_eq!(sha256, sha(&broker));
            assert_eq!(bytes, BAKERY.len() as u64);
            assert_eq!(policy_hash, hex::encode(policy));
            assert!(!overtaken);
            assert!(events.iter().any(|e| matches!(e, Event::ApprovalPending { effect: e2, pending } if *e2 == effect && *pending == action)));
            assert!(events.iter().any(
                |e| matches!(e, Event::DeliverableRetained { effect: e2, .. } if *e2 == effect)
            ));
            let approved = at(&events, is_approved).unwrap();
            let closed = at(
                &events,
                |e| matches!(e, Event::ApprovalClosed { effect: e2 } if *e2 == effect),
            );
            assert!(
                approved < p && p < closed.unwrap(),
                "receipt before closure"
            );
            // The journaled receipt is the broker's: another candidate shows.
            let receipt = crate::verifier::Receipt {
                sha256: hex::decode(&sha256).unwrap().try_into().unwrap(),
                bytes,
                policy_hash: policy,
            };
            let facts = f.contract().page_facts.unwrap().approved().unwrap();
            let report = crate::verifier::verify_page(BAKERY.as_bytes(), &facts, Some(&receipt));
            assert_eq!(
                report.first_failure().map(|c| c.to_string()),
                other.then(|| "published_mismatch".to_string())
            );
        }
        "dp1-denied" | "dp1-stopped" => {
            let m = f.session(vec![file("index.html", BAKERY)], f.journal.clone());
            let id = f.waiting(&m);
            if case == "dp1-denied" {
                f.deny(id);
                m.advance(id).unwrap();
                assert_eq!(m.state(id), Ok(Some(MissionState::Denied)));
            } else {
                assert_eq!(m.stop(id), Ok(MissionState::Stopped));
            }
            assert_eq!(count(&f.events(), is_receipt), 0);
            assert_eq!(f.written(), None);
        }
        "dp2-receipt" | "dp2-policy" => {
            let m = f.session(vec![file("index.html", BAKERY)], f.journal.clone());
            let id = f.waiting(&m);
            f.publish(id);
            let code = if case == "dp2-receipt" {
                seam(|_| None);
                "receipt_missing"
            } else {
                seam(|r| {
                    r.map(|mut r| {
                        r.policy_hash = None;
                        r
                    })
                });
                "receipt_policy_missing"
            };
            let r = m.advance(id).unwrap();
            assert_eq!(r.to, MissionState::Failed);
            assert_eq!(r.reason.as_deref(), Some(code));
            assert_eq!(r.explanation.as_deref(), Some(WRITTEN));
            assert_eq!(f.written().unwrap(), BAKERY.as_bytes());
            let events = f.events();
            let approved = at(&events, is_approved).expect("the publication stays recorded");
            assert_eq!(count(&events, is_receipt), 0);
            assert_eq!(lanes_after(&events, approved), 0, "no verifier");
            assert_eq!(count(&events, is_dispatch), 1);
            let reasons = format!("{events:?} {r:?}");
            assert!(!reasons.contains("not_published") && !reasons.contains("NotPublished"));
            // Terminal, no retry: nothing more is asked or written.
            assert!(m.advance(id).is_err());
            assert_eq!(m.state(id), Ok(Some(MissionState::Failed)));
            assert_eq!(f.events(), events);
        }
        "dp3-retention" | "dp3-approved" | "dp3-receipt" => {
            let needle = match case.as_str() {
                "dp3-retention" => "\"event\":\"deliverable_retained\"",
                "dp3-approved" => "\"trigger\":\"action_approved\"",
                _ => "\"event\":\"publication_receipt\"",
            };
            let store = FailOn {
                memory: f.journal.clone(),
                needle,
            };
            let m = f.session(vec![file("index.html", BAKERY)], store);
            if case == "dp3-retention" {
                let id = f.started(&m, 2);
                assert!(m.advance(id).is_err());
                assert_eq!(count(&f.events(), is_dispatch), 0);
                assert!(f.port.pending(id).is_none(), "nothing proposed");
                assert_eq!(f.written(), None);
            } else {
                let id = f.waiting(&m);
                f.publish(id);
                let r = m.advance(id).unwrap();
                assert_eq!(r.to, MissionState::Failed);
                assert_eq!(r.explanation.as_deref(), Some(WRITTEN));
                assert_eq!(f.written().unwrap(), BAKERY.as_bytes());
                let events = f.events();
                assert_eq!(count(&events, is_receipt), 0);
                assert_eq!(
                    count(&events, |e| matches!(e, Event::ApprovalClosed { .. })),
                    0
                );
                let after = at(&events, is_dispatch).unwrap();
                assert_eq!(lanes_after(&events, after), 0);
                assert!(f.journal.is_poisoned());
                assert!(m.advance(id).is_err(), "nothing continues");
            }
        }
        "overtaken-stop" => {
            let m = f.session(vec![file("index.html", BAKERY)], f.journal.clone());
            let id = f.waiting(&m);
            f.publish(id);
            assert_eq!(m.stop(id), Ok(MissionState::Blocked));
            let events = f.events();
            assert_eq!(count(&events, is_receipt), 1);
            let p = at(&events, is_receipt).unwrap();
            assert!(
                matches!(&events[p], Event::PublicationReceipt { overtaken: true, sha256, .. } if *sha256 == sha(BAKERY.as_bytes()))
            );
            assert!(matches!(
                &events[p + 1],
                Event::Transition {
                    trigger: Trigger::Settled { .. }
                }
            ));
            assert_eq!(count(&events, is_approved), 0);
            assert_eq!(lanes_after(&events, p), 0);
            assert_eq!(f.written().unwrap(), BAKERY.as_bytes());
        }
        "overtaken-nopolicy" => {
            // Db-Q4: settlement evidence is kept even without a policy.
            let m = f.session(vec![file("index.html", BAKERY)], f.journal.clone());
            let id = f.waiting(&m);
            f.publish(id);
            seam(|r| {
                r.map(|mut r| {
                    r.policy_hash = None;
                    r
                })
            });
            assert_eq!(m.stop(id), Ok(MissionState::Blocked));
            let events = f.events();
            assert_eq!(count(&events, is_receipt), 1, "the receipt is not dropped");
            let p = at(&events, is_receipt).unwrap();
            let json = serde_json::to_value(&events[p]).unwrap();
            assert_eq!(json["overtaken"], true);
            assert!(
                json.get("policy_hash").is_none(),
                "policy absent, not invented"
            );
            assert_eq!(json["sha256"], sha(BAKERY.as_bytes()));
            assert!(matches!(
                &events[p + 1],
                Event::Transition {
                    trigger: Trigger::Settled { .. }
                }
            ));
            assert_eq!(count(&events, is_approved), 0, "no success transition");
            assert_eq!(lanes_after(&events, p), 0, "no verifier");
            assert_eq!(m.state(id), Ok(Some(MissionState::Blocked)));
        }
        other => panic!("unknown case {other}"),
    }
    println!("m3d-db-receipt:{case}");
}
