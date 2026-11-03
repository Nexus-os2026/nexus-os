//! M2d-B2 oracles for the broker's Files execute step (design v2 §6.1 steps
//! 4-6, §6.3-§6.7, §6.10 D-O1, D-O5-D-O9, D-O11, D-O12, D-O16, D-O17,
//! D-O19, D-O21, D-O24 at the broker; §11 H2′ and M4′; Codex D2 conditions
//! 3-7), pre-registered before the code exists. Everything goes through the
//! broker's own routes with the real kernel: a door's admitted run asks, the
//! Owner authorizes and executes at the front door, and the observer is the
//! one the broker builds. Each race is a forced order at a named hook
//! (`files.before_final`, `files.after_final`, `files.undo_approved`) or in
//! the Owner's dialog, never a sleep.

use super::{AgentDoor, AgentOutcome, GovernedControl, GrantRequest, Intent};
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
    UndoConfirmation,
};
use crate::authority::clock::ManualClock;
use crate::authority::commitment::CommitmentView;
use crate::authority::evidence::{EvidencePhase, EvidenceRecord, MemoryEvidence};
use crate::authority::ids::{AgentId, CommitmentId, GrantId, RunId};
use crate::authority::AuthorityError;
use crate::broker::Vault;
use crate::control::EffectOutput;
use crate::files::{
    parameters_digest, Components, Exclusions, FileCreate, OwnerView, RecoveryFinding, KERNEL_AGENT,
};
use crate::harness_tests::{temp_root, TempRoot};
use crate::test_hooks::{at, Gate};
use nexus_kernel::coding_run::{make_non_dumpable, LedgerFile};
use nexus_kernel::emergency_stop::StopAuthority;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

#[path = "files_exec_review1_tests.rs"]
mod review1;
#[path = "files_exec_review2_tests.rs"]
mod review2;
#[path = "files_exec_review3_tests.rs"]
mod review3;
#[path = "files_exec_review4_tests.rs"]
mod review4;
#[path = "files_exec_review5_tests.rs"]
mod review5;
#[path = "files_exec_s1a_d_tests.rs"]
mod s1a_d;

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const LABEL: &str = "msn-000001/1/1";
/// Plain text (S1a-D's O4: Files creates only `*.html`, checked, and
/// `*.txt`); these oracles are about the publication, not the page check.
const NAME: &str = "notes.txt";
const CONTENT: &str = "<h1>Rosa's bakery</h1>\n<p>Fresh bread daily.</p>\n";
/// The Files commitment's lifetime (the deadline that also bounds
/// publication).
const FILE_TTL: Duration = Duration::from_secs(10 * 60);
/// A Files grant that outlives every commitment of a test.
const LONG: Duration = Duration::from_secs(24 * 3600);
/// The bound within which a Stop must return while execution is held.
const STOP_RETURNS_WITHIN: Duration = Duration::from_secs(5);

fn mother() -> AgentId {
    AgentId::new("mother").unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    hex::encode(Sha256::digest(bytes))
}

fn class<T: std::fmt::Debug>(result: Result<T, AuthorityError>) -> &'static str {
    result.expect_err("refused").class()
}

// ---------------------------------------------------------------- owner

type During = Box<dyn FnOnce() + Send>;

/// The Owner: yes to every dialog unless told otherwise; keeps every undo
/// dialog it was shown and the facts of every action it approved, and runs
/// `during_undo` once inside the next undo dialog.
struct Owner {
    actions: AtomicBool,
    undo: AtomicBool,
    undos: Mutex<Vec<UndoConfirmation>>,
    approved: Mutex<Vec<ActionConfirmation>>,
    during_undo: Mutex<Option<During>>,
}

impl Owner {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            actions: AtomicBool::new(true),
            undo: AtomicBool::new(true),
            undos: Mutex::new(Vec::new()),
            approved: Mutex::new(Vec::new()),
            during_undo: Mutex::new(None),
        })
    }

    fn during_undo(&self, f: impl FnOnce() + Send + 'static) {
        *self.during_undo.lock().unwrap() = Some(Box::new(f));
    }

    fn undos(&self) -> Vec<UndoConfirmation> {
        self.undos.lock().unwrap().clone()
    }
}

impl ControlConfirmer for Owner {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.approved.lock().unwrap().push(request.clone());
        self.actions.load(Ordering::SeqCst)
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
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        true
    }
    fn confirm_undo(&self, request: &UndoConfirmation) -> bool {
        self.undos.lock().unwrap().push(request.clone());
        let during = self.during_undo.lock().unwrap().take();
        if let Some(during) = during {
            during();
        }
        self.undo.load(Ordering::SeqCst)
    }
}

/// Collects what `WorkingDetail::show` shows.
#[derive(Default)]
struct Shown(Vec<(String, String)>);

impl OwnerView for Shown {
    fn line(&mut self, label: &str, value: &str) {
        self.0.push((label.to_string(), value.to_string()));
    }
}

// ---------------------------------------------------------------- fixture

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A private directory tree removed when dropped: `home/ws` (the empty
/// workspace), `ident`, `reserved`.
struct Dirs(PathBuf);

impl Dirs {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "nexus-m2d-b2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        for sub in ["home/ws", "ident", "reserved"] {
            std::fs::create_dir_all(base.join(sub)).unwrap();
        }
        Self(std::fs::canonicalize(&base).unwrap())
    }

    fn exclusions(&self) -> Exclusions {
        Exclusions {
            home: self.0.join("home"),
            identity_home: self.0.join("ident"),
            reserved: vec![self.0.join("reserved"), self.0.join("ident/.nexus")],
            uid: crate::launcher::effective_uid(),
        }
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The broker with a manual clock and its own kernel stop authority, the
/// Owner's empty workspace, a Files grant on it, Mother's door, and one of
/// the door's runs admitted to the Owner's consent.
struct Fx {
    gov: Arc<GovernedControl>,
    evidence: Arc<MemoryEvidence>,
    clock: Arc<ManualClock>,
    stop: Arc<StopAuthority>,
    owner: Arc<Owner>,
    door: Arc<AgentDoor>,
    run: RunId,
    grant: GrantId,
    ws: PathBuf,
    _dirs: Dirs,
    root: TempRoot,
}

/// A broker on `root` (a later one on the same root is a restart).
fn broker(
    root: &TempRoot,
    evidence: &Arc<MemoryEvidence>,
    clock: &Arc<ManualClock>,
    stop: &Arc<StopAuthority>,
) -> Arc<GovernedControl> {
    Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            evidence.clone(),
            clock.clone(),
        )
        .unwrap()
        .with_kernel_stop(stop.clone()),
    )
}

