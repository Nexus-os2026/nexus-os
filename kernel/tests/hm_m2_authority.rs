//! P3-C11-HM-M2: the dormant kernel authority core of P3-HM-CONTRACT-02
//! (sealed context, §4.2 resolution, labelled writes, route-first recall,
//! §12.4 admission, reply acceptance, events and back-fill).
//!
//! Synthetic databases only: files in a test-owned temporary directory.
//! Crash steps run in child processes of this test binary; readiness is a
//! line on a pipe, never a sleep. Test names carry their node (`n1_` …).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use nexus_kernel::hm::{
    backfill_authority_events, evaluate_label, run_t0_option_a, unit_start_state, unit_taint_step,
    verify_at_start, AgentRegistry, BackfillReport, ConfigInput, ConfigReadError, ConfigReading,
    DispatchContext, DispatchIssuer, EvaluatedLabel, MemberState, NewMemory, OriginLabel, Parent,
    PolicySnapshot, Principal, ProbePoint, PromptBuilder, PromptEnvelope, RecallQuery,
    RecallRefused, RecalledRecord, RecordTable, Refused, ReplyRefused, RouteAttestation,
    RouteClass, SourceKind, StartStatus, StrategyWrite, TaskWrite, TransportEvidence,
    TransportFailure, Unit, Unresolved, WriteRefused, MAX_RECORDS,
};
use nexus_persistence::hm::{PolicyChangeRow, PolicyRow};
use nexus_persistence::NexusDatabase;
use uuid::Uuid;

// ── shared helpers ──────────────────────────────────────────────────────────

const AGENT_A: &str = "0a000000-0000-4000-8000-00000000000a";
const AGENT_B: &str = "0b000000-0000-4000-8000-00000000000b";
const AGENT_C: &str = "0c000000-0000-4000-8000-00000000000c";
const AGENT_D: &str = "0d000000-0000-4000-8000-00000000000d";
const AGENT_E: &str = "0e000000-0000-4000-8000-00000000000e";

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

    fn remove(&self, agent_id: Uuid) {
        self.agents.lock().unwrap().remove(&agent_id);
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

/// The untrusted config file stand-in: a `local_only` value per agent, a
/// content digest, and a switch that makes every read fail.
struct TestConfig {
    local: Mutex<HashMap<Uuid, bool>>,
    digest: Mutex<String>,
    fail: AtomicBool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            local: Mutex::new(HashMap::new()),
            digest: Mutex::new("cfg-digest-0".into()),
            fail: AtomicBool::new(false),
        }
    }
}

impl TestConfig {
    fn set(&self, agent_id: Uuid, local_only: bool) {
        self.local.lock().unwrap().insert(agent_id, local_only);
    }

    fn set_digest(&self, digest: &str) {
        *self.digest.lock().unwrap() = digest.into();
    }

    fn set_failing(&self, fail: bool) {
        self.fail.store(fail, Ordering::SeqCst);
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
            content_digest: self.digest.lock().unwrap().clone(),
        })
    }
}

/// A migrated synthetic database file, its registry, config and issuer.
struct Env {
    _dir: tempfile::TempDir,
    path: PathBuf,
    db: Arc<NexusDatabase>,
    reg: Arc<TestRegistry>,
    cfg: Arc<TestConfig>,
    issuer: DispatchIssuer,
}

impl Env {
    /// Another issuer on the same database handle (another process's).
    fn issuer_on(&self, db: Arc<NexusDatabase>) -> DispatchIssuer {
        DispatchIssuer::new(db, self.reg.clone(), self.cfg.clone()).unwrap()
    }

    /// A second handle on the same file.
    fn second_handle(&self) -> Arc<NexusDatabase> {
        Arc::new(open(&self.path))
    }
}

fn open(path: &Path) -> NexusDatabase {
    NexusDatabase::open(path).expect("open synthetic database")
}

/// T₀ (Option A) over `t0_agents`; `registered` are known to the registry.
fn env_with(t0_agents: &[&str], registered: &[&str]) -> Env {
    env_seeded(t0_agents, registered, "")
}

/// As `env_with`, with pre-T₀ rows inserted by `seed_sql` first.
fn env_seeded(t0_agents: &[&str], registered: &[&str], seed_sql: &str) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let db = Arc::new(open(&path));
    if !seed_sql.is_empty() {
        must(&db, seed_sql);
    }
    let t0: Vec<Uuid> = t0_agents.iter().map(|s| id(s)).collect();
    run_t0_option_a(&db, &t0).unwrap();
    let reg = Arc::new(TestRegistry::default());
    for s in registered {
        reg.add(id(s));
    }
    let cfg = Arc::new(TestConfig::default());
    let issuer = DispatchIssuer::new(db.clone(), reg.clone(), cfg.clone()).unwrap();
    Env {
        _dir: dir,
        path,
        db,
        reg,
        cfg,
        issuer,
    }
}

/// Agents A–D at T₀ (all local-only under Option A), A–E registered.
fn env() -> Env {
    env_with(
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D],
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D, AGENT_E],
    )
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

/// One text column of every result row.
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

/// `local_only|policy_generation|revocation_seq|basis` of one agent.
fn policy_of(db: &NexusDatabase, agent_id: &str) -> String {
    strings(
        db,
        &format!(
            "SELECT local_only || '|' || policy_generation || '|' || revocation_seq || '|' || basis
             FROM hm_agent_policy WHERE agent_id = '{agent_id}'"
        ),
    )
    .join(";")
}

/// `generation|local_only|basis|cause` history rows of one agent, in order.
fn history_of(db: &NexusDatabase, agent_id: &str) -> Vec<String> {
    strings(
        db,
        &format!(
            "SELECT policy_generation || '|' || local_only || '|' || basis || '|' || cause
             FROM hm_policy_change WHERE agent_id = '{agent_id}' ORDER BY policy_generation"
        ),
    )
}

/// `(subject, agent_id, fields)` of every event of one kind, in order.
fn events_of(db: &NexusDatabase, kind: &str) -> Vec<(String, Option<String>, String)> {
    db.with_transaction(|tx| {
        let mut stmt = tx
            .prepare("SELECT subject, agent_id, fields FROM hm_event WHERE kind = ?1 ORDER BY n")?;
        let rows = stmt.query_map([kind], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row?);
        }
        Ok(v)
    })
    .unwrap()
}

fn assert_obs_subject(subject: &str, kind: &str) {
    let rest = subject
        .strip_prefix(&format!("obs:{kind}:"))
        .unwrap_or_else(|| panic!("observational subject expected: {subject}"));
    assert_eq!(rest.len(), 32, "128-bit id");
    assert!(rest.chars().all(|c| c.is_ascii_hexdigit()));
}

/// TEST FIXTURE, not product code. Owner loosening (§4.4, §4.5) is not in
/// this mission; a test that needs an unrestricted agent commits an
/// `owner_loosen` history row and the policy update directly through the
/// store. Returns the new generation.
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

/// TEST FIXTURE: corrupts one policy row in the synthetic database,
/// bypassing the §5.1 policy triggers and CHECK constraints.
fn fixture_corrupt_policy(db: &NexusDatabase, agent_id: &str, set_clause: &str) {
    must(
        db,
        "DROP TRIGGER hm_agent_policy_no_lowering;
         DROP TRIGGER hm_agent_policy_update_requires_history;",
    );
    must(
        db,
        &format!(
            "PRAGMA ignore_check_constraints = ON;
             UPDATE hm_agent_policy SET {set_clause} WHERE agent_id = '{agent_id}';
             PRAGMA ignore_check_constraints = OFF;"
        ),
    );
}

const LABELS: [OriginLabel; 3] = [
    OriginLabel::UnrestrictedOrigin,
    OriginLabel::LegacyUnknown,
    OriginLabel::LocalOnlyOrigin,
];

const ROUTES: [RouteClass; 2] = [RouteClass::Local, RouteClass::NonLocal];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Every `.rs` file of the `nexus_kernel::hm` module.
fn hm_sources() -> Vec<(String, String)> {
    let kernel = workspace_root().join("kernel/src");
    let mut files = vec![kernel.join("hm.rs")];
    let mut sub: Vec<PathBuf> = std::fs::read_dir(kernel.join("hm"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    sub.sort();
    files.extend(sub);
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(workspace_root())
                .unwrap()
                .display()
                .to_string();
            (rel, std::fs::read_to_string(&p).unwrap())
        })
        .collect()
}

// ── N1: taint algebra (§2.3, §2.12, §6.3, §7.5) ─────────────────────────────

#[test]
fn hm_m2_n1_origin_order_and_join_exhaustive() {
    use OriginLabel::*;
    assert!(UnrestrictedOrigin < LegacyUnknown && LegacyUnknown < LocalOnlyOrigin);
    for a in LABELS {
        assert_eq!(a.join(a), a, "idempotent");
        assert_eq!(
            a.join(UnrestrictedOrigin),
            a,
            "UNRESTRICTED is the identity"
        );
        assert_eq!(
            a.join(LocalOnlyOrigin),
            LocalOnlyOrigin,
            "LOCAL_ONLY absorbs"
        );
        for b in LABELS {
            let j = a.join(b);
            assert_eq!(j, b.join(a), "commutative");
            assert!(j >= a && j >= b, "upper bound");
            // least upper bound: no smaller label bounds both
            for c in LABELS {
                if c >= a && c >= b {
                    assert!(j <= c, "least upper bound {a:?} {b:?} {c:?}");
                }
                assert_eq!(a.join(b).join(c), a.join(b.join(c)), "associative");
            }
        }
    }
    let table: Vec<(&str, &str, &str)> = LABELS
        .iter()
        .flat_map(|a| {
            LABELS
                .iter()
                .map(move |b| (a.class_name(), b.class_name(), a.join(*b).class_name()))
        })
        .collect();
    assert_eq!(
        table,
        vec![
            (
                "UNRESTRICTED_ORIGIN",
                "UNRESTRICTED_ORIGIN",
                "UNRESTRICTED_ORIGIN"
            ),
            ("UNRESTRICTED_ORIGIN", "LEGACY_UNKNOWN", "LEGACY_UNKNOWN"),
            (
                "UNRESTRICTED_ORIGIN",
                "LOCAL_ONLY_ORIGIN",
                "LOCAL_ONLY_ORIGIN"
            ),
            ("LEGACY_UNKNOWN", "UNRESTRICTED_ORIGIN", "LEGACY_UNKNOWN"),
            ("LEGACY_UNKNOWN", "LEGACY_UNKNOWN", "LEGACY_UNKNOWN"),
            ("LEGACY_UNKNOWN", "LOCAL_ONLY_ORIGIN", "LOCAL_ONLY_ORIGIN"),
            (
                "LOCAL_ONLY_ORIGIN",
                "UNRESTRICTED_ORIGIN",
                "LOCAL_ONLY_ORIGIN"
            ),
            ("LOCAL_ONLY_ORIGIN", "LEGACY_UNKNOWN", "LOCAL_ONLY_ORIGIN"),
            (
                "LOCAL_ONLY_ORIGIN",
                "LOCAL_ONLY_ORIGIN",
                "LOCAL_ONLY_ORIGIN"
            ),
        ]
    );
}

#[test]
fn hm_m2_n1_unbound_evaluates_local_only() {
    assert_eq!(
        EvaluatedLabel::Unbound.effective(),
        OriginLabel::LocalOnlyOrigin
    );
    for l in LABELS {
        assert_eq!(EvaluatedLabel::Bound(l).effective(), l);
        assert!(EvaluatedLabel::Unbound.effective() >= l);
    }
}

#[test]
fn hm_m2_n1_unit_start_state_all_combinations() {
    for local_only in [false, true] {
        for floor in [false, true] {
            let (taint, route) = unit_start_state(local_only, floor);
            if local_only || floor {
                assert_eq!(
                    (taint, route),
                    (OriginLabel::LocalOnlyOrigin, RouteClass::Local)
                );
            } else {
                assert_eq!(
                    (taint, route),
                    (OriginLabel::UnrestrictedOrigin, RouteClass::NonLocal)
                );
            }
        }
    }
}

#[test]
fn hm_m2_n1_taint_step_exhaustive_never_falls_and_route_tightens_only() {
    // Every (taint, route, label): the taint joins the label; the route
    // class becomes local once the taint is restricted and never loosens.
    for taint in LABELS {
        for route in ROUTES {
            for label in LABELS {
                let (t, r) = unit_taint_step(taint, route, label);
                assert_eq!(t, taint.join(label), "{taint:?} {route:?} {label:?}");
                assert!(t >= taint, "taint never falls");
                let restricted = t != OriginLabel::UnrestrictedOrigin;
                let expect = if route == RouteClass::Local || restricted {
                    RouteClass::Local
                } else {
                    RouteClass::NonLocal
                };
                assert_eq!(r, expect, "{taint:?} {route:?} {label:?}");
                // idempotent
                assert_eq!(unit_taint_step(t, r, label), (t, r));
            }
        }
    }
    // Every sequence of three steps from every start state: monotone, and
    // the result is the join of the start and the sequence.
    for local_only in [false, true] {
        for floor in [false, true] {
            let start = unit_start_state(local_only, floor);
            for a in LABELS {
                for b in LABELS {
                    for c in LABELS {
                        let mut state = start;
                        let mut prev = state;
                        for l in [a, b, c] {
                            state = unit_taint_step(state.0, state.1, l);
                            assert!(state.0 >= prev.0);
                            assert!(
                                !(prev.1 == RouteClass::Local && state.1 == RouteClass::NonLocal)
                            );
                            prev = state;
                        }
                        assert_eq!(state.0, start.0.join(a).join(b).join(c));
                    }
                }
            }
        }
    }
}

/// Public functions of `nexus_kernel::hm` whose parameters name a label or a
/// route class. Only the pure algebra may take one: a caller-named label is
/// never an authority input.
fn label_taking_pub_fns(src: &str) -> Vec<String> {
    let flat: String = src.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = Vec::new();
    let mut rest = flat.as_str();
    while let Some(i) = rest.find("pub fn ") {
        rest = &rest[i + "pub fn ".len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let Some(open) = rest.find('(') else { break };
        let mut depth = 0usize;
        let mut end = open;
        for (j, ch) in rest[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + j;
                        break;
                    }
                }
                _ => {}
            }
        }
        let params = &rest[open + 1..end];
        if ["OriginLabel", "EvaluatedLabel", "RouteClass"]
            .iter()
            .any(|t| params.contains(t))
        {
            out.push(name);
        }
    }
    out
}

