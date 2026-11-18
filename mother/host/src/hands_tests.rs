//! Review 1: deterministic production-path schedules, test-only child hooks.
use crate::hands::run;
use crate::session_home::SessionServices;
use crate::stop::StopRoute;
use crate::stop_tests::support::{Fixture, Pty};
use crate::terminal::Shared;
use nexus_governed_control::authority::evidence::EvidencePhase;
use nexus_governed_control::authority::evidence::{EvidenceProjection, EvidenceRecord};
use nexus_governed_control::files::FileCreate;
use nexus_governed_control::host_session::SessionSeed;
use nexus_mother::adapter::SessionPort;
use nexus_mother::effect::{EffectPort, EffectRequest};
use nexus_mother::journal::MissionId;
use nexus_mother::lane::LaneRole;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const CHILD: &str = "M2DD_REVIEW1_CHILD";
static ENGAGED: AtomicBool = AtomicBool::new(false);

pub(crate) fn before_internal_admission(screen: &Shared, home: &SessionServices) {
    if std::env::var(CHILD).as_deref() == Ok("admission") && !ENGAGED.swap(true, Ordering::SeqCst) {
        assert!(!screen.stop.ingress.emergency_seen());
        // The outer Mission check has succeeded. Perform exactly the first
        // memory-only step of Stop ingress; its worker has not been scheduled.
        screen.stop.hold_dispatch.store(true, Ordering::SeqCst);
        screen.stop.press();
        assert!(!home.session.stop_snapshot().broker_stopped);
        assert_eq!(screen.stop.retained_handles(), 0);
    }
}

pub(crate) fn before_internal_dispatch(route: &StopRoute, role: LaneRole) {
    if std::env::var(CHILD).as_deref() == Ok("dispatch") && role == LaneRole::Builder {
        // Lane was admitted and returned its request, but the coordinator has
        // not handed that request to the real port yet.
        route.hold_dispatch.store(true, Ordering::SeqCst);
        route.press();
        ENGAGED.store(true, Ordering::SeqCst);
    }
}

pub(crate) fn after_internal_admission(port: &SessionPort, home: &SessionServices, id: MissionId) {
    let scenario = std::env::var(CHILD).unwrap_or_default();
    if scenario != "admission" && !(scenario == "dispatch" && ENGAGED.load(Ordering::SeqCst)) {
        return;
    }
    let runs = home.session.run_count();
    let bindings = port.file_binding_count();
    let dispatches = port.performed();
    let models = home
        .recent_evidence()
        .iter()
        .filter(|e| e.phase == EvidencePhase::ModelCall)
        .count();
    // These are real backend/port observations, not the host's outer-admit counters.
    eprintln!(
        "REAL PORT runs={runs} bindings={bindings} dispatches={dispatches} model_attempts={models}"
    );
    let admitted_before_stop = usize::from(scenario == "dispatch");
    assert_eq!(
        (
            runs - admitted_before_stop,
            bindings - admitted_before_stop,
            dispatches,
            models
        ),
        (0, 0, 0, 0),
        "Stop between outer check and internal File admission"
    );
    // A retained port must also refuse direct effect dispatch in recovery.
    let result = port.perform_file(
        id,
        1,
        &EffectRequest::CreateFile(FileCreate {
            name: "late.txt".into(),
            content: "late".into(),
        }),
    );
    assert!(
        result.is_err(),
        "retained port must refuse recovery dispatch"
    );
    assert_eq!(port.performed(), 0, "zero real effect dispatches");
    eprintln!("INTERNAL ADMISSION REFUSED WITH ZERO REAL COUNTERS");
}

struct PanicRecording;
impl EvidenceProjection for PanicRecording {
    fn project(&self, record: &EvidenceRecord) {
        if record.phase == EvidencePhase::EmergencyStop {
            panic!("review1 backend recording panic");
        }
    }
}

pub(crate) fn open_home(
    seed: SessionSeed,
    path: &std::path::Path,
    confirmer: crate::Confirmer,
) -> Result<SessionServices, crate::HomeError> {
    if std::env::var(M2ED).is_ok() {
        // M2e-D: the real session, stashed for child-side reads, and a
        // phase-only projection (records committed by the store).
        let opened = SessionServices::open_projected(seed, path, confirmer, vec![Arc::new(Phases)]);
        if let Ok(home) = &opened {
            let _ = SESSION.set(home.session.clone());
        }
        return opened;
    }
    if std::env::var(CHILD).as_deref() != Ok("panic") {
        SessionServices::open(seed, path, confirmer)
    } else {
        SessionServices::open_projected(seed, path, confirmer, vec![Arc::new(PanicRecording)])
    }
}

#[test]
fn review1_internal_admission_after_outer_check() {
    if std::env::var(CHILD).as_deref() == Ok("admission") {
        let args = std::env::var("M2DD_REVIEW1_ARGS").unwrap();
        let _clean = run(args.split('\n').map(str::to_owned));
        return;
    }
    let f = Fixture::new();
    let mut p = child(
        &f,
        "admission",
        "review1_internal_admission_after_outer_check",
    );
    p.ready();
    p.line("run");
    p.yes();
    p.yes();
    p.yes();
    p.expect("INTERNAL ADMISSION REFUSED WITH ZERO REAL COUNTERS");
    p.line("quit");
    assert!(p.finish().success());
    f.empty();
}

#[test]
fn review1_backend_recording_panic_is_failed_never_completed() {
    if std::env::var(CHILD).as_deref() == Ok("panic") {
        let args = std::env::var("M2DD_REVIEW1_ARGS").unwrap();
        assert!(
            !run(args.split('\n').map(str::to_owned)),
            "backend panic must keep nonzero host outcome"
        );
        return;
    }
    let f = Fixture::new();
    let mut p = child(
        &f,
        "panic",
        "review1_backend_recording_panic_is_failed_never_completed",
    );
    p.ready();
    p.line("stop");
    p.expect("review1 backend recording panic");
    assert!(
        p.finish().success(),
        "child must assert the nonzero host outcome"
    );
    assert!(
        !p.output.contains("STOP RECORDING COMPLETED"),
        "panicked recording must never be called completed: {}",
        p.output
    );
    assert!(
        p.output.contains("STOP RECORDING FAILED"),
        "explicit recording failure receipt: {}",
        p.output
    );
    f.empty();
}

