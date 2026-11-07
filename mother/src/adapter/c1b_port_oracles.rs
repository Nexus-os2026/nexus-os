//! M2e-C1b-1 oracles (port and adapter), pre-registered; N on base d4338fb0
//! (the port's model methods, record classification and model classes are
//! missing). Real ordinary broker + Mother's fake Ollama, one child per case
//! (one broker per process). The coordinator stays unwired (asserted).
//! Plant "ignore ModelCallOutcome.record" -> c1_s3 (classify; port owed and
//! uncertain). The `#[cfg(test)]` record seam `GovernedPort::model_record`
//! substitutes Owed/Uncertain for one real Recorded attempt: the broker's
//! store fault hooks are crate-private, and its own `pr75_r1_m1_b` proves
//! real Owed/Uncertain production.
use super::c1b_fake_ollama::{chat, spec, Answer, FakeOllama, DIGEST, NAME};
use super::m2c_support::{port_for, Home};
use super::{classify, create_private, GovernedPort};
use crate::contract::{ContractError, MissionContract, ModelChoice};
use crate::coordinator::{Coordinator, CoordinatorError, FileSession, Lanes};
use crate::effect::{EffectPort, ModelAttempt, RecordDebt};
use crate::evidence::{BlockReason, BrokerClass};
use crate::journal::{MemoryJournal, MissionId};
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole};
use crate::oracles::lite_support::FilesOwner;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, keyed_lanes, Recording};
use crate::state::MissionState;
use nexus_governed_control::authority::approval::MissionConfirmation;
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::governed::GrantRequest;
use nexus_governed_control::inference::{
    CallDeadline, CallMeter, Charged, MeterRefusal, ModelError, ModelIdentity, ModelRequest,
    ResponseSchema,
};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CASE: &str = "NEXUS_M2E_C1B1_CASE";
const REPLY: &str = "C1B_REPLY_CANARY";

/// Each case in its own child (one broker per process).
fn port_children(test: &str, cases: &[&str]) {
    children("adapter::c1b_port_oracles", CASE, test, cases, run);
}
pub(crate) fn children(module: &str, variable: &str, test: &str, cases: &[&str], run: fn(&str)) {
    if let Ok(case) = std::env::var(variable) {
        run(&case);
        println!("case-ok:{case}");
        return;
    }
    let mut failures = Vec::new();
    for case in cases {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("{module}::{test}"), "--nocapture"])
            .env(variable, case)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let read = |mut pipe: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut text = String::new();
                let _ = pipe.read_to_string(&mut text);
                text
            })
        };
        let out = read(Box::new(child.stdout.take().unwrap()));
        let err = read(Box::new(child.stderr.take().unwrap()));
        // A bounded wait for the child's exit (not a success condition).
        let deadline = Instant::now() + Duration::from_secs(180);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() > deadline {
                child
                    .kill()
                    .unwrap_or_else(|e| panic!("{case}: kill owned child: {e}"));
                child
                    .wait()
                    .unwrap_or_else(|e| panic!("{case}: reap owned child: {e}"));
                break None;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let (out, err) = (out.join().unwrap(), err.join().unwrap());
        if !status.is_some_and(|s| s.success()) || !out.contains(&format!("case-ok:{case}")) {
            failures.push(format!("{case}: {status:?}\n{out}\n{err}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n---\n"));
}

fn run(case: &str) {
    let (name, arg) = case.split_once('/').unwrap_or((case, ""));
    match name {
        "session" => session_denies(),
        "consent" => consent(arg),
        "attempt" => attempt(arg),
        "identity" => identity(arg),
        "closed" => closed(),
        other => panic!("unknown case {other}"),
    }
}

/// A Builder with a model cap. The coordinator never invokes it here.
struct ModelLane(Arc<AtomicUsize>);
impl Lane for ModelLane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Builder,
            name: "c1b-model".into(),
            version: "1".into(),
        }
    }
    fn max_model_calls(&self) -> u32 {
        2
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("no model lane runs in C1b-1")
    }
}
fn model_lanes(invoked: &Arc<AtomicUsize>) -> Vec<Arc<dyn Lane>> {
    let mut lanes: Vec<Arc<dyn Lane>> = keyed_lanes()
        .into_iter()
        .filter(|l| l.identity().role != LaneRole::Builder)
        .collect();
    lanes.push(Arc::new(ModelLane(invoked.clone())));
    lanes
}

