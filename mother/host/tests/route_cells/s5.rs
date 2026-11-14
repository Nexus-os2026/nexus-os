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
struct Mother {
    m: FileSession<SessionPort>,
    id: MissionId,
    calls: Arc<AtomicU32>,
    recorded: Recorded,
    request: u64,
    /// The request context (not the port or door): S4b's halt is read here.
    context: SessionContext,
    peer: Arc<Mutex<Vec<String>>>,
}
fn mother(f: &Fixture, o: &Opened) -> Mother {
    let b = boot(f, o);
    let (calls, recorded) = (Arc::default(), Recorded::default());
    let journal = FileJournal::open(&f.0.join("mother-journal.jsonl")).unwrap();
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
struct Profile {
    opening: usize,
    recheck: usize,
    classify: usize,
    /// The claimed attempt's `call.gate` entries.
    call_gate: usize,
    /// Whether a broker session-binding entry exists.
    bound: bool,
}
type Neighbour = (&'static str, Option<u64>);
/// An expect row's S5 fields.
#[derive(Clone, Copy)]
struct Expect {
    attempts: usize,
    profile: Profile,
    /// Each session-only site's count per recorded invocation (A.4 item 3).
    per_invocation: [(&'static str, usize); 4],
    /// Whether the cell credits a request-level property (A.4 item 5).
    request_level: bool,
    /// Whether it credits an attempt-level property (never on `bound: no`).
    attempt_level: bool,
    /// The multiplicity of each attempt site the profile does not count.
    multiplicity: [(&'static str, usize); 4],
    /// An S5 site and its S1 neighbours by instant, as (point, ordinal).
    position: Option<(&'static str, Neighbour, Neighbour)>,
}
/// The recorded path's measured row (A.6).
const RECORDED: Expect = Expect {
    attempts: 1,
    profile: Profile {
        opening: 1,
        recheck: 1,
        classify: 1,
        call_gate: 4,
        bound: true,
    },
    per_invocation: [
        ("mother.lane.prelock", 1),
        ("mother.admit", 1),
        ("mother.admit.ok", 1),
        ("mother.lane.returned", 1),
    ],
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
    for (_, invocation) in &d.recorded {
        for (site, n) in e.per_invocation {
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
        let plants: [Plant; 39] = [
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
pub(super) struct Facts {
    pub(super) session: SessionIdentity,
    pub(super) request: u64,
    pub(super) recorded: Vec<(LaneRole, InvocationId)>,
    /// `credit`'s claim (A.3): the session-bound run and the opening's attempt.
    pub(super) claim: Option<(RunId, u32)>,
    /// The held points' acknowledged entries, in hold order.
    pub(super) acks: Vec<s1::Entry>,
    /// The value applied while the last point was held (`none`: a control).
    pub(super) value: &'static str,
    pub(super) broker: Vec<s1::Entry>,
    pub(super) mother: Vec<s5::Entry>,
    /// The journal's entries since the hold began, through `journal_view`.
    pub(super) journal: Vec<Journaled>,
    /// The projection's records since the hold began.
    pub(super) native: Vec<EvidenceRecord>,
    /// The peer's requests since the hold began, in served order.
    pub(super) peer: Vec<String>,
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
        let witness = render(&drive(&f));
        let nonce = std::env::var(NONCE).expect("the parent's nonce");
        let mut append = std::fs::OpenOptions::new();
        let mut result = append.append(true).open(f.0.join("result")).unwrap();
        let line = format!("CELL {nonce} {id} {witness}\n");
        std::io::Write::write_all(&mut result, line.as_bytes()).unwrap();
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

/// The chained hold (Behaviour 6; no new accessor). The Builder's advance
/// runs on a worker, held at S5 `mother.charge.entry` (between ord 3's
/// `model.call.admitted` and ord 4's `call.gate`); each of `points` is armed
/// while the hold before it stands, and that hold is then released, so each
/// acknowledgement is the chat's (ord 4). Every wait is a bounded receive
/// (`PATIENCE`); the last hold is released before the worker is joined.
/// Returns the S1 acknowledgements, in order, and the advance's steps.
fn chained(f: &Fixture, m: &Mother, points: &[&'static str]) -> (Vec<s1::Entry>, Vec<String>) {
    let (cell, timeout) = ("u5", timeout as s5::Timeout);
    let entry = s5::arm("mother.charge.entry", s5::Action::Pause { cell, timeout });
    std::thread::scope(|s| {
        let advancing = s.spawn(|| m.advance_to_call());
        let reached = entry.acknowledged.recv_timeout(PATIENCE);
        assert!(reached.is_ok(), "mother.charge.entry: not acknowledged");
        let (mut first, mut before, mut acks) = (Some(entry), None, Vec::new());
        for point in points {
            let held = f.pause(point);
            drop((first.take(), before.take()));
            let Ok(ack) = held.acknowledged.recv_timeout(PATIENCE) else {
                panic!("{point}: not acknowledged");
            };
            acks.push(ack);
            before = Some(held);
        }
        drop(before);
        (acks, advancing.join().unwrap())
    })
}

/// Behaviour 10: `ctl_call_chat`'s driver. Held at `call.gate` and then at
/// `model.call.observed` on ord 4, released with no value, the chat reaches
/// `model.call.admitted` and a recorded outcome.
pub(super) fn ctl_call_chat(f: &Fixture) -> Facts {
    let (o, native) = opened(f);
    let mother = mother(f, &o);
    let _s1 = s1::arm("reg.reserve", s1::Action::Trace);
    let _s5 = s5::arm("mother.charge", s5::Action::Trace);
    s1::drain_trace();
    s5::drain_trace();
    let since = (
        mother.m.journal_view().unwrap().len(),
        native.records().len(),
        mother.peer.lock().unwrap().len(),
    );
    let (acks, steps) = chained(f, &mother, &["call.gate", "model.call.observed"]);
    // The T6 child half and `credit` judge the run first (`collect`).
    let facts = collect(&o, &mother, &native, since, acks, "none");
    assert!(
        steps.iter().all(|s| s == "Ok(())"),
        "the advance: {steps:?}"
    );
    facts
}

/// Behaviour 7: the passive observations, read only once both workers are
/// joined: both traces (drained, never written), the journal through
/// `journal_view` only, the projection's records and the peer's slice. Then
/// the T6 child half (Behaviour 11) and `credit` (A.2-A.4) over them.
fn collect(
    o: &Opened,
    m: &Mother,
    native: &MemoryEvidence,
    since: (usize, usize, usize),
    acks: Vec<s1::Entry>,
    value: &'static str,
) -> Facts {
    let (broker, trace) = (s1::drain_trace(), s5::drain_trace());
    let d = m.driver(o);
    let credited = credit(&d, &RECORDED, &broker, &trace);
    let claim = credited.as_ref().ok().and_then(|w| w.claim);
    let seams = t6(
        cfg!(mother_route_seams),
        d.session,
        &d,
        claim,
        &broker,
        &trace,
    );
    if let Err(finding) = seams {
        panic!("T6 child half: {finding}");
    }
    if let Err(reason) = credited {
        panic!("credit: {reason}");
    }
    Facts {
        session: d.session,
        request: d.request,
        recorded: d.recorded,
        claim,
        acks,
        value,
        broker,
        mother: trace,
        journal: m.m.journal_view().unwrap()[since.0..].to_vec(),
        native: native.records()[since.1..].to_vec(),
        peer: m.peer.lock().unwrap()[since.2..].to_vec(),
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
