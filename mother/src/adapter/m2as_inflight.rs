//! M2a-S, oracle added after the gate review (it does not replace or edit
//! the earlier ones), Owner decision "cancel before and after": an action
//! already executing at the broker when the coordinator breaks is
//! interrupted by the fence (its run's cancellation token), not waited for.
//! The fence runs under the coordinator lock, so waiting would keep the
//! action running into the world after the break and hang the Owner's Stop.
//! Run from `real_broker.rs` (one broker per process).

use super::m2as_support::{rig, Disk, Rig};
use super::{Home, MOTHER_AGENT};
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, Yes};
use crate::state::MissionState;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the world holds the request (longer than the broker's egress
/// timeout, 20 s: a fence that waits for the action shows as a long break).
const HOLD: Duration = Duration::from_secs(25);
/// A fence that interrupts the action returns well within this.
const PROMPT: Duration = Duration::from_secs(5);

/// The world, holding any request whose line contains `/inflight-hold`
/// open (no answer) until released; answering every other one.
struct HoldingWorld {
    held: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    lines: Arc<Mutex<Vec<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HoldingWorld {
    fn start(listener: &TcpListener) -> Self {
        let listener = listener.try_clone().unwrap();
        let (held, release, stop) = (
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (h, r, s, l) = (held.clone(), release.clone(), stop.clone(), lines.clone());
        let thread = std::thread::spawn(move || {
            let mut holding = Vec::new();
            let mut held_at: Option<Instant> = None;
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let mut request = [0u8; 4096];
                        let n = stream.read(&mut request).unwrap_or(0);
                        let line = String::from_utf8_lossy(&request[..n])
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .to_string();
                        l.lock().unwrap().push(line.clone());
                        if line.contains("/inflight-hold") {
                            h.store(true, Ordering::SeqCst);
                            held_at.get_or_insert_with(Instant::now);
                            holding.push(stream);
                        } else {
                            let _ = stream.write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nmother",
                            );
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        if r.load(Ordering::SeqCst) || held_at.is_some_and(|at| at.elapsed() > HOLD)
                        {
                            holding.clear();
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(1)),
                }
            }
        });
        Self {
            held,
            release,
            stop,
            lines,
            thread: Some(thread),
        }
    }

    fn wait_held(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        while !self.held.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    fn stop(mut self) {
        self.release.store(true, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn approved(r: &Rig, markers: &str) -> MissionId {
    let id = r.mother.draft(contract_with(markers)).unwrap();
    r.mother.submit(id).unwrap();
    assert_eq!(
        r.mother.request_consent(id, &Yes),
        Ok(MissionState::Approved)
    );
    id
}

fn the_fence_interrupts_an_action_in_flight(home: &Home, origin: &str, world: &HoldingWorld) {
    let control = &home.control;
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/inflight-hold"));
    for _ in 0..2 {
        r.mother.advance(b).unwrap();
    }
    let mother = r.mother.clone();
    let b_thread = std::thread::spawn(move || mother.advance(b).map(|x| x.to));
    assert!(
        world.wait_held(Duration::from_secs(10)),
        "B's request never reached the world"
    );

    // The break, while B's action executes at the broker.
    let started = Instant::now();
    disk.fail(true, true);
    assert!(r.mother.stop(victim).is_err());
    disk.fail(false, false);
    let took = started.elapsed();
    assert!(r.mother.broken());
    assert!(
        took < PROMPT,
        "the fence waited {took:?} for the action in flight instead of interrupting it"
    );

    let answer = b_thread.join().unwrap();
    assert!(answer.is_err(), "B answered {answer:?} after the break");
    let run = r.port.run_of(b).expect("B's run");
    assert!(control.authority().runs().view(run).unwrap().cancelled);
    assert_eq!(
        world
            .lines
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.contains("/inflight-hold"))
            .count(),
        1,
        "the request was sent once, before the break"
    );
    assert_eq!(
        control
            .authority()
            .runs()
            .views()
            .iter()
            .filter(|v| v.agent.to_string() == MOTHER_AGENT && !v.cancelled && !v.finished)
            .filter(|v| Some(v.id) == r.port.run_of(b) || Some(v.id) == r.port.run_of(victim))
            .count(),
        0
    );
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = HoldingWorld::start(listener);
    the_fence_interrupts_an_action_in_flight(home, origin, &world);
    world.stop();
}
