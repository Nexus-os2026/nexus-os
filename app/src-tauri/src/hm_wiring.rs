//! Historical-memory wiring of the desktop (P3-HM-CONTRACT-02 §4.2, §9,
//! §11.0; mission P3-C11-HM-M3 R6).
//!
//! - [`DesktopAgentRegistry`]: supervisor- and meta-registered agents, with
//!   the names the D-2 policy resolution used (`lib.rs` `resolve_agent_llm_policy`).
//! - [`DesktopConfigInput`]: the untrusted config file as an input that can
//!   only tighten: `load_config`, the assignment keys of the D-2 rule, and the
//!   SHA-256 of the file bytes as its content digest. A read failure is an
//!   error.
//! - [`build_dispatch_issuer`]: the single `DispatchIssuer::new` of the
//!   process, called while `AppState` is built.
//! - [`start_historical_memory`]: at start, after the legacy database
//!   cleanup and the database open (also for the in-memory fallback): the T₀
//!   migration with the AD-5 Option A baseline over the registered agents ∪
//!   the agent ids of the in-scope rows, then the ledger verification, then
//!   the authority-event back-fill.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use nexus_kernel::hm::{
    backfill_authority_events, run_t0_option_a_at_start, verify_at_start, AgentRegistry,
    ConfigInput, ConfigReadError, ConfigReading, DispatchIssuer, StartStatus, T0Outcome,
};
use nexus_kernel::supervisor::{AgentId, Supervisor};
use nexus_persistence::NexusDatabase;
use sha2::Digest;
use uuid::Uuid;

use crate::AgentMeta;

/// Supervisor identity for the issuer (§2.5, §4.2 step 1): an agent is
/// registered when the supervisor or the desktop's metadata holds it.
pub(crate) struct DesktopAgentRegistry {
    supervisor: Arc<Mutex<Supervisor>>,
    meta: Arc<Mutex<HashMap<AgentId, AgentMeta>>>,
}

impl DesktopAgentRegistry {
    pub(crate) fn new(
        supervisor: Arc<Mutex<Supervisor>>,
        meta: Arc<Mutex<HashMap<AgentId, AgentMeta>>>,
    ) -> Self {
        Self { supervisor, meta }
    }

    /// Every registered agent id: the supervisor's and the metadata's.
    pub(crate) fn registered(&self) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = self
            .meta
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .keys()
            .copied()
            .collect();
        {
            let supervisor = self.supervisor.lock().unwrap_or_else(|p| p.into_inner());
            ids.extend(
                supervisor
                    .health_check()
                    .into_iter()
                    .map(|status| status.id),
            );
        }
        ids.sort();
        ids.dedup();
        ids
    }
}

impl AgentRegistry for DesktopAgentRegistry {
    fn is_registered(&self, agent_id: &Uuid) -> bool {
        !self.names(agent_id).is_empty()
    }

    /// The metadata name, then the supervisor's manifest name, as the D-2
    /// policy resolution read them. The two guards are never held together.
    fn names(&self, agent_id: &Uuid) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(entry) = self
            .meta
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(agent_id)
        {
            names.push(entry.name.clone());
        }
        if let Some(handle) = self
            .supervisor
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_agent(*agent_id)
        {
            names.push(handle.manifest.name.clone());
        }
        names
    }
}

/// The config file assignment keys of an agent, in the D-2 order
/// (`lib.rs` `resolve_agent_llm_policy`): the id and its normalized form,
/// then each registered name normalized and as given.
pub(crate) fn agent_config_keys(agent_id: &str, names: &[String]) -> Vec<String> {
    let mut keys = vec![
        agent_id.to_string(),
        crate::normalize_agent_config_key(agent_id),
    ];
    for name in names {
        keys.push(crate::normalize_agent_config_key(name));
        keys.push(name.clone());
    }
    keys
}

/// Where the config file is read from.
enum ConfigSource {
    /// `nexus_kernel::config::load_config`, at `config_path()`.
    Live,
    /// A given file (tests).
    #[cfg(test)]
    File(PathBuf),
}

/// The config file as an untrusted input (§3, §4.1, §4.2 step 3).
pub(crate) struct DesktopConfigInput {
    source: ConfigSource,
}

impl DesktopConfigInput {
    /// The production input: the operator's config file.
    pub(crate) fn live() -> Self {
        Self {
            source: ConfigSource::Live,
        }
    }

