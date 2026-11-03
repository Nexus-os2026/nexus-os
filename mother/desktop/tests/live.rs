//! M2b-A PR-2 live oracles (design §7.1 and §11 A1, A3), registered before
//! the desktop they judge. Each test re-runs itself alone in a child
//! process (one broker per process: the front door claims the process's
//! Stop controller), under the harness's rules (`live/harness.rs`). Every
//! press is a real XTest click through `xdotool`, never `emit_clicked`.

#[path = "live/harness.rs"]
mod harness;
#[path = "live/support.rs"]
mod support;

use gtk::prelude::*;
use gtk4 as gtk;
use harness::{live, progress, Opts};
use nexus_kernel::emergency_stop::StopAuthority;
use nexus_mother_desktop::app::{finish, start, StartError, QUIT_WITHIN};
use nexus_mother_desktop::confirm::NoDialogs;
use nexus_mother_desktop::stop::{Joined, Press, QuitOutcome, MAX_STOP_WORKERS};
use nexus_mother_host::Home;
use std::sync::Arc;
use std::time::Duration;
use support::{drive, gated_broker, launch_isolated, wait_until, StopGate};

fn on_display() -> Opts {
    Opts {
        display: true,
        ..Opts::default()
    }
}

fn kernel_stopped() -> bool {
    StopAuthority::global().is_stopped()
}

/// The production start (`Home::open`) on the isolated display.
fn production(label: &str) -> nexus_mother_desktop::app::Started {
    let started = start(&support::home(label), None).expect("the desktop starts");
    support::assert_isolated_display();
    started
}

// ── H1: hostile inherited display variables ─────────────────────────────────

