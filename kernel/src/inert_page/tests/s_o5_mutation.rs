//! S-O5: 10,000 seeded single-byte mutations and 1,000 swaps of accepted
//! pages. Nothing panics, and every accepted mutant is canonical and passes
//! an independent naive oracle.

use super::s_o4_round_trip::generated_pages;
use super::*;

const NEEDLES: [&[u8]; 5] = [b"://", b"url(", b"@import", b"javascript:", b"data:"];
const ENTITIES: [&[u8]; 5] = [b"&amp;", b"&lt;", b"&gt;", b"&quot;", b"&#39;"];
const NAMES: [&[u8]; 22] = [
    b"header", b"main", b"footer", b"section", b"h1", b"h2", b"h3", b"p", b"address", b"ul", b"ol",
    b"li", b"strong", b"em", b"small", b"br", b"html", b"head", b"body", b"meta", b"title",
    b"style",
];

fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

/// Naive substring checks, independent of the validator's code: one
/// `<style`, no `<script`, no comment, no needle, only the five entities,
/// and every raw `<` opening an allowed lowercase name (or the doctype at
/// offset 0) with no ` on` inside the tag.
fn naive(m: &[u8]) -> Result<(), String> {
    let lower = m.to_ascii_lowercase();
    if count(&lower, b"<style") != 1 || count(&lower, b"<script") != 0 || count(m, b"<!--") != 0 {
        return Err("style, script or comment".into());
    }
    if let Some(n) = NEEDLES.iter().find(|n| count(&lower, n) != 0) {
        return Err(format!("needle {:?}", String::from_utf8_lossy(n)));
    }
    for (i, &b) in m.iter().enumerate() {
        if b == b'&' && !ENTITIES.iter().any(|e| m[i..].starts_with(e)) {
            return Err(format!("entity at {i}"));
        }
        if b != b'<' || (i == 0 && m.starts_with(b"<!doctype html>")) {
            continue;
        }
        let start = if m.get(i + 1) == Some(&b'/') {
            i + 2
        } else {
            i + 1
        };
        let end = start
            + m[start..]
                .iter()
                .take_while(|c| c.is_ascii_alphanumeric())
                .count();
        if !NAMES.contains(&&m[start..end]) {
            return Err(format!("tag name at {i}"));
        }
        let close = m[i..]
            .iter()
            .position(|&c| c == b'>')
            .map_or(m.len(), |k| i + k);
        if count(&lower[i..close], b" on") != 0 {
            return Err(format!("on-attribute at {i}"));
        }
    }
    Ok(())
}

#[test]
fn s_o5_mutants_never_panic_and_accepted_mutants_are_canonical_and_naive_clean() {
    let mut seeds = vec![REFERENCE.as_bytes().to_vec()];
    seeds.extend(generated_pages(0x5EED_0005, 20));
    for seed in &seeds {
        assert!(check(seed).is_ok(), "a seed page is accepted");
    }
    const BYTES: &[u8] = b"<>/=\"'&;!? \n-:(@#.aAeEoOnNsStT01\x00\x7f\x80\xc3\xa9\xe2\xff";
    let mut rng = Rng::new(0x5EED_0005);
    let mut accepted = 0usize;
    let mut run = |m: Vec<u8>| {
        let _ = lexical_findings(&m);
        if let Ok(page) = check(&m) {
            accepted += 1;
            assert_eq!(serialize(&page), m, "an accepted mutant is canonical");
            if let Err(why) = naive(&m) {
                panic!("an accepted mutant fails the naive oracle: {why}");
            }
        }
    };
    for _ in 0..10_000 {
        let mut m = rng.pick(&seeds).clone();
        let at = rng.below(m.len());
        let byte = if rng.chance(70) {
            *rng.pick(BYTES)
        } else {
            rng.below(256) as u8
        };
        match rng.below(3) {
            0 => m[at] = byte,
            1 => m.insert(at, byte),
            _ => {
                m.remove(at);
            }
        }
        run(m);
    }
    for k in 0..1_000 {
        let seed = rng.pick(&seeds);
        let m = if k % 2 == 0 {
            let mut m = seed.clone();
            let (a, b) = (rng.below(m.len()), rng.below(m.len()));
            m.swap(a, b);
            m
        } else {
            let mut lines: Vec<&[u8]> = seed.split_inclusive(|&b| b == b'\n').collect();
            let (a, b) = (rng.below(lines.len()), rng.below(lines.len()));
            lines.swap(a, b);
            lines.concat()
        };
        run(m);
    }
    assert!(accepted > 0, "some mutants (text edits) are accepted");
}
