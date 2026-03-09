//! Speculative execution engine — shadow simulation before high-risk actions.
//!
//! When a high-risk operation requires approval (Tier2+), the kernel can fork
//! a shadow state, simulate the action, and present predicted outcomes to the
//! human reviewer alongside the approval request.

use crate::audit::AuditTrail;
use crate::autonomy::AutonomyLevel;
use crate::consent::{GovernedOperation, HitlTier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Risk Level — synthesised from HitlTier + AutonomyLevel
// ---------------------------------------------------------------------------

/// Human-readable risk classification derived from governance tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    /// Derive a risk level from the HITL tier and autonomy level of the agent.
    pub fn from_governance(tier: HitlTier, autonomy: AutonomyLevel) -> Self {
        match (tier, autonomy) {
            (HitlTier::Tier3, _) => RiskLevel::Critical,
            (HitlTier::Tier2, AutonomyLevel::L4 | AutonomyLevel::L5) => RiskLevel::High,
            (HitlTier::Tier2, _) => RiskLevel::Medium,
            (HitlTier::Tier1, _) => RiskLevel::Low,
            (HitlTier::Tier0, _) => RiskLevel::Low,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Predicted change types
// ---------------------------------------------------------------------------

/// The kind of change a simulated action would make to a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Create,
    Modify,
    Delete,
}

/// A predicted file-system change from a simulated action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_kind: ChangeKind,
    pub size_before: u64,
    pub size_after: u64,
    pub preview: String,
}

/// A predicted network call from a simulated action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkCall {
    pub target: String,
    pub method: String,
    pub estimated_bytes: u64,
}

/// A predicted data modification from a simulated action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataModification {
    pub resource: String,
    pub description: String,
}

/// A single predicted action within a simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionPreview {
    FileChange(FileChange),
    NetworkCall(NetworkCall),
    DataModification(DataModification),
    LlmCall {
        prompt_len: usize,
        max_tokens: u32,
        estimated_fuel: u64,
    },
}

// ---------------------------------------------------------------------------
// Resource impact
// ---------------------------------------------------------------------------

/// Estimated resource consumption of a simulated action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResourceImpact {
    pub disk_bytes_delta: i64,
    pub fuel_cost: u64,
    pub llm_calls: u32,
    pub network_calls: u32,
    pub file_operations: u32,
}

// ---------------------------------------------------------------------------
// Simulation result
// ---------------------------------------------------------------------------

/// The complete output of a speculative simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationResult {
    pub simulation_id: Uuid,
    pub agent_id: Uuid,
    pub operation: GovernedOperation,
    pub predicted_changes: Vec<ActionPreview>,
    pub resource_impact: ResourceImpact,
    pub risk_level: RiskLevel,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// State snapshot
// ---------------------------------------------------------------------------

/// A frozen snapshot of agent state at the moment of fork.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub agent_id: Uuid,
    pub fuel_remaining: u64,
    pub autonomy_level: AutonomyLevel,
    pub capabilities: Vec<String>,
    pub audit_event_count: usize,
    pub snapshot_id: Uuid,
}

// ---------------------------------------------------------------------------
// Speculative engine
// ---------------------------------------------------------------------------

/// Shadow simulation engine that forks state and predicts action outcomes.
#[derive(Debug, Default)]
pub struct SpeculativeEngine {
    snapshots: HashMap<Uuid, StateSnapshot>,
    results: HashMap<Uuid, SimulationResult>,
    pending_for_request: HashMap<String, Uuid>,
}

