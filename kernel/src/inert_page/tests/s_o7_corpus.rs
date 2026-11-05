//! S-O7's corpus tie (a fixture check, not a behavioural oracle). The HTML5
//! differential (`kernel/tests/s1a_html5_oracle.rs`) includes
//! S-O4's generator and runs over every S-O4 page; both pin the same digest
//! of that corpus, so the two cannot drift apart unnoticed.

use sha2::{Digest, Sha256};

use super::s_o4_round_trip::generated_pages;

/// SHA-256 over S-O4's 2,000 pages (seed `0x5EED_0004`), each prefixed by
/// its length as a u64 (big-endian). Also pinned in the differential.
const S_O4_CORPUS_SHA256: &str = "3400966d8489c6b9176e03a201809b81c2321fd953e619bc38416e9672cd1ed5";

#[test]
fn s_o7_corpus_is_s_o4s_corpus() {
    let mut h = Sha256::new();
    for page in generated_pages(0x5EED_0004, 2_000) {
        h.update((page.len() as u64).to_be_bytes());
        h.update(&page);
    }
    assert_eq!(hex::encode(h.finalize()), S_O4_CORPUS_SHA256);
}
