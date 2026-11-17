//! U4a-2 S5 (design 4.4; AM-2 Part A, revision 4.2). Every cell with an S5
//! witness runs in this module (A.2 item 1). Its only model call is in
//! `drive_call` (A.2 item 2); the `SessionPort` is moved into
//! `Coordinator::open` and never cloned (A.2 item 3 (c)); every scripted
//! lane is a driver-owned `RecordingLane` (A.4 item 3). `credit` is the A.2
//! item 4/5, A.3 and A.4 check over the driver's own facts and both traces.
// SB3E-R42.4: the parent's items by name, the guard's allow-list; no glob.
use super::{child, fake_ollama, spec, Fixture, Opened, CHILD, DIGEST, NAME, PATIENCE};
use nexus_governed_control::authority::approval::ResumeConfirmation;
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation,
};
use nexus_governed_control::authority::approval::{AgentDoorConfirmation, ControlConfirmer};
use nexus_governed_control::authority::approval::{GrantConfirmation, MissionConfirmation};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::{
    EvidenceProjection, EvidenceRecord, MemoryEvidence,
};
use nexus_governed_control::authority::file_session::HaltCell;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::broker::route_seams as s1;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::host_session::HostSession;
use nexus_governed_control::host_session::{CommandKind, SessionDoor, SessionSeed};
use nexus_governed_control::host_session::{SessionContext, SessionIdentity};
use nexus_governed_control::inference::{ModelIdentity, ModelRequest, ResponseSchema};
use nexus_mother::adapter::{FileJournal, FilePrivate, SessionPort};
use nexus_mother::contract::{Budget, MissionContract, ModelChoice, PassCriterion};
use nexus_mother::coordinator::{Coordinator, FileSession, Lanes};
use nexus_mother::journal::{Entry as Journaled, MissionId};
use nexus_mother::lane::{InvocationId, Lane, LaneContext, LaneIdentity, LaneInput, LaneOutput};
use nexus_mother::lane::{LaneRole, ScriptedLane, Verdict};
use nexus_mother::route_seams as s5;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

pub(super) type Recorded = Arc<Mutex<Vec<(LaneRole, InvocationId)>>>;

/// A.2 item 2: the only model-call site in this module. The driver's
/// per-session count rises before the call, so a refused or panicking call
/// is counted too.
fn drive_call(calls: &AtomicU32, context: &LaneContext<'_>) -> bool {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    let request = ModelRequest::new("system".into(), "prompt".into(), schema).unwrap();
    calls.fetch_add(1, Ordering::SeqCst);
    context
        .model
        .expect("a model caller")
        .call(&request)
        .is_ok()
}

/// The Builder: one model call, through `drive_call`. It records its own
/// invocation (A.4 item 3).
struct ModelLane(Arc<AtomicU32>, Recorded);
impl Lane for ModelLane {
    fn max_model_calls(&self) -> u32 {
        1
    }
    fn identity(&self) -> LaneIdentity {
        let (name, version) = ("route-model".into(), "1".into());
        let role = LaneRole::Builder;
        LaneIdentity {
            role,
            name,
            version,
        }
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        panic!("invoked with its context")
    }
    fn invoke_in(&self, input: &LaneInput, context: &LaneContext<'_>) -> LaneOutput {
        self.1.lock().unwrap().push((input.role, input.invocation));
        let text = if drive_call(&self.0, context) {
            "built"
        } else {
            "refused"
        };
        ScriptedLane::new(LaneRole::Builder, text, Verdict::Continue).output
    }
}
/// A driver-owned wrapper (A.4 item 3): records `(role, invocation)`, then
/// delegates.
struct RecordingLane(ScriptedLane, Recorded);
impl Lane for RecordingLane {
    fn identity(&self) -> LaneIdentity {
        self.0.identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        self.1.lock().unwrap().push((input.role, input.invocation));
        self.0.invoke(input)
    }
}
/// The six lanes: the model Builder and five recorded scripted lanes.
pub(super) fn lanes(calls: &Arc<AtomicU32>, recorded: &Recorded) -> Lanes {
    let lane = |role| -> Arc<dyn Lane> {
        if role == LaneRole::Builder {
            return Arc::new(ModelLane(calls.clone(), recorded.clone()));
        }
        let pass = role == LaneRole::Verifier;
        let verdict = if pass {
            Verdict::Pass
        } else {
            Verdict::Continue
        };
        let scripted = ScriptedLane::new(role, "scripted", verdict);
        Arc::new(RecordingLane(scripted, recorded.clone()))
    };
    Lanes::new(LaneRole::ALL.into_iter().map(lane).collect()).unwrap()
}

/// The public bootstrap to one original door (Step 0 item 3): the model
/// registered on the fake peer, the workspace, a Run request and its door.
pub(super) struct Boot {
    pub(super) door: SessionDoor,
    pub(super) context: SessionContext,
    pub(super) name: String,
    pub(super) request: u64,
    /// The fake peer's ordered log (Behaviour 5).
    pub(super) peer: Arc<Mutex<Vec<String>>>,
}
pub(super) fn boot(f: &Fixture, o: &Opened) -> Boot {
    let (endpoint, peer) = fake_ollama();
    o.session.register_local_model(&endpoint, spec()).unwrap();
    let workspace = f.0.join("workspace");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&workspace)
        .unwrap();
    let name = o.session.bootstrap_workspace(&workspace, &[]).unwrap().name;
    o.session.finish_bootstrap().unwrap();
    let request = o.ingress.accept(CommandKind::Run, 1).unwrap();
    let door = o.session.open_file_door(&request).unwrap();
    let (context, request) = (door.context(), request.sequence());
    Boot {
        door,
        context,
        name,
        request,
        peer,
    }
}
/// A consented mission with the pinned model, ready to advance.
pub(super) fn consented(m: &FileSession<SessionPort>, folder: &str) -> MissionId {
    let digest = ModelIdentity::from_spec(&spec()).unwrap().digest();
    let identity = digest.iter().map(|b| format!("{b:02x}")).collect();
    let (name, digest) = (NAME.into(), DIGEST.into());
    let contract = MissionContract {
        model: Some(ModelChoice {
            name,
            digest,
            identity,
        }),
        notes: vec![],
        deliverable: Some("index.html".into()),
        page_facts: None,
        owner_words: "Make a page.".into(),
        restatement: "A page.".into(),
        requested_folder: Some(folder.into()),
        budget: Budget {
            max_duration_ms: 60_000,
            max_model_calls: 20,
            max_money_minor: 0,
        },
        forbidden_effects: Default::default(),
        questions: vec![],
        pass_criteria: vec![PassCriterion {
            id: "p1".into(),
            text: "A page.".into(),
        }],
    };
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    m.request_consent(id).unwrap();
    id
}

