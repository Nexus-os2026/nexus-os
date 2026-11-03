//! S-O15: the check is linear. A deterministic work counter (test builds
//! only) counts every bounded step of `PageVerdict::evaluate`: S1 to S4, the
//! facts check and the candidate's SHA-256. On four adversarial 64 KiB
//! generators (maximum elements and depth; all-entity text; near-miss
//! needles; `<`-dense input) it stays at or below `64·n + 4096`.
//!
//! Each generator's outcome is pinned, so none is refused early by accident,
//! and each count is at least `n`, so the counter is live. The wall time is
//! printed (a release build's goes in the PR); a debug build must stay under
//! 1 s per input.

use std::time::{Duration, Instant};

use super::super::work;
use super::facts_fixtures::*;
use super::*;

const N: usize = 65_536;

/// Words of `a` (at most 40 characters, single spaces), exactly `len`
/// characters long.
fn filler(len: usize) -> String {
    let mut out = String::new();
    let mut left = len;
    while left > 0 {
        match left {
            1..=40 => {
                out.push_str(&"a".repeat(left));
                left = 0;
            }
            41 => {
                out.push_str(&"a".repeat(39));
                out.push_str(" a");
                left = 0;
            }
            _ => {
                out.push_str(&"a".repeat(40));
                out.push(' ');
                left -= 41;
            }
        }
    }
    out
}

/// The reference head, a body of `before` then a filler paragraph then
/// `after`, and the epilogue: exactly `N` bytes.
fn exact_page(before: &str, after: &str) -> String {
    let head = &REFERENCE[..REFERENCE.find("<header>").unwrap()];
    let epilogue = "</body>\n</html>\n";
    let fixed = head.len() + before.len() + "<p></p>\n".len() + after.len() + epilogue.len();
    assert!(fixed < N, "the generator overflows: {fixed}");
    let page = format!(
        "{head}{before}<p>{}</p>\n{after}{epilogue}",
        filler(N - fixed)
    );
    assert_eq!(page.len(), N);
    page
}

const HEADER: &str = "<header>\n<h1>Rosie&#39;s Bakery</h1>\n</header>\n<main>\n<section>\n";
const HOURS_LIST: &str = "<ul class=\"plain\">\n<li>Monday to Friday: 07:00 – 17:00</li>\n\
                          <li>Saturday: 08:00 – 14:00</li>\n<li>Sunday: closed</li>\n</ul>\n";
const PHONE_P: &str = "<p>Phone: <strong>01632 960123</strong></p>\n";
const CLOSE: &str = "</section>\n</main>\n";

/// 400 elements and depth 8: one leaf of 388 spans, mostly in chains of
/// five nested spans down to depth 8, each holding the phone number and long
/// text; the exact phone comes after it, so FC-3 looks at every span.
fn max_elements_and_depth() -> String {
    let words = filler(600);
    let chain = format!(
        "<strong><em><small><strong><em>01632 960123 {words}</em></strong></small></em></strong>"
    );
    let chains = vec![chain; 77].join(" ");
    let extra = " <small>x</small> <small>y</small> <small>z</small>";
    let before = format!("{HEADER}{HOURS_LIST}<p>{chains}{extra}</p>\n{PHONE_P}");
    let page = exact_page(&before, CLOSE);
    assert_eq!(
        page.matches('<').count() - page.matches("</").count(),
        400 + 8,
        "400 body elements (and eight start tags in the prologue)"
    );
    page
}

/// Text that is all entities.
fn all_entity_text() -> String {
    let word = "&amp;&lt;&gt;&quot;&#39;&amp;&lt;&gt;&quot;&#39;";
    let para = format!("<p>{}</p>\n", [word; 15].join(" "));
    let before = format!("{HEADER}{HOURS_LIST}{PHONE_P}{}", para.repeat(80));
    exact_page(&before, CLOSE)
}

/// Near misses everywhere: of every needle, of the hours lines (same
/// length, last character different; the real lines come last), and of a
/// phone-like token (six digits).
fn near_miss_needles() -> String {
    let near = "javascript data url @impor :/ /: javascrip: dat: url) @IMPOR :/ 123 456";
    let misses = [
        "Monday to Friday: 07:00 – 17:01",
        "Saturday: 08:00 – 14:01",
        "Sunday: closeD",
    ];
    let mut items = String::new();
    for k in 0..330 {
        items.push_str(&format!("<li>{}</li>\n", misses[k % 3]));
    }
    let list = format!(
        "<ul class=\"plain\">\n{items}<li>Monday to Friday: 07:00 – 17:00</li>\n\
         <li>Saturday: 08:00 – 14:00</li>\n<li>Sunday: closed</li>\n</ul>\n"
    );
    let para = format!("<p>{}</p>\n", [near; 9].join(" "));
    let before = format!("{HEADER}{list}{}{PHONE_P}", para.repeat(50));
    exact_page(&before, CLOSE)
}

/// Every other byte a `<`: tag starts, end tags, attributes, declarations,
/// processing instructions and stray `<`.
fn lt_dense() -> Vec<u8> {
    let unit = b"<a b c d><b/></c d><!x><?y><<p <q\n<r s=\"<\"><t u='<'>";
    let mut out: Vec<u8> = unit.iter().copied().cycle().take(N - 1).collect();
    out.push(b'\n');
    out
}

#[test]
fn s_o15_work_is_linear_on_four_adversarial_generators() {
    let facts = reference_facts();
    let inputs: [(&str, Vec<u8>, bool); 4] = [
        (
            "max elements and depth",
            max_elements_and_depth().into(),
            true,
        ),
        ("all-entity text", all_entity_text().into(), true),
        ("near-miss needles", near_miss_needles().into(), true),
        ("<-dense input", lt_dense(), false),
    ];
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    for (name, input, passes) in &inputs {
        let n = input.len();
        assert_eq!(n, N, "{name}");
        work::take();
        let start = Instant::now();
        let v = PageVerdict::evaluate(input, Some(&facts));
        let elapsed = start.elapsed();
        let steps = work::take();
        eprintln!("S-O15 {name}: {n} bytes, {steps} steps, {elapsed:?} ({profile})");
        if *passes {
            assert_eq!(v.outcome, PageOutcome::Passed, "{name}");
        } else {
            assert!(
                matches!(v.outcome, PageOutcome::Refused(_)),
                "{name}: {:?}",
                v.outcome
            );
        }
        assert!(steps >= n as u64, "{name}: the counter is live ({steps})");
        assert!(
            steps <= 64 * n as u64 + 4096,
            "{name}: {steps} steps for {n} bytes"
        );
        if cfg!(debug_assertions) {
            assert!(
                elapsed < Duration::from_secs(1),
                "{name}: {elapsed:?} in a debug build"
            );
        }
    }
}