fn child(f: &Fixture, scenario: &str, test: &str) -> Pty {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &format!("hands_tests::{test}"), "--nocapture"])
        .env(CHILD, scenario)
        .env(
            "M2DD_REVIEW1_ARGS",
            format!(
                "--home\n{}\n--workspace\n{}\n--name\nhello.txt\n--text\noracle",
                f.home.display(),
                f.workspace.display()
            ),
        );
    Pty::start(command)
}

#[test]
fn review1_internal_dispatch_after_lane_reservation() {
    if std::env::var(CHILD).as_deref() == Ok("dispatch") {
        let args = std::env::var("M2DD_REVIEW1_ARGS").unwrap();
        let _clean = run(args.split('\n').map(str::to_owned));
        return;
    }
    let f = Fixture::new();
    let mut p = child(
        &f,
        "dispatch",
        "review1_internal_dispatch_after_lane_reservation",
    );
    p.ready();
    p.line("run");
    p.yes();
    p.yes();
    p.yes();
    p.expect("INTERNAL ADMISSION REFUSED WITH ZERO REAL COUNTERS");
    p.line("quit");
    assert!(p.finish().success());
    f.empty();
}

// Decision 6: Stop uses the actual reader ingress with its worker unscheduled.
const REVIEW2: &str = "M2DD_REVIEW2_CHILD";
pub(crate) fn review2_before_step(screen: &Shared, home: &SessionServices, step: &str) {
    if std::env::var(REVIEW2).as_deref() != Ok(step) {
        return;
    }
    screen.stop.hold_dispatch.store(true, Ordering::SeqCst);
    screen.receive(crate::terminal::Line::Stop, false);
    assert_eq!(screen.stop.receipt().outstanding, 1);
    assert!(!home.session.stop_snapshot().broker_stopped);
    assert!(!home.session.stop_snapshot().kernel_stopped);
}
pub(crate) fn review2_after_step(
    screen: &Shared,
    home: &SessionServices,
    step: &str,
    refused: bool,
) {
    if std::env::var(REVIEW2).as_deref() != Ok(step) {
        return;
    }
    assert!(
        refused,
        "{step}: Stop ingress must refuse mission admission before any worker runs"
    );
    assert!(home.session.run_count() == 0, "zero backend runs");
    assert!(
        !home
            .recent_evidence()
            .iter()
            .any(|r| r.phase == EvidencePhase::ModelCall),
        "zero model calls"
    );
    screen.emit(&format!("REVIEW2 {step} REFUSED; ZERO BACKEND RUNS\n"));
    screen.stop.hold_dispatch.store(false, Ordering::SeqCst);
    screen.stop.dispatch_pending();
}

fn review2_child(f: &Fixture, scenario: &str, test: &str) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &format!("hands_tests::{test}"), "--nocapture"])
        .env(REVIEW2, scenario)
        .env(
            "M2DD_REVIEW2_ARGS",
            format!(
                "--home\n{}\n--workspace\n{}\n--name\nhello.txt\n--text\noracle",
                f.home.display(),
                f.workspace.display()
            ),
        );
    command
}
fn review2_run_child() -> bool {
    if std::env::var(REVIEW2).is_err() {
        return false;
    }
    let args = std::env::var("M2DD_REVIEW2_ARGS").unwrap();
    let _clean = run(args.split('\n').map(str::to_owned));
    true
}
fn review2_mission(scenario: &str, test: &str) {
    if review2_run_child() {
        return;
    }
    let f = Fixture::new();
    let mut p = Pty::start(review2_child(&f, scenario, test));
    p.ready();
    p.line("run");
    p.yes();
    p.yes();
    p.expect(&format!("REVIEW2 {scenario} REFUSED; ZERO BACKEND RUNS"));
    p.expect("STOP RECORDING COMPLETED");
    p.line("quit");
    assert!(
        p.finish().success(),
        "all child assertions passed: {}",
        p.output
    );
    f.empty();
}
#[test]
fn review2_stop_before_draft_worker_unscheduled() {
    review2_mission("draft", "review2_stop_before_draft_worker_unscheduled");
}
#[test]
fn review2_stop_between_draft_and_submit_worker_unscheduled() {
    review2_mission(
        "submit",
        "review2_stop_between_draft_and_submit_worker_unscheduled",
    );
}

