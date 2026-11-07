//! Frozen SB-2a oracles. Q/R/C/L are N/A for Bootstrap registration.
use super::*;
use crate::inference::{ModelError, ModelPinSpec, RuntimeConfig, Think};
use crate::test_hooks::Gate;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize},
    Mutex,
};
const PATHS: [&str; 4] = ["/api/version", "/api/status", "/api/tags", "/api/show"];
const CANARY: &str = "SB2_METADATA_PRIVATE_CANARY";
pub(super) const CHAT: &str = r#"{"model":"qwen3:8b","message":{"role":"assistant","content":"{\"sb3b\":\"reply\"}"},"done":true,"done_reason":"stop","prompt_eval_count":587,"eval_count":378}"#;
pub(super) fn pin() -> ModelPinSpec {
    ModelPinSpec {
        name: "qwen3:8b".into(),
        digest: "ab".repeat(32),
        runtime_version: "0.40.13".into(),
        config: RuntimeConfig {
            num_ctx: 8192,
            temperature_milli: 0,
            seed: 7,
            think: Think::Off,
            truncate: false,
        },
    }
}
// Core-only tests use an original Bootstrap on the real pre-input cell.
fn bootstrap(core: &Arc<Core>) -> Arc<RequestScope> {
    Arc::new(RequestScope {
        core: Arc::downgrade(core),
        session: core.identity,
        request: 0,
        snapshot: 0,
        deadline: core.created + Duration::from_secs(300),
        kind: ScopeKind::Bootstrap,
        next_step: AtomicU64::new(0),
        facts: outcome::RequestFacts::default(),
        restore: outcome::RestoreFacts::default(),
        mission_deadlines: OnceLock::new(),
        mother_run: OnceLock::new(),
    })
}
#[test]
fn sb2_one_session_operation_active() {
    let seed = SessionSeed::new().unwrap();
    let q = bootstrap(&seed.core);
    let owner = model::reserve(&seed.core, &q).expect("operation reservation");
    assert!(model::reserve(&seed.core, &q).is_err());
    let o = seed.core.model.snapshot().unwrap();
    assert!(o.active && !o.owner_complete && !o.child_complete);
    assert!(!seed.core.model.ready().unwrap() && !seed.core.model.quiescent());
    owner.complete("not_required").unwrap();
    assert!(seed.core.model.quiescent());
    assert!(
        !seed.core.model.ready().unwrap(),
        "failed configured startup cannot become no-model"
    );
    assert!(
        model::reserve(&seed.core, &q).is_err(),
        "registration_attempts_eq_one"
    );
}
#[test]
fn sb2_owner_drop_never_discharges_debt() {
    let seed = SessionSeed::new().unwrap();
    let q = bootstrap(&seed.core);
    let owner = model::reserve(&seed.core, &q).expect("operation reservation");
    // Drop must not take this lock, perform I/O or clear retained ownership.
    let hold = seed.core.model.state.lock().unwrap();
    drop(owner);
    drop(hold);
    assert!(seed.failed());
    assert!(!seed.core.model.quiescent());
    let o = seed.core.model.snapshot().unwrap();
    assert!(o.uncertain && o.active && !o.owner_complete);
}
#[test]
fn sb2_unjoined_or_owed_prevents_clean_close() {
    // Real seal path: all other prerequisites satisfied; only model ownership varies.
    for active in [false, true] {
        let f = fixture();
        assert!(f.session.startup_report().ready());
        use sha2::{Digest, Sha256};
        let previous = "0".repeat(64);
        let event = r#"{"event":"session_opened"}"#;
        let mut hash = Sha256::new();
        hash.update(b"nexus-mother-journal-v2\n");
        for part in [
            previous.as_bytes(),
            &[2],
            &0u64.to_be_bytes(),
            &0u64.to_be_bytes(),
            event.as_bytes(),
        ] {
            hash.update((part.len() as u64).to_be_bytes());
            hash.update(part);
        }
        let hash = hex::encode(hash.finalize());
        std::fs::write(f.root.0.path().join("mother-journal.jsonl"), format!("{{\"format\":2,\"seq\":0,\"prev\":\"{previous}\",\"mission\":0,\"event\":{event},\"hash\":\"{hash}\"}}\n")).unwrap();
        let owner = model::reserve(&f.session.core, &f.session.bootstrap.scope).unwrap();
        let quit = f.session.ingress().accept(CommandKind::Quit, 1).unwrap();
        f.session.settle_close_resources(&quit).unwrap();
        f.session.settle_close_authority(&quit).unwrap();
        if active {
            assert!(
                f.session.seal_close(&quit).is_err(),
                "unjoined_or_owed_prevents_clean_close"
            );
        }
        owner.complete("recorded").unwrap();
        assert!(
            f.session.seal_close(&quit).is_ok(),
            "conclusive model seal positive control"
        );
    }
    let seed = SessionSeed::new().unwrap();
    let q = bootstrap(&seed.core);
    assert!(seed.core.model.quiescent());
    let producer = seed
        .ingress()
        .producer(close::ProducerKind::Startup)
        .unwrap();
    let owner = model::reserve(&seed.core, &q).expect("operation reservation");
    assert!(!seed.core.model.quiescent());
    owner.complete("recorded").unwrap();
    assert!(seed.core.model.quiescent());
    let more: Vec<_> = (0..8)
        .map(|_| seed.ingress().producer(close::ProducerKind::Work).unwrap())
        .collect();
    assert!(
        seed.ingress().producer(close::ProducerKind::Work).is_err(),
        "registration return must not join its host producer"
    );
    for p in more {
        p.joined();
    }
    producer.joined();
    let seed = SessionSeed::new().unwrap();
    let q = bootstrap(&seed.core);
    let owner = model::reserve(&seed.core, &q).unwrap();
    assert!(owner.complete("unknown").is_err());
    assert!(
        !seed.core.model.quiescent(),
        "unknown registration recording blocks seal"
    );
}
#[test]
fn sb2_same_shutdown_deadline_and_no_post_seal_model_events() {
    let seed = SessionSeed::new().unwrap();
    let q = bootstrap(&seed.core);
    let first = seed.ingress().begin_shutdown(Instant::now());
    assert_eq!(
        first,
        seed.ingress()
            .begin_shutdown(Instant::now() + Duration::from_secs(1))
    );
    seed.core.cell.session_compare(0, SEALED).unwrap();
    assert!(model::reserve(&seed.core, &q).is_err());
    assert_eq!(seed.core.model.snapshot().unwrap().operation, "none");
}

