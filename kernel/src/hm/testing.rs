//! Test support for the kernel's unit tests (`cfg(test)` only; never
//! compiled into the product).
//!
//! - [`TestHm`]: an in-memory state database after T₀, an issuer over it
//!   with an injected registry and config, and the TEST FIXTURE loosening.
//! - [`ScriptedLlm`]: a model that dispatches through the issuer exactly as
//!   a production adapter does (§12.4 admission, §12.6 acceptance), on a
//!   transport attested `local` or `non_local`, answering from a script and
//!   recording every admitted prompt.

#![allow(dead_code)] // shared by many test modules; each uses a part

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use nexus_persistence::hm::{PolicyChangeRow, PolicyRow};
use nexus_persistence::NexusDatabase;
use uuid::Uuid;

use super::{
    run_t0_option_a, AgentRegistry, ConfigInput, ConfigReadError, ConfigReading, DerivedItem,
    DispatchContext, DispatchError, DispatchIssuer, Principal, PromptSource, RouteAttestation,
    RouteClass, TransportEvidence, Unit,
};

/// Agents registered for a test: every agent id given to `add`.
#[derive(Default)]
pub(crate) struct TestRegistry {
    agents: Mutex<HashMap<Uuid, Vec<String>>>,
    /// When set, every UUID is registered (loop tests whose supervisor mints
    /// its own agents).
    any: AtomicBool,
}

impl TestRegistry {
    pub(crate) fn add(&self, agent_id: Uuid, name: &str) {
        self.agents
            .lock()
            .unwrap()
            .insert(agent_id, vec![name.to_string()]);
    }
}

impl AgentRegistry for TestRegistry {
    fn is_registered(&self, agent_id: &Uuid) -> bool {
        self.any.load(Ordering::SeqCst) || self.agents.lock().unwrap().contains_key(agent_id)
    }

    fn names(&self, agent_id: &Uuid) -> Vec<String> {
        self.agents
            .lock()
            .unwrap()
            .get(agent_id)
            .cloned()
            .unwrap_or_else(|| vec![format!("synthetic-{agent_id}")])
    }
}

/// The untrusted config stand-in: a local-only flag per agent, or a read
/// failure.
#[derive(Default)]
pub(crate) struct TestConfig {
    local: Mutex<HashMap<Uuid, bool>>,
    pub(crate) fail: AtomicBool,
}

impl TestConfig {
    pub(crate) fn set(&self, agent_id: Uuid, local_only: bool) {
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
            content_digest: "test-config".into(),
        })
    }
}

/// A migrated in-memory state database and its issuer.
pub(crate) struct TestHm {
    pub(crate) db: Arc<NexusDatabase>,
    pub(crate) issuer: Arc<DispatchIssuer>,
    pub(crate) registry: Arc<TestRegistry>,
    pub(crate) config: Arc<TestConfig>,
}

impl TestHm {
    /// T₀ over an empty in-memory database; no agent registered.
    pub(crate) fn new() -> Self {
        Self::over(Arc::new(
            NexusDatabase::in_memory().expect("in-memory database"),
        ))
    }

    /// T₀ over `db` (an existing test database); no agent registered.
    pub(crate) fn over(db: Arc<NexusDatabase>) -> Self {
        run_t0_option_a(&db, &[]).expect("T0");
        let registry = Arc::new(TestRegistry::default());
        let config = Arc::new(TestConfig::default());
        let issuer = Arc::new(
            DispatchIssuer::new(db.clone(), registry.clone(), config.clone()).expect("issuer"),
        );
        Self {
            db,
            issuer,
            registry,
            config,
        }
    }

    /// Every UUID is a registered agent (its policy row is created
    /// restrictive at its first resolution).
    pub(crate) fn register_every_agent(self) -> Self {
        self.registry.any.store(true, Ordering::SeqCst);
        self
    }

    /// A newly registered agent: local-only (default restrictive).
    pub(crate) fn agent(&self, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.registry.add(id, name);
        id
    }

