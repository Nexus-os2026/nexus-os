//! P0-A oracles (design §5.4: P0-O1 to O9, O19 and O22; O23 and the parts
//! that name the designed API are in `p0_api.rs`). Pre-registered: written
//! and committed before the implementation they judge, against main's API,
//! and not edited after a result is seen.
//!
//! Each oracle plants the canaries `C` and `Cl` and scans everywhere a
//! canary must not be (`P0World::scan`): every file of Mother's home but
//! `private/`, the broker evidence, and the `Debug` of every returned Mother
//! error, `Advance` and `EffectRefusal`. The private store is excluded from
//! the scan and asserted to hold the content.

use super::p0_support::*;
use crate::contract::Question;
use crate::coordinator::CoordinatorError;
use crate::effect::{ApprovalStatus, EffectOutcome, EffectRequest, PendingAction};
use crate::journal::{Event, MissionId};
use crate::oracles::{contract_with, get, refusal, PortCall};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::connector::ConnectorIntent;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::tool::ToolIntent;
use std::sync::atomic::Ordering;

fn clean(w: &P0World, cs: &Canaries, what: &str) {
    let hits = w.scan(cs);
    assert!(
        hits.is_empty(),
        "{what}: a canary outside the private store: {hits:#?}"
    );
}

fn no_effects(_: MissionId) -> Vec<EffectRequest> {
    vec![]
}

/// What the Builder asks for in the waiting oracles: a first request that
/// waits for the Owner, then the rest of the phase, which carries the
/// canaries (URL path and query, a header, a tool's input, a connector's
/// body, a request body), and one request naming the mission, so that no
/// two missions' rests are the same content.
fn asked(cs: &Canaries, mission: MissionId) -> Vec<EffectRequest> {
    vec![
        get("http://127.0.0.1:9/first"),
        EffectRequest::Network(EgressIntent {
            method: "GET".into(),
            url: format!("http://127.0.0.1:9/{}/x?q={}", cs.c, cs.cl),
            headers: vec![("accept".into(), cs.cl.clone())],
            body: None,
        }),
        EffectRequest::Process(ToolIntent {
            tool: "git".into(),
            input: serde_json::json!({ "args": ["log", cs.c.clone()] }),
        }),
        EffectRequest::ModelTool(ConnectorIntent {
            operation: "llm.complete".into(),
            input: serde_json::json!({ "prompt": format!("{} {}", cs.c, cs.cl) }),
        }),
        EffectRequest::Network(EgressIntent {
            method: "POST".into(),
            url: "http://127.0.0.1:9/post".into(),
            headers: vec![],
            body: Some(format!("body {} {}", cs.c, base64_std(cs.cl.as_bytes()))),
        }),
        get(&format!("http://127.0.0.1:9/mission-{}", mission.0)),
    ]
}

fn asking_world(label: &str, cs: &Canaries) -> P0World {
    let (c, cl) = (cs.c.clone(), cs.cl.clone());
    let cs2 = Canaries { c, cl };
    P0World::new(label, p0_lanes("lane reason", move |m| asked(&cs2, m)))
}

// ---------------------------------------------------------------- P0-O1

/// P0-O1: the contract's fields (words, restatement, folder, question id,
/// text and answer, criterion id and text), through draft, submit,
/// consent, revise, done, close and reopen, are absent from every record;
/// the reopened contract is equal (a done mission's and an unfinished
/// one's).
#[test]
fn p0_o1_the_contract_lives_only_in_the_private_store() {
    let cs = canaries();
    let mut w = P0World::new("o1", p0_lanes("lane reason", no_effects));
    let done = w.draft(canary_contract(&cs, "first"));
    assert_eq!(w.submit(done), Ok(MissionState::AwaitingConsent));
    assert_eq!(w.consent_yes(done), Ok(MissionState::Approved));
    let revised = canary_contract(&cs, "revised");
    let revise = w.mother.revise(done, revised.clone());
    w.note(&revise);
    assert_eq!(revise, Ok(MissionState::Draft));
    assert_eq!(w.submit(done), Ok(MissionState::AwaitingConsent));
    assert_eq!(w.consent_yes(done), Ok(MissionState::Approved));
    for _ in 0..5 {
        w.advance(done).unwrap();
    }
    assert_eq!(w.mother.state(done), Some(MissionState::Done));
    let unfinished_contract = canary_contract(&cs, "unfinished");
    let unfinished = w.draft(unfinished_contract.clone());

    w.reopen_clean().unwrap();
    assert_eq!(w.mother.state(done), Some(MissionState::Done));
    assert_eq!(
        w.mother.contract(done),
        Some(revised),
        "O1: the done mission"
    );
    assert_eq!(
        w.mother.contract(unfinished),
        Some(unfinished_contract),
        "O1: the unfinished mission"
    );
    clean(&w, &cs, "O1");
    assert!(
        w.private_contains(&cs.c) && w.private_contains(&cs.cl),
        "O1: the private store holds the contract"
    );
}

