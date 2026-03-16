use super::world::SimulatedWorld;
use crate::cognitive::PlannerLlm;
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub title: String,
    pub detail: String,
    #[serde(default = "default_finding_confidence")]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpinionShift {
    pub topic: String,
    pub before: f64,
    pub after: f64,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coalition {
    pub name: String,
    pub members: Vec<String>,
    pub focus_topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurningPoint {
    pub tick: u64,
    pub description: String,
    pub shift_magnitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictionReport {
    pub summary: String,
    pub key_findings: Vec<Finding>,
    pub opinion_shifts: Vec<OpinionShift>,
    pub coalitions: Vec<Coalition>,
    pub turning_points: Vec<TurningPoint>,
    pub prediction: String,
    pub confidence: f64,
    pub uncertainties: Vec<String>,
}

fn default_finding_confidence() -> f64 {
    0.5
}

pub fn generate_prediction_report(
    world: &SimulatedWorld,
    llm: &dyn PlannerLlm,
) -> Result<PredictionReport, AgentError> {
    let opinion_shifts = aggregate_opinion_shifts(world);
    let coalitions = detect_coalitions(world);
    let turning_points = detect_turning_points(world);
    let uncertainties = detect_uncertainties(world);
    let prediction = predict_outcome(world, &opinion_shifts, &coalitions);
    let confidence = confidence_from_world(world, &opinion_shifts, &uncertainties);
    let findings = vec![
        Finding {
            title: "Most active trend".to_string(),
            detail: prediction.clone(),
            confidence,
        },
        Finding {
            title: "Emergent patterns".to_string(),
            detail: world
                .timeline
                .ticks
                .last()
                .map(|tick| tick.emergent_patterns.join("; "))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "No dominant pattern detected.".to_string()),
            confidence: (confidence * 0.92).clamp(0.0, 1.0),
        },
    ];
    let llm_summary = llm.plan_query(&format!(
        "Analyze this simulation summary and produce a concise executive summary. Scenario: {}. Prediction: {}. Confidence: {:.2}. Turning points: {:?}.",
        world.description, prediction, confidence, turning_points
    ));
    let summary = llm_summary
        .ok()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| format!("{prediction} Confidence {:.2}.", confidence));
    Ok(PredictionReport {
        summary,
        key_findings: findings,
        opinion_shifts,
        coalitions,
        turning_points,
        prediction,
        confidence,
        uncertainties,
    })
}

pub(crate) fn aggregate_opinion_shifts(world: &SimulatedWorld) -> Vec<OpinionShift> {
    let mut first_seen: HashMap<String, f64> = HashMap::new();
    let mut last_seen: HashMap<String, f64> = HashMap::new();
    for persona in &world.personas {
        for (topic, value) in &persona.beliefs {
            let entry = first_seen.entry(topic.clone()).or_insert(0.0);
            *entry += *value;
            *last_seen.entry(topic.clone()).or_insert(0.0) += *value;
        }
    }
    let count = world.personas.len().max(1) as f64;
    let mut shifts = last_seen
        .into_iter()
        .map(|(topic, after_total)| {
            let before_total = first_seen.get(&topic).copied().unwrap_or(after_total);
            let before = before_total / count;
            let after = after_total / count;
            OpinionShift {
                topic,
                before,
                after,
                delta: after - before,
            }
        })
        .collect::<Vec<_>>();
    shifts.sort_by(|left, right| {
        right
            .delta
            .abs()
            .partial_cmp(&left.delta.abs())
            .unwrap_or(Ordering::Equal)
    });
    shifts
}

