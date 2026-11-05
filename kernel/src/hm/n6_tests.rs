//! P3-C11-HM-M3 N6: production acceptance tests of the dispatch decision
//! (P3-HM-CONTRACT-02 §18.2 A-C3, A-P4) through `DispatchIssuer::dispatch`,
//! the single dispatch every production adapter makes.

use super::testing::{ScriptedLlm, TestConfig, TestHm};
use super::{
    DispatchContext, DispatchError, DispatchIssuer, NewMemory, Principal, ProbePoint,
    PromptBuilder, RecallSpec, RecordTable, Refused, ReplyRefused, RouteClass, SourceKind, Unit,
};
use nexus_persistence::NexusDatabase;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

fn text_prompt<'a>(
    text: &'a str,
) -> impl FnMut(&mut DispatchContext) -> Option<super::PromptEnvelope> + 'a {
    move |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        b.text(text);
        Some(b.build())
    }
}

/// A-C3 (mutant 14). A context that may not carry retained data (a
/// `System` or `OwnerDirect` unit, or an agent-routed chat turn) never
/// includes a retained record: a prompt including one is refused even on an
/// attested local transport, and nothing is sent.
#[test]
fn hm_m3_n6_a_c3_no_retained_record_in_a_context_that_may_not_carry_one() {
    let hm = TestHm::new();
    let agent = hm.open_agent("c3-agent");
    // A record of the agent, recalled verbatim in its own goal unit.
    let mut unit = hm.goal(agent);
    hm.issuer
        .write_memory(
            &unit,
            &NewMemory {
                source_kind: SourceKind::LoopEpisodic,
                memory_type: "episodic",
                key: "c3",
                value_json: "\"SYNTH-c3\"",
                relevance_score: 1.0,
            },
            &[],
        )
        .unwrap();
    let records = hm
        .issuer
        .recall_spec(&mut unit, &RecallSpec::table(RecordTable::AgentMemory, 10))
        .unwrap();
    assert_eq!(records.len(), 1, "precondition: one record recalled");

    let owner = hm
        .issuer
        .begin_unit(
            Principal::OwnerDirect,
            Unit::ChatTurn {
                turn_id: uuid::Uuid::new_v4(),
            },
        )
        .unwrap();
    let chat = hm
        .issuer
        .begin_unit(
            Principal::Agent(agent),
            Unit::ChatTurn {
                turn_id: uuid::Uuid::new_v4(),
            },
        )
        .unwrap();
    for (name, mut ctx) in [("system", hm.system()), ("owner", owner), ("chat", chat)] {
        // Control: the same context dispatches a constant prompt locally.
        let control = ScriptedLlm::replying(&hm.issuer, RouteClass::Local, "reply");
        control
            .ask(&mut ctx, &mut text_prompt("SYNTH-constant"))
            .unwrap_or_else(|e| panic!("{name}: control refused: {e:?}"));
        let llm = ScriptedLlm::replying(&hm.issuer, RouteClass::Local, "reply");
        let result = llm.ask(&mut ctx, &mut |c: &mut DispatchContext| {
            let mut b = PromptBuilder::new(c);
            b.text("constant text");
            b.include(&records[0]);
            Some(b.build())
        });
        assert!(
            matches!(
                result,
                Err(DispatchError::Refused(Refused::RetainedDataNotPermitted))
            ),
            "{name}: a retained record was admitted: {result:?}"
        );
        assert!(llm.sent().is_empty(), "{name}: {:?}", llm.sent());
    }
}