// ---------------------------------------------------------------- P0-O2

/// P0-O2: a paused phase's rest (URL, header, tool input, connector body)
/// is absent from every record; after a clean reopen the rest that runs is
/// identical; after an unclean reopen the action is withdrawn (Mother died
/// with the wait journaled: held, and the Owner's Stop withdraws it; or
/// before it: recovery withdraws it) and no rest runs.
#[test]
fn p0_o2_a_paused_phase_s_rest_lives_only_in_the_private_store() {
    let cs = canaries();

    // Clean.
    let mut w = asking_world("o2-clean", &cs);
    w.port.set_pending(Some(action(1)));
    let id = w.approved(contract_with("P0-ASK-BUILDER"));
    w.to_waiting(id);
    clean(&w, &cs, "O2 waiting");
    assert!(
        w.private_contains(&cs.c) && w.private_contains(&cs.cl),
        "O2: the private store holds the rest"
    );
    w.reopen_clean().unwrap();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    w.port.approval_reply(ApprovalStatus::Approved);
    assert_eq!(
        w.advance(id).map(|a| a.to),
        Ok(MissionState::Verifying),
        "O2: the approved phase goes on"
    );
    assert_eq!(
        w.port.requests_of(id),
        asked(&cs, id),
        "O2: the rest that ran after a clean reopen is identical"
    );
    clean(&w, &cs, "O2 after a clean reopen");

    // Unclean, after the wait was journaled: held; Stop withdraws.
    let mut w = asking_world("o2-unclean", &cs);
    w.port.set_pending(Some(action(2)));
    let id = w.approved(contract_with("P0-ASK-BUILDER"));
    w.to_waiting(id);
    w.crash_reopen().unwrap();
    assert!(
        w.mother.held(id).is_some(),
        "O2: held after an unclean reopen"
    );
    let mark = w.port.calls().len();
    assert_eq!(w.stop(id), Ok(MissionState::Stopped));
    assert!(
        w.port.calls()[mark..].contains(&PortCall::Cancel(id)),
        "O2: the Stop withdrew the action"
    );
    assert_eq!(w.port.requests_of(id).len(), 1, "O2: no rest ran");
    clean(&w, &cs, "O2 unclean, held");

    // Unclean, before the wait was journaled: recovery withdraws.
    let mut w = asking_world("o2-crash", &cs);
    w.port.set_pending(Some(action(3)));
    let id = w.approved(contract_with("P0-ASK-BUILDER"));
    w.to_waiting(id);
    let last = w.drop_last_line();
    assert!(last.contains("await_approval"), "{last}");
    let mark = w.port.calls().len();
    w.crash_reopen().unwrap();
    assert!(
        w.port.calls()[mark..].contains(&PortCall::Cancel(id)),
        "O2: recovery withdrew the action"
    );
    assert!(w.mother.state(id).unwrap().is_terminal());
    assert_eq!(w.port.requests_of(id).len(), 1, "O2: no rest ran");
    clean(&w, &cs, "O2 unclean, before the wait");
}

// ---------------------------------------------------------------- P0-O3

