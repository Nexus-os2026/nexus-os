//! M2d-A pre-registered oracles (design v2 §6.3, §6.6, §6.7, §6.10 and the
//! binding §11, with Codex's D1/D2 conditions): one create-only publication
//! after a final check of every authority, both orderings for every cause the
//! kernel can see, cleanup ownership, the undo checkpoint with watches taken
//! when the undo starts (H2′), temp ids recorded before creation and the
//! startup recovery report (M6′), with crash-point fixtures.
//!
//! Each test uses its own stop authorities (never the process's): `stop` is
//! the run's own kernel authority, `broker` the authority the run observes
//! (the broker's kernel stop). The observer stands in for the broker's
//! execution guard. Rows marked "control" are green on the base by design.

use super::p0b_ledger_privacy::{leaks, Canary};
use super::*;
use crate::coding_run::apply::{
    HookPoint, APPLY_HOOK, REMOVE_TEMP_FAILS, RENAME_FAILS, SYNC_FAILS,
};
use crate::coding_run::ledger::path_sha256;
use crate::coding_run::scope::TEMP_PREFIX;
use crate::emergency_stop::{StopAuthority, StopController};
use std::cell::{Cell, RefCell};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Instant;

const NAME: &str = "notes.txt";
const CONTENT: &str = "first line\nsecond line\n";
const WAIT: Duration = Duration::from_secs(10);

// ── Fixture ─────────────────────────────────────────────────────────────────

struct Pick(PathBuf);

impl FolderPicker for Pick {
    fn pick_folder(&self) -> Option<PathBuf> {
        Some(self.0.clone())
    }
}

struct Ws {
    _tmp: tempfile::TempDir,
    /// The owner's folder, empty at selection.
    root: PathBuf,
    /// Backend state: the ledger and the staging parent live here.
    state: PathBuf,
    ledger_path: PathBuf,
    ledger: Arc<CodingRunLedger>,
    registry: Arc<WorkspaceAuthorityRegistry>,
    projects: ProjectRegistry,
    info: ProjectInfo,
    stop: Arc<StopAuthority>,
    stop_ctl: StopController,
    broker: Arc<StopAuthority>,
    broker_ctl: StopController,
}

/// A workspace in a process made non-dumpable, as the host does at startup
/// before any Files work (Codex review 2 of PR #58). Process-wide and
/// idempotent.
fn new_ws() -> Ws {
    nix::sys::prctl::set_dumpable(false).expect("PR_SET_DUMPABLE 0");
    fixture_ws()
}

/// A workspace, the process left as it is.
fn fixture_ws() -> Ws {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    let root = base.join("workspace");
    let state = base.join("state");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&state).unwrap();
    let ledger_path = state.join("ledger.db");
    let ledger = Arc::new(CodingRunLedger::open(&ledger_path).unwrap());
    let registry = Arc::new(WorkspaceAuthorityRegistry::new());
    let projects = ProjectRegistry::reserving(Arc::clone(&registry), &[&state]);
    let info = projects.select(&Pick(root.clone())).unwrap();
    let stop = StopAuthority::new();
    let stop_ctl = stop.claim_controller().unwrap();
    let broker = StopAuthority::new();
    let broker_ctl = broker.claim_controller().unwrap();
    Ws {
        _tmp: tmp,
        root,
        state,
        ledger_path,
        ledger,
        registry,
        projects,
        info,
        stop,
        stop_ctl,
        broker,
        broker_ctl,
    }
}

impl Ws {
    fn store(&self) -> Arc<dyn LedgerStore> {
        Arc::clone(&self.ledger) as Arc<dyn LedgerStore>
    }

    fn staging(&self) -> StagingParent {
        StagingParent::within(&self.state).unwrap()
    }

    fn write_grant(&self, binding: WorkspaceBinding) -> ProjectGrant {
        self.projects
            .grant_for_apply(self.info.id, binding)
            .unwrap()
    }

    /// Approve the run's review, then apply it observed by `guard`.
    fn observed(
        &self,
        run: &mut CodingRun,
        binding: WorkspaceBinding,
        guard: &dyn ApplyInterrupt,
    ) -> Result<ApplyReport, ApplyError> {
        let approval = run
            .request_approval(&self.info.name, &Answer::yes())
            .unwrap();
        run.apply_observed(approval, self.write_grant(binding), &self.staging(), guard)
    }

    fn records(&self, run: &CodingRun) -> Vec<(String, Value)> {
        self.ledger
            .verify_run(run.id().ledger_key())
            .unwrap()
            .into_iter()
            .map(|r| (r.event_kind, serde_json::from_str(&r.payload).unwrap()))
            .collect()
    }

    fn kinds(&self, run: &CodingRun) -> Vec<String> {
        self.records(run).into_iter().map(|(k, _)| k).collect()
    }

    /// The temp id the apply recorded for its one file before creating it.
    fn temp_id(&self, run: &CodingRun) -> String {
        let records = self.records(run);
        let prepared = last_of(&records, "apply.prepared").expect("apply.prepared");
        prepared["files"][0]["temp_id"]
            .as_str()
            .expect("files[0].temp_id recorded")
            .to_string()
    }
}

fn last_of<'a>(records: &'a [(String, Value)], kind: &str) -> Option<&'a Value> {
    records
        .iter()
        .rev()
        .find(|(k, _)| k == kind)
        .map(|(_, v)| v)
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn files_scopes(names: &[&str]) -> RunScopes {
    RunScopes::new(
        ScopeSet::new([ScopeEntry::WholeProject]),
        ScopeSet::new(names.iter().map(|n| ScopeEntry::File(rel(n)))),
        ScopeSet::default(),
    )
    .unwrap()
}

/// A run observing `ws.broker`, admitted under `ws.stop`, staged from the
/// workspace, with `edits` applied and structurally verified.
fn verified_with(
    ws: &Ws,
    store: Arc<dyn LedgerStore>,
    edits: Vec<CandidateEdit>,
    scopes: RunScopes,
) -> (CodingRun, WorkspaceBinding) {
    let binding = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    };
    let grant = ws.projects.grant_for_run(ws.info.id, binding).unwrap();
    let mut run = with_test_stop_authority(ws.stop.clone(), || {
        CodingRun::create_for_project_observing(store, grant, scopes, &ws.broker)
    })
    .unwrap();
    run.grant(&ws.staging()).unwrap();
    run.snapshot().unwrap();
    for edit in edits {
        run.edit(edit).unwrap();
    }
    assert!(run.verify_structural().unwrap().passed());
    (run, binding)
}

fn verified_file_with(ws: &Ws, store: Arc<dyn LedgerStore>) -> (CodingRun, WorkspaceBinding) {
    verified_with(
        ws,
        store,
        vec![create(NAME, CONTENT)],
        files_scopes(&[NAME]),
    )
}

fn verified_file(ws: &Ws) -> (CodingRun, WorkspaceBinding) {
    verified_with(
        ws,
        ws.store(),
        vec![create(NAME, CONTENT)],
        files_scopes(&[NAME]),
    )
}

/// The owner's native confirmation stand-in: remembers what it showed and
/// can act while the dialog is open.
struct Answer {
    yes: bool,
    during: RefCell<Option<Box<dyn FnOnce()>>>,
    shown: RefCell<Vec<ConfirmationRequest>>,
}

impl Answer {
    fn yes() -> Self {
        Self {
            yes: true,
            during: RefCell::new(None),
            shown: RefCell::new(Vec::new()),
        }
    }
    fn no() -> Self {
        Self {
            yes: false,
            ..Self::yes()
        }
    }
    fn yes_after(during: impl FnOnce() + 'static) -> Self {
        Self {
            during: RefCell::new(Some(Box::new(during))),
            ..Self::yes()
        }
    }
}

impl OwnerConfirmer for Answer {
    fn confirm(&self, request: &ConfirmationRequest) -> bool {
        self.shown.borrow_mut().push(request.clone());
        let during = self.during.borrow_mut().take();
        if let Some(during) = during {
            during();
        }
        self.yes
    }
}

/// The broker's execution guard as the kernel sees it.
#[derive(Default)]
struct Guard {
    cause: Cell<Option<InterruptCause>>,
    asked: Cell<usize>,
    passed: Cell<usize>,
    on_pass: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl ApplyInterrupt for Guard {
    fn interrupted(&self) -> Option<InterruptCause> {
        self.asked.set(self.asked.get() + 1);
        self.cause.get()
    }
    fn passed_final_check(&self) {
        self.passed.set(self.passed.get() + 1);
        let on_pass = self.on_pass.borrow_mut().take();
        if let Some(on_pass) = on_pass {
            on_pass();
        }
    }
}

fn with_hooks<T>(
    hook: impl FnMut(HookPoint, &str) -> bool + 'static,
    body: impl FnOnce() -> T,
) -> T {
    APPLY_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
    let out = body();
    APPLY_HOOK.with(|h| *h.borrow_mut() = None);
    out
}

fn seam(cell: &'static std::thread::LocalKey<Cell<bool>>, on: bool) {
    cell.with(|c| c.set(on));
}

/// Everything that can interrupt a publication, as the kernel sees it.
#[derive(Debug, Clone, Copy)]
enum Cause {
    /// The run's own kernel stop (the global one in production).
    RunStop,
    /// The broker's kernel stop the run observes.
    BrokerStop,
    /// The broker's guard: emergency stop, mission cancel, Files grant
    /// revocation, Files grant expiry, commitment deadline.
    Guard(InterruptCause),
    /// The kernel's write grant is revoked.
    WriteRevoked,
    /// The kernel's write grant expires.
    WriteExpired,
}

impl Cause {
    fn expected(self) -> InterruptCause {
        match self {
            Cause::RunStop | Cause::BrokerStop => InterruptCause::EmergencyStop,
            Cause::Guard(cause) => cause,
            Cause::WriteRevoked => InterruptCause::Revoked,
            Cause::WriteExpired => InterruptCause::Expired,
        }
    }

    fn is_stop(self) -> bool {
        matches!(self, Cause::RunStop | Cause::BrokerStop)
    }
}

/// Lands one cause from inside a hook or the guard's callback.
struct Injector {
    stop: Arc<StopAuthority>,
    broker: Arc<StopAuthority>,
    registry: Arc<WorkspaceAuthorityRegistry>,
    grant: WorkspaceGrantId,
    binding: WorkspaceBinding,
    guard: Rc<Guard>,
}

impl Injector {
    fn new(ws: &Ws, grant: &ProjectGrant, guard: &Rc<Guard>) -> Self {
        Self {
            stop: ws.stop.clone(),
            broker: ws.broker.clone(),
            registry: Arc::clone(&ws.registry),
            grant: grant.grant_id(),
            binding: grant.binding(),
            guard: Rc::clone(guard),
        }
    }