fn request() -> ModelRequest {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    ModelRequest::new("C1B system".into(), "C1B prompt".into(), schema).unwrap()
}
fn identity_bytes() -> [u8; 32] {
    ModelIdentity::from_spec(&spec()).unwrap().digest()
}
fn model_choice() -> ModelChoice {
    ModelChoice {
        name: NAME.into(),
        digest: DIGEST.into(),
        identity: hex::encode(identity_bytes()),
    }
}

/// Assigns indices from 1 and keeps what was settled.
#[derive(Default)]
struct Meter {
    charged: AtomicU32,
    settled: Mutex<Vec<u32>>,
}
impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        Ok(Charged {
            call: self.charged.fetch_add(1, Ordering::SeqCst) + 1,
        })
    }
    fn settle(&self, call: u32) {
        self.settled.lock().unwrap().push(call);
    }
}
struct Never;
impl CallDeadline for Never {
    fn expired(&self) -> bool {
        false
    }
}

pub(crate) struct Fx {
    pub(crate) home: Home,
    pub(crate) fake: FakeOllama,
    pub(crate) port: Arc<GovernedPort>,
    pub(crate) folder: String,
}
impl Fx {
    pub(crate) fn with(register: bool) -> Self {
        assert!(nexus_governed_control::files::make_non_dumpable());
        let home = Home::fresh_temp("c1b1").unwrap();
        let workspace = home.dir.join("workspace");
        create_private(&workspace).unwrap();
        let info = home.control.register_workspace(&workspace, &[]).unwrap();
        home.control
            .request_grant(
                &GrantRequest::Files { workspace: info.id },
                Duration::from_secs(86400),
                &FilesOwner::default(),
            )
            .unwrap();
        let fake = FakeOllama::start();
        if register {
            let view = home
                .control
                .register_local_model(&fake.endpoint(), spec())
                .unwrap();
            let choice = model_choice();
            assert_eq!(
                (view.name, view.digest, view.identity),
                (choice.name, choice.digest, choice.identity),
                "the registered view equals the independently computed identity"
            );
        }
        let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
        Self {
            home,
            fake,
            port,
            folder: info.name,
        }
    }
    fn mother(&self, lanes: Vec<Arc<dyn Lane>>) -> FileSession<GovernedPort> {
        Coordinator::open(
            MemoryJournal::new(),
            self.port.clone(),
            Lanes::new(lanes).unwrap(),
            Arc::new(ManualClock::default()),
            self.home.private(),
        )
        .unwrap()
        .into_file_session()
        .unwrap()
    }
    fn contract(&self) -> MissionContract {
        let mut c = contract_with("");
        c.requested_folder = Some(self.folder.clone());
        c.model = Some(model_choice());
        c.deliverable = Some("index.html".into());
        c
    }
    /// Consented (the native request kept) and advanced once, so the
    /// mission's File run exists. Scripted lanes only.
    fn consented(&self, m: &FileSession<GovernedPort>) -> (MissionId, MissionConfirmation) {
        let id = m.draft(self.contract()).unwrap();
        m.submit(id).unwrap();
        let owner = Recording::default();
        assert_eq!(
            answering(&owner, || m.request_consent(id)),
            Ok(MissionState::Approved)
        );
        m.advance(id).unwrap();
        assert!(self.port.run_of(id).is_some(), "the mission's File run");
        let asked = owner.0.lock().unwrap()[0].clone();
        (id, asked)
    }
    fn attempts(&self) -> usize {
        self.home.gate.count("model_call")
    }
}
fn admit(
    port: &GovernedPort,
    id: MissionId,
    asked: &MissionConfirmation,
    request: &str,
    identity: [u8; 32],
) -> Result<(), BrokerClass> {
    port.admit_inference(id, &asked.contract_hash, request, identity)
        .map_err(|r| r.class)
}

// ---------------------------------------------------------------- C1-S1