#[test]
fn hm_m2_n1_no_public_fn_takes_a_label_as_input() {
    // Planted negative controls: the scanner must flag these.
    assert_eq!(
        label_taking_pub_fns("impl X { pub fn admit_as(&self,\n label: OriginLabel) {} }"),
        vec!["admit_as"]
    );
    assert_eq!(
        label_taking_pub_fns("pub fn f(e: EvaluatedLabel) -> u8 { 0 }"),
        vec!["f"]
    );
    assert_eq!(
        label_taking_pub_fns("pub fn g(r: RouteClass) {}"),
        vec!["g"]
    );
    assert!(label_taking_pub_fns("pub fn h(&self) -> OriginLabel { x }").is_empty());

    const PURE_ALGEBRA: [&str; 2] = ["join", "unit_taint_step"];
    let mut found = Vec::new();
    for (file, src) in hm_sources() {
        for name in label_taking_pub_fns(&src) {
            if !PURE_ALGEBRA.contains(&name.as_str()) {
                found.push(format!("{file}: {name}"));
            }
        }
    }
    assert!(found.is_empty(), "label-taking public fns: {found:?}");
}

// ── N2: issuer and §4.2 ─────────────────────────────────────────────────────

fn state(c: &DispatchContext) -> (OriginLabel, RouteClass) {
    (c.taint(), c.route_class())
}

const LO: (OriginLabel, RouteClass) = (OriginLabel::LocalOnlyOrigin, RouteClass::Local);
const UN: (OriginLabel, RouteClass) = (OriginLabel::UnrestrictedOrigin, RouteClass::NonLocal);

#[test]
fn hm_m2_n2_o1_unregistered_unmigrated_malformed_unreadable_are_unresolved() {
    // Unregistered: no context, and no policy row is created for it.
    let e = env();
    let stranger = Uuid::new_v4();
    assert_eq!(
        e.issuer
            .begin_unit(Principal::Agent(stranger), goal())
            .unwrap_err(),
        Unresolved::UnregisteredAgent
    );
    assert_eq!(
        count(
            &e.db,
            &format!("SELECT COUNT(*) FROM hm_agent_policy WHERE agent_id = '{stranger}'")
        ),
        0
    );
    // A registration withdrawn later is unresolved from then on.
    e.issuer.begin_unit(agent(AGENT_E), goal()).unwrap();
    e.reg.remove(id(AGENT_E));
    assert_eq!(
        e.issuer.begin_unit(agent(AGENT_E), goal()).unwrap_err(),
        Unresolved::UnregisteredAgent
    );

    // Unmigrated store: no hm_* table is created by resolution.
    let dir = tempfile::tempdir().unwrap();
    let raw = Arc::new(open(&dir.path().join("state.db")));
    let issuer = DispatchIssuer::new(raw.clone(), e.reg.clone(), e.cfg.clone()).unwrap();
    assert_eq!(
        issuer.begin_unit(agent(AGENT_A), goal()).unwrap_err(),
        Unresolved::StoreNotMigrated
    );
    assert_eq!(
        count(
            &raw,
            "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'hm\\_%' ESCAPE '\\'"
        ),
        0
    );

    // Malformed rows: wrong type, generation < 1, sequence < 0, bad values.
    for set in [
        "policy_generation = 0",
        "revocation_seq = -1",
        "local_only = 'x'",
        "local_only = 2",
        "policy_generation = 'two'",
        "basis = 'free'",
    ] {
        let e = env();
        fixture_corrupt_policy(&e.db, AGENT_A, set);
        assert_eq!(
            e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap_err(),
            Unresolved::MalformedPolicy,
            "{set}"
        );
        assert_eq!(
            e.issuer
                .resolve_agent_route_policy(id(AGENT_A))
                .unwrap_err(),
            Unresolved::MalformedPolicy,
            "{set}"
        );
        assert_eq!(
            history_of(&e.db, AGENT_A).len(),
            1,
            "{set}: nothing written"
        );
    }

    // Unreadable config: unresolved, and nothing is written (the config is
    // read before any write; E has no row yet).
    let e = env();
    e.cfg.set_failing(true);
    for who in [AGENT_A, AGENT_E] {
        assert_eq!(
            e.issuer.begin_unit(agent(who), goal()).unwrap_err(),
            Unresolved::ConfigUnreadable
        );
    }
    assert_eq!(policy_of(&e.db, AGENT_E), "");
    assert_eq!(policy_of(&e.db, AGENT_A), "1|1|0|t0_baseline");
}

/// Registers a probe that, at `point`, records what a second handle sees:
/// `(policy rows matching the expected committed row, events with subject)`.
fn probe_committed_before_event(
    e: &Env,
    point: ProbePoint,
    agent_id: &'static str,
    expect_policy: &'static str,
    subject: String,
) -> Arc<Mutex<Vec<(String, i64)>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (path, out) = (e.path.clone(), seen.clone());
    e.issuer.install_probe(Box::new(move |p| {
        if p == point {
            let db = open(&path);
            let committed = policy_of(&db, agent_id);
            let events = count(
                &db,
                &format!("SELECT COUNT(*) FROM hm_event WHERE subject = '{subject}'"),
            );
            assert_eq!(committed, expect_policy);
            out.lock().unwrap().push((committed, events));
        }
    }));
    seen
}

#[test]
fn hm_m2_n2_missing_row_created_restrictive_with_history_then_event() {
    // E is registered after T₀: no row.
    let e = env();
    let subject = format!("policy:{AGENT_E}:1");
    let seen = probe_committed_before_event(
        &e,
        ProbePoint::PolicyRowCreated,
        AGENT_E,
        "1|1|0|default_restrictive",
        subject.clone(),
    );
    let ctx = e.issuer.begin_unit(agent(AGENT_E), goal()).unwrap();
    assert_eq!(state(&ctx), LO);
    assert!(ctx.policy().local_only());
    assert_eq!(ctx.policy().policy_generation(), 1);
    assert_eq!(ctx.policy().revocation_seq(), 0);
    // The probe saw the row committed and its event not yet written.
    assert_eq!(
        *seen.lock().unwrap(),
        vec![("1|1|0|default_restrictive".to_string(), 0)]
    );
    assert_eq!(policy_of(&e.db, AGENT_E), "1|1|0|default_restrictive");
    assert_eq!(
        history_of(&e.db, AGENT_E),
        vec!["1|1|default_restrictive|row_created"]
    );
    let ev = events_of(&e.db, "memory.policy_row_created");
    assert_eq!(
        ev,
        vec![(
            subject,
            Some(AGENT_E.to_string()),
            r#"{"local_only":1,"policy_generation":1}"#.to_string()
        )]
    );
    // A second resolution writes nothing.
    e.issuer.begin_unit(agent(AGENT_E), goal()).unwrap();
    assert_eq!(history_of(&e.db, AGENT_E).len(), 1);
    assert_eq!(events_of(&e.db, "memory.policy_row_created").len(), 1);
}

#[test]
fn hm_m2_n2_config_tightening_commits_generation_plus_one_with_history_then_event() {
    let e = env();
    assert_eq!(fixture_owner_loosen(&e.db, AGENT_A), 2);
    let before = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&before), UN);

    // An out-of-band config edit: observed at the next O1.
    e.cfg.set(id(AGENT_A), true);
    let subject = format!("policy:{AGENT_A}:3");
    let seen = probe_committed_before_event(
        &e,
        ProbePoint::PolicyTightened,
        AGENT_A,
        "1|3|0|tightened",
        subject.clone(),
    );
    let ctx = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&ctx), LO);
    assert_eq!(ctx.policy().policy_generation(), 3);
    assert_eq!(
        *seen.lock().unwrap(),
        vec![("1|3|0|tightened".to_string(), 0)]
    );
    assert_eq!(policy_of(&e.db, AGENT_A), "1|3|0|tightened");
    assert_eq!(
        history_of(&e.db, AGENT_A),
        vec![
            "1|1|t0_baseline|t0_baseline",
            "2|0|owner_confirmed|owner_loosen",
            "3|1|tightened|config_tighten"
        ]
    );
    assert_eq!(
        events_of(&e.db, "memory.policy_tightened_by_config"),
        vec![(
            subject,
            Some(AGENT_A.to_string()),
            r#"{"local_only":1,"policy_generation":3}"#.to_string()
        )]
    );
    // Already local-only: a further reading changes nothing.
    e.issuer.resolve_agent_route_policy(id(AGENT_A)).unwrap();
    assert_eq!(history_of(&e.db, AGENT_A).len(), 3);
    // The running unit that started before the commit keeps its snapshot.
    assert_eq!(before.policy().policy_generation(), 2);
}

#[test]
fn hm_m2_n2_config_loosening_never_loosens_and_is_audited_once_per_digest() {
    // A-P3 (v) at O1: local-only agents; the config says false.
    let e = env();
    e.cfg.set(id(AGENT_A), false);
    e.cfg.set_digest("cfg-digest-1");
    for _ in 0..3 {
        let ctx = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
        assert_eq!(state(&ctx), LO);
        let p = e.issuer.resolve_agent_route_policy(id(AGENT_A)).unwrap();
        assert!(p.local_only());
        assert_eq!(p.policy_generation(), 1);
    }
    assert_eq!(policy_of(&e.db, AGENT_A), "1|1|0|t0_baseline");
    assert_eq!(history_of(&e.db, AGENT_A).len(), 1);
    let kind = "memory.policy_loosening_not_authorised";
    let ev = events_of(&e.db, kind);
    assert_eq!(ev.len(), 1, "once per (agent, digest)");
    assert_obs_subject(&ev[0].0, kind);
    assert_eq!(ev[0].1.as_deref(), Some(AGENT_A));
    assert_eq!(ev[0].2, r#"{"policy_generation":1}"#);

    // New config content: audited once more.
    e.cfg.set_digest("cfg-digest-2");
    e.issuer.resolve_agent_route_policy(id(AGENT_A)).unwrap();
    e.issuer.resolve_agent_route_policy(id(AGENT_A)).unwrap();
    assert_eq!(events_of(&e.db, kind).len(), 2);

    // A config-tightened agent whose config later says false stays local.
    fixture_owner_loosen(&e.db, AGENT_B);
    e.cfg.set(id(AGENT_B), true);
    e.issuer.resolve_agent_route_policy(id(AGENT_B)).unwrap();
    e.cfg.set(id(AGENT_B), false);
    let p = e.issuer.resolve_agent_route_policy(id(AGENT_B)).unwrap();
    assert!(p.local_only());
    assert_eq!(policy_of(&e.db, AGENT_B), "1|3|0|tightened");
    assert_eq!(events_of(&e.db, kind).len(), 3);

    // Another process (issuer) audits again; still nothing loosens.
    let other = e.issuer_on(e.second_handle());
    other.resolve_agent_route_policy(id(AGENT_A)).unwrap();
    assert_eq!(events_of(&e.db, kind).len(), 4);
    assert_eq!(policy_of(&e.db, AGENT_A), "1|1|0|t0_baseline");
}

#[test]
fn hm_m2_n2_derivations_never_lower_taint_a_t1_a_h1() {
    let e = env();
    // A and B unrestricted (fixture); C and D local-only (T₀ Option A).
    fixture_owner_loosen(&e.db, AGENT_A);
    fixture_owner_loosen(&e.db, AGENT_B);
    let issuer = &e.issuer;
    let wf = Uuid::new_v4();

    // A-T1: unrestricted director, local-only member.
    let mut director = issuer
        .begin_unit(
            agent(AGENT_A),
            Unit::TeamTask {
                workflow_id: wf,
                task_index: 0,
            },
        )
        .unwrap();
    assert_eq!(state(&director), UN);
    let local_member = director.derive_member(issuer, id(AGENT_C), 1).unwrap();
    assert_eq!(state(&local_member), LO);
    assert_eq!(local_member.principal(), agent(AGENT_C));
    assert_eq!(
        local_member.unit(),
        Unit::TeamTask {
            workflow_id: wf,
            task_index: 1
        }
    );
    assert!(!local_member.floor_local_only());
    let open_member = director.derive_member(issuer, id(AGENT_B), 2).unwrap();
    assert_eq!(state(&open_member), UN);
    director.join_child(&open_member).unwrap();
    assert_eq!(state(&director), UN);
    director.join_child(&local_member).unwrap();
    assert_eq!(state(&director), LO, "member result joins the director");
    assert!(director
        .working_set()
        .derived_labels()
        .contains(&OriginLabel::LocalOnlyOrigin));
    // The director is now restricted by taint alone: a member derived now
    // inherits it, although both agents are unrestricted.
    let late = director.derive_member(issuer, id(AGENT_B), 3).unwrap();
    assert!(!late.floor_local_only() && !late.policy().local_only());
    assert_eq!(state(&late), LO, "inherited taint never falls");

    // Local-only director: the floor binds an unrestricted member.
    let strict = issuer
        .begin_unit(
            agent(AGENT_C),
            Unit::TeamTask {
                workflow_id: Uuid::new_v4(),
                task_index: 0,
            },
        )
        .unwrap();
    let floored = strict.derive_member(issuer, id(AGENT_B), 1).unwrap();
    assert!(floored.floor_local_only());
    assert!(floored.restrictions().local_only());
    assert!(!floored.policy().local_only());
    assert_eq!(state(&floored), LO);

    // Every director state × member policy: child ≥ director, and
    // child = director taint ⊔ start(member ∨ floor).
    for (dir_agent, member_agent) in [
        (AGENT_A, AGENT_B),
        (AGENT_A, AGENT_D),
        (AGENT_C, AGENT_B),
        (AGENT_C, AGENT_D),
    ] {
        let d = issuer
            .begin_unit(
                agent(dir_agent),
                Unit::TeamTask {
                    workflow_id: Uuid::new_v4(),
                    task_index: 0,
                },
            )
            .unwrap();
        let m = d.derive_member(issuer, id(member_agent), 1).unwrap();
        let start = unit_start_state(m.policy().local_only(), d.restrictions().local_only());
        assert!(m.taint() >= d.taint());
        assert_eq!(m.taint(), d.taint().join(start.0));
        assert_eq!(
            m.route_class() == RouteClass::Local,
            m.taint() != OriginLabel::UnrestrictedOrigin
        );
    }

    // A-H1: sub-task taints join into the merge, in any order.
    let session_unit = Unit::HiveSubtask {
        session_id: Uuid::new_v4(),
        subtask_id: 0,
    };
    let session = issuer.begin_unit(agent(AGENT_B), session_unit).unwrap();
    let s1 = session.derive_subtask(issuer, id(AGENT_A), 1).unwrap();
    let s2 = session.derive_subtask(issuer, id(AGENT_D), 2).unwrap();
    assert_eq!((state(&s1), state(&s2)), (UN, LO));
    for order in [[&s1, &s2], [&s2, &s1]] {
        let mut merge = issuer.begin_unit(agent(AGENT_B), session_unit).unwrap();
        for child in order {
            merge.join_child(child).unwrap();
        }
        assert_eq!(state(&merge), LO);
    }

    // Post-goal keeps the goal's taint and policy snapshot; nothing is
    // re-resolved, so a later loosening cannot reach it.
    let mut g = issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    g.join_child(&local_member).unwrap();
    let pg = g.derive_post_goal().unwrap();
    assert_eq!(state(&pg), LO);
    assert_eq!(pg.policy(), g.policy());
    assert!(matches!(pg.unit(), Unit::PostGoal { .. }));
    let clean = issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&clean.derive_post_goal().unwrap()), UN);
    fixture_owner_loosen(&e.db, AGENT_C);
    assert_eq!(state(&local_member), LO, "a running unit never loosens");

    // Wrong unit kinds and unregistered children are refused.
    assert_eq!(
        g.derive_member(issuer, id(AGENT_B), 1).unwrap_err(),
        Unresolved::UnsupportedUnit
    );
    assert_eq!(
        pg.derive_post_goal().unwrap_err(),
        Unresolved::UnsupportedUnit
    );
    assert_eq!(
        session
            .derive_subtask(issuer, Uuid::new_v4(), 9)
            .unwrap_err(),
        Unresolved::UnregisteredAgent
    );
}