impl Fx {
    fn new() -> Self {
        Self::with(None, LONG)
    }

    /// `hold`: the Owner's startup hold before every publication; `ttl`:
    /// the Files grant's lifetime.
    fn with(hold: Option<Duration>, ttl: Duration) -> Self {
        assert!(make_non_dumpable(), "the host's startup step");
        let root = temp_root("m2d-b2");
        let evidence = Arc::new(MemoryEvidence::new(100_000));
        let clock = Arc::new(ManualClock::default());
        let stop = StopAuthority::new();
        let gov = broker(&root, &evidence, &clock, &stop);
        if let Some(hold) = hold {
            gov.hold_files_before_publish(hold)
                .expect("the Owner's startup hold");
        }
        let dirs = Dirs::new();
        let ws = dirs.0.join("home/ws");
        let info = gov
            .register_workspace_in(&ws, &dirs.exclusions())
            .expect("an empty workspace");
        let owner = Owner::new();
        let grant = gov
            .request_grant(&GrantRequest::Files { workspace: info.id }, ttl, &*owner)
            .expect("the Owner grants Files");
        let door = Arc::new(gov.issue_agent_door(mother(), owner.clone()).unwrap());
        door.confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: LABEL.into(),
            summary: vec!["A one-page site for Rosa's bakery".into()],
            page_facts: None,
        })
        .expect("the Owner consents");
        let run = door.open_run().unwrap();
        door.admit_files(run, HASH, LABEL).unwrap();
        Self {
            gov,
            evidence,
            clock,
            stop,
            owner,
            door,
            run,
            grant,
            ws,
            _dirs: dirs,
            root,
        }
    }

    fn propose(&self, name: &str, content: &str) -> Result<CommitmentView, AuthorityError> {
        match self.door.agent_action(
            self.run,
            &Intent::File(FileCreate {
                name: name.into(),
                content: content.into(),
            }),
        )? {
            AgentOutcome::AwaitingApproval(view) => Ok(view),
            AgentOutcome::Done(_) => panic!("a Files create always waits for the Owner"),
        }
    }

    fn authorize(&self, view: &CommitmentView) -> Result<(), AuthorityError> {
        self.gov
            .authorize(view.id, &mother(), self.run, &*self.owner)
    }

    fn execute(&self, view: &CommitmentView) -> Result<EffectOutput, AuthorityError> {
        self.gov.execute(view.id, &mother(), self.run)
    }

    /// Proposed and authorized, ready to execute.
    fn ready(&self, name: &str, content: &str) -> CommitmentView {
        let view = self.propose(name, content).expect("prepared");
        self.authorize(&view).expect("authorized");
        view
    }

    fn create(&self, name: &str, content: &str) -> Result<EffectOutput, AuthorityError> {
        let view = self.ready(name, content);
        self.execute(&view)
    }

    fn entries(&self) -> Vec<String> {
        entries(&self.ws)
    }

    /// The broker's Files state directory (ledger and staging parent).
    fn state(&self) -> PathBuf {
        self.gov.files().state_dir_for_test()
    }

    fn staged(&self) -> Vec<String> {
        let parent = self.state().join("coding-runs");
        if parent.exists() {
            entries(&parent)
        } else {
            Vec::new()
        }
    }

    /// Every kernel ledger event of every Files run: (run, event, payload).
    fn ledger(&self) -> Vec<(String, String, serde_json::Value)> {
        let path = self.state().join("ledger.db");
        if !path.exists() {
            return Vec::new();
        }
        let ledger = LedgerFile::open(&path).unwrap();
        let mut out = Vec::new();
        for id in ledger.run_ids().unwrap() {
            for record in ledger.store().verified_records(id.ledger_key()).unwrap() {
                out.push((
                    id.to_string(),
                    record.event_kind.clone(),
                    serde_json::from_str(&record.payload).unwrap(),
                ));
            }
        }
        out
    }

    fn events(&self) -> Vec<String> {
        self.ledger()
            .into_iter()
            .map(|(_, event, _)| event)
            .collect()
    }

    fn records_of(&self, id: CommitmentId) -> Vec<EvidenceRecord> {
        let id = id.to_string();
        self.evidence
            .records()
            .into_iter()
            .filter(|r| r.commitment.as_deref() == Some(id.as_str()))
            .collect()
    }

    fn finished(&self, id: CommitmentId) -> EvidenceRecord {
        self.records_of(id)
            .into_iter()
            .find(|r| r.phase == EvidencePhase::Finished)
            .expect("a finished record")
    }

    fn detail(&self, id: CommitmentId) -> Option<Vec<(String, String)>> {
        let detail = self.gov.files_working_detail(id)?;
        let mut shown = Shown::default();
        detail.show(&mut shown);
        Some(shown.0)
    }
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------- causes

/// What can interrupt a publication, injected at a hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cause {
    EmergencyStop,
    Cancel,
    Revoke,
    GrantExpiry,
    DeadlineMonotonic,
    DeadlineWall,
    KernelStop,
    WriteGrant,
}