// Test-only held startup, with a parent/child readiness handshake and deadline.
// No production environment seam; no sleeps establish any ordering.
pub(crate) fn review2_startup(stage: &str) {
    use std::io::{Read, Write};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixStream};
    let scenario = std::env::var(REVIEW2).unwrap_or_default();
    if scenario != stage && !(stage == "recovery" && scenario == "panic-startup") {
        return;
    }
    let socket = std::env::var("M2DD_REVIEW2_SOCKET").unwrap();
    let mut stream =
        UnixStream::connect_addr(&SocketAddr::from_abstract_name(socket.as_bytes()).unwrap())
            .unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(b"H").unwrap();
    let mut release = [0];
    stream.read_exact(&mut release).unwrap();
    assert_eq!(release, [b'R']);
}
fn review2_startup_receipts(scenario: &str, test: &str) {
    use std::io::{Read, Write};
    use std::os::fd::AsFd;
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener};
    if review2_run_child() {
        return;
    }
    let f = Fixture::new();
    let socket = format!("m2dd.review2.{}.{}", std::process::id(), scenario);
    let listener =
        UnixListener::bind_addr(&SocketAddr::from_abstract_name(socket.as_bytes()).unwrap())
            .unwrap();
    let mut cmd = review2_child(&f, scenario, test);
    cmd.env("M2DD_REVIEW2_SOCKET", socket);
    if scenario == "panic-startup" {
        cmd.env(CHILD, "panic");
    }
    let mut p = Pty::start(cmd);
    let mut fds = [nix::poll::PollFd::new(
        listener.as_fd(),
        nix::poll::PollFlags::POLLIN,
    )];
    assert_eq!(nix::poll::poll(&mut fds, 10000u16).unwrap(), 1);
    let (mut held, _) = listener.accept().unwrap();
    held.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let mut ready = [0];
    held.read_exact(&mut ready).unwrap();
    assert_eq!(ready, [b'H']);
    p.send(b"stop\nstop\n");
    p.expect("STOP REQUEST RECEIVED epoch=1");
    p.expect("STOP REQUEST RECEIVED epoch=2");
    if scenario == "panic-startup" {
        p.expect("STOP RECORDING FAILED");
    } else if scenario == "recovery" {
        // Up to four Stop workers record concurrently, so outcome receipts
        // follow completion order. Both must arrive while recovery is held.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !["count=1 (", "count=2 ("].iter().all(|event| {
            p.output
                .contains(&format!("STOP RECORDING COMPLETED {event}"))
        }) {
            assert!(
                p.read_once(deadline),
                "both Stop outcome receipts while recovery is held: {}",
                p.output
            );
        }
    }
    held.write_all(b"R").unwrap();
    if scenario != "panic-startup" {
        p.ready();
        if scenario == "home" {
            // Completion may precede HANDS READY, so inspect the whole transcript.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !p.output.contains("STOP RECORDING COMPLETED count=2") {
                assert!(p.read_once(deadline));
            }
        }
        p.line("quit");
    }
    assert!(p.finish().success(), "child assertions: {}", p.output);
    assert_eq!(
        p.output.matches("STOP REQUEST RECEIVED epoch=").count(),
        2,
        "one receipt for EVERY ingress"
    );
    let expected = if scenario == "panic-startup" {
        "STOP RECORDING FAILED"
    } else {
        "STOP RECORDING COMPLETED count="
    };
    assert_eq!(
        p.output.matches(expected).count(),
        2,
        "one receipt for EVERY worker outcome: {}",
        p.output
    );
    f.empty();
}
#[test]
fn review2_receipts_while_home_opening_is_held() {
    review2_startup_receipts("home", "review2_receipts_while_home_opening_is_held");
}
#[test]
fn review2_receipts_while_recovery_reporting_is_held() {
    review2_startup_receipts(
        "recovery",
        "review2_receipts_while_recovery_reporting_is_held",
    );
}
#[test]
fn review2_recording_panics_before_startup_finishes_are_receipted() {
    review2_startup_receipts(
        "panic-startup",
        "review2_recording_panics_before_startup_finishes_are_receipted",
    );
}

// Decision 11: inject reader ingress immediately after a real Owner route
// returns. The Stop worker is held until the command has reported its result.
const REVIEW4: &str = "M2DD_REVIEW4_CHILD";
pub(crate) fn review4_returned(screen: &Shared, home: &SessionServices, route: &str) {
    let scenario = std::env::var(REVIEW4).unwrap_or_default();
    if scenario.split('-').next() != Some(route) {
        return;
    }
    screen.stop.hold_dispatch.store(true, Ordering::SeqCst);
    screen.receive(crate::terminal::Line::Stop, false);
    assert!(home.session.ingress().emergency_seen());
    assert_eq!(screen.stop.receipt().outstanding, 1);
}

pub(crate) fn review4_reported(screen: &Shared, home: &SessionServices, route: &str) {
    let scenario = std::env::var(REVIEW4).unwrap_or_default();
    if scenario.split('-').next() != Some(route) {
        return;
    }
    if route == "lift" {
        assert!(
            home.session.stop_snapshot().kernel_stopped,
            "native halt must be re-engaged before the next host action"
        );
    }
    assert!(screen.stop.ingress.emergency_seen());
    assert_ne!(
        home.session.state(),
        nexus_governed_control::host_session::SessionState::Normal
    );
    screen.stop.hold_dispatch.store(false, Ordering::SeqCst);
    screen.stop.dispatch_pending();
    screen.emit("REVIEW4 NO LATER WORK\n");
}

fn review4_outcome(scenario: &str, test: &str) {
    if std::env::var(REVIEW4).is_ok() {
        let args = std::env::var("M2DD_REVIEW4_ARGS").unwrap();
        let clean = run(args.split('\n').map(str::to_owned));
        assert_eq!(
            clean,
            !scenario.starts_with("undo"),
            "post-undo emergency retains resource debt"
        );
        return;
    }
    let f = Fixture::new();
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", &format!("hands_tests::{test}"), "--nocapture"])
        .env(REVIEW4, scenario)
        .env(
            "M2DD_REVIEW4_ARGS",
            format!(
                "--home\n{}\n--workspace\n{}\n--name\nhello.txt\n--text\noracle",
                f.home.display(),
                f.workspace.display()
            ),
        );
    let mut p = Pty::start(cmd);
    p.ready();
    if scenario.starts_with("undo") {
        p.create();
    }
    if scenario == "lift" {
        p.line("stop");
        p.expect("STOP RECORDING COMPLETED");
        p.line("lift-for-undo");
        p.yes();
        p.expect(
            "Stop received while your confirmed lift was finishing; the native halt is re-engaged.",
        );
    } else if scenario.starts_with("undo") {
        p.line("undo");
        if scenario == "undo-completed" {
            p.yes();
        } else {
            let code = p.code();
            std::fs::write(f.workspace.join("hello.txt"), b"Owner edited").unwrap();
            p.line(&code);
        }
        let outcome = if scenario == "undo-completed" {
            "completed"
        } else {
            "was refused"
        };
        p.expect(&if outcome == "completed" {
            "Stop received after undo handoff. The restore completed; remaining obligations are reported separately.".to_owned()
        } else {
            "Stop received after undo handoff. The restore was refused.".to_owned()
        });
        assert_eq!(
            f.workspace.join("hello.txt").exists(),
            scenario == "undo-refused"
        );
    } else {
        p.line("cleanup");
        p.expect("CLEANUP");
        assert!(
            !p.output.contains("confirmed cleanup"),
            "cleanup is requested, never natively confirmed"
        );
    }
    p.expect("REVIEW4 NO LATER WORK");
    // The reporter may have emitted the receipt earlier; inspect full output.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !p.output.contains("STOP RECORDING COMPLETED") {
        assert!(p.read_once(deadline));
    }
    p.line("evidence");
    p.expect("phase=emergency_stopped");
    p.expect("EVIDENCE HEALTH");
    p.line("run");
    p.expect("REFUSED recovery_only");
    if scenario.starts_with("undo") {
        let staging = f.home.join("broker/files/coding-runs");
        let retained = crate::stop_tests::support::privacy::files_under(&staging, None);
        assert!(
            !retained.is_empty(),
            "native resources remain after completed/refused restore"
        );
        p.line("quit");
        assert!(
            p.finish().success(),
            "child must verify nonzero host result: {}",
            p.output
        );
        assert!(p.output.contains("UNCLEAN EXIT:"));
        assert!(!p.output.contains("SESSION CLOSED recorded"));
        assert_eq!(
            crate::stop_tests::support::privacy::files_under(&staging, None),
            retained,
            "restricted close/destruction must retain resources"
        );
    } else {
        p.quit();
    }
}
#[test]
fn decision11_host_undo_completed_is_honest() {
    review4_outcome("undo-completed", "decision11_host_undo_completed_is_honest");
}
#[test]
fn decision11_host_undo_refused_is_honest() {
    review4_outcome("undo-refused", "decision11_host_undo_refused_is_honest");
}
#[test]
fn decision11_host_cleanup_is_honest() {
    review4_outcome("cleanup", "decision11_host_cleanup_is_honest");
}
#[test]
fn session_lift_is_unavailable_before_and_after_emergency_without_dialogue() {
    const CHILD: &str = "M2DD_SESSION_LIFT_CHILD";
    if let Ok(args) = std::env::var(CHILD) {
        assert!(run(args.split('\n').map(str::to_owned)));
        return;
    }
    let f = Fixture::new();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "hands_tests::session_lift_is_unavailable_before_and_after_emergency_without_dialogue",
            "--nocapture",
        ])
        .env(
            CHILD,
            format!(
                "--home\n{}\n--workspace\n{}\n--name\nhello.txt\n--text\nno lift authority",
                f.home.display(),
                f.workspace.display()
            ),
        );
    let mut p = Pty::start(command);
    p.ready();
    for emergency in [false, true] {
        if emergency {
            p.line("stop");
            p.expect("STOP RECORDING COMPLETED");
        }
        let before = p.output.matches("Answer code:").count();
        p.line("lift-for-undo");
        p.expect("REFUSED unavailable\n");
        assert_eq!(p.output.matches("Answer code:").count(), before);
    }
    f.empty();
    p.quit();
}