#[test]
fn h1_the_child_never_uses_an_inherited_display() {
    live(
        "h1_the_child_never_uses_an_inherited_display",
        Opts {
            display: true,
            hostile: true,
            ..Opts::default()
        },
        || {
            let started = production("h1");
            assert!(
                std::env::var_os("GTK_DEBUG").is_none(),
                "GTK_DEBUG reached the child"
            );
            drive(&started, |_| ());
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

// ── S1–S3: the front door holds the Stop controller; Home before GTK ──────

#[test]
fn s1_the_front_door_holds_the_stop_controller_after_start() {
    live(
        "s1_the_front_door_holds_the_stop_controller_after_start",
        on_display(),
        || {
            let started = production("s1");
            assert!(StopAuthority::global().halt_audit_attached());
            let second = Home::open(&support::home("s1-second"), Arc::new(NoDialogs));
            assert!(second.is_err(), "a second front door was built");
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

#[test]
fn s2_a_real_click_on_stop_stops_the_broker_and_the_kernel() {
    live(
        "s2_a_real_click_on_stop_stops_the_broker_and_the_kernel",
        on_display(),
        || {
            let started = production("s2");
            assert!(!kernel_stopped() && !started.owner.status().emergency_stopped);
            let epoch = started.route.desk().epoch();
            drive(&started, move |d| {
                d.stop.click();
                wait_until("the click's ingress", || d.route.receipts().presses == 1);
                assert_eq!(d.route.desk().epoch(), epoch + 1);
                wait_until("the worker's Stop", || {
                    kernel_stopped() && d.owner.status().emergency_stopped
                });
                wait_until("the worker finished", || d.route.receipts().finished == 1);
            });
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

#[test]
fn s2_the_shortcut_stops_too() {
    live("s2_the_shortcut_stops_too", on_display(), || {
        let started = production("s2-key");
        drive(&started, |d| {
            d.stop.key("ctrl+shift+s");
            wait_until("the shortcut's ingress", || d.route.receipts().presses == 1);
            wait_until("the worker's Stop", || {
                kernel_stopped() && d.owner.status().emergency_stopped
            });
        });
        assert!(finish(started, QUIT_WITHIN).is_clean());
    });
}

#[test]
fn s3_without_a_display_the_home_was_opened_first() {
    live(
        "s3_without_a_display_the_home_was_opened_first",
        Opts::default(),
        || {
            assert!(std::env::var_os("DISPLAY").is_none());
            let home = support::home("s3");
            let refused = start(&home, None);
            assert!(
                matches!(refused, Err(StartError::Display(_))),
                "{:?}",
                refused.err()
            );
            // The broker was built before GTK was touched: its runtime
            // directory exists and it claimed the Stop controller.
            assert!(home.join("broker").is_dir(), "Home::open did not run first");
        },
    );
}

// ── S9: the inspector ─────────────────────────────────────────────────────

#[test]
fn s9_gtk_debug_interactive_means_the_app_does_not_start() {
    live(
        "s9_gtk_debug_interactive_means_the_app_does_not_start",
        Opts {
            display: true,
            env: vec![("GTK_DEBUG", "interactive".to_string())],
            ..Opts::default()
        },
        || {
            let home = support::home("s9");
            let gtk_debug = std::env::var_os("GTK_DEBUG");
            assert!(gtk_debug.is_some());
            let refused = start(&home, gtk_debug.as_deref());
            assert!(
                matches!(refused, Err(StartError::InspectorRequested)),
                "{:?}",
                refused.err()
            );
            assert!(!home.exists(), "the home was opened");
            assert!(!StopAuthority::global().halt_audit_attached());
        },
    );
}

// ── S6, S7: Stop reaches memory while the store is stalled; GTK runs ──────

#[test]
fn s6_a_real_click_reaches_memory_while_the_store_is_stalled() {
    live(
        "s6_a_real_click_reaches_memory_while_the_store_is_stalled",
        on_display(),
        || {
            let gate = StopGate::new();
            let started = launch_isolated("s6", gated_broker(gate.clone()));
            let epoch = started.route.desk().epoch();
            let held = gate.clone();
            drive(&started, move |d| {
                held.arm();
                d.stop.click();
                wait_until("step 0: the epoch moved", || {
                    d.route.desk().epoch() == epoch + 1
                });
                held.wait_entered(1);
                assert!(kernel_stopped(), "the kernel stop is in memory");
                assert!(
                    d.owner.status().emergency_stopped,
                    "the broker stop is in memory"
                );
                assert_eq!(d.route.receipts().finished, 0, "the write is still stalled");
                held.release();
                wait_until("the worker finished", || d.route.receipts().finished == 1);
            });
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

#[test]
fn s7_the_main_loop_keeps_running_while_the_backend_is_stalled() {
    live(
        "s7_the_main_loop_keeps_running_while_the_backend_is_stalled",
        on_display(),
        || {
            let gate = StopGate::new();
            let started = launch_isolated("s7", gated_broker(gate.clone()));
            let held = gate.clone();
            drive(&started, move |d| {
                held.arm();
                d.stop.click();
                held.wait_entered(1);
                let beat = |p: &nexus_mother_desktop::window::Probe| {
                    p.heartbeat.load(std::sync::atomic::Ordering::SeqCst)
                };
                let at = beat(&d.probe);
                wait_until("the heartbeat advances", || beat(&d.probe) >= at + 10);
                // Every handler returns: a second press is handled while the
                // first worker is still stalled.
                d.stop.click();
                wait_until("the second press was handled", || {
                    d.probe
                        .stop_presses
                        .load(std::sync::atomic::Ordering::SeqCst)
                        == 2
                });
                let at = beat(&d.probe);
                wait_until("the heartbeat still advances", || beat(&d.probe) >= at + 10);
                assert_eq!(d.route.receipts().finished, 0);
                held.release();
                wait_until("both workers finished", || d.route.receipts().finished == 2);
            });
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

// ── A1: the two-Stop schedule ─────────────────────────────────────────────

#[test]
fn a1_a_second_stop_does_not_wait_behind_the_first_stop_s_write() {
    live(
        "a1_a_second_stop_does_not_wait_behind_the_first_stop_s_write",
        on_display(),
        || {
            let gate = StopGate::new();
            let started = launch_isolated("a1", gated_broker(gate.clone()));
            let epoch = started.route.desk().epoch();
            let held = gate.clone();
            drive(&started, move |d| {
                held.arm();
                // 1. Stop A's worker is blocked in its evidence write.
                d.stop.click();
                held.wait_entered(1);
                assert_eq!(d.route.receipts().live, 1);
                // 2. Stop B, a real click.
                d.stop.click();
                // 3. B's ingress happens without waiting for A's write.
                wait_until("B's ingress", || d.route.desk().epoch() == epoch + 2);
                let receipts = d.route.receipts();
                assert_eq!(receipts.presses, 2);
                assert_eq!(receipts.dispatched, 2, "a worker per press");
                assert_eq!(receipts.finished, 0, "A is still in its write");
                assert_eq!(held.left(), 0);
                assert!(kernel_stopped() && d.owner.status().emergency_stopped);
                // Decision 2: B's dispatch has completed (its press returned,
                // so its worker's handle is registered) before the store is
                // released, so no worker can finish unregistered.
                wait_until("B's dispatch completed", || {
                    d.probe
                        .stop_presses
                        .load(std::sync::atomic::Ordering::SeqCst)
                        == 2
                });
                // 4. The store released: both workers finish and are joined.
                held.release();
                assert_eq!(
                    d.route.join_within(support::WAIT),
                    Joined::All { joined: 2 }
                );
            });
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

// ── A3: the cap, and Quit with the store still stalled ───────────────────

#[test]
fn a3_at_the_cap_a_press_still_does_its_ingress_and_is_coalesced() {
    live(
        "a3_at_the_cap_a_press_still_does_its_ingress_and_is_coalesced",
        on_display(),
        || {
            let gate = StopGate::new();
            let started = launch_isolated("a3-cap", gated_broker(gate.clone()));
            let epoch = started.route.desk().epoch();
            let held = gate.clone();
            drive(&started, move |d| {
                held.arm();
                for n in 1..=MAX_STOP_WORKERS {
                    d.stop.click();
                    held.wait_entered(n);
                }
                assert_eq!(MAX_STOP_WORKERS, 4);
                assert_eq!(d.route.receipts().live, 4);
                // The fifth press.
                d.stop.click();
                wait_until("the fifth ingress", || d.route.desk().epoch() == epoch + 5);
                let receipts = d.route.receipts();
                assert_eq!(receipts.presses, 5);
                assert_eq!(receipts.live, 4, "the worker count stays at the cap");
                assert_eq!(receipts.dispatched, 4);
                assert_eq!(receipts.coalesced, 1, "the coalescing receipt");
                assert_eq!(held.entered(), 4);
                held.release();
                assert_eq!(
                    d.route.join_within(support::WAIT),
                    Joined::All { joined: 4 }
                );
            });
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

#[test]
fn a3_the_route_coalesces_at_the_cap_without_the_window() {
    live(
        "a3_the_route_coalesces_at_the_cap_without_the_window",
        Opts::default(),
        || {
            // The route alone (no GTK): the press's ingress and its receipts
            // are memory only.
            let gate = StopGate::new();
            let owner = gated_broker(gate.clone())(&support::home("a3-route")).unwrap();
            let desk = nexus_mother_desktop::stop::Desk::new();
            let route = nexus_mother_desktop::stop::StopRoute::new(desk.clone(), owner);
            gate.arm();
            for n in 1..=4u64 {
                assert!(matches!(route.press(), Press::Dispatched { epoch, .. } if epoch == n));
                gate.wait_entered(n as usize);
            }
            assert_eq!(route.press(), Press::Coalesced { epoch: 5 });
            assert_eq!(desk.epoch(), 5);
            assert_eq!(route.receipts().coalesced, 1);
            gate.release();
            assert_eq!(route.join_within(support::WAIT), Joined::All { joined: 4 });
            progress("route checked");
        },
    );
}

#[test]
fn a3_quit_with_the_store_still_stalled_is_never_clean() {
    live(
        "a3_quit_with_the_store_still_stalled_is_never_clean",
        on_display(),
        || {
            let gate = StopGate::new();
            let started = launch_isolated("a3-quit", gated_broker(gate.clone()));
            let held = gate.clone();
            drive(&started, move |d| {
                held.arm();
                d.stop.click();
                held.wait_entered(1);
            });
            let outcome = finish(started, Duration::from_millis(300));
            assert_eq!(
                outcome,
                QuitOutcome::Unclean {
                    stop_workers_running: 1,
                    shut_down: false
                }
            );
            assert!(!outcome.is_clean());
            assert_ne!(outcome.exit_code(), 0, "Quit exits non-zero");
            let report = outcome.report();
            assert!(report.contains("still running"), "{report}");
            assert!(!report.to_lowercase().contains("clean"), "{report}");
            // Only now is the store released: the worker was really stalled.
            assert_eq!(gate.left(), 0);
            gate.release();
        },
    );
}

// ── G13 (live): no bus name, no exported actions or menus ─────────────────

#[test]
fn g13_the_desktop_owns_no_bus_name_and_exports_nothing() {
    live(
        "g13_the_desktop_owns_no_bus_name_and_exports_nothing",
        Opts {
            display: true,
            dbus: true,
            env: vec![("NEXUS_DESKTOP_LIVE_DBUS", "1".to_string())],
            ..Opts::default()
        },
        || {
            assert!(std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some());
            let started = production("g13");
            drive(&started, |_| {
                let me = std::process::id().to_string();
                // A name may disappear between the listing and the query (the
                // query tools' own connections do): the live desktop's own
                // connection cannot, so a vanished name is not the desktop's.
                let bus = |method: &str, arg: Option<&str>| -> Option<String> {
                    let mut args = vec![
                        "--session".to_string(),
                        "--print-reply".to_string(),
                        "--dest=org.freedesktop.DBus".to_string(),
                        "/org/freedesktop/DBus".to_string(),
                        format!("org.freedesktop.DBus.{method}"),
                    ];
                    if let Some(arg) = arg {
                        args.push(format!("string:{arg}"));
                    }
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    support::try_tool("dbus-send", &args)
                };
                let names: Vec<String> = bus("ListNames", None)
                    .expect("the bus lists its names")
                    .lines()
                    .filter_map(|l| l.trim().strip_prefix("string \""))
                    .map(|l| l.trim_end_matches('"').to_string())
                    .collect();
                let mut mine = Vec::new();
                for name in &names {
                    if name == "org.freedesktop.DBus" {
                        continue;
                    }
                    let Some(pid) = bus("GetConnectionUnixProcessID", Some(name)) else {
                        continue;
                    };
                    if pid.lines().any(|l| l.trim() == format!("uint32 {me}")) {
                        mine.push(name.clone());
                    }
                }
                let owned: Vec<&String> = mine.iter().filter(|n| !n.starts_with(':')).collect();
                assert_eq!(owned, Vec::<&String>::new(), "the desktop owns a bus name");
                assert!(
                    !mine.is_empty(),
                    "the desktop's own connection was not found"
                );
                for connection in &mine {
                    let tree = support::run_tool(
                        "gdbus",
                        &[
                            "introspect",
                            "--session",
                            "--dest",
                            connection,
                            "--object-path",
                            "/",
                            "--recurse",
                        ],
                    );
                    for exported in ["org.gtk.Actions", "org.gtk.Menus"] {
                        assert!(!tree.contains(exported), "{connection} exports {exported}");
                    }
                }
                progress("bus checked");
            });
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}

// ── G14 (live): the content area holds labels, frames and the Stop button ──

#[test]
fn g14_the_content_area_is_labels_frames_and_the_stop_button() {
    live(
        "g14_the_content_area_is_labels_frames_and_the_stop_button",
        on_display(),
        || {
            let started = production("g14");
            drive(&started, |_| ());
            let mut buttons = Vec::new();
            let mut kinds = Vec::new();
            let mut todo: Vec<gtk::Widget> = vec![started.window.content.clone().upcast()];
            while let Some(widget) = todo.pop() {
                let kind = widget.type_().name().to_string();
                if kind == "GtkButton" {
                    buttons.push(widget.clone());
                }
                kinds.push(kind);
                let mut child = widget.first_child();
                while let Some(c) = child {
                    child = c.next_sibling();
                    todo.push(c);
                }
            }
            for kind in &kinds {
                assert!(
                    [
                        "GtkBox",
                        "GtkFrame",
                        "GtkLabel",
                        "GtkListBox",
                        "GtkListBoxRow",
                        "GtkButton"
                    ]
                    .contains(&kind.as_str()),
                    "{kind} in the content area: {kinds:?}"
                );
            }
            let stop: gtk::Widget = started.window.stop_button.clone().upcast();
            assert_eq!(buttons, vec![stop], "the Stop button is the only button");
            assert!(finish(started, QUIT_WITHIN).is_clean());
        },
    );
}
