//! Perception and input, live, on a private agent display (a backend-owned
//! Xvfb on a random display number with a private cookie). These tests
//! never address the owner's desktop: they read `DISPLAY` only to prove the
//! agent display is a different one.

use super::{AgentDisplay, Button, InputIntent, PerceptionIntent, Rect};
use crate::authority::commitment::CommitmentState;
use crate::authority::effect::EffectClass;
use crate::authority::AuthorityError;
use crate::control::EffectOutput;
use crate::harness_tests::{harness, temp_root, Harness, TempRoot, Yes};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux, EventMask, PropMode,
    WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

/// A started agent display (kept with its temporary root), or `None` where
/// Xvfb is not installed. CI installs it (ci.yml), so there a missing Xvfb
/// fails the tests instead of skipping them.
fn display() -> Option<(AgentDisplay, TempRoot)> {
    if !std::path::Path::new("/usr/bin/Xvfb").exists() {
        assert!(
            std::env::var_os("CI").is_none(),
            "Xvfb is missing: CI must exercise the live agent display"
        );
        eprintln!("Xvfb is not installed: live agent-display tests are skipped");
        return None;
    }
    let root = temp_root("display");
    let display = AgentDisplay::new(root.0.clone());
    display.start(640, 480, || Ok(()), || false).unwrap();
    Some((display, root))
}

fn grant_perception(h: &Harness) {
    h.control
        .authority()
        .grants()
        .request(
            AgentDisplay::perception_scope(),
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
}

fn grant_input(h: &Harness, steps: u32, session_r1: bool) {
    h.control
        .authority()
        .grants()
        .request(
            AgentDisplay::input_scope(steps, session_r1).unwrap(),
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
}

fn observe(
    h: &Harness,
    display: &AgentDisplay,
    intent: PerceptionIntent,
) -> Result<EffectOutput, AuthorityError> {
    let preparation = display.prepare_observation(h.control.authority(), h.run, &intent)?;
    let view = h.control.propose(&h.agent, h.run, preparation)?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))?;
    h.control.execute(view.id, &h.agent, h.run)
}

fn act(
    h: &Harness,
    display: &AgentDisplay,
    intent: InputIntent,
    approve: bool,
) -> Result<EffectOutput, AuthorityError> {
    let preparation = display.prepare_input(h.control.authority(), h.run, &intent)?;
    let view = h.control.propose(&h.agent, h.run, preparation)?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(approve))?;
    h.control.execute(view.id, &h.agent, h.run)
}

/// A test application window on the agent display.
fn window(conn: &RustConnection, title: &str, rect: Rect) -> u32 {
    let screen = &conn.setup().roots[0];
    let id = conn.generate_id().unwrap();
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        id,
        screen.root,
        rect.x as i16,
        rect.y as i16,
        rect.width,
        rect.height,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .override_redirect(1)
            .event_mask(EventMask::BUTTON_PRESS | EventMask::KEY_PRESS),
    )
    .unwrap();
    conn.change_property8(
        PropMode::REPLACE,
        id,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        title.as_bytes(),
    )
    .unwrap();
    conn.map_window(id).unwrap();
    conn.sync().unwrap();
    id
}

