//! Frontend integration types.

use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::artifacts::ProjectArtifact;
use crate::economy::FactoryEconomy;
use crate::factory::SoftwareFactory;
use crate::governance::FactoryPolicy;
use crate::pipeline::PipelineStage;
use crate::project::Project;
use crate::quality::QualityGateResult;
use crate::roles::FactoryRole;

/// In-memory state held by the Tauri app.
pub struct FactoryState {
    pub factory: RwLock<SoftwareFactory>,
    pub policy: FactoryPolicy,
}

impl Default for FactoryState {
    fn default() -> Self {
        let policy = FactoryPolicy::default();
        Self {
            factory: RwLock::new(SoftwareFactory::new(policy.max_concurrent_projects)),
            policy,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageInfo {
    pub name: String,
    pub display_name: String,
    pub responsible_role: String,
    pub output_artifact: String,
    pub base_cost: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub stages: Vec<(String, u64)>,
    pub total: u64,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn factory_create_project(
    state: &FactoryState,
    title: &str,
    user_request: &str,
) -> Result<String, String> {
    state
        .factory
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .create_project(title.into(), user_request.into())
        .map_err(|e| e.to_string())
}

pub fn factory_assign_member(
    state: &FactoryState,
    project_id: &str,
    agent_id: &str,
    agent_name: &str,
    role: &str,
    autonomy: u8,
    score: Option<f64>,
) -> Result<(), String> {
    let r = parse_role(role)?;
    state
        .factory
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .assign_team_member(project_id, agent_id, agent_name, r, autonomy, score)
        .map_err(|e| e.to_string())
}

pub fn factory_start_pipeline(state: &FactoryState, project_id: &str) -> Result<(), String> {
    state
        .factory
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .start_pipeline(project_id)
        .map_err(|e| e.to_string())
}

pub fn factory_submit_artifact(
    state: &FactoryState,
    project_id: &str,
    artifact_json: &str,
) -> Result<QualityGateResult, String> {
    let artifact: ProjectArtifact =
        serde_json::from_str(artifact_json).map_err(|e| format!("Invalid artifact: {e}"))?;
    state
        .factory
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .submit_artifact(project_id, artifact)
        .map_err(|e| e.to_string())
}

pub fn factory_get_project(state: &FactoryState, project_id: &str) -> Result<Project, String> {
    state
        .factory
        .read()
        .map_err(|e| format!("lock: {e}"))?
        .get_project(project_id)
        .cloned()
        .map_err(|e| e.to_string())
}

pub fn factory_list_projects(state: &FactoryState) -> Result<Vec<Project>, String> {
    let f = state.factory.read().map_err(|e| format!("lock: {e}"))?;
    let mut all = f.active_projects().to_vec();
    all.extend_from_slice(f.completed_projects());
    Ok(all)
}

pub fn factory_get_cost(state: &FactoryState, project_id: &str) -> Result<CostBreakdown, String> {
    let f = state.factory.read().map_err(|e| format!("lock: {e}"))?;
    let project = f.get_project(project_id).map_err(|e| e.to_string())?;
    let stages: Vec<(String, u64)> = PipelineStage::all()
        .iter()
        .filter(|s| **s <= project.current_stage)
        .map(|s| (s.display_name().into(), s.base_cost()))
        .collect();
    let total = stages.iter().map(|(_, c)| c).sum();
    Ok(CostBreakdown { stages, total })
}

pub fn factory_get_policy(state: &FactoryState) -> FactoryPolicy {
    state.policy.clone()
}

pub fn factory_get_pipeline_stages() -> Vec<StageInfo> {
    PipelineStage::all()
        .iter()
        .map(|s| StageInfo {
            name: format!("{s:?}"),
            display_name: s.display_name().into(),
            responsible_role: s.responsible_role().display_name().into(),
            output_artifact: s.output_artifact().into(),
            base_cost: s.base_cost(),
        })
        .collect()
}

pub fn factory_estimate_cost() -> u64 {
    FactoryEconomy::estimate_total_cost()
}

fn parse_role(s: &str) -> Result<FactoryRole, String> {
    match s {
        "ProductManager" => Ok(FactoryRole::ProductManager),
        "Architect" => Ok(FactoryRole::Architect),
        "Developer" => Ok(FactoryRole::Developer),
        "QualityAssurance" => Ok(FactoryRole::QualityAssurance),
        "DevOps" => Ok(FactoryRole::DevOps),
        other => Err(format!("Unknown role: {other}")),
    }
}