    fn land(&self, cause: Cause) {
        match cause {
            Cause::RunStop => {
                self.stop.engage();
            }
            Cause::BrokerStop => {
                self.broker.engage();
            }
            Cause::Guard(c) => self.guard.cause.set(Some(c)),
            Cause::WriteRevoked => self.registry.revoke(self.grant, self.binding).unwrap(),
            Cause::WriteExpired => {
                // Wait until the expiry instant has passed (checked, not
                // assumed): reaching it exactly already means expired.
                let expires = self
                    .registry
                    .resolve(self.grant, self.binding)
                    .ok()
                    .and_then(|g| g.expires_at());
                let deadline = Instant::now() + WAIT;
                while let Some(at) = expires {
                    if SystemTime::now() >= at {
                        break;
                    }
                    assert!(Instant::now() < deadline, "the write grant never expired");
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

fn grant_for(ws: &Ws, binding: WorkspaceBinding, cause: Cause) -> ProjectGrant {
    if matches!(cause, Cause::WriteExpired) {
        ws.projects
            .grant_for_apply_lasting(ws.info.id, binding, Duration::from_millis(1500))
            .unwrap()
    } else {
        ws.write_grant(binding)
    }
}

fn lift(ws: &Ws, cause: Cause) {
    match cause {
        Cause::RunStop => assert!(ws.stop_ctl.lift_if(ws.stop.stop_number())),
        Cause::BrokerStop => assert!(ws.broker_ctl.lift_if(ws.broker.stop_number())),
        _ => {}
    }
}

fn undo(
    ws: &Ws,
    run: &mut CodingRun,
    binding: WorkspaceBinding,
) -> Result<ApplyReport, ApplyError> {
    let approval = run.request_restore(&ws.info.name, &Answer::yes())?;
    run.restore(approval, ws.write_grant(binding))
}

// ── D-O7: Ordering A, a cause before the final check ────────────────────────

/// What the workspace held when the final check came: its entry names, and
/// the bytes of each anonymous file of it this process holds.
type Seen = Rc<RefCell<Option<(Vec<String>, Vec<Vec<u8>>)>>>;

fn ordering_a(cause: Cause) {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let grant = grant_for(&ws, binding, cause);
    let guard = Rc::new(Guard::default());
    let injector = Injector::new(&ws, &grant, &guard);
    let root = ws.root.clone();
    let seen: Seen = Rc::new(RefCell::new(None));
    let saw = Rc::clone(&seen);
    let result = with_hooks(
        move |point, _| {
            if point == HookPoint::BeforeFinalCheck {
                let anonymous = anonymous_files(&root)
                    .into_iter()
                    .map(|fd| std::fs::read(format!("/proc/self/fd/{fd}")).unwrap())
                    .collect();
                *saw.borrow_mut() = Some((entries(&root), anonymous));
                injector.land(cause);
            }
            false
        },
        || run.apply_observed(approval, grant, &ws.staging(), &*guard),
    );
    assert_eq!(
        result,
        Err(ApplyError::Interrupted {
            cause: cause.expected()
        }),
        "{cause:?}"
    );
    // Staging, content and fsync finished before the handoff: the complete
    // candidate existed as an anonymous file of the workspace, with no
    // pathname to substitute, after its intent id was recorded.
    let _temp_id = ws.temp_id(&run);
    assert_eq!(
        seen.borrow().clone(),
        Some((Vec::new(), vec![CONTENT.as_bytes().to_vec()])),
        "{cause:?}: the complete candidate was not held anonymously when the final check came"
    );
    assert!(entries(&ws.root).is_empty(), "{cause:?}: published or left");
    assert_eq!(run.apply_state(), ApplyState::RolledBack, "{cause:?}");
    assert_eq!(guard.passed.get(), 0, "{cause:?}");
    let records = ws.records(&run);
    let outcome = last_of(&records, "apply.rolled_back").expect("apply.rolled_back");
    assert_eq!(outcome["failed"], "interrupted", "{cause:?}");
    assert_eq!(outcome["interrupted"], cause.expected().code(), "{cause:?}");
    assert!(!ws
        .kinds(&run)
        .iter()
        .any(|k| k == "apply.op" || k == "apply.completed"));
    // No retry, no second file.
    assert_eq!(
        run.request_approval(&ws.info.name, &Answer::yes())
            .unwrap_err(),
        ApplyError::InvalidState
    );
}

macro_rules! ordering_a {
    ($($name:ident: $cause:expr;)*) => {$(
        #[test]
        fn $name() {
            ordering_a($cause);
        }
    )*};
}

ordering_a! {
    m2d_a_d_o7_run_stop_before_the_final_check_publishes_nothing: Cause::RunStop;
    m2d_a_d_o7_broker_stop_before_the_final_check_publishes_nothing: Cause::BrokerStop;
    m2d_a_d_o7_guard_stop_before_the_final_check_publishes_nothing: Cause::Guard(InterruptCause::EmergencyStop);
    m2d_a_d_o7_cancel_before_the_final_check_publishes_nothing: Cause::Guard(InterruptCause::Cancelled);
    m2d_a_d_o7_files_revoke_before_the_final_check_publishes_nothing: Cause::Guard(InterruptCause::Revoked);
    m2d_a_d_o7_files_expiry_before_the_final_check_publishes_nothing: Cause::Guard(InterruptCause::Expired);
    m2d_a_d_o7_commitment_deadline_before_the_final_check_publishes_nothing: Cause::Guard(InterruptCause::CommitmentExpired);
    m2d_a_d_o7_write_grant_revoked_before_the_final_check_publishes_nothing: Cause::WriteRevoked;
    m2d_a_d_o7_write_grant_expired_before_the_final_check_publishes_nothing: Cause::WriteExpired;
}

// ── D-O8 / M4′: Ordering B, a cause after the final check ───────────────────

fn ordering_b(cause: Cause, through_guard: bool) {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let grant = grant_for(&ws, binding, cause);
    let guard = Rc::new(Guard::default());
    let injector = Rc::new(Injector::new(&ws, &grant, &guard));
    let hook_injector = Rc::clone(&injector);
    if through_guard {
        *guard.on_pass.borrow_mut() = Some(Box::new(move || injector.land(cause)));
    }
    let result = with_hooks(
        move |point, _| {
            if point == HookPoint::AfterFinalCheck && !through_guard {
                hook_injector.land(cause);
            }
            false
        },
        || run.apply_observed(approval, grant, &ws.staging(), &*guard),
    );
    let report = result.unwrap_or_else(|e| panic!("{cause:?}: {e:?}"));
    assert_eq!(report.files, vec![(rel(NAME), ChangeKind::Create)]);
    // Exactly the approved file, once; nothing else.
    assert_eq!(entries(&ws.root), vec![NAME.to_string()], "{cause:?}");
    let path = ws.root.join(NAME);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CONTENT);
    assert_eq!(std::fs::metadata(&path).unwrap().nlink(), 1);
    assert_eq!((guard.asked.get(), guard.passed.get()), (1, 1), "{cause:?}");
    let kinds = ws.kinds(&run);
    assert_eq!(
        kinds.last().map(String::as_str),
        Some("apply.completed"),
        "{cause:?}: {kinds:?}"
    );
    assert_eq!(run.apply_state(), ApplyState::Applied);
    assert_eq!(
        run.request_approval(&ws.info.name, &Answer::yes())
            .unwrap_err(),
        ApplyError::InvalidState
    );
    // The publication is recorded and can be undone: after the owner's
    // resume for a stop, at once otherwise.
    if cause.is_stop() {
        assert_eq!(
            run.request_restore(&ws.info.name, &Answer::yes())
                .unwrap_err(),
            ApplyError::Refused(Refusal::EmergencyStopped)
        );
        assert_eq!(entries(&ws.root), vec![NAME.to_string()]);
        lift(&ws, cause);
    }
    undo(&ws, &mut run, binding).unwrap_or_else(|e| panic!("{cause:?} undo: {e:?}"));
    assert!(entries(&ws.root).is_empty(), "{cause:?}");
    assert_eq!(run.apply_state(), ApplyState::Restored);
}

macro_rules! ordering_b {
    ($($name:ident: $cause:expr, $through_guard:expr;)*) => {$(
        #[test]
        fn $name() {
            ordering_b($cause, $through_guard);
        }
    )*};
}

ordering_b! {
    m2d_a_d_o8_run_stop_after_the_final_check_publishes_once: Cause::RunStop, false;
    m2d_a_d_o8_broker_stop_after_the_final_check_publishes_once: Cause::BrokerStop, true;
    m2d_a_d_o8_guard_stop_after_the_final_check_publishes_once: Cause::Guard(InterruptCause::EmergencyStop), true;
    m2d_a_d_o8_cancel_after_the_final_check_publishes_once: Cause::Guard(InterruptCause::Cancelled), false;
    m2d_a_d_o8_files_revoke_after_the_final_check_publishes_once: Cause::Guard(InterruptCause::Revoked), true;
    m2d_a_d_o8_files_expiry_after_the_final_check_publishes_once: Cause::Guard(InterruptCause::Expired), false;
    m2d_a_d_o8_commitment_deadline_after_the_final_check_publishes_once: Cause::Guard(InterruptCause::CommitmentExpired), true;
    m2d_a_d_o8_write_grant_revoked_after_the_final_check_publishes_once: Cause::WriteRevoked, false;
    m2d_a_d_o8_write_grant_expired_after_the_final_check_publishes_once: Cause::WriteExpired, true;
}

// ── D-O9 (kernel): Stop never waits for the publication ─────────────────────

#[test]
fn m2d_a_d_o9_stop_is_acknowledged_while_the_publication_is_held() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let grant = ws.write_grant(binding);
    let staging = ws.staging();
    let (at_handoff, handoff) = mpsc::channel::<()>();
    let (release, released) = mpsc::channel::<()>();
    let (finished, done) = mpsc::channel::<()>();
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            let guard = Guard::default();
            let out = with_hooks(
                move |point, _| {
                    if point == HookPoint::AfterFinalCheck {
                        at_handoff.send(()).unwrap();
                        released.recv_timeout(Duration::from_secs(30)).unwrap();
                    }
                    false
                },
                || run.apply_observed(approval, grant, &staging, &guard),
            );
            finished.send(()).unwrap();
            (out, run)
        });
        handoff
            .recv_timeout(WAIT)
            .expect("the apply reached its handoff");
        let started = Instant::now();
        ws.stop.engage();
        ws.broker.engage();
        assert!(started.elapsed() < Duration::from_secs(5), "Stop waited");
        assert!(
            done.try_recv().is_err(),
            "the publication finished before the stop was acknowledged"
        );
        release.send(()).unwrap();
        let (out, _run) = worker.join().unwrap();
        out.unwrap();
    });
    assert_eq!(entries(&ws.root), vec![NAME.to_string()]);
}

