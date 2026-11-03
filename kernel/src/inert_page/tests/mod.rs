//! S1a-A oracles (pre-registered; design v2 §8): S-O1, S-O1b, S-O2 to S-O6
//! and S-O18. S1a-B oracles: S-O3 (facts part), S-O9, S-O10, S-O13 (verdict
//! part) and S-O15, with S-O7's corpus tie (S-O7 and S-O8 are hosted in
//! `agents/web-builder/tests/`).

use super::*;

mod facts_fixtures;
mod s_o10_facts;
mod s_o15_bound;
mod s_o18_label_guard;
mod s_o1_policy;
mod s_o2_planted;
mod s_o3_bounds;
mod s_o3_facts_bounds;
mod s_o4_round_trip;
mod s_o5_mutation;
mod s_o6_canonical;
mod s_o7_corpus;
mod s_o9_binding;

/// The reference page (design §4.6).
const REFERENCE: &str = include_str!("../fixtures/reference.html");

/// `base` with its one occurrence of `from` replaced by `to`.
fn edit_in(base: &str, from: &str, to: &str) -> String {
    assert_eq!(base.matches(from).count(), 1, "anchor {from:?}");
    base.replacen(from, to, 1)
}

/// The reference page with one edit.
fn edit(from: &str, to: &str) -> String {
    edit_in(REFERENCE, from, to)
}

/// The refusal code of `input`, or `None` if it is accepted.
fn code_of(input: &[u8]) -> Option<RefusalCode> {
    check(input).err().map(|r| r.code)
}

/// The reference page with one more paragraph before `</main>`, whose text
/// pads the page to exactly `len` bytes (words of 40 letters).
fn padded_to(len: usize) -> String {
    let fixed = REFERENCE.len() + "<p></p>\n".len();
    let text: String = (0..len - fixed)
        .map(|i| if i % 41 == 40 { ' ' } else { 'a' })
        .collect();
    assert!(!text.ends_with(' '));
    let page = edit("</main>\n", &format!("<p>{text}</p>\n</main>\n"));
    assert_eq!(page.len(), len);
    page
}

/// The number of start tags in the reference page's body.
fn reference_body_elements() -> usize {
    let body =
        &REFERENCE[REFERENCE.find("<body>\n").unwrap() + 7..REFERENCE.find("</body>").unwrap()];
    body.matches('<').count() - body.matches("</").count()
}

/// A local xorshift64* generator (no dependency).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `0..n` (`n > 0`).
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// True with probability `percent`/100.
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}