// PTY-R1 A1: Quit accepted by the live reader before main continues startup.
// Test-only hold after Terminal::open; condvar wait with a deadline, no sleeps.
const PTYR1: &str = "M2DD_PTYR1_EARLY_QUIT_CHILD";
pub(crate) fn ptyr1_after_terminal_open(screen: &Shared) {
    if std::env::var(PTYR1).is_err() {
        return;
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut input = screen.input();
    while input.quit.is_none() {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!left.is_zero(), "bounded early Quit acceptance");
        input = screen.changed.wait_timeout(input, left).unwrap().0;
    }
}
#[test]
fn ptyr1_quit_before_startup_still_reports_recovery_and_unclean_close() {
    if std::env::var(PTYR1).is_ok() {
        let args = std::env::var("M2DD_PTYR1_ARGS").unwrap();
        assert!(
            !run(args.split('\n').map(str::to_owned)),
            "early Quit over invalid history is never a clean close"
        );
        return;
    }
    use nexus_mother::journal::{Entry, Event, GENESIS, SESSION};
    let f = Fixture::new();
    nexus_mother::adapter::create_private(&f.home).unwrap();
    let raw = format!(
        "{}\n",
        Entry::chained(0, GENESIS, SESSION, Event::SessionClosed).to_line()
    );
    let journal = f.home.join("mother-journal.jsonl");
    std::fs::write(&journal, &raw).unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "hands_tests::ptyr1_quit_before_startup_still_reports_recovery_and_unclean_close",
            "--nocapture",
        ])
        .env(PTYR1, "1")
        .env(
            "M2DD_PTYR1_ARGS",
            format!(
                "--home\n{}\n--workspace\n{}\n--name\nhello.txt\n--text\nmust not start",
                f.home.display(),
                f.workspace.display()
            ),
        );
    let mut p = Pty::start(command);
    p.line("quit");
    assert!(
        p.finish().success(),
        "child: run() returned non-success: {}",
        p.output
    );
    for required in [
        "SOURCES mother_verified=false",
        "RECOVERY FAILED mother_history",
        "RESULT: ",
    ] {
        assert!(
            p.output.contains(required),
            "early Quit must still show {required:?}: {}",
            p.output
        );
    }
    assert!(!p.output.contains("HANDS READY"), "{}", p.output);
    assert_eq!(std::fs::read_to_string(&journal).unwrap(), raw);
    f.empty();
}

// M2e-D (U17; rulings SB3E-R131, R134; register docs/security/m2e-d-oracles.md):
// model-mode startup. Every oracle runs production `run` in a child of this
// test binary on a real PTY, with the real HostSession, store and kernel
// transport, against a loopback fake runtime. Test-only parts: the endpoint
// hook, the session stash and phase projection in `open_home`, and the
// LD_PRELOAD shim. No oracle runs the shipped binary in model mode.
const M2ED: &str = "M2ED_CHILD";
const M2ED_ARGS: &str = "M2ED_ARGS";
const M2ED_ENDPOINT: &str = "M2ED_ENDPOINT";
const M2ED_SOCKET: &str = "M2ED_SOCKET";
const MODEL_MODE: &str = "--mission\none-page-site";
const FILE_MODE: &str = "--name\nhello.txt\n--text\noracle";
const PIN_NAME: &str = "qwen3.6:35b-a3b";
const PIN_DIGEST: &str = "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";
/// Frozen at Step 0 (R134.3): the qualified pin's identity digest.
const PIN_IDENTITY: &str = "4d7211f035868b327c0e10f2bc23155a7d492d7beecc5b505edfa373db82f9a4";
const MODEL_PATHS: [&str; 4] = ["/api/version", "/api/status", "/api/tags", "/api/show"];
const MODEL_READY: &str = "Model verified; the page mission is not available in this build. No model call is made.\nHANDS READY\n";
const UNKNOWN_MODEL: &str = "CLOSE MODEL operation=unknown active=unknown owner=unknown child=unknown record=unknown uncertain=true";
const NOT_REQUIRED: &str = "operation=registration active=false owner_complete=true child_complete=true record=not_required uncertain=false";
static SESSION: std::sync::OnceLock<Arc<nexus_governed_control::host_session::HostSession>> =
    std::sync::OnceLock::new();
