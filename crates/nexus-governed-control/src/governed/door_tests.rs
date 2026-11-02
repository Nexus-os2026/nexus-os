//! M2c-A (PR-A) oracles for the agent door, pre-registered before the
//! door exists (design v3 §7.1): A2 cross-agent, A3 group confinement, A7
//! every door issue asks, and the broker side of A4 (consent) and A5/A6
//! (held-mission resume). Each refusal carries a positive control on the
//! same door, so a door that refuses everything fails.
//!
//! Every race uses a forced order (a confirmer that waits for the other
//! thread), never a stress loop or a fixed sleep; every thread is joined
//! through a channel with a bounded wait, which only detects a deadlock.

use super::{AgentOutcome, GovernedControl, GrantRequest, Intent, Lookup};
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};
use crate::authority::clock::SystemClock;
use crate::authority::evidence::{EvidencePhase, EvidenceRecord, MemoryEvidence};
use crate::authority::ids::{AgentId, CommitmentId, RunId};
use crate::authority::run::RunOrigin;
use crate::authority::AuthorityError;
use crate::broker::Vault;
use crate::egress::EgressIntent;
use crate::evidence_store::tests::Scratch;
use crate::evidence_store::{Fault, PrimaryEvidence};
use crate::harness_tests::{temp_root, Reply, TempRoot, TestServer, Yes};
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// A bounded join: it detects a deadlock and never makes a test pass.
const JOIN: Duration = Duration::from_secs(10);

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// ---------------------------------------------------------------- owner

/// The Owner at the door's dialogs: answers each kind as set, counts and
/// keeps every request, and can wait inside the door dialog until `meet`
/// door dialogs are open at once (a forced concurrent schedule).
struct Owner {
    door: bool,
    mission: bool,
    held: bool,
    doors: Mutex<Vec<AgentDoorConfirmation>>,
    missions: Mutex<Vec<MissionConfirmation>>,
    helds: Mutex<Vec<MissionResumeConfirmation>>,
    meet: usize,
    open: Mutex<usize>,
    met: Condvar,
    during: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Owner {
    fn new(door: bool, mission: bool, held: bool) -> Arc<Self> {
        Arc::new(Self {
            door,
            mission,
            held,
            doors: Mutex::new(Vec::new()),
            missions: Mutex::new(Vec::new()),
            helds: Mutex::new(Vec::new()),
            meet: 0,
            open: Mutex::new(0),
            met: Condvar::new(),
            during: Mutex::new(None),
        })
    }

    fn yes() -> Arc<Self> {
        Self::new(true, true, true)
    }

    fn meeting(n: usize) -> Arc<Self> {
        let mut owner = Self::new(true, false, false);
        Arc::get_mut(&mut owner).unwrap().meet = n;
        owner
    }

    /// Run `f` once, inside the next dialog of any kind.
    fn during(&self, f: impl FnOnce() + Send + 'static) {
        *self.during.lock().unwrap() = Some(Box::new(f));
    }

    fn fire(&self) {
        let f = self.during.lock().unwrap().take();
        if let Some(f) = f {
            f();
        }
    }

    fn doors(&self) -> Vec<AgentDoorConfirmation> {
        self.doors.lock().unwrap().clone()
    }

    fn missions(&self) -> Vec<MissionConfirmation> {
        self.missions.lock().unwrap().clone()
    }

    fn helds(&self) -> Vec<MissionResumeConfirmation> {
        self.helds.lock().unwrap().clone()
    }
}

impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        self.missions.lock().unwrap().push(request.clone());
        self.fire();
        self.mission
    }
    fn confirm_agent_door(&self, request: &AgentDoorConfirmation) -> bool {
        self.doors.lock().unwrap().push(request.clone());
        if self.meet > 0 {
            let mut open = self.open.lock().unwrap();
            *open += 1;
            self.met.notify_all();
            let (open, timeout) = self
                .met
                .wait_timeout_while(open, JOIN, |open| *open < self.meet)
                .unwrap();
            assert!(
                !timeout.timed_out(),
                "the other door dialog never opened while this one was open: {}",
                *open
            );
        }
        self.fire();
        self.door
    }
    fn confirm_held_mission(&self, request: &MissionResumeConfirmation) -> bool {
        self.helds.lock().unwrap().push(request.clone());
        self.fire();
        self.held
    }
}

/// A confirmer that predates the door: it keeps the trait's default
/// answers for the door and held-mission dialogs.
struct Legacy;
impl ControlConfirmer for Legacy {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
}

