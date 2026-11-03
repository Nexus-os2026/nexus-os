//! M2d-C after merging main (#79, S1a-D+B): `BrokerClass` mirrors every
//! `AuthorityError::class()` (I1), so the four classes S1a-D added are in
//! the registry; before this, a Files create refused for its file type
//! ended the mission `broker_refused:unknown`. Pre-registered before them.

use crate::evidence::{BlockReason, BrokerClass};
use nexus_governed_control::authority::AuthorityError;

#[test]
fn m2d_c_broker_class_mirrors_the_s1a_d_classes() {
    for error in [
        AuthorityError::FactsInvalid("x"),
        AuthorityError::FileTypeNotAllowed,
        AuthorityError::PageNotInert("x"),
        AuthorityError::PageFactMissing("x"),
    ] {
        let class = BrokerClass::of(error.class());
        assert_ne!(class, BrokerClass::Unknown, "{}", error.class());
        assert_eq!(class.to_string(), error.class());
        let reason = format!("broker_refused:{}", error.class());
        assert_eq!(
            BlockReason::parse(&reason),
            Some(BlockReason::BrokerRefused(class))
        );
    }
}