/// Events the test window received within a moment.
fn events(conn: &RustConnection) -> Vec<Event> {
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        match conn.poll_for_event().unwrap() {
            Some(event) => out.push(event),
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    out
}

#[test]
fn the_agent_display_is_private_and_is_not_the_owners() {
    let Some((display, _root)) = display() else {
        return;
    };
    let status = display.status().unwrap();
    assert!((200..1000).contains(&status.number));
    if let Ok(owner) = std::env::var("DISPLAY") {
        assert_ne!(
            owner.trim_start_matches(':').split('.').next(),
            Some(status.number.to_string().as_str())
        );
    }
    // Without the cookie, no connection.
    let stream =
        std::os::unix::net::UnixStream::connect(format!("/tmp/.X11-unix/X{}", status.number))
            .unwrap();
    let (stream, _) = x11rb::rust_connection::DefaultStream::from_unix_stream(stream).unwrap();
    assert!(
        RustConnection::connect_to_stream(stream, 0).is_err(),
        "an unauthenticated client is refused"
    );
    // No TCP listener, and no abstract socket.
    assert!(std::net::TcpStream::connect(("127.0.0.1", 6000 + status.number as u16)).is_err());
    {
        use std::os::linux::net::SocketAddrExt;
        let name = format!("/tmp/.X11-unix/X{}", status.number);
        let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
        assert!(
            std::os::unix::net::UnixStream::connect_addr(&address).is_err(),
            "an abstract socket answers"
        );
    }
    // Stopping ends it, and the server removes its lock file and socket.
    display.stop();
    assert!(display.status().is_none());
    let lock = format!("/tmp/.X{}-lock", status.number);
    let socket = format!("/tmp/.X11-unix/X{}", status.number);
    assert!(
        !std::path::Path::new(&lock).exists(),
        "{lock} is left behind"
    );
    assert!(
        !std::path::Path::new(&socket).exists(),
        "{socket} is left behind"
    );
}

/// Nexus connects to the agent display only through a socket this user owns
/// (not a link) in a directory no other user can replace entries of.
#[test]
fn only_this_users_socket_in_a_safe_directory_is_trusted() {
    use super::server::socket_trusted;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let root = temp_root("socket");
    let dir = root.0.path().join("x11");
    std::fs::create_dir(&dir).unwrap();
    let uid = std::fs::metadata(&dir).unwrap().uid();
    let socket = dir.join("X7");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let file = dir.join("X8");
    std::fs::write(&file, b"").unwrap();
    let link = dir.join("X9");
    std::os::unix::fs::symlink(&socket, &link).unwrap();
    let linked_dir = root.0.path().join("x11-link");
    std::os::unix::fs::symlink(&dir, &linked_dir).unwrap();
    let mode =
        |mode| std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
    mode(0o700);
    assert!(socket_trusted(&dir, &socket, uid));
    assert!(!socket_trusted(&dir, &socket, uid + 1), "another user's");
    assert!(!socket_trusted(&dir, &file, uid), "not a socket");
    assert!(!socket_trusted(&dir, &link, uid), "a link to a socket");
    assert!(
        !socket_trusted(&linked_dir, &linked_dir.join("X7"), uid),
        "a linked directory"
    );
    mode(0o1777);
    assert!(socket_trusted(&dir, &socket, uid), "a sticky directory");
    mode(0o777);
    assert!(!socket_trusted(&dir, &socket, uid), "anyone may replace it");
    mode(0o700);
    // The decision itself, for owners a test cannot create files as.
    use super::server::{trusted, Seen};
    let own_dir = Seen {
        dir: true,
        socket: false,
        uid: 1000,
        mode: 0o40700,
    };
    let own_socket = Seen {
        dir: false,
        socket: true,
        uid: 1000,
        mode: 0o140755,
    };
    assert!(trusted(own_dir, own_socket, 1000));
    assert!(
        !trusted(
            own_dir,
            Seen {
                uid: 1001,
                ..own_socket
            },
            1000
        ),
        "another user's socket in this user's directory"
    );
    let root_dir = |mode| Seen {
        uid: 0,
        mode,
        ..own_dir
    };
    assert!(
        trusted(root_dir(0o41777), own_socket, 1000),
        "root's sticky directory"
    );
    assert!(
        !trusted(root_dir(0o40777), own_socket, 1000),
        "root's directory anyone may change"
    );
    assert!(
        !trusted(
            Seen {
                uid: 1001,
                ..own_dir
            },
            own_socket,
            1000
        ),
        "another user's directory"
    );
    assert!(
        !trusted(
            own_dir,
            Seen {
                socket: false,
                ..own_socket
            },
            1000
        ),
        "not a socket"
    );
}

#[test]
fn observations_need_a_grant_and_leave_only_a_digest_in_evidence() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    assert_eq!(
        observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    grant_perception(&h);
    let out = observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let png = out.bytes.unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let digest = out
        .meta
        .iter()
        .find(|(k, _)| k == "sha256")
        .unwrap()
        .1
        .clone();
    let records = h.evidence.records();
    let finished = records.iter().rev().find(|r| r.outcome.is_some()).unwrap();
    assert_eq!(finished.class, Some(EffectClass::R0));
    assert!(
        finished.detail.iter().any(|(_, v)| *v == digest),
        "the digest is evidence"
    );
    // Only metadata reaches the evidence: never pixels, not even a prefix.
    let keys: std::collections::BTreeSet<&str> =
        finished.detail.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from(["format", "height", "observation", "sha256", "width"])
    );
    let head = hex::encode(&png[..24]);
    assert!(!format!("{records:?}").contains(&head), "never pixels");
    // A region outside the display is refused.
    assert!(observe(
        &h,
        &display,
        PerceptionIntent::Screen {
            region: Some(Rect {
                x: 600,
                y: 0,
                width: 100,
                height: 10
            })
        }
    )
    .is_err());
}

