//! B-R1-6: a SOURCE-DERIVED MODEL of the helper serial allocation in
//! crates/nexus-verifier-sandbox/src/launcher.rs at bb0dfec0 (standard
//! library only; not the production launcher):
//!
//!     static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);
//!     ...
//!     serial: NEXT_SERIAL.fetch_add(1, Ordering::Relaxed),
//!
//! It starts a local counter where the global one would stand after
//! 2^64 - 3 allocations (no 2^64 spawns are attempted), applies the same
//! expression, and shows what it hands out.
use std::sync::atomic::{AtomicU64, Ordering};

fn main() {
    let first_ever = AtomicU64::new(1).load(Ordering::Relaxed);
    let next = AtomicU64::new(u64::MAX - 2);
    let issued: Vec<u64> = (0..5).map(|_| next.fetch_add(1, Ordering::Relaxed)).collect();
    println!("identities issued near the end of the range: {issued:?}");
    let zero = issued.contains(&0);
    let reused = issued.contains(&first_ever);
    println!("zero issued as an identity: {zero}");
    println!("the first identity ever issued ({first_ever}) issued again: {reused}");
    println!("RESULT: the allocation wraps; it {} reuse identities", if reused { "does" } else { "does not" });
}