    /// An input reading `path` (tests: a synthetic config file).
    #[cfg(test)]
    pub(crate) fn file(path: PathBuf) -> Self {
        Self {
            source: ConfigSource::File(path),
        }
    }

    /// The parsed configuration and the bytes of the file it came from.
    fn read(&self) -> Result<(nexus_kernel::config::NexusConfig, Vec<u8>), ConfigReadError> {
        let path: PathBuf = match &self.source {
            ConfigSource::Live => {
                nexus_kernel::config::config_path().map_err(|_| ConfigReadError)?
            }
            #[cfg(test)]
            ConfigSource::File(path) => path.clone(),
        };
        let config = match &self.source {
            ConfigSource::Live => nexus_kernel::config::load_config(),
            #[cfg(test)]
            ConfigSource::File(path) => nexus_kernel::config::load_config_from_path(path),
        }
        .map_err(|_| ConfigReadError)?;
        let bytes = std::fs::read(&path).map_err(|_| ConfigReadError)?;
        Ok((config, bytes))
    }
}

impl ConfigInput for DesktopConfigInput {
    fn read_local_only(
        &self,
        agent_id: &Uuid,
        names: &[String],
    ) -> Result<ConfigReading, ConfigReadError> {
        let (config, bytes) = self.read()?;
        let keys = agent_config_keys(&agent_id.to_string(), names);
        let local_only = keys.iter().any(|key| {
            config
                .agent_llm_assignments
                .get(key)
                .is_some_and(|assignment| assignment.local_only)
        });
        Ok(ConfigReading {
            local_only,
            content_digest: hex::encode(sha2::Sha256::digest(&bytes)),
        })
    }
}

/// The process's single dispatch issuer (§11.0), over the primary state
/// database. Its failure leaves no issuer: no agent dispatch on any route.
pub(crate) fn build_dispatch_issuer(
    db: Arc<NexusDatabase>,
    registry: Arc<DesktopAgentRegistry>,
    config: Arc<dyn ConfigInput>,
) -> Result<Arc<DispatchIssuer>, String> {
    DispatchIssuer::new(db, registry, config)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

/// What the start-up sequence found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HmStart {
    pub(crate) t0: Result<T0Outcome, String>,
    pub(crate) verified: Result<StartStatus, String>,
    pub(crate) backfill: Result<nexus_kernel::hm::BackfillReport, String>,
}

/// T₀ (Option A over the registered agents ∪ the agent ids of the in-scope
/// rows), then the ledger checks, then the back-fill of authority events.
/// Each failure is reported; none grants anything (§9.3: until T₀ is
/// complete every agent is `Unresolved`).
pub(crate) fn start_historical_memory(
    db: &NexusDatabase,
    registry: &DesktopAgentRegistry,
) -> HmStart {
    let t0 = run_t0_option_a_at_start(db, &registry.registered()).map_err(|e| e.to_string());
    if let Err(e) = &t0 {
        eprintln!("historical memory: T0 migration failed: {e}; every agent is unresolved");
    }
    let verified = verify_at_start(db).map_err(|e| e.to_string());
    match &verified {
        Ok(StartStatus::Verified(report)) if !report.checks.all_pass() => eprintln!(
            "historical memory: ledger checks failed: {:?}",
            report.checks.failed()
        ),
        Err(e) => eprintln!("historical memory: ledger verification failed: {e}"),
        _ => {}
    }
    let backfill = backfill_authority_events(db).map_err(|e| e.to_string());
    if let Err(e) = &backfill {
        eprintln!("historical memory: event back-fill failed: {e}");
    }
    HmStart {
        t0,
        verified,
        backfill,
    }
}

/// Test support shared by the desktop's tests (`cfg(test)` only).
#[cfg(test)]
pub(crate) mod test_support {
    use nexus_kernel::errors::AgentError;
    use nexus_kernel::hm::{DispatchContext, Principal, PromptBuilder, Unit};
    use uuid::Uuid;

    use crate::AppState;