/// The S5 fixture: the port is moved into `Coordinator::open`.
pub(super) struct Mother {
    pub(super) m: FileSession<SessionPort>,
    id: MissionId,
    calls: Arc<AtomicU32>,
    recorded: Recorded,
    request: u64,
    /// The request context (not the port or door): S4b's halt is read here.
    context: SessionContext,
    peer: Arc<Mutex<Vec<String>>>,
    /// Behaviour 5: the door's `lifecycle_halt` view, taken before
    /// `SessionPort::new` (for `original_cell_unscheduled`).
    halt: HaltCell,
}
fn mother(f: &Fixture, o: &Opened) -> Mother {
    let b = boot(f, o);
    let halt = b.door.lifecycle_halt();
    let (calls, recorded) = (Arc::default(), Recorded::default());
    // U9a M8 (SB3E-R123): the journal under the history root, as production
    // (session_home.rs:85), so the seal's history check reads it (close.rs:456-462).
    let journal = FileJournal::open(&f.0.join("home/mother-journal.jsonl")).unwrap();
    let private = FilePrivate::open_session(&f.0.join("private"), b.context.clone());
    let private = private.unwrap();
    let clock = Arc::new(SystemClock::default());
    let m = Coordinator::open(
        journal,
        Arc::new(SessionPort::new(b.door, b.name.clone())),
        lanes(&calls, &recorded),
        clock,
        private,
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let id = consented(&m, &b.name);
    Mother {
        m,
        id,
        calls,
        recorded,
        request: b.request,
        context: b.context,
        peer: b.peer,
        halt,
    }
}
impl Mother {
    /// Advances to Done; each step must be accepted.
    fn run(&self) {
        for _ in 0..8 {
            self.m.advance(self.id).unwrap();
            if self.m.state(self.id).unwrap() == Some(nexus_mother::state::MissionState::Done) {
                return;
            }
        }
        panic!("not Done: {:?}", self.m.state(self.id));
    }
    fn driver(&self, o: &Opened) -> Driver {
        Driver {
            session: o.session.identity(),
            request: self.request,
            calls: self.calls.load(Ordering::SeqCst),
            recorded: self.recorded.lock().unwrap().clone(),
        }
    }
}

/// Driver-owned facts (A.2 item 2, A.4): the session it opened, its request,
/// its own call count, and the invocations its lanes recorded.
#[derive(Clone)]
struct Driver {
    session: SessionIdentity,
    request: u64,
    calls: u32,
    recorded: Vec<(LaneRole, InvocationId)>,
}
/// A path profile (A.2 item 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Profile {
    pub(super) opening: usize,
    pub(super) recheck: usize,
    pub(super) classify: usize,
    /// The claimed attempt's `call.gate` entries.
    pub(super) call_gate: usize,
    /// Whether a broker session-binding entry exists.
    pub(super) bound: bool,
}
pub(super) type Neighbour = (&'static str, Option<u64>);
/// An expect row's S5 fields.
#[derive(Clone, Copy)]
pub(super) struct Expect {
    pub(super) attempts: usize,
    pub(super) profile: Profile,
    /// Each session-only site's count per recorded invocation (A.4 item 3),
    /// selected by the invocation's role through the row's closed lookup.
    pub(super) roles: Roles,
    /// Whether the cell credits a request-level property (A.4 item 5).
    pub(super) request_level: bool,
    /// Whether it credits an attempt-level property (never on `bound: no`).
    pub(super) attempt_level: bool,
    /// The multiplicity of each attempt site the profile does not count.
    pub(super) multiplicity: [(&'static str, usize); 4],
    /// An S5 site and its S1 neighbours by instant, as (point, ordinal).
    pub(super) position: Option<(&'static str, Neighbour, Neighbour)>,
}
/// The recorded path's measured row (A.6).
pub(super) const RECORDED: Expect = Expect {
    attempts: 1,
    profile: Profile {
        opening: 1,
        recheck: 1,
        classify: 1,
        call_gate: 4,
        bound: true,
    },
    roles: done_roles,
    request_level: true,
    attempt_level: true,
    multiplicity: [
        ("mother.charge.entry", 1),
        ("mother.charge", 1),
        ("mother.charge.ok", 1),
        ("mother.settle", 1),
    ],
    position: Some((
        "mother.charge.entry",
        ("model.call.admitted", Some(3)),
        ("call.gate", Some(4)),
    )),
};
/// SB3E-R49.3 (mission Behaviour 8): a row's closed role lookup. A role
/// only selects its invocations' expected counts, never a total: `None` is
/// no entry (a recorded invocation of that role is `role_unexpected`), and
/// an entry with no recorded invocation is `role_missing`.
pub(super) type Roles = fn(LaneRole) -> Option<[(&'static str, usize); 4]>;
/// The four session-only sites (A.3), each `n` times per invocation.
pub(super) fn sessions(n: usize) -> Option<[(&'static str, usize); 4]> {
    let sites = [
        "mother.lane.prelock",
        "mother.admit",
        "mother.admit.ok",
        "mother.lane.returned",
    ];
    Some(sites.map(|site| (site, n)))
}
/// `RECORDED`'s path to Done: every role, each invocation once.
fn done_roles(role: LaneRole) -> Option<[(&'static str, usize); 4]> {
    match role {
        LaneRole::ResearcherA | LaneRole::ResearcherB | LaneRole::Skeptic => sessions(1),
        LaneRole::Builder | LaneRole::Verifier | LaneRole::Sentinel => sessions(1),
    }
}
/// `HELD`'s path, to the Builder's returned call: no Verifier yet.
fn held_roles(role: LaneRole) -> Option<[(&'static str, usize); 4]> {
    match role {
        LaneRole::ResearcherA | LaneRole::ResearcherB | LaneRole::Skeptic => sessions(1),
        LaneRole::Builder | LaneRole::Sentinel => sessions(1),
        LaneRole::Verifier => None,
    }
}
/// `REFUSED`'s path: the Builder's call refused, its lane never returns, so
/// none of its session-only sites fires (its attempt sites do); no Verifier.
fn refused_roles(role: LaneRole) -> Option<[(&'static str, usize); 4]> {
    match role {
        LaneRole::ResearcherA | LaneRole::ResearcherB | LaneRole::Skeptic => sessions(1),
        LaneRole::Sentinel => sessions(1),
        LaneRole::Builder => sessions(0),
        LaneRole::Verifier => None,
    }
}
/// Behaviour 10's control path (A.6, measured at U5 Step 0 item 5): held at
/// ord 4 and released with no value, the recorded path's profile up to the
/// Builder's returned call.
pub(super) const HELD: Expect = Expect {
    roles: held_roles,
    ..RECORDED
};
/// Behaviour 8 (A.6, measured on each of the six value paths alike): opening
/// 1, recheck 1, classify 1, `call.gate` 4, bound; each attempt site once;
/// `mother.charge.entry` between ord 3's `model.call.admitted` and ord 4's
/// `call.gate`; Sentinel (twice), ResearcherA, ResearcherB and Skeptic 1/1/1/1
/// per invocation, the Builder 0/0/0/0.
pub(super) const REFUSED: Expect = Expect {
    roles: refused_roles,
    ..RECORDED
};
/// Site domains (AM-2 rev 3 A.3, unchanged).
const ATTEMPT: [&str; 6] = [
    "mother.charge.entry",
    "mother.charge",
    "mother.charge.ok",
    "mother.settle",
    "mother.recheck",
    "mother.classify",
];
/// The broker's session-carrying entries that bind a run (A.3): `CallGate`
/// (with its refusal notes), `attempt.slot`, `model.attempt.reserved` and
/// `Attempt::route_ctx`.
const BINDING: [&str; 8] = [
    "attempt.slot",
    "model.attempt.reserved",
    "model.attempt.record.before",
    "complete",
    "call.gate",
    "model.call.sampled",
    "model.call.observed",
    "model.call.admitted",
];
fn binding(b: &s1::Entry) -> bool {
    BINDING.contains(&b.point) || b.point.starts_with("refused:")
}
/// One S5 entry as the witness records it (A.3): its site, invocation,
/// computed claim, and `request_compared` (`None`: not applicable).
type Site = (
    &'static str,
    Option<InvocationId>,
    Option<(RunId, u32)>,
    Option<bool>,
);
#[derive(Debug)]
struct Witness {
    run: Option<RunId>,
    claim: Option<(RunId, u32)>,
    profile: Profile,
    /// The position site's S1 neighbours, as measured.
    neighbours: Option<(Neighbour, Neighbour)>,
    sites: Vec<Site>,
}

/// A.2 item 4/5, A.3 and A.4: credit, or the first refusing check's own
/// reason code.
fn credit(
    d: &Driver,
    e: &Expect,
    broker: &[s1::Entry],
    mother: &[s5::Entry],
) -> Result<Witness, &'static str> {
    // The expect row itself (SB3E-R40.2).
    let p = e.profile;
    if [p.opening, p.recheck, p.classify].iter().any(|n| *n > 1) {
        return Err("row_counts");
    }
    if !p.bound && e.attempt_level {
        return Err("attempt_level");
    }
    if p.call_gate == 0 && e.request_level {
        return Err("request_level");
    }
    if d.calls != 1 {
        return Err("driver_calls");
    }
    if mother.iter().any(|m| m.session != Some(d.session)) {
        return Err("session");
    }
    if mother.iter().any(|m| m.request != Some(d.request)) {
        return Err("request");
    }
    // SB3E-R40.1: every site but `mother.classify` carries an invocation.
    let bare = |m: &s5::Entry| m.point != "mother.classify" && m.invocation.is_none();
    if mother.iter().any(bare) {
        return Err("missing_invocation");
    }
    let both = || {
        broker
            .iter()
            .filter(|b| b.session.is_some() && b.run.is_some())
    };
    if both().any(|x| both().any(|y| x.run == y.run && x.session != y.session)) {
        return Err("run_in_two_sessions");
    }
    // A.3: the run bound to the driver's session by broker-owned entries.
    let mine = |b: &&s1::Entry| b.session == Some(d.session);
    let runs = broker.iter().filter(mine).filter(|b| binding(b));
    let runs: Option<Vec<RunId>> = runs.map(|b| b.run).collect();
    let mut runs = runs.ok_or("binding_without_run")?;
    runs.sort_unstable();
    runs.dedup();
    if runs.len() > 1 {
        return Err("two_bound_runs");
    }
    let run = runs.first().copied();
    if !e.profile.bound && run.is_some() {
        return Err("binding_on_unbound");
    }
    // A.2 item 4: openings, each instance counted, never deduplicated.
    let openings: Vec<&s1::Entry> = broker
        .iter()
        .filter(|b| b.point == "model.attempt.granted" && !b.note)
        .collect();
    if !e.profile.bound && !openings.is_empty() {
        return Err("opening_on_unbound");
    }
    if openings.iter().any(|o| o.run != run) {
        return Err("opening_run");
    }
    if openings.len() > 1 {
        return Err("second_opening");
    }
    if openings.len() != e.attempts {
        return Err("attempts");
    }
    let claim = match openings.first() {
        Some(o) => Some(o.run.zip(o.attempt).ok_or("opening_fields")?),
        None => None,
    };
    // SB3E-R42.8: an attempt-level credit needs a claim, whatever `bound` is.
    if e.attempt_level && claim.is_none() {
        return Err("attempt_level");
    }
    // A.2 item 5: the path profile, exactly.
    let gates: Vec<&s1::Entry> = broker
        .iter()
        .filter(mine)
        .filter(|b| b.point == "call.gate")
        .collect();
    if gates.iter().any(|g| g.run.zip(g.attempt) != claim) {
        return Err("gate_attempt");
    }
    let count = |p: &str| mother.iter().filter(|m| m.point == p).count();
    let profile = Profile {
        opening: openings.len(),
        recheck: count("mother.recheck"),
        classify: count("mother.classify"),
        call_gate: gates.len(),
        bound: run.is_some(),
    };
    if profile != e.profile {
        return Err("profile");
    }
    if e.multiplicity.iter().any(|(site, n)| count(site) != *n) {
        return Err("multiplicity");
    }
    // A.4 item 3: invocations, per invocation.
    let recorded = |i: &InvocationId| d.recorded.iter().any(|r| r.1 == *i);
    if mother
        .iter()
        .filter_map(|m| m.invocation)
        .any(|i| !recorded(&i))
    {
        return Err("unrecorded_invocation");
    }
    let builder = d.recorded.iter().filter(|r| r.0 == LaneRole::Builder);
    let builder: Vec<InvocationId> = builder.map(|r| r.1).collect();
    let attempting = mother.iter().filter(|m| ATTEMPT.contains(&m.point));
    if attempting
        .filter_map(|m| m.invocation)
        .any(|i| builder != [i])
    {
        return Err("not_model_invocation");
    }
    // SB3E-R49.3: the role only selects each invocation's own counts.
    if d.recorded.iter().any(|r| (e.roles)(r.0).is_none()) {
        return Err("role_unexpected");
    }
    let mut entries = LaneRole::ALL
        .into_iter()
        .filter(|r| (e.roles)(*r).is_some());
    if entries.any(|role| d.recorded.iter().all(|r| r.0 != role)) {
        return Err("role_missing");
    }
    for (role, invocation) in &d.recorded {
        for (site, n) in (e.roles)(*role).into_iter().flatten() {
            let at = |m: &&s5::Entry| m.point == site && m.invocation == Some(*invocation);
            if mother.iter().filter(at).count() != n {
                return Err("per_invocation");
            }
        }
    }
    // A.3: classify bears no invocation; it is correlated by run alone.
    let classify = || mother.iter().filter(|m| m.point == "mother.classify");
    if classify().any(|m| m.run != run) {
        return Err("classify_run");
    }
    if classify().any(|m| m.invocation.is_some()) {
        return Err("classify_invocation");
    }
    // A.3 claims; A.4 item 5: request_compared where `call.gate` is positive.
    let mut sites = Vec::new();
    for m in mother {
        let claims = match m.point {
            "mother.classify" => claim.filter(|c| Some(c.0) == m.run),
            p if ATTEMPT.contains(&p) => claim,
            _ => None,
        };
        let compared = match claims {
            Some(c) if e.profile.call_gate > 0 => {
                let same = |g: &&&s1::Entry| g.run.zip(g.attempt) == Some(c);
                if !gates.iter().filter(same).any(|g| g.request == m.request) {
                    return Err("request_compared");
                }
                Some(true)
            }
            _ => None,
        };
        sites.push((m.point, m.invocation, claims, compared));
    }
    // SB3E-R40.2: a request-level credit needs a compared attempt entry.
    if e.request_level && !sites.iter().any(|s| s.3 == Some(true)) {
        return Err("no_compared");
    }
    // A.4 item 6: position, evidence only, through the run-to-session map.
    let mut neighbours = None;
    if let Some((site, before, after)) = e.position {
        let mut named = mother.iter().filter(|m| m.point == site);
        let at = named.next().ok_or("position_site")?;
        if named.next().is_some() {
            return Err("position_repeated");
        }
        let ours =
            |b: &&s1::Entry| mine(b) || (b.session.is_none() && b.run.is_some() && b.run == run);
        let ours: Vec<&s1::Entry> = broker.iter().filter(ours).collect();
        if ours.iter().any(|b| b.instant == at.instant) {
            return Err("tie");
        }
        let prev = ours
            .iter()
            .filter(|b| b.instant < at.instant)
            .max_by_key(|b| b.instant);
        let next = ours
            .iter()
            .filter(|b| b.instant > at.instant)
            .min_by_key(|b| b.instant);
        let key = |b: Option<&&s1::Entry>| b.map(|b| (b.point, b.request_ordinal));
        if key(prev) != Some(before) || key(next) != Some(after) {
            return Err("neighbour");
        }
        neighbours = Some((before, after));
    }
    Ok(Witness {
        run,
        claim,
        profile,
        neighbours,
        sites,
    })
}

/// The recorded path: the driver's facts and both traces, from Done.
fn recorded(f: &Fixture) -> (Driver, Vec<s1::Entry>, Vec<s5::Entry>) {
    let o = f.open();
    let mother = mother(f, &o);
    let _s1 = s1::arm("reg.reserve", s1::Action::Trace);
    let _s5 = s5::arm("mother.charge", s5::Action::Trace);
    s1::drain_trace();
    s5::drain_trace();
    mother.run();
    let traces = (s1::drain_trace(), s5::drain_trace());
    (mother.driver(&o), traces.0, traces.1)
}

/// A.6: the recorded path measured on this fixture. `driver_calls` is 1,
/// the profile is 1/1/1, `call.gate` 4, bound yes, and every S5 site is
/// present with its domain, claim and invocation; any difference fails.
#[test]
fn route_s5_recorded_path_is_measured_and_credited() {
    child("s5::route_s5_recorded_path_is_measured_and_credited", |f| {
        let (driver, broker, mother) = recorded(f);
        let witness = credit(&driver, &RECORDED, &broker, &mother).unwrap();
        let run = witness.run.expect("a session-bound run");
        assert_eq!(witness.claim, Some((run, 1)), "{witness:?}");
        let builder = driver.recorded.iter().find(|r| r.0 == LaneRole::Builder);
        let builder = builder.map(|r| r.1);
        let mut points: Vec<&str> = witness.sites.iter().map(|s| s.0).collect();
        points.sort_unstable();
        points.dedup();
        assert_eq!(points.len(), 10, "every S5 site: {points:?}");
        for (site, invocation, claim, compared) in &witness.sites {
            let attempt = ATTEMPT.contains(site);
            let expected = match *site {
                "mother.classify" => (None, Some((run, 1)), Some(true)),
                _ if attempt => (builder, Some((run, 1)), Some(true)),
                _ => {
                    let ours = |i: InvocationId| driver.recorded.iter().any(|r| r.1 == i);
                    assert!(
                        invocation.is_some_and(ours),
                        "{site}: a recorded invocation"
                    );
                    (*invocation, None, None)
                }
            };
            assert_eq!((*invocation, *claim, *compared), expected, "{site}");
            let domain = if attempt { "attempt" } else { "session" };
            let role = driver.recorded.iter().find(|r| Some(r.1) == *invocation);
            let role = role.map(|r| r.0);
            println!("WITNESS {site} domain={domain} role={role:?} claim={claim:?} compared={compared:?}");
        }
        println!(
            "WITNESS driver_calls={} recorded={}",
            driver.calls,
            driver.recorded.len()
        );
        let (p, claim) = (witness.profile, witness.claim);
        println!(
            "WITNESS profile {p:?} run={run} attempt={:?}",
            claim.map(|c| c.1)
        );
        let (site, ..) = RECORDED.position.unwrap();
        let neighbours = witness.neighbours;
        println!("WITNESS position {site} between {neighbours:?}");
    });
}

/// A.7: each join plant on the recorded path's own facts and traces must
/// refuse, each for its own reason; the unplanted path credits.
#[test]
fn route_s5_join_plants_each_refuse() {
    child("s5::route_s5_join_plants_each_refuse", |f| {
        let (d, b, m) = recorded(f);
        assert!(credit(&d, &RECORDED, &b, &m).is_ok());
        // Another session's identity: a fresh seed's (one broker per process).
        let elsewhere = SessionSeed::new().unwrap().identity();
        let other = RunId::parse(&format!("run-{}", "0".repeat(32))).unwrap();
        let role = |builder: bool| {
            d.recorded
                .iter()
                .find(|r| (r.0 == LaneRole::Builder) == builder)
        };
        let (builder, scripted) = (role(true).unwrap().1, role(false).unwrap().1);
        let unknown = InvocationId {
            seq: 1 << 40,
            ..builder
        };
        let case = Case {
            d,
            e: RECORDED,
            b,
            m,
            other,
            elsewhere,
            builder,
            scripted,
            unknown,
        };
        let plants: [Plant; 43] = [
            ("two calls", "driver_calls", |c| {
                c.d = Driver {
                    calls: 2,
                    ..c.d.clone()
                }
            }),
            ("no call with S5 entries", "driver_calls", |c| {
                c.d = Driver {
                    calls: 0,
                    ..c.d.clone()
                }
            }),
            ("wrong session", "session", |c| {
                c.m("mother.charge").session = Some(c.elsewhere)
            }),
            ("wrong request", "request", |c| {
                c.m("mother.charge").request = Some(c.d.request + 1)
            }),
            ("run_in_two_sessions", "run_in_two_sessions", |c| {
                c.b("attempt.slot").session = Some(c.elsewhere)
            }),
            ("two runs bound", "two_bound_runs", |c| {
                c.b("attempt.slot").run = Some(c.other)
            }),
            ("binding on bound: no", "binding_on_unbound", |c| {
                c.e.profile.bound = false;
                c.e.attempt_level = false;
            }),
            ("opening on bound: no", "opening_on_unbound", |c| {
                c.b.retain(|x| !binding(x));
                c.e.profile.bound = false;
                c.e.attempt_level = false;
            }),
            ("opening off the run", "opening_run", |c| {
                c.b("model.attempt.granted").run = Some(c.other)
            }),
            ("second opening, same key", "second_opening", |c| {
                let again = c.b("model.attempt.granted").clone();
                c.b.push(again);
            }),
            ("claim not the expect row's", "attempts", |c| {
                c.e.attempts = 0
            }),
            ("call.gate 3", "profile", |c| {
                let gate = c.b.iter().rposition(|e| e.point == "call.gate").unwrap();
                c.b.remove(gate);
            }),
            ("second recheck", "profile", |c| {
                let again = c.m("mother.recheck").clone();
                c.m.push(again);
            }),
            ("profile not the expect row's", "profile", |c| {
                c.e.profile.classify = 0
            }),
            ("unrecorded invocation", "unrecorded_invocation", |c| {
                c.m("mother.charge").invocation = Some(c.unknown)
            }),
            ("unrecorded, session-only", "unrecorded_invocation", |c| {
                c.m("mother.admit").invocation = Some(c.unknown)
            }),
            ("not the model lane's", "not_model_invocation", |c| {
                c.m("mother.charge").invocation = Some(c.scripted)
            }),
            ("session-only made to claim", "multiplicity", |c| {
                let (point, builder) = ("mother.lane.prelock", Some(c.builder));
                let at =
                    c.m.iter()
                        .position(|e| e.point == point && e.invocation == builder);
                c.m[at.unwrap()].point = "mother.charge.ok";
            }),
            (
                "a session-only site's count per invocation",
                "per_invocation",
                |c| {
                    let (point, builder) = ("mother.lane.prelock", Some(c.builder));
                    let at =
                        c.m.iter()
                            .position(|e| e.point == point && e.invocation == builder);
                    c.m[at.unwrap()].point = "mother.admit.ok";
                },
            ),
            ("classify on another run", "classify_run", |c| {
                c.m("mother.classify").run = Some(c.other)
            }),
            ("request_compared false", "request_compared", |c| {
                let gates = c.b.iter_mut().filter(|e| e.point == "call.gate");
                gates.for_each(|e| e.request = e.request.map(|r| r + 1));
            }),
            ("request-level, call.gate 0", "request_level", |c| {
                c.b.retain(|e| e.point != "call.gate");
                c.e.profile.call_gate = 0;
            }),
            ("wrong neighbour", "neighbour", |c| {
                let (site, _, after) = c.e.position.unwrap();
                c.e.position = Some((site, ("model.call.admitted", Some(2)), after));
            }),
            ("call.gate off the claim", "gate_attempt", |c| {
                c.b("call.gate").attempt = Some(2)
            }),
            ("no position site", "position_site", |c| {
                c.m.retain(|e| e.point != "mother.charge.entry");
                c.e.multiplicity[0].1 = 0;
            }),
            ("tie", "tie", |c| {
                let instant = c.m("mother.charge.entry").instant;
                let gate = c.b.iter().rposition(|e| e.point == "call.gate").unwrap();
                c.b[gate].instant = instant;
            }),
            ("wrong invocation, classify", "classify_invocation", |c| {
                c.m("mother.classify").invocation = Some(c.builder)
            }),
            // SB3E-R40.1 and R40.2, R40.5: each new rule's own plant.
            (
                "invocation removed, attempt site",
                "missing_invocation",
                |c| c.m("mother.charge").invocation = None,
            ),
            (
                "invocation removed, session-only",
                "missing_invocation",
                |c| c.m("mother.admit").invocation = None,
            ),
            ("row count 2", "row_counts", |c| c.e.profile.recheck = 2),
            // SB3E-R42.8: no opening, `attempts: 0`, `bound: yes`.
            ("attempt-level with no claim", "attempt_level", |c| {
                let opening = |e: &s1::Entry| e.point == "model.attempt.granted" && !e.note;
                c.b.retain(|e| !opening(e));
                (c.e.attempts, c.e.profile.opening) = (0, 0);
            }),
            ("attempt-level on bound: no", "attempt_level", |c| {
                c.e.profile.bound = false
            }),
            ("request-level, nothing compared", "no_compared", |c| {
                c.m.retain(|e| !ATTEMPT.contains(&e.point));
                (c.e.profile.recheck, c.e.profile.classify) = (0, 0);
                c.e.multiplicity.iter_mut().for_each(|m| m.1 = 0);
            }),
            ("second charge", "multiplicity", |c| {
                let again = c.m("mother.charge").clone();
                c.m.push(again);
            }),
            ("position site repeated", "position_repeated", |c| {
                let (_, before, after) = c.e.position.unwrap();
                c.e.position = Some(("mother.lane.prelock", before, after));
            }),
            ("binding entry without a run", "binding_without_run", |c| {
                c.b("attempt.slot").run = None
            }),
            ("opening without an attempt", "opening_fields", |c| {
                c.b("model.attempt.granted").attempt = None
            }),
            (
                "classify on another run, no invocation",
                "classify_run",
                |c| c.m("mother.classify").run = None,
            ),
            (
                "request-level, call.gate 0 row only",
                "request_level",
                |c| c.e.profile.call_gate = 0,
            ),
            // SB3E-R49.3 (P-CREDIT): the role lookup selects, never totals.
            ("a wrong count for one role", "per_invocation", |c| {
                c.e.roles = |r| match r == LaneRole::Skeptic {
                    true => sessions(2),
                    false => done_roles(r),
                }
            }),
            (
                "a recorded role not in the lookup",
                "role_unexpected",
                |c| c.e.roles = |r| done_roles(r).filter(|_| r != LaneRole::Skeptic),
            ),
            ("a role in the lookup, none recorded", "role_missing", |c| {
                let verifier = |r: &(LaneRole, InvocationId)| r.0 == LaneRole::Verifier;
                let gone: Vec<_> =
                    c.d.recorded
                        .iter()
                        .filter(|r| verifier(r))
                        .map(|r| r.1)
                        .collect();
                c.d.recorded.retain(|r| !verifier(r));
                c.m.retain(|e| !e.invocation.is_some_and(|i| gone.contains(&i)));
            }),
            (
                "Sentinel's invocations 2/0, not 1/1",
                "per_invocation",
                |c| {
                    let sentinel = |r: &&(LaneRole, InvocationId)| r.0 == LaneRole::Sentinel;
                    let two: Vec<_> = c.d.recorded.iter().filter(sentinel).map(|r| r.1).collect();
                    let second =
                        |e: &s5::Entry| e.point == "mother.admit" && e.invocation == Some(two[1]);
                    let at = c.m.iter().position(second).unwrap();
                    c.m[at].invocation = Some(two[0]);
                },
            ),
        ];
        for (name, reason, plant) in plants {
            let mut c = case.clone();
            plant(&mut c);
            assert_eq!(credit(&c.d, &c.e, &c.b, &c.m).err(), Some(reason), "{name}");
            println!("WITNESS plant {name:?} refused {reason:?}");
        }
    });
}
/// A join plant: its name, the reason code it must give, its mutation.
type Plant = (&'static str, &'static str, fn(&mut Case));
/// One join plant's material: the recorded facts, row and traces.
#[derive(Clone)]
struct Case {
    d: Driver,
    e: Expect,
    b: Vec<s1::Entry>,
    m: Vec<s5::Entry>,
    other: RunId,
    elsewhere: SessionIdentity,
    builder: InvocationId,
    scripted: InvocationId,
    unknown: InvocationId,
}
impl Case {
    fn b(&mut self, point: &str) -> &mut s1::Entry {
        let at = self.b.iter().position(|e| e.point == point).unwrap();
        &mut self.b[at]
    }
    fn m(&mut self, point: &str) -> &mut s5::Entry {
        let at = self.m.iter().position(|e| e.point == point).unwrap();
        &mut self.m[at]
    }
}

/// S5's pause timeout (design 3.3): the harness writes the line to this
/// child's result file and aborts; S5 itself touches no host resource.
/// Without a child root it aborts without writing.
fn timeout(line: &str) -> ! {
    use std::io::Write;
    let Some(root) = std::env::var_os(CHILD).map(PathBuf::from) else {
        std::process::abort() // SB3E-R40.5: never relative to the cwd.
    };
    let mut open = std::fs::OpenOptions::new();
    if let Ok(mut result) = open.create(true).append(true).open(root.join("result")) {
        let _ = writeln!(result, "{line}");
    }
    std::process::abort()
}

/// SB3E-R42.5: a child's output, its wait bounded (killed after 30 s), as
/// `child_with` waits.
fn bounded(command: &mut std::process::Command) -> std::process::Output {
    let mut process = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while process.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            process.kill().unwrap();
        }
        std::thread::sleep(Duration::from_millis(5)); // Polls an exit only.
    }
    process.wait_with_output().unwrap()
}

/// SB3E-R40.5: without a child root, the harness `timeout` aborts and
/// writes nothing, so no result file appears relative to the cwd.
#[test]
fn route_s5_timeout_without_a_child_root_writes_nothing() {
    const NO_ROOT: &str = "SB3E_ROUTE_NO_ROOT";
    if std::env::var_os(NO_ROOT).is_some() {
        timeout("TIMEOUT u4a2 no-root");
    }
    let name = "s5::route_s5_timeout_without_a_child_root_writes_nothing";
    let cwd = std::env::temp_dir().join(format!("sb3e-route-{}-no-root", std::process::id()));
    std::fs::DirBuilder::new().mode(0o700).create(&cwd).unwrap();
    let cwd = Fixture(cwd); // Removed when the test ends.
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--nocapture", "--test-threads=1"]);
    let out = bounded(
        command
            .env_remove(CHILD)
            .env(NO_ROOT, "1")
            .current_dir(&cwd.0),
    );
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(out.status.signal(), Some(6), "aborted");
    let written: Vec<_> = std::fs::read_dir(&cwd.0).unwrap().collect();
    assert!(written.is_empty(), "nothing written: {written:?}");
}

/// One `mother.*` point, paused: acknowledged with the driver's session,
/// request and invocation, the route held before its effect and completed
/// on release; then the same point armed to panic begins an unwind.
#[test]
fn route_s5_pause_release_and_panic_at_one_point() {
    child("s5::route_s5_pause_release_and_panic_at_one_point", |f| {
        let o = f.open();
        let mother = mother(f, &o);
        mother.m.advance(mother.id).unwrap(); // To Investigating: no lane yet.
        let (cell, timeout) = ("u4a2", timeout as s5::Timeout);
        let held = s5::arm("mother.lane.prelock", s5::Action::Pause { cell, timeout });
        let journaled = || mother.m.journal().unwrap().len();
        let held_at = std::thread::scope(|s| {
            let advancing = s.spawn(|| mother.m.advance(mother.id).map(|_| ()));
            let Ok(entry) = held.acknowledged.recv_timeout(PATIENCE) else {
                panic!("not acknowledged: {:?}", advancing.join());
            };
            let recorded = mother.recorded.lock().unwrap().clone();
            assert_eq!(entry.session, Some(o.session.identity()));
            assert_eq!(entry.request, Some(mother.request));
            assert_eq!(entry.invocation, recorded.last().map(|r| r.1));
            // SB3E-R40.5: held, by readiness. The release is still unsent;
            // the held point is the trace's last entry and no later route
            // entry, nor its LaneReturned, exists.
            let held_at = journaled();
            let trace = s5::drain_trace();
            assert_eq!(trace.last().map(|e| e.tick), Some(entry.tick), "{trace:?}");
            assert!(!advancing.is_finished(), "the route is held");
            assert_eq!(journaled(), held_at, "held before its LaneReturned");
            drop(held);
            assert_eq!(advancing.join().unwrap(), Ok(()), "released, it completes");
            held_at
        });
        assert!(journaled() > held_at, "its LaneReturned, after release");
        // File mode contains an unwind as a halted, poisoned call (lite.rs
        // `call`); the coordinator lock was not held, so it stays readable.
        let held = s5::arm("mother.lane.prelock", s5::Action::Panic);
        let unwound = mother.m.advance(mother.id).map(|_| ());
        assert_eq!(
            held.acknowledged.try_recv().unwrap().point,
            "mother.lane.prelock"
        );
        assert_eq!(format!("{unwound:?}"), "Err(Poisoned)");
        assert!(mother.m.state(mother.id).is_ok(), "no lock was poisoned");
    });
}

/// S4b (design 4.2, 4.5): applied while the route is paused before the
/// coordinator lock (`mother.lane.prelock`, before `co:1968`), the released
/// step's `try_with_lock` returns `Poisoned` and engages `DoorHalted`: the
/// lock stays poisoned, and the request's own admission is now refused.
#[test]
fn route_s4b_poisons_the_coordinator_before_its_lock() {
    child(
        "s5::route_s4b_poisons_the_coordinator_before_its_lock",
        |f| {
            let o = f.open();
            let mother = mother(f, &o);
            mother.m.advance(mother.id).unwrap(); // To Investigating: no lane yet.
            let (cell, timeout) = ("u4a2", timeout as s5::Timeout);
            let held = s5::arm("mother.lane.prelock", s5::Action::Pause { cell, timeout });
            let step = nexus_governed_control::host_session::MotherStep::LaneInvocation;
            let observed = || {
                format!(
                    "{:?} {:?} {:?}",
                    o.ingress.failed(),
                    o.session.state(),
                    mother.context.admit(step)
                )
            };
            let before = observed();
            let advanced = std::thread::scope(|s| {
                let advancing = s.spawn(|| mother.m.advance(mother.id).map(|_| ()));
                held.acknowledged.recv_timeout(PATIENCE).unwrap();
                mother.m.route_poison_coordinator();
                drop(held);
                advancing.join().unwrap()
            });
            assert_eq!(before, "false Normal Ok(())", "paused before its lock");
            assert_eq!(format!("{advanced:?}"), "Err(Poisoned)");
            assert_eq!(
                observed(),
                "false Normal Err(Restricted)",
                "the cell engaged"
            );
            let state = mother.m.state(mother.id).map(|_| ());
            assert_eq!(
                format!("{state:?}"),
                "Err(Poisoned)",
                "the lock is poisoned"
            );
        },
    );
}

/// SB3E-R39.3: a panicking model call is still counted (`drive_call` counts
/// before the call). `mother.recheck` runs inside `ModelCaller::call` on the
/// lane's own thread (S1's `call.gate` runs on the broker's exchange thread
/// and does not unwind through the call); armed to panic there, the unwind
/// leaves the call, and the driver's count is 1 beside S5 entries, so A.2
/// item 4 never sees `driver_calls == 0`.
#[test]
fn route_s5_panicking_call_is_counted() {
    child("s5::route_s5_panicking_call_is_counted", |f| {
        let o = f.open();
        let mother = mother(f, &o);
        let _s1 = s1::arm("call.gate", s1::Action::Trace);
        let held = s5::arm("mother.recheck", s5::Action::Panic);
        s1::drain_trace();
        s5::drain_trace();
        let mut advanced = Ok(());
        for _ in 0..8 {
            advanced = mother.m.advance(mother.id).map(|_| ());
            if advanced.is_err() {
                break;
            }
        }
        let fired = held.acknowledged.try_recv().map(|e| e.point);
        assert_eq!(fired, Ok("mother.recheck"), "the call unwound");
        assert_eq!(format!("{advanced:?}"), "Err(Poisoned)", "out of the call");
        let (driver, broker, trace) = (mother.driver(&o), s1::drain_trace(), s5::drain_trace());
        assert_eq!(driver.calls, 1, "counted before the call");
        let entry = trace.iter().any(|e| e.point == "mother.charge.entry");
        assert!(entry, "S5 entries of the call");
        let uncounted = Driver {
            calls: 0,
            ..driver.clone()
        };
        let refused = credit(&uncounted, &RECORDED, &broker, &trace).err();
        assert_eq!(refused, Some("driver_calls"), "A.2 item 4");
        let measured = credit(&driver, &RECORDED, &broker, &trace).err();
        assert_ne!(measured, Some("driver_calls"));
        println!(
            "WITNESS panic driver_calls={} s5={}",
            driver.calls,
            trace.len()
        );
    });
}

/// SB3E-R39.1: an unreleased pause times out. After S5's 10 s patience the
/// harness's `timeout` writes `TIMEOUT <cell> <point>` and aborts; the route
/// never resumes.
#[test]
fn route_s5_unreleased_pause_times_out_and_aborts() {
    let name = "s5::route_s5_unreleased_pause_times_out_and_aborts";
    if let Some(root) = std::env::var_os(CHILD) {
        let f = Fixture(PathBuf::from(root));
        let o = f.open();
        let mother = mother(&f, &o);
        mother.m.advance(mother.id).unwrap(); // To Investigating: no lane yet.
        let (cell, timeout) = ("u4a3", timeout as s5::Timeout);
        let _held = s5::arm("mother.lane.prelock", s5::Action::Pause { cell, timeout });
        let resumed = mother.m.advance(mother.id);
        return println!("ROUTE CHILD RESUMED {name} {resumed:?}");
    }
    let root = std::env::temp_dir().join(format!("sb3e-route-{}-timeout", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let root = Fixture(root); // Removed when the test ends.
    let started = Instant::now();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--nocapture", "--test-threads=1"]);
    let out = bounded(command.env(CHILD, &root.0));
    let elapsed = started.elapsed();
    let result = std::fs::read_to_string(root.0.join("result")).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(out.status.signal(), Some(6), "aborted: {stdout}");
    assert_eq!(result, "TIMEOUT u4a3 mother.lane.prelock\n");
    assert!(!stdout.contains("ROUTE CHILD RESUMED"), "{stdout}");
    assert!(elapsed >= PATIENCE, "after its patience: {elapsed:?}");
    println!("WITNESS timeout {result:?} after {elapsed:?}");
}

// SB3E-R86.4 (U5a): the cell runner (design 3.3; mission Behaviours 5, 7 and
// 10), the facts it hands to `witness.rs`, and the T6 child half (Behaviour
// 11). It credits no cell.

/// The variable that carries a cell child's nonce (design 3.3).
const NONCE: &str = "SB3E_ROUTE_NONCE";

/// `Fixture::open`'s answers (main.rs `Owner`), copied: `s5.rs` imports no
/// other item of its parent (G-S3). T7 names it as a difference.
struct Answers;
impl ControlConfirmer for Answers {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

/// Behaviours 5 and 7: `Fixture::open` with a `MemoryEvidence` projection,
/// the only native observation (R49.7; it sees the sealed records, R86.3).
fn opened(f: &Fixture) -> (Opened, Arc<MemoryEvidence>) {
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = f.0.join("home");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&home)
        .unwrap();
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    let native = Arc::new(MemoryEvidence::new(4096));
    let projection: Arc<dyn EvidenceProjection> = native.clone();
    let db = home.join("broker/p3-evidence/evidence.sqlite3");
    let evidence = PrimaryEvidence::open(&db, vec![projection]).unwrap();
    let clock = Arc::new(SystemClock::default());
    let session = HostSession::open_with_history(
        seed,
        &home.join("broker"),
        &home,
        evidence.clone(),
        clock,
        Arc::new(Answers),
    )
    .unwrap();
    assert!(session.startup_report().ready());
    let o = Opened {
        session,
        ingress,
        evidence,
    };
    (o, native)
}

/// What a cell's child observed, passively, once every worker is joined
/// (Behaviour 7); `witness.rs` renders it with S, Q, RUN, INV, ATT bound.
#[derive(Clone)]
pub(super) struct Facts {
    pub(super) session: SessionIdentity,
    pub(super) request: u64,
    pub(super) recorded: Vec<(LaneRole, InvocationId)>,
    /// `credit`'s claim (A.3): the session-bound run and the opening's attempt.
    pub(super) claim: Option<(RunId, u32)>,
    /// The points the chain held, in order (R88.1: each acknowledgement must
    /// answer its own point), and their acknowledged entries.
    pub(super) points: Vec<&'static str>,
    pub(super) acks: Vec<Ack>,
    /// The value applied while the last point was held (`none`: a control).
    pub(super) value: &'static str,
    /// W0b B1: the cell's table row, `(sink, dim)`, so `render` reads its
    /// frozen row; and when its value was applied (`ack=held` or `armed`).
    pub(super) row: (&'static str, &'static str),
    pub(super) applied: Option<Instant>,
    pub(super) broker: Vec<s1::Entry>,
    pub(super) mother: Vec<s5::Entry>,
    /// The journal's entries in the window, read from its file (R1-2): since
    /// `start` captured the sources (`collect`), or since the hold (`collect_close`).
    pub(super) journal: Vec<Journaled>,
    /// The projection's records in the window.
    pub(super) native: Vec<EvidenceRecord>,
    /// The peer's requests in the window, in served order.
    pub(super) peer: Vec<String>,
    /// The child's own checks that failed (T6, `credit`, the driver's): they
    /// fail the child after its record is written, so the parent still
    /// compares the witness and names its fields (P-CELL attribution).
    pub(super) findings: Vec<String>,
}

/// W0b A2: an acknowledgement, and a standing hold, of S1 or of S5.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Ack {
    S1(s1::Entry),
    S5(s5::Entry),
}
// Held only to stand until dropped (its drop releases the hold).
#[allow(dead_code)]
pub(super) enum Hold {
    S1(s1::Held),
    S5(s5::Held),
}
/// W0b B1', B2', U9a M1: each collection of this child (its collector's
/// name and its facts), and the running cell, kept where no section reaches;
/// `cell` decides from them, not from what a driver returns.
static KEPT: Mutex<Vec<(&'static str, Facts)>> = Mutex::new(Vec::new());
static RUNNING: Mutex<&str> = Mutex::new("none");
/// W0b F-R5: a finding s5's own check makes after the collection (an
/// applier's): in the facts it returns, and in the kept collection, which
/// `verdict` reads; never only in what a section hands back.
fn later(facts: &mut Facts, finding: String) {
    if let Some((_, kept)) = KEPT.lock().unwrap().last_mut() {
        kept.findings.push(finding.clone());
    }
    facts.findings.push(finding);
}
/// W0b B2': the sinks with no Mother (the table's `d_register` and
/// `d_bootstrap` rows), the only ones `collect_broker` may collect.
pub(super) const BROKER_SINKS: [&str; 8] = [
    "registration_install",
    "registration_readiness",
    "registration_record",
    "registration_reserve",
    "registration_show",
    "registration_status",
    "registration_tags",
    "registration_version",
];
/// U9a M1: the sinks of the table's `d_close` rows, the only ones
/// `collect_close` may collect.
pub(super) const CLOSE_SINKS: [&str; 6] = [
    "close_obligations",
    "delivery",
    "mother_end",
    "primary_end",
    "producer_join",
    "seal",
];
/// W0b B1', B2', U9a M1: the one collection of cell `id`, as `cell` judges it:
/// its own findings, its driver's facts unchanged, its row this cell's (the
/// slug is lowercase, the value keeps its case), and its collector the row's
/// (`collect_broker` exactly for `BROKER_SINKS`, `collect_close` exactly for
/// `CLOSE_SINKS`).
fn verdict(id: &str, returned: &Facts, render: fn(&Facts) -> String) -> Vec<String> {
    let kept = std::mem::take(&mut *KEPT.lock().unwrap());
    let [(collector, facts)] = &kept[..] else {
        return vec![format!("collect: {} collections, not 1", kept.len())];
    };
    // The kept findings are the collection's and s5's own later ones
    // (`later`, W0b F-R5): they decide, and the returned facts must carry
    // exactly them, so a section can neither clear nor add one.
    let mut found = facts.findings.clone();
    if render(returned) != render(facts) || returned.findings != facts.findings {
        found.push("collect: the driver's facts are not its collection's".into());
    }
    let ((sink, dim), value) = (facts.row, facts.value);
    let own = match id.split_once('_') {
        Some(("rc", slug)) => {
            let row = format!("{sink}_{dim}_{value}").to_lowercase();
            slug.split_once('_').map(|x| x.1) == Some(&*row)
        }
        Some(("ctl", control)) => control == sink && value == "none",
        _ => false,
    };
    if !own {
        found.push(format!(
            "collect: {sink}/{dim}/{value} is not the row of {id}"
        ));
    }
    let broker = *collector == "collect_broker";
    if broker != BROKER_SINKS.contains(&sink) {
        found.push(format!(
            "collect: {sink} collected by collect_broker {broker}"
        ));
    }
    let close = *collector == "collect_close";
    if close != CLOSE_SINKS.contains(&sink) {
        found.push(format!(
            "collect: {sink} collected by collect_close {close}"
        ));
    }
    found
}

/// One cell child's run, as its parent saw it (design 3.3).
pub(super) struct Ran {
    pub(super) id: &'static str,
    pub(super) nonce: String,
    /// The child's private result file, whole.
    pub(super) result: String,
    pub(super) passed: bool,
    pub(super) output: String,
}

/// The cell runner (design 3.3). In the parent: a fresh nonce by env and one
/// child in an owned root, waited only through `bounded`; `Some` of what the
/// child left. In the child: `drive`, then `render`'s witness written as the
/// one `CELL <nonce> <id> <witness>` record of its root's result file; `None`.
pub(super) fn cell(
    name: &str,
    id: &'static str,
    drive: fn(&Fixture) -> Facts,
    render: fn(&Facts) -> String,
) -> Option<Ran> {
    if let Some(root) = std::env::var_os(CHILD) {
        let f = Fixture(PathBuf::from(root));
        *RUNNING.lock().unwrap() = id;
        let facts = drive(&f);
        let witness = render(&facts);
        let findings = verdict(id, &facts, render);
        let nonce = std::env::var(NONCE).expect("the parent's nonce");
        let mut append = std::fs::OpenOptions::new();
        let mut result = append.append(true).open(f.0.join("result")).unwrap();
        let line = format!("CELL {nonce} {id} {witness}\n");
        std::io::Write::write_all(&mut result, line.as_bytes()).unwrap();
        if !findings.is_empty() {
            panic!("{}; witness {witness}", findings.join("; "));
        }
        return None;
    }
    let mut bytes = [0u8; 16];
    let mut random = std::fs::File::open("/dev/urandom").unwrap();
    std::io::Read::read_exact(&mut random, &mut bytes).unwrap();
    let nonce: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let leaf = format!("sb3e-route-{}-cell-{id}", std::process::id());
    let root = std::env::temp_dir().join(leaf);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let root = Fixture(root); // This child's owned root, removed on return.
    std::fs::File::create(root.0.join("result")).unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--nocapture", "--test-threads=1"]);
    let out = bounded(command.env(CHILD, &root.0).env(NONCE, &nonce));
    let result = std::fs::read_to_string(root.0.join("result")).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let output = format!("{stdout}{}", String::from_utf8_lossy(&out.stderr));
    let passed = out.status.success();
    Some(Ran {
        id,
        nonce,
        result,
        passed,
        output,
    })
}

impl Mother {
    /// W0b B4: the consented mission, for a section's Stop (its `id` stays
    /// unexported, so `PORT_USES` keeps its row).
    pub(super) fn mission(&self) -> MissionId {
        self.id
    }
    /// Advances until the advance that made the Builder's one counted call
    /// has returned; each step's result, as text.
    fn advance_to_call(&self) -> Vec<String> {
        let mut steps = Vec::new();
        for _ in 0..8 {
            let step = self.m.advance(self.id).map(|_| ());
            steps.push(format!("{step:?}"));
            if step.is_err() || self.calls.load(Ordering::SeqCst) > 0 {
                break;
            }
        }
        steps
    }
}

/// F-S5-SPLIT item 1(c)(ii), 1(f) (SB3E-R95.7): the S5 pause helper, the only
/// way a section arms an S5 pause. It takes no callback: it passes this
/// module's pinned `timeout` itself. Its body is a pinned region.
pub(super) fn hold(point: &'static str) -> s5::Held {
    let (cell, timeout) = (*RUNNING.lock().unwrap(), timeout as s5::Timeout);
    s5::arm(point, s5::Action::Pause { cell, timeout })
}

/// W0b B2: where a chained hold is anchored. `Mother(point)` pauses S5 at
/// `point`; U5's is `mother.charge.entry`, between ord 3's
/// `model.call.admitted` and ord 4's `call.gate`. A chain with no further
/// point ends on that S5 hold (W0b A2). `Arrival(n)` is first-arrival anchoring: S1 paused at
/// the chain's first point from before the advance, its `n`-th arrival
/// acknowledged, re-armed between arrivals (as `ord3_quit` did), for points
/// before the charge or outside it.
#[derive(Clone, Copy)]
pub(super) enum Anchor {
    Mother(&'static str),
    Arrival(usize),
}
/// A cell's chain: its anchor and the points held in order, the row's own
/// point last.
pub(super) struct Chain {
    pub(super) row: (&'static str, &'static str),
    pub(super) anchor: Anchor,
    pub(super) points: &'static [&'static str],
}
impl Chain {
    /// The held points, in order: the further points, or the S5 anchor's.
    fn anchor_point(&self) -> Vec<&'static str> {
        match (self.anchor, self.points) {
            (Anchor::Mother(point), []) => vec![point],
            _ => self.points.to_vec(),
        }
    }
}

/// The chained hold (Behaviour 6; no new accessor). The Builder's advance
/// runs on a worker, held at `chain`'s anchor; each further point is armed
/// while the hold before it stands, and that hold is then released, so on
/// U5's anchor each acknowledgement is the chat's (ord 4). Every wait is a
/// bounded receive (`PATIENCE`). `apply` runs while the last hold stands,
/// with that hold and the advancing worker's thread, and releases it before
/// the worker is joined. Returns the S1 acknowledgements, in order, the
/// advance's steps and what `apply` returned.
fn chained<R>(
    f: &Fixture,
    m: &Mother,
    chain: &Chain,
    apply: impl FnOnce(Hold, ThreadId) -> R,
) -> (Vec<Ack>, Vec<String>, R) {
    let (anchor, mut points) = (chain.anchor, chain.points);
    let entry = match anchor {
        Anchor::Mother(point) => Some(hold(point)),
        Anchor::Arrival(_) => None,
    };
    let arrival = match anchor {
        Anchor::Arrival(_) => Some(f.pause(points[0])),
        Anchor::Mother(_) => None,
    };
    std::thread::scope(|s| {
        let advancing = s.spawn(|| m.advance_to_call());
        let (mut before, mut acks) = (None, Vec::new());
        if let Some(entry) = &entry {
            let Ok(reached) = entry.acknowledged.recv_timeout(PATIENCE) else {
                panic!("{:?}: not acknowledged", chain.anchor_point());
            };
            if points.is_empty() {
                acks.push(Ack::S5(reached));
            }
        }
        if let (Anchor::Arrival(n), Some(mut held)) = (anchor, arrival) {
            for k in 1..=n {
                let Ok(ack) = held.acknowledged.recv_timeout(PATIENCE) else {
                    panic!("{}: arrival {k} not acknowledged", points[0]);
                };
                match k < n {
                    true => drop(std::mem::replace(&mut held, f.pause(points[0]))),
                    false => acks.push(Ack::S1(ack)),
                }
            }
            (before, points) = (Some(held), &points[1..]);
        }
        let mut first = entry;
        for point in points {
            let held = f.pause(point);
            drop((first.take(), before.take()));
            let Ok(ack) = held.acknowledged.recv_timeout(PATIENCE) else {
                panic!("{point}: not acknowledged");
            };
            acks.push(Ack::S1(ack));
            before = Some(held);
        }
        // W0b A2: with no S1 point after the anchor, the S5 hold stands.
        let last = match (before, first) {
            (Some(held), _) => Hold::S1(held),
            (None, Some(held)) => Hold::S5(held),
            (None, None) => panic!("a chain with no point"),
        };
        let applied = apply(last, advancing.thread().id());
        (acks, advancing.join().unwrap(), applied)
    })
}

/// A cell's start (Behaviour 5): the session with its projection, the S5
/// fixture, both traces armed and drained, and where the observations begin
/// (journal, projection, peer).
pub(super) struct Start {
    pub(super) o: Opened,
    pub(super) mother: Mother,
}
/// W0b R1-2: the running cell's observation sources, captured by `start` and
/// private to s5.rs: the journal file, the projection, the peer, both
/// traces, the driver's own counters and where each observation begins. So
/// `collect` reads them after the Mother's `FileSession` is released by
/// value (`prepare_session_end`, R95.2); a section supplies none of them.
struct Sources {
    cell: &'static str,
    journal: PathBuf,
    native: Arc<MemoryEvidence>,
    peer: Arc<Mutex<Vec<String>>>,
    driver: (SessionIdentity, u64, Arc<AtomicU32>, Recorded),
    since: (usize, usize, usize),
    _traces: (s1::Held, s5::Held),
}
static SOURCES: Mutex<Option<Sources>> = Mutex::new(None);
/// W0b N-b: when s5's own applier applied the cell's value (`driven`); a
/// section can neither supply nor change it.
static APPLIED: Mutex<Option<Instant>> = Mutex::new(None);
/// The Mother journal's entries, read from its file (no handle is opened).
fn journal_at(path: &std::path::Path) -> Result<Vec<Journaled>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("journal: {:?}", e.kind()))?;
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    nexus_mother::journal::verify(&lines).map_err(|e| format!("journal: {e:?}"))
}
pub(super) fn start(f: &Fixture) -> Start {
    let (o, native) = opened(f);
    let mother = mother(f, &o);
    let traces = (
        s1::arm("reg.reserve", s1::Action::Trace),
        s5::arm("mother.charge", s5::Action::Trace),
    );
    s1::drain_trace();
    s5::drain_trace();
    let journal = f.0.join("home/mother-journal.jsonl");
    let peer = mother.peer.clone();
    let since = (
        journal_at(&journal).unwrap().len(),
        native.records().len(),
        peer.lock().unwrap().len(),
    );
    let m = &mother;
    *SOURCES.lock().unwrap() = Some(Sources {
        cell: *RUNNING.lock().unwrap(),
        journal,
        native,
        peer,
        driver: (
            o.session.identity(),
            m.request,
            m.calls.clone(),
            m.recorded.clone(),
        ),
        since,
        _traces: traces,
    });
    *APPLIED.lock().unwrap() = None;
    Start { o, mother }
}
/// A held cell: the chained hold of `chain`, `apply` while the last
/// stands (Behaviour 6), then the passive collection under the measured
/// `row` (Behaviours 7, 8). Returns the facts, the advance's steps and what
/// `apply` returned.
pub(super) fn driven<R>(
    f: &Fixture,
    value: &'static str,
    chain: &Chain,
    row: &Expect,
    apply: impl FnOnce(&Opened, &Mother, Hold, ThreadId) -> R,
) -> (Facts, Vec<String>, R) {
    let c = start(f);
    let (o, m) = (&c.o, &c.mother);
    let (acks, steps, applied) = chained(f, m, chain, |held, chat| {
        *APPLIED.lock().unwrap() = Some(Instant::now());
        apply(o, m, held, chat)
    });
    println!("WITNESS {value}: the advance {steps:?}");
    let facts = collect((chain.anchor_point(), acks), (chain.row, value), row);
    (facts, steps, applied)
}

/// W0b R1-2: `ctl_call_chat`'s path, then the Mother's `FileSession` released
/// by value (`prepare_session_end`, R95.2, as the close chain does), and only
/// then the collection, from the sources `start` captured.
pub(super) fn released(f: &Fixture) -> Facts {
    let Start { o, mother } = start(f);
    let chain = Chain {
        row: ("call_chat", "ingress"),
        anchor: Anchor::Mother("mother.charge.entry"),
        points: &["call.gate", "model.call.observed"],
    };
    let (acks, steps, ()) = chained(f, &mother, &chain, |held, _| drop(held));
    let ended = mother.m.prepare_session_end().map(|_| ());
    println!("WITNESS released: the advance {steps:?}, the end {ended:?}");
    let facts = collect((chain.anchor_point(), acks), (chain.row, "none"), &HELD);
    drop(o);
    facts
}
/// Behaviour 10: `ctl_call_chat`'s driver. Held at `call.gate` and then at
/// `model.call.observed` on ord 4, released with no value, the chat reaches
/// `model.call.admitted` and a recorded outcome.
pub(super) fn ctl_call_chat(f: &Fixture) -> Facts {
    let chain = Chain {
        row: ("call_chat", "ingress"),
        anchor: Anchor::Mother("mother.charge.entry"),
        points: &["call.gate", "model.call.observed"],
    };
    let (facts, steps, ()) = driven(f, "none", &chain, &HELD, |_, _, held, _| drop(held));
    assert!(
        steps.iter().all(|s| s == "Ok(())"),
        "the advance: {steps:?}"
    );
    facts
}

// SB3E-R86.4 (U5b): the six credited values (rows 93-96, 98, 99; mission
// Behaviour 6), each applied while ord 4's chat is held at
// `model.call.observed`, each with its own entry for its `cells!` arm.
const OBSERVED: [&str; 1] = ["model.call.observed"];
/// W0b B2: U5's rows' chain, anchored at the charge, held at ord 4's
/// `model.call.observed`.
const U5: Chain = Chain {
    row: ("call_chat", "ingress"),
    anchor: Anchor::Mother("mother.charge.entry"),
    points: &OBSERVED,
};
/// Design 3.4: `quit` and `fatal` start the close deadline, so the chat hold
/// is released within this short post-shutdown pause budget (6.1).
const SHUTDOWN_PAUSE: Duration = Duration::from_secs(1);
/// SB3E-R86.2: the bounded wait for `door.cancel_run`'s acknowledgement,
/// `PATIENCE / 2`: shorter than the chat hold's own 10 s, so a missing one is
/// the cell's `cancel_run_ack`, never that hold's TIMEOUT abort. A slow host
/// can only fail the cell (fail closed).
const CANCEL_ACK_PATIENCE: Duration = Duration::from_secs(PATIENCE.as_secs() / 2);

/// Row 93: `engage_emergency()` on the `lifecycle_halt` view.
pub(super) fn original_cell_unscheduled(f: &Fixture) -> Facts {
    unscheduled(f, &U5, &REFUSED)
}
/// W0b B2: `original_cell_unscheduled` at `chain`'s last point, under `row`.
pub(super) fn unscheduled(f: &Fixture, chain: &Chain, row: &Expect) -> Facts {
    let value = "original_cell_unscheduled";
    let (mut facts, _, engaged) = driven(f, value, chain, row, |_, m, held, _| {
        m.halt.engage_emergency();
        drop(held);
        m.halt.is_engaged()
    });
    if !engaged {
        later(&mut facts, format!("{value}: the cell is not engaged"));
    }
    facts
}
/// Row 94: `emergency()` twice; both results and both applications (two
/// `ingress.emergency` entries of S) are recorded.
pub(super) fn repeated(f: &Fixture) -> Facts {
    repeat(f, &U5, &REFUSED)
}
/// W0b B2: `repeated` at `chain`'s last point, under `row`.
pub(super) fn repeat(f: &Fixture, chain: &Chain, row: &Expect) -> Facts {
    let (mut facts, _, results) = driven(f, "repeated", chain, row, |o, _, held, _| {
        let results = [o.ingress.emergency(), o.ingress.emergency()];
        drop(held);
        results.map(|r| format!("{:?}", r.map(|_| ())))
    });
    let mine = |e: &&s1::Entry| e.point == "ingress.emergency" && e.session == Some(facts.session);
    let applied = facts.broker.iter().filter(mine).count();
    println!("WITNESS repeated: {results:?}, {applied} ingress.emergency entries");
    if results != ["Ok(())", "Ok(())"] || applied != 2 {
        let found = format!("repeated: {results:?}, {applied} applications");
        later(&mut facts, found);
    }
    facts
}
/// Row 95: `FileSession::stop(<mission>)` (Behaviour 6.2).
pub(super) fn mission_stop(f: &Fixture) -> Facts {
    stop(f, "mission_stop", false, &U5, &REFUSED)
}
/// Row 96: `FileSession::stop_all()`, the File API's own path (`lite.rs:390`),
/// not `Coordinator::stop_all` (Behaviour 6.2).
pub(super) fn stop_all(f: &Fixture) -> Facts {
    stop(f, "stop_all", true, &U5, &REFUSED)
}
/// Row 98: `fail()`, within the short post-shutdown pause budget.
pub(super) fn fatal(f: &Fixture) -> Facts {
    shutdown(f, "fatal", &U5, &REFUSED, |o| {
        o.ingress.fail();
        Ok(())
    })
}
/// Row 99: `accept(CommandKind::Quit, 2)`, within the same budget.
pub(super) fn quit(f: &Fixture) -> Facts {
    shutdown(f, "quit", &U5, &REFUSED, |o| {
        let accepted = o.ingress.accept(CommandKind::Quit, 2);
        accepted.map(|_| ()).map_err(|e| format!("{e:?}"))
    })
}
/// 6.1: `apply`, then the chat released within `SHUTDOWN_PAUSE` (W0b B2: at
/// `chain`'s last point, under `row`).
pub(super) fn shutdown(
    f: &Fixture,
    value: &'static str,
    chain: &Chain,
    row: &Expect,
    apply: fn(&Opened) -> Result<(), String>,
) -> Facts {
    let (mut facts, _, (result, released)) = driven(f, value, chain, row, |o, _, held, _| {
        let at = Instant::now();
        let result = apply(o);
        drop(held);
        (result, at.elapsed())
    });
    println!("WITNESS {value}: {result:?}, released after {released:?}");
    if let Err(e) = result {
        later(&mut facts, format!("{value}: {e}"));
    }
    if released >= SHUTDOWN_PAUSE {
        let late = format!("{value}: released after {released:?}, not within {SHUTDOWN_PAUSE:?}");
        later(&mut facts, late);
    }
    facts
}

/// Behaviour 6.2 (R49.2), steps 2-6: `door.cancel_run` armed while the chat
/// is held; the Stop worker launched, its `ThreadId` recorded before it
/// starts work; its acknowledgement awaited (bounded); the chat released,
/// then the cancellation; the worker joined (7). Returns the
/// acknowledgement, if any, the worker's thread and its result.
fn stopping(
    f: &Fixture,
    m: &Mother,
    held: Hold,
    all: bool,
) -> (Option<s1::Entry>, ThreadId, String) {
    let cancel = f.pause("door.cancel_run");
    std::thread::scope(|s| {
        let (go, gone) = std::sync::mpsc::channel::<()>();
        let worker = s.spawn(move || {
            assert!(gone.recv_timeout(PATIENCE).is_ok(), "the Stop worker's go");
            match all {
                false => format!("{:?}", m.m.stop(m.mission())),
                true => format!("{:?}", m.m.stop_all()),
            }
        });
        let thread = worker.thread().id();
        go.send(()).unwrap();
        let ack = cancel.acknowledged.recv_timeout(CANCEL_ACK_PATIENCE).ok();
        drop(held);
        drop(cancel);
        (ack, thread, worker.join().unwrap())
    })
}
/// 6.2 step 4: the acknowledgement is `door.cancel_run`'s, of S, Q and RUN,
/// on the Stop worker's thread. It proves arrival, and by program order that
/// OwnerStop was engaged first; not that the cancellation completed.
fn cancel_ack(f: &Facts, ack: Option<&s1::Entry>, worker: ThreadId) -> Result<(), String> {
    let run = f.claim.map(|c| c.0);
    let ours = |a: &&s1::Entry| {
        (a.point, a.note, a.session, a.request)
            == ("door.cancel_run", false, Some(f.session), Some(f.request))
            && run.is_some()
            && a.run == run
            && a.thread == worker
    };
    match ack.filter(ours) {
        Some(_) => Ok(()),
        None => Err(format!(
            "cancel_run_ack: {:?}, the Stop worker {worker:?}",
            ack.map(|a| (a.point, a.request, a.run, a.thread))
        )),
    }
}
/// 6.2: the Stop cell, then step 7's checks: the chat worker refused
/// (`Err(Stopped)`), the Stop worker returned `Ok` with its state or count
/// (W0b B2: at `chain`'s last point, under `row`).
pub(super) fn stop(
    f: &Fixture,
    value: &'static str,
    all: bool,
    chain: &Chain,
    row: &Expect,
) -> Facts {
    let (mut facts, steps, (ack, worker, stopped)) =
        driven(f, value, chain, row, |_, m, held, _| {
            stopping(f, m, held, all)
        });
    println!("WITNESS {value}: the Stop worker {stopped}, its acknowledgement {ack:?}");
    let want = if all { "Ok(1)" } else { "Ok(Stopped)" };
    let refused = steps.last().map(String::as_str) == Some("Err(Stopped)");
    let findings = [
        cancel_ack(&facts, ack.as_ref(), worker).err(),
        (stopped != want).then(|| format!("{value}: the Stop worker returned {stopped}")),
        (!refused).then(|| format!("{value}: the chat worker returned {steps:?}")),
    ];
    for finding in findings.into_iter().flatten() {
        later(&mut facts, finding);
    }
    facts
}
/// R49.2 (P-STOP attribution controls): the measured acknowledgement is the
/// Stop worker's; checked against the chat worker's thread, or with another
/// run, it is each time `cancel_run_ack`.
#[test]
fn route_s5_cancel_run_ack_controls_each_refuse() {
    child("s5::route_s5_cancel_run_ack_controls_each_refuse", |f| {
        let (facts, _, ((ack, worker, _), chat)) =
            driven(f, "mission_stop", &U5, &REFUSED, |_, m, held, chat| {
                (stopping(f, m, held, false), chat)
            });
        assert!(facts.findings.is_empty(), "{:?}", facts.findings);
        assert_eq!(cancel_ack(&facts, ack.as_ref(), worker), Ok(()));
        let mut moved = ack.clone().expect("door.cancel_run acknowledged");
        moved.run = Some(RunId::parse(&format!("run-{}", "0".repeat(32))).unwrap());
        for (control, refused) in [
            (
                "the chat worker's thread",
                cancel_ack(&facts, ack.as_ref(), chat),
            ),
            ("another run", cancel_ack(&facts, Some(&moved), worker)),
        ] {
            assert!(
                refused
                    .as_ref()
                    .is_err_and(|e| e.starts_with("cancel_run_ack: ")),
                "{control}"
            );
            println!("WITNESS cancel_run_ack control {control:?}: {refused:?}");
        }
    });
}

/// Design 7.2 harness control: `quit` applied at ord 3's
/// `model.call.observed`, not ord 4's (S1 held from the call's first arrival
/// and chained to its third); `entry` must differ.
pub(super) fn ord3_quit(f: &Fixture) -> Facts {
    let ord3 = Chain {
        row: ("call_chat", "ingress"),
        anchor: Anchor::Arrival(3),
        points: &OBSERVED,
    };
    let (facts, _, quit) = driven(f, "quit", &ord3, &REFUSED, |o, _, held, _| {
        let quit = o.ingress.accept(CommandKind::Quit, 2).map(|_| ());
        drop(held);
        quit
    });
    println!("WITNESS ord3 quit {quit:?}");
    facts
}
/// Design 7.2 harness control: the `quit` cell with ATT bound to another
/// attempt; the identity fields must differ.
pub(super) fn quit_other_attempt(f: &Fixture) -> Facts {
    let mut facts = quit(f);
    facts.claim = facts.claim.map(|(run, attempt)| (run, attempt + 1));
    facts
}
/// Design 7.2 harness control: an earlier refusal, `quit` accepted before
/// the mission advances; `check` and `out` must differ.
pub(super) fn early_quit(f: &Fixture) -> Facts {
    let c = start(f);
    let quit = c.o.ingress.accept(CommandKind::Quit, 2).map(|_| ());
    let steps = c.mother.advance_to_call();
    println!("WITNESS early quit {quit:?}, the advance {steps:?}");
    collect((OBSERVED.to_vec(), Vec::new()), (U5.row, "quit"), &REFUSED)
}

/// Behaviour 7: the passive observations, read only once both workers are
/// joined: both traces (drained, never written), the journal through
/// `journal_view` only, the projection's records and the peer's slice. Then
/// the T6 child half (Behaviour 11) and `credit` (A.2-A.4) over them.
pub(super) fn collect(
    (points, acks): (Vec<&'static str>, Vec<Ack>),
    (cell, value): ((&'static str, &'static str), &'static str),
    row: &Expect,
) -> Facts {
    let (broker, trace) = (s1::drain_trace(), s5::drain_trace());
    let s = SOURCES
        .lock()
        .unwrap()
        .take()
        .expect("collect: start captured no observation");
    let (session, request, calls, recorded) = &s.driver;
    let recorded = recorded.lock().unwrap().clone();
    let made = calls.load(Ordering::SeqCst);
    let d = Driver {
        session: *session,
        request: *request,
        calls: made,
        recorded,
    };
    let credited = credit(&d, row, &broker, &trace);
    let claim = credited.as_ref().ok().and_then(|w| w.claim);
    let seams = t6(
        cfg!(mother_route_seams),
        d.session,
        &d,
        claim,
        &broker,
        &trace,
    );
    let mut findings = Vec::new();
    if let Err(finding) = seams {
        findings.push(format!("T6 child half: {finding}"));
    }
    if let Err(reason) = credited {
        findings.push(format!("credit: {reason}"));
    }
    let running = *RUNNING.lock().unwrap();
    if s.cell != running {
        findings.push(format!("collect: observed for {}, not {running}", s.cell));
    }
    let journal = journal_at(&s.journal).unwrap_or_else(|e| {
        findings.push(e);
        Vec::new()
    });
    let facts = Facts {
        session: d.session,
        request: d.request,
        recorded: d.recorded,
        claim,
        points,
        acks,
        value,
        row: cell,
        applied: APPLIED.lock().unwrap().take(),
        broker,
        mother: trace,
        journal: journal.get(s.since.0..).unwrap_or_default().to_vec(),
        native: s.native.records()[s.since.1..].to_vec(),
        peer: s.peer.lock().unwrap()[s.since.2..].to_vec(),
        findings,
    };
    KEPT.lock().unwrap().push(("collect", facts.clone()));
    facts
}

/// W0b D1: a broker-only opening for a cell with no Mother: the session with
/// its own `MemoryEvidence` projection, a fake peer (no model registered),
/// S1 armed and drained, and where the observations begin. Its request is
/// the session's Bootstrap request (0; main.rs `route_correlated_identity_is_recorded`).
pub(super) struct Broker {
    pub(super) o: Opened,
    pub(super) endpoint: String,
    native: Arc<MemoryEvidence>,
    peer: Arc<Mutex<Vec<String>>>,
    since: (usize, usize),
    _trace: s1::Held,
}
pub(super) fn open_broker(f: &Fixture) -> Broker {
    let (o, native) = opened(f);
    let (endpoint, peer) = fake_ollama();
    let _trace = s1::arm("reg.reserve", s1::Action::Trace);
    s1::drain_trace();
    let since = (native.records().len(), peer.lock().unwrap().len());
    Broker {
        o,
        endpoint,
        native,
        peer,
        since,
        _trace,
    }
}

/// W0b B3: the passive observations of a cell with no Mother (no journal, no
/// `credit`), read only once its workers are joined: the S1 trace (drained,
/// never written), the opening's projection records and peer slice since it
/// opened. Then T6's S1 half: `cfg` is true and the armed S1 instance
/// recorded this operation under the opening's session and request (D1).
pub(super) fn collect_broker(
    b: &Broker,
    (points, acks): (Vec<&'static str>, Vec<Ack>),
    (cell, value): ((&'static str, &'static str), &'static str),
) -> Facts {
    // W0b N-b: no s5 applier runs for a cell with no Mother, so nothing was
    // applied before an acknowledgement (`held`, never `armed`).
    let (broker, request, applied) = (s1::drain_trace(), 0, None);
    let session = b.o.session.identity();
    let mut findings = Vec::new();
    if let Err(finding) = t6_s1(cfg!(mother_route_seams), session, request, &broker) {
        findings.push(format!("T6 child half: {finding}"));
    }
    let facts = Facts {
        session,
        request,
        recorded: Vec::new(),
        claim: None,
        points,
        acks,
        value,
        row: cell,
        applied,
        broker,
        mother: Vec::new(),
        journal: Vec::new(),
        native: b.native.records()[b.since.0..].to_vec(),
        peer: b.peer.lock().unwrap()[b.since.1..].to_vec(),
        findings,
    };
    KEPT.lock().unwrap().push(("collect_broker", facts.clone()));
    facts
}
/// W0b B3: T6's S1 half for a cell with no Mother: `cfg`, then the armed S1
/// instance's entries of the bound session and request.
fn t6_s1(
    cfg: bool,
    session: SessionIdentity,
    request: u64,
    broker: &[s1::Entry],
) -> Result<(), &'static str> {
    if !cfg {
        return Err("cfg");
    }
    if broker.is_empty() {
        return Err("s1_inactive");
    }
    let ours = |e: &s1::Entry| e.session == Some(session) && e.request == Some(request);
    match broker.iter().any(ours) {
        true => Ok(()),
        false => Err("seam_identity"),
    }
}

/// Behaviour 11 (R49.5, R50.2, R54 U5-4): the T6 child half, on the first and
/// only counted call. The harness-armed S1 and S5 instances recorded this
/// call under its bound identity: ord 4's chat under S, Q, RUN, ATT, and the
/// call's `mother.charge.entry` under S, Q and the Builder's invocation, its
/// run the claim's. This shows participation, not the dependency closure
/// (A7's, which does not yet refuse a repeated `--extern`: F-A7-DUP).
fn t6(
    cfg: bool,
    session: SessionIdentity,
    d: &Driver,
    claim: Option<(RunId, u32)>,
    broker: &[s1::Entry],
    trace: &[s5::Entry],
) -> Result<(), &'static str> {
    if !cfg {
        return Err("cfg");
    }
    if d.calls != 1 {
        return Err("driver_calls");
    }
    if broker.is_empty() {
        return Err("s1_inactive");
    }
    if trace.is_empty() {
        return Err("s5_inactive");
    }
    let builder = d.recorded.iter().find(|r| r.0 == LaneRole::Builder);
    let builder = builder.map(|r| r.1);
    let chat = |e: &s1::Entry| {
        e.session == Some(session)
            && e.request == Some(d.request)
            && e.run.zip(e.attempt) == claim
            && e.kind == Some("chat")
            && e.request_ordinal == Some(4)
    };
    let charged = |e: &s5::Entry| {
        e.point == "mother.charge.entry"
            && e.session == Some(session)
            && e.request == Some(d.request)
            && e.invocation == builder
    };
    let bound = claim.is_some() && builder.is_some();
    if !bound || !broker.iter().any(chat) || !trace.iter().any(charged) {
        return Err("seam_identity");
    }
    Ok(())
}

/// W0b B3, D3: the broker-only collection on the registration route (no
/// Mother), through `open_broker`: the S1 trace of the bound session and the Bootstrap request (0), the
/// projection's registration record and the peer's slice, no finding; T6's S1
/// half refuses an unarmed S1, `cfg` false and another request.
#[test]
fn route_s5_collect_broker_on_registration() {
    child("s5::route_s5_collect_broker_on_registration", |f| {
        let b = open_broker(f);
        b.o.session
            .register_local_model(&b.endpoint, spec())
            .unwrap();
        let held = (Vec::new(), Vec::new());
        let facts = collect_broker(&b, held, (("registration_reserve", "ingress"), "none"));
        let points: Vec<&str> = facts.broker.iter().map(|e| e.point).collect();
        println!(
            "WITNESS collect_broker broker={} native={} peer={:?} points={points:?}",
            facts.broker.len(),
            facts.native.len(),
            facts.peer
        );
        assert!(facts.findings.is_empty(), "{:?}", facts.findings);
        assert!(!facts.native.is_empty() && !facts.peer.is_empty());
        let unarmed = t6_s1(true, facts.session, 0, &[]);
        assert_eq!(unarmed, Err("s1_inactive"), "S1 unarmed");
        assert_eq!(t6_s1(false, facts.session, 0, &facts.broker), Err("cfg"));
        let other = t6_s1(true, facts.session, 7, &facts.broker);
        assert_eq!(other, Err("seam_identity"), "another request");
    });
}

// SB3E-R117, R123 (U9a section M; R115.4): the d_close collection. A cell
// holds one S1 point, its sink's (`CLOSE_POINTS`); s5 reads the state and
// records when the value was applied (M4), opens the window at the hold's
// acknowledgement (M3), checks the sink's own result (M5) and collects (M2).
use nexus_governed_control::host_session::SessionRequest;

/// M4: each d_close sink's held point (close.rs:271, 232, 205 (the first
/// arrival), 524, 49, 367-370); only `closed` arms one.
const CLOSE_POINTS: [(&str, &str); 6] = [
    ("close_obligations", "close.obligations"),
    ("delivery", "close.delivery"),
    ("mother_end", "close.mother_end"),
    ("primary_end", "close.primary"),
    ("producer_join", "producer.join"),
    ("seal", "close.seal"),
];
/// M6: the opening, `opened` only (Bootstrap, h:141-144) or `start`'s Mother.
pub(super) enum Setup {
    Bootstrap,
    Mother,
}
/// M4: the value applied before the point is armed, or while it is held.
pub(super) enum Mode {
    Armed,
    Paused,
}
/// M5: what a section's closures reach; `mother` is taken by value for
/// `prepare_session_end` (R95.2).
pub(super) struct Close {
    pub(super) o: Opened,
    pub(super) mother: Option<Mother>,
}
/// M7: the Quit s5 accepted, the cell's Q (`close.seal` records it).
static CLOSE_Q: Mutex<Option<u64>> = Mutex::new(None);
impl Close {
    /// M7: `accept(Quit, 2)`, above the boot's Run 1 (h:1704-1709).
    pub(super) fn accept_quit(&self) -> Result<SessionRequest, String> {
        let quit = self.o.ingress.accept(CommandKind::Quit, 2);
        let quit = quit.map_err(|e| format!("{e:?}"))?;
        *CLOSE_Q.lock().unwrap() = Some(quit.sequence());
        Ok(quit)
    }
}
/// M6: `opened` and nothing else, both traces armed and drained; no Mother,
/// so no journal file, peer or counted call.
fn bootstrap(f: &Fixture) -> Close {
    let (o, native) = opened(f);
    let traces = (
        s1::arm("reg.reserve", s1::Action::Trace),
        s5::arm("mother.charge", s5::Action::Trace),
    );
    s1::drain_trace();
    s5::drain_trace();
    let (cell, since) = (*RUNNING.lock().unwrap(), (0, native.records().len(), 0));
    let driver = (o.session.identity(), 0, Arc::default(), Recorded::default());
    *SOURCES.lock().unwrap() = Some(Sources {
        cell,
        journal: f.0.join("home/mother-journal.jsonl"),
        native,
        peer: Arc::default(),
        driver,
        since,
        _traces: traces,
    });
    *APPLIED.lock().unwrap() = None;
    Close { o, mother: None }
}
/// M3: the window opens at the hold's acknowledgement, before any apply.
fn reopen() {
    let mut sources = SOURCES.lock().unwrap();
    let s = sources.as_mut().expect("closed: no sources");
    let journal = journal_at(&s.journal).map_or(0, |j| j.len());
    let (native, peer) = (s.native.records().len(), s.peer.lock().unwrap().len());
    s.since = (journal, native, peer);
}
/// M3-M6 (SB3E-R123): `prefix` runs, then s5 reads the state: `Armed`, it is
/// `value` and `APPLIED` is recorded before arming; `Paused`, it is not yet
/// `value` (S2, but for Q3's two rows). The sink runs on a worker held at its
/// point; at the (bounded) acknowledgement the window opens and `apply` runs;
/// `Paused`, the state is then `value` and `APPLIED` is recorded. A missing
/// acknowledgement, a late release after a close deadline, and a sink result
/// other than `want` (S1) are findings. A control (`none`) reads no state.
#[allow(clippy::too_many_arguments)]
pub(super) fn closed<S: Send, R: Send>(
    f: &Fixture,
    setup: Setup,
    row: (&'static str, &'static str),
    value: &'static str,
    mode: Mode,
    want: Result<(), &'static str>,
    prefix: impl FnOnce(&mut Close) -> S,
    sink: impl FnOnce(&Close, S) -> (Result<(), String>, R) + Send,
    apply: impl FnOnce(&Close),
) -> (Facts, R) {
    *CLOSE_Q.lock().unwrap() = None;
    let (mut c, journal) = match setup {
        Setup::Mother => (start(f).into(), true),
        Setup::Bootstrap => (bootstrap(f), false),
    };
    let point = CLOSE_POINTS.iter().find(|p| p.0 == row.0).map(|p| p.1);
    let point = point.expect("closed: a d_close sink");
    let s = prefix(&mut c);
    let state = |c: &Close| format!("{:?}", c.o.session.state());
    let (control, before, mut findings) = (value == "none", state(&c), Vec::new());
    // SB3E-R117.3 Q3: the values that precede every arrival at their point.
    let q3 = [
        ("close_obligations", "Bootstrap"),
        ("producer_join", "ClosingNormal"),
    ];
    match mode {
        _ if control => {}
        Mode::Armed if before == value => *APPLIED.lock().unwrap() = Some(Instant::now()),
        Mode::Armed => findings.push(format!("value: state {before}, not {value}")),
        Mode::Paused if before == value && !q3.contains(&(row.0, value)) => {
            findings.push(format!(
                "value: no transition, state {before} before the hold"
            ));
        }
        Mode::Paused => {}
    }
    let c = &c;
    let held = f.pause(point);
    let (after, (result, r)) = std::thread::scope(|scope| {
        let worker = scope.spawn(move || sink(c, s));
        let after = held.acknowledged.recv_timeout(PATIENCE).ok().map(|ack| {
            let acked = Instant::now();
            reopen();
            apply(c);
            let late = c.o.ingress.close_deadline().is_some() && acked.elapsed() >= SHUTDOWN_PAUSE;
            (ack, state(c), late.then(|| acked.elapsed()))
        });
        drop(held);
        (after, worker.join().unwrap())
    });
    let ack = match after {
        None => {
            findings.push(format!("closed: {point} not acknowledged"));
            None
        }
        Some((ack, after, late)) => {
            if let Some(late) = late {
                findings.push(format!(
                    "closed: released after {late:?}, not within {SHUTDOWN_PAUSE:?}"
                ));
            }
            match (mode, control, after == value) {
                (Mode::Paused, false, true) => *APPLIED.lock().unwrap() = Some(Instant::now()),
                (Mode::Paused, false, false) => {
                    findings.push(format!("value: state {after}, not {value}"))
                }
                _ => {}
            }
            println!(
                "WITNESS {}/{} {value}: state {before} before the hold, {after} after",
                row.0, row.1
            );
            Some(ack)
        }
    };
    let shown = |r: Result<(), &str>| r.map_or_else(|e| format!("Err({e})"), |()| "Ok".into());
    let got = shown(result.as_ref().map(|_| ()).map_err(String::as_str));
    println!("WITNESS {}/{} {value}: the sink {got}", row.0, row.1);
    if got != shown(want) {
        findings.push(format!(
            "closed: the sink returned {got}, not {}",
            shown(want)
        ));
    }
    let facts = collect_close(point, ack, journal, (row, value), findings);
    (facts, r)
}
impl From<Start> for Close {
    fn from(Start { o, mother }: Start) -> Self {
        let mother = Some(mother);
        Close { o, mother }
    }
}
/// M2, M3 (R100.2, R101.3: points are compared only with `CLOSE_POINTS`' and
/// the literal lists): the passive d_close collection once the worker is
/// joined: both traces, the journal (a Mother's, from its file), projection
/// and peer since the hold's acknowledgement, `credit_close` and `t6_close`.
/// Its Q is the Quit s5 accepted (M7), else the driver's request.
fn collect_close(
    point: &'static str,
    ack: Option<s1::Entry>,
    journal: bool,
    (cell, value): ((&'static str, &'static str), &'static str),
    mut findings: Vec<String>,
) -> Facts {
    let mother = s5::drain_trace();
    let broker = s1::drain_trace();
    let s = SOURCES.lock().unwrap().take();
    let s = s.expect("collect_close: no observation");
    let (session, request, calls, recorded) = &s.driver;
    let counted = calls.load(Ordering::SeqCst);
    if let Err(reason) = credit_close(counted, *session, &broker, &mother) {
        findings.push(format!("credit_close: {reason}"));
    }
    if let Err(reason) = t6_close(cfg!(mother_route_seams), *session, point, &broker) {
        findings.push(format!("T6 close: {reason}"));
    }
    let running = *RUNNING.lock().unwrap();
    if s.cell != running {
        findings.push(format!(
            "collect_close: observed for {}, not {running}",
            s.cell
        ));
    }
    let read = match journal {
        true => journal_at(&s.journal).map(|mut j| j.split_off(s.since.0.min(j.len()))),
        false => Ok(Vec::new()),
    };
    let window = read.unwrap_or_else(|e| {
        findings.push(e);
        Vec::new()
    });
    let native = s.native.records().split_off(s.since.1);
    let peer = s
        .peer
        .lock()
        .unwrap()
        .get(s.since.2..)
        .unwrap_or_default()
        .to_vec();
    let phases: Vec<&str> = native.iter().map(|r| r.phase.as_str()).collect();
    let events: Vec<String> = window.iter().map(|e| format!("{:?}", e.event)).collect();
    println!("WITNESS window: journal {events:?}, native {phases:?}, peer {peer:?}");
    let applied = APPLIED.lock().unwrap().take();
    let facts = Facts {
        session: *session,
        request: CLOSE_Q.lock().unwrap().take().unwrap_or(*request),
        recorded: recorded.lock().unwrap().clone(),
        claim: None,
        points: vec![point],
        acks: ack.into_iter().map(Ack::S1).collect(),
        value,
        row: cell,
        applied,
        broker,
        mother,
        journal: window,
        native,
        peer,
        findings,
    };
    KEPT.lock().unwrap().push(("collect_close", facts.clone()));
    facts
}
/// A.2 item 4's opening (s5.rs `credit`).
const ATTEMPT_OPENING: &str = "model.attempt.granted";
/// M2 (SB3E-R117.3 Q1): no model traffic on a d_close path: no counted call
/// (`drive_call`'s counter), S5 attempt site, opening, broker binding entry
/// or refusal note, and no S5 entry of another session.
fn credit_close(
    calls: u32,
    session: SessionIdentity,
    broker: &[s1::Entry],
    mother: &[s5::Entry],
) -> Result<(), &'static str> {
    let attempt = mother.iter().any(|m| ATTEMPT.contains(&m.point));
    let opening = broker.iter().any(|b| b.point == ATTEMPT_OPENING);
    let foreign = mother.iter().any(|m| m.session != Some(session));
    match () {
        _ if calls != 0 => Err("driver_calls"),
        _ if attempt => Err("attempt_site"),
        _ if opening => Err("opening"),
        _ if broker.iter().any(binding) => Err("binding"),
        _ if foreign => Err("session"),
        _ => Ok(()),
    }
}
/// M2: T6 for a d_close cell: `cfg`, the armed S1 instance, and the row's
/// point entered for the bound session.
fn t6_close(
    cfg: bool,
    session: SessionIdentity,
    point: &str,
    broker: &[s1::Entry],
) -> Result<(), &'static str> {
    let entered = |e: &s1::Entry| !e.note && e.point == point && e.session == Some(session);
    match () {
        _ if !cfg => Err("cfg"),
        _ if broker.is_empty() => Err("s1_inactive"),
        _ if !broker.iter().any(entered) => Err("seam_identity"),
        _ => Ok(()),
    }
}
/// The prefix to the seal (d2:185-214): Quit, the Mother end prepared (its
/// type inferred), resources and authority settled; counts of the journal
/// and projection records before and after it.
type Counts = ((usize, usize), (usize, usize));
fn to_the_seal(c: &mut Close) -> (SessionRequest, Counts) {
    let counts = || {
        let s = SOURCES.lock().unwrap();
        let s = s.as_ref().unwrap();
        (
            journal_at(&s.journal).unwrap().len(),
            s.native.records().len(),
        )
    };
    let (before, quit) = (counts(), c.accept_quit().unwrap());
    drop(c.mother.take().unwrap().m.prepare_session_end().unwrap());
    c.o.session.settle_close_resources(&quit).unwrap();
    c.o.session.settle_close_authority(&quit).unwrap();
    (quit, (before, counts()))
}
/// A real d_close control run to the seal (`ctl_seal`'s path).
fn sealed(f: &Fixture) -> (Facts, Counts) {
    let sink = |c: &Close, (quit, counts): (SessionRequest, Counts)| {
        let sealed = c.o.session.seal_close(&quit).map(|_| ());
        (sealed.map_err(|e| format!("{e:?}")), counts)
    };
    let (row, mode) = (("seal", "lifecycle"), Mode::Paused);
    closed(
        f,
        Setup::Mother,
        row,
        "none",
        mode,
        Ok(()),
        to_the_seal,
        sink,
        |_| {},
    )
}
/// M2 (rule level, over a real d_close run's facts): each rule of
/// `credit_close` and `t6_close` refuses for its own reason; the run credits.
#[test]
fn route_s5_close_credit_plants_each_refuse() {
    child("s5::route_s5_close_credit_plants_each_refuse", |f| {
        let (facts, _) = sealed(f);
        assert!(facts.findings.is_empty(), "{:?}", facts.findings);
        let (s, b, m) = (facts.session, facts.broker.clone(), facts.mother.clone());
        let other = SessionSeed::new().unwrap().identity();
        let e1 = |point, note| {
            let (mut b, mut e) = (b.clone(), b[0].clone());
            (e.point, e.note) = (point, note);
            b.push(e);
            b
        };
        let e5 = |point, session| {
            let (tick, request, run, invocation) = (1, Some(facts.request), None, None);
            let (thread, instant) = (std::thread::current().id(), Instant::now());
            let session = Some(session);
            vec![s5::Entry {
                tick,
                point,
                session,
                request,
                run,
                invocation,
                thread,
                instant,
            }]
        };
        let cc = |calls, b: &[s1::Entry], m: &[s5::Entry]| credit_close(calls, s, b, m);
        let t6 = |cfg, session, point, b: &[s1::Entry]| t6_close(cfg, session, point, b);
        let seal = "close.seal";
        assert_eq!(cc(0, &b, &m).and(t6(true, s, seal, &b)), Ok(()));
        for (refused, want) in [
            (cc(1, &b, &m), "driver_calls"),
            (cc(0, &b, &e5("mother.charge", s)), "attempt_site"),
            (cc(0, &e1(ATTEMPT_OPENING, false), &m), "opening"),
            (cc(0, &e1("call.gate", false), &m), "binding"),
            (cc(0, &e1("refused:8.cas", true), &m), "binding"),
            (cc(0, &b, &e5("mother.admit", other)), "session"),
            (t6(false, s, seal, &b), "cfg"),
            (t6(true, s, seal, &[]), "s1_inactive"),
            (t6(true, s, "close.delivery", &b), "seam_identity"),
            (t6(true, other, seal, &b), "seam_identity"),
        ] {
            assert_eq!(refused, Err(want));
            println!("WITNESS close credit plant refused {want:?}");
        }
    });
}
/// M3: the window opens at the hold's acknowledgement: the prefix wrote
/// journal and projection records, and none of them is in the window.
#[test]
fn route_s5_close_window_opens_at_the_hold() {
    child("s5::route_s5_close_window_opens_at_the_hold", |f| {
        let (facts, (before, after)) = sealed(f);
        assert!(facts.findings.is_empty(), "{:?}", facts.findings);
        assert!(
            after.0 > before.0 && after.1 > before.1,
            "{before:?} {after:?}"
        );
        let (journal, native) = (&facts.journal, &facts.native);
        assert!(
            journal.is_empty() && native.is_empty(),
            "the window: {native:?}"
        );
        println!("WITNESS close window: prefix records {before:?} -> {after:?}, window empty");
    });
}