pub(super) struct Fixture {
    pub(super) session: Arc<HostSession>,
    pub(super) root: crate::harness_tests::TempRoot,
    pub(super) primary: Arc<crate::evidence_store::PrimaryEvidence>,
    _selected: crate::harness_tests::TempRoot,
}
pub(super) fn fixture() -> Fixture {
    assert!(crate::files::make_non_dumpable());
    let root = crate::harness_tests::temp_root("sb2a-registration");
    let selected = crate::harness_tests::temp_root("sb2a-workspace");
    let workspace = selected.0.path().join("selected/workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let primary = crate::evidence_store::PrimaryEvidence::open(
        &root.0.path().join("p3-evidence/evidence.sqlite3"),
        vec![],
    )
    .unwrap();
    let session = Arc::new(
        HostSession::open(
            SessionSeed::new().unwrap(),
            root.0.path(),
            primary.clone(),
            Arc::new(crate::authority::clock::SystemClock::default()),
            Arc::new(crate::harness_tests::Yes::new(true)),
        )
        .unwrap(),
    );
    session.bootstrap_workspace(&workspace, &[]).unwrap();
    Fixture {
        session,
        root,
        primary,
        _selected: selected,
    }
}
pub(super) struct Peer {
    pub(super) endpoint: String,
    pub(super) paths: Arc<Mutex<Vec<String>>>,
    pub(super) events: Receiver<(String, bool)>,
    done: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Peer {
    pub(super) fn new(held: Option<usize>, bad: Option<usize>) -> Self {
        Self::serve(held, bad, false)
    }
    /// SB-3B: also answers the call's chat request (never registration's).
    pub(super) fn chat(held: Option<usize>, bad: Option<usize>) -> Self {
        Self::serve(held, bad, true)
    }
    fn serve(held: Option<usize>, bad: Option<usize>, chat: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let paths = Arc::new(Mutex::new(Vec::new()));
        let seen = paths.clone();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = done.clone();
        let (tx, events) = channel();
        let thread = std::thread::spawn(move || {
            let limit = Instant::now() + Duration::from_secs(10);
            while !stop.load(Ordering::SeqCst) && Instant::now() < limit {
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("accept: {e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .unwrap();
                let mut head = Vec::new();
                let mut b = [0];
                while !head.ends_with(b"\r\n\r\n") && head.len() < 8192 {
                    if stream.read(&mut b).ok() != Some(1) {
                        break;
                    }
                    head.push(b[0]);
                }
                let head = String::from_utf8(head).unwrap();
                let path = head.split_whitespace().nth(1).unwrap().to_owned();
                let length = head
                    .lines()
                    .find_map(|l| {
                        l.split_once(':')
                            .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                            .and_then(|(_, n)| n.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if stream.read_exact(&mut vec![0; length]).is_err() {
                    continue;
                }
                let i = {
                    let mut seen = seen.lock().unwrap();
                    let i = seen.len();
                    seen.push(path.clone());
                    i
                };
                tx.send((path.clone(), false)).unwrap();
                if held == Some(i) {
                    while !stop.load(Ordering::SeqCst) && Instant::now() < limit {
                        match stream.read(&mut b) {
                            Ok(0) => {
                                let _ = tx.send((path.clone(), true));
                                break;
                            }
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                                ) => {}
                            _ => break,
                        }
                    }
                } else {
                    let body = if bad == Some(i) {
                        "{}".into()
                    } else {
                        match path.as_str() {
                            "/api/version" => r#"{"version":"0.40.13"}"#.into(),
                            "/api/status" => r#"{"cloud":{"disabled":true,"source":"env"}}"#.into(),
                            "/api/tags" => format!(
                                r#"{{"models":[{{"name":"qwen3:8b","digest":"{}"}}]}}"#,
                                "ab".repeat(32)
                            ),
                            "/api/show" => {
                                format!(r#"{{"capabilities":["thinking"],"private":"{CANARY}"}}"#)
                            }
                            "/api/chat" if chat => CHAT.into(),
                            _ => panic!("forbidden {path}"),
                        }
                    };
                    reply(&mut stream, &body);
                }
            }
        });
        Self {
            endpoint,
            paths,
            events,
            done,
            thread: Some(thread),
        }
    }
    pub(super) fn prefix(&self, n: usize) {
        assert_eq!(
            *self.paths.lock().unwrap(),
            PATHS[..n],
            "peer prefix; no chat/retry"
        );
    }
}
fn reply(s: &mut TcpStream, body: &str) {
    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
pub(super) fn nth(
    session: &HostSession,
    point: &'static str,
    n: usize,
    action: impl Fn() + Send + Sync + 'static,
) {
    let count = AtomicUsize::new(0);
    session.core.cell.1.every(point, move || {
        if count.fetch_add(1, Ordering::SeqCst) == n {
            action();
        }
    });
}
#[test]
fn sb2_registration_records_before_install_once() {
    let f = fixture();
    let p = Peer::new(None, None);
    assert!(f
        .session
        .register_model_private(&p.endpoint, &pin())
        .is_ok());
    p.prefix(4);
    let rows = f.primary.store().rows();
    assert_eq!(
        rows.iter()
            .filter(|r| r.phase == "model_registered")
            .count(),
        1
    );
    assert!(f.session.core.model.ready().unwrap());
    assert!(f
        .session
        .register_model_private(&p.endpoint, &pin())
        .is_err());
    p.prefix(4);
    assert_eq!(f.primary.store().rows().len(), rows.len());
    f.session.finish_bootstrap().unwrap();
}
#[test]
fn sb2_unrecorded_failed_or_late_registration_is_absent() {
    for fault in 0..9 {
        let f = fixture();
        let p = Peer::new(None, (fault < 4).then_some(fault));
        if fault == 4 {
            f.primary.store().refuse_next_insert();
        }
        if fault == 5 {
            let ingress = f.session.ingress();
            f.session.core.cell.1.arm("model.install.before", move || {
                ingress.emergency().unwrap();
            });
        }
        if fault == 6 {
            f.session.expire_request_for_test(0);
        }
        if fault == 7 {
            f.session.ingress().accept(CommandKind::Quit, 1).unwrap();
        }
        let endpoint = if fault == 8 {
            "http://example.invalid:1"
        } else {
            &p.endpoint
        };
        assert!(f.session.register_model_private(endpoint, &pin()).is_err());
        assert!(
            !f.session.core.model.ready().unwrap(),
            "unrecorded_model_is_absent fault={fault}"
        );
        assert!(
            !f.session.core.model.installed(),
            "unrecorded_model_is_absent fault={fault}"
        );
        assert!(f.session.finish_bootstrap().is_err());
        let before = p.paths.lock().unwrap().len();
        assert!(f
            .session
            .register_model_private(&p.endpoint, &pin())
            .is_err());
        assert_eq!(
            p.paths.lock().unwrap().len(),
            before,
            "registration_attempts_eq_one"
        );
    }
}
#[test]
fn sb2_bootstrap_cannot_overtake_registration() {
    let f = fixture();
    f.session.finish_bootstrap().unwrap();
    let f = fixture();
    let p = Peer::new(None, None);
    let gate = Gate::new();
    let paused = gate.clone();
    f.session
        .core
        .cell
        .1
        .arm("model.reserved", move || paused.hold());
    std::thread::scope(|s| {
        let call = s.spawn(|| f.session.register_model_private(&p.endpoint, &pin()));
        gate.reached();
        assert!(f.session.finish_bootstrap().is_err());
        gate.open();
        assert!(call.join().unwrap().is_ok());
    });
    f.session.finish_bootstrap().unwrap();
}
#[test]
fn session_override_required_at_each_request() {
    // Prove the same real transport reaches all four boundaries without interference.
    let f = fixture();
    let p = Peer::new(None, None);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    f.session.core.cell.1.every("model.gate.before", move || {
        observed.fetch_add(1, Ordering::SeqCst);
    });
    assert!(f
        .session
        .register_model_private(&p.endpoint, &pin())
        .is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), PATHS.len());
    p.prefix(4);

    for i in 0..4 {
        let f = fixture();
        let p = Peer::new(None, None);
        let core = Arc::downgrade(&f.session.core);
        let scope = f.session.bootstrap.scope.clone();
        let paths = p.paths.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let contended = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let held = contended.clone();
        let (release, rx) = channel();
        let rx = Mutex::new(Some(rx));
        let handles = Arc::new(Mutex::new(Vec::new()));
        let threads = handles.clone();
        // Count metadata callbacks, not RequestScope admissions: Bootstrap and
        // reservation also consume next_step before the first HTTP boundary.
        f.session.core.cell.1.every("model.gate.before", move || {
            let core = core.upgrade().expect("registration session retained");
            let request = observed.fetch_add(1, Ordering::SeqCst);
            if request != i {
                return;
            }
            assert_eq!(*paths.lock().unwrap(), PATHS[..i], "boundary prefix {i}");
            let holding_core = core.clone();
            let rx = rx.lock().unwrap().take().unwrap();
            let (tx, ready) = channel();
            threads.lock().unwrap().push(std::thread::spawn(move || {
                let _hold = holding_core.model.state.lock().unwrap();
                tx.send(()).unwrap();
                // A timeout is a failed schedule, never a successful release.
                rx.recv_timeout(Duration::from_secs(3))
                    .expect("contention release");
            }));
            ready.recv_timeout(Duration::from_secs(1)).unwrap();
            // These are exactly RegistrationGate::cancelled's deny-only inputs.
            // Ordinary cancellation must remain live while the owning hold is busy.
            assert!(scope.live(core.cell.session_word()));
            assert!(!core.model.uncertain.load(Ordering::SeqCst));
            assert!(matches!(
                core.model.state.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
            held.store(true, Ordering::SeqCst);
        });
        let result = f.session.register_model_private(&p.endpoint, &pin());
        let _ = release.send(());
        for h in handles.lock().unwrap().drain(..) {
            h.join().unwrap();
        }
        assert!(
            contended.load(Ordering::SeqCst),
            "boundary {i} was not reached"
        );
        assert_eq!(calls.load(Ordering::SeqCst), i + 1, "no successor gate {i}");
        assert!(
            matches!(result, Err(SessionError::Backend("model_cancelled"))),
            "session_override_required_at_each_request {i}: {result:?}"
        );
        p.prefix(i);
        assert!(!f.session.core.model.installed());
    }
}
#[test]
fn pre_gate_ingress_request_count_eq_zero() {
    for i in 0..4 {
        let f = fixture();
        let p = Peer::new(None, None);
        let ingress = f.session.ingress();
        nth(&f.session, "model.gate.before", i, move || {
            ingress.emergency().unwrap();
        });
        assert!(f
            .session
            .register_model_private(&p.endpoint, &pin())
            .is_err());
        p.prefix(i);
    }
}
#[test]
fn sb2_failed_verification_is_not_retried() {
    for (i, path) in PATHS.iter().enumerate() {
        let f = fixture();
        // Only response i is malformed. A forbidden second verify would restart
        // at /api/version, with cancellation still live and valid responses.
        let p = Peer::new(None, Some(i));
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let core = Arc::downgrade(&f.session.core);
        let scope = f.session.bootstrap.scope.clone();
        f.session.core.cell.1.every("model.gate.before", move || {
            let core = core.upgrade().expect("registration session retained");
            assert!(scope.live(core.cell.session_word()));
            assert!(!core.model.uncertain.load(Ordering::SeqCst));
            observed.fetch_add(1, Ordering::SeqCst);
        });
        let result = f.session.register_model_private(&p.endpoint, &pin());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            i + 1,
            "verification_attempts_eq_one after failed {}",
            path
        );
        p.prefix(i + 1);
        assert!(result.is_err(), "failed verification must refuse {}", path);
        assert!(f
            .session
            .bootstrap
            .scope
            .live(f.session.core.cell.session_word()));
        assert!(!f.session.core.model.installed());
        assert!(!f.session.core.model.ready().unwrap());
        assert!(f.session.finish_bootstrap().is_err());
        // A caller retry is also forbidden and must not reach another gate or peer.
        assert!(f
            .session
            .register_model_private(&p.endpoint, &pin())
            .is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            i + 1,
            "registration_attempts_eq_one"
        );
        p.prefix(i + 1);
    }
}
#[test]
fn sb2_paused_before_cas_ingress_wins_dispatches_nothing() {
    for i in 0..4 {
        let f = fixture();
        let p = Peer::new(None, None);
        let gate = Gate::new();
        let paused = gate.clone();
        nth(&f.session, "model.gate.checked", i, move || paused.hold());
        std::thread::scope(|s| {
            let call = s.spawn(|| f.session.register_model_private(&p.endpoint, &pin()));
            gate.reached();
            f.session.ingress().emergency().unwrap();
            gate.open();
            assert!(call.join().unwrap().is_err());
        });
        p.prefix(i);
    }
}
#[test]
fn sb2_held_registration_request_cancels_and_joins() {
    for (i, path) in PATHS.iter().enumerate() {
        let f = fixture();
        let p = Peer::new(Some(i), None);
        std::thread::scope(|s| {
            let call = s.spawn(|| f.session.register_model_private(&p.endpoint, &pin()));
            for path in PATHS.iter().take(i + 1) {
                assert_eq!(
                    p.events.recv_timeout(Duration::from_secs(2)).unwrap(),
                    ((*path).into(), false)
                );
            }
            let start = Instant::now();
            f.session.ingress().emergency().unwrap();
            assert_eq!(
                p.events.recv_timeout(Duration::from_secs(1)).unwrap(),
                ((*path).into(), true)
            );
            assert!(call.join().unwrap().is_err());
            assert!(start.elapsed() < Duration::from_secs(1));
        });
        p.prefix(i + 1);
    }
}
#[test]
fn sb2_model_inventory_and_records_are_content_free() {
    let f = fixture();
    let p = Peer::new(None, None);
    f.session
        .register_model_private(&p.endpoint, &pin())
        .unwrap();
    let inventory = format!("{:?}", f.session.close_obligations().unwrap());
    assert!(!inventory.contains(CANARY));
    for entry in std::fs::read_dir(f.root.0.path().join("p3-evidence")).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(!bytes.windows(CANARY.len()).any(|w| w == CANARY.as_bytes()));
    }
}

#[test]
fn sb2_registration_uses_original_deadline_and_watch() {
    use crate::inference::{AbsoluteDeadline, ModelError};
    use nexus_kernel::inference::{CallCancel, DeadlineClock, RequestKind};
    struct Clock(Arc<(AtomicU64, AtomicU64)>);
    impl DeadlineClock for Clock {
        fn wall_ms(&self) -> u64 {
            self.0 .0.load(Ordering::SeqCst)
        }
        fn monotonic_ms(&self) -> u64 {
            self.0 .1.load(Ordering::SeqCst)
        }
    }
    for wall in [false, true] {
        let scratch = crate::evidence_store::tests::Scratch::new();
        let primary =
            crate::evidence_store::PrimaryEvidence::open(&scratch.store(), vec![]).unwrap();
        let seed = SessionSeed::new().unwrap();
        let q = bootstrap(&seed.core);
        let owner = model::reserve(&seed.core, &q).expect("operation reservation");
        let stop = nexus_kernel::emergency_stop::StopAuthority::new();
        let controller = stop.claim_controller().unwrap();
        let watch = stop.admit().unwrap();
        let clock = Arc::new((AtomicU64::new(0), AtomicU64::new(0)));
        let deadline = AbsoluteDeadline::within(Clock(clock.clone()), Duration::from_secs(30));
        let gate = model::RegistrationGate {
            core: &seed.core,
            scope: &q,
            primary: &primary,
            watch: &watch,
            deadline: &deadline,
        };
        assert_eq!(gate.cancelled(), None);
        assert_eq!(gate.admit_request(RequestKind::Metadata), Ok(()));
        assert!(gate.admit_request(RequestKind::Chat).is_err());
        if wall {
            clock.0.store(30_000, Ordering::SeqCst);
        } else {
            clock.1.store(30_000, Ordering::SeqCst);
        }
        assert_eq!(
            gate.admit_request(RequestKind::Metadata),
            Err(ModelError::Timeout)
        );
        clock.0.store(0, Ordering::SeqCst);
        clock.1.store(0, Ordering::SeqCst);
        let n = controller.engage();
        assert!(controller.lift_if(n));
        assert_eq!(
            gate.admit_request(RequestKind::Metadata),
            Err(ModelError::Stopped)
        );
        owner.complete("not_required").unwrap();
    }
}

#[test]
fn sb2_install_completion_is_atomic_to_close_observers() {
    let f = fixture();
    let p = Peer::new(None, None);
    let gate = Gate::new();
    let paused = gate.clone();
    f.session
        .core
        .cell
        .1
        .arm("model.completion.before", move || paused.hold());
    std::thread::scope(|s| {
        let call = s.spawn(|| f.session.register_model_private(&p.endpoint, &pin()));
        gate.reached();
        // Concurrent close observers must not enter a gap after installation.
        assert!(!f.session.core.model.quiescent());
        assert!(f.session.close_obligations().is_err());
        let hold = f.session.core.model.state.try_lock();
        gate.open();
        let result = call.join().unwrap();
        assert!(
            hold.is_err(),
            "installation hold released before completion"
        );
        assert!(result.is_ok(), "{result:?}");
    });
    p.prefix(4);
    let o = f.session.close_obligations().unwrap().model;
    assert!(!o.active && o.owner_complete && o.child_complete && !o.uncertain);
    assert_eq!(o.record, "recorded");
    assert!(f.session.core.model.quiescent());
}

// SB-2b frozen accounting oracles: no model dispatch, charge or chat claimed.
use crate::authority::evidence::{
    EndStatus, EvidenceRecord, EvidenceSink, EvidenceUnavailable, MemoryEvidence,
};
use crate::authority::ids::{AgentId, Digest, RunId};
use crate::authority::run::{AdmittedWorkspace, ConsentKind, RunClass, RunOrigin};
use crate::authority::Authority;
pub(super) struct EndSink {
    pub(super) memory: MemoryEvidence,
    pub(super) mode: AtomicUsize,
    pub(super) hooks: crate::test_hooks::Hooks,
}
impl EvidenceSink for EndSink {
    fn record(&self, r: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
        self.memory.record(r)
    }
    fn record_end(&self, r: &EvidenceRecord) -> EndStatus {
        self.hooks.fire("attempt.record");
        match self.mode.load(Ordering::SeqCst) {
            1 => EndStatus::Owed,
            2 => EndStatus::Uncertain,
            3 => panic!("recording panic"),
            _ => self.memory.record_end(r),
        }
    }
}
// A2 uses the real broker issuance path; setup records are kept separately.
struct IssuedEvidence(Arc<dyn EvidenceSink>, MemoryEvidence);
impl EvidenceSink for IssuedEvidence {
    fn record(&self, r: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
        self.1.record(r)
    }
    fn record_end(&self, r: &EvidenceRecord) -> EndStatus {
        if r.phase == crate::authority::evidence::EvidencePhase::ModelCall {
            self.0.record_end(r)
        } else {
            self.1.record_end(r)
        }
    }
}
struct IssueYes;
impl crate::authority::approval::ControlConfirmer for IssueYes {
    fn confirm_action(&self, _: &crate::authority::approval::ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &crate::authority::approval::GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &crate::authority::approval::ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(
        &self,
        _: &crate::authority::approval::AcknowledgementConfirmation,
    ) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &crate::authority::approval::AgentDoorConfirmation) -> bool {
        true
    }
}
pub(super) struct IssuedAuthority(Arc<crate::governed::GovernedControl>);
impl std::ops::Deref for IssuedAuthority {
    type Target = Authority;
    fn deref(&self) -> &Authority {
        self.0.authority()
    }
}
pub(super) struct Accounting {
    pub(super) seed: SessionSeed,
    pub(super) authority: IssuedAuthority,
    pub(super) run: RunId,
    agent: AgentId,
    group: u64,
    door_id: String,
    // SB-3B: the R3 grant and live invocation every ticket requires.
    live: (
        crate::authority::ids::GrantId,
        crate::inference::InvocationOwner,
    ),
    setup: (Arc<IssuedEvidence>, usize),
    _door: crate::governed::AgentDoor,
    _root: crate::harness_tests::TempRoot,
}
impl Accounting {
    pub(super) fn new(sink: Arc<dyn EvidenceSink>) -> Self {
        let seed = SessionSeed::new().unwrap();
        seed.core.cell.session_compare(0, READY).unwrap();
        let q = seed.ingress().accept(CommandKind::Run, 1).unwrap();
        let cell = seed.core.cell.scoped(q.scope.clone());
        let root = crate::harness_tests::temp_root("sb2b-issued");
        let issued = Arc::new(IssuedEvidence(sink, MemoryEvidence::new(32)));
        let authority = IssuedAuthority(Arc::new(
            crate::governed::GovernedControl::with_secrets(
                root.0.path(),
                crate::broker::Vault::Disabled.source(),
                issued.clone(),
                Arc::new(crate::authority::clock::SystemClock::default()),
            )
            .unwrap(),
        ));
        let agent = AgentId::new("sb2b-agent").unwrap();
        let door = authority
            .0
            .issue_agent_door_with_halt(agent.clone(), Arc::new(IssueYes), Some(cell.clone()))
            .unwrap();
        let group = door.origin();
        let door_id = issued
            .1
            .records()
            .iter()
            .find(|r| r.phase == crate::authority::evidence::EvidencePhase::AgentDoorOpened)
            .unwrap()
            .detail
            .iter()
            .find(|(k, _)| k == "door")
            .unwrap()
            .1
            .clone();
        assert!(
            door_id.len() == 32
                && door_id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        let grant = (authority.grants())
            .request(
                crate::authority::policy::GrantScope::Files {
                    workspace: "workspace".into(),
                    root: "/sb2b/workspace".into(),
                    identity: Digest::of("test", &[]),
                },
                Duration::from_secs(600),
                &IssueYes,
            )
            .unwrap();
        let runs = authority.runs();
        let run = runs
            .open_with_cell(
                agent.clone(),
                RunOrigin::AgentGoal,
                RunClass::Agent,
                1,
                Some(group),
                Some(cell),
            )
            .unwrap();
        runs.consent_slot(group, &agent)
            .unwrap()
            .fill(
                &"cd".repeat(32),
                "msn-000001/1/1",
                7,
                ConsentKind::Mission,
                9,
                None,
            )
            .unwrap();
        runs.admit_files(
            run,
            &agent,
            group,
            &"cd".repeat(32),
            "msn-000001/1/1",
            AdmittedWorkspace {
                id: "workspace".into(),
                identity: Digest::of("test", &[]),
            },
        )
        .unwrap();
        runs.admit_inference(
            run,
            &agent,
            group,
            &"cd".repeat(32),
            "msn-000001/1/1",
            [42; 32],
        )
        .unwrap();
        Self {
            seed,
            authority,
            run,
            agent,
            group,
            door_id,
            live: (grant, crate::inference::InvocationOwner::new()),
            setup: (issued.clone(), issued.1.records().len()),
            _door: door,
            _root: root,
        }
    }
    pub(super) fn acquire(&self) -> Result<model::Attempt, SessionError> {
        self.acquire_run(self.run)
    }
    fn acquire_run(&self, run: RunId) -> Result<model::Attempt, SessionError> {
        self.acquire_as(&self.seed.core, (run, &self.agent, self.group, [42; 32]))
    }
    fn acquire_as(
        &self,
        core: &Arc<Core>,
        key: (RunId, &AgentId, u64, [u8; 32]),
    ) -> Result<model::Attempt, SessionError> {
        let (grants, clock) = (self.authority.grants(), self.authority.clock());
        let clock = (clock.monotonic_ms(), clock.wall_ms());
        let grant = crate::authority::run::OriginalGrant {
            grants,
            grant: self.live.0,
            clock,
        };
        let observer = self.live.1.observer();
        model::Attempt::acquire(core, &self.authority, key, &core.cell, &grant, &observer).map_err(
            |e| match e {
                crate::authority::AuthorityError::ModelCall(e) => model::model_error(e),
                _ => SessionError::Restricted,
            },
        )
    }
    fn counts(&self) -> (u32, usize) {
        assert_eq!(
            self.setup.0 .1.records().len(),
            self.setup.1,
            "setup memory unchanged after fixture"
        );
        self.authority.runs().model_accounting(self.run).unwrap()
    }
}
fn end_sink() -> Arc<EndSink> {
    Arc::new(EndSink {
        memory: MemoryEvidence::new(256),
        mode: AtomicUsize::new(0),
        hooks: Default::default(),
    })
}
#[test]
fn sb2_attempt_count_and_retention_are_one_admission() {
    let sink = end_sink();
    let f = Accounting::new(sink.clone());
    for n in 1..=8 {
        let mut attempt = f.acquire().expect("issued attempt 1..8");
        assert_eq!(f.counts(), (n, 1), "attempt_record_original_run");
        assert!(f.acquire().is_err(), "one active operation");
        assert!(matches!(
            attempt
                .record_return(&f.authority, Err(ModelError::Cancelled))
                .unwrap(),
            EndStatus::Recorded { .. }
        ));
        assert_eq!(f.counts(), (n, 1), "owner return still owed");
        attempt.complete().unwrap();
        assert_eq!(f.counts(), (n, 0));
    }
    assert!(
        matches!(f.acquire(), Err(SessionError::Backend("model_over_budget"))),
        "ninth attempt is OverBudget"
    );
    assert_eq!(f.counts(), (8, 0), "no refund of issued attempts");
    assert_eq!(sink.memory.records().len(), 8);
    let f = Accounting::new(end_sink());
    let ingress = f.seed.ingress();
    f.seed.core.cell.1.arm("session.before_compare", move || {
        ingress.emergency().unwrap();
    });
    assert!(f.acquire().is_err(), "original-scope CAS must refuse");
    assert_eq!(f.counts(), (0, 0), "refused CAS changes neither counter");
}
#[test]
fn sb2_attempt_record_keeps_original_run_and_consent() {
    let sink = end_sink();
    let f = Accounting::new(sink.clone());
    let mut attempt = f.acquire().expect("retained attempt");
    assert_eq!(f.counts(), (1, 1), "attempt_record_original_run");
    f.authority.runs().cancel(f.run);
    f.authority.runs().finish(f.run);
    f.authority.runs().set_capacity_for_test(1);
    assert!(f
        .authority
        .runs()
        .open(f.agent.clone(), RunOrigin::AgentGoal, RunClass::Agent, 2)
        .is_err());
    assert!(matches!(
        attempt
            .record_return(&f.authority, Err(ModelError::Cancelled))
            .unwrap(),
        EndStatus::Recorded { .. }
    ));
    let rows = sink.memory.records();
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.run.as_deref(), Some(f.run.to_string().as_str()));
    assert_eq!(r.agent.as_deref(), Some("sb2b-agent"));
    for (k, v) in [
        ("door", f.door_id.clone()),
        ("contract_hash", "cd".repeat(32)),
        ("request", "msn-000001/1/1".into()),
        ("identity", hex::encode([42; 32])),
        ("call", "1".into()),
        ("outcome", "cancelled".into()),
    ] {
        assert!(
            r.detail.contains(&(k.into(), v)),
            "retained correlation {k}"
        );
    }
    attempt.complete().unwrap();
    assert_eq!(
        f.counts(),
        (1, 0),
        "only model pin released, File preservation retained"
    );
}
#[test]
fn sb2_record_debt_blocks_seal_and_reuse() {
    for mode in [0, 1, 2] {
        let sink = end_sink();
        sink.mode.store(mode, Ordering::SeqCst);
        let f = Accounting::new(sink.clone());
        let mut attempt = f.acquire().expect("issued debt owner");
        let record = attempt
            .record_return(&f.authority, Err(ModelError::Cancelled))
            .unwrap();
        let expected = match mode {
            0 => "recorded",
            1 => "owed",
            _ => "uncertain",
        };
        assert_eq!(record.as_str(), expected);
        let o = f.seed.core.model.snapshot().unwrap();
        assert_eq!(o.operation, "attempt");
        assert!(o.active && o.child_complete && !o.owner_complete);
        assert_eq!(o.record, expected, "record status is never upgraded");
        // Cancellation and simulated meter settlement do not own this obligation.
        f.authority.runs().cancel(f.run);
        assert!(!f.seed.core.model.quiescent());
        assert!(f.acquire().is_err());
        assert!(
            attempt.record_return(&f.authority, Ok(())).is_err(),
            "exactly one result record"
        );
        assert_eq!(attempt.complete().is_ok(), mode == 0);
        assert_eq!(
            f.counts(),
            (1, usize::from(mode != 0)),
            "attempt_record_original_run"
        );
        assert_eq!(f.seed.core.model.quiescent(), mode == 0);
    }
}
#[test]
fn sb2_attempt_drop_panic_and_poison_retain_ownership() {
    for fault in 0..3 {
        let sink = end_sink();
        let f = Accounting::new(sink.clone());
        let mut attempt = f.acquire().expect("issued debt owner");
        if fault == 1 {
            sink.mode.store(3, Ordering::SeqCst);
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || attempt.record_return(&f.authority, Ok(()))
            ))
            .is_err());
        }
        if fault == 2 {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _hold = f.seed.core.model.state.lock().unwrap();
                panic!("poison owning state");
            }));
        }
        // Drop with the real lock held must neither lock nor record nor unpin.
        let hold = f
            .seed
            .core
            .model
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        drop(attempt);
        drop(hold);
        assert_eq!(f.counts(), (1, 1), "drop retains model pin");
        assert!(f.seed.failed());
        assert!(sink.memory.records().is_empty());
        assert!(!f.seed.core.model.quiescent());
        assert!(f.acquire().is_err());
    }
}
#[test]
fn sb2_attempt_recording_does_not_block_ingress() {
    let sink = end_sink();
    let f = Accounting::new(sink.clone());
    let mut attempt = f.acquire().expect("issued debt owner");
    let core = Arc::downgrade(&f.seed.core);
    let runs = f.authority.runs().clone();
    let run = f.run;
    sink.hooks.arm("attempt.record", move || {
        let core = core.upgrade().unwrap();
        assert!(core.producers.try_lock().is_ok());
        assert!(core.model.state.try_lock().is_ok());
        assert_eq!(runs.model_accounting(run), Some((1, 1)));
        SessionIngress { core }.emergency().unwrap();
    });
    assert!(matches!(
        attempt
            .record_return(&f.authority, Err(ModelError::Cancelled))
            .unwrap(),
        EndStatus::Recorded { .. }
    ));
    attempt.complete().unwrap(); // Settlement needs no refreshed Work admission.
    assert!(f.seed.core.model.quiescent());
    assert_eq!(f.counts(), (1, 0));
}
#[test]
fn sb2_missing_outcome_is_unknown_not_success() {
    assert_eq!(
        model::historical_outcome(None),
        "unknown",
        "missing outcome is unknown"
    );
    assert_eq!(model::historical_outcome(Some(EndStatus::Owed)), "owed");
    assert_eq!(
        model::historical_outcome(Some(EndStatus::Uncertain)),
        "uncertain"
    );
    assert_eq!(
        model::historical_outcome(Some(EndStatus::Recorded { seq: 7 })),
        "recorded"
    );
}
#[test]
fn sb2_attempt_binding_refusals_issue_no_ticket() {
    let f = Accounting::new(end_sink());
    let other = SessionSeed::new().unwrap();
    for (core, run, agent, group, identity) in [
        (&other.core, f.run, f.agent.clone(), f.group, [42; 32]),
        (
            &f.seed.core,
            RunId::fresh(),
            f.agent.clone(),
            f.group,
            [42; 32],
        ),
        (
            &f.seed.core,
            f.run,
            AgentId::new("wrong").unwrap(),
            f.group,
            [42; 32],
        ),
        (&f.seed.core, f.run, f.agent.clone(), f.group + 1, [42; 32]),
        (&f.seed.core, f.run, f.agent.clone(), f.group, [43; 32]),
    ] {
        assert!(f.acquire_as(core, (run, &agent, group, identity)).is_err());
        assert_eq!(f.counts(), (0, 0));
        assert!(f.seed.core.model.quiescent());
    }
    f.acquire()
        .expect("denials did not consume slot")
        .record_return(&f.authority, Ok(()))
        .unwrap();
}
#[test]
fn sb2_attempt_recorded_end_survives_failed_delivery() {
    // Real seal/end path; deliberately leave delivery unacknowledged.
    use sha2::{Digest as _, Sha256};
    fn line(seq: u64, previous: &str, event: &str) -> (String, String) {
        let event = format!("{{\"event\":\"{event}\"}}");
        let mut h = Sha256::new();
        h.update(b"nexus-mother-journal-v2\n");
        for p in [
            previous.as_bytes(),
            &[2],
            &seq.to_be_bytes(),
            &0u64.to_be_bytes(),
            event.as_bytes(),
        ] {
            h.update((p.len() as u64).to_be_bytes());
            h.update(p);
        }
        let hash = hex::encode(h.finalize());
        (format!("{{\"format\":2,\"seq\":{seq},\"prev\":\"{previous}\",\"mission\":0,\"event\":{event},\"hash\":\"{hash}\"}}\n"), hash)
    }
    let f = fixture();
    let (opened, hash) = line(0, &"0".repeat(64), "session_opened");
    let journal = f.root.0.path().join("mother-journal.jsonl");
    std::fs::write(&journal, &opened).unwrap();
    let q = f.session.ingress().accept(CommandKind::Quit, 1).unwrap();
    f.session.settle_close_resources(&q).unwrap();
    f.session.settle_close_authority(&q).unwrap();
    let seal = f.session.seal_close(&q).unwrap();
    seal.begin_mother_end(f.session.identity()).unwrap();
    let (closed, _) = line(1, &hash, "session_closed");
    std::fs::write(journal, format!("{opened}{closed}")).unwrap();
    f.session.observe_mother_end(&seal).unwrap();
    f.session.close_primary(&seal).unwrap();
    let before = f.session.close_records();
    assert_eq!(
        before.primary,
        close::FinalStatus::Recorded,
        "recorded end preserved"
    );
    assert_eq!(before.mother, close::FinalStatus::Recorded);
    drop(seal);
    let after = f.session.close_records();
    assert_eq!(
        after.primary,
        close::FinalStatus::Recorded,
        "recorded end preserved"
    );
    assert_eq!(after.mother, close::FinalStatus::Recorded);
    assert_ne!(after.delivery, close::FinalStatus::Recorded);
}