const CAUSES: [Cause; 8] = [
    Cause::EmergencyStop,
    Cause::Cancel,
    Cause::Revoke,
    Cause::GrantExpiry,
    Cause::DeadlineMonotonic,
    Cause::DeadlineWall,
    Cause::KernelStop,
    Cause::WriteGrant,
];

/// The Files grant's lifetime short enough to expire inside the
/// commitment's.
const SHORT: Duration = Duration::from_secs(60);

impl Cause {
    fn fixture(self) -> Fx {
        match self {
            Cause::GrantExpiry => Fx::with(None, SHORT),
            _ => Fx::new(),
        }
    }

    /// The kernel's code for this cause at the final check.
    fn code(self) -> &'static str {
        match self {
            Cause::EmergencyStop | Cause::KernelStop => "EmergencyStop",
            Cause::Cancel => "Cancelled",
            Cause::Revoke | Cause::WriteGrant => "Revoked",
            Cause::GrantExpiry => "Expired",
            Cause::DeadlineMonotonic | Cause::DeadlineWall => "CommitmentExpired",
        }
    }

    /// Whether the run's cancellation token is set by it.
    fn cancels(self) -> bool {
        matches!(self, Cause::EmergencyStop | Cause::Cancel)
    }

    /// Arm the cause at `point`, once.
    fn arm(self, f: &Fx, point: &'static str) {
        let gov = f.gov.clone();
        let door = f.door.clone();
        let clock = f.clock.clone();
        let stop = f.stop.clone();
        let (run, grant) = (f.run, f.grant);
        f.gov.files().hooks().arm(point, move || match self {
            Cause::EmergencyStop => {
                gov.emergency_stop();
            }
            Cause::Cancel => {
                let _ = door.cancel_run(run).unwrap().write();
            }
            Cause::Revoke => gov.revoke_grant(grant).unwrap(),
            Cause::GrantExpiry => clock.advance(SHORT),
            Cause::DeadlineMonotonic => clock.advance(FILE_TTL),
            Cause::DeadlineWall => clock.suspend(FILE_TTL),
            Cause::KernelStop => {
                stop.engage();
            }
            Cause::WriteGrant => assert!(gov.files().revoke_write_grant_for_test()),
        });
    }
}

// ---------------------------------------------------------------- D-O1

/// D-O1 at the broker: the Owner's dialog shows the complete content and
/// the facts; once approved and executed, exactly the bytes are at the name
/// (no temp left, mode 0644); the result says what happened in codes only;
/// the ledger holds the run's prepared, op and completed records and the
/// evidence its whole chain, Restricted; Mother continues: a second file in
/// the same run is created the same way, and the undo slot then holds it.
#[test]
fn m2d_b2_d_o1_create_composed() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fx::new();
    let view = f.propose(NAME, CONTENT).unwrap();
    assert_eq!(view.operation, "create");
    assert!(view
        .summary
        .contains(&format!("SHA-256: {}", sha256_hex(CONTENT.as_bytes()))));
    assert!(view
        .summary
        .iter()
        .any(|l| l.contains("<h1>Rosa's bakery</h1>")));
    assert_eq!(f.entries(), Vec::<String>::new(), "nothing before approval");
    f.authorize(&view).unwrap();
    let shown = f.owner.approved.lock().unwrap().last().cloned().unwrap();
    assert_eq!(shown.summary, view.summary, "the Owner saw every line");

    let output = f.execute(&view).expect("created");
    assert_eq!(std::fs::read(f.ws.join(NAME)).unwrap(), CONTENT.as_bytes());
    assert_eq!(f.entries(), vec![NAME.to_string()], "no temp left");
    let mode = std::fs::metadata(f.ws.join(NAME))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o644);
    assert_eq!(
        output.meta,
        vec![
            ("op".to_string(), "create".to_string()),
            ("bytes".to_string(), CONTENT.len().to_string()),
            ("lines".to_string(), "2".to_string()),
        ]
    );

    let events = f.events();
    for event in [
        "run.created",
        "review.computed",
        "approval.granted",
        "apply.prepared",
        "apply.op",
        "apply.completed",
    ] {
        assert!(events.iter().any(|e| e == event), "{event} recorded");
    }
    let phases: Vec<EvidencePhase> = f.records_of(view.id).iter().map(|r| r.phase).collect();
    for phase in [
        EvidencePhase::Prepared,
        EvidencePhase::Approved,
        EvidencePhase::Authorized,
        EvidencePhase::Started,
        EvidencePhase::Finished,
    ] {
        assert!(phases.contains(&phase), "{phase:?} recorded");
    }
    let end = f.finished(view.id);
    assert_eq!(end.outcome, Some("succeeded"));
    assert!(!end.cancelled);
    assert_eq!(end.target, None, "a door's run is Restricted");
    assert!(end.target_digest.is_some());

    // Mother continues in the same run: File B.
    f.create("style.txt", "h1 { color: brown; }\n").expect("B");
    assert_eq!(f.entries(), vec![NAME.to_string(), "style.txt".to_string()]);
    f.gov
        .files_undo(&*f.owner)
        .expect("the last create is undone");
    assert_eq!(f.entries(), vec![NAME.to_string()], "only B was undone");
    assert_eq!(f.owner.undos().last().unwrap().file, "style.txt");
}

// ---------------------------------------------------------------- D-O5, D-O6

