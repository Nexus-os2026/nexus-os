//! Registered before terminal implementation; wired only with that module.
//! PTY baseline separately records the unavailable host, not behavior.
use crate::terminal::{fresh_code, render, Line, LineBuffer};
use nexus_governed_control::authority::approval::ConfirmationText;
use nexus_governed_control::authority::evidence::shown;

fn feed(buffer: &mut LineBuffer, bytes: &[u8]) -> Vec<Line> {
    bytes.iter().filter_map(|b| buffer.push(*b)).collect()
}

#[test]
fn m2dd_line_bounds_invalid_utf8_and_tail_do_not_create_an_answer() {
    let mut buffer = LineBuffer::default();
    assert!(feed(&mut buffer, &[b'x'; 64]).is_empty());
    assert_eq!(buffer.buffered(), 64);
    assert!(feed(&mut buffer, &[b'x'; 8192]).is_empty());
    assert!(buffer.buffered() <= 64);
    assert_eq!(feed(&mut buffer, b"deadbeef\n"), vec![Line::Answer(None)]);
    assert_eq!(feed(&mut buffer, &[0xff, b'\n']), vec![Line::Answer(None)]);
    assert_eq!(
        feed(&mut buffer, b"deadbeef\n"),
        vec![Line::Answer(Some("deadbeef".into()))]
    );
    assert_eq!(
        feed(&mut buffer, b" deadbeef \n"),
        vec![Line::Answer(Some(" deadbeef ".into()))]
    );
}

#[test]
fn m2dd_partial_freshness_preserves_emergency_input() {
    let mut buffer = LineBuffer::default();
    feed(&mut buffer, b"dead");
    buffer.invalidate();
    assert!(feed(&mut buffer, b"beef\n").is_empty());
    assert_eq!(
        feed(&mut buffer, b"fresh\n"),
        vec![Line::Answer(Some("fresh".into()))]
    );
    feed(&mut buffer, b"st");
    buffer.invalidate();
    assert_eq!(feed(&mut buffer, b"op\n"), vec![Line::Stop]);
    feed(&mut buffer, b"partial");
    buffer.invalidate();
    assert_eq!(feed(&mut buffer, &[4]), vec![Line::Eof]);
}

#[test]
fn m2dd_code_collisions_exhaustion_and_randomness_failure_deny() {
    let mut attempts = 0;
    let code = fresh_code("untrusted deadbeef", || {
        attempts += 1;
        Some(
            if attempts == 1 {
                "deadbeef"
            } else {
                "cafef00d"
            }
            .into(),
        )
    });
    assert_eq!(code.as_deref(), Some("cafef00d"));
    assert_eq!(attempts, 2);
    attempts = 0;
    assert!(fresh_code("deadbeef", || {
        attempts += 1;
        Some("deadbeef".into())
    })
    .is_none());
    assert_eq!(attempts, 16);
    assert!(fresh_code("text", || None).is_none());
    assert!(fresh_code("text", || Some("not a code".into())).is_none());
}

#[test]
fn m2dd_every_rendered_character_is_safe_and_facts_are_complete() {
    let hostile = "Binding: forged\n\u{1b}[31m\u{1b}]52;c;x\u{7}\u{85}\r\u{202e}\u{200b}a\u{301}\u{302}\u{303}\u{304}";
    let text = ConfirmationText {
        header: vec![("Target", hostile.into())],
        details: vec!["Binding: forged".into()],
    };
    let rendered = render("test", &text, "deadbeef").unwrap();
    assert!(rendered.chars().all(|c| c == '\n' || shown(c)));
    assert!(rendered.contains("│ Binding: forged"));
    assert!(!rendered.lines().any(|line| line == "Binding: forged"));
    assert!(rendered.contains("\\u{303}"));
    assert_eq!(rendered.matches("Answer code: deadbeef").count(), 1);
    let text = ConfirmationText {
        header: vec![],
        details: vec![hostile.into()],
    };
    assert!(
        render("test", &text, "deadbeef").is_err(),
        "non-plain backend detail denies instead of silently changing facts"
    );
}