static PHASES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
struct Phases;
impl EvidenceProjection for Phases {
    fn project(&self, record: &EvidenceRecord) {
        PHASES.lock().unwrap().push(record.phase.as_str());
    }
}

/// The `#[cfg(test)]` endpoint of the model step (hands.rs). Unset is an
/// empty endpoint, which the backend refuses.
pub(crate) fn model_endpoint() -> &'static str {
    static ENDPOINT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ENDPOINT.get_or_init(|| std::env::var(M2ED_ENDPOINT).unwrap_or_default())
}

/// The backend's model obligations, read from the stashed real session.
fn obligations() -> String {
    match SESSION.get().map(|s| s.close_obligations()) {
        Some(Ok(o)) => format!(
            "operation={} active={} owner_complete={} child_complete={} record={} uncertain={}",
            o.model.operation,
            o.model.active,
            o.model.owner_complete,
            o.model.child_complete,
            o.model.record,
            o.model.uncertain
        ),
        Some(Err(_)) => "unavailable".into(),
        None => "no_session".into(),
    }
}

/// The child side: an optional barrier helper (it reads the obligations
/// when the parent says the shim holds), then production `run`. The exit
/// status is `run`'s outcome: 0 for true, 17 for false.
fn m2ed_child() -> bool {
    use std::io::{Read, Write};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixStream};
    if std::env::var(M2ED).is_err() {
        return false;
    }
    if let Ok(name) = std::env::var(M2ED_SOCKET) {
        let mut stream =
            UnixStream::connect_addr(&SocketAddr::from_abstract_name(name.as_bytes()).unwrap())
                .unwrap();
        std::thread::spawn(move || {
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(20)))
                .unwrap();
            let mut go = [0];
            stream.read_exact(&mut go).unwrap();
            assert_eq!(go, [b'C']);
            let phases = PHASES.lock().unwrap().clone();
            eprintln!("M2ED HELD {} phases={phases:?}", obligations());
            stream.write_all(b"S").unwrap();
        });
    }
    let args = std::env::var(M2ED_ARGS).unwrap();
    let clean = run(args.split('\n').map(str::to_owned));
    eprintln!("M2ED RUN RETURNED clean={clean}");
    eprintln!("M2ED OBLIGATIONS {}", obligations());
    eprintln!("M2ED PHASES {:?}", PHASES.lock().unwrap());
    std::process::exit(if clean { 0 } else { 17 })
}

fn m2ed(f: &Fixture, test: &str, endpoint: &str, mode: &str) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &format!("hands_tests::{test}"), "--nocapture"])
        .env(M2ED, test)
        .env(M2ED_ENDPOINT, endpoint)
        .env(
            M2ED_ARGS,
            format!(
                "--home\n{}\n--workspace\n{}\n{mode}",
                f.home.display(),
                f.workspace.display()
            ),
        );
    command
}

/// The rest of the first transcript line that starts with `prefix`.
fn line_of<'a>(output: &'a str, prefix: &str) -> Option<&'a str> {
    output.lines().find_map(|l| l.strip_prefix(prefix))
}

#[derive(Clone, Copy, PartialEq)]
enum Serve {
    Healthy,
    WrongVersion,
    CloudEnabled,
    NoModel,
    /// `/api/show` is never answered; the peer reports the connection's end.
    HoldShow,
    /// Arm the shim when `/api/show` arrives, then answer it.
    ArmShow,
    /// Arm the shim when `/api/show` arrives, and never answer it.
    HoldShowArm,
}
enum PeerEvent {
    ShowHeld,
    ShowClosed { at: std::time::Instant, eof: bool },
}
struct Peer {
    endpoint: String,
    paths: Arc<std::sync::Mutex<Vec<String>>>,
    events: std::sync::mpsc::Receiver<PeerEvent>,
}
impl Peer {
    fn paths(&self) -> Vec<String> {
        self.paths.lock().unwrap().clone()
    }
    fn event(&self) -> PeerEvent {
        self.events
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("bounded peer event")
    }
}
/// A loopback runtime answering as the pinned model, one request per
/// connection; it logs every request path in the order received.
fn peer(serve: Serve) -> Peer {
    armed_peer(serve, None)
}
/// As `peer`, writing the shim's arm file when `/api/show` arrives.
fn armed_peer(serve: Serve, arm: Option<std::path::PathBuf>) -> Peer {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let paths: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
    let (events, received) = std::sync::mpsc::channel();
    let log = paths.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().map_while(Result::ok) {
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(20)))
                .unwrap();
            let (mut head, mut byte) = (Vec::new(), [0u8; 1]);
            while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).is_ok_and(|n| n == 1) {
                head.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&head).to_ascii_lowercase();
            let length = head.split("content-length:").nth(1);
            let length: Option<usize> = length.and_then(|l| l.lines().next()?.trim().parse().ok());
            let _ = stream.read_exact(&mut vec![0u8; length.unwrap_or(0)]);
            let path = head
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_string();
            log.lock().unwrap().push(path.clone());
            let show = path == "/api/show";
            if show && matches!(serve, Serve::ArmShow | Serve::HoldShowArm) {
                std::fs::write(arm.as_ref().unwrap(), b"armed").unwrap();
            }
            if show && matches!(serve, Serve::HoldShow | Serve::HoldShowArm) {
                let _ = events.send(PeerEvent::ShowHeld);
                let mut rest = [0u8; 64];
                let eof = matches!(stream.read(&mut rest), Ok(0));
                let at = std::time::Instant::now();
                let _ = events.send(PeerEvent::ShowClosed { at, eof });
                continue;
            }
            let body = match (serve, path.as_str()) {
                (Serve::WrongVersion, "/api/version") => r#"{"version":"0.40.12"}"#.to_string(),
                (_, "/api/version") => r#"{"version":"0.40.13"}"#.to_string(),
                (Serve::CloudEnabled, "/api/status") => {
                    r#"{"cloud":{"disabled":false,"source":"env"}}"#.to_string()
                }
                (_, "/api/status") => r#"{"cloud":{"disabled":true,"source":"env"}}"#.to_string(),
                (Serve::NoModel, "/api/tags") => r#"{"models":[]}"#.to_string(),
                (_, "/api/tags") => format!(
                    r#"{{"models":[{{"name":"{PIN_NAME}","model":"{PIN_NAME}","digest":"{PIN_DIGEST}"}}]}}"#
                ),
                _ => r#"{"capabilities":["completion","thinking"]}"#.to_string(),
            };
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    Peer {
        endpoint,
        paths,
        events: received,
    }
}
/// A loopback address with no listener.
fn refused_endpoint() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    endpoint
}