#[test]
fn a_window_is_bound_by_identity_and_a_title_only_selects_it() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    let client = display.test_client();
    let rect = Rect {
        x: 50,
        y: 60,
        width: 200,
        height: 100,
    };
    let id = window(&client, "Nexus Test Window", rect);
    observe(
        &h,
        &display,
        PerceptionIntent::Window {
            title: "Nexus Test Window".into(),
        },
    )
    .unwrap();
    // The window moves between preparation and execution: refused.
    let preparation = display
        .prepare_observation(
            h.control.authority(),
            h.run,
            &PerceptionIntent::Window {
                title: "Nexus Test Window".into(),
            },
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    client
        .configure_window(id, &ConfigureWindowAux::new().x(300))
        .unwrap();
    client.sync().unwrap();
    assert_eq!(
        h.control.execute(view.id, &h.agent, h.run).unwrap_err(),
        AuthorityError::TargetChanged
    );
    // Two windows with one title: no binding.
    window(
        &client,
        "Nexus Test Window",
        Rect {
            x: 10,
            y: 300,
            width: 50,
            height: 50,
        },
    );
    assert!(matches!(
        display.prepare_observation(
            h.control.authority(),
            h.run,
            &PerceptionIntent::Window {
                title: "Nexus Test Window".into()
            }
        ),
        Err(AuthorityError::InvalidAction(_))
    ));
}

#[test]
fn a_click_needs_an_observation_and_approval_and_lands_on_the_bound_window() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, false);
    let client = display.test_client();
    let id = window(
        &client,
        "Target",
        Rect {
            x: 100,
            y: 100,
            width: 200,
            height: 150,
        },
    );
    let click = InputIntent::Click {
        x: 150,
        y: 150,
        button: Some(Button::Left),
    };
    assert!(
        matches!(
            display.prepare_input(h.control.authority(), h.run, &click),
            Err(AuthorityError::InvalidAction(_))
        ),
        "observe first"
    );
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let preparation = display
        .prepare_input(h.control.authority(), h.run, &click)
        .unwrap();
    assert_eq!(preparation.action.class, EffectClass::R2);
    assert!(preparation.action.target.display.contains("Target"));
    assert_eq!(
        act(&h, &display, click.clone(), false).unwrap_err(),
        AuthorityError::Declined
    );
    assert!(events(&client).is_empty(), "a declined click is never sent");
    act(&h, &display, click, true).unwrap();
    let presses: Vec<_> = events(&client)
        .into_iter()
        .filter_map(|e| match e {
            Event::ButtonPress(p) => Some((p.event, p.event_x, p.event_y, p.detail)),
            _ => None,
        })
        .collect();
    assert_eq!(presses, vec![(id, 50, 50, 1)]);
}

#[test]
fn a_window_appearing_over_the_point_fails_the_click() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, true);
    let client = display.test_client();
    let target = window(
        &client,
        "Target",
        Rect {
            x: 100,
            y: 100,
            width: 200,
            height: 150,
        },
    );
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let preparation = display
        .prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Click {
                x: 150,
                y: 150,
                button: None,
            },
        )
        .unwrap();
    assert_eq!(
        preparation.action.class,
        EffectClass::R1,
        "an R1 input session"
    );
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    let popup = window(
        &client,
        "Popup",
        Rect {
            x: 120,
            y: 120,
            width: 80,
            height: 80,
        },
    );
    assert_eq!(
        h.control.execute(view.id, &h.agent, h.run).unwrap_err(),
        AuthorityError::TargetChanged
    );
    assert!(
        events(&client)
            .into_iter()
            .all(|e| !matches!(e, Event::ButtonPress(p) if p.event == target || p.event == popup)),
        "nothing was clicked"
    );
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Failed
    );
}

