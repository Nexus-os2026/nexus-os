//! Director — decomposes a user intent into an `ExecutionDag`.
//!
//! Default type name is [`Director`]. See the `nexus-conductor` crate
//! investigation note in Phase 1.G below; if semantic overlap is found, the
//! type is re-exported as [`SwarmDirector`].

use crate::budget::Budget;
use crate::dag::{DagNode, DagNodeStatus, ExecutionDag};
use crate::error::SwarmError;
use crate::profile::TaskProfile;
use crate::provider::{InvokeRequest, Provider};
use crate::registry::CapabilityRegistry;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

/// JSON schema the Director expects back from its planning provider.
#[derive(Debug, Clone, Deserialize)]
pub struct PlanSchema {
    pub nodes: Vec<PlanNode>,
    pub edges: Vec<PlanEdge>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanNode {
    pub id: String,
    pub capability_id: String,
    pub profile: TaskProfile,
    #[serde(default)]
    pub inputs: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanEdge {
    pub from: String,
    pub to: String,
}

pub struct Director {
    planner: Arc<dyn Provider>,
    planner_model: String,
}

/// Alias retained for forward-compatibility with the `nexus-conductor`
/// orchestration crate. See `directors.rs` header for investigation notes —
/// if `nexus-conductor` is renamed to implement the swarm Director contract,
/// this alias is dropped.
pub type SwarmDirector = Director;

impl Director {
    pub fn new(planner: Arc<dyn Provider>, planner_model: String) -> Self {
        Self {
            planner,
            planner_model,
        }
    }

    /// Produce an ExecutionDag for the intent. One retry on malformed JSON;
    /// second failure → `DirectorParse`.
    pub async fn plan(
        &self,
        intent: &str,
        registry: &CapabilityRegistry,
        _budget: &Budget,
    ) -> Result<ExecutionDag, SwarmError> {
        let prompt = build_prompt(intent, registry);

        for attempt in 0..2 {
            let req = InvokeRequest {
                model_id: self.planner_model.clone(),
                prompt: if attempt == 0 {
                    prompt.clone()
                } else {
                    format!(
                        "{prompt}\n\nYour previous response was not valid JSON matching the schema.\n\
                         Return ONLY a JSON object with top-level `nodes` and `edges` arrays.\n\
                         No prose. No markdown fences."
                    )
                },
                max_tokens: 2048,
                temperature: Some(0.2),
                metadata: Value::Null,
            };

            let resp = self
                .planner
                .invoke(req)
                .await
                .map_err(|e| SwarmError::DirectorUnavailable(e.to_string()))?;

            match parse_plan(&resp.text) {
                Ok(plan) => return build_dag(plan, registry),
                Err(e) if attempt == 0 => {
                    tracing::warn!(
                        target: "nexus_swarm::director",
                        "planning attempt 1 failed: {e}; retrying once"
                    );
                    continue;
                }
                Err(e) => return Err(SwarmError::DirectorParse(e)),
            }
        }
        Err(SwarmError::DirectorParse(
            "exhausted retries without a valid plan".into(),
        ))
    }
}

fn build_prompt(intent: &str, registry: &CapabilityRegistry) -> String {
    let capabilities: Vec<Value> = registry
        .list()
        .into_iter()
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "name": d.name,
                "role": d.role,
                "max_parallel": d.max_parallel,
                "stub": d.is_stub(),
                "todo_reason": d.todo_reason,
            })
        })
        .collect();

    format!(
        "You are the Director of an autonomous agent swarm. Decompose the user intent \n\
         into a DAG of capability invocations.\n\n\
         Available capabilities (DO NOT select any with stub=true):\n{caps}\n\n\
         User intent: {intent}\n\n\
         Respond with JSON matching this schema:\n\
         {{\n  \"nodes\": [{{ \"id\": string, \"capability_id\": string, \"profile\": TaskProfile, \"inputs\": object }}, ...],\n  \"edges\": [{{ \"from\": string, \"to\": string }}, ...]\n}}\n\n\
         TaskProfile example: {{ \"reasoning\": \"Medium\", \"tool_use\": \"Basic\", \"latency\": \"Batch\", \"context\": \"Medium\", \"privacy\": \"Public\", \"cost\": \"Low\" }}\n\
         Return ONLY JSON. No prose, no markdown fences.",
        caps = serde_json::to_string_pretty(&capabilities).unwrap_or_else(|_| "[]".into()),
        intent = intent,
    )
}

