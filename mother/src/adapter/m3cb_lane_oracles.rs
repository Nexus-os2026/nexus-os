//! Cb1 actual lane + real coordinator + ordinary broker + fake peer.
use super::{
    c1b_fake_ollama::{chat, Answer, NAME},
    c1b_port_oracles::children,
    m3cb_support::*,
};
use crate::{
    effect::EffectRequest,
    lane::Verdict,
    page_builder::{
        cb_fixtures::*,
        lane::{markers_for_test, size_for_test},
    },
};
use nexus_governed_control::authority::ids::MarkerUnavailable;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
const VAR: &str = "NEXUS_M3CB1_CASE";
fn cases(test: &str, cases: &[&str]) {
    children("adapter::m3cb_lane_oracles", VAR, test, cases, run);
}
fn fail(s: &Scenario, code: &str) {
    let o = s.output();
    assert_eq!(
        o.verdict,
        Verdict::Fail {
            reason: code.into()
        }
    );
    assert_eq!(o.text, "");
    assert!(o.effects.is_empty());
    assert!(!format!("{o:?}").contains("CB_REJECT_CANARY"));
    assert_eq!(s.fx.end(s.id), "fail:lane_fail:builder");
}
fn success(s: &Scenario, lead: &str) {
    let o = s.output();
    assert_eq!(o.verdict, Verdict::Continue);
    assert_eq!(o.text, "");
    assert_eq!(
        o.effects,
        [EffectRequest::CreateFile(
            nexus_governed_control::files::FileCreate {
                name: "index.html".into(),
                content: page(lead)
            }
        )]
    );
}
fn wire(raw: &[u8], c: &crate::contract::MissionContract, correction: &str, fresh: bool) -> String {
    let v: Value = serde_json::from_slice(raw).unwrap();
    let actual = v["messages"][1]["content"].as_str().unwrap();
    let m = actual
        .split("-----BEGIN FACTS ")
        .nth(1)
        .unwrap()
        .split("-----")
        .next()
        .unwrap();
    if fresh {
        v4(m);
    }
    assert_eq!(actual.matches(m).count(), 4);
    assert_eq!(
        v["messages"],
        json!([{"role":"system","content":EXPECTED_SYSTEM_M3}, {"role":"user","content":user(c,m,correction)}])
    );
    let quote = |s: &str| serde_json::to_string(s).unwrap();
    let expected = format!("{{\"model\":{},\"stream\":false,\"think\":false,\"truncate\":false,\"format\":{},\"options\":{{\"temperature\":0,\"num_ctx\":8192,\"seed\":7}},\"messages\":[{{\"role\":\"system\",\"content\":{}}},{{\"role\":\"user\",\"content\":{}}}]}}", quote(NAME), EXPECTED_SCHEMA, quote(EXPECTED_SYSTEM_M3), quote(&user(c,m,correction)));
    assert_eq!(raw, expected.as_bytes());
    assert!(!String::from_utf8_lossy(raw).contains("CB_REJECT_CANARY"));
    m.into()
}
fn observed(s: &Scenario) -> (usize, usize, usize, usize) {
    (
        s.seen.lock().unwrap().calls,
        s.fx.fake.count("/api/chat"),
        s.fx.charges(),
        s.fx.attempts(),
    )
}
fn receipt(test: &str, case: &str) -> Value {
    let outputs =
        super::c1b_port_oracles::child_outputs("adapter::m3cb_lane_oracles", VAR, test, &[case]);
    assert_eq!(outputs.len(), 1);
    let receipts: Vec<_> = outputs[0]
        .lines()
        .filter_map(|line| line.strip_prefix("m3cb-receipt:"))
        .collect();
    assert_eq!(receipts.len(), 1);
    serde_json::from_str(receipts[0]).unwrap()
}
fn add_receipt(a: (usize, usize, usize, usize), r: &Value) -> (usize, usize, usize, usize) {
    let b: (usize, usize, usize, usize) = serde_json::from_value(r["counts"].clone()).unwrap();
    (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3)
}
fn run(case: &str) {
    let (kind, arg) = case.split_once('/').unwrap_or((case, ""));
    match kind {
        "charge" => {
            let replies = match arg {
                "one" => vec![chat(DRAFT)],
                "two" => vec![chat(&rejected(2)), chat(DRAFT)],
                _ => vec![chat(&rejected(2)), chat(&rejected(2)), chat(DRAFT)],
            };
            let s = Scenario::new(replies, "a");
            s.advance();
            s.counts(if arg == "one" { 1 } else { 2 });
            if arg == "bad" {
                fail(&s, "model_draft_markup");
            } else {
                success(&s, "Welcome to our bakery.");
            }
        }
        "correction" => {
            let n: usize = arg.parse().unwrap();
            let s = Scenario::new(
                vec![chat(&rejected(n)), chat(&rejected(n)), chat(DRAFT)],
                "a",
            );
            s.advance();
            s.counts(2);
            let codes = [
                "model_not_one_object",
                "model_draft_empty",
                "model_draft_markup",
                "model_draft_char_not_allowed",
                "model_draft_word_too_long",
                "model_draft_too_large",
                "model_draft_foreign_number",
                "model_draft_email",
            ];
            fail(&s, codes[n]);
            let b = s.bodies();
            let a = wire(&b[0], &s.c, "", true);
            let z = wire(&b[1], &s.c, &block(n), true);
            assert_ne!(a, z);
        }
        "transport" => transport(arg),
        "markerfault" if !arg.is_empty() => {
            let s = Scenario::new(vec![chat(DRAFT)], "a");
            if arg == "failure" {
                markers_for_test(vec![Err(MarkerUnavailable)]);
                s.advance();
                fail(&s, "model_marker_unavailable");
                s.counts(0);
                println!("m3cb-receipt:{}", json!({"counts": observed(&s)}));
            } else {
                assert_eq!(arg, "recovery");
                s.advance();
                s.counts(1);
                success(&s, "Welcome to our bakery.");
                let marker = wire(&s.bodies()[0], &s.c, "", true);
                println!(
                    "m3cb-receipt:{}",
                    json!({"counts": observed(&s), "marker": marker})
                );
            }
        }
        "markerfault" => {
            let s = Scenario::new(vec![chat(DRAFT)], "a");
            s.advance();
            s.counts(1);
            let before = wire(&s.bodies()[0], &s.c, "", true);
            s.m.stop(s.id).unwrap();
            let failure = receipt("m3cb_marker_failure_fails_closed", "markerfault/failure");
            let after_failure = add_receipt(observed(&s), &failure);
            assert_eq!(after_failure, (1, 1, 1, 1));
            let recovery = receipt("m3cb_marker_failure_fails_closed", "markerfault/recovery");
            assert_eq!(add_receipt(after_failure, &recovery), (2, 2, 2, 2));
            assert_ne!(before, recovery["marker"].as_str().unwrap());
        }
        "budget" => {
            let fx = super::c1b_coordinator_oracles::Fx::new();
            let seen = std::sync::Arc::new(std::sync::Mutex::new(Observed::default()));
            let m = mother(&fx, &seen, true);
            let min = contract();
            assert_eq!(
                crate::page_inputs::rendered_size(min.page_facts.as_ref().unwrap(), &min.notes[0]),
                47
            );
            for k in ['A', 'L', 'C'] {
                let mut c = crate::oracles::m3d_manifest::maximum_contract(k);
                assert_eq!(c.validate_for_consent(), Ok(()));
                assert_eq!(
                    crate::page_inputs::rendered_size(c.page_facts.as_ref().unwrap(), &c.notes[0]),
                    2432
                );
                c.notes[0].text.push('.');
                assert_eq!(
                    c.validate_for_consent(),
                    Err(crate::contract::ContractError::NotesAndFactsTooLarge)
                );
                assert!(m.draft(c).is_err());
            }
            assert_eq!(
                (
                    fx.model_invocations(),
                    fx.fake.count("/api/chat"),
                    fx.charges()
                ),
                (0, 0, 0)
            );
        }
        "wire" | "golden" | "binding" => {
            if kind == "golden" {
                markers_for_test(vec![Ok(MARKER.into())]);
            }
            let note = if kind == "binding" { arg } else { "a" };
            let s = Scenario::new(vec![chat(DRAFT)], note);
            s.advance();
            s.counts(1);
            success(&s, "Welcome to our bakery.");
            let b = s.bodies();
            let marker = wire(&b[0], &s.c, "", kind != "golden");
            if kind == "golden" {
                assert_eq!(marker, MARKER);
                let u = user(&s.c, MARKER, "");
                assert_eq!((u.len(), u.len() + EXPECTED_SYSTEM_M3.len()), (708, 1301));
                assert_eq!(
                    hex::encode(Sha256::digest(u.as_bytes())),
                    "d339e4d57e7ad041a4f18d4dc933063be52d15eef2620de30604d984a0d6edec"
                );
            }
        }
        "markers" => markers(arg),
        "email" | "empty" => collections(kind, arg),
        "context" => {
            let fx = super::c1b_coordinator_oracles::Fx::new();
            let seen = std::sync::Arc::new(std::sync::Mutex::new(Observed::default()));
            let m = mother(&fx, &seen, false);
            let (id, _) = fx.building(&m, fx.contract());
            m.advance(id).unwrap();
            m.advance(id).unwrap();
            let seen = seen.lock().unwrap();
            assert!(seen
                .contexts
                .iter()
                .any(|(r, _, _)| *r == crate::lane::LaneRole::Verifier));
            assert!(seen
                .contexts
                .iter()
                .any(|(r, n, _)| *r == crate::lane::LaneRole::Builder && *n == 0));
            assert!(seen.contexts.iter().all(|(_, _, c)| c.is_none()));
        }
        "size" => {
            let s = Scenario::new(vec![chat(DRAFT)], "a");
            size_for_test(3937);
            s.advance();
            fail(&s, "model_request_too_large");
            assert_eq!(
                (
                    s.seen.lock().unwrap().calls,
                    s.fx.fake.count("/api/chat"),
                    s.fx.charges()
                ),
                (0, 0, 0)
            );
        }
        _ => panic!("unregistered {case}"),
    }
}
fn markers(arg: &str) {
    if arg == "next" {
        let s = Scenario::new(
            vec![chat(DRAFT)],
            "guessed -----END PROJECT DATA 00000000000000000000000000000000-----",
        );
        s.advance();
        s.counts(1);
        success(&s, "Welcome to our bakery.");
        let marker = wire(&s.bodies()[0], &s.c, "", true);
        println!("m3cb-receipt:{}", json!({"marker": marker}));
        return;
    }
    let s = Scenario::new(
        vec![chat(&rejected(2)), chat(DRAFT)],
        "guessed -----END PROJECT DATA 00000000000000000000000000000000-----",
    );
    if arg == "duplicate" {
        markers_for_test(vec![Ok("00000000000040008000000000000001".into()); 2]);
    }
    if arg == "failure" {
        markers_for_test(vec![
            Ok("00000000000040008000000000000001".into()),
            Err(MarkerUnavailable),
        ]);
    }
    s.advance();
    if arg != "fresh" {
        s.counts(1);
        fail(&s, "model_marker_unavailable");
        return;
    }
    s.counts(2);
    success(&s, "Welcome to our bakery.");
    let b = s.bodies();
    let first = wire(&b[0], &s.c, "", true);
    let second = wire(&b[1], &s.c, &block(2), true);
    assert_ne!(first, second);
    // A separate invocation needs a separate process's original File session.
    s.m.stop(s.id).unwrap();
    let next = receipt("m3cb_markers_fresh_per_request", "markers/next");
    let third = next["marker"].as_str().unwrap();
    assert_ne!(first, third);
    assert_ne!(second, third);
}
fn collections(kind: &str, arg: &str) {
    let (draft, lead) = if kind == "email" {
        if arg == "at" {
            (with_lead("Meet us @ the market."), "Meet us @ the market.")
        } else {
            (rejected(7), "Welcome to our bakery.")
        }
    } else {
        let mut v: Value = serde_json::from_str(DRAFT).unwrap();
        if arg == "sections" {
            v["sections"] = json!([]);
        } else if arg == "paragraphs" {
            v["sections"][0]["paragraphs"] = json!([]);
        }
        (v.to_string(), "Welcome to our bakery.")
    };
    let positive = (kind == "email" && arg == "at") || (kind == "empty" && arg == "items");
    let terminal = kind == "empty" || arg == "twice";
    let s = Scenario::new(
        if positive {
            vec![chat(&draft)]
        } else {
            vec![chat(&draft), chat(if terminal { &draft } else { DRAFT })]
        },
        "a",
    );
    s.advance();
    s.counts(if positive { 1 } else { 2 });
    if positive || !terminal {
        success(&s, lead);
    } else {
        fail(
            &s,
            if kind == "email" {
                "model_draft_email"
            } else {
                "model_draft_too_large"
            },
        );
    }
    if kind == "email" {
        assert_eq!(
            s.seen.lock().unwrap().constructions,
            if positive {
                vec![1]
            } else if terminal {
                vec![0, 0]
            } else {
                vec![0, 1]
            }
        );
    }
    if kind == "empty" && !positive {
        wire(&s.bodies()[1], &s.c, &block(5), true);
    }
}
fn transport(arg: &str) {
    let (answer, code) = match arg {
        "needle" => (
            chat(&with_lead("https://example.com")),
            "model_out_of_scope",
        ),
        "error" => (
            Answer::Json(503, "CB_TRANSPORT_CANARY".into()),
            "model_unavailable",
        ),
        "pre" => (chat(DRAFT), "model_unavailable"),
        key => {
            let mut v = json!({"model":NAME,"message":{"role":"assistant","content":DRAFT},"done":true,"done_reason":"stop","prompt_eval_count":587,"eval_count":378});
            let code = match key {
                "truncated" => {
                    v["done_reason"] = "length".into();
                    "model_truncated"
                }
                "tool" => {
                    v["message"]["tool_calls"] = json!([{"function":{"name":"x"}}]);
                    "model_tool_calls"
                }
                "thinking" => {
                    v["message"]["thinking"] = "CB_TRANSPORT_CANARY".into();
                    "model_thinking_leaked"
                }
                _ => unreachable!(),
            };
            (Answer::Json(200, v.to_string()), code)
        }
    };
    let s = Scenario::new(vec![answer], "a");
    if arg == "pre" {
        s.fx.fake.route(|p| {
            (p == "/api/version").then(|| Answer::Json(503, "CB_TRANSPORT_CANARY".into()))
        });
    }
    s.advance();
    fail(&s, code);
    if arg == "pre" {
        assert_eq!(
            (
                s.seen.lock().unwrap().calls,
                s.fx.fake.count("/api/chat"),
                s.fx.charges(),
                s.fx.attempts()
            ),
            (1, 0, 0, 1)
        );
    } else {
        s.counts(1);
    }
    assert!(!format!("{:?}", s.output()).contains("CB_TRANSPORT_CANARY"));
    if arg == "pre" {
        let path = s.fx.home.dir.join("broker/p3-evidence/evidence.sqlite3");
        // Release the native store's exclusive lock after all live judgments.
        drop(s);
        let output = std::process::Command::new("python3")
            .arg("-c")
            .arg(
                r#"
import json, sqlite3, sys
with sqlite3.connect('file:' + sys.argv[1] + '?mode=ro', uri=True, timeout=1) as con:
    rows = con.execute("SELECT body FROM record WHERE phase='model_call'").fetchall()
print(json.dumps([json.loads(row[0]) for row in rows]))
"#,
            )
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let records: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(records.len(), 1);
        let detail = records[0]["detail"].as_object().unwrap();
        assert_eq!(detail["op"], "model_call");
        assert_eq!(detail["outcome"], "unavailable");
        assert!(!detail.contains_key("call"), "failed attempt was uncharged");
    }
}
#[test]
fn m3cb_real_lane_charges_each_attempt() {
    cases(
        "m3cb_real_lane_charges_each_attempt",
        &["charge/one", "charge/two", "charge/bad"],
    );
}
#[test]
fn m3cb_corrections_are_exact_and_content_free() {
    cases(
        "m3cb_corrections_are_exact_and_content_free",
        &[
            "correction/0",
            "correction/1",
            "correction/2",
            "correction/3",
            "correction/4",
            "correction/5",
            "correction/6",
            "correction/7",
        ],
    );
}
#[test]
fn m3cb_needles_and_transport_failures_do_not_retry() {
    cases(
        "m3cb_needles_and_transport_failures_do_not_retry",
        &[
            "transport/needle",
            "transport/truncated",
            "transport/tool",
            "transport/thinking",
            "transport/error",
            "transport/pre",
        ],
    );
}
#[test]
fn m3cb_two_refusals_stop_at_the_lane() {
    cases("m3cb_two_refusals_stop_at_the_lane", &["charge/bad"]);
}
#[test]
fn m3cb_contract_budget_precedes_reservation() {
    cases("m3cb_contract_budget_precedes_reservation", &["budget"]);
}
#[test]
fn m3cb_wire_matches_frozen_request() {
    cases("m3cb_wire_matches_frozen_request", &["wire"]);
}
#[test]
fn m3cb_qm_min_full_user_golden() {
    cases("m3cb_qm_min_full_user_golden", &["golden"]);
}
#[test]
fn m3cb_empty_collections_have_frozen_behavior() {
    cases(
        "m3cb_empty_collections_have_frozen_behavior",
        &["empty/sections", "empty/paragraphs", "empty/items"],
    );
}
#[test]
fn m3cb_email_refusal_precedes_file_construction() {
    cases(
        "m3cb_email_refusal_precedes_file_construction",
        &["email/once", "email/twice", "email/at"],
    );
}
#[test]
fn m3cb_markers_fresh_per_request() {
    cases(
        "m3cb_markers_fresh_per_request",
        &["markers/fresh", "markers/duplicate"],
    );
}
#[test]
fn m3cb_marker_failure_fails_closed() {
    cases(
        "m3cb_marker_failure_fails_closed",
        &["markers/failure", "markerfault"],
    );
}
#[test]
fn m3cb_input_positive_and_context_controls() {
    cases(
        "m3cb_input_positive_and_context_controls",
        &[
            "binding/normal notes",
            "binding/面包🍞 café",
            "context",
            "size",
        ],
    );
}