/// Says yes to the door dialog only; every other answer is the trait's or
/// no.
struct DoorOnly;
impl ControlConfirmer for DoorOnly {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
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
}

// ---------------------------------------------------------------- fixture

fn mother() -> AgentId {
    AgentId::new("mother").unwrap()
}

struct Fixture {
    control: Arc<GovernedControl>,
    evidence: Arc<MemoryEvidence>,
    server: TestServer,
    _root: TempRoot,
}

impl Fixture {
    /// A front door with in-memory evidence, a world server and a grant
    /// for GET and POST to it.
    fn new() -> Self {
        let root = temp_root("m2c-door");
        let evidence = Arc::new(MemoryEvidence::new(10_000));
        let control = Arc::new(
            GovernedControl::with_secrets(
                root.0.path(),
                Vault::Disabled.source(),
                evidence.clone(),
                Arc::new(SystemClock::default()),
            )
            .unwrap(),
        );
        let server = TestServer::start(|_| Reply::ok("done"));
        control
            .request_grant(
                &GrantRequest::Egress {
                    origin: server.origin(),
                    methods: vec!["GET".into(), "POST".into()],
                    allow_private: true,
                },
                Duration::from_secs(600),
                &Yes::new(true),
            )
            .unwrap();
        Self {
            control,
            evidence,
            server,
            _root: root,
        }
    }

    fn door_with(&self, owner: &Arc<Owner>) -> super::AgentDoor {
        self.control
            .issue_agent_door(mother(), owner.clone())
            .expect("the Owner said yes to the door")
    }

    fn get(&self, path: &str) -> Intent {
        Intent::Request(EgressIntent {
            method: "GET".into(),
            url: format!("{}{path}", self.server.origin()),
            headers: vec![],
            body: None,
        })
    }

    fn post(&self, path: &str) -> Intent {
        Intent::Request(EgressIntent {
            method: "POST".into(),
            url: format!("{}{path}", self.server.origin()),
            headers: vec![],
            body: Some("x".into()),
        })
    }

    /// How many requests for `path` reached the world.
    fn reached(&self, path: &str) -> usize {
        self.server
            .received()
            .iter()
            .filter(|r| r.path == path)
            .count()
    }

    fn records(&self, phase: EvidencePhase) -> Vec<EvidenceRecord> {
        self.evidence
            .records()
            .into_iter()
            .filter(|r| r.phase == phase)
            .collect()
    }

    fn live(&self, run: RunId) -> bool {
        let view = self.control.authority().runs().view(run).unwrap();
        !view.cancelled && !view.finished
    }
}

fn detail<'a>(record: &'a EvidenceRecord, key: &str) -> Option<&'a str> {
    record
        .detail
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn mission_request(label: &str) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: HASH.into(),
        request: label.into(),
        summary: vec!["A one-page site for Rosa's bakery".into()],
    }
}

fn held_request(label: &str) -> MissionResumeConfirmation {
    MissionResumeConfirmation {
        mission: "m-1".into(),
        contract_hash: HASH.into(),
        request: label.into(),
        held_reason: "unclean_shutdown".into(),
        summary: vec!["A one-page site for Rosa's bakery".into()],
    }
}

/// An action of `run` that waits for the Owner (a POST).
fn waiting(f: &Fixture, door: &super::AgentDoor, run: RunId, path: &str) -> CommitmentId {
    match door.agent_action(run, &f.post(path)).unwrap() {
        AgentOutcome::AwaitingApproval(view) => view.id,
        AgentOutcome::Done(_) => panic!("a POST waits for the Owner"),
    }
}

// ---------------------------------------------------------------- A2

