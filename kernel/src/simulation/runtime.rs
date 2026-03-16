use super::persona::{
    generate_personas, persona_decide, persona_decide_batch, Persona, PersonaAction,
    PersonaActionEnvelope, PersonaMemory,
};
use super::report::{generate_prediction_report, PredictionReport};
use super::seed::WorldSeed;
use super::timeline::{WorldEvent, WorldTick};
use super::world::{SimulatedWorld, WorldStatus};
use crate::cognitive::algorithms::{EvolutionEngine as CognitiveEvolutionEngine, SwarmCoordinator};
use crate::cognitive::types::{AgentStep, PlannedAction};
use crate::cognitive::PlannerLlm;
use crate::errors::AgentError;
use chrono::Utc;
use nexus_persistence::NexusDatabase;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_SIMULATION_WALL_CLOCK: Duration = Duration::from_secs(60);

pub trait SimulationObserver: Send + Sync {
    fn on_tick(&self, _progress: &SimulationProgress) {}
    fn on_complete(&self, _world_id: &str, _report: &PredictionReport) {}
}

#[derive(Default)]
pub struct NoOpSimulationObserver;

impl SimulationObserver for NoOpSimulationObserver {}

#[derive(Debug, Clone, Default)]
pub struct SimulationControl {
    pub paused: Arc<AtomicBool>,
    pub stopped: Arc<AtomicBool>,
    pub pending_injections: Arc<Mutex<VecDeque<(String, String)>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationProgress {
    pub world_id: String,
    pub tick: u64,
    pub status: String,
    pub events_count: usize,
    pub events: Vec<SimulationLiveEvent>,
    pub belief_summary: HashMap<String, f64>,
    pub fuel_consumed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationLiveEvent {
    pub actor_id: String,
    pub actor_name: String,
    pub action_type: String,
    pub content: String,
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    pub impact: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetaSimulationAnalysis {
    pub consensus_prediction: String,
    pub convergence_ratio: f64,
    pub confidence: f64,
    pub divergence_factors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptimalSimConfig {
    pub persona_count: usize,
    pub max_ticks: u64,
    pub belief_update_rate: f64,
    pub personality_variance: f64,
    pub consistency_score: f64,
    pub generation: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedSimulationState {
    pub world: SimulatedWorld,
    pub max_ticks: u64,
    pub tick_interval_ms: u64,
    pub batch_size: usize,
    #[serde(default = "default_persona_decision_timeout_ms")]
    pub persona_decision_timeout_ms: u64,
    pub belief_update_rate: f64,
    pub fuel_consumed: f64,
}

fn default_persona_decision_timeout_ms() -> u64 {
    30_000
}

pub struct SimulationRuntime {
    pub world: SimulatedWorld,
    pub llm: Arc<dyn PlannerLlm>,
    pub db: Arc<NexusDatabase>,
    pub tick_interval_ms: u64,
    pub max_ticks: u64,
    pub batch_size: usize,
    pub persona_decision_timeout_ms: u64,
    pub belief_update_rate: f64,
    pub control: SimulationControl,
    pub observer: Arc<dyn SimulationObserver>,
    fuel_consumed: f64,
    stabilized_for_ticks: u64,
    stabilized: bool,
}

impl SimulationRuntime {
    pub fn new(world: SimulatedWorld, llm: Arc<dyn PlannerLlm>, db: Arc<NexusDatabase>) -> Self {
        Self {
            world,
            llm,
            db,
            tick_interval_ms: 1_000,
            max_ticks: 100,
            batch_size: 25,
            persona_decision_timeout_ms: default_persona_decision_timeout_ms(),
            belief_update_rate: 0.12,
            control: SimulationControl::default(),
            observer: Arc::new(NoOpSimulationObserver),
            fuel_consumed: 0.0,
            stabilized_for_ticks: 0,
            stabilized: false,
        }
    }

    pub fn with_control(mut self, control: SimulationControl) -> Self {
        self.control = control;
        self
    }

    pub fn with_observer(mut self, observer: Arc<dyn SimulationObserver>) -> Self {
        self.observer = observer;
        self
    }

    pub fn fuel_consumed(&self) -> f64 {
        self.fuel_consumed
    }

    pub fn persisted_state(&self) -> PersistedSimulationState {
        PersistedSimulationState {
            world: self.world.clone(),
            max_ticks: self.max_ticks,
            tick_interval_ms: self.tick_interval_ms,
            batch_size: self.batch_size,
            persona_decision_timeout_ms: self.persona_decision_timeout_ms,
            belief_update_rate: self.belief_update_rate,
            fuel_consumed: self.fuel_consumed,
        }
    }

    pub fn has_stabilized(&self) -> bool {
        self.stabilized
    }

    pub fn run_simulation(&mut self) -> Result<PredictionReport, AgentError> {
        let started_at = Instant::now();
        self.world.status = WorldStatus::Running;
        let state = self.persisted_state();
        persist_world_snapshot(&self.db, &state, None)?;
        for tick in 0..self.max_ticks {
            if started_at.elapsed() >= MAX_SIMULATION_WALL_CLOCK {
                break;
            }
            if self.control.stopped.load(Ordering::Relaxed) {
                break;
            }
            while self.control.paused.load(Ordering::Relaxed) {
                self.world.status = WorldStatus::Paused;
                thread::sleep(Duration::from_millis(25));
            }
            self.world.status = WorldStatus::Running;
            let injections = self.drain_injections();
            for (key, value) in &injections {
                self.world.inject_variable(key.clone(), value.clone());
            }
            let tick_events = self.run_tick(tick, injections.clone())?;
            let total_shift = tick_events
                .belief_shifts
                .iter()
                .map(|(_, _, value)| value.abs())
                .sum::<f64>();
            if total_shift < 0.05 {
                self.stabilized_for_ticks += 1;
            } else {
                self.stabilized_for_ticks = 0;
            }
            self.world.tick_count = tick + 1;
            self.world.timeline.current_tick = tick + 1;
            self.world
                .timeline
                .events_per_tick
                .push(tick_events.events.clone());
            self.world.timeline.ticks.push(tick_events.clone());
            let progress = SimulationProgress {
                world_id: self.world.id.clone(),
                tick: self.world.tick_count,
                status: format!("{:?}", self.world.status).to_ascii_lowercase(),
                events_count: tick_events.events.len(),
                events: build_live_events(&self.world.personas, &tick_events.events),
                belief_summary: summarize_beliefs(&self.world.personas),
                fuel_consumed: self.fuel_consumed,
            };
            self.observer.on_tick(&progress);
            let state = self.persisted_state();
            persist_world_snapshot(&self.db, &state, None)?;
            if self.tick_interval_ms > 0 {
                thread::sleep(Duration::from_millis(self.tick_interval_ms));
            }
            if self.stabilized_for_ticks >= 5 {
                self.stabilized = true;
                break;
            }
        }
        self.world.status = WorldStatus::Completed;
        let report = generate_prediction_report(&self.world, self.llm.as_ref())?;
        let state = self.persisted_state();
        persist_world_snapshot(&self.db, &state, Some(&report))?;
        self.observer.on_complete(&self.world.id, &report);
        Ok(report)
    }

    pub fn inject_variable(&mut self, key: &str, value: &str) -> Result<(), AgentError> {
        self.world
            .inject_variable(key.to_string(), value.to_string());
        let state = self.persisted_state();
        persist_world_snapshot(&self.db, &state, None)?;
        Ok(())
    }

    pub fn chat_with_persona(&self, persona_id: &str, message: &str) -> Result<String, AgentError> {
        let persona = self
            .world
            .personas
            .iter()
            .find(|persona| persona.id == persona_id)
            .ok_or_else(|| AgentError::SupervisorError(format!("unknown persona {persona_id}")))?;
        let prompt = format!(
            "Respond in character as {name}, a {role}. Beliefs: {beliefs:?}. Recent memories: {memories:?}. User says: {message}",
            name = persona.name,
            role = persona.role,
            beliefs = persona.beliefs,
            memories = persona.memories.iter().rev().take(10).collect::<Vec<_>>(),
        );
        self.llm.plan_query(&prompt)
    }

    fn drain_injections(&self) -> Vec<(String, String)> {
        let mut guard = self
            .control
            .pending_injections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.drain(..).collect()
    }

    fn run_tick(
        &mut self,
        tick: u64,
        injections: Vec<(String, String)>,
    ) -> Result<WorldTick, AgentError> {
        let mut events = Vec::new();
        let mut belief_shifts = Vec::new();
        let mut patterns = Vec::new();
        let persona_ids = self
            .world
            .personas
            .iter()
            .map(|persona| persona.id.clone())
            .collect::<Vec<_>>();
        let batch_size = self.batch_size.clamp(5, 10);
        for start in (0..persona_ids.len()).step_by(batch_size) {
            let batch_ids = &persona_ids[start..usize::min(start + batch_size, persona_ids.len())];
            let actor_snapshots = batch_ids
                .iter()
                .map(|persona_id| {
                    self.world
                        .personas
                        .iter()
                        .find(|persona| &persona.id == persona_id)
                        .cloned()
                        .ok_or_else(|| {
                            AgentError::SupervisorError(
                                "persona disappeared during tick".to_string(),
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let decisions = self.decide_batch(tick, &actor_snapshots);
            for actor_snapshot in actor_snapshots {
                let actor_index = self
                    .world
                    .personas
                    .iter()
                    .position(|persona| persona.id == actor_snapshot.id)
                    .ok_or_else(|| {
                        AgentError::SupervisorError("persona disappeared during tick".to_string())
                    })?;
                let decision = decisions
                    .get(&actor_snapshot.id)
                    .cloned()
                    .unwrap_or_else(|| fallback_persona_action(&actor_snapshot, tick, "missing"));
                self.fuel_consumed += fuel_for_decision(&actor_snapshot, &decision);
                let observers =
                    determine_observers(&actor_snapshot, &decision.action, &self.world.personas);
                let impact = event_impact(&actor_snapshot, &decision.action);
                {
                    let actor = &mut self.world.personas[actor_index];
                    actor.last_action = Some(action_label(&decision.action));
                }
                let event = WorldEvent {
                    tick,
                    actor_id: actor_snapshot.id.clone(),
                    action: decision.action.clone(),
                    observers: observers.clone(),
                    impact,
                };
                persist_event(&self.db, &self.world.id, &event)?;
                apply_event_memories(&mut self.world.personas, &actor_snapshot, &event);
                let shifts = apply_event_influence(
                    &mut self.world.personas,
                    &actor_snapshot,
                    &event,
                    self.belief_update_rate,
                );
                belief_shifts.extend(shifts);
                events.push(event);
            }
        }
        patterns.extend(detect_patterns(&self.world.personas));
        Ok(WorldTick {
            tick_number: tick,
            events,
            variable_injections: injections,
            belief_shifts,
            emergent_patterns: patterns,
        })
    }

    fn decide_batch(
        &self,
        tick: u64,
        personas: &[Persona],
    ) -> HashMap<String, PersonaActionEnvelope> {
        if personas.is_empty() {
            return HashMap::new();
        }

        if personas.len() == 1 {
            let persona = personas[0].clone();
            let nearby_refs = self
                .world
                .personas
                .iter()
                .filter(|other| other.id != persona.id)
                .take(3)
                .collect::<Vec<_>>();
            return match persona_decide(
                &persona,
                &self.world.environment,
                &nearby_refs,
                self.llm.as_ref(),
            ) {
                Ok(decision) => HashMap::from([(persona.id.clone(), decision)]),
                Err(_) => HashMap::from([(
                    persona.id.clone(),
                    fallback_persona_action(&persona, tick, "single-decision-error"),
                )]),
            };
        }

        let llm = self.llm.clone();
        let environment = self.world.environment.clone();
        let world_personas = self.world.personas.clone();
        let batch = personas.to_vec();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = persona_decide_batch(&batch, &environment, &world_personas, llm.as_ref());
            let _ = tx.send(result);
        });

        match rx.recv_timeout(Duration::from_millis(self.persona_decision_timeout_ms)) {
            Ok(Ok(decisions)) => decisions,
            Ok(Err(_)) | Err(_) => personas
                .iter()
                .map(|persona| {
                    (
                        persona.id.clone(),
                        fallback_persona_action(persona, tick, "timeout-or-parse-error"),
                    )
                })
                .collect(),
        }
    }
}

pub fn run_parallel_simulations(
    seed: &WorldSeed,
    variant_count: usize,
    llm: Arc<dyn PlannerLlm>,
    db: Arc<NexusDatabase>,
) -> Result<Vec<PredictionReport>, AgentError> {
    let swarm = SwarmCoordinator;
    let parallel_batch_size = prepare_parallel_batch_size(seed, &swarm);
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for variant in 0..variant_count {
            let llm = llm.clone();
            let db = db.clone();
            let seed = seed.clone();
            handles.push(
                scope.spawn(move || -> Result<PredictionReport, AgentError> {
                    let personas = generate_personas(&seed.scenario, 5, llm.as_ref())?
                        .into_iter()
                        .enumerate()
                        .map(|(index, mut persona)| {
                            let adjustment = (((variant + index) % 5) as f64 - 2.0) * 0.05;
                            for value in persona.beliefs.values_mut() {
                                *value = (*value + adjustment).clamp(-1.0, 1.0);
                            }
                            persona
                        })
                        .collect::<Vec<_>>();
                    let mut world = SimulatedWorld::from_seed(
                        format!("variant-{variant}"),
                        format!("Variant {variant}"),
                        seed.scenario.clone(),
                        &seed,
                        personas,
                        llm.as_ref(),
                    )?;
                    world.environment.global_state.insert(
                        "variant_bias".to_string(),
                        format!("{:.2}", (variant as f64 * 0.07) - 0.2),
                    );
                    let mut runtime = SimulationRuntime::new(world, llm.clone(), db);
                    runtime.tick_interval_ms = 0;
                    runtime.max_ticks = 8;
                    runtime.batch_size = parallel_batch_size.max(2);
                    runtime.run_simulation()
                }),
            );
        }
        handles
            .into_iter()
            .map(|handle| {
                handle.join().unwrap_or_else(|_| {
                    Err(AgentError::SupervisorError(
                        "parallel simulation panicked".to_string(),
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()
    })
}

pub fn compare_reports(reports: &[PredictionReport]) -> MetaSimulationAnalysis {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for report in reports {
        *counts.entry(report.prediction.clone()).or_insert(0) += 1;
    }
    let (consensus_prediction, wins) = counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .unwrap_or_else(|| ("No consensus".to_string(), 0));
    let divergence_factors = reports
        .iter()
        .flat_map(|report| report.uncertainties.clone())
        .take(3)
        .collect::<Vec<_>>();
    let convergence_ratio = if reports.is_empty() {
        0.0
    } else {
        wins as f64 / reports.len() as f64
    };
    MetaSimulationAnalysis {
        consensus_prediction,
        convergence_ratio,
        confidence: convergence_ratio.clamp(0.1, 0.99),
        divergence_factors,
    }
}

pub fn evolve_simulation(
    seed: &WorldSeed,
    generations: u32,
    llm: Arc<dyn PlannerLlm>,
    db: Arc<NexusDatabase>,
) -> Result<OptimalSimConfig, AgentError> {
    let optimizer = CognitiveEvolutionEngine;
    let mut population = vec![
        OptimalSimConfig {
            persona_count: 6,
            max_ticks: 8,
            belief_update_rate: 0.08,
            personality_variance: 0.10,
            consistency_score: 0.0,
            generation: 0,
        },
        OptimalSimConfig {
            persona_count: 8,
            max_ticks: 10,
            belief_update_rate: 0.12,
            personality_variance: 0.15,
            consistency_score: 0.0,
            generation: 0,
        },
        OptimalSimConfig {
            persona_count: 10,
            max_ticks: 12,
            belief_update_rate: 0.16,
            personality_variance: 0.20,
            consistency_score: 0.0,
            generation: 0,
        },
        OptimalSimConfig {
            persona_count: 12,
            max_ticks: 14,
            belief_update_rate: 0.20,
            personality_variance: 0.25,
            consistency_score: 0.0,
            generation: 0,
        },
        OptimalSimConfig {
            persona_count: 14,
            max_ticks: 16,
            belief_update_rate: 0.24,
            personality_variance: 0.30,
            consistency_score: 0.0,
            generation: 0,
        },
    ];
    let mut best = population[0].clone();
    for generation in 0..generations {
        for config in &mut population {
            let _optimized_plan = optimizer.optimize_plan(simulation_config_steps(config));
            let personas = generate_personas(&seed.scenario, config.persona_count, llm.as_ref())?
                .into_iter()
                .enumerate()
                .map(|(index, mut persona)| {
                    let drift = ((index % 4) as f64 - 1.5) * config.personality_variance * 0.1;
                    persona.personality.openness =
                        (persona.personality.openness + drift).clamp(0.0, 1.0);
                    persona
                })
                .collect::<Vec<_>>();
            let mut runtime = SimulationRuntime::new(
                SimulatedWorld::from_seed(
                    format!("evolve-{generation}-{}", config.persona_count),
                    "Evolution candidate",
                    seed.scenario.clone(),
                    seed,
                    personas,
                    llm.as_ref(),
                )?,
                llm.clone(),
                db.clone(),
            );
            runtime.tick_interval_ms = 0;
            runtime.max_ticks = config.max_ticks;
            runtime.belief_update_rate = config.belief_update_rate;
            let report = runtime.run_simulation()?;
            let support_score = 1.0 - (report.uncertainties.len() as f64 * 0.05);
            config.consistency_score =
                ((report.confidence * 0.7) + (support_score * 0.3)).clamp(0.0, 1.0);
            config.generation = generation;
        }
        population.sort_by(|left, right| {
            right
                .consistency_score
                .partial_cmp(&left.consistency_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        best = population[0].clone();
        let mate = population[1].clone();
        population = population
            .into_iter()
            .take(3)
            .enumerate()
            .map(|(index, mut config)| {
                if index > 0 {
                    config.persona_count = ((config.persona_count + mate.persona_count) / 2).max(4);
                    config.max_ticks = ((config.max_ticks + mate.max_ticks) / 2).max(6);
                    config.belief_update_rate =
                        ((config.belief_update_rate + mate.belief_update_rate) / 2.0 + 0.01)
                            .clamp(0.05, 0.35);
                }
                config
            })
            .collect::<Vec<_>>();
        while population.len() < 5 {
            let mut clone = best.clone();
            clone.persona_count += population.len();
            clone.belief_update_rate =
                (clone.belief_update_rate + population.len() as f64 * 0.01).clamp(0.05, 0.35);
            clone.generation = generation;
            population.push(clone);
        }
    }
    Ok(best)
}

pub fn estimate_simulation_fuel(persona_count: usize, max_ticks: u64, batch_size: usize) -> u64 {
    let decisions = persona_count as u64 * max_ticks;
    let batching_discount = batch_size.max(1) as f64 / 10.0;
    ((decisions as f64 * (6.0 - batching_discount)).round() as u64).max(10)
}

fn fuel_for_decision(persona: &Persona, decision: &PersonaActionEnvelope) -> f64 {
    let base = match decision.action {
        PersonaAction::Speak { .. } => 8.0,
        PersonaAction::Whisper { .. } => 6.0,
        PersonaAction::Act { .. } => 7.0,
        PersonaAction::Observe => 3.0,
        PersonaAction::Nothing => 1.0,
    };
    base + persona.goals.len() as f64 * 0.2 + persona.memories.len().min(10) as f64 * 0.1
}

fn fallback_persona_action(persona: &Persona, tick: u64, reason: &str) -> PersonaActionEnvelope {
    let observe = ((persona.id.len() as u64) + tick).is_multiple_of(2);
    PersonaActionEnvelope {
        action: if observe {
            PersonaAction::Observe
        } else {
            PersonaAction::Nothing
        },
        reasoning: format!("Fallback persona action used due to {reason}."),
    }
}

fn prepare_parallel_batch_size(seed: &WorldSeed, swarm: &SwarmCoordinator) -> usize {
    let mut step = AgentStep::new(
        "parallel-simulation".to_string(),
        PlannedAction::LlmQuery {
            prompt: format!(
                "Coordinate parallel simulation variants for {}",
                seed.scenario
            ),
            context: seed.suggested_personas.clone(),
        },
    );
    swarm.prepare_parallel_step(&mut step);
    step.max_retries.max(2) as usize
}

fn simulation_config_steps(config: &OptimalSimConfig) -> Vec<AgentStep> {
    vec![
        AgentStep::new(
            format!("evolve-{}", config.generation),
            PlannedAction::KnowledgeGraphQuery {
                query: format!("Evaluate consistency for {} personas", config.persona_count),
            },
        ),
        AgentStep::new(
            format!("evolve-{}", config.generation),
            PlannedAction::LlmQuery {
                prompt: format!(
                    "Assess stability over {} ticks with belief rate {:.2}",
                    config.max_ticks, config.belief_update_rate
                ),
                context: vec![format!("variance={:.2}", config.personality_variance)],
            },
        ),
    ]
}

fn determine_observers(
    actor: &Persona,
    action: &PersonaAction,
    personas: &[Persona],
) -> Vec<String> {
    match action {
        PersonaAction::Whisper { target_id, .. } => personas
            .iter()
            .filter(|persona| persona.id == *target_id)
            .map(|persona| persona.id.clone())
            .collect(),
        PersonaAction::Nothing => Vec::new(),
        _ => personas
            .iter()
            .filter(|persona| persona.id != actor.id)
            .map(|persona| persona.id.clone())
            .collect(),
    }
}

fn action_label(action: &PersonaAction) -> String {
    match action {
        PersonaAction::Speak { content } => format!("speak: {content}"),
        PersonaAction::Whisper { target_id, content } => {
            format!("whisper to {target_id}: {content}")
        }
        PersonaAction::Act { action } => format!("act: {action}"),
        PersonaAction::Observe => "observe".to_string(),
        PersonaAction::Nothing => "nothing".to_string(),
    }
}

fn event_impact(actor: &Persona, action: &PersonaAction) -> f64 {
    let intensity = match action {
        PersonaAction::Speak { content } => 0.3 + sentiment_score(content).abs(),
        PersonaAction::Whisper { content, .. } => 0.2 + sentiment_score(content).abs() * 0.8,
        PersonaAction::Act { action } => 0.4 + sentiment_score(action).abs(),
        PersonaAction::Observe => 0.1,
        PersonaAction::Nothing => 0.0,
    };
    (actor.normalized_influence() * intensity).clamp(0.0, 1.0)
}

fn apply_event_memories(personas: &mut [Persona], actor: &Persona, event: &WorldEvent) {
    let event_text = action_label(&event.action);
    for observer_id in &event.observers {
        if let Some(observer) = personas
            .iter_mut()
            .find(|persona| persona.id == *observer_id)
        {
            observer.remember(PersonaMemory {
                event: format!("Observed {} {}", actor.name, event_text),
                timestamp: event.tick,
                emotional_impact: event.impact,
                source: actor.id.clone(),
            });
        }
    }
}

fn apply_event_influence(
    personas: &mut [Persona],
    actor: &Persona,
    event: &WorldEvent,
    belief_update_rate: f64,
) -> Vec<(String, String, f64)> {
    let topic = infer_topic(personas, actor, event);
    let directional_shift =
        sentiment_score_from_action(&event.action) * event.impact * belief_update_rate;
    let mut belief_shifts = Vec::new();
    for observer_id in &event.observers {
        if let Some(observer) = personas
            .iter_mut()
            .find(|persona| persona.id == *observer_id)
        {
            let relationship = observer
                .relationships
                .get(&actor.id)
                .copied()
                .unwrap_or(0.0);
            let multiplier = 1.0 + relationship * 0.5;
            let new_value = (observer.beliefs.get(&topic).copied().unwrap_or(0.0)
                + directional_shift * multiplier)
                .clamp(-1.0, 1.0);
            observer.beliefs.insert(topic.clone(), new_value);
            let relation_delta = if directional_shift >= 0.0 {
                0.04
            } else {
                -0.04
            };
            observer
                .relationships
                .entry(actor.id.clone())
                .and_modify(|value| *value = (*value + relation_delta).clamp(-1.0, 1.0))
                .or_insert(relation_delta);
            belief_shifts.push((observer.id.clone(), topic.clone(), new_value));
        }
    }
    belief_shifts
}

fn infer_topic(personas: &[Persona], actor: &Persona, event: &WorldEvent) -> String {
    if let Some(topic) = actor.beliefs.keys().next() {
        return topic.clone();
    }
    for observer_id in &event.observers {
        if let Some(observer) = personas.iter().find(|persona| persona.id == *observer_id) {
            if let Some(topic) = observer.beliefs.keys().next() {
                return topic.clone();
            }
        }
    }
    "general_sentiment".to_string()
}

fn sentiment_score_from_action(action: &PersonaAction) -> f64 {
    match action {
        PersonaAction::Speak { content } => sentiment_score(content),
        PersonaAction::Whisper { content, .. } => sentiment_score(content),
        PersonaAction::Act { action } => sentiment_score(action),
        PersonaAction::Observe => 0.05,
        PersonaAction::Nothing => 0.0,
    }
}

fn sentiment_score(text: &str) -> f64 {
    let lowered = text.to_ascii_lowercase();
    let positive = ["support", "approve", "grow", "stabilize", "invest", "ally"]
        .iter()
        .filter(|token| lowered.contains(**token))
        .count() as f64;
    let negative = ["oppose", "panic", "risk", "conflict", "attack", "block"]
        .iter()
        .filter(|token| lowered.contains(**token))
        .count() as f64;
    ((positive - negative) * 0.18).clamp(-1.0, 1.0)
}

fn detect_patterns(personas: &[Persona]) -> Vec<String> {
    let mut patterns = Vec::new();
    let beliefs = summarize_beliefs(personas);
    for (topic, average) in beliefs {
        if average.abs() > 0.4 {
            patterns.push(format!(
                "{:.0}% of personas lean {} on {}",
                average.abs() * 100.0,
                if average > 0.0 {
                    "positive"
                } else {
                    "negative"
                },
                topic
            ));
        }
    }
    if patterns.is_empty() {
        patterns.push("No dominant coalition has formed yet".to_string());
    }
    patterns
}

fn summarize_beliefs(personas: &[Persona]) -> HashMap<String, f64> {
    let mut totals: HashMap<String, f64> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for persona in personas {
        for (topic, value) in &persona.beliefs {
            *totals.entry(topic.clone()).or_insert(0.0) += *value;
            *counts.entry(topic.clone()).or_insert(0) += 1;
        }
    }
    totals
        .into_iter()
        .map(|(topic, total)| {
            let count = counts.get(&topic).copied().unwrap_or(1) as f64;
            (topic, total / count)
        })
        .collect()
}

fn build_live_events(personas: &[Persona], events: &[WorldEvent]) -> Vec<SimulationLiveEvent> {
    events
        .iter()
        .map(|event| {
            let actor_name = personas
                .iter()
                .find(|persona| persona.id == event.actor_id)
                .map(|persona| persona.name.clone())
                .unwrap_or_else(|| event.actor_id.clone());
            let (action_type, content, target_id) = match &event.action {
                PersonaAction::Speak { content } => ("speak".to_string(), content.clone(), None),
                PersonaAction::Whisper { target_id, content } => (
                    "whisper".to_string(),
                    content.clone(),
                    Some(target_id.clone()),
                ),
                PersonaAction::Act { action } => ("act".to_string(), action.clone(), None),
                PersonaAction::Observe => (
                    "observe".to_string(),
                    "Observed the world state".to_string(),
                    None,
                ),
                PersonaAction::Nothing => {
                    ("nothing".to_string(), "Held position".to_string(), None)
                }
            };
            let target_name = target_id.as_ref().and_then(|id| {
                personas
                    .iter()
                    .find(|persona| &persona.id == id)
                    .map(|persona| persona.name.clone())
            });
            SimulationLiveEvent {
                actor_id: event.actor_id.clone(),
                actor_name,
                action_type,
                content,
                target_id,
                target_name,
                impact: event.impact,
            }
        })
        .collect()
}

fn persist_world_snapshot(
    db: &NexusDatabase,
    state: &PersistedSimulationState,
    report: Option<&PredictionReport>,
) -> Result<(), AgentError> {
    let world = &state.world;
    db.save_simulation_world(
        &world.id,
        &world.name,
        &world.description,
        match world.status {
            WorldStatus::Building => "building",
            WorldStatus::Running => "running",
            WorldStatus::Paused => "paused",
            WorldStatus::Completed => "completed",
        },
        world.tick_count as i64,
        world.personas.len() as i64,
        &serde_json::to_string(state)
            .map_err(|error| AgentError::SupervisorError(format!("serialize world: {error}")))?,
        report
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AgentError::SupervisorError(format!("serialize report: {error}")))?
            .as_deref(),
        report.map(|_| Utc::now().to_rfc3339()).as_deref(),
    )
    .map_err(|error| AgentError::SupervisorError(format!("persist simulation world: {error}")))?;
    db.replace_simulation_personas(
        &world.id,
        &world
            .personas
            .iter()
            .map(|persona| {
                (
                    format!("{}::{}", world.id, persona.id),
                    persona.name.clone(),
                    persona.role.clone(),
                    serde_json::to_string(&persona.personality).unwrap_or_default(),
                    serde_json::to_string(&persona.beliefs).unwrap_or_default(),
                    serde_json::to_string(&persona.memories).unwrap_or_default(),
                    serde_json::to_string(&persona.relationships).unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|error| {
        AgentError::SupervisorError(format!("persist simulation personas: {error}"))
    })?;
    Ok(())
}

fn persist_event(db: &NexusDatabase, world_id: &str, event: &WorldEvent) -> Result<(), AgentError> {
    let (action_type, content, target_id) = match &event.action {
        PersonaAction::Speak { content } => ("speak", Some(content.as_str()), None),
        PersonaAction::Whisper { target_id, content } => {
            ("whisper", Some(content.as_str()), Some(target_id.as_str()))
        }
        PersonaAction::Act { action } => ("act", Some(action.as_str()), None),
        PersonaAction::Observe => ("observe", None, None),
        PersonaAction::Nothing => ("nothing", None, None),
    };
    db.append_simulation_event(
        world_id,
        event.tick as i64,
        &event.actor_id,
        action_type,
        content,
        target_id,
        event.impact,
    )
    .map_err(|error| AgentError::SupervisorError(format!("persist simulation event: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{parse_seed, SimulatedWorld, WorldSeed};
    use crate::{cognitive::PlannerLlm, errors::AgentError};
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockPlanner {
        decisions: Mutex<VecDeque<String>>,
    }

    impl MockPlanner {
        fn with_decisions(decisions: &[&str]) -> Self {
            Self {
                decisions: Mutex::new(decisions.iter().map(|item| item.to_string()).collect()),
            }
        }
    }

    impl PlannerLlm for MockPlanner {
        fn plan_query(&self, prompt: &str) -> Result<String, AgentError> {
            if prompt.contains("Analyze this text and extract") {
                return Ok(json!({
                    "scenario": "A contested climate bill enters parliament",
                    "entities": [
                        {"name": "Parliament", "entity_type": "organization"},
                        {"name": "Climate Bill", "entity_type": "policy"}
                    ],
                    "relationships": [
                        {"from": "Parliament", "to": "Climate Bill", "relation_type": "debates"}
                    ],
                    "variables": [
                        {"key": "bill_passes", "description": "Whether the bill passes"}
                    ],
                    "suggested_personas": ["activist", "lawmaker", "journalist"]
                })
                .to_string());
            }
            if prompt.contains("Extract all entities") {
                return Ok(
                    json!({
                        "entities": [
                            {"entity_name": "Parliament", "entity_type": "organization", "properties": {"region": "EU"}},
                            {"entity_name": "Climate Bill", "entity_type": "policy", "properties": {"stage": "debate"}}
                        ],
                        "relationships": [
                            {"from": "Parliament", "to": "Climate Bill", "relation_type": "debates", "strength": 0.8}
                        ]
                    })
                    .to_string(),
                );
            }
            if prompt.contains("Generate") && prompt.contains("diverse personas") {
                let requested = prompt
                    .split("Generate ")
                    .nth(1)
                    .and_then(|rest| rest.split(" diverse personas").next())
                    .and_then(|digits| digits.parse::<usize>().ok())
                    .unwrap_or(3);
                let personas = (0..requested)
                    .map(|index| {
                        json!({
                            "id": format!("p-{index}"),
                            "name": format!("Persona {index}"),
                            "role": match index % 3 {
                                0 => "activist",
                                1 => "lawmaker",
                                _ => "journalist",
                            },
                            "personality": {
                                "openness": 0.6,
                                "conscientiousness": 0.5,
                                "extraversion": 0.6,
                                "agreeableness": 0.55,
                                "neuroticism": 0.35
                            },
                            "beliefs": {
                                "climate_bill": if index % 2 == 0 { 0.4 } else { -0.2 }
                            },
                            "goals": ["shape the public narrative"],
                            "memories": [],
                            "relationships": {},
                            "behavior_rules": ["react to news", "influence peers"],
                            "last_action": null,
                            "influence_score": 0.5 + (index as f64 * 0.05)
                        })
                    })
                    .collect::<Vec<_>>();
                return Ok(serde_json::to_string(&personas).unwrap());
            }
            if prompt.contains("Return as JSON array of persona decisions") {
                let requested = prompt.matches("\"id\":\"").count();
                let mut guard = self.decisions.lock().unwrap();
                let mut batch = Vec::with_capacity(requested.max(1));
                for index in 0..requested.max(1) {
                    let raw = guard.pop_front().unwrap_or_else(|| {
                        json!({
                            "action": "nothing",
                            "target": null,
                            "content": null,
                            "reasoning": "Default no-op."
                        })
                        .to_string()
                    });
                    let mut value: serde_json::Value =
                        serde_json::from_str(&raw).unwrap_or_else(|_| {
                            json!({
                                "action": "nothing",
                                "target": null,
                                "content": null,
                                "reasoning": "Recovered default no-op."
                            })
                        });
                    if let Some(map) = value.as_object_mut() {
                        map.insert("id".to_string(), json!(format!("p-{index}")));
                    }
                    batch.push(value);
                }
                return Ok(serde_json::to_string(&batch).unwrap());
            }
            if prompt.contains("What do you do next?") {
                let mut guard = self.decisions.lock().unwrap();
                return Ok(guard.pop_front().unwrap_or_else(|| {
                    json!({
                        "action": "nothing",
                        "target": null,
                        "content": null,
                        "reasoning": "Default no-op."
                    })
                    .to_string()
                }));
            }
            if prompt.contains("Respond in character") {
                return Ok(
                    "I still believe the bill can pass if public pressure holds.".to_string(),
                );
            }
            if prompt.contains("Analyze this simulation summary") {
                return Ok("The simulation converged toward a coordinated reform push.".to_string());
            }
            Ok("{}".to_string())
        }
    }

    fn build_seed_and_world() -> (
        WorldSeed,
        SimulatedWorld,
        Arc<MockPlanner>,
        Arc<NexusDatabase>,
    ) {
        let llm = Arc::new(MockPlanner::with_decisions(&[
            r#"{"action":"speak","target":null,"content":"I support the climate bill","reasoning":"Public pressure helps."}"#,
            r#"{"action":"whisper","target":"p-0","content":"We should coordinate support","reasoning":"Private coordination works."}"#,
            r#"{"action":"act","target":null,"content":"organize town hall","reasoning":"Visible action matters."}"#,
            r#"{"action":"observe","target":null,"content":null,"reasoning":"Taking in signals."}"#,
            r#"{"action":"nothing","target":null,"content":null,"reasoning":"Waiting."}"#,
            r#"{"action":"speak","target":null,"content":"I oppose the bill risk","reasoning":"Voicing concern."}"#,
            r#"{"action":"speak","target":null,"content":"We should stabilize support","reasoning":"Keep the coalition together."}"#,
            r#"{"action":"observe","target":null,"content":null,"reasoning":"Listening."}"#,
            r#"{"action":"observe","target":null,"content":null,"reasoning":"Listening."}"#,
        ]));
        let db = Arc::new(NexusDatabase::in_memory().unwrap());
        let seed = parse_seed("A contested climate bill enters parliament", llm.as_ref()).unwrap();
        let personas = generate_personas(&seed.scenario, 3, llm.as_ref()).unwrap();
        let world = SimulatedWorld::from_seed(
            "world-1",
            "Climate Vote",
            seed.scenario.clone(),
            &seed,
            personas,
            llm.as_ref(),
        )
        .unwrap();
        (seed, world, llm, db)
    }

    #[test]
    fn test_parse_seed_text_into_entities_and_relationships() {
        let llm = MockPlanner::default();
        let seed = parse_seed("text", &llm).unwrap();
        assert_eq!(seed.entities.len(), 2);
        assert_eq!(seed.relationships[0].relation_type, "debates");
    }

    #[test]
    fn test_world_model_build_from_seed() {
        let llm = MockPlanner::default();
        let world_model =
            crate::cognitive::algorithms::WorldModel::build_from_seed("seed", &llm).unwrap();
        assert_eq!(world_model.entities.len(), 2);
        assert_eq!(world_model.relationships.len(), 1);
    }

    #[test]
    fn test_generate_personas_from_seed() {
        let llm = MockPlanner::default();
        let personas = generate_personas("seed", 3, &llm).unwrap();
        assert_eq!(personas.len(), 3);
        assert_eq!(personas[0].role, "activist");
    }

    #[test]
    fn test_persona_decide_returns_speak_action() {
        let (.., llm, _) = build_seed_and_world();
        let personas = generate_personas("seed", 2, llm.as_ref()).unwrap();
        let decision = persona_decide(
            &personas[0],
            &super::super::world::WorldEnvironment::default(),
            &[&personas[1]],
            llm.as_ref(),
        )
        .unwrap();
        assert!(matches!(decision.action, PersonaAction::Speak { .. }));
    }

    #[test]
    fn test_persona_decide_returns_whisper_action() {
        let llm = MockPlanner::with_decisions(&[
            r#"{"action":"whisper","target":"p-1","content":"quiet support","reasoning":"Private note"}"#,
        ]);
        let personas = generate_personas("seed", 2, &llm).unwrap();
        let decision = persona_decide(
            &personas[0],
            &super::super::world::WorldEnvironment::default(),
            &[&personas[1]],
            &llm,
        )
        .unwrap();
        assert!(matches!(decision.action, PersonaAction::Whisper { .. }));
    }

    #[test]
    fn test_create_world_from_seed() {
        let (seed, world, ..) = build_seed_and_world();
        assert_eq!(world.description, seed.scenario);
        assert_eq!(world.personas.len(), 3);
    }

    #[test]
    fn test_run_five_ticks_with_three_personas() {
        let (_, world, llm, db) = build_seed_and_world();
        let mut runtime = SimulationRuntime::new(world, llm, db);
        runtime.tick_interval_ms = 0;
        runtime.max_ticks = 5;
        runtime.batch_size = 3;
        let report = runtime.run_simulation().unwrap();
        assert!(runtime.world.tick_count <= 5);
        assert!(!report.summary.is_empty());
    }

    #[test]
    fn test_belief_shifts_after_events() {
        let (_, world, llm, db) = build_seed_and_world();
        let original = world.personas[1]
            .beliefs
            .get("climate_bill")
            .copied()
            .unwrap_or_default();
        let mut runtime = SimulationRuntime::new(world, llm, db);
        runtime.tick_interval_ms = 0;
        runtime.max_ticks = 2;
        runtime.run_simulation().unwrap();
        let new_value = runtime.world.personas[1]
            .beliefs
            .get("climate_bill")
            .copied()
            .unwrap_or_default();
        assert_ne!(original, new_value);
    }

    #[test]
    fn test_inject_variable_changes_environment() {
        let (_, world, llm, db) = build_seed_and_world();
        let mut runtime = SimulationRuntime::new(world, llm, db);
        runtime.inject_variable("bill_passes", "true").unwrap();
        assert_eq!(
            runtime.world.environment.global_state.get("bill_passes"),
            Some(&"true".to_string())
        );
    }

    #[test]
    fn test_convergence_detection_stops_simulation_early() {
        let llm = Arc::new(MockPlanner::with_decisions(&[
            r#"{"action":"nothing","target":null,"content":null,"reasoning":"stable"}"#,
            r#"{"action":"nothing","target":null,"content":null,"reasoning":"stable"}"#,
            r#"{"action":"nothing","target":null,"content":null,"reasoning":"stable"}"#,
        ]));
        let seed = parse_seed("seed", llm.as_ref()).unwrap();
        let personas = generate_personas(&seed.scenario, 3, llm.as_ref()).unwrap();
        let world = SimulatedWorld::from_seed(
            "stable-world",
            "Stable",
            seed.scenario.clone(),
            &seed,
            personas,
            llm.as_ref(),
        )
        .unwrap();
        let db = Arc::new(NexusDatabase::in_memory().unwrap());
        let mut runtime = SimulationRuntime::new(world, llm, db);
        runtime.tick_interval_ms = 0;
        runtime.max_ticks = 20;
        runtime.run_simulation().unwrap();
        assert!(runtime.has_stabilized());
        assert!(runtime.world.tick_count < 20);
    }

    #[test]
    fn test_chat_with_persona_returns_in_character_response() {
        let (_, world, llm, db) = build_seed_and_world();
        let runtime = SimulationRuntime::new(world, llm, db);
        let response = runtime
            .chat_with_persona("p-0", "What happens next?")
            .unwrap();
        assert!(response.contains("bill"));
    }

    #[test]
    fn test_generate_report_from_completed_simulation() {
        let (_, world, llm, db) = build_seed_and_world();
        let mut runtime = SimulationRuntime::new(world, llm.clone(), db);
        runtime.tick_interval_ms = 0;
        runtime.max_ticks = 3;
        let report = runtime.run_simulation().unwrap();
        assert!(!report.key_findings.is_empty());
        assert!(report.confidence > 0.0);
    }

    #[test]
    fn test_parallel_simulations_return_multiple_reports() {
        let (seed, _, llm, db) = build_seed_and_world();
        let reports = run_parallel_simulations(&seed, 3, llm, db).unwrap();
        assert_eq!(reports.len(), 3);
    }

    #[test]
    fn test_parallel_confidence_reflects_convergence() {
        let reports = vec![
            PredictionReport {
                summary: "a".to_string(),
                key_findings: vec![],
                opinion_shifts: vec![],
                coalitions: vec![],
                turning_points: vec![],
                prediction: "Outcome X".to_string(),
                confidence: 0.7,
                uncertainties: vec!["swing voters".to_string()],
            },
            PredictionReport {
                summary: "b".to_string(),
                key_findings: vec![],
                opinion_shifts: vec![],
                coalitions: vec![],
                turning_points: vec![],
                prediction: "Outcome X".to_string(),
                confidence: 0.8,
                uncertainties: vec!["markets".to_string()],
            },
            PredictionReport {
                summary: "c".to_string(),
                key_findings: vec![],
                opinion_shifts: vec![],
                coalitions: vec![],
                turning_points: vec![],
                prediction: "Outcome Y".to_string(),
                confidence: 0.4,
                uncertainties: vec!["coalition split".to_string()],
            },
        ];
        let analysis = compare_reports(&reports);
        assert_eq!(analysis.consensus_prediction, "Outcome X");
        assert!(analysis.confidence > 0.5);
    }

    #[test]
    fn test_evolve_simulation_returns_config() {
        let (seed, _, llm, db) = build_seed_and_world();
        let config = evolve_simulation(&seed, 2, llm, db).unwrap();
        assert!(config.persona_count >= 6);
        assert!(config.consistency_score >= 0.0);
    }

    #[test]
    fn test_fuel_consumed_per_persona_decision() {
        let (_, world, llm, db) = build_seed_and_world();
        let mut runtime = SimulationRuntime::new(world, llm, db);
        runtime.tick_interval_ms = 0;
        runtime.max_ticks = 1;
        runtime.run_simulation().unwrap();
        assert!(runtime.fuel_consumed() > 0.0);
    }

    #[test]
    fn test_estimate_simulation_fuel_positive() {
        assert!(estimate_simulation_fuel(10, 5, 2) > 0);
    }

    #[test]
    fn test_persisted_world_status_becomes_completed() {
        let (_, world, llm, db) = build_seed_and_world();
        let mut runtime = SimulationRuntime::new(world, llm, db.clone());
        runtime.tick_interval_ms = 0;
        runtime.max_ticks = 1;
        runtime.run_simulation().unwrap();
        let row = db.load_simulation_world("world-1").unwrap().unwrap();
        assert_eq!(row.status, "completed");
    }

    #[test]
    fn test_observers_receive_memories() {
        let (_, world, llm, db) = build_seed_and_world();
        let mut runtime = SimulationRuntime::new(world, llm, db);
        runtime.tick_interval_ms = 0;
        runtime.max_ticks = 1;
        runtime.run_simulation().unwrap();
        assert!(!runtime.world.personas[1].memories.is_empty());
    }

    #[test]
    fn test_meta_analysis_captures_divergence_factors() {
        let reports = vec![
            PredictionReport {
                summary: String::new(),
                key_findings: vec![],
                opinion_shifts: vec![],
                coalitions: vec![],
                turning_points: vec![],
                prediction: "A".to_string(),
                confidence: 0.5,
                uncertainties: vec!["factor-1".to_string()],
            },
            PredictionReport {
                summary: String::new(),
                key_findings: vec![],
                opinion_shifts: vec![],
                coalitions: vec![],
                turning_points: vec![],
                prediction: "B".to_string(),
                confidence: 0.5,
                uncertainties: vec!["factor-2".to_string()],
            },
        ];
        let analysis = compare_reports(&reports);
        assert!(!analysis.divergence_factors.is_empty());
    }
}
