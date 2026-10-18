//! P3-C11-HM-M3: kernel gates of the historical-memory authority core
//! (P3-HM-CONTRACT-02 §6.3, §11.2, §12.4–§12.6).
//!
//! N1: the label of an inclusion is the label evaluated at recall joined
//! with the label evaluated now; an inclusion enters the including
//! context's working set whatever the admission outcome; reply acceptance
//! raises the taint, belongs to its own context and checks the generation;
//! the single dispatch entry (`DispatchIssuer::dispatch`) by class (§12.1),
//! and contexts of every principal.
//!
//! Synthetic databases only: files in a test-owned temporary directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use nexus_kernel::hm::{
    evaluate_label, run_t0_option_a, AgentRegistry, ConfigInput, ConfigReadError, ConfigReading,
    DerivedItem, DispatchContext, DispatchError, DispatchIssuer, EvaluatedLabel, NewMemory,
    OriginLabel, Principal, PromptBuilder, PromptEnvelope, RecallQuery, RecalledRecord,
    RecordTable, Refused, ReplyRefused, RouteAttestation, RouteClass, SourceKind,
    TransportEvidence, Unit, Unresolved, WriteRefused,
};
use nexus_persistence::hm::{PolicyChangeRow, PolicyRow};
use nexus_persistence::NexusDatabase;
use uuid::Uuid;

// ── shared helpers ──────────────────────────────────────────────────────────

const AGENT_A: &str = "0a000000-0000-4000-8000-00000000000a";
const AGENT_B: &str = "0b000000-0000-4000-8000-00000000000b";
const AGENT_D: &str = "0d000000-0000-4000-8000-00000000000d";

fn id(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap()
}

fn agent(s: &str) -> Principal {
    Principal::Agent(id(s))
}

fn goal() -> Unit {
    Unit::Goal {
        goal_id: Uuid::new_v4(),
    }
}

/// Registered agents and their names (the supervisor stand-in).
#[derive(Default)]
struct TestRegistry {
    agents: Mutex<HashMap<Uuid, Vec<String>>>,
}

impl TestRegistry {
    fn add(&self, agent_id: Uuid) {
        self.agents
            .lock()
            .unwrap()
            .insert(agent_id, vec![format!("synthetic-{agent_id}")]);
    }
}

impl AgentRegistry for TestRegistry {
    fn is_registered(&self, agent_id: &Uuid) -> bool {
        self.agents.lock().unwrap().contains_key(agent_id)
    }

    fn names(&self, agent_id: &Uuid) -> Vec<String> {
        self.agents
            .lock()
            .unwrap()
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
    }
}

/// The untrusted config file stand-in.
struct TestConfig {
    local: Mutex<HashMap<Uuid, bool>>,
    fail: AtomicBool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            local: Mutex::new(HashMap::new()),
            fail: AtomicBool::new(false),
        }
    }
}

impl TestConfig {
    fn set(&self, agent_id: Uuid, local_only: bool) {
        self.local.lock().unwrap().insert(agent_id, local_only);
    }
}

impl ConfigInput for TestConfig {
    fn read_local_only(
        &self,
        agent_id: &Uuid,
        _names: &[String],
    ) -> Result<ConfigReading, ConfigReadError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(ConfigReadError);
        }
        Ok(ConfigReading {
            local_only: self
                .local
                .lock()
                .unwrap()
                .get(agent_id)
                .copied()
                .unwrap_or(false),
            content_digest: "cfg-digest-0".into(),
        })
    }
}

/// A migrated synthetic database file, its registry, config and issuer.
struct Env {
    _dir: tempfile::TempDir,
    #[allow(dead_code)]
    path: PathBuf,
    db: Arc<NexusDatabase>,
    cfg: Arc<TestConfig>,
    issuer: DispatchIssuer,
}

fn open(path: &Path) -> NexusDatabase {
    NexusDatabase::open(path).expect("open synthetic database")
}

/// Agents A, B and D at T₀ (all local-only under Option A), all registered.
fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let db = Arc::new(open(&path));
    let agents = [id(AGENT_A), id(AGENT_B), id(AGENT_D)];
    run_t0_option_a(&db, &agents).unwrap();
    let reg = Arc::new(TestRegistry::default());
    for a in agents {
        reg.add(a);
    }
    let cfg = Arc::new(TestConfig::default());
    let issuer = DispatchIssuer::new(db.clone(), reg, cfg.clone()).unwrap();
    Env {
        _dir: dir,
        path,
        db,
        cfg,
        issuer,
    }
}

