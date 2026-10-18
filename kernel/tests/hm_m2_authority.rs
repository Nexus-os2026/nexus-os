//! P3-C11-HM-M2: the dormant kernel authority core of P3-HM-CONTRACT-02
//! (sealed context, §4.2 resolution, labelled writes, route-first recall,
//! §12.4 admission, reply acceptance, events and back-fill).
//!
//! Synthetic databases only: files in a test-owned temporary directory.
//! Crash steps run in child processes of this test binary; readiness is a
//! line on a pipe, never a sleep. Test names carry their node (`n1_` …).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use nexus_kernel::hm::{
    run_t0_option_a, unit_start_state, unit_taint_step, AgentRegistry, ConfigInput,
    ConfigReadError, ConfigReading, DispatchContext, DispatchIssuer, EvaluatedLabel, OriginLabel,
    Principal, ProbePoint, RouteClass, Unit, Unresolved, MAX_RECORDS,
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
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let db = Arc::new(open(&path));
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