/// The sealed session keeps inference default-deny: no identity, no
/// admission, no attempt (nothing charged).
#[test]
fn c1_s1_sealed_session_denies_inference() {
    port_children("c1_s1_sealed_session_denies_inference", &["session"]);
}
fn session_denies() {
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = super::session_support::Fixture::new("c1b1-s1");
    let port = home.port.clone();
    assert!(port.model_identity().is_none(), "no model identity");
    assert_eq!(
        port.admit_inference(MissionId(1), &"ab".repeat(32), "request", identity_bytes())
            .map_err(|r| r.class),
        Err(BrokerClass::NoBrokerRoute)
    );
    let meter = Meter::default();
    assert_eq!(
        port.model_call(MissionId(1), &request(), &meter, &Never)
            .map(|_| ())
            .map_err(|r| r.class),
        Err(BrokerClass::NoBrokerRoute)
    );
    assert_eq!(
        meter.charged.load(Ordering::SeqCst),
        0,
        "nothing was charged"
    );
}

/// Static seal: the host session facade names no model or inference route;
/// SessionPort keeps the three model methods' default denial; the session
/// door's arms refuse.
fn sb2_public_model_item(line: &str) -> bool {
    line.starts_with("pub ")
        && line != "pub struct ModelObligations {"
        && !line.starts_with("pub model: ModelObligations,")
        && {
            let lower = line.to_ascii_lowercase();
            lower.contains("model") || lower.contains("inference")
        }
}
fn sb2_facade_line(line: &str) -> bool {
    (!line.contains("inference")
        && !line.contains("model_")
        && !line
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|w| w == "model"))
        || matches!(
            line,
            "mod model;" | "mod model_tests;" | "mod model_death_tests;"
                // Existing private Core storage and its initialization only.
                | "model: model::ModelSlot," | "model: model::ModelSlot::default(),"
                | "if !self.core.model.ready()? {"
        )
}
#[test]
fn sb2_facade_rejects_multiline_registration() {
    let plant = "pub fn register_local(\n&self,\nspec: &crate::inference::ModelPinSpec,\n) -> Result<crate::inference::ModelIdentityView, _> { self.register_model_private(..) }";
    assert!(!plant.lines().map(str::trim).all(sb2_facade_line));
    assert!([
        "mod model;",
        "mod model_tests;",
        "mod model_death_tests;",
        "pub fn status(&self) {}"
    ]
    .into_iter()
    .all(sb2_facade_line));
}
#[test]
fn sb2_facade_rejects_model_module_aliases() {
    for plant in [
        "use model as hidden;",
        "use self::model as hidden;",
        "use super::{model as hidden};",
    ] {
        assert!(
            !sb2_facade_line(plant),
            "reject model module alias: {plant}"
        );
    }
}
#[test]
fn c1_s1_session_sources_have_no_model_route() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    fn walk(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, files);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let facade = "crates/nexus-governed-control/src/host_session.rs";
    let mut files = vec![root.join(facade)];
    walk(
        &root.join("crates/nexus-governed-control/src/host_session"),
        &mut files,
    );
    let private = [
        "crates/nexus-governed-control/src/host_session/model.rs",
        "crates/nexus-governed-control/src/host_session/close.rs",
        "crates/nexus-governed-control/src/host_session/model_tests.rs",
        "crates/nexus-governed-control/src/host_session/model_death_tests.rs",
    ];
    assert!(files.len() > 1);
    assert!(sb2_public_model_item(
        "pub fn model_call(&self, id: u64) {}"
    ));
    assert!(!sb2_public_model_item(
        "pub(crate) fn model_call(id: u64) {}"
    ));
    assert!(!sb2_public_model_item("pub model: ModelObligations,"));
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let permitted = private.iter().any(|p| path == root.join(p));
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with("//")) {
            if path == root.join(facade) {
                assert!(sb2_facade_line(line), "facade: {line}");
            } else if !permitted {
                assert!(
                    !line.contains("inference") && !line.contains("model_"),
                    "{}: {line}",
                    path.display()
                );
            }
            // C1-S1-SB2: private machinery and content-free close fields only.
            assert!(
                !sb2_public_model_item(line),
                "public model route: {}: {line}",
                path.display()
            );
        }
    }
    let port = std::fs::read_to_string(root.join("mother/src/adapter/session_port.rs")).unwrap();
    let block = port
        .split("impl EffectPort for SessionPort")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(block.contains("fn withdraw"), "the impl block was found");
    for method in ["fn model_identity", "fn admit_inference", "fn model_call"] {
        assert!(!block.contains(method), "SessionPort overrides {method}");
    }
    for (method, refusal) in [
        ("fn model_identity(", "Self::Session(_) => None"),
        (
            "fn admit_inference(",
            "Self::Session(_) => Err(AuthorityError::Unavailable(",
        ),
        (
            "fn model_call(",
            "Self::Session(_) => Err(AuthorityError::Unavailable(",
        ),
    ] {
        let body = port
            .split(&format!("pub(super) {method}"))
            .nth(1)
            .unwrap_or_else(|| panic!("PortDoor {method}"))
            .split("pub(super) fn ")
            .next()
            .unwrap();
        assert!(
            body.contains(refusal),
            "PortDoor {method}: the session arm refuses"
        );
    }
}