#[test]
fn typing_sends_exactly_the_text_to_the_window_under_the_pointer() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, true);
    let client = display.test_client();
    let id = window(
        &client,
        "Editor",
        Rect {
            x: 10,
            y: 10,
            width: 300,
            height: 200,
        },
    );
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    act(&h, &display, InputIntent::Move { x: 50, y: 50 }, true).unwrap();
    let preparation = display
        .prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Type { text: "Hi!".into() },
        )
        .unwrap();
    assert!(
        preparation.action.summary.iter().any(|l| l.contains("Hi!")),
        "the owner sees the text"
    );
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    h.control.execute(view.id, &h.agent, h.run).unwrap();
    let keys: Vec<u8> = events(&client)
        .into_iter()
        .filter_map(|e| match e {
            Event::KeyPress(k) if k.event == id => Some(k.detail),
            _ => None,
        })
        .collect();
    // Shift, h, i, Shift, 1 (as `!`): five presses.
    assert_eq!(keys.len(), 5, "{keys:?}");
    // The typed text itself is not in the evidence.
    assert!(!format!("{:?}", h.evidence.records()).contains("Hi!"));
    assert!(matches!(
        display.prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Type {
                text: "naïve".into()
            }
        ),
        Err(AuthorityError::InvalidAction(_))
    ));
}

#[test]
fn input_steps_are_bounded_by_the_grant() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 2, true);
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    act(&h, &display, InputIntent::Move { x: 1, y: 1 }, true).unwrap();
    act(&h, &display, InputIntent::Move { x: 2, y: 2 }, true).unwrap();
    assert_eq!(
        act(&h, &display, InputIntent::Move { x: 3, y: 3 }, true).unwrap_err(),
        AuthorityError::Closed("the input grant's steps are used up")
    );
}

#[test]
fn a_restarted_display_fails_everything_bound_to_the_old_one() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    let preparation = display
        .prepare_observation(
            h.control.authority(),
            h.run,
            &PerceptionIntent::Screen { region: None },
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    display.stop();
    display.start(640, 480, || Ok(()), || false).unwrap();
    assert_eq!(
        h.control.execute(view.id, &h.agent, h.run).unwrap_err(),
        AuthorityError::TargetChanged
    );
}

/// Keys go to the keyboard focus, not to the pointer: typing for the window
/// under the pointer fails while another window holds the focus, and
/// nothing is typed anywhere; with the focus on the bound window it types.
#[test]
fn typing_fails_while_another_window_holds_the_focus() {
    use x11rb::protocol::xproto::InputFocus;
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, true);
    let client = display.test_client();
    let rect = |x| Rect {
        x,
        y: 10,
        width: 200,
        height: 200,
    };
    let editor = window(&client, "Editor", rect(10));
    let thief = window(&client, "Thief", rect(300));
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    act(&h, &display, InputIntent::Move { x: 50, y: 50 }, true).unwrap();
    let typing = |text: &str| {
        let preparation = display
            .prepare_input(
                h.control.authority(),
                h.run,
                &InputIntent::Type { text: text.into() },
            )
            .unwrap();
        let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
        h.control
            .authorize(view.id, &h.agent, h.run, &Yes::new(true))
            .unwrap();
        view.id
    };
    let focus = |window| {
        client
            .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .unwrap();
        client.sync().unwrap();
    };
    let stolen = typing("1234");
    focus(thief);
    assert!(h.control.execute(stolen, &h.agent, h.run).is_err());
    assert!(
        events(&client)
            .into_iter()
            .all(|e| !matches!(e, Event::KeyPress(_))),
        "nothing was typed"
    );
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(stolen)
            .unwrap()
            .state,
        CommitmentState::Failed
    );
    let bound = typing("ok");
    focus(editor);
    h.control.execute(bound, &h.agent, h.run).unwrap();
    assert_eq!(
        events(&client)
            .into_iter()
            .filter(|e| matches!(e, Event::KeyPress(k) if k.event == editor))
            .count(),
        2
    );
}

/// The step budget is spent when an action happens: actions prepared and
/// approved together cannot exceed it.
#[test]
fn prepared_actions_cannot_exceed_the_step_budget() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 2, true);
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let moves: Vec<_> = (1..=3)
        .map(|i| {
            let preparation = display
                .prepare_input(
                    h.control.authority(),
                    h.run,
                    &InputIntent::Move { x: i, y: i },
                )
                .unwrap();
            let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
            h.control
                .authorize(view.id, &h.agent, h.run, &Yes::new(true))
                .unwrap();
            view.id
        })
        .collect();
    h.control.execute(moves[0], &h.agent, h.run).unwrap();
    h.control.execute(moves[1], &h.agent, h.run).unwrap();
    assert!(h.control.execute(moves[2], &h.agent, h.run).is_err());
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(moves[2])
            .unwrap()
            .state,
        CommitmentState::Failed
    );
    assert!(matches!(
        display.prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Move { x: 4, y: 4 }
        ),
        Err(AuthorityError::Closed(
            "the input grant's steps are used up"
        ))
    ));
}

