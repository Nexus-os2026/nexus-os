//! P0-B oracles: the coding-run ledger keeps only evidence (digests, sizes,
//! fixed codes and controlled ids), enforced at its single write point,
//! `ledger::record`. Pre-registered in the first commit of the P0-B PR, red
//! at base `f0f88b49`, and never edited after a result is seen
//! (`p0-m2d-design.md` §5.4).
//!
//! | ID     | Oracle                                                                  | Base  |
//! |--------|-------------------------------------------------------------------------|-------|
//! | P0-O10 | worker rejected paths (invalid, too long, out of scope)                 | red   |
//! | P0-O11 | apply and restore refusals and failures named with canaries             | red   |
//! | P0-O12 | run scopes, snapshot scope, worker reads, edit rejections, op errors,   | red   |
//! |        | model pin (canaries in the model name and the endpoint path)            |       |
//! | P0-O13 | `ledger::record`: unlisted key, untyped string, unregistered code       | red   |
//! |        | → `Rejected`; an operation whose record is refused leaves project and   |       |
//! |        | staging untouched                                                       |       |
//! | P0-O15 | control: the task and the summary are recorded by digest only           | green |
//! | P0-O16 | control: a failing model leaves only fixed codes                        | green |
//! | P0-O22 | the scanner's own negative control: a canary only in the WAL, and one   | green |
//! |        | split across two fields, are both detected (test code; MP0-12 is red)   |       |
//! | P0-O24 | inventory: every event kind has a schema; every producer runs with      | red   |
//! |        | canaries and stays valid; the single write point (a guard)              |       |
//!
//! Canaries: `C` = `NXCANARY-` + 32 random hex, and the code-shaped `Cl` =
//! `nxsecret_` + 16 lowercase hex. Each is planted raw, upper and lower case,
//! as hex of its bytes, base64 (std and url), percent-encoded and JSON
//! `\u`-escaped; in project paths only the spellings that are one valid path
//! component. The scan reads the ledger's main file, its `-wal` and `-shm`
//! files and every verified record (`verify_run`), and looks for each canary
//! raw, upper and lower case, as hex (both cases), base64 std and url at every
//! byte alignment, percent-encoded and `\u`-escaped (both cases), and for four
//! overlapping fragments (at least 10 characters each) of the raw, upper and
//! lower spellings, each fragment separately.

use super::p1_apply::{approve_and_apply, env, verified, verified_with, Env};
use super::*;
use crate::coding_run::apply::{HookPoint, AFTER_EXCHANGE, APPLY_HOOK, WRITE_GRANT_REVOKE_FAILS};
use crate::coding_run::ledger::{record, EventKind};
use crate::coding_run::local_model::{pin_for_test, pin_for_test_at};
use crate::coding_run::scope::{validate_component, TEMP_PREFIX};
use crate::coding_run::worker::run_worker_with_limits;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::os::fd::OwnedFd;
use std::sync::Mutex;

// ── Canaries ────────────────────────────────────────────────────────────────

pub(super) struct Canary {
    pub(super) c: String,
    pub(super) cl: String,
}

impl Canary {
    pub(super) fn new() -> Self {
        Self {
            c: format!("NXCANARY-{}", hex::encode(rand::random::<[u8; 16]>())),
            cl: format!("nxsecret_{}", hex::encode(rand::random::<[u8; 8]>())),
        }
    }

    fn secrets(&self) -> [(&'static str, &str); 2] {
        [("C", &self.c), ("Cl", &self.cl)]
    }

    /// Every planted spelling of both canaries, without repeats.
    pub(super) fn spellings(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, secret) in self.secrets() {
            for spelling in spellings(secret) {
                if !out.contains(&spelling) {
                    out.push(spelling);
                }
            }
        }
        out
    }

    /// The spellings that are one valid path component.
    pub(super) fn names(&self) -> Vec<String> {
        self.spellings()
            .into_iter()
            .filter(|spelling| validate_component(spelling).is_ok())
            .collect()
    }
}

fn spellings(secret: &str) -> Vec<String> {
    vec![
        secret.to_string(),
        secret.to_ascii_uppercase(),
        secret.to_ascii_lowercase(),
        hex::encode(secret),
        STANDARD.encode(secret),
        URL_SAFE_NO_PAD.encode(secret),
        percent(secret, false),
        json_escaped(secret, false),
    ]
}

fn percent(text: &str, lower: bool) -> String {
    text.bytes()
        .map(|b| {
            if lower {
                format!("%{b:02x}")
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn json_escaped(text: &str, upper: bool) -> String {
    text.chars()
        .map(|c| {
            if upper {
                format!("\\u{:04X}", c as u32)
            } else {
                format!("\\u{:04x}", c as u32)
            }
        })
        .collect()
}

/// Four overlapping fragments covering `text`, each at least 10 characters.
fn fragments(text: &str) -> Vec<&str> {
    let len = text.len();
    let width = len.div_ceil(4).max(10).min(len);
    (0..4)
        .map(|i| {
            let start = i * (len - width) / 3;
            &text[start..start + width]
        })
        .collect()
}

/// The base64 characters that depend only on `bytes` when they start at
/// byte offset `shift` (mod 3) of an encoded stream.
fn aligned_base64(engine: &base64::engine::GeneralPurpose, bytes: &[u8], shift: usize) -> String {
    let mut padded = vec![0u8; shift];
    padded.extend_from_slice(bytes);
    let encoded = engine.encode(&padded);
    let front = [0, 2, 3][shift];
    let back = usize::from(!(shift + bytes.len()).is_multiple_of(3));
    encoded[front..encoded.len() - back].to_string()
}

/// What the scanner looks for, one secret at a time.
fn needles(secret: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (form, text) in [
        ("raw", secret.to_string()),
        ("upper", secret.to_ascii_uppercase()),
        ("lower", secret.to_ascii_lowercase()),
    ] {
        for (index, fragment) in fragments(&text).into_iter().enumerate() {
            out.push((format!("{form} fragment {index}"), fragment.to_string()));
        }
        out.push((form.to_string(), text));
    }
    out.push(("hex".to_string(), hex::encode(secret)));
    out.push(("HEX".to_string(), hex::encode_upper(secret)));
    for (form, engine) in [
        ("base64", &STANDARD_NO_PAD),
        ("base64url", &URL_SAFE_NO_PAD),
    ] {
        for shift in 0..3 {
            out.push((
                format!("{form}@{shift}"),
                aligned_base64(engine, secret.as_bytes(), shift),
            ));
        }
    }
    out.push(("percent".to_string(), percent(secret, false)));
    out.push(("percent-lower".to_string(), percent(secret, true)));
    out.push(("json-u".to_string(), json_escaped(secret, false)));
    out.push(("json-U".to_string(), json_escaped(secret, true)));
    out
}

/// One canary hit.
#[derive(Debug, Clone)]
pub(super) struct Leak {
    pub(super) secret: &'static str,
    pub(super) form: String,
    pub(super) target: String,
}

impl std::fmt::Display for Leak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} in {}", self.secret, self.form, self.target)
    }
}

/// The ledger's files: the main database, its WAL and its shared memory.
fn file_targets(db: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        let path = PathBuf::from(format!("{}{suffix}", db.display()));
        if let Ok(bytes) = std::fs::read(&path) {
            out.push((format!("ledger.db{suffix}"), bytes));
        }
    }
    out
}

/// Verified logical records, as text.
fn record_targets(records: &[LedgerRecord]) -> Vec<(String, Vec<u8>)> {
    records
        .iter()
        .map(|r| {
            (
                format!("record {} #{}", r.event_kind, r.seq),
                format!("{}\n{}\n{}", r.actor_kind, r.event_kind, r.payload).into_bytes(),
            )
        })
        .collect()
}