// ── D1: create-only, refusing a destination that appears late ───────────────

#[test]
fn m2d_a_d1_create_only_refuses_a_destination_that_appears_after_preflight() {
    for point in [HookPoint::BeforeFinalCheck, HookPoint::AfterFinalCheck] {
        let ws = new_ws();
        let (mut run, binding) = verified_file(&ws);
        let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
        let guard = Guard::default();
        let target = ws.root.join(NAME);
        let owner_file = Rc::new(Cell::new(None));
        let made = Rc::clone(&owner_file);
        let result = with_hooks(
            move |at, _| {
                if at == point {
                    std::fs::write(&target, "the owner's own words\n").unwrap();
                    made.set(Some(std::fs::metadata(&target).unwrap().ino()));
                }
                false
            },
            || run.apply_observed(approval, ws.write_grant(binding), &ws.staging(), &guard),
        );
        assert!(
            matches!(result, Err(ApplyError::RolledBack { .. })),
            "{point:?}: {result:?}"
        );
        let path = ws.root.join(NAME);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "the owner's own words\n",
            "{point:?}: the owner's file was replaced"
        );
        assert_eq!(
            Some(std::fs::metadata(&path).unwrap().ino()),
            owner_file.get()
        );
        // No alternate destination, no second file, no temp left.
        assert_eq!(entries(&ws.root), vec![NAME.to_string()], "{point:?}");
        assert_eq!(guard.asked.get(), 1, "{point:?}: retried");
        let records = ws.records(&run);
        let outcome = last_of(&records, "apply.rolled_back").expect("apply.rolled_back");
        assert_eq!(outcome["failed"], "publish", "{point:?}");
        assert_eq!(run.apply_state(), ApplyState::RolledBack);
    }
}

#[test]
fn m2d_a_d1_an_observed_apply_publishes_exactly_one_new_root_level_file() {
    let cases: Vec<(&str, Vec<CandidateEdit>, Vec<&str>)> = vec![
        (
            "two files",
            vec![create("a.txt", "a\n"), create("b.txt", "b\n")],
            vec!["a.txt", "b.txt"],
        ),
        (
            "a nested file",
            vec![create("dir/c.txt", "c\n")],
            vec!["dir/c.txt"],
        ),
        (
            "a replacement",
            vec![replace("existing.txt", "new\n")],
            vec!["existing.txt"],
        ),
    ];
    for (what, edits, names) in cases {
        let ws = new_ws();
        std::fs::write(ws.root.join("existing.txt"), "old\n").unwrap();
        let before = digest(&ws.root);
        let (mut run, binding) = verified_with(&ws, ws.store(), edits, files_scopes(&names));
        let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
        let result = run.apply_observed(
            approval,
            ws.write_grant(binding),
            &ws.staging(),
            &Guard::default(),
        );
        assert!(
            matches!(result, Err(ApplyError::Refused(Refusal::Unsupported(_)))),
            "{what}: {result:?}"
        );
        assert_eq!(digest(&ws.root), before, "{what}");
        assert!(
            !ws.kinds(&run).iter().any(|k| k == "apply.prepared"),
            "{what}"
        );
    }
}

// ── D-O10: cleanup ownership, injected failures ─────────────────────────────

#[test]
fn m2d_a_d_o10_an_interruption_leaves_nothing_to_remove_even_when_removal_would_fail() {
    for seam_cell in [&REMOVE_TEMP_FAILS, &SYNC_FAILS] {
        let ws = new_ws();
        let (mut run, binding) = verified_file(&ws);
        let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
        let guard = Rc::new(Guard::default());
        let flag = Rc::clone(&guard);
        let result = with_hooks(
            move |point, _| {
                if point == HookPoint::BeforeFinalCheck {
                    flag.cause.set(Some(InterruptCause::Cancelled));
                    seam(seam_cell, true);
                }
                false
            },
            || run.apply_observed(approval, ws.write_grant(binding), &ws.staging(), &*guard),
        );
        seam(seam_cell, false);
        assert_eq!(
            result,
            Err(ApplyError::Interrupted {
                cause: InterruptCause::Cancelled
            })
        );
        assert!(entries(&ws.root).is_empty(), "published or left");
        assert_eq!(run.apply_state(), ApplyState::RolledBack);
        assert!(run.cleanup_owed().is_empty());
        assert_eq!(run.retry_cleanup(), Err(ApplyError::InvalidState));
        let records = ws.records(&run);
        let outcome = last_of(&records, "apply.rolled_back").expect("apply.rolled_back");
        assert_eq!(outcome["failed"], "interrupted");
        assert_eq!(outcome["interrupted"], "Cancelled");
        assert!(!ws
            .kinds(&run)
            .iter()
            .any(|k| k == "apply.recovery_required"));
        assert!(analyze_apply(ws.ledger.as_ref(), run.id().ledger_key())
            .unwrap()
            .is_empty());
    }
}

#[test]
fn m2d_a_d_o10_a_failed_rename_publishes_nothing_and_removes_the_temp() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let guard = Guard::default();
    seam(&RENAME_FAILS, true);
    let result = run.apply_observed(approval, ws.write_grant(binding), &ws.staging(), &guard);
    seam(&RENAME_FAILS, false);
    assert_eq!(
        result,
        Err(ApplyError::RolledBack {
            failed: "publish".to_string()
        })
    );
    assert!(entries(&ws.root).is_empty());
    assert_eq!((guard.asked.get(), guard.passed.get()), (1, 1));
    assert_eq!(run.apply_state(), ApplyState::RolledBack);
    let records = ws.records(&run);
    assert_eq!(
        last_of(&records, "apply.rolled_back").unwrap()["failed"],
        "publish"
    );
}

#[test]
fn m2d_a_d_o10_a_failed_sync_after_publication_keeps_the_file_and_its_undo() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let result = with_hooks(
        |point, _| {
            if point == HookPoint::AfterFinalCheck {
                seam(&SYNC_FAILS, true);
            }
            false
        },
        || {
            run.apply_observed(
                approval,
                ws.write_grant(binding),
                &ws.staging(),
                &Guard::default(),
            )
        },
    );
    seam(&SYNC_FAILS, false);
    assert_eq!(
        result,
        Err(ApplyError::RecoveryRequired {
            failed: "dir_sync".to_string(),
            unrestored: Vec::new(),
        })
    );
    assert_eq!(
        std::fs::read_to_string(ws.root.join(NAME)).unwrap(),
        CONTENT
    );
    assert_eq!(run.apply_state(), ApplyState::RecoveryRequired);
    assert_eq!(
        analyze_apply(ws.ledger.as_ref(), run.id().ledger_key()).unwrap(),
        vec![ApplyFinding::RecoveryRequired { failed: "dir_sync" }]
    );
    undo(&ws, &mut run, binding).unwrap();
    assert!(entries(&ws.root).is_empty());
}

/// Fails the first append of one event kind.
struct FailOn {
    inner: Arc<CodingRunLedger>,
    kind: &'static str,
    done: std::sync::atomic::AtomicBool,
}

impl FailOn {
    #[allow(clippy::new_ret_no_self)]
    fn new(ws: &Ws, kind: &'static str) -> Arc<dyn LedgerStore> {
        Arc::new(Self {
            inner: Arc::clone(&ws.ledger),
            kind,
            done: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

impl LedgerStore for FailOn {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
        if event.event_kind == self.kind && !self.done.swap(true, Ordering::SeqCst) {
            return Err(LedgerFailure::Unavailable);
        }
        LedgerStore::append(self.inner.as_ref(), event)
    }
    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
        self.inner.verified_records(run)
    }
}

/// Control (green on the base, whose apply already writes nothing when its
/// prepare record fails): the new protocol keeps that.
#[test]
fn m2d_a_d_o10_control_a_ledger_failure_at_prepare_writes_nothing() {
    let ws = new_ws();
    let (mut run, binding) = verified_file_with(&ws, FailOn::new(&ws, "apply.prepared"));
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let reached = Rc::new(Cell::new(false));
    let saw = Rc::clone(&reached);
    let result = with_hooks(
        move |_, _| {
            saw.set(true);
            false
        },
        || {
            run.apply_observed(
                approval,
                ws.write_grant(binding),
                &ws.staging(),
                &Guard::default(),
            )
        },
    );
    assert_eq!(result, Err(ApplyError::Refused(Refusal::Unrecorded)));
    assert!(!reached.get(), "a write was attempted");
    assert!(entries(&ws.root).is_empty());
}

#[test]
fn m2d_a_d_o10_a_ledger_failure_after_publication_unpublishes_only_when_established() {
    for (kind, removal_fails) in [
        ("apply.op", false),
        ("apply.completed", false),
        ("apply.op", true),
    ] {
        let ws = new_ws();
        let (mut run, binding) = verified_file_with(&ws, FailOn::new(&ws, kind));
        let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
        seam(&REMOVE_TEMP_FAILS, removal_fails);
        let result = run.apply_observed(
            approval,
            ws.write_grant(binding),
            &ws.staging(),
            &Guard::default(),
        );
        seam(&REMOVE_TEMP_FAILS, false);
        assert!(
            !ws.root.join(NAME).exists(),
            "{kind}: the publication stayed"
        );
        let temp_id = ws.temp_id(&run);
        if removal_fails {
            assert_eq!(
                result,
                Err(ApplyError::CleanupOwed {
                    cause: CleanupCause::Unrecorded,
                    temps: vec![temp_id.clone()],
                })
            );
            assert_eq!(entries(&ws.root), vec![format!("{TEMP_PREFIX}{temp_id}")]);
            assert_eq!(run.apply_state(), ApplyState::RecoveryRequired);
        } else {
            assert!(
                matches!(result, Err(ApplyError::RolledBack { .. })),
                "{kind}: {result:?}"
            );
            assert!(entries(&ws.root).is_empty(), "{kind}");
            assert_eq!(run.apply_state(), ApplyState::RolledBack);
        }
    }
}

/// Control (green on the base): a clean publication needs no removal, so
/// a removal that would fail changes nothing.
#[test]
fn m2d_a_d_o10_control_the_publication_consumes_the_temp_without_a_removal() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    seam(&REMOVE_TEMP_FAILS, true);
    let result = run.apply_observed(
        approval,
        ws.write_grant(binding),
        &ws.staging(),
        &Guard::default(),
    );
    seam(&REMOVE_TEMP_FAILS, false);
    result.unwrap();
    assert_eq!(entries(&ws.root), vec![NAME.to_string()]);
    assert_eq!(run.apply_state(), ApplyState::Applied);
}

// ── D-O11 / M6′: crash points, the startup report, no sweep ─────────────────

/// Freezes the calling thread for good at a crash point: nothing after it
/// runs, no destructor runs (process death as seen from disk and ledger).
fn crash(signal: &mpsc::Sender<()>) -> ! {
    let _ = signal.send(());
    loop {
        std::thread::park();
    }
}

/// A ledger whose append of `kind` never returns (`before`: never written;
/// otherwise written, then the process dies).
struct CrashStore {
    inner: Arc<CodingRunLedger>,
    kind: &'static str,
    before: bool,
    signal: Mutex<mpsc::Sender<()>>,
}

impl LedgerStore for CrashStore {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
        let hit = event.event_kind == self.kind;
        if hit && self.before {
            crash(&self.signal.lock().unwrap().clone());
        }
        let out = LedgerStore::append(self.inner.as_ref(), event);
        if hit {
            crash(&self.signal.lock().unwrap().clone());
        }
        out
    }
    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
        self.inner.verified_records(run)
    }
}