    /// TEST FIXTURE, not product code (mission AMENDMENT 1 A1.9): an Owner
    /// loosening (M4 builds the native confirmation), committed through the
    /// store with its `owner_loosen` history row first (§5.1 trigger). The
    /// agent's restrictive row is created first if it is missing.
    pub(crate) fn owner_loosen(state: &AppState, agent_id: &str) {
        let issuer = state.hm_issuer().expect("dispatch issuer");
        issuer
            .resolve_agent_route_policy(Uuid::parse_str(agent_id).expect("agent UUID"))
            .expect("registered agent");
        crate::hm_m3_tests::fixture_owner_loosen(&state.db, agent_id);
    }

    /// A goal unit of `agent_id`, as the runtime starts one (O1); a refusal
    /// is reported as the desktop reports an unestablished policy.
    pub(crate) fn goal_unit(
        state: &AppState,
        agent_id: &str,
    ) -> Result<DispatchContext, AgentError> {
        let refused = |e: String| {
            AgentError::SupervisorError(format!(
                "{}: {e}",
                crate::commands::cognitive::unresolved_policy_error()
            ))
        };
        let issuer = state.hm_issuer().map_err(refused)?;
        let agent = Uuid::parse_str(agent_id).map_err(|e| refused(e.to_string()))?;
        issuer
            .begin_unit(
                Principal::Agent(agent),
                Unit::Goal {
                    goal_id: Uuid::new_v4(),
                },
            )
            .map_err(|e| refused(e.to_string()))
    }

    /// One planning dispatch of `prompt` (backend text) in a fresh goal unit
    /// of `agent_id`, through the production adapter of a goal's loop.
    pub(crate) fn plan_text(
        state: &AppState,
        agent_id: &str,
        prompt: &str,
    ) -> Result<String, AgentError> {
        use nexus_kernel::cognitive::PlannerLlm;
        let mut ctx = goal_unit(state, agent_id)?;
        crate::hm_dispatch::GatewayPlannerLlm::for_loop(state, None)
            .plan_query(&mut ctx, &mut |c: &mut DispatchContext| {
                let mut b = PromptBuilder::new(c);
                b.text(prompt);
                Some(b.build())
            })
            .map(|reply| reply.text().to_string())
    }
}

#[cfg(test)]
mod tests {
    //! N2 (mission R6): the start-up sequence on synthetic databases (A-M1),
    //! the config input, the registry, and the Flash (L2) path conditions,
    //! built here with `flash-infer`.

    use super::*;
    use crate::hm_m3_tests::{
        fixture_owner_loosen, label_keys, labels, ledger, ledger_digest, migrations, policies,
    };
    use nexus_kernel::hm::{
        EvaluatedLabel, OriginLabel, Principal, RecallQuery, RecordTable, RouteClass, Unit,
    };
    use std::path::Path;

    const AGENT_X: &str = "0a000000-0000-4000-8000-00000000000a";
    const AGENT_Y: &str = "0b000000-0000-4000-8000-00000000000b";

