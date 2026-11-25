//! SB-3E CELL-R-A (SB3E-R265): the Stop-first schedule of race A, and its name.
//! A run has two cancellers: the bound Stop's `hands-stop` worker (`stop_all`)
//! and the advancing worker (`SessionDoor::cancel_run` -> `cancel_one`); the
//! first to take the runs lock writes the record. The barrier picks Stop: a
//! second pause at `door.cancel_run` (before any lock), armed while the sink's
//! hold stands; the sink is released, and at the worker's acknowledgement the
//! Stop's settle is awaited, then the door released. Harness composition until
//! M-5b (R216 F1). Door-first has no route point and is not forced (docs 14.26).
use super::s5::{attempted, d_lane, read_identified, Ack, Facts};
use super::{Fixture, Opened, PATIENCE};
use nexus_governed_control::authority::evidence::{EvidencePhase, EvidenceRecord};
use nexus_governed_control::broker::route_seams as s1;
use nexus_governed_control::host_session::stop::{StopObservation, StopPhase};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// The barrier's point: `SessionDoor::cancel_run`'s first statement.
const DOOR: &str = "door.cancel_run";
/// The inverse controls' switch (`unforced`): the barrier left out.
static UNFORCED: AtomicBool = AtomicBool::new(false);
/// The running cell's barrier facts, until its `Reading` closes (`taken`).
static SCHED: Mutex<Option<Sched>> = Mutex::new(None);
/// The barrier's facts, taken once, for the reader's facts (`s5::Read`).
pub(super) fn taken() -> Option<Sched> {
    SCHED.lock().unwrap().take()
}
/// What the barrier itself saw. It names nothing alone (`named`).
#[derive(Clone, Debug)]
pub(super) struct Sched {
    /// The acknowledgement at `door.cancel_run`, if one arrived.
    door: Option<s1::Entry>,
    /// The bound Stop's workers had finished while the door stood held.
    settled: Result<(), String>,
    /// Each stop event at the release: its generation and, once finished,
    /// the runs its `stop_all` cancelled (the Stop's own receipt).
    receipts: Vec<(u64, Option<usize>)>,
    /// When the door hold was released.
    released: Instant,
}
/// SB3E-R265: the schedule is forced exactly where the row's frozen witness
/// names one; every other row runs as it did.
fn scheduled((sink, dim): (&str, &str), value: &str) -> bool {
    let row = super::witness::frozen(sink, dim, value);
    row.is_some_and(|w| w.split(';').any(|p| p.starts_with("sched=")))
}
/// The Stop-first barrier. `sink` is the row's own standing hold; `settle`
/// is s5's settle barrier. Every wait is bounded by `PATIENCE`.
pub(super) fn stop_first<H>(
    f: &Fixture,
    o: &Opened,
    sink: H,
    settle: fn(&Opened) -> Result<(), String>,
    (row, value): ((&str, &str), &str),
) {
    if UNFORCED.load(Ordering::SeqCst) || !scheduled(row, value) {
        return drop(sink);
    }
    let door = f.pause(DOOR);
    drop(sink);
    let ack = door.acknowledged.recv_timeout(PATIENCE / 2).ok();
    let settled = match ack {
        Some(_) => settle(o),
        None => Err("no door.cancel_run acknowledgement".into()),
    };
    let cancelled = |e: &StopObservation| match &e.phase {
        StopPhase::Finished(r) => Some(r.cancelled),
        _ => None,
    };
    let events = o.session.stop_events().unwrap_or_default();
    let receipts = events.iter().map(|e| (e.event.generation(), cancelled(e)));
    let receipts = receipts.collect();
    let released = Instant::now();
    drop(door);
    *SCHED.lock().unwrap() = Some(Sched {
        door: ack,
        settled,
        receipts,
        released,
    });
}
fn detail(r: &EvidenceRecord, k: &str) -> String {
    let found = r.detail.iter().find(|(key, _)| key == k);
    found.map_or("-".into(), |(_, x)| x.clone())
}
fn at(a: &Ack) -> (std::thread::ThreadId, Instant) {
    match a {
        Ack::S1(e) => (e.thread, e.instant),
        Ack::S5(e) => (e.thread, e.instant),
        Ack::S7(e) => (e.thread, e.instant),
    }
}
/// SB3E-R265.2 F2 (soundness): the schedule's name, from raw facts only, all
/// of them: the barrier's acknowledgement is `door.cancel_run`'s, of S and
/// RUN, on the thread of the sink's own acknowledgement (the advancing
/// worker) and is the trace's own entry; the bound Stop had settled while it
/// stood (alone that proves nothing: `settled` is Ok with no stop event);
/// exactly one `emergency_stop` commit (S1 `store.commit.result`) precedes
/// the release; that record says it cancelled one run; the Stop's receipt
/// says so too; and the door wrote no `run_cancelled` of the run. With no
/// barrier facts: `unscheduled`.
pub(super) fn named(f: &Facts) -> String {
    let Some(s) = f.read.as_ref().and_then(|r| r.sched.as_ref()) else {
        return "unscheduled".into();
    };
    let (worker, run) = (f.acks.last().map(|a| at(a).0), f.claim.map(|c| c.0));
    let door = s.door.as_ref().is_some_and(|d| {
        let keyed = (d.point, d.note, d.session) == (DOOR, false, Some(f.session));
        keyed && run.is_some() && d.run == run && Some(d.thread) == worker && f.broker.contains(d)
    });
    let stop = |r: &&EvidenceRecord| r.phase == EvidencePhase::EmergencyStop;
    let by_stop = f.native.iter().filter(stop);
    let by_stop: Vec<String> = by_stop.map(|r| detail(r, "runs_cancelled")).collect();
    let run = run.map(|r| r.to_string());
    let ours = |r: &&EvidenceRecord| r.phase == EvidencePhase::RunCancelled && r.run == run;
    let by_door = f.native.iter().filter(ours).count();
    let commit = |e: &&s1::Entry| {
        e.point == "store.commit.result" && !e.note && e.phase.as_deref() == Some("emergency_stop")
    };
    let commits: Vec<&s1::Entry> = f.broker.iter().filter(commit).collect();
    let committed = commits.len() == 1 && commits.iter().all(|e| e.instant < s.released);
    let receipts: Vec<Option<usize>> = s.receipts.iter().map(|r| r.1).collect();
    let all = [
        door,
        s.settled.is_ok(),
        committed,
        by_stop == ["1"],
        receipts == [Some(1)],
        by_door == 0,
    ];
    if all.iter().all(|x| *x) {
        return "stop_first".into();
    }
    format!("unforced{all:?}")
}
/// R255.8 P8: the raw records behind the name, for the parent's log.
pub(super) fn raw(f: &Facts) -> String {
    let stops = |r: &EvidenceRecord| (r.phase.as_str(), detail(r, "runs_cancelled"));
    let native: Vec<(&str, String)> = f.native.iter().map(stops).collect();
    let sched = f.read.as_ref().and_then(|r| r.sched.as_ref());
    format!("sched={} barrier={sched:?} native={native:?}", named(f))
}
/// The typed Stop of each sink's group, and C1 (measured, Step 0 section 4).
const STOPPED: [&str; 3] = [
    "Err(Stopped)",
    "Stop { reason: EmergencyStop }",
    "Err(Invalidated)",
];
fn drive(f: &Fixture, sink: &'static str) -> Facts {
    let row = (sink, "ingress");
    let typed = match sink {
        "call_slot_reserve" | "post_record_recheck" | "record_first_classify" => ("EOF", "04"),
        _ => ("CR", "stop0d"),
    };
    match sink {
        "inference_binding" => read_identified(f, row, typed, (STOPPED[0], STOPPED[1])),
        "attempt_ticket" | "call_slot_reserve" | "meter_settle" => {
            attempted(f, row, (typed.0, Some(typed.1)), STOPPED[0])
        }
        _ => d_lane(f, row, (typed.0, Some(typed.1)), STOPPED),
    }
}
/// The inverse control's drive: the sink's typed Stop with the barrier left
/// out. Its schedule is never named (`unscheduled`).
pub(super) fn unforced(f: &Fixture, sink: &'static str) -> Facts {
    UNFORCED.store(true, Ordering::SeqCst);
    drive(f, sink)
}
/// The ledger control's drive (R202 S1): the forced schedule, and one stray
/// non-Stop record in the window.
pub(super) fn stray(f: &Fixture, sink: &'static str) -> Facts {
    let mut facts = drive(f, sink);
    let mut stray = facts.native.first().cloned().expect("the Stop record");
    stray.phase = EvidencePhase::ModelRegistered;
    facts.native.push(stray);
    facts
}
/// F2 at rule level, over a real typed bind run under the barrier: the name
/// needs the barrier's facts and production's records together; without the
/// barrier's facts it is `unscheduled`, and each of the eight single-clause
/// inversions loses it.
#[test]
fn route_sched_names_only_a_forced_stop_first() {
    super::child("sched::route_sched_names_only_a_forced_stop_first", |f| {
        let facts = drive(f, "inference_binding");
        assert!(facts.findings.is_empty(), "{:?}", facts.findings);
        assert_eq!(named(&facts), "stop_first", "{}", raw(&facts));
        let (sink, since) = facts
            .acks
            .last()
            .map(at)
            .expect("the sink's acknowledgement");
        let with = |edit: &dyn Fn(&mut Facts, &mut Sched)| {
            let mut x = facts.clone();
            let read = x.read.as_mut().expect("the reader");
            let mut s = read.sched.take().expect("the barrier's facts");
            edit(&mut x, &mut s);
            x.read.as_mut().expect("the reader").sched = Some(s);
            named(&x)
        };
        let mut bare = facts.clone();
        bare.read.as_mut().expect("the reader").sched = None;
        assert_eq!(named(&bare), "unscheduled", "no barrier facts");
        let here = std::thread::current().id();
        assert_ne!(here, sink);
        let other = nexus_governed_control::authority::ids::RunId::parse(&format!(
            "run-{}",
            "0".repeat(32)
        ));
        let cases: [(&str, &dyn Fn(&mut Facts, &mut Sched)); 8] = [
            ("no acknowledgement", &|_, s| s.door = None),
            ("the harness's thread", &|_, s| {
                s.door.as_mut().unwrap().thread = here
            }),
            ("another run", &|_, s| s.door.as_mut().unwrap().run = other),
            ("not settled", &|_, s| s.settled = Err("settle".into())),
            ("released before the Stop's commit", &|_, s| {
                s.released = since
            }),
            ("the Stop cancelled no run", &|x, _| {
                let stop = x
                    .native
                    .iter_mut()
                    .find(|r| r.phase == EvidencePhase::EmergencyStop);
                let detail = &mut stop.expect("the Stop record").detail;
                detail
                    .iter_mut()
                    .find(|d| d.0 == "runs_cancelled")
                    .unwrap()
                    .1 = "0".into();
            }),
            ("the receipt cancelled no run", &|_, s| {
                s.receipts = vec![(1, Some(0))]
            }),
            ("the door's own run_cancelled", &|x, _| {
                let mut door = x.native.first().cloned().expect("the Stop record");
                door.phase = EvidencePhase::RunCancelled;
                door.run = x.claim.map(|c| c.0.to_string());
                x.native.push(door);
            }),
        ];
        for (what, edit) in cases {
            let name = with(edit);
            assert!(name.starts_with("unforced["), "{what}: named {name}");
            println!("WITNESS sched rule: {what}: {name}");
        }
    });
}