#[test]
fn hm_m2_n2_begin_unit_refuses_non_agent_principals_and_derived_only_units() {
    let e = env();
    for p in [Principal::OwnerDirect, Principal::System("synthetic")] {
        assert_eq!(
            e.issuer.begin_unit(p, goal()).unwrap_err(),
            Unresolved::UnsupportedPrincipal
        );
    }
    assert_eq!(
        e.issuer
            .begin_unit(
                agent(AGENT_A),
                Unit::PostGoal {
                    goal_id: Uuid::new_v4()
                }
            )
            .unwrap_err(),
        Unresolved::UnsupportedUnit
    );
    let chat = e
        .issuer
        .begin_unit(
            agent(AGENT_A),
            Unit::ChatTurn {
                turn_id: Uuid::new_v4(),
            },
        )
        .unwrap();
    assert!(!chat.restrictions().retained_data_permitted());
    let g = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert!(g.restrictions().retained_data_permitted());
    assert_eq!(g.restrictions().max_records(), MAX_RECORDS);
}

#[test]
fn hm_m2_n2_contexts_are_bound_to_their_issuer() {
    let e = env();
    let other = e.issuer_on(e.second_handle());
    let wf = Unit::TeamTask {
        workflow_id: Uuid::new_v4(),
        task_index: 0,
    };
    let mut mine = e.issuer.begin_unit(agent(AGENT_A), wf).unwrap();
    let theirs = other.begin_unit(agent(AGENT_B), wf).unwrap();
    assert!(!mine.same_issuer(&theirs));
    assert_eq!(
        mine.derive_member(&other, id(AGENT_B), 1).unwrap_err(),
        Unresolved::WrongIssuer
    );
    assert_eq!(
        mine.join_child(&theirs).unwrap_err(),
        Unresolved::WrongIssuer
    );
    let child = mine.derive_member(&e.issuer, id(AGENT_B), 1).unwrap();
    assert!(mine.same_issuer(&child));
    mine.join_child(&child).unwrap();
}

// ── N3: labelled writes (§6.1–§6.3, §6.7, §14) ──────────────────────────────

fn mem<'a>(key: &'a str, value_json: &'a str) -> NewMemory<'a> {
    NewMemory {
        source_kind: SourceKind::MemoryStoreAction,
        memory_type: "episodic",
        key,
        value_json,
        relevance_score: 0.5,
    }
}

/// `origin|source_kind|policy_generation|content_sha256` of a label row.
fn label_row(db: &NexusDatabase, table: &str, record_id: &str) -> String {
    strings(
        db,
        &format!(
            "SELECT origin || '|' || source_kind || '|' || policy_generation || '|' || content_sha256
             FROM hm_record_label WHERE record_table = '{table}' AND record_id = '{record_id}'"
        ),
    )
    .join(";")
}

fn origin_of(db: &NexusDatabase, table: &str, record_id: &str) -> String {
    label_row(db, table, record_id)
        .split('|')
        .next()
        .unwrap_or("")
        .to_string()
}

fn edges(db: &NexusDatabase) -> Vec<String> {
    strings(
        db,
        "SELECT child_table || ':' || child_id || '<-' || parent_table || ':' || parent_id
                || '=' || parent_label_at_derivation
         FROM hm_derivation ORDER BY child_table, child_id, parent_table, parent_id",
    )
}

#[test]
fn hm_m2_n3_write_is_labelled_from_the_context_and_binds() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let ca = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let cb = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();

    let ra = e
        .issuer
        .write_memory(&ca, &mem("SYNTH-a", "\"SYNTH-va\""), &[])
        .unwrap();
    assert_eq!(ra.label(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(ra.record_ref().table(), RecordTable::AgentMemory);
    let a_id = ra.record_ref().record_id().to_string();
    assert!(label_row(&e.db, "agent_memory", &a_id)
        .starts_with("LOCAL_ONLY_ORIGIN|memory_store_action|1|"));
    assert_eq!(
        evaluate_label(&e.db, RecordTable::AgentMemory, &a_id).unwrap(),
        EvaluatedLabel::Bound(OriginLabel::LocalOnlyOrigin)
    );

    let rb = e
        .issuer
        .write_memory(&cb, &mem("SYNTH-b", "\"SYNTH-vb\""), &[])
        .unwrap();
    let b_id = rb.record_ref().record_id().to_string();
    assert_eq!(rb.label(), OriginLabel::UnrestrictedOrigin);
    // Bookkeeping columns need no context and keep the binding (§6.7).
    must(
        &e.db,
        &format!(
            "UPDATE agent_memory SET relevance_score = 0.01, access_count = 7,
               last_accessed = '2030-01-01T00:00:00Z', expires_at = '2031-01-01T00:00:00Z'
             WHERE id = {b_id}"
        ),
    );
    assert_eq!(
        evaluate_label(&e.db, RecordTable::AgentMemory, &b_id).unwrap(),
        EvaluatedLabel::Bound(OriginLabel::UnrestrictedOrigin)
    );

    // An explicit record parent joins its evaluated label and gets an edge.
    let rc = e
        .issuer
        .write_memory(
            &cb,
            &mem("SYNTH-c", "\"SYNTH-vc\""),
            &[
                Parent::Record(ra.record_ref()),
                Parent::Record(rb.record_ref()),
            ],
        )
        .unwrap();
    assert_eq!(rc.label(), OriginLabel::LocalOnlyOrigin);
    let c_id = rc.record_ref().record_id().to_string();
    assert_eq!(
        edges(&e.db),
        vec![
            format!("agent_memory:{c_id}<-agent_memory:{a_id}=LOCAL_ONLY_ORIGIN"),
            format!("agent_memory:{c_id}<-agent_memory:{b_id}=UNRESTRICTED_ORIGIN"),
        ]
    );

    // A context from another issuer cannot write here.
    let other = e.issuer_on(e.second_handle());
    assert_eq!(
        other
            .write_memory(&ca, &mem("SYNTH-x", "1"), &[])
            .unwrap_err(),
        WriteRefused::WrongIssuer
    );

    // Not migrated (§9.4): refused, nothing written.
    must(&e.db, "DROP TABLE hm_migration;");
    let before = count(&e.db, "SELECT COUNT(*) FROM agent_memory");
    assert_eq!(
        e.issuer
            .write_memory(&cb, &mem("SYNTH-y", "1"), &[])
            .unwrap_err(),
        WriteRefused::NotMigrated
    );
    assert_eq!(count(&e.db, "SELECT COUNT(*) FROM agent_memory"), before);
}

#[test]
fn hm_m2_n3_a_n12_loosening_mid_unit_keeps_step2_local() {
    let e = env();
    let ctx = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&ctx), LO);
    let step1 = e
        .issuer
        .write_memory(&ctx, &mem("SYNTH-step1", "1"), &[])
        .unwrap();
    assert_eq!(step1.label(), OriginLabel::LocalOnlyOrigin);

    // The Owner loosens A between step 1 and step 2 (fixture).
    fixture_owner_loosen(&e.db, AGENT_A);
    let step2 = e
        .issuer
        .write_memory(&ctx, &mem("SYNTH-step2", "2"), &[])
        .unwrap();
    assert_eq!(step2.label(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(state(&ctx), LO, "step 2 stays local");
    assert_eq!(
        origin_of(&e.db, "agent_memory", step2.record_ref().record_id()),
        "LOCAL_ONLY_ORIGIN"
    );

    // Only a unit that starts after the loosening commit is unrestricted.
    let next = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&next), UN);
    assert_eq!(
        e.issuer
            .write_memory(&next, &mem("SYNTH-next", "3"), &[])
            .unwrap()
            .label(),
        OriginLabel::UnrestrictedOrigin
    );
}

#[test]
fn hm_m2_n3_a_d1_strategy_upsert_across_a_tightening_keeps_local_only() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_A);
    let s = |goal_type: &'static str| StrategyWrite {
        strategy_hash: "SYNTH-hash",
        goal_type,
        success: true,
        fuel: 1.0,
        duration_secs: 2.0,
    };

    let u1 = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let r1 = e.issuer.upsert_strategy(&u1, &s("SYNTH-t1"), &[]).unwrap();
    assert_eq!(r1.label(), OriginLabel::UnrestrictedOrigin);
    let sid = r1.record_ref().record_id().to_string();

    // Out-of-band tightening, observed at the next unit start.
    e.cfg.set(id(AGENT_A), true);
    let u2 = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&u2), LO);
    let r2 = e.issuer.upsert_strategy(&u2, &s("SYNTH-t2"), &[]).unwrap();
    assert_eq!(r2.record_ref().record_id(), sid, "same aggregate");
    assert_eq!(r2.label(), OriginLabel::LocalOnlyOrigin);

    // Loosened again (fixture); a later unrestricted contribution never
    // lowers the aggregate: the last writer does not decide.
    e.cfg.set(id(AGENT_A), false);
    fixture_owner_loosen(&e.db, AGENT_A);
    let u3 = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    assert_eq!(state(&u3), UN);
    let r3 = e.issuer.upsert_strategy(&u3, &s("SYNTH-t3"), &[]).unwrap();
    assert_eq!(r3.label(), OriginLabel::LocalOnlyOrigin);
    assert_eq!(
        origin_of(&e.db, "strategy_scores", &sid),
        "LOCAL_ONLY_ORIGIN"
    );
    // The label binds the overwritten goal_type; the counters aggregate.
    assert_eq!(
        evaluate_label(&e.db, RecordTable::StrategyScores, &sid).unwrap(),
        EvaluatedLabel::Bound(OriginLabel::LocalOnlyOrigin)
    );
    assert_eq!(
        strings(
            &e.db,
            &format!("SELECT goal_type || '|' || uses || '|' || successes FROM strategy_scores WHERE id = {sid}")
        ),
        vec!["SYNTH-t3|3|3"]
    );
}

const PRE_T0_TASK_A: &str = "11111111-0000-4000-8000-0000000000a1";
const PRE_T0_TASK_B: &str = "11111111-0000-4000-8000-0000000000b1";

#[test]
fn hm_m2_n3_a_d2_pre_t0_task_completed_after_t0() {
    let seed = format!(
        "INSERT INTO task_history (id, agent_id, goal, status, steps_json, result_json, fuel_consumed, started_at, completed_at, success) VALUES
           ('{PRE_T0_TASK_A}', '{AGENT_A}', 'SYNTH-goal-a', 'running', '[]', NULL, 0.0, '2026-01-01T00:00:00Z', NULL, 0),
           ('{PRE_T0_TASK_B}', '{AGENT_B}', 'SYNTH-goal-b', 'running', '[]', NULL, 0.0, '2026-01-01T00:00:00Z', NULL, 0);"
    );
    let e = env_seeded(
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D],
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D, AGENT_E],
        &seed,
    );
    fixture_owner_loosen(&e.db, AGENT_B);
    let ledger = |task: &str| {
        strings(
            &e.db,
            &format!(
                "SELECT t0_content_sha256 FROM hm_t0_member
                 WHERE record_table = 'task_history' AND record_id = '{task}'"
            ),
        )
    };
    let ledger_a = ledger(PRE_T0_TASK_A);
    let ledger_b = ledger(PRE_T0_TASK_B);
    assert_eq!(
        origin_of(&e.db, "task_history", PRE_T0_TASK_A),
        "LEGACY_UNKNOWN"
    );

    let complete = |task: &str| TaskWrite {
        task_id: id(task),
        goal: "ignored on update",
        status: "completed",
        steps_json: "[\"SYNTH-s\"]",
        result_json: Some("\"SYNTH-r\""),
        completed: true,
        success: true,
    };
    // A is local-only: LEGACY ⊔ LOCAL_ONLY.
    let ca = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let ra = e
        .issuer
        .save_task(&ca, &complete(PRE_T0_TASK_A), &[])
        .unwrap();
    assert_eq!(ra.label(), OriginLabel::LocalOnlyOrigin);
    // B is unrestricted: the join keeps LEGACY_UNKNOWN.
    let cb = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let rb = e
        .issuer
        .save_task(&cb, &complete(PRE_T0_TASK_B), &[])
        .unwrap();
    assert_eq!(rb.label(), OriginLabel::LegacyUnknown);

    for (task, label, ledger_before) in [
        (PRE_T0_TASK_A, OriginLabel::LocalOnlyOrigin, ledger_a),
        (PRE_T0_TASK_B, OriginLabel::LegacyUnknown, ledger_b),
    ] {
        // The label binds the new digest; the ledger row is unchanged.
        let row = label_row(&e.db, "task_history", task);
        assert!(
            row.starts_with(&format!("{}|legacy|0|", label.class_name())),
            "{row}"
        );
        assert_ne!(row.rsplit('|').next().unwrap(), ledger_before[0]);
        assert_eq!(ledger(task), ledger_before);
        assert_eq!(
            evaluate_label(&e.db, RecordTable::TaskHistory, task).unwrap(),
            EvaluatedLabel::Bound(label)
        );
    }
    let StartStatus::Verified(report) = verify_at_start(&e.db).unwrap() else {
        panic!("migrated")
    };
    assert!(report.checks.all_pass());
    for (task, label) in [
        (PRE_T0_TASK_A, OriginLabel::LocalOnlyOrigin),
        (PRE_T0_TASK_B, OriginLabel::LegacyUnknown),
    ] {
        let m = report
            .members
            .iter()
            .find(|m| m.table == RecordTable::TaskHistory && m.record_id == task)
            .unwrap();
        assert_eq!(
            m.state,
            MemberState::Present {
                label: EvaluatedLabel::Bound(label),
                unchanged_since_t0: false
            }
        );
    }
    // Another agent's task cannot be updated through this context.
    assert_eq!(
        e.issuer
            .save_task(&ca, &complete(PRE_T0_TASK_B), &[])
            .unwrap_err(),
        WriteRefused::WrongAgent
    );
}

