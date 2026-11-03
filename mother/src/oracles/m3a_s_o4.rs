//! M3-O6's differential (design M3 §5.3, pre-registered): over S-O4's
//! 2,000 seeded pages, the second reader's title and leaf lines equal
//! S1a's own extraction (`Page::title`, `Leaf::text`), tag for tag. The
//! generator is the kernel oracle's, included unchanged; the corpus digest
//! is pinned as in S-O7, so the two cannot drift apart unnoticed.

use crate::verifier::read_visible_text;
use nexus_governed_control::inert_page::*;
use sha2::{Digest, Sha256};

/// S-O4's generator (`kernel/src/inert_page/tests/s_o4_round_trip.rs`).
#[path = "../../../kernel/src/inert_page/tests/s_o4_round_trip.rs"]
mod s_o4_round_trip;

/// The kernel oracles' local generator, restated for S-O4's included
/// generator (as in S-O7's host).
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

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const S_O4_SEED: u64 = 0x5EED_0004;
const S_O4_PAGES: usize = 2_000;
/// The same digest as `kernel/src/inert_page/tests/s_o7_corpus.rs`.
const S_O4_CORPUS_SHA256: &str = "3400966d8489c6b9176e03a201809b81c2321fd953e619bc38416e9672cd1ed5";

/// S1a's leaf lines of `page`: each leaf's tag and `Leaf::text`, in order
/// (also `m3a_verifier.rs`'s).
pub(super) fn s1a_lines(page: &Page) -> Vec<(&'static str, String)> {
    fn blocks(bs: &[Block], out: &mut Vec<(&'static str, String)>) {
        for b in bs {
            match b {
                Block::H1(l) => out.push(("h1", l.text())),
                Block::H2(l) => out.push(("h2", l.text())),
                Block::H3(l) => out.push(("h3", l.text())),
                Block::P(l) => out.push(("p", l.text())),
                Block::Address(l) => out.push(("address", l.text())),
                Block::Ul(list) | Block::Ol(list) => {
                    out.extend(list.items.iter().map(|li| ("li", li.text())));
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(h) = page.header() {
        blocks(&h.blocks, &mut out);
    }
    for item in &page.main().items {
        match item {
            MainItem::Section(s) => blocks(&s.blocks, &mut out),
            MainItem::Block(b) => blocks(std::slice::from_ref(b), &mut out),
        }
    }
    if let Some(f) = page.footer() {
        blocks(&f.blocks, &mut out);
    }
    out
}

#[test]
fn m3_o6_the_reader_agrees_with_s1a_on_s_o4_s_2000_pages() {
    let mut gen = s_o4_round_trip::Gen::new(S_O4_SEED);
    let trees: Vec<Page> = (0..S_O4_PAGES).map(|_| gen.page()).collect();
    let pages: Vec<Vec<u8>> = trees.iter().map(serialize).collect();
    assert_eq!(
        pages,
        s_o4_round_trip::generated_pages(S_O4_SEED, S_O4_PAGES)
    );
    let mut h = Sha256::new();
    for page in &pages {
        h.update((page.len() as u64).to_be_bytes());
        h.update(page);
    }
    assert_eq!(
        hex::encode(h.finalize()),
        S_O4_CORPUS_SHA256,
        "S-O4's corpus"
    );

    let mut failures = Vec::new();
    for (k, (tree, bytes)) in trees.iter().zip(&pages).enumerate() {
        let Ok(text) = read_visible_text(bytes) else {
            failures.push(format!("S-O4 page {k}: reader_unsupported"));
            continue;
        };
        if text.title != tree.title() {
            failures.push(format!("S-O4 page {k}: the title differs"));
        }
        let reader: Vec<(&str, &str)> = text
            .lines
            .iter()
            .map(|l| (l.tag.name(), l.text.as_str()))
            .collect();
        let s1a = s1a_lines(tree);
        let s1a: Vec<(&str, &str)> = s1a.iter().map(|(t, s)| (*t, s.as_str())).collect();
        if reader != s1a {
            let at = reader.iter().zip(&s1a).take_while(|(a, b)| a == b).count();
            failures.push(format!(
                "S-O4 page {k}: line {at} of {}/{}: reader {:?}, S1a {:?}",
                reader.len(),
                s1a.len(),
                reader.get(at),
                s1a.get(at)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {S_O4_PAGES} pages differ; the first: {}",
        failures.len(),
        failures
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