fn crash_store(
    ws: &Ws,
    kind: &'static str,
    before: bool,
) -> (Arc<dyn LedgerStore>, mpsc::Receiver<()>) {
    let (signal, crashed) = mpsc::channel();
    let store = Arc::new(CrashStore {
        inner: Arc::clone(&ws.ledger),
        kind,
        before,
        signal: Mutex::new(signal),
    });
    (store, crashed)
}

/// The state a restart would find: the run's findings from a freshly opened
/// ledger, and both directories' contents before and after the report.
fn report_after_restart(ws: &Ws, run: Uuid) -> Vec<ApplyFinding> {
    let runs_dir = ws.state.join("coding-runs");
    let before = (digest(&ws.root), digest(&runs_dir));
    let fresh = CodingRunLedger::open(&ws.ledger_path).unwrap();
    assert!(fresh.run_ids().unwrap().contains(&run));
    let _reopened = StagingParent::within(&ws.state).unwrap();
    let findings = analyze_apply(&fresh, run).unwrap();
    assert_eq!(
        (digest(&ws.root), digest(&runs_dir)),
        before,
        "the startup report swept something"
    );
    findings
}

fn prepared_file(temp_id: &str) -> PreparedFile {
    PreparedFile {
        path_sha256: path_sha256(NAME),
        temp_id: Some(temp_id.to_string()),
    }
}

/// Runs `body` on a thread that may crash; returns once it crashed.
fn until_crash(signal: mpsc::Receiver<()>, body: impl FnOnce() + Send + 'static) {
    std::thread::spawn(body);
    signal
        .recv_timeout(WAIT)
        .expect("the crash point was reached");
}

#[test]
fn m2d_a_d_o11_crash_after_prepare_reports_the_planned_temp_id() {
    let ws = new_ws();
    let (store, crashed) = crash_store(&ws, "apply.prepared", false);
    let (mut run, binding) = verified_file_with(&ws, store);
    let id = run.id().ledger_key();
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let grant = ws.write_grant(binding);
    let staging = ws.staging();
    until_crash(crashed, move || {
        let _ = run.apply_observed(approval, grant, &staging, &Guard::default());
    });
    let records = ws.ledger.verify_run(id).unwrap();
    let prepared: Value = serde_json::from_str(&records.last().unwrap().payload).unwrap();
    let temp_id = prepared["files"][0]["temp_id"]
        .as_str()
        .expect("temp id recorded")
        .to_string();
    // Recorded before creation: the temp does not exist yet.
    assert!(entries(&ws.root).is_empty());
    assert_eq!(
        report_after_restart(&ws, id),
        vec![ApplyFinding::PreparedWithoutOutcome {
            files: vec![prepared_file(&temp_id)]
        }]
    );
}

#[test]
fn m2d_a_d_o11_crash_at_the_final_check_leaves_no_pathname_and_is_uncertain() {
    let ws = new_ws();
    let (signal, crashed) = mpsc::channel();
    let (mut run, binding) = verified_file(&ws);
    let id = run.id().ledger_key();
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let grant = ws.write_grant(binding);
    let staging = ws.staging();
    until_crash(crashed, move || {
        with_hooks(
            move |point, _| {
                if point == HookPoint::BeforeFinalCheck {
                    crash(&signal);
                }
                false
            },
            || {
                let _ = run.apply_observed(approval, grant, &staging, &Guard::default());
            },
        );
    });
    let temp_id = {
        let records = ws.ledger.verify_run(id).unwrap();
        let prepared = records
            .iter()
            .rev()
            .find(|r| r.event_kind == "apply.prepared")
            .unwrap();
        let payload: Value = serde_json::from_str(&prepared.payload).unwrap();
        payload["files"][0]["temp_id"].as_str().unwrap().to_string()
    };
    assert!(entries(&ws.root).is_empty(), "a pathname exists");
    assert_eq!(
        report_after_restart(&ws, id),
        vec![ApplyFinding::PreparedWithoutOutcome {
            files: vec![prepared_file(&temp_id)]
        }]
    );
    assert!(entries(&ws.root).is_empty(), "a pathname exists");
}

#[test]
fn m2d_a_d_o11_crash_after_publication_before_its_record_is_uncertain() {
    let ws = new_ws();
    let (store, crashed) = crash_store(&ws, "apply.op", true);
    let (mut run, binding) = verified_file_with(&ws, store);
    let id = run.id().ledger_key();
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let grant = ws.write_grant(binding);
    let staging = ws.staging();
    until_crash(crashed, move || {
        let _ = run.apply_observed(approval, grant, &staging, &Guard::default());
    });
    assert_eq!(entries(&ws.root), vec![NAME.to_string()]);
    let findings = report_after_restart(&ws, id);
    assert!(
        matches!(findings.as_slice(), [ApplyFinding::PreparedWithoutOutcome { files }] if files.len() == 1 && files[0].temp_id.is_some()),
        "{findings:?}"
    );
}

#[test]
fn m2d_a_d_o11_crash_with_an_undo_prepared_is_reported() {
    let ws = new_ws();
    let (store, crashed) = crash_store(&ws, "restore.prepared", false);
    let (mut run, binding) = verified_file_with(&ws, store);
    let id = run.id().ledger_key();
    ws.observed(&mut run, binding, &Guard::default()).unwrap();
    let approval = run.request_restore(&ws.info.name, &Answer::yes()).unwrap();
    let grant = ws.write_grant(binding);
    until_crash(crashed, move || {
        let _ = run.restore(approval, grant);
    });
    assert_eq!(
        entries(&ws.root),
        vec![NAME.to_string()],
        "touched before U1"
    );
    let findings = report_after_restart(&ws, id);
    assert!(
        matches!(findings.as_slice(), [ApplyFinding::RestorePreparedWithoutOutcome { files }] if files.len() == 1 && files[0].path_sha256 == path_sha256(NAME) && files[0].temp_id.is_some()),
        "{findings:?}"
    );
}

#[test]
fn m2d_a_d_o11_staging_awaiting_approval_is_reported_and_kept() {
    let ws = new_ws();
    let (mut run, _binding) = verified_file(&ws);
    run.review().unwrap();
    let id = run.id().ledger_key();
    let staging = ws.state.join("coding-runs").join(run.id().to_string());
    assert!(staging.is_dir());
    std::mem::forget(run); // the process dies with the candidate awaiting approval
    assert_eq!(
        report_after_restart(&ws, id),
        vec![ApplyFinding::StagingAwaitingApproval]
    );
    assert!(staging.is_dir(), "the staging was swept");
}

/// Control: a completed and a cleanly interrupted publication leave
/// nothing to report.
#[test]
fn m2d_a_d_o11_control_settled_runs_report_nothing() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    ws.observed(&mut run, binding, &Guard::default()).unwrap();
    assert!(report_after_restart(&ws, run.id().ledger_key()).is_empty());
}

// ── H2′: undo watches taken when the undo starts, checked at U1 ─────────────

fn published(ws: &Ws) -> (CodingRun, WorkspaceBinding) {
    let (mut run, binding) = verified_file(ws);
    ws.observed(&mut run, binding, &Guard::default()).unwrap();
    (run, binding)
}

fn untouched(ws: &Ws, ino: u64) {
    assert_eq!(entries(&ws.root), vec![NAME.to_string()]);
    let path = ws.root.join(NAME);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CONTENT);
    assert_eq!(std::fs::metadata(&path).unwrap().ino(), ino);
}

#[test]
fn m2d_a_h2_stop_and_resume_during_the_undo_dialogue_needs_a_new_undo() {
    for broker in [false, true] {
        let ws = new_ws();
        let (mut run, binding) = published(&ws);
        let ino = std::fs::metadata(ws.root.join(NAME)).unwrap().ino();
        let authority = if broker {
            ws.broker.clone()
        } else {
            ws.stop.clone()
        };
        let ctl_authority = authority.clone();
        // Stop during the dialog; the owner's resume right after it.
        let answer = Answer::yes_after(move || {
            ctl_authority.engage();
        });
        let approval = run.request_restore(&ws.info.name, &answer).unwrap();
        let ctl = if broker { &ws.broker_ctl } else { &ws.stop_ctl };
        assert!(ctl.lift_if(authority.stop_number()));
        let result = run.restore(approval, ws.write_grant(binding));
        assert_eq!(
            result,
            Err(ApplyError::Refused(Refusal::Interrupted(
                InterruptCause::EmergencyStop
            ))),
            "broker={broker}"
        );
        untouched(&ws, ino);
        assert_eq!(run.apply_state(), ApplyState::Applied);
        undo(&ws, &mut run, binding).unwrap();
        assert!(entries(&ws.root).is_empty());
    }
}