/// Evidence first, and a stop wins: a display that cannot be recorded, or
/// that a stop overtakes while its server starts, does not run, and its
/// server ends unused.
#[test]
fn a_stop_wins_over_a_start_and_nothing_starts_unrecorded() {
    use std::sync::atomic::{AtomicU32, Ordering};
    if !std::path::Path::new("/usr/bin/Xvfb").exists() {
        assert!(
            std::env::var_os("CI").is_none(),
            "Xvfb is missing: CI must exercise the live agent display"
        );
        return;
    }
    let root = temp_root("display-start");
    let display = AgentDisplay::new(root.0.clone());
    assert_eq!(
        display.start(640, 480, || Ok(()), || true).unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert_eq!(
        display
            .start(
                640,
                480,
                || Err(AuthorityError::EvidenceUnavailable),
                || false
            )
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    // The stop arrives while the server starts (after the first check).
    let checks = AtomicU32::new(0);
    assert!(display
        .start(
            640,
            480,
            || Ok(()),
            || checks.fetch_add(1, Ordering::SeqCst) > 0
        )
        .is_err());
    assert!(display.status().is_none());
    assert_eq!(
        std::fs::read_dir(root.0.path()).unwrap().count(),
        0,
        "the overtaken server is gone"
    );
    // A plain stop while no display runs yet still overtakes a start in
    // progress, and says it stopped nothing.
    let mut stopped_nothing = false;
    assert_eq!(
        display
            .start(
                640,
                480,
                || {
                    stopped_nothing = display.stop().is_none();
                    Ok(())
                },
                || false
            )
            .unwrap_err(),
        AuthorityError::Closed("the agent display was stopped while it started")
    );
    assert!(stopped_nothing);
    assert!(display.status().is_none());
    assert_eq!(std::fs::read_dir(root.0.path()).unwrap().count(), 0);
}

/// A drag whose press point changed too cannot be dropped back there: it is
/// cancelled with Escape, with the server held, before its button goes.
#[test]
fn a_drag_that_cannot_go_back_is_cancelled_before_its_release() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, true);
    let escape = display.current().unwrap().1.keycode(0xff1b).unwrap().0;
    let client = display.test_client();
    let screen = client.setup().roots[0].clone();
    let source = client.generate_id().unwrap();
    client
        .create_window(
            COPY_DEPTH_FROM_PARENT,
            source,
            screen.root,
            10,
            10,
            100,
            100,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new()
                .background_pixel(screen.white_pixel)
                .override_redirect(1)
                .event_mask(EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE),
        )
        .unwrap();
    client.map_window(source).unwrap();
    client.sync().unwrap();
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let preparation = display
        .prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Drag {
                from_x: 50,
                from_y: 50,
                to_x: 400,
                to_y: 300,
            },
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    // The moment the button goes down, windows cover the drop point and the
    // press point.
    let watcher = std::thread::spawn(move || {
        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match client.poll_for_event().unwrap() {
                Some(Event::ButtonPress(_)) => {
                    seen.push("press".to_string());
                    for (title, x, y) in [("Cover", 350, 250), ("Over", 30, 30)] {
                        window(
                            &client,
                            title,
                            Rect {
                                x,
                                y,
                                width: 100,
                                height: 100,
                            },
                        );
                    }
                }
                Some(Event::KeyPress(key)) => seen.push(format!("key {}", key.detail)),
                Some(Event::ButtonRelease(release)) => {
                    seen.push(format!("release {} {}", release.root_x, release.root_y));
                    break;
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        seen
    });
    assert!(h.control.execute(view.id, &h.agent, h.run).is_err());
    assert_eq!(
        watcher.join().unwrap(),
        [
            "press".to_string(),
            format!("key {escape}"),
            "release 50 50".to_string()
        ]
    );
}

/// Keys bound to the display background are refused while a window holds
/// the focus, and no key or click is sent while another client holds the
/// keyboard or the pointer grabbed (it would receive them).
#[test]
fn background_keys_and_grabbed_input_are_refused() {
    use x11rb::protocol::xproto::{GrabMode, GrabStatus, InputFocus};
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 20, true);
    let client = display.test_client();
    let window_rect = Rect {
        x: 300,
        y: 300,
        width: 100,
        height: 100,
    };
    let focused = window(&client, "Focused", window_rect);
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    act(&h, &display, InputIntent::Move { x: 20, y: 20 }, true).unwrap();
    client
        .set_input_focus(InputFocus::PARENT, focused, x11rb::CURRENT_TIME)
        .unwrap();
    client.sync().unwrap();
    assert!(act(&h, &display, InputIntent::Type { text: "x".into() }, true).is_err());
    assert!(events(&client)
        .into_iter()
        .all(|e| !matches!(e, Event::KeyPress(_))));
    // Over the window, with the focus there, but the keyboard grabbed.
    act(&h, &display, InputIntent::Move { x: 350, y: 350 }, true).unwrap();
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let grab = client
        .grab_keyboard(
            true,
            focused,
            x11rb::CURRENT_TIME,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )
        .unwrap()
        .reply()
        .unwrap();
    assert_eq!(grab.status, GrabStatus::SUCCESS);
    assert!(act(&h, &display, InputIntent::Type { text: "y".into() }, true).is_err());
    assert!(act(
        &h,
        &display,
        InputIntent::Click {
            x: 350,
            y: 350,
            button: Some(Button::Left)
        },
        true
    )
    .is_err());
    assert!(events(&client)
        .into_iter()
        .all(|e| !matches!(e, Event::KeyPress(_) | Event::ButtonPress(_))));
}

/// The input grant tells the owner what never asks: moves and scrolls.
#[test]
fn the_input_grant_says_what_never_asks() {
    for (session_r1, gestures) in [
        (
            false,
            "Every click, drag and key: asks for your approval (R2)",
        ),
        (
            true,
            "Clicks, drags and keys: without asking you each time (R1)",
        ),
    ] {
        let lines = AgentDisplay::input_scope(10, session_r1)
            .unwrap()
            .describe();
        assert_eq!(
            lines[2..],
            [
                gestures.to_string(),
                "Pointer moves and scrolls: never ask you (R1)".to_string()
            ]
        );
    }
}

/// A window title cannot close the quotes the owner reads it between.
#[test]
fn a_window_title_cannot_close_its_quotes() {
    assert_eq!(
        super::quoted_title(r#"Bank" (id 7), then "Mail"#),
        r#"Bank\" (id 7), then \"Mail"#
    );
    assert_eq!(super::quoted_title(r#"back\" slash"#), r#"back\\\" slash"#);
    // A quote past a cluster's shown marks is escaped as a code point,
    // never as `\"`, so it cannot pass for the end of the title.
    let reph = "\u{0D4E}".repeat(3);
    assert_eq!(
        super::quoted_title(&format!("Trash{reph}\"x")),
        format!("Trash{reph}\\u{{22}}x")
    );
}

/// The owner reads a window's title escaped exactly once: a title that
/// imitates the rest of the target line cannot close its quotes there.
#[test]
fn a_target_line_shows_a_title_escaped_once() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, true);
    let client = display.test_client();
    let id = window(
        &client,
        r#"Mail" (id 12) on agent display 201, and window "Bank"#,
        Rect {
            x: 10,
            y: 10,
            width: 100,
            height: 100,
        },
    );
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let preparation = display
        .prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Click {
                x: 50,
                y: 50,
                button: Some(Button::Left),
            },
        )
        .unwrap();
    let number = display.status().unwrap().number;
    assert_eq!(
        preparation.action.target.display,
        format!(
            r#"window "Mail\" (id 12) on agent display 201, and window \"Bank" (id {id}) on agent display {number}"#
        )
    );
}

/// Whatever an action leaves pressed when it ends early is released.
#[test]
fn whatever_an_action_left_pressed_is_released() {
    use x11rb::protocol::xproto::KeyButMask;
    let Some((display, _root)) = display() else {
        return;
    };
    let (_, server) = display.current().unwrap();
    let shift = server.keycode(0xffe1).expect("a Shift key").0;
    server.key(shift, true).unwrap();
    server.button(1, true).unwrap();
    drop(super::Pressed {
        server: Some(server.clone()),
        keys: vec![shift],
        buttons: vec![1],
        pressed_at: None,
        escape: None,
    });
    let client = display.test_client();
    let keys = client.query_keymap().unwrap().reply().unwrap().keys;
    assert_eq!(keys[usize::from(shift / 8)] & (1 << (shift % 8)), 0);
    let root = client.setup().roots[0].root;
    let pointer = client.query_pointer(root).unwrap().reply().unwrap();
    assert!(!pointer.mask.contains(KeyButMask::BUTTON1));
    // A button pressed somewhere else is let go where it was pressed.
    server.pointer(40, 40).unwrap();
    server.button(1, true).unwrap();
    server.pointer(300, 200).unwrap();
    drop(super::Pressed {
        server: Some(server.clone()),
        keys: Vec::new(),
        buttons: vec![1],
        pressed_at: Some(((40, 40), 0)),
        escape: None,
    });
    let pointer = client.query_pointer(root).unwrap().reply().unwrap();
    assert!(!pointer.mask.contains(KeyButMask::BUTTON1));
    assert_eq!((pointer.root_x, pointer.root_y), (40, 40));
}

/// A drag's drop is checked when it happens: a window appearing at the drop
/// point while the button is down fails the drag, and the button is let go
/// where it was pressed, so nothing is dropped anywhere new.
#[test]
fn an_interrupted_drag_drops_back_where_it_was_picked_up() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, true);
    let client = display.test_client();
    let screen = client.setup().roots[0].clone();
    let source = client.generate_id().unwrap();
    client
        .create_window(
            COPY_DEPTH_FROM_PARENT,
            source,
            screen.root,
            10,
            10,
            100,
            100,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new()
                .background_pixel(screen.white_pixel)
                .override_redirect(1)
                .event_mask(EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE),
        )
        .unwrap();
    client.map_window(source).unwrap();
    client.sync().unwrap();
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let preparation = display
        .prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Drag {
                from_x: 50,
                from_y: 50,
                to_x: 400,
                to_y: 300,
            },
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    // The moment the button goes down, a window covers the drop point.
    let watcher = std::thread::spawn(move || {
        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match client.poll_for_event().unwrap() {
                Some(Event::ButtonPress(press)) => {
                    seen.push((true, press.root_x, press.root_y));
                    window(
                        &client,
                        "Cover",
                        Rect {
                            x: 350,
                            y: 250,
                            width: 100,
                            height: 100,
                        },
                    );
                }
                Some(Event::ButtonRelease(release)) => {
                    seen.push((false, release.root_x, release.root_y));
                    break;
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        seen
    });
    assert_eq!(
        h.control.execute(view.id, &h.agent, h.run).unwrap_err(),
        AuthorityError::Unavailable("the effect failed")
    );
    assert_eq!(
        watcher.join().unwrap(),
        vec![(true, 50, 50), (false, 50, 50)]
    );
    let records = h.evidence.records();
    let finished = records.iter().rev().find(|r| r.outcome.is_some()).unwrap();
    assert_eq!(finished.failure, Some("target_changed"));
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Failed
    );
}

