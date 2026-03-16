use crate::cognitive::PlannerLlm;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorldEntity {
    pub entity_name: String,
    pub entity_type: String,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldRelationship {
    pub from: String,
    pub to: String,
    pub relation_type: String,
    pub strength: f64,
}

impl Default for WorldRelationship {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            relation_type: "related_to".to_string(),
            strength: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldPrediction {
    pub decision_id: String,
    pub alternative: String,
    pub predicted_outcome: String,
    pub confidence: f64,
}

impl Default for WorldPrediction {
    fn default() -> Self {
        Self {
            decision_id: String::new(),
            alternative: String::new(),
            predicted_outcome: String::new(),
            confidence: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorldModel {
    #[serde(default)]
    pub entities: Vec<WorldEntity>,
    #[serde(default)]
    pub relationships: Vec<WorldRelationship>,
    #[serde(default)]
    pub predictions: Vec<WorldPrediction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum SeedGraphResponse {
    Wrapped {
        entities: Vec<WorldEntity>,
        relationships: Vec<WorldRelationship>,
    },
    Flat(Vec<SeedGraphEntry>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum SeedGraphEntry {
    Entity {
        entity_name: String,
        entity_type: String,
        #[serde(default)]
        properties: HashMap<String, String>,
    },
    Relationship {
        from: String,
        to: String,
        relation_type: String,
        strength: f64,
    },
}

impl WorldModel {
    pub fn simulate_action(&self, decision_id: &str, alternative: &str) -> serde_json::Value {
        let entity_hint = self
            .entities
            .first()
            .map(|entity| entity.entity_name.as_str())
            .unwrap_or("the current scenario");
        json!({
            "decision_id": decision_id,
            "alternative": alternative,
            "predicted_outcome": format!("Simulated outcome for '{alternative}' in {entity_hint}."),
            "confidence": self.heuristic_confidence(),
        })
    }

    pub fn build_from_seed(seed_text: &str, llm: &dyn PlannerLlm) -> Result<Self, AgentError> {
        let prompt = format!(
            "Extract all entities (people, organizations, concepts, events) and relationships from this text. Return as JSON object with keys `entities` and `relationships`. Entities must be {{entity_name, entity_type, properties}}. Relationships must be {{from, to, relation_type, strength}}.\n\nTEXT:\n{seed_text}"
        );
        let response = llm.plan_query(&prompt)?;
        let value = crate::simulation::extract_json_value(&response)?;
        match serde_json::from_value::<SeedGraphResponse>(value).map_err(|error| {
            AgentError::SupervisorError(format!("invalid world model json: {error}"))
        })? {
            SeedGraphResponse::Wrapped {
                entities,
                relationships,
            } => Ok(Self {
                entities,
                relationships,
                predictions: Vec::new(),
            }),
            SeedGraphResponse::Flat(entries) => {
                let mut entities = Vec::new();
                let mut relationships = Vec::new();
                for entry in entries {
                    match entry {
                        SeedGraphEntry::Entity {
                            entity_name,
                            entity_type,
                            properties,
                        } => entities.push(WorldEntity {
                            entity_name,
                            entity_type,
                            properties,
                        }),
                        SeedGraphEntry::Relationship {
                            from,
                            to,
                            relation_type,
                            strength,
                        } => relationships.push(WorldRelationship {
                            from,
                            to,
                            relation_type,
                            strength,
                        }),
                    }
                }
                Ok(Self {
                    entities,
                    relationships,
                    predictions: Vec::new(),
                })
            }
        }
    }

    pub fn heuristic_confidence(&self) -> f64 {
        let raw =
            0.35 + (self.entities.len() as f64 * 0.05) + (self.relationships.len() as f64 * 0.03);
        raw.clamp(0.1, 0.95)
    }
}