#[test]
fn hm_m2_n3_a_n10_derived_records_inherit_their_parents() {
    let e = env();
    // A local-only goal writes its task record.
    let goal_ctx = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let task = e
        .issuer
        .save_task(
            &goal_ctx,
            &TaskWrite {
                task_id: Uuid::new_v4(),
                goal: "SYNTH-goal",
                status: "completed",
                steps_json: "[]",
                result_json: Some("\"SYNTH-result\""),
                completed: true,
                success: true,
            },
            &[],
        )
        .unwrap();
    assert_eq!(task.label(), OriginLabel::LocalOnlyOrigin);

    // A is loosened; a later unrestricted post-goal evolution derives a
    // strategy row and a procedural memory from that task.
    fixture_owner_loosen(&e.db, AGENT_A);
    let later = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let evo = later.derive_post_goal().unwrap();
    assert_eq!(state(&evo), UN);
    let parent = [Parent::Record(task.record_ref())];
    let strat = e
        .issuer
        .upsert_strategy(
            &evo,
            &StrategyWrite {
                strategy_hash: "SYNTH-derived-hash",
                goal_type: "SYNTH-type",
                success: true,
                fuel: 0.5,
                duration_secs: 0.5,
            },
            &parent,
        )
        .unwrap();
    assert_eq!(strat.label(), OriginLabel::LocalOnlyOrigin);
    let proc_mem = e
        .issuer
        .write_memory(
            &evo,
            &NewMemory {
                source_kind: SourceKind::EvolutionProcedural,
                memory_type: "procedural",
                key: "SYNTH-proc",
                value_json: "\"SYNTH-p\"",
                relevance_score: 0.9,
            },
            &parent,
        )
        .unwrap();
    assert_eq!(proc_mem.label(), OriginLabel::LocalOnlyOrigin);
    let t = task.record_ref().record_id();
    assert_eq!(
        edges(&e.db),
        vec![
            format!(
                "agent_memory:{}<-task_history:{t}=LOCAL_ONLY_ORIGIN",
                proc_mem.record_ref().record_id()
            ),
            format!(
                "strategy_scores:{}<-task_history:{t}=LOCAL_ONLY_ORIGIN",
                strat.record_ref().record_id()
            ),
        ]
    );
    assert!(
        label_row(&e.db, "agent_memory", proc_mem.record_ref().record_id())
            .starts_with("LOCAL_ONLY_ORIGIN|evolution_procedural|")
    );
    // An unrestricted parent keeps an unrestricted child unrestricted.
    let open_parent = e
        .issuer
        .write_memory(&evo, &mem("SYNTH-open", "1"), &[])
        .unwrap();
    let child = e
        .issuer
        .write_memory(
            &evo,
            &mem("SYNTH-open-child", "2"),
            &[Parent::Record(open_parent.record_ref())],
        )
        .unwrap();
    assert_eq!(child.label(), OriginLabel::UnrestrictedOrigin);
}

#[test]
fn hm_m2_n3_a_n5_cross_agent_copy_refused_or_labelled() {
    let e = env();
    // B and C unrestricted (fixture); A and D local-only.
    fixture_owner_loosen(&e.db, AGENT_B);
    fixture_owner_loosen(&e.db, AGENT_C);
    let ca = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let cb = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let restricted = e
        .issuer
        .write_memory(&ca, &mem("SYNTH-learned-a", "\"SYNTH-la\""), &[])
        .unwrap();
    let open = e
        .issuer
        .write_memory(&cb, &mem("SYNTH-learned-b", "\"SYNTH-lb\""), &[])
        .unwrap();
    let rows = || count(&e.db, "SELECT COUNT(*) FROM agent_memory");
    let n = rows();

    // Restricted source → unrestricted target: refused, nothing written.
    let target_c = e.issuer.begin_unit(agent(AGENT_C), goal()).unwrap();
    assert_eq!(
        e.issuer
            .copy_to_agent(&target_c, restricted.record_ref())
            .unwrap_err(),
        WriteRefused::CrossAgentRestricted
    );
    assert_eq!(rows(), n);
    assert!(edges(&e.db).is_empty());

    // Restricted source → local-only target: labelled LOCAL_ONLY, edge.
    let target_d = e.issuer.begin_unit(agent(AGENT_D), goal()).unwrap();
    let copy_d = e
        .issuer
        .copy_to_agent(&target_d, restricted.record_ref())
        .unwrap();
    assert_eq!(copy_d.label(), OriginLabel::LocalOnlyOrigin);
    assert!(
        label_row(&e.db, "agent_memory", copy_d.record_ref().record_id())
            .starts_with("LOCAL_ONLY_ORIGIN|cross_agent_learning|")
    );

    // Unrestricted source → unrestricted target: allowed, unrestricted.
    let copy_c = e
        .issuer
        .copy_to_agent(&target_c, open.record_ref())
        .unwrap();
    assert_eq!(copy_c.label(), OriginLabel::UnrestrictedOrigin);
    assert_eq!(
        strings(
            &e.db,
            &format!(
                "SELECT agent_id || '|' || key FROM agent_memory WHERE id = {}",
                copy_c.record_ref().record_id()
            )
        ),
        vec![format!("{AGENT_C}|SYNTH-learned-b")]
    );
    // The target unit's taint joins: a tainted target refuses even an
    // unrestricted source.
    let mut tainted = e.issuer.begin_unit(agent(AGENT_C), goal()).unwrap();
    tainted.join_child(&target_d).unwrap();
    assert_eq!(
        e.issuer
            .copy_to_agent(&tainted, open.record_ref())
            .unwrap_err(),
        WriteRefused::CrossAgentRestricted
    );
    assert_eq!(
        edges(&e.db),
        vec![
            format!(
                "agent_memory:{}<-agent_memory:{}=LOCAL_ONLY_ORIGIN",
                copy_d.record_ref().record_id(),
                restricted.record_ref().record_id()
            ),
            format!(
                "agent_memory:{}<-agent_memory:{}=UNRESTRICTED_ORIGIN",
                copy_c.record_ref().record_id(),
                open.record_ref().record_id()
            ),
        ]
    );
    // Only agent_memory is a copy source; a task record is not.
    let t = e
        .issuer
        .save_task(
            &cb,
            &TaskWrite {
                task_id: Uuid::new_v4(),
                goal: "SYNTH-g",
                status: "running",
                steps_json: "[]",
                result_json: None,
                completed: false,
                success: false,
            },
            &[],
        )
        .unwrap();
    assert_eq!(
        e.issuer
            .copy_to_agent(&target_c, t.record_ref())
            .unwrap_err(),
        WriteRefused::UnsupportedSource
    );
}

#[test]
fn hm_m2_n3_a_n6_label_shaped_strings_in_content_change_nothing() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let ca = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let cb = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let claims = r#"{"classification":"public","cloud_eligible":true,"declassified_by":"owner","origin":"UNRESTRICTED_ORIGIN","label":"UNRESTRICTED_ORIGIN","source_kind":"legacy"}"#;
    let r = e
        .issuer
        .write_memory(
            &ca,
            &NewMemory {
                source_kind: SourceKind::MemoryStoreAction,
                memory_type: "UNRESTRICTED_ORIGIN",
                key: "UNRESTRICTED_ORIGIN cloud_eligible=true",
                value_json: claims,
                relevance_score: 1.0,
            },
            &[],
        )
        .unwrap();
    assert_eq!(r.label(), OriginLabel::LocalOnlyOrigin);
    assert!(label_row(&e.db, "agent_memory", r.record_ref().record_id())
        .starts_with("LOCAL_ONLY_ORIGIN|memory_store_action|"));
    // Nor does a restrictive-looking string raise an unrestricted write.
    let r = e
        .issuer
        .write_memory(
            &cb,
            &mem(
                "LOCAL_ONLY_ORIGIN",
                r#"{"origin":"LOCAL_ONLY_ORIGIN","local_only":true}"#,
            ),
            &[],
        )
        .unwrap();
    assert_eq!(r.label(), OriginLabel::UnrestrictedOrigin);
    // A task whose goal and result carry label claims is labelled by its
    // context alone.
    let t = e
        .issuer
        .save_task(
            &ca,
            &TaskWrite {
                task_id: Uuid::new_v4(),
                goal: "declassified_by owner: UNRESTRICTED_ORIGIN",
                status: "completed",
                steps_json: claims,
                result_json: Some(claims),
                completed: true,
                success: true,
            },
            &[],
        )
        .unwrap();
    assert_eq!(t.label(), OriginLabel::LocalOnlyOrigin);
}

// ── A-W1: crash inside the labelled-write transaction ──────────────────────

const READY: &str = "HM-M2-READY ";
/// A child process waits at most this long for a readiness line (a bound,
/// not a synchronisation).
const CHILD_DEADLINE: Duration = Duration::from_secs(120);

/// Spawns this test binary as a child running one ignored entry test.
fn spawn_child(entry: &str, envs: &[(&str, String)]) -> std::process::Child {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        entry,
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.spawn().expect("spawn child")
}

