//! Session §1.4/R17: real staging calls, with an unscheduled Stop worker.
//! Baseline fixture uses #83's selected File route. The fixture is migrated
//! to the mandatory facade with the implementation; expected events stay put.
use super::*;
use crate::authority::approval::*;
use crate::authority::clock::ManualClock;
use crate::files::FileCreate;
use crate::harness_tests::{temp_root, TempRoot};
use crate::host_session::{CommandKind, HostSession, SessionDoor, SessionSeed};
use nexus_kernel::coding_run::LedgerFile;
use std::sync::atomic::{AtomicUsize, Ordering};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const CONSENT: &str = "msn-000001/1/1";

struct Yes;
impl ControlConfirmer for Yes {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        false
    }
    fn confirm_undo(&self, _: &UndoConfirmation) -> bool {
        true
    }
}

struct Fixture {
    gov: Arc<GovernedControl>,
    primary: Arc<crate::evidence_store::PrimaryEvidence>,
    door: Arc<SessionDoor>,
    _session: Arc<HostSession>,
    request: crate::host_session::SessionRequest,
    sequence: AtomicUsize,
    run: RunId,
    clock: Arc<ManualClock>,
    grant: crate::authority::ids::GrantId,
    attachment: Arc<crate::authority::file_session::FileAttachment>,
    _root: TempRoot,
    _dirs: TempRoot,
}
impl Fixture {
    fn new() -> Self {
        let root = temp_root("session-stage-oracle");
        let dirs = temp_root("session-stage-selected");
        Self::with_roots(root, dirs)
    }
    fn with_roots(root: TempRoot, dirs: TempRoot) -> Self {
        Self::with_roots_and_duration(root, dirs, Duration::from_secs(3600))
    }
    fn with_roots_and_duration(root: TempRoot, dirs: TempRoot, duration: Duration) -> Self {
        assert!(crate::files::make_non_dumpable());
        let clock = Arc::new(ManualClock::default());
        let seed = SessionSeed::new().unwrap();
        let ingress = seed.ingress();
        let evidence = crate::evidence_store::PrimaryEvidence::open(
            &root.0.path().join("p3-evidence/evidence.sqlite3"),
            vec![],
        )
        .unwrap();
        let session = Arc::new(
            HostSession::open(
                seed,
                root.0.path(),
                evidence.clone(),
                clock.clone(),
                Arc::new(Yes),
            )
            .unwrap(),
        );
        let workspace = dirs.0.path().join("selected/workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        session.bootstrap_workspace(&workspace, &[]).unwrap();
        session.finish_bootstrap().unwrap();
        let request = ingress.accept(CommandKind::Run, 1).unwrap();
        let door = session.door_for_test(&request, duration).unwrap();
        let gov = session.control_for_test();
        let grant = gov.grants().into_iter().next().unwrap().id;
        let attachment = door.file_port().unwrap().attach().unwrap();
        door.claim_file_session(&attachment).unwrap();
        door.confirm_mission(&MissionConfirmation {
            contract_hash: HASH.into(),
            request: CONSENT.into(),
            summary: vec!["One plain text file".into()],
            page_facts: None,
        })
        .unwrap();
        let run = door.open_file_run(&attachment).unwrap();
        door.admit_files(run, HASH, CONSENT).unwrap();
        Self {
            primary: evidence,
            gov,
            door: Arc::new(door),
            _session: session,
            request,
            sequence: AtomicUsize::new(1),
            run,
            clock,
            grant,
            attachment: Arc::new(attachment),
            _root: root,
            _dirs: dirs,
        }
    }
    fn propose(&self) -> Result<AgentOutcome, AuthorityError> {
        self.propose_content("session.txt", "session oracle\n")
    }
    fn propose_content(&self, name: &str, content: &str) -> Result<AgentOutcome, AuthorityError> {
        self.door
            .create_file(
                self.run,
                &FileCreate {
                    name: name.into(),
                    content: content.into(),
                },
            )
            .map_err(Into::into)
    }
    fn cleanup(&self) -> Result<crate::files::CleanupRetry, crate::host_session::SessionFailure> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        let request = self
            ._session
            .ingress()
            .accept(CommandKind::Cleanup, seq)
            .map_err(|_| crate::host_session::SessionFailure::from(AuthorityError::Halted))?;
        self._session.cleanup(&request)
    }
    fn undo(&self) -> Result<crate::files::UndoReport, crate::host_session::SessionFailure> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        let request = self
            ._session
            .ingress()
            .accept(CommandKind::Undo, seq)
            .map_err(|_| crate::host_session::SessionFailure::from(AuthorityError::Halted))?;
        self._session.undo(&request)
    }
    fn ledger_path(&self) -> std::path::PathBuf {
        self.gov.files().state_dir_for_test().join("ledger.db")
    }
    fn pending(&self) -> CommitmentId {
        match self.propose().unwrap() {
            AgentOutcome::AwaitingApproval(view) => view.id,
            AgentOutcome::Done(_) => panic!("File must require separate approval"),
        }
    }
    fn approve(&self, id: CommitmentId) -> Result<EffectOutput, AuthorityError> {
        assert_eq!(self._session.pending_for_test(), Some(id));
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        let request = self
            ._session
            .ingress()
            .accept(CommandKind::Approve, sequence)
            .map_err(|_| AuthorityError::Halted)?;
        self._session.approve_file(&request).map_err(Into::into)
    }
}

#[test]
fn session_approval_children_use_the_separately_accepted_request_without_refreshing_run() {
    let f = Fixture::new();
    let id = f.pending();
    f._session.expire_request_for_test(1);
    assert!(
        f.propose_content("stale.txt", "stale\n").is_err(),
        "retained original Run stays expired"
    );
    assert!(
        f.approve(id).is_ok(),
        "new accepted approval owns its child steps; native grants and commitment remain live"
    );
    assert_eq!(
        std::fs::read(f._dirs.0.path().join("selected/workspace/session.txt")).unwrap(),
        b"session oracle\n"
    );
    assert!(
        f.propose_content("stale.txt", "stale\n").is_err(),
        "approval never refreshes a retained Run handle"
    );
}