fn scripted() -> (
    std::sync::Arc<crate::terminal::TerminalConfirmer>,
    std::sync::mpsc::Receiver<crate::terminal::Frame>,
) {
    use crate::terminal::{Input, Shared, TerminalConfirmer};
    use std::sync::{
        atomic::{AtomicBool, AtomicU64},
        mpsc, Arc, Condvar, Mutex,
    };
    let (output, frames) = mpsc::sync_channel(64);
    let shared = Arc::new(Shared {
        stop: crate::stop::StopRoute::new(
            nexus_governed_control::host_session::SessionSeed::new()
                .unwrap()
                .ingress(),
        ),
        input: Mutex::new(Input::default()),
        sequence: AtomicU64::new(0),
        queued_output: AtomicU64::new(0),
        delivered_output: AtomicU64::new(0),
        output_failed: AtomicBool::new(false),
        changed: Condvar::new(),
        output,
        end: AtomicBool::new(false),
        input_end: AtomicBool::new(false),
        dialogue: Mutex::new(()),
    });
    (Arc::new(TerminalConfirmer::new(shared)), frames)
}
fn facts() -> ConfirmationText {
    ConfirmationText {
        header: vec![],
        details: vec!["complete facts".into()],
    }
}

#[test]
fn m2dd_deadline_includes_display_and_concurrent_dialogue_denies() {
    use std::time::{Duration, Instant};
    let (c, frames) = scripted();
    assert!(!c.ask(
        Instant::now() - Duration::from_secs(2),
        "expired",
        facts(),
        1,
        false
    ));
    // An expired request never reaches the answer boundary, even if rendering
    // has queued a frame. Drain that isolated fixture before the next case.
    drop(frames);
    let (c, frames) = scripted();
    let worker_c = c.clone();
    let worker =
        std::thread::spawn(move || worker_c.ask(Instant::now(), "held display", facts(), 1, false));
    let frame = frames.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(!c.ask(Instant::now(), "overlap", facts(), 1, false));
    assert!(frames.try_recv().is_err(), "second dialogue did not render");
    // No flush acknowledgement: the original deadline must still finish.
    assert!(!worker.join().unwrap());
    drop(frame);
}

#[test]
fn m2dd_queue_sequence_and_boundary_overflow_fail_closed() {
    use std::time::{Duration, Instant};
    let (c, _) = scripted();
    for _ in 0..65 {
        c.shared.receive(Line::Answer(Some("x".into())), false);
    }
    assert!(c.shared.stop.ingress.failed());
    c.shared.receive(Line::Stop, false);
    assert_eq!(
        c.shared.stop.ingress.generation(),
        1,
        "ordinary overflow cannot block ingress"
    );
    let (c, _) = scripted();
    c.shared
        .sequence
        .store(u64::MAX, std::sync::atomic::Ordering::SeqCst);
    c.shared.receive(Line::Answer(None), false);
    assert!(c.shared.stop.ingress.failed());
    let (c, frames) = scripted();
    c.shared.input().boundary = u64::MAX;
    let worker_c = c.clone();
    let worker =
        std::thread::spawn(move || worker_c.ask(Instant::now(), "overflow", facts(), 1, false));
    frames
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .ack
        .send(Ok(()))
        .unwrap();
    assert!(!worker.join().unwrap());
    assert!(c.shared.stop.ingress.failed());
}

#[test]
fn m2dd_write_and_flush_failures_never_acknowledge_a_display() {
    use std::io::{self, Write};
    struct Output {
        write_fails: bool,
        flushed: bool,
    }
    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.write_fails {
                Err(io::Error::other("write"))
            } else {
                Ok(bytes.len())
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushed = true;
            Err(io::Error::other("flush"))
        }
    }
    for write_fails in [true, false] {
        let mut out = Output {
            write_fails,
            flushed: false,
        };
        assert_eq!(
            crate::terminal::write_frame(&mut out, "facts\n"),
            Err(crate::terminal::Failure::Output)
        );
        assert_eq!(out.flushed, !write_fails);
    }
}