pub(super) fn leaks(canary: &Canary, targets: &[(String, Vec<u8>)]) -> Vec<Leak> {
    let mut out = Vec::new();
    for (name, secret) in canary.secrets() {
        for (form, needle) in needles(secret) {
            let finder = regex::bytes::Regex::new(&regex::escape(&needle)).unwrap();
            for (target, bytes) in targets {
                if finder.is_match(bytes) {
                    out.push(Leak {
                        secret: name,
                        form: form.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
    }
    out
}

// ── Digests the records must carry ─────────────────────────────────────────

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// `nexus.evidence.path.v1` over a path spelling.
pub(super) fn path_digest(text: &str) -> String {
    domain_digest(b"nexus.evidence.path.v1", text.as_bytes())
}

/// `nexus.evidence.text.v1` over other free text.
pub(super) fn text_digest(text: &str) -> String {
    domain_digest(b"nexus.evidence.text.v1", text.as_bytes())
}

fn sha_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The record names `path` by digest and size only.
pub(super) fn names_path(p: &Value, path: &str) -> bool {
    p["path_sha256"] == path_digest(path) && p["path_bytes"] == path.len()
}

fn hex64(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

// ── Labs: one scenario's ledger, its runs and what its records must show ────

pub(super) struct Rec {
    pub(super) run: Uuid,
    pub(super) record: LedgerRecord,
    pub(super) payload: Value,
}

type Check = Box<dyn Fn(&[Rec]) -> Result<(), String>>;

pub(super) struct Expect {
    what: String,
    check: Check,
}

pub(super) struct Lab {
    pub(super) name: String,
    _keep: Box<dyn std::any::Any>,
    db: PathBuf,
    ledger: Arc<CodingRunLedger>,
    pub(super) runs: Vec<Uuid>,
    expect: Vec<Expect>,
}

fn ledger_path(f: &Fixture) -> PathBuf {
    f.staging_parent.parent().unwrap().join("ledger.db")
}

impl Lab {
    fn env(name: &str, e: Env, runs: Vec<Uuid>, expect: Vec<Expect>) -> Self {
        let db = ledger_path(&e.f);
        let ledger = Arc::clone(&e.f.ledger);
        Self {
            name: name.to_string(),
            _keep: Box::new(e),
            db,
            ledger,
            runs,
            expect,
        }
    }

    fn fixture(name: &str, f: Fixture, runs: Vec<Uuid>, expect: Vec<Expect>) -> Self {
        let db = ledger_path(&f);
        let ledger = Arc::clone(&f.ledger);
        Self {
            name: name.to_string(),
            _keep: Box::new(f),
            db,
            ledger,
            runs,
            expect,
        }
    }

    pub(super) fn records(&self) -> Vec<Rec> {
        self.runs
            .iter()
            .flat_map(|run| {
                self.ledger
                    .verify_run(*run)
                    .unwrap()
                    .into_iter()
                    .map(|record| Rec {
                        run: *run,
                        payload: serde_json::from_str(&record.payload).unwrap(),
                        record,
                    })
            })
            .collect()
    }

    fn leaks(&self, canary: &Canary) -> Vec<Leak> {
        let mut targets = file_targets(&self.db);
        let records: Vec<LedgerRecord> = self.records().into_iter().map(|r| r.record).collect();
        targets.extend(record_targets(&records));
        leaks(canary, &targets)
    }
}

/// Scan every lab, then check what each lab's records must show.
pub(super) fn verify(canary: &Canary, labs: &[Lab]) {
    let hits: Vec<String> = labs
        .iter()
        .flat_map(|lab| {
            lab.leaks(canary)
                .into_iter()
                .map(move |leak| format!("{}: {leak}", lab.name))
        })
        .collect();
    assert!(
        hits.is_empty(),
        "{} canary hit(s) in the coding-run ledger; the first: {:#?}",
        hits.len(),
        &hits[..hits.len().min(12)]
    );
    for lab in labs {
        let records = lab.records();
        for expect in &lab.expect {
            if let Err(why) = (expect.check)(&records) {
                panic!("{}: expected {}: {why}", lab.name, expect.what);
            }
        }
    }
}

/// Some record of `kind` satisfies `holds`.
fn any(
    what: impl Into<String>,
    kind: &'static str,
    holds: impl Fn(&Value) -> bool + 'static,
) -> Expect {
    Expect {
        what: what.into(),
        check: Box::new(move |records| {
            let of_kind: Vec<&Value> = records
                .iter()
                .filter(|r| r.record.event_kind == kind)
                .map(|r| &r.payload)
                .collect();
            if of_kind.iter().any(|p| holds(p)) {
                Ok(())
            } else {
                Err(format!(
                    "no {kind} record matches; {kind} records: {of_kind:?}"
                ))
            }
        }),
    }
}

/// The run's `worker.proposal_rejected` records are exactly `expected`, in
/// order, each naming its raw path by digest and size.
fn rejected_in_order(what: &str, expected: Vec<(String, &'static str)>) -> Expect {
    Expect {
        what: what.to_string(),
        check: Box::new(move |records| {
            let seen: Vec<(Value, Value, Value)> = records
                .iter()
                .filter(|r| r.record.event_kind == "worker.proposal_rejected")
                .map(|r| {
                    (
                        r.payload["path_sha256"].clone(),
                        r.payload["path_bytes"].clone(),
                        r.payload["reason"].clone(),
                    )
                })
                .collect();
            let want: Vec<(Value, Value, Value)> = expected
                .iter()
                .map(|(path, reason)| (json!(path_digest(path)), json!(path.len()), json!(reason)))
                .collect();
            if seen == want {
                Ok(())
            } else {
                let at = seen.iter().zip(&want).position(|(a, b)| a != b);
                Err(format!(
                    "{} recorded, {} expected; first difference at {at:?}: {:?}",
                    seen.len(),
                    want.len(),
                    at.map(|i| (&seen[i], &want[i]))
                ))
            }
        }),
    }
}

// ── Test doubles ────────────────────────────────────────────────────────────

/// A scripted local model; its answers are untrusted data.
struct Script {
    pin: ModelPin,
    answers: Mutex<VecDeque<Result<String, ModelError>>>,
}

impl Script {
    fn new(pin: ModelPin, answers: Vec<Value>) -> Self {
        Self::raw(
            pin,
            answers.into_iter().map(|a| Ok(a.to_string())).collect(),
        )
    }

    fn raw(pin: ModelPin, answers: Vec<Result<String, ModelError>>) -> Self {
        Self {
            pin,
            answers: Mutex::new(answers.into()),
        }
    }
}

impl LocalModel for Script {
    fn pin(&self) -> &ModelPin {
        &self.pin
    }

    fn complete(
        &self,
        _messages: &[ModelMessage],
        _timeout: Duration,
        _max_response_bytes: u64,
    ) -> Result<String, ModelError> {
        self.answers
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(json!({"action": "finish", "summary": "done"}).to_string()))
    }
}

/// The owner's native answer, fixed.
struct Answer(bool);

impl OwnerConfirmer for Answer {
    fn confirm(&self, _request: &ConfirmationRequest) -> bool {
        self.0
    }
}

impl VerifierLaunchConfirmer for Answer {
    fn confirm_launch(&self, _request: &VerifierLaunchRequest) -> bool {
        self.0
    }
}

type TapAction = Box<dyn FnOnce() + Send>;
type Armed = (&'static str, &'static str, Option<LedgerFailure>, TapAction);

/// Delegates to the real ledger. Once armed, the first append of `kind`
/// whose payload contains `contains` runs `action` first and then, if
/// `fail` is set, is refused with it.
struct Tap {
    inner: Arc<CodingRunLedger>,
    armed: Mutex<Option<Armed>>,
}

impl Tap {
    fn new(inner: &Arc<CodingRunLedger>) -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::clone(inner),
            armed: Mutex::new(None),
        })
    }

    fn arm(
        &self,
        kind: &'static str,
        contains: &'static str,
        fail: Option<LedgerFailure>,
        action: impl FnOnce() + Send + 'static,
    ) {
        *self.armed.lock().unwrap() = Some((kind, contains, fail, Box::new(action)));
    }
}

impl LedgerStore for Tap {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
        let fired = {
            let mut armed = self.armed.lock().unwrap();
            match armed.as_ref() {
                Some((kind, contains, _, _))
                    if event.event_kind == *kind && event.payload.contains(contains) =>
                {
                    armed.take()
                }
                _ => None,
            }
        };
        if let Some((_, _, fail, action)) = fired {
            action();
            if let Some(failure) = fail {
                return Err(failure);
            }
        }
        LedgerStore::append(self.inner.as_ref(), event)
    }

    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
        self.inner.verified_records(run)
    }
}

fn with_apply_hook<T>(
    hook: impl FnMut(HookPoint, &str) -> bool + 'static,
    body: impl FnOnce() -> T,
) -> T {
    APPLY_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
    let out = body();
    APPLY_HOOK.with(|h| *h.borrow_mut() = None);
    out
}

// ── Scenario helpers ────────────────────────────────────────────────────────

/// A selected project holding `src/<name>.rs` for every canary name.
pub(super) fn canary_env(canary: &Canary) -> Env {
    let e = env();
    for name in canary.names() {
        write(
            &e.f.project.join(format!("src/{name}.rs")),
            &format!("// {name}\n"),
        );
    }
    e
}

fn c_file(canary: &Canary) -> String {
    format!("src/{}.rs", canary.c)
}

fn cl_file(canary: &Canary) -> String {
    format!("src/{}.rs", canary.cl)
}

/// Scopes whose entries carry the canaries.
fn canary_scopes(canary: &Canary) -> RunScopes {
    RunScopes::new(
        ScopeSet::new([
            ScopeEntry::Tree(rel("src")),
            ScopeEntry::Tree(rel("tests")),
            ScopeEntry::File(rel(&format!("docs/{}.md", canary.c))),
            ScopeEntry::File(rel(&format!("{}.txt", canary.cl))),
        ]),
        ScopeSet::new([ScopeEntry::Tree(rel("src"))]),
        ScopeSet::new([ScopeEntry::Tree(rel("tests"))]),
    )
    .unwrap()
}

fn canary_staged(f: &Fixture, canary: &Canary, ledger: Arc<dyn LedgerStore>) -> CodingRun {
    let mut run = new_run_with(f, ledger, canary_scopes(canary));
    run.grant(&parent(f)).unwrap();
    run.snapshot().unwrap();
    run
}

fn input_dir(e: &Env) -> OwnedFd {
    let path =
        e.f.staging_parent
            .parent()
            .unwrap()
            .join(format!("p0b-input-{}", Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    OwnedFd::from(std::fs::File::open(&path).unwrap())
}

// ── P0-O10: worker rejected paths ───────────────────────────────────────────

pub(super) fn o10_worker_rejected_paths(canary: &Canary) -> Vec<Lab> {
    let mut labs = Vec::new();

    // Invalid and too-long spellings, missing reads, refused proposals and
    // edits outside the write scope.
    let f = fixture();
    let mut expected: Vec<(String, &'static str)> = Vec::new();
    let mut paths = Vec::new();
    for s in canary.spellings() {
        for (prefix, reason) in [
            ("../", "InvalidPath(Traversal)"),
            ("/", "InvalidPath(Absolute)"),
            (".git/", "InvalidPath(GitMetadata)"),
        ] {
            paths.push(format!("{prefix}{s}"));
            expected.push((format!("{prefix}{s}"), reason));
        }
    }
    for (pad, secret) in [("x".repeat(235), &canary.c), ("y".repeat(240), &canary.cl)] {
        // One component over the 255-byte cap, its secret straddling the
        // 256-byte bound the base kept in records.
        let raw = format!("{pad}{secret}");
        paths.push(raw.clone());
        expected.push((raw, "InvalidPath(TooLong)"));
        let deep = format!("{}{secret}", "d/".repeat(2100));
        paths.push(deep.clone());
        expected.push((deep, "InvalidPath(TooLong)"));
    }
    let missing: Vec<String> = canary
        .names()
        .iter()
        .map(|n| format!("src/{n}.missing"))
        .collect();
    for path in &missing {
        expected.push((path.clone(), "Refused(NotStaged)"));
    }
    let mut refused_edits = Vec::new();
    for s in canary.spellings() {
        let raw = format!("../{s}");
        refused_edits.push(json!({"path": raw, "op": "create", "content": "x\n"}));
        expected.push((raw, "InvalidPath(Traversal)"));
    }
    for n in canary.names() {
        let raw = format!("src/{n}.rs");
        refused_edits.push(json!({"path": raw, "op": "delete", "content": ""}));
        expected.push((raw, "UnsupportedOperation"));
    }
    for n in canary.names() {
        let raw = format!("src/{n}.bin");
        refused_edits.push(json!({"path": raw, "op": "create", "content": "a\u{0}b"}));
        expected.push((raw, "NotText"));
    }
    let outside: Vec<String> = canary
        .names()
        .iter()
        .map(|n| format!("docs/{n}.md"))
        .collect();
    let outside_edits: Vec<Value> = outside
        .iter()
        .map(|p| json!({"path": p, "op": "create", "content": "x\n"}))
        .collect();
    let model = Script::new(
        pin_for_test("m:1"),
        vec![
            json!({"action": "read", "paths": paths}),
            json!({"action": "read", "paths": missing}),
            json!({"action": "edit", "edits": refused_edits}),
            json!({"action": "edit", "edits": outside_edits}),
            json!({"action": "finish", "summary": format!("done {} {}", canary.c, canary.cl)}),
        ],
    );
    let mut run = staged_run(&f);
    run.pin_model(model.pin().clone()).unwrap();
    let task = format!("Fix {} {}", canary.c, canary.cl);
    let report = run_worker(&mut run, &model, &task).unwrap();
    assert!(report.accepted.is_empty());
    assert_eq!(report.rejected as usize, expected.len() + outside.len());
    assert_eq!(run.state(), RunState::Cancelled);
    let mut expect = vec![rejected_in_order(
        "every rejected raw path named by digest, in order",
        expected,
    )];
    for path in outside {
        expect.push(any(
            format!("edit.rejected names {path:?} by digest"),
            "edit.rejected",
            move |p| names_path(p, &path) && p["reason"] == "OutsideWriteScope",
        ));
    }
    labs.push(Lab::fixture(
        "o10 invalid, too long and refused paths",
        f,
        vec![run.id().ledger_key()],
        expect,
    ));

    // Worker bounds reached on canary paths.
    let f = fixture();
    let (c_rs, c_big, cl_rs) = (
        c_file(canary),
        format!("src/{}.big", canary.c),
        cl_file(canary),
    );
    let model = Script::new(
        pin_for_test("m:1"),
        vec![
            json!({"action": "read", "paths": ["src/lib.rs", c_rs, cl_rs]}),
            json!({"action": "edit", "edits": [
                {"path": c_big, "op": "create", "content": "x".repeat(100)},
                {"path": "src/a.rs", "op": "create", "content": "a\n"},
                {"path": cl_rs, "op": "create", "content": "b\n"},
            ]}),
            json!({"action": "finish", "summary": "done"}),
        ],
    );
    let limits = WorkerLimits {
        max_files_read: 1,
        max_proposals: 2,
        max_candidate_bytes: 16,
        ..WORKER_LIMITS
    };
    let mut run = staged_run(&f);
    run.pin_model(model.pin().clone()).unwrap();
    let report = run_worker_with_limits(&mut run, &model, "task", limits).unwrap();
    assert_eq!(report.files_read, 1);
    assert_eq!(report.accepted, vec![rel("src/a.rs")]);
    labs.push(Lab::fixture(
        "o10 worker bounds",
        f,
        vec![run.id().ledger_key()],
        vec![rejected_in_order(
            "bounded refusals named by digest",
            vec![
                (c_rs, "LimitReached"),
                (c_big, "TooLarge"),
                (cl_rs, "LimitReached"),
            ],
        )],
    ));

    // Reads outside a narrow read scope.
    let f = fixture();
    let scopes = RunScopes::new(
        ScopeSet::new([ScopeEntry::Tree(rel("src"))]),
        ScopeSet::new([ScopeEntry::Tree(rel("src"))]),
        ScopeSet::default(),
    )
    .unwrap();
    let outside: Vec<String> = canary
        .names()
        .iter()
        .map(|n| format!("docs/{n}.md"))
        .collect();
    let model = Script::new(
        pin_for_test("m:1"),
        vec![
            json!({"action": "read", "paths": outside}),
            json!({"action": "finish", "summary": "done"}),
        ],
    );
    let mut run = new_run_with(&f, Arc::clone(&f.ledger) as Arc<dyn LedgerStore>, scopes);
    run.grant(&parent(&f)).unwrap();
    run.snapshot().unwrap();
    run.pin_model(model.pin().clone()).unwrap();
    let report = run_worker(&mut run, &model, "task").unwrap();
    assert_eq!(report.files_read, 0);
    labs.push(Lab::fixture(
        "o10 reads outside the read scope",
        f,
        vec![run.id().ledger_key()],
        vec![rejected_in_order(
            "out-of-scope reads named by digest",
            outside
                .into_iter()
                .map(|p| (p, "Refused(OutsideReadScope)"))
                .collect(),
        )],
    ));
    labs
}

#[test]
fn p0b_o10_worker_rejected_paths_reach_the_ledger_only_as_digests() {
    let canary = Canary::new();
    let labs = o10_worker_rejected_paths(&canary);
    verify(&canary, &labs);
}

// ── P0-O11: apply and restore refusals and failures ─────────────────────────

const OWNER_VERSION: &str = "// the owner's concurrent version\n";

pub(super) fn o11_apply_and_restore(canary: &Canary) -> Vec<Lab> {
    let mut labs = Vec::new();
    let (c, cl) = (c_file(canary), cl_file(canary));

    // Stale: the owner edits a target after approval.
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n"), replace(&cl, "// Cl\n")]);
    let approval = run.request_approval(&e.info.name, &Answer(true)).unwrap();
    std::fs::write(e.f.project.join(&c), "// the owner's edit\n").unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(approval, grant, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::Stale(c.clone()))
    );
    let path = c.clone();
    labs.push(Lab::env(
        "o11 stale",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "apply.rejected Stale by digest",
            "apply.rejected",
            move |p| p["reason"] == "Stale" && names_path(p, &path),
        )],
    ));

    // Redirect: a target became a symlink.
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n"), replace(&cl, "// Cl\n")]);
    let outside = e.f.project.parent().unwrap().join("outside.rs");
    std::fs::write(&outside, "// outside\n").unwrap();
    std::fs::remove_file(e.f.project.join(&cl)).unwrap();
    std::os::unix::fs::symlink(&outside, e.f.project.join(&cl)).unwrap();
    assert_eq!(
        approve_and_apply(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Redirect(cl.clone()))
    );
    let path = cl.clone();
    labs.push(Lab::env(
        "o11 redirect",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "apply.rejected Redirect by digest",
            "apply.rejected",
            move |p| p["reason"] == "Redirect" && names_path(p, &path),
        )],
    ));

    // OutsideScope: the candidate is no longer editable (defense in depth;
    // reached by narrowing the frozen scopes in the test).
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n")]);
    let approval = run.request_approval(&e.info.name, &Answer(true)).unwrap();
    run.scopes = RunScopes::new(
        ScopeSet::new([ScopeEntry::WholeProject]),
        ScopeSet::new([ScopeEntry::Tree(rel("docs"))]),
        ScopeSet::default(),
    )
    .unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(approval, grant, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::OutsideScope(c.clone()))
    );
    let path = c.clone();
    labs.push(Lab::env(
        "o11 outside scope",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "apply.rejected OutsideScope by digest",
            "apply.rejected",
            move |p| p["reason"] == "OutsideScope" && names_path(p, &path),
        )],
    ));

    // Unsupported: a base file missing from the candidate (defense in
    // depth; reached by extending the base in the test).
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n")]);
    let approval = run.request_approval(&e.info.name, &Answer(true)).unwrap();
    let gone = format!("src/{}.gone", canary.cl);
    run.base
        .as_mut()
        .unwrap()
        .insert(rel(&gone), ManifestEntry::of(b"gone\n"));
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(approval, grant, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::Unsupported(gone.clone()))
    );
    labs.push(Lab::env(
        "o11 unsupported",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "apply.rejected Unsupported by digest",
            "apply.rejected",
            move |p| p["reason"] == "Unsupported" && names_path(p, &gone),
        )],
    ));

    // RolledBack: a write fails; the applied file is restored.
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n"), replace(&cl, "// Cl\n")]);
    let target = cl.clone();
    let result = with_apply_hook(
        move |point, path| point == HookPoint::BeforeWrite && path == target,
        || approve_and_apply(&e, &mut run, binding),
    );
    assert_eq!(
        result.unwrap_err(),
        ApplyError::RolledBack { failed: cl.clone() }
    );
    let path = cl.clone();
    labs.push(Lab::env(
        "o11 rolled back",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "apply.rolled_back names the failed file by digest",
            "apply.rolled_back",
            move |p| p["failed"] == "file" && names_path(p, &path),
        )],
    ));

    // RecoveryRequired: the owner changed an applied file before the
    // rollback, so it cannot be restored.
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n"), replace(&cl, "// Cl\n")]);
    let (target, owner_file) = (cl.clone(), e.f.project.join(&c));
    let result = with_apply_hook(
        move |point, path| {
            if point == HookPoint::BeforeWrite && path == target {
                std::fs::write(&owner_file, "// owner edit after apply\n").unwrap();
                return true;
            }
            false
        },
        || approve_and_apply(&e, &mut run, binding),
    );
    assert_eq!(
        result.unwrap_err(),
        ApplyError::RecoveryRequired {
            failed: cl.clone(),
            unrestored: vec![c.clone()]
        }
    );
    let (failed, unrestored) = (cl.clone(), c.clone());
    labs.push(Lab::env(
        "o11 recovery required",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "apply.recovery_required names the failed and unrestored files by digest",
            "apply.recovery_required",
            move |p| {
                p["failed"] == "file"
                    && names_path(p, &failed)
                    && p["unrestored"]
                        .as_array()
                        .is_some_and(|list| list.iter().any(|u| names_path(u, &unrestored)))
            },
        )],
    ));

    // OwnerVersionAt: the owner's concurrent version cannot be put back and
    // stays under a backend temp name.
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n")]);
    AFTER_EXCHANGE.with(|h| {
        *h.borrow_mut() = Some(Box::new(|dir: &Path, temp: &str, name: &str| {
            std::fs::write(dir.join(temp), OWNER_VERSION).unwrap();
            std::fs::remove_file(dir.join(name)).unwrap();
        }))
    });
    let result = approve_and_apply(&e, &mut run, binding);
    AFTER_EXCHANGE.with(|h| *h.borrow_mut() = None);
    let Err(ApplyError::RecoveryRequired { failed, unrestored }) = result else {
        panic!("a hidden owner version must require recovery: {result:?}");
    };
    assert_eq!(failed, c);
    assert!(unrestored
        .iter()
        .any(|u| u.contains("your concurrent version is saved as")));
    let (path, src) = (c.clone(), e.f.project.join("src"));
    labs.push(Lab::env(
        "o11 owner version",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "the owner's version is located by its backend temp id",
            "apply.recovery_required",
            move |p| {
                p["unrestored"].as_array().is_some_and(|list| {
                    list.iter().any(|u| {
                        names_path(u, &path)
                            && u["owner_version_temp"].as_str().is_some_and(|id| {
                                std::fs::read_to_string(src.join(format!("{TEMP_PREFIX}{id}")))
                                    .is_ok_and(|text| text == OWNER_VERSION)
                            })
                    })
                })
            },
        )],
    ));

    // Restore refused: the owner changed an applied file.
    let e = canary_env(canary);
    let created = format!("src/{}/{}.rs", canary.c, canary.cl);
    let (mut run, binding) = verified(
        &e,
        vec![replace(&c, "// C\n"), create(&created, "// new\n")],
    );
    approve_and_apply(&e, &mut run, binding).unwrap();
    std::fs::write(e.f.project.join(&c), "// the owner's edit\n").unwrap();
    let approval = run.request_restore(&e.info.name, &Answer(true)).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.restore(approval, grant).unwrap_err(),
        ApplyError::Refused(Refusal::Stale(c.clone()))
    );
    let path = c.clone();
    labs.push(Lab::env(
        "o11 restore stale",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "restore.rejected Stale by digest",
            "restore.rejected",
            move |p| p["reason"] == "Stale" && names_path(p, &path),
        )],
    ));

    // A clean rollback whose record cannot be written.
    let e = canary_env(canary);
    let tap = Tap::new(&e.f.ledger);
    tap.arm(
        "apply.rolled_back",
        "",
        Some(LedgerFailure::Unavailable),
        || {},
    );
    let (mut run, binding) = verified_with(
        &e,
        Arc::clone(&tap) as Arc<dyn LedgerStore>,
        vec![replace(&c, "// C\n"), replace(&cl, "// Cl\n")],
    );
    let target = cl.clone();
    let result = with_apply_hook(
        move |point, path| point == HookPoint::BeforeWrite && path == target,
        || approve_and_apply(&e, &mut run, binding),
    );
    assert_eq!(
        result.unwrap_err(),
        ApplyError::CompletionUnrecorded {
            operation: "rollback"
        }
    );
    let path = cl.clone();
    labs.push(Lab::env(
        "o11 rollback unrecorded",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "the unrecorded rollback names its failed file by digest",
            "apply.recovery_required",
            move |p| {
                p["completion_unrecorded"] == "rollback"
                    && p["failed"] == "file"
                    && names_path(p, &path)
            },
        )],
    ));

    // A partial restore: the owner changes the applied file between the
    // restore's preflight and its writes.
    let e = canary_env(canary);
    let tap = Tap::new(&e.f.ledger);
    let owner_file = e.f.project.join(&c);
    let (mut run, binding) = verified_with(
        &e,
        Arc::clone(&tap) as Arc<dyn LedgerStore>,
        vec![replace(&c, "// C\n")],
    );
    approve_and_apply(&e, &mut run, binding).unwrap();
    tap.arm("restore.prepared", "", None, move || {
        std::fs::write(&owner_file, "// the owner's edit\n").unwrap();
    });
    let approval = run.request_restore(&e.info.name, &Answer(true)).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.restore(approval, grant).unwrap_err(),
        ApplyError::RecoveryRequired {
            failed: "restore".to_string(),
            unrestored: vec![c.clone()]
        }
    );
    let path = c.clone();
    labs.push(Lab::env(
        "o11 partial restore",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "the partial restore names the unrestored file by digest",
            "apply.recovery_required",
            move |p| {
                p["failed"] == "restore"
                    && p["unrestored"]
                        .as_array()
                        .is_some_and(|list| list.iter().any(|u| names_path(u, &path)))
            },
        )],
    ));
    labs
}

