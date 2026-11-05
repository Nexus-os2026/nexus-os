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
