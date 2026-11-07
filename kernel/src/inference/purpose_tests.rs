//! M3-Cq Cq-0 oracles C0-1..C0-4 (register: docs/security/m3-cq-oracles.md).

use super::purpose::{
    find, freeze_floor, no_duplicate, qualified_floor, Floor, Purpose, QualificationRecord,
    QUALIFICATIONS,
};

const SOURCE: &str = include_str!("purpose.rs");

/// Spec v2 §3.1: the identity v2 digests of the fake pin per purpose, and
/// the retired v1 digest.
const PAGE_BUILDER_V2: &str = "c5b838e3070d2ffcfb20f15e4420cca44c4f17456ff08a2232c0cd2a91c5b978";
const N1_WORKER_V2: &str = "5fad3aa48de2db319cb39dc9659aa54eeb75807c3ec5c88f2174adc010df9c9c";
const JUDGE_V2: &str = "b46c2ff5a58505f7f8a01cbb24a50818683dc1ad51a2ec55733aec506a008584";
const IDENTITY_V1: &str = "4d7211f035868b327c0e10f2bc23155a7d492d7beecc5b505edfa373db82f9a4";

const PURPOSES: [Purpose; 3] = [Purpose::PageBuilder, Purpose::N1Worker, Purpose::Judge];

fn digest(hex: &str) -> [u8; 32] {
    let bytes = hex::decode(hex).unwrap();
    bytes.try_into().unwrap()
}

fn record(purpose: Purpose, identity: &str, floor: u32) -> QualificationRecord {
    QualificationRecord {
        purpose,
        identity_sha256: digest(identity),
        floor: freeze_floor(floor + 32).unwrap(),
        evidence: "cq0-test",
    }
}

/// C0-1 (P-Cq-2): the floor is the measured minimum minus 32, checked, and
/// frozen only when positive; usable in a `const`.
#[test]
fn cq0_freeze_floor_is_checked_and_positive() {
    assert_eq!(freeze_floor(31), None);
    assert_eq!(freeze_floor(32), None);
    for count in 0..=32 {
        assert_eq!(freeze_floor(count), None, "{count}");
    }
    assert_eq!(freeze_floor(33).map(Floor::get), Some(1));
    assert_eq!(freeze_floor(u32::MAX).map(Floor::get), Some(u32::MAX - 32));
    const FROZEN: Option<Floor> = freeze_floor(619);
    assert_eq!(FROZEN.map(Floor::get), Some(587));
}

/// C0-2 (P-Cq-0a): the purpose codes byte for byte; distinct; none a prefix
/// of another.
#[test]
fn cq0_purpose_codes_are_stable() {
    assert_eq!(Purpose::PageBuilder.code().as_bytes(), b"page_builder");
    assert_eq!(Purpose::N1Worker.code().as_bytes(), b"n1_worker");
    assert_eq!(Purpose::Judge.code().as_bytes(), b"judge");
    for a in PURPOSES {
        for b in PURPOSES {
            if a != b {
                assert!(!a.code().starts_with(b.code()), "{a:?} {b:?}");
            }
        }
    }
}

/// C0-3 (P-Cq-0b): a record matches only on both its purpose and the whole
/// identity digest; duplicates are refused.
#[test]
fn cq0_record_matches_whole_identity() {
    let table = [record(Purpose::PageBuilder, PAGE_BUILDER_V2, 300)];
    let floor = find(&table, Purpose::PageBuilder, &digest(PAGE_BUILDER_V2));
    assert_eq!(floor.map(Floor::get), Some(300));
    // The same purpose for another identity: not qualified.
    for other in [N1_WORKER_V2, JUDGE_V2, IDENTITY_V1] {
        assert_eq!(find(&table, Purpose::PageBuilder, &digest(other)), None);
    }
    // Another purpose for the same identity: not qualified.
    assert_eq!(find(&table, Purpose::Judge, &digest(PAGE_BUILDER_V2)), None);
    assert_eq!(
        find(&table, Purpose::N1Worker, &digest(PAGE_BUILDER_V2)),
        None
    );
    // Two records: each answers only for itself.
    let table = [
        record(Purpose::PageBuilder, PAGE_BUILDER_V2, 300),
        record(Purpose::Judge, JUDGE_V2, 100),
    ];
    let judge = find(&table, Purpose::Judge, &digest(JUDGE_V2));
    assert_eq!(judge.map(Floor::get), Some(100));
    assert!(no_duplicate(&table));
    let twice = [table[0], record(Purpose::PageBuilder, PAGE_BUILDER_V2, 400)];
    assert!(!no_duplicate(&twice));
}

/// C0-4: no model is qualified for any purpose, in every build (the items
/// carry no `cfg`).
#[test]
fn cq0_production_qualifications_are_empty() {
    assert!(QUALIFICATIONS.is_empty());
    for purpose in PURPOSES {
        for identity in [PAGE_BUILDER_V2, N1_WORKER_V2, JUDGE_V2, IDENTITY_V1] {
            assert_eq!(qualified_floor(purpose, &digest(identity)), None);
        }
    }
    let code: Vec<&str> = SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    for line in code {
        assert!(!line.contains("cfg"), "{line}");
        let line = line.trim_start();
        assert!(
            !line.starts_with("static ") && !line.contains("pub static"),
            "{line}"
        );
        assert!(!line.contains("std::env"), "{line}");
    }
}
