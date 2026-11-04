//! Review 1: deterministic production-path schedules, test-only child hooks.
use crate::hands::run;
use crate::stop::Gate;
use crate::stop_tests::support::{Fixture, Pty};
use crate::terminal::Shared;
use crate::Home;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::EvidencePhase;
use nexus_governed_control::authority::evidence::{EvidenceProjection, EvidenceRecord};
use nexus_governed_control::files::FileCreate;
use nexus_mother::adapter::GovernedPort;
use nexus_mother::effect::{EffectPort, EffectRequest};
use nexus_mother::journal::MissionId;
use nexus_mother::lane::LaneRole;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const CHILD: &str = "M2DD_REVIEW1_CHILD";
static ENGAGED: AtomicBool = AtomicBool::new(false);

pub(crate) fn before_internal_admission(screen: &Shared, home: &Home) {
    if std::env::var(CHILD).as_deref() == Ok("admission") && !ENGAGED.swap(true, Ordering::SeqCst) {
        assert!(!screen.stop.gate.recovery());
        // The outer Mission check has succeeded. Perform exactly the first
        // memory-only step of Stop ingress; its worker has not been scheduled.
        screen.stop.gate.emergency();
        assert!(!home.owner().stop_snapshot().broker_stopped);
        assert_eq!(screen.stop.receipt().outstanding, 0);
    }
}

pub(crate) fn before_internal_dispatch(gate: &Gate, role: LaneRole) {
    if std::env::var(CHILD).as_deref() == Ok("dispatch") && role == LaneRole::Builder {
        // Lane was admitted and returned its request, but the coordinator has
        // not handed that request to the real port yet.
        gate.emergency();
        ENGAGED.store(true, Ordering::SeqCst);
    }
}

pub(crate) fn after_internal_admission(port: &GovernedPort, home: &Home, id: MissionId) {
    let scenario = std::env::var(CHILD).unwrap_or_default();
    if scenario != "admission" && !(scenario == "dispatch" && ENGAGED.load(Ordering::SeqCst)) {
        return;
    }
    let runs = home.owner().authority().runs().views().len();
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
    path: &std::path::Path,
    confirmer: crate::Confirmer,
) -> Result<Home, crate::HomeError> {
    if std::env::var(CHILD).as_deref() != Ok("panic") {
        return Home::open(path, confirmer);
    }
    use nexus_governed_control::authority::evidence::MemoryEvidence;
    use nexus_governed_control::{
        broker::Vault, evidence_store::PrimaryEvidence, governed::GovernedControl,
    };
    nexus_mother::adapter::create_private(path).unwrap();
    let broker = path.join("broker");
    let recent = Arc::new(MemoryEvidence::new(256));
    let evidence = PrimaryEvidence::open(
        &broker.join("p3-evidence/evidence.sqlite3"),
        vec![recent.clone(), Arc::new(PanicRecording)],
    )
    .unwrap();
    let control = Arc::new(
        GovernedControl::new(
            &broker,
            Vault::Disabled,
            evidence.clone(),
            Arc::new(SystemClock::default()),
        )
        .unwrap(),
    );
    Ok(Home {
        dir: path.into(),
        control,
        confirmer,
        evidence,
        recent,
    })
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
