//! M2a-S (oracle first): Stop and denial durability across restarts.
//!
//! The invariant: a Stop or denial that returns Ok holds forever, across
//! any restart. One that cannot be made durable returns Err, nothing more
//! happens in that process, and after any restart Mother dispatches no new
//! effect for any unfinished mission until the Owner explicitly decides.
//!
//! The case these pin: a journal append fails and poisoning the journal
//! fails too, so a valid prefix without the Stop (or denial) reopens. Every
//! open journals a session record; only an orderly shutdown of a coordinator
//! that is not broken journals a clean close. A journal whose last session
//! has no clean close holds every unfinished mission until the Owner
//! resumes (journaled) or stops it.

use super::*;
use crate::journal::{Entry, Event, JournalError, JournalStore, SESSION};
use crate::lane::InvocationId;
use crate::state::Trigger;
use std::sync::atomic::AtomicBool;
use MissionState::*;

/// Every lane that asks for an effect does so in Building and Verifying,
/// so a mission that carried on would reach the port.
const MARKERS: &str = "ASK-BUILDER-NETWORK ASK-VERIFIER-NETWORK";

pub(super) fn action() -> PendingAction {
    PendingAction {
        run: "run-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a".into(),
        commitment: "cmt-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a".into(),
        deadline_mono_ms: 600_000,
    }
}

// ---------------------------------------------------------------- a failing disk

/// A journal store over a memory journal whose appends, and whose
/// poisoning, can be made to fail: a disk that stops taking writes.
#[derive(Clone)]
pub(crate) struct Faulty {
    pub inner: MemoryJournal,
    pub appends_fail: Arc<AtomicBool>,
    pub poison_fails: Arc<AtomicBool>,
}