    /// Whether a unit was ever started for `agent_id`: its route policy is
    /// resolved first, and the first resolution creates its row.
    pub(crate) fn unit_started(&self, agent_id: &str) -> bool {
        self.db
            .hm()
            .read(|tx| tx.policy_row_raw(agent_id))
            .expect("policy row read")
            .is_some()
    }

    /// A newly registered agent the Owner loosened (TEST FIXTURE).
    pub(crate) fn open_agent(&self, name: &str) -> Uuid {
        let id = self.agent(name);
        self.loosen(id);
        id
    }

    /// TEST FIXTURE, not product code (mission A1.9): an Owner loosening
    /// committed through the store, with its `owner_loosen` history row.
    /// The policy row is created restrictive first if it is missing.
    pub(crate) fn loosen(&self, agent_id: Uuid) {
        self.issuer
            .resolve_agent_route_policy(agent_id)
            .expect("registered agent");
        let agent = agent_id.to_string();
        self.db
            .hm()
            .immediate(|tx| {
                let raw = tx.policy_row_raw(&agent)?.expect("policy row");
                let generation = raw.policy_generation.unwrap() + 1;
                tx.insert_policy_change(&PolicyChangeRow {
                    agent_id: agent.clone(),
                    policy_generation: generation,
                    local_only: 0,
                    basis: "owner_confirmed".into(),
                    cause: "owner_loosen".into(),
                })?;
                tx.update_policy(&PolicyRow {
                    agent_id: agent.clone(),
                    local_only: 0,
                    policy_generation: generation,
                    revocation_seq: raw.revocation_seq.unwrap(),
                    basis: "owner_confirmed".into(),
                })?;
                Ok(())
            })
            .expect("TEST FIXTURE owner loosen");
    }

    /// A goal unit of `agent_id`.
    pub(crate) fn goal(&self, agent_id: Uuid) -> DispatchContext {
        self.issuer
            .begin_unit(
                Principal::Agent(agent_id),
                Unit::Goal {
                    goal_id: Uuid::new_v4(),
                },
            )
            .expect("goal unit")
    }

    /// A plain `System` request context (classes SY, XS).
    pub(crate) fn system(&self) -> DispatchContext {
        self.issuer
            .begin_unit(
                Principal::System("test"),
                Unit::Request {
                    request_id: Uuid::new_v4(),
                },
            )
            .expect("system unit")
    }
}

/// The attestation of a test transport of route class `route`.
pub(crate) fn attest(route: RouteClass) -> RouteAttestation {
    RouteAttestation::from_transport_evidence(match route {
        RouteClass::Local => TransportEvidence::VerifiedLoopbackOllama,
        RouteClass::NonLocal => TransportEvidence::Unattested,
    })
}

/// A reply computed from the admitted prompt.
type ReplyFn = Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// A scripted model: every dispatch goes through `DispatchIssuer::dispatch`
/// on a transport attested `route`; replies come from the script in order
/// (the last one repeats), or from `reply_with`.
pub(crate) struct ScriptedLlm {
    issuer: Arc<DispatchIssuer>,
    route: RouteClass,
    script: Mutex<VecDeque<Result<String, String>>>,
    last: Mutex<Option<Result<String, String>>>,
    reply_with: Option<ReplyFn>,
    /// Every admitted prompt, in order.
    pub(crate) prompts: Arc<Mutex<Vec<String>>>,
}

