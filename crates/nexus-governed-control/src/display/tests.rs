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
/// Xvfb is not installed.
fn display() -> Option<(AgentDisplay, TempRoot)> {
    if !std::path::Path::new("/usr/bin/Xvfb").exists() {
        eprintln!("Xvfb is not installed: live agent-display tests are skipped");
        return None;
    }
    let root = temp_root("display");
    let display = AgentDisplay::new(root.0.clone());
    display.start(640, 480).unwrap();
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
    display.start(640, 480).unwrap();
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
