use crate::cognitive::PlannerLlm;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldSeed {
    pub scenario: String,
    #[serde(default)]
    pub entities: Vec<SeedEntity>,
    #[serde(default)]
    pub relationships: Vec<SeedRelationship>,
    #[serde(default)]
    pub variables: Vec<SeedVariable>,
    #[serde(default)]
    pub suggested_personas: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeedEntity {
    pub name: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeedRelationship {
    pub from: String,
    pub to: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeedVariable {
    pub key: String,
    pub description: String,
}

pub fn parse_seed(text: &str, llm: &dyn PlannerLlm) -> Result<WorldSeed, AgentError> {
    let prompt = format!(
        "Analyze this text and extract: 1) The key scenario/situation described. 2) All entities involved (people, organizations, concepts). 3) All relationships between entities. 4) Key variables that could change the outcome. 5) Suggested persona archetypes that would exist in this scenario. Return as structured JSON.\n\nTEXT:\n{text}"
    );
    let response = llm.plan_query(&prompt)?;
    serde_json::from_value::<WorldSeed>(crate::simulation::extract_json_value(&response)?)
        .map_err(|error| AgentError::SupervisorError(format!("invalid world seed json: {error}")))
}