impl Faulty {
    pub fn over(inner: MemoryJournal) -> Self {
        Self {
            inner,
            appends_fail: Arc::new(AtomicBool::new(false)),
            poison_fails: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Appends and poisoning fail (`true`) or work again (`false`).
    pub fn fail(&self, appends: bool, poison: bool) {
        self.appends_fail.store(appends, Ordering::SeqCst);
        self.poison_fails.store(poison, Ordering::SeqCst);
    }
}

impl JournalStore for Faulty {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.inner.load()
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        if self.appends_fail.load(Ordering::SeqCst) {
            return Err(JournalError::Store("injected append failure".into()));
        }
        self.inner.append(line)
    }

    fn poison(&mut self) -> Result<(), JournalError> {
        if self.poison_fails.load(Ordering::SeqCst) {
            return Err(JournalError::Store("injected poison failure".into()));
        }
        self.inner.poison()
    }
}

// ---------------------------------------------------------------- worlds

pub(super) fn keyed() -> Vec<Arc<KeyedLane>> {
    LaneRole::ALL
        .iter()
        .map(|role| Arc::new(KeyedLane::new(*role)))
        .collect()
}

/// A Mother process over `store`, with a port of its own (a restart
/// rebuilds the port).
pub(super) fn open_on(
    store: impl JournalStore + 'static,
    journal: MemoryJournal,
    clock: Arc<ManualClock>,
    keyed: Vec<Arc<KeyedLane>>,
) -> Result<World, CoordinatorError> {
    let port = Arc::new(RecordingPort::default());
    let lanes: Vec<Arc<dyn Lane>> = keyed.iter().map(|l| l.clone() as Arc<dyn Lane>).collect();
    let mother = Coordinator::open(
        store,
        port.clone(),
        Lanes::new(lanes.clone()).unwrap(),
        clock.clone(),
    )?;
    Ok(World {
        port,
        journal,
        clock,
        lanes,
        keyed,
        mother: Arc::new(mother),
    })
}

/// A new home whose journal sits on a disk that can fail.
pub(super) fn faulty_world() -> (World, Faulty) {
    let disk = Faulty::over(MemoryJournal::new());
    let w = open_on(
        disk.clone(),
        disk.inner.clone(),
        Arc::new(ManualClock::default()),
        keyed(),
    )
    .unwrap();
    (w, disk)
}

/// Mother dies (no orderly shutdown) and a new process opens the journal.
pub(super) fn crash(w: &World) -> World {
    try_crash(w).expect("the journal opens")
}

fn try_crash(w: &World) -> Result<World, CoordinatorError> {
    open_on(
        w.journal.clone(),
        w.journal.clone(),
        w.clock.clone(),
        w.keyed.clone(),
    )
}

/// Mother shuts down in order (the coordinator is taken by `close`), then
/// a new process opens the journal.
pub(super) fn close_and_reopen(w: World) -> (Result<(), CoordinatorError>, World) {
    let World {
        journal,
        clock,
        keyed,
        mother,
        ..
    } = w;
    let closed = Arc::try_unwrap(mother)
        .ok()
        .expect("one handle on the coordinator")
        .close();
    let w = open_on(journal.clone(), journal, clock, keyed).expect("the journal opens");
    (closed, w)
}

pub(super) fn unfinished() -> Vec<MissionState> {
    MissionState::ALL
        .into_iter()
        .filter(|s| !s.is_terminal())
        .collect()
}

/// A mission in `state` whose lanes would send effects if it carried on;
/// a waiting one has its broker action journaled.
pub(super) fn reach(w: &World, state: MissionState) -> MissionId {
    let advanced = |steps: usize| {
        let id = w.approved(contract_with(MARKERS));
        for _ in 0..steps {
            w.mother.advance(id).unwrap();
        }
        id
    };
    let id = match state {
        Draft => w.mother.draft(contract_with(MARKERS)).unwrap(),
        AwaitingConsent => w.submitted(contract_with(MARKERS)),
        Approved => advanced(0),
        Investigating => advanced(1),
        Building => advanced(2),
        Verifying => advanced(3),
        Explaining => advanced(4),
        AwaitingApproval => {
            w.port.set_pending(Some(action()));
            let id = advanced(2);
            w.port.reply(Ok(EffectOutcome::AwaitingApproval));
            w.mother.advance(id).unwrap();
            w.port.set_pending(None);
            id
        }
        other => panic!("{other:?} is not unfinished"),
    };
    assert_eq!(w.mother.state(id), Some(state), "reaching {state:?}");
    id
}

fn adopted(w: &World) -> Vec<(MissionId, PendingAction)> {
    w.port
        .adopted()
        .into_iter()
        .map(|(id, pending, _)| (id, pending))
        .collect()
}

pub(super) fn performs(w: &World) -> usize {
    w.port
        .calls()
        .iter()
        .filter(|c| matches!(c, PortCall::Perform { .. }))
        .count()
}

/// Held after an unclean shutdown: in its state, and nothing but the
/// Owner's resume or Stop moves it. No lane runs, nothing reaches the
/// port, and Mother does not even ask the broker about a waited action.
pub(super) fn assert_held(w: &World, id: MissionId, state: MissionState, lanes: usize) {
    assert_eq!(w.mother.state(id), Some(state), "{state:?}");
    assert_eq!(
        w.mother.held(id).as_deref(),
        Some("unclean_shutdown"),
        "{state:?}"
    );
    for _ in 0..6 {
        assert_eq!(
            w.mother.advance(id),
            Err(CoordinatorError::Held),
            "{state:?}"
        );
    }
    assert_eq!(
        w.mother.request_consent(id, &Yes),
        Err(CoordinatorError::Held),
        "{state:?}"
    );
    assert!(w.mother.submit(id).is_err(), "{state:?}");
    assert!(
        w.mother.revise(id, contract_with(MARKERS)).is_err(),
        "{state:?}"
    );
    assert_eq!(w.mother.state(id), Some(state), "{state:?}");
    assert_eq!(performs(w), 0, "{state:?}: an effect reached the port");
    assert_eq!(w.port.approval_queries(), 0, "{state:?}");
    assert_eq!(w.keyed_calls(), lanes, "{state:?}: a lane ran");
}

pub(super) fn events_of(w: &World, id: MissionId) -> Vec<Event> {
    w.mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .map(|e| e.event)
        .collect()
}

// ---------------------------------------------------------------- Stop

#[test]
fn m2as_a_stop_neither_journaled_nor_poisoned_never_comes_back_working() {
    for state in unfinished() {
        let (w, disk) = faulty_world();
        let id = reach(&w, state);
        let lanes = w.keyed_calls();
        let before = disk.inner.lines();

        disk.fail(true, true);
        assert!(
            matches!(w.mother.stop(id), Err(CoordinatorError::Journal(_))),
            "{state:?}: a Stop that is not durable is an error"
        );
        // The case: a valid journal without the Stop, and not poisoned.
        assert_eq!(disk.inner.lines(), before, "{state:?}");
        assert!(!disk.inner.is_poisoned(), "{state:?}");

        // Nothing further happens in this process, even once the disk
        // takes writes again: the coordinator stays broken.
        disk.fail(false, false);
        let effects = w.port.effects().len();
        for _ in 0..3 {
            assert!(w.mother.advance(id).is_err(), "{state:?}");
        }
        assert!(w.mother.request_consent(id, &Yes).is_err(), "{state:?}");
        assert!(w.mother.draft(contract()).is_err(), "{state:?}");
        assert_eq!(w.port.effects().len(), effects, "{state:?}");
        assert_eq!(disk.inner.lines(), before, "{state:?}: journaled after");

        // A broken coordinator does not shut down cleanly.
        let (closed, w) = close_and_reopen(w);
        assert!(closed.is_err(), "{state:?}: a broken coordinator closed");

        // After the restart the mission is held, and dispatches nothing.
        assert_held(&w, id, state, lanes);
        if state == AwaitingApproval {
            // Handed back to the rebuilt port as for any waiting mission,
            // so that a Stop can withdraw it; never asked about or approved.
            assert_eq!(adopted(&w), vec![(id, action())]);
            assert!(!w.port.calls().contains(&PortCall::Cancel(id)));
        }

        // Held across any restart, clean or not, until the Owner decides.
        let w = crash(&w);
        assert_held(&w, id, state, lanes);
        let (closed, w) = close_and_reopen(w);
        closed.unwrap();
        assert_held(&w, id, state, lanes);

        // The Owner's Stop of a held mission holds, and withdraws a waited
        // request.
        assert_eq!(w.mother.stop(id), Ok(Stopped), "{state:?}");
        assert_eq!(w.mother.held(id), None, "{state:?}");
        if state == AwaitingApproval {
            assert!(w.port.calls().contains(&PortCall::Cancel(id)));
            assert!(events_of(&w, id).contains(&Event::ApprovalClosed { effect: 0 }));
        }
        let w = crash(&w);
        assert_eq!(w.mother.state(id), Some(Stopped), "{state:?}");
        assert_eq!(w.mother.resume(id).ok(), None, "{state:?}");
        assert!(w.mother.advance(id).is_err(), "{state:?}");
        assert_eq!(performs(&w), 0, "{state:?}");
    }
}

#[test]
fn m2as_a_stop_lost_right_after_a_clean_reopen_is_still_held() {
    // Every open journals its session, so a Stop lost before anything
    // else was written still leaves the session without a clean close.
    let (w, disk) = faulty_world();
    let id = reach(&w, Building);
    let lanes = w.keyed_calls();
    let World {
        journal,
        clock,
        keyed,
        mother,
        ..
    } = w;
    Arc::try_unwrap(mother).ok().unwrap().close().unwrap();
    let w = open_on(disk.clone(), journal, clock, keyed).unwrap();
    assert_eq!(w.mother.held(id), None, "a clean close holds nothing");

    disk.fail(true, true);
    assert!(w.mother.stop(id).is_err());
    assert!(!disk.inner.is_poisoned());
    disk.fail(false, false);
    let w = crash(&w);
    assert_held(&w, id, Building, lanes);
}

#[test]
fn m2as_a_stop_that_returns_ok_holds_across_every_restart() {
    for state in unfinished() {
        let w = world();
        let id = reach(&w, state);
        assert_eq!(w.mother.stop(id), Ok(Stopped), "{state:?}");
        let w = crash(&w);
        assert_eq!(w.mother.state(id), Some(Stopped), "{state:?}");
        assert_eq!(w.mother.held(id), None, "{state:?}");
        assert!(w.mother.resume(id).is_err(), "{state:?}");
        let (closed, w) = close_and_reopen(w);
        closed.unwrap();
        assert_eq!(w.mother.state(id), Some(Stopped), "{state:?}");
        assert!(w.mother.advance(id).is_err(), "{state:?}");
        assert_eq!(performs(&w), 0, "{state:?}");
    }
}

// ---------------------------------------------------------------- Deny

#[test]
fn m2as_a_consent_denial_neither_journaled_nor_poisoned_never_comes_back_approvable() {
    let (w, disk) = faulty_world();
    let id = reach(&w, AwaitingConsent);
    let lanes = w.keyed_calls();
    let before = disk.inner.lines();

    disk.fail(true, true);
    assert!(
        matches!(
            w.mother.request_consent(id, &No),
            Err(CoordinatorError::Journal(_))
        ),
        "a denial that is not durable is an error"
    );
    assert_eq!(disk.inner.lines(), before);
    assert!(!disk.inner.is_poisoned());

    disk.fail(false, false);
    assert!(w.mother.request_consent(id, &Yes).is_err());
    assert!(w.mother.advance(id).is_err());
    assert_eq!(disk.inner.lines(), before, "journaled after the failure");
    assert_eq!(performs(&w), 0);

    let w = crash(&w);
    assert_held(&w, id, AwaitingConsent, lanes);
    let w = crash(&w);
    assert_held(&w, id, AwaitingConsent, lanes);
}

#[test]
fn m2as_an_action_denial_neither_journaled_nor_poisoned_never_comes_back_working() {
    let (w, disk) = faulty_world();
    let id = reach(&w, AwaitingApproval);
    let lanes = w.keyed_calls();
    let performed = performs(&w);
    let before = disk.inner.lines();

    w.port.approval_reply(ApprovalStatus::Denied);
    disk.fail(true, true);
    assert!(
        matches!(w.mother.advance(id), Err(CoordinatorError::Journal(_))),
        "a denial that is not durable is an error"
    );
    assert_eq!(disk.inner.lines(), before);
    assert!(!disk.inner.is_poisoned());

    disk.fail(false, false);
    for _ in 0..3 {
        assert!(w.mother.advance(id).is_err());
    }
    assert_eq!(performs(&w), performed, "an effect after the denial");
    assert_eq!(disk.inner.lines(), before, "journaled after the failure");

    let w = crash(&w);
    assert_held(&w, id, AwaitingApproval, lanes);
    // Handed back, never asked about or approved; the Owner's Stop
    // withdraws it and it is journaled closed.
    assert_eq!(adopted(&w), vec![(id, action())]);
    assert!(!w.port.calls().contains(&PortCall::Cancel(id)));
    assert_eq!(w.mother.stop(id), Ok(Stopped));
    assert!(w.port.calls().contains(&PortCall::Cancel(id)));
    assert!(events_of(&w, id).contains(&Event::ApprovalClosed { effect: 0 }));
    assert_eq!(performs(&w), 0);
}

#[test]
fn m2as_a_denial_that_returns_ok_holds_across_every_restart() {
    let w = world();
    let id = reach(&w, AwaitingConsent);
    assert_eq!(
        w.mother.request_consent(id, &No),
        Err(CoordinatorError::Denied)
    );
    let w = crash(&w);
    assert_eq!(w.mother.state(id), Some(Denied));
    assert_eq!(w.mother.held(id), None);
    assert!(w.mother.resume(id).is_err());
    assert!(w.mother.request_consent(id, &Yes).is_err());

    let w = world();
    let id = reach(&w, AwaitingApproval);
    w.port.approval_reply(ApprovalStatus::Denied);
    assert_eq!(w.mother.advance(id).unwrap().to, Denied);
    let w = crash(&w);
    assert_eq!(w.mother.state(id), Some(Denied));
    assert!(w.mother.resume(id).is_err());
    assert!(w.mother.advance(id).is_err());
    assert_eq!(performs(&w), 0);
}

// ---------------------------------------------------------------- sessions

#[test]
fn m2as_a_crash_holds_every_unfinished_mission_until_the_owner_resumes() {
    let w = world();
    let ids: Vec<(MissionState, MissionId)> = unfinished()
        .into_iter()
        .map(|s| (s, reach(&w, s)))
        .collect();
    let ended: Vec<(MissionState, MissionId)> = [Done, Stopped, Denied, Failed, Blocked]
        .into_iter()
        .map(|s| (s, w.mission_in(s)))
        .collect();
    let lanes = w.keyed_calls();
    let before = w.journal.lines();

    let w = crash(&w);
    for (state, id) in &ids {
        assert_held(&w, *id, *state, lanes);
    }
    // Opening wrote nothing: the holds are journaled with the session's
    // first record.
    assert_eq!(w.journal.lines(), before);
    for (state, id) in &ended {
        assert_eq!(w.mother.state(*id), Some(*state), "{state:?}");
        assert_eq!(w.mother.held(*id), None, "{state:?}");
        assert!(w.mother.resume(*id).is_err(), "{state:?}");
    }
    let waiting = ids.iter().find(|(s, _)| *s == AwaitingApproval).unwrap().1;
    assert!(!w.port.calls().contains(&PortCall::Cancel(waiting)));
    assert_eq!(adopted(&w), vec![(waiting, action())]);

    // The Owner resumes one: journaled, and it carries on to Done,
    // sending its effects.
    let building = ids.iter().find(|(s, _)| *s == Building).unwrap().1;
    assert_eq!(w.mother.resume(building), Ok(Building));
    assert_eq!(events_of(&w, building).last(), Some(&Event::Resumed));
    // The session's first record: its opening, then every hold, once.
    let session: Vec<Entry> =
        crate::journal::verify(&w.journal.lines()).unwrap()[before.len()..].to_vec();
    assert_eq!(session[0].event, Event::SessionOpened);
    for (state, id) in &ids {
        let held: Vec<&Entry> = session
            .iter()
            .filter(|e| e.mission == *id && matches!(e.event, Event::Held { .. }))
            .collect();
        assert_eq!(held.len(), 1, "{state:?}: the hold is journaled once");
        assert_eq!(held[0].event, held_event(), "{state:?}");
    }
    assert_eq!(w.mother.held(building), None);
    assert!(w.mother.resume(building).is_err(), "resume twice");
    for _ in 0..3 {
        w.mother.advance(building).unwrap();
    }
    assert_eq!(w.mother.state(building), Some(Done));
    assert_eq!(performs(&w), 2, "the Builder's and the Verifier's effects");

    // The others are still held; a resumed waiting mission is never
    // approved by Mother: she only asks the broker.
    assert_eq!(w.mother.advance(waiting), Err(CoordinatorError::Held));
    assert_eq!(w.mother.resume(waiting), Ok(AwaitingApproval));
    assert_eq!(w.mother.advance(waiting).unwrap().to, AwaitingApproval);
    assert_eq!(performs(&w), 2);

    // The resume is durable: a clean restart keeps it resumed.
    let approved = ids.iter().find(|(s, _)| *s == Approved).unwrap().1;
    assert_eq!(w.mother.resume(approved), Ok(Approved));
    let (closed, w) = close_and_reopen(w);
    closed.unwrap();
    assert_eq!(w.mother.held(approved), None);
    assert_eq!(w.mother.advance(approved).unwrap().to, Investigating);
    let investigating = ids.iter().find(|(s, _)| *s == Investigating).unwrap().1;
    assert_eq!(
        w.mother.held(investigating).as_deref(),
        Some("unclean_shutdown")
    );

    // A second crash holds every unfinished mission again.
    let w = crash(&w);
    assert_held(&w, approved, Investigating, w.keyed_calls());
}

#[test]
fn m2as_a_clean_close_resumes_every_mission_normally() {
    let w = world();
    let ids: Vec<(MissionState, MissionId)> = unfinished()
        .into_iter()
        .map(|s| (s, reach(&w, s)))
        .collect();

    let (closed, w) = close_and_reopen(w);
    closed.unwrap();
    for (state, id) in &ids {
        assert_eq!(w.mother.state(*id), Some(*state), "{state:?}");
        assert_eq!(w.mother.held(*id), None, "{state:?}");
    }
    let lines = w.journal.lines();
    let entries = crate::journal::verify(&lines).unwrap();
    assert!(!entries
        .iter()
        .any(|e| matches!(e.event, Event::Held { .. })));
    let n = entries.len();
    assert_eq!(entries[n - 2].event, Event::SessionClosed);
    assert_eq!(entries[n - 1].event, Event::SessionOpened);
    assert_eq!(entries[n - 1].mission, SESSION);

    // A waiting mission's request is handed back, not withdrawn.
    let waiting = ids.iter().find(|(s, _)| *s == AwaitingApproval).unwrap().1;
    assert!(!w.port.calls().contains(&PortCall::Cancel(waiting)));
    assert_eq!(w.port.adopted().len(), 1);

    // Missions carry on as before the restart.
    let building = ids.iter().find(|(s, _)| *s == Building).unwrap().1;
    for _ in 0..3 {
        w.mother.advance(building).unwrap();
    }
    assert_eq!(w.mother.state(building), Some(Done));
    assert_eq!(performs(&w), 2);
    let pending = ids.iter().find(|(s, _)| *s == AwaitingConsent).unwrap().1;
    assert_eq!(w.mother.request_consent(pending, &Yes), Ok(Approved));

    // And close again, cleanly.
    let (closed, w) = close_and_reopen(w);
    closed.unwrap();
    assert_eq!(w.mother.held(pending), None);
}

#[test]
fn m2as_a_close_the_journal_did_not_take_is_an_unclean_shutdown() {
    for poison_fails in [true, false] {
        let (w, disk) = faulty_world();
        let id = reach(&w, Building);
        let lanes = w.keyed_calls();
        disk.fail(true, poison_fails);
        let (closed, mother) = {
            let World {
                journal,
                clock,
                keyed,
                mother,
                ..
            } = w;
            (
                Arc::try_unwrap(mother).ok().unwrap().close(),
                (journal, clock, keyed),
            )
        };
        assert!(matches!(closed, Err(CoordinatorError::Journal(_))));
        disk.fail(false, false);
        let (journal, clock, keyed) = mother;
        let reopened = open_on(journal.clone(), journal, clock, keyed);
        if poison_fails {
            assert_held(&reopened.unwrap(), id, Building, lanes);
        } else {
            assert_eq!(
                reopened.err(),
                Some(CoordinatorError::Journal(JournalError::Poisoned))
            );
        }
    }
}

#[test]
fn m2as_no_clean_close_while_a_call_is_in_flight_and_nothing_after_close() {
    let held = Arc::new(HeldLane::new(LaneRole::ResearcherA, vec![]));
    let mut lanes: Vec<Arc<dyn Lane>> = vec![held.clone()];
    lanes.extend(
        keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::ResearcherA),
    );
    let w = world_with(lanes);
    let id = w.approved(contract());
    w.mother.advance(id).unwrap();
    let mother = w.mother.clone();
    let working = std::thread::spawn(move || mother.advance(id));
    assert!(held.wait_entered());
    assert_eq!(w.mother.close_ref(), Err(CoordinatorError::Busy));
    assert!(!w
        .journal
        .lines()
        .iter()
        .any(|l| l.contains("session_closed")));
    held.release();
    assert_eq!(working.join().unwrap().unwrap().to, Building);