/// D-O5: a commitment that reached its deadline before it started is
/// refused, with nothing written, on either clock.
#[test]
fn m2d_b2_d_o5_commitment_expiry_before_start() {
    for wall in [false, true] {
        let f = Fx::new();
        let view = f.propose(NAME, CONTENT).unwrap();
        if wall {
            f.clock.suspend(FILE_TTL);
        } else {
            f.clock.advance(FILE_TTL);
        }
        assert_eq!(class(f.authorize(&view)), "expired");
        assert!(f.execute(&view).is_err());
        let view = f.ready("b.txt", "b\n");
        if wall {
            f.clock.suspend(FILE_TTL);
        } else {
            f.clock.advance(FILE_TTL);
        }
        assert_eq!(class(f.execute(&view)), "expired");
        assert_eq!(f.entries(), Vec::<String>::new(), "wall: {wall}");
        assert!(!f.events().iter().any(|e| e == "apply.prepared"));
    }
}

/// D-O6 at the broker: an authorized commitment executes once; executing
/// or authorizing it again is refused, and one file exists.
#[test]
fn m2d_b2_d_o6_replay_is_refused() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.execute(&view).unwrap();
    assert!(f.execute(&view).is_err(), "executed once");
    assert!(f.authorize(&view).is_err(), "authorized once");
    assert_eq!(f.entries(), vec![NAME.to_string()]);
    let applies = f.events().iter().filter(|e| *e == "apply.prepared").count();
    assert_eq!(applies, 1);
}

// ---------------------------------------------------------------- D-O7

/// D-O7, ordering A (MD-2, MD-3): each cause landing at the final check
/// (`files.before_final`, after the candidate is written and synced)
/// refuses the publication: no file at the name, no temp, the kernel's
/// rollback record names the cause, the broker's end record is not a
/// success, and only codes reach the caller.
#[test]
fn m2d_b2_d_o7_ordering_a_each_cause_refuses_publication() {
    for cause in CAUSES {
        let f = cause.fixture();
        let view = f.ready(NAME, CONTENT);
        cause.arm(&f, at::FILE_BEFORE_FINAL);
        let refused = f.execute(&view).expect_err("never published");
        assert_eq!(
            f.entries(),
            Vec::<String>::new(),
            "{cause:?}: nothing at all"
        );
        let rolled: Vec<serde_json::Value> = f
            .ledger()
            .into_iter()
            .filter(|(_, event, _)| event == "apply.rolled_back")
            .map(|(_, _, payload)| payload)
            .collect();
        assert_eq!(rolled.len(), 1, "{cause:?}");
        assert_eq!(rolled[0]["failed"], "interrupted", "{cause:?}");
        assert_eq!(rolled[0]["interrupted"], cause.code(), "{cause:?}");
        assert!(!f.events().iter().any(|e| e == "apply.completed"));
        let end = f.finished(view.id);
        assert_ne!(end.outcome, Some("succeeded"), "{cause:?}");
        assert_eq!(end.cancelled, cause.cancels(), "{cause:?}");
        let text = format!("{refused:?} {refused}");
        assert!(!text.contains(NAME) && !text.contains("Rosa"), "{cause:?}");
        // Nothing is left to undo.
        assert!(f.gov.files_undo(&*f.owner).is_err(), "{cause:?}");
        assert!(f.owner.undos().is_empty(), "{cause:?}");
    }
}

// ---------------------------------------------------------------- D-O8

/// D-O8 and M4′, ordering B: each cause landing just after the final check
/// (`files.after_final`) lets exactly that one publication complete: one
/// file with the approved bytes, `apply.completed`, the broker's
/// `Succeeded` with `cancelled` set only when the run's token was (never
/// manufactured), and nothing after it: the run's next create is refused
/// whenever the cause ended the run or its authority. The file is
/// undoable: after the Owner's resume for the emergency stop, at once
/// otherwise, and refused while the kernel stop is in force.
#[test]
fn m2d_b2_d_o8_ordering_b_each_cause_lets_one_publication_complete() {
    for cause in CAUSES {
        let f = cause.fixture();
        let view = f.ready(NAME, CONTENT);
        cause.arm(&f, at::FILE_AFTER_FINAL);
        let output = f.execute(&view).expect("the one publication completes");
        assert_eq!(output.meta[0], ("op".to_string(), "create".to_string()));
        assert_eq!(f.entries(), vec![NAME.to_string()], "{cause:?}");
        assert_eq!(std::fs::read(f.ws.join(NAME)).unwrap(), CONTENT.as_bytes());
        assert!(f.events().iter().any(|e| e == "apply.completed"));
        let end = f.finished(view.id);
        assert_eq!(end.outcome, Some("succeeded"), "{cause:?}");
        assert_eq!(end.cancelled, cause.cancels(), "{cause:?}");

        let next = f.propose("next.txt", "x\n");
        match cause {
            Cause::EmergencyStop | Cause::Cancel | Cause::Revoke | Cause::GrantExpiry => {
                assert!(next.is_err(), "{cause:?}: nothing after it")
            }
            _ => {}
        }
        assert!(!f.ws.join("next.txt").exists());

        match cause {
            Cause::EmergencyStop => {
                assert_eq!(class(f.gov.files_undo(&*f.owner)), "emergency_stopped");
                f.gov.resume(&*f.owner).unwrap();
                f.gov.files_undo(&*f.owner).expect("undo after resume");
            }
            Cause::KernelStop => {
                assert_eq!(class(f.gov.files_undo(&*f.owner)), "emergency_stopped");
                assert_eq!(f.entries(), vec![NAME.to_string()]);
                continue;
            }
            _ => {
                f.gov.files_undo(&*f.owner).expect("undo offered");
            }
        }
        assert_eq!(f.entries(), Vec::<String>::new(), "{cause:?}: undone");
    }
}

// ---------------------------------------------------------------- D-O9, Codex 3 and 5

/// Execute `view` on another thread, held at `point` by a gate.
fn held(
    f: &Fx,
    view: &CommitmentView,
    point: &'static str,
) -> (Gate, std::thread::JoinHandle<bool>) {
    let gate = Gate::new();
    let holding = gate.clone();
    f.gov.files().hooks().arm(point, move || holding.hold());
    let (gov, id, run) = (f.gov.clone(), view.id, f.run);
    let worker = std::thread::spawn(move || gov.execute(id, &mother(), run).is_ok());
    gate.reached();
    (gate, worker)
}