/// B0: the mode's exact arguments, and `run` refused before any lane.
#[test]
fn m2ed_mode_parse_and_run_refused() {
    if m2ed_child() {
        return;
    }
    let test = "m2ed_mode_parse_and_run_refused";
    let f = Fixture::new();
    for mode in [
        "--mission\nother-page",
        "--mission\none-page-site\n--name\nhello.txt",
        "--mission\none-page-site\n--text\noracle",
        "--mission\none-page-site\n--hold-before-publish\n0",
        "--mission\none-page-site\n--mission\none-page-site",
    ] {
        let mut p = Pty::start(m2ed(&f, test, "", mode));
        let status = p.finish();
        assert_eq!(status.code(), Some(17), "{mode:?}: {}", p.output);
        assert!(
            p.output.contains("invalid_arguments\n"),
            "{mode:?}: {}",
            p.output
        );
        assert!(!p.output.contains("SOURCES"), "{mode:?}: {}", p.output);
    }
    assert!(!f.home.exists(), "a refused argument list opens no home");
    let peer = peer(Serve::Healthy);
    let mut p = Pty::start(m2ed(&f, test, &peer.endpoint, MODEL_MODE));
    p.expect(MODEL_READY);
    p.line("run");
    p.expect(
        "RESULT: request refused; no new operation was admitted.\nREFUSED page_mission_unavailable\n",
    );
    p.line("quit");
    p.expect("SESSION CLOSED recorded\n");
    assert_eq!(p.finish().code(), Some(0), "{}", p.output);
    for absent in ["Answer code:", "LANE ", "MISSION ", "DOOR "] {
        assert!(!p.output.contains(absent), "{absent:?}: {}", p.output);
    }
    assert_eq!(peer.paths(), MODEL_PATHS, "no chat request");
    f.empty();
}

/// B1: the fixed pin is registered before readiness, and the host shows
/// only the backend's verified view.
#[test]
fn m2ed_registration_precedes_readiness() {
    if m2ed_child() {
        return;
    }
    let f = Fixture::new();
    let peer = peer(Serve::Healthy);
    let test = "m2ed_registration_precedes_readiness";
    let mut p = Pty::start(m2ed(&f, test, &peer.endpoint, MODEL_MODE));
    let before = p.expect("MODEL VERIFIED ");
    assert!(!before.contains("HANDS READY"), "{}", p.output);
    let view = p.expect("\n");
    assert_eq!(
        view,
        format!("name={PIN_NAME} digest={PIN_DIGEST} identity={PIN_IDENTITY}\n"),
        "B1: the verified identity is the frozen pin's"
    );
    assert_eq!(p.expect(MODEL_READY), MODEL_READY, "{}", p.output);
    assert_eq!(peer.paths(), MODEL_PATHS, "{}", p.output);
    p.line("quit");
    p.expect("SESSION CLOSED recorded\n");
    assert_eq!(p.finish().code(), Some(0), "{}", p.output);
    assert!(!p.output.contains("One File mission"), "{}", p.output);
    assert_eq!(p.output.matches("MODEL VERIFIED ").count(), 1);
    assert_eq!(
        line_of(&p.output, "M2ED OBLIGATIONS "),
        Some("operation=registration active=false owner_complete=true child_complete=true record=recorded uncertain=false"),
        "{}",
        p.output
    );
    assert_eq!(
        line_of(&p.output, "M2ED PHASES "),
        Some("[\"model_registered\"]")
    );
    assert_eq!(peer.paths(), MODEL_PATHS, "one registration, no chat");
    f.empty();
}

/// B1c (static): no flag, variable or file chooses the endpoint or pin in
/// a non-test build. hands.rs has no `env` text at all.
#[test]
fn m2ed_no_override_in_production_source() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hands.rs")).unwrap();
    assert!(!source.contains("env"), "B1c: hands.rs names env");
    assert_eq!(
        source
            .matches("#[cfg(not(test))]\nconst ENDPOINT: &str = \"http://127.0.0.1:11434\";")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("#[cfg(not(test))]\n                        let endpoint = ENDPOINT;")
            .count(),
        1
    );
    assert_eq!(source.matches("ENDPOINT").count(), 2);
    assert_eq!(source.matches("register_local_model(").count(), 1);
    assert_eq!(
        source
            .matches(".register_local_model(endpoint, qualified_pin())")
            .count(),
        1
    );
    assert_eq!(source.matches("qualified_pin()").count(), 2);
    assert_eq!(source.matches("model_endpoint()").count(), 1);
}

/// B2: every registration refusal blocks readiness; one attempt, no retry,
/// no fallback. Assertions read the transcript after exit (R134.3).
#[test]
fn m2ed_registration_failure_blocks_readiness() {
    if m2ed_child() {
        return;
    }
    let test = "m2ed_registration_failure_blocks_readiness";
    let classes: [(&str, Option<Serve>, &[&str]); 5] = [
        (
            "wrong_version",
            Some(Serve::WrongVersion),
            &MODEL_PATHS[..1],
        ),
        (
            "cloud_enabled",
            Some(Serve::CloudEnabled),
            &MODEL_PATHS[..2],
        ),
        ("digest_absent", Some(Serve::NoModel), &MODEL_PATHS[..3]),
        ("no_listener", None, &[]),
        ("stop_during_show", Some(Serve::HoldShow), &MODEL_PATHS),
    ];
    for (class, serve, served) in classes {
        let f = Fixture::new();
        let peer = serve.map(peer);
        let endpoint = peer
            .as_ref()
            .map_or_else(refused_endpoint, |p| p.endpoint.clone());
        let mut p = Pty::start(m2ed(&f, test, &endpoint, MODEL_MODE));
        let stop = class == "stop_during_show";
        if let (true, Some(peer)) = (stop, &peer) {
            assert!(matches!(peer.event(), PeerEvent::ShowHeld));
            p.line("stop");
            assert!(matches!(
                peer.event(),
                PeerEvent::ShowClosed { eof: true, .. }
            ));
        }
        p.expect("STARTUP REFUSED model_registration_refused\n");
        p.line("run");
        if stop {
            p.expect("REFUSED recovery_only\n");
            p.line("quit");
        }
        let status = p.finish();
        let out = &p.output;
        assert_eq!(out.matches("STARTUP REFUSED ").count(), 1, "{class}: {out}");
        for absent in [
            "MODEL VERIFIED",
            "Model verified;",
            "One File mission",
            "Answer code:",
        ] {
            assert!(!out.contains(absent), "{class}: {absent:?}: {out}");
        }
        let paths = peer.as_ref().map(Peer::paths).unwrap_or_default();
        assert_eq!(paths, served, "{class}: one attempt, no retry");
        assert_eq!(
            line_of(out, "M2ED OBLIGATIONS "),
            Some(NOT_REQUIRED),
            "{class}: {out}"
        );
        if stop {
            assert!(
                out.contains("Report only; no work authority is available.\nHANDS READY\n"),
                "{out}"
            );
            assert_eq!(status.code(), Some(0), "{out}");
        } else {
            assert!(out.contains("HANDS REPORT ONLY"), "{class}: {out}");
            assert!(!out.contains("HANDS READY"), "{class}: {out}");
            assert!(
                out.contains("REFUSED request_unavailable\n"),
                "{class}: {out}"
            );
            assert!(out.contains(UNKNOWN_MODEL), "{class}: {out}");
            assert!(!out.contains("SESSION CLOSED recorded"), "{class}: {out}");
            assert_eq!(status.code(), Some(17), "{class}: {out}");
        }
        f.empty();
    }
}

