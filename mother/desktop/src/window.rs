//! The status window (design §5 and §11): labels, frames and the Stop
//! button, `set_text` only. Every GTK callback body runs inside
//! [`guarded`], so no panic escapes into gtk-rs (G15).

use crate::status::StatusFeed;
use crate::stop::StopRoute;
use gtk4 as gtk;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

pub const TITLE: &str = "Mother";
pub const STOP_LABEL: &str = "Stop everything";
/// The Stop shortcut, in the main window's own shortcut controller.
pub const STOP_SHORTCUT: &str = "<Control><Shift>s";

/// Run one GTK callback body; a panic is contained, counted and answered
/// `None`.
pub fn guarded<R>(what: &'static str, f: impl FnOnce() -> R) -> Option<R> {
    let _ = (what, f);
    unimplemented!("M2b-A PR-2 code")
}

/// Panics contained by [`guarded`] in this process.
pub fn callback_panics() -> u64 {
    unimplemented!("M2b-A PR-2 code")
}

/// Counters the window advances, readable from any thread (tests read
/// them; nothing reads them for a decision).
#[derive(Default)]
pub struct Probe {
    /// Advanced by the window's periodic drain on the GTK thread.
    pub heartbeat: AtomicU64,
    /// Stop presses the window's handlers passed to the route.
    pub stop_presses: AtomicU64,
}

/// Whether the broker was stopped when the desktop started, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtStart {
    NotStopped,
    /// The broker started stopped because recovery is required.
    StoppedForRecovery,
}

/// The built window.
pub struct MainWindow {
    pub window: gtk::Window,
    /// The content area (G14): labels, frames and the Stop button only.
    pub content: gtk::Box,
    pub stop_button: gtk::Button,
}

/// Build the window. `quit` is called (on the GTK thread) by Mother → Quit
/// and by the window's ×.
pub fn build(
    route: Arc<StopRoute>,
    feed: StatusFeed,
    home: &Path,
    at_start: AtStart,
    probe: Arc<Probe>,
    quit: Rc<dyn Fn()>,
) -> MainWindow {
    let _ = (route, feed, home, at_start, probe, quit);
    unimplemented!("M2b-A PR-2 code")
}