#[test]
fn p0b_o11_apply_and_restore_refusals_and_failures_reach_the_ledger_only_as_codes_and_digests() {
    let canary = Canary::new();
    let labs = o11_apply_and_restore(&canary);
    verify(&canary, &labs);
}

// ── P0-O12: scopes, snapshot, reads, edit rejections, op errors, model pin ──

fn snapshot_refused(
    name: &str,
    scopes: RunScopes,
    plant: impl FnOnce(&Path) -> String,
    refusal: fn(String) -> SnapshotRejection,
    detail: &'static str,
) -> Lab {
    let f = fixture();
    let path = plant(&f.project);
    let mut run = new_run_with(&f, Arc::clone(&f.ledger) as Arc<dyn LedgerStore>, scopes);
    run.grant(&parent(&f)).unwrap();
    assert_eq!(
        run.snapshot(),
        Err(RunError::Snapshot(refusal(path.clone())))
    );
    assert_eq!(
        run.state(),
        RunState::Failed(FailureReason::SnapshotRejected)
    );
    Lab::fixture(
        name,
        f,
        vec![run.id().ledger_key()],
        vec![
            any(
                format!("the {detail} refusal names its path by digest"),
                "op.outcome",
                move |p| {
                    p["ok"] == false
                        && p["error"] == "Snapshot"
                        && p["detail"] == detail
                        && names_path(p, &path)
                },
            ),
            any("the run failed with a fixed code", "run.failed", |p| {
                p["reason"] == "SnapshotRejected"
            }),
        ],
    )
}

