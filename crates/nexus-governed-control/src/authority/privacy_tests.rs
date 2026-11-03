//! P0-C oracles: the broker's sticky privacy class (design v2 §5.4 P0-O14,
//! P0-O20, P0-O21, the scanner's own control P0-O22, §5.2's choke point,
//! and §11 R4's non-door agent control), registered before the class
//! exists. From this commit on, the oracle bodies are frozen.
//!
//! The rule judged: a run's privacy class is fixed when the backend admits
//! it, `Owner` only for a backend-issued owner run and `Restricted` for
//! every other run, an unknown one included. It is copied into each
//! commitment and credential lease when they are created and never derived
//! from door membership again. A Restricted record keeps evidence only: no
//! target text; the registered counts, ids and codes; every other detail
//! value as `<key>_sha256` (I6). Owner records are unchanged.
//!
//! Canaries: `C` (`NXCANARY-` + 32 random hex) and `Cl` (`nxsecret_` + 16
//! lowercase hex: code-shaped). Each is scanned for raw, upper and lower
//! case, as hex of its bytes, base64 (standard and URL-safe, padded or
//! not), percent-encoded and JSON `\u`-escaped, and as four fragments of
//! at least 8 characters, each one alone. Scanned: the evidence store's
//! main file and its `-wal` and `-shm` files while it is open, its logical
//! rows while open and again after it is closed and reopened (the reopen
//! verifies the whole chain), the main file once closed, and the in-memory
//! history (each record's JSON and `Debug`).
//!
//! Every race is a forced order (a gate held inside the world server or
//! inside the effect), never a sleep; every thread is joined through a
//! channel with a bounded wait, which only detects a hang.

use super::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, ResumeConfirmation,
};
use super::clock::SystemClock;
use super::commitment::{
    CommitmentView, ExecutionGuard, FailureClass, PreparedAction, TargetIdentity,
};
use super::effect::{CapabilityKind, EffectClass};
use super::evidence::{EvidencePhase, EvidenceRecord, EvidenceSink, MemoryEvidence};
use super::halt::HaltReason;
use super::ids::{random16, AgentId, Digest, GrantId, LeaseId, RunId};
use super::policy::GrantScope;
use super::run::{GroupId, RunOrigin};
use super::{Authority, AuthorityError};
use crate::broker::Vault;
use crate::broker::{CredentialBroker, CredentialSpec, Placement, SecretSource, SecretUnavailable};
use crate::control::{Control, EffectOutput, PendingEffect, Preparation};
use crate::egress::destination::Destination;
use crate::egress::{EgressIntent, ReleaseCredential};
use crate::evidence_store::tests::Scratch;
use crate::evidence_store::PrimaryEvidence;
use crate::governed::{AgentDoor, AgentOutcome, GovernedControl, GrantRequest, Intent};
use crate::harness_tests::{Received, Reply, TestServer, Yes};
use crate::test_hooks::Gate;
use std::any::Any;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroizing;

/// A bounded join: it detects a hang and never makes a test pass.
const JOIN: Duration = Duration::from_secs(10);

/// The text digest domain (I9): a Restricted detail value `v` under key
/// `k` is kept as `k_sha256` = this digest of `v`.
const TEXT_DOMAIN: &str = "nexus.evidence.text.v1";

fn text_digest(text: &str) -> String {
    Digest::of(TEXT_DOMAIN, &[text.as_bytes()]).to_hex()
}

// ---------------------------------------------------------------- canaries

struct Canaries {
    c: String,
    cl: String,
}

impl Canaries {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            c: format!("NXCANARY-{}", hex::encode(random16())),
            cl: format!("nxsecret_{}", &hex::encode(random16())[..16]),
        })
    }

    /// The content type every world response carries: both canaries.
    fn content_type(&self) -> String {
        format!("text/plain; c={}; s={}", self.c, self.cl)
    }

    /// A redirect to another origin, its host `C` (lower case, as a host
    /// is canonical) and its path `C`.
    fn location(&self) -> String {
        format!("http://{}.invalid/{}", self.c.to_lowercase(), self.c)
    }

    /// The origin of [`Self::location`], as egress records it.
    fn location_origin(&self) -> String {
        format!("http://{}.invalid:80", self.c.to_lowercase())
    }

    /// A path and query carrying both canaries.
    fn path(&self, prefix: &str) -> String {
        format!("/{prefix}/{}?q={}", self.c, self.cl)
    }

    /// Free text carrying both (a target, a failure).
    fn text(&self, label: &str) -> String {
        format!("{label} {} {}", self.c, self.cl)
    }

    /// Result metadata with both canaries, some under the registered keys:
    /// a code-shaped secret is not a code, a canary is not a count.
    fn meta(&self) -> Vec<(String, String)> {
        vec![
            ("status".into(), "200".into()),
            ("bytes".into(), "12".into()),
            ("content_type".into(), self.content_type()),
            ("redirects".into(), self.c.clone()),
            ("reason".into(), self.cl.clone()),
            ("service".into(), self.cl.clone()),
            ("op".into(), self.cl.clone()),
            ("lease".into(), self.cl.clone()),
            ("door".into(), self.c.to_lowercase()),
            ("note".into(), self.text("note")),
        ]
    }

    /// Every form scanned for: its label, and the text.
    fn needles(&self) -> Vec<(String, String)> {
        let mut all = forms("C", &self.c);
        all.extend(forms("Cl", &self.cl));
        all
    }
}

/// Four fragments of at least 8 characters: a long canary in four parts;
/// a short one (`Cl`) in overlapping windows, its all-hex one 12 long.
fn fragments(canary: &str) -> Vec<String> {
    let chars: Vec<char> = canary.chars().collect();
    let n = chars.len();
    let spans = if n >= 40 {
        let q = n / 4;
        vec![(0, q), (q, 2 * q), (2 * q, 3 * q), (3 * q, n)]
    } else {
        vec![(0, 8), (4, 13), (8, 20), (n - 12, n)]
    };
    spans
        .into_iter()
        .map(|(a, b)| {
            assert!(b - a >= 8, "a fragment is at least 8 characters");
            chars[a..b].iter().collect()
        })
        .collect()
}

fn base64_of(text: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
}

fn percent_of(text: &str) -> String {
    text.bytes().map(|b| format!("%{b:02X}")).collect()
}

fn forms(name: &str, canary: &str) -> Vec<(String, String)> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    use base64::Engine;
    let bytes = canary.as_bytes();
    let mut out = vec![
        (format!("{name} raw"), canary.to_string()),
        (format!("{name} upper"), canary.to_uppercase()),
        (format!("{name} lower"), canary.to_lowercase()),
        (format!("{name} hex"), hex::encode(bytes)),
        (format!("{name} HEX"), hex::encode_upper(bytes)),
        (format!("{name} base64"), STANDARD.encode(bytes)),
        (
            format!("{name} base64 unpadded"),
            STANDARD_NO_PAD.encode(bytes),
        ),
        (format!("{name} base64url"), URL_SAFE.encode(bytes)),
        (
            format!("{name} base64url unpadded"),
            URL_SAFE_NO_PAD.encode(bytes),
        ),
        (format!("{name} percent"), percent_of(canary)),
        (
            format!("{name} percent lower"),
            bytes.iter().map(|b| format!("%{b:02x}")).collect(),
        ),
        (
            format!("{name} json-u"),
            canary
                .chars()
                .map(|c| format!("\\u{:04x}", c as u32))
                .collect(),
        ),
        (
            format!("{name} JSON-U"),
            canary
                .chars()
                .map(|c| format!("\\u{:04X}", c as u32))
                .collect(),
        ),
    ];
    for (i, fragment) in fragments(canary).into_iter().enumerate() {
        out.push((
            format!("{name} fragment {i} upper"),
            fragment.to_uppercase(),
        ));
        out.push((
            format!("{name} fragment {i} lower"),
            fragment.to_lowercase(),
        ));
        out.push((format!("{name} fragment {i}"), fragment));
    }
    out
}