/// P0-O3: a lane's Halt, Fail and Verifier reasons are absent from every
/// record; the mission's reason is a code (`lane_halt:<role>`,
/// `lane_fail:<role>`, `verifier_fail`), also after a reopen.
#[test]
fn p0_o3_lane_reasons_are_codes() {
    let cs = canaries();
    let reason = format!("because {} and {}", cs.c, cs.cl);
    let mut w = P0World::new("o3", p0_lanes(&reason, no_effects));

    let halted = w.approved(contract_with("P0-HALT-SENTINEL"));
    w.advance(halted).unwrap();
    let a = w.advance(halted).unwrap();
    assert_eq!(a.to, MissionState::Blocked);
    assert_eq!(a.reason.as_deref(), Some("lane_halt:sentinel"));

    let failed = w.approved(contract_with("P0-FAIL-RESEARCHERA"));
    w.advance(failed).unwrap();
    let a = w.advance(failed).unwrap();
    assert_eq!(a.to, MissionState::Failed);
    assert_eq!(a.reason.as_deref(), Some("lane_fail:researcher_a"));

    let verified = w.approved(contract_with("P0-FAIL-VERIFIER"));
    for _ in 0..3 {
        w.advance(verified).unwrap();
    }
    let a = w.advance(verified).unwrap();
    assert_eq!(a.to, MissionState::Failed);
    assert_eq!(a.reason.as_deref(), Some("verifier_fail"));

    w.reopen_clean().unwrap();
    for (id, code) in [
        (halted, "lane_halt:sentinel"),
        (failed, "lane_fail:researcher_a"),
        (verified, "verifier_fail"),
    ] {
        assert_eq!(w.mother.reason(id).as_deref(), Some(code), "O3 reopened");
    }
    clean(&w, &cs, "O3");
}

// ---------------------------------------------------------------- P0-O4

/// P0-O4: a port class outside the broker's registry (`C`, or the
/// code-shaped `Cl`) is `unknown` wherever Mother keeps or returns it: a
/// refusal, a lapsed approval, a failed withdrawal.
#[test]
fn p0_o4_a_port_class_outside_the_registry_is_unknown() {
    let cs = canaries();
    let w = P0World::new("o4", p0_lanes("r", |_| vec![get("http://127.0.0.1:9/a")]));
    for (_, class) in cs.both() {
        let id = w.approved(contract_with("P0-ASK-BUILDER"));
        w.advance(id).unwrap();
        w.advance(id).unwrap();
        let refused = refusal(class);
        w.note(&refused);
        w.port.reply(Err(refused));
        let a = w.advance(id).unwrap();
        assert_eq!(a.to, MissionState::Blocked);
        assert_eq!(a.reason.as_deref(), Some("broker_refused:unknown"));
        assert_eq!(a.effects[0].outcome, "unknown");
        let settled: Vec<String> = w
            .journal_values()
            .iter()
            .filter(|v| {
                v["mission"].as_u64() == Some(id.0) && v["event"]["event"] == "effect_settled"
            })
            .map(|v| v["event"]["outcome"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(settled, vec!["refused:unknown".to_string()]);

        let lapsed = w.approved(contract_with("P0-ASK-BUILDER"));
        w.to_waiting(lapsed);
        w.port.approval_reply(ApprovalStatus::Lapsed {
            class: class.to_string(),
        });
        let a = w.advance(lapsed).unwrap();
        assert_eq!(a.to, MissionState::Blocked);
        assert_eq!(a.reason.as_deref(), Some("approval_lapsed:unknown"));

        let withdrawn = w.approved(contract_with("P0-ASK-BUILDER"));
        w.port.fail_withdrawal_with(withdrawn, class);
        assert!(w.stop(withdrawn).is_err(), "O4: the withdrawal failed");
    }
    clean(&w, &cs, "O4");
}

// ---------------------------------------------------------------- P0-O5

/// Assert that the reopen withdrew `id` (a port cancellation made after
/// `mark`) before journaling its Stop for `private_content_unavailable`.
fn withdrawn_then_stopped(w: &P0World, id: MissionId, mark: usize, what: &str) {
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped), "{what}");
    let stops = w.stops(id);
    let (index, reason) = stops.last().expect("a journaled Stop").clone();
    assert_eq!(reason, "private_content_unavailable", "{what}");
    let before = w.calls_before_append(index);
    assert!(
        before > mark && w.port.calls()[mark..before].contains(&PortCall::Cancel(id)),
        "{what}: withdrawn before the Stop was journaled"
    );
}