// ---------------------------------------------------------------- C1-S2

/// At the port: the native admission binds the mission's original request,
/// hash and existing run once. The same admission is idempotent; a changed
/// binding, another door (no run is opened), a cancelled run and a Stop
/// before installation refuse. The ordinary door exposes the identity.
#[test]
fn c1_s2_native_consent_binds_once_at_the_port() {
    port_children(
        "c1_s2_native_consent_binds_once_at_the_port",
        &["consent/bound", "consent/stop-first"],
    );
}
fn consent(arg: &str) {
    let f = Fx::with(true);
    let m = f.mother(keyed_lanes());
    let (id, asked) = f.consented(&m);
    let id_bytes = identity_bytes();
    if arg == "stop-first" {
        assert_eq!(m.stop(id), Ok(MissionState::Stopped));
        assert_eq!(
            admit(&f.port, id, &asked, &asked.request, id_bytes),
            Err(BrokerClass::Halted)
        );
        assert_eq!(f.attempts(), 0);
        return;
    }
    let view = f
        .port
        .model_identity()
        .expect("the ordinary door's identity");
    assert_eq!(view.identity, hex::encode(id_bytes));
    assert_eq!(admit(&f.port, id, &asked, &asked.request, id_bytes), Ok(()));
    assert_eq!(
        admit(&f.port, id, &asked, &asked.request, id_bytes),
        Ok(()),
        "the same admission is the same binding"
    );
    assert_eq!(
        admit(
            &f.port,
            id,
            &asked,
            &format!("{}x", asked.request),
            id_bytes
        ),
        Err(BrokerClass::NoConsent),
        "a changed binding"
    );
    let runs = f.home.control.authority().runs().views().len();
    let other = port_for(&f.home.control, true).with_workspace(f.folder.clone());
    assert_eq!(
        admit(&other, id, &asked, &asked.request, id_bytes),
        Err(BrokerClass::NotAdmitted),
        "another door"
    );
    assert_eq!(
        f.home.control.authority().runs().views().len(),
        runs,
        "no run is opened for a model step"
    );
    f.home
        .control
        .cancel_run(f.port.run_of(id).unwrap())
        .unwrap();
    assert!(
        admit(&f.port, id, &asked, &asked.request, id_bytes).is_err(),
        "a cancelled run"
    );
    assert_eq!(f.attempts(), 0);
}

// ---------------------------------------------------------------- C1-S3

/// The record is read before the reply: only Recorded releases a reply;
/// Owed and Uncertain discard it, whatever the transport said.
#[test]
fn c1_s3_the_record_is_read_before_the_reply() {
    let content = || Ok::<String, AuthorityError>(REPLY.to_string());
    let failed = || Err::<String, _>(AuthorityError::ModelCall(ModelError::Unavailable));
    assert!(matches!(
        classify(EndStatus::Recorded { seq: 1 }, content()),
        ModelAttempt::Replied(c) if c == REPLY
    ));
    assert!(matches!(
        classify(EndStatus::Recorded { seq: 1 }, failed()),
        ModelAttempt::Failed(BrokerClass::ModelUnavailable)
    ));
    for (status, debt) in [
        (EndStatus::Owed, RecordDebt::Owed),
        (EndStatus::Uncertain, RecordDebt::Uncertain),
    ] {
        let attempt = classify(status, content());
        assert!(!format!("{attempt:?}").contains(REPLY));
        assert!(matches!(attempt, ModelAttempt::Unrecorded(d) if d == debt));
        assert!(matches!(classify(status, failed()), ModelAttempt::Unrecorded(d) if d == debt));
    }
}