fn exec(db: &NexusDatabase, sql: &str) -> Result<(), String> {
    db.with_transaction(|tx| {
        tx.execute_batch(sql)?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

fn must(db: &NexusDatabase, sql: &str) {
    exec(db, sql).unwrap_or_else(|e| panic!("{e} for: {sql}"));
}

fn count(db: &NexusDatabase, sql: &str) -> i64 {
    db.with_transaction(|tx| Ok(tx.query_row(sql, [], |r| r.get::<_, i64>(0))?))
        .unwrap_or_else(|e| panic!("{e} for: {sql}"))
}

fn strings(db: &NexusDatabase, sql: &str) -> Vec<String> {
    db.with_transaction(|tx| {
        let mut stmt = tx.prepare(sql)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .unwrap_or_else(|e| panic!("{e} for: {sql}"))
}

/// TEST FIXTURE, not product code (as in `hm_m2_authority.rs`): an Owner
/// loosening committed directly through the store, with its history row.
fn fixture_owner_loosen(db: &NexusDatabase, agent_id: &str) -> i64 {
    db.hm()
        .immediate(|tx| {
            let raw = tx.policy_row_raw(agent_id)?.expect("policy row");
            let generation = raw.policy_generation.unwrap() + 1;
            tx.insert_policy_change(&PolicyChangeRow {
                agent_id: agent_id.into(),
                policy_generation: generation,
                local_only: 0,
                basis: "owner_confirmed".into(),
                cause: "owner_loosen".into(),
            })?;
            tx.update_policy(&PolicyRow {
                agent_id: agent_id.into(),
                local_only: 0,
                policy_generation: generation,
                revocation_seq: raw.revocation_seq.unwrap(),
                basis: "owner_confirmed".into(),
            })?;
            Ok(generation)
        })
        .unwrap()
}

fn mem_r(key: &str, relevance_score: f64) -> NewMemory<'_> {
    NewMemory {
        source_kind: SourceKind::LoopEpisodic,
        memory_type: "episodic",
        key,
        value_json: "\"SYNTH\"",
        relevance_score,
    }
}

fn memories(limit: usize) -> RecallQuery {
    RecallQuery {
        table: RecordTable::AgentMemory,
        limit,
    }
}

fn state(c: &DispatchContext) -> (OriginLabel, RouteClass) {
    (c.taint(), c.route_class())
}

const LO: (OriginLabel, RouteClass) = (OriginLabel::LocalOnlyOrigin, RouteClass::Local);
const UN: (OriginLabel, RouteClass) = (OriginLabel::UnrestrictedOrigin, RouteClass::NonLocal);

/// A unit of `who` whose route class is local through its taint.
fn local_unit(e: &Env, who: &str) -> DispatchContext {
    let mut ctx = e.issuer.begin_unit(agent(who), goal()).unwrap();
    let restricted = e.issuer.begin_unit(agent(AGENT_D), goal()).unwrap();
    ctx.join_child(&restricted).unwrap();
    assert_eq!(ctx.route_class(), RouteClass::Local);
    ctx
}

fn attest(route: RouteClass) -> RouteAttestation {
    RouteAttestation::from_transport_evidence(match route {
        RouteClass::Local => TransportEvidence::VerifiedLoopbackOllama,
        RouteClass::NonLocal => TransportEvidence::Unattested,
    })
}

fn admission_count(db: &NexusDatabase) -> i64 {
    count(db, "SELECT COUNT(*) FROM hm_dispatch_admission")
}

fn non_local_admissions(db: &NexusDatabase) -> i64 {
    count(
        db,
        "SELECT COUNT(*) FROM hm_dispatch_admission WHERE route_class = 'non_local'",
    )
}

fn outcome_of(db: &NexusDatabase, admission_id: &str) -> String {
    strings(
        db,
        &format!("SELECT outcome FROM hm_dispatch_admission WHERE admission_id = '{admission_id}'"),
    )
    .join(";")
}

fn key_of(db: &NexusDatabase, r: &RecalledRecord) -> String {
    strings(
        db,
        &format!(
            "SELECT key FROM agent_memory WHERE id = {}",
            r.record_ref().record_id()
        ),
    )
    .remove(0)
}

/// Builds a prompt with fixed text and `records` included verbatim, for a
/// transport attested `route`.
fn including<'r>(
    records: &'r [&'r RecalledRecord],
    route: RouteClass,
) -> impl FnMut(&mut DispatchContext) -> Option<(PromptEnvelope, RouteAttestation)> + 'r {
    move |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        b.text("SYNTH-instruction\n");
        for r in records {
            b.include(r);
        }
        Some((b.build(), attest(route)))
    }
}

