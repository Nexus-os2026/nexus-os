//! The knowledge-graph actuator (P3-HM-CONTRACT-02 §6.5, §6.8, §12.3).
//!
//! The graph lives in the primary state database. It opens no database of
//! its own: an update is the labelled knowledge-graph replace (the delete
//! and the labelled inserts in one `BEGIN IMMEDIATE`), and a query is a
//! route-first recall whose rows are returned verbatim. Both run only as a
//! step of a unit ([`Actuator::execute_in`]); without one they are refused.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, ActuatorScope};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use crate::hm::{Parent, RecallSpec, RecordTable, MAX_RECORDS};

const FUEL_COST_GRAPH_UPDATE: f64 = 4.0;
const FUEL_COST_GRAPH_QUERY: f64 = 2.0;

/// The refusal of a graph action outside a unit.
const NO_UNIT: &str =
    "knowledge graph: refused: it runs only as a step of a unit (P3-HM-CONTRACT-02 §6.1)";

#[derive(Debug, Clone, Default)]
pub struct KnowledgeGraphActuator;

impl KnowledgeGraphActuator {
    pub fn new() -> Self {
        Self
    }
}

fn require_capability(context: &ActuatorContext) -> Result<(), ActuatorError> {
    if !has_capability(
        context.capabilities.iter().map(String::as_str),
        "knowledge.graph",
    ) {
        return Err(ActuatorError::CapabilityDenied("knowledge.graph".into()));
    }
    Ok(())
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
        require_capability(context)?;
        match action {
            PlannedAction::KnowledgeGraphUpdate { .. }
            | PlannedAction::KnowledgeGraphQuery { .. } => {
                Err(ActuatorError::IoError(NO_UNIT.into()))
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }

    fn execute_in(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        scope: &mut ActuatorScope<'_>,
    ) -> Result<ActionResult, ActuatorError> {
        require_capability(context)?;
        match action {
            PlannedAction::KnowledgeGraphUpdate {
                entities,
                relationships,
            } => {
                // The graph rows come from the plan: it is their parent.
                scope
                    .issuer
                    .replace_world_model(
                        scope.ctx,
                        entities,
                        relationships,
                        &[Parent::Derived(scope.plan)],
                    )
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
                let mut found = (0, 0);
                for (table, column) in [
                    (RecordTable::WorldModelEntities, "entity_json"),
                    (RecordTable::WorldModelRelationships, "relationship_json"),
                ] {
                    let spec = RecallSpec {
                        contains: Some((column, query.as_str())),
                        newest_first: false,
                        ..RecallSpec::table(table, MAX_RECORDS)
                    };
                    let records = scope
                        .issuer
                        .recall_spec(scope.ctx, &spec)
                        .map_err(|error| {
                            ActuatorError::IoError(format!("load knowledge graph: {error}"))
                        })?;
                    match table {
                        RecordTable::WorldModelEntities => found.0 = records.len(),
                        _ => found.1 = records.len(),
                    }
                    scope.recalled.extend(records);
                }

                Ok(ActionResult {
                    success: true,
                    // Content-free: the rows themselves are verbatim items.
                    output: format!("found {} entities and {} relationships", found.0, found.1),
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
    use crate::hm::testing::{ScriptedLlm, TestHm};
    use crate::hm::{PromptBuilder, RouteClass};
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(tempdir: &TempDir, agent_id: &str) -> ActuatorContext {
        let mut capabilities = HashSet::new();
        capabilities.insert("knowledge.graph".to_string());
        ActuatorContext {
            agent_id: agent_id.into(),
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

    /// P0-002C5B, strengthened (P3-HM mission R7): the graph actuator opens
    /// no database of its own at all; outside a unit both actions are
    /// refused, whatever identity home exists.
    #[test]
    fn p0_002c5b_without_an_identity_home_the_graph_database_is_unavailable() {
        let tempdir = TempDir::new().unwrap();
        let context = make_context(&tempdir, "agent");
        let actuator = KnowledgeGraphActuator::new();
        for action in [
            PlannedAction::KnowledgeGraphUpdate {
                entities: vec!["{}".into()],
                relationships: vec![],
            },
            PlannedAction::KnowledgeGraphQuery { query: "x".into() },
        ] {
            let error = actuator.execute(&action, &context).unwrap_err();
            assert!(
                error.to_string().contains("only as a step of a unit"),
                "{error}"
            );
        }
    }

    #[test]
    fn stores_and_queries_graph_state() {
        let tempdir = TempDir::new().unwrap();
        let hm = TestHm::new();
        let agent = hm.agent("graph-agent");
        let actuator = KnowledgeGraphActuator::new();
        let context = make_context(&tempdir, &agent.to_string());
        let mut unit = hm.goal(agent);
        let plan = unit.produce("the plan");

        let update = PlannedAction::KnowledgeGraphUpdate {
            entities: vec![r#"{"id":"rust","type":"language"}"#.to_string()],
            relationships: vec![r#"{"from":"rust","to":"cargo","type":"uses"}"#.to_string()],
        };
        let mut scope = ActuatorScope {
            issuer: &hm.issuer,
            ctx: &mut unit,
            plan: &plan,
            recalled: Vec::new(),
        };
        actuator.execute_in(&update, &context, &mut scope).unwrap();

        let query = PlannedAction::KnowledgeGraphQuery {
            query: "rust".into(),
        };
        let result = actuator.execute_in(&query, &context, &mut scope).unwrap();
        assert_eq!(result.output, "found 1 entities and 1 relationships");
        let recalled = std::mem::take(&mut scope.recalled);
        assert_eq!(recalled.len(), 2);

        // The rows reach a prompt verbatim (included, §11.2).
        let llm = ScriptedLlm::replying(&hm.issuer, RouteClass::Local, "ok");
        llm.ask(&mut unit, &mut |c| {
            let mut b = PromptBuilder::new(c);
            for record in &recalled {
                b.include(record);
            }
            Some(b.build())
        })
        .unwrap();
        let sent = llm.sent();
        assert!(sent[0].contains("rust"));
        assert!(sent[0].contains("cargo"));
    }
}
