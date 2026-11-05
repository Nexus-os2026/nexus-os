//! Decision 6, oracle first: no recording worker is ever scheduled here.
//! Each hook places ingress after an earlier check, before the real effect.
use super::*;

fn selected() -> (Fx, crate::authority::file_session::FileAttachment) {
    let mut f = Fx::new();
    let port = f.door.file_port().unwrap();
    let attached = port.attach().unwrap();
    f.door.claim_file_session(&attached).unwrap();
    f.run = f.door.open_file_run(&attached).unwrap();
    (f, attached)
}

fn schedule(point: &'static str) {
    let (f, attached) = selected();
    let runs = f.gov.authority().runs();
    let group = f.door.group_for_test();
    if point == "model.before_attempt" {
        runs.admit_inference(f.run, &mother(), group, HASH, LABEL, [42; 32])
            .unwrap();
    }
    let baseline = runs.review2_counts();
    let preparations = Arc::new(AtomicUsize::new(0));
    let seen = preparations.clone();
    f.gov.files().hooks().every(at::FILE_STAGED, move || {
        seen.fetch_add(1, Ordering::SeqCst);
    });
    let engaged = Arc::new(AtomicBool::new(false));
    let saw = engaged.clone();
    let sessions = f.gov.file_sessions.clone();
    let ingress = move || {
        sessions.emergency(); // the existing memory-only backend ingress
        saw.store(true, Ordering::SeqCst);
    };
    if point == "files.validation" {
        f.gov.files().hooks().arm(point, ingress);
    } else if point == at::ADMIT_BEFORE_BIND {
        f.gov.hooks().arm(point, ingress);
    } else {
        runs.review2_hooks.arm(point, ingress);
    }
    let refused = match point {
        "run.before_insert" => f.door.open_file_run(&attached).is_err(),
        "inference.before_install" => runs
            .admit_inference(f.run, &mother(), group, HASH, LABEL, [42; 32])
            .is_err(),
        "model.before_attempt" => runs
            .admit_model_call(f.run, &mother(), group, &[42; 32])
            .is_err(),
        _ => f.door.admit_files(f.run, HASH, LABEL).is_err(),
    };
    assert!(
        engaged.load(Ordering::SeqCst),
        "exact ingress boundary reached"
    );
    assert!(
        !f.gov.stop_snapshot().broker_stopped,
        "recording worker never ran"
    );
    assert!(!f.gov.stop_snapshot().kernel_stopped);
    let after = runs.review2_counts();
    assert_eq!(after, baseline, "{point}: zero new backend runs / File bindings / inference bindings / model attempts after ingress");
    assert!(refused, "{point}: admission must refuse after validation");
    assert!(f.door.open_file_run(&attached).is_err());
    assert!(f.door.admit_files(f.run, HASH, LABEL).is_err());
    assert!(runs
        .admit_inference(f.run, &mother(), group, HASH, LABEL, [42; 32])
        .is_err());
    assert!(runs
        .admit_model_call(f.run, &mother(), group, &[42; 32])
        .is_err());
    assert!(f.propose(NAME, CONTENT).is_err());
    assert_eq!(runs.review2_counts(), baseline);
    assert_eq!(
        preparations.load(Ordering::SeqCst),
        0,
        "zero backend Files dispatches"
    );
    assert!(f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .is_empty());
}

#[test]
fn review2_stop_during_workspace_validation() {
    schedule("files.validation");
}
#[test]
fn review2_stop_between_workspace_validation_and_binding_hold() {
    schedule(at::ADMIT_BEFORE_BIND);
}
#[test]
fn review2_stop_between_validation_and_file_install() {
    schedule("files.before_install");
}
#[test]
fn review2_stop_between_validation_and_inference_install() {
    schedule("inference.before_install");
}
#[test]
fn review2_stop_between_validation_and_run_install() {
    schedule("run.before_insert");
}
#[test]
fn review2_stop_between_validation_and_model_attempt() {
    schedule("model.before_attempt");
}

#[test]
fn review2_backend_counters_have_real_positive_controls() {
    let (f, attached) = selected();
    let runs = f.gov.authority().runs();
    let before = runs.review2_counts();
    f.door.open_file_run(&attached).unwrap();
    f.door.admit_files(f.run, HASH, LABEL).unwrap();
    runs.admit_inference(
        f.run,
        &mother(),
        f.door.group_for_test(),
        HASH,
        LABEL,
        [42; 32],
    )
    .unwrap();
    runs.admit_model_call(f.run, &mother(), f.door.group_for_test(), &[42; 32])
        .unwrap();
    f.propose(NAME, CONTENT).unwrap();
    assert_eq!(runs.review2_counts(), before.map(|n| n + 1));
    assert_eq!(f.gov.authority().commitments().views_of_run(f.run).len(), 1);
}