impl SpeculativeEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fork agent state into a snapshot for simulation.
    pub fn fork_state(
        &mut self,
        agent_id: Uuid,
        fuel_remaining: u64,
        autonomy_level: AutonomyLevel,
        capabilities: Vec<String>,
        audit_event_count: usize,
    ) -> StateSnapshot {
        let snapshot = StateSnapshot {
            agent_id,
            fuel_remaining,
            autonomy_level,
            capabilities,
            audit_event_count,
            snapshot_id: Uuid::new_v4(),
        };
        self.snapshots.insert(snapshot.snapshot_id, snapshot.clone());
        snapshot
    }

    /// Simulate an action against a forked snapshot.
    ///
    /// This is a dry-run cost/impact estimator. Since file I/O and LLM calls
    /// are mock in AgentContext, the simulation predicts *what would be called*
    /// and the fuel cost, not actual content diffs.
    pub fn simulate(
        &mut self,
        snapshot: &StateSnapshot,
        operation: GovernedOperation,
        tier: HitlTier,
        payload: &[u8],
        audit: &mut AuditTrail,
    ) -> SimulationResult {
        let risk_level = RiskLevel::from_governance(tier, snapshot.autonomy_level);
        let (predicted_changes, resource_impact) =
            Self::predict_impact(operation, payload, snapshot.fuel_remaining);

        let summary = Self::build_summary(operation, &resource_impact, risk_level);

        let result = SimulationResult {
            simulation_id: Uuid::new_v4(),
            agent_id: snapshot.agent_id,
            operation,
            predicted_changes,
            resource_impact,
            risk_level,
            summary,
        };

        audit.append_event(
            snapshot.agent_id,
            crate::audit::EventType::ToolCall,
            serde_json::json!({
                "action": "speculative_simulation",
                "simulation_id": result.simulation_id,
                "operation": operation.as_str(),
                "risk_level": risk_level.as_str(),
                "fuel_cost": result.resource_impact.fuel_cost,
            }),
        );

        self.results.insert(result.simulation_id, result.clone());
        result
    }

    /// Associate a simulation result with a consent approval request.
    pub fn attach_to_request(&mut self, request_id: &str, simulation_id: Uuid) {
        self.pending_for_request
            .insert(request_id.to_string(), simulation_id);
    }

    /// Retrieve the simulation result attached to an approval request.
    pub fn get_for_request(&self, request_id: &str) -> Option<&SimulationResult> {
        self.pending_for_request
            .get(request_id)
            .and_then(|sid| self.results.get(sid))
    }

    /// Get a simulation result by its ID.
    pub fn get_result(&self, simulation_id: &Uuid) -> Option<&SimulationResult> {
        self.results.get(simulation_id)
    }

    /// Commit: the action was approved — clean up simulation state.
    pub fn commit(&mut self, request_id: &str) {
        if let Some(sid) = self.pending_for_request.remove(request_id) {
            self.results.remove(&sid);
            // snapshot may have been used for multiple simulations, leave it
        }
    }

    /// Rollback: the action was rejected — clean up simulation state.
    pub fn rollback(&mut self, request_id: &str, audit: &mut AuditTrail) {
        if let Some(sid) = self.pending_for_request.remove(request_id) {
            if let Some(result) = self.results.remove(&sid) {
                audit.append_event(
                    result.agent_id,
                    crate::audit::EventType::UserAction,
                    serde_json::json!({
                        "action": "speculative_rollback",
                        "simulation_id": sid,
                        "operation": result.operation.as_str(),
                        "reason": "user_rejected",
                    }),
                );
            }
        }
    }

    /// List all pending simulation results.
    pub fn pending_simulations(&self) -> Vec<(&str, &SimulationResult)> {
        self.pending_for_request
            .iter()
            .filter_map(|(req_id, sid)| {
                self.results.get(sid).map(|r| (req_id.as_str(), r))
            })
            .collect()
    }

    /// Whether an operation at a given tier should trigger automatic simulation.
    pub fn should_simulate(tier: HitlTier) -> bool {
        matches!(tier, HitlTier::Tier2 | HitlTier::Tier3)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn predict_impact(
        operation: GovernedOperation,
        payload: &[u8],
        fuel_remaining: u64,
    ) -> (Vec<ActionPreview>, ResourceImpact) {
        let mut changes = Vec::new();
        let mut impact = ResourceImpact::default();

        match operation {
            GovernedOperation::ToolCall => {
                // Parse payload as JSON to extract action details if possible
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(payload) {
                    if let Some(action) = value.get("action").and_then(|a| a.as_str()) {
                        match action {
                            "llm_query" => {
                                let prompt_len =
                                    value.get("prompt_len").and_then(|v| v.as_u64()).unwrap_or(100)
                                        as usize;
                                let max_tokens = value
                                    .get("max_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(1024)
                                    as u32;
                                let fuel = 10;
                                changes.push(ActionPreview::LlmCall {
                                    prompt_len,
                                    max_tokens,
                                    estimated_fuel: fuel,
                                });
                                impact.fuel_cost += fuel;
                                impact.llm_calls += 1;
                            }
                            "write_file" => {
                                let path = value
                                    .get("path")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let size = value
                                    .get("size")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                changes.push(ActionPreview::FileChange(FileChange {
                                    path,
                                    change_kind: ChangeKind::Create,
                                    size_before: 0,
                                    size_after: size,
                                    preview: "[simulated write]".to_string(),
                                }));
                                impact.fuel_cost += 8;
                                impact.disk_bytes_delta += size as i64;
                                impact.file_operations += 1;
                            }
                            "read_file" => {
                                impact.fuel_cost += 2;
                                impact.file_operations += 1;
                            }
                            _ => {
                                impact.fuel_cost += 5;
                            }
                        }
                    }
                }
            }
            GovernedOperation::TerminalCommand => {
                let cmd = std::str::from_utf8(payload).unwrap_or("[binary]");
                changes.push(ActionPreview::DataModification(DataModification {
                    resource: "terminal".to_string(),
                    description: format!("Execute command: {}", truncate(cmd, 100)),
                }));
                impact.fuel_cost += 10;
            }
            GovernedOperation::SocialPostPublish => {
                let content = std::str::from_utf8(payload).unwrap_or("[binary]");
                changes.push(ActionPreview::NetworkCall(NetworkCall {
                    target: "social_media_api".to_string(),
                    method: "POST".to_string(),
                    estimated_bytes: payload.len() as u64,
                }));
                changes.push(ActionPreview::DataModification(DataModification {
                    resource: "social_media".to_string(),
                    description: format!("Publish post: {}", truncate(content, 80)),
                }));
                impact.fuel_cost += 15;
                impact.network_calls += 1;
            }
            GovernedOperation::SelfMutationApply => {
                changes.push(ActionPreview::DataModification(DataModification {
                    resource: "agent_code".to_string(),
                    description: "Apply self-mutation to agent configuration or code".to_string(),
                }));
                impact.fuel_cost += 20;
            }
            GovernedOperation::MultiAgentOrchestrate => {
                changes.push(ActionPreview::DataModification(DataModification {
                    resource: "multi_agent".to_string(),
                    description: "Orchestrate action across multiple agents".to_string(),
                }));
                impact.fuel_cost += 10;
            }
            GovernedOperation::DistributedEnable => {
                changes.push(ActionPreview::NetworkCall(NetworkCall {
                    target: "cluster_nodes".to_string(),
                    method: "REPLICATE".to_string(),
                    estimated_bytes: payload.len() as u64,
                }));
                changes.push(ActionPreview::DataModification(DataModification {
                    resource: "distributed_state".to_string(),
                    description: "Enable distributed operation across nodes".to_string(),
                }));
                impact.fuel_cost += 25;
                impact.network_calls += 1;
            }
        }

        // Check if fuel is sufficient
        if impact.fuel_cost > fuel_remaining {
            changes.push(ActionPreview::DataModification(DataModification {
                resource: "fuel".to_string(),
                description: format!(
                    "WARNING: Estimated fuel cost ({}) exceeds remaining fuel ({})",
                    impact.fuel_cost, fuel_remaining
                ),
            }));
        }

        (changes, impact)
    }

    fn build_summary(
        operation: GovernedOperation,
        impact: &ResourceImpact,
        risk: RiskLevel,
    ) -> String {
        let op_desc = match operation {
            GovernedOperation::ToolCall => "tool call",
            GovernedOperation::TerminalCommand => "terminal command execution",
            GovernedOperation::SocialPostPublish => "social media post publication",
            GovernedOperation::SelfMutationApply => "agent self-mutation",
            GovernedOperation::MultiAgentOrchestrate => "multi-agent orchestration",
            GovernedOperation::DistributedEnable => "distributed operation enablement",
        };
        format!(
            "Simulated {op_desc}: {risk} risk, estimated {fuel} fuel, {files} file ops, {net} network calls, {llm} LLM calls",
            fuel = impact.fuel_cost,
            files = impact.file_operations,
            net = impact.network_calls,
            llm = impact.llm_calls,
        )
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditTrail;
    use crate::autonomy::AutonomyLevel;
    use crate::consent::{GovernedOperation, HitlTier};

    #[test]
    fn risk_level_from_governance_tier0_is_low() {
        assert_eq!(
            RiskLevel::from_governance(HitlTier::Tier0, AutonomyLevel::L0),
            RiskLevel::Low
        );
    }

    #[test]
    fn risk_level_from_governance_tier1_is_low() {
        assert_eq!(
            RiskLevel::from_governance(HitlTier::Tier1, AutonomyLevel::L1),
            RiskLevel::Low
        );
    }

    #[test]
    fn risk_level_from_governance_tier2_medium() {
        assert_eq!(
            RiskLevel::from_governance(HitlTier::Tier2, AutonomyLevel::L2),
            RiskLevel::Medium
        );
    }

    #[test]
    fn risk_level_from_governance_tier2_high_autonomy() {
        assert_eq!(
            RiskLevel::from_governance(HitlTier::Tier2, AutonomyLevel::L4),
            RiskLevel::High
        );
        assert_eq!(
            RiskLevel::from_governance(HitlTier::Tier2, AutonomyLevel::L5),
            RiskLevel::High
        );
    }

    #[test]
    fn risk_level_from_governance_tier3_critical() {
        assert_eq!(
            RiskLevel::from_governance(HitlTier::Tier3, AutonomyLevel::L0),
            RiskLevel::Critical
        );
        assert_eq!(
            RiskLevel::from_governance(HitlTier::Tier3, AutonomyLevel::L5),
            RiskLevel::Critical
        );
    }

    #[test]
    fn risk_level_display() {
        assert_eq!(RiskLevel::Low.to_string(), "low");
        assert_eq!(RiskLevel::Critical.to_string(), "critical");
    }

    #[test]
    fn fork_state_creates_snapshot() {
        let mut engine = SpeculativeEngine::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec!["llm.query".into()], 10);
        assert_eq!(snap.agent_id, agent_id);
        assert_eq!(snap.fuel_remaining, 5000);
        assert_eq!(snap.autonomy_level, AutonomyLevel::L2);
        assert!(engine.snapshots.contains_key(&snap.snapshot_id));
    }

    #[test]
    fn simulate_terminal_command_produces_result() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::TerminalCommand,
            HitlTier::Tier2,
            b"rm -rf /tmp/test",
            &mut audit,
        );

        assert_eq!(result.operation, GovernedOperation::TerminalCommand);
        assert_eq!(result.risk_level, RiskLevel::Medium);
        assert!(!result.predicted_changes.is_empty());
        assert!(result.resource_impact.fuel_cost > 0);
        assert!(result.summary.contains("terminal command"));
        assert!(engine.results.contains_key(&result.simulation_id));
    }

    #[test]
    fn simulate_social_post_shows_network_call() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::SocialPostPublish,
            HitlTier::Tier2,
            b"Hello world post content",
            &mut audit,
        );

        assert_eq!(result.resource_impact.network_calls, 1);
        let has_network = result.predicted_changes.iter().any(|c| {
            matches!(c, ActionPreview::NetworkCall(_))
        });
        assert!(has_network);
    }

    #[test]
    fn simulate_self_mutation_is_critical() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L4, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::SelfMutationApply,
            HitlTier::Tier3,
            b"mutation payload",
            &mut audit,
        );

        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn simulate_tool_call_with_llm_query_payload() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec!["llm.query".into()], 0);

        let payload = serde_json::json!({
            "action": "llm_query",
            "prompt_len": 200,
            "max_tokens": 512,
        });
        let payload_bytes = serde_json::to_vec(&payload).unwrap();

        let result = engine.simulate(
            &snap,
            GovernedOperation::ToolCall,
            HitlTier::Tier2,
            &payload_bytes,
            &mut audit,
        );

        assert_eq!(result.resource_impact.llm_calls, 1);
        assert_eq!(result.resource_impact.fuel_cost, 10);
    }

    #[test]
    fn simulate_tool_call_with_write_file_payload() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let payload = serde_json::json!({
            "action": "write_file",
            "path": "/tmp/output.txt",
            "size": 4096,
        });
        let payload_bytes = serde_json::to_vec(&payload).unwrap();

        let result = engine.simulate(
            &snap,
            GovernedOperation::ToolCall,
            HitlTier::Tier2,
            &payload_bytes,
            &mut audit,
        );

        assert_eq!(result.resource_impact.file_operations, 1);
        assert_eq!(result.resource_impact.disk_bytes_delta, 4096);
    }

    #[test]
    fn simulation_does_not_modify_real_state() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let audit_count_before = audit.events().len();

        let _result = engine.simulate(
            &snap,
            GovernedOperation::TerminalCommand,
            HitlTier::Tier2,
            b"echo test",
            &mut audit,
        );

        // Audit trail gets a simulation event, but the snapshot fuel is unchanged
        assert_eq!(snap.fuel_remaining, 5000);
        // One audit event was appended (the simulation record itself)
        assert_eq!(audit.events().len(), audit_count_before + 1);
    }

    #[test]
    fn attach_and_get_for_request() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::TerminalCommand,
            HitlTier::Tier2,
            b"ls",
            &mut audit,
        );
        let sim_id = result.simulation_id;
        engine.attach_to_request("req-001", sim_id);

        let fetched = engine.get_for_request("req-001");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().simulation_id, sim_id);
    }

    #[test]
    fn commit_cleans_up() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::TerminalCommand,
            HitlTier::Tier2,
            b"ls",
            &mut audit,
        );
        engine.attach_to_request("req-002", result.simulation_id);

        engine.commit("req-002");
        assert!(engine.get_for_request("req-002").is_none());
    }

    #[test]
    fn rollback_cleans_up_and_audits() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::SocialPostPublish,
            HitlTier::Tier2,
            b"post content",
            &mut audit,
        );
        engine.attach_to_request("req-003", result.simulation_id);

        let events_before = audit.events().len();
        engine.rollback("req-003", &mut audit);

        assert!(engine.get_for_request("req-003").is_none());
        // Rollback appended an audit event
        assert_eq!(audit.events().len(), events_before + 1);
    }

    #[test]
    fn should_simulate_tier2_and_tier3_only() {
        assert!(!SpeculativeEngine::should_simulate(HitlTier::Tier0));
        assert!(!SpeculativeEngine::should_simulate(HitlTier::Tier1));
        assert!(SpeculativeEngine::should_simulate(HitlTier::Tier2));
        assert!(SpeculativeEngine::should_simulate(HitlTier::Tier3));
    }

    #[test]
    fn fuel_warning_when_insufficient() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        // Only 5 fuel remaining, but self-mutation costs 20
        let snap = engine.fork_state(agent_id, 5, AutonomyLevel::L4, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::SelfMutationApply,
            HitlTier::Tier3,
            b"mutation",
            &mut audit,
        );

        let has_fuel_warning = result.predicted_changes.iter().any(|c| {
            matches!(c, ActionPreview::DataModification(dm) if dm.resource == "fuel")
        });
        assert!(has_fuel_warning);
    }

    #[test]
    fn pending_simulations_lists_attached() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L2, vec![], 0);

        let r1 = engine.simulate(&snap, GovernedOperation::TerminalCommand, HitlTier::Tier2, b"cmd1", &mut audit);
        let r2 = engine.simulate(&snap, GovernedOperation::SocialPostPublish, HitlTier::Tier2, b"post", &mut audit);

        engine.attach_to_request("req-a", r1.simulation_id);
        engine.attach_to_request("req-b", r2.simulation_id);

        let pending = engine.pending_simulations();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn distributed_enable_simulation() {
        let mut engine = SpeculativeEngine::new();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        let snap = engine.fork_state(agent_id, 5000, AutonomyLevel::L5, vec![], 0);

        let result = engine.simulate(
            &snap,
            GovernedOperation::DistributedEnable,
            HitlTier::Tier3,
            b"cluster config",
            &mut audit,
        );

        assert_eq!(result.risk_level, RiskLevel::Critical);
        assert_eq!(result.resource_impact.network_calls, 1);
        assert!(result.resource_impact.fuel_cost >= 25);
    }
}