/// B5: the default File mode makes no model request, and its transcript up
/// to readiness is the base's, byte for byte (measured at Step 0).
#[test]
fn m2ed_no_model_mode_makes_no_request() {
    if m2ed_child() {
        return;
    }
    let f = Fixture::new();
    let counter = peer(Serve::Healthy);
    let test = "m2ed_no_model_mode_makes_no_request";
    let mut p = Pty::start(m2ed(&f, test, &counter.endpoint, FILE_MODE));
    let prefix = p.expect("HANDS READY\n");
    let start = prefix.find("SOURCES ").expect("startup report");
    assert_eq!(
        &prefix[start..],
        "SOURCES mother_verified=true resources_verified=true primary_latched=false primary_owed=0 primary_recovery_required=false primary_unsettled=0 primary_unclean_sessions=0\n\
         Fresh start: no earlier session in this home; nothing to recover.\n\
         One File mission per fresh process. Emergency ends mission work.\nHANDS READY\n"
    );
    p.quit();
    assert_eq!(counter.paths(), Vec::<String>::new(), "no model request");
    assert_eq!(
        line_of(&p.output, "M2ED OBLIGATIONS "),
        Some("operation=none active=false owner_complete=true child_complete=true record=not_required uncertain=false")
    );
    f.empty();
}

/// B3a H3-R (R131.2): a real registration worker held at its uncancellable
/// evidence append (model.rs:305), through the shim, reaches the real
/// bounded `failed_startup` fallback. Slot active, the normal close reporter
/// skipped, only the unknown model report, no affirmative close notice,
/// nonzero exit within the bound; the hold is read from the backend, not
/// from the MODEL_UNKNOWN text alone.
#[test]
fn m2ed_h3_held_registration_fallback() {
    use std::io::{Read, Write};
    use std::os::fd::AsFd;
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener};
    if m2ed_child() {
        return;
    }
    let f = Fixture::new();
    let fault = crate::stop_tests::support::faults::Fault::new(&f, "evidence");
    let peer = armed_peer(Serve::ArmShow, Some(fault.arm.clone()));
    let socket = format!("m2ed.h3r.{}", std::process::id());
    let listener =
        UnixListener::bind_addr(&SocketAddr::from_abstract_name(socket.as_bytes()).unwrap())
            .unwrap();
    let mut command = m2ed(
        &f,
        "m2ed_h3_held_registration_fallback",
        &peer.endpoint,
        MODEL_MODE,
    );
    fault.attach(&f, &mut command);
    command.env(M2ED_SOCKET, &socket);
    let mut p = Pty::start(command);
    let mut fds = [nix::poll::PollFd::new(
        listener.as_fd(),
        nix::poll::PollFlags::POLLIN,
    )];
    assert_eq!(nix::poll::poll(&mut fds, 10000u16).unwrap(), 1);
    let (mut helper, _) = listener.accept().unwrap();
    helper
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    // The first armed evidence write, after the peer answered `/api/show`.
    let receipt = fault.reached();
    assert!(
        receipt
            .path
            .contains("/broker/p3-evidence/evidence.sqlite3"),
        "{}",
        receipt.path
    );
    assert_eq!(peer.paths(), MODEL_PATHS, "verification finished");
    helper.write_all(b"C").unwrap();
    let mut read = [0];
    helper.read_exact(&mut read).unwrap();
    assert_eq!(read, [b'S']);
    p.expect("M2ED HELD ");
    let held = p.expect("\n").trim_end().to_string();
    // Verified, owner incomplete, the record pending and not committed:
    // the worker is inside model.rs:295-330 at the append.
    assert_eq!(
        held,
        "operation=registration active=true owner_complete=false child_complete=true record=pending uncertain=false phases=[]",
        "{}",
        p.output
    );
    assert!(!p.output.contains("STOP"), "no other evidence producer");
    let quit = std::time::Instant::now();
    p.line("quit");
    let status = p.finish();
    let elapsed = quit.elapsed();
    drop(receipt);
    let out = &p.output;
    assert_eq!(status.code(), Some(17), "nonzero exit: {out}");
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "Quit to exit {elapsed:?}: {out}"
    );
    println!("M2ED H3-R quit_to_exit_ms={}", elapsed.as_millis());
    let close: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("CLOSE MODEL "))
        .collect();
    assert_eq!(close, [UNKNOWN_MODEL], "{out}");
    for absent in [
        "SESSION CLOSED recorded",
        "CLOSE FAILED stage=",
        "HANDS READY",
        "MODEL VERIFIED",
    ] {
        assert!(!out.contains(absent), "{absent:?}: {out}");
    }
    assert!(out.contains(crate::terminal::UNCLEAN_NOTICE), "{out}");
    assert_eq!(line_of(out, "M2ED RUN RETURNED "), Some("clean=false"));
    // The worker is still held when `run` returns.
    assert_eq!(
        line_of(out, "M2ED OBLIGATIONS "),
        Some("operation=registration active=true owner_complete=false child_complete=true record=pending uncertain=false"),
        "{out}"
    );
    assert_eq!(line_of(out, "M2ED PHASES "), Some("[]"));
    assert_eq!(peer.paths(), MODEL_PATHS);
    f.empty();
}