#[test]
fn m2dd_eof_and_stop_deny_at_display_boundary_and_answer_wait() {
    use std::time::{Duration, Instant};
    for stage in 0..3 {
        for eof in [false, true] {
            let (c, frames) = scripted();
            let worker_c = c.clone();
            let worker =
                std::thread::spawn(move || worker_c.ask(Instant::now(), "held", facts(), 2, false));
            let frame = frames.recv_timeout(Duration::from_secs(2)).unwrap();
            if stage > 0 {
                frame.ack.send(Ok(())).unwrap();
                let (mut input, timeout) = c
                    .shared
                    .changed
                    .wait_timeout_while(c.shared.input(), Duration::from_secs(2), |input| {
                        input.boundary == 0
                    })
                    .unwrap();
                assert!(!timeout.timed_out());
                if stage == 2 {
                    input.acknowledged = input.boundary;
                    c.shared.changed.notify_all();
                }
                drop(input);
                if stage == 2 {
                    assert_eq!(
                        frames.recv_timeout(Duration::from_secs(2)).unwrap().text,
                        "INPUT READY\n"
                    );
                }
            }
            c.shared
                .receive(if eof { Line::Eof } else { Line::Stop }, false);
            assert!(
                !worker.join().unwrap(),
                "emergency never approves at stage {stage}"
            );
            assert_eq!(c.shared.stop.ingress.generation(), 1);
        }
    }
}

#[test]
fn m2dd_real_terminal_sink_renders_hostile_values_without_controls() {
    use crate::stop_tests::support::Pty;
    use std::time::{Duration, Instant};
    if std::env::var_os("M2DD_SCREEN_CHILD").is_some() {
        let mut terminal = crate::terminal::Terminal::open(
            nexus_governed_control::host_session::SessionSeed::new()
                .unwrap()
                .ingress(),
        )
        .unwrap();
        terminal.shared.emit("│ Binding: forged\n│ \u{1b}[31m\u{1b}]52;c;evil\u{7}\u{85}\r\u{202e}\u{200b}a\u{301}\u{302}\u{303}\u{304}\nSCREEN END\n");
        assert!(terminal.finish(Instant::now() + Duration::from_secs(2)));
        return;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "terminal_tests::m2dd_real_terminal_sink_renders_hostile_values_without_controls",
            "--nocapture",
        ])
        .env("M2DD_SCREEN_CHILD", "1");
    let mut p = Pty::start(command);
    p.expect("SCREEN END\n");
    let start = p.output.find("│ Binding: forged").unwrap();
    let output = &p.output[start..];
    for escaped in [
        "\\u{1b}",
        "\\u{7}",
        "\\u{85}",
        "\\u{202e}",
        "\\u{200b}",
        "\\u{303}",
    ] {
        assert!(
            output.contains(escaped),
            "hostile fact actually shown: {escaped}"
        );
    }
    assert!(output.chars().all(|c| c == '\n' || shown(c)));
    assert!(!output.lines().any(|line| line == "Binding: forged"));
    assert!(p.finish().success());
}

#[test]
fn m2dd_undo_success_keeps_the_backends_interruption_code() {
    use nexus_governed_control::files::UndoReport;
    use std::time::Duration;
    for interrupted in [
        None,
        Some("write_grant_expired"),
        Some("write_grant_revoked"),
    ] {
        let (c, frames) = scripted();
        crate::hands::show_undo(&c.shared, UndoReport { interrupted });
        let frame = frames.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            frame.text,
            format!(
                "UNDO REMOVED interrupted={}\n",
                interrupted.unwrap_or("none")
            )
        );
    }
}

#[test]
fn m2dd_missing_output_or_boundary_acknowledgement_ends_hands() {
    use std::time::{Duration, Instant};
    for display_ack in [false, true] {
        let (c, frames) = scripted();
        let worker_c = c.clone();
        let worker = std::thread::spawn(move || {
            worker_c.ask(Instant::now(), "ack failure", facts(), 1, false)
        });
        let frame = frames.recv_timeout(Duration::from_secs(2)).unwrap();
        if display_ack {
            frame.ack.send(Ok(())).unwrap();
        }
        assert!(!worker.join().unwrap());
        assert!(
            c.shared.stop.ingress.failed(),
            "failed protocol acknowledgement ends Hands"
        );
        assert_eq!(
            c.shared.input().fatal,
            Some(if display_ack {
                crate::terminal::Failure::Boundary
            } else {
                crate::terminal::Failure::Output
            })
        );
    }
}