    /// A test-owned temporary directory, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("nexus-hm-m3-n2-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn must(db: &NexusDatabase, sql: &str) {
        db.with_transaction(|tx| {
            tx.execute_batch(sql)?;
            Ok(())
        })
        .unwrap_or_else(|e| panic!("{e} for: {sql}"));
    }

    const IN_SCOPE: [&str; 5] = [
        "agent_memory",
        "task_history",
        "strategy_scores",
        "world_model_entities",
        "world_model_relationships",
    ];

    /// `table|id|sha256(every column)` of every row of the five tables.
    fn row_hashes(db: &NexusDatabase) -> Vec<String> {
        let mut out = Vec::new();
        for table in IN_SCOPE {
            let rows = db
                .with_transaction(|tx| {
                    let mut stmt = tx.prepare(&format!("SELECT * FROM {table} ORDER BY id"))?;
                    let n = stmt.column_count();
                    let rows = stmt.query_map([], |r| {
                        let mut h = sha2::Sha256::new();
                        let mut id = String::new();
                        for i in 0..n {
                            let v = r.get_ref(i)?;
                            let text = match v {
                                rusqlite::types::ValueRef::Null => "NULL".to_string(),
                                rusqlite::types::ValueRef::Integer(x) => format!("i{x}"),
                                rusqlite::types::ValueRef::Real(x) => format!("r{x}"),
                                rusqlite::types::ValueRef::Text(t) => {
                                    format!("t{}", String::from_utf8_lossy(t))
                                }
                                rusqlite::types::ValueRef::Blob(b) => {
                                    format!("b{}", hex::encode(b))
                                }
                            };
                            if i == 0 {
                                id = text.clone();
                            }
                            h.update(text.len().to_be_bytes());
                            h.update(text.as_bytes());
                        }
                        Ok(format!("{table}|{id}|{}", hex::encode(h.finalize())))
                    })?;
                    let mut v = Vec::new();
                    for row in rows {
                        v.push(row?);
                    }
                    Ok(v)
                })
                .unwrap();
            out.extend(rows);
        }
        out
    }

    /// Pre-T₀ rows in all five tables: agents X and Y (UUIDs), and an id
    /// that is no UUID.
    fn seed_pre_t0(db: &NexusDatabase) {
        must(
            db,
            &format!(
                "INSERT INTO agent_memory (agent_id, memory_type, key, value_json, relevance_score,
                   access_count, created_at, last_accessed)
                 VALUES ('{AGENT_X}', 'semantic', 'SYNTH-m1', '\"SYNTH\"', 0.9, 0,
                   '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                        ('{AGENT_X}', 'episodic', 'SYNTH-m2', '\"SYNTH\"', 0.8, 3,
                   '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z'),
                        ('not-a-uuid', 'semantic', 'SYNTH-n1', '\"SYNTH\"', 0.5, 0,
                   '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z');
                 INSERT INTO task_history (id, agent_id, goal, status, steps_json, started_at)
                   VALUES ('11111111-0000-4000-8000-0000000000a1', '{AGENT_X}', 'SYNTH-goal',
                     'completed', '[]', '2026-01-01T00:00:00Z');
                 INSERT INTO strategy_scores (agent_id, strategy_hash, goal_type, uses, successes,
                   total_fuel, total_duration_secs, composite_score, updated_at)
                   VALUES ('{AGENT_Y}', 'SYNTH-hash', 'SYNTH-type', 2, 1, 3.0, 4.0, 0.0,
                     '2026-01-01T00:00:00Z');
                 INSERT INTO world_model_entities (agent_id, entity_json, created_at)
                   VALUES ('{AGENT_X}', '{{\"SYNTH\":1}}', '2026-01-01T00:00:00Z');
                 INSERT INTO world_model_relationships (agent_id, relationship_json, created_at)
                   VALUES ('{AGENT_Y}', '{{\"SYNTH\":2}}', '2026-01-01T00:00:00Z');"
            ),
        );
    }

    fn empty_registry() -> DesktopAgentRegistry {
        DesktopAgentRegistry::new(
            Arc::new(Mutex::new(Supervisor::new())),
            Arc::new(Mutex::new(HashMap::new())),
        )
    }

    fn registry_with(agent: Uuid, name: &str) -> Arc<DesktopAgentRegistry> {
        let reg = empty_registry();
        reg.meta.lock().unwrap().insert(
            agent,
            AgentMeta {
                name: name.to_string(),
                last_action: String::new(),
            },
        );
        Arc::new(reg)
    }

    /// A-M1 (production start-up): on a synthetic pre-T₀ database file every
    /// row is kept byte for byte, labelled `LEGACY_UNKNOWN` and in the
    /// ledger; the count and digest are stored; the Option A baseline covers
    /// the registered agents ∪ the agent UUIDs of the rows; a second start
    /// is a no-op; a non-local recall excludes all of it.
    #[test]
    fn hm_m3_n2_a_m1_start_up_migrates_a_pre_t0_database_exactly_once() {
        let dir = TempDir::new();
        let path = dir.0.join("state.db");
        let db = Arc::new(NexusDatabase::open(&path).unwrap());
        seed_pre_t0(&db);
        let before = row_hashes(&db);
        assert_eq!(before.len(), 7);

        let registered = Uuid::new_v4();
        let reg = registry_with(registered, "synthetic-registered");
        let start = start_historical_memory(&db, &reg);
        let Ok(T0Outcome::Migrated {
            member_count,
            member_set_sha256,
        }) = start.t0.clone()
        else {
            panic!("{start:?}");
        };
        assert_eq!(member_count, 7);
        assert!(
            matches!(&start.verified, Ok(StartStatus::Verified(r)) if r.checks.all_pass()),
            "{start:?}"
        );
        assert!(start.backfill.is_ok());
        assert_eq!(row_hashes(&db), before, "per-row SHA-256 unchanged");

        // Every row labelled LEGACY_UNKNOWN, `legacy`, in the ledger, bound.
        let label_rows = labels(&db);
        assert_eq!(label_rows.len(), 7);
        let mut kinds: Vec<String> = label_rows
            .iter()
            .map(|l| format!("{}|{}|{}", l.origin, l.source_kind, l.policy_generation))
            .collect();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds, vec!["LEGACY_UNKNOWN|legacy|0"]);
        let members = ledger(&db);
        assert_eq!(
            label_rows
                .iter()
                .filter(|l| members.iter().any(|m| m.record_table == l.record_table
                    && m.record_id == l.record_id
                    && m.t0_content_sha256 == l.content_sha256
                    && m.agent_id == l.agent_id))
                .count(),
            7
        );
        for table in RecordTable::ALL {
            for member in members
                .iter()
                .filter(|m| m.record_table == table.table_name())
            {
                assert_eq!(
                    nexus_kernel::hm::evaluate_label(&db, table, &member.record_id).unwrap(),
                    EvaluatedLabel::Bound(OriginLabel::LegacyUnknown)
                );
            }
        }
        // Count and digest stored; the digest is the ledger's.
        assert_eq!(ledger_digest(&members), member_set_sha256);
        assert_eq!(
            migrations(&db)
                .iter()
                .map(|m| format!("{}|{}|{}", m.state, m.member_count, m.member_set_sha256))
                .collect::<Vec<_>>(),
            vec![format!("COMPLETE|7|{member_set_sha256}")]
        );
        // Option A over registered ∪ row agents; a non-UUID id has no row.
        let mut expected = vec![
            AGENT_X.to_string(),
            AGENT_Y.to_string(),
            registered.to_string(),
        ];
        expected.sort();
        let policy_rows = policies(&db);
        assert_eq!(
            policy_rows
                .iter()
                .map(|p| p.agent_id.clone())
                .collect::<Vec<_>>(),
            expected
        );
        let mut baselines: Vec<String> = policy_rows
            .iter()
            .map(|p| {
                format!(
                    "{}|{}|{}",
                    p.local_only.unwrap_or(-1),
                    p.policy_generation.unwrap_or(-1),
                    p.basis.as_deref().unwrap_or("NULL")
                )
            })
            .collect();
        baselines.sort();
        baselines.dedup();
        assert_eq!(baselines, vec!["1|1|t0_baseline"]);

        // A second start (a new handle on the same file) is a no-op.
        drop(db);
        let db = Arc::new(NexusDatabase::open(&path).unwrap());
        let keys = label_keys(&db);
        let again = start_historical_memory(&db, &empty_registry());
        assert_eq!(again.t0, Ok(T0Outcome::AlreadyComplete));
        assert_eq!(row_hashes(&db), before);
        assert_eq!(label_keys(&db), keys);
        assert_eq!(ledger(&db).len(), 7);

        // A non-local recall of X excludes all of its history.
        let x = Uuid::parse_str(AGENT_X).unwrap();
        let reg = registry_with(x, "synthetic-x");
        let dir_cfg = TempDir::new();
        let cfg = dir_cfg.0.join("config.toml");
        let issuer =
            build_dispatch_issuer(db.clone(), reg, Arc::new(DesktopConfigInput::file(cfg)))
                .unwrap();
        fixture_owner_loosen(&db, AGENT_X);
        let mut ctx = issuer
            .begin_unit(
                Principal::Agent(x),
                Unit::Goal {
                    goal_id: Uuid::new_v4(),
                },
            )
            .unwrap();
        assert_eq!(ctx.route_class(), RouteClass::NonLocal);
        for table in RecordTable::ALL {
            assert!(issuer
                .recall(&mut ctx, RecallQuery { table, limit: 64 })
                .unwrap()
                .is_empty());
        }
    }

    /// The config input: the D-2 assignment keys (id, normalized id, each
    /// name normalized and as given), the SHA-256 of the file bytes, and an
    /// unreadable file as an error.
    #[test]
    fn hm_m3_n2_config_input_matches_the_d2_keys_and_digests_the_file() {
        let dir = TempDir::new();
        let path = dir.0.join("config.toml");
        let input = DesktopConfigInput::file(path.clone());
        let id = Uuid::parse_str(AGENT_X).unwrap();
        let names = vec!["Synthetic Agent".to_string()];
        // A missing file is the first run: the default configuration.
        let first = input.read_local_only(&id, &names).unwrap();
        assert!(!first.local_only);
        assert_eq!(
            first.content_digest,
            hex::encode(sha2::Sha256::digest(std::fs::read(&path).unwrap()))
        );
        assert_eq!(
            agent_config_keys(AGENT_X, &names),
            vec![
                AGENT_X.to_string(),
                AGENT_X.replace('-', "_"),
                "synthetic_agent".to_string(),
                "Synthetic Agent".to_string(),
            ]
        );
        for (key, expected) in [
            (AGENT_X.to_string(), true),
            (AGENT_X.replace('-', "_"), true),
            ("synthetic_agent".to_string(), true),
            ("Synthetic Agent".to_string(), true),
            ("another_agent".to_string(), false),
        ] {
            let mut config = nexus_kernel::config::NexusConfig::default();
            config.agent_llm_assignments.insert(
                key.clone(),
                nexus_kernel::config::AgentLlmAssignment {
                    provider_id: String::new(),
                    local_only: true,
                    budget_dollars: 0,
                    budget_tokens: 0,
                },
            );
            nexus_kernel::config::save_config_to_path(&path, &config).unwrap();
            let reading = input.read_local_only(&id, &names).unwrap();
            assert_eq!(reading.local_only, expected, "{key}");
            assert_eq!(
                reading.content_digest,
                hex::encode(sha2::Sha256::digest(std::fs::read(&path).unwrap()))
            );
        }
        // `local_only = false` under a key is no tightening.
        let mut config = nexus_kernel::config::NexusConfig::default();
        config.agent_llm_assignments.insert(
            AGENT_X.to_string(),
            nexus_kernel::config::AgentLlmAssignment {
                provider_id: "anthropic".into(),
                local_only: false,
                budget_dollars: 0,
                budget_tokens: 0,
            },
        );
        nexus_kernel::config::save_config_to_path(&path, &config).unwrap();
        assert!(!input.read_local_only(&id, &names).unwrap().local_only);
        // Unreadable: an error, never a reading.
        std::fs::write(&path, "local_only = [").unwrap();
        assert_eq!(
            input.read_local_only(&id, &names).unwrap_err(),
            ConfigReadError
        );
    }

    /// The registry: the metadata's and the supervisor's agents, with their
    /// names in the D-2 order; anything else is not registered.
    #[test]
    fn hm_m3_n2_registry_is_the_supervisor_and_the_metadata() {
        let meta_only = Uuid::new_v4();
        let reg = registry_with(meta_only, "Meta Name");
        let manifest = nexus_kernel::manifest::parse_manifest(
            r#"
name = "supervised-agent"
version = "1.0.0"
capabilities = ["llm.query"]
fuel_budget = 100
"#,
        )
        .unwrap();
        let supervised = reg
            .supervisor
            .lock()
            .unwrap()
            .start_agent(manifest)
            .unwrap();
        reg.meta.lock().unwrap().insert(
            supervised,
            AgentMeta {
                name: "Desktop Name".into(),
                last_action: String::new(),
            },
        );
        assert!(reg.is_registered(&meta_only));
        assert!(reg.is_registered(&supervised));
        assert!(!reg.is_registered(&Uuid::new_v4()));
        assert_eq!(reg.names(&meta_only), vec!["Meta Name".to_string()]);
        assert_eq!(
            reg.names(&supervised),
            vec!["Desktop Name".to_string(), "supervised-agent".to_string()]
        );
        let mut all = vec![meta_only, supervised];
        all.sort();
        assert_eq!(reg.registered(), all);
    }

    /// The desktop state is built with the issuer, after T₀.
    #[test]
    fn hm_m3_n2_app_state_runs_t0_and_holds_the_issuer() {
        let state = crate::AppState::new_in_memory();
        assert!(state.hm_issuer().is_ok());
        assert_eq!(
            migrations(&state.db)
                .into_iter()
                .map(|m| m.state)
                .collect::<Vec<_>>(),
            vec!["COMPLETE"]
        );
    }

    fn flash_at(model: &Path) -> nexus_connectors_llm::providers::FlashProvider {
        let model = model.to_string_lossy().into_owned();
        nexus_connectors_llm::providers::FlashProvider::new(
            model.clone(),
            nexus_flash_infer::LoadConfig {
                model_path: model,
                ..Default::default()
            },
            nexus_flash_infer::GenerationConfig::fast(),
        )
    }

    /// A-F4 (feature on) and A-F1's attestation: with in-process inference
    /// compiled in, a Flash provider is local only when its model is a
    /// regular file under the model directory, after canonicalisation.
    #[test]
    fn hm_m3_n2_a_f4_flash_is_local_only_with_its_model_inside_the_model_directory() {
        use nexus_connectors_llm::attest::{attest_route, attest_route_in};
        let root = TempDir::new();
        let models = root.0.join("models");
        let outside = root.0.join("outside");
        std::fs::create_dir_all(models.join("nested")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let inside = models.join("nested/synthetic.gguf");
        std::fs::write(&inside, b"SYNTH").unwrap();
        let elsewhere = outside.join("synthetic.gguf");
        std::fs::write(&elsewhere, b"SYNTH").unwrap();
        let class = |model: &Path| attest_route_in(&flash_at(model), Some(&models)).class();

        assert_eq!(class(&inside), RouteClass::Local);
        let shared = Arc::new(flash_at(&inside));
        assert_eq!(
            attest_route_in(&shared, Some(&models)).class(),
            RouteClass::Local
        );
        // Outside the directory, through `..`, a directory, missing, the
        // directory itself, no directory established.
        assert_eq!(class(&elsewhere), RouteClass::NonLocal);
        assert_eq!(
            class(&models.join("../outside/synthetic.gguf")),
            RouteClass::NonLocal
        );
        assert_eq!(class(&models.join("nested")), RouteClass::NonLocal);
        assert_eq!(class(&models.join("absent.gguf")), RouteClass::NonLocal);
        assert_eq!(class(&models), RouteClass::NonLocal);
        assert_eq!(
            attest_route_in(&flash_at(&inside), None).class(),
            RouteClass::NonLocal
        );
        // A link inside the directory to a file outside it.
        #[cfg(unix)]
        {
            let link = models.join("link.gguf");
            std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
            assert_eq!(class(&link), RouteClass::NonLocal);
        }
        // The production directory is the identity home's model storage,
        // never this test's directory.
        assert_eq!(
            attest_route(&flash_at(&inside)).class(),
            RouteClass::NonLocal
        );
        // The derived directory equals the Flash storage's own, for a
        // synthetic home.
        let home = TempDir::new();
        assert_eq!(
            nexus_connectors_llm::attest::flash_model_dir_for_home(&home.0).unwrap(),
            nexus_flash_infer::ModelStorage::for_home(&home.0)
                .unwrap()
                .base_dir()
                .to_path_buf()
        );
    }

    /// AMENDMENT 1, A1.5 (b): the model directory is a component-wise path
    /// prefix, never a string prefix. With in-process inference compiled in,
    /// a model file under a sibling directory whose name starts with the
    /// model directory's name (`<root>/models-other/x.gguf` beside
    /// `<root>/models`) is non-local.
    #[test]
    fn hm_m3_n3_a15b_flash_model_in_a_sibling_prefix_directory_is_non_local() {
        use nexus_connectors_llm::attest::attest_route_in;
        let root = TempDir::new();
        let models = root.0.join("models");
        let sibling = root.0.join("models-other");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        let inside = models.join("x.gguf");
        std::fs::write(&inside, b"SYNTH").unwrap();
        let beside = sibling.join("x.gguf");
        std::fs::write(&beside, b"SYNTH").unwrap();

        // Control: in-process inference is compiled in, so a regular model
        // file inside the model directory is local.
        assert_eq!(
            attest_route_in(&flash_at(&inside), Some(&models)).class(),
            RouteClass::Local
        );
        // The sibling's canonical path starts with the model directory's
        // canonical path as a string, but not component-wise.
        let (dir, model) = (
            std::fs::canonicalize(&models).unwrap(),
            std::fs::canonicalize(&beside).unwrap(),
        );
        assert!(model
            .to_string_lossy()
            .starts_with(dir.to_string_lossy().as_ref()));
        assert!(!model.starts_with(&dir));
        assert_eq!(
            attest_route_in(&flash_at(&beside), Some(&models)).class(),
            RouteClass::NonLocal
        );
        let shared = Arc::new(flash_at(&beside));
        assert_eq!(
            attest_route_in(&shared, Some(&models)).class(),
            RouteClass::NonLocal
        );
    }
}
