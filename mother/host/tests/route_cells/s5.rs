//! U4a-2 S5 (design 4.4; AM-2 Part A, revision 4.2). Every cell with an S5
//! witness runs in this module (A.2 item 1). Its only model call is in
//! `drive_call` (A.2 item 2); the `SessionPort` is moved into
//! `Coordinator::open` and never cloned (A.2 item 3 (c)); every scripted
//! lane is a driver-owned `RecordingLane` (A.4 item 3). `credit` is the A.2
//! item 4/5, A.3 and A.4 check over the driver's own facts and both traces.
use super::*;
use nexus_governed_control::broker::route_seams as s1;
use nexus_governed_control::host_session::{SessionContext, SessionIdentity};
use nexus_mother::adapter::{FileJournal, FilePrivate, SessionPort};
use nexus_mother::contract::{Budget, MissionContract, ModelChoice, PassCriterion};
use nexus_mother::coordinator::{Coordinator, FileSession, Lanes};
use nexus_mother::journal::MissionId;
use nexus_mother::lane::{InvocationId, Lane, LaneContext, LaneIdentity, LaneInput, LaneOutput};
use nexus_mother::lane::{LaneRole, ScriptedLane, Verdict};
use nexus_mother::route_seams as s5;

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
}
pub(super) fn boot(f: &Fixture, o: &Opened) -> Boot {
    o.session
        .register_local_model(&fake_ollama().0, spec())
        .unwrap();
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
    if d.calls != 1 {
        return Err("driver_calls");
    }
    if mother.iter().any(|m| m.session != Some(d.session)) {
        return Err("session");
    }
    if mother.iter().any(|m| m.request != Some(d.request)) {
        return Err("request");
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
    let mut runs: Vec<RunId> = runs.filter_map(|b| b.run).collect();
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
    let claim = openings
        .first()
        .map(|o| (o.run.unwrap(), o.attempt.unwrap()));
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
    // A.4 item 3: invocations, per invocation.
    let recorded = |i: &InvocationId| d.recorded.iter().any(|r| r.1 == *i);
    if mother
        .iter()
        .filter_map(|m| m.invocation)
        .any(|i| !recorded(&i))
    {
        return Err("unrecorded_invocation");
    }
    let model = d.recorded.iter().filter(|r| r.0 == LaneRole::Builder);
    let model: Vec<InvocationId> = model.map(|r| r.1).collect();
    let attempting = mother.iter().filter(|m| ATTEMPT.contains(&m.point));
    if attempting
        .filter_map(|m| m.invocation)
        .any(|i| model != [i])
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
    let mut classify = mother.iter().filter(|m| m.point == "mother.classify");
    if classify.any(|m| m.run != run || m.invocation.is_some()) {
        return Err("classify_run");
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
    if e.profile.call_gate == 0 && e.request_level {
        return Err("request_level");
    }
    // A.4 item 6: position, evidence only, through the run-to-session map.
    if let Some((site, before, after)) = e.position {
        let at = mother
            .iter()
            .find(|m| m.point == site)
            .ok_or("position_site")?;
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
    }
    Ok(Witness { run, claim, sites })
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
        let model = driver.recorded.iter().find(|r| r.0 == LaneRole::Builder);
        let model = model.map(|r| r.1);
        let mut points: Vec<&str> = witness.sites.iter().map(|s| s.0).collect();
        points.sort_unstable();
        points.dedup();
        assert_eq!(points.len(), 10, "every S5 site: {points:?}");
        for (site, invocation, claim, compared) in &witness.sites {
            let attempt = ATTEMPT.contains(site);
            let expected = match *site {
                "mother.classify" => (None, Some((run, 1)), Some(true)),
                _ if attempt => (model, Some((run, 1)), Some(true)),
                _ => (*invocation, None, None),
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
        let plants: [Plant; 26] = [
            ("two calls", "driver_calls", |c| c.d.calls = 2),
            ("no call with S5 entries", "driver_calls", |c| c.d.calls = 0),
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
                c.e.profile.bound = false
            }),
            ("opening on bound: no", "opening_on_unbound", |c| {
                c.b.retain(|x| !binding(x));
                c.e.profile.bound = false;
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
            ("session-only made to claim", "per_invocation", |c| {
                let (point, builder) = ("mother.lane.prelock", Some(c.builder));
                let at =
                    c.m.iter()
                        .position(|e| e.point == point && e.invocation == builder);
                c.m[at.unwrap()].point = "mother.charge.ok";
            }),
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
                c.m.retain(|e| e.point != "mother.charge.entry")
            }),
            ("tie", "tie", |c| {
                let instant = c.m("mother.charge.entry").instant;
                let gate = c.b.iter().rposition(|e| e.point == "call.gate").unwrap();
                c.b[gate].instant = instant;
            }),
            ("wrong invocation, classify", "classify_run", |c| {
                c.m("mother.classify").invocation = Some(c.builder)
            }),
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
fn timeout(line: &str) -> ! {
    use std::io::Write;
    let root = PathBuf::from(std::env::var_os(CHILD).unwrap_or_default());
    let mut open = std::fs::OpenOptions::new();
    if let Ok(mut result) = open.create(true).append(true).open(root.join("result")) {
        let _ = writeln!(result, "{line}");
    }
    std::process::abort()
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
            let held_at = journaled();
            std::thread::sleep(Duration::from_millis(200)); // Must stay held.
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
