//! P3-C11-HM-M4 N1: the entry-label algebra (C-5; three Architect mutants
//! survived every M3 suite: the code was right, the tests were missing) and
//! the admission row of an Owner's backend-shaped request (C-3).

use super::context::{DispatchClass, EntryLabel};
use super::testing::{ScriptedLlm, TestHm};
use super::{DispatchContext, OriginLabel, Principal, PromptBuilder, RouteClass, Unit};
use uuid::Uuid;

fn text_prompt<'a>(
    text: &'a str,
) -> impl FnMut(&mut DispatchContext) -> Option<super::PromptEnvelope> + 'a {
    move |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        b.text(text);
        Some(b.build())
    }
}

/// C-5 (a), mutant 15: an entry with no label carried into a unit is
/// `LOCAL_ONLY_ORIGIN` (§11.3): the taint becomes `LOCAL_ONLY_ORIGIN` and
/// the route class `local`.
#[test]
fn hm_m4_n1_c5_a_an_unlabelled_entry_makes_the_unit_local_only() {
    let hm = TestHm::new();
    let mut turn = hm
        .issuer
        .begin_unit(
            Principal::OwnerDirect,
            Unit::ChatTurn {
                turn_id: Uuid::new_v4(),
            },
        )
        .unwrap();
    assert_eq!(
        (turn.taint(), turn.route_class()),
        (OriginLabel::UnrestrictedOrigin, RouteClass::NonLocal),
        "precondition: an Owner's chat turn starts unrestricted and non-local"
    );
    turn.admit_entry(None).unwrap();
    assert_eq!(turn.taint(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(turn.route_class(), RouteClass::Local);
}

/// C-5 (b), mutant 16: the dream unit of an entry with no label is an
/// Owner's retained entry (class HR) that is local-only and
/// `LOCAL_ONLY_ORIGIN`, so it is never non-local.
#[test]
fn hm_m4_n1_c5_b_an_unlabelled_entry_unit_is_a_local_only_hr_unit() {
    let hm = TestHm::new();
    let ctx = hm
        .issuer
        .begin_unlabelled_entry_unit(Unit::Dream {
            entry_id: Uuid::new_v4(),
        })
        .unwrap();
    assert_eq!(ctx.principal(), Principal::OwnerDirect);
    assert_eq!(ctx.class, DispatchClass::OwnerRetained);
    assert_eq!(ctx.taint(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(ctx.route_class(), RouteClass::Local);
    assert!(ctx.restrictions().local_only());
}

/// C-5 (c), mutant 17: a product combining entries of one principal
/// carries the ⊔ of their labels (§11.3), whatever their order; entries of
/// two principals have no combined label.
#[test]
fn hm_m4_n1_c5_c_combined_entries_join_their_labels_within_one_principal() {
    let open = EntryLabel {
        principal: Principal::OwnerDirect,
        label: OriginLabel::UnrestrictedOrigin,
    };
    let local = EntryLabel {
        principal: Principal::OwnerDirect,
        label: OriginLabel::LocalOnlyOrigin,
    };
    assert_eq!(
        EntryLabel::combine(&[&open, &local]),
        Some(EntryLabel {
            principal: Principal::OwnerDirect,
            label: OriginLabel::LocalOnlyOrigin,
        })
    );
    assert_eq!(
        EntryLabel::combine(&[&local, &open]).map(|e| e.label()),
        Some(OriginLabel::LocalOnlyOrigin)
    );
    let agent = EntryLabel {
        principal: Principal::Agent(Uuid::new_v4()),
        label: OriginLabel::UnrestrictedOrigin,
    };
    assert_eq!(EntryLabel::combine(&[&open, &agent]), None);
    assert_eq!(EntryLabel::combine(&[]), None);
}

/// C-3 (mutant 8; §12.4 steps 1 and 5, §5.1 `hm_dispatch_admission`): an
/// Owner's request shaped by backend state with no registered agent (class
/// HB) starts at the issuer as an unrestricted, non-local `OwnerDirect`
/// request unit; each of its dispatches writes one admission row (agent
/// NULL, unit kind `request`, outcome updated as for HR) and its event. HD
/// (an Owner's chat turn or request) and SY/XS (a `System` request) write
/// none.
#[test]
fn hm_m4_n1_c3_an_hb_request_records_each_admission_and_hd_sy_xs_none() {
    let hm = TestHm::new();
    let admissions = || hm.db.hm().read(|tx| tx.admission_rows()).unwrap();
    let events = |kind: &str| {
        hm.db
            .hm()
            .read(|tx| tx.event_kinds())
            .unwrap()
            .into_iter()
            .filter(|(k, agent)| k == kind && agent.is_none())
            .count()
    };
    let mut hb = hm.issuer.begin_backend_shaped_request().unwrap();
    assert_eq!(hb.principal(), Principal::OwnerDirect);
    assert!(matches!(hb.unit(), Unit::Request { .. }));
    assert_eq!(hb.class, DispatchClass::OwnerBackendShaped);
    assert_eq!(
        (hb.taint(), hb.route_class()),
        (OriginLabel::UnrestrictedOrigin, RouteClass::NonLocal)
    );
    assert!(!hb.restrictions().retained_data_permitted());

    let answering = ScriptedLlm::replying(&hm.issuer, RouteClass::NonLocal, "an answer");
    answering.ask(&mut hb, &mut text_prompt("HB one")).unwrap();
    answering.ask(&mut hb, &mut text_prompt("HB two")).unwrap();
    let failing = ScriptedLlm::new(&hm.issuer, RouteClass::NonLocal).then_fail("down");
    assert!(failing.ask(&mut hb, &mut text_prompt("HB three")).is_err());
    let rows = admissions();
    assert_eq!(
        rows.iter()
            .map(|a| format!(
                "{}|{}|{}|{}|{}",
                a.agent_id.as_deref().unwrap_or("NULL"),
                a.unit_kind,
                a.route_class,
                a.record_count,
                a.outcome
            ))
            .collect::<Vec<_>>(),
        vec![
            "NULL|request|non_local|0|reply_used",
            "NULL|request|non_local|0|reply_used",
            "NULL|request|non_local|0|transport_failed",
        ]
    );
    assert_eq!(events("memory.dispatch_admitted"), 3);
    assert_eq!(events("memory.transport_failed"), 1);

    // HD, SY and XS: no admission row, no event.
    let owner_turn = hm
        .issuer
        .begin_unit(
            Principal::OwnerDirect,
            Unit::ChatTurn {
                turn_id: Uuid::new_v4(),
            },
        )
        .unwrap();
    let owner_request = hm
        .issuer
        .begin_unit(
            Principal::OwnerDirect,
            Unit::Request {
                request_id: Uuid::new_v4(),
            },
        )
        .unwrap();
    for mut ctx in [owner_turn, owner_request, hm.system()] {
        answering
            .ask(&mut ctx, &mut text_prompt("HD SY XS"))
            .expect("precondition: the request is dispatched");
    }
    assert_eq!(answering.sent().len(), 5);
    assert_eq!(admissions().len(), 3);
    assert_eq!(events("memory.dispatch_admitted"), 3);
}

/// A-S5 (P3-HM-CONTRACT-02 §18.2, AD-6), the kernel's test-only capture
/// seam at its transport boundary: every AR and AC request carries the
/// admission id it was admitted by; HD and SY/XS requests carry none and no
/// planted retained-record marker. A refused dispatch is never captured.
#[test]
fn hm_m4_n4_a_s5_ar_ac_requests_carry_admission_ids_and_hd_sy_no_record() {
    use super::testing::take_captured;
    use super::{NewMemory, RecallSpec, RecordTable, SourceKind};
    const MARKER: &str = "P3-HM-M4-A-S5-RECORD-c0de";
    let hm = TestHm::new();
    let agent = hm.open_agent("a-s5-agent");
    let mut goal = hm.goal(agent);
    hm.issuer
        .write_memory(
            &goal,
            &NewMemory {
                source_kind: SourceKind::LoopEpisodic,
                memory_type: "episodic",
                key: "a-s5",
                value_json: &format!("\"{MARKER}\""),
                relevance_score: 1.0,
            },
            &[],
        )
        .unwrap();
    take_captured();
    let llm = ScriptedLlm::replying(&hm.issuer, RouteClass::NonLocal, "ok");
    // AR: a goal's prompt carrying the retained record verbatim.
    let issuer = hm.issuer.clone();
    llm.ask(&mut goal, &mut |c: &mut DispatchContext| {
        let records = issuer
            .recall_spec(c, &RecallSpec::table(RecordTable::AgentMemory, 10))
            .ok()?;
        let mut b = PromptBuilder::new(c);
        b.text("AR request\n");
        for r in &records {
            b.include(r);
        }
        Some(b.build())
    })
    .unwrap();
    // AC, HD and SY/XS.
    let turn = |principal| {
        hm.issuer
            .begin_unit(
                principal,
                Unit::ChatTurn {
                    turn_id: Uuid::new_v4(),
                },
            )
            .unwrap()
    };
    llm.ask(
        &mut turn(Principal::Agent(agent)),
        &mut text_prompt("AC turn"),
    )
    .unwrap();
    llm.ask(
        &mut turn(Principal::OwnerDirect),
        &mut text_prompt("HD turn"),
    )
    .unwrap();
    llm.ask(&mut hm.system(), &mut text_prompt("SY request"))
        .unwrap();
    // A refused dispatch (a local-only agent on a non-local transport).
    let local = hm.agent("a-s5-local");
    assert!(llm
        .ask(&mut hm.goal(local), &mut text_prompt("refused"))
        .is_err());

    let captured = take_captured();
    assert_eq!(
        captured.iter().map(|c| c.class).collect::<Vec<_>>(),
        ["AR", "AC", "HD", "SY/XS"],
        "A-S5: every transmitted request captured once, refused ones never"
    );
    assert!(
        captured[0].prompt.contains(MARKER),
        "precondition: the record reached the AR request"
    );
    let admitted: Vec<String> = hm
        .db
        .hm()
        .read(|tx| tx.admission_rows())
        .unwrap()
        .into_iter()
        .map(|a| a.admission_id)
        .collect();
    for c in &captured[..2] {
        let id = c
            .admission_id
            .as_deref()
            .unwrap_or_else(|| panic!("A-S5: an {} request without an admission id", c.class));
        assert!(
            admitted.iter().any(|a| a == id),
            "A-S5: {id} is no committed admission"
        );
    }
    for c in &captured[2..] {
        assert_eq!(c.admission_id, None, "{}", c.class);
        assert!(
            !c.prompt.contains(MARKER),
            "A-S5: a retained record in a {} request",
            c.class
        );
    }
    assert!(
        super::transmission_in_progress().is_none(),
        "cleared after send"
    );
}