// ---------------------------------------------------------------- scanner

/// What a scan found: `<form> in <source>`, one entry each.
struct Scan {
    needles: Vec<(String, String)>,
    found: Vec<String>,
}

impl Scan {
    fn new(canaries: &Canaries) -> Self {
        Self {
            needles: canaries.needles(),
            found: Vec::new(),
        }
    }

    /// Every needle is ASCII, so a lossy view of binary bytes keeps every
    /// occurrence of one.
    fn text(&mut self, source: &str, text: &str) {
        for (label, needle) in &self.needles {
            if text.contains(needle.as_str()) {
                self.found.push(format!("{label} in {source}"));
            }
        }
    }

    fn bytes(&mut self, source: &str, bytes: &[u8]) {
        self.text(source, &String::from_utf8_lossy(bytes));
    }

    /// The store's files: the main database, then its `-wal` and `-shm`
    /// files when they exist. Returns the suffixes read.
    fn files(&mut self, label: &str, store: &Path) -> Vec<String> {
        let mut read = Vec::new();
        for suffix in ["", "-wal", "-shm"] {
            let path = PathBuf::from(format!("{}{suffix}", store.display()));
            if let Ok(bytes) = std::fs::read(&path) {
                self.bytes(&format!("{label} store file{suffix}"), &bytes);
                read.push(suffix.to_string());
            }
        }
        read
    }

    /// The store's logical rows, every field.
    fn rows(&mut self, label: &str, primary: &PrimaryEvidence) {
        for row in primary.store().rows() {
            let text = format!(
                "{} {} {} {} {:?} {} {} {}",
                row.seq,
                row.session,
                row.stored_ms,
                row.phase,
                row.commitment,
                row.body,
                row.prev,
                row.tag
            );
            self.text(&format!("{label} row {}", row.seq), &text);
        }
    }

    /// The history: each record's JSON and its `Debug`.
    fn memory(&mut self, label: &str, history: &MemoryEvidence) {
        let records = history.records();
        let each: Vec<String> = records
            .iter()
            .map(|record| format!("{} {:?}", record.to_json(), record))
            .collect();
        let all = each.join("\n");
        if !self.needles.iter().any(|(_, n)| all.contains(n.as_str())) {
            return;
        }
        for (record, text) in records.iter().zip(&each) {
            let source = format!("{label} {}", record.phase.as_str());
            self.text(&source, text);
        }
    }

    /// The store, closed: every handle on it dropped (`primary` must be
    /// the last), reopened (its chain verified), its rows read, closed
    /// again, and its main file read.
    fn reopened(&mut self, primary: Arc<PrimaryEvidence>, store: &Path) {
        assert_eq!(
            Arc::strong_count(&primary),
            1,
            "nothing else holds the evidence store"
        );
        drop(primary);
        let reopened =
            PrimaryEvidence::open(store, vec![]).expect("the store reopens, its chain verified");
        self.rows("reopened store", &reopened);
        drop(reopened);
        self.files("closed", store);
    }
}

// ---------------------------------------------------------------- shared

fn mother() -> AgentId {
    AgentId::new("mother").unwrap()
}

fn helper() -> AgentId {
    AgentId::new("helper-agent").unwrap()
}

fn detail<'a>(record: &'a EvidenceRecord, key: &str) -> Option<&'a str> {
    record
        .detail
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn records_of(history: &MemoryEvidence, run: RunId) -> Vec<EvidenceRecord> {
    let run = run.to_string();
    history
        .records()
        .into_iter()
        .filter(|r| r.run.as_deref() == Some(run.as_str()))
        .collect()
}

fn end_of<'a>(records: &'a [EvidenceRecord], id: &CommitmentView) -> &'a EvidenceRecord {
    let id = id.id.to_string();
    records
        .iter()
        .find(|r| {
            matches!(r.phase, EvidencePhase::Finished | EvidencePhase::Revoked)
                && r.commitment.as_deref() == Some(id.as_str())
        })
        .expect("the commitment's end record")
}

/// What a Restricted record may keep (I6): no target text; a commitment's
/// records keep their target digest; the counts, ids and codes; every other
/// detail value as `<key>_sha256`, a SHA-256.
fn restricted_shape(records: &[EvidenceRecord]) {
    assert!(!records.is_empty(), "the schedule recorded nothing");
    for record in records {
        let phase = record.phase.as_str();
        assert_eq!(
            record.target, None,
            "a Restricted {phase} keeps target text"
        );
        if matches!(
            record.phase,
            EvidencePhase::Prepared
                | EvidencePhase::Approved
                | EvidencePhase::ApprovalDeclined
                | EvidencePhase::Authorized
                | EvidencePhase::Started
                | EvidencePhase::Finished
                | EvidencePhase::Denied
                | EvidencePhase::Expired
                | EvidencePhase::Revoked
        ) {
            assert!(
                record.target_digest.as_deref().is_some_and(is_sha256),
                "a Restricted {phase} keeps its target digest: {:?}",
                record.target_digest
            );
        }
        for (key, value) in &record.detail {
            match key.as_str() {
                "status" | "bytes" | "redirects" | "lines" => assert!(
                    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
                    "{phase}: {key} is a count"
                ),
                "lease" => assert!(LeaseId::parse(value).is_some(), "{phase}: a lease id"),
                "door" => assert!(
                    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()),
                    "{phase}: a door id"
                ),
                "service" | "reason" | "class" | "area" | "op" => {}
                other => {
                    assert!(
                        other.ends_with("_sha256"),
                        "a Restricted {phase} keeps `{other}` as text"
                    );
                    assert!(is_sha256(value), "{phase}: {other} is a SHA-256");
                }
            }
        }
    }
}

fn joined<T: Send + 'static>(rx: &Receiver<T>) -> T {
    rx.recv_timeout(JOIN)
        .expect("the execution ended within the bound")
}

// ---------------------------------------------------------------- composed

/// The Owner's door dialog: yes. Every other dialog: no.
struct DoorYes;
impl ControlConfirmer for DoorYes {
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

/// The world: every response's content type carries both canaries;
/// `/redirect/` answers 302 to another origin named by `C`; `/hold/` waits
/// at `hold` first.
fn world(canaries: &Canaries, hold: &Gate, request: &Received) -> Reply {
    if request.path.starts_with("/hold/") {
        hold.hold();
    }
    let content_type = ("Content-Type".to_string(), canaries.content_type());
    if request.path.starts_with("/redirect/") {
        let mut reply = Reply::redirect(302, &canaries.location());
        reply.headers.push(content_type);
        return reply;
    }
    let mut reply = Reply::ok("done");
    reply.headers = vec![content_type];
    reply
}

/// The front door over the evidence store (the history its projection),
/// a world server on loopback and the Owner's grant for GET and POST to it.
struct Composed {
    control: Option<Arc<GovernedControl>>,
    history: Arc<MemoryEvidence>,
    primary: Option<Arc<PrimaryEvidence>>,
    server: TestServer,
    hold: Gate,
    canaries: Arc<Canaries>,
    scratch: Scratch,
}

impl Composed {
    fn new(canaries: &Arc<Canaries>) -> Self {
        let scratch = Scratch::new();
        let history = Arc::new(MemoryEvidence::new(20_000));
        let primary = PrimaryEvidence::open(&scratch.store(), vec![history.clone()]).unwrap();
        let control = Arc::new(
            GovernedControl::with_secrets(
                &scratch.path("runtime"),
                Vault::Disabled.source(),
                primary.clone(),
                Arc::new(SystemClock::default()),
            )
            .unwrap(),
        );
        let hold = Gate::new();
        let server = {
            let (canaries, hold) = (canaries.clone(), hold.clone());
            TestServer::start(move |request| world(&canaries, &hold, request))
        };
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
            control: Some(control),
            history,
            primary: Some(primary),
            server,
            hold,
            canaries: canaries.clone(),
            scratch,
        }
    }