// ── N1: P1–P3 (Architect findings on the M2 base) ──────────────────────────

/// P1: a record recalled by another context and included in this context's
/// prompt enters this context's working set, whatever the admission outcome:
/// a later write of this unit carries that record's label.
#[test]
fn hm_m3_n1_p1_an_inclusion_recalled_elsewhere_labels_the_including_units_writes() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);

    // A (local-only at T₀) writes a memory; a local unit of A recalls it.
    let a_unit = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let w = e
        .issuer
        .write_memory(&a_unit, &mem_r("SYNTH-a-local-only", 0.9), &[])
        .unwrap();
    assert_eq!(w.label(), OriginLabel::LocalOnlyOrigin);
    let mut a_local = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&a_local), LO);
    let recs = e.issuer.recall(&mut a_local, memories(10)).unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(
        recs[0].label(),
        EvaluatedLabel::Bound(OriginLabel::LocalOnlyOrigin)
    );

    // A non-local unit of B includes that record in a prompt attested
    // local: admitted.
    let mut b = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(state(&b), UN);
    let included = [&recs[0]];
    let admission = e
        .issuer
        .admit(&mut b, &mut including(&included, RouteClass::Local))
        .unwrap();
    assert_eq!(admission.route_class(), RouteClass::Local);
    // The admission is dropped without accepting a reply.
    drop(admission);

    // B's unit writes a memory with no parents: LOCAL_ONLY, because the
    // included record is now a verbatim item of B's working set.
    let after = e
        .issuer
        .write_memory(&b, &mem_r("SYNTH-b-after-inclusion", 0.1), &[])
        .unwrap();
    assert_eq!(
        after.label(),
        OriginLabel::LocalOnlyOrigin,
        "the write of a unit whose prompt included a LOCAL_ONLY record"
    );
    assert!(
        b.working_set()
            .verbatim()
            .any(|(r, l)| r == recs[0].record_ref()
                && l == EvaluatedLabel::Bound(OriginLabel::LocalOnlyOrigin)),
        "the inclusion entered the including context's working set"
    );
}

/// P2/P3 setup: B writes memory m (UNRESTRICTED); an unlabelled writer
/// alters it (UNBOUND); a local unit of B recalls it with the altered
/// bytes; the bytes are then restored, so m binds again.
fn recalled_while_unbound(e: &Env) -> (RecalledRecord, DispatchContext) {
    fixture_owner_loosen(&e.db, AGENT_B);
    let un = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let m = e
        .issuer
        .write_memory(&un, &mem_r("SYNTH-m", 0.9), &[])
        .unwrap();
    assert_eq!(m.label(), OriginLabel::UnrestrictedOrigin);
    let m_id = m.record_ref().record_id().to_string();

    must(
        &e.db,
        &format!("UPDATE agent_memory SET value_json = '\"SYNTH-altered\"' WHERE id = {m_id}"),
    );
    assert_eq!(
        evaluate_label(&e.db, RecordTable::AgentMemory, &m_id).unwrap(),
        EvaluatedLabel::Unbound
    );

    let mut local = local_unit(e, AGENT_B);
    let mut recs = e.issuer.recall(&mut local, memories(10)).unwrap();
    assert_eq!(recs.len(), 1);
    let rec = recs.remove(0);
    assert_eq!(key_of(&e.db, &rec), "SYNTH-m");
    assert_eq!(rec.label(), EvaluatedLabel::Unbound);
    // The recalled content holds the altered bytes: shown through a local
    // admission of the local unit.
    let shown = [&rec];
    let local_admission = e
        .issuer
        .admit(&mut local, &mut including(&shown, RouteClass::Local))
        .unwrap();
    assert!(local_admission.prompt().contains("SYNTH-altered"));
    drop(local_admission);

    must(
        &e.db,
        &format!("UPDATE agent_memory SET value_json = '\"SYNTH\"' WHERE id = {m_id}"),
    );
    assert_eq!(
        evaluate_label(&e.db, RecordTable::AgentMemory, &m_id).unwrap(),
        EvaluatedLabel::Bound(OriginLabel::UnrestrictedOrigin),
        "m binds again"
    );
    (rec, local)
}

