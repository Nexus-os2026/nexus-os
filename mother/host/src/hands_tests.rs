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
        p.expect("STOP RECORDING COMPLETED count=1");
        p.expect("STOP RECORDING COMPLETED count=2");
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