    fn control(&self) -> &Arc<GovernedControl> {
        self.control.as_ref().expect("the front door")
    }

    fn door(&self) -> AgentDoor {
        self.control()
            .issue_agent_door(mother(), Arc::new(DoorYes))
            .expect("the Owner opens a door for Mother")
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

    /// The Owner executes `id` of `agent`'s `run`, on another thread.
    fn executes(
        &self,
        id: &CommitmentView,
        agent: &AgentId,
        run: RunId,
    ) -> Receiver<Result<EffectOutput, AuthorityError>> {
        let (tx, rx) = channel();
        let (control, id, agent) = (self.control().clone(), id.id, agent.clone());
        std::thread::spawn(move || {
            let result = control.execute(id, &agent, run);
            drop(control);
            let _ = tx.send(result);
        });
        rx
    }

    fn records_of(&self, run: RunId) -> Vec<EvidenceRecord> {
        records_of(&self.history, run)
    }

    /// Scan everything, then close the store and scan it again. Every door
    /// must have been dropped.
    fn scan(mut self) -> Vec<String> {
        let mut scan = Scan::new(&self.canaries);
        scan.memory("history", &self.history);
        let primary = self.primary.take().expect("the store");
        scan.rows("open store", &primary);
        let store = self.scratch.store();
        let read = scan.files("open", &store);
        assert!(
            read.iter().any(|suffix| suffix == "-wal"),
            "the open store has its write-ahead log: {read:?}"
        );
        drop(self.control.take());
        scan.reopened(primary, &store);
        scan.found
    }
}

// ---------------------------------------------------------------- piped

/// A credential the fixture leases (never a real service).
const SPEC: CredentialSpec = CredentialSpec {
    service: "Fixture",
    scope: "p0c",
    name: "p0c.token",
    placement: Placement::Authorization("Bearer"),
};

struct FixedVault;
impl SecretSource for FixedVault {
    fn read(&self, _scope: &str, _name: &str) -> Result<Zeroizing<String>, SecretUnavailable> {
        Ok(Zeroizing::new("p0c-fixture-token".to_string()))
    }
}

/// How a fixture effect ends: its metadata, or its failure.
type Ending = Result<Vec<(String, String)>, (FailureClass, String)>;

/// What a fixture effect does: wait at `hold` (past any cancellation, on
/// purpose: it completes after it), release a lease, then end with
/// `result`.
#[derive(Default)]
struct Script {
    hold: Option<Gate>,
    release: Option<(Arc<CredentialBroker>, LeaseId, Destination)>,
    result: Option<Ending>,
}

impl Script {
    fn ok(meta: Vec<(String, String)>) -> Self {
        Self {
            result: Some(Ok(meta)),
            ..Self::default()
        }
    }

    fn failing(class: FailureClass, detail: String) -> Self {
        Self {
            result: Some(Err((class, detail))),
            ..Self::default()
        }
    }

    fn held(mut self, gate: &Gate) -> Self {
        self.hold = Some(gate.clone());
        self
    }

    fn releasing(
        mut self,
        broker: &Arc<CredentialBroker>,
        lease: LeaseId,
        at: &Destination,
    ) -> Self {
        self.release = Some((broker.clone(), lease, at.clone()));
        self
    }
}

struct Scripted {
    target: Digest,
    parameters: Digest,
    script: Script,
}

impl PendingEffect for Scripted {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(self.target)
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        let Script {
            hold,
            release,
            result,
        } = self.script;
        if let Some(gate) = hold {
            gate.hold();
        }
        if let Some((broker, lease, destination)) = release {
            let _ = broker.release(lease, guard, &destination);
        }
        result.unwrap_or(Ok(Vec::new())).map(|meta| EffectOutput {
            meta,
            ..EffectOutput::default()
        })
    }
}

static NEXT_PARAMETERS: AtomicU64 = AtomicU64::new(0);

/// The pipeline itself (the door's own machinery underneath: door groups
/// reserved and activated, runs inserted into them), over the evidence
/// store with the history as its projection, or over the history alone
/// (the schedules that open thousands of runs to make the registry prune
/// one), with a Perception grant and a credential broker.
struct Piped {
    control: Option<Arc<Control>>,
    broker: Option<Arc<CredentialBroker>>,
    history: Arc<MemoryEvidence>,
    primary: Option<Arc<PrimaryEvidence>>,
    grant: GrantId,
    lives: Vec<Arc<dyn Any + Send + Sync>>,
    canaries: Arc<Canaries>,
    scratch: Scratch,
}

impl Piped {
    fn new(canaries: &Arc<Canaries>, store: bool) -> Self {
        let scratch = Scratch::new();
        let history = Arc::new(MemoryEvidence::new(20_000));
        let (sink, primary): (Arc<dyn EvidenceSink>, Option<Arc<PrimaryEvidence>>) = if store {
            let primary = PrimaryEvidence::open(&scratch.store(), vec![history.clone()]).unwrap();
            (primary.clone() as Arc<dyn EvidenceSink>, Some(primary))
        } else {
            (history.clone() as Arc<dyn EvidenceSink>, None)
        };
        let control = Arc::new(Control::new(Authority::new(
            sink,
            Arc::new(SystemClock::default()),
        )));
        let grant = control
            .authority()
            .grants()
            .request(
                GrantScope::Perception {
                    display: "agent".into(),
                },
                Duration::from_secs(600),
                &Yes::new(true),
            )
            .unwrap();
        let broker = CredentialBroker::new(control.authority(), Arc::new(FixedVault));
        Self {
            control: Some(control),
            broker: Some(broker),
            history,
            primary,
            grant,
            lives: Vec::new(),
            canaries: canaries.clone(),
            scratch,
        }
    }

    fn control(&self) -> &Arc<Control> {
        self.control.as_ref().expect("the pipeline")
    }

    fn authority(&self) -> &Authority {
        self.control().authority()
    }

    fn broker(&self) -> &Arc<CredentialBroker> {
        self.broker.as_ref().expect("the credential broker")
    }

    /// A run of Mother's in a new door group (reserved, activated, the run
    /// inserted into it, as a door's `open_run` does).
    fn door_run(&mut self) -> (RunId, GroupId) {
        let runs = self.authority().runs();
        let group = runs.next_group();
        let life: Arc<dyn Any + Send + Sync> = Arc::new(());
        runs.reserve_door(group, &mother(), Arc::downgrade(&life))
            .unwrap();
        runs.activate_door(group).unwrap();
        self.lives.push(life);
        let run = self.authority().open_run_in(mother(), group).unwrap();
        (run, group)
    }

    /// The origin a lease names: its host is `C`.
    fn lease_origin(&self) -> Destination {
        Destination::parse(&format!(
            "https://{}.example",
            self.canaries.c.to_lowercase()
        ))
        .unwrap()
    }

    fn propose(
        &self,
        agent: &AgentId,
        run: RunId,
        class: EffectClass,
        script: Script,
        leases: Vec<LeaseId>,
    ) -> CommitmentView {
        self.propose_showing(
            agent,
            run,
            class,
            &self.canaries.text("target"),
            script,
            leases,
        )
    }