#[test]
fn session_inert_page_check_and_recheck_are_separate_mutating_calls() {
    const PAGE: &str = include_str!("../../../../kernel/src/inert_page/fixtures/reference.html");
    let mut failures = Vec::new();
    for (point, event, execute) in [
        ("session.before.page_verify", "page.inert_check", false),
        ("session.before.page_recheck", "page.inert_recheck", true),
    ] {
        for ingress_count in 0..=3 {
            let f = Fixture::new();
            let pending = execute.then(|| match f.propose_content("index.html", PAGE).unwrap() {
                AgentOutcome::AwaitingApproval(view) => view.id,
                _ => panic!("page requires separate approval"),
            });
            let reached = Arc::new(AtomicUsize::new(0));
            let seen = reached.clone();
            // Fresh fixture: this page-check or publication recheck has not
            // occurred. Do not open/create the lazy ledger ahead of staging.
            let baseline = 0;
            let cell = f.gov.file_session_halt();
            f.gov.files().hooks().arm(point, move || {
                seen.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingress_count {
                    cell.engage_emergency();
                }
            });
            let success = if let Some(id) = pending {
                f.approve(id).is_ok()
            } else {
                matches!(
                    f.propose_content("index.html", PAGE),
                    Ok(AgentOutcome::AwaitingApproval(_))
                )
            };
            assert_eq!(
                reached.load(Ordering::SeqCst),
                1,
                "available page route control"
            );
            let delta = matching(&events(&f.ledger_path()), event, None) - baseline;
            let correct = if ingress_count == 0 {
                success && delta == 1
            } else {
                !success && delta == 0
            };
            println!("PAGE/{point}/ingresses={ingress_count}: native_workers=0 event_delta={delta} conforms={correct}");
            if !correct {
                failures.push((point, ingress_count));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "eight executed page schedules: {failures:?}"
    );
}

#[test]
fn session_read_constructor_and_write_grant_require_individual_handoffs() {
    let mut failures = Vec::new();
    for (before, after, execute) in [
        (
            "session.before.read_grant",
            "session.after.read_grant",
            false,
        ),
        (
            "session.before.run_construct",
            "session.after.run_construct",
            false,
        ),
        (
            "session.before.write_grant",
            "session.after.write_grant",
            true,
        ),
    ] {
        for ingress_count in 0..=3 {
            let f = Fixture::new();
            let pending = execute.then(|| f.pending());
            let reached = Arc::new(AtomicUsize::new(0));
            let completed = Arc::new(AtomicUsize::new(0));
            let seen = reached.clone();
            let done = completed.clone();
            let cell = f.gov.file_session_halt();
            f.gov.files().hooks().arm(before, move || {
                seen.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingress_count {
                    cell.engage_emergency();
                }
            });
            // A test-only receipt directly after the actual native call,
            // not an admission result supplied by the production classifier.
            f.gov.files().hooks().arm(after, move || {
                done.fetch_add(1, Ordering::SeqCst);
            });
            let success = if let Some(id) = pending {
                f.approve(id).is_ok()
            } else {
                matches!(f.propose(), Ok(AgentOutcome::AwaitingApproval(_)))
            };
            assert_eq!(reached.load(Ordering::SeqCst), 1);
            let calls = completed.load(Ordering::SeqCst);
            let correct = if ingress_count == 0 {
                success && calls == 1
            } else {
                !success && calls == 0
            };
            println!("GRANT/{before}/ingresses={ingress_count}: native_workers=0 actual_call_returns={calls} conforms={correct}");
            if !correct {
                failures.push((before, ingress_count));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "twelve executed grant/construction schedules: {failures:?}"
    );
}

#[test]
fn session_discard_after_normal_cleanup_reservation_rechecks_original_lifetime() {
    use std::os::unix::fs::PermissionsExt;
    let mut failures = Vec::new();
    for ingresses in 0..=3 {
        let f = Fixture::new();
        f.pending();
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let dir = std::fs::read_dir(&staging)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(dir.is_dir());
        let outcome = f.door.cancel_run(f.run).unwrap().write();
        assert_eq!((outcome.owed, outcome.uncertain), (0, 0));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        f.gov.settle();
        assert!(
            dir.exists(),
            "real failed staging removal creates retry debt"
        );
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let before = resources(&staging);
        assert!(!before.is_empty());
        let reached = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let seen = reached.clone();
        let done = completed.clone();
        let cell = f.gov.file_session_halt();
        f.gov
            .files()
            .hooks()
            .arm("session.before.discard_staging", move || {
                seen.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingresses {
                    cell.engage_emergency();
                }
            });
        f.gov
            .files()
            .hooks()
            .arm("session.after.discard_staging", move || {
                done.fetch_add(1, Ordering::SeqCst);
            });
        let result = f.cleanup();
        assert_eq!(reached.load(Ordering::SeqCst), 1);
        let calls = completed.load(Ordering::SeqCst);
        let after = resources(&staging);
        let correct = if ingresses == 0 {
            result.is_ok() && calls == 1 && after.is_empty()
        } else {
            result.is_err() && calls == 0 && before == after
        };
        println!("CLEANUP/discard_staging/ingresses={ingresses}: actual_call_returns={calls} retained={} conforms={correct}", before == after);
        if !correct {
            failures.push(ingresses);
        }
    }
    assert!(
        failures.is_empty(),
        "four executed cleanup schedules: {failures:?}"
    );
}

#[path = "session_death_tests.rs"]
mod death;
#[path = "session_setup_tests.rs"]
mod setup;

fn events(path: &std::path::Path) -> Vec<(String, serde_json::Value)> {
    let ledger = LedgerFile::open(path).unwrap();
    ledger
        .run_ids()
        .unwrap()
        .into_iter()
        .flat_map(|id| {
            ledger
                .store()
                .verified_records(id.ledger_key())
                .unwrap()
                .into_iter()
                .map(|record| {
                    (
                        record.event_kind,
                        serde_json::from_str(&record.payload).unwrap(),
                    )
                })
        })
        .collect()
}

/// Events are literal kernel facts, not production admission return values.
const STEPS: [(&str, &str, &str, Option<&str>); 5] = [
    (
        "staging_grant",
        "session.before.staging_grant",
        "run.granted",
        None,
    ),
    (
        "snapshot",
        "session.before.snapshot",
        "op.prepared",
        Some("snapshot"),
    ),
    ("edit", "session.before.edit", "op.prepared", Some("edit")),
    (
        "structural_verify",
        "session.before.structural_verify",
        "verify.structural",
        None,
    ),
    ("review", "session.before.review", "review.computed", None),
];

fn matching(rows: &[(String, serde_json::Value)], event: &str, kind: Option<&str>) -> usize {
    rows.iter()
        .filter(|(name, facts)| {
            name == event
                && kind.is_none_or(|kind| {
                    facts.get("kind").and_then(serde_json::Value::as_str) == Some(kind)
                })
        })
        .count()
}

#[test]
fn session_r17_executed_staging_schedules_compare_verified_ledger_events() {
    let mut failures = Vec::new();
    let mut executed = 0;
    for (sink, hook, event, kind) in STEPS {
        for ingress_count in 0..=3 {
            let f = Fixture::new();
            let at = Arc::new(AtomicUsize::new(0));
            let before = Arc::new(AtomicUsize::new(usize::MAX));
            let (seen, captured) = (at.clone(), before.clone());
            let path = f.ledger_path();
            let cell = f.gov.file_session_halt();
            f.gov.files().hooks().arm(hook, move || {
                captured.store(matching(&events(&path), event, kind), Ordering::SeqCst);
                seen.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingress_count {
                    cell.engage_emergency();
                }
            });
            let result = f.propose();
            assert_eq!(at.load(Ordering::SeqCst), 1, "fixture did not reach {sink}");
            assert!(!f.gov.stop_snapshot().broker_stopped);
            assert!(!f.gov.stop_snapshot().kernel_stopped);
            let delta =
                matching(&events(&f.ledger_path()), event, kind) - before.load(Ordering::SeqCst);
            let correct = if ingress_count == 0 {
                result.is_ok() && delta > 0
            } else {
                result.is_err() && delta == 0
            };
            println!("R17/{sink}/before/ingresses={ingress_count}: reached=1 native_workers=0 event_delta={delta} conforms={correct}");
            if !correct {
                failures.push(format!(
                    "{sink}: ingresses={ingress_count}, forbidden event delta={delta}"
                ));
            }
            executed += 1;
        }
    }
    assert_eq!(executed, 20);
    assert!(
        failures.is_empty(),
        "executed {executed} real schedules: {}",
        failures.join("; ")
    );
}

const RETURNS: [(&str, &str, &str, Option<&str>); 4] = [
    (
        "staging_grant",
        "session.after.staging_grant",
        "op.prepared",
        Some("snapshot"),
    ),
    (
        "snapshot",
        "session.after.snapshot",
        "op.prepared",
        Some("edit"),
    ),
    ("edit", "session.after.edit", "verify.structural", None),
    (
        "structural_verify",
        "session.after.structural_verify",
        "review.computed",
        None,
    ),
];

#[test]
fn session_r17_return_from_one_call_cannot_admit_the_successor() {
    let mut failures = Vec::new();
    let mut executed = 0;
    for (sink, hook, successor_event, successor_kind) in RETURNS {
        for ingress_count in 0..=3 {
            let f = Fixture::new();
            let before = Arc::new(std::sync::Mutex::new(None));
            let observed = before.clone();
            let path = f.ledger_path();
            let cell = f.gov.file_session_halt();
            f.gov.files().hooks().arm(hook, move || {
                *observed.lock().unwrap() = Some(events(&path));
                for _ in 0..ingress_count {
                    cell.engage_emergency();
                }
            });
            let result = f.propose();
            let before = before
                .lock()
                .unwrap()
                .take()
                .expect("entered call returned");
            let after = events(&f.ledger_path());
            let delta = matching(&after, successor_event, successor_kind)
                - matching(&before, successor_event, successor_kind);
            let correct = if ingress_count == 0 {
                result.is_ok() && delta > 0
            } else {
                result.is_err() && delta == 0
            };
            assert!(!f.gov.stop_snapshot().broker_stopped);
            assert!(!f.gov.stop_snapshot().kernel_stopped);
            println!("R17/{sink}/returned/ingresses={ingress_count}: successor_delta={delta} conforms={correct}");
            if !correct {
                failures.push(format!(
                    "{sink}: ingress={ingress_count}, successor delta={delta}"
                ));
            }
            executed += 1;
        }
    }
    assert_eq!(executed, 16);
    assert!(
        failures.is_empty(),
        "executed {executed} return schedules: {}",
        failures.join("; ")
    );
}

#[test]
fn session_r21_r22_call_admissions_check_native_ledger_independently() {
    let points = [
        (
            "R21/revalidation_review",
            "session.before.revalidation_review",
            "review.computed",
        ),
        (
            "R22/request_approval",
            "session.before.request_approval",
            "approval.granted",
        ),
        ("R22/apply", "session.before.apply", "apply.prepared"),
    ];
    let mut failures = Vec::new();
    let mut executed = 0;
    for (sink, hook, event) in points {
        for ingress_count in 0..=3 {
            let f = Fixture::new();
            let id = f.pending();
            let seen = Arc::new(AtomicUsize::new(0));
            let before = Arc::new(AtomicUsize::new(usize::MAX));
            let (reached, captured) = (seen.clone(), before.clone());
            let path = f.ledger_path();
            let cell = f.gov.file_session_halt();
            f.gov.files().hooks().arm(hook, move || {
                captured.store(matching(&events(&path), event, None), Ordering::SeqCst);
                reached.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingress_count {
                    cell.engage_emergency();
                }
            });
            let result = f.approve(id);
            assert_eq!(
                seen.load(Ordering::SeqCst),
                1,
                "real route must reach {sink}"
            );
            let delta =
                matching(&events(&f.ledger_path()), event, None) - before.load(Ordering::SeqCst);
            let correct = if ingress_count == 0 {
                result.is_ok() && delta > 0
            } else {
                result.is_err() && delta == 0
            };
            assert!(!f.gov.stop_snapshot().broker_stopped);
            assert!(!f.gov.stop_snapshot().kernel_stopped);
            println!("{sink}/before/ingresses={ingress_count}: native_workers=0 event_delta={delta} conforms={correct}");
            if !correct {
                failures.push(format!("{sink}: ingress={ingress_count}, delta={delta}"));
            }
            executed += 1;
        }
    }
    assert_eq!(executed, 12);
    assert!(
        failures.is_empty(),
        "executed {executed} execution schedules: {}",
        failures.join("; ")
    );
}

#[test]
fn session_r27_normal_restore_admission_and_post_return_no_discard() {
    let mut failures = Vec::new();
    let mut executed = 0;
    for (hook, handed_off) in [
        ("session.before.request_restore", false),
        ("session.before.restore", false),
        ("session.after.restore", true),
    ] {
        for ingress_count in 0..=3 {
            let f = Fixture::new();
            let id = f.pending();
            f.approve(id).unwrap();
            let seen = Arc::new(AtomicUsize::new(0));
            let captured = Arc::new(std::sync::Mutex::new(None));
            let (reached, before) = (seen.clone(), captured.clone());
            let path = f.ledger_path();
            let cell = f.gov.file_session_halt();
            f.gov.files().hooks().arm(hook, move || {
                *before.lock().unwrap() = Some(events(&path));
                reached.fetch_add(1, Ordering::SeqCst);
                for _ in 0..ingress_count {
                    cell.engage_emergency();
                }
            });
            let result = f.undo();
            assert_eq!(
                seen.load(Ordering::SeqCst),
                1,
                "normal restore route must reach {hook}"
            );
            let before = captured.lock().unwrap().take().unwrap();
            let after = events(&f.ledger_path());
            // request_restore is itself mutating, including a declined
            // dialogue. Absence of a completed restore cannot prove that
            // this earlier call was refused before entry.
            let dialogue_records = |records: &[(String, serde_json::Value)]| {
                matching(records, "restore.approved", None)
                    + matching(records, "restore.declined", None)
            };
            let dialogue_delta = dialogue_records(&after) - dialogue_records(&before);
            if hook == "session.before.request_restore" {
                assert_eq!(
                    dialogue_delta,
                    usize::from(ingress_count == 0),
                    "request_restore must not record even a decline after ingress"
                );
            }
            let restored = matching(&after, "restore.completed", None);
            let discarded = matching(&after, "staging.discarded", None)
                - matching(&before, "staging.discarded", None);
            let correct = if ingress_count == 0 {
                result.is_ok() && restored == 1
            } else if handed_off {
                restored == 1 && discarded == 0
            } else {
                result.is_err() && restored == 0 && discarded == 0
            };
            assert!(!f.gov.stop_snapshot().broker_stopped);
            assert!(!f.gov.stop_snapshot().kernel_stopped);
            println!("R27/{hook}/ingresses={ingress_count}: restored={restored} next_discard={discarded} conforms={correct}");
            if !correct {
                failures.push(format!("{hook}: ingress={ingress_count}, restored={restored}, next_discard={discarded}"));
            }
            executed += 1;
        }
    }
    assert_eq!(executed, 12);
    assert!(
        failures.is_empty(),
        "executed {executed} restore schedules: {}",
        failures.join("; ")
    );
}

type ResourceSnapshot = std::collections::BTreeMap<std::path::PathBuf, (bool, u64, u64, Vec<u8>)>;
fn resources(root: &std::path::Path) -> ResourceSnapshot {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut ResourceSnapshot) {
        use std::os::unix::fs::MetadataExt;
        if !dir.exists() {
            return;
        }
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            assert!(!meta.file_type().is_symlink());
            let bytes = if meta.is_dir() {
                Vec::new()
            } else {
                std::fs::read(&path).unwrap()
            };
            out.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                (meta.is_dir(), meta.dev(), meta.ino(), bytes),
            );
            if meta.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn session_r18_r24_r30_r35_terminal_wrappers_and_last_owner_conserve_staging() {
    let mut failures = Vec::new();
    for route in [
        "deny",
        "cancel",
        "finish",
        "prune",
        "shutdown",
        "last_owner",
    ] {
        let f = Fixture::new();
        let id = f.pending();
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let before = resources(&staging);
        assert!(!before.is_empty(), "real pending staging");
        f.gov.file_session_halt().engage_emergency();
        match route {
            "deny" => {
                let _ = f.gov.deny(id, f.door.agent(), f.run);
            }
            "cancel" => {
                let outcome = f.door.cancel_run(f.run).unwrap().write();
                assert_eq!((outcome.owed, outcome.uncertain), (0, 0));
            }
            "finish" => {
                f.gov.finish_run(f.run);
            }
            "prune" => {
                f.gov.settle();
            }
            "shutdown" => {
                f.gov.shut_down();
            }
            "last_owner" => {}
            _ => unreachable!(),
        }
        let after_operation = resources(&staging);
        let Fixture {
            gov,
            door,
            _session,
            _root,
            _dirs,
            ..
        } = f;
        drop(door);
        drop(gov);
        drop(_session);
        // The fixture's parent roots are kept alive. Only actual broker-owned
        // destructors have run; fixture cleanup is not mistaken for the SUT.
        let after_last_owner = resources(&staging);
        let correct = before == after_operation && before == after_last_owner;
        println!(
            "R35/{route}: before={} after_call={} after_last_owner={} conforms={correct}",
            before.len(),
            after_operation.len(),
            after_last_owner.len()
        );
        if !correct {
            failures.push(route);
        }
        drop((_root, _dirs));
    }
    assert!(
        failures.is_empty(),
        "unadmitted destructor/removal in {}",
        failures.join(", ")
    );
}

#[test]
fn session_preparation_unwind_preserves_allocated_objects_without_reacquiring_metadata() {
    let mut failures = Vec::new();
    for point in [
        "session.after.staging_grant",
        "session.after.snapshot",
        "session.after.edit",
        "session.after.structural_verify",
        "session.after.review",
    ] {
        let f = Fixture::new();
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let captured = Arc::new(std::sync::Mutex::new(None));
        let before = captured.clone();
        let observed = staging.clone();
        let cell = f.gov.file_session_halt();
        f.gov.files().hooks().arm(point, move || {
            *before.lock().unwrap() = Some(resources(&observed));
            cell.engage_emergency();
            panic!("registered session ownership unwind");
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.propose()));
        assert!(result.is_err(), "the registered unwind must occur");
        let before = captured.lock().unwrap().take().unwrap();
        let after = resources(&staging);
        let correct = before == after;
        println!(
            "R18/unwind/{point}: before={} after={} conforms={correct}",
            before.len(),
            after.len()
        );
        if !correct {
            failures.push(point);
        }
    }
    assert!(
        failures.is_empty(),
        "preparation unwind exposed kernel cleanup destructors: {}",
        failures.join(", ")
    );
}

#[test]
fn session_r25_unknown_ack_is_not_a_successful_duplicate() {
    let f = Fixture::new();
    let id = f.pending();
    let unknown = CommitmentId::fresh();
    assert!(
        f.door.ack_commitment(unknown).is_err(),
        "unknown action has no exact successful receipt"
    );
    assert!(
        f.door.ack_commitment(id).is_err(),
        "pending physical outcome is not conclusive"
    );
}

#[test]
fn session_undo_index_replacement_conserves_both_runs_after_the_second_handoff() {
    let mut failures = Vec::new();
    for ingresses in 0..=3 {
        let f = Fixture::new();
        let first = f.pending();
        f.approve(first).unwrap();
        let second = match f.propose_content("second.txt", "second fixture\n").unwrap() {
            AgentOutcome::AwaitingApproval(view) => view.id,
            _ => panic!("the second real file requires its own approval"),
        };
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let captured = Arc::new(std::sync::Mutex::new(None));
        let observed = captured.clone();
        let at = staging.clone();
        let cell = f.gov.file_session_halt();
        f.gov.files().hooks().arm("session.after.apply", move || {
            *observed.lock().unwrap() = Some(resources(&at));
            for _ in 0..ingresses {
                cell.engage_emergency();
            }
        });
        let result = f.approve(second);
        let before = captured
            .lock()
            .unwrap()
            .take()
            .expect("second native apply returned");
        assert!(!before.is_empty());
        let after = resources(&staging);
        let workspace = f._dirs.0.path().join("selected/workspace");
        assert_eq!(
            std::fs::read(workspace.join("session.txt")).unwrap(),
            b"session oracle\n"
        );
        assert_eq!(
            std::fs::read(workspace.join("second.txt")).unwrap(),
            b"second fixture\n"
        );
        let correct = if ingresses == 0 {
            result.is_ok()
        } else {
            before == after
        };
        println!("REPLACEMENT/ingresses={ingresses}: both_published=true resources_before={} resources_after={} conforms={correct}", before.len(), after.len());
        if !correct {
            failures.push(ingresses);
        }
    }
    assert!(
        failures.is_empty(),
        "undo-index replacement loses independently observed resources: {failures:?}"
    );
}

#[test]
fn session_cleanup_fixed_tail_is_retained_when_ingress_follows_one_discard() {
    use std::os::unix::fs::PermissionsExt;
    let mut failures = Vec::new();
    for ingresses in 0..=3 {
        let f = Fixture::new();
        let first = f.pending();
        let second = match f.propose_content("second.txt", "second fixture\n").unwrap() {
            AgentOutcome::AwaitingApproval(view) => view.id,
            _ => panic!("pending second file"),
        };
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let roots: Vec<_> = std::fs::read_dir(&staging)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_dir())
            .collect();
        assert_eq!(roots.len(), 2);
        for root in &roots {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o500)).unwrap();
        }
        f.gov.deny(first, f.door.agent(), f.run).unwrap();
        f.gov.deny(second, f.door.agent(), f.run).unwrap();
        assert!(
            roots.iter().all(|p| p.is_dir()),
            "two actual failed removals are retained"
        );
        for root in &roots {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let captured = Arc::new(std::sync::Mutex::new(None));
        let observed = captured.clone();
        let at = staging.clone();
        let cell = f.gov.file_session_halt();
        f.gov
            .files()
            .hooks()
            .arm("session.after.discard_staging", move || {
                *observed.lock().unwrap() = Some(resources(&at));
                for _ in 0..ingresses {
                    cell.engage_emergency();
                }
            });
        let result = f.cleanup();
        let before = captured
            .lock()
            .unwrap()
            .take()
            .expect("one native discard completed");
        let after = resources(&staging);
        let correct = if ingresses == 0 {
            result.is_ok() && after.is_empty()
        } else {
            !before.is_empty() && before == after
        };
        println!(
            "CLEANUP-TAIL/ingresses={ingresses}: tail_before={} tail_after={} conforms={correct}",
            before.len(),
            after.len()
        );
        if !correct {
            failures.push(ingresses);
        }
    }
    assert!(
        failures.is_empty(),
        "one discard handoff does not admit the cleanup tail: {failures:?}"
    );
}

#[test]
fn session_commitment_capacity_is_reserved_before_native_staging_allocation() {
    let mut failures = Vec::new();
    for capacity in [1, 2] {
        let f = Fixture::new();
        f.gov
            .authority()
            .commitments()
            .set_capacity_for_test(capacity);
        let first = f.pending();
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let before = resources(&staging);
        let result = f.propose_content("second.txt", "capacity fixture\n");
        let records = events(&f.ledger_path());
        let created = matching(&records, "run.granted", None);
        let after = resources(&staging);
        let correct = if capacity == 1 {
            result.is_err() && created == 1 && after == before
        } else {
            result.is_ok() && created == 2
        };
        assert!(
            f.gov.authority().commitments().view(first).is_some(),
            "capacity never evicts the existing pending obligation"
        );
        println!(
            "COMMITMENT-CAPACITY/available={}: native_allocations={created} conforms={correct}",
            capacity - 1
        );
        if !correct {
            failures.push(capacity);
        }
    }
    assert!(
        failures.is_empty(),
        "capacity checked after allocation: {failures:?}"
    );
}

#[test]
fn session_all_256_retained_entries_precede_refusal_of_the_257th_native_allocation() {
    let f = Fixture::new();
    let staging = f.gov.files().state_dir_for_test().join("coding-runs");
    let mut retained = Vec::new();
    for action in 0..256 {
        let pending = match f
            .propose_content(&format!("item{action}.txt"), "capacity fixture\n")
            .unwrap()
        {
            AgentOutcome::AwaitingApproval(view) => view.id,
            _ => panic!("one real approval per native allocation"),
        };
        f.gov.deny(pending, f.door.agent(), f.run).unwrap();
        retained.push(pending);
        if matches!(action, 0 | 254 | 255) {
            println!(
                "INVENTORY-BOUNDARY/created={}: resource_nodes={}",
                action + 1,
                resources(&staging).len()
            );
        }
    }
    let before = resources(&staging);
    let extra = f.propose_content("overflow.txt", "must not allocate\n");
    let allocations = matching(&events(&f.ledger_path()), "run.granted", None);
    let after = resources(&staging);
    println!("INVENTORY-FULL/created=256: native_allocations={allocations} retained_nodes={} overflow_refused={}", before.len(), extra.is_err());
    assert!(
        retained
            .iter()
            .all(|id| f.gov.authority().commitments().view(*id).is_some()),
        "unacknowledged commitments remain retained"
    );
    assert!(
        extra.is_err(),
        "257th action must refuse before resource creation"
    );
    assert_eq!(allocations, 256);
    assert_eq!(before, after);
    let directories = std::fs::read_dir(&staging)
        .unwrap()
        .filter(|entry| entry.as_ref().unwrap().path().is_dir())
        .count();
    assert_eq!(
        directories, 256,
        "settlement and denial do not discard session resources"
    );
    // Stop receipts and native narrowing have an independent fixed reserve.
    let event = f._session.ingress().emergency().unwrap();
    let stop = f._session.record_stop(&event).unwrap();
    assert!(matches!(
        stop.record,
        crate::authority::evidence::EndStatus::Recorded { .. }
    ));
    assert_eq!(f._session.stop_obligations().unwrap().recorded, 1);
    assert_eq!(
        resources(&staging),
        before,
        "Stop settlement never consumes cleanup ownership"
    );
}

#[test]
fn session_poisoned_ownership_holds_preserve_resources_through_last_owner_drop() {
    let mut failures = Vec::new();
    for hold in [
        "history",
        "runs",
        "commitments",
        "pending",
        "slot",
        "kept",
        "deferred",
        "unclosed",
        "receipts",
    ] {
        let f = Fixture::new();
        let pending = f.pending();
        if hold == "slot" {
            f.approve(pending).unwrap();
        }
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let before = resources(&staging);
        assert!(!before.is_empty());
        f.gov.file_session_halt().engage_emergency();
        let _poison = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match hold {
            "history" => f.gov.file_sessions.review5_poison(),
            "runs" => f.gov.authority().runs().panic_holding_the_runs_lock(),
            "commitments" => f.gov.authority().commitments().panic_holding_entries(),
            "pending" => f.gov.control.panic_holding_pending(),
            _ => f.gov.files().session_poison_for_test(hold),
        }));
        let _settlement = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.gov.settle()));
        let Fixture {
            gov,
            door,
            _session,
            _root,
            _dirs,
            ..
        } = f;
        let _destruction = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(door);
            drop(gov);
            drop(_session);
        }));
        let after = resources(&staging);
        let correct = before == after;
        println!(
            "POISON/{hold}: resource_nodes_before={} resource_nodes_after={} conforms={correct}",
            before.len(),
            after.len()
        );
        if !correct {
            failures.push(hold);
        }
        drop((_root, _dirs));
    }
    assert!(
        failures.is_empty(),
        "poisoned inventory ownership exposed cleanup destructors: {failures:?}"
    );
}

