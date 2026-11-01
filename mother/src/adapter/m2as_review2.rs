//! M2a-S oracles added after the second fresh-context review (they do not
//! replace or edit the earlier ones). Run from `real_broker.rs`.
//!
//! - MINOR 4: the broker itself refuses an effect whose run the fence
//!   cancelled: the coordinator breaks after the port has the run, just
//!   before it asks the broker to act (past the port's own fence check).
//! - MINOR 1: a journaled action two missions claim is handed back to
//!   neither, but recorded: its withdrawal is an error while the broker
//!   still holds it open, so the open fails closed instead of succeeding
//!   with the request still approvable; and the port's own withdraw of such
//!   an action is an error.

use super::m2as_support::{executed, open_requests, rig, Disk, Rig, WorldServer};
use super::{GovernedPort, Home};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::EffectPort;
use crate::journal::{verify, Entry, Event, MemoryJournal, MissionId, GENESIS};
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::CommitmentState;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::sync::{Arc, Weak};

fn approved(r: &Rig, markers: &str) -> MissionId {
    let id = r.mother.draft(contract_with(markers)).unwrap();
    r.mother.submit(id).unwrap();
    assert_eq!(
        r.mother.request_consent(id, &Yes),
        Ok(MissionState::Approved)
    );
    id
}

fn breaker(
    disk: &Disk,
    mother: &Arc<Coordinator<GovernedPort>>,
    victim: MissionId,
) -> impl FnOnce() + Send + 'static {
    let (disk, mother): (Disk, Weak<Coordinator<GovernedPort>>) =
        (disk.clone(), Arc::downgrade(mother));
    move || {
        disk.fail(true, true);
        let mother = mother.upgrade().expect("the coordinator");
        assert!(mother.stop(victim).is_err());
        assert!(mother.broken());
        disk.fail(false, false);
    }
}

fn the_broker_itself_refuses_an_effect_of_a_fenced_run(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let b = approved(
        &r,
        &format!("FETCH-BUILDER:{origin}/review2-1 FETCH-VERIFIER:{origin}/review2-2"),
    );
    for _ in 0..3 {
        r.mother.advance(b).unwrap();
    }
    assert!(world.saw("/review2-1"));
    let run = r.port.run_of(b).expect("B's run");
    let runs = BTreeSet::from([run]);
    let before = executed(control, &runs);
    // Past run_for (and its fence check): only the broker can refuse now.
    r.port
        .before_broker_action
        .arm(0, breaker(&disk, &r.mother, victim));
    assert!(r.mother.advance(b).is_err());
    assert!(!r.port.before_broker_action.armed(), "the hook ran");
    assert!(
        !world.saw("/review2-2"),
        "the broker ran an effect of a cancelled run"
    );
    assert_eq!(executed(control, &runs), before);
    let views = control.authority().commitments().views_of_run(run);
    assert!(
        views.iter().all(|v| v.state != CommitmentState::Prepared),
        "{views:?}"
    );
}

/// The journal's lines with `edit` applied to each entry, re-chained.
fn rechained(lines: &[String], mut edit: impl FnMut(&Entry) -> Event) -> Vec<String> {
    let mut prev = GENESIS.to_string();
    verify(lines)
        .unwrap()
        .iter()
        .map(|entry| {
            let next = Entry::chained(entry.seq, &prev, entry.mission, edit(entry));
            prev = next.hash.clone();
            next.to_line()
        })
        .collect()
}

fn a_doubly_claimed_live_action_fails_the_open_closed(
    control: &Arc<GovernedControl>,
    origin: &str,
) {
    // Two missions, each waiting on its own live request.
    let journal = MemoryJournal::new();
    let port = Arc::new(GovernedPort::new(control.clone()));
    let lanes = || Lanes::new(keyed_lanes()).unwrap();
    let clock = || Arc::new(SystemClock::default());
    let mother = Coordinator::open(journal.clone(), port.clone(), lanes(), clock()).unwrap();
    let mut ids = Vec::new();
    for n in 0..2 {
        let id = mother
            .draft(contract_with(&format!(
                "POST-BUILDER:{origin}/review2-claim-{n}"
            )))
            .unwrap();
        mother.submit(id).unwrap();
        assert_eq!(mother.request_consent(id, &Yes), Ok(MissionState::Approved));
        for _ in 0..3 {
            mother.advance(id).unwrap();
        }
        assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
        ids.push(id);
    }
    let first = port.pending(ids[0]).unwrap();
    drop(mother);

    // The journal (tampered) names the first request for both, and both
    // missions were stopped before their closures were journaled.
    let mut lines = rechained(&journal.lines(), |e| match &e.event {
        Event::ApprovalPending { effect, .. } => Event::ApprovalPending {
            effect: *effect,
            pending: first.clone(),
        },
        other => other.clone(),
    });
    for id in &ids {
        let entries = verify(&lines).unwrap();
        let last = entries.last().unwrap();
        lines.push(
            Entry::chained(
                last.seq + 1,
                &last.hash,
                *id,
                Event::Transition {
                    trigger: Trigger::Stop {
                        reason: "owner".into(),
                    },
                },
            )
            .to_line(),
        );
    }
    journal.set_lines(lines);

    // A rebuilt port over the same live broker: neither mission is given
    // the request back, so it cannot be withdrawn: the open fails closed.
    let rebuilt = Arc::new(GovernedPort::new(control.clone()));
    assert!(
        Coordinator::open(journal.clone(), rebuilt.clone(), lanes(), clock()).is_err(),
        "an open succeeded with a journaled request it could not withdraw"
    );

    // The port's own withdraw of a handed, unheld, live action is an error.
    let spare = GovernedPort::new(control.clone());
    spare.hand(MissionId(9_101), &first);
    assert!(spare.withdraw(MissionId(9_101)).is_err());

    // Clean up through the port that holds both runs.
    for id in ids {
        port.withdraw(id).unwrap();
    }
    assert!(open_requests(
        control,
        &[port.run_of(MissionId(1)), port.run_of(MissionId(2))]
            .into_iter()
            .flatten()
            .collect()
    )
    .is_empty());
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    the_broker_itself_refuses_an_effect_of_a_fenced_run(&home.control, origin, &world);
    a_doubly_claimed_live_action_fails_the_open_closed(&home.control, origin);
    world.stop();
}