fn undo_with_u1(
    ws: &Ws,
    run: &mut CodingRun,
    grant: ProjectGrant,
    at_u1: impl FnMut() + 'static,
) -> Result<ApplyReport, ApplyError> {
    let approval = run.request_restore(&ws.info.name, &Answer::yes())?;
    let mut at_u1 = at_u1;
    with_hooks(
        move |point, _| {
            if point == HookPoint::BeforeUnpublish {
                at_u1();
            }
            false
        },
        || run.restore(approval, grant),
    )
}

#[test]
fn m2d_a_h2_stop_and_resume_during_the_undo_preflight_needs_a_new_undo() {
    let ws = new_ws();
    let (mut run, binding) = published(&ws);
    let ino = std::fs::metadata(ws.root.join(NAME)).unwrap().ino();
    let stop = ws.stop.clone();
    // Engaged here; lifted below, before the next undo.
    let result = undo_with_u1(&ws, &mut run, ws.write_grant(binding), move || {
        stop.engage();
    });
    assert!(ws.stop_ctl.lift_if(ws.stop.stop_number()));
    assert_eq!(
        result,
        Err(ApplyError::Refused(Refusal::Interrupted(
            InterruptCause::EmergencyStop
        )))
    );
    untouched(&ws, ino);
    let records = ws.records(&run);
    let rejected = last_of(&records, "restore.rejected").expect("restore.rejected");
    assert_eq!(rejected["reason"], "Interrupted");
    assert_eq!(rejected["interrupted"], "EmergencyStop");
    undo(&ws, &mut run, binding).unwrap();
    assert!(entries(&ws.root).is_empty());
}

#[test]
fn m2d_a_h2_a_second_stop_after_restore_admission_needs_a_new_undo() {
    let ws = new_ws();
    let (mut run, binding) = published(&ws);
    let ino = std::fs::metadata(ws.root.join(NAME)).unwrap().ino();
    // Stop 1 and the owner's resume, with no undo in progress: the run's own
    // (halted) admission watch does not keep a new undo from starting.
    ws.broker.engage();
    assert!(ws.broker_ctl.lift_if(ws.broker.stop_number()));
    let broker = ws.broker.clone();
    let result = undo_with_u1(&ws, &mut run, ws.write_grant(binding), move || {
        broker.engage(); // Stop 2, after this undo was admitted
    });
    assert!(ws.broker_ctl.lift_if(ws.broker.stop_number()));
    assert_eq!(
        result,
        Err(ApplyError::Refused(Refusal::Interrupted(
            InterruptCause::EmergencyStop
        )))
    );
    untouched(&ws, ino);
    undo(&ws, &mut run, binding).unwrap();
    assert!(entries(&ws.root).is_empty());
}

/// Added after registration (H2′, "Stop → resume during the preflight"):
/// a stop engaged and lifted after the undo was admitted and before its
/// U1 still ends that undo: U1 checks the watches the undo took at its
/// start and never takes fresh ones. A new undo then works.
#[test]
fn m2d_a_h2_stop_and_resume_both_before_u1_still_end_the_undo() {
    for broker in [false, true] {
        let ws = new_ws();
        let other = StopAuthority::new();
        let other_ctl = other.claim_controller().unwrap();
        // `other` is the run's own (kernel) or observed (broker) authority,
        // so its controller can act inside the hook.
        let (own, observed) = if broker {
            (ws.stop.clone(), other.clone())
        } else {
            (other.clone(), ws.broker.clone())
        };
        let binding = WorkspaceBinding {
            agent_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
        };
        let grant = ws.projects.grant_for_run(ws.info.id, binding).unwrap();
        let mut run = with_test_stop_authority(own, || {
            CodingRun::create_for_project_observing(
                ws.store(),
                grant,
                files_scopes(&[NAME]),
                &observed,
            )
        })
        .unwrap();
        run.grant(&ws.staging()).unwrap();
        run.snapshot().unwrap();
        run.edit(create(NAME, CONTENT)).unwrap();
        assert!(run.verify_structural().unwrap().passed());
        ws.observed(&mut run, binding, &Guard::default()).unwrap();
        let ino = std::fs::metadata(ws.root.join(NAME)).unwrap().ino();
        let result = undo_with_u1(&ws, &mut run, ws.write_grant(binding), move || {
            other.engage();
            assert!(other_ctl.lift_if(other.stop_number()));
        });
        assert_eq!(
            result,
            Err(ApplyError::Refused(Refusal::Interrupted(
                InterruptCause::EmergencyStop
            ))),
            "broker={broker}"
        );
        untouched(&ws, ino);
        assert_eq!(run.apply_state(), ApplyState::Applied);
        undo(&ws, &mut run, binding).unwrap();
        assert!(entries(&ws.root).is_empty());
    }
}

#[test]
fn m2d_a_h2_the_restore_grant_revoked_or_expired_at_u1_touches_nothing() {
    for expire in [false, true] {
        let ws = new_ws();
        let (mut run, binding) = published(&ws);
        let ino = std::fs::metadata(ws.root.join(NAME)).unwrap().ino();
        let grant = if expire {
            ws.projects
                .grant_for_apply_lasting(ws.info.id, binding, Duration::from_millis(1500))
                .unwrap()
        } else {
            ws.write_grant(binding)
        };
        let guard = Rc::new(Guard::default());
        let injector = Injector::new(&ws, &grant, &guard);
        let cause = if expire {
            Cause::WriteExpired
        } else {
            Cause::WriteRevoked
        };
        let result = undo_with_u1(&ws, &mut run, grant, move || injector.land(cause));
        assert_eq!(
            result,
            Err(ApplyError::Refused(Refusal::Interrupted(cause.expected()))),
            "{cause:?}"
        );
        untouched(&ws, ino);
        assert_eq!(run.apply_state(), ApplyState::Applied);
        undo(&ws, &mut run, binding).unwrap();
        assert!(entries(&ws.root).is_empty());
    }
}

// ── D-O19 (kernel): the undo ────────────────────────────────────────────────

#[test]
fn m2d_a_d_o19_both_dialogs_show_the_full_facts() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let review = run.review().unwrap();
    let sha = hex::encode(ManifestEntry::of(CONTENT.as_bytes()).sha256);
    let asked = Answer::yes();
    let approval = run.request_approval(&ws.info.name, &asked).unwrap();
    let shown = asked.shown.borrow()[0].clone();
    assert_eq!(shown.kind, ConfirmationKind::Apply);
    assert_eq!(shown.review_hash, review.binding.hash());
    assert_eq!(shown.bytes, Some(CONTENT.len() as u64));
    assert_eq!(shown.sha256_full.as_deref(), Some(sha.as_str()));
    assert_eq!(shown.identity, None);
    run.apply_observed(
        approval,
        ws.write_grant(binding),
        &ws.staging(),
        &Guard::default(),
    )
    .unwrap();
    let meta = std::fs::metadata(ws.root.join(NAME)).unwrap();
    let asked = Answer::no();
    assert_eq!(
        run.request_restore(&ws.info.name, &asked).unwrap_err(),
        ApplyError::Refused(Refusal::Declined)
    );
    let shown = asked.shown.borrow()[0].clone();
    assert_eq!(shown.kind, ConfirmationKind::Restore);
    assert_eq!(shown.review_hash, review.binding.hash());
    assert_eq!(shown.bytes, Some(CONTENT.len() as u64));
    assert_eq!(shown.sha256_full.as_deref(), Some(sha.as_str()));
    assert_eq!(
        shown.identity,
        Some(FileIdentity {
            dev: meta.dev(),
            ino: meta.ino()
        })
    );
    // Denied: nothing changed, the undo is still offered.
    untouched(&ws, meta.ino());
    assert_eq!(run.apply_state(), ApplyState::Applied);
}

#[test]
fn m2d_a_d_o19_a_file_replaced_after_u1_is_renamed_back_and_stale() {
    let ws = new_ws();
    let (mut run, binding) = published(&ws);
    let path = ws.root.join(NAME);
    let side = ws.root.join("owner-save.tmp");
    let (moved_path, moved_side) = (path.clone(), side.clone());
    let result = undo_with_u1(&ws, &mut run, ws.write_grant(binding), move || {
        // An editor's save-by-rename: same bytes, another file.
        std::fs::write(&moved_side, CONTENT).unwrap();
        std::fs::rename(&moved_side, &moved_path).unwrap();
    });
    assert_eq!(
        result,
        Err(ApplyError::Refused(Refusal::Stale(NAME.to_string())))
    );
    assert_eq!(entries(&ws.root), vec![NAME.to_string()], "no temp left");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CONTENT);
    assert_eq!(run.apply_state(), ApplyState::Applied);
}

#[test]
fn m2d_a_d_o19_an_unremovable_undo_is_owed_until_a_retry() {
    let ws = new_ws();
    let (mut run, binding) = published(&ws);
    seam(&REMOVE_TEMP_FAILS, true);
    let result = undo(&ws, &mut run, binding);
    seam(&REMOVE_TEMP_FAILS, false);
    let records = ws.records(&run);
    let prepared = last_of(&records, "restore.prepared").expect("restore.prepared");
    let temp_id = prepared["planned"][0]["temp_id"]
        .as_str()
        .expect("planned temp")
        .to_string();
    assert_eq!(
        result,
        Err(ApplyError::CleanupOwed {
            cause: CleanupCause::Undo,
            temps: vec![temp_id.clone()],
        })
    );
    assert_eq!(entries(&ws.root), vec![format!("{TEMP_PREFIX}{temp_id}")]);
    assert_eq!(run.apply_state(), ApplyState::RecoveryRequired);
    assert!(matches!(
        analyze_apply(ws.ledger.as_ref(), run.id().ledger_key()).unwrap().as_slice(),
        [ApplyFinding::CleanupOwed { temps }] if temps.len() == 1 && temps[0].temp_id == temp_id
    ));
    run.retry_cleanup().unwrap();
    assert!(entries(&ws.root).is_empty());
    assert_eq!(run.apply_state(), ApplyState::Restored);
    assert!(ws.kinds(&run).iter().any(|k| k == "restore.completed"));
    assert!(analyze_apply(ws.ledger.as_ref(), run.id().ledger_key())
        .unwrap()
        .is_empty());
}