pub(super) fn o12_runs_snapshots_reads_edits_and_pins(canary: &Canary) -> Vec<Lab> {
    let mut labs = Vec::new();
    let (c, cl) = (canary.c.clone(), canary.cl.clone());
    let names = canary.names();

    // Scopes, the snapshot scope, a pin, a read, edit rejections and edits.
    let f = fixture();
    for name in &names {
        write(&f.project.join(format!("docs/{name}.md")), "doc\n");
    }
    let read_path = format!("src/{c}/f.rs");
    let keep = format!("src/{c}/keep.rs");
    let cl_rs = format!("src/{cl}.rs");
    write(&f.project.join(&read_path), "f\n");
    write(&f.project.join(&keep), "keep\n");
    write(&f.project.join(&cl_rs), "cl\n");
    let mut read = vec![ScopeEntry::Tree(rel("src"))];
    read.extend(
        names
            .iter()
            .map(|n| ScopeEntry::File(rel(&format!("docs/{n}.md")))),
    );
    let n_read = read.len();
    let scopes = RunScopes::new(
        ScopeSet::new(read),
        ScopeSet::new([
            ScopeEntry::Tree(rel(&format!("src/{c}"))),
            ScopeEntry::File(rel(&cl_rs)),
        ]),
        ScopeSet::new([ScopeEntry::File(rel(&keep))]),
    )
    .unwrap();
    let mut run = new_run_with(&f, Arc::clone(&f.ledger) as Arc<dyn LedgerStore>, scopes);
    run.grant(&parent(&f)).unwrap();
    run.snapshot().unwrap();
    let endpoint = format!("http://127.0.0.1:11434/{c}/{cl}");
    let model = format!("{c}:{cl}");
    run.pin_model(pin_for_test_at(&endpoint, &model)).unwrap();
    assert_eq!(run.read_staged(&rel(&read_path), 1024).unwrap(), b"f\n");
    let docs_c = format!("docs/{c}.md");
    let bin = format!("src/{c}/bin.rs");
    let missing = format!("src/{c}/missing.rs");
    let rejected = [
        (replace(&docs_c, "x\n"), docs_c.clone(), "OutsideWriteScope"),
        (replace(&keep, "x\n"), keep.clone(), "ProtectedInput"),
        (
            CandidateEdit::Create {
                path: rel(&bin),
                content: b"a\0b".to_vec(),
            },
            bin.clone(),
            "NotText",
        ),
        (
            create(&read_path, "dup\n"),
            read_path.clone(),
            "AlreadyExists",
        ),
        (replace(&missing, "x\n"), missing.clone(), "NotStaged"),
    ];
    let mut expect = Vec::new();
    for (edit, path, reason) in rejected {
        assert!(matches!(run.edit(edit), Err(RunError::EditRejected(_))));
        expect.push(any(
            format!("edit.rejected {reason} by digest"),
            "edit.rejected",
            move |p| names_path(p, &path) && p["reason"] == reason,
        ));
    }
    let created = format!("src/{c}/new_{cl}.rs");
    run.edit(replace(&cl_rs, "new\n")).unwrap();
    run.edit(create(&created, "n\n")).unwrap();
    assert!(run.verify_structural().unwrap().passed());
    expect.push(any(
        "run.created records each scope as entries and a digest",
        "run.created",
        move |p| {
            p["read"]["entries"] == n_read
                && hex64(&p["read"]["sha256"])
                && p["write"]["entries"] == 2
                && hex64(&p["write"]["sha256"])
                && p["protected"]["entries"] == 1
                && hex64(&p["protected"]["sha256"])
        },
    ));
    expect.push(any(
        "the snapshot records its scope the same way",
        "op.prepared",
        move |p| {
            p["kind"] == "snapshot"
                && p["facts"]["read"]["entries"] == n_read
                && hex64(&p["facts"]["read"]["sha256"])
        },
    ));
    expect.push(any(
        "the model pin keeps digests and the loopback fact",
        "run.model_pinned",
        move |p| {
            p["provider"] == "ollama"
                && p["endpoint_loopback"] == true
                && p["endpoint_sha256"] == text_digest(&endpoint)
                && p["model_sha256"] == text_digest(&model)
        },
    ));
    expect.push(any(
        "the worker read names its path by digest",
        "worker.read",
        move |p| names_path(p, &read_path) && p["size"] == 2 && p["sha256"] == sha_hex(b"f\n"),
    ));
    expect.push(any(
        "the accepted replace names its path by digest",
        "op.prepared",
        move |p| {
            p["kind"] == "edit"
                && names_path(&p["facts"], &cl_rs)
                && p["facts"]["kind"] == "replace"
        },
    ));
    expect.push(any(
        "the accepted create names its path by digest",
        "op.prepared",
        move |p| {
            p["kind"] == "edit"
                && names_path(&p["facts"], &created)
                && p["facts"]["kind"] == "create"
        },
    ));
    labs.push(Lab::fixture(
        "o12 scopes, pin, read and edits",
        f,
        vec![run.id().ledger_key()],
        expect,
    ));

    // Snapshot refusals: each op error names only a code and a digest.
    let c2 = c.clone();
    labs.push(snapshot_refused(
        "o12 symlink",
        default_scopes(),
        move |project| {
            let path = format!("src/{c2}");
            std::os::unix::fs::symlink("../README.md", project.join(&path)).unwrap();
            path
        },
        SnapshotRejection::Symlink,
        "Symlink",
    ));
    let cl2 = cl.clone();
    labs.push(snapshot_refused(
        "o12 special file",
        default_scopes(),
        move |project| {
            let path = format!("src/{cl2}");
            nix::unistd::mkfifo(&project.join(&path), nix::sys::stat::Mode::S_IRWXU).unwrap();
            path
        },
        SnapshotRejection::SpecialFile,
        "SpecialFile",
    ));
    let c2 = c.clone();
    labs.push(snapshot_refused(
        "o12 hard link",
        default_scopes(),
        move |project| {
            let path = format!("src/{c2}.a");
            write(&project.join(&path), "a\n");
            std::fs::hard_link(project.join(&path), project.join(format!("src/{c2}.b"))).unwrap();
            path
        },
        SnapshotRejection::HardLink,
        "HardLink",
    ));
    let cl2 = cl.clone();
    labs.push(snapshot_refused(
        "o12 invalid name",
        default_scopes(),
        move |project| {
            let path = format!("src/{cl2}\\x");
            write(&project.join(&path), "x\n");
            path
        },
        SnapshotRejection::InvalidName,
        "InvalidName",
    ));
    let c2 = c.clone();
    labs.push(snapshot_refused(
        "o12 leftover temporary",
        default_scopes(),
        move |project| {
            let path = format!("src/{TEMP_PREFIX}{c2}");
            write(&project.join(&path), "left\n");
            path
        },
        SnapshotRejection::LeftoverTemporary,
        "LeftoverTemporary",
    ));
    let cl2 = cl.clone();
    labs.push(snapshot_refused(
        "o12 file too large",
        default_scopes(),
        move |project| {
            let path = format!("src/{cl2}.big");
            std::fs::File::create(project.join(&path))
                .unwrap()
                .set_len(StructuralProfile::V1.max_file_bytes + 1)
                .unwrap();
            path
        },
        SnapshotRejection::FileTooLarge,
        "FileTooLarge",
    ));
    let c2 = c.clone();
    labs.push(snapshot_refused(
        "o12 not a directory",
        RunScopes::new(
            ScopeSet::new([ScopeEntry::File(rel(&format!("{c}/x")))]),
            ScopeSet::default(),
            ScopeSet::default(),
        )
        .unwrap(),
        move |project| {
            write(&project.join(&c2), "file\n");
            c2
        },
        SnapshotRejection::NotADirectory,
        "NotADirectory",
    ));
    labs
}