pub(crate) fn detect_coalitions(world: &SimulatedWorld) -> Vec<Coalition> {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut bridge = Vec::new();
    for persona in &world.personas {
        let average_belief = if persona.beliefs.is_empty() {
            0.0
        } else {
            persona.beliefs.values().sum::<f64>() / persona.beliefs.len() as f64
        };
        if average_belief > 0.2 {
            right.push(persona.name.clone());
        } else if average_belief < -0.2 {
            left.push(persona.name.clone());
        } else {
            bridge.push(persona.name.clone());
        }
    }
    let mut coalitions = Vec::new();
    if !left.is_empty() {
        coalitions.push(Coalition {
            name: "Reform bloc".to_string(),
            members: left,
            focus_topics: vec!["change".to_string()],
        });
    }
    if !right.is_empty() {
        coalitions.push(Coalition {
            name: "Stability bloc".to_string(),
            members: right,
            focus_topics: vec!["continuity".to_string()],
        });
    }
    if !bridge.is_empty() {
        coalitions.push(Coalition {
            name: "Swing bloc".to_string(),
            members: bridge,
            focus_topics: vec!["uncertainty".to_string()],
        });
    }
    coalitions
}

pub(crate) fn detect_turning_points(world: &SimulatedWorld) -> Vec<TurningPoint> {
    let mut turning_points = world
        .timeline
        .ticks
        .iter()
        .map(|tick| TurningPoint {
            tick: tick.tick_number,
            description: tick
                .emergent_patterns
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{} events reshaped the world", tick.events.len())),
            shift_magnitude: tick
                .belief_shifts
                .iter()
                .map(|(_, _, value)| value.abs())
                .sum::<f64>(),
        })
        .collect::<Vec<_>>();
    turning_points.sort_by(|left, right| {
        right
            .shift_magnitude
            .partial_cmp(&left.shift_magnitude)
            .unwrap_or(Ordering::Equal)
    });
    turning_points.truncate(5);
    turning_points
}

pub(crate) fn detect_uncertainties(world: &SimulatedWorld) -> Vec<String> {
    let mut values: HashMap<String, Vec<f64>> = HashMap::new();
    for persona in &world.personas {
        for (topic, value) in &persona.beliefs {
            values.entry(topic.clone()).or_default().push(*value);
        }
    }
    let mut uncertainties = values
        .into_iter()
        .filter_map(|(topic, belief_values)| {
            if belief_values.len() < 2 {
                return None;
            }
            let mean = belief_values.iter().sum::<f64>() / belief_values.len() as f64;
            let variance = belief_values
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / belief_values.len() as f64;
            (variance > 0.15).then(|| format!("{topic} remains highly contested"))
        })
        .collect::<Vec<_>>();
    if uncertainties.is_empty() {
        uncertainties.push("Key topics broadly converged by the end of the run".to_string());
    }
    uncertainties
}

fn predict_outcome(
    world: &SimulatedWorld,
    opinion_shifts: &[OpinionShift],
    coalitions: &[Coalition],
) -> String {
    let leading_topic = opinion_shifts
        .first()
        .map(|shift| format!("Momentum centers on {}", shift.topic))
        .unwrap_or_else(|| "No strong opinion movement detected".to_string());
    let largest_coalition = coalitions
        .iter()
        .max_by_key(|coalition| coalition.members.len())
        .map(|coalition| coalition.name.clone())
        .unwrap_or_else(|| "fragmented actors".to_string());
    format!(
        "{leading_topic}; the likely outcome is consolidation around {largest_coalition} after {} ticks.",
        world.tick_count
    )
}

fn confidence_from_world(
    world: &SimulatedWorld,
    opinion_shifts: &[OpinionShift],
    uncertainties: &[String],
) -> f64 {
    let activity = world
        .timeline
        .ticks
        .iter()
        .map(|tick| tick.events.len() as f64)
        .sum::<f64>()
        / world.tick_count.max(1) as f64;
    let movement = opinion_shifts
        .iter()
        .map(|shift| shift.delta.abs())
        .sum::<f64>();
    let uncertainty_penalty = uncertainties.len() as f64 * 0.08;
    (0.35 + (activity * 0.02) + (movement * 0.15) - uncertainty_penalty).clamp(0.1, 0.95)
}