#[test]
fn session_ack_without_exact_durable_mother_reference_retains_every_pin() {
    let mut failures = Vec::new();
    for state in [
        "pending",
        "authorized",
        "declined",
        "published",
        "uncertain",
    ] {
        let f = Fixture::new();
        let id = f.pending();
        match state {
            "authorized" => {
                f.gov.authorize(id, f.door.agent(), f.run, &Yes).unwrap();
            }
            "declined" => {
                f.gov.deny(id, f.door.agent(), f.run).unwrap();
            }
            "published" => {
                f.approve(id).unwrap();
            }
            "uncertain" => {
                f.gov.files().hooks().arm("files.before_final", || {
                    panic!("lost native outcome after handoff")
                });
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.approve(id)))
                        .is_err()
                );
            }
            _ => {}
        }
        assert!(f.gov.authority().commitments().pinned_for_test(id));
        let first = f.door.ack_commitment(id);
        let second = f.door.ack_commitment(id);
        let pinned = f.gov.authority().commitments().pinned_for_test(id);
        let correct = first.is_err() && second.is_err() && pinned;
        println!("ACK/{state}/mother_reference=absent: first_refused={} second_refused={} pinned={pinned} conforms={correct}", first.is_err(), second.is_err());
        if !correct {
            failures.push(state);
        }
    }
    assert!(
        failures.is_empty(),
        "no exact durable Mother record was supplied: {failures:?}"
    );
}