/// A drag is bound to where it is dropped as well: the owner is told, and
/// a window appearing at the drop point fails the drag before anything is
/// pressed.
#[test]
fn a_drag_is_bound_to_where_it_is_dropped() {
    let Some((display, _root)) = display() else {
        return;
    };
    let h = harness();
    grant_perception(&h);
    grant_input(&h, 10, true);
    let client = display.test_client();
    window(
        &client,
        "Source",
        Rect {
            x: 10,
            y: 10,
            width: 100,
            height: 100,
        },
    );
    observe(&h, &display, PerceptionIntent::Screen { region: None }).unwrap();
    let preparation = display
        .prepare_input(
            h.control.authority(),
            h.run,
            &InputIntent::Drag {
                from_x: 50,
                from_y: 50,
                to_x: 400,
                to_y: 300,
            },
        )
        .unwrap();
    assert!(preparation
        .action
        .summary
        .iter()
        .any(|line| line == "Dropped on the display background"));
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    window(
        &client,
        "Cover",
        Rect {
            x: 350,
            y: 250,
            width: 100,
            height: 100,
        },
    );
    assert!(h.control.execute(view.id, &h.agent, h.run).is_err());
    assert!(events(&client)
        .into_iter()
        .all(|e| !matches!(e, Event::ButtonPress(_))));
}
