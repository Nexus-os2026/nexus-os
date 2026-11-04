//! Review 5: every engagement ingress crossed with all four healthy touches.
//! A fresh real broker per child; channels force the post-engagement schedule.
use super::*;
use crate::adapter::lite_clock::{past_the_start, LiteHome};
use crate::adapter::m2c_support::port_for;
use crate::adapter::{create_private, mother_agent, GovernedPort};
use crate::evidence::{BrokerClass, PublicationCause};
use crate::journal::MemoryJournal;
use crate::oracles::lite_support::{files_lanes, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::governed::GrantRequest;
use std::sync::mpsc;
use std::time::Duration;
const CASE: &str = "NEXUS_LITE_REVIEW5";
const TEST: &str = "coordinator::lite::review5::lite_every_engagement_competing_touch";
struct Fixture {
    home: LiteHome,
    port: Arc<GovernedPort>,
    journal: MemoryJournal,
    clock: Arc<ManualClock>,
    folder: String,
    owner: FilesOwner,
}
impl Fixture {
    fn new() -> Self {
        assert!(nexus_governed_control::files::make_non_dumpable());
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home =
            LiteHome::open(&std::env::temp_dir().join(format!("nexus-lite-review5-{unique}")))
                .unwrap();
        let workspace = home.dir.join("workspace");
        create_private(&workspace).unwrap();
        let info = home.control.register_workspace(&workspace, &[]).unwrap();
        let owner = FilesOwner::default();
        home.control
            .request_grant(
                &GrantRequest::Files { workspace: info.id },
                Duration::from_secs(86400),
                &owner,
            )
            .unwrap();
        Self {
            port: Arc::new(port_for(&home.control, true).with_workspace(info.name.clone())),
            home,
            journal: MemoryJournal::default(),
            clock: Arc::new(ManualClock::default()),
            folder: info.name,
            owner,
        }
    }
    fn open(&self) -> Arc<FileSession<GovernedPort>> {
        Arc::new(
            Coordinator::open(
                self.journal.clone(),
                self.port.clone(),
                Lanes::new(files_lanes()).unwrap(),
                self.clock.clone(),
                self.home.private(),
            )
            .unwrap()
            .into_file_session()
            .unwrap(),
        )
    }
    fn approved(&self, m: &FileSession<GovernedPort>, markers: &str, calls: u32) -> MissionId {
        let mut contract = contract_with(markers);
        contract.requested_folder = Some(self.folder.clone());
        contract.budget.max_model_calls = calls;
        let id = m.draft(contract).unwrap();
        m.submit(id).unwrap();
        assert_eq!(
            answering(&Yes, || m.request_consent(id)),
            Ok(MissionState::Approved)
        );
        id
    }
    fn wait(&self, m: &FileSession<GovernedPort>, markers: &str) -> MissionId {
        let id = self.approved(m, markers, 50);
        for _ in 0..3 {
            m.advance(id).unwrap();
        }
        assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
        id
    }
}

const SITES: &[&str] = &[
    "unknown-cause",
    "denied",
    "expired",
    "cancelled",
    "published-interrupted",
    "unknown",
    "deadline-empty",
    "deadline-published",
    "deadline-uncertain",
    "public-stop",
    "public-stop-all",
    "budget",
    "callback-panic",
    "port-fence",
    "port-poison",
    "lock-poison",
    "owed-failure",
    "journal-failure",
    "non-file-terminal",
    "emergency",
    "replacement-port",
    "replacement-attachment",
];
const TOUCHES: &[&str] = &["state", "advance", "stop", "close"];

fn records(lines: &[String], id: MissionId) -> (Vec<SettleCause>, Vec<Trigger>) {
    let mut missions = BTreeMap::new();
    let mut next = 1;
    let mut session = Session::Never;
    let mut ends = Vec::new();
    let mut causes = Vec::new();
    for entry in verify(lines).unwrap() {
        if entry.mission == id {
            if let Event::SettlementStarted { cause, .. } = entry.event {
                causes.push(cause);
            }
            if let Event::Transition { trigger } = &entry.event {
                let from = missions
                    .get(&id)
                    .map(|m: &Mission| m.state)
                    .unwrap_or(MissionState::Draft);
                if transition(from, trigger).unwrap().is_terminal() {
                    ends.push(trigger.clone());
                }
            }
        }
        check(&entry.event).unwrap();
        replay(&mut missions, &mut next, &mut session, &entry).unwrap();
    }
    (causes, ends)
}

#[test]
fn lite_every_engagement_competing_touch() {
    let Ok(case) = std::env::var(CASE) else {
        let sites = SITES.iter().map(|s| s.to_string()).chain(
            SettleCause::every()
                .iter()
                .map(|c| format!("file-stop:{}", c.as_str()))
                .collect::<Vec<_>>(),
        );
        let mut failures = Vec::new();
        let mut count = 0;
        for site in sites {
            for touch in TOUCHES {
                let case = format!("{site}/{touch}");
                let out = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", TEST, "--nocapture"])
                    .env(CASE, &case)
                    .output()
                    .unwrap();
                let stdout = String::from_utf8_lossy(&out.stdout);
                if !out.status.success() || !stdout.contains(&format!("review5:{case}")) {
                    failures.push(format!(
                        "{case}:\n{stdout}\n{}",
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
                count += 1;
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        println!("review5-matrix:{count} cases");
        return;
    };
    run_case(&case);
}

fn run_case(case: &str) {
    let (site, touch) = case.rsplit_once('/').unwrap();
    let f = Fixture::new();
    let m = f.open();
    let deadline = site.starts_with("deadline-");
    let ordinary = matches!(
        site,
        "denied" | "expired" | "cancelled" | "published-interrupted" | "unknown"
    ) || deadline;
    let prefile = matches!(
        site,
        "callback-panic"
            | "unknown-cause"
            | "port-poison"
            | "lock-poison"
            | "owed-failure"
            | "journal-failure"
            | "non-file-terminal"
    );
    if deadline {
        let clock = f.home.clock.clone();
        f.port.in_broker_action.arm(0, move || {
            let reads = std::sync::atomic::AtomicUsize::new(0);
            let advance = clock.clone();
            clock.arm(
                move || reads.fetch_add(1, Ordering::SeqCst) == 1,
                move || advance.advance(Duration::from_secs(30)),
            );
        });
    }
    let id = if prefile {
        f.approved(&m, "engagement without File dispatch", 50)
    } else {
        f.wait(&m, "MKFILE:a.txt MKFILE:b.txt")
    };
    let pending = f.port.pending(id);
    let mut execution = None;
    let mut release_execution = None;
    let mut expected_cause = site
        .strip_prefix("file-stop:")
        .map(|code| SettleCause::parse(code).unwrap());
    let mut expected_end = None;
    if let Some(pending) = &pending {
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = CommitmentId::parse(&pending.commitment).unwrap();
        match site {
            "denied" => {
                assert!(f
                    .home
                    .control
                    .authorize(commitment, &mother_agent(), run, &crate::oracles::No)
                    .is_err());
            }
            "expired" => {
                f.home.clock.advance(Duration::from_secs(3600));
            }
            "cancelled" => {
                f.home.control.cancel_run(run).unwrap();
            }
            "published-interrupted" | "deadline-published" | "deadline-uncertain" => {
                f.home
                    .control
                    .authorize(commitment, &mother_agent(), run, &f.owner)
                    .unwrap();
                if site == "published-interrupted" {
                    let control = f.home.control.clone();
                    f.home.clock.arm(past_the_start(&f.home, run), move || {
                        control.cancel_run(run).unwrap()
                    });
                }
                if site == "deadline-uncertain" {
                    let (tx, rx) = mpsc::sync_channel(1);
                    let (release_tx, release_rx) = mpsc::sync_channel(1);
                    f.home.clock.arm(past_the_start(&f.home, run), move || {
                        tx.send(()).unwrap();
                        release_rx.recv_timeout(Duration::from_secs(30)).unwrap();
                    });
                    let control = f.home.control.clone();
                    execution = Some(std::thread::spawn(move || {
                        control.execute(commitment, &mother_agent(), run)
                    }));
                    rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    release_execution = Some(release_tx);
                } else {
                    f.home
                        .control
                        .execute(commitment, &mother_agent(), run)
                        .unwrap();
                }
            }
            "unknown" => {
                f.port.review5_forget_wait(id);
            }
            _ => {}
        }
        if deadline {
            use nexus_governed_control::authority::clock::Clock;
            let left = pending
                .deadline_mono_ms
                .saturating_sub(f.home.clock.monotonic_ms());
            f.home.clock.advance(Duration::from_millis(left + 1));
        }
        if ordinary {
            expected_end = Some(match site {
                "denied" => Trigger::ActionDenied { effect: 0 },
                "expired" | "deadline-empty" => Trigger::ApprovalLapsed {
                    effect: 0,
                    reason: LapseReason::ApprovalExpired,
                },
                "cancelled" => Trigger::ApprovalLapsed {
                    effect: 0,
                    reason: LapseReason::ApprovalLapsed(BrokerClass::Cancelled),
                },
                _ => {
                    let read = if matches!(site, "unknown" | "deadline-uncertain") {
                        SettlementOutcome::Uncertain
                    } else if site == "deadline-published" {
                        SettlementOutcome::Published {
                            recovery_required: false,
                            interrupted: vec![],
                        }
                    } else {
                        SettlementOutcome::Published {
                            recovery_required: false,
                            interrupted: vec![PublicationCause::Cancelled],
                        }
                    };
                    let end = read.ordinary_end().or_else(|| read.deadline_end()).unwrap();
                    Trigger::Settled {
                        effect: 0,
                        action: Some(pending.clone()),
                        outcome: read,
                        end,
                    }
                }
            });
        }
    }
    let hook_site = if site.starts_with("file-stop:") {
        "file-stop"
    } else if deadline || site == "expired" {
        "deadline"
    } else {
        match site {
            "denied" | "cancelled" => "nonpublication",
            "published-interrupted" | "unknown" => "publication",
            "journal-failure" => "port-fence",
            other => other,
        }
    };
    // The broker-only ingresses have their immediate atomic boundary tested in
    // the broker matrix too; this clock read precedes any Mother mission lock.
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let hold = move || {
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(30)).unwrap();
    };
    if matches!(
        site,
        "emergency" | "replacement-port" | "replacement-attachment"
    ) {
        // No broker call takes a Mother lock: return is still before its first touch.
        m.hooks_for_test().file_after_halt.arm(0, hold);
    } else if hook_site.starts_with("port-") {
        f.port
            .file_engagement
            .arm(hook_site_static(hook_site), hold);
    } else {
        m.hooks_for_test()
            .file_engagement
            .arm(hook_site_static(hook_site), hold);
    }
    if site == "budget" {
        f.clock.advance(Duration::from_secs(86400));
    }
    let worker = {
        let m = m.clone();
        let port = f.port.clone();
        let control = f.home.control.clone();
        let site = site.to_string();
        let journal = f.journal.clone();
        let gate = f.home.gate.clone();
        std::thread::spawn(move || {
            match site.as_str() {
                "unknown-cause" => {
                    m.coordinator
                        .hooks
                        .file_missing_cause
                        .store(true, Ordering::SeqCst);
                    assert!(m
                        .coordinator
                        .file_stop(id, SettleCause::Ended(BlockReason::BudgetModelCalls))
                        .is_err());
                }
                "public-stop" => {
                    let _ = m.stop(id);
                }
                "public-stop-all" => {
                    let _ = m.stop_all();
                }
                "callback-panic" => {
                    let _: Result<(), _> = m.call(|_| panic!("external callback"));
                }
                "port-fence" => {
                    let _ = port.fence().write_checked();
                }
                "port-poison" => {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        port.review5_poison();
                    }));
                    let _ = port.file_publication(id);
                }
                "lock-poison" => {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        m.poison_for_test()
                    }));
                    let _ = m.coordinator.try_with_lock(|_| Ok(()));
                }
                "owed-failure" => {
                    // A typed failure at the actual Owed write boundary.
                    m.coordinator.file_admit(id).unwrap();
                    let run = port.run_of(id).unwrap();
                    gate.arm_with("run_cancelled", Some(run), || panic!("owed write failure"));
                    let batch = port.review5_cancel(run);
                    m.coordinator.inner.lock().unwrap().owed.push(batch);
                    let _ = m.coordinator.try_with_lock(|_| Ok(()));
                }
                "journal-failure" => {
                    journal.fail_after(0);
                    let _ = m.coordinator.try_with_lock(|inner| {
                        inner.append(
                            id,
                            Event::SettlementStarted {
                                effect: 0,
                                cause: SettleCause::Stop(StopReason::Owner),
                            },
                        )
                    });
                }
                "non-file-terminal" => {
                    let _ = m.coordinator.end_non_file_with(
                        id,
                        Trigger::Block {
                            reason: BlockReason::WaitUnknown,
                        },
                    );
                }
                "emergency" => {
                    control.emergency_stop();
                    let _ = m.state(id);
                }
                "replacement-port" => {
                    port.review5_reopen();
                    let _ = m.state(id);
                }
                "replacement-attachment" => {
                    let _ = port.file_attach();
                    let _ = m.state(id);
                }
                _ if site.starts_with("file-stop:") => {
                    let cause =
                        SettleCause::parse(site.strip_prefix("file-stop:").unwrap()).unwrap();
                    let _ = m.coordinator.file_stop(id, cause);
                }
                _ => {
                    let _ = m.advance(id);
                }
            }
        })
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        m.cell_for_test().is_engaged(),
        "{case}: exact engagement reached"
    );
    let tagged = m.cell_for_test().reason().unwrap().1.is_some();
    // Poison cannot journal while poisoned. A healthy follow-up uses the same
    // original cell after test-only repair of the poisoned fixture mutex.
    if site == "lock-poison" {
        m.coordinator.inner.clear_poison();
    }
    if site == "port-poison" {
        f.port.review5_clear_poison();
    }
    let broken = site == "journal-failure";
    let (touch_tx, touch_rx) = mpsc::sync_channel(1);
    let competing = {
        let m = m.clone();
        let touch = touch.to_owned();
        std::thread::spawn(move || {
            let result = match touch.as_str() {
                "state" => m.state(id).map(|_| ()),
                "advance" => m.advance(id).map(|_| ()),
                "stop" => m.stop(id).map(|_| ()),
                "close" => m.close(),
                _ => panic!("touch"),
            };
            touch_tx.send(result).unwrap();
        })
    };
    // Journal failure is already inside the mission lock: that touch is
    // allowed to finish only after release, and must report the journal error.
    let touched = if broken {
        None
    } else {
        touch_rx.recv_timeout(Duration::from_secs(2)).ok()
    };
    let at_touch = f.journal.lines();
    release_tx.send(()).unwrap();
    worker.join().unwrap();
    let touched = touched.unwrap_or_else(|| touch_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    competing.join().unwrap();
    if let Some(tx) = release_execution {
        tx.send(()).unwrap();
    }
    if let Some(worker) = execution {
        let _ = worker.join().unwrap();
    }
    if broken {
        assert!(
            touched.is_err(),
            "failed journal cannot claim a recorded terminal"
        );
        assert!(records(&at_touch, id).1.is_empty());
    } else {
        if !ordinary && expected_cause.is_none() {
            expected_cause = Some(match site {
                "public-stop" | "public-stop-all" => SettleCause::Stop(StopReason::Owner),
                "budget" => SettleCause::TimeBudget,
                "emergency" => SettleCause::Stop(StopReason::EmergencyStop),
                "non-file-terminal" => SettleCause::Ended(BlockReason::WaitUnknown),
                _ => SettleCause::Stop(StopReason::DoorHalted),
            });
        }
        let (causes, ends) = records(&at_touch, id);
        if let Some(cause) = expected_cause {
            if prefile {
                expected_end = Some(match cause {
                    SettleCause::Stop(reason) => Trigger::Stop { reason },
                    SettleCause::Ended(reason) => Trigger::Block { reason },
                    SettleCause::TimeBudget => Trigger::Block {
                        reason: BlockReason::BudgetTime,
                    },
                });
                assert!(causes.is_empty());
            } else {
                assert_eq!(causes, vec![cause], "{case}: exact intended cause");
                expected_end = Some(Trigger::Settled {
                    effect: 0,
                    action: pending.clone(),
                    outcome: SettlementOutcome::NotPublished,
                    end: cause.end(&SettlementOutcome::NotPublished),
                });
            }
        } else {
            assert!(
                causes.is_empty(),
                "{case}: ordinary terminal must not become Stop"
            );
        }
        assert_eq!(
            ends,
            vec![expected_end.unwrap()],
            "{case}: exact intended terminal"
        );
        assert_eq!(
            records(&f.journal.lines(), id),
            (causes, ends),
            "no replacement terminal on release"
        );
        if touch == "state" {
            assert!(touched.is_ok());
        }
    }
    assert!(
        tagged,
        "{case}: engagement must carry an explicit exact tag"
    );
    assert!(!f.home.dir.join("workspace/b.txt").exists());
    println!("review5:{case}");
}

