//! The status window (design §5 and §11): labels, frames and the Stop
//! button, `set_text` only. Every GTK callback body runs inside
//! [`guarded`], so no panic escapes into gtk-rs (G15).

use crate::status::{Snapshot, StatusFeed, NOT_LIVE_AFTER};
use crate::stop::{Receipts, StopRoute, MAX_STOP_WORKERS};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const TITLE: &str = "Mother";
pub const STOP_LABEL: &str = "Stop everything";
/// The Stop shortcut, in the main window's own shortcut controller.
pub const STOP_SHORTCUT: &str = "<Control><Shift>s";

/// How often the window drains the status feed and redraws its labels.
const DRAIN_EVERY: Duration = Duration::from_millis(20);

static PANICS: AtomicU64 = AtomicU64::new(0);

/// Run one GTK callback body; a panic is contained, counted and answered
/// `None`.
pub fn guarded<R>(what: &'static str, f: impl FnOnce() -> R) -> Option<R> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => Some(value),
        Err(_) => {
            PANICS.fetch_add(1, Ordering::SeqCst);
            eprintln!("nexus-mother: a {what} callback panicked; it was contained");
            None
        }
    }
}

/// Panics contained by [`guarded`] in this process.
pub fn callback_panics() -> u64 {
    PANICS.load(Ordering::SeqCst)
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

fn label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_selectable(false);
    label
}

fn framed(title: &str, labels: &[&gtk::Label]) -> gtk::Frame {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    column.set_margin_top(6);
    column.set_margin_bottom(6);
    column.set_margin_start(8);
    column.set_margin_end(8);
    for label in labels {
        column.append(*label);
    }
    let frame = gtk::Frame::new(Some(title));
    frame.set_child(Some(&column));
    frame
}

fn yes_no(stopped: bool) -> &'static str {
    if stopped {
        "IN FORCE"
    } else {
        "not in force"
    }
}

fn broker_text(snapshot: Option<&Snapshot>) -> String {
    match snapshot {
        None => "No reading of the broker yet.".to_string(),
        Some(s) => {
            let live = if s.taken.elapsed() > NOT_LIVE_AFTER {
                "NOT LIVE"
            } else {
                "live"
            };
            format!(
                "Emergency stop (broker): {}\nEmergency stop (kernel): {}, stop number {}\n\
                 Policy generation {}; doors issued {}, alive {}\n\
                 Reading {} ({live}, {} ms old). Source: the broker's status and the kernel's Stop.",
                yes_no(s.broker_stopped),
                yes_no(s.kernel_stopped),
                s.stop_number,
                s.policy_generation,
                s.doors_issued,
                s.live_doors,
                s.seq,
                s.taken.elapsed().as_millis(),
            )
        }
    }
}

fn stop_text(r: &Receipts) -> String {
    format!(
        "Stop presses {}; Stop epoch {}; workers running {} of {MAX_STOP_WORKERS}; \
         finished {}; coalesced at the cap {}; failed to start {}; panicked {}.\n\
         Source: this window's Stop route (in memory).",
        r.presses, r.epoch, r.live, r.finished, r.coalesced, r.spawn_failures, r.panicked,
    )
}

fn start_text(at_start: AtStart) -> &'static str {
    match at_start {
        AtStart::NotStopped => "At start, the broker was not stopped.",
        AtStart::StoppedForRecovery => {
            "At start, the broker was stopped: recovery is required (an earlier session \
             may have acted without a record). It stays stopped until the Owner resumes it."
        }
    }
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
    let window = gtk::Window::new();
    window.set_title(Some(TITLE));
    window.set_default_size(720, 480);

    let broker = label(&broker_text(None));
    let started = label(start_text(at_start));
    let stops = label(&stop_text(&route.receipts()));
    let cannot_act = label(
        "Mother cannot act from this window. It has no confirmation dialogs yet (M2b-A), \
         so every question Mother or the broker asks is answered no: no door is asked for \
         and none is issued.",
    );
    let home_line = label(&format!("Home: {}", home.display()));
    let stop_button = gtk::Button::with_label(STOP_LABEL);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);
    // Stop first: nothing above it changes size, so the status redraws
    // below can never move it from under the Owner's pointer.
    content.append(&stop_button);
    content.append(&framed("Broker", &[&broker, &started]));
    content.append(&framed("Stop", &[&stops]));
    content.append(&framed("Mother", &[&cannot_act, &home_line]));

    // Mother → Quit, in a window-local action group (never exported).
    let actions = gio::SimpleActionGroup::new();
    let quit_action = gio::SimpleAction::new("quit", None);
    {
        let quit = quit.clone();
        quit_action.connect_activate(move |_, _| {
            guarded("quit", || quit());
        });
    }
    actions.add_action(&quit_action);
    window.insert_action_group("mother", Some(&actions));
    let menu = gio::Menu::new();
    let mother = gio::Menu::new();
    mother.append(Some("Quit"), Some("mother.quit"));
    menu.append_submenu(Some("Mother"), &mother);
    let bar = gtk::PopoverMenuBar::from_model(Some(&menu));

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&bar);
    outer.append(&content);
    window.set_child(Some(&outer));

    // The Stop button and the shortcut: the ingress only, then return.
    {
        let route = route.clone();
        let probe = probe.clone();
        stop_button.connect_clicked(move |_| {
            guarded("stop button", || {
                route.press();
                probe.stop_presses.fetch_add(1, Ordering::SeqCst);
            });
        });
    }
    let shortcuts = gtk::ShortcutController::new();
    shortcuts.set_scope(gtk::ShortcutScope::Global);
    {
        let route = route.clone();
        let probe = probe.clone();
        shortcuts.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string(STOP_SHORTCUT),
            Some(gtk::CallbackAction::new(move |_, _| {
                guarded("stop shortcut", || {
                    route.press();
                    probe.stop_presses.fetch_add(1, Ordering::SeqCst);
                });
                glib::Propagation::Stop
            })),
        ));
    }
    window.add_controller(shortcuts);

    // The main window's × quits. Quit closes the window itself, after the
    // main loop ended, so the close request goes no further.
    window.connect_close_request(move |_| {
        guarded("close", || quit());
        glib::Propagation::Stop
    });

    // The drain: the newest snapshot and the route's receipts, redrawn.
    glib::timeout_add_local(DRAIN_EVERY, move || {
        guarded("drain", || {
            probe.heartbeat.fetch_add(1, Ordering::SeqCst);
            broker.set_text(&broker_text(feed.latest().as_ref()));
            stops.set_text(&stop_text(&route.receipts()));
        });
        glib::ControlFlow::Continue
    });

    window.present();
    MainWindow {
        window,
        content,
        stop_button,
    }
}