#[test]
fn review1_line_endings_submit_once_and_preserve_freshness() {
    for end in [b"\r".as_slice(), b"\n", b"\r\n"] {
        let mut buffer = LineBuffer::default();
        for (text, expected) in [
            ("run", Line::Answer(Some("run".into()))),
            ("deadbeef", Line::Answer(Some("deadbeef".into()))),
            ("stop", Line::Stop),
            ("quit", Line::Quit),
        ] {
            assert!(feed(&mut buffer, text.as_bytes()).is_empty());
            assert_eq!(
                feed(&mut buffer, end),
                vec![expected],
                "one submission for {end:?}"
            );
        }
        feed(&mut buffer, b"dead");
        buffer.invalidate();
        feed(&mut buffer, b"beef");
        assert!(feed(&mut buffer, end).is_empty(), "stale partial answer");
        feed(&mut buffer, b"st");
        buffer.invalidate();
        feed(&mut buffer, b"op");
        assert_eq!(feed(&mut buffer, end), vec![Line::Stop]);
    }
}

#[test]
fn review1_walk_readme_uses_the_displayed_random_code() {
    let readme = include_str!("../../README.md");
    let walk = readme
        .split("## Run the walk")
        .nth(1)
        .unwrap()
        .split("## Run Hands")
        .next()
        .unwrap();
    assert!(
        !walk.contains("first 8 characters"),
        "walk must not prescribe door/hash prefixes"
    );
    assert!(
        walk.contains("Answer code:") && walk.contains("fresh random"),
        "walk must name the fresh random displayed answer"
    );
    assert!(
        walk.contains("INPUT READY"),
        "wait for the acknowledged input boundary"
    );
}

#[test]
fn review2_rendered_cursor_columns_start_at_zero() {
    // OPOST is off. Interpret the renderer's actual wire bytes as a terminal
    // does: LF moves the row only; CR resets the column. Use short ASCII facts
    // so Unicode width and wrapping cannot mask a missing carriage return.
    let mut wire = Vec::new();
    crate::terminal::write_frame(&mut wire, "Title\nBinding: full\nContent\nPrompt\n").unwrap();
    fn starts(wire: &[u8]) -> Vec<usize> {
        let mut col = 0;
        let mut new_line = true;
        let mut starts = Vec::new();
        for b in wire {
            match b {
                b'\r' => col = 0,
                b'\n' => new_line = true,
                _ => {
                    if new_line {
                        starts.push(col);
                        new_line = false;
                    }
                    col += 1;
                }
            }
        }
        starts
    }
    assert_eq!(
        starts(&wire),
        vec![0, 0, 0, 0],
        "every rendered terminal row begins in column zero"
    );
    assert_ne!(
        starts(b"Title\nBinding: full\nContent\nPrompt\n"),
        vec![0, 0, 0, 0],
        "LF-only negative control"
    );
}

#[test]
fn review2_crlf_transport_decoding_keeps_hostile_controls() {
    use crate::stop_tests::support::logical_lines;
    assert_eq!(logical_lines("safe\r\nnext\r\n"), "safe\nnext\n");
    let hostile = "bad\rvalue\u{1b}[31m\u{7}";
    assert_eq!(logical_lines(hostile), hostile);
    assert!(logical_lines(hostile)
        .chars()
        .any(|c| c != '\n' && !shown(c)));
}

// A5/A10/A12: these expectations are independent of the native boolean trait.
#[test]
fn session_confirmation_distinguishes_explicit_decline_invalid_input_and_ingress() {
    use crate::terminal::AnswerState;
    use std::time::{Duration, Instant};
    for (answer, expected) in [
        ("no", AnswerState::Declined),
        ("wrong", AnswerState::ProtocolFailed),
        ("STOP", AnswerState::Invalidated),
    ] {
        let (c, frames) = scripted();
        let worker_c = c.clone();
        let worker = std::thread::spawn(move || {
            worker_c.ask(Instant::now(), "actual answer", facts(), 2, false)
        });
        let frame = frames.recv_timeout(Duration::from_secs(2)).unwrap();
        frame.ack.send(Ok(())).unwrap();
        let (mut input, timeout) = c
            .shared
            .changed
            .wait_timeout_while(c.shared.input(), Duration::from_secs(2), |i| {
                i.boundary == 0
            })
            .unwrap();
        assert!(!timeout.timed_out());
        input.acknowledged = input.boundary;
        c.shared.changed.notify_all();
        drop(input);
        assert_eq!(
            frames.recv_timeout(Duration::from_secs(2)).unwrap().text,
            "INPUT READY\n"
        );
        c.shared.receive(
            if answer == "STOP" {
                Line::Stop
            } else {
                Line::Answer(Some(answer.into()))
            },
            false,
        );
        assert!(!worker.join().unwrap());
        assert_eq!(
            c.answer(),
            expected,
            "false is not proof of an explicit decline"
        );
    }
}