    fn propose_showing(
        &self,
        agent: &AgentId,
        run: RunId,
        class: EffectClass,
        display: &str,
        script: Script,
        leases: Vec<LeaseId>,
    ) -> CommitmentView {
        let target = Digest::of("p0c.fixture.target", &[display.as_bytes()]);
        let n = NEXT_PARAMETERS.fetch_add(1, Ordering::SeqCst);
        let parameters = Digest::of("p0c.fixture.parameters", &[&n.to_be_bytes()]);
        self.control()
            .propose(
                agent,
                run,
                Preparation {
                    action: PreparedAction {
                        kind: CapabilityKind::Perception,
                        class,
                        operation: "p0c.fixture",
                        target: TargetIdentity {
                            display: display.to_string(),
                            digest: target,
                        },
                        parameters,
                        grants: vec![self.grant],
                        leases,
                        summary: vec!["A P0-C fixture action".into()],
                    },
                    effect: Box::new(Scripted {
                        target,
                        parameters,
                        script,
                    }),
                    ttl: Duration::from_secs(600),
                },
            )
            .expect("the fixture action is committed")
    }

    /// Authorize: R2 with the Owner's native yes.
    fn authorize(&self, id: &CommitmentView, agent: &AgentId, run: RunId) {
        self.control()
            .authorize(id.id, agent, run, &Yes::new(true))
            .expect("authorized");
    }

    fn execute(
        &self,
        id: &CommitmentView,
        agent: &AgentId,
        run: RunId,
    ) -> Result<EffectOutput, AuthorityError> {
        self.control().execute(id.id, agent, run)
    }

    /// Execute on another thread.
    fn executes(
        &self,
        id: &CommitmentView,
        agent: &AgentId,
        run: RunId,
    ) -> Receiver<Result<EffectOutput, AuthorityError>> {
        let (tx, rx) = channel();
        let (control, id, agent) = (self.control().clone(), id.id, agent.clone());
        std::thread::spawn(move || {
            let result = control.execute(id, &agent, run);
            drop(control);
            let _ = tx.send(result);
        });
        rx
    }

    /// Open runs until the registry, at its capacity, prunes the ended
    /// `run` (its entry, and its place in its group, are gone).
    fn prune_ended(&self, run: RunId) {
        let filler = AgentId::new("filler").unwrap();
        for _ in 0..5_000 {
            if self.authority().runs().view(run).is_none() {
                return;
            }
            self.authority()
                .open_run(filler.clone(), RunOrigin::AgentGoal)
                .unwrap();
        }
        panic!("the registry never pruned the ended run");
    }

    /// A door halt's steps, as the front door takes them: the group halted
    /// and its runs cancelled, their commitments revoked, their leases
    /// ended, the batch written.
    fn halt(&self, group: GroupId) {
        let authority = self.authority();
        let mut batch = authority.batch();
        let (members, _) = authority.runs().halt_group(
            group,
            &hex::encode(random16()),
            &mother(),
            HaltReason::JournalFailed,
            authority.clock().wall_ms(),
            authority.policy_generation(),
            &mut batch,
        );
        let leases =
            authority
                .commitments()
                .revoke_runs(&members, &AuthorityError::Halted, &mut batch);
        authority.commitments().end_leases_even_if_poisoned(&leases);
        self.broker().end_runs(&members);
        self.control().drop_pending_of(&members);
        batch.write();
    }

    fn emergency_stop(&self) {
        let stopped = self.control().stop_effects(None);
        self.authority().write_stop_records(stopped);
    }

    fn records_of(&self, run: RunId) -> Vec<EvidenceRecord> {
        records_of(&self.history, run)
    }

    fn scan(mut self) -> Vec<String> {
        let mut scan = Scan::new(&self.canaries);
        scan.memory("history", &self.history);
        if let Some(primary) = self.primary.take() {
            scan.rows("open store", &primary);
            let store = self.scratch.store();
            let read = scan.files("open", &store);
            assert!(
                read.iter().any(|suffix| suffix == "-wal"),
                "the open store has its write-ahead log: {read:?}"
            );
            drop(self.broker.take());
            drop(self.control.take());
            scan.reopened(primary, &store);
        }
        scan.found
    }
}

// ---------------------------------------------------------------- P0-O22

/// P0-O22, the scanner's own negative control: a canary written only to
/// the write-ahead log (no checkpoint since) is found there and not in the
/// main file; two halves in two fields are found as fragments, the whole
/// nowhere; an encoded canary is found encoded; and the closing scan finds
/// what was planted in the reopened rows and the closed main file. Planted
/// through an owner run, whose records keep their text.
#[test]
fn p0c_o22_the_scanner_sees_the_wal_fragments_and_encodings() {
    let whole = Canaries::new();
    let halves = Canaries::new();
    let encoded = Canaries::new();
    let f = Piped::new(&whole, true);
    let store = f.scratch.store();
    for canaries in [&whole, &halves, &encoded] {
        let mut quiet = Scan::new(canaries);
        quiet.files("empty", &store);
        quiet.memory("history", &f.history);
        assert!(
            quiet.found.is_empty(),
            "nothing planted yet: {:?}",
            quiet.found
        );
    }
    let (owner, run) = f.authority().open_owner_run(vec![]).unwrap();
    let plant = |display: &str, script: Script| {
        let view = f.propose_showing(&owner, run, EffectClass::R1, display, script, vec![]);
        f.authorize(&view, &owner, run);
        let _ = f.execute(&view, &owner, run);
    };
    plant(&whole.c, Script::ok(vec![]));
    plant(
        &halves.c[..20],
        Script::failing(FailureClass::Actuator, halves.c[20..].to_string()),
    );
    plant(
        &format!("{} {}", base64_of(&encoded.c), percent_of(&encoded.cl)),
        Script::ok(vec![]),
    );

    // (1) Only in the write-ahead log: not in the main file yet.
    let mut main = Scan::new(&whole);
    main.bytes("main file", &std::fs::read(&store).unwrap());
    assert!(
        main.found.is_empty(),
        "not checkpointed yet: {:?}",
        main.found
    );
    let mut files = Scan::new(&whole);
    let read = files.files("open", &store);
    assert!(
        files
            .found
            .iter()
            .any(|f| f == "C raw in open store file-wal"),
        "the scanner reads the write-ahead log: {:?} (read {read:?})",
        files.found
    );

    // (2) Two halves in two fields: no whole, every fragment.
    let mut parts = Scan::new(&halves);
    parts.files("open", &store);
    parts.memory("history", &f.history);
    assert!(
        !parts.found.iter().any(|f| f.starts_with("C raw ")
            || f.starts_with("C upper ")
            || f.starts_with("C lower ")),
        "the whole was never written: {:?}",
        parts.found
    );
    for i in 0..4 {
        assert!(
            parts
                .found
                .iter()
                .any(|f| f.starts_with(&format!("C fragment {i} in "))),
            "fragment {i} is found alone: {:?}",
            parts.found
        );
    }

    // (3) Encoded forms.
    let mut forms = Scan::new(&encoded);
    forms.memory("history", &f.history);
    forms.files("open", &store);
    assert!(
        forms.found.iter().any(|f| f.starts_with("C base64 in")),
        "base64: {:?}",
        forms.found
    );
    assert!(
        forms.found.iter().any(|f| f.starts_with("Cl percent in")),
        "percent-encoded: {:?}",
        forms.found
    );

    // (4) The closing scan: the reopened rows and the closed main file.
    let found = f.scan();
    for source in ["reopened store row", "closed store file"] {
        assert!(
            found
                .iter()
                .any(|f| f.starts_with("C raw in ") && f.contains(source)),
            "the closing scan reads the {source}: {found:?}"
        );
    }
}

// ---------------------------------------------------------------- P0-O14

