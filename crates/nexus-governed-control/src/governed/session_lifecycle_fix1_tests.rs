//! Review 1 mechanism B: the first shutdown and every close call share one boundary.
use super::*;
use crate::host_session::SessionState;
use crate::test_hooks::Gate;

fn first_failure_deadline(eof: bool) {
    let f = Fixture::new();
    let gate = Gate::new();
    let held = gate.clone();
    f.gov
        .file_session_halt()
        .1
        .arm("close.failed.published", move || held.hold());
    let ingress = f._session.ingress();
    let failing = ingress.clone();
    let worker = std::thread::spawn(move || failing.fail());
    gate.reached();
    let original_latest = std::time::Instant::now() + Duration::from_secs(3);
    assert!(ingress.failed(), "the first denial already won its CAS");
    let observed = if eof {
        // The actual reader EOF order: emergency, fail, then publish EOF.
        ingress.emergency().unwrap();
        ingress.fail();
        ingress.close_deadline()
    } else {
        Some(ingress.accept(CommandKind::Quit, 2).unwrap().deadline())
    };
    gate.open();
    worker.join().unwrap();
    assert!(
        observed.is_some(),
        "EOF must not publish shutdown with a missing deadline"
    );
    let observed = observed.unwrap();
    assert!(
        observed <= original_latest,
        "later Quit must not refresh the first failure deadline"
    );
    assert_eq!(
        ingress.close_deadline(),
        Some(observed),
        "published first deadline is immutable"
    );
    println!("FIX1 B first-failure eof={eof} deadline-present=true no-refresh=true");
}

#[test]
fn fix1_b_first_failure_deadline_cannot_be_refreshed_by_concurrent_quit() {
    first_failure_deadline(false);
}

#[test]
fn fix1_b_first_failure_deadline_is_available_to_concurrent_eof() {
    first_failure_deadline(true);
}

#[test]
fn fix1_b_retained_close_resources_refuse_before_native_entry_after_seal() {
    let f = Fixture::new();
    close_opening(&f);
    let quit = f._session.ingress().accept(CommandKind::Quit, 2).unwrap();
    f._session.settle_close_resources(&quit).unwrap();
    f._session.settle_close_authority(&quit).unwrap();
    let seal = f._session.seal_close(&quit).unwrap();
    assert_eq!(f._session.state(), SessionState::SealedFinalizing);
    let entered = Arc::new(AtomicUsize::new(0));
    let count = entered.clone();
    f.gov
        .file_session_halt()
        .1
        .every("close.resources.native", move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
    let resource_result = f._session.settle_close_resources(&quit);
    let authority_result = f._session.settle_close_authority(&quit);
    drop(seal);
    let closed_result = f._session.settle_close_resources(&quit);
    assert!(
        resource_result.is_err(),
        "Sealed-finalizing retained Quit must refuse"
    );
    assert!(authority_result.is_err());
    assert!(
        closed_result.is_err(),
        "Closed-unclean retained Quit must refuse"
    );
    assert_eq!(
        entered.load(Ordering::SeqCst),
        0,
        "no native resource settlement may start after seal"
    );
    println!("FIX1 B retained-close native-calls=0 sealed-and-closed-refused=true");
}

#[test]
fn fix1_b_each_close_operation_remains_accounted_until_native_return() {
    let mut wrongly_sealed = Vec::new();
    for authority in [false, true] {
        let f = Fixture::new();
        close_opening(&f);
        let quit = Arc::new(f._session.ingress().accept(CommandKind::Quit, 2).unwrap());
        f._session.settle_close_resources(&quit).unwrap();
        f._session.settle_close_authority(&quit).unwrap();
        let gate = Gate::new();
        let held = gate.clone();
        let point = if authority {
            "close.authority.native"
        } else {
            "close.resources.native"
        };
        f.gov.file_session_halt().1.arm(point, move || held.hold());
        let session = f._session.clone();
        let request = quit.clone();
        let worker = std::thread::spawn(move || {
            if authority {
                session.settle_close_authority(&request).is_ok()
            } else {
                session.settle_close_resources(&request).is_ok()
            }
        });
        gate.reached();
        let attempted = f._session.seal_close(&quit);
        if attempted.is_ok() {
            wrongly_sealed.push(point);
        }
        gate.open();
        assert!(
            worker.join().unwrap(),
            "already registered close operation may complete"
        );
        if let Ok(seal) = attempted {
            drop(seal);
        } else {
            drop(
                f._session
                    .seal_close(&quit)
                    .expect("completed close operation releases its exact producer"),
            );
        }
        println!(
            "FIX1 B accounted-close point={point} sealed-while-held={}",
            wrongly_sealed.contains(&point)
        );
    }
    assert!(
        wrongly_sealed.is_empty(),
        "unaccounted native close calls: {wrongly_sealed:?}"
    );
}

#[test]
fn fix1_b_expired_close_resources_refuse_before_native_entry() {
    let f = Fixture::new();
    close_opening(&f);
    let quit = f._session.ingress().accept(CommandKind::Quit, 2).unwrap();
    let entered = Arc::new(AtomicUsize::new(0));
    let count = entered.clone();
    f.gov
        .file_session_halt()
        .1
        .every("close.resources.native", move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
    // This is the lifetime under test, not a guessed synchronization delay.
    std::thread::sleep(
        quit.deadline()
            .saturating_duration_since(std::time::Instant::now()),
    );
    let resources = f._session.settle_close_resources(&quit);
    let authority = f._session.settle_close_authority(&quit);
    assert!(
        resources.is_err(),
        "expired close cannot begin native settlement"
    );
    assert!(authority.is_err());
    assert_eq!(entered.load(Ordering::SeqCst), 0);
    println!("FIX1 B original-deadline-expired native-calls=0");
}