/// At the port, over the real broker: a recorded reply and a recorded
/// failure pass; an owed or uncertain record releases nothing; a refusal
/// before any attempt is a distinct `Err` (nothing charged, sent or
/// recorded).
#[test]
fn c1_s3_port_releases_only_recorded_replies() {
    port_children(
        "c1_s3_port_releases_only_recorded_replies",
        &[
            "attempt/recorded",
            "attempt/failed",
            "attempt/owed",
            "attempt/uncertain",
            "attempt/pre-attempt",
        ],
    );
}
fn attempt(arg: &str) {
    let f = Fx::with(true);
    let m = f.mother(keyed_lanes());
    let (id, asked) = f.consented(&m);
    let meter = Meter::default();
    if arg == "pre-attempt" {
        let refused = f.port.model_call(id, &request(), &meter, &Never);
        assert_eq!(
            refused.map(|_| ()).map_err(|r| r.class),
            Err(BrokerClass::NotAdmitted)
        );
        assert_eq!(meter.charged.load(Ordering::SeqCst), 0);
        assert_eq!(f.fake.count("/api/chat"), 0);
        assert_eq!(f.attempts(), 0);
        return;
    }
    admit(&f.port, id, &asked, &asked.request, identity_bytes()).unwrap();
    if arg == "failed" {
        f.fake
            .route(|p| (p == "/api/chat").then(|| Answer::Json(500, "{}".into())));
    } else {
        f.fake.route(|p| (p == "/api/chat").then(|| chat(REPLY)));
    }
    match arg {
        "owed" => *f.port.model_record.lock().unwrap() = Some(EndStatus::Owed),
        "uncertain" => *f.port.model_record.lock().unwrap() = Some(EndStatus::Uncertain),
        _ => {}
    }
    let attempt = f.port.model_call(id, &request(), &meter, &Never).unwrap();
    assert_eq!(meter.charged.load(Ordering::SeqCst), 1);
    assert_eq!(*meter.settled.lock().unwrap(), vec![1]);
    assert_eq!(f.fake.count("/api/chat"), 1);
    assert_eq!(f.attempts(), 1, "the broker's own attempt record");
    assert!(!format!("{attempt:?}").contains(REPLY));
    match arg {
        "recorded" => assert!(matches!(attempt, ModelAttempt::Replied(r) if r.content() == REPLY)),
        "failed" => assert!(matches!(
            attempt,
            ModelAttempt::Failed(BrokerClass::ModelUnavailable)
        )),
        "owed" => assert!(matches!(
            attempt,
            ModelAttempt::Unrecorded(RecordDebt::Owed)
        )),
        _ => assert!(matches!(
            attempt,
            ModelAttempt::Unrecorded(RecordDebt::Uncertain)
        )),
    }
}

/// The record seam exists only in test builds: every source line naming it
/// is the item directly under `#[cfg(test)]`, and the crate has no features.
#[test]
fn c1_s3_record_seam_is_test_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("[features]"));
    let mut pending = vec![root.join("src")];
    let mut named = 0;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs")
                || path.ends_with("adapter/c1b_port_oracles.rs")
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = text.lines().map(str::trim).collect();
            for (at, line) in lines.iter().enumerate() {
                if line.contains("model_record") {
                    let above = lines[..at].iter().rev().find(|l| !l.is_empty());
                    assert_eq!(
                        above,
                        Some(&"#[cfg(test)]"),
                        "{}:{}",
                        path.display(),
                        at + 1
                    );
                    named += 1;
                }
            }
        }
    }
    assert_eq!(named, 3, "the field, its initializer and its one use");
}

// ---------------------------------------------------------------- E-O42

/// The broker independently refuses an identity other than the registered
/// one, and a port with no registered model has no identity, admission or
/// attempt: nothing is charged, sent or recorded.
#[test]
fn e_o42_native_identity_mismatch_refuses() {
    port_children(
        "e_o42_native_identity_mismatch_refuses",
        &["identity/native", "identity/absent"],
    );
}
fn identity(arg: &str) {
    let f = Fx::with(arg == "native");
    let m = f.mother(keyed_lanes());
    let (id, asked) = f.consented(&m);
    let meter = Meter::default();
    if arg == "native" {
        assert_eq!(
            admit(&f.port, id, &asked, &asked.request, [7; 32]),
            Err(BrokerClass::ModelIdentityMismatch)
        );
        assert_eq!(
            admit(&f.port, id, &asked, &asked.request, identity_bytes()),
            Ok(())
        );
    } else {
        assert!(f.port.model_identity().is_none());
        assert_eq!(
            admit(&f.port, id, &asked, &asked.request, identity_bytes()),
            Err(BrokerClass::ModelNotRegistered)
        );
        assert_eq!(
            f.port
                .model_call(id, &request(), &meter, &Never)
                .map(|_| ())
                .map_err(|r| r.class),
            Err(BrokerClass::ModelNotRegistered)
        );
    }
    assert_eq!(meter.charged.load(Ordering::SeqCst), 0);
    assert_eq!(f.fake.count("/api/chat"), 0);
    assert_eq!(f.attempts(), 0);
}