#[test]
fn sb2_owner_completion_retries_benign_observer_contention() {
    let seed = SessionSeed::new().unwrap();
    let owner = model::reserve(&seed.core, &bootstrap(&seed.core)).unwrap();
    let (seen_tx, seen_rx) = std::sync::mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
    seed.core.cell.1.arm("model.complete.contended", move || {
        seen_tx.send(()).unwrap();
        resume_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    });
    let observer = seed.core.model.state.lock().unwrap();
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || done_tx.send(owner.complete("not_required")).unwrap());
    seen_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(
        done_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    drop(observer);
    resume_tx.send(()).unwrap();
    let result = done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.join().unwrap();
    assert!(
        result.is_ok(),
        "benign observer contention must complete: {result:?}"
    );
    assert!(seed.core.model.quiescent());
    assert!(!seed.failed());
}
#[test]
fn sb2_owner_completion_is_bounded_and_poison_fails_closed() {
    for poison in [false, true] {
        let seed = SessionSeed::new().unwrap();
        let owner = model::reserve(&seed.core, &bootstrap(&seed.core)).unwrap();
        if poison {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _hold = seed.core.model.state.lock().unwrap();
                panic!("poison completion state");
            }));
        }
        let observer = seed
            .core
            .model
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || tx.send(owner.complete("not_required")).unwrap());
        assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap().is_err());
        drop(observer);
        worker.join().unwrap();
        assert!(seed.failed());
        assert!(!seed.core.model.quiescent());
    }
}
#[test]
fn sb2_bootstrap_model_contention_is_unavailable() {
    let f = fixture();
    let _observer = f.session.core.model.state.lock().unwrap();
    assert!(matches!(
        f.session.finish_bootstrap(),
        Err(SessionError::Unavailable)
    ));
}

