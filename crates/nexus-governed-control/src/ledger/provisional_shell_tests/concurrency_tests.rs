//! Several threads through the store's one lock (`Gate`), on a real file.
//! Every result a thread is given is checked against the file afterwards:
//! Committed is in it; an expired wait is not; the word ends with nothing
//! alive, nothing unapplied and no handover fault.

use super::*;
use std::time::Duration;

#[test]
fn threads_through_the_gate_get_truthful_answers() {
    const THREADS: usize = 8;
    const EACH: usize = 25;
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let gate = Arc::new(Gate::new(opened(&path)));
    let long = || Instant::now() + Duration::from_secs(60);
    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let gate = Arc::clone(&gate);
            std::thread::spawn(move || {
                let mut committed = Vec::new();
                let mut expired = Vec::new();
                for i in 0..EACH {
                    let identity = format!("t{t}-r{i}");
                    let (mut ticket, built) = gate
                        .with(long(), |store| {
                            (
                                store.health().arm().expect("below the ticket limit"),
                                store.fact(identity.as_bytes(), None, b"c"),
                            )
                        })
                        .expect("the store is entered");
                    // Every fifth wait is too short to be sure of entering.
                    let deadline = if i % 5 == 4 { Instant::now() } else { long() };
                    let outcome = gate
                        .append_facts(deadline, &mut ticket, std::slice::from_ref(&built))
                        .unwrap();
                    ticket.confirm();
                    match outcome {
                        FactOutcome::Committed => committed.push(built.key()),
                        FactOutcome::NotApplied {
                            reason: Reason::Entry(Entry::Expired),
                            ..
                        } => expired.push(built.key()),
                        other => panic!("{identity}: {other:?}"),
                    }
                }
                (committed, expired)
            })
        })
        .collect();
    let mut committed = Vec::new();
    let mut expired = Vec::new();
    for handle in handles {
        let (c, e) = handle.join().unwrap();
        committed.extend(c);
        expired.extend(e);
    }
    assert_eq!(committed.len() + expired.len(), THREADS * EACH);
    let view = gate.with(long(), |store| store.health().view()).unwrap();
    assert_eq!(view.live(), 0);
    assert_eq!(view.unapplied(), 0);
    assert!(!view.handover_fault());
    assert!(view.admits());
    drop(gate);
    let on_disk = rows_on_disk(&path);
    for key in &committed {
        assert!(on_disk
            .iter()
            .any(|(_, id, digest)| id == &key.id && digest == &key.digest));
    }
    for key in &expired {
        assert!(!on_disk.iter().any(|(_, id, _)| id == &key.id));
    }
    eprintln!(
        "gate: {} committed, {} expired",
        committed.len(),
        expired.len()
    );
    Store::open(&path, LIMITS).expect("the file reopens with its chain whole");
}