impl ScriptedLlm {
    pub(crate) fn new(issuer: &Arc<DispatchIssuer>, route: RouteClass) -> Self {
        Self {
            issuer: issuer.clone(),
            route,
            script: Mutex::new(VecDeque::new()),
            last: Mutex::new(None),
            reply_with: None,
            prompts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Replies `text` to every dispatch.
    pub(crate) fn replying(issuer: &Arc<DispatchIssuer>, route: RouteClass, text: &str) -> Self {
        Self::new(issuer, route).then(text)
    }

    /// Adds `text` as the next reply.
    pub(crate) fn then(self, text: &str) -> Self {
        self.script.lock().unwrap().push_back(Ok(text.to_string()));
        self
    }

    /// Adds a transport failure as the next reply.
    pub(crate) fn then_fail(self, error: &str) -> Self {
        self.script
            .lock()
            .unwrap()
            .push_back(Err(error.to_string()));
        self
    }

    /// Replies with `f(prompt)` to every dispatch.
    pub(crate) fn answering(
        issuer: &Arc<DispatchIssuer>,
        route: RouteClass,
        f: impl Fn(&str) -> String + Send + Sync + 'static,
    ) -> Self {
        Self::answering_or_failing(issuer, route, move |prompt| Ok(f(prompt)))
    }

    /// Replies with `f(prompt)`, or fails the transport with its error.
    pub(crate) fn answering_or_failing(
        issuer: &Arc<DispatchIssuer>,
        route: RouteClass,
        f: impl Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        let mut llm = Self::new(issuer, route);
        llm.reply_with = Some(Box::new(f));
        llm
    }

    fn next_reply(&self, prompt: &str) -> Result<String, String> {
        if let Some(f) = &self.reply_with {
            return f(prompt);
        }
        let mut script = self.script.lock().unwrap();
        let mut last = self.last.lock().unwrap();
        if let Some(next) = script.pop_front() {
            *last = Some(next.clone());
            return next;
        }
        last.clone()
            .unwrap_or_else(|| Err("no scripted reply".to_string()))
    }

    /// The single dispatch of every trait method.
    pub(crate) fn ask(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, DispatchError> {
        let route = self.route;
        self.issuer.dispatch(
            ctx,
            &mut |c: &mut DispatchContext| prompt(c).map(|env| (env, (), attest(route))),
            &mut |(), text: &str| {
                self.prompts.lock().unwrap().push(text.to_string());
                self.next_reply(text)
            },
        )
    }

    /// The admitted prompts so far.
    pub(crate) fn sent(&self) -> Vec<String> {
        self.prompts.lock().unwrap().clone()
    }
}

// ── The six model traits (§11.4) over the issuer ────────────────────────────

impl crate::cognitive::PlannerLlm for ScriptedLlm {
    fn plan_query(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, crate::errors::AgentError> {
        self.ask(ctx, prompt)
            .map_err(|e| crate::errors::AgentError::SupervisorError(e.to_string()))
    }
}

impl crate::cognitive::LlmQueryHandler for ScriptedLlm {
    fn query(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String> {
        self.ask(ctx, prompt).map_err(|e| e.to_string())
    }
}

impl crate::cognitive::EvolutionLlm for ScriptedLlm {
    fn optimize_prompt(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, String> {
        self.ask(ctx, prompt).map_err(|e| e.to_string())
    }
}

impl crate::cognitive::HivemindLlm for ScriptedLlm {
    fn decompose(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, crate::errors::AgentError> {
        self.ask(ctx, prompt)
            .map_err(|e| crate::errors::AgentError::SupervisorError(e.to_string()))
    }

    fn merge(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
    ) -> Result<DerivedItem, crate::errors::AgentError> {
        self.ask(ctx, prompt)
            .map_err(|e| crate::errors::AgentError::SupervisorError(e.to_string()))
    }
}

// ── Stores over the synthetic database ──────────────────────────────────────

/// The cognitive `MemoryStore` over the synthetic state database (its reads
/// and bookkeeping; content writes go through the labelled API).
pub(crate) struct DbMemoryStore {
    pub(crate) db: Arc<NexusDatabase>,
}

impl crate::cognitive::MemoryStore for DbMemoryStore {
    fn save_memory(
        &self,
        agent_id: &str,
        memory_type: &str,
        key: &str,
        value_json: &str,
    ) -> Result<(), String> {
        use nexus_persistence::StateStore;
        self.db
            .save_memory(agent_id, memory_type, key, value_json)
            .map_err(|e| e.to_string())
    }

    fn load_memories(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<crate::cognitive::MemoryEntry>, String> {
        use nexus_persistence::StateStore;
        Ok(self
            .db
            .load_memories(agent_id, memory_type, limit)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| crate::cognitive::MemoryEntry {
                id: r.id,
                agent_id: r.agent_id,
                memory_type: r.memory_type,
                key: r.key,
                value_json: r.value_json,
                relevance_score: r.relevance_score,
                access_count: r.access_count,
                created_at: r.created_at,
                last_accessed: r.last_accessed,
            })
            .collect())
    }

    fn touch_memory(&self, id: i64) -> Result<(), String> {
        use nexus_persistence::StateStore;
        self.db.touch_memory(id).map_err(|e| e.to_string())
    }

    fn decay_memories(&self, agent_id: &str, decay_factor: f64) -> Result<(), String> {
        use nexus_persistence::StateStore;
        self.db
            .decay_memories(agent_id, decay_factor)
            .map_err(|e| e.to_string())
    }
}

// ── The kernel's test-only capture seam (mission P3-C11-HM-M4 N4, AD-6) ────

/// One request the issuer handed to a transport on this thread.
#[derive(Debug, Clone)]
pub(crate) struct Captured {
    pub(crate) class: &'static str,
    pub(crate) admission_id: Option<String>,
    pub(crate) prompt: String,
}

thread_local! {
    static CAPTURED: std::cell::RefCell<Vec<Captured>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Records one outbound request at the kernel's transport boundary.
pub(crate) fn capture(transmission: &super::Transmission, prompt: &str) {
    CAPTURED.with(|c| {
        c.borrow_mut().push(Captured {
            class: transmission.class(),
            admission_id: transmission.admission_id().map(str::to_string),
            prompt: prompt.to_string(),
        })
    });
}

/// The requests captured on this thread so far (and forgets them).
pub(crate) fn take_captured() -> Vec<Captured> {
    CAPTURED.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Every row of a query whose columns are all TEXT or NULL (CAST other
/// columns in the query); NULL reads as "NULL".
/// The `source_kind` of the label of row `record_id` of `table`, or
/// `unlabelled` (read through the store, never by naming its tables).
pub(crate) fn label_source(db: &NexusDatabase, table: &str, record_id: &str) -> String {
    db.hm()
        .read(|tx| tx.label_row(table, record_id))
        .unwrap()
        .map(|label| label.source_kind)
        .unwrap_or_else(|| "unlabelled".to_string())
}

/// The origins of the labels of `agent_id`'s rows of `table` (read through
/// the store, never by naming its tables).
pub(crate) fn label_origins(db: &NexusDatabase, table: &str, agent_id: &str) -> Vec<String> {
    db.hm()
        .read(|tx| tx.label_rows())
        .unwrap()
        .into_iter()
        .filter(|label| label.record_table == table && label.agent_id == agent_id)
        .map(|label| label.origin)
        .collect()
}

/// How many events of `kind` name `agent_id` (read through the store,
/// never by naming its tables).
pub(crate) fn event_count(db: &NexusDatabase, kind: &str, agent_id: &str) -> usize {
    db.hm()
        .read(|tx| tx.event_kinds())
        .unwrap()
        .into_iter()
        .filter(|(k, a)| k == kind && a.as_deref() == Some(agent_id))
        .count()
}

pub(crate) fn rows(db: &NexusDatabase, sql: &str) -> Vec<Vec<String>> {
    db.with_transaction(|tx| {
        let mut stmt = tx.prepare(sql)?;
        let n = stmt.column_count();
        let mapped = stmt.query_map([], |r| {
            let mut row = Vec::with_capacity(n);
            for i in 0..n {
                row.push(
                    r.get::<_, Option<String>>(i)?
                        .unwrap_or_else(|| "NULL".to_string()),
                );
            }
            Ok(row)
        })?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row?);
        }
        Ok(out)
    })
    .unwrap_or_else(|e| panic!("{e} for: {sql}"))
}

/// Runs `sql` in a transaction (TEST FIXTURE statements).
pub(crate) fn exec(db: &NexusDatabase, sql: &str) {
    db.with_transaction(|tx| {
        tx.execute_batch(sql)?;
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{e} for: {sql}"));
}
