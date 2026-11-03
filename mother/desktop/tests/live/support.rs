//! Support for the live tests: the stalled evidence write, the isolated
//! start, real XTest clicks and the driver that runs beside the GTK loop.
//!
//! **The stalled store.** The broker's own stall fault is test-only inside
//! the broker crate, and M2b changes no broker code. So the live tests open
//! the real broker on the real evidence store, as `Home::open` does, with
//! one extra projection: [`StopGate`]. The store calls every projection
//! synchronously on the writer's thread, after its commit and outside its
//! lock (`PrimaryEvidence::project`). Once armed, the gate holds every
//! thread that writes an `emergency_stop` record there, until released: the
//! caller of `emergency_stop()` is then blocked inside its evidence write,
//! after the Stop's memory step, exactly as a slow store blocks it.

#![allow(dead_code)]

use super::harness::{progress, EXPECTED_DISPLAY};
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::{
    EvidencePhase, EvidenceProjection, EvidenceRecord, MemoryEvidence,
};
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use nexus_mother_desktop::app::{launch, Opened, Started};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The longest any one wait of a live test may take: shorter than the
/// harness's watchdog step, so a failed wait reports its own assertion.
pub const WAIT: Duration = Duration::from_secs(8);

#[derive(Default)]
struct GateState {
    armed: bool,
    released: bool,
    /// Threads that reached the gate with an `emergency_stop` record.
    entered: usize,
    /// Of those, the ones that left.
    left: usize,
}

/// Holds every `emergency_stop` record's writer once armed, until released.
#[derive(Default)]
pub struct StopGate {
    state: Mutex<GateState>,
    changed: Condvar,
}

impl StopGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn arm(&self) {
        self.state.lock().unwrap().armed = true;
    }

    pub fn release(&self) {
        self.state.lock().unwrap().released = true;
        self.changed.notify_all();
    }

    pub fn entered(&self) -> usize {
        self.state.lock().unwrap().entered
    }

    pub fn left(&self) -> usize {
        self.state.lock().unwrap().left
    }

    /// Wait until `n` writers are held at the gate.
    pub fn wait_entered(&self, n: usize) {
        progress(&format!("{n} Stop writer(s) held in the store"));
        let deadline = Instant::now() + WAIT;
        let mut state = self.state.lock().unwrap();
        while state.entered < n {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "{n} Stop writers never reached the store");
            state = self.changed.wait_timeout(state, left).unwrap().0;
        }
    }
}

impl EvidenceProjection for StopGate {
    fn project(&self, record: &EvidenceRecord) {
        if record.phase != EvidencePhase::EmergencyStop {
            return;
        }
        let mut state = self.state.lock().unwrap();
        if !state.armed {
            return;
        }
        state.entered += 1;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).unwrap();
        }
        state.left += 1;
        self.changed.notify_all();
    }
}

/// A private directory (0700), created with its parents.
fn private(dir: &Path) {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .unwrap();
}

/// The evidence store `gated_broker` opened in this process (one broker
/// per process), so the desktop can close its session at Quit.
static GATED_EVIDENCE: OnceLock<Arc<PrimaryEvidence>> = OnceLock::new();

/// The broker as `Home::open` builds it, with `gate` as one more
/// projection.
pub fn gated_broker(
    gate: Arc<StopGate>,
) -> impl FnOnce(&Path) -> Result<Arc<GovernedControl>, String> {
    move |dir: &Path| {
        private(dir);
        let broker = dir.join("broker");
        private(&broker);
        let evidence = PrimaryEvidence::open(
            &broker.join("p3-evidence").join("evidence.sqlite3"),
            vec![gate, Arc::new(MemoryEvidence::new(256))],
        )
        .map_err(|e| format!("evidence store: {}", e.as_str()))?;
        let _ = GATED_EVIDENCE.set(evidence.clone());
        let control = GovernedControl::new(
            &broker,
            Vault::Disabled,
            evidence,
            Arc::new(SystemClock::default()),
        )
        .map_err(|e| format!("broker: {e:?}"))?;
        Ok(Arc::new(control))
    }
}

/// A fresh home directory path (not created: the start creates it).
pub fn home(label: &str) -> PathBuf {
    super::harness::fresh_dir(label).join("mother")
}

/// The child is on the harness's own X11 display, never an inherited one.
pub fn assert_isolated_display() {
    let expected = std::env::var(EXPECTED_DISPLAY).expect("the harness names the display");
    let display = gtk::gdk::Display::default().expect("a default display");
    assert_eq!(display.type_().name(), "GdkX11Display");
    assert_eq!(display.name().as_str(), expected);
    for hostile in [
        "WAYLAND_DISPLAY",
        "WAYLAND_SOCKET",
        "XDG_SESSION_TYPE",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if hostile == "DBUS_SESSION_BUS_ADDRESS"
            && std::env::var_os("NEXUS_DESKTOP_LIVE_DBUS").is_some()
        {
            continue;
        }
        assert!(
            std::env::var_os(hostile).is_none(),
            "{hostile} reached the child"
        );
    }
    assert_eq!(std::env::var("GDK_BACKEND").as_deref(), Ok("x11"));
    progress("display asserted");
}