#[test]
fn p0b_o12_scopes_snapshot_reads_edit_rejections_op_errors_and_the_model_pin_are_digests() {
    let canary = Canary::new();
    let labs = o12_runs_snapshots_reads_edits_and_pins(&canary);
    verify(&canary, &labs);
}

// ── P0-O13: the schema at the single write point ────────────────────────────

#[test]
fn p0b_o13_record_refuses_unlisted_keys_untyped_strings_and_unregistered_codes() {
    let canary = Canary::new();
    let (c, cl) = (canary.c.clone(), canary.cl.clone());
    let f = fixture();
    let run = Uuid::new_v4();
    let hex = "ab".repeat(32);
    let refused: Vec<(EventKind, Value, &str)> = vec![
        (
            EventKind::RunCancelled,
            json!({"reason": cl}),
            "an unregistered, code-shaped reason",
        ),
        (
            EventKind::RunRevoked,
            json!({"reason": "EmergencyStop", "note": c}),
            "an unlisted key",
        ),
        (
            EventKind::WorkerRead,
            json!({"path": format!("src/{c}.rs"), "size": 1}),
            "a raw path",
        ),
        (
            EventKind::WorkerRead,
            json!({"path_sha256": c, "path_bytes": 1}),
            "text as a digest",
        ),
        (
            EventKind::ApplyOp,
            json!({"path_sha256": hex, "path_bytes": c, "ok": true}),
            "text as a count",
        ),
        (
            EventKind::ApplyOp,
            json!({"path_sha256": hex, "path_bytes": 1, "ok": c}),
            "text as a bool",
        ),
        (
            EventKind::RunGranted,
            json!({"staging_grant": c}),
            "text as an id",
        ),
        (
            EventKind::ApplyRejected,
            json!({"reason": format!("Stale({c:?})")}),
            "Debug text as a reason",
        ),
        (
            EventKind::OperationOutcome,
            json!({"op": 0, "ok": false, "error": format!("snapshot rejected: Symlink({c:?})")}),
            "error text",
        ),
        (
            EventKind::OperationPrepared,
            json!({"op": 0, "kind": "edit", "facts": {"path": c}}),
            "a raw path in nested facts",
        ),
        (
            EventKind::ApplyPrepared,
            json!({"binding": hex, "files": [{"path": c}]}),
            "a raw path in a list",
        ),
        (
            EventKind::RunCreated,
            json!({"read": [format!("file:{c}")]}),
            "scope text",
        ),
        (
            EventKind::ModelPinned,
            json!({"provider": "ollama", "model": c}),
            "a model name",
        ),
        (
            EventKind::StagingDiscarded,
            json!({"status": cl}),
            "an unregistered status",
        ),
        (
            EventKind::ApplyOp,
            json!(c),
            "a payload that is not an object",
        ),
    ];
    for (kind, payload, what) in &refused {
        assert!(
            matches!(
                record(f.ledger.as_ref(), run, *kind, payload),
                Err(LedgerFailure::Rejected)
            ),
            "{what} was recorded for {kind:?}"
        );
    }
    assert!(
        f.ledger.verify_run(run).unwrap().is_empty(),
        "nothing refused reached the ledger"
    );
    // Controls: evidence of the allowed classes is recorded.
    record(
        f.ledger.as_ref(),
        run,
        EventKind::RunCancelled,
        &json!({"reason": "cancelled"}),
    )
    .unwrap();
    record(
        f.ledger.as_ref(),
        run,
        EventKind::WorkerRead,
        &json!({"path_sha256": path_digest("src/lib.rs"), "path_bytes": 10, "size": 3, "sha256": hex}),
    )
    .unwrap();
    assert_eq!(f.ledger.verify_run(run).unwrap().len(), 2);

    // An operation whose record is refused is not performed: staging and
    // the project are untouched.
    let tap = Tap::new(&f.ledger);
    let mut staged = new_run_with(
        &f,
        Arc::clone(&tap) as Arc<dyn LedgerStore>,
        default_scopes(),
    );
    staged.grant(&parent(&f)).unwrap();
    staged.snapshot().unwrap();
    let staging = staged.staging_path_for_test().unwrap();
    let (project_before, staging_before) = (digest(&f.project), digest(&staging));
    tap.arm(
        "op.prepared",
        "\"kind\":\"edit\"",
        Some(LedgerFailure::Rejected),
        || {},
    );
    assert_eq!(
        staged.edit(create(&format!("src/{c}.rs"), "x\n")),
        Err(RunError::Ledger(LedgerFailure::Rejected))
    );
    assert_eq!(digest(&f.project), project_before);
    assert_eq!(digest(&staging), staging_before);
    let runs = vec![run, staged.id().ledger_key()];
    verify(&canary, &[Lab::fixture("o13", f, runs, Vec::new())]);
}

