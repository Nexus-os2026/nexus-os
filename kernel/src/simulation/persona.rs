use super::world::WorldEnvironment;
use crate::cognitive::PlannerLlm;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Persona {
    pub id: String,
    pub name: String,
    pub role: String,
    pub personality: PersonalityProfile,
    #[serde(default)]
    pub beliefs: HashMap<String, f64>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub memories: Vec<PersonaMemory>,
    #[serde(default)]
    pub relationships: HashMap<String, f64>,
    #[serde(default)]
    pub behavior_rules: Vec<String>,
    pub last_action: Option<String>,
    pub influence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonalityProfile {
    pub openness: f64,
    pub conscientiousness: f64,
    pub extraversion: f64,
    pub agreeableness: f64,
    pub neuroticism: f64,
}

impl Default for PersonalityProfile {
    fn default() -> Self {
        Self {
            openness: 0.5,
            conscientiousness: 0.5,
            extraversion: 0.5,
            agreeableness: 0.5,
            neuroticism: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaMemory {
    pub event: String,
    pub timestamp: u64,
    pub emotional_impact: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PersonaAction {
    Speak { content: String },
    Whisper { target_id: String, content: String },
    Act { action: String },
    Observe,
    Nothing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaActionEnvelope {
    pub action: PersonaAction,
    pub reasoning: String,
}

#[derive(Debug, Deserialize)]
struct PersonaDecisionResponse {
    action: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PersonaBatchDecisionResponse {
    id: String,
    action: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

impl Persona {
    pub fn summary(&self) -> String {
        format!(
            "{} ({}) beliefs={:?} goals={:?}",
            self.name, self.role, self.beliefs, self.goals
        )
    }

    pub fn remember(&mut self, memory: PersonaMemory) {
        self.memories.push(memory);
        if self.memories.len() > 128 {
            let drain = self.memories.len().saturating_sub(128);
            self.memories.drain(0..drain);
        }
    }

    pub fn normalized_influence(&self) -> f64 {
        self.influence_score.clamp(0.0, 1.0)
    }
}

pub fn generate_personas(
    seed: &str,
    count: usize,
    llm: &dyn PlannerLlm,
) -> Result<Vec<Persona>, AgentError> {
    let prompt = format!(
        "Given this context: {seed}. Generate {count} diverse personas that would exist in this scenario. Each persona needs: id, name, role, personality traits (Big Five 0-1), initial beliefs on key topics (-1 to 1), goals, behavior rules, and influence_score. Return as JSON array."
    );
    let response = llm.plan_query(&prompt)?;
    let personas = serde_json::from_value::<Vec<Persona>>(crate::simulation::extract_json_value(
        &response,
    )?)
    .map_err(|error| AgentError::SupervisorError(format!("invalid persona json: {error}")))?;
    if personas.len() != count {
        return Err(AgentError::SupervisorError(format!(
            "expected {count} personas, got {}",
            personas.len()
        )));
    }
    Ok(personas)
}

pub fn persona_decide(
    persona: &Persona,
    environment: &WorldEnvironment,
    nearby_personas: &[&Persona],
    llm: &dyn PlannerLlm,
) -> Result<PersonaActionEnvelope, AgentError> {
    let recent_memories = persona
        .memories
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    let nearby = nearby_personas
        .iter()
        .map(|other| format!("{} ({})", other.name, other.role))
        .collect::<Vec<_>>();
    let prompt = format!(
        "You are {name}, a {role}. Your personality: {traits:?}. Your beliefs: {beliefs:?}. Your recent memories: {recent_memories:?}. Current environment: {global_state:?}. Nearby people: {nearby:?}. What do you do next? Choose ONE action: speak, whisper, act, observe, or nothing. Return as JSON: {{\"action\": string, \"target\": string|null, \"content\": string|null, \"reasoning\": string}}.",
        name = persona.name,
        role = persona.role,
        traits = persona.personality,
        beliefs = persona.beliefs,
        recent_memories = recent_memories,
        global_state = environment.global_state,
        nearby = nearby,
    );
    let response = llm.plan_query(&prompt)?;
    let parsed = serde_json::from_value::<PersonaDecisionResponse>(
        crate::simulation::extract_json_value(&response)?,
    )
    .map_err(|error| {
        AgentError::SupervisorError(format!("invalid persona action json: {error}"))
    })?;
    Ok(parse_persona_decision_response(parsed))
}

pub fn persona_decide_batch(
    personas: &[Persona],
    environment: &WorldEnvironment,
    world_personas: &[Persona],
    llm: &dyn PlannerLlm,
) -> Result<HashMap<String, PersonaActionEnvelope>, AgentError> {
    if personas.is_empty() {
        return Ok(HashMap::new());
    }

    let batch_payload = personas
        .iter()
        .map(|persona| {
            let recent_memories = persona
                .memories
                .iter()
                .rev()
                .take(5)
                .cloned()
                .collect::<Vec<_>>();
            let nearby = world_personas
                .iter()
                .filter(|other| other.id != persona.id)
                .take(3)
                .map(|other| format!("{} ({})", other.name, other.role))
                .collect::<Vec<_>>();
            json!({
                "id": persona.id,
                "name": persona.name,
                "role": persona.role,
                "personality": persona.personality,
                "beliefs": persona.beliefs,
                "recent_memories": recent_memories,
                "nearby_people": nearby,
            })
        })
        .collect::<Vec<_>>();

    let prompt = format!(
        "Given these personas and the current environment, decide what each one does next. Choose ONE action per persona: speak, whisper, act, observe, or nothing. Return as JSON array of persona decisions with fields: id, action, target, content, reasoning.\nENVIRONMENT: {environment:?}\nPERSONAS: {personas_json}",
        environment = environment.global_state,
        personas_json = serde_json::to_string(&batch_payload).unwrap_or_else(|_| "[]".to_string()),
    );
    let response = llm.plan_query(&prompt)?;
    let parsed = serde_json::from_value::<Vec<PersonaBatchDecisionResponse>>(
        crate::simulation::extract_json_value(&response)?,
    )
    .map_err(|error| {
        AgentError::SupervisorError(format!("invalid persona batch action json: {error}"))
    })?;

    let mut envelopes = HashMap::with_capacity(parsed.len());
    for decision in parsed {
        envelopes.insert(
            decision.id,
            parse_persona_decision_response(PersonaDecisionResponse {
                action: decision.action,
                target: decision.target,
                content: decision.content,
                reasoning: decision.reasoning,
            }),
        );
    }

    Ok(envelopes)
}

fn parse_persona_decision_response(parsed: PersonaDecisionResponse) -> PersonaActionEnvelope {
    let action = match parsed.action.trim().to_ascii_lowercase().as_str() {
        "speak" => PersonaAction::Speak {
            content: parsed.content.unwrap_or_default(),
        },
        "whisper" => PersonaAction::Whisper {
            target_id: parsed.target.unwrap_or_default(),
            content: parsed.content.unwrap_or_default(),
        },
        "act" => PersonaAction::Act {
            action: parsed.content.unwrap_or_else(|| "take action".to_string()),
        },
        "observe" => PersonaAction::Observe,
        _ => PersonaAction::Nothing,
    };
    PersonaActionEnvelope {
        action,
        reasoning: parsed
            .reasoning
            .unwrap_or_else(|| "No reasoning provided.".to_string()),
    }
}