/// D-O9 (Codex D2 condition 5): the emergency Stop returns within a bounded
/// deadline while an execution is held at either side of the final check;
/// the held execution then publishes (after) or does not (before).
#[test]
fn m2d_b2_d_o9_stop_does_not_wait_for_a_held_execution() {
    for (point, publishes) in [(at::FILE_BEFORE_FINAL, false), (at::FILE_AFTER_FINAL, true)] {
        let f = Fx::new();
        let view = f.ready(NAME, CONTENT);
        let (gate, worker) = held(&f, &view, point);
        let started = Instant::now();
        f.gov.emergency_stop();
        assert!(
            started.elapsed() < STOP_RETURNS_WITHIN,
            "{point}: Stop waited"
        );
        assert!(f.gov.status().emergency_stopped);
        gate.open();
        assert_eq!(worker.join().unwrap(), publishes, "{point}");
        assert_eq!(f.ws.join(NAME).exists(), publishes, "{point}");
    }
}

/// Codex D2 condition 3: Files operations are serialized, so at most one
/// can have crossed the final check when Stop arrives: a second execution
/// while one is held after its final check is refused with nothing
/// written; after the Stop, exactly one file exists.
#[test]
fn m2d_b2_concurrent_executions_at_most_one_crosses() {
    let f = Fx::new();
    let first = f.ready(NAME, CONTENT);
    let second = f.ready("second.txt", "two\n");
    let (gate, worker) = held(&f, &first, at::FILE_AFTER_FINAL);
    assert!(
        f.execute(&second).is_err(),
        "refused while one is under way"
    );
    assert!(!f.ws.join("second.txt").exists());
    f.gov.emergency_stop();
    gate.open();
    assert!(worker.join().unwrap());
    assert_eq!(f.entries(), vec![NAME.to_string()], "exactly one");
}

/// Codex D2 condition 5: a Stop that ends a staged, unexecuted create does
/// no staging cleanup itself (dropping its effect only hands the run over).
/// Amended (Codex review 1 of PR #64, M2): the recovery report deletes and
/// records nothing either; it lists the staging. The cleanup is owned and
/// done only by the next Files call or settle (or the Owner's retry),
/// recorded, and the report then lists nothing for it.
#[test]
fn m2d_b2_stop_defers_staging_cleanup() {
    let f = Fx::new();
    let view = f.propose(NAME, CONTENT).unwrap();
    assert_eq!(f.staged().len(), 1, "one staging area");
    f.gov.emergency_stop();
    assert_eq!(f.staged().len(), 1, "Stop removed nothing itself");
    assert!(f.execute(&view).is_err());
    let report = f.gov.files_recovery_report().unwrap();
    assert_eq!(f.staged().len(), 1, "the report deletes nothing");
    assert!(
        !f.events().iter().any(|e| e == "staging.discarded"),
        "nor records a cleanup"
    );
    assert!(
        report.iter().any(|entry| entry
            .findings
            .contains(&RecoveryFinding::StagingAwaitingApproval)),
        "it lists the staging"
    );
    f.gov.settle();
    assert!(f.staged().is_empty(), "the next settle cleaned it");
    assert!(f.events().iter().any(|e| e == "staging.discarded"));
    let report = f.gov.files_recovery_report().unwrap();
    assert!(report.iter().all(|entry| entry.findings.is_empty()));
    assert_eq!(f.entries(), Vec::<String>::new());
}

// ---------------------------------------------------------------- hold (R3, M5′)

/// R3: the Owner's hold is startup configuration of at most 30 seconds,
/// set before the workspace; during it, a cancellation or the commitment's
/// expiry is seen at once and publishes nothing; without one, the hold
/// ends and the ordinary final check still decides.
#[test]
fn m2d_b2_the_hold_is_bounded_and_never_a_bypass() {
    let f = Fx::new();
    assert_eq!(
        class(f.gov.hold_files_before_publish(Duration::from_secs(1))),
        "invalid_action",
        "after the workspace: not startup configuration"
    );
    let root = temp_root("m2d-b2-hold");
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let fresh = broker(
        &root,
        &evidence,
        &Arc::new(ManualClock::default()),
        &StopAuthority::new(),
    );
    assert_eq!(
        class(fresh.hold_files_before_publish(Duration::from_secs(31))),
        "invalid_action"
    );
    fresh
        .hold_files_before_publish(Duration::from_secs(30))
        .expect("30 seconds is allowed");

    let hold = Duration::from_secs(20);
    for expiry in [false, true] {
        let f = Fx::with(Some(hold), LONG);
        let view = f.ready(NAME, CONTENT);
        let (tx, rx) = mpsc::channel();
        f.gov
            .files()
            .hooks()
            .arm(at::FILE_BEFORE_FINAL, move || tx.send(()).unwrap());
        let (gov, id, run) = (f.gov.clone(), view.id, f.run);
        let worker = std::thread::spawn(move || gov.execute(id, &mother(), run).is_ok());
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let interrupted = Instant::now();
        if expiry {
            f.clock.advance(FILE_TTL);
        } else {
            let _ = f.door.cancel_run(f.run).unwrap().write();
        }
        assert!(!worker.join().unwrap(), "expiry: {expiry}");
        assert!(interrupted.elapsed() < hold / 2, "seen during the hold");
        assert_eq!(f.entries(), Vec::<String>::new());
    }

    let short = Duration::from_millis(300);
    let f = Fx::with(Some(short), LONG);
    let view = f.ready(NAME, CONTENT);
    let started = Instant::now();
    f.execute(&view).expect("published after the hold");
    assert!(started.elapsed() >= short, "held");
    let f = Fx::with(Some(short), LONG);
    let view = f.ready(NAME, CONTENT);
    Cause::Revoke.arm(&f, at::FILE_BEFORE_FINAL);
    assert!(
        f.execute(&view).is_err(),
        "the final check follows the hold"
    );
    assert_eq!(f.entries(), Vec::<String>::new());
}