// ── P0-O15/O16: controls ────────────────────────────────────────────────────

#[test]
fn p0b_o15_control_the_task_and_the_summary_are_recorded_by_digest_only() {
    let canary = Canary::new();
    let f = fixture();
    let summary = format!("done: {} {}", canary.c, canary.cl);
    let task = format!("Fix {} and {}", canary.c, canary.cl);
    let model = Script::new(
        pin_for_test("m:1"),
        vec![json!({"action": "finish", "summary": summary})],
    );
    let mut run = staged_run(&f);
    run.pin_model(model.pin().clone()).unwrap();
    run_worker(&mut run, &model, &task).unwrap();
    let (task_sha, summary_sha) = (sha_hex(task.as_bytes()), sha_hex(summary.as_bytes()));
    verify(
        &canary,
        &[Lab::fixture(
            "o15",
            f,
            vec![run.id().ledger_key()],
            vec![
                any("the task by digest", "worker.started", move |p| {
                    p["task_sha256"] == task_sha
                }),
                any("the summary by digest", "worker.finished", move |p| {
                    p["summary_sha256"] == summary_sha
                }),
            ],
        )],
    );
}

#[test]
fn p0b_o16_control_a_failing_model_leaves_only_fixed_codes() {
    let canary = Canary::new();
    let f = fixture();
    let model = Script::raw(
        pin_for_test("m:1"),
        vec![
            Ok(format!("not json {} {}", canary.c, canary.cl)),
            Err(ModelError::Unavailable),
        ],
    );
    let mut run = staged_run(&f);
    run.pin_model(model.pin().clone()).unwrap();
    assert_eq!(
        run_worker(&mut run, &model, "task"),
        Err(WorkerError::Model(ModelError::Unavailable))
    );
    verify(
        &canary,
        &[Lab::fixture(
            "o16",
            f,
            vec![run.id().ledger_key()],
            vec![
                any("a malformed answer by code", "worker.turn", |p| {
                    p["action"] == "malformed"
                }),
                any("the model failure by code", "run.failed", |p| {
                    p["reason"] == "ModelUnavailable"
                }),
            ],
        )],
    );
}

// ── P0-O22: the scanner's own negative control ──────────────────────────────

#[test]
fn p0b_o22_the_scanner_detects_a_wal_only_canary_and_a_canary_split_across_two_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("ledger.db");
    let ledger = CodingRunLedger::open(&db).unwrap();
    let append = |run: Uuid, payload: Value| {
        ledger
            .append(NewLedgerEvent {
                run_id: run,
                timestamp_unix_nanos: 1,
                actor_kind: "backend",
                event_kind: "run.cancelled",
                payload: &payload.to_string(),
            })
            .unwrap();
    };

    // Written only to the WAL: the connection stays open, nothing checkpoints.
    let wal_only = Canary::new();
    let run = Uuid::new_v4();
    append(run, json!({"reason": "cancelled"}));
    append(run, json!({"note": wal_only.c}));
    let main = std::fs::read(&db).unwrap();
    assert!(
        !main
            .windows(wal_only.c.len())
            .any(|w| w == wal_only.c.as_bytes()),
        "precondition: the canary is not in the main database file"
    );
    let found = leaks(&wal_only, &file_targets(&db));
    assert!(
        found
            .iter()
            .any(|l| l.secret == "C" && l.form == "raw" && l.target == "ledger.db-wal"),
        "the scanner missed a canary written only to the WAL: {found:?}"
    );

    // Two halves of each canary in two fields: no field holds a whole one.
    let split = Canary::new();
    let run = Uuid::new_v4();
    let (c1, c2) = split.c.split_at(split.c.len() / 2);
    let (l1, l2) = split.cl.split_at(split.cl.len() / 2);
    append(run, json!({"a": c1, "b": c2, "x": l1, "y": l2}));
    let mut targets = file_targets(&db);
    targets.extend(record_targets(&ledger.verify_run(run).unwrap()));
    let found = leaks(&split, &targets);
    assert!(
        !found.iter().any(|l| l.form == "raw"),
        "precondition: no whole canary was written"
    );
    for secret in ["C", "Cl"] {
        for half in ["raw fragment 0", "raw fragment 3"] {
            assert!(
                found.iter().any(|l| l.secret == secret && l.form == half),
                "the scanner missed {secret} {half}: {found:?}"
            );
        }
    }
}

// ── P0-O24: inventory ───────────────────────────────────────────────────────

/// Every event kind, in declaration order.
fn all_kinds() -> Vec<EventKind> {
    use EventKind::*;
    let all = vec![
        RunCreated,
        RunGranted,
        OperationPrepared,
        OperationOutcome,
        EditRejected,
        StructuralVerification,
        RunCancelled,
        RunRevoked,
        RunFailed,
        RecoveryRequired,
        StagingDiscarded,
        ModelPinned,
        WorkerStarted,
        WorkerTurn,
        WorkerRead,
        ProposalRejected,
        WorkerFinished,
        ReviewComputed,
        CandidateWithdrawn,
        ApprovalGranted,
        ApprovalDeclined,
        ApplyRejected,
        ApplyPrepared,
        ApplyOp,
        ApplyCompleted,
        ApplyRolledBack,
        ApplyRecoveryRequired,
        RestoreApproved,
        RestoreDeclined,
        RestoreRejected,
        RestorePrepared,
        RestoreCompleted,
        VerificationPrepared,
        VerificationApproved,
        VerificationDeclined,
        VerificationLaunched,
        VerificationFinished,
        VerificationCleanup,
    ];
    // Exhaustive: a new kind does not compile here until it is listed.
    let position = |kind: EventKind| match kind {
        RunCreated => 0,
        RunGranted => 1,
        OperationPrepared => 2,
        OperationOutcome => 3,
        EditRejected => 4,
        StructuralVerification => 5,
        RunCancelled => 6,
        RunRevoked => 7,
        RunFailed => 8,
        RecoveryRequired => 9,
        StagingDiscarded => 10,
        ModelPinned => 11,
        WorkerStarted => 12,
        WorkerTurn => 13,
        WorkerRead => 14,
        ProposalRejected => 15,
        WorkerFinished => 16,
        ReviewComputed => 17,
        CandidateWithdrawn => 18,
        ApprovalGranted => 19,
        ApprovalDeclined => 20,
        ApplyRejected => 21,
        ApplyPrepared => 22,
        ApplyOp => 23,
        ApplyCompleted => 24,
        ApplyRolledBack => 25,
        ApplyRecoveryRequired => 26,
        RestoreApproved => 27,
        RestoreDeclined => 28,
        RestoreRejected => 29,
        RestorePrepared => 30,
        RestoreCompleted => 31,
        VerificationPrepared => 32,
        VerificationApproved => 33,
        VerificationDeclined => 34,
        VerificationLaunched => 35,
        VerificationFinished => 36,
        VerificationCleanup => 37,
    };
    for (index, kind) in all.iter().enumerate() {
        assert_eq!(position(*kind), index);
    }
    assert_eq!(all.len(), 38);
    all
}