/// P2: a record that was UNBOUND when it was recalled stays ineligible for
/// a non-local route, even after it binds again: its recall-time label
/// joins the label evaluated in the admission transaction.
#[test]
fn hm_m3_n1_p2_an_inclusion_unbound_at_recall_is_never_admitted_non_locally() {
    let e = env();
    let (rec, _local) = recalled_while_unbound(&e);
    let rows_before = admission_count(&e.db);

    let mut fresh = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(state(&fresh), UN);
    let included = [&rec];
    let result = e
        .issuer
        .admit(&mut fresh, &mut including(&included, RouteClass::NonLocal));
    match result {
        Ok(a) => panic!(
            "admitted non-locally (route {:?}); the prompt carries the altered bytes: {}",
            a.route_class(),
            a.prompt().contains("SYNTH-altered")
        ),
        Err(r) => assert_eq!(r, Refused::Ineligible),
    }
    assert_eq!(admission_count(&e.db), rows_before, "no admission row");
    assert_eq!(non_local_admissions(&e.db), 0);
}

/// P3: the same inclusion on a local route is admitted, and its reply is
/// labelled with the admission's raw label (recall-time ⊔ now): LOCAL_ONLY,
/// as is the unit taint after the reply is accepted.
#[test]
fn hm_m3_n1_p3_a_reply_to_an_inclusion_unbound_at_recall_is_local_only() {
    let e = env();
    let (rec, _local) = recalled_while_unbound(&e);

    let mut fresh = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(state(&fresh), UN);
    let included = [&rec];
    let admission = e
        .issuer
        .admit(&mut fresh, &mut including(&included, RouteClass::Local))
        .unwrap();
    assert_eq!(admission.route_class(), RouteClass::Local);
    let reply = e
        .issuer
        .accept_reply(&mut fresh, admission, Ok("SYNTH-reply".into()))
        .unwrap();
    assert_eq!(
        (reply.label(), fresh.taint()),
        (OriginLabel::LocalOnlyOrigin, OriginLabel::LocalOnlyOrigin),
        "(reply label, unit taint)"
    );
    assert_eq!(fresh.route_class(), RouteClass::Local);
}

// ── N1: survivors of the Architect's mutants on the M2 base ────────────────