/// A2: a `mother` door given another agent's run or commitment, or an owner
/// run, refuses every run call and leaves the run as it was; `commitment`
/// says `NotMine`. `settled` keeps the port's semantics: another agent's
/// action is never settled for this door, final or not.
#[test]
fn m2c_a2_a_door_refuses_another_agent_s_runs_and_commitments() {
    let f = Fixture::new();
    let owner = Owner::yes();
    let door = f.door_with(&owner);
    let other = AgentId::new("another-agent").unwrap();
    let theirs = f
        .control
        .open_run(other.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let AgentOutcome::AwaitingApproval(their_wait) = f
        .control
        .agent_action(&other, theirs, &f.post("/a2-theirs"))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    let (_, owners) = f.control.open_owner_run(vec![]).unwrap();
    for run in [theirs, owners] {
        assert!(door.agent_action(run, &f.get("/a2-refused")).is_err());
        assert!(door.cancel_run(run).is_err());
        assert!(door.finish_when_settled(run).is_err());
        assert!(door.adopt(run, their_wait.id).is_err());
        assert!(f.live(run), "a refused call changed the run");
    }
    assert_eq!(
        door.agent_action(theirs, &f.get("/a2-refused"))
            .unwrap_err(),
        AuthorityError::WrongAgent
    );
    assert_eq!(f.reached("/a2-refused"), 0);
    assert!(matches!(
        door.commitment(their_wait.id),
        Lookup::NotMine { settled: false }
    ));
    f.control.cancel_run(theirs).unwrap();
    assert!(
        matches!(
            door.commitment(their_wait.id),
            Lookup::NotMine { settled: false }
        ),
        "another agent's final action is still not settled for this door"
    );
    let none = CommitmentId::parse("cmt-00000000000000000000000000000000").unwrap();
    assert!(matches!(door.commitment(none), Lookup::Unknown));

    // Positive control: the door's own run works.
    let mine = door.open_run().unwrap();
    assert!(matches!(
        door.agent_action(mine, &f.get("/a2-mine")).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(f.reached("/a2-mine"), 1);
    let wait = waiting(&f, &door, mine, "/a2-wait");
    assert!(matches!(door.commitment(wait), Lookup::Mine(view) if view.id == wait));
    door.cancel_run(mine).unwrap();
    assert!(!f.live(mine));
    assert!(matches!(
        door.commitment(wait),
        Lookup::Mine(view) if view.id == wait && view.run == mine
    ));
    assert_eq!(door.agent(), &mother());
    assert_eq!(f.reached("/a2-wait"), 0);
}

// ---------------------------------------------------------------- A3

/// A3 (Codex R2-1): two doors of the same agent. D2 cannot act on D1's run
/// until it adopts it; after that it can, and the run is still D1's too.
#[test]
fn m2c_a3_a_door_acts_only_on_the_runs_of_its_group() {
    let f = Fixture::new();
    let owner = Owner::yes();
    let d1 = f.door_with(&owner);
    let d2 = f.door_with(&owner);
    let run = d1.open_run().unwrap();
    let wait = waiting(&f, &d1, run, "/a3-wait");

    assert_eq!(
        d2.agent_action(run, &f.get("/a3-d2")).unwrap_err(),
        AuthorityError::NotMember
    );
    assert_eq!(d2.cancel_run(run).unwrap_err(), AuthorityError::NotMember);
    assert_eq!(
        d2.finish_when_settled(run).unwrap_err(),
        AuthorityError::NotMember
    );
    assert_eq!(f.reached("/a3-d2"), 0, "a counting effect: zero effects");
    assert!(f.live(run), "the run is unchanged");
    assert!(matches!(
        d2.commitment(wait),
        Lookup::NotMine { settled: false }
    ));

    // Positive controls: D2 on its own run, then on D1's run once adopted.
    let own = d2.open_run().unwrap();
    assert!(matches!(
        d2.agent_action(own, &f.get("/a3-own")).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(f.reached("/a3-own"), 1);
    let view = d2.adopt(run, wait).unwrap();
    assert_eq!((view.id, view.run), (wait, run));
    assert!(matches!(
        d2.agent_action(run, &f.get("/a3-adopted")).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(f.reached("/a3-adopted"), 1);
    assert!(matches!(d2.commitment(wait), Lookup::Mine(_)));

    // Membership only grows: the run is still in D1's group.
    assert!(matches!(
        d1.agent_action(run, &f.get("/a3-d1")).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(f.reached("/a3-d1"), 1);
    assert!(matches!(d1.commitment(wait), Lookup::Mine(_)));

    // A same-agent action of another group: settled only once final.
    let d3 = f.door_with(&owner);
    assert!(matches!(
        d3.commitment(wait),
        Lookup::NotMine { settled: false }
    ));
    d1.cancel_run(run).unwrap();
    assert!(matches!(
        d3.commitment(wait),
        Lookup::NotMine { settled: true }
    ));
    assert_eq!(f.reached("/a3-wait"), 0);
}

// ---------------------------------------------------------------- A4

/// A4, at the broker: the door asks its bound confirmer once, with the
/// exact request. No: declined and recorded. Yes: an approval naming the
/// hash and the request, recorded. (A halted door refusing before the
/// dialog is PR-B's.)
#[test]
fn m2c_a4_consent_through_the_door_asks_the_bound_confirmer() {
    let f = Fixture::new();
    let declines = Owner::new(true, false, false);
    let door = f.door_with(&declines);
    let request = mission_request("m-1/7/1");
    assert_eq!(
        door.confirm_mission(&request).unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(declines.missions(), vec![request.clone()]);
    let declined = f.records(EvidencePhase::MissionDeclined);
    assert_eq!(declined.len(), 1);
    assert_eq!(detail(&declined[0], "contract_hash"), Some(HASH));
    assert_eq!(detail(&declined[0], "request"), Some("m-1/7/1"));

    let owner = Owner::yes();
    let door = f.door_with(&owner);
    let request = mission_request("m-1/8/2");
    let approval = door.confirm_mission(&request).unwrap();
    assert_eq!(owner.missions(), vec![request]);
    assert_eq!(
        (approval.contract_hash(), approval.request()),
        (HASH, "m-1/8/2")
    );
    let consented = f.records(EvidencePhase::MissionConsented);
    assert_eq!(consented.len(), 1);
    assert_eq!(detail(&consented[0], "request"), Some("m-1/8/2"));
}

// ---------------------------------------------------------------- A5/A6

/// A5 and A6 at the broker: the held-mission dialog asks the bound
/// confirmer once, with the exact request. No: declined, recorded, no
/// approval. Yes: the recorded approval names the hash and the request.
#[test]
fn m2c_a5_resume_through_the_door_asks_the_bound_confirmer() {
    let f = Fixture::new();
    let declines = Owner::new(true, false, false);
    let door = f.door_with(&declines);
    let request = held_request("m-1/9/1");
    assert_eq!(
        door.confirm_held_mission(&request).unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(declines.helds(), vec![request.clone()]);
    let declined = f.records(EvidencePhase::MissionResumeDeclined);
    assert_eq!(declined.len(), 1);
    assert_eq!(detail(&declined[0], "contract_hash"), Some(HASH));
    assert_eq!(detail(&declined[0], "request"), Some("m-1/9/1"));
    assert!(f.records(EvidencePhase::MissionResumeConfirmed).is_empty());

    let owner = Owner::yes();
    let door = f.door_with(&owner);
    let request = held_request("m-1/10/2");
    let approval = door.confirm_held_mission(&request).unwrap();
    assert_eq!(owner.helds(), vec![request]);
    assert_eq!(
        (approval.contract_hash(), approval.request()),
        (HASH, "m-1/10/2")
    );
    let confirmed = f.records(EvidencePhase::MissionResumeConfirmed);
    assert_eq!(confirmed.len(), 1);
    assert_eq!(detail(&confirmed[0], "request"), Some("m-1/10/2"));
    assert!(approval.evidence_seq() > 0);

    // A confirmer without the held-mission dialog says no: the trait's
    // default answer, and nothing is approved.
    let door = f
        .control
        .issue_agent_door(mother(), Arc::new(DoorOnly))
        .unwrap();
    assert_eq!(
        door.confirm_held_mission(&held_request("m-1/11/3"))
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(f.records(EvidencePhase::MissionResumeConfirmed).len(), 1);
}

/// A6-f at the broker: under an emergency stop the held-mission dialog is
/// refused before it is raised, and nothing is recorded; a stop pressed
/// while the Owner is asked wins over the yes (no record of a yes).
#[test]
fn m2c_a6_f_resume_under_an_emergency_stop_is_refused_before_the_dialog() {
    let f = Fixture::new();
    let owner = Owner::yes();
    let door = f.door_with(&owner);
    let bad = MissionResumeConfirmation {
        contract_hash: "not-a-hash".into(),
        ..held_request("m-1/1/1")
    };
    assert!(matches!(
        door.confirm_held_mission(&bad),
        Err(AuthorityError::InvalidAction(_))
    ));
    assert!(owner.helds().is_empty(), "refused before the dialog");

    let control = f.control.clone();
    owner.during(move || {
        control.emergency_stop();
    });
    assert_eq!(
        door.confirm_held_mission(&held_request("m-1/2/2"))
            .unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert_eq!(owner.helds().len(), 1);
    assert!(f.records(EvidencePhase::MissionResumeConfirmed).is_empty());

    assert_eq!(
        door.confirm_held_mission(&held_request("m-1/3/3"))
            .unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert_eq!(owner.helds().len(), 1, "refused before the dialog");
    assert!(door.emergency_stopped());
    assert_eq!(
        door.confirm_mission(&mission_request("m-1/4/4"))
            .unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert!(owner.missions().is_empty());
}

// ---------------------------------------------------------------- A7

/// A7-a: no → no door and `agent_door_declined`; yes → `agent_door_opened`,
/// then a working door; a confirmer with the trait's default → no.
#[test]
fn m2c_a7_a_every_door_issue_asks_the_owner() {
    let f = Fixture::new();
    let no = Owner::new(false, true, true);
    assert_eq!(
        f.control
            .issue_agent_door(mother(), no.clone())
            .unwrap_err(),
        AuthorityError::Declined
    );
    let asked = no.doors();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].agent, "mother");
    let declined = f.records(EvidencePhase::AgentDoorDeclined);
    assert_eq!(declined.len(), 1);
    assert_eq!(detail(&declined[0], "door"), Some(asked[0].door.as_str()));
    assert!(f.records(EvidencePhase::AgentDoorOpened).is_empty());

    assert_eq!(
        f.control
            .issue_agent_door(mother(), Arc::new(Legacy))
            .unwrap_err(),
        AuthorityError::Declined,
        "the trait's default answer is no"
    );

    let yes = Owner::yes();
    let door = f.door_with(&yes);
    let asked = yes.doors();
    assert_eq!(asked.len(), 1);
    let opened = f.records(EvidencePhase::AgentDoorOpened);
    assert_eq!(opened.len(), 1);
    assert_eq!(detail(&opened[0], "door"), Some(asked[0].door.as_str()));
    assert_eq!(opened[0].agent.as_deref(), Some("mother"));
    let run = door.open_run().unwrap();
    assert!(matches!(
        door.agent_action(run, &f.get("/a7-a")).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(f.reached("/a7-a"), 1);
    // Door ids are opaque and never reused.
    let again = f.door_with(&yes);
    let ids: Vec<String> = yes.doors().into_iter().map(|d| d.door).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(ids[0].len(), 32);
    assert!(ids[0].bytes().all(|b| b.is_ascii_hexdigit()));
    drop(again);
}

/// A primary evidence store and a front door over it.
fn store_fixture() -> (Arc<GovernedControl>, Arc<PrimaryEvidence>, Scratch) {
    let scratch = Scratch::new();
    let evidence = PrimaryEvidence::open(&scratch.store(), vec![]).unwrap();
    let control = Arc::new(
        GovernedControl::with_secrets(
            &scratch.path("runtime"),
            Vault::Disabled.source(),
            evidence.clone(),
            Arc::new(SystemClock::default()),
        )
        .unwrap(),
    );
    (control, evidence, scratch)
}

/// A7-b: the gating record fails → no door; the next issue works. A failed
/// COMMIT latches the store: then every issue is refused before the dialog.
#[test]
fn m2c_a7_b_an_unrecorded_door_is_no_door() {
    let (control, evidence, _scratch) = store_fixture();
    let owner = Owner::yes();
    evidence.store().refuse_next_insert();
    assert_eq!(
        control
            .issue_agent_door(mother(), owner.clone())
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(owner.doors().len(), 1);
    let door = control
        .issue_agent_door(mother(), owner.clone())
        .expect("positive control: the next issue works");
    assert!(door.open_run().is_ok());
    assert_eq!(owner.doors().len(), 2);

    evidence.store().plant_fault(Fault::FailCommit);
    assert_eq!(
        control
            .issue_agent_door(mother(), owner.clone())
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(owner.doors().len(), 3);
    assert!(evidence.health().latched);
    assert_eq!(
        control
            .issue_agent_door(mother(), owner.clone())
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        owner.doors().len(),
        3,
        "a latched store: refused before the dialog"
    );
}

/// A7-c: two concurrent issuers are each asked once, and neither waits for
/// the other's dialog (each dialog waits until both are open). The halt of
/// another door during an issue is PR-B's (no halt exists in PR-A).
#[test]
fn m2c_a7_c_concurrent_issuers_are_each_asked_once() {
    let f = Fixture::new();
    let owner = Owner::meeting(2);
    let (done, results) = channel();
    let mut threads = Vec::new();
    for _ in 0..2 {
        let (control, owner, done) = (f.control.clone(), owner.clone(), done.clone());
        threads.push(std::thread::spawn(move || {
            let issued = control.issue_agent_door(mother(), owner);
            let _ = done.send(issued.map(|door| door.agent().clone()));
        }));
    }
    for _ in 0..2 {
        let issued = results.recv_timeout(JOIN).expect("an issuer deadlocked");
        assert_eq!(issued, Ok(mother()));
    }
    for thread in threads {
        thread.join().unwrap();
    }
    let asked = owner.doors();
    assert_eq!(asked.len(), 2);
    assert_ne!(asked[0].door, asked[1].door);
    assert_eq!(f.records(EvidencePhase::AgentDoorOpened).len(), 2);
    f.control.probe_locks();
}

/// A7-d (N5): live limits with reclamation. 64 live doors; the 65th is
/// refused before any dialog or record; a dropped door's place is reused;
/// 1,000 issue-and-drop cycles never hit the limit. Eight live memberships
/// of one run; a ninth adoption is refused and changes nothing; dropping
/// one of those doors makes room.
#[test]
fn m2c_a7_d_live_door_and_membership_limits_reclaim_dropped_doors() {
    let f = Fixture::new();
    let owner = Owner::yes();
    let mut doors: Vec<_> = (0..64).map(|_| f.door_with(&owner)).collect();
    assert_eq!(f.control.status().live_doors, 64);
    let asked = owner.doors().len();
    let opened = f.records(EvidencePhase::AgentDoorOpened).len();
    assert_eq!(
        f.control
            .issue_agent_door(mother(), owner.clone())
            .unwrap_err(),
        AuthorityError::Capacity
    );
    assert_eq!(owner.doors().len(), asked, "refused before the dialog");
    assert_eq!(f.records(EvidencePhase::AgentDoorOpened).len(), opened);
    assert!(f.records(EvidencePhase::AgentDoorDeclined).is_empty());
    doors.pop();
    doors.push(f.door_with(&owner));
    doors.clear();
    for cycle in 0..1_000 {
        let door = f
            .control
            .issue_agent_door(mother(), owner.clone())
            .unwrap_or_else(|e| panic!("cycle {cycle}: {e:?}"));
        drop(door);
    }
    assert!(f.control.status().doors_issued >= 1_065);

    // Eight live memberships of one run, then a ninth.
    let first = f.door_with(&owner);
    let run = first.open_run().unwrap();
    let wait = waiting(&f, &first, run, "/a7-d-wait");
    let mut members = vec![first];
    for _ in 0..7 {
        let door = f.door_with(&owner);
        door.adopt(run, wait).unwrap();
        members.push(door);
    }
    let ninth = f.door_with(&owner);
    assert_eq!(
        ninth.adopt(run, wait).unwrap_err(),
        AuthorityError::Capacity
    );
    assert_eq!(
        ninth.agent_action(run, &f.get("/a7-d-ninth")).unwrap_err(),
        AuthorityError::NotMember,
        "a refused adoption changed nothing"
    );
    assert!(f.live(run));
    members.remove(3);
    let view = ninth.adopt(run, wait).unwrap();
    assert_eq!(view.id, wait);
    assert!(matches!(
        ninth.agent_action(run, &f.get("/a7-d-ninth")).unwrap(),
        AgentOutcome::Done(_)
    ));
    assert_eq!(f.reached("/a7-d-ninth"), 1);
    f.control.probe_locks();
}

/// A7-e: with no halt, no crash, in the same process, every issue asks: a
/// second door for the same agent is asked for again, and recorded again.
#[test]
fn m2c_a7_e_there_is_no_path_that_issues_without_asking() {
    let f = Fixture::new();
    let owner = Owner::yes();
    let first = f.door_with(&owner);
    let second = f.door_with(&owner);
    drop(first);
    let third = f.door_with(&owner);
    assert_eq!(owner.doors().len(), 3);
    assert_eq!(f.records(EvidencePhase::AgentDoorOpened).len(), 3);
    assert_eq!(f.control.status().live_doors, 2);
    assert_eq!(f.control.status().doors_issued, 3);
    drop((second, third));

    // Under an emergency stop, refused before the dialog.
    f.control.emergency_stop();
    assert_eq!(
        f.control
            .issue_agent_door(mother(), owner.clone())
            .unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert_eq!(owner.doors().len(), 3);

    // A stop pressed while the Owner is asked wins over the yes.
    let f = Fixture::new();
    let owner = Owner::yes();
    let control = f.control.clone();
    owner.during(move || {
        control.emergency_stop();
    });
    assert_eq!(
        f.control
            .issue_agent_door(mother(), owner.clone())
            .unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert_eq!(owner.doors().len(), 1);
    assert!(f.records(EvidencePhase::AgentDoorOpened).is_empty());
    assert_eq!(f.control.status().live_doors, 0);
}