// ---------------------------------------------------------------- closed

/// C1b-2: the ordinary route alone is insufficient. A non-File coordinator
/// refuses the same model lane at submit and never invokes it.
#[test]
fn c1b1_model_lane_admission_stays_closed() {
    port_children("c1b1_model_lane_admission_stays_closed", &["closed"]);
}
fn closed() {
    let f = Fx::with(true);
    assert!(f.port.model_identity().is_some(), "the route exists");
    let invoked = Arc::new(AtomicUsize::new(0));
    let m = Coordinator::open(
        MemoryJournal::new(),
        f.port.clone(),
        Lanes::new(model_lanes(&invoked)).unwrap(),
        Arc::new(ManualClock::default()),
        f.home.private(),
    )
    .unwrap();
    let id = m.draft(f.contract()).unwrap();
    assert_eq!(
        m.submit(id),
        Err(CoordinatorError::Contract(
            ContractError::ModelLaneUnavailable
        ))
    );
    assert_eq!(invoked.load(Ordering::SeqCst), 0);
    assert_eq!(f.attempts(), 0);
}

// ---------------------------------------------------------------- C1-S6

/// Closed code mappings: every broker model class and Mother's own model
/// codes round-trip, stay distinct, and carry no content.
#[test]
fn c1_s6_model_codes_are_closed_and_round_trip() {
    for code in ModelError::CODES {
        let name = format!("model_{code}");
        let class = BrokerClass::of(&name);
        assert_ne!(class, BrokerClass::Unknown, "{name}");
        assert_eq!(class.code(), name);
        assert_eq!(BrokerClass::parse(&name), Some(class));
        let reason = BlockReason::BrokerRefused(class);
        assert_eq!(BlockReason::parse(&reason.as_str()), Some(reason));
    }
    for error in [
        AuthorityError::ModelNotRegistered,
        AuthorityError::ModelIdentityMismatch,
        AuthorityError::ModelCall(ModelError::Late),
        AuthorityError::ModelCall(ModelError::UnsafeDelimiter),
    ] {
        assert_eq!(BrokerClass::of(error.class()).code(), error.class());
    }
    for (reason, code) in [
        (BlockReason::ModelNotPinned, "model_not_pinned"),
        (BlockReason::ModelIdentityChanged, "model_identity_changed"),
        (
            BlockReason::ModelAdmissionRefused,
            "model_admission_refused",
        ),
    ] {
        assert_eq!(reason.as_str(), code);
        assert_eq!(BlockReason::parse(code), Some(reason));
        assert_eq!(
            BrokerClass::parse(code),
            None,
            "Mother's codes are not broker classes"
        );
        assert!(BlockReason::every().contains(&reason));
    }
    let family = BrokerClass::every()
        .into_iter()
        .filter(|c| c.code().starts_with("model_"))
        .count();
    assert_eq!(
        family,
        ModelError::CODES.len() + 2,
        "exactly the broker's model classes"
    );
    assert_eq!(
        BrokerClass::of("model_C1B_ERROR_CANARY"),
        BrokerClass::Unknown
    );
}

/// Reuse the existing cfg(test) seam from the fixture module already scanned.
pub(crate) fn record_next(port: &GovernedPort, status: EndStatus) {
    *port.model_record.lock().unwrap() = Some(status);
}

#[test]
fn sb2_facade_rejects_multiline_model_alias() {
    let plant = "pub unsafe fn raw(\n&self,\n) -> model::RegistrationReceipt { unreachable!() }";
    assert!(
        !plant.lines().map(str::trim).all(sb2_facade_line),
        "alias facade escape"
    );
}