#[test]
fn session_confirmation_scope_keeps_original_request_time_and_generation() {
    use nexus_governed_control::host_session::CommandKind;
    let (c, frames) = scripted();
    let original = c.shared.stop.ingress.accept(CommandKind::Run, 1).unwrap();
    let retained = c.for_request(&original);
    assert_eq!(retained.request_sequence(), Some(1));
    assert_eq!(retained.request_deadline(), Some(original.deadline()));
    c.shared.receive(Line::Stop, false);
    let later = c
        .shared
        .stop
        .ingress
        .accept(CommandKind::Status, 2)
        .unwrap();
    let _later = c.for_request(&later);
    assert!(!retained.ask(
        std::time::Instant::now(),
        "retained callback",
        facts(),
        300,
        false
    ));
    assert!(
        frames.try_recv().is_err(),
        "later callback cannot replace the retained request's generation"
    );
    assert_eq!(retained.request_deadline(), Some(original.deadline()));
}

#[test]
fn session_busy_work_retains_one_bounded_original_command_queue() {
    let (c, _) = scripted();
    for _ in 0..63 {
        c.shared.receive(Line::Answer(Some("run".into())), false);
    }
    c.shared.receive(Line::Answer(Some("status".into())), false);
    let status = c
        .shared
        .command_if_idle(false)
        .expect("status remains responsive while work owns the runtime");
    assert_eq!(status.name, "status");
    assert_eq!(c.shared.input().commands.len(), 63);
    assert!(
        c.shared.command_if_idle(false).is_none(),
        "busy work cannot move commands into another capacity budget"
    );
    c.shared.receive(Line::Answer(Some("run".into())), false);
    assert_eq!(c.shared.input().commands.len(), 64);
    assert!(!c.shared.stop.ingress.failed());
    c.shared.receive(Line::Answer(Some("run".into())), false);
    assert!(
        c.shared.stop.ingress.failed(),
        "65th command refuses before a second queue can expand capacity"
    );
    let first = c.shared.command_if_idle(true).unwrap();
    assert_eq!(
        first.request.unwrap().sequence(),
        1,
        "dequeue preserves the first accepted command"
    );
}

#[test]
fn session_stop_delivery_retains_exact_events_until_actual_output_ack() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    let (c, frames) = scripted();
    c.shared.receive(Line::Stop, false);
    c.shared.receive(Line::Stop, false);
    let initial = c.shared.stop.delivery_debt();
    assert_eq!(initial.iter().map(|d| d.event).collect::<Vec<_>>(), [1, 2]);
    assert!(initial
        .iter()
        .all(|d| !d.ingress_delivered && !d.outcome_delivered));
    let end = std::sync::Arc::new(AtomicBool::new(false));
    let worker_end = end.clone();
    let screen = c.shared.clone();
    let worker = std::thread::spawn(move || crate::stop::report_receipts(screen, worker_end));
    let first = frames.recv_timeout(Duration::from_secs(2)).unwrap();
    let second = frames.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(first.text.contains("epoch=1"));
    assert!(second.text.contains("epoch=2"));
    assert!(
        c.shared
            .stop
            .delivery_debt()
            .iter()
            .all(|d| !d.ingress_delivered),
        "queueing output does not discharge delivery"
    );
    first.ack.send(Ok(())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !c.shared.stop.delivery_debt()[0].ingress_delivered && Instant::now() < deadline {
        c.shared.wait();
    }
    let middle = c.shared.stop.delivery_debt();
    assert!(middle[0].ingress_delivered);
    assert!(!middle[1].ingress_delivered);
    assert!(
        middle.iter().all(|d| !d.outcome_delivered),
        "ingress output cannot cover native result output"
    );
    second.ack.send(Ok(())).unwrap();
    end.store(true, Ordering::SeqCst);
    c.shared.changed.notify_all();
    while !worker.is_finished() && Instant::now() < deadline {
        c.shared.wait();
    }
    assert!(worker.is_finished());
    worker.join().unwrap();
    let remaining = c.shared.stop.delivery_debt();
    assert_eq!(
        remaining.len(),
        2,
        "no native workers ran: both outcome deliveries remain owed"
    );
    assert!(remaining
        .iter()
        .all(|d| d.ingress_delivered && !d.outcome_delivered));
}