/// Every producer site of the coding-run ledger at base `f0f88b49`, by the
/// record it writes (`apply.recovery_required` and `op.*` are split by the
/// branch that writes them).
pub(super) const PRODUCERS: &[(&str, &str)] = &[
    ("run.created", "coding_run.rs:832-844 create_inner"),
    ("run.cancelled", "coding_run.rs:1002 record_state"),
    ("run.revoked", "coding_run.rs:1002 record_state"),
    ("run.failed", "coding_run.rs:1002 record_state"),
    ("run.recovery_required", "coding_run.rs:1002 record_state"),
    ("op.prepared:allocate_staging", "coding_run.rs:1220 grant"),
    (
        "op.outcome:allocate_staging:ok",
        "coding_run.rs:1242-1245 grant",
    ),
    (
        "op.outcome:allocate_staging:error",
        "coding_run.rs:1243-1245 grant",
    ),
    ("run.granted", "coding_run.rs:1272-1273 grant"),
    (
        "op.prepared:snapshot",
        "coding_run.rs:1160 mutate, facts 1300",
    ),
    (
        "op.outcome:snapshot:ok",
        "coding_run.rs:1172 mutate, facts 1303",
    ),
    (
        "op.outcome:snapshot:error",
        "coding_run.rs:1170-1172 mutate",
    ),
    ("op.prepared:edit", "coding_run.rs:1160 mutate, facts 1380"),
    (
        "op.outcome:edit:ok",
        "coding_run.rs:1172 mutate, facts 1386",
    ),
    ("op.outcome:edit:error", "coding_run.rs:1170-1172 mutate"),
    ("edit.rejected", "coding_run.rs:1439-1444 reject_edit"),
    (
        "verify.structural",
        "coding_run.rs:1581-1592 verify_structural",
    ),
    ("review.computed", "coding_run.rs:1651-1663 review"),
    (
        "review.candidate_withdrawn",
        "coding_run.rs:1838 withdraw_verification",
    ),
    ("run.model_pinned", "coding_run.rs:1859-1864 pin_model"),
    ("worker.read", "coding_run.rs:1927-1932 read_staged"),
    (
        "staging.discarded",
        "coding_run.rs:2094-2095 discard_staging",
    ),
    (
        "worker.started",
        "worker.rs:253 via coding_run.rs:1984 record_worker",
    ),
    ("worker.turn", "worker.rs:325 via record_worker"),
    ("worker.finished", "worker.rs:418 via record_worker"),
    (
        "worker.proposal_rejected",
        "worker.rs:602 reject, via record_worker",
    ),
    ("approval.granted", "apply.rs:445 request_approval"),
    ("approval.declined", "apply.rs:445 request_approval"),
    ("apply.rejected", "apply.rs:484 apply"),
    (
        "apply.recovery_required:write_grant",
        "apply.rs:499 authority_not_closed",
    ),
    (
        "apply.prepared",
        "apply.rs:581-591 apply_inner, preimages 842-851",
    ),
    ("apply.op", "apply.rs:623 apply_inner"),
    ("apply.completed", "apply.rs:636 apply_inner"),
    ("apply.rolled_back", "apply.rs:681 finish_failed"),
    (
        "apply.recovery_required:partial",
        "apply.rs:698 finish_failed",
    ),
    (
        "apply.recovery_required:completion_unrecorded",
        "apply.rs:712 completion_unrecorded",
    ),
    ("restore.approved", "apply.rs:883 request_restore"),
    ("restore.declined", "apply.rs:883 request_restore"),
    ("restore.rejected", "apply.rs:938 restore"),
    ("restore.prepared", "apply.rs:999 restore_inner"),
    ("restore.completed", "apply.rs:1017 restore_inner"),
    (
        "apply.recovery_required:restore_partial",
        "apply.rs:1031 restore_inner",
    ),
    ("verify.prepared", "verifier.rs:599 prepare_verification"),
    (
        "verify.approval_granted",
        "verifier.rs:668 request_verification_approval",
    ),
    (
        "verify.approval_declined",
        "verifier.rs:668 request_verification_approval",
    ),
    ("verify.launch", "verifier.rs:719 begin_verification"),
    ("verify.result", "verifier.rs:811 finish_verification"),
    (
        "verify.cleanup",
        "verifier.rs:846 confirm_verification_cleanup",
    ),
];

/// Producer branches no test can reach without fault injection below the
/// kernel, with the reason.
pub(super) const FAULT_ONLY: &[(&str, &str)] = &[(
    "op.outcome:allocate_staging:begin_failed",
    "coding_run.rs:1231: StagingLease::begin fails only when duplicating the staging \
     parent's descriptor fails (descriptor exhaustion); it writes the same keys as \
     op.outcome:allocate_staging:error, which runs",
)];

/// The producer sites a lab's records show.
pub(super) fn sites(records: &[Rec]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut ops: HashMap<(Uuid, u64), String> = HashMap::new();
    for r in records {
        let p = &r.payload;
        let site = match r.record.event_kind.as_str() {
            "op.prepared" => {
                let kind = p["kind"].as_str().unwrap_or("?").to_string();
                if let Some(op) = p["op"].as_u64() {
                    ops.insert((r.run, op), kind.clone());
                }
                format!("op.prepared:{kind}")
            }
            "op.outcome" => {
                let kind = p["op"]
                    .as_u64()
                    .and_then(|op| ops.get(&(r.run, op)))
                    .cloned()
                    .unwrap_or_else(|| "?".to_string());
                let ok = if p["ok"] == true { "ok" } else { "error" };
                format!("op.outcome:{kind}:{ok}")
            }
            "apply.recovery_required" => {
                let branch = if p.get("attempt_ok").is_some() {
                    "write_grant"
                } else if p.get("completion_unrecorded").is_some() {
                    "completion_unrecorded"
                } else if p["failed"] == "restore" {
                    "restore_partial"
                } else {
                    "partial"
                };
                format!("apply.recovery_required:{branch}")
            }
            kind => kind.to_string(),
        };
        out.insert(site);
    }
    out
}