fn hook_site_static(site: &str) -> &'static str {
    [
        "file-stop",
        "unknown-cause",
        "deadline",
        "nonpublication",
        "publication",
        "public-stop",
        "public-stop-all",
        "budget",
        "callback-panic",
        "port-fence",
        "port-poison",
        "lock-poison",
        "owed-failure",
        "non-file-terminal",
    ]
    .into_iter()
    .find(|s| *s == site)
    .unwrap()
}

#[test]
fn lite_no_untagged_engagement_source_guard() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let mut files = std::collections::BTreeSet::new();
    production_modules(&root.join("mother/src/lib.rs"), &mut files);
    production_modules(
        &root.join("crates/nexus-governed-control/src/lib.rs"),
        &mut files,
    );
    assert!(files.len() > 50, "production module walk is not vacuous");
    let mut bad = Vec::new();
    let mut kernel_calls = 0;
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        if file.ends_with("authority/file_session.rs") {
            assert_eq!(
                source.matches(".fetch_update(").count(),
                1,
                "one tagged atomic latch, including emergency"
            );
            for raw_write in [".store(", ".swap(", ".compare_exchange(", ".fetch_or("] {
                assert!(
                    !source.contains(raw_write),
                    "no alternate untagged atomic writer: {raw_write}"
                );
            }
        }
        for (line, text) in source.lines().enumerate() {
            let text = text.trim();
            // These are kernel StopController calls, not File HaltCells. The
            // exact three existing calls are retained; no broad file exemption.
            if file.ends_with("src/governed.rs")
                && matches!(text, "controller.engage();" | "self.kernel_stop.engage();")
            {
                kernel_calls += 1;
                continue;
            }
            if text.contains(".engage_with(")
                || text.contains(".engage_reason(")
                || text.contains(".engage()")
            {
                bad.push(format!("{}:{}:{}", file.display(), line + 1, text));
            }
        }
    }
    assert_eq!(kernel_calls, 3);
    assert!(
        bad.is_empty(),
        "untagged File engagement calls:\n{}",
        bad.join("\n")
    );
}

