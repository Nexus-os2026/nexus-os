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
    door: Arc<SessionDoor>,
    _session: Arc<HostSession>,
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
            HostSession::open(seed, root.0.path(), evidence, clock.clone(), Arc::new(Yes)).unwrap(),
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
            gov,
            door: Arc::new(door),
            _session: session,
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