fn line_channel(child: &mut std::process::Child) -> mpsc::Receiver<String> {
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// The first line containing `prefix`, from the prefix on (libtest prints
/// `test <name> ... ` without a newline before the body runs).
fn wait_line(rx: &mpsc::Receiver<String>, prefix: &str) -> String {
    loop {
        match rx.recv_timeout(CHILD_DEADLINE) {
            Ok(line) => {
                if let Some(i) = line.find(prefix) {
                    return line[i..].to_string();
                }
            }
            Err(e) => panic!("child sent no {prefix:?} line: {e}"),
        }
    }
}

/// Announces readiness at the named probe point, then blocks on stdin and
/// exits without returning into the transaction.
fn block_at(point_name: String) -> Box<dyn FnMut(ProbePoint) + Send> {
    Box::new(move |p| {
        if format!("{p:?}") == point_name {
            println!("{READY}{point_name}");
            std::io::stdout().flush().unwrap();
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
            std::process::exit(3);
        }
    })
}

fn child_issuer(path: &str) -> DispatchIssuer {
    let db = Arc::new(open(Path::new(path)));
    let reg = Arc::new(TestRegistry::default());
    reg.add(id(AGENT_A));
    DispatchIssuer::new(db, reg, Arc::new(TestConfig::default())).unwrap()
}

#[test]
#[ignore = "child process entry for A-W1; run by the parent test"]
fn hm_m2_child_write_kill_point() {
    let Ok(path) = std::env::var("HM_M2_CHILD_DB") else {
        return;
    };
    let point = std::env::var("HM_M2_CHILD_POINT").unwrap();
    let issuer = child_issuer(&path);
    let ctx = issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let parent = issuer
        .write_memory(&ctx, &mem("SYNTH-W1-parent", "\"p\""), &[])
        .unwrap();
    issuer.install_probe(block_at(point));
    let _ = issuer.write_memory(
        &ctx,
        &mem("SYNTH-W1-child", "\"c\""),
        &[Parent::Record(parent.record_ref())],
    );
    println!("HM-M2-UNEXPECTED-COMMIT");
    std::process::exit(4);
}

#[test]
fn hm_m2_n3_a_w1_child_killed_inside_write_leaves_neither_row_nor_label() {
    for point in [
        ProbePoint::WriteRecordWritten,
        ProbePoint::WriteLabelWritten,
        ProbePoint::WriteEdgesWritten,
    ] {
        let name = format!("{point:?}");
        let e = env();
        let mut child = spawn_child(
            "hm_m2_child_write_kill_point",
            &[
                ("HM_M2_CHILD_DB", e.path.display().to_string()),
                ("HM_M2_CHILD_POINT", name.clone()),
            ],
        );
        let rx = line_channel(&mut child);
        assert_eq!(wait_line(&rx, READY), format!("{READY}{name}"));
        child.kill().expect("kill child");
        assert!(!child.wait().unwrap().success(), "{name}: killed");

        let db = open(&e.path);
        let n = |sql: &str| count(&db, sql);
        // The earlier write committed; the killed one left nothing.
        assert_eq!(
            n("SELECT COUNT(*) FROM agent_memory WHERE key = 'SYNTH-W1-parent'"),
            1
        );
        assert_eq!(
            n("SELECT COUNT(*) FROM agent_memory WHERE key = 'SYNTH-W1-child'"),
            0,
            "{name}"
        );
        assert_eq!(
            n("SELECT COUNT(*) FROM hm_record_label WHERE record_table = 'agent_memory'"),
            1,
            "{name}"
        );
        assert_eq!(n("SELECT COUNT(*) FROM hm_derivation"), 0, "{name}");
        assert_eq!(
            n("SELECT COUNT(*) FROM agent_memory m WHERE NOT EXISTS (
                 SELECT 1 FROM hm_record_label l
                 WHERE l.record_table = 'agent_memory' AND l.record_id = CAST(m.id AS TEXT))"),
            0,
            "{name}: no row without a label"
        );

        // Restart: the same write commits row, label and edge together.
        let issuer = child_issuer(&e.path.display().to_string());
        let ctx = issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
        let p = issuer
            .write_memory(&ctx, &mem("SYNTH-W1-p2", "1"), &[])
            .unwrap();
        let c = issuer
            .write_memory(
                &ctx,
                &mem("SYNTH-W1-c2", "2"),
                &[Parent::Record(p.record_ref())],
            )
            .unwrap();
        assert_eq!(c.label(), OriginLabel::LocalOnlyOrigin);
        assert_eq!(n("SELECT COUNT(*) FROM hm_derivation"), 1);
        assert_eq!(
            n("SELECT COUNT(*) FROM hm_record_label WHERE record_table = 'agent_memory'"),
            3
        );
    }
}

// ── N4: route-first recall (§12.3) ──────────────────────────────────────────

fn memories(limit: usize) -> RecallQuery {
    RecallQuery {
        table: RecordTable::AgentMemory,
        limit,
    }
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

/// The `agent_memory.key` of each recalled record, in recall order.
fn recalled_keys(db: &NexusDatabase, recs: &[RecalledRecord]) -> Vec<String> {
    recs.iter()
        .map(|r| {
            assert_eq!(r.record_ref().table(), RecordTable::AgentMemory);
            strings(
                db,
                &format!(
                    "SELECT key FROM agent_memory WHERE id = {}",
                    r.record_ref().record_id()
                ),
            )
            .remove(0)
        })
        .collect()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// The fields of the events of `kind`, parsed.
fn event_fields(db: &NexusDatabase, kind: &str) -> Vec<serde_json::Value> {
    events_of(db, kind)
        .into_iter()
        .map(|(subject, _, fields)| {
            assert_obs_subject(&subject, kind);
            serde_json::from_str(&fields).unwrap()
        })
        .collect()
}

/// An older binary's write: a row with no label.
fn insert_unlabelled(db: &NexusDatabase, agent_id: &str, key: &str, relevance: f64) {
    must(
        db,
        &format!(
            "INSERT INTO agent_memory (agent_id, memory_type, key, value_json, relevance_score,
               access_count, created_at, last_accessed)
             VALUES ('{agent_id}', 'episodic', '{key}', '\"SYNTH\"', {relevance}, 0,
               '2026-06-01T00:00:00Z', '2026-06-01T00:00:00Z')"
        ),
    );
}

/// Content changed out of band: the label no longer binds.
fn tamper(db: &NexusDatabase, key: &str) {
    must(
        db,
        &format!("UPDATE agent_memory SET value_json = '\"SYNTH-tampered\"' WHERE key = '{key}'"),
    );
}

fn seed_legacy_memories(agent_id: &str, keys: &[(&str, f64)]) -> String {
    keys.iter()
        .map(|(k, r)| {
            format!(
                "INSERT INTO agent_memory (agent_id, memory_type, key, value_json, relevance_score,
                   access_count, created_at, last_accessed)
                 VALUES ('{agent_id}', 'semantic', '{k}', '\"SYNTH\"', {r}, 0,
                   '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');"
            )
        })
        .collect()
}

/// A unit of `who` whose route class is local through its taint.
fn local_unit(e: &Env, who: &str) -> DispatchContext {
    let mut ctx = e.issuer.begin_unit(agent(who), goal()).unwrap();
    let restricted = e.issuer.begin_unit(agent(AGENT_D), goal()).unwrap();
    ctx.join_child(&restricted).unwrap();
    assert_eq!(ctx.route_class(), RouteClass::Local);
    ctx
}

#[test]
fn hm_m2_n4_a_n1_non_local_recall_excludes_legacy_local_only_and_unbound() {
    let seed = seed_legacy_memories(AGENT_B, &[("SYNTH-legacy-1", 0.9), ("SYNTH-legacy-2", 0.8)])
        + &format!(
            "INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at)
               VALUES ('{PRE_T0_TASK_B}', '{AGENT_B}', 'SYNTH-goal', 'running', '[]', '2026-01-01T00:00:00Z');
             INSERT INTO world_model_entities (agent_id, entity_json, created_at)
               VALUES ('{AGENT_B}', '{{\"SYNTH\":1}}', '2026-01-01T00:00:00Z');"
        );
    let e = env_seeded(
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D],
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D, AGENT_E],
        &seed,
    );
    // B local-only: two LOCAL_ONLY records.
    let lo = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for k in ["SYNTH-lo-1", "SYNTH-lo-2"] {
        e.issuer.write_memory(&lo, &mem_r(k, 0.7), &[]).unwrap();
    }
    // B loosened: three unrestricted records, one later tampered.
    fixture_owner_loosen(&e.db, AGENT_B);
    let un = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for k in ["SYNTH-u-1", "SYNTH-u-2", "SYNTH-u-3", "SYNTH-u-altered"] {
        e.issuer.write_memory(&un, &mem_r(k, 0.5), &[]).unwrap();
    }
    tamper(&e.db, "SYNTH-u-altered");
    insert_unlabelled(&e.db, AGENT_B, "SYNTH-unlabelled", 0.6);
    // Another agent's unrestricted record is never in B's recall.
    fixture_owner_loosen(&e.db, AGENT_C);
    let other = e.issuer.begin_unit(agent(AGENT_C), goal()).unwrap();
    e.issuer
        .write_memory(&other, &mem_r("SYNTH-other-agent", 0.99), &[])
        .unwrap();

    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(ctx.route_class(), RouteClass::NonLocal);
    let recs = e.issuer.recall(&mut ctx, memories(64)).unwrap();
    assert_eq!(
        sorted(recalled_keys(&e.db, &recs)),
        vec!["SYNTH-u-1", "SYNTH-u-2", "SYNTH-u-3"]
    );
    for r in &recs {
        assert_eq!(
            r.label(),
            EvaluatedLabel::Bound(OriginLabel::UnrestrictedOrigin)
        );
        assert_eq!(r.agent_id(), AGENT_B);
        assert_eq!(r.source_kind(), "loop_episodic");
    }
    assert_eq!(ctx.working_set().verbatim().count(), 3);
    for table in [RecordTable::TaskHistory, RecordTable::WorldModelEntities] {
        assert!(e
            .issuer
            .recall(&mut ctx, RecallQuery { table, limit: 64 })
            .unwrap()
            .is_empty());
    }
    let filtered = event_fields(&e.db, "memory.recall_filtered");
    assert_eq!(
        filtered[0],
        serde_json::json!({"route_class":"non_local","eligible_count":3,"withheld_count":6})
    );
    let unlabelled = event_fields(&e.db, "memory.unlabelled_record_observed");
    assert_eq!(
        unlabelled,
        vec![serde_json::json!({"route_class":"non_local","unbound_count":2})]
    );

    // A local unit sees every label; UNBOUND rows evaluate as such.
    let mut local = local_unit(&e, AGENT_B);
    let all = e.issuer.recall(&mut local, memories(64)).unwrap();
    let mut by_key: Vec<(String, EvaluatedLabel)> = recalled_keys(&e.db, &all)
        .into_iter()
        .zip(all.iter().map(|r| r.label()))
        .collect();
    by_key.sort_by(|a, b| a.0.cmp(&b.0));
    use EvaluatedLabel::{Bound, Unbound};
    use OriginLabel::*;
    assert_eq!(
        by_key,
        vec![
            ("SYNTH-legacy-1".into(), Bound(LegacyUnknown)),
            ("SYNTH-legacy-2".into(), Bound(LegacyUnknown)),
            ("SYNTH-lo-1".into(), Bound(LocalOnlyOrigin)),
            ("SYNTH-lo-2".into(), Bound(LocalOnlyOrigin)),
            ("SYNTH-u-1".into(), Bound(UnrestrictedOrigin)),
            ("SYNTH-u-2".into(), Bound(UnrestrictedOrigin)),
            ("SYNTH-u-3".into(), Bound(UnrestrictedOrigin)),
            ("SYNTH-u-altered".into(), Unbound),
            ("SYNTH-unlabelled".into(), Unbound),
        ]
    );
    assert_eq!(
        event_fields(&e.db, "memory.unlabelled_record_observed")[1],
        serde_json::json!({"route_class":"local","unbound_count":2})
    );
    assert_eq!(event_fields(&e.db, "memory.recall_filtered").len(), 3);
}

#[test]
fn hm_m2_n4_withheld_rows_never_displace_eligible_rows() {
    let seed = seed_legacy_memories(
        AGENT_B,
        &[
            ("SYNTH-legacy-1", 0.97),
            ("SYNTH-legacy-2", 0.97),
            ("SYNTH-legacy-3", 0.97),
        ],
    );
    let e = env_seeded(
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D],
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D, AGENT_E],
        &seed,
    );
    let lo = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for i in 0..5 {
        e.issuer
            .write_memory(&lo, &mem_r(&format!("SYNTH-lo-{i}"), 0.98), &[])
            .unwrap();
    }
    fixture_owner_loosen(&e.db, AGENT_B);
    let un = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    // Unrestricted-labelled, most relevant, then tampered: they pass the
    // SQL filter and fail the binding while the cursor is walked.
    for i in 0..5 {
        let k = format!("SYNTH-altered-{i}");
        e.issuer.write_memory(&un, &mem_r(&k, 0.99), &[]).unwrap();
        tamper(&e.db, &k);
    }
    insert_unlabelled(&e.db, AGENT_B, "SYNTH-unlabelled-1", 0.995);
    insert_unlabelled(&e.db, AGENT_B, "SYNTH-unlabelled-2", 0.995);
    for (k, r) in [
        ("SYNTH-eligible-1", 0.1),
        ("SYNTH-eligible-2", 0.2),
        ("SYNTH-eligible-3", 0.3),
        ("SYNTH-eligible-4", 0.4),
    ] {
        e.issuer.write_memory(&un, &mem_r(k, r), &[]).unwrap();
    }

    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let top3 = e.issuer.recall(&mut ctx, memories(3)).unwrap();
    assert_eq!(
        recalled_keys(&e.db, &top3),
        vec!["SYNTH-eligible-4", "SYNTH-eligible-3", "SYNTH-eligible-2"]
    );
    assert_eq!(
        event_fields(&e.db, "memory.recall_filtered"),
        vec![serde_json::json!({"route_class":"non_local","eligible_count":3,"withheld_count":15})]
    );
    let all = e.issuer.recall(&mut ctx, memories(10)).unwrap();
    assert_eq!(
        recalled_keys(&e.db, &all),
        vec![
            "SYNTH-eligible-4",
            "SYNTH-eligible-3",
            "SYNTH-eligible-2",
            "SYNTH-eligible-1"
        ]
    );
}

#[test]
fn hm_m2_n4_a_c2_deleted_or_mismatched_label_is_unbound() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let un = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let r1 = e
        .issuer
        .write_memory(&un, &mem_r("SYNTH-label-deleted", 0.9), &[])
        .unwrap();
    let r2 = e
        .issuer
        .write_memory(&un, &mem_r("SYNTH-digest-altered", 0.8), &[])
        .unwrap();
    e.issuer
        .write_memory(&un, &mem_r("SYNTH-intact", 0.7), &[])
        .unwrap();
    let id1 = r1.record_ref().record_id().to_string();
    let id2 = r2.record_ref().record_id().to_string();
    must(
        &e.db,
        &format!(
            "DELETE FROM hm_record_label WHERE record_table = 'agent_memory' AND record_id = '{id1}'"
        ),
    );
    tamper(&e.db, "SYNTH-digest-altered");
    for i in [&id1, &id2] {
        assert_eq!(
            evaluate_label(&e.db, RecordTable::AgentMemory, i).unwrap(),
            EvaluatedLabel::Unbound
        );
    }

    // Never eligible non-locally.
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let recs = e.issuer.recall(&mut ctx, memories(64)).unwrap();
    assert_eq!(recalled_keys(&e.db, &recs), vec!["SYNTH-intact"]);
    assert_eq!(
        event_fields(&e.db, "memory.unlabelled_record_observed"),
        vec![serde_json::json!({"route_class":"non_local","unbound_count":2})]
    );

    // Still visible locally, as UNBOUND (= LOCAL_ONLY); observed.
    let mut local = local_unit(&e, AGENT_B);
    let all = e.issuer.recall(&mut local, memories(64)).unwrap();
    let labels: Vec<(String, EvaluatedLabel)> = recalled_keys(&e.db, &all)
        .into_iter()
        .zip(all.iter().map(|r| r.label()))
        .collect();
    assert_eq!(
        labels,
        vec![
            ("SYNTH-label-deleted".to_string(), EvaluatedLabel::Unbound),
            ("SYNTH-digest-altered".to_string(), EvaluatedLabel::Unbound),
            (
                "SYNTH-intact".to_string(),
                EvaluatedLabel::Bound(OriginLabel::UnrestrictedOrigin)
            ),
        ]
    );
    assert_eq!(all[0].source_kind(), "unlabelled");
    assert_eq!(
        event_fields(&e.db, "memory.unlabelled_record_observed")[1],
        serde_json::json!({"route_class":"local","unbound_count":2})
    );
    // A write in the unit that holds them is LOCAL_ONLY: UNBOUND verbatim
    // items evaluate as LOCAL_ONLY_ORIGIN.
    let mut holder = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(holder.route_class(), RouteClass::NonLocal);
    let intact = e.issuer.recall(&mut holder, memories(64)).unwrap();
    assert_eq!(intact.len(), 1);
    let w = e
        .issuer
        .write_memory(&holder, &mem_r("SYNTH-after", 0.1), &[])
        .unwrap();
    assert_eq!(w.label(), OriginLabel::UnrestrictedOrigin);
    let w = e
        .issuer
        .write_memory(&local, &mem_r("SYNTH-after-local", 0.1), &[])
        .unwrap();
    assert_eq!(w.label(), OriginLabel::LocalOnlyOrigin);
}

#[test]
fn hm_m2_n4_recall_scope_limits_refusals_and_write_reevaluation() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let un = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for i in 0..(MAX_RECORDS + 6) {
        e.issuer
            .write_memory(&un, &mem_r(&format!("SYNTH-many-{i}"), 0.5), &[])
            .unwrap();
    }
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(
        e.issuer.recall(&mut ctx, memories(10_000)).unwrap().len(),
        MAX_RECORDS
    );
    assert!(e.issuer.recall(&mut ctx, memories(0)).unwrap().is_empty());

    // A chat turn carries no retained data; another issuer's context is
    // refused.
    let mut chat = e
        .issuer
        .begin_unit(
            agent(AGENT_B),
            Unit::ChatTurn {
                turn_id: Uuid::new_v4(),
            },
        )
        .unwrap();
    assert_eq!(
        e.issuer.recall(&mut chat, memories(5)).unwrap_err(),
        RecallRefused::RetainedDataNotPermitted
    );
    let other = e.issuer_on(e.second_handle());
    assert_eq!(
        other.recall(&mut ctx, memories(5)).unwrap_err(),
        RecallRefused::WrongIssuer
    );

    // A recalled verbatim item is re-evaluated at every write: its label
    // raised elsewhere raises the holder's next write.
    let s = |goal_type: &'static str| StrategyWrite {
        strategy_hash: "SYNTH-shared",
        goal_type,
        success: true,
        fuel: 1.0,
        duration_secs: 1.0,
    };
    e.issuer.upsert_strategy(&un, &s("SYNTH-a"), &[]).unwrap();
    let mut holder = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let got = e
        .issuer
        .recall(
            &mut holder,
            RecallQuery {
                table: RecordTable::StrategyScores,
                limit: 5,
            },
        )
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(
        e.issuer
            .write_memory(&holder, &mem_r("SYNTH-before", 0.1), &[])
            .unwrap()
            .label(),
        OriginLabel::UnrestrictedOrigin
    );
    e.cfg.set(id(AGENT_B), true);
    let tightened = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    e.issuer
        .upsert_strategy(&tightened, &s("SYNTH-b"), &[])
        .unwrap();
    assert_eq!(
        e.issuer
            .write_memory(&holder, &mem_r("SYNTH-after", 0.1), &[])
            .unwrap()
            .label(),
        OriginLabel::LocalOnlyOrigin
    );

    // Not migrated: refused.
    must(&e.db, "DROP TABLE hm_migration;");
    assert_eq!(
        e.issuer.recall(&mut holder, memories(5)).unwrap_err(),
        RecallRefused::NotMigrated
    );
}

