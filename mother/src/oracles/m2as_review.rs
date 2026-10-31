//! M2a-S oracles added after the fresh-context review (they do not replace
//! or edit the earlier ones): a broken coordinator acknowledges nothing, a
//! session the live coordinator could not write is refused, a clean
//! journal survives a failed session record, partial session openings, and
//! a lost Stop while another mission's lane call is in flight.

use super::m2as_durability::*;
use super::*;
use crate::journal::{Event, JournalError, JournalStore, SESSION};
use crate::state::Trigger;
use MissionState::*;

/// A store over a memory journal whose appends fail once `left` more have
/// succeeded, and whose poisoning always fails.
#[derive(Clone)]
struct Countdown {
    inner: MemoryJournal,
    left: Arc<Mutex<Option<usize>>>,
}

impl Countdown {
    fn over(inner: MemoryJournal) -> Self {
        Self {
            inner,
            left: Arc::new(Mutex::new(None)),
        }
    }

    fn fail_after(&self, n: usize) {
        *self.left.lock().unwrap() = Some(n);
    }
}

impl JournalStore for Countdown {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.inner.load()
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let mut left = self.left.lock().unwrap();
        match left.as_mut() {
            Some(0) => return Err(JournalError::Store("injected failure".into())),
            Some(n) => *n -= 1,
            None => {}
        }
        self.inner.append(line)
    }

    fn poison(&mut self) -> Result<(), JournalError> {
        Err(JournalError::Store("injected poison failure".into()))
    }
}

#[test]
fn m2as_review_a_broken_coordinator_never_acknowledges_a_lost_stop_or_denial() {
    for state in unfinished() {
        let (w, disk) = faulty_world();
        let id = reach(&w, state);
        disk.fail(true, true);
        assert!(w.mother.stop(id).is_err(), "{state:?}");
        disk.fail(false, false);
        // Retried, the Stop is still not durable: never Ok.
        assert!(
            w.mother.stop(id).is_err(),
            "{state:?}: a retried Stop said Ok"
        );
        assert_eq!(w.mother.stop_all(), 0, "{state:?}");
        let w = crash(&w);
        assert_eq!(w.mother.held(id).as_deref(), Some("unclean_shutdown"));
    }

    // An action denial the journal lost: a Stop after it is not Ok either.
    let (w, disk) = faulty_world();
    let id = reach(&w, AwaitingApproval);
    w.port.approval_reply(ApprovalStatus::Denied);
    disk.fail(true, true);
    assert!(w.mother.advance(id).is_err());
    disk.fail(false, false);
    assert!(
        w.mother.stop(id).is_err(),
        "a Stop after a lost denial said Ok"
    );
    assert_eq!(w.mother.stop_all(), 0);

    // A mission that ended durably before the failure: Stop does not claim
    // anything on a broken coordinator either.
    let (w, disk) = faulty_world();
    let done = w.mission_in(Done);
    let live = reach(&w, Building);
    disk.fail(true, true);
    assert!(w.mother.stop(live).is_err());
    assert!(w.mother.stop(done).is_err());
}

#[test]
fn m2as_review_a_broken_coordinator_asks_the_owner_and_the_broker_nothing() {
    let (w, disk) = faulty_world();
    let id = reach(&w, AwaitingConsent);
    disk.fail(true, true);
    assert!(w.mother.request_consent(id, &No).is_err());
    disk.fail(false, false);
    let shown = Recording::default();
    assert!(w.mother.request_consent(id, &shown).is_err());
    assert_eq!(
        shown.0.lock().unwrap().len(),
        0,
        "the Owner was asked again"
    );

    let (w, disk) = faulty_world();
    let waiting = reach(&w, AwaitingApproval);
    let other = reach(&w, Building);
    disk.fail(true, true);
    assert!(w.mother.stop(other).is_err());
    disk.fail(false, false);
    let queries = w.port.approval_queries();
    assert!(w.mother.advance(waiting).is_err());
    assert_eq!(w.port.approval_queries(), queries, "the broker was asked");
    assert_eq!(performs(&w), 2, "only the effects before the failure");
}

#[test]
fn m2as_review_replay_refuses_a_clean_close_that_skips_the_holds() {
    let w = world();
    let building = reach(&w, Building);
    let base = w.journal.lines(); // its session never closed
    let opened = forged(&base, SESSION, Event::SessionOpened);

    // What the live coordinator writes after an unclean session opens.
    let held = forged(&opened, building, held_event());
    let w2 =
        opens(forged(&held, SESSION, Event::SessionClosed)).expect("holds, then a clean close");
    assert_eq!(
        w2.mother.held(building).as_deref(),
        Some("unclean_shutdown")
    );
    let stopped = forged(
        &opened,
        building,
        Event::Transition {
            trigger: Trigger::Stop {
                reason: "owner".into(),
            },
        },
    );
    opens(forged(&stopped, SESSION, Event::SessionClosed)).expect("ended, then a clean close");

    // What it never writes.
    refused(
        forged(&opened, SESSION, Event::SessionClosed),
        "a clean close with an unfinished mission never held",
    );
    refused(
        forged(
            &opened,
            building,
            Event::Transition {
                trigger: Trigger::PhaseDone,
            },
        ),
        "work before the hold an unclean session requires",
    );
}