#[test]
fn session_each_native_work_step_rechecks_its_actual_prerequisites() {
    let mut failures = Vec::new();
    let mut executed = 0;
    let points = [
        (
            "staging_grant",
            "session.before.staging_grant",
            "session.entered.staging_grant",
            false,
        ),
        (
            "snapshot",
            "session.before.snapshot",
            "session.entered.snapshot",
            false,
        ),
        ("edit", "session.before.edit", "session.entered.edit", false),
        (
            "structural_verify",
            "session.before.structural_verify",
            "session.entered.structural_verify",
            false,
        ),
        (
            "review",
            "session.before.review",
            "session.entered.review",
            false,
        ),
        (
            "revalidation_review",
            "session.before.revalidation_review",
            "session.entered.revalidation_review",
            true,
        ),
        (
            "request_approval",
            "session.before.request_approval",
            "session.entered.request_approval",
            true,
        ),
        (
            "apply",
            "session.before.apply",
            "session.entered.apply",
            true,
        ),
    ];
    for (sink, point, entry, execution) in points {
        for cause in [
            "none",
            "mission_stop",
            "cancel",
            "revoke",
            "grant_expiry",
            "kernel_stop",
            "commitment_monotonic",
            "commitment_wall",
        ] {
            // No backend commitment exists during the preparation half.
            if !execution && cause.starts_with("commitment_") {
                continue;
            }
            let duration = if cause == "grant_expiry" {
                Duration::from_secs(60)
            } else {
                Duration::from_secs(3600)
            };
            let f = Fixture::with_roots_and_duration(
                temp_root("session-prerequisite-state"),
                temp_root("session-prerequisite-selected"),
                duration,
            );
            let id = execution.then(|| f.pending());
            let reached = Arc::new(AtomicUsize::new(0));
            let entered = Arc::new(AtomicUsize::new(0));
            let (seen, called) = (reached.clone(), entered.clone());
            let (gov, door, clock, run, grant) = (
                f.gov.clone(),
                f.door.clone(),
                f.clock.clone(),
                f.run,
                f.grant,
            );
            f.gov.files().hooks().arm(point, move || {
                seen.fetch_add(1, Ordering::SeqCst);
                match cause {
                    "none" => {}
                    "mission_stop" => gov.file_session_halt().engage(),
                    "cancel" => {
                        door.cancel_run(run).unwrap().write();
                    }
                    "revoke" => gov.revoke_grant(grant).unwrap(),
                    "grant_expiry" => clock.advance(Duration::from_secs(60)),
                    "kernel_stop" => {
                        gov.kernel_stop_authority().engage();
                    }
                    "commitment_monotonic" => clock.advance(Duration::from_secs(600)),
                    "commitment_wall" => clock.suspend(Duration::from_secs(600)),
                    _ => unreachable!(),
                }
            });
            f.gov.files().hooks().arm(entry, move || {
                called.fetch_add(1, Ordering::SeqCst);
            });
            let success = if let Some(id) = id {
                f.approve(id).is_ok()
            } else {
                f.propose().is_ok()
            };
            assert_eq!(
                reached.load(Ordering::SeqCst),
                1,
                "real {sink} route reached"
            );
            let calls = entered.load(Ordering::SeqCst);
            let correct = if cause == "none" {
                success && calls == 1
            } else {
                !success && calls == 0
            };
            println!("PREREQUISITE/{sink}/{cause}: handoffs={calls} success={success} conforms={correct}");
            if !correct {
                failures.push((sink, cause));
            }
            executed += 1;
        }
    }
    assert_eq!(executed, 54);
    assert!(
        failures.is_empty(),
        "54 executed prerequisite schedules: {failures:?}"
    );
}

#[test]
fn session_pending_capacity_refuses_before_the_33rd_native_allocation() {
    let f = Fixture::new();
    let mut pending = Vec::new();
    for index in 0..32 {
        let id = match f
            .propose_content(&format!("pending{index}.txt"), "pending capacity\n")
            .unwrap()
        {
            AgentOutcome::AwaitingApproval(view) => view.id,
            _ => panic!("real pending allocation"),
        };
        pending.push(id);
        if matches!(index, 0 | 30 | 31) {
            println!("PENDING-BOUNDARY/retained={}", index + 1);
        }
    }
    let staging = f.gov.files().state_dir_for_test().join("coding-runs");
    let before = resources(&staging);
    let overflow = f.propose_content("pendingoverflow.txt", "must not allocate\n");
    let allocations = matching(&events(&f.ledger_path()), "run.granted", None);
    println!(
        "PENDING-FULL/available=0 native_allocations={allocations} overflow_refused={}",
        overflow.is_err()
    );
    assert!(overflow.is_err());
    assert!(pending
        .iter()
        .all(|id| f.gov.authority().commitments().pinned_for_test(*id)));
    assert_eq!(resources(&staging), before);
    assert_eq!(
        allocations, 32,
        "complete capacity reservation precedes native staging"
    );
}

#[test]
fn session_precommitment_retention_survives_finish_and_run_capacity_pressure() {
    let mut failures = Vec::new();
    for point in [
        "session.before.staging_grant",
        "session.after.staging_grant",
        "session.after.review",
        "prepare.before_insert",
    ] {
        let f = Fixture::new();
        f.gov.authority().runs().set_capacity_for_test(1);
        assert!(
            f.door.open_file_run(&f.attachment).is_err(),
            "full live registry refuses another run"
        );
        let observed = Arc::new(std::sync::Mutex::new(None));
        let captured = observed.clone();
        let (gov, door, attachment, run) =
            (f.gov.clone(), f.door.clone(), f.attachment.clone(), f.run);
        let hooks = if point == "prepare.before_insert" {
            f.gov.hooks()
        } else {
            f.gov.files().hooks()
        };
        hooks.arm(point, move || {
            gov.finish_run(run);
            let rejected = door.open_file_run(&attachment).is_err();
            let retained = gov
                .authority()
                .runs()
                .views()
                .iter()
                .any(|view| view.id == run);
            *captured.lock().unwrap() = Some((rejected, retained));
        });
        let _result = f.propose();
        let (rejected, retained) = observed
            .lock()
            .unwrap()
            .take()
            .expect("actual preparation boundary");
        let correct = rejected && retained;
        println!("RUN-RETENTION/{point}: new_run_refused={rejected} original_retained={retained} conforms={correct}");
        if !correct {
            failures.push(point);
        }
    }
    let control = Fixture::new();
    control.gov.authority().runs().set_capacity_for_test(2);
    assert!(
        control.door.open_file_run(&control.attachment).is_ok(),
        "available run capacity positive control"
    );
    assert!(
        failures.is_empty(),
        "pre-commitment preparation lost its original run: {failures:?}"
    );
}

#[test]
fn session_same_word_atomic_admission_is_ordered_after_unscheduled_ingress() {
    use crate::authority::file_session::HaltCell;
    for ingresses in 0..=3 {
        let cell = HaltCell::new_host();
        let original = cell.session_word();
        let competing = cell.clone();
        cell.1.arm("session.before_compare", move || {
            for _ in 0..ingresses {
                competing.engage_emergency();
            }
        });
        let admitted = cell.session_compare(original, original);
        assert_eq!(
            admitted.is_ok(),
            ingresses == 0,
            "the same-value CAS must order against ingress, not a pre-hook load"
        );
        assert_eq!(cell.ingress_epoch(), ingresses);
        println!(
            "ATOMIC/ingresses={ingresses}: admitted={} generation={}",
            admitted.is_ok(),
            cell.ingress_epoch()
        );
    }
}