// ---------------------------------------------------------------- D-O11

/// D-O11 at the broker, a crash-point fixture: while one execution is held
/// after its final check (publication status unknown) and another create
/// awaits approval, a second broker on the same state (a restart) reports
/// the first as prepared without an outcome, with its planned temp id and
/// path digest, and the second as staging awaiting approval. The report
/// sweeps nothing (MD-15): a temp at the planned name, the staging and the
/// workspace are as they were. A ledger that cannot be read is an error,
/// never an empty report.
#[test]
fn m2d_b2_d_o11_startup_recovery_report_sweeps_nothing() {
    let f = Fx::new();
    let _waiting = f.propose("waiting.txt", "w\n").unwrap();
    let view = f.ready(NAME, CONTENT);
    let (gate, worker) = held(&f, &view, at::FILE_AFTER_FINAL);

    let restarted = broker(
        &f.root,
        &Arc::new(MemoryEvidence::new(1_000)),
        &Arc::new(ManualClock::default()),
        &StopAuthority::new(),
    );
    let report = restarted.files_recovery_report().unwrap();
    let findings: Vec<RecoveryFinding> = report.iter().flat_map(|e| e.findings.clone()).collect();
    let prepared = findings
        .iter()
        .find_map(|finding| match finding {
            RecoveryFinding::PreparedWithoutOutcome { files } => Some(files.clone()),
            _ => None,
        })
        .expect("the held apply is prepared without an outcome");
    assert_eq!(prepared.len(), 1);
    let temp_id = prepared[0].temp_id.clone().expect("its planned temp id");
    let digest = prepared[0].path_sha256.clone().expect("its path digest");
    assert_eq!(digest.len(), 64);
    assert!(findings.contains(&RecoveryFinding::StagingAwaitingApproval));

    let temp = f.ws.join(format!(".nexus-coding-run-tmp-{temp_id}"));
    std::fs::write(&temp, b"left behind").unwrap();
    let (before, staged) = (f.entries(), f.staged());
    assert_eq!(restarted.files_recovery_report().unwrap(), report);
    assert_eq!(f.entries(), before, "no file or temp deleted");
    assert_eq!(f.staged(), staged, "no staging deleted");
    gate.open();
    assert!(worker.join().unwrap());
    assert!(temp.exists());

    let root = temp_root("m2d-b2-unreadable");
    let fresh = broker(
        &root,
        &Arc::new(MemoryEvidence::new(1_000)),
        &Arc::new(ManualClock::default()),
        &StopAuthority::new(),
    );
    let state = fresh.files().state_dir_for_test();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("ledger.db"), b"not a ledger").unwrap();
    assert!(fresh.files_recovery_report().is_err(), "never empty");
}

// ---------------------------------------------------------------- D-O12

/// D-O12 with the kernel staging: a create refused after its staging
/// exists (here: its run cancelled while the commitment is recorded)
/// leaves no staging, and the discard is recorded; refusals before it
/// stage nothing.
#[test]
fn m2d_b2_d_o12_refused_prepares_leave_no_staging() {
    let f = Fx::new();
    for (name, content) in [(".x", "x"), ("big.txt", &*"x".repeat(8 * 1024 + 1))] {
        assert!(f.propose(name, content).is_err());
    }
    assert!(f.staged().is_empty());
    assert!(f.ledger().is_empty(), "nothing staged, nothing recorded");
    let door = f.door.clone();
    let run = f.run;
    f.gov.hooks().arm(at::PREPARE_BEFORE_INSERT, move || {
        let _ = door.cancel_run(run).unwrap().write();
    });
    assert!(f.propose(NAME, CONTENT).is_err());
    assert!(
        f.staged().is_empty(),
        "a refused prepare discards its staging"
    );
    assert!(f.events().iter().any(|e| e == "staging.discarded"));
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .iter()
        .all(|v| v.state.is_final()));
}

// ---------------------------------------------------------------- D-O16, D-O17

/// D-O16 through the composed route: no yes, execute without an
/// authorization, the approval of A used for B, the staged candidate
/// changed after the approval, and the policy generation moved after it
/// are each refused with nothing written; A itself still executes.
#[test]
fn m2d_b2_d_o16_the_bridge_through_the_composed_route() {
    let f = Fx::new();
    f.owner.actions.store(false, Ordering::SeqCst);
    let declined = f.propose("declined.txt", "no\n").unwrap();
    assert_eq!(class(f.authorize(&declined)), "declined");
    assert!(f.execute(&declined).is_err());
    f.owner.actions.store(true, Ordering::SeqCst);

    let unapproved = f.propose("unapproved.txt", "u\n").unwrap();
    assert!(f.execute(&unapproved).is_err(), "no authorization");
    let a = f.ready(NAME, CONTENT);
    let b = f.propose("b.txt", "b\n").unwrap();
    assert!(f.execute(&b).is_err(), "A's approval is not B's");
    assert!(f.entries().is_empty());
    f.execute(&a).expect("A executes");
    assert_eq!(f.entries(), vec![NAME.to_string()]);

    // The staged candidate changed after the Owner approved it.
    let g = Fx::new();
    let view = g.ready(NAME, CONTENT);
    let staged = find(&g.state().join("coding-runs"), NAME).expect("the staged candidate");
    std::fs::write(&staged, b"<script>not what the Owner saw</script>\n").unwrap();
    assert!(g.execute(&view).is_err(), "staging changed after authorize");
    assert!(g.entries().is_empty());

    // The policy generation moved after the approval.
    let h = Fx::new();
    let view = h.ready(NAME, CONTENT);
    let other = h
        .gov
        .request_grant(
            &GrantRequest::Files {
                workspace: h.gov.workspace().unwrap().id,
            },
            LONG,
            &*h.owner,
        )
        .unwrap();
    h.gov.revoke_grant(other).unwrap();
    assert!(h.execute(&view).is_err(), "the generation moved");
    let state = h.gov.authority().commitments().view(view.id).unwrap().state;
    assert_eq!(
        state,
        crate::authority::commitment::CommitmentState::Revoked
    );
    assert!(h.entries().is_empty());
}

fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.file_name().is_some_and(|n| n == name) && path.is_file() {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find(&path, name) {
                return Some(found);
            }
        }
    }
    None
}

/// D-O17 at the broker: a Files preparation's effect digests exactly what
/// its action commits to, and that is `parameters_digest` of its
/// components: the admission, the broker run, the kernel run, the Files
/// grant, the kernel binding (Mother's fixed agent bytes and the broker
/// run's), the full review hash and the name.
#[test]
fn m2d_b2_d_o17_the_effect_digests_its_components() {
    let f = Fx::new();
    let preparation = f
        .gov
        .files()
        .prepare(
            f.gov.authority(),
            f.run,
            &FileCreate {
                name: NAME.into(),
                content: CONTENT.into(),
            },
            &f.stop,
        )
        .expect("prepared");
    assert_eq!(
        preparation.effect.parameters(),
        preparation.action.parameters
    );
    let ledger = f.ledger();
    let kernel_run = ledger[0].0.clone();
    let review = ledger
        .iter()
        .rev()
        .find(|(_, event, _)| event == "review.computed")
        .map(|(_, _, payload)| payload["binding"].as_str().unwrap().to_string())
        .unwrap();
    let admission = f.gov.authority().runs().files_admission(f.run).unwrap();
    let kernel_run: [u8; 16] = hex::decode(kernel_run.replace('-', ""))
        .unwrap()
        .try_into()
        .unwrap();
    let review_hash: [u8; 32] = hex::decode(review).unwrap().try_into().unwrap();
    let expected = parameters_digest(&Components {
        admission: &admission,
        broker_run: f.run,
        kernel_run,
        grant: f.grant,
        binding_agent: KERNEL_AGENT,
        binding_run: *f.run.as_bytes(),
        review_hash,
        name: NAME,
    });
    assert_eq!(preparation.effect.parameters(), expected);
    assert_eq!(&KERNEL_AGENT, b"MotherNexusFiles");
}

// ---------------------------------------------------------------- D-O19, H2′

/// D-O19 at the broker: the Owner's dialog shows the workspace, the name,
/// the exact bytes, the full SHA-256 and the identity of the file that
/// will be removed; after it the workspace is exactly as before the create;
/// a second undo is refused; after a restart nothing is offered.
#[test]
fn m2d_b2_d_o19_undo_restores_the_exact_pre_state_once() {
    use std::os::unix::fs::MetadataExt;
    let f = Fx::new();
    f.create(NAME, CONTENT).unwrap();
    let meta = std::fs::metadata(f.ws.join(NAME)).unwrap();
    f.gov.files_undo(&*f.owner).expect("undone");
    assert_eq!(f.entries(), Vec::<String>::new(), "the exact pre-state");
    assert_eq!(
        f.owner.undos(),
        vec![UndoConfirmation {
            workspace: "ws".into(),
            file: NAME.into(),
            bytes: CONTENT.len() as u64,
            sha256: sha256_hex(CONTENT.as_bytes()),
            device: meta.dev(),
            inode: meta.ino(),
        }]
    );
    assert!(f.events().iter().any(|e| e == "restore.completed"));
    assert!(f.gov.files_undo(&*f.owner).is_err(), "once");
    assert_eq!(f.owner.undos().len(), 1, "no second dialog");

    let g = Fx::new();
    g.create(NAME, CONTENT).unwrap();
    let restarted = broker(
        &g.root,
        &Arc::new(MemoryEvidence::new(1_000)),
        &Arc::new(ManualClock::default()),
        &StopAuthority::new(),
    );
    assert!(restarted.files_undo(&*g.owner).is_err(), "after a restart");
    assert!(g.owner.undos().is_empty());
    assert_eq!(std::fs::read(g.ws.join(NAME)).unwrap(), CONTENT.as_bytes());
}

/// D-O19: an Owner edit makes the undo stale with nothing touched; a no
/// does nothing and the undo stays offered; while a stop is in force no
/// dialog is shown.
#[test]
fn m2d_b2_d_o19_stale_denied_and_stopped_touch_nothing() {
    let f = Fx::new();
    f.create(NAME, CONTENT).unwrap();
    f.owner.undo.store(false, Ordering::SeqCst);
    assert_eq!(class(f.gov.files_undo(&*f.owner)), "declined");
    assert_eq!(f.entries(), vec![NAME.to_string()]);
    f.owner.undo.store(true, Ordering::SeqCst);

    f.gov.emergency_stop();
    let shown = f.owner.undos().len();
    assert_eq!(class(f.gov.files_undo(&*f.owner)), "emergency_stopped");
    assert_eq!(f.owner.undos().len(), shown, "no dialog while stopped");
    f.gov.resume(&*f.owner).unwrap();

    std::fs::write(f.ws.join(NAME), b"the Owner's edit\n").unwrap();
    assert_eq!(class(f.gov.files_undo(&*f.owner)), "target_changed");
    assert_eq!(
        std::fs::read(f.ws.join(NAME)).unwrap(),
        b"the Owner's edit\n"
    );
    assert_eq!(f.entries(), vec![NAME.to_string()], "nothing touched");
}