// ── N5: admission and reply acceptance (§12.4–§12.7) ────────────────────────

fn attest(route: RouteClass) -> RouteAttestation {
    RouteAttestation::from_transport_evidence(match route {
        RouteClass::Local => TransportEvidence::VerifiedLoopbackOllama,
        RouteClass::NonLocal => TransportEvidence::Unattested,
    })
}

/// The transport a dispatch selects, given the unit's route class.
fn follow(route: RouteClass) -> RouteClass {
    route
}

fn non_local(_: RouteClass) -> RouteClass {
    RouteClass::NonLocal
}

type Build<'a> =
    Box<dyn FnMut(&mut DispatchContext) -> Option<(PromptEnvelope, RouteAttestation)> + 'a>;

/// Builds a prompt from fixed text and a fresh recall of `recall` memories,
/// for the transport `transport` selects.
fn build_with(
    issuer: &DispatchIssuer,
    recall: usize,
    transport: fn(RouteClass) -> RouteClass,
) -> Build<'_> {
    Box::new(move |ctx| {
        let recs = if recall > 0 {
            issuer.recall(ctx, memories(recall)).ok()?
        } else {
            Vec::new()
        };
        let mut b = PromptBuilder::new(ctx);
        b.text("SYNTH-instruction\n");
        for r in &recs {
            b.include(r);
        }
        Some((b.build(), attest(transport(ctx.route_class()))))
    })
}

/// `route_class|outcome|raw_label|policy_generation|record_count|grant_dep_count`.
fn admission_rows(db: &NexusDatabase) -> Vec<String> {
    strings(
        db,
        "SELECT route_class || '|' || outcome || '|' || raw_label || '|' || policy_generation
                || '|' || record_count || '|' || grant_dep_count
         FROM hm_dispatch_admission ORDER BY rowid",
    )
}

fn outcome_of(db: &NexusDatabase, admission_id: &str) -> String {
    strings(
        db,
        &format!("SELECT outcome FROM hm_dispatch_admission WHERE admission_id = '{admission_id}'"),
    )
    .join(";")
}

fn non_local_admissions(db: &NexusDatabase) -> i64 {
    count(
        db,
        "SELECT COUNT(*) FROM hm_dispatch_admission WHERE route_class = 'non_local'",
    )
}

/// An unrestricted agent B with two unrestricted memories.
fn env_open_b() -> Env {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let w = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for k in ["SYNTH-open-1", "SYNTH-open-2"] {
        e.issuer.write_memory(&w, &mem_r(k, 0.5), &[]).unwrap();
    }
    e
}

#[test]
fn hm_m2_n5_admission_records_row_and_event_and_releases_the_prompt() {
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
        .unwrap();
    assert_eq!(a.route_class(), RouteClass::NonLocal);
    assert_eq!(a.raw_label(), OriginLabel::UnrestrictedOrigin);
    assert_eq!(a.admission_id().len(), 32);
    assert!(a.prompt().starts_with("SYNTH-instruction\n"));
    assert!(a.prompt().contains("key: SYNTH-open-1") && a.prompt().contains("key: SYNTH-open-2"));
    assert_eq!(
        admission_rows(&e.db),
        vec!["non_local|admitted|UNRESTRICTED_ORIGIN|2|2|0"]
    );
    let ev = events_of(&e.db, "memory.dispatch_admitted");
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].0, format!("admission:{}", a.admission_id()));
    assert_eq!(ev[0].1.as_deref(), Some(AGENT_B));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&ev[0].2).unwrap(),
        serde_json::json!({"route_class":"non_local","record_count":2,"grant_dep_count":0,
            "raw_label":"UNRESTRICTED_ORIGIN","policy_generation":2,"revocation_seq":0})
    );

    // Reply used: a derived item labelled with the raw label; it enters the
    // working set and can be carried into the next non-local prompt.
    let id_a = a.admission_id().to_string();
    let reply = e
        .issuer
        .accept_reply(&mut ctx, a, Ok("SYNTH-reply".into()))
        .unwrap();
    assert_eq!(reply.label(), OriginLabel::UnrestrictedOrigin);
    assert_eq!(reply.text(), "SYNTH-reply");
    assert_eq!(outcome_of(&e.db, &id_a), "reply_used");
    assert_eq!(
        ctx.working_set().derived_labels(),
        &[OriginLabel::UnrestrictedOrigin]
    );
    let mut carry = |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        b.carry(&reply);
        Some((b.build(), attest(RouteClass::NonLocal)))
    };
    let next = e.issuer.admit(&mut ctx, &mut carry).unwrap();
    assert_eq!(next.route_class(), RouteClass::NonLocal);

    // A transport error: recorded as possibly disclosed, with its event.
    let id_n = next.admission_id().to_string();
    assert_eq!(
        e.issuer
            .accept_reply(&mut ctx, next, Err(TransportFailure))
            .unwrap_err(),
        ReplyRefused::TransportFailed
    );
    assert_eq!(outcome_of(&e.db, &id_n), "transport_failed");
    assert_eq!(
        events_of(&e.db, "memory.transport_failed")[0].0,
        format!("admission:{id_n}:outcome")
    );

    // A local-only agent's prompt with its own records: a local admission
    // whose reply is LOCAL_ONLY, and the unit stays restricted.
    let mut la = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    e.issuer
        .write_memory(&la, &mem_r("SYNTH-a-own", 0.5), &[])
        .unwrap();
    let a = e
        .issuer
        .admit(&mut la, &mut *build_with(&e.issuer, 10, follow))
        .unwrap();
    assert_eq!(a.route_class(), RouteClass::Local);
    assert_eq!(a.raw_label(), OriginLabel::LocalOnlyOrigin);
    let lo_reply = e
        .issuer
        .accept_reply(&mut la, a, Ok("SYNTH-local-reply".into()))
        .unwrap();
    assert_eq!(lo_reply.label(), OriginLabel::LocalOnlyOrigin);
    // A derived parent joins its fixed label into a write elsewhere.
    let w = e
        .issuer
        .write_memory(
            &ctx,
            &mem_r("SYNTH-from-local-reply", 0.1),
            &[Parent::Derived(&lo_reply)],
        )
        .unwrap();
    assert_eq!(w.label(), OriginLabel::LocalOnlyOrigin);
    // ... and carrying it into a non-local prompt is ineligible (§2.13).
    let mut carry_lo = |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        b.carry(&lo_reply);
        Some((b.build(), attest(RouteClass::NonLocal)))
    };
    assert_eq!(
        e.issuer.admit(&mut ctx, &mut carry_lo).unwrap_err(),
        Refused::Ineligible
    );
    let refused = event_fields(&e.db, "memory.dispatch_refused");
    assert_eq!(
        refused.last().unwrap(),
        &serde_json::json!({"reason":"ineligible","route_class":"non_local"})
    );
}

#[test]
fn hm_m2_n5_a_p1_tighten_before_admission_after_admission_and_twice() {
    // (a) Tightened before the admission: O2 commits it, the admission
    // transaction sees the moved generation, one rebuild, local only.
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    e.cfg.set(id(AGENT_B), true);
    let mut calls = 0;
    let mut inner = build_with(&e.issuer, 10, follow);
    let mut counting = |c: &mut DispatchContext| {
        calls += 1;
        inner(c)
    };
    let a = e.issuer.admit(&mut ctx, &mut counting).unwrap();
    assert_eq!(calls, 2, "one rebuild");
    assert_eq!(a.route_class(), RouteClass::Local);
    assert_eq!(state(&ctx), LO);
    assert_eq!(non_local_admissions(&e.db), 0);
    assert_eq!(
        history_of(&e.db, AGENT_B).last().unwrap(),
        "3|1|tightened|config_tighten"
    );
    // A non-local transport after the same tightening is refused.
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    e.cfg.set(id(AGENT_B), true);
    assert_eq!(
        e.issuer
            .admit(&mut ctx, &mut *build_with(&e.issuer, 10, non_local))
            .unwrap_err(),
        Refused::RouteMismatch
    );
    assert!(admission_rows(&e.db).is_empty());

    // (b) Tightened after the admission, before the reply: discarded and
    // recorded; the disclosure stands.
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
        .unwrap();
    let aid = a.admission_id().to_string();
    e.cfg.set(id(AGENT_B), true);
    assert_eq!(
        e.issuer
            .accept_reply(&mut ctx, a, Ok("SYNTH-late-reply".into()))
            .unwrap_err(),
        ReplyRefused::Discarded
    );
    assert_eq!(outcome_of(&e.db, &aid), "reply_discarded");
    let ev = events_of(&e.db, "memory.reply_discarded");
    assert_eq!(ev[0].0, format!("admission:{aid}:outcome"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&ev[0].2).unwrap(),
        serde_json::json!({"outcome":"reply_discarded","route_class":"non_local"})
    );
    assert_eq!(state(&ctx), LO, "the observed tightening reaches the unit");
    assert!(ctx.working_set().derived_labels().is_empty());

    // (c) Moved twice in one window: generation_unstable, nothing admitted.
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    e.cfg.set(id(AGENT_B), true);
    let mut calls = 0;
    let (db, cfg) = (e.db.clone(), e.cfg.clone());
    let mut moving = |c: &mut DispatchContext| {
        calls += 1;
        if calls == 2 {
            // Between the rebuild and its admission: another change commits.
            cfg.set(id(AGENT_B), false);
            fixture_owner_loosen(&db, AGENT_B);
        }
        let b = PromptBuilder::new(c);
        Some((b.build(), attest(c.route_class())))
    };
    assert_eq!(
        e.issuer.admit(&mut ctx, &mut moving).unwrap_err(),
        Refused::GenerationUnstable
    );
    assert_eq!(calls, 2, "no retry loop");
    assert!(admission_rows(&e.db).is_empty());
    assert_eq!(
        event_fields(&e.db, "memory.dispatch_refused")
            .last()
            .unwrap()["reason"],
        "generation_unstable"
    );
}

#[test]
fn hm_m2_n5_a_p2_route_class_mismatch_and_ineligible_prompts_refused() {
    let e = env_open_b();
    // A local-only agent: a transport without a complete attestation (an
    // unattested Flash override) is non-local and refused.
    let mut la = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let mut unattested = |c: &mut DispatchContext| {
        Some((
            PromptBuilder::new(c).build(),
            RouteAttestation::from_transport_evidence(TransportEvidence::Unattested),
        ))
    };
    assert_eq!(
        e.issuer.admit(&mut la, &mut unattested).unwrap_err(),
        Refused::RouteMismatch
    );
    // A verified in-process Flash (L2) is local and admitted.
    let mut flash = |c: &mut DispatchContext| {
        Some((
            PromptBuilder::new(c).build(),
            RouteAttestation::from_transport_evidence(TransportEvidence::VerifiedInProcessFlash),
        ))
    };
    assert_eq!(
        e.issuer.admit(&mut la, &mut flash).unwrap().route_class(),
        RouteClass::Local
    );
    // A local transport always fits, also for a non-local unit.
    let mut nb = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(
        e.issuer
            .admit(
                &mut nb,
                &mut *build_with(&e.issuer, 10, |_| RouteClass::Local)
            )
            .unwrap()
            .route_class(),
        RouteClass::Local
    );
    // A-T1: a local-only member of an unrestricted director never
    // dispatches non-locally.
    let director = e
        .issuer
        .begin_unit(
            agent(AGENT_B),
            Unit::TeamTask {
                workflow_id: Uuid::new_v4(),
                task_index: 0,
            },
        )
        .unwrap();
    let mut member = director.derive_member(&e.issuer, id(AGENT_A), 1).unwrap();
    assert_eq!(
        e.issuer
            .admit(&mut member, &mut *build_with(&e.issuer, 0, non_local))
            .unwrap_err(),
        Refused::RouteMismatch
    );
    // An envelope built from another context is refused.
    let other = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let mut foreign = |_: &mut DispatchContext| {
        Some((
            PromptBuilder::new(&other).build(),
            attest(RouteClass::NonLocal),
        ))
    };
    assert_eq!(
        e.issuer.admit(&mut nb, &mut foreign).unwrap_err(),
        Refused::EnvelopeMismatch
    );
    // A unit without retained data cannot include a record.
    let mut holder = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let recs = e.issuer.recall(&mut holder, memories(1)).unwrap();
    let mut chat = e
        .issuer
        .begin_unit(
            agent(AGENT_B),
            Unit::ChatTurn {
                turn_id: Uuid::new_v4(),
            },
        )
        .unwrap();
    let mut smuggle = |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        b.include(&recs[0]);
        Some((b.build(), attest(RouteClass::NonLocal)))
    };
    assert_eq!(
        e.issuer.admit(&mut chat, &mut smuggle).unwrap_err(),
        Refused::RetainedDataNotPermitted
    );
    // No transport could be built: refused, nothing admitted.
    let rows = admission_rows(&e.db).len();
    assert_eq!(
        e.issuer.admit(&mut nb, &mut |_| None).unwrap_err(),
        Refused::NoTransport
    );
    assert_eq!(admission_rows(&e.db).len(), rows);
}

#[test]
fn hm_m2_n5_a_c2_inclusion_unbound_at_admission_is_refused() {
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let recs = e.issuer.recall(&mut ctx, memories(10)).unwrap();
    assert_eq!(recs.len(), 2);
    // Both eligible when recalled; one label is deleted, one record's
    // content altered, before the admission re-evaluates them.
    let ids: Vec<String> = recs
        .iter()
        .map(|r| r.record_ref().record_id().to_string())
        .collect();
    for (i, r) in recs.iter().enumerate() {
        let mut fresh = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
        let mut one = |c: &mut DispatchContext| {
            let mut b = PromptBuilder::new(c);
            b.include(r);
            Some((b.build(), attest(RouteClass::NonLocal)))
        };
        if i == 0 {
            must(
                &e.db,
                &format!(
                    "DELETE FROM hm_record_label WHERE record_table = 'agent_memory' AND record_id = '{}'",
                    ids[0]
                ),
            );
        } else {
            must(
                &e.db,
                &format!(
                    "UPDATE agent_memory SET value_json = '\"SYNTH-x\"' WHERE id = {}",
                    ids[1]
                ),
            );
        }
        assert_eq!(
            e.issuer.admit(&mut fresh, &mut one).unwrap_err(),
            Refused::Ineligible,
            "inclusion {i}"
        );
    }
    assert_eq!(non_local_admissions(&e.db), 0);
    // On a local transport the same prompt is admitted with a LOCAL_ONLY
    // raw label (UNBOUND evaluates as LOCAL_ONLY_ORIGIN).
    let mut local = local_unit(&e, AGENT_B);
    let mut both = |c: &mut DispatchContext| {
        let mut b = PromptBuilder::new(c);
        for r in &recs {
            b.include(r);
        }
        Some((b.build(), attest(RouteClass::Local)))
    };
    let a = e.issuer.admit(&mut local, &mut both).unwrap();
    assert_eq!(a.raw_label(), OriginLabel::LocalOnlyOrigin);
}