    let other = w.approved(contract());
    assert_eq!(w.mother.close_ref(), Ok(()));
    let lines = w.journal.lines();
    assert!(lines.last().unwrap().contains("session_closed"));
    // A closed coordinator acknowledges nothing and journals nothing.
    assert_eq!(w.mother.close_ref(), Err(CoordinatorError::Closed));
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Closed));
    assert_eq!(w.mother.advance(other), Err(CoordinatorError::Closed));
    assert_eq!(w.mother.stop(id), Err(CoordinatorError::Closed));
    assert_eq!(w.mother.resume(id), Err(CoordinatorError::Closed));
    assert_eq!(
        w.mother.draft(contract()).err(),
        Some(CoordinatorError::Closed)
    );
    assert_eq!(w.mother.state(id), Some(Building), "reads still answer");
    assert_eq!(w.journal.lines(), lines);
}

// ---------------------------------------------------------------- replay

pub(super) fn forged(lines: &[String], mission: MissionId, event: Event) -> Vec<String> {
    let entries = crate::journal::verify(lines).unwrap();
    let prev = entries
        .last()
        .map_or(crate::journal::GENESIS.to_string(), |e| e.hash.clone());
    let mut out = lines.to_vec();
    out.push(Entry::chained(entries.len() as u64, &prev, mission, event).to_line());
    out
}