/// A-P4 (production R8). Two connections to one state database: a
/// tightening that another connection commits before the admission
/// transaction refuses the non-local dispatch (nothing is sent); an
/// admission holding the write lock makes a tightening on the other
/// connection wait, so the admitted request is sent. A tightening committed
/// before the reply discards that reply and governs the next dispatch.
#[test]
fn hm_m3_n6_a_p4_two_connections_ordering_by_the_admission_transaction() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("state.db");
    let hm = TestHm::over(Arc::new(NexusDatabase::open(&path).unwrap()));
    let first = hm.open_agent("p4-first");
    let second = hm.open_agent("p4-second");
    // The other connection, with an issuer whose configuration tightens
    // both agents.
    let cfg2 = Arc::new(TestConfig::default());
    cfg2.set(first, true);
    cfg2.set(second, true);
    let issuer2 = Arc::new(
        DispatchIssuer::new(
            Arc::new(NexusDatabase::open(&path).unwrap()),
            hm.registry.clone(),
            cfg2,
        )
        .unwrap(),
    );

    // (1) The tightening commits on the other connection after the unit
    // started and before its admission: refused, nothing sent.
    let mut ctx = hm.goal(first);
    assert_eq!(ctx.route_class(), RouteClass::NonLocal);
    let p = issuer2.resolve_agent_route_policy(first).unwrap();
    assert!(p.local_only());
    let llm = ScriptedLlm::replying(&hm.issuer, RouteClass::NonLocal, "reply");
    let refused = llm.ask(&mut ctx, &mut text_prompt("SYNTH-p4-one"));
    assert!(
        matches!(refused, Err(DispatchError::Refused(_))),
        "(1) admitted after the other connection's tightening: {refused:?}"
    );
    assert!(llm.sent().is_empty(), "(1): {:?}", llm.sent());
    assert!(hm
        .db
        .hm()
        .read(|tx| tx.admission_rows())
        .unwrap()
        .is_empty());

    // (2) The admission holds the write lock; the other connection's
    // tightening waits for it.
    let db3 = NexusDatabase::open(&path).unwrap();
    super::testing::exec(&db3, "PRAGMA busy_timeout = 0;");
    let mut ctx = hm.goal(second);
    // Control: without a concurrent tightening, admission and reply
    // acceptance both succeed through the same dispatch entry.
    let control = ScriptedLlm::replying(&hm.issuer, RouteClass::NonLocal, "reply");
    let accepted = control.ask(&mut ctx, &mut text_prompt("SYNTH-p4-control"));
    assert!(
        accepted.is_ok(),
        "(2) control did not succeed: {accepted:?}"
    );
    assert_eq!(control.sent(), ["SYNTH-p4-control"]);
    let generation = ctx.policy().policy_generation();
    let lock_held: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    type Tightening = std::thread::JoinHandle<()>;
    let worker: Arc<Mutex<Option<Tightening>>> = Arc::new(Mutex::new(None));
    let (committed, completion) = mpsc::channel();
    let fired = AtomicBool::new(false);
    let (held, slot, i2) = (lock_held.clone(), worker.clone(), issuer2.clone());
    hm.issuer.install_probe(Box::new(move |point| {
        if point != ProbePoint::AdmissionPolicyRead || fired.swap(true, Ordering::SeqCst) {
            return;
        }
        // Inside the admission transaction: a writer that does not wait
        // fails at once, so the write lock is held now.
        *held.lock().unwrap() = Some(db3.hm().immediate(|_| Ok(())).is_err());
        let (started, ready) = mpsc::channel();
        let issuer = i2.clone();
        let committed = committed.clone();
        *slot.lock().unwrap() = Some(std::thread::spawn(move || {
            started.send(()).unwrap();
            let policy = issuer
                .resolve_agent_route_policy(second)
                .expect("(2) the waiting tightening must commit");
            committed.send(policy).unwrap();
        }));
        ready.recv_timeout(Duration::from_secs(30)).unwrap();
    }));
    let db = hm.db.clone();
    let completion = Mutex::new(completion);
    let llm = ScriptedLlm::answering(&hm.issuer, RouteClass::NonLocal, move |_| {
        // Reaching the transport proves admission succeeded. Verify its
        // committed row at the original generation, before O3 can run.
        let rows = db.hm().read(|tx| tx.admission_rows()).unwrap();
        assert_eq!(rows.len(), 2, "(2) control and concurrent admission");
        let admission = &rows[1];
        assert_eq!(admission.agent_id, Some(second.to_string()));
        assert_eq!(admission.route_class, "non_local");
        assert_eq!(admission.policy_generation, generation);
        assert_eq!(admission.outcome, "admitted");

        // A worker-start signal does not order its commit relative to O3.
        // Hold the reply until the tightening commits: the request stands,
        // but its reply must now be discarded (§12.6).
        let policy = completion
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(30))
            .expect("(2) tightening did not finish after admission committed");
        assert!(policy.local_only());
        assert_eq!(policy.policy_generation(), generation + 1);
        "reply".to_string()
    });
    let result = llm.ask(&mut ctx, &mut text_prompt("SYNTH-p4-two"));
    hm.issuer.install_probe(Box::new(|_| {}));
    worker.lock().unwrap().take().unwrap().join().unwrap();
    assert_eq!(*lock_held.lock().unwrap(), Some(true), "(2) lock not held");
    assert_eq!(
        result.unwrap_err(),
        DispatchError::Reply(ReplyRefused::Discarded),
        "(2) the tightening committed before reply acceptance"
    );
    assert_eq!(llm.sent(), ["SYNTH-p4-two"], "(2) exactly one request sent");
    let rows = hm.db.hm().read(|tx| tx.admission_rows()).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].outcome, "reply_used");
    assert_eq!(rows[1].outcome, "reply_discarded");
    assert_eq!(ctx.working_set().derived_labels().len(), 1, "control only");
    // The tightening governs the next dispatch of that unit.
    let next = llm.ask(&mut ctx, &mut text_prompt("SYNTH-p4-three"));
    assert!(
        matches!(next, Err(DispatchError::Refused(_))),
        "(2) a dispatch after the tightening was admitted: {next:?}"
    );
    assert_eq!(llm.sent().len(), 1, "(2): {:?}", llm.sent());
}
