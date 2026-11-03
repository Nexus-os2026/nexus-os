//! M2d-C (Codex review 1 of PR #78, HIGH 2; the Architect's decision): a
//! commitment's view says when its effect succeeded with recovery
//! required (a Files publication whose directory sync failed reports
//! `reason=recovery_required` beside the success). Exactly when the
//! Succeeded commitment's Finished record keeps that detail (its first 16
//! entries); never otherwise. Pre-registered before the field exists. A
//! child of the pipeline tests, for their fixture.

use super::*;

/// An effect that succeeds with `meta`, or fails.
struct Reporting {
    meta: Vec<(String, String)>,
    fails: bool,
}

impl PendingEffect for Reporting {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(target_digest(false))
    }

    fn parameters(&self) -> Digest {
        Digest::of("fake.params", &[b"x"])
    }

    fn execute(
        self: Box<Self>,
        _: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        if self.fails {
            return Err((FailureClass::Actuator, "recovery_required".into()));
        }
        Ok(EffectOutput {
            meta: self.meta,
            ..Default::default()
        })
    }
}

fn pair(k: &str, v: &str) -> (String, String) {
    (k.to_string(), v.to_string())
}

fn ran(
    f: &Fixture,
    meta: Vec<(String, String)>,
    fails: bool,
) -> crate::authority::commitment::CommitmentView {
    let id = f.propose_effect(Box::new(Reporting { meta, fails }));
    f.control
        .authorize(id, &f.agent, f.run, &Yes(false))
        .unwrap();
    let _ = f.control.execute(id, &f.agent, f.run);
    f.control.authority().commitments().view(id).unwrap()
}

#[test]
fn m2d_c_r1_the_view_says_a_success_requires_recovery() {
    let f = fixture();
    let recovery = pair("reason", "recovery_required");
    let view = ran(&f, vec![pair("op", "create"), recovery.clone()], false);
    assert_eq!(view.state, CommitmentState::Succeeded);
    assert!(view.recovery_required, "{view:?}");
    assert!(view.interrupted.is_empty());

    // A plain success, another reason, the detail past the record's 16
    // entries, and a failure that names it: no.
    let plain = ran(&f, vec![pair("op", "create")], false);
    assert!(!plain.recovery_required);
    let other = ran(&f, vec![pair("reason", "something_else")], false);
    assert!(!other.recovery_required);
    let mut late: Vec<(String, String)> = (0..16).map(|i| pair("k", &i.to_string())).collect();
    late.push(recovery);
    let late = ran(&f, late, false);
    assert_eq!(late.state, CommitmentState::Succeeded);
    assert!(!late.recovery_required);
    let failed = ran(&f, vec![], true);
    assert_eq!(failed.state, CommitmentState::Failed);
    assert!(!failed.recovery_required);
}