// Force each real cleanup hold to meet an observer; no timing-based release.
fn attempt_observer_schedule(case: usize) {
    let sink = end_sink();
    let f = Accounting::new(sink.clone());
    let mut attempt = (case != 1).then(|| f.acquire().unwrap());
    let record = |attempt: &mut model::Attempt| attempt.record_return(&f.authority, Ok(()));
    if case == 2 {
        record(attempt.as_mut().unwrap()).unwrap();
    }
    let (start_tx, start_rx) = std::sync::mpsc::channel();
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let retried = Arc::new(AtomicBool::new(false));
    let seen = retried.clone();
    let release = release_tx.clone();
    f.seed.core.cell.1.arm("model.complete.contended", move || {
        if case == 0 {
            assert_eq!(
                sink.memory.records().len(),
                1,
                "record I/O precedes metadata hold"
            );
        }
        seen.store(true, Ordering::SeqCst);
        release.send(()).unwrap();
    });
    let point = [
        "model.attempt.return",
        "model.attempt.refused",
        "model.settle.before",
    ][case];
    f.seed.core.cell.1.arm(point, move || {
        start_tx.send(()).unwrap();
        held_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    });
    std::thread::scope(|scope| {
        let f = &f;
        scope.spawn(move || {
            start_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            let _observer = if case == 2 {
                (None, Some(f.authority.runs().model_observer()))
            } else {
                (Some(f.seed.core.model.state.lock().unwrap()), None)
            };
            held_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        });
        let result = match case {
            0 => record(attempt.as_mut().unwrap()).map(|_| ()),
            1 => f.acquire_run(RunId::fresh()).map(|_| ()),
            _ => attempt.take().unwrap().complete(),
        };
        let _ = release_tx.send(());
        assert_eq!(
            result.is_ok(),
            case != 1,
            "conclusive result after observer"
        );
    });
    assert!(
        retried.load(Ordering::SeqCst),
        "ordinary contention must retry"
    );
    if let Some(mut attempt) = attempt {
        assert!(attempt.record_return(&f.authority, Ok(())).is_err());
        attempt.complete().unwrap();
    }
    assert_eq!(f.counts(), (u32::from(case != 1), 0));
    assert!(f.seed.core.model.quiescent() && !f.seed.failed());
}
#[test]
fn sb2_attempt_record_survives_held_observer() {
    attempt_observer_schedule(0);
}
#[test]
fn sb2_attempt_refusal_survives_held_observer() {
    attempt_observer_schedule(1);
}
#[test]
fn sb2_attempt_settlement_survives_held_observer() {
    attempt_observer_schedule(2);
}