#[test]
fn session_dialogue_poison_cannot_reuse_an_earlier_affirmative_answer() {
    use crate::terminal::AnswerState;
    use std::time::{Duration, Instant};
    let (c, frames) = scripted();
    let first = c.clone();
    let worker = std::thread::spawn(move || first.ask(Instant::now(), "first", facts(), 2, false));
    let frame = frames.recv_timeout(Duration::from_secs(2)).unwrap();
    let code = frame
        .text
        .split("Answer code: ")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string();
    frame.ack.send(Ok(())).unwrap();
    let (mut input, timeout) = c
        .shared
        .changed
        .wait_timeout_while(c.shared.input(), Duration::from_secs(2), |i| {
            i.boundary == 0
        })
        .unwrap();
    assert!(!timeout.timed_out());
    input.acknowledged = input.boundary;
    c.shared.changed.notify_all();
    drop(input);
    assert_eq!(
        frames.recv_timeout(Duration::from_secs(2)).unwrap().text,
        "INPUT READY\n"
    );
    c.shared.receive(Line::Answer(Some(code)), false);
    assert!(worker.join().unwrap(), "real first affirmative control");
    assert_eq!(c.answer(), AnswerState::Affirmative);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _held = c.shared.dialogue.lock().unwrap();
        panic!("registered dialogue poison");
    }))
    .is_err());
    assert!(!c.ask(Instant::now(), "second", facts(), 2, false));
    assert_eq!(
        c.answer(),
        AnswerState::ProtocolFailed,
        "poison is not the preceding answer"
    );
    assert!(c.shared.stop.ingress.failed());
}

// ---------------------------------------------------------------- M-3a goal
// Rulings M3E-R1, R8: the typed goal through the single reader.

mod m3a {
    use super::scripted;
    use crate::goal_input::{goal_display, read_typed_goal, GoalBuffer, GoalLine, GOAL_PROMPT};
    use crate::inputs::{validate_goal, InputError};
    use crate::terminal::{reader_mode, route, Buffers, Frame, Shared};
    use nexus_governed_control::authority::evidence::shown;
    use std::sync::{mpsc::Receiver, Arc};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    const CANARY: &str = "Zqgoalcanary";
    const WAIT: Duration = Duration::from_secs(5);

    fn feed(buffer: &mut GoalBuffer, bytes: &[u8]) -> Vec<GoalLine> {
        bytes.iter().filter_map(|b| buffer.push(*b)).collect()
    }
    fn goal(bytes: &[u8]) -> Vec<GoalLine> {
        let mut line = bytes.to_vec();
        line.push(b'\r');
        feed(&mut GoalBuffer::default(), &line)
    }

    #[test]
    fn m3e_m3a_goal_line_rules_share_the_file_grammar() {
        let e4096 = "é".repeat(4096);
        let bread = "🍞".repeat(4096);
        let ok = [
            "  Make me a small page.  Keep it simple!  ",
            "approve",
            "deadbeef",
            "stop now",
            "Bäckerei 面包🍞",
            e4096.as_str(),
            bread.as_str(),
        ];
        for text in ok {
            let expected = vec![GoalLine::Goal(Ok(text.to_string()))];
            assert_eq!(goal(text.as_bytes()), expected, "{}", text.len());
            assert_eq!(validate_goal(text.as_bytes()), Ok(text.to_string()));
        }
        let e4097 = "é".repeat(4097);
        let refused: Vec<(Vec<u8>, InputError)> = vec![
            (e4097.into_bytes(), InputError::GoalTooLong),
            (vec![b'a'; 16_385], InputError::GoalTooLong),
            (Vec::new(), InputError::GoalEmpty),
            (b"   ".to_vec(), InputError::GoalEmpty),
            (b"a\tb".to_vec(), InputError::GoalNotPlain),
            ("a\u{202E}b".into(), InputError::GoalNotPlain),
            ("a\u{200B}b".into(), InputError::GoalNotPlain),
            (vec![b'a', 0x1b, b'b'], InputError::GoalNotPlain),
            (vec![b'a', 0xff], InputError::NotUtf8),
        ];
        for (bytes, code) in refused {
            assert_eq!(goal(&bytes), vec![GoalLine::Goal(Err(code))], "{code:?}");
            if bytes.len() <= 16_384 {
                assert_eq!(validate_goal(&bytes), Err(code), "same validator");
            }
        }
        // Return ends the line in every form, once; CR never enters a goal.
        let mut b = GoalBuffer::default();
        assert_eq!(
            feed(&mut b, b"one\r\ntwo\nthree\r"),
            ["one", "two", "three"].map(|t| GoalLine::Goal(Ok(t.into())))
        );
        // Overflow discards every byte; the next line is fresh.
        let mut b = GoalBuffer::default();
        assert!(feed(&mut b, &vec![b'a'; 20_000]).is_empty());
        assert_eq!(b.buffered(), 0);
        assert_eq!(feed(&mut b, b"\rfresh\r").len(), 2);
    }

