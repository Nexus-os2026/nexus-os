//! M2d-B2 unit oracles for the approval bridge (design v2 §6.4, D-O16's
//! unit half, MD-9), pre-registered before it exists: `BrokerApproved`
//! answers the kernel's confirmation yes only for an apply of exactly the
//! committed review (all 32 bytes), and it is built at one site only.

use super::BrokerApproved;
use nexus_kernel::coding_run::{ConfirmationKind, ConfirmationRequest, OwnerConfirmer, RunId};

const REVIEW: [u8; 32] = [0x5a; 32];

fn request(kind: ConfirmationKind, review_hash: [u8; 32]) -> ConfirmationRequest {
    ConfirmationRequest {
        kind,
        project_name: "ws".into(),
        run_id: RunId::parse("2f1c3e4d-5a6b-4c7d-8e9f-0a1b2c3d4e5f").unwrap(),
        creates: 1,
        replaces: 0,
        deletes: 0,
        review_short: "5a5a5a5a5a5a".into(),
        candidate_short: "000000000000".into(),
        base_short: "000000000000".into(),
        paths: vec!["create index.html".into()],
        more_paths: 0,
        review_hash,
        bytes: Some(12),
        sha256_full: Some("0".repeat(64)),
        identity: None,
    }
}

/// D-O16 (unit), MD-9: yes for an apply of the committed review hash; no
/// for any other review hash (each byte differing on its own) and no for a
/// restore of the same review.
#[test]
fn m2d_b2_d_o16_the_bridge_confirms_only_its_own_apply() {
    let approved = BrokerApproved {
        review_hash: REVIEW,
    };
    assert!(approved.confirm(&request(ConfirmationKind::Apply, REVIEW)));
    for at in 0..32 {
        let mut other = REVIEW;
        other[at] ^= 1;
        assert!(
            !approved.confirm(&request(ConfirmationKind::Apply, other)),
            "byte {at} differs"
        );
    }
    assert!(!approved.confirm(&request(ConfirmationKind::Restore, REVIEW)));
}

/// §6.4: one construction site in the Files module's own source (a guard,
/// not a proof), and the type is not `Clone`.
#[test]
fn m2d_b2_the_bridge_has_one_construction_site() {
    let source = include_str!("mod.rs");
    assert_eq!(source.matches("BrokerApproved { review_hash:").count(), 1);
    let at = source.find("BrokerApproved { review_hash:").unwrap();
    let execute = source.find("fn execute(").expect("FilesEffect::execute");
    assert!(at > execute, "built inside execute");
    assert!(
        !source[execute..at].contains("\n    fn "),
        "inside execute itself, not a later method"
    );
    let definition = source
        .find("struct BrokerApproved")
        .expect("the bridge type");
    let before = &source[..definition];
    let attributes = &before[before.rfind("\n\n").unwrap_or(0)..];
    assert!(!attributes.contains("Clone"), "BrokerApproved is not Clone");
}