/// P0-O5: a private blob deleted, for a waiting mission (its contract; its
/// rest) and for a working one: on reopen the journaled request is
/// withdrawn first, then the mission stops (`private_content_unavailable`);
/// no lane, consent or effect.
#[test]
fn p0_o5_missing_content_withdraws_first_then_stops() {
    let cs = canaries();
    let mut w = asking_world("o5", &cs);
    let waiting = |n: u8| {
        w.port.set_pending(Some(action(n)));
        let id = w.approved(contract_with(&format!("P0-ASK-BUILDER m{n}")));
        w.to_waiting(id);
        id
    };
    let no_contract = waiting(1);
    let no_rest = waiting(2);
    let untouched = waiting(3);
    let working = w.approved(contract_with("working"));
    assert_eq!(
        w.advance(working).map(|a| a.to),
        Ok(MissionState::Investigating)
    );
    assert!(
        w.delete_blob(no_contract, "contract"),
        "O5: a contract blob"
    );
    assert!(w.delete_blob(no_rest, "paused_phase"), "O5: a rest blob");
    assert!(w.delete_blob(working, "contract"), "O5: a contract blob");

    let (lanes, consents, performed) = (
        w.lane_calls.load(Ordering::SeqCst),
        w.consent_records(),
        w.port.effects().len(),
    );
    let mark = w.port.calls().len();
    w.reopen_clean().unwrap();
    for id in [no_contract, no_rest, working] {
        withdrawn_then_stopped(&w, id, mark, &format!("O5 {id}"));
        assert!(w.advance(id).is_err());
    }
    assert_eq!(
        w.mother.state(untouched),
        Some(MissionState::AwaitingApproval)
    );
    assert_eq!(w.lane_calls.load(Ordering::SeqCst), lanes, "O5: no lane");
    assert_eq!(w.consent_records(), consents, "O5: no consent");
    assert_eq!(w.port.effects().len(), performed, "O5: no effect");
    clean(&w, &cs, "O5");
}

// ---------------------------------------------------------------- P0-O6

/// P0-O6: a swapped blob is treated as a missing one: other bytes under a
/// valid content id (the same contract, not in its canonical bytes; a rest
/// of the same size), or a valid blob whose `contract.hash()` differs (the
/// journal rechained to name another mission's contract).
#[test]
fn p0_o6_swapped_content_withdraws_first_then_stops() {
    let cs = canaries();
    let mut w = asking_world("o6", &cs);
    let waiting = |n: u8| {
        w.port.set_pending(Some(action(n)));
        let id = w.approved(contract_with(&format!("P0-ASK-BUILDER m{n}")));
        w.to_waiting(id);
        id
    };
    let noncanonical = waiting(1);
    let other_rest = waiting(2);
    let other_contract = waiting(3);
    let named = waiting(4);

    assert!(
        w.replace_blob(noncanonical, "contract", |bytes| {
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            serde_json::to_vec_pretty(&value).unwrap()
        }),
        "O6: a contract blob"
    );
    assert!(
        w.replace_blob(other_rest, "paused_phase", |bytes| {
            String::from_utf8(bytes)
                .unwrap()
                .replacen("127.0.0.1:9/", "127.0.0.1:8/", 1)
                .into_bytes()
        }),
        "O6: a rest blob"
    );
    let (content, bytes) = w.content_of(named, "contract").expect("O6: a content id");
    assert!(
        w.rechain(|mission, event| {
            if mission != other_contract || event["event"] != "drafted" {
                return false;
            }
            event["contract"]["content"] = content.clone().into();
            event["contract"]["bytes"] = bytes.into();
            true
        }),
        "O6: the journal names the other mission's contract"
    );

    let mark = w.port.calls().len();
    w.reopen_clean().unwrap();
    for id in [noncanonical, other_rest, other_contract] {
        withdrawn_then_stopped(&w, id, mark, &format!("O6 {id}"));
    }
    assert_eq!(w.mother.state(named), Some(MissionState::AwaitingApproval));
    clean(&w, &cs, "O6");
}

// ---------------------------------------------------------------- P0-O7

