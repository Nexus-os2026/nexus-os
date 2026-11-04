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