#[test]
fn session_normal_final_owner_drop_is_not_an_implicit_cleanup_request() {
    let mut failures = Vec::new();
    for published in [false, true] {
        let f = Fixture::new();
        let id = f.pending();
        if published {
            f.approve(id).unwrap();
        }
        let staging = f.gov.files().state_dir_for_test().join("coding-runs");
        let before = resources(&staging);
        assert!(!before.is_empty());
        let Fixture {
            gov,
            door,
            _session,
            _root,
            _dirs,
            ..
        } = f;
        drop(door);
        drop(gov);
        drop(_session);
        let after = resources(&staging);
        let correct = before == after;
        println!(
            "NORMAL-DROP/published={published}: before={} after={} conforms={correct}",
            before.len(),
            after.len()
        );
        if !correct {
            failures.push(published);
        }
        drop((_root, _dirs));
    }
    assert!(
        failures.is_empty(),
        "ordinary final-owner destruction removed unretired resources: {failures:?}"
    );
}

#[test]
fn session_r19_r20_registry_insertion_and_authorization_keep_original_lifetime() {
    use crate::authority::commitment::CommitmentState;
    let mut failures = Vec::new();
    let mut cases = 0;
    for (point, authorization) in [
        ("prepare.before_insert", false),
        ("place.before_fill", false),
        ("decision8.pending_validated", false),
        ("authorize.recorded", true),
        ("complete.checked", true),
    ] {
        for ingress_count in 0..=3 {
            let f = Fixture::new();
            let pending = authorization.then(|| f.pending());
            let before = Arc::new(std::sync::Mutex::new(None));
            let capture = before.clone();
            let staging = f.gov.files().state_dir_for_test().join("coding-runs");
            let observed = staging.clone();
            let cell = f.gov.file_session_halt();
            f.gov.hooks().arm(point, move || {
                *capture.lock().unwrap() = Some(resources(&observed));
                for _ in 0..ingress_count {
                    cell.engage_emergency();
                }
            });
            let success = if let Some(id) = pending {
                f.gov.authorize(id, f.door.agent(), f.run, &Yes).is_ok()
            } else {
                matches!(f.propose(), Ok(AgentOutcome::AwaitingApproval(_)))
            };
            let before = before
                .lock()
                .unwrap()
                .take()
                .expect("real insertion boundary reached");
            let views = f.gov.authority().commitments().views_of_run(f.run);
            let live = views.iter().any(|view| {
                matches!(
                    view.state,
                    CommitmentState::Prepared
                        | CommitmentState::Authorized
                        | CommitmentState::Executing
                )
            });
            let unchanged = before == resources(&staging);
            let correct = if ingress_count == 0 {
                success && live
            } else {
                !success && !live && unchanged
            };
            println!("R19-R20/{point}/ingresses={ingress_count}: native_workers=0 success={success} live={live} resources_retained={unchanged} conforms={correct}");
            if !correct {
                failures.push(format!("{point}/ingresses={ingress_count}"));
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 20);
    assert!(
        failures.is_empty(),
        "executed {cases} registry schedules: {}",
        failures.join(", ")
    );
}

#[test]
fn session_discharge_requires_separate_sync_and_keeps_the_owner_file() {
    for published in [false, true] {
        for interrupt_sync in [false, true] {
            let f = Fixture::new();
            let id = f.pending();
            if published {
                f.approve(id).unwrap();
                let cleanup = f.cleanup().unwrap();
                assert_eq!(cleanup.settled, 0, "the currently offered undo is excluded");
                assert_eq!(
                    cleanup.still_owed, 1,
                    "excluded resources stay in the total"
                );
            } else {
                f.gov.deny(id, f.door.agent(), f.run).unwrap();
            }
            let entered = Arc::new(AtomicUsize::new(0));
            let seen = entered.clone();
            f.gov
                .files()
                .hooks()
                .arm("session.entered.finalization_sync", move || {
                    seen.fetch_add(1, Ordering::SeqCst);
                });
            if interrupt_sync {
                let ingress = f._session.ingress();
                f.gov
                    .files()
                    .hooks()
                    .arm("session.before.finalization_sync", move || {
                        ingress.emergency().unwrap();
                    });
            }
            let seq = f.sequence.fetch_add(1, Ordering::SeqCst) as u64 + 1;
            let quit = f._session.ingress().accept(CommandKind::Quit, seq).unwrap();
            let result = f._session.settle_close_resources(&quit);
            let inventory = f._session.inventory().unwrap();
            assert_eq!(inventory.entries.len(), 1);
            let entry = &inventory.entries[0];
            assert!(
                entry.staging_absent,
                "actual discard precedes the separate sync boundary"
            );
            assert_eq!(entered.load(Ordering::SeqCst), usize::from(!interrupt_sync));
            assert_eq!(
                entry.parent_synced, !interrupt_sync,
                "absence is not durability"
            );
            assert_eq!(entry.discharged, !interrupt_sync);
            assert_eq!(entry.intentionally_retained, published);
            assert_eq!(result.is_ok(), !interrupt_sync);
            let workspace = f._dirs.0.path().join("selected/workspace/session.txt");
            assert_eq!(workspace.exists(), published);
            if published {
                assert_eq!(std::fs::read(workspace).unwrap(), b"session oracle\n");
            }
            let staging = f.gov.files().state_dir_for_test().join("coding-runs");
            let retained: Vec<_> = std::fs::read_dir(staging)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(retained.len(), usize::from(published));
            assert!(retained
                .iter()
                .all(|name| name.to_string_lossy().ends_with(".apply")));
        }
    }
}

#[test]
fn session_undo_grant_has_its_own_original_request_handoff() {
    for point in ["session.before.undo_grant", "session.reserved.undo_grant"] {
        for interrupt in [false, true] {
            let f = Fixture::new();
            let id = f.pending();
            f.approve(id).unwrap();
            let entered = Arc::new(AtomicUsize::new(0));
            let seen = entered.clone();
            f.gov
                .files()
                .hooks()
                .arm("session.entered.undo_grant", move || {
                    seen.fetch_add(1, Ordering::SeqCst);
                });
            if interrupt {
                let ingress = f._session.ingress();
                f.gov.files().hooks().arm(point, move || {
                    ingress.emergency().unwrap();
                });
            }
            let result = f.undo();
            assert_eq!(result.is_ok(), !interrupt);
            assert_eq!(entered.load(Ordering::SeqCst), usize::from(!interrupt));
            assert_eq!(
                f._dirs
                    .0
                    .path()
                    .join("selected/workspace/session.txt")
                    .exists(),
                interrupt
            );
            assert_eq!(
                matching(&events(&f.ledger_path()), "restore.completed", None),
                usize::from(!interrupt)
            );
        }
    }
}

#[test]
fn session_execution_guard_unwind_retains_the_unrecorded_outcome() {
    let f = Fixture::new();
    let id = f.pending();
    f.gov.files().hooks().arm("files.before_final", || {
        panic!("registered in-flight native unwind");
    });
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.approve(id))).is_err());
    // Read actual primary-store rows under its existing connection hold;
    // this store deliberately excludes an independent SQLite connection.
    let rows = f.primary.store().rows();
    let count = |phase: &str| -> usize {
        rows.iter()
            .filter(|row| {
                row.commitment.as_deref() == Some(id.to_string().as_str()) && row.phase == phase
            })
            .count()
    };
    assert_eq!(
        count("started"),
        1,
        "positive control: actual execution started"
    );
    assert_eq!(
        count("finished"),
        0,
        "a session guard destructor must not create an outcome record"
    );
    assert!(f.gov.authority().commitments().pinned_for_test(id));
    assert_eq!(
        f.gov.authority().commitments().view(id).unwrap().state,
        crate::authority::commitment::CommitmentState::Executing
    );
    let staging = f.gov.files().state_dir_for_test().join("coding-runs");
    let before = resources(&staging);
    assert!(!before.is_empty());
    let Fixture {
        gov,
        door,
        _session,
        _root,
        _dirs,
        ..
    } = f;
    drop(door);
    drop(gov);
    drop(_session);
    assert_eq!(
        resources(&staging),
        before,
        "uncertain ownership survives every last-owner drop"
    );
    drop((_root, _dirs));
}

#[test]
fn session_stop_workers_record_exact_ingress_without_minting_another_event() {
    for count in [1, 4, 5] {
        let f = Fixture::new();
        let ingress = f._session.ingress();
        let events: Vec<_> = (0..count).map(|_| ingress.emergency().unwrap()).collect();
        assert_eq!(events.last().unwrap().generation(), count as u64);
        let mut sequences = std::collections::BTreeSet::new();
        for event in &events {
            let receipt = f._session.record_stop(event).unwrap();
            assert_eq!(receipt.event, *event);
            assert!(receipt.snapshot.kernel_stopped && receipt.snapshot.broker_stopped);
            let crate::authority::evidence::EndStatus::Recorded { seq } = receipt.record else {
                panic!("real positive control requires the actual durable record");
            };
            assert!(
                sequences.insert(seq),
                "each event has its own existing-schema record"
            );
            let rows = f.primary.store().rows();
            let row = rows.iter().find(|row| row.seq == seq).unwrap();
            assert_eq!(row.phase, "emergency_stop");
            let body: serde_json::Value = serde_json::from_str(&row.body).unwrap();
            assert_eq!(
                body["detail"]["stop_number"],
                receipt.native_stop_number.to_string()
            );
            assert_eq!(receipt.consequences.owed, 0);
            assert_eq!(receipt.consequences.uncertain, 0);
            let before = f.primary.store().rows();
            assert_eq!(f._session.record_stop(event).unwrap(), receipt);
            assert_eq!(
                f.primary.store().rows(),
                before,
                "duplicate is the exact retained receipt"
            );
        }
        let rows = f.primary.store().rows();
        assert_eq!(
            rows.iter()
                .filter(|row| row.phase == "emergency_stop")
                .count(),
            count
        );
        assert_eq!(
            ingress.accept(CommandKind::Status, 2).unwrap().generation(),
            count as u64,
            "native narrowing must not issue another session ingress"
        );
        let ledger = f._session.stop_obligations().unwrap();
        assert_eq!(
            (
                ledger.received,
                ledger.recorded,
                ledger.pending,
                ledger.in_flight
            ),
            (count, count, 0, 0)
        );
    }
}

#[test]
fn session_stop_unknown_receipt_and_failed_record_remain_unsettled() {
    let f = Fixture::new();
    let event = f._session.ingress().emergency().unwrap();
    let other = Fixture::new();
    let foreign = other._session.ingress().emergency().unwrap();
    let before = f.primary.store().rows();
    assert!(f._session.record_stop(&foreign).is_err());
    assert_eq!(
        f.primary.store().rows(),
        before,
        "foreign ingress cannot narrow or record"
    );
    f.primary.store().refuse_inserts(true);
    let receipt = f._session.record_stop(&event).unwrap();
    assert!(receipt.snapshot.kernel_stopped && receipt.snapshot.broker_stopped);
    assert_eq!(receipt.record, crate::authority::evidence::EndStatus::Owed);
    let debt = f._session.stop_obligations().unwrap();
    assert_eq!((debt.received, debt.recorded, debt.owed), (1, 0, 1));
    assert!(!f
        .primary
        .store()
        .rows()
        .iter()
        .any(|row| row.phase == "emergency_stop"));
    f.primary.store().refuse_inserts(false);
}