/// P0-O7: no authority from content. A blob without a journaled Consent
/// gives no Approved; an orphan blob is ignored; a held mission with
/// missing content can only be stopped.
#[test]
fn p0_o7_content_carries_no_authority() {
    let cs = canaries();
    let mut w = P0World::new("o7", p0_lanes("r", no_effects));
    let asked = w.draft(canary_contract(&cs, "asked"));
    assert_eq!(w.submit(asked), Ok(MissionState::AwaitingConsent));
    let held = w.approved(contract_with("held"));
    assert_eq!(
        w.advance(held).map(|a| a.to),
        Ok(MissionState::Investigating)
    );
    let orphan = canary_contract(&cs, "orphan");
    w.put_orphan("contract", &serde_json::to_vec(&orphan).unwrap());
    w.put_orphan("paused_phase", b"[]");

    // Unclean: the unfinished missions come back held; a clean close then
    // journals the holds.
    w.crash_reopen().unwrap();
    assert!(w.mother.held(held).is_some());
    w.reopen_clean().unwrap();
    assert!(w.mother.held(held).is_some());
    assert!(w.delete_blob(held, "contract"), "O7: a contract blob");
    let calls = w.lane_calls.load(Ordering::SeqCst);
    let mark = w.port.calls().len();
    w.reopen_clean().unwrap();

    assert_eq!(
        w.mother.missions(),
        vec![asked, held],
        "O7: the orphan is ignored"
    );
    assert_eq!(
        w.mother.state(asked),
        Some(MissionState::AwaitingConsent),
        "O7: no Approved without a journaled consent"
    );
    assert_eq!(w.mother.consented_hash(asked), None);
    assert!(w.advance(asked).is_err());
    withdrawn_then_stopped(&w, held, mark, "O7 held");
    assert_eq!(w.mother.held(held), None);
    let resumed = w.mother.resume(held);
    w.note(&resumed);
    assert!(
        resumed.is_err(),
        "O7: a held mission without content is not resumed"
    );
    assert!(w.advance(held).is_err());
    assert_eq!(w.lane_calls.load(Ordering::SeqCst), calls, "O7: no lane");
    assert_eq!(w.port.effects(), vec![], "O7: no effect");
    clean(&w, &cs, "O7");
}

// ---------------------------------------------------------------- P0-O8

/// P0-O8 (the behaviour; `p0_api.rs` names the error): a v1 journal (main's
/// format) is refused, and its file is left byte for byte.
#[test]
fn p0_o8_a_v1_journal_is_refused_and_left_as_it_is() {
    let w = P0World::new("o8", p0_lanes("r", no_effects));
    std::fs::write(w.journal_path(), V1_JOURNAL).unwrap();
    let opened = w.open();
    assert!(opened.is_err(), "O8: a v1 journal opened");
    assert_eq!(
        std::fs::read_to_string(w.journal_path()).unwrap(),
        V1_JOURNAL,
        "O8: the file changed"
    );
    assert!(!w.dir.join("mother-journal.jsonl.poisoned").exists());
}

// ---------------------------------------------------------------- P0-O9

/// P0-O9 (an id field; `p0_api.rs` has every field): an action identity a
/// port reports with `C`/`Cl` in its run and commitment is refused at
/// append (the mission ends and the action is withdrawn); the same entry
/// chained by hand is refused at replay.
#[test]
fn p0_o9_an_id_with_a_canary_is_refused_at_append_and_at_replay() {
    let cs = canaries();
    let w = asking_world("o9", &cs);
    w.port.set_pending(Some(PendingAction {
        run: cs.c.clone(),
        commitment: cs.cl.clone(),
        deadline_mono_ms: 600_000,
    }));
    let id = w.approved(contract_with("P0-ASK-BUILDER"));
    w.advance(id).unwrap();
    w.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    let mark = w.port.calls().len();
    let _ = w.advance(id);
    assert!(
        w.mother.state(id).unwrap().is_terminal(),
        "O9: the mission ended"
    );
    assert!(w.port.calls()[mark..].contains(&PortCall::Cancel(id)));
    clean(&w, &cs, "O9 append");

    // By hand: a valid waiting mission's action renamed in the journal.
    w.port.set_pending(Some(action(9)));
    let waiting = w.approved(contract_with("P0-ASK-BUILDER m9"));
    w.to_waiting(waiting);
    w.mother.close_ref().unwrap();
    assert!(w.rechain(|mission, event| {
        if mission != waiting || event["event"] != "approval_pending" {
            return false;
        }
        event["pending"]["run"] = cs.c.clone().into();
        true
    }));
    let reopened = w.open();
    assert!(
        matches!(reopened, Err(CoordinatorError::Journal(_))),
        "O9: a hand-chained entry with a canary id was replayed"
    );
}

// ---------------------------------------------------------------- P0-O19