/// A non-local unit admits a restricted inclusion on a local route; the
/// accepted reply raises the unit taint by the reply label, and the unit's
/// route class tightens to local.
#[test]
fn hm_m3_n1_accept_reply_raises_the_taint_by_the_reply_label() {
    let e = env();
    // B's LOCAL_ONLY record, written while B was local-only.
    let lo = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    e.issuer
        .write_memory(&lo, &mem_r("SYNTH-b-local-only", 0.9), &[])
        .unwrap();
    fixture_owner_loosen(&e.db, AGENT_B);
    let mut local = local_unit(&e, AGENT_B);
    let recs = e.issuer.recall(&mut local, memories(10)).unwrap();
    assert_eq!(
        recs[0].label(),
        EvaluatedLabel::Bound(OriginLabel::LocalOnlyOrigin)
    );

    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(state(&ctx), UN);
    let included = [&recs[0]];
    let admission = e
        .issuer
        .admit(&mut ctx, &mut including(&included, RouteClass::Local))
        .unwrap();
    assert_eq!(admission.raw_label(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(
        state(&ctx),
        UN,
        "an inclusion alone does not raise the taint"
    );
    let reply = e
        .issuer
        .accept_reply(&mut ctx, admission, Ok("SYNTH-reply".into()))
        .unwrap();
    assert_eq!(reply.label(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(state(&ctx), LO, "the accepted reply raised the taint");
    assert_eq!(
        ctx.working_set().derived_labels(),
        &[OriginLabel::LocalOnlyOrigin]
    );
}

/// An admission is accepted only by the context that was admitted: another
/// context of the same issuer gets `NotThisContext`, and nothing changes.
#[test]
fn hm_m3_n1_accept_reply_refuses_an_admission_of_another_context() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let mut first = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let mut second = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert!(first.same_issuer(&second));
    let none: [&RecalledRecord; 0] = [];
    let admission = e
        .issuer
        .admit(&mut first, &mut including(&none, RouteClass::NonLocal))
        .unwrap();
    let aid = admission.admission_id().to_string();
    assert_eq!(
        e.issuer
            .accept_reply(&mut second, admission, Ok("SYNTH-reply".into()))
            .unwrap_err(),
        ReplyRefused::NotThisContext
    );
    assert!(second.working_set().derived_labels().is_empty());
    assert_eq!(state(&second), UN);
    assert_eq!(outcome_of(&e.db, &aid), "admitted", "no outcome recorded");
    assert!(first.working_set().derived_labels().is_empty());
}

/// Reply acceptance checks the policy generation: a tightening followed by
/// a loosening between a non-local admission and its reply discards the
/// reply, and so does a loosening during a local admission.
#[test]
fn hm_m3_n1_reply_acceptance_checks_the_generation() {
    // (a) Non-local admission; tightened, then loosened, before the reply.
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let none: [&RecalledRecord; 0] = [];
    let admission = e
        .issuer
        .admit(&mut ctx, &mut including(&none, RouteClass::NonLocal))
        .unwrap();
    assert_eq!(admission.route_class(), RouteClass::NonLocal);
    let generation = admission.policy_generation();
    let aid = admission.admission_id().to_string();
    e.cfg.set(id(AGENT_B), true);
    let tightened = e.issuer.resolve_agent_route_policy(id(AGENT_B)).unwrap();
    assert_eq!(
        (tightened.local_only(), tightened.policy_generation()),
        (true, generation + 1)
    );
    e.cfg.set(id(AGENT_B), false);
    assert_eq!(fixture_owner_loosen(&e.db, AGENT_B), generation + 2);
    assert_eq!(
        e.issuer
            .accept_reply(&mut ctx, admission, Ok("SYNTH-reply".into()))
            .unwrap_err(),
        ReplyRefused::Discarded
    );
    assert_eq!(outcome_of(&e.db, &aid), "reply_discarded");

    // (b) Local admission of a local-only agent; loosened before the reply.
    let e = env();
    let mut la = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let admission = e
        .issuer
        .admit(&mut la, &mut including(&none, RouteClass::Local))
        .unwrap();
    assert_eq!(admission.route_class(), RouteClass::Local);
    let aid = admission.admission_id().to_string();
    fixture_owner_loosen(&e.db, AGENT_A);
    assert_eq!(
        e.issuer
            .accept_reply(&mut la, admission, Ok("SYNTH-reply".into()))
            .unwrap_err(),
        ReplyRefused::Discarded
    );
    assert_eq!(outcome_of(&e.db, &aid), "reply_discarded");
}

// ── N1 (R2): the single dispatch entry and contexts of every principal ─────

fn request() -> Unit {
    Unit::Request {
        request_id: Uuid::new_v4(),
    }
}

fn chat() -> Unit {
    Unit::ChatTurn {
        turn_id: Uuid::new_v4(),
    }
}

fn dream() -> Unit {
    Unit::Dream {
        entry_id: Uuid::new_v4(),
    }
}

/// The transport stand-in: every request it is handed, as `(handle, prompt)`.
#[derive(Default)]
struct Transport {
    sent: Vec<(u32, String)>,
}

/// A build for `dispatch`: fixed text, `records` included, `carried`
/// carried, transport handle `handle`, attested `route`.
fn build_for<'r>(
    records: &'r [&'r RecalledRecord],
    carried: &'r [&'r DerivedItem],
    handle: u32,
    route: RouteClass,
) -> impl FnMut(&mut DispatchContext) -> Option<(PromptEnvelope, u32, RouteAttestation)> + 'r {
    move |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        b.text("SYNTH-instruction\n");
        for r in records {
            b.include(r);
        }
        for d in carried {
            b.carry(d);
        }
        Some((b.build(), handle, attest(route)))
    }
}

fn admission_agents(db: &NexusDatabase) -> Vec<String> {
    strings(
        db,
        "SELECT COALESCE(agent_id, 'NULL') || '|' || unit_kind || '|' || route_class || '|'
                || raw_label || '|' || outcome
         FROM hm_dispatch_admission ORDER BY rowid",
    )
}

