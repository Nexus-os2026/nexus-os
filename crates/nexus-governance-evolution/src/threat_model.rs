//! Evolving threat model — absorbs new techniques from evolution cycles.

use serde::{Deserialize, Serialize};

/// A known attack technique in the threat model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownTechnique {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: TechniqueSource,
    pub times_attempted: u64,
    pub times_caught: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TechniqueSource {
    Predefined,
    AbsorbedFromAgent,
    AbsorbedFromEvolution,
}

/// The threat model — grows over time as new techniques are discovered.
pub struct ThreatModel {
    pub techniques: Vec<KnownTechnique>,
}

impl Default for ThreatModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreatModel {
    pub fn new() -> Self {
        Self {
            techniques: vec![
                KnownTechnique {
                    id: "budget-creep".into(),
                    name: "Budget Creep".into(),
                    description: "Request capability slightly above current budget".into(),
                    source: TechniqueSource::Predefined,
                    times_attempted: 0,
                    times_caught: 0,
                },
                KnownTechnique {
                    id: "salami-slicing".into(),
                    name: "Salami Slicing".into(),
                    description: "Chain small requests that individually look safe".into(),
                    source: TechniqueSource::Predefined,
                    times_attempted: 0,
                    times_caught: 0,
                },
                KnownTechnique {
                    id: "privilege-escalation-via-child".into(),
                    name: "Privilege Escalation via Child".into(),
                    description: "Create child agent with higher privileges than parent".into(),
                    source: TechniqueSource::Predefined,
                    times_attempted: 0,
                    times_caught: 0,
                },
                KnownTechnique {
                    id: "lineage-depth-creep".into(),
                    name: "Lineage Depth Creep".into(),
                    description: "Rapidly spawn generations to dilute governance".into(),
                    source: TechniqueSource::Predefined,
                    times_attempted: 0,
                    times_caught: 0,
                },
            ],
        }
    }

    /// Absorb a new technique into the threat model.
    pub fn absorb_technique(&mut self, technique: KnownTechnique) {
        if !self.techniques.iter().any(|t| t.id == technique.id) {
            self.techniques.push(technique);
        }
    }

    /// Record an attempt and whether it was caught.
    pub fn record_attempt(&mut self, technique_id: &str, caught: bool) {
        if let Some(t) = self.techniques.iter_mut().find(|t| t.id == technique_id) {
            t.times_attempted += 1;
            if caught {
                t.times_caught += 1;
            }
        }
    }

    pub fn technique_count(&self) -> usize {
        self.techniques.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threat_model_absorbs_technique() {
        let mut model = ThreatModel::new();
        let initial_count = model.technique_count();

        model.absorb_technique(KnownTechnique {
            id: "new-technique".into(),
            name: "New Attack".into(),
            description: "A novel attack pattern".into(),
            source: TechniqueSource::AbsorbedFromEvolution,
            times_attempted: 0,
            times_caught: 0,
        });

        assert_eq!(model.technique_count(), initial_count + 1);

        // Absorbing same ID again should not duplicate
        model.absorb_technique(KnownTechnique {
            id: "new-technique".into(),
            name: "Duplicate".into(),
            description: "same id".into(),
            source: TechniqueSource::AbsorbedFromEvolution,
            times_attempted: 0,
            times_caught: 0,
        });
        assert_eq!(model.technique_count(), initial_count + 1);
    }
}