/// P0-O19 (the behaviour; `p0_api.rs` has the index and `WorkingDetail`):
/// a question id `C`/`Cl` is in no returned error; nor in a replay
/// violation, a failed withdrawal or a store failure.
#[test]
fn p0_o19_returned_errors_carry_codes_only() {
    let cs = canaries();
    let mut w = P0World::new("o19", p0_lanes("r", no_effects));
    for (_, canary) in cs.both() {
        let mut c = contract_with("o19");
        c.questions.push(Question {
            id: canary.to_string(),
            text: "Which one?".into(),
            answer: None,
        });
        let id = w.draft(c.clone());
        assert!(w.submit(id).is_err(), "O19: an unanswered question");
        assert_eq!(w.mother.state(id), Some(MissionState::Draft));
    }

    // A journal that submitted the unanswered contract (chained by hand).
    let mut c = contract_with("o19 replay");
    c.questions.push(Question {
        id: cs.c.clone(),
        text: format!("Which {}?", cs.cl),
        answer: None,
    });
    let hash = c.hash();
    let replayed = w.draft(c);
    w.mother.close_ref().unwrap();
    w.append_chained(
        replayed,
        Event::Transition {
            trigger: Trigger::Submit { hash },
        },
    );
    let _ = w.crash_reopen();
    clean(&w, &cs, "O19 submit and replay");

    // A failed withdrawal.
    let mut w = P0World::new("o19-withdraw", p0_lanes("r", no_effects));
    let id = w.approved(contract_with("o19 withdraw"));
    w.port.fail_withdrawal_with(id, &cs.c);
    assert!(w.stop(id).is_err());

    // The journal file's own failures, at a path that names a canary.
    let dir = w.dir.join(&cs.c).join(&cs.cl);
    std::fs::create_dir_all(&dir).unwrap();
    let opened = super::FileJournal::open(&dir);
    w.note(&opened.as_ref().err());
    assert!(opened.is_err());
    let _ = w.reopen_clean();
    clean(&w, &cs, "O19");
}

// ---------------------------------------------------------------- P0-O22

/// P0-O22: the scanner's own negative control. A `C` written only to a
/// store's `-wal` file (as before a checkpoint), and `C` in two halves in
/// two fields, are both found; so is every encoded form; the private store
/// is all the scan leaves out.
#[test]
fn p0_o22_the_scanner_finds_what_it_must() {
    let cs = canaries();
    let dir = fresh_dir("o22");
    std::fs::write(
        dir.join("evidence.sqlite3"),
        b"SQLite format 3\0 nothing here",
    )
    .unwrap();
    let mut wal = b"WAL frame header ".to_vec();
    wal.extend_from_slice(cs.c.as_bytes());
    wal.extend_from_slice(b" more frames");
    std::fs::write(dir.join("evidence.sqlite3-wal"), wal).unwrap();
    std::fs::write(dir.join("evidence.sqlite3-shm"), [0u8; 64]).unwrap();
    let hits = scan_dir(&dir, None, &cs);
    assert!(
        hits.iter()
            .any(|h| h.starts_with("evidence.sqlite3-wal: C/")),
        "O22: a canary only in the WAL: {hits:?}"
    );
    assert!(!hits.iter().any(|h| h.starts_with("evidence.sqlite3:")));

    let half = cs.c.len() / 2;
    let line = serde_json::json!({ "a": &cs.c[..half], "b": &cs.c[half..] }).to_string();
    assert!(
        !hits_in(line.as_bytes(), "split", &cs).is_empty(),
        "O22: two halves in two fields"
    );

    for (name, canary) in cs.both() {
        let forms = [
            canary.to_uppercase(),
            canary.to_lowercase(),
            hex::encode(canary.as_bytes()),
            hex::encode_upper(canary.as_bytes()),
            format!("x{}", base64_std(format!("ab{canary}").as_bytes())),
            base64_url(format!("a{canary}").as_bytes()),
            base64_std(canary.as_bytes()),
            percent_all(canary, true),
            json_u(canary, false),
            serde_json::to_string(&format!("\"{canary}\"")).unwrap(),
        ];
        for form in forms {
            assert!(
                !hits_in(form.as_bytes(), "form", &cs).is_empty(),
                "O22: {name} as {form}"
            );
        }
        for fragment in fragments(canary) {
            assert!(fragment.chars().count() >= 8);
            assert!(!hits_in(fragment.as_bytes(), "fragment", &cs).is_empty());
        }
    }

    let private = dir.join("private");
    std::fs::create_dir_all(private.join("contract")).unwrap();
    std::fs::write(private.join("contract").join("x"), cs.cl.as_bytes()).unwrap();
    assert!(scan_dir(&dir, Some(&private), &cs)
        .iter()
        .all(|h| !h.starts_with("private")));
    assert!(scan_dir(&dir, None, &cs)
        .iter()
        .any(|h| h.starts_with("private")));
    let _ = std::fs::remove_dir_all(&dir);
}