/// P0-O14 (Mother): a door run's network effects. A GET whose path and
/// query carry both canaries, answered by a redirect to an origin named by
/// `C` with both canaries in its content type; and a POST (R2) the Owner
/// approves and executes. No canary is in any evidence; each commitment
/// record keeps its target digest; the metadata is kept as digests of the
/// known values. The caller still receives the metadata.
#[test]
fn p0c_o14_a_mother_network_effects_leave_no_text_in_evidence() {
    let canaries = Canaries::new();
    let f = Composed::new(&canaries);
    let door = f.door();
    let run = door.open_run().unwrap();
    let AgentOutcome::Done(got) = door
        .agent_action(run, &f.get(&canaries.path("redirect")))
        .unwrap()
    else {
        panic!("a GET runs at once");
    };
    assert!(
        got.meta
            .iter()
            .any(|(k, v)| k == "content_type" && *v == canaries.content_type()),
        "the caller gets the response metadata: {:?}",
        got.meta
    );
    let AgentOutcome::AwaitingApproval(wait) = door
        .agent_action(run, &f.post(&canaries.path("post")))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    f.control()
        .authorize(wait.id, &mother(), run, &Yes::new(true))
        .unwrap();
    f.control().execute(wait.id, &mother(), run).unwrap();
    drop(door);
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a Mother run's evidence carries canaries: {found:?}"
    );
    restricted_shape(&records);
    let ends: Vec<&EvidenceRecord> = records
        .iter()
        .filter(|r| r.phase == EvidencePhase::Finished)
        .collect();
    assert_eq!(ends.len(), 2, "both effects ended");
    let redirect = ends
        .iter()
        .find(|r| detail(r, "status") == Some("302"))
        .expect("the redirect's end");
    assert_eq!(
        detail(redirect, "content_type_sha256"),
        Some(text_digest(&canaries.content_type()).as_str())
    );
    assert_eq!(
        detail(redirect, "redirect_not_followed_sha256"),
        Some(text_digest(&canaries.location_origin()).as_str())
    );
    let posted = ends
        .iter()
        .find(|r| detail(r, "status") == Some("200"))
        .expect("the post's end");
    assert_eq!(
        detail(posted, "content_type_sha256"),
        Some(text_digest(&canaries.content_type()).as_str())
    );
}

/// P0-O14 (Mother, through the pipeline): a door run's target text, a
/// failure's free text (a tool's failure detail) and result metadata with
/// canaries under registered keys (a canary is not a count, a code-shaped
/// secret is not a code or an id) are kept only as digests.
#[test]
fn p0c_o14_b_mother_target_and_failure_text_are_digests() {
    let canaries = Canaries::new();
    let mut f = Piped::new(&canaries, true);
    let (run, _) = f.door_run();
    let failure = canaries.text("the tool said");
    let failed = f.propose(
        &mother(),
        run,
        EffectClass::R1,
        Script::failing(FailureClass::Actuator, failure.clone()),
        vec![],
    );
    f.authorize(&failed, &mother(), run);
    assert!(f.execute(&failed, &mother(), run).is_err());
    let done = f.propose(
        &mother(),
        run,
        EffectClass::R2,
        Script::ok(canaries.meta()),
        vec![],
    );
    f.authorize(&done, &mother(), run);
    f.execute(&done, &mother(), run).unwrap();
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a Mother run's evidence carries canaries: {found:?}"
    );
    restricted_shape(&records);
    let end = end_of(&records, &failed);
    assert_eq!(end.failure, Some("actuator"));
    assert_eq!(
        detail(end, "detail_sha256"),
        Some(text_digest(&failure).as_str())
    );
    assert_eq!(detail(end, "detail"), None);
    let end = end_of(&records, &done);
    assert_eq!(end.outcome, Some("succeeded"));
    assert_eq!(detail(end, "status"), Some("200"));
    assert_eq!(detail(end, "bytes"), Some("12"));
    for (key, value) in canaries.meta().into_iter().skip(2) {
        assert_eq!(detail(end, &key), None, "{key} is not kept as text");
        assert_eq!(
            detail(end, &format!("{key}_sha256")),
            Some(text_digest(&value).as_str()),
            "{key} is kept as the digest of its value"
        );
    }
}

/// P0-O14's control (Owner): a backend-issued owner run's records are
/// unchanged: the target, the response metadata and a failure's text are
/// kept as they are, and the scanner sees them.
#[test]
fn p0c_o14_c_control_an_owner_run_keeps_its_text() {
    let canaries = Canaries::new();
    let f = Composed::new(&canaries);
    let (owner, run) = f.control().open_owner_run(vec![]).unwrap();
    let view = f
        .control()
        .propose(&owner, run, &f.get(&canaries.path("redirect")))
        .unwrap();
    f.control()
        .authorize(view.id, &owner, run, &Yes::new(true))
        .unwrap();
    f.control().execute(view.id, &owner, run).unwrap();
    let records = f.records_of(run);
    let origin = f.server.origin();
    let found = f.scan();
    assert!(
        found.iter().any(|f| f.starts_with("C raw in ")),
        "the owner's records keep the canaries, and the scan sees them: {found:?}"
    );
    let end = end_of(&records, &view);
    assert_eq!(
        detail(end, "content_type"),
        Some(canaries.content_type().as_str())
    );
    assert_eq!(
        detail(end, "redirect_not_followed"),
        Some(canaries.location_origin().as_str())
    );
    assert_eq!(
        end.target.as_deref(),
        Some(origin.as_str()),
        "the owner's record keeps its target"
    );

    let canaries = Canaries::new();
    let f = Piped::new(&canaries, true);
    let (owner, run) = f.authority().open_owner_run(vec![]).unwrap();
    let failure = canaries.text("the tool said");
    let failed = f.propose(
        &owner,
        run,
        EffectClass::R1,
        Script::failing(FailureClass::Actuator, failure.clone()),
        vec![],
    );
    f.authorize(&failed, &owner, run);
    assert!(f.execute(&failed, &owner, run).is_err());
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.iter().any(|f| f.starts_with("C raw in ")),
        "the scan sees the owner's text: {found:?}"
    );
    let end = end_of(&records, &failed);
    assert_eq!(
        end.target.as_deref(),
        Some(canaries.text("target").as_str())
    );
    assert_eq!(detail(end, "detail"), Some(failure.as_str()));
}

// ---------------------------------------------------------------- R4

/// §11 R4's control: an agent run that no door opened or adopted (the
/// front door's `open_run`) is Restricted too, through the front door's
/// network effect and through the pipeline's target and failure text.
#[test]
fn p0c_r4_control_a_non_door_agent_run_is_restricted() {
    let canaries = Canaries::new();
    let f = Composed::new(&canaries);
    let run = f
        .control()
        .open_run(helper(), RunOrigin::AgentGoal)
        .unwrap();
    let AgentOutcome::Done(_) = f
        .control()
        .agent_action(&helper(), run, &f.get(&canaries.path("redirect")))
        .unwrap()
    else {
        panic!("a GET runs at once");
    };
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a non-door agent run's evidence carries canaries: {found:?}"
    );
    restricted_shape(&records);

    let canaries = Canaries::new();
    let f = Piped::new(&canaries, true);
    let run = f
        .authority()
        .open_run(helper(), RunOrigin::AgentGoal)
        .unwrap();
    let failure = canaries.text("the tool said");
    let failed = f.propose(
        &helper(),
        run,
        EffectClass::R1,
        Script::failing(FailureClass::Actuator, failure.clone()),
        vec![],
    );
    f.authorize(&failed, &helper(), run);
    assert!(f.execute(&failed, &helper(), run).is_err());
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a non-door agent run's evidence carries canaries: {found:?}"
    );
    restricted_shape(&records);
    assert_eq!(
        detail(end_of(&records, &failed), "detail_sha256"),
        Some(text_digest(&failure).as_str())
    );
}

// ---------------------------------------------------------------- P0-O20