#[test]
fn hm_m2_n5_a_p4_two_handles_ordering_by_the_admission_transaction() {
    // (1) A tightening on another handle that commits before the admission
    // transaction refuses a non-local admission.
    let e = env_open_b();
    let cfg2 = Arc::new(TestConfig::default());
    cfg2.set(id(AGENT_B), true);
    let issuer2 = Arc::new(DispatchIssuer::new(e.second_handle(), e.reg.clone(), cfg2).unwrap());
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(ctx.policy().policy_generation(), 2);
    let p = issuer2.resolve_agent_route_policy(id(AGENT_B)).unwrap();
    assert_eq!((p.local_only(), p.policy_generation()), (true, 3));
    assert_eq!(
        e.issuer
            .admit(&mut ctx, &mut *build_with(&e.issuer, 10, non_local))
            .unwrap_err(),
        Refused::RouteMismatch
    );
    assert!(admission_rows(&e.db).is_empty());

    // (2) An admission holding the write lock makes a tightening on the
    // other handle wait: the tightening commits after the admission.
    let e = env_open_b();
    let cfg2 = Arc::new(TestConfig::default());
    cfg2.set(id(AGENT_B), true);
    let issuer2 = Arc::new(DispatchIssuer::new(e.second_handle(), e.reg.clone(), cfg2).unwrap());
    let db3 = e.second_handle();
    must(&db3, "PRAGMA busy_timeout = 0;");
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let lock_held: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    type Tighten = std::thread::JoinHandle<Result<PolicySnapshot, Unresolved>>;
    let worker: Arc<Mutex<Option<Tighten>>> = Arc::new(Mutex::new(None));
    let fired = AtomicBool::new(false);
    let (held, slot) = (lock_held.clone(), worker.clone());
    e.issuer.install_probe(Box::new(move |p| {
        if p != ProbePoint::AdmissionPolicyRead || fired.swap(true, Ordering::SeqCst) {
            return;
        }
        // Inside the admission transaction: a writer that does not wait
        // fails at once, so the write lock is held now.
        *held.lock().unwrap() = Some(db3.hm().immediate(|_| Ok(())).is_err());
        let (started, ready) = mpsc::channel();
        let i2 = issuer2.clone();
        *slot.lock().unwrap() = Some(std::thread::spawn(move || {
            started.send(()).unwrap();
            i2.resolve_agent_route_policy(id(AGENT_B))
        }));
        ready.recv_timeout(CHILD_DEADLINE).unwrap();
    }));
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
        .unwrap();
    let tightened = worker
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(
        *lock_held.lock().unwrap(),
        Some(true),
        "lock held during admission"
    );
    assert_eq!(a.route_class(), RouteClass::NonLocal);
    assert_eq!(
        a.policy_generation(),
        2,
        "governed by changes committed before it"
    );
    assert_eq!(
        (tightened.local_only(), tightened.policy_generation()),
        (true, 3)
    );
    assert_eq!(
        admission_rows(&e.db),
        vec!["non_local|admitted|UNRESTRICTED_ORIGIN|2|2|0"]
    );
    assert_eq!(policy_of(&e.db, AGENT_B), "1|3|0|tightened");
    // The tightening governs the reply and every later admission.
    assert_eq!(
        e.issuer
            .accept_reply(&mut ctx, a, Ok("SYNTH-r".into()))
            .unwrap_err(),
        ReplyRefused::Discarded
    );
}

#[test]
fn hm_m2_n5_admission_event_failure_refuses() {
    let e = env_open_b();
    must(
        &e.db,
        "CREATE TRIGGER synthetic_fail_admitted BEFORE INSERT ON hm_event
         WHEN NEW.kind = 'memory.dispatch_admitted'
         BEGIN SELECT RAISE(ABORT, 'synthetic audit failure'); END;",
    );
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for transport in [follow as fn(RouteClass) -> RouteClass, |_| {
        RouteClass::Local
    }] {
        assert_eq!(
            e.issuer
                .admit(&mut ctx, &mut *build_with(&e.issuer, 10, transport))
                .unwrap_err(),
            Refused::AdmissionNotRecorded
        );
    }
    assert!(
        admission_rows(&e.db).is_empty(),
        "rolled back with its event"
    );
    must(&e.db, "DROP TRIGGER synthetic_fail_admitted;");
    e.issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
        .unwrap();
    assert_eq!(admission_rows(&e.db).len(), 1);
}

/// The names of the public functions in `src`.
fn pub_fn_names(src: &str) -> Vec<String> {
    src.match_indices("pub fn ")
        .map(|(i, m)| {
            src[i + m.len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect()
        })
        .collect()
}

#[test]
fn hm_m2_n5_a_g0_no_grant_state_and_no_grant_entry_point() {
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for _ in 0..3 {
        let a = e
            .issuer
            .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
            .unwrap();
        e.issuer.accept_reply(&mut ctx, a, Ok("r".into())).unwrap();
    }
    assert_eq!(count(&e.db, "SELECT COUNT(*) FROM hm_grant"), 0);
    assert_eq!(
        count(
            &e.db,
            "SELECT COUNT(*) FROM hm_dispatch_admission WHERE grant_dep_count <> 0"
        ),
        0
    );
    assert_eq!(
        count(&e.db, "SELECT COUNT(*) FROM hm_dispatch_admission"),
        3
    );
    // Negative control for the name scan.
    assert_eq!(
        pub_fn_names("impl X { pub fn grant_record(&self) {} pub fn other() {} }"),
        vec!["grant_record", "other"]
    );
    for (file, src) in hm_sources() {
        for name in pub_fn_names(&src) {
            let n = name.to_lowercase();
            assert!(
                !n.contains("grant") && !n.contains("revok") && !n.contains("declassif"),
                "{file}: grant entry point {name}"
            );
        }
        assert!(!src.contains("hm_grant"), "{file} names hm_grant");
    }
    let store = std::fs::read_to_string(workspace_root().join("persistence/src/hm.rs")).unwrap();
    let squashed = store
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    for write in ["into hm_grant", "update hm_grant", "from hm_grant"] {
        assert!(!squashed.contains(write), "persistence: {write}");
    }
}

/// Makes the agent's policy unresolvable in one of the four §4.6 ways.
fn make_unresolvable(e: &Env, how: &str) -> Unresolved {
    match how {
        "unregistered" => {
            e.reg.remove(id(AGENT_B));
            Unresolved::UnregisteredAgent
        }
        "unmigrated" => {
            must(&e.db, "DROP TABLE hm_migration;");
            Unresolved::StoreNotMigrated
        }
        "malformed" => {
            fixture_corrupt_policy(&e.db, AGENT_B, "policy_generation = 0");
            Unresolved::MalformedPolicy
        }
        "config" => {
            e.cfg.set_failing(true);
            Unresolved::ConfigUnreadable
        }
        _ => unreachable!(),
    }
}

#[test]
fn hm_m2_n5_o2_refuses_and_o3_discards_when_the_policy_is_unresolved() {
    for how in ["unregistered", "unmigrated", "malformed", "config"] {
        // O2: the dispatch is refused; nothing is admitted.
        let e = env_open_b();
        let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
        let why = make_unresolvable(&e, how);
        assert_eq!(
            e.issuer
                .admit(&mut ctx, &mut *build_with(&e.issuer, 0, follow))
                .unwrap_err(),
            Refused::Unresolved(why),
            "O2 {how}"
        );
        assert!(admission_rows(&e.db).is_empty(), "O2 {how}");

        // O3: the reply is discarded and the outcome recorded.
        let e = env_open_b();
        let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
        let a = e
            .issuer
            .admit(&mut ctx, &mut *build_with(&e.issuer, 0, follow))
            .unwrap();
        let aid = a.admission_id().to_string();
        make_unresolvable(&e, how);
        assert_eq!(
            e.issuer
                .accept_reply(&mut ctx, a, Ok("SYNTH-r".into()))
                .unwrap_err(),
            ReplyRefused::Discarded,
            "O3 {how}"
        );
        assert_eq!(outcome_of(&e.db, &aid), "reply_discarded", "O3 {how}");
    }
}

#[test]
fn hm_m2_n5_a_p3_out_of_band_config_edit() {
    // (i) An admission committed before the next observation point stands
    // and its request is sent; (iii) the reply that returns after the O3
    // commit is discarded.
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
        .unwrap();
    e.cfg.set(id(AGENT_B), true);
    assert_eq!(
        policy_of(&e.db, AGENT_B),
        "0|2|0|owner_confirmed",
        "no effect yet"
    );
    assert_eq!(outcome_of(&e.db, a.admission_id()), "admitted");
    let sent = a.prompt().to_string();
    assert!(sent.contains("SYNTH-open-1"));
    let aid = a.admission_id().to_string();
    assert_eq!(
        e.issuer
            .accept_reply(&mut ctx, a, Ok("SYNTH-r".into()))
            .unwrap_err(),
        ReplyRefused::Discarded
    );
    assert_eq!(policy_of(&e.db, AGENT_B), "1|3|0|tightened");
    assert_eq!(
        history_of(&e.db, AGENT_B).last().unwrap(),
        "3|1|tightened|config_tighten"
    );
    assert_eq!(outcome_of(&e.db, &aid), "reply_discarded");

    // (ii) First observed at O2: committed with its generation and history
    // row; the dispatch is rebuilt local-only (or refused, see A-P1 (a)).
    let e = env_open_b();
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    e.cfg.set(id(AGENT_B), true);
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
        .unwrap();
    assert_eq!(a.route_class(), RouteClass::Local);
    assert_eq!(a.policy_generation(), 3);
    assert_eq!(
        history_of(&e.db, AGENT_B).last().unwrap(),
        "3|1|tightened|config_tighten"
    );
    assert_eq!(
        events_of(&e.db, "memory.policy_tightened_by_config").len(),
        1
    );

    // (iv) An unreadable config refuses at O2 and discards at O3: see
    // hm_m2_n5_o2_refuses_and_o3_discards_when_the_policy_is_unresolved.

    // (v) A config edit to false never loosens at O2 or O3.
    let e = env();
    e.cfg.set(id(AGENT_A), false);
    let mut la = e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap();
    let a = e
        .issuer
        .admit(&mut la, &mut *build_with(&e.issuer, 0, non_local))
        .unwrap_err();
    assert_eq!(a, Refused::RouteMismatch);
    let a = e
        .issuer
        .admit(&mut la, &mut *build_with(&e.issuer, 0, follow))
        .unwrap();
    e.issuer.accept_reply(&mut la, a, Ok("r".into())).unwrap();
    assert_eq!(policy_of(&e.db, AGENT_A), "1|1|0|t0_baseline");
    assert_eq!(history_of(&e.db, AGENT_A).len(), 1);
    assert_eq!(state(&la), LO);
    assert_eq!(
        events_of(&e.db, "memory.policy_loosening_not_authorised").len(),
        1,
        "once per (agent, digest) across O1, O2 and O3"
    );
}

// ── N6: events and back-fill (§5.3, §15.1, §15.3) ───────────────────────────

fn subject_count(db: &NexusDatabase, subject: &str) -> i64 {
    count(
        db,
        &format!("SELECT COUNT(*) FROM hm_event WHERE subject = '{subject}'"),
    )
}

/// TEST FIXTURE: makes every insert of one event kind fail.
fn plant_event_failure(db: &NexusDatabase, name: &str, kind: &str) {
    must(
        db,
        &format!(
            "CREATE TRIGGER {name} BEFORE INSERT ON hm_event WHEN NEW.kind = '{kind}'
             BEGIN SELECT RAISE(ABORT, 'synthetic audit failure'); END;"
        ),
    );
}

#[test]
#[ignore = "child process entry for A-G7; run by the parent test"]
fn hm_m2_child_tighten_kill_point() {
    let Ok(path) = std::env::var("HM_M2_G7_DB") else {
        return;
    };
    let db = Arc::new(open(Path::new(&path)));
    let reg = Arc::new(TestRegistry::default());
    reg.add(id(AGENT_B));
    let cfg = Arc::new(TestConfig::default());
    cfg.set(id(AGENT_B), true);
    let issuer = DispatchIssuer::new(db, reg, cfg).unwrap();
    issuer.install_probe(block_at(format!("{:?}", ProbePoint::PolicyTightened)));
    let _ = issuer.begin_unit(agent(AGENT_B), goal());
    println!("HM-M2-UNEXPECTED-RETURN");
    std::process::exit(4);
}

#[test]
fn hm_m2_n6_a_g7_killed_after_tightening_commit_change_stands_event_backfilled_once() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    let mut child = spawn_child(
        "hm_m2_child_tighten_kill_point",
        &[("HM_M2_G7_DB", e.path.display().to_string())],
    );
    let rx = line_channel(&mut child);
    assert_eq!(wait_line(&rx, READY), format!("{READY}PolicyTightened"));
    child.kill().expect("kill child");
    assert!(!child.wait().unwrap().success());

    // Restart: a new process on the same database.
    let db = Arc::new(open(&e.path));
    assert_eq!(
        policy_of(&db, AGENT_B),
        "1|3|0|tightened",
        "the change stands"
    );
    assert_eq!(
        history_of(&db, AGENT_B).last().unwrap(),
        "3|1|tightened|config_tighten"
    );
    let subject = format!("policy:{AGENT_B}:3");
    assert_eq!(subject_count(&db, &subject), 0, "killed before its event");

    let report = backfill_authority_events(&db).unwrap();
    assert_eq!(
        report,
        BackfillReport {
            policy_events: 2,
            tombstone_events: 0,
            complete: true
        }
    );
    assert_eq!(
        events_of(&db, "memory.policy_tightened_by_config"),
        vec![(
            subject.clone(),
            Some(AGENT_B.to_string()),
            r#"{"local_only":1,"policy_generation":3}"#.to_string()
        )]
    );
    // The fixture loosening is described too; t0_baseline rows never are
    // (their event is migration:t0, A9).
    assert_eq!(
        events_of(&db, "memory.policy_loosened"),
        vec![(
            format!("policy:{AGENT_B}:2"),
            Some(AGENT_B.to_string()),
            r#"{"local_only":0,"policy_generation":2}"#.to_string()
        )]
    );
    for a in [AGENT_A, AGENT_B, AGENT_C, AGENT_D] {
        assert_eq!(subject_count(&db, &format!("policy:{a}:1")), 0);
    }
    // Exactly once.
    assert_eq!(
        backfill_authority_events(&db).unwrap(),
        BackfillReport {
            policy_events: 0,
            tombstone_events: 0,
            complete: true
        }
    );
    assert_eq!(subject_count(&db, &subject), 1);
    // The restarted process sees the tightening.
    let issuer =
        DispatchIssuer::new(db.clone(), e.reg.clone(), Arc::new(TestConfig::default())).unwrap();
    assert_eq!(
        state(&issuer.begin_unit(agent(AGENT_B), goal()).unwrap()),
        LO
    );
}