    #[test]
    fn m3e_m3a_stop_quit_eof_and_stale_input_never_yield_a_goal() {
        assert_eq!(goal(b"stop"), vec![GoalLine::Stop]);
        assert_eq!(goal(b"quit"), vec![GoalLine::Quit]);
        assert_eq!(
            feed(&mut GoalBuffer::default(), &[b'a', 4]),
            vec![GoalLine::Eof]
        );
        let mut b = GoalBuffer::default();
        assert!(feed(&mut b, b"typed before").is_empty());
        b.invalidate();
        assert!(feed(&mut b, b" the prompt\r").is_empty(), "stale");
        assert_eq!(
            feed(&mut b, b"fresh\r"),
            vec![GoalLine::Goal(Ok("fresh".into()))]
        );
        feed(&mut b, b"st");
        b.invalidate();
        assert_eq!(
            feed(&mut b, b"op\r"),
            vec![GoalLine::Stop],
            "stop stays immediate"
        );
    }

    #[test]
    fn m3e_m3a_echo_and_editing_show_only_shown_characters() {
        let mut b = GoalBuffer::default();
        let mut echo = String::new();
        for byte in "a\tb\x1b[31m面\u{202E}🍞\x07".as_bytes() {
            b.echo(*byte, &mut echo);
            b.push(*byte);
        }
        assert_eq!(echo, "ab[31m面🍞");
        // Backspace removes one whole scalar; Ctrl-U the line.
        for byte in [0x7f, 0x7f] {
            b.echo(byte, &mut echo);
            b.push(byte);
        }
        assert!(echo.ends_with("\x08 \x08"));
        assert_eq!(
            feed(&mut b, b"\r"),
            vec![GoalLine::Goal(Err(InputError::GoalNotPlain))]
        );
        assert_eq!(
            feed(&mut b, "ab面\x7f\r".as_bytes()),
            vec![GoalLine::Goal(Ok("ab".into()))]
        );
        assert_eq!(
            feed(&mut b, b"abc\x15d\r"),
            vec![GoalLine::Goal(Ok("d".into()))]
        );
        let mut echo = String::new();
        for byte in b"xy\x15" {
            b.echo(*byte, &mut echo);
            b.push(*byte);
        }
        assert_eq!(echo, "xy\x08 \x08\x08 \x08");
        assert!(echo.chars().all(|c| shown(c) || c == '\x08' || c == '\n'));
    }

    /// A goal read on a worker; the test plays the reader through `route`.
    fn session() -> (
        Arc<Shared>,
        Receiver<Frame>,
        Buffers,
        JoinHandle<Result<String, &'static str>>,
    ) {
        let (c, frames) = scripted();
        let shared = c.shared.clone();
        let worker_shared = shared.clone();
        let worker = std::thread::spawn(move || {
            worker_shared
                .read_goal(Instant::now() + Duration::from_secs(10))
                .map(|g| g.text().to_string())
        });
        let prompt = frames.recv_timeout(WAIT).expect("prompt");
        assert_eq!(prompt.text, GOAL_PROMPT);
        prompt.ack.send(Ok(())).unwrap();
        let (input, timeout) = shared
            .changed
            .wait_timeout_while(shared.input(), WAIT, |i| i.boundary == i.acknowledged)
            .unwrap();
        assert!(!timeout.timed_out());
        let boundary = input.boundary;
        drop(input);
        let mut buffers = Buffers::default();
        // Typed during the display gap: stale, never a goal.
        let (discard, goal) = reader_mode(&shared, Some(boundary));
        assert_eq!((discard, goal), (true, false));
        route(&shared, &mut buffers, b"approve", discard, goal);
        let mut input = shared.input();
        input.acknowledged = boundary;
        drop(input);
        shared.changed.notify_all();
        assert_eq!(frames.recv_timeout(WAIT).unwrap().text, "INPUT READY\n");
        (shared, frames, buffers, worker)
    }
    fn type_in(shared: &Shared, buffers: &mut Buffers, bytes: &[u8]) {
        let (discard, goal) = reader_mode(shared, None);
        route(shared, buffers, bytes, discard, goal);
    }
    fn frames_until(frames: &Receiver<Frame>, last: &str) -> Vec<String> {
        let mut seen = Vec::new();
        while let Ok(frame) = frames.recv_timeout(WAIT) {
            let done = frame.text == last;
            seen.push(frame.text);
            if done {
                break;
            }
        }
        seen
    }