/// P0-O20 (a), pruning: Mother's R2 request is prepared, the Owner
/// executes it, and while it runs its door is dropped and a new door is
/// issued, which reclaims the dropped door's group (the run is no longer
/// a member of any door). The response then completes, with both canaries
/// in its content type. Its end record stays Restricted.
#[test]
fn p0c_o20_a_door_dropped_and_group_pruned_mid_execution() {
    let canaries = Canaries::new();
    let f = Composed::new(&canaries);
    let first = f.door();
    let run = first.open_run().unwrap();
    let runs = f.control().authority().runs().clone();
    let group = (0..64)
        .find(|group| runs.is_member(run, *group))
        .expect("the run is in its door's group");
    let AgentOutcome::AwaitingApproval(wait) = first
        .agent_action(run, &f.post(&canaries.path("hold")))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    f.control()
        .authorize(wait.id, &mother(), run, &Yes::new(true))
        .unwrap();
    let done = f.executes(&wait, &mother(), run);
    f.hold.reached();
    drop(first);
    let second = f.door();
    assert!(
        !runs.is_member(run, group),
        "issuing the second door reclaimed the first door's group"
    );
    f.hold.open();
    let output = joined(&done).expect("the effect completed");
    assert!(output
        .meta
        .iter()
        .any(|(k, v)| k == "content_type" && *v == canaries.content_type()));
    drop(second);
    drop(runs);
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a pruned Mother run's late end carries canaries: {found:?}"
    );
    restricted_shape(&records);
    let end = end_of(&records, &wait);
    assert_eq!(end.outcome, Some("succeeded"));
    assert_eq!(
        detail(end, "content_type_sha256"),
        Some(text_digest(&canaries.content_type()).as_str())
    );
}

/// P0-O20 (b), cancellation, through the door: Mother's R2 request is
/// executing (held at the world) beside another waiting one when Mother's
/// door cancels the run; then an emergency stop ends a waiting request of
/// another of the door's runs. Every record of both runs is Restricted.
#[test]
fn p0c_o20_b_door_cancellation_mid_execution() {
    let canaries = Canaries::new();
    let f = Composed::new(&canaries);
    let door = f.door();
    let run = door.open_run().unwrap();
    let AgentOutcome::AwaitingApproval(held) = door
        .agent_action(run, &f.post(&canaries.path("hold")))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    let AgentOutcome::AwaitingApproval(sibling) = door
        .agent_action(run, &f.post(&canaries.path("sibling")))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    f.control()
        .authorize(held.id, &mother(), run, &Yes::new(true))
        .unwrap();
    let done = f.executes(&held, &mother(), run);
    f.hold.reached();
    door.cancel_run(run).unwrap().write();
    let error = joined(&done).expect_err("the cancelled request ended without a result");
    assert_eq!(error.cause(), &AuthorityError::RunCancelled);
    f.hold.open();
    let other = door.open_run().unwrap();
    let AgentOutcome::AwaitingApproval(stopped) = door
        .agent_action(other, &f.post(&canaries.path("stopped")))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    f.control().emergency_stop();
    drop(door);
    let records = f.records_of(run);
    let others = f.records_of(other);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a cancelled Mother run's evidence carries canaries: {found:?}"
    );
    restricted_shape(&records);
    restricted_shape(&others);
    let end = end_of(&records, &held);
    assert_eq!(end.outcome, Some("cancelled"));
    assert!(end.cancelled);
    assert!(detail(end, "detail_sha256").is_some_and(is_sha256));
    assert_eq!(detail(end, "detail"), None);
    let end = end_of(&records, &sibling);
    assert_eq!(end.phase, EvidencePhase::Revoked);
    assert_eq!(end.failure, Some("run_cancelled"));
    let end = end_of(&others, &stopped);
    assert_eq!(end.phase, EvidencePhase::Revoked);
    assert_eq!(end.failure, Some("emergency_stopped"));
}

/// P0-O20 (b), cancellation, completed anyway: a door run's R2 effect
/// completes, with both canaries in its metadata, after its run was
/// cancelled (the record says so beside the outcome); a waiting sibling is
/// revoked. Then a door halt revokes another door run's waiting action and
/// an emergency stop a third's. Every record is Restricted.
#[test]
fn p0c_o20_b_completion_after_cancellation_halt_and_stop() {
    let canaries = Canaries::new();
    let mut f = Piped::new(&canaries, true);
    let (run, _) = f.door_run();
    let gate = Gate::new();
    let held = f.propose(
        &mother(),
        run,
        EffectClass::R2,
        Script::ok(canaries.meta()).held(&gate),
        vec![],
    );
    let sibling = f.propose(&mother(), run, EffectClass::R2, Script::ok(vec![]), vec![]);
    f.authorize(&held, &mother(), run);
    let done = f.executes(&held, &mother(), run);
    gate.reached();
    f.control().cancel_run(run).unwrap();
    gate.open();
    joined(&done).expect("the effect completed after the cancellation");
    let (halted, group) = f.door_run();
    let revoked = f.propose(
        &mother(),
        halted,
        EffectClass::R2,
        Script::ok(vec![]),
        vec![],
    );
    f.halt(group);
    let (third, _) = f.door_run();
    let last = f.propose(
        &mother(),
        third,
        EffectClass::R2,
        Script::ok(vec![]),
        vec![],
    );
    f.emergency_stop();
    let records = f.records_of(run);
    let halts = f.records_of(halted);
    let stops = f.records_of(third);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a cancelled Mother run's evidence carries canaries: {found:?}"
    );
    for records in [&records, &halts, &stops] {
        restricted_shape(records);
    }
    let end = end_of(&records, &held);
    assert_eq!(end.outcome, Some("succeeded"));
    assert!(
        end.cancelled,
        "the cancellation is recorded beside the outcome"
    );
    assert_eq!(
        detail(end, "content_type_sha256"),
        Some(text_digest(&canaries.content_type()).as_str())
    );
    assert_eq!(end_of(&records, &sibling).failure, Some("run_cancelled"));
    assert_eq!(end_of(&halts, &revoked).failure, Some("halted"));
    assert_eq!(end_of(&stops, &last).failure, Some("emergency_stopped"));
}

/// P0-O20 (c), completion after `prune_ended`: a door run's effect is
/// executing when its run is cancelled and the registry, at its capacity,
/// prunes it (the run is unknown from then on); the effect then completes
/// with both canaries in its metadata. Its end record stays Restricted.
#[test]
fn p0c_o20_c_completion_after_the_run_is_pruned() {
    let canaries = Canaries::new();
    let mut f = Piped::new(&canaries, false);
    let (run, _) = f.door_run();
    let gate = Gate::new();
    let held = f.propose(
        &mother(),
        run,
        EffectClass::R1,
        Script::ok(canaries.meta()).held(&gate),
        vec![],
    );
    f.authorize(&held, &mother(), run);
    let done = f.executes(&held, &mother(), run);
    gate.reached();
    f.control().cancel_run(run).unwrap();
    f.prune_ended(run);
    gate.open();
    joined(&done).expect("the effect completed after its run was pruned");
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a pruned run's late end carries canaries: {found:?}"
    );
    restricted_shape(&records);
    let end = end_of(&records, &held);
    assert_eq!(end.outcome, Some("succeeded"));
    assert_eq!(
        detail(end, "content_type_sha256"),
        Some(text_digest(&canaries.content_type()).as_str())
    );
}

