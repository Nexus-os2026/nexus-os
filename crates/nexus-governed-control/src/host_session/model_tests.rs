//! Frozen SB-2a oracles. Q/R/C/L are N/A for Bootstrap registration.
use super::*;
use crate::inference::{ModelPinSpec, RuntimeConfig, Think};
use crate::test_hooks::Gate;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{atomic::AtomicUsize, Mutex};
const PATHS: [&str; 4] = ["/api/version", "/api/status", "/api/tags", "/api/show"];
const CANARY: &str = "SB2_METADATA_PRIVATE_CANARY";
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
    assert!(!seed.core.model.ready() && !seed.core.model.quiescent());
    owner.complete("not_required").unwrap();
    assert!(seed.core.model.quiescent());
    assert!(
        !seed.core.model.ready(),
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
    paths: Arc<Mutex<Vec<String>>>,
    events: Receiver<(String, bool)>,
    done: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Peer {
    pub(super) fn new(held: Option<usize>, bad: Option<usize>) -> Self {
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
fn nth(
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
    assert!(f.session.core.model.ready());
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
            !f.session.core.model.ready(),
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
        assert!(!f.session.core.model.ready());
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