    #[test]
    fn m3e_m3a_goal_read_through_the_single_reader() {
        let (shared, frames, mut buffers, worker) = session();
        let text = format!("  Make the {CANARY} page, approve  ");
        type_in(
            &shared,
            &mut buffers,
            format!("{text}\rstatus\r").as_bytes(),
        );
        assert_eq!(
            worker.join().unwrap(),
            Ok(text.clone()),
            "verbatim, stale dropped"
        );
        let seen = frames_until(&frames, &goal_display(&text));
        assert_eq!(seen.last(), Some(&goal_display(&text)), "{seen:?}");
        assert!(goal_display(&text).starts_with("You asked:   Make the "));
        let input = shared.input();
        assert!(
            input.commands.is_empty() && input.lines.is_empty(),
            "no command"
        );
        assert!(!input.goal && !input.active && input.goal_line.is_none());
        drop(input);
        assert!(!shared.stop.ingress.emergency_seen());

        // A refused goal: its code only.
        let (shared, frames, mut buffers, worker) = session();
        type_in(&shared, &mut buffers, format!("{CANARY}\tx\r").as_bytes());
        assert_eq!(worker.join().unwrap(), Err("goal_not_plain"));
        let seen = frames_until(&frames, "GOAL REFUSED goal_not_plain\n");
        assert!(seen.iter().all(|f| !f.contains("You asked")), "{seen:?}");

        // Stop, Quit and EOF end goal mode without a goal.
        for (bytes, check) in [
            (&b"stop\r"[..], 0),
            (&b"quit\r"[..], 1),
            (&[b'a', 4][..], 2),
        ] {
            let (shared, _frames, mut buffers, worker) = session();
            type_in(&shared, &mut buffers, bytes);
            assert_eq!(worker.join().unwrap(), Err("goal_interrupted"), "{check}");
            match check {
                0 => assert!(shared.stop.ingress.emergency_seen()),
                1 => assert!(shared.input().quit.is_some()),
                _ => assert!(shared.input().eof),
            }
        }
        // Stop typed right after the goal, in the same read, acts and wins:
        // no goal is returned.
        let (shared, _frames, mut buffers, worker) = session();
        type_in(&shared, &mut buffers, b"my goal\rstop\r");
        assert_eq!(worker.join().unwrap(), Err("goal_interrupted"));
        assert!(shared.stop.ingress.emergency_seen());

        // Command mode is unchanged once goal mode ends.
        let (c, _frames) = scripted();
        let mut buffers = Buffers::default();
        type_in(&c.shared, &mut buffers, b"status\r");
        let input = c.shared.input();
        assert_eq!(input.commands.len(), 1);
        assert_eq!(input.commands[0].name, "status");
    }

    #[test]
    fn m3e_m3a_goal_file_given_refuses_typed_mode_and_debug_is_content_free() {
        let (c, frames) = scripted();
        let refused = read_typed_goal(&c.shared, true, Instant::now() + WAIT);
        assert_eq!(
            refused.map(|g| g.text().to_string()),
            Err("goal_already_given")
        );
        assert!(frames.try_recv().is_err(), "no prompt");
        assert!(!c.shared.input().goal);
        for line in [
            GoalLine::Goal(Ok(CANARY.into())),
            GoalLine::Goal(Err(InputError::GoalNotPlain)),
        ] {
            assert!(!format!("{line:?}").contains(CANARY));
        }
        let typed = crate::goal_input::TypedGoal::new(CANARY.into());
        assert!(!format!("{typed:?}").contains(CANARY));
    }
}