#[test]
fn session_stop_four_workers_leave_the_fifth_event_as_recording_debt() {
    let f = Fixture::new();
    let gates: Arc<Vec<_>> = Arc::new((0..4).map(|_| crate::test_hooks::Gate::new()).collect());
    let reached = Arc::new(AtomicUsize::new(0));
    let observe = reached.clone();
    let held = gates.clone();
    f.gov
        .authority()
        .hooks()
        .every("session.stop.started", move || {
            let index = observe.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = held.get(index) {
                gate.hold();
            }
        });
    let mut workers = Vec::new();
    for index in 0..4 {
        let event = f._session.ingress().emergency().unwrap();
        let session = f._session.clone();
        workers.push(std::thread::spawn(move || session.record_stop(&event)));
        gates[index].reached();
    }
    let fifth = f._session.ingress().emergency().unwrap();
    let refused = f._session.record_stop(&fifth);
    let held_debt = f._session.stop_obligations().unwrap();
    // Release before assertions, so even a detected plant leaves no held worker.
    for gate in gates.iter() {
        gate.open();
    }
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    assert!(refused.is_err());
    assert_eq!(
        (held_debt.received, held_debt.in_flight, held_debt.pending),
        (5, 4, 1)
    );
    assert_eq!(held_debt.recorded, 0);
    f._session.record_stop(&fifth).unwrap();
    let rows = f.primary.store().rows();
    assert_eq!(
        rows.iter()
            .filter(|row| row.phase == "emergency_stop")
            .count(),
        5
    );
    assert_eq!(reached.load(Ordering::SeqCst), 5);
    assert_eq!(f._session.stop_obligations().unwrap().recorded, 5);
}

#[test]
fn session_stop_lost_worker_result_is_unknown_even_after_actual_record_commit() {
    let f = Fixture::new();
    let event = f._session.ingress().emergency().unwrap();
    f.gov.authority().hooks().arm("session.stop.returned", || {
        panic!("lost result after native Stop")
    });
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f
        ._session
        .record_stop(&event)))
    .is_err());
    let rows = f.primary.store().rows();
    assert_eq!(
        rows.iter()
            .filter(|row| row.phase == "emergency_stop")
            .count(),
        1
    );
    let debt = f._session.stop_obligations().unwrap();
    assert_eq!(
        (debt.received, debt.uncertain, debt.recorded, debt.in_flight),
        (1, 1, 0, 0)
    );
    assert!(
        f._session.record_stop(&event).is_err(),
        "an unknown result is not a successful duplicate"
    );
    assert_eq!(f.primary.store().rows(), rows);
}

#[test]
fn session_stop_receipts_are_bounded_without_losing_overflow_denial() {
    let f = Fixture::new();
    for number in 1..=256 {
        assert_eq!(
            f._session.ingress().emergency().unwrap().generation(),
            number
        );
    }
    assert!(f._session.ingress().emergency().is_err());
    let debt = f._session.stop_obligations().unwrap();
    assert_eq!((debt.received, debt.pending, debt.recorded), (256, 256, 0));
    assert!(debt.overflowed);
    assert_eq!(
        f._session.state(),
        crate::host_session::SessionState::Failed
    );
    assert!(f._session.ingress().accept(CommandKind::Run, 2).is_err());
}

#[test]
fn session_stop_poison_cannot_be_reported_as_completed_recording() {
    for hold in ["runs", "commitments", "pending", "lease_slot"] {
        let f = Fixture::new();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match hold {
                "runs" => f.gov.authority().runs().panic_holding_the_runs_lock(),
                "commitments" => f.gov.authority().commitments().panic_holding_entries(),
                "pending" => f.gov.control.panic_holding_pending(),
                "lease_slot" => f.gov.authority().commitments().panic_holding_lease_slot(),
                _ => unreachable!(),
            }))
            .is_err()
        );
        let event = f._session.ingress().emergency().unwrap();
        let _result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f._session.record_stop(&event)
        }));
        let debt = f._session.stop_obligations().unwrap();
        assert_eq!(
            debt.recorded, 0,
            "poisoned {hold} is not a conclusive worker result"
        );
        assert_eq!(
            debt.uncertain + debt.failed,
            1,
            "poisoned {hold} retains its exact event obligation"
        );
    }
}

#[test]
fn session_stop_keeps_every_consequence_status_beside_the_final_record() {
    let f = Fixture::new();
    for name in ["first.txt", "second.txt"] {
        assert!(matches!(
            f.propose_content(name, "two pending actions\n").unwrap(),
            AgentOutcome::AwaitingApproval(_)
        ));
    }
    let staging = f.gov.files().state_dir_for_test().join("coding-runs");
    let before = resources(&staging);
    let event = f._session.ingress().emergency().unwrap();
    f.primary.store().refuse_next_insert();
    let receipt = f._session.record_stop(&event).unwrap();
    assert_eq!(
        (
            receipt.consequences.recorded,
            receipt.consequences.owed,
            receipt.consequences.uncertain
        ),
        (1, 1, 0),
        "the last successful consequence cannot replace the first append's owed result"
    );
    assert!(matches!(
        receipt.record,
        crate::authority::evidence::EndStatus::Recorded { .. }
    ));
    assert_eq!(f._session.stop_obligations().unwrap().recorded, 0);
    assert_eq!(f._session.stop_obligations().unwrap().owed, 1);
    assert_eq!(resources(&staging), before);
}

#[test]
fn session_stop_spawn_failure_retains_the_unrecorded_event() {
    let f = Fixture::new();
    let event = f._session.ingress().emergency().unwrap();
    let before = f.primary.store().rows();
    f._session.stop_spawn_failed(&event).unwrap();
    assert_eq!(f.primary.store().rows(), before);
    let debt = f._session.stop_obligations().unwrap();
    assert_eq!((debt.received, debt.failed, debt.recorded), (1, 1, 0));
    assert!(f._session.record_stop(&event).is_err());
    assert_eq!(
        f._session.state(),
        crate::host_session::SessionState::Failed
    );
}

#[test]
fn session_mother_context_refuses_its_cancelled_native_run_without_session_stop() {
    let f = Fixture::new();
    let context = f.door.context();
    assert!(context
        .admit(crate::host_session::MotherStep::LaneInvocation)
        .is_ok());
    f.door.cancel_run(f.run).unwrap().write();
    for step in [
        crate::host_session::MotherStep::LaneInvocation,
        crate::host_session::MotherStep::Transition,
        crate::host_session::MotherStep::PrivateWrite,
    ] {
        assert!(
            context.admit(step).is_err(),
            "the original request CAS cannot substitute for the native run prerequisite"
        );
    }
}

#[test]
fn session_continuation_native_preparation_uses_the_actual_approval_scope() {
    let f = Fixture::new();
    let first = f.pending();
    let captured = Arc::new(std::sync::Mutex::new(None));
    let (seen, gov, run) = (captured.clone(), f.gov.clone(), f.run);
    f.gov.files().hooks().arm("files.execute_start", move || {
        *seen.lock().unwrap() = Some(
            gov.authority()
                .commitments()
                .session_request(first, run)
                .unwrap(),
        );
    });
    f.approve(first).unwrap();
    let approval = captured
        .lock()
        .unwrap()
        .take()
        .expect("actual native approval handoff");
    f._session.expire_request_for_test(1);
    assert!(f.propose_content("stale.txt", "stale\n").is_err());
    let second = f
        .gov
        .propose_session_file(
            f.door.agent(),
            f.run,
            &FileCreate {
                name: "continued.txt".into(),
                content: "continued\n".into(),
            },
            &approval,
        )
        .unwrap();
    assert_eq!(
        second.state,
        crate::authority::commitment::CommitmentState::Prepared
    );
    assert_eq!(
        matching(&events(&f.ledger_path()), "run.granted", None),
        2,
        "new root owns every actual native preparation child while the original remains expired"
    );
    assert!(!f
        ._dirs
        .0
        .path()
        .join("selected/workspace/continued.txt")
        .exists());
    assert!(f.propose_content("stale.txt", "stale\n").is_err());
}

fn close_opening(f: &Fixture) -> (std::path::PathBuf, String) {
    use sha2::{Digest, Sha256};
    // Existing canonical schema, independently prepared administrative
    // Mother opening. It creates no operational Mother authority.
    let previous = "0".repeat(64);
    let event = r#"{"event":"session_opened"}"#;
    let mut hash = Sha256::new();
    hash.update(b"nexus-mother-journal-v2\n");
    for part in [
        previous.as_bytes(),
        &[2],
        &0u64.to_be_bytes(),
        &0u64.to_be_bytes(),
        event.as_bytes(),
    ] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    let hash = hex::encode(hash.finalize());
    let line = format!("{{\"format\":2,\"seq\":0,\"prev\":\"{previous}\",\"mission\":0,\"event\":{event},\"hash\":\"{hash}\"}}\n");
    let path = f._root.0.path().join("mother-journal.jsonl");
    std::fs::write(&path, &line).unwrap();
    std::fs::File::open(&path).unwrap().sync_all().unwrap();
    std::fs::File::open(f._root.0.path())
        .unwrap()
        .sync_all()
        .unwrap();
    (path, line)
}

#[test]
fn session_final_seal_orders_against_ingress_and_reassesses_the_exact_event() {
    for interferes in [false, true] {
        let f = Fixture::new();
        let (path, line) = close_opening(&f);
        let ingress = f._session.ingress();
        let quit = ingress.accept(CommandKind::Quit, 2).unwrap();
        f._session.settle_close_authority(&quit).unwrap();
        let received = Arc::new(std::sync::Mutex::new(None));
        if interferes {
            let received = received.clone();
            let late = ingress.clone();
            f.gov
                .file_session_halt()
                .1
                .arm("session.before_compare", move || {
                    *received.lock().unwrap() = Some(late.emergency().unwrap());
                });
        }
        let sealed = f._session.seal_close(&quit);
        assert_eq!(
            sealed.is_ok(),
            !interferes,
            "ingress between assessment and seal invalidates the old proof"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            line,
            "assessment/seal writes neither end record"
        );
        if interferes {
            assert_eq!(
                f._session.state(),
                crate::host_session::SessionState::ClosingRestricted
            );
            let event = received.lock().unwrap().take().unwrap();
            f._session.record_stop(&event).unwrap();
            assert_eq!(
                f.primary
                    .store()
                    .rows()
                    .iter()
                    .filter(|row| row.phase == "emergency_stop")
                    .count(),
                1
            );
            let seal = f
                ._session
                .seal_close(&quit)
                .expect("fresh proof after exact Stop settlement may seal idle emergency");
            assert_eq!(
                ingress.emergency(),
                Err(crate::host_session::SessionError::SessionEnded)
            );
            drop(seal);
        } else {
            drop(sealed.unwrap());
        }
        assert_eq!(
            f._session.state(),
            crate::host_session::SessionState::ClosedUnclean,
            "dropping the finalizer cannot manufacture either end or delivery"
        );
    }
}