/// Controls (green on the base): an owner edit makes the undo stale with
/// nothing touched; a second undo is refused.
#[test]
fn m2d_a_d_o19_control_owner_edit_is_stale_and_a_second_undo_is_refused() {
    let ws = new_ws();
    let (mut run, binding) = published(&ws);
    std::fs::write(ws.root.join(NAME), "the owner edited it\n").unwrap();
    assert_eq!(
        undo(&ws, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Stale(NAME.to_string()))
    );
    assert_eq!(
        std::fs::read_to_string(ws.root.join(NAME)).unwrap(),
        "the owner edited it\n"
    );
    assert_eq!(run.apply_state(), ApplyState::Applied);
    let ws = new_ws();
    let (mut run, binding) = published(&ws);
    undo(&ws, &mut run, binding).unwrap();
    assert_eq!(
        undo(&ws, &mut run, binding).unwrap_err(),
        ApplyError::InvalidState
    );
}

// ── The review and the observing run ────────────────────────────────────────

#[test]
fn m2d_a_review_single_complete_names_only_one_new_file_shown_whole() {
    let ws = new_ws();
    let (mut run, _) = verified_file(&ws);
    let review = run.review().unwrap();
    let only = review.single_complete().expect("one new file, shown whole");
    assert_eq!(only.path, rel(NAME));
    assert_eq!(only.kind, ChangeKind::Create);
    assert_eq!(only.new, Some(ManifestEntry::of(CONTENT.as_bytes())));

    let ws = new_ws();
    let (mut run, _) = verified_with(
        &ws,
        ws.store(),
        vec![create("a.txt", "a\n"), create("b.txt", "b\n")],
        files_scopes(&["a.txt", "b.txt"]),
    );
    assert!(
        run.review().unwrap().single_complete().is_none(),
        "two files"
    );

    let ws = new_ws();
    std::fs::write(ws.root.join(NAME), "old\n").unwrap();
    let (mut run, _) = verified_with(
        &ws,
        ws.store(),
        vec![replace(NAME, "new\n")],
        files_scopes(&[NAME]),
    );
    assert!(
        run.review().unwrap().single_complete().is_none(),
        "a replacement"
    );

    let ws = new_ws();
    let long = "x".repeat(59) + "\n";
    let content = long.repeat(1300); // above the per-file diff bound
    let (mut run, _) = verified_with(
        &ws,
        ws.store(),
        vec![create(NAME, &content)],
        files_scopes(&[NAME]),
    );
    assert!(
        run.review().unwrap().single_complete().is_none(),
        "a truncated diff"
    );
}

#[test]
fn m2d_a_a_run_observing_the_broker_stop_is_refused_and_halted_by_it() {
    // Engaged before creation: refused, and the run's read grant closed.
    let ws = new_ws();
    ws.broker.engage();
    let binding = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    };
    let grant = ws.projects.grant_for_run(ws.info.id, binding).unwrap();
    let grant_id = grant.grant_id();
    let refused = with_test_stop_authority(ws.stop.clone(), || {
        CodingRun::create_for_project_observing(
            ws.store(),
            grant,
            files_scopes(&[NAME]),
            &ws.broker,
        )
    });
    assert_eq!(
        refused.unwrap_err(),
        RunError::Revoked(RevocationReason::EmergencyStop)
    );
    assert!(ws.registry.resolve(grant_id, binding).is_err());

    // Engaged while the run works: the run ends; after the resume it stays
    // ended and a new run works.
    assert!(ws.broker_ctl.lift_if(ws.broker.stop_number()));
    let binding = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    };
    let grant = ws.projects.grant_for_run(ws.info.id, binding).unwrap();
    let mut run = with_test_stop_authority(ws.stop.clone(), || {
        CodingRun::create_for_project_observing(
            ws.store(),
            grant,
            files_scopes(&[NAME]),
            &ws.broker,
        )
    })
    .unwrap();
    run.grant(&ws.staging()).unwrap();
    run.snapshot().unwrap();
    ws.broker.engage();
    assert!(ws.broker_ctl.lift_if(ws.broker.stop_number()));
    assert_eq!(
        run.edit(create(NAME, CONTENT)).unwrap_err(),
        RunError::Revoked(RevocationReason::EmergencyStop)
    );
    assert_eq!(
        run.state(),
        RunState::Revoked(RevocationReason::EmergencyStop)
    );
    let (mut fresh, binding) = verified_file(&ws);
    let approval = fresh
        .request_approval(&ws.info.name, &Answer::yes())
        .unwrap();
    fresh
        .apply_observed(
            approval,
            ws.write_grant(binding),
            &ws.staging(),
            &Guard::default(),
        )
        .unwrap();
}

// ── D-O21 (kernel): no name or content in the ledger or the report ─────────

fn ledger_targets(ws: &Ws) -> Vec<(String, Vec<u8>)> {
    let mut targets = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        let path = PathBuf::from(format!("{}{suffix}", ws.ledger_path.display()));
        if let Ok(bytes) = std::fs::read(&path) {
            targets.push((format!("ledger.db{suffix}"), bytes));
        }
    }
    for id in ws.ledger.run_ids().unwrap() {
        for r in ws.ledger.verify_run(id).unwrap() {
            targets.push((format!("record {}", r.event_kind), r.payload.into_bytes()));
        }
    }
    targets
}

#[test]
fn m2d_a_d_o21_name_and_content_canaries_stay_out_of_the_ledger_and_the_report() {
    let canary = Canary::new();
    let name = canary.c.clone();
    let content = format!("{}\n{}\n", canary.c, canary.cl);
    let (mut findings, mut targets) = (Vec::new(), Vec::new());
    // 0: a publication whose record fails, undone, with a temp that cannot
    // be removed; 1: a publication and a stale undo; 2: a publication and an
    // unremovable undo. Each owed cleanup is then retried.
    for scenario in 0..3 {
        let ws = new_ws();
        let edits = vec![create(&name, &content)];
        let store = if scenario == 0 {
            FailOn::new(&ws, "apply.op")
        } else {
            ws.store()
        };
        let (mut run, binding) = verified_with(&ws, store, edits, files_scopes(&[name.as_str()]));
        let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
        seam(&REMOVE_TEMP_FAILS, scenario == 0);
        let _ = run.apply_observed(
            approval,
            ws.write_grant(binding),
            &ws.staging(),
            &Guard::default(),
        );
        if scenario == 1 {
            std::fs::write(ws.root.join(&name), "edited\n").unwrap();
        }
        if scenario > 0 {
            seam(&REMOVE_TEMP_FAILS, scenario == 2);
            let _ = undo(&ws, &mut run, binding);
        }
        findings.extend(analyze_apply(ws.ledger.as_ref(), run.id().ledger_key()).unwrap());
        seam(&REMOVE_TEMP_FAILS, false);
        let _ = run.retry_cleanup();
        targets.extend(ledger_targets(&ws));
    }
    assert_eq!(
        findings.len(),
        2,
        "both owed cleanups are reported: {findings:?}"
    );
    targets.push(("findings".to_string(), format!("{findings:?}").into_bytes()));
    let found = leaks(&canary, &targets);
    assert!(
        found.is_empty(),
        "{} leak(s); the first: {}",
        found.len(),
        found[0]
    );
}

// ── Added after Codex review 1 of PR #58 (HIGH, MEDIUM 1, MEDIUM 2) ────────

/// This process's own descriptors that hold an anonymous file (`O_TMPFILE`,
/// no name) of the directory `root`, identified exactly: the file has no
/// link, and the descriptor's target reads `<root>/#<its inode> (deleted)`.
fn anonymous_files(root: &Path) -> Vec<i32> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc/self/fd").unwrap().flatten() {
        let Ok(fd) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let link = PathBuf::from(format!("/proc/self/fd/{fd}"));
        let (Ok(target), Ok(meta)) = (std::fs::read_link(&link), std::fs::metadata(&link)) else {
            continue;
        };
        let expected = format!("{}/#{} (deleted)", root.display(), meta.ino());
        if meta.file_type().is_file()
            && meta.nlink() == 0
            && target.as_os_str() == expected.as_str()
        {
            found.push(fd);
        }
    }
    found
}

/// How the candidate is replaced or altered before the final check.
#[derive(Debug, Clone, Copy)]
enum Substitute {
    /// Another file, with other bytes, under the candidate's name.
    OtherFile,
    /// A symlink, to a file outside the workspace, under that name.
    Symlink,
    /// Other bytes written into the candidate itself (named or anonymous).
    Tamper,
}

fn overwrite(path: &Path) {
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .unwrap();
    file.write_all(b"unapproved\n").unwrap();
    file.sync_all().unwrap();
}

fn substitute(root: &Path, outside: &Path, how: Substitute) {
    for name in entries(root) {
        let path = root.join(&name);
        match how {
            Substitute::OtherFile => {
                std::fs::remove_file(&path).unwrap();
                std::fs::write(&path, "unapproved\n").unwrap();
            }
            Substitute::Symlink => {
                std::fs::remove_file(&path).unwrap();
                std::os::unix::fs::symlink(outside, &path).unwrap();
            }
            Substitute::Tamper => overwrite(&path),
        }
    }
    if let Substitute::Tamper = how {
        for fd in anonymous_files(root) {
            overwrite(&PathBuf::from(format!("/proc/self/fd/{fd}")));
        }
    }
}

/// Added after registration (Codex review 1 of PR #58, HIGH): whatever
/// replaces or alters the candidate before the final check (another file,
/// a symlink, other bytes in the candidate itself) is never published as
/// approved: the result is not Applied with it, and nothing unapproved
/// appears under the name.
#[test]
fn m2d_a_a_substituted_candidate_is_never_published_as_approved() {
    for how in [
        Substitute::OtherFile,
        Substitute::Symlink,
        Substitute::Tamper,
    ] {
        let ws = new_ws();
        let (mut run, binding) = verified_file(&ws);
        let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
        let outside = ws.state.join("outside.txt");
        std::fs::write(&outside, "unapproved\n").unwrap();
        let (root, target) = (ws.root.clone(), outside.clone());
        let result = with_hooks(
            move |point, _| {
                if point == HookPoint::BeforeFinalCheck {
                    substitute(&root, &target, how);
                }
                false
            },
            || {
                run.apply_observed(
                    approval,
                    ws.write_grant(binding),
                    &ws.staging(),
                    &Guard::default(),
                )
            },
        );
        let path = ws.root.join(NAME);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                assert!(
                    meta.file_type().is_file(),
                    "{how:?}: a non-file was published"
                );
                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    CONTENT.as_bytes(),
                    "{how:?}: unapproved bytes were published ({result:?})"
                );
            }
            Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
        }
        if result.is_ok() {
            assert_eq!(run.apply_state(), ApplyState::Applied);
        } else {
            assert_ne!(run.apply_state(), ApplyState::Applied, "{how:?}");
        }
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "unapproved\n");
    }
}