/// An agent's dispatch: admission, then one send with the admitted bytes
/// and the handle of the admitted build, then reply acceptance. A refusal
/// sends nothing; a transport failure is recorded.
#[test]
fn hm_m3_n1_dispatch_admits_sends_once_and_accepts_for_an_agent() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let mut transport = Transport::default();
    let reply = e
        .issuer
        .dispatch(
            &mut ctx,
            &mut build_for(&[], &[], 7, RouteClass::NonLocal),
            &mut |h, prompt| {
                transport.sent.push((h, prompt.to_string()));
                Ok("SYNTH-reply".into())
            },
        )
        .unwrap();
    assert_eq!(transport.sent, vec![(7, "SYNTH-instruction\n".to_string())]);
    assert_eq!(
        (reply.label(), reply.text()),
        (OriginLabel::UnrestrictedOrigin, "SYNTH-reply")
    );
    assert_eq!(
        admission_agents(&e.db),
        vec![format!(
            "{AGENT_B}|goal|non_local|UNRESTRICTED_ORIGIN|reply_used"
        )]
    );

    // A rebuild: the generation moved before the admission; the handle of
    // the second build is the one sent to, once, on the local route.
    e.cfg.set(id(AGENT_B), true);
    let mut builds = 0;
    let mut transport = Transport::default();
    let mut rebuilt = |c: &mut DispatchContext| {
        builds += 1;
        let b = PromptBuilder::new(c);
        Some((b.build(), builds, attest(c.route_class())))
    };
    e.issuer
        .dispatch(&mut ctx, &mut rebuilt, &mut |h, p| {
            transport.sent.push((h, p.to_string()));
            Ok("SYNTH-local".into())
        })
        .unwrap();
    assert_eq!(builds, 2);
    assert_eq!(transport.sent, vec![(2, String::new())]);
    assert_eq!(state(&ctx), LO);

    // Refused: nothing is sent.
    let mut transport = Transport::default();
    assert_eq!(
        e.issuer
            .dispatch(
                &mut ctx,
                &mut build_for(&[], &[], 9, RouteClass::NonLocal),
                &mut |h, p| {
                    transport.sent.push((h, p.to_string()));
                    Ok(String::new())
                },
            )
            .unwrap_err(),
        DispatchError::Refused(Refused::RouteMismatch)
    );
    assert!(transport.sent.is_empty());

    // A transport failure: its message, and the outcome recorded.
    let err = e
        .issuer
        .dispatch(
            &mut ctx,
            &mut build_for(&[], &[], 3, RouteClass::Local),
            &mut |_, _| Err("SYNTH-transport-down".into()),
        )
        .unwrap_err();
    assert_eq!(err, DispatchError::Transport("SYNTH-transport-down".into()));
    assert_eq!(
        admission_agents(&e.db).last().unwrap(),
        &format!("{AGENT_B}|goal|local|LOCAL_ONLY_ORIGIN|transport_failed")
    );

    // A reply discarded at acceptance: `Reply(Discarded)`.
    let mut la = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let db = e.db.clone();
    let err = e
        .issuer
        .dispatch(
            &mut la,
            &mut build_for(&[], &[], 1, RouteClass::Local),
            &mut |_, _| {
                fixture_owner_loosen(&db, AGENT_A);
                Ok("SYNTH-late".into())
            },
        )
        .unwrap_err();
    assert_eq!(err, DispatchError::Reply(ReplyRefused::Discarded));
}

/// Contexts of non-agent principals (mission R2): no §4.2, no retained
/// data, starting unrestricted and non-local; agent-only units refused.
#[test]
fn hm_m3_n1_begin_unit_supports_every_principal_without_retained_data() {
    let e = env();
    for (p, u) in [
        (Principal::OwnerDirect, chat()),
        (Principal::OwnerDirect, request()),
        (Principal::System("synthetic"), request()),
    ] {
        let mut ctx = e.issuer.begin_unit(p, u).unwrap();
        assert_eq!(state(&ctx), UN);
        assert_eq!(ctx.principal(), p);
        assert!(!ctx.restrictions().retained_data_permitted());
        assert!(!ctx.restrictions().local_only());
        assert!(ctx.working_set().verbatim().next().is_none());
        assert_eq!(
            e.issuer.recall(&mut ctx, memories(5)).unwrap_err(),
            nexus_kernel::hm::RecallRefused::NotAnAgent
        );
        assert_eq!(
            e.issuer
                .write_memory(&ctx, &mem_r("SYNTH-never", 0.1), &[])
                .unwrap_err(),
            WriteRefused::NotAnAgent
        );
    }
    for (p, u) in [
        (Principal::OwnerDirect, goal()),
        (
            Principal::OwnerDirect,
            Unit::TeamTask {
                workflow_id: Uuid::new_v4(),
                task_index: 0,
            },
        ),
        (Principal::System("synthetic"), chat()),
        (Principal::System("synthetic"), goal()),
    ] {
        assert_eq!(
            e.issuer.begin_unit(p, u).unwrap_err(),
            Unresolved::UnsupportedPrincipal
        );
    }
    // A request is a non-agent unit; dreams start only from their entry.
    assert_eq!(
        e.issuer.begin_unit(agent(AGENT_A), request()).unwrap_err(),
        Unresolved::UnsupportedUnit
    );
    for p in [agent(AGENT_A), Principal::OwnerDirect] {
        assert_eq!(
            e.issuer.begin_unit(p, dream()).unwrap_err(),
            Unresolved::UnsupportedUnit
        );
    }
}