/// P0-O20 (d), a record for an unknown run: a door run's effect holds a
/// credential lease whose origin is named by `C`; the run is cancelled and
/// pruned while the effect runs; the effect then asks for the credential.
/// The release is recorded (before it is refused) for a run the registry
/// no longer knows, and that record is Restricted.
#[test]
fn p0c_o20_d_a_record_for_an_unknown_run_is_restricted() {
    let canaries = Canaries::new();
    let mut f = Piped::new(&canaries, false);
    let (run, _) = f.door_run();
    let origin = f.lease_origin();
    let lease = f
        .broker()
        .lease(&mother(), run, SPEC, &origin, Duration::from_secs(600))
        .unwrap();
    let gate = Gate::new();
    let held = f.propose(
        &mother(),
        run,
        EffectClass::R1,
        Script::ok(canaries.meta())
            .held(&gate)
            .releasing(f.broker(), lease, &origin),
        vec![lease],
    );
    f.authorize(&held, &mother(), run);
    let done = f.executes(&held, &mother(), run);
    gate.reached();
    f.control().cancel_run(run).unwrap();
    f.prune_ended(run);
    gate.open();
    joined(&done).expect("the effect completed");
    assert!(
        f.authority().runs().view(run).is_none(),
        "the run is unknown"
    );
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a record for an unknown run carries canaries: {found:?}"
    );
    restricted_shape(&records);
    let released = records
        .iter()
        .find(|r| r.phase == EvidencePhase::CredentialReleased)
        .expect("the release was recorded before it was refused");
    assert_eq!(detail(released, "lease"), Some(lease.to_string().as_str()));
}

// ---------------------------------------------------------------- P0-O21

/// P0-O21: the lease and credential records of a Mother run carry no
/// origin text (they keep the lease id). Control: an owner run's lease
/// records keep the origin, and the scanner sees it.
#[test]
fn p0c_o21_lease_and_credential_records_of_a_mother_run_carry_no_origin() {
    let canaries = Canaries::new();
    let mut f = Piped::new(&canaries, true);
    let (run, _) = f.door_run();
    let origin = f.lease_origin();
    let lease = f
        .broker()
        .lease(&mother(), run, SPEC, &origin, Duration::from_secs(600))
        .unwrap();
    let used = f.propose(
        &mother(),
        run,
        EffectClass::R1,
        Script::ok(vec![]).releasing(f.broker(), lease, &origin),
        vec![lease],
    );
    f.authorize(&used, &mother(), run);
    f.execute(&used, &mother(), run).unwrap();
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a Mother run's lease records carry canaries: {found:?}"
    );
    restricted_shape(&records);
    for phase in [
        EvidencePhase::LeaseIssued,
        EvidencePhase::CredentialReleased,
    ] {
        let record = records
            .iter()
            .find(|r| r.phase == phase)
            .unwrap_or_else(|| panic!("a {} record", phase.as_str()));
        assert_eq!(record.target, None);
        assert_eq!(detail(record, "lease"), Some(lease.to_string().as_str()));
    }

    let canaries = Canaries::new();
    let f = Piped::new(&canaries, true);
    let (owner, run) = f.authority().open_owner_run(vec![]).unwrap();
    let origin = f.lease_origin();
    let lease = f
        .broker()
        .lease(&owner, run, SPEC, &origin, Duration::from_secs(600))
        .unwrap();
    let used = f.propose(
        &owner,
        run,
        EffectClass::R1,
        Script::ok(vec![]).releasing(f.broker(), lease, &origin),
        vec![lease],
    );
    f.authorize(&used, &owner, run);
    f.execute(&used, &owner, run).unwrap();
    let records = f.records_of(run);
    let found = f.scan();
    assert!(
        found.iter().any(|f| f.starts_with("C lower in ")),
        "the owner's lease records keep the origin, and the scan sees it: {found:?}"
    );
    for phase in [
        EvidencePhase::LeaseIssued,
        EvidencePhase::CredentialReleased,
    ] {
        let record = records.iter().find(|r| r.phase == phase).unwrap();
        assert_eq!(
            record.target.as_deref(),
            Some(origin.origin_text().as_str())
        );
    }
}

// ---------------------------------------------------------------- choke point

/// The crate's production sources (`src`, relative path → text): every
/// `.rs` file except the test files and the test-only modules.
fn production_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                if !(relative.ends_with("_tests")
                    || relative.ends_with("/tests")
                    || relative == "tests")
                {
                    walk(&path, root, out);
                }
            } else if relative.ends_with(".rs")
                && !relative.ends_with("tests.rs")
                && relative != "authority/scripted.rs"
                && relative != "test_hooks.rs"
            {
                out.push((relative, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out
}

/// `text` with every method-chain continuation (a line starting with `.`)
/// joined to the line before it, and the source line each joined line
/// starts at.
fn joined_chains(text: &str) -> (String, Vec<usize>) {
    let mut out = String::new();
    let mut starts = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('.') && !out.is_empty() {
            while out.ends_with('\n') {
                out.pop();
            }
            out.push_str(trimmed);
        } else {
            out.push_str(line);
            starts.push(index + 1);
        }
        out.push('\n');
    }
    (out, starts)
}

/// The span of the body that follows the first `marker` in `text`: from
/// its first `{` to the matching `}`.
fn body_span(text: &str, marker: &str) -> Option<(usize, usize)> {
    let start = text.find(marker)?;
    let open = start + text[start..].find('{')?;
    let mut depth = 0usize;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, open + i));
                }
            }
            _ => {}
        }
    }
    None
}

/// §5.2: one choke point. Outside `authority::write_record` and
/// `authority::write_end` (in `authority/evidence.rs`), no production code
/// writes to an evidence sink (`.record(` or `.record_end(` on a receiver
/// that names an evidence sink, or `EvidenceSink::record`); the only other
/// place is the test fan-out `TeeEvidence`, itself a sink. And the 15
/// `EvidenceRecord::new` sites are where the inventory says. A guard, not
/// a proof.
#[test]
fn p0c_one_choke_point_writes_every_record() {
    let mut writes = Vec::new();
    let mut builders = BTreeMap::new();
    for (path, text) in production_sources() {
        let (text, starts) = joined_chains(&text);
        let allowed: Vec<(usize, usize)> = if path == "authority/evidence.rs" {
            [
                "fn write_record",
                "fn write_end",
                "impl EvidenceSink for TeeEvidence",
            ]
            .iter()
            .filter_map(|marker| body_span(&text, marker))
            .collect()
        } else {
            Vec::new()
        };
        for method in [".record(", ".record_end(", "EvidenceSink::record"] {
            for (at, _) in text.match_indices(method) {
                let receiver: String = text[..at]
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '&' | '*'))
                    .collect::<Vec<char>>()
                    .into_iter()
                    .rev()
                    .collect();
                let lower = receiver.to_ascii_lowercase();
                let sink = method.starts_with("EvidenceSink")
                    || lower.contains("evidence")
                    || lower.contains("sink");
                if !sink || allowed.iter().any(|(a, b)| *a < at && at < *b) {
                    continue;
                }
                let line = starts[text[..at].matches('\n').count()];
                writes.push(format!("{path}:{line}: {receiver}{method}"));
            }
        }
        let sites = text.matches("EvidenceRecord::new(").count();
        if sites > 0 {
            builders.insert(path, sites);
        }
    }
    assert!(
        writes.is_empty(),
        "evidence written outside authority::write_record and write_end: {writes:#?}"
    );
    let inventory: BTreeMap<String, usize> = [
        ("authority/commitment.rs", 1),
        ("authority/mod.rs", 9),
        ("authority/policy.rs", 1),
        ("authority/run.rs", 2),
        ("broker.rs", 2),
    ]
    .into_iter()
    .map(|(path, n)| (path.to_string(), n))
    .collect();
    assert_eq!(builders, inventory, "the 15 record builders");
}

// ---------------------------------------------------------------- consent and resume

use super::approval::{MissionConfirmation, MissionResumeConfirmation};

/// The Owner at a door's mission and held-mission dialogs: always yes or
/// always no; every dialog raised is counted.
struct Answers {
    yes: bool,
    asked: std::sync::atomic::AtomicUsize,
}