/// The producers the other oracles do not reach: verification, approvals,
/// apply and restore end to end; cancel, discard, revoke, a worker panic, a
/// withdrawn candidate, a failed allocation, a failed edit and an unclosed
/// write grant. Every run carries canaries.
pub(super) fn o24_remaining_producers(canary: &Canary) -> Vec<Lab> {
    let mut labs = Vec::new();
    let (c, cl) = (c_file(canary), cl_file(canary));

    // Verification (declined, then launched with a failed cleanup that is
    // later confirmed), approval declined then granted, apply, restore
    // declined then completed.
    let e = canary_env(canary);
    let mut edits: Vec<CandidateEdit> = canary
        .names()
        .iter()
        .map(|n| replace(&format!("src/{n}.rs"), &format!("// new {n}\n")))
        .collect();
    edits.push(create(
        &format!("src/{}/{}.rs", canary.c, canary.cl),
        "// created\n",
    ));
    let (mut run, binding) = verified(&e, edits);
    let inputs = VerifierInputs {
        profile_hash: [1; 32],
        toolchain_digest: [2; 32],
        toolchain_generation: 1,
        sandbox_policy_hash: [4; 32],
        resource_policy_hash: [5; 32],
    };
    let facts = VerifierLaunchFacts {
        profile_display: format!("{} {}", canary.c, canary.cl),
        wall_timeout_secs: 600,
        memory_max_bytes: 1 << 30,
        cpus: 1,
        processes: 16,
    };
    run.prepare_verification(inputs).unwrap();
    run.materialize_verification_input(input_dir(&e)).unwrap();
    assert_eq!(
        run.request_verification_approval(&facts, &Answer(false))
            .unwrap_err(),
        VerificationError::Declined
    );
    run.prepare_verification(inputs).unwrap();
    run.materialize_verification_input(input_dir(&e)).unwrap();
    let approval = run
        .request_verification_approval(&facts, &Answer(true))
        .unwrap();
    let generation = run.begin_verification(approval, inputs).unwrap();
    run.verification_running(generation).unwrap();
    run.verification_finalizing(generation).unwrap();
    let stream = |fill| StreamSummary {
        bytes: 9,
        sha256: [fill; 32],
        truncated: true,
    };
    run.finish_verification(
        generation,
        VerifierOutcome {
            exit: VerifierExit::Failed { exit_code: -3 },
            duration_ms: 12,
            stdout: stream(7),
            stderr: stream(8),
            cleanup: VerifierCleanup::Failed,
        },
    )
    .unwrap();
    run.confirm_verification_cleanup(generation).unwrap();
    assert_eq!(
        run.request_approval(&e.info.name, &Answer(false))
            .unwrap_err(),
        ApplyError::Refused(Refusal::Declined)
    );
    approve_and_apply(&e, &mut run, binding).unwrap();
    assert_eq!(
        run.request_restore(&e.info.name, &Answer(false))
            .unwrap_err(),
        ApplyError::Refused(Refusal::Declined)
    );
    let approval = run.request_restore(&e.info.name, &Answer(true)).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    run.restore(approval, grant).unwrap();
    let (c2, c3) = (c.clone(), c.clone());
    labs.push(Lab::env(
        "o24 verification, approvals, apply and restore",
        e,
        vec![run.id().ledger_key()],
        vec![
            any("the verifier result by code", "verify.result", |p| {
                p["exit"] == "failed" && p["detail"] == -3 && p["cleanup"] == "failed"
            }),
            any(
                "apply.prepared names files and pre-images by digest",
                "apply.prepared",
                move |p| {
                    p["files"]
                        .as_array()
                        .is_some_and(|files| files.iter().any(|f| names_path(f, &c2)))
                        && p["preimages"].as_array().is_some_and(|pre| {
                            pre.iter().any(|x| {
                                names_path(x, &c2) && x["slot"].is_u64() && x["mode"].is_u64()
                            })
                        })
                },
            ),
            any("apply.op names its file by digest", "apply.op", move |p| {
                names_path(p, &c3) && p["ok"] == true
            }),
        ],
    ));

    // Cancel, then discard the staging.
    let f = fixture();
    let mut run = canary_staged(&f, canary, Arc::clone(&f.ledger) as Arc<dyn LedgerStore>);
    run.cancel().unwrap();
    assert_eq!(run.discard_staging().unwrap(), CleanupStatus::Discarded);
    labs.push(Lab::fixture(
        "o24 cancel and discard",
        f,
        vec![run.id().ledger_key()],
        vec![any("staging.discarded by code", "staging.discarded", |p| {
            p["status"] == "Discarded"
        })],
    ));

    // Revoke.
    let f = fixture();
    let mut run = canary_staged(&f, canary, Arc::clone(&f.ledger) as Arc<dyn LedgerStore>);
    run.revoke().unwrap();
    labs.push(Lab::fixture(
        "o24 revoke",
        f,
        vec![run.id().ledger_key()],
        vec![any("run.revoked by code", "run.revoked", |p| {
            p["reason"] == "Explicit"
        })],
    ));

    // A worker panic whose payload carries the canary.
    let f = fixture();
    let mut run = canary_staged(&f, canary, Arc::clone(&f.ledger) as Arc<dyn LedgerStore>);
    let payload = format!("{} {}", canary.c, canary.cl);
    assert!(run
        .guard_worker(move |_run: &mut CodingRun| panic_with(payload))
        .is_err());
    labs.push(Lab::fixture(
        "o24 worker panic",
        f,
        vec![run.id().ledger_key()],
        vec![any(
            "run.recovery_required by code",
            "run.recovery_required",
            |p| p["reason"] == "WorkerPanicked",
        )],
    ));

    // A verified candidate changed in staging before its review.
    let e = canary_env(canary);
    let (mut run, _) = verified(&e, vec![replace(&c, "// C\n")]);
    std::fs::write(
        run.staging_path_for_test().unwrap().join(&c),
        "// changed\n",
    )
    .unwrap();
    assert_eq!(run.review().unwrap_err(), RunError::CandidateChanged);
    labs.push(Lab::env(
        "o24 candidate withdrawn",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "the withdrawal by code",
            "review.candidate_withdrawn",
            |p| p["reason"] == "CandidateChanged",
        )],
    ));

    // The staging allocation fails: the staging parent was replaced.
    let f = fixture();
    let staging = parent(&f);
    std::fs::rename(
        &f.staging_parent,
        f.staging_parent.with_file_name("staging-moved"),
    )
    .unwrap();
    std::fs::create_dir(&f.staging_parent).unwrap();
    let mut run = new_run_with(
        &f,
        Arc::clone(&f.ledger) as Arc<dyn LedgerStore>,
        canary_scopes(canary),
    );
    assert_eq!(run.grant(&staging), Err(RunError::IdentityChanged));
    labs.push(Lab::fixture(
        "o24 allocation failure",
        f,
        vec![run.id().ledger_key()],
        vec![any("the allocation error by code", "op.outcome", |p| {
            p["ok"] == false && p["error"] == "IdentityChanged"
        })],
    ));

    // A staged edit fails: a staging parent became a symlink after the
    // probe.
    let f = fixture();
    let tap = Tap::new(&f.ledger);
    let mut run = canary_staged(&f, canary, Arc::clone(&tap) as Arc<dyn LedgerStore>);
    let link = run
        .staging_path_for_test()
        .unwrap()
        .join(format!("src/{}", canary.c));
    tap.arm("op.prepared", "\"kind\":\"edit\"", None, move || {
        std::os::unix::fs::symlink("..", &link).unwrap();
    });
    assert_eq!(
        run.edit(create(&format!("src/{}/{}.rs", canary.c, canary.cl), "x\n")),
        Err(RunError::StagingRedirect)
    );
    labs.push(Lab::fixture(
        "o24 edit failure",
        f,
        vec![run.id().ledger_key()],
        vec![any("the edit error by code", "op.outcome", |p| {
            p["ok"] == false && p["error"] == "StagingRedirect"
        })],
    ));

    // The write grant cannot be closed after an apply.
    let e = canary_env(canary);
    let (mut run, binding) = verified(&e, vec![replace(&c, "// C\n"), replace(&cl, "// Cl\n")]);
    let approval = run.request_approval(&e.info.name, &Answer(true)).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    WRITE_GRANT_REVOKE_FAILS.with(|fails| fails.set(true));
    let result = run.apply(approval, grant, &parent(&e.f));
    WRITE_GRANT_REVOKE_FAILS.with(|fails| fails.set(false));
    assert!(matches!(result, Err(ApplyError::AuthorityNotClosed { .. })));
    labs.push(Lab::env(
        "o24 write grant not closed",
        e,
        vec![run.id().ledger_key()],
        vec![any(
            "the unclosed grant by code",
            "apply.recovery_required",
            |p| p["failed"] == "write grant closure" && p["attempt_ok"] == true,
        )],
    ));
    labs
}

fn panic_with(payload: String) {
    std::panic::panic_any(payload)
}

/// A guard, not a proof: only `ledger::record` builds a ledger event.
fn single_write_point() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src.join("coding_run.rs")];
    for entry in std::fs::read_dir(src.join("coding_run")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.ends_with(".rs") && !name.ends_with("tests.rs") {
            files.push(path);
        }
    }
    let builders: Vec<String> = files
        .iter()
        .filter(|path| {
            std::fs::read_to_string(path)
                .unwrap()
                .contains("NewLedgerEvent {")
        })
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        builders,
        vec!["ledger.rs".to_string()],
        "only ledger::record builds a ledger event"
    );
}

#[test]
fn p0b_o24_every_event_kind_has_a_schema_and_every_producer_runs_with_canaries() {
    // (a) Every event kind refuses an unlisted key.
    let f = fixture();
    let run = Uuid::new_v4();
    for kind in all_kinds() {
        assert!(
            matches!(
                record(
                    f.ledger.as_ref(),
                    run,
                    kind,
                    &json!({"nx_unlisted_key": true})
                ),
                Err(LedgerFailure::Rejected)
            ),
            "{kind:?} has no schema: an unlisted key was recorded"
        );
    }
    // (b) The single write point.
    single_write_point();
    // (c) Every producer runs with canaries and its records are clean.
    let canary = Canary::new();
    let mut labs = o10_worker_rejected_paths(&canary);
    labs.extend(o11_apply_and_restore(&canary));
    labs.extend(o12_runs_snapshots_reads_edits_and_pins(&canary));
    labs.extend(o24_remaining_producers(&canary));
    verify(&canary, &labs);
    let mut seen = BTreeSet::new();
    for lab in &labs {
        seen.extend(sites(&lab.records()));
    }
    let missing: Vec<&(&str, &str)> = PRODUCERS
        .iter()
        .filter(|(id, _)| !seen.contains(*id))
        .collect();
    assert!(missing.is_empty(), "producers not exercised: {missing:#?}");
    let known: BTreeSet<&str> = PRODUCERS
        .iter()
        .chain(FAULT_ONLY)
        .map(|(id, _)| *id)
        .collect();
    let unknown: Vec<&String> = seen
        .iter()
        .filter(|s| !known.contains(s.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "records outside the inventory: {unknown:?}"
    );
}