fn parse_plan(text: &str) -> Result<PlanSchema, String> {
    let cleaned = strip_fences(text);
    serde_json::from_str(&cleaned).map_err(|e| format!("{e}"))
}

fn strip_fences(s: &str) -> String {
    let s = s.trim();
    // Strip ```json ... ``` fences if present.
    if let Some(rest) = s.strip_prefix("```json") {
        if let Some(body) = rest.trim_start().strip_suffix("```") {
            return body.trim().to_string();
        }
    }
    if let Some(rest) = s.strip_prefix("```") {
        if let Some(body) = rest.trim_start().strip_suffix("```") {
            return body.trim().to_string();
        }
    }
    s.to_string()
}

fn build_dag(plan: PlanSchema, registry: &CapabilityRegistry) -> Result<ExecutionDag, SwarmError> {
    let mut dag = ExecutionDag::new();
    for n in &plan.nodes {
        // Reject references to unknown or stub capabilities.
        let cap = registry
            .get(&n.capability_id)
            .ok_or_else(|| SwarmError::RegistryMiss(n.capability_id.clone()))?;
        if cap.descriptor().is_stub() {
            return Err(SwarmError::DirectorParse(format!(
                "plan references stub capability `{}`",
                n.capability_id
            )));
        }
        dag.add_node(DagNode {
            id: n.id.clone(),
            capability_id: n.capability_id.clone(),
            profile: n.profile,
            inputs: n.inputs.clone(),
            status: DagNodeStatus::Pending,
        })?;
    }
    for e in &plan.edges {
        dag.add_edge(&e.from, &e.to)?;
    }
    Ok(dag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
    use crate::events::ProviderHealth;
    use crate::profile::{CostClass, PrivacyClass};
    use crate::provider::{
        InvokeRequest, InvokeResponse, ModelDescriptor, ProviderCapabilities, ProviderError,
    };
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct CannedProvider {
        id: String,
        responses: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Provider for CannedProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                models: vec![ModelDescriptor {
                    id: "planner".into(),
                    param_count_b: None,
                    tier: crate::profile::ReasoningTier::Heavy,
                    context_window: 32000,
                }],
                supports_tool_use: false,
                supports_streaming: false,
                max_context: 32000,
                cost_class: CostClass::Standard,
                privacy_class: PrivacyClass::Public,
            }
        }
        async fn health_check(&self) -> ProviderHealth {
            unreachable!()
        }
        async fn invoke(&self, _r: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
            let mut q = self.responses.lock().unwrap();
            let text = q.remove(0);
            Ok(InvokeResponse {
                text,
                tokens_in: 1,
                tokens_out: 1,
                cost_cents: 0,
                latency_ms: 1,
                model_id: "planner".into(),
            })
        }
    }

    fn test_registry() -> CapabilityRegistry {
        struct Cap;
        #[async_trait]
        impl SwarmCapability for Cap {
            fn descriptor(&self) -> AgentCapabilityDescriptor {
                AgentCapabilityDescriptor {
                    id: "artisan".into(),
                    name: "Artisan".into(),
                    role: "coder".into(),
                    task_profile_default: TaskProfile::local_light(),
                    input_schema: serde_json::json!({}),
                    output_schema: serde_json::json!({}),
                    max_parallel: 1,
                    cost_class: CostClass::Free,
                    todo_reason: None,
                }
            }
            async fn run(&self, _: CapabilityInvocation) -> Result<serde_json::Value, SwarmError> {
                Ok(serde_json::json!({}))
            }
        }
        let mut r = CapabilityRegistry::new();
        r.register(Arc::new(Cap));
        r
    }

    #[tokio::test]
    async fn valid_plan_produces_dag() {
        let plan = r#"{"nodes":[{"id":"n1","capability_id":"artisan","profile":{"reasoning":"Light","tool_use":"None","latency":"Batch","context":"Small","privacy":"StrictLocal","cost":"Free"},"inputs":{}}],"edges":[]}"#;
        let provider = Arc::new(CannedProvider {
            id: "planner".into(),
            responses: Mutex::new(vec![plan.into()]),
        });
        let director = Director::new(provider, "planner".into());
        let registry = test_registry();
        let dag = director
            .plan(
                "write hello world",
                &registry,
                &Budget::unlimited_for_tests(),
            )
            .await
            .unwrap();
        assert_eq!(dag.node_count(), 1);
    }

    #[tokio::test]
    async fn malformed_then_valid_succeeds_on_retry() {
        let plan = r#"{"nodes":[{"id":"n1","capability_id":"artisan","profile":{"reasoning":"Light","tool_use":"None","latency":"Batch","context":"Small","privacy":"StrictLocal","cost":"Free"},"inputs":{}}],"edges":[]}"#;
        let provider = Arc::new(CannedProvider {
            id: "planner".into(),
            responses: Mutex::new(vec!["not json at all".into(), plan.into()]),
        });
        let director = Director::new(provider, "planner".into());
        let registry = test_registry();
        let dag = director
            .plan("x", &registry, &Budget::unlimited_for_tests())
            .await
            .unwrap();
        assert_eq!(dag.node_count(), 1);
    }

    #[tokio::test]
    async fn two_malformed_responses_fail() {
        let provider = Arc::new(CannedProvider {
            id: "planner".into(),
            responses: Mutex::new(vec!["nope".into(), "still nope".into()]),
        });
        let director = Director::new(provider, "planner".into());
        let registry = test_registry();
        let err = director
            .plan("x", &registry, &Budget::unlimited_for_tests())
            .await
            .unwrap_err();
        assert!(matches!(err, SwarmError::DirectorParse(_)));
    }

    #[test]
    fn strip_fences_drops_json_block() {
        assert_eq!(strip_fences("```json\n{}\n```"), "{}");
        assert_eq!(strip_fences("```\n{}\n```"), "{}");
        assert_eq!(strip_fences("{}"), "{}");
    }

    #[test]
    fn plan_referencing_stub_is_rejected() {
        struct Stub;
        #[async_trait]
        impl SwarmCapability for Stub {
            fn descriptor(&self) -> AgentCapabilityDescriptor {
                AgentCapabilityDescriptor {
                    id: "scout".into(),
                    name: "Scout".into(),
                    role: "stub".into(),
                    task_profile_default: TaskProfile::local_light(),
                    input_schema: serde_json::json!({}),
                    output_schema: serde_json::json!({}),
                    max_parallel: 0,
                    cost_class: CostClass::Free,
                    todo_reason: Some("Scout crate missing"),
                }
            }
            async fn run(&self, _: CapabilityInvocation) -> Result<serde_json::Value, SwarmError> {
                Err(SwarmError::RegistryMiss("scout-stub".into()))
            }
        }
        let mut r = CapabilityRegistry::new();
        r.register(Arc::new(Stub));
        let plan_json = r#"{"nodes":[{"id":"n1","capability_id":"scout","profile":{"reasoning":"Light","tool_use":"None","latency":"Batch","context":"Small","privacy":"StrictLocal","cost":"Free"},"inputs":{}}],"edges":[]}"#;
        let plan: PlanSchema = serde_json::from_str(plan_json).unwrap();
        let err = build_dag(plan, &r).unwrap_err();
        assert!(matches!(err, SwarmError::DirectorParse(_)));
    }
}