impl Answers {
    fn new(yes: bool) -> Arc<Self> {
        Arc::new(Self {
            yes,
            asked: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl ControlConfirmer for Answers {
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
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.yes
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.yes
    }
}

/// A contract hash: 64 lowercase hex digits.
const CONTRACT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn consent(request: &str, summary: &str) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: CONTRACT.into(),
        request: request.into(),
        summary: vec![summary.into()],
        page_facts: None,
    }
}

fn resume(
    mission: &str,
    request: &str,
    held_reason: &str,
    summary: &str,
) -> MissionResumeConfirmation {
    MissionResumeConfirmation {
        mission: mission.into(),
        contract_hash: CONTRACT.into(),
        request: request.into(),
        held_reason: held_reason.into(),
        summary: vec![summary.into()],
    }
}

fn mission_records(history: &MemoryEvidence) -> Vec<EvidenceRecord> {
    history
        .records()
        .into_iter()
        .filter(|r| {
            matches!(
                r.phase,
                EvidencePhase::MissionConsented
                    | EvidencePhase::MissionDeclined
                    | EvidencePhase::MissionResumeConfirmed
                    | EvidencePhase::MissionResumeDeclined
            )
        })
        .collect()
}

/// One attempt at a door's dialog, and what it came to.
struct Attempt {
    case: String,
    yes: bool,
    asked: usize,
    recorded: usize,
    result: Result<(), AuthorityError>,
}

/// Codex PR #54 review 1 (HIGH): the mission-consent and held-mission
/// records a door raises carry only controlled values. Through an agent
/// door, for a yes and for a no: a canary in the request, the mission or
/// the held reason is refused before any dialog or record; a canary in the
/// summary is shown to the Owner and recorded only as its digest. No canary
/// reaches any evidence. Controls: a controlled consent and resume are
/// asked and recorded with their contract hash, request, mission and held
/// reason as they are (what Mother's M2a oracles read); and the Owner's own
/// front door keeps its behaviour (its label is recorded as given, and the
/// scanner sees it).
#[test]
fn p0c_o25_door_consent_and_resume_records_carry_no_agent_text() {
    let canaries = Canaries::new();
    let f = Composed::new(&canaries);
    let (c, cl) = (canaries.c.clone(), canaries.cl.clone());
    let plain = "A one-page site";
    let summary = canaries.text("summary");
    let refused_consents = [
        ("consent request C", consent(&c, plain)),
        ("consent request Cl", consent(&cl, plain)),
        (
            "consent request ending in C",
            consent(&format!("msn-000001/1/{c}"), plain),
        ),
    ];
    let refused_resumes = [
        (
            "resume request C",
            resume("msn-000001", &c, "unclean_shutdown", plain),
        ),
        (
            "resume mission C",
            resume(&c, "msn-000001/2/1", "unclean_shutdown", plain),
        ),
        (
            "resume mission Cl",
            resume(&cl, "msn-000001/2/1", "unclean_shutdown", plain),
        ),
        (
            "resume held reason C",
            resume("msn-000001", "msn-000001/2/1", &c, plain),
        ),
        (
            "resume held reason Cl",
            resume("msn-000001", "msn-000001/2/1", &cl, plain),
        ),
    ];
    let mut attempts = Vec::new();
    for yes in [true, false] {
        let owner = Answers::new(yes);
        let door = f
            .control()
            .issue_agent_door(mother(), owner.clone())
            .expect("the Owner opens a door for Mother");
        let mut attempt = |case: &str, run: &dyn Fn() -> Result<(), AuthorityError>| {
            let (asked, recorded) = (owner.asked(), mission_records(&f.history).len());
            let result = run();
            attempts.push(Attempt {
                case: case.to_string(),
                yes,
                asked: owner.asked() - asked,
                recorded: mission_records(&f.history).len() - recorded,
                result,
            });
        };
        for (case, request) in &refused_consents {
            attempt(case, &|| door.confirm_mission(request).map(|_| ()));
        }
        for (case, request) in &refused_resumes {
            attempt(case, &|| door.confirm_held_mission(request).map(|_| ()));
        }
        let shown = consent("msn-000001/1/1", &summary);
        attempt("consent summary C", &|| {
            door.confirm_mission(&shown).map(|_| ())
        });
        let shown = resume("msn-000001", "msn-000001/2/1", "unclean_shutdown", &summary);
        attempt("resume summary C", &|| {
            door.confirm_held_mission(&shown).map(|_| ())
        });
        let control = consent("msn-000002/3/1", plain);
        attempt("consent control", &|| {
            door.confirm_mission(&control).map(|_| ())
        });
        let control = resume("msn-000002", "msn-000002/4/1", "unclean_shutdown", plain);
        attempt("resume control", &|| {
            door.confirm_held_mission(&control).map(|_| ())
        });
    }
    let records = mission_records(&f.history);
    let found = f.scan();
    assert!(
        found.is_empty(),
        "a door's consent or resume records carry canaries: {found:?}"
    );
    for a in &attempts {
        let answer = if a.yes { "yes" } else { "no" };
        if a.case.contains("summary") || a.case.contains("control") {
            assert_eq!(
                (a.asked, a.recorded),
                (1, 1),
                "{} ({answer}): asked once, recorded once",
                a.case
            );
            if a.yes {
                assert_eq!(a.result, Ok(()), "{} ({answer})", a.case);
            } else {
                assert_eq!(
                    a.result,
                    Err(AuthorityError::Declined),
                    "{} ({answer})",
                    a.case
                );
            }
        } else {
            assert!(
                matches!(a.result, Err(AuthorityError::InvalidAction(_))),
                "{} ({answer}) is refused: {:?}",
                a.case,
                a.result
            );
            assert_eq!(
                (a.asked, a.recorded),
                (0, 0),
                "{} ({answer}): refused before any dialog or record",
                a.case
            );
        }
    }
    assert_eq!(
        records.len(),
        8,
        "the summary and control answers, each recorded"
    );
    for record in &records {
        assert_eq!(record.target, None);
        for (key, value) in &record.detail {
            match key.as_str() {
                "contract_hash" => assert_eq!(value, CONTRACT),
                "summary_digest" => assert!(is_sha256(value), "{value}"),
                "request" => assert!(value.starts_with("msn-00000"), "{value}"),
                "mission" => assert!(value.starts_with("msn-00000"), "{value}"),
                "held_reason" => assert_eq!(value, "unclean_shutdown"),
                other => panic!("a mission record keeps `{other}`"),
            }
        }
    }
    for (phase, request) in [
        (EvidencePhase::MissionConsented, "msn-000002/3/1"),
        (EvidencePhase::MissionDeclined, "msn-000002/3/1"),
        (EvidencePhase::MissionResumeConfirmed, "msn-000002/4/1"),
        (EvidencePhase::MissionResumeDeclined, "msn-000002/4/1"),
    ] {
        let record = records
            .iter()
            .find(|r| r.phase == phase && detail(r, "request") == Some(request))
            .unwrap_or_else(|| panic!("the control's {} record", phase.as_str()));
        assert_eq!(detail(record, "contract_hash"), Some(CONTRACT));
        if request.ends_with("/4/1") {
            assert_eq!(detail(record, "mission"), Some("msn-000002"));
            assert_eq!(detail(record, "held_reason"), Some("unclean_shutdown"));
        }
    }

    // The Owner's own front door keeps its behaviour.
    let canaries = Canaries::new();
    let f = Composed::new(&canaries);
    let owner = Answers::new(true);
    f.control()
        .confirm_mission(&consent(&canaries.c, plain), &*owner)
        .expect("the front door records the Owner's yes to its own label");
    let records = mission_records(&f.history);
    assert_eq!(records.len(), 1);
    assert_eq!(detail(&records[0], "request"), Some(canaries.c.as_str()));
    let found = f.scan();
    assert!(
        found.iter().any(|f| f.starts_with("C raw in ")),
        "the front door's label is recorded as given, and the scan sees it: {found:?}"
    );
}