// Walk production module declarations, including platform-gated modules. A
// new module is covered automatically. Test-only external modules are excluded
// because frozen oracles intentionally exercise the compatibility shorthands.
fn production_modules(
    file: &std::path::Path,
    seen: &mut std::collections::BTreeSet<std::path::PathBuf>,
) {
    let file = file.canonicalize().unwrap();
    if !seen.insert(file.clone()) {
        return;
    }
    let source = std::fs::read_to_string(&file).unwrap();
    let parent = file.parent().unwrap();
    let directory = if matches!(
        file.file_name().unwrap().to_str().unwrap(),
        "mod.rs" | "lib.rs"
    ) {
        parent.to_owned()
    } else {
        parent.join(file.file_stem().unwrap())
    };
    let mut test_only = false;
    let mut path = None;
    for line in source.lines().map(str::trim) {
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        if line.starts_with("#[") {
            test_only |= line == "#[cfg(test)]" || line.starts_with("#[cfg(all(test,");
            if line.starts_with("#[path = ") {
                path = Some(line.split('"').nth(1).unwrap().to_owned());
            }
            continue;
        }
        let declaration = line
            .strip_prefix("mod ")
            .or_else(|| line.strip_prefix("pub mod "))
            .or_else(|| line.strip_prefix("pub(crate) mod "));
        if let Some(name) = declaration.and_then(|text| text.strip_suffix(';')) {
            if !test_only {
                let child = match path.take() {
                    Some(path) => parent.join(path),
                    None => {
                        let flat = directory.join(format!("{name}.rs"));
                        if flat.is_file() {
                            flat
                        } else {
                            directory.join(name).join("mod.rs")
                        }
                    }
                };
                production_modules(&child, seen);
            }
        }
        test_only = false;
        path = None;
    }
}