#[test]
fn session_close_keeps_private_resource_and_exact_ack_debt_independent() {
    for private_debt in [false, true] {
        let f = Fixture::new();
        let (path, opening) = close_opening(&f);
        let expected_file = if private_debt {
            let path = f._root.0.path().join("retained-private-temp");
            let obligation = f.door.context().reserve_private(&path, None).unwrap();
            std::fs::write(&path, b"owned private material").unwrap();
            obligation
                .observe_created(std::fs::metadata(&path).unwrap())
                .unwrap();
            drop(obligation);
            path
        } else {
            let id = f.pending();
            f.approve(id).unwrap();
            f._dirs.0.path().join("selected/workspace/session.txt")
        };
        let before = std::fs::read(&expected_file).unwrap();
        let request = f._session.ingress().accept(CommandKind::Quit, 3).unwrap();
        let inventory = f._session.settle_close_resources(&request).unwrap();
        assert!(inventory.entries.iter().all(|e| e.discharged));
        f._session.settle_close_authority(&request).unwrap();
        let resources = f._session.resource_obligations().unwrap();
        if private_debt {
            assert_eq!(resources.private_unresolved, 1);
        } else {
            assert_eq!(resources.unresolved, 0, "native resource closure does not discharge the missing exact Mother acknowledgement");
        }
        assert!(
            f._session.seal_close(&request).is_err(),
            "independent {} debt must refuse seal",
            if private_debt {
                "private resource"
            } else {
                "exact acknowledgement"
            }
        );
        assert_eq!(std::fs::read(&expected_file).unwrap(), before);
        assert_eq!(std::fs::read_to_string(path).unwrap(), opening);
        assert!(!f
            .primary
            .store()
            .rows()
            .iter()
            .any(|r| r.phase == "session_closed"));
    }
}

#[test]
fn session_undo_reports_native_handoff_and_physical_result_independently() {
    use crate::host_session::outcome::{GrantClosure, RestoreOutcome};
    for (hook, entered) in [
        ("session.before.request_restore", false),
        ("session.before.restore", false),
        ("session.after.restore", true),
    ] {
        for interfere in [false, true] {
            let f = Fixture::new();
            let id = f.pending();
            f.approve(id).unwrap();
            let request = f._session.ingress().accept(CommandKind::Undo, 3).unwrap();
            let ingress = f._session.ingress();
            f.gov.files().hooks().arm(hook, move || {
                if interfere {
                    ingress.emergency().unwrap();
                }
            });
            let _result = f._session.undo(&request);
            let facts = request.restore_observation();
            assert_eq!(facts.request, 3);
            assert_eq!(facts.target, request.context());
            assert_eq!(facts.handed_off, !interfere || entered);
            assert_eq!(
                facts.outcome,
                if !interfere || entered {
                    RestoreOutcome::Completed
                } else {
                    RestoreOutcome::NotEntered
                }
            );
            assert_eq!(facts.interrupted, interfere);
            let target = f._dirs.0.path().join("selected/workspace/session.txt");
            assert_eq!(
                target.exists(),
                interfere && !entered,
                "primitive target observation independently checks completion"
            );
            assert_eq!(
                matching(&events(&f.ledger_path()), "restore.completed", None),
                usize::from(!interfere || entered)
            );
            if !interfere || entered {
                assert_eq!(facts.grant, GrantClosure::Closed);
            }
        }
    }
    let f = Fixture::new();
    let id = f.pending();
    f.approve(id).unwrap();
    let target = f._dirs.0.path().join("selected/workspace/session.txt");
    std::fs::write(&target, b"Owner changed bytes").unwrap();
    let request = f._session.ingress().accept(CommandKind::Undo, 3).unwrap();
    assert!(f._session.undo(&request).is_err());
    let facts = request.restore_observation();
    assert!(
        facts.handed_off,
        "native restore was entered and refused its changed target"
    );
    assert_eq!(facts.outcome, RestoreOutcome::Refused);
    assert_eq!(facts.grant, GrantClosure::Closed);
    assert_eq!(std::fs::read(target).unwrap(), b"Owner changed bytes");
}

#[test]
fn session_undo_acceptance_has_no_busy_target_and_returned_slot_has_new_version() {
    use std::sync::atomic::AtomicBool;
    struct DeclineWhileQueued {
        ingress: crate::host_session::SessionIngress,
        accepted: AtomicBool,
    }
    impl ControlConfirmer for DeclineWhileQueued {
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
        fn confirm_undo(&self, _: &UndoConfirmation) -> bool {
            self.accepted.store(
                self.ingress.accept(CommandKind::Undo, 5).is_ok(),
                Ordering::SeqCst,
            );
            false
        }
    }
    let f = Fixture::new();
    let id = f.pending();
    f.approve(id).unwrap();
    let ingress = f._session.ingress();
    let first = ingress.accept(CommandKind::Undo, 3).unwrap();
    let queued = ingress.accept(CommandKind::Undo, 4).unwrap();
    assert_eq!(first.context(), queued.context());
    let confirmation = DeclineWhileQueued {
        ingress: ingress.clone(),
        accepted: AtomicBool::new(false),
    };
    assert!(f._session.undo_confirmed(&first, &confirmation).is_err());
    assert!(
        !confirmation.accepted.load(Ordering::SeqCst),
        "reader cannot bind undo while the exact slot is borrowed"
    );
    assert!(
        f._session.undo(&queued).is_err(),
        "returned slot is a new version, not the queued command's target"
    );
    let fresh = ingress.accept(CommandKind::Undo, 6).unwrap();
    assert_ne!(fresh.context(), first.context());
    assert!(
        f._session.undo(&fresh).is_ok(),
        "fresh same-session normal undo remains available after decline"
    );
    assert!(!f
        ._dirs
        .0
        .path()
        .join("selected/workspace/session.txt")
        .exists());
    assert_eq!(
        matching(&events(&f.ledger_path()), "restore.completed", None),
        1
    );
}

#[test]
fn session_result_facts_are_exact_observations_not_authority_or_generic_success() {
    use crate::host_session::outcome::RecordingStatus;
    for (hook, after) in [
        ("session.before.apply", false),
        ("session.after.apply", true),
    ] {
        for interfere in [false, true] {
            let f = Fixture::new();
            let id = f.pending();
            let request = f
                ._session
                .ingress()
                .accept(CommandKind::Approve, 2)
                .unwrap();
            let before = f._session.operation_observation(&request).unwrap();
            assert_eq!(before.session, f._session.identity());
            assert_eq!(before.request, 2);
            assert_eq!(before.target, request.context());
            assert_eq!(before.admitted_steps, 0, "acceptance is not admission");
            assert!(!before.apply_handed_off);
            assert_eq!(before.kernel, RecordingStatus::NotStarted);
            assert_eq!(before.primary, RecordingStatus::NotStarted);
            assert!(before.calls.is_empty());
            let ingress = f._session.ingress();
            f.gov.files().hooks().arm(hook, move || {
                if interfere {
                    ingress.emergency().unwrap();
                }
            });
            let _returned = f._session.approve_file(&request);
            let facts = f._session.operation_observation(&request).unwrap();
            let published = !interfere || after;
            assert_eq!(facts.apply_handed_off, published);
            assert!(facts.admitted_steps > 0);
            assert_eq!(facts.ingresses, u64::from(interfere));
            assert_eq!(facts.actions.len(), 1);
            assert_eq!(facts.actions[0].commitment, Some(id));
            assert_eq!(facts.actions[0].run, f.run);
            assert_eq!(
                facts.actions[0].mother,
                RecordingStatus::Unknown,
                "the native-only fixture has no Mother terminal record"
            );
            assert_ne!(
                facts.actions[0].acknowledgement,
                crate::host_session::outcome::Acknowledgement::Verified
            );
            assert_eq!(
                std::fs::read(f._dirs.0.path().join("selected/workspace/session.txt")).ok(),
                published.then(|| b"session oracle\n".to_vec())
            );
            assert_eq!(
                matching(&events(&f.ledger_path()), "apply.completed", None),
                usize::from(published)
            );
            if published {
                assert_eq!(facts.kernel, RecordingStatus::Recorded);
                assert_eq!(facts.primary, RecordingStatus::Recorded);
                assert!(matches!(
                    facts.actions[0].publication,
                    crate::authority::commitment::PublicationClass::Published {
                        recovery_required: false,
                        ..
                    }
                ));
            } else {
                assert_ne!(facts.kernel, RecordingStatus::Recorded);
            }
            assert!(!facts.inventory.as_ref().unwrap().entries.is_empty());
            assert!(facts.resources.as_ref().unwrap().reserved > 0);
            let other = Fixture::new();
            assert!(
                other._session.operation_observation(&request).is_err(),
                "foreign request cannot select observations"
            );
        }
    }
}

#[test]
fn session_result_retains_each_entered_native_call_separately() {
    const PAGE: &str = include_str!("../../../../kernel/src/inert_page/fixtures/reference.html");
    fn calls(f: &Fixture, request: &crate::host_session::SessionRequest, expected: &[&str]) {
        let facts = f._session.operation_observation(request).unwrap();
        let actual: std::collections::BTreeMap<_, _> = facts
            .calls
            .iter()
            .map(|c| (c.call, (c.entered, c.returned, c.failed)))
            .collect();
        let expected: std::collections::BTreeMap<_, _> =
            expected.iter().map(|name| (*name, (1, 1, 0))).collect();
        assert_eq!(
            actual, expected,
            "each returned real native call has its own retained fact"
        );
    }
    for page in [false, true] {
        let f = Fixture::new();
        let name = if page { "index.html" } else { "session.txt" };
        let bytes = if page { PAGE } else { "native result oracle\n" };
        f.propose_content(name, bytes).unwrap();
        let mut preparation = vec![
            "read_grant",
            "run_construct",
            "staging_grant",
            "snapshot",
            "edit",
            "structural_verify",
            "review",
        ];
        if page {
            preparation.push("page_verify");
        }
        calls(&f, &f.request, &preparation);
        let approval = f
            ._session
            .ingress()
            .accept(CommandKind::Approve, 2)
            .unwrap();
        f._session.approve_file(&approval).unwrap();
        let mut execution = vec![
            "revalidation_review",
            "request_approval",
            "write_grant",
            "apply",
        ];
        if page {
            execution.push("page_recheck");
        }
        calls(&f, &approval, &execution);
        assert_eq!(
            std::fs::read(f._dirs.0.path().join("selected/workspace").join(name)).unwrap(),
            bytes.as_bytes()
        );
        let undo = f._session.ingress().accept(CommandKind::Undo, 3).unwrap();
        f._session.undo(&undo).unwrap();
        calls(&f, &undo, &["request_restore", "undo_grant", "restore"]);
        assert!(!f
            ._dirs
            .0
            .path()
            .join("selected/workspace")
            .join(name)
            .exists());
        assert_eq!(
            matching(&events(&f.ledger_path()), "restore.completed", None),
            1
        );
    }
    let f = Fixture::new();
    f.pending();
    let recorded = f.door.cancel_run(f.run).unwrap().write();
    assert_eq!((recorded.owed, recorded.uncertain), (0, 0));
    f.gov.settle();
    let cleanup = f
        ._session
        .ingress()
        .accept(CommandKind::Cleanup, 2)
        .unwrap();
    f._session.cleanup(&cleanup).unwrap();
    calls(&f, &cleanup, &["discard_staging", "finalization_sync"]);
    assert!(resources(&f.gov.files().state_dir_for_test().join("coding-runs")).is_empty());
}

