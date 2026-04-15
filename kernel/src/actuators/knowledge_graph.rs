use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use nexus_persistence::NexusDatabase;
use std::path::PathBuf;

const FUEL_COST_GRAPH_UPDATE: f64 = 4.0;
const FUEL_COST_GRAPH_QUERY: f64 = 2.0;

#[derive(Debug, Clone)]
pub struct KnowledgeGraphActuator {
    db_path: PathBuf,
}

impl Default for KnowledgeGraphActuator {
    fn default() -> Self {
        Self {
            db_path: NexusDatabase::default_db_path(),
        }
    }
}

impl KnowledgeGraphActuator {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn db(&self) -> Result<NexusDatabase, ActuatorError> {
        NexusDatabase::open(&self.db_path)
            .map_err(|error| ActuatorError::IoError(format!("open knowledge graph db: {error}")))
    }
}

impl Actuator for KnowledgeGraphActuator {
    fn name(&self) -> &str {
        "knowledge_graph_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["knowledge.graph".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "knowledge.graph",
        ) {
            return Err(ActuatorError::CapabilityDenied("knowledge.graph".into()));
        }

        let database = self.db()?;
        match action {
            PlannedAction::KnowledgeGraphUpdate {
                entities,
                relationships,
            } => {
                database
                    .replace_world_model_state(&context.agent_id, entities, relationships)
                    .map_err(|error| {
                        ActuatorError::IoError(format!("persist knowledge graph: {error}"))
                    })?;

                Ok(ActionResult {
                    success: true,
                    output: format!(
                        "stored {} entities and {} relationships",
                        entities.len(),
                        relationships.len()
                    ),
                    fuel_cost: FUEL_COST_GRAPH_UPDATE,
                    side_effects: vec![],
                })
            }
            PlannedAction::KnowledgeGraphQuery { query } => {
                let needle = query.to_lowercase();
                let entities = database
                    .load_world_model_entities(&context.agent_id)
                    .map_err(|error| {
                        ActuatorError::IoError(format!("load graph entities: {error}"))
                    })?
                    .into_iter()
                    .filter(|row| row.entity_json.to_lowercase().contains(&needle))
                    .map(|row| row.entity_json)
                    .collect::<Vec<_>>();
                let relationships = database
                    .load_world_model_relationships(&context.agent_id)
                    .map_err(|error| {
                        ActuatorError::IoError(format!("load graph relationships: {error}"))
                    })?
                    .into_iter()
                    .filter(|row| row.relationship_json.to_lowercase().contains(&needle))
                    .map(|row| row.relationship_json)
                    .collect::<Vec<_>>();

                Ok(ActionResult {
                    success: true,
                    output: serde_json::json!({
                        "query": query,
                        "entities": entities,
                        "relationships": relationships,
                    })
                    .to_string(),
                    fuel_cost: FUEL_COST_GRAPH_QUERY,
                    side_effects: vec![],
                })
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(tempdir: &TempDir) -> ActuatorContext {
        let mut capabilities = HashSet::new();
        capabilities.insert("knowledge.graph".to_string());
        ActuatorContext {
            agent_id: "agent".into(),
            agent_name: "agent".into(),
            working_dir: tempdir.path().to_path_buf(),
            autonomy_level: AutonomyLevel::L2,
            capabilities,
            fuel_remaining: 100.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn stores_and_queries_graph_state() {
        let tempdir = TempDir::new().unwrap();
        let db_path = tempdir.path().join("graph.db");
        let actuator = KnowledgeGraphActuator::new(db_path);
        let context = make_context(&tempdir);

        let update = PlannedAction::KnowledgeGraphUpdate {
            entities: vec![r#"{"id":"rust","type":"language"}"#.to_string()],
            relationships: vec![r#"{"from":"rust","to":"cargo","type":"uses"}"#.to_string()],
        };
        actuator.execute(&update, &context).unwrap();

        let query = PlannedAction::KnowledgeGraphQuery {
            query: "rust".into(),
        };
        let result = actuator.execute(&query, &context).unwrap();
        assert!(result.output.contains("rust"));
        assert!(result.output.contains("cargo"));
    }
}