/// A plain non-agent dispatch (HD, SY, XS): no admission row; any inclusion
/// is refused; on a non-local route the taint and carried items must be
/// unrestricted; the reply is a derived item of the raw label.
#[test]
fn hm_m3_n1_plain_dispatch_records_nothing_and_refuses_retained_content() {
    let e = env();
    let lo = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    e.issuer
        .write_memory(&lo, &mem_r("SYNTH-a-local-only", 0.9), &[])
        .unwrap();
    let mut a_local = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let recs = e.issuer.recall(&mut a_local, memories(5)).unwrap();
    // A LOCAL_ONLY derived item, from a local admission of A.
    let lo_reply = e
        .issuer
        .dispatch(
            &mut a_local,
            &mut build_for(&[], &[], 0, RouteClass::Local),
            &mut |_, _| Ok("SYNTH-lo-reply".into()),
        )
        .unwrap();
    assert_eq!(lo_reply.label(), OriginLabel::LocalOnlyOrigin);
    let rows = admission_agents(&e.db).len();

    let mut hd = e.issuer.begin_unit(Principal::OwnerDirect, chat()).unwrap();
    let mut sent = 0;
    let mut send = |_: u32, _: &str| {
        sent += 1;
        Ok("SYNTH-hd-reply".to_string())
    };
    let included = [&recs[0]];
    for route in [RouteClass::NonLocal, RouteClass::Local] {
        assert_eq!(
            e.issuer
                .dispatch(&mut hd, &mut build_for(&included, &[], 0, route), &mut send)
                .unwrap_err(),
            DispatchError::Refused(Refused::RetainedDataNotPermitted)
        );
    }
    let r = e
        .issuer
        .dispatch(
            &mut hd,
            &mut build_for(&[], &[], 0, RouteClass::NonLocal),
            &mut send,
        )
        .unwrap();
    assert_eq!(r.label(), OriginLabel::UnrestrictedOrigin);
    assert_eq!(sent, 1);
    assert_eq!(admission_agents(&e.db).len(), rows, "no admission row");

    // A restricted carried item: refused on a non-local route, sent on a
    // local one; the taint rises in both cases (R1 (ii)).
    let mut sy = e
        .issuer
        .begin_unit(Principal::System("synthetic"), request())
        .unwrap();
    let carried = [&lo_reply];
    let sent = std::cell::Cell::new(0);
    let mut send = |_: u32, _: &str| {
        sent.set(sent.get() + 1);
        Ok("SYNTH-sy-reply".to_string())
    };
    assert_eq!(
        e.issuer
            .dispatch(
                &mut sy,
                &mut build_for(&[], &carried, 0, RouteClass::NonLocal),
                &mut send,
            )
            .unwrap_err(),
        DispatchError::Refused(Refused::Ineligible)
    );
    assert_eq!(state(&sy), LO);
    let local = e
        .issuer
        .dispatch(
            &mut sy,
            &mut build_for(&[], &carried, 0, RouteClass::Local),
            &mut send,
        )
        .unwrap();
    assert_eq!(local.label(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(sent.get(), 1);
    // Once restricted, a non-local transport no longer fits the unit.
    assert_eq!(
        e.issuer
            .dispatch(
                &mut sy,
                &mut build_for(&[], &[], 0, RouteClass::NonLocal),
                &mut send,
            )
            .unwrap_err(),
        DispatchError::Refused(Refused::RouteMismatch)
    );
    assert_eq!(sent.get(), 1);
    assert_eq!(admission_agents(&e.db).len(), rows, "no admission row");
}

/// Entry labels (§11.3): set only from a chat turn, the ⊔ of its taint and
/// route restriction, with the turn's principal. An Owner's entry starts
/// an HR dream unit (an admission row with no agent, non-local only when
/// unrestricted); an agent's entry starts a dream unit of that agent.
#[test]
fn hm_m3_n1_entry_labels_and_dream_units_of_every_principal() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let noop = |_: u32, _: &str| Ok::<_, String>("SYNTH".to_string());

    // Not a chat turn, or a System principal: no entry label.
    let g = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert!(g.entry_label().is_none());
    let sy = e
        .issuer
        .begin_unit(Principal::System("synthetic"), request())
        .unwrap();
    assert!(sy.entry_label().is_none());

    // An Owner's turn on a non-local transport: unrestricted entry.
    let mut hd = e.issuer.begin_unit(Principal::OwnerDirect, chat()).unwrap();
    e.issuer
        .dispatch(
            &mut hd,
            &mut build_for(&[], &[], 0, RouteClass::NonLocal),
            &mut noop.clone(),
        )
        .unwrap();
    let open_entry = hd.entry_label().unwrap();
    assert_eq!(
        (open_entry.principal(), open_entry.label()),
        (Principal::OwnerDirect, OriginLabel::UnrestrictedOrigin)
    );
    // ... its dream unit (HR) may go non-local; the admission has no agent.
    let mut hr = e.issuer.begin_entry_unit(&open_entry, dream()).unwrap();
    assert_eq!(state(&hr), UN);
    assert!(!hr.restrictions().retained_data_permitted());
    e.issuer
        .dispatch(
            &mut hr,
            &mut build_for(&[], &[], 0, RouteClass::NonLocal),
            &mut noop.clone(),
        )
        .unwrap();
    assert_eq!(
        admission_agents(&e.db),
        vec!["NULL|dream|non_local|UNRESTRICTED_ORIGIN|reply_used".to_string()]
    );

    // An Owner's turn on an attested local transport: LOCAL_ONLY entry;
    // its dream unit is local and a non-local dispatch is refused.
    let mut hd_local = e.issuer.begin_unit(Principal::OwnerDirect, chat()).unwrap();
    e.issuer
        .dispatch(
            &mut hd_local,
            &mut build_for(&[], &[], 0, RouteClass::Local),
            &mut noop.clone(),
        )
        .unwrap();
    let local_entry = hd_local.entry_label().unwrap();
    assert_eq!(local_entry.label(), OriginLabel::LocalOnlyOrigin);
    let mut hr_local = e.issuer.begin_entry_unit(&local_entry, dream()).unwrap();
    assert_eq!(state(&hr_local), LO);
    let mut sent = 0;
    assert_eq!(
        e.issuer
            .dispatch(
                &mut hr_local,
                &mut build_for(&[], &[], 0, RouteClass::NonLocal),
                &mut |_, _| {
                    sent += 1;
                    Ok(String::new())
                },
            )
            .unwrap_err(),
        DispatchError::Refused(Refused::RouteMismatch)
    );
    assert_eq!(sent, 0);
    let r = e
        .issuer
        .dispatch(
            &mut hr_local,
            &mut build_for(&[], &[], 0, RouteClass::Local),
            &mut noop.clone(),
        )
        .unwrap();
    assert_eq!(r.label(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(
        admission_agents(&e.db).last().unwrap(),
        "NULL|dream|local|LOCAL_ONLY_ORIGIN|reply_used"
    );

    // An agent-routed turn of a local-only agent: an entry of that agent,
    // LOCAL_ONLY; its dream unit is a local unit of that agent.
    let a_chat = e.issuer.begin_unit(agent(AGENT_A), chat()).unwrap();
    let a_entry = a_chat.entry_label().unwrap();
    assert_eq!(
        (a_entry.principal(), a_entry.label()),
        (agent(AGENT_A), OriginLabel::LocalOnlyOrigin)
    );
    let a_dream = e.issuer.begin_entry_unit(&a_entry, dream()).unwrap();
    assert_eq!(a_dream.principal(), agent(AGENT_A));
    assert_eq!(state(&a_dream), LO);
    // An unrestricted agent's turn that went to a local transport: the
    // entry is LOCAL_ONLY, so the agent's dream unit stays local.
    let mut b_chat = e.issuer.begin_unit(agent(AGENT_B), chat()).unwrap();
    e.issuer
        .dispatch(
            &mut b_chat,
            &mut build_for(&[], &[], 0, RouteClass::Local),
            &mut noop.clone(),
        )
        .unwrap();
    let b_entry = b_chat.entry_label().unwrap();
    assert_eq!(b_entry.label(), OriginLabel::LocalOnlyOrigin);
    let b_dream = e.issuer.begin_entry_unit(&b_entry, dream()).unwrap();
    assert_eq!(state(&b_dream), LO);
    // Only dream units start from an entry.
    assert_eq!(
        e.issuer.begin_entry_unit(&b_entry, goal()).unwrap_err(),
        Unresolved::UnsupportedUnit
    );
}