/// Actual owed restore temporary removal, separated from staging discard.
#[test]
fn session_retry_cleanup_handoff_and_successor_are_independently_admitted() {
    const NAME: &str = "governed::session_routes::session_retry_cleanup_handoff_and_successor_are_independently_admitted";
    let Ok(stop_at) = std::env::var("M2DD_RETRY_CASE") else {
        let root = temp_root("session-retry-preload");
        let lib = root.0.path().join("faults.so");
        let build = std::process::Command::new("cc")
            .args([
                "-shared", "-fPIC", "-std=c11", "-Wall", "-Wextra", "-Werror", "-o",
            ])
            .arg(&lib)
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../mother/host/tests/support/faults.c"),
            )
            .args(["-ldl"])
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        for point in [
            "none",
            "session.before.retry_cleanup",
            "session.after.retry_cleanup",
        ] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME, "--nocapture"])
                .env("LD_PRELOAD", &lib)
                .env("M2DD_FAULT", "restore-owed")
                .env("M2DD_RETRY_CASE", point)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while child.try_wait().unwrap().is_none() {
                if std::time::Instant::now() >= until {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("bounded retry child: {point}");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{point}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        return;
    };
    let f = Fixture::new();
    let id = f.pending();
    f.approve(id).unwrap();
    let workspace = f._dirs.0.path().join("selected/workspace");
    assert!(f.undo().is_err());
    let before = resources(&workspace);
    assert!(
        !before.is_empty(),
        "failed native restore retains actual owned temporary"
    );
    assert!(!workspace.join("session.txt").exists());
    let entered = Arc::new(AtomicUsize::new(0));
    let seen = entered.clone();
    f.gov
        .files()
        .hooks()
        .arm("session.entered.retry_cleanup", move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
    let discards = Arc::new(AtomicUsize::new(0));
    let seen = discards.clone();
    f.gov
        .files()
        .hooks()
        .arm("session.entered.discard_staging", move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
    if stop_at != "none" {
        let ingress = f._session.ingress();
        let point = if stop_at == "session.before.retry_cleanup" {
            "session.before.retry_cleanup"
        } else {
            "session.after.retry_cleanup"
        };
        f.gov.files().hooks().arm(point, move || {
            ingress.emergency().unwrap();
        });
    }
    // Close retires the old undo index; only its already-owned temporary is
    // eligible. It cannot call restore or remove a published Owner path.
    let quit = f._session.ingress().accept(CommandKind::Quit, 10).unwrap();
    let _result = f._session.settle_close_resources(&quit);
    let expected_retry = usize::from(stop_at != "session.before.retry_cleanup");
    assert_eq!(entered.load(Ordering::SeqCst), expected_retry, "{stop_at}");
    assert_eq!(
        discards.load(Ordering::SeqCst),
        usize::from(stop_at == "none"),
        "{stop_at}: retry permission cannot authorize discard"
    );
    let after = resources(&workspace);
    if expected_retry == 0 {
        assert_eq!(before, after);
    } else {
        assert!(after.is_empty());
    }
    assert_eq!(
        matching(&events(&f.ledger_path()), "restore.completed", None),
        expected_retry
    );
    println!(
        "RETRY/{stop_at}: native_workers=0 retry={expected_retry} remaining_temporaries={}",
        after.len()
    );
    let staging = f.gov.files().state_dir_for_test().join("coding-runs");
    let staged = resources(&staging);
    drop(quit);
    let Fixture {
        gov,
        door,
        _session,
        _root,
        _dirs,
        ..
    } = f;
    drop(door);
    drop(gov);
    drop(_session);
    assert_eq!(
        resources(&workspace),
        after,
        "last-owner drop cannot destroy an owed OwnedTemp"
    );
    assert_eq!(
        resources(&staging),
        staged,
        "last-owner drop cannot discard the retained staging lease"
    );
    drop((_root, _dirs));
}

/// Count actual mutation/callback completions, even if a subsequent check
/// refuses the overall operation. Original ingress runs no native Stop worker.
#[test]
fn session_registry_each_final_mutation_has_an_independent_admission() {
    let selected = std::env::var("M2DD_REGISTRY_FINAL").ok();
    let mut failures = Vec::new();
    let mut executed = 0;
    for (route, before, after, ordinal, approving) in [
        (
            "pending_reserve",
            "registry.pending_reserve.before",
            "registry.pending_reserve.after",
            1,
            false,
        ),
        (
            "undo_confirm",
            "registry.undo_confirm.before",
            "registry.undo_confirm.after",
            1,
            false,
        ),
        (
            "action_reserve",
            "registry.action_reserve.before",
            "registry.action_reserve.after",
            1,
            false,
        ),
        (
            "preparation_reserve",
            "registry.preparation_reserve.before",
            "registry.preparation_reserve.after",
            1,
            false,
        ),
        (
            "preparation_validate",
            "registry.preparation_validate.before",
            "registry.preparation_validate.after",
            1,
            false,
        ),
        (
            "preparation_retention",
            "registry.retention.before",
            "registry.retention.after",
            1,
            false,
        ),
        (
            "resource_reserve",
            "registry.resource_reserve.before",
            "registry.resource_reserve.after",
            1,
            false,
        ),
        (
            "commitment_create",
            "registry.retention.before",
            "registry.retention.after",
            2,
            false,
        ),
        (
            "pending_install",
            "registry.pending_install.before",
            "registry.pending_install.after",
            1,
            false,
        ),
        (
            "action_confirm",
            "registry.action_confirm.before",
            "registry.action_confirm.after",
            1,
            true,
        ),
        (
            "token_issue",
            "registry.token_issue.before",
            "registry.token_issue.after",
            1,
            true,
        ),
        (
            "authorize",
            "registry.authorize.before",
            "registry.authorize.after",
            1,
            true,
        ),
        (
            "commitment_start",
            "registry.commitment_start.before",
            "registry.commitment_start.after",
            1,
            true,
        ),
    ] {
        for ingresses in 0..=3 {
            let key = format!("{route}/{ingresses}");
            if selected.as_ref().is_some_and(|value| value != &key) {
                continue;
            }
            let f = Fixture::new();
            let id = approving.then(|| f.pending());
            if route == "undo_confirm" {
                let id = f.pending();
                f.approve(id).unwrap();
            }
            let before_count = Arc::new(AtomicUsize::new(0));
            let after_count = Arc::new(AtomicUsize::new(0));
            let cell = f.gov.file_session_halt();
            let stop = cell.clone();
            let called = before_count.clone();
            cell.1.every(before, move || {
                if called.fetch_add(1, Ordering::SeqCst) + 1 == ordinal {
                    for _ in 0..ingresses {
                        stop.engage_emergency();
                    }
                }
            });
            let entered = after_count.clone();
            cell.1.every(after, move || {
                entered.fetch_add(1, Ordering::SeqCst);
            });
            let success = if route == "undo_confirm" {
                f.undo().is_ok()
            } else if let Some(id) = id {
                f.approve(id).is_ok()
            } else {
                f.propose().is_ok()
            };
            let reached = before_count.load(Ordering::SeqCst) >= ordinal;
            let observed = after_count.load(Ordering::SeqCst);
            let permitted = if ingresses == 0 {
                observed >= ordinal && success
            } else {
                observed == ordinal - 1 && !success
            };
            let conforms = reached && permitted;
            assert!(!f.gov.stop_snapshot().broker_stopped);
            assert!(!f.gov.stop_snapshot().kernel_stopped);
            println!("REGISTRY-FINAL/{key}: actual_mutations={observed} boundary={ordinal} native_workers=0 conforms={conforms}");
            if !conforms {
                failures.push(key);
            }
            executed += 1;
        }
    }
    assert_eq!(executed, if selected.is_some() { 1 } else { 52 });
    assert!(
        failures.is_empty(),
        "{executed} actual registry schedules: {failures:?}"
    );
}

#[test]
fn session_reservation_transfer_releases_exactly_its_own_capacity() {
    for transfer in [false, true] {
        let f = Fixture::new();
        f.gov.authority().commitments().set_capacity_for_test(2);
        let a = f.gov.control.reserve_host_proposal(f.run).unwrap();
        let b = f.gov.control.reserve_host_proposal(f.run).unwrap();
        assert_eq!(f.gov.control.pending_places(), 2);
        assert!(f.gov.control.reserve_host_proposal(f.run).is_err());
        assert_eq!(
            f.gov.control.pending_places(),
            2,
            "failed compound reservation returns its own Place"
        );
        if transfer {
            let cell = f
                .gov
                .authority()
                .runs()
                .session_cell(f.run)
                .unwrap()
                .unwrap();
            let preparation = f
                .gov
                .files()
                .prepare_session(
                    &f.gov.control,
                    f.run,
                    &FileCreate {
                        name: "transfer.txt".into(),
                        content: "retained transfer\n".into(),
                    },
                    f.gov.kernel_stop.authority(),
                    &cell,
                )
                .unwrap();
            let view = f
                .gov
                .control
                .propose_reserved(f.door.agent(), f.run, preparation, a)
                .unwrap();
            assert!(f.gov.authority().commitments().pinned_for_test(view.id));
            assert_eq!(f.gov.control.pending_held(f.run), 1);
        } else {
            drop(a);
        }
        assert_eq!(
            f.gov.control.pending_places(),
            1,
            "transfer/drop must not return B's reservation"
        );
        drop(b);
        assert_eq!(f.gov.control.pending_places(), 0);
        let c = f
            .gov
            .control
            .reserve_host_proposal(f.run)
            .expect("released preparation capacity is really reusable");
        assert_eq!(f.gov.control.pending_places(), 1);
        drop(c);
        assert_eq!(f.gov.control.pending_places(), 0);
        println!(
            "RESERVATION-OWNERSHIP/transfer={transfer}: native_pending={} reusable=true",
            f.gov.control.pending_held(f.run)
        );
    }
}

#[path = "session_native_record_fix1_tests.rs"]
mod native_record_fix1;
#[path = "session_lifecycle_fix1_tests.rs"]
mod lifecycle_fix1;