// These defensive selection branches precede FileSession creation and have no
// mission to journal. Force only the fault; never bypass File eligibility.
#[test]
fn lite_selection_engagements_are_tagged() {
    const ENV: &str = "NEXUS_LITE_REVIEW5_SELECTION";
    let Ok(site) = std::env::var(ENV) else {
        for site in ["selection-poison", "port-claim"] {
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "coordinator::lite::review5::lite_selection_engagements_are_tagged",
                    "--nocapture",
                ])
                .env(ENV, site)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{site}: {}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        return;
    };
    let f = Fixture::new();
    let coordinator = Coordinator::open(
        f.journal.clone(),
        f.port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        f.clock.clone(),
        f.home.private(),
    )
    .unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let hold = move || {
        tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    };
    if site == "selection-poison" {
        coordinator
            .hooks
            .file_selection_poison
            .store(true, Ordering::SeqCst);
        coordinator
            .hooks
            .file_engagement
            .arm("selection-poison", hold);
    } else {
        f.port.file_duplicate_claim.store(true, Ordering::SeqCst);
        f.port.file_engagement.arm("port-claim", hold);
    }
    let worker = std::thread::spawn(move || coordinator.into_file_session().err().unwrap());
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let cell = f.port.review5_cell();
    let first = cell.reason();
    cell.engage();
    release_tx.send(()).unwrap();
    let error = worker.join().unwrap();
    assert_eq!(
        error,
        if site == "selection-poison" {
            CoordinatorError::Poisoned
        } else {
            CoordinatorError::FileSession(FileSessionError::AlreadySelected)
        }
    );
    assert_eq!(
        first,
        Some((HaltCause::Fence, std::num::NonZeroU16::new(u16::MAX - 3)))
    );
    assert_eq!(cell.reason(), first);
    assert_eq!(
        records(&f.journal.lines(), MissionId(1)),
        (vec![], vec![]),
        "no invented mission or terminal before selection"
    );
}