pub(super) fn opens(lines: Vec<String>) -> Result<World, CoordinatorError> {
    let journal = MemoryJournal::new();
    journal.set_lines(lines);
    open_on(
        journal.clone(),
        journal,
        Arc::new(ManualClock::default()),
        keyed(),
    )
}

pub(super) fn refused(lines: Vec<String>, what: &str) {
    assert!(
        matches!(
            opens(lines).err(),
            Some(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
        ),
        "{what}"
    );
}

pub(super) fn held_event() -> Event {
    Event::Held {
        reason: "unclean_shutdown".into(),
    }
}

#[test]
fn m2as_replay_refuses_session_and_hold_records_the_coordinator_would_not_write() {
    let w = world();
    let approved = w.approved(contract());
    let investigating = reach(&w, Investigating);
    let stopped = w.mission_in(Stopped);
    let base = w.journal.lines();
    let start = Event::Transition {
        trigger: Trigger::Start,
    };
    let stop = Event::Transition {
        trigger: Trigger::Stop {
            reason: "owner".into(),
        },
    };

    // Positive controls: what the coordinator does write opens.
    let held = forged(&base, approved, held_event());
    let w2 = opens(held.clone()).expect("a hold opens");
    assert_eq!(
        w2.mother.held(approved).as_deref(),
        Some("unclean_shutdown")
    );
    opens(forged(&held, approved, stop.clone())).expect("a held mission stops");
    let resumed = forged(&held, approved, Event::Resumed);
    opens(forged(&resumed, approved, start.clone())).expect("a resumed mission starts");
    let closed = forged(&base, SESSION, Event::SessionClosed);
    opens(forged(&closed, SESSION, Event::SessionOpened)).expect("close, then open");

    // Session records.
    refused(
        forged(&[], SESSION, Event::SessionClosed),
        "a close with no session open",
    );
    refused(
        forged(&closed, SESSION, Event::SessionClosed),
        "a close twice",
    );
    refused(
        forged(&closed, approved, start.clone()),
        "a record after a clean close",
    );
    refused(
        forged(&base, approved, Event::SessionOpened),
        "a session record naming a mission",
    );
    refused(
        forged(&base, approved, Event::SessionClosed),
        "a session close naming a mission",
    );

    // Holds and resumes.
    refused(
        forged(&base, stopped, held_event()),
        "a hold of an ended mission",
    );
    refused(forged(&held, approved, held_event()), "a hold twice");
    refused(
        forged(&base, approved, Event::Resumed),
        "a resume of a mission not held",
    );
    refused(
        forged(&base, SESSION, held_event()),
        "a hold naming the session",
    );
    refused(
        forged(&held, approved, start),
        "a transition other than Stop while held",
    );
    let held_working = forged(&base, investigating, held_event());
    let seq = held_working.len() as u64;
    refused(
        forged(
            &held_working,
            investigating,
            Event::LaneInvoked {
                invocation: InvocationId {
                    mission: investigating,
                    seq,
                },
                identity: keyed()[0].identity(),
                input_hash: "00".repeat(32),
            },
        ),
        "a lane call while held",
    );
    refused(
        forged(
            &held_working,
            investigating,
            Event::EffectDispatched {
                effect: 0,
                kind: EffectKind::Network,
                request_hash: "00".repeat(32),
            },
        ),
        "an effect while held",
    );
}

#[test]
fn m2as_a_journal_from_before_sessions_is_an_unclean_shutdown() {
    // Strip the session records and re-chain: a journal an earlier Mother
    // wrote, which never records a clean close.
    let w = world();
    let ids: Vec<(MissionState, MissionId)> = unfinished()
        .into_iter()
        .map(|s| (s, reach(&w, s)))
        .collect();
    let lanes = w.keyed_calls();
    let mut lines: Vec<String> = Vec::new();
    // Lane calls are named by their journal sequence: renumber them.
    let mut moved = std::collections::BTreeMap::new();
    let mut renumbered = |invocation: InvocationId, seq: Option<u64>| {
        if let Some(seq) = seq {
            moved.insert(invocation.seq, seq);
        }
        InvocationId {
            seq: moved[&invocation.seq],
            ..invocation
        }
    };
    for entry in crate::journal::verify(&w.journal.lines()).unwrap() {
        let seq = lines.len() as u64;
        let event = match entry.event {
            Event::LaneInvoked {
                invocation,
                identity,
                input_hash,
            } => Event::LaneInvoked {
                invocation: renumbered(invocation, Some(seq)),
                identity,
                input_hash,
            },
            Event::LaneReturned {
                invocation,
                output_hash,
            } => Event::LaneReturned {
                invocation: renumbered(invocation, None),
                output_hash,
            },
            Event::LaneOvertaken { invocation } => Event::LaneOvertaken {
                invocation: renumbered(invocation, None),
            },
            other => other,
        };
        if entry.mission != SESSION {
            lines = forged(&lines, entry.mission, event);
        }
    }
    let journal = MemoryJournal::new();
    journal.set_lines(lines);
    let w = World {
        journal,
        port: Arc::new(RecordingPort::default()),
        ..w
    };
    let w = crash(&w);
    for (state, id) in ids {
        assert_held(&w, id, state, lanes);
    }
}