#[test]
fn hm_m2_n6_a_g7_failing_audit_never_undoes_a_tightening_or_a_restrictive_row() {
    let e = env();
    fixture_owner_loosen(&e.db, AGENT_B);
    plant_event_failure(
        &e.db,
        "synthetic_fail_tighten",
        "memory.policy_tightened_by_config",
    );
    plant_event_failure(&e.db, "synthetic_fail_created", "memory.policy_row_created");

    e.cfg.set(id(AGENT_B), true);
    let b = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    assert_eq!(state(&b), LO);
    assert_eq!(policy_of(&e.db, AGENT_B), "1|3|0|tightened");
    assert_eq!(subject_count(&e.db, &format!("policy:{AGENT_B}:3")), 0);

    let new_agent = e.issuer.begin_unit(agent(AGENT_E), goal()).unwrap();
    assert_eq!(state(&new_agent), LO);
    assert_eq!(policy_of(&e.db, AGENT_E), "1|1|0|default_restrictive");
    assert_eq!(subject_count(&e.db, &format!("policy:{AGENT_E}:1")), 0);
    // Later observations keep the stored restriction.
    e.cfg.set(id(AGENT_B), false);
    assert!(e
        .issuer
        .resolve_agent_route_policy(id(AGENT_B))
        .unwrap()
        .local_only());

    // Once audit works again, back-fill describes every change once.
    must(
        &e.db,
        "DROP TRIGGER synthetic_fail_tighten; DROP TRIGGER synthetic_fail_created;",
    );
    let report = backfill_authority_events(&e.db).unwrap();
    assert_eq!(report.policy_events, 3);
    for s in [
        format!("policy:{AGENT_B}:2"),
        format!("policy:{AGENT_B}:3"),
        format!("policy:{AGENT_E}:1"),
    ] {
        assert_eq!(subject_count(&e.db, &s), 1, "{s}");
    }
}

#[test]
fn hm_m2_n6_tombstone_events_backfilled_once() {
    let seed = seed_legacy_memories(AGENT_B, &[("SYNTH-t-1", 0.5), ("SYNTH-t-2", 0.5)])
        + &format!(
            "INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at)
               VALUES ('{PRE_T0_TASK_B}', '{AGENT_B}', 'SYNTH-goal', 'running', '[]', '2026-01-01T00:00:00Z');"
        );
    let e = env_seeded(
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D],
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D, AGENT_E],
        &seed,
    );
    let ids = strings(
        &e.db,
        "SELECT CAST(id AS TEXT) FROM agent_memory WHERE key LIKE 'SYNTH-t-%' ORDER BY id",
    );
    let members = count(&e.db, "SELECT COUNT(*) FROM hm_t0_member");
    // Any delete path fires the §5.1 trigger; here a direct DELETE.
    must(
        &e.db,
        &format!("DELETE FROM agent_memory WHERE id = {}", ids[0]),
    );
    let r = backfill_authority_events(&e.db).unwrap();
    assert_eq!(r.tombstone_events, 1);
    let first = format!("tombstone:agent_memory:{}", ids[0]);
    assert_eq!(
        events_of(&e.db, "memory.t0_member_deleted"),
        vec![(
            first.clone(),
            Some(AGENT_B.to_string()),
            r#"{"record_count":1}"#.to_string()
        )]
    );
    assert_eq!(
        backfill_authority_events(&e.db).unwrap().tombstone_events,
        0
    );
    assert_eq!(subject_count(&e.db, &first), 1);

    must(
        &e.db,
        &format!("DELETE FROM task_history WHERE id = '{PRE_T0_TASK_B}'"),
    );
    assert_eq!(
        backfill_authority_events(&e.db).unwrap().tombstone_events,
        1
    );
    assert_eq!(
        subject_count(&e.db, &format!("tombstone:task_history:{PRE_T0_TASK_B}")),
        1
    );
    // A post-T₀ row has no tombstone and no event when deleted.
    fixture_owner_loosen(&e.db, AGENT_B);
    let w = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let r = e
        .issuer
        .write_memory(&w, &mem_r("SYNTH-post", 0.5), &[])
        .unwrap();
    must(
        &e.db,
        &format!(
            "DELETE FROM agent_memory WHERE id = {}",
            r.record_ref().record_id()
        ),
    );
    assert_eq!(
        backfill_authority_events(&e.db).unwrap().tombstone_events,
        0
    );
    assert_eq!(events_of(&e.db, "memory.t0_member_deleted").len(), 2);
    // The ledger is untouched by deletion and by back-fill.
    assert_eq!(count(&e.db, "SELECT COUNT(*) FROM hm_t0_member"), members);
    let StartStatus::Verified(report) = verify_at_start(&e.db).unwrap() else {
        panic!("migrated")
    };
    assert!(report.checks.all_pass());
}

/// The §15.1 field allowlist per event kind.
fn allowed_fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "memory.policy_row_created"
        | "memory.policy_tightened"
        | "memory.policy_tightened_by_config"
        | "memory.policy_loosened" => &["local_only", "policy_generation"],
        "memory.policy_loosening_not_authorised" => &["policy_generation"],
        "memory.recall_filtered" => &["route_class", "eligible_count", "withheld_count"],
        "memory.unlabelled_record_observed" => &["route_class", "unbound_count"],
        "memory.dispatch_admitted" => &[
            "route_class",
            "record_count",
            "grant_dep_count",
            "raw_label",
            "policy_generation",
            "revocation_seq",
        ],
        "memory.dispatch_refused" => &["reason", "route_class"],
        "memory.reply_discarded" | "memory.transport_failed" => &["outcome", "route_class"],
        "memory.t0_member_deleted" => &["record_count"],
        "memory.migration_t0" => &["rows", "member_count", "member_set_sha256"],
        "memory.member_set_mismatch" => &["failed_checks"],
        "memory.member_record_inconsistent" => &["missing_untombstoned", "reappeared"],
        other => panic!("unknown event kind {other}"),
    }
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// The subject form of each kind (§5.3).
fn subject_fits(kind: &str, subject: &str) -> bool {
    let authority = |prefix: &str| subject.starts_with(prefix);
    match kind {
        "memory.policy_row_created"
        | "memory.policy_tightened"
        | "memory.policy_tightened_by_config"
        | "memory.policy_loosened" => {
            let parts: Vec<&str> = subject.split(':').collect();
            parts.len() == 3
                && parts[0] == "policy"
                && Uuid::parse_str(parts[1]).is_ok()
                && parts[2].parse::<i64>().is_ok_and(|g| g >= 1)
        }
        "memory.dispatch_admitted" => subject
            .strip_prefix("admission:")
            .is_some_and(|id| is_hex(id, 32)),
        "memory.reply_discarded" | "memory.transport_failed" => subject
            .strip_prefix("admission:")
            .and_then(|r| r.strip_suffix(":outcome"))
            .is_some_and(|id| is_hex(id, 32)),
        "memory.t0_member_deleted" => authority("tombstone:"),
        "memory.migration_t0" => subject == "migration:t0",
        _ => subject
            .strip_prefix(&format!("obs:{kind}:"))
            .is_some_and(|id| is_hex(id, 32)),
    }
}

/// A field value is a count, a generation, or a class name.
fn value_fits(key: &str, v: &serde_json::Value) -> bool {
    use serde_json::Value;
    let class = |allowed: &[&str]| v.as_str().is_some_and(|s| allowed.contains(&s));
    match key {
        "route_class" => class(&["local", "non_local"]),
        "raw_label" => class(&["UNRESTRICTED_ORIGIN", "LEGACY_UNKNOWN", "LOCAL_ONLY_ORIGIN"]),
        "outcome" => class(&[
            "admitted",
            "reply_used",
            "reply_discarded",
            "transport_failed",
        ]),
        "reason" => v
            .as_str()
            .is_some_and(|s| s.chars().all(|c| c.is_ascii_lowercase() || c == '_')),
        "local_only" => matches!(v.as_i64(), Some(0 | 1)),
        "failed_checks" => v.as_array().is_some_and(|a| {
            a.iter()
                .all(|x| matches!(x.as_str(), Some("L1" | "L2" | "L3" | "L4")))
        }),
        "rows" => v
            .as_object()
            .is_some_and(|o| o.values().all(|n| n.as_u64().is_some())),
        "member_set_sha256" => v.as_str().is_some_and(|s| is_hex(s, 64)),
        _ => matches!(v, Value::Number(n) if n.as_u64().is_some()),
    }
}

#[test]
fn hm_m2_n6_every_event_field_in_the_allowlist_and_content_free() {
    let seed = seed_legacy_memories(AGENT_B, &[("SYNTH-legacy-1", 0.5), ("SYNTH-legacy-2", 0.4)])
        + &format!(
            "INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at)
               VALUES ('{PRE_T0_TASK_B}', '{AGENT_B}', 'SYNTH-goal-text', 'running', '[\"SYNTH-step\"]', '2026-01-01T00:00:00Z');"
        );
    let e = env_seeded(
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D],
        &[AGENT_A, AGENT_B, AGENT_C, AGENT_D, AGENT_E],
        &seed,
    );
    // Every kind this module writes, plus the T₀ and verification kinds.
    e.issuer.begin_unit(agent(AGENT_A), goal()).unwrap(); // loosening not authorised
    e.issuer.begin_unit(agent(AGENT_E), goal()).unwrap(); // row created
    fixture_owner_loosen(&e.db, AGENT_B); // loosened (back-filled)
    let w = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    for k in ["SYNTH-open-1", "SYNTH-open-2"] {
        e.issuer.write_memory(&w, &mem_r(k, 0.9), &[]).unwrap();
    }
    insert_unlabelled(&e.db, AGENT_B, "SYNTH-unlabelled", 0.95);
    let mut ctx = e.issuer.begin_unit(agent(AGENT_B), goal()).unwrap();
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 10, follow))
        .unwrap(); // recall_filtered, unlabelled observed, admitted
    e.issuer
        .accept_reply(&mut ctx, a, Ok("SYNTH-reply".into()))
        .unwrap();
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 0, follow))
        .unwrap();
    e.issuer
        .accept_reply(&mut ctx, a, Err(TransportFailure))
        .unwrap_err(); // transport failed
    let a = e
        .issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 0, follow))
        .unwrap();
    e.cfg.set(id(AGENT_B), true);
    e.issuer
        .accept_reply(&mut ctx, a, Ok("SYNTH-late".into()))
        .unwrap_err(); // tightened by config, reply discarded
    e.issuer
        .admit(&mut ctx, &mut *build_with(&e.issuer, 0, non_local))
        .unwrap_err(); // dispatch refused
    must(
        &e.db,
        &format!("DELETE FROM task_history WHERE id = '{PRE_T0_TASK_B}'"),
    );
    backfill_authority_events(&e.db).unwrap(); // t0 member deleted, loosened
                                               // Verification events: a member removed out of band with its delete
                                               // trigger dropped (L4 fails; missing-untombstoned).
    must(
        &e.db,
        "DROP TRIGGER hm_del_agent_memory;
         DELETE FROM agent_memory WHERE key = 'SYNTH-legacy-2';",
    );
    verify_at_start(&e.db).unwrap();

    let all = strings(&e.db, "SELECT kind FROM hm_event ORDER BY n");
    let kinds: std::collections::BTreeSet<&str> = all.iter().map(|s| s.as_str()).collect();
    for k in [
        "memory.migration_t0",
        "memory.policy_row_created",
        "memory.policy_tightened_by_config",
        "memory.policy_loosened",
        "memory.policy_loosening_not_authorised",
        "memory.recall_filtered",
        "memory.unlabelled_record_observed",
        "memory.dispatch_admitted",
        "memory.dispatch_refused",
        "memory.reply_discarded",
        "memory.transport_failed",
        "memory.t0_member_deleted",
        "memory.member_set_mismatch",
        "memory.member_record_inconsistent",
    ] {
        assert!(kinds.contains(k), "scenario produced no {k}");
    }

    // Strings that must never appear in a field: agent ids, admission ids,
    // task ids, record keys and content markers, config digests, subjects.
    let mut forbidden: Vec<String> = [AGENT_A, AGENT_B, AGENT_C, AGENT_D, AGENT_E, PRE_T0_TASK_B]
        .iter()
        .map(|s| s.to_string())
        .collect();
    forbidden.extend(strings(
        &e.db,
        "SELECT admission_id FROM hm_dispatch_admission",
    ));
    forbidden.extend(["SYNTH".to_string(), "cfg-digest".to_string()]);
    let subjects = strings(&e.db, "SELECT subject FROM hm_event");
    forbidden.extend(subjects.iter().cloned());

    let rows: Vec<(String, String, Option<String>, String)> =
        e.db.with_transaction(|tx| {
            let mut stmt =
                tx.prepare("SELECT subject, kind, agent_id, fields FROM hm_event ORDER BY n")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            let mut v = Vec::new();
            for row in rows {
                v.push(row?);
            }
            Ok(v)
        })
        .unwrap();
    for (subject, kind, agent_id, fields) in &rows {
        assert!(subject_fits(kind, subject), "{kind}: subject {subject}");
        if let Some(a) = agent_id {
            assert!(Uuid::parse_str(a).is_ok(), "{kind}: agent column is a UUID");
        }
        let v: serde_json::Value = serde_json::from_str(fields).unwrap();
        let obj = v
            .as_object()
            .unwrap_or_else(|| panic!("{kind}: fields object"));
        let allowed = allowed_fields(kind);
        for (key, value) in obj {
            assert!(
                allowed.contains(&key.as_str()),
                "{kind}: field {key} not allowed"
            );
            assert!(value_fits(key, value), "{kind}: {key} = {value}");
        }
        for needle in &forbidden {
            assert!(
                !fields.contains(needle.as_str()),
                "{kind}: field leaks {needle}"
            );
        }
    }
    // Negative controls for the checks themselves.
    assert!(!subject_fits(
        "memory.dispatch_admitted",
        "admission:not-hex"
    ));
    assert!(!value_fits("route_class", &serde_json::json!("SYNTH-key")));
    assert!(!value_fits("withheld_count", &serde_json::json!("7")));
    assert!(!allowed_fields("memory.recall_filtered").contains(&"key"));
}
