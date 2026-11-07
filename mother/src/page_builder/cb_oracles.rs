//! Direct oracles registered against the inert actual lane.
use super::{cb_fixtures::*, lane::*, prompt};
use crate::contract::ContractError;
use crate::evidence::BrokerClass;
use crate::lane::{Lane, LaneContext, LaneOutput, LaneRole, ModelCallError, ModelCaller, Verdict};
use nexus_governed_control::{
    authority::ids::MarkerUnavailable,
    inference::{ModelReply, ModelRequest},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
struct Counting {
    calls: AtomicUsize,
    sizes: Mutex<Vec<usize>>,
    error: ModelCallError,
}
impl Counting {
    fn new(error: ModelCallError) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            sizes: Mutex::new(vec![]),
            error,
        }
    }
    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl ModelCaller for Counting {
    fn call(&self, r: &ModelRequest) -> Result<ModelReply, ModelCallError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.sizes.lock().unwrap().push(r.rendered_bytes());
        Err(self.error)
    }
}
pub(crate) fn failed(o: &LaneOutput, code: &str) {
    assert_eq!(
        o.verdict,
        Verdict::Fail {
            reason: code.into()
        }
    );
    assert_eq!(o.text, "");
    assert!(o.effects.is_empty());
    assert!(!format!("{o:?}").contains("CB_REJECT_CANARY"));
}
fn run(
    c: &crate::contract::MissionContract,
    i: &crate::lane::LaneInput,
    caller: &Counting,
) -> LaneOutput {
    PageBuilderLane.invoke_in(
        i,
        &LaneContext {
            model: Some(caller),
            contract: Some(c),
        },
    )
}
#[test]
fn m3cb_identity_and_context_are_fixed() {
    let lane = PageBuilderLane;
    assert_eq!(
        (
            lane.identity().name.as_str(),
            lane.identity().version.as_str(),
            lane.identity().role,
            lane.max_model_calls()
        ),
        ("page-builder", "m3-1", LaneRole::Builder, 2)
    );
    let c = contract();
    let i = input(&c);
    let caller = Counting::new(ModelCallError::RecordUnavailable);
    failed(&lane.invoke(&i), "model_caller_unavailable");
    failed(
        &lane.invoke_in(
            &i,
            &LaneContext {
                model: None,
                contract: Some(&c),
            },
        ),
        "model_caller_unavailable",
    );
    failed(
        &lane.invoke_in(
            &i,
            &LaneContext {
                model: Some(&caller),
                contract: None,
            },
        ),
        "page_input_identity",
    );
    let mut bad = i.clone();
    bad.builder_input = None;
    failed(&run(&c, &bad, &caller), "page_input_identity");
    bad.role = LaneRole::Verifier;
    failed(
        &lane.invoke_in(
            &bad,
            &LaneContext {
                model: None,
                contract: None,
            },
        ),
        "page_input_identity",
    );
    assert_eq!(caller.count(), 0);
}
#[test]
fn m3cb_validates_immediately_before_borrow() {
    let c = contract();
    let good = input(&c);
    for case in 0..5 {
        let mut i = good.clone();
        let mut changed = c.clone();
        match case {
            0 => i.contract_hash = crate::oracles::contract_with("changed").hash(),
            1 => changed.notes[0].text.push('b'),
            2 => i.invocation.seq += 1,
            3 | 4 => i
                .builder_input
                .as_mut()
                .unwrap()
                .alter_skipped_for_test(case == 4),
            _ => unreachable!(),
        }
        let caller = Counting::new(ModelCallError::RecordUnavailable);
        failed(&run(&changed, &i, &caller), "page_input_identity");
        assert_eq!(caller.count(), 0);
    }
    let caller = Counting::new(ModelCallError::RecordUnavailable);
    failed(&run(&c, &good, &caller), "model_record_unavailable");
    assert_eq!(caller.count(), 1);
}
#[test]
fn m3cb_contract_credential_screen_is_not_ca_validation() {
    let mut c = contract();
    c.notes[0].text = "sk-SYNTHETICONLYNOTASECRET123456789".into();
    let facts = c.page_facts.as_ref().unwrap().approved().unwrap();
    assert!(prompt::render_user(&facts, &c.notes[0], MARKER, None).is_ok());
    assert_eq!(
        c.validate_for_consent(),
        Err(ContractError::NoteLooksLikeCredential { index: 0 })
    );
    let caller = Counting::new(ModelCallError::RecordUnavailable);
    failed(&run(&c, &input(&c), &caller), "page_input_invalid");
    assert_eq!(caller.count(), 0);
    for text in ["normal notes", "面包🍞 café"] {
        c.notes[0].text = text.into();
        let caller = Counting::new(ModelCallError::RecordUnavailable);
        failed(&run(&c, &input(&c), &caller), "model_record_unavailable");
        assert_eq!(caller.count(), 1);
    }
}
#[test]
fn m3cb_facts_ready_precedes_answer() {
    for name in ["info@example.com".to_string(), "x".repeat(41)] {
        let mut c = contract();
        c.page_facts.as_mut().unwrap().name = name;
        let caller = Counting::new(ModelCallError::RecordUnavailable);
        failed(&run(&c, &input(&c), &caller), "facts_not_page_ready");
        assert_eq!(caller.count(), 0);
    }
    let c = contract();
    let caller = Counting::new(ModelCallError::RecordUnavailable);
    failed(&run(&c, &input(&c), &caller), "model_record_unavailable");
    assert_eq!(caller.count(), 1);
}
#[test]
fn m3cb_caller_errors_are_terminal_and_content_free() {
    let mut cases = vec![
        (ModelCallError::Invalidated, "model_invalidated"),
        (
            ModelCallError::RecordUnavailable,
            "model_record_unavailable",
        ),
    ];
    for &class in BrokerClass::ALL {
        cases.push((ModelCallError::Refused(class), class.code()));
        cases.push((ModelCallError::Failed(class), class.code()));
    }
    let c = contract();
    for (error, code) in cases {
        let caller = Counting::new(error);
        failed(&run(&c, &input(&c), &caller), code);
        assert_eq!(caller.count(), 1, "{error:?}");
    }
}
#[test]
fn m3cb_oversize_never_enters_caller() {
    let c = contract();
    for size in [3937, 3936] {
        size_for_test(size);
        let caller = Counting::new(ModelCallError::RecordUnavailable);
        failed(
            &run(&c, &input(&c), &caller),
            if size == 3937 {
                "model_request_too_large"
            } else {
                "model_record_unavailable"
            },
        );
        assert_eq!(caller.count(), usize::from(size == 3936));
        if size == 3936 {
            assert_eq!(*caller.sizes.lock().unwrap(), [3936]);
        }
    }
}
#[test]
fn m3cb_marker_failure_fails_closed() {
    let c = contract();
    let caller = Counting::new(ModelCallError::RecordUnavailable);
    failed(&run(&c, &input(&c), &caller), "model_record_unavailable");
    markers_for_test(vec![Err(MarkerUnavailable)]);
    failed(&run(&c, &input(&c), &caller), "model_marker_unavailable");
    assert_eq!(caller.count(), 1);
    failed(&run(&c, &input(&c), &caller), "model_record_unavailable");
    assert_eq!(caller.count(), 2);
    for marker in ["bad", "0000000000000000000000000000000G"] {
        markers_for_test(vec![Ok(marker.into())]);
        failed(&run(&c, &input(&c), &caller), "model_marker_unavailable");
        assert_eq!(caller.count(), 2);
    }
}
#[test]
fn m3cb_marker_source_is_broker_only() {
    fn pinned(s: &str) -> bool {
        use sha2::{Digest, Sha256};
        let norm = |s: &str| s.split_whitespace().collect::<String>();
        let Some((_, invoke)) = s.split_once("    fn invoke_in(") else {
            return false;
        };
        let Some((invoke, _)) = invoke.split_once("\n    }\n}") else {
            return false;
        };
        let invoke = norm(invoke);
        if invoke.matches("matchmarker()").count() != 1
            || hex::encode(Sha256::digest(invoke.as_bytes()))
                != "19bab345de1243d60e485ce075c30a77bda0352db54da36f6baf70b3f3f6df63"
        {
            return false;
        }
        let Some((_, body)) = s.split_once("fn marker() -> Result<String, MarkerUnavailable> {")
        else {
            return false;
        };
        let Some((body, _)) = body.split_once("\n}\n") else {
            return false;
        };
        norm(body)
            == norm(
                r#"
            #[cfg(test)]
            if let Some(result) = MARKERS.with(|m| m.borrow_mut().pop_front()) { return result; }
            nexus_governed_control::authority::ids::fresh_page_marker()
        "#,
            )
    }
    assert!(pinned(include_str!("lane.rs")));
    assert!(!pinned(&include_str!("lane.rs").replace("match marker()", r#"match { static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0); Ok::<_, MarkerUnavailable>(format!("00000000000040008000{:012x}", N.fetch_add(1, std::sync::atomic::Ordering::SeqCst))) }"#)));
    for body in [
        "Ok(\"00000000000040008000000000000001\".into())",
        "counter_v4()",
        "seeded_v4()",
        "timestamp_v4()",
        "cached_marker()",
    ] {
        assert!(!pinned(&include_str!("lane.rs").replace(
            "nexus_governed_control::authority::ids::fresh_page_marker()",
            body
        )));
    }
    // These deterministic substitutes pass shape and distinctness, but not source control.
    let a = "00000000000040008000000000000001";
    let b = "00000000000040008000000000000002";
    v4(a);
    v4(b);
    assert_ne!(a, b);
}