/// Start the desktop with `open`, on the isolated display.
pub fn launch_isolated(
    label: &str,
    open: impl FnOnce(&Path) -> Result<Arc<GovernedControl>, String>,
) -> Started {
    let started = launch(&home(label), None, |dir: &Path| {
        let owner = open(dir)?;
        let evidence = GATED_EVIDENCE
            .get()
            .cloned()
            .ok_or("the gated broker kept no evidence store")?;
        Ok(Opened {
            owner,
            close_session: Box::new(move || evidence.close_session()),
        })
    })
    .expect("the desktop starts");
    assert_isolated_display();
    started
}

/// A bounded tool that exited unsuccessfully: its status and stderr.
#[derive(Debug)]
pub struct ToolFailure {
    pub status: std::process::ExitStatus,
    pub stderr: String,
}

/// Run a bounded tool; its output, or how it failed.
pub fn try_tool(program: &str, args: &[&str]) -> Result<String, ToolFailure> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("{program} is required: {e}"));
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            return if status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                Err(ToolFailure {
                    status,
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                })
            };
        }
        if Instant::now() > deadline {
            super::harness::kill_and_reap(&mut child);
            panic!("{program} {args:?} overran {WAIT:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// G13: whether a failed bus query failed only because its name vanished
/// between the listing and the query: the bus's own NameHasNoOwner error,
/// and nothing else.
pub fn vanished_name(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        line.trim_start()
            .starts_with("Error org.freedesktop.DBus.Error.NameHasNoOwner:")
    })
}

/// Run a bounded tool; its output.
pub fn run_tool(program: &str, args: &[&str]) -> String {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("{program} is required: {e}"));
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            assert!(
                status.success(),
                "{program} {args:?}: {status}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return String::from_utf8_lossy(&output.stdout).into_owned();
        }
        if Instant::now() > deadline {
            super::harness::kill_and_reap(&mut child);
            panic!("{program} {args:?} overran {WAIT:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Where a real click must land: the X window and a point in it.
#[derive(Clone, Debug)]
pub struct Target {
    pub window: String,
    pub x: i32,
    pub y: i32,
}

impl Target {
    /// A real click (XTest, through `xdotool`): never `emit_clicked`.
    pub fn click(&self) {
        run_tool(
            "xdotool",
            &[
                "mousemove",
                "--window",
                &self.window,
                &self.x.to_string(),
                &self.y.to_string(),
                "click",
                "1",
            ],
        );
    }

    /// Real key input to the window.
    pub fn key(&self, keys: &str) {
        run_tool("xdotool", &["key", "--window", &self.window, keys]);
    }
}

/// Wait for `what` to hold; the wait is a readiness check, never a proof.
pub fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
    progress(what);
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What the driver thread gets.
pub struct Driver {
    pub stop: Target,
    pub route: Arc<nexus_mother_desktop::stop::StopRoute>,
    pub owner: Arc<GovernedControl>,
    pub probe: Arc<nexus_mother_desktop::window::Probe>,
}

/// Run the GTK main loop on this thread while `scenario` runs on a driver
/// thread; the loop ends when the scenario returns. A panic in the
/// scenario fails the test after the loop ended.
pub fn drive<R: Send + 'static>(
    started: &Started,
    scenario: impl FnOnce(Driver) -> R + Send + 'static,
) -> R {
    let (point_tx, point_rx) = mpsc::channel::<(i32, i32)>();
    let done = Arc::new(AtomicBool::new(false));
    let window = started.window.window.clone();
    let button = started.window.stop_button.clone();
    let main_loop = started.main_loop.clone();
    let sent = std::cell::Cell::new(false);
    let watching = done.clone();
    glib::timeout_add_local(Duration::from_millis(10), move || {
        if !sent.get() && window.is_mapped() && button.width() > 0 {
            let center = gtk::graphene::Point::new(
                button.width() as f32 / 2.0,
                button.height() as f32 / 2.0,
            );
            if let Some(point) = button.compute_point(&window, &center) {
                let (sx, sy) = window.surface_transform();
                let _ = point_tx.send((
                    (point.x() as f64 + sx).round() as i32,
                    (point.y() as f64 + sy).round() as i32,
                ));
                sent.set(true);
            }
        }
        if watching.load(Ordering::SeqCst) {
            main_loop.quit();
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    let route = started.route.clone();
    let owner = started.owner.clone();
    let probe = started.probe.clone();
    let finished = done.clone();
    let driver = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (x, y) = point_rx.recv_timeout(WAIT).expect("the window maps");
            let found = run_tool(
                "xdotool",
                &[
                    "search",
                    "--sync",
                    "--name",
                    &format!("^{}$", nexus_mother_desktop::window::TITLE),
                ],
            );
            let window = found
                .lines()
                .next()
                .expect("the window is found")
                .to_string();
            progress("window mapped");
            scenario(Driver {
                stop: Target { window, x, y },
                route,
                owner,
                probe,
            })
        }));
        finished.store(true, Ordering::SeqCst);
        result
    });
    started.main_loop.run();
    match driver.join().expect("the driver thread") {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