#[test]
fn m2as_review_a_failed_session_record_does_not_poison_a_clean_journal() {
    let w = world();
    let id = reach(&w, Building);
    let (closed, w) = close_and_reopen(w);
    closed.unwrap();
    let World {
        journal,
        clock,
        keyed,
        mother,
        ..
    } = w;
    Arc::try_unwrap(mother).ok().unwrap().close().unwrap();
    let lines = journal.lines();

    // The disk refuses the new session's record: the open fails, and the
    // journal (which lost nothing) is not poisoned.
    let disk = Faulty::over(journal.clone());
    disk.fail(true, false);
    assert!(matches!(
        open_on(disk.clone(), journal.clone(), clock.clone(), keyed.clone()).err(),
        Some(CoordinatorError::Journal(_))
    ));
    assert!(!journal.is_poisoned());
    assert_eq!(journal.lines(), lines);

    // Once the disk works, it opens as cleanly closed.
    let w = open_on(journal.clone(), journal, clock, keyed).unwrap();
    assert_eq!(w.mother.held(id), None);
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.state(id), Some(Verifying));
}

#[test]
fn m2as_review_a_session_opening_cut_short_still_holds_every_mission() {
    // Fail at every write of an unclean session's opening (its record,
    // then each hold): whatever made it to the journal, the next open
    // holds both missions and nothing runs.
    for n in 0..4 {
        let w = world();
        let a = reach(&w, Building);
        let b = reach(&w, Investigating);
        let lanes = w.keyed_calls();
        let store = Countdown::over(w.journal.clone());
        let w2 = open_on(
            store.clone(),
            w.journal.clone(),
            w.clock.clone(),
            w.keyed.clone(),
        )
        .unwrap();
        store.fail_after(n);
        let resumed = w2.mother.resume(a);
        if n < 3 {
            assert!(resumed.is_err(), "{n}");
        }
        let w3 = crash(&w2);
        if resumed.is_ok() {
            assert_eq!(
                w3.mother.held(a).as_deref(),
                Some("unclean_shutdown"),
                "{n}: an unclean session"
            );
        } else {
            assert_held(&w3, a, Building, lanes);
        }
        assert_held(&w3, b, Investigating, lanes);
    }
}

#[test]
fn m2as_review_a_stop_lost_while_another_mission_s_lane_runs() {
    let held = Arc::new(HeldLane::new(
        LaneRole::ResearcherA,
        vec![get("https://example.org/")],
    ));
    let mut lanes: Vec<Arc<dyn Lane>> = vec![held.clone()];
    lanes.extend(
        keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::ResearcherA),
    );
    let disk = Faulty::over(MemoryJournal::new());
    let port = Arc::new(RecordingPort::default());
    let clock = Arc::new(ManualClock::default());
    let mother = Arc::new(
        Coordinator::open(
            disk.clone(),
            port.clone(),
            Lanes::new(lanes.clone()).unwrap(),
            clock.clone(),
        )
        .unwrap(),
    );
    let w = World {
        port,
        journal: disk.inner.clone(),
        clock,
        lanes,
        keyed: vec![],
        mother,
    };
    let stopped = w.approved(contract());
    let running = w.approved(contract());
    w.mother.advance(running).unwrap();
    w.mother.advance(stopped).unwrap();

    let mother = w.mother.clone();
    let working = std::thread::spawn(move || mother.advance(running));
    assert!(held.wait_entered());
    disk.fail(true, true);
    assert!(w.mother.stop(stopped).is_err());
    disk.fail(false, false);
    held.release();
    assert!(
        working.join().unwrap().is_err(),
        "the lane's answer was kept"
    );
    assert_eq!(performs(&w), 0, "an effect after the lost Stop");

    let w2 = Coordinator::open(
        w.journal.clone(),
        Arc::new(RecordingPort::default()),
        Lanes::new(w.lanes.clone()).unwrap(),
        w.clock.clone(),
    )
    .unwrap();
    for id in [stopped, running] {
        assert_eq!(w2.held(id).as_deref(), Some("unclean_shutdown"));
        assert_eq!(w2.advance(id), Err(CoordinatorError::Held));
    }
}