/// H2′: a Stop and resume during the undo dialog, or a second Stop after
/// the undo is admitted (at `files.undo_approved`), or the undo's write
/// grant revoked there, interrupts it with nothing done; a new undo
/// request (with its own dialog) is then offered and completes.
#[test]
fn m2d_b2_h2_undo_watches_are_taken_at_its_start() {
    for interference in ["dialog", "admitted", "write_grant"] {
        let f = Fx::new();
        f.create(NAME, CONTENT).unwrap();
        let (gov, owner) = (f.gov.clone(), f.owner.clone());
        match interference {
            "dialog" => f.owner.during_undo(move || {
                gov.emergency_stop();
                gov.resume(&*owner).unwrap();
            }),
            "admitted" => f.gov.files().hooks().arm(at::FILE_UNDO_APPROVED, move || {
                gov.emergency_stop();
                gov.resume(&*owner).unwrap();
            }),
            _ => f.gov.files().hooks().arm(at::FILE_UNDO_APPROVED, move || {
                assert!(gov.files().revoke_write_grant_for_test());
            }),
        }
        assert!(f.gov.files_undo(&*f.owner).is_err(), "{interference}");
        assert_eq!(
            f.entries(),
            vec![NAME.to_string()],
            "{interference}: nothing done"
        );
        f.gov.files_undo(&*f.owner).expect("a new undo request");
        assert_eq!(f.owner.undos().len(), 2, "{interference}: asked again");
        assert!(f.entries().is_empty(), "{interference}");
    }
}

// ---------------------------------------------------------------- D-O21, D-O24

/// D-O21 at the broker for a create that succeeds: canaries in the name
/// and the content reach neither the ledger's bytes (main, `-wal`, `-shm`)
/// nor any evidence record nor the result.
#[test]
fn m2d_b2_d_o21_a_created_file_leaves_no_trace() {
    let f = Fx::new();
    let c = format!(
        "NXCANARY-{}",
        hex::encode(crate::authority::ids::random16())
    );
    let cl = format!(
        "nxsecret_{}",
        &hex::encode(crate::authority::ids::random16())[..16]
    );
    let name = format!("{c}.txt");
    let output = f
        .create(&name, &format!("{c}\n{cl}\n"))
        .expect("a valid name");
    assert!(f.ws.join(&name).exists());
    let mut haystack = format!("{output:?}");
    for record in f.evidence.records() {
        haystack.push_str(&format!("{record:?}\n{}\n", record.to_json()));
    }
    let mut bytes = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        if let Ok(found) = std::fs::read(f.state().join(format!("ledger.db{suffix}"))) {
            bytes.extend(found);
        }
    }
    haystack.push_str(&String::from_utf8_lossy(&bytes));
    for canary in [&c, &cl] {
        let mut forms = vec![
            canary.to_string(),
            canary.to_lowercase(),
            canary.to_uppercase(),
            hex::encode(canary.as_bytes()),
        ];
        // Fragments of at least 8 characters, as the P0 scanner splits.
        let size = (canary.len() / 4).max(8);
        let mut at = 0;
        while at + size <= canary.len() {
            forms.push(canary[at..at + size].to_string());
            at += size;
        }
        for form in forms {
            assert!(!haystack.contains(&form), "{form} leaked");
        }
    }
}

/// D-O24 at the broker: a Files failure reaches the caller and the evidence
/// as codes only; its detail (the name, the kernel's outcome) is shown
/// only through `files_working_detail`, once.
#[test]
fn m2d_b2_d_o24_failures_are_codes_and_detail_is_the_owner_s() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    Cause::Revoke.arm(&f, at::FILE_BEFORE_FINAL);
    let refused = f.execute(&view).expect_err("interrupted");
    let mut text = format!("{refused:?} {refused}");
    for record in f.records_of(view.id) {
        text.push_str(&format!("{record:?}\n{}\n", record.to_json()));
    }
    assert!(!text.contains(NAME), "no name in codes");
    assert!(!text.contains("interrupted before publication"));
    let detail = f.detail(view.id).expect("the Owner's detail");
    let all: String = detail.iter().map(|(l, v)| format!("{l}: {v}\n")).collect();
    assert!(all.contains(NAME), "{all}");
    assert!(all.contains("Revoked"), "{all}");
    assert!(f.detail(view.id).is_none(), "taken once");
    assert!(f.detail(CommitmentId::fresh()).is_none());
}

// ---------------------------------------------------------------- oracle-add: MD-15

/// D-O11, oracle-add: MD-15 (a report that sweeps) survived the frozen
/// D-O11 oracle above, which compares the state only between two report
/// calls of a restarted broker (one without the workspace). No report call
/// sweeps anything, the first included, on a restarted broker and on the
/// broker that holds the workspace: a left temp in the workspace and the
/// staging of a create awaiting approval are as they were before the first
/// report, and that create still executes.
#[test]
fn m2d_b2_d_o11_b_no_report_call_sweeps_anything() {
    let f = Fx::new();
    let waiting = f.ready("waiting.txt", "w\n");
    let temp =
        f.ws.join(".nexus-coding-run-tmp-0123456789abcdef0123456789abcdef");
    std::fs::write(&temp, b"left behind").unwrap();
    let (entries, staged) = (f.entries(), f.staged());
    assert!(!staged.is_empty(), "the waiting create is staged");
    let restarted = broker(
        &f.root,
        &Arc::new(MemoryEvidence::new(1_000)),
        &Arc::new(ManualClock::default()),
        &StopAuthority::new(),
    );
    restarted.files_recovery_report().unwrap();
    f.gov.files_recovery_report().unwrap();
    assert_eq!(f.entries(), entries, "no workspace entry deleted");
    assert_eq!(f.staged(), staged, "no staging deleted");
    assert_eq!(std::fs::read(&temp).unwrap(), b"left behind");
    f.execute(&waiting)
        .expect("the waiting create still executes");
    assert!(f.ws.join("waiting.txt").exists());
}
