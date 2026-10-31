//! M0-FIX F5: the owner's emergency stop halts coding runs in every phase
//! (a worker turn, a model call in flight, apply, verification), and new
//! runs are refused until the owner's resume. Each test uses its own stop
//! authority (never the process's). The model is a scripted stand-in; the
//! real local model's in-flight call is ended by the stop in the kernel's
//! `m0_f5_emergency_stop` integration test.

use super::p1_apply::{default_edits, env, verified, Confirm};
use super::*;
use crate::emergency_stop::StopAuthority;
use std::sync::mpsc;
use std::sync::Mutex;

type Hook = Box<dyn Fn(usize) + Send + Sync>;

/// Answers in order; `hook(n)` runs inside call `n` (while it is in flight).
struct StopModel {
    pin: ModelPin,
    answers: Mutex<Vec<serde_json::Value>>,
    calls: AtomicUsize,
    hook: Hook,
}

impl StopModel {
    fn new(answers: Vec<serde_json::Value>, hook: Hook) -> Self {
        Self {
            pin: local_model::pin_for_test("f5-model"),
            answers: Mutex::new(answers),
            calls: AtomicUsize::new(0),
            hook,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl LocalModel for StopModel {
    fn pin(&self) -> &ModelPin {
        &self.pin
    }

    fn complete(
        &self,
        _messages: &[ModelMessage],
        _timeout: Duration,
        _max_response_bytes: u64,
    ) -> Result<String, ModelError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        (self.hook)(n);
        let mut answers = self.answers.lock().unwrap();
        Ok(if answers.is_empty() {
            json!({"action": "finish", "summary": "done"})
        } else {
            answers.remove(0)
        }
        .to_string())
    }
}

fn halted_by_stop(run: &CodingRun) -> bool {
    run.state() == RunState::Revoked(RevocationReason::EmergencyStop)
}

fn revoked_by_stop_recorded(f: &Fixture, run: &CodingRun) -> bool {
    f.ledger
        .verify_run(run.id().ledger_key())
        .unwrap()
        .iter()
        .any(|r| r.event_kind == "run.revoked" && r.payload.contains("EmergencyStop"))
}

/// Stop pressed during a worker turn: the model's answer (an edit) is
/// never applied, no further turn starts, and the run ends revoked by the
/// stop, recorded, its staging authority closed.
#[test]
fn m0_f5_stop_halts_a_worker_turn() {
    let stop = StopAuthority::new();
    let f = fixture();
    let (mut run, model) = with_test_stop_authority(stop.clone(), || {
        let trigger = stop.clone();
        let model = StopModel::new(
            vec![
                json!({"action": "read", "paths": ["src/lib.rs"]}),
                json!({"action": "edit", "edits": [{"path": "src/lib.rs", "op": "replace", "content": "stopped\n"}]}),
            ],
            Box::new(move |n| {
                if n == 1 {
                    trigger.engage();
                }
            }),
        );
        let mut run = staged_run(&f);
        run.pin_model(model.pin().clone()).unwrap();
        (run, model)
    });
    let result = worker::run_worker(&mut run, &model, "change the answer");
    assert!(result.is_err(), "{result:?}");
    assert_eq!(model.calls(), 2, "a turn started after the stop");
    assert!(halted_by_stop(&run), "{:?}", run.state());
    assert!(revoked_by_stop_recorded(&f, &run));
    assert!(run.staged_files().is_empty() || run.verification().is_none());
    assert!(!kinds(&f, &run).iter().any(|k| k == "operation.prepared"));
}

/// Stop pressed while a model call is in flight (readiness: the model
/// reports it was entered): the worker ends at once when the call returns
/// and records the stop.
#[test]
fn m0_f5_stop_halts_a_worker_mid_model_call() {
    let stop = StopAuthority::new();
    let f = fixture();
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let entered_tx = Mutex::new(entered_tx);
    let release_rx = Mutex::new(release_rx);
    let (mut run, model) = with_test_stop_authority(stop.clone(), || {
        let model = StopModel::new(
            vec![
                json!({"action": "edit", "edits": [{"path": "src/lib.rs", "op": "replace", "content": "late\n"}]}),
            ],
            Box::new(move |n| {
                if n == 0 {
                    entered_tx.lock().unwrap().send(()).unwrap();
                    release_rx
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(30))
                        .unwrap();
                }
            }),
        );
        let mut run = staged_run(&f);
        run.pin_model(model.pin().clone()).unwrap();
        (run, model)
    });
    let result = std::thread::scope(|scope| {
        let worker = scope.spawn(|| worker::run_worker(&mut run, &model, "task"));
        entered_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("the model call started");
        stop.engage();
        release_tx.send(()).unwrap();
        worker.join().unwrap()
    });
    assert!(result.is_err(), "{result:?}");
    assert_eq!(model.calls(), 1);
    assert!(halted_by_stop(&run), "{:?}", run.state());
    assert!(revoked_by_stop_recorded(&f, &run));
    assert!(!kinds(&f, &run).iter().any(|k| k == "operation.prepared"));
}

/// Stop pressed after the owner approved the review: apply is refused and
/// the project is untouched.
#[test]
fn m0_f5_stop_refuses_apply() {
    let stop = StopAuthority::new();
    let e = env();
    let before = std::fs::read_to_string(e.f.project.join("src/lib.rs")).unwrap();
    let (mut run, binding) =
        with_test_stop_authority(stop.clone(), || verified(&e, default_edits()));
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    stop.engage();
    assert!(run.apply(approval, grant, &parent(&e.f)).is_err());
    assert_eq!(
        std::fs::read_to_string(e.f.project.join("src/lib.rs")).unwrap(),
        before,
        "apply wrote after the stop"
    );
    assert!(!e.f.project.join("src/new/deep.rs").exists());
    assert!(halted_by_stop(&run), "{:?}", run.state());
}

/// Stop pressed before a verification: it is refused, before anything is
/// prepared or launched.
#[test]
fn m0_f5_stop_refuses_verification() {
    let stop = StopAuthority::new();
    let e = env();
    let (mut run, _binding) =
        with_test_stop_authority(stop.clone(), || verified(&e, default_edits()));
    stop.engage();
    let inputs = verifier::VerifierInputs {
        profile_hash: [1; 32],
        toolchain_digest: [2; 32],
        toolchain_generation: 1,
        sandbox_policy_hash: [4; 32],
        resource_policy_hash: [5; 32],
    };
    assert_eq!(
        run.prepare_verification(inputs).unwrap_err(),
        verifier::VerificationError::EmergencyStopped
    );
    assert!(halted_by_stop(&run), "{:?}", run.state());
    assert!(!kinds(&e.f, &run)
        .iter()
        .any(|k| k == "verification.prepared"));
}

/// New runs are refused while the stop is in force; after the owner's
/// resume a new run works, while a run the stop ended stays ended.
#[test]
fn m0_f5_new_runs_are_refused_until_the_owner_resumes() {
    let stop = StopAuthority::new();
    let controller = stop.claim_controller().unwrap();
    let f = fixture();
    with_test_stop_authority(stop.clone(), || {
        let mut old = staged_run(&f);
        stop.engage();
        assert_eq!(
            CodingRun::create(
                Arc::clone(&f.ledger) as Arc<dyn LedgerStore>,
                Arc::clone(&f.registry),
                f.grant,
                f.binding,
                default_scopes(),
            )
            .unwrap_err(),
            RunError::Revoked(RevocationReason::EmergencyStop)
        );
        assert!(controller.lift_if(stop.stop_number()));
        assert!(old.edit(replace("src/lib.rs", "x\n")).is_err());
        assert!(halted_by_stop(&old), "{:?}", old.state());
        let mut fresh = staged_run(&f);
        fresh
            .edit(replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n"))
            .unwrap();
        assert!(fresh.verify_structural().unwrap().passed());
    });
}