/// Added after registration (Architect condition 4 on the Codex HIGH fix):
/// when the workspace cannot hold an anonymous file, Files is refused,
/// fail closed: nothing is published and no named temporary file is used
/// instead.
#[test]
fn m2d_a_a_without_anonymous_files_the_publication_is_refused() {
    use crate::coding_run::apply::TMPFILE_UNSUPPORTED;
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let root = ws.root.clone();
    let named = Rc::new(RefCell::new(Vec::new()));
    let saw = Rc::clone(&named);
    seam(&TMPFILE_UNSUPPORTED, true);
    let result = with_hooks(
        move |_, _| {
            saw.borrow_mut().extend(entries(&root));
            false
        },
        || {
            run.apply_observed(
                approval,
                ws.write_grant(binding),
                &ws.staging(),
                &Guard::default(),
            )
        },
    );
    seam(&TMPFILE_UNSUPPORTED, false);
    assert!(
        matches!(result, Err(ApplyError::RolledBack { .. })),
        "{result:?}"
    );
    assert!(entries(&ws.root).is_empty(), "something was published");
    assert!(named.borrow().is_empty(), "a named temporary file was used");
    assert_eq!(run.apply_state(), ApplyState::RolledBack);
}

/// Added after registration (Codex review 1 of PR #58, MEDIUM 1): an Owner
/// file found under the name is put back, but the undo is `Stale` only once
/// that return is durable (the directory synced); otherwise recovery stays
/// required and the prepared undo, with its planned temp id, stays reported.
#[test]
fn m2d_a_d_o19_a_put_back_owner_file_is_stale_only_once_its_return_is_synced() {
    let ws = new_ws();
    let (mut run, binding) = published(&ws);
    let path = ws.root.join(NAME);
    let side = ws.root.join("owner-save.tmp");
    let (moved_path, moved_side) = (path.clone(), side.clone());
    let result = undo_with_u1(&ws, &mut run, ws.write_grant(binding), move || {
        std::fs::write(&moved_side, CONTENT).unwrap();
        std::fs::rename(&moved_side, &moved_path).unwrap();
        seam(&SYNC_FAILS, true);
    });
    seam(&SYNC_FAILS, false);
    assert!(
        matches!(result, Err(ApplyError::RecoveryRequired { .. })),
        "{result:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CONTENT);
    assert_eq!(run.apply_state(), ApplyState::RecoveryRequired);
    let records = ws.records(&run);
    let prepared = last_of(&records, "restore.prepared").expect("restore.prepared");
    let planned = prepared["planned"][0]["temp_id"]
        .as_str()
        .expect("planned temp")
        .to_string();
    let findings = analyze_apply(ws.ledger.as_ref(), run.id().ledger_key()).unwrap();
    assert!(
        findings.contains(&ApplyFinding::RestorePreparedWithoutOutcome {
            files: vec![prepared_file(&planned)]
        }),
        "the prepared undo was cleared: {findings:?}"
    );
}

/// Added after registration (Codex review 1 of PR #58, MEDIUM 2): an Owner
/// version kept under a temporary name is reported at startup with its
/// temp id and path digest, and never as disposable cleanup.
#[test]
fn m2d_a_d_o11_a_kept_owner_version_is_reported_never_as_cleanup() {
    use crate::coding_run::apply::AFTER_EXCHANGE;
    let ws = new_ws();
    std::fs::write(ws.root.join("existing.txt"), "old\n").unwrap();
    let (mut run, binding) = verified_with(
        &ws,
        ws.store(),
        vec![replace("existing.txt", "new\n")],
        files_scopes(&["existing.txt"]),
    );
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    AFTER_EXCHANGE.with(|h| {
        *h.borrow_mut() = Some(Box::new(|dir: &Path, temp: &str, name: &str| {
            // The Owner saves a version in place, and the name disappears:
            // the exchange cannot be undone, the Owner's version stays aside.
            std::fs::write(dir.join(temp), "the owner's version\n").unwrap();
            std::fs::remove_file(dir.join(name)).unwrap();
        }))
    });
    let result = run.apply(approval, ws.write_grant(binding), &ws.staging());
    AFTER_EXCHANGE.with(|h| *h.borrow_mut() = None);
    assert!(
        matches!(result, Err(ApplyError::RecoveryRequired { .. })),
        "{result:?}"
    );
    let records = ws.records(&run);
    let failed = last_of(&records, "apply.recovery_required").expect("recorded");
    let temp_id = failed["unrestored"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|u| u["owner_version_temp"].as_str())
        .expect("the owner version's temp id is recorded")
        .to_string();
    let kept = ws.root.join(format!("{TEMP_PREFIX}{temp_id}"));
    assert_eq!(
        std::fs::read_to_string(&kept).unwrap(),
        "the owner's version\n"
    );
    let findings = report_after_restart(&ws, run.id().ledger_key());
    let shown = format!("{findings:?}");
    assert!(
        shown.contains(&temp_id) && shown.contains(&path_sha256("existing.txt")),
        "the kept owner version is not reported: {shown}"
    );
    assert!(
        !findings
            .iter()
            .any(|f| matches!(f, ApplyFinding::CleanupOwed { temps }
            if temps.iter().any(|t| t.temp_id == temp_id))),
        "the owner's version is reported as disposable cleanup"
    );
    assert_eq!(
        std::fs::read_to_string(&kept).unwrap(),
        "the owner's version\n"
    );
}

// ── Added after Codex review 2 of PR #58: the process is non-dumpable ──────

const HELPER_MODE: &str = "NEXUS_M2D_A_HELPER";

fn child_line(lines: &mpsc::Receiver<String>, prefix: &str) -> String {
    loop {
        let line = lines
            .recv_timeout(WAIT)
            .unwrap_or_else(|_| panic!("the child never said {prefix}"));
        // The harness may print its own text before ours on a line.
        if let Some((_, rest)) = line.split_once(&format!("{prefix} ")) {
            return rest.trim().to_string();
        }
    }
}

/// The test binary re-run as a held child in `mode`
/// (`m2d_a_helper_child`), owned through `GuardedChild` and ended with
/// `finish`; its stdout lines arrive on the receiver.
fn helper_child(
    mode: &str,
) -> (
    crate::emergency_stop::GuardedChild,
    std::process::ChildStdin,
    mpsc::Receiver<String>,
) {
    use crate::emergency_stop::{GuardedChild, HaltArea};
    use std::io::BufRead as _;
    let authority = StopAuthority::new();
    let watch = authority.admit().unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "coding_run::tests::m2d_a::m2d_a_helper_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(HELPER_MODE, mode)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let (child, pipes) = GuardedChild::spawn(&mut command, &watch, HaltArea::Coding).unwrap();
    let stdout = pipes.stdout.unwrap();
    let (send, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    (child, pipes.stdin.unwrap(), lines)
}

fn go(stdin: &mut std::process::ChildStdin) {
    use std::io::Write as _;
    stdin.write_all(b"GO\n").unwrap();
    stdin.flush().unwrap();
}

fn wait_go() {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

fn say(line: String) {
    use std::io::Write as _;
    println!("{line}");
    std::io::stdout().flush().unwrap();
}

/// One observed publication in `ws`: its refusal code (or `-`), whether
/// anything was published, whether the candidate step was ever reached,
/// and whether an apply was prepared.
fn attempt(ws: &Ws) -> String {
    let (mut run, binding) = verified_file(ws);
    let reached = Rc::new(Cell::new(false));
    let saw = Rc::clone(&reached);
    let result = with_hooks(
        move |_, _| {
            saw.set(true);
            false
        },
        || ws.observed(&mut run, binding, &Guard::default()),
    );
    let code = result
        .as_ref()
        .err()
        .and_then(ApplyError::detail_code)
        .unwrap_or("-");
    let prepared = ws.kinds(&run).iter().any(|k| k == "apply.prepared");
    format!(
        "code={code} published={} reached={} prepared={prepared} entries={}",
        ws.root.join(NAME).exists(),
        reached.get(),
        entries(&ws.root).len()
    )
}

/// The child side of the non-dumpable oracles; a no-op unless run by them.
#[test]
#[ignore]
fn m2d_a_helper_child() {
    let Ok(mode) = std::env::var(HELPER_MODE) else {
        return;
    };
    match mode.as_str() {
        // A fresh, dumpable process attempts one publication.
        "dumpable" => {
            let dumpable = nix::sys::prctl::get_dumpable().unwrap();
            say(format!("DUMPABLE {dumpable}"));
            say(format!("OUTCOME {}", attempt(&fixture_ws())));
        }
        "live" => {
            say(format!("FIRST {}", attempt(&fixture_ws())));
            // Control: while dumpable, another process of the same user can
            // open an anonymous file of ours for writing.
            let ws = fixture_ws();
            let control = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .mode(0o600)
                .custom_flags(nix::libc::O_TMPFILE)
                .open(&ws.root)
                .unwrap();
            say(format!(
                "CONTROL {} {}",
                std::process::id(),
                std::os::fd::AsRawFd::as_raw_fd(&control)
            ));
            wait_go();
            drop(control);
            nix::sys::prctl::set_dumpable(false).unwrap();
            say(format!(
                "NONDUMPABLE {}",
                !nix::sys::prctl::get_dumpable().unwrap()
            ));
            let (mut run, binding) = verified_file(&ws);
            let root = ws.root.clone();
            let result = with_hooks(
                move |point, _| {
                    if point == HookPoint::BeforeFinalCheck {
                        for fd in anonymous_files(&root) {
                            say(format!("CANDIDATE {} {fd}", std::process::id()));
                        }
                        wait_go();
                    }
                    false
                },
                || ws.observed(&mut run, binding, &Guard::default()),
            );
            let content = std::fs::read(ws.root.join(NAME)).map(hex::encode);
            say(format!(
                "DONE ok={} content={}",
                result.is_ok(),
                content.unwrap_or_else(|_| "-".to_string())
            ));
        }
        other => panic!("unknown helper mode {other}"),
    }
}

/// Added after registration (Codex review 2 of PR #58, HIGH): in a process
/// that is still dumpable, so that other processes of the same user could
/// open its descriptors, the publication is refused before any candidate
/// exists: nothing prepared, no candidate step, nothing published.
#[test]
fn m2d_a_a_a_dumpable_process_never_publishes() {
    use crate::emergency_stop::ChildEnd;
    let (child, _stdin, lines) = helper_child("dumpable");
    assert_eq!(
        child_line(&lines, "DUMPABLE"),
        "true",
        "the helper must start dumpable"
    );
    assert_eq!(
        child_line(&lines, "OUTCOME"),
        "code=ProcessDumpable published=false reached=false prepared=false entries=0"
    );
    assert!(matches!(child.finish(), ChildEnd::Exited(status) if status.success()));
}

/// Added after registration (Codex review 2 of PR #58, HIGH), live: a
/// dumpable process is refused; once the process is non-dumpable, another
/// process of the same user can no longer open its anonymous candidate for
/// writing (control: it could while the process was dumpable), and the
/// publication then succeeds with exactly the approved bytes.
#[test]
fn m2d_a_a_a_non_dumpable_process_keeps_its_candidate_to_itself() {
    use crate::emergency_stop::ChildEnd;
    let (child, mut stdin, lines) = helper_child("live");
    let pid = child.id().to_string();
    assert!(
        child_line(&lines, "FIRST").starts_with("code=ProcessDumpable published=false"),
        "a dumpable process published"
    );
    let open_for_writing = |line: String| {
        let (owner, fd) = line.split_once(' ').unwrap();
        assert_eq!(owner, pid);
        std::fs::OpenOptions::new()
            .write(true)
            .open(format!("/proc/{owner}/fd/{fd}"))
    };
    let control = open_for_writing(child_line(&lines, "CONTROL"));
    assert!(control.is_ok(), "control: {control:?}");
    go(&mut stdin);
    assert_eq!(child_line(&lines, "NONDUMPABLE"), "true");
    let refused = open_for_writing(child_line(&lines, "CANDIDATE"));
    let error = refused.expect_err("another process opened the candidate for writing");
    assert!(
        matches!(error.raw_os_error(), Some(code) if code == nix::libc::EACCES || code == nix::libc::EPERM),
        "{error:?}"
    );
    go(&mut stdin);
    assert_eq!(
        child_line(&lines, "DONE"),
        format!("ok=true content={}", hex::encode(CONTENT))
    );
    drop(stdin);
    assert!(matches!(child.finish(), ChildEnd::Exited(status) if status.success()));
}

// ── M2d-K2: the write grant's state when the attempt closed it (§11 M4′) ───
//
// Codex re-check 2 of PR #64, HIGH 3: a kernel write grant revoked or
// expired after the final check lets the publication complete, and the
// kernel's own closure at the attempt's end then hides it. The run now
// keeps what the grant's state was right before the kernel closed it.

/// Lands a write-grant lapse from any thread (the ledger tap below).
fn land_lapse(
    registry: &WorkspaceAuthorityRegistry,
    grant: WorkspaceGrantId,
    binding: WorkspaceBinding,
    cause: Cause,
) {
    match cause {
        Cause::WriteRevoked => registry.revoke(grant, binding).unwrap(),
        Cause::WriteExpired => {
            let expires = registry
                .resolve(grant, binding)
                .ok()
                .and_then(|g| g.expires_at());
            let deadline = Instant::now() + WAIT;
            while let Some(at) = expires {
                if SystemTime::now() >= at {
                    break;
                }
                assert!(Instant::now() < deadline, "the write grant never expired");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        other => panic!("not a write-grant lapse: {other:?}"),
    }
}

/// The workspace ledger, running `on` once, right after the first record of
/// `kind` is appended.
struct Tap {
    inner: Arc<CodingRunLedger>,
    kind: &'static str,
    on: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl LedgerStore for Tap {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
        let record = LedgerStore::append(self.inner.as_ref(), event)?;
        if event.event_kind == self.kind {
            let on = self.on.lock().unwrap().take();
            if let Some(on) = on {
                on();
            }
        }
        Ok(record)
    }
    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
        self.inner.verified_records(run)
    }
}

/// An observed apply, with `cause` (a write-grant lapse) landing at the
/// kernel's `AfterFinalCheck`, once the final check has passed.
fn apply_with(
    ws: &Ws,
    run: &mut CodingRun,
    binding: WorkspaceBinding,
    cause: Option<Cause>,
) -> Result<ApplyReport, ApplyError> {
    let approval = run.request_approval(&ws.info.name, &Answer::yes()).unwrap();
    let grant = match cause {
        Some(cause) => grant_for(ws, binding, cause),
        None => ws.write_grant(binding),
    };
    let guard = Rc::new(Guard::default());
    let injector = Injector::new(ws, &grant, &guard);
    with_hooks(
        move |point, _| {
            if let (HookPoint::AfterFinalCheck, Some(cause)) = (point, cause) {
                injector.land(cause);
            }
            false
        },
        || run.apply_observed(approval, grant, &ws.staging(), &*guard),
    )
}

/// An undo whose write grant lapses (`cause`) after U1, when its completion
/// is recorded, through `tap` (the run's ledger).
fn undo_with(
    ws: &Ws,
    run: &mut CodingRun,
    binding: WorkspaceBinding,
    tap: &Tap,
    cause: Option<Cause>,
) -> Result<ApplyReport, ApplyError> {
    let approval = run.request_restore(&ws.info.name, &Answer::yes()).unwrap();
    let grant = match cause {
        Some(cause) => grant_for(ws, binding, cause),
        None => ws.write_grant(binding),
    };
    if let Some(cause) = cause {
        let (registry, id) = (Arc::clone(&ws.registry), grant.grant_id());
        *tap.on.lock().unwrap() = Some(Box::new(move || land_lapse(&registry, id, binding, cause)));
    }
    run.restore(approval, grant)
}

/// A run over the workspace whose ledger is a tap on `kind`.
fn tapped(ws: &Ws, kind: &'static str) -> (CodingRun, WorkspaceBinding, Arc<Tap>) {
    let tap = Arc::new(Tap {
        inner: Arc::clone(&ws.ledger),
        kind,
        on: Mutex::new(None),
    });
    let (run, binding) = verified_file_with(ws, Arc::clone(&tap) as Arc<dyn LedgerStore>);
    (run, binding, tap)
}

#[test]
fn m2d_k2_a_write_grant_revoked_after_the_final_check_is_reported_at_close() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    assert_eq!(run.write_grant_at_close(), None, "no attempt yet");
    apply_with(&ws, &mut run, binding, Some(Cause::WriteRevoked)).unwrap();
    assert_eq!(entries(&ws.root), vec![NAME.to_string()], "published once");
    assert_eq!(run.apply_state(), ApplyState::Applied);
    assert_eq!(run.write_grant_at_close(), Some(InterruptCause::Revoked));
}

#[test]
fn m2d_k2_a_write_grant_expired_after_the_final_check_is_reported_at_close() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    apply_with(&ws, &mut run, binding, Some(Cause::WriteExpired)).unwrap();
    assert_eq!(entries(&ws.root), vec![NAME.to_string()], "published once");
    assert_eq!(run.write_grant_at_close(), Some(InterruptCause::Expired));
}

#[test]
fn m2d_k2_a_live_write_grant_closed_by_the_kernel_is_none() {
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    apply_with(&ws, &mut run, binding, None).unwrap();
    assert_eq!(entries(&ws.root), vec![NAME.to_string()]);
    assert_eq!(run.write_grant_at_close(), None);
}

#[test]
fn m2d_k2_each_attempt_reports_its_own_write_grant() {
    // The apply's grant lapsed; the undo's did not.
    let ws = new_ws();
    let (mut run, binding, tap) = tapped(&ws, "restore.completed");
    apply_with(&ws, &mut run, binding, Some(Cause::WriteRevoked)).unwrap();
    assert_eq!(run.write_grant_at_close(), Some(InterruptCause::Revoked));
    undo_with(&ws, &mut run, binding, &tap, None).unwrap();
    assert!(entries(&ws.root).is_empty());
    assert_eq!(run.write_grant_at_close(), None, "a stale apply value");
}

#[test]
fn m2d_k2_undo_write_grant_revoked_or_expired_after_u1_is_reported_at_close() {
    for (cause, expected) in [
        (Cause::WriteRevoked, InterruptCause::Revoked),
        (Cause::WriteExpired, InterruptCause::Expired),
    ] {
        let ws = new_ws();
        let (mut run, binding, tap) = tapped(&ws, "restore.completed");
        apply_with(&ws, &mut run, binding, None).unwrap();
        assert_eq!(run.write_grant_at_close(), None, "{cause:?}");
        undo_with(&ws, &mut run, binding, &tap, Some(cause))
            .unwrap_or_else(|e| panic!("{cause:?}: {e:?}"));
        assert!(entries(&ws.root).is_empty(), "{cause:?}: undone");
        assert_eq!(run.apply_state(), ApplyState::Restored, "{cause:?}");
        assert_eq!(run.write_grant_at_close(), Some(expected), "{cause:?}");
    }
}

#[test]
fn m2d_k2_an_attempt_that_never_reached_its_close_reports_nothing_stale() {
    // The apply's grant lapsed; the undo then never reaches its close (a
    // defect: it panics before U1). Its own attempt reads nothing, never the
    // apply's value.
    let ws = new_ws();
    let (mut run, binding) = verified_file(&ws);
    apply_with(&ws, &mut run, binding, Some(Cause::WriteRevoked)).unwrap();
    assert_eq!(run.write_grant_at_close(), Some(InterruptCause::Revoked));
    let approval = run.request_restore(&ws.info.name, &Answer::yes()).unwrap();
    let grant = ws.write_grant(binding);
    let panicked = with_hooks(
        |point, _| {
            if point == HookPoint::BeforeUnpublish {
                panic!("a defect inside the undo attempt");
            }
            false
        },
        || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run.restore(approval, grant)
            }))
            .is_err()
        },
    );
    assert!(panicked);
    assert_eq!(run.write_grant_at_close(), None, "a stale apply value");
}
