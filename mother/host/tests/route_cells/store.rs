//! U4a store faults (design 4.2, 4.2.1): S2 refuses one record before its
//! insert; the external shim fails one WAL fsync of one thread inside an
//! ordinary COMMIT. Each test asserts the returned outcome and latch state
//! of its record class, and reads consumption back by kind, record, phase,
//! thread and file: S2's and the COMMIT error's notes through
//! `drain_trace`, the shim's own log through `std::fs`. No new accessor.
use super::*;
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::host_session::SessionError;
use nexus_governed_control::inference::ModelCallOutcome;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::thread::ThreadId;

const WAL: &str = "home/broker/p3-evidence/evidence.sqlite3-wal";
const LOG: &str = "ROUTE_SHIM_LOG";
const CONTROL: &str = "ROUTE_SHIM_CONTROL";
pub(super) const SHIM_SHA256: &str = "ROUTE_SHIM_SHA256";

/// The parent's half of a shim child: compile the shim at test time in the
/// child's owned root (`/usr` gcc is a mutable host input, so the witness
/// is the built library's digest), and preload it into that child only.
fn shim_env(root: &Path) -> Vec<(&'static str, PathBuf)> {
    let (source, library) = (root.join("route_fsync.c"), root.join("route_fsync.so"));
    std::fs::write(&source, include_str!("route_fsync.c")).unwrap();
    let built = std::process::Command::new("cc")
        .args(["-shared", "-fPIC", "-Wall", "-Wextra", "-Werror", "-o"])
        .arg(&library)
        .arg(&source)
        .arg("-ldl")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(built.status.success(), "the shim builds: {stderr}");
    // SB3E-R32.1: the digest, taken before the child starts, which the child
    // must report for the library it actually has mapped.
    let digest = PathBuf::from(sha256_hex(&library));
    let (log, control) = (root.join("shim.log"), root.join("shim.control"));
    vec![
        ("LD_PRELOAD", library),
        (LOG, log),
        (CONTROL, control),
        (SHIM_SHA256, digest),
    ]
}
fn sha256_hex(path: &Path) -> String {
    let digest = Sha256::digest(std::fs::read(path).unwrap());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
/// The child's half of the witness: the preloaded library is mapped here,
/// and its digest is the parent's; reported on the stdout the parent asserts.
pub(super) fn shim_witness() {
    let library = std::env::var_os("LD_PRELOAD").map(PathBuf::from).unwrap();
    let library = std::fs::canonicalize(library).unwrap();
    let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
    let mut mapped = maps.lines().filter_map(|l| l.split_whitespace().nth(5));
    assert!(
        mapped.any(|p| Path::new(p) == library),
        "the shim is not mapped"
    );
    let hex = sha256_hex(&library);
    assert_eq!(std::env::var(SHIM_SHA256).ok(), Some(hex.clone()));
    println!("ROUTE SHIM SHA256 {hex}");
}
/// The parent's check of the child's report: exactly one line naming the
/// 64-hex digest it took itself; the parent records it in its own output.
pub(super) fn shim_witnessed(want: &str, stdout: &str) {
    let hex = want.len() == 64 && want.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let line = format!("ROUTE SHIM SHA256 {want}\n");
    assert!(
        hex && stdout.matches(&line).count() == 1,
        "shim witness {want}: {stdout}"
    );
    let reports = stdout.matches("ROUTE SHIM SHA256 ").count();
    assert_eq!(reports, 1, "one shim witness: {stdout}");
    println!("ROUTE SHIM SHA256 {want}");
}
/// This thread's OS id, as the shim sees it (`gettid`).
fn os_tid() -> String {
    let link = std::fs::read_link("/proc/thread-self").unwrap();
    link.file_name().unwrap().to_string_lossy().into_owned()
}
fn wal(f: &Fixture) -> String {
    let path = std::fs::canonicalize(f.0.join(WAL)).unwrap();
    path.to_string_lossy().into_owned()
}
/// While the route is paused: name the thread and the WAL to the shim by
/// renaming a staged control file into place. No evidence API is called.
fn arm_shim(f: &Fixture, tid: &str) {
    let control = PathBuf::from(std::env::var_os(CONTROL).expect("a shim child"));
    let staged = control.with_extension("staged");
    std::fs::write(&staged, format!("{tid} {}\n", wal(f))).unwrap();
    std::fs::rename(&staged, &control).unwrap();
}
fn shim_log() -> String {
    let log = std::env::var_os(LOG).map(std::fs::read_to_string);
    log.map(Result::unwrap).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mech {
    S2,
    Shim,
}
/// The one consumption a fault must have: its mechanism, the driver thread
/// (its OS id for the shim, its `ThreadId` for the entries), and the record
/// (an ordinary append of `phase` and `commitment`).
struct Target {
    mech: Mech,
    tid: String,
    thread: ThreadId,
    phase: &'static str,
    commitment: Option<&'static str>,
}
/// Design 4.2.1: exactly one consumption, by the target's mechanism, of an
/// ordinary append of the target record; the shim's on the target thread
/// and the store's WAL, inside a COMMIT (its error note). Anything else
/// names why a cell must fail.
fn consumed(trace: &[Entry], log: &str, wal: &str, t: &Target) -> Result<(), &'static str> {
    let notes = |point| {
        trace
            .iter()
            .filter(move |e: &&Entry| e.note && e.point == point)
    };
    let fired: Vec<Vec<&str>> = log
        .lines()
        .filter(|l| l.starts_with("FIRED "))
        .map(|l| l.splitn(4, ' ').collect())
        .collect();
    let (refused, errors) = ("store.refused", "store.commit.result");
    let entries: Vec<&Entry> = match t.mech {
        Mech::S2 if notes(errors).count() + fired.len() > 0 => return Err("another mechanism"),
        Mech::S2 => notes(refused).collect(),
        Mech::Shim if notes(refused).count() > 0 => return Err("another mechanism"),
        Mech::Shim => {
            match fired.as_slice() {
                [] => return Err("unused"),
                [one] if one[2] != t.tid => return Err("wrong thread"),
                [one] if one[3] != wal => return Err("wrong file"),
                [_] => {}
                _ => return Err("repeated"),
            }
            let errors: Vec<&Entry> = notes(errors).collect();
            if errors.is_empty() {
                return Err("fired outside a COMMIT");
            }
            errors
        }
    };
    let entry = match entries.as_slice() {
        [] => return Err("unused"),
        [one] => one,
        _ => return Err("repeated"),
    };
    if entry.thread != t.thread {
        return Err("wrong thread");
    }
    if entry.kind != Some("ordinary") {
        return Err("wrong kind");
    }
    if entry.phase.as_deref() != Some(t.phase) {
        return Err("wrong phase");
    }
    if entry.commitment.as_deref() != t.commitment {
        return Err("wrong record");
    }
    Ok(())
}
/// Every wrong consumption, made from a real recorded one, fails the check.
fn rejects_wrong_consumption(trace: &[Entry], log: &str, wal: &str, t: &Target) {
    let point = match t.mech {
        Mech::S2 => "store.refused",
        Mech::Shim => "store.commit.result",
    };
    let at = trace.iter().position(|e| e.note && e.point == point);
    let at = at.expect("the real consumption");
    let with = |change: &dyn Fn(&mut Vec<Entry>)| {
        let mut changed = trace.to_vec();
        change(&mut changed);
        consumed(&changed, log, wal, t)
    };
    assert_eq!(
        with(&|v| v[at].kind = Some("owed_flush")),
        Err("wrong kind")
    );
    assert_eq!(
        with(&|v| v[at].phase = Some("gate_refused".into())),
        Err("wrong phase")
    );
    assert_eq!(
        with(&|v| v[at].commitment = Some("c".into())),
        Err("wrong record")
    );
    assert_eq!(with(&|v| v.push(v[at].clone())), Err("repeated"));
    let other = std::thread::spawn(|| std::thread::current().id())
        .join()
        .unwrap();
    assert_eq!(with(&|v| v[at].thread = other), Err("wrong thread"));
    let missing = match t.mech {
        Mech::S2 => "unused",
        Mech::Shim => "fired outside a COMMIT",
    };
    assert_eq!(with(&|v| drop(v.remove(at))), Err(missing));
    if t.mech == Mech::Shim {
        let fired = log.lines().find(|l| l.starts_with("FIRED ")).unwrap();
        let swap = |from: &str, to: &str| log.replace(fired, &fired.replacen(from, to, 1));
        let thread = swap(&format!(" {} ", t.tid), " 1 ");
        assert_eq!(
            consumed(trace, &log.replace(fired, ""), wal, t),
            Err("unused")
        );
        let twice = format!("{log}{fired}\n");
        assert_eq!(consumed(trace, &twice, wal, t), Err("repeated"));
        assert_eq!(consumed(trace, &thread, wal, t), Err("wrong thread"));
        assert_eq!(
            consumed(trace, &swap("-wal", ""), wal, t),
            Err("wrong file")
        );
    }
}

/// What a faulted drive returned, with its consumption target and record.
struct Faulted<T> {
    out: T,
    target: Target,
    trace: Vec<Entry>,
    log: String,
}
/// Drive `run` on its own thread with one fault on `record`. S2 is armed
/// while the route is held at the record's `point` (or before the drive,
/// for the close statement), before the evidence mutex is taken. The shim
/// is armed for the driver thread while it is held at its ordinary
/// `store.commit`, inside the mutex, where no evidence API is called.
fn faulted<T: Send>(
    f: &Fixture,
    evidence: &PrimaryEvidence,
    (mech, point): (Mech, Option<&'static str>),
    (phase, commitment): (&'static str, Option<&'static str>),
    run: impl FnOnce() -> T + Send,
) -> Faulted<T> {
    let _recording = arm("reg.reserve", Action::Trace);
    drain_trace();
    let arm_fault = || match mech {
        Mech::S2 => {
            evidence.route_refuse_next(phase, commitment);
            None
        }
        Mech::Shim => Some(f.pause("store.commit")),
    };
    let held = point.map(|p| f.pause(p));
    let mut commit = if held.is_none() { arm_fault() } else { None };
    let (out, (tid, thread)) = std::thread::scope(|s| {
        let (driver, named) = std::sync::mpsc::channel();
        let running = s.spawn(move || {
            driver
                .send((os_tid(), std::thread::current().id()))
                .unwrap();
            run()
        });
        let driver = named.recv_timeout(PATIENCE).unwrap();
        if let Some(held) = held {
            held.acknowledged.recv_timeout(PATIENCE).unwrap();
            commit = arm_fault();
            drop(held);
        }
        if let Some(commit) = commit {
            let entry = commit.acknowledged.recv_timeout(PATIENCE).unwrap();
            let held_at = (
                entry.kind,
                entry.phase.as_deref(),
                entry.commitment.as_deref(),
            );
            assert_eq!(held_at, (Some("ordinary"), Some(phase), commitment));
            arm_shim(f, &driver.0);
            drop(commit);
        }
        (running.join().unwrap(), driver)
    });
    let target = Target {
        mech,
        tid,
        thread,
        phase,
        commitment,
    };
    let (trace, log) = (drain_trace(), shim_log());
    Faulted {
        out,
        target,
        trace,
        log,
    }
}
impl<T> Faulted<T> {
    fn consumed(&self, f: &Fixture) -> Result<(), &'static str> {
        consumed(&self.trace, &self.log, &wal(f), &self.target)
    }
}
/// A registered model, an original door, and its File run admitted to it.
fn admitted(o: &Opened, root: &Path) -> (SessionDoor, RunId) {
    let (endpoint, _) = fake_ollama();
    o.session.register_local_model(&endpoint, spec()).unwrap();
    let (door, run, _) = o.door(root);
    let ask = MissionConfirmation {
        contract_hash: "ab".repeat(32),
        request: "msn-000001/1/1".into(),
        summary: vec!["Route cell mission.".into()],
        page_facts: None,
    };
    door.confirm_mission(&ask).unwrap();
    let (hash, label) = (&ask.contract_hash, &ask.request);
    door.admit_files(run, hash, label).unwrap();
    let identity = ModelIdentity::from_spec(&spec()).unwrap().digest();
    door.admit_inference(run, hash, label, identity).unwrap();
    (door, run)
}
/// One model call through the public door; its end is the attempt record.
fn call_model(door: &SessionDoor, run: RunId) -> ModelCallOutcome {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    let call = ModelRequest::new("system".into(), "prompt".into(), schema).unwrap();
    let (owner, meter) = (InvocationOwner::new(), Meter(AtomicU32::new(0)));
    let outcome = door.model_call(run, &call, &meter, &Never, &owner.observer());
    outcome.unwrap()
}
fn latched_owed(evidence: &PrimaryEvidence) -> (bool, usize) {
    let health = evidence.health();
    (health.latched, health.owed)
}
const GATE: (Option<&str>, (&str, Option<&str>)) =
    (Some("model.record.before"), ("model_registered", None));
const END: (Option<&str>, (&str, Option<&str>)) =
    (Some("model.attempt.record.before"), ("model_call", None));
const CLOSE: (Option<&str>, (&str, Option<&str>)) = (None, ("session_closed", None));

/// The gate record (`model_registered`): returned as its registration's error.
fn gate(f: &Fixture, mech: Mech) -> (Result<(), SessionError>, (bool, usize)) {
    let o = f.open();
    let (endpoint, _) = fake_ollama();
    let register = || {
        o.session
            .register_local_model(&endpoint, spec())
            .map(|_| ())
    };
    let r = faulted(f, &o.evidence, (mech, GATE.0), GATE.1, register);
    assert_eq!(r.consumed(f), Ok(()));
    (r.out, latched_owed(&o.evidence))
}
/// The end record (`model_call`) of one attempt.
fn end(f: &Fixture, mech: Mech) -> (EndStatus, bool, (bool, usize)) {
    let o = f.open();
    let (door, run) = admitted(&o, &f.0);
    let r = faulted(f, &o.evidence, (mech, END.0), END.1, || {
        call_model(&door, run)
    });
    assert_eq!(r.consumed(f), Ok(()));
    let called = r.out.result().is_ok();
    (r.out.record(), called, latched_owed(&o.evidence))
}

/// 4.2.1 gate, S2: `Err(EvidenceUnavailable)` returned as
/// `evidence_unavailable`; nothing owed, not latched.
#[test]
fn route_s2_gate_is_refused_unlatched_and_owes_nothing() {
    child(
        "store::route_s2_gate_is_refused_unlatched_and_owes_nothing",
        |f| {
            let refused = Err(SessionError::Backend("evidence_unavailable"));
            assert_eq!(gate(f, Mech::S2), (refused, (false, 0)));
        },
    );
}
/// 4.2.1 gate, COMMIT error (shim): the same error; the store latched.
#[test]
fn route_shim_gate_is_refused_and_latched() {
    child_with(
        "store::route_shim_gate_is_refused_and_latched",
        shim_env,
        |f| {
            let refused = Err(SessionError::Backend("evidence_unavailable"));
            assert_eq!(gate(f, Mech::Shim), (refused, (true, 0)));
        },
    );
}
/// 4.2.1 end, S2: `EndStatus::Owed`, not latched; the call's own result
/// stands. First a wrong record (another commitment of the same phase) and a
/// wrong phase, each armed at the same point: each goes unused and the end
/// is recorded.
#[test]
fn route_s2_end_is_owed_and_unlatched() {
    child("store::route_s2_end_is_owed_and_unlatched", |f| {
        let o = f.open();
        let (door, run) = admitted(&o, &f.0);
        for decoy in [(END.1 .0, Some("another-commitment")), CLOSE.1] {
            let call = || call_model(&door, run);
            let r = faulted(f, &o.evidence, (Mech::S2, END.0), decoy, call);
            assert!(matches!(r.out.record(), EndStatus::Recorded { .. }));
            assert_eq!(r.consumed(f), Err("unused"), "{decoy:?} goes unused");
        }
        let r = faulted(f, &o.evidence, (Mech::S2, END.0), END.1, || {
            call_model(&door, run)
        });
        assert_eq!(
            (r.out.record(), r.out.result().is_ok()),
            (EndStatus::Owed, true)
        );
        assert_eq!(latched_owed(&o.evidence), (false, 1));
        assert_eq!(r.consumed(f), Ok(()));
    });
}
/// 4.2.1 end, COMMIT error (shim): `EndStatus::Uncertain`; latched.
#[test]
fn route_shim_end_is_uncertain_and_latched() {
    child_with(
        "store::route_shim_end_is_uncertain_and_latched",
        shim_env,
        |f| {
            assert_eq!(end(f, Mech::Shim), (EndStatus::Uncertain, true, (true, 0)));
        },
    );
}
/// 4.2.1 close statement, S2: as End, `Owed` and not latched. The next close
/// statement pays the owed record and is not refused again (one-shot); the
/// real consumption, mutated, fails each wrong-consumption check.
#[test]
fn route_s2_close_statement_is_owed_and_unlatched() {
    child(
        "store::route_s2_close_statement_is_owed_and_unlatched",
        |f| {
            let o = f.open();
            let close = || o.evidence.close_session();
            let mut r = faulted(f, &o.evidence, (Mech::S2, CLOSE.0), CLOSE.1, close);
            assert_eq!(r.out, EndStatus::Owed);
            assert_eq!(latched_owed(&o.evidence), (false, 1));
            assert!(matches!(
                o.evidence.close_session(),
                EndStatus::Recorded { .. }
            ));
            assert_eq!(latched_owed(&o.evidence), (false, 0));
            r.trace.extend(drain_trace());
            assert_eq!(r.consumed(f), Ok(()), "consumed once, not repeated");
            rejects_wrong_consumption(&r.trace, &r.log, &wal(f), &r.target);
        },
    );
}
/// 4.2.1 close statement, COMMIT error (shim): as End, `Uncertain` and
/// latched; the real consumption, mutated, fails each check.
#[test]
fn route_shim_close_statement_is_uncertain_and_latched() {
    child_with(
        "store::route_shim_close_statement_is_uncertain_and_latched",
        shim_env,
        |f| {
            let o = f.open();
            let close = || o.evidence.close_session();
            let r = faulted(f, &o.evidence, (Mech::Shim, CLOSE.0), CLOSE.1, close);
            assert_eq!(
                (r.out, latched_owed(&o.evidence)),
                (EndStatus::Uncertain, (true, 0))
            );
            assert_eq!(r.consumed(f), Ok(()));
            rejects_wrong_consumption(&r.trace, &r.log, &wal(f), &r.target);
        },
    );
}
/// SB3E-R12.1 owed flush, with the controls of 4.2.1, each half on one
/// record (the close statement: the same phase and commitment). The
/// `store.commit` hold, offered to that record's owed flush, is passed by
/// it and acknowledged by the next ordinary append of the record. S2,
/// offered to the owed flush, stays armed past it and refuses that append.
#[test]
fn route_owed_flush_leaves_the_ordinary_hold_and_s2_unconsumed() {
    child(
        "store::route_owed_flush_leaves_the_ordinary_hold_and_s2_unconsumed",
        |f| {
            let o = f.open();
            let close = || o.evidence.close_session();
            let owed = || {
                let r = faulted(f, &o.evidence, (Mech::S2, CLOSE.0), CLOSE.1, close);
                let owed = (r.out, latched_owed(&o.evidence));
                assert_eq!(owed, (EndStatus::Owed, (false, 1)));
                r
            };
            let commits = |trace: &[Entry]| -> Vec<(Option<&str>, Option<String>, Option<String>)> {
                let commits = trace.iter().filter(|e| e.point == "store.commit");
                commits
                    .map(|e| (e.kind, e.phase.clone(), e.commitment.clone()))
                    .collect()
            };
            let (flush, ordinary) = (Some("owed_flush"), Some("ordinary"));
            let record = |kind| (kind, Some(CLOSE.1 .0.to_string()), None);
            owed();
            let _recording = arm("reg.reserve", Action::Trace);
            drain_trace();
            let held = f.pause("store.commit");
            let closed = std::thread::scope(|s| {
                let closing = s.spawn(close);
                let entry = held.acknowledged.recv_timeout(PATIENCE).unwrap();
                let acknowledged = (entry.kind, entry.phase.as_deref(), entry.commitment);
                assert_eq!(
                    acknowledged,
                    (ordinary, Some(CLOSE.1 .0), CLOSE.1 .1.map(str::to_owned))
                );
                drop(held);
                closing.join().unwrap()
            });
            assert!(matches!(closed, EndStatus::Recorded { .. }));
            assert_eq!(latched_owed(&o.evidence), (false, 0));
            assert_eq!(commits(&drain_trace()), [record(flush), record(ordinary)]);
            owed();
            let r = owed();
            assert_eq!(r.consumed(f), Ok(()), "unused by the flush, then consumed");
            assert_eq!(
                commits(&r.trace),
                [record(flush)],
                "the flush committed past S2"
            );
        },
    );
}
/// 4.2.1 control: S2 armed after its branch (while the record's own append
/// is held at `store.commit`, past BEGIN) goes unused, and its cell fails.
#[test]
fn route_s2_armed_after_its_branch_goes_unused() {
    child("store::route_s2_armed_after_its_branch_goes_unused", |f| {
        let o = f.open();
        let (endpoint, _) = fake_ollama();
        let (phase, commitment) = GATE.1;
        let register = || {
            let thread = (os_tid(), std::thread::current().id());
            (
                thread,
                o.session
                    .register_local_model(&endpoint, spec())
                    .map(|_| ()),
            )
        };
        let _recording = arm("reg.reserve", Action::Trace);
        drain_trace();
        let held = f.pause("store.commit");
        let ((tid, thread), registered) = std::thread::scope(|s| {
            let registering = s.spawn(register);
            let entry = held.acknowledged.recv_timeout(PATIENCE).unwrap();
            assert_eq!(entry.phase.as_deref(), Some(phase));
            // It waits for the held mutex: after the branch, whatever the order.
            let arming = s.spawn(|| o.evidence.route_refuse_next(phase, commitment));
            drop(held);
            arming.join().unwrap();
            registering.join().unwrap()
        });
        assert_eq!(
            (registered, latched_owed(&o.evidence)),
            (Ok(()), (false, 0))
        );
        let (mech, trace) = (Mech::S2, drain_trace());
        let t = Target {
            mech,
            tid,
            thread,
            phase,
            commitment,
        };
        assert_eq!(consumed(&trace, "", "", &t), Err("unused"));
    });
}
/// 4.2.1 control: the shim fails the WAL fsync of a checkpoint, not of a
/// COMMIT (the close checkpoint of the last `PrimaryEvidence`, dropped on
/// this thread); its cell fails.
#[test]
fn route_shim_fault_during_a_checkpoint_is_not_a_commit() {
    child_with(
        "store::route_shim_fault_during_a_checkpoint_is_not_a_commit",
        shim_env,
        |f| {
            let o = f.open();
            o.session
                .register_local_model(&fake_ollama().0, spec())
                .unwrap();
            let _recording = arm("reg.reserve", Action::Trace);
            drain_trace();
            let (tid, thread, wal) = (os_tid(), std::thread::current().id(), wal(f));
            arm_shim(f, &tid);
            let evidence = o.evidence.clone();
            drop(o);
            assert_eq!(Arc::strong_count(&evidence), 1, "the last PrimaryEvidence");
            drop(evidence);
            let (trace, log) = (drain_trace(), shim_log());
            let fired = format!("FIRED fsync {tid} {wal}\n");
            assert_eq!(log.matches("FIRED").count(), 1, "{log}");
            assert!(log.contains(&fired), "{log}");
            let (phase, commitment) = GATE.1;
            let t = Target {
                mech: Mech::Shim,
                tid,
                thread,
                phase,
                commitment,
            };
            assert_eq!(
                consumed(&trace, &log, &wal, &t),
                Err("fired outside a COMMIT")
            );
        },
    );
}
/// The shim's own restriction to its thread: armed (while held at the close
/// statement's `store.commit`) for another live thread, it fails nothing.
#[test]
fn route_shim_fires_only_on_its_named_thread() {
    child_with(
        "store::route_shim_fires_only_on_its_named_thread",
        shim_env,
        |f| {
            let o = f.open();
            let (other, parked) = std::sync::mpsc::channel();
            let (release, idle) = std::sync::mpsc::channel::<()>();
            let bystander = std::thread::spawn(move || {
                other.send(os_tid()).unwrap();
                idle.recv_timeout(PATIENCE).ok();
            });
            let other = parked.recv_timeout(PATIENCE).unwrap();
            let _recording = arm("reg.reserve", Action::Trace);
            let held = f.pause("store.commit");
            let closed = std::thread::scope(|s| {
                let closing = s.spawn(|| (os_tid(), o.evidence.close_session()));
                held.acknowledged.recv_timeout(PATIENCE).unwrap();
                arm_shim(f, &other);
                drop(held);
                closing.join().unwrap()
            });
            release.send(()).unwrap();
            bystander.join().unwrap();
            assert!(matches!(closed.1, EndStatus::Recorded { .. }));
            assert_eq!(latched_owed(&o.evidence), (false, 0));
            let log = shim_log();
            assert!(!log.contains("FIRED") && log.contains(&format!("CALL fsync {} ", closed.0)));
        },
    );
}
/// The shim's own restriction to its file: armed for the database file, the
/// close checkpoint's WAL fsync on this thread passes and its database fsync
/// fails; a WAL target's cell fails on the wrong file.
#[test]
fn route_shim_fires_only_on_its_named_file() {
    child_with(
        "store::route_shim_fires_only_on_its_named_file",
        shim_env,
        |f| {
            let o = f.open();
            let (tid, thread, wal) = (os_tid(), std::thread::current().id(), wal(f));
            let database = wal.strip_suffix("-wal").unwrap().to_string();
            let control = PathBuf::from(std::env::var_os(CONTROL).unwrap());
            let staged = control.with_extension("staged");
            std::fs::write(&staged, format!("{tid} {database}\n")).unwrap();
            std::fs::rename(&staged, &control).unwrap();
            drop(o);
            let log = shim_log();
            let (passed, fired) = (
                format!("CALL fsync {tid} {wal}\n"),
                format!("FIRED fsync {tid} {database}\n"),
            );
            let (passed, fired) = (log.find(&passed), log.find(&fired));
            assert!(
                passed.is_some() && fired.is_some() && passed < fired,
                "{log}"
            );
            let (phase, commitment) = GATE.1;
            let t = Target {
                mech: Mech::Shim,
                tid,
                thread,
                phase,
                commitment,
            };
            assert_eq!(consumed(&[], &log, &wal, &t), Err("wrong file"));
        },
    );
}
/// The shim is one-shot: after it fails the close statement's COMMIT, the
/// same thread's next WAL fsync (the close checkpoint of the last
/// `PrimaryEvidence`, dropped there) passes.
#[test]
fn route_shim_fires_once() {
    child_with("store::route_shim_fires_once", shim_env, |f| {
        let o = f.open();
        let (evidence, wal) = (o.evidence.clone(), wal(f));
        let _recording = arm("reg.reserve", Action::Trace);
        let held = f.pause("store.commit");
        let (named, driver_tid) = std::sync::mpsc::channel();
        let (go, wait) = std::sync::mpsc::channel::<()>();
        let driver = std::thread::spawn(move || {
            named.send(os_tid()).unwrap();
            let status = evidence.close_session();
            wait.recv_timeout(PATIENCE).unwrap();
            assert_eq!(Arc::strong_count(&evidence), 1, "the last PrimaryEvidence");
            drop(evidence);
            status
        });
        let tid = driver_tid.recv_timeout(PATIENCE).unwrap();
        held.acknowledged.recv_timeout(PATIENCE).unwrap();
        arm_shim(f, &tid);
        drop(held);
        drop(o);
        go.send(()).unwrap();
        assert_eq!(driver.join().unwrap(), EndStatus::Uncertain);
        let log = shim_log();
        let fired = log.find(&format!("FIRED fsync {tid} {wal}\n"));
        let after = fired.and_then(|at| log[at..].find(&format!("CALL fsync {tid} {wal}\n")));
        assert_eq!(
            (log.matches("FIRED").count(), after.is_some()),
            (1, true),
            "{log}"
        );
    });
}