/// B3b H3-C (R131.2, fail-safe control): an open registration request is
/// cancelled at the shutdown trigger and the peer sees the connection's end
/// before the producer deadline, so it cannot reach the deadline. At Quit
/// the close is clean; at an input failure it is the unclean fallback.
#[test]
fn m2ed_h3_open_request_cancelled_at_quit() {
    if m2ed_child() {
        return;
    }
    let test = "m2ed_h3_open_request_cancelled_at_quit";
    for trigger in ["quit", "input_failure"] {
        let f = Fixture::new();
        let fault = crate::stop_tests::support::faults::Fault::new(&f, "read-fail");
        let failure = trigger == "input_failure";
        let peer = if failure {
            armed_peer(Serve::HoldShowArm, Some(fault.arm.clone()))
        } else {
            peer(Serve::HoldShow)
        };
        let mut command = m2ed(&f, test, &peer.endpoint, MODEL_MODE);
        if failure {
            fault.attach(&f, &mut command);
        }
        let mut p = Pty::start(command);
        assert!(matches!(peer.event(), PeerEvent::ShowHeld));
        let sent = std::time::Instant::now();
        if failure {
            // The next stdin read returns EIO: the reader's input failure.
            p.send(b"x");
        } else {
            p.line("quit");
        }
        let PeerEvent::ShowClosed { at, eof } = peer.event() else {
            panic!("{trigger}: the held request's end")
        };
        let alive = p.child.try_wait().unwrap().is_none();
        assert!(eof && alive, "{trigger}: EOF {eof}, child alive {alive}");
        let latency = at.duration_since(sent);
        assert!(
            latency < std::time::Duration::from_millis(2750),
            "{trigger}: EOF after {latency:?}"
        );
        println!("M2ED H3-C {trigger} eof_ms={}", latency.as_millis());
        let status = p.finish();
        let out = &p.output;
        assert_eq!(peer.paths(), MODEL_PATHS, "{trigger}: no further request");
        assert!(
            out.contains("STARTUP REFUSED model_registration_refused\n"),
            "{out}"
        );
        for absent in ["MODEL VERIFIED", "HANDS READY"] {
            assert!(!out.contains(absent), "{trigger}: {absent:?}: {out}");
        }
        assert_eq!(
            line_of(out, "M2ED OBLIGATIONS "),
            Some(NOT_REQUIRED),
            "{out}"
        );
        let close: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("CLOSE MODEL "))
            .collect();
        if failure {
            assert_eq!(status.code(), Some(17), "{out}");
            assert_eq!(close, [UNKNOWN_MODEL], "{out}");
            assert!(!out.contains("SESSION CLOSED recorded"), "{out}");
        } else {
            assert_eq!(status.code(), Some(0), "{out}");
            assert!(close.is_empty(), "{out}");
            assert!(out.contains("SESSION CLOSED recorded\n"), "{out}");
        }
        f.empty();
    }
}

/// B4 X1: SIGKILL of the model-mode host at three barriers (the
/// `/api/show` request open at the peer; the registration append held by
/// the shim; after readiness), then two restarts on the same home. Each
/// restart is report-only from the backend's startup report: no model
/// request, no registration, no readiness, `run` refused, no dialogue.
#[test]
fn m2ed_x1_death_restart_report_only() {
    use std::os::unix::process::ExitStatusExt;
    if m2ed_child() {
        return;
    }
    let test = "m2ed_x1_death_restart_report_only";
    for barrier in ["show_open", "append_held", "ready"] {
        let f = Fixture::new();
        let fault = crate::stop_tests::support::faults::Fault::new(&f, "evidence");
        let (serve, arm) = match barrier {
            "show_open" => (Serve::HoldShow, None),
            "append_held" => (Serve::ArmShow, Some(fault.arm.clone())),
            _ => (Serve::Healthy, None),
        };
        let first = armed_peer(serve, arm);
        let mut command = m2ed(&f, test, &first.endpoint, MODEL_MODE);
        if barrier == "append_held" {
            fault.attach(&f, &mut command);
        }
        let mut p = Pty::start(command);
        let receipt = match barrier {
            "show_open" => {
                assert!(matches!(first.event(), PeerEvent::ShowHeld));
                None
            }
            "append_held" => Some(fault.reached()),
            _ => {
                p.expect(MODEL_READY);
                None
            }
        };
        p.child.kill().unwrap();
        let killed = p.child.wait().unwrap();
        assert_eq!(killed.signal(), Some(9), "{barrier}: killed");
        drop(receipt);
        drop(p);
        for restart in 1..=2 {
            let counter = peer(Serve::Healthy);
            let mut q = Pty::start(m2ed(&f, test, &counter.endpoint, MODEL_MODE));
            q.expect("HANDS REPORT ONLY");
            q.line("run");
            q.expect("REFUSED");
            q.line("quit");
            let status = q.finish();
            let out = &q.output;
            let sources = line_of(out, "SOURCES ").unwrap_or_default();
            assert!(
                sources.ends_with(&format!(" primary_unclean_sessions={restart}")),
                "{barrier} restart {restart}: {sources}"
            );
            assert_eq!(
                counter.paths(),
                Vec::<String>::new(),
                "{barrier} restart {restart}: {out}"
            );
            for absent in [
                "MODEL VERIFIED",
                "HANDS READY",
                "Model verified;",
                "Answer code:",
            ] {
                assert!(
                    !out.contains(absent),
                    "{barrier} restart {restart}: {absent:?}: {out}"
                );
            }
            assert_eq!(
                line_of(out, "M2ED OBLIGATIONS "),
                Some("operation=none active=false owner_complete=true child_complete=true record=not_required uncertain=false"),
                "{barrier} restart {restart}: no registration was attempted"
            );
            assert_eq!(
                status.code(),
                Some(17),
                "{barrier} restart {restart}: {out}"
            );
        }
        f.empty();
    }
}
