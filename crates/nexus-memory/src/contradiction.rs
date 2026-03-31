//! Contradiction detection and resolution for semantic memory entries.
//!
//! When a new semantic entry arrives, [`detect_contradictions`] checks existing
//! entries for conflicts.  Contradictions are **never** silently resolved — they
//! are always surfaced with a recommended resolution strategy.
//!
//! ## Detection rules
//!
//! | Content type | Contradiction condition |
//! |---|---|
//! | Triple | Same subject + predicate, different object |
//! | EntityRecord | Same name + type, conflicting attribute values |
//! | Assertion | Negation patterns ("X is Y" vs "X is not Y") |
//! | TemporalFact | Overlapping time ranges with different statements |

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::*;

/// A detected contradiction between memory entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    /// The ID of the existing entry that conflicts.
    pub existing_entry_id: MemoryId,
    /// The ID of the new entry being written.
    pub new_entry_id: MemoryId,
    /// What kind of contradiction was detected.
    pub contradiction_type: ContradictionType,
    /// The recommended way to resolve this contradiction.
    pub recommended_resolution: ContradictionResolution,
}

/// The specific nature of a contradiction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContradictionType {
    /// Same subject+predicate, different object (Triple).
    ConflictingTriple {
        subject: String,
        predicate: String,
        existing_object: String,
        new_object: String,
    },
    /// Same entity, conflicting attribute values.
    ConflictingAttribute {
        entity_name: String,
        attribute: String,
        existing_value: serde_json::Value,
        new_value: serde_json::Value,
    },
    /// Directly contradicting assertions (detected by negation keywords).
    ConflictingAssertion {
        existing_statement: String,
        new_statement: String,
    },
    /// Overlapping temporal facts with different values.
    TemporalOverlap {
        existing_statement: String,
        new_statement: String,
        overlap_start: DateTime<Utc>,
        overlap_end: Option<DateTime<Utc>>,
    },
}

/// How to resolve a contradiction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContradictionResolution {
    /// Both kept; the agent sees both with explanatory context.
    CoexistWithContext { context_note: String },
    /// New entry supersedes the old one (strict conditions met).
    Supersede { reason: String },
    /// Flagged for human review.
    FlagForReview { reason: String },
    /// Temporal resolution — old fact's `valid_to` set to now.
    TemporalSuccession { reason: String },
}

// ── Detection ────────────────────────────────────────────────────────────────

/// Scans `existing_entries` for contradictions with `new_entry`.
///
/// Returns all detected contradictions with recommended resolutions.
pub fn detect_contradictions(
    existing_entries: &[MemoryEntry],
    new_entry: &MemoryEntry,
) -> Vec<Contradiction> {
    let mut contradictions = Vec::new();

    for existing in existing_entries {
        if existing.validation_state == ValidationState::Revoked {
            continue; // skip already-invalidated entries
        }

        if let Some(ct) = detect_pair(existing, new_entry) {
            let resolution = recommend_resolution(existing, new_entry, &ct);
            contradictions.push(Contradiction {
                existing_entry_id: existing.id,
                new_entry_id: new_entry.id,
                contradiction_type: ct,
                recommended_resolution: resolution,
            });
        }
    }

    contradictions
}

/// Checks a single existing/new pair for contradiction.
fn detect_pair(existing: &MemoryEntry, new: &MemoryEntry) -> Option<ContradictionType> {
    match (&existing.content, &new.content) {
        // Triple: same subject + predicate, different object
        (
            MemoryContent::Triple {
                subject: es,
                predicate: ep,
                object: eo,
            },
            MemoryContent::Triple {
                subject: ns,
                predicate: np,
                object: no,
            },
        ) => {
            if eq_ci(es, ns) && eq_ci(ep, np) && !eq_ci(eo, no) {
                Some(ContradictionType::ConflictingTriple {
                    subject: ns.clone(),
                    predicate: np.clone(),
                    existing_object: eo.clone(),
                    new_object: no.clone(),
                })
            } else {
                None
            }
        }

        // EntityRecord: same name + type, conflicting attributes
        (
            MemoryContent::EntityRecord {
                name: en,
                entity_type: et,
                attributes: ea,
            },
            MemoryContent::EntityRecord {
                name: nn,
                entity_type: nt,
                attributes: na,
            },
        ) => {
            if eq_ci(en, nn) && eq_ci(et, nt) {
                // Find first conflicting attribute
                for (key, new_val) in na {
                    if let Some(existing_val) = ea.get(key) {
                        if existing_val != new_val {
                            return Some(ContradictionType::ConflictingAttribute {
                                entity_name: nn.clone(),
                                attribute: key.clone(),
                                existing_value: existing_val.clone(),
                                new_value: new_val.clone(),
                            });
                        }
                    }
                }
                None
            } else {
                None
            }
        }

        // Assertion: detect negation patterns
        (
            MemoryContent::Assertion { statement: es, .. },
            MemoryContent::Assertion { statement: ns, .. },
        ) => detect_assertion_contradiction(es, ns),

        // TemporalFact: overlapping time ranges with different statements
        (
            MemoryContent::TemporalFact {
                statement: es,
                effective_from: ef,
                effective_to: et,
                ..
            },
            MemoryContent::TemporalFact {
                statement: ns,
                effective_from: nf,
                effective_to: nt,
                ..
            },
        ) => {
            if es == ns {
                return None; // same statement — no contradiction
            }

            // Check overlap: existing_from < new_to AND new_from < existing_to
            let existing_end = et.unwrap_or(DateTime::<Utc>::MAX_UTC);
            let new_end = nt.unwrap_or(DateTime::<Utc>::MAX_UTC);

            if *ef < new_end && *nf < existing_end {
                let overlap_start = (*ef).max(*nf);
                let overlap_end_dt = existing_end.min(new_end);
                let overlap_end = if overlap_end_dt == DateTime::<Utc>::MAX_UTC {
                    None
                } else {
                    Some(overlap_end_dt)
                };

                Some(ContradictionType::TemporalOverlap {
                    existing_statement: es.clone(),
                    new_statement: ns.clone(),
                    overlap_start,
                    overlap_end,
                })
            } else {
                None
            }
        }

        _ => None,
    }
}

/// Heuristic assertion contradiction detection.
///
/// Looks for negation patterns: "X is Y" vs "X is not Y",
/// "X has Y" vs "X does not have Y", etc.
fn detect_assertion_contradiction(existing: &str, new: &str) -> Option<ContradictionType> {
    let el = existing.to_lowercase();
    let nl = new.to_lowercase();

    // Pattern: one statement is the negation of the other
    let negation_pairs = [
        (" is ", " is not "),
        (" has ", " has not "),
        (" has ", " does not have "),
        (" can ", " cannot "),
        (" can ", " can not "),
        (" will ", " will not "),
        (" should ", " should not "),
    ];

    for (positive, negative) in &negation_pairs {
        // Check if existing is positive and new is negative (or vice versa)
        if (el.contains(positive) && nl.contains(negative))
            || (el.contains(negative) && nl.contains(positive))
        {
            // Extract subject (text before the verb)
            let es = extract_subject(&el, positive, negative);
            let ns = extract_subject(&nl, positive, negative);

            // If subjects overlap, likely a contradiction
            if let (Some(es), Some(ns)) = (es, ns) {
                if eq_ci(&es, &ns) {
                    return Some(ContradictionType::ConflictingAssertion {
                        existing_statement: existing.into(),
                        new_statement: new.into(),
                    });
                }
            }
        }
    }

    None
}

/// Extracts the subject (text before the verb phrase) from a statement.
fn extract_subject(text: &str, positive: &str, negative: &str) -> Option<String> {
    if let Some(idx) = text.find(negative) {
        Some(text[..idx].trim().to_string())
    } else {
        text.find(positive)
            .map(|idx| text[..idx].trim().to_string())
    }
}

// ── Resolution ───────────────────────────────────────────────────────────────

/// Recommends a resolution strategy based on trust, confidence, recency, and
/// validation state of both entries.
pub fn recommend_resolution(
    existing: &MemoryEntry,
    new: &MemoryEntry,
    contradiction: &ContradictionType,
) -> ContradictionResolution {
    // 1. Auto-supersede when ALL conditions met:
    //    - new.trust > existing.trust
    //    - new.confidence >= existing.confidence
    //    - existing last accessed > 7 days ago
    //    - existing is NOT Corroborated
    let seven_days_ago = Utc::now() - chrono::Duration::days(7);
    if new.trust_score > existing.trust_score
        && new.confidence >= existing.confidence
        && existing.last_accessed < seven_days_ago
        && existing.validation_state != ValidationState::Corroborated
    {
        return ContradictionResolution::Supersede {
            reason: format!(
                "New entry has higher trust ({:.2} > {:.2}) and confidence ({:.2} >= {:.2}), \
                 existing entry last accessed >7 days ago",
                new.trust_score, existing.trust_score, new.confidence, existing.confidence,
            ),
        };
    }

    // 2. Temporal succession for TemporalFact
    if let ContradictionType::TemporalOverlap { .. } = contradiction {
        if new.valid_from > existing.valid_from {
            return ContradictionResolution::TemporalSuccession {
                reason: format!(
                    "New temporal fact starts later ({} > {})",
                    new.valid_from.format("%Y-%m-%d"),
                    existing.valid_from.format("%Y-%m-%d"),
                ),
            };
        }
    }

    // 3. Flag for review when both entries are high-trust
    if new.trust_score > 0.7 && existing.trust_score > 0.7 {
        return ContradictionResolution::FlagForReview {
            reason: format!(
                "Both entries have high trust ({:.2} and {:.2}) — requires human judgment",
                existing.trust_score, new.trust_score,
            ),
        };
    }

    // 4. Default: coexist
    let note = match contradiction {
        ContradictionType::ConflictingTriple {
            subject,
            predicate,
            existing_object,
            new_object,
        } => format!(
            "Conflicting values for {subject}.{predicate}: \"{existing_object}\" vs \"{new_object}\""
        ),
        ContradictionType::ConflictingAttribute {
            entity_name,
            attribute,
            ..
        } => format!("Conflicting attribute '{attribute}' on entity '{entity_name}'"),
        ContradictionType::ConflictingAssertion { .. } => {
            "Potentially contradicting assertions detected".into()
        }
        ContradictionType::TemporalOverlap { .. } => {
            "Overlapping temporal facts with different statements".into()
        }
    };

    ContradictionResolution::CoexistWithContext { context_note: note }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Case-insensitive equality.
fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_entry(content: MemoryContent, trust: f32, confidence: f32) -> MemoryEntry {
        let now = Utc::now();
        MemoryEntry {
            id: Uuid::new_v4(),
            schema_version: 1,
            agent_id: "agent-1".into(),
            memory_type: MemoryType::Semantic,
            epistemic_class: EpistemicClass::Observation,
            validation_state: ValidationState::Unverified,
            content,
            embedding: None,
            created_at: now,
            updated_at: now,
            valid_from: now,
            valid_to: None,
            trust_score: trust,
            importance: 0.5,
            confidence,
            supersedes: None,
            derived_from: vec![],
            source_task_id: None,
            source_conversation_id: None,
            scope: MemoryScope::Agent,
            sensitivity: SensitivityClass::Internal,
            access_count: 0,
            last_accessed: now,
            version: 1,
            ttl: None,
            tags: vec![],
        }
    }

    fn triple(subject: &str, predicate: &str, object: &str) -> MemoryContent {
        MemoryContent::Triple {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }

    fn entity(name: &str, etype: &str, attrs: Vec<(&str, serde_json::Value)>) -> MemoryContent {
        let mut attributes = HashMap::new();
        for (k, v) in attrs {
            attributes.insert(k.into(), v);
        }
        MemoryContent::EntityRecord {
            name: name.into(),
            entity_type: etype.into(),
            attributes,
        }
    }

    fn assertion(statement: &str) -> MemoryContent {
        MemoryContent::Assertion {
            statement: statement.into(),
            citations: vec![],
        }
    }

    fn temporal(statement: &str, from: DateTime<Utc>, to: Option<DateTime<Utc>>) -> MemoryContent {
        MemoryContent::TemporalFact {
            statement: statement.into(),
            effective_from: from,
            effective_to: to,
            context: "test".into(),
        }
    }

    // ── Triple contradictions ────────────────────────────────────────────

    #[test]
    fn detect_conflicting_triples() {
        let existing = make_entry(triple("Earth", "orbits", "Sun"), 0.9, 0.9);
        let new = make_entry(triple("Earth", "orbits", "Moon"), 0.5, 0.5);

        let contradictions = detect_contradictions(&[existing], &new);
        assert_eq!(contradictions.len(), 1);
        assert!(matches!(
            contradictions[0].contradiction_type,
            ContradictionType::ConflictingTriple { .. }
        ));
    }

    #[test]
    fn no_contradiction_different_subject() {
        let existing = make_entry(triple("Earth", "orbits", "Sun"), 0.9, 0.9);
        let new = make_entry(triple("Mars", "orbits", "Sun"), 0.9, 0.9);

        let contradictions = detect_contradictions(&[existing], &new);
        assert!(contradictions.is_empty());
    }

    #[test]
    fn no_contradiction_different_predicate() {
        let existing = make_entry(triple("Earth", "orbits", "Sun"), 0.9, 0.9);
        let new = make_entry(triple("Earth", "radius", "6371km"), 0.9, 0.9);

        let contradictions = detect_contradictions(&[existing], &new);
        assert!(contradictions.is_empty());
    }

    #[test]
    fn triple_contradiction_case_insensitive() {
        let existing = make_entry(triple("earth", "orbits", "Sun"), 0.9, 0.9);
        let new = make_entry(triple("Earth", "ORBITS", "Moon"), 0.5, 0.5);

        let contradictions = detect_contradictions(&[existing], &new);
        assert_eq!(contradictions.len(), 1);
    }

    // ── EntityRecord contradictions ──────────────────────────────────────

    #[test]
    fn detect_conflicting_entity_attributes() {
        let existing = make_entry(
            entity(
                "Nexus",
                "Software",
                vec![("version", serde_json::json!("9.0"))],
            ),
            0.9,
            0.9,
        );
        let new = make_entry(
            entity(
                "Nexus",
                "Software",
                vec![("version", serde_json::json!("10.0"))],
            ),
            0.9,
            0.9,
        );

        let contradictions = detect_contradictions(&[existing], &new);
        assert_eq!(contradictions.len(), 1);
        assert!(matches!(
            contradictions[0].contradiction_type,
            ContradictionType::ConflictingAttribute { .. }
        ));
    }

    #[test]
    fn no_contradiction_different_entity_type() {
        let existing = make_entry(
            entity(
                "Nexus",
                "Software",
                vec![("version", serde_json::json!("9.0"))],
            ),
            0.9,
            0.9,
        );
        let new = make_entry(
            entity(
                "Nexus",
                "Company",
                vec![("version", serde_json::json!("10.0"))],
            ),
            0.9,
            0.9,
        );

        let contradictions = detect_contradictions(&[existing], &new);
        assert!(contradictions.is_empty());
    }

    // ── Assertion contradictions ─────────────────────────────────────────

    #[test]
    fn detect_assertion_negation() {
        let existing = make_entry(assertion("Rust is memory safe"), 0.9, 0.9);
        let new = make_entry(assertion("Rust is not memory safe"), 0.5, 0.5);

        let contradictions = detect_contradictions(&[existing], &new);
        assert_eq!(contradictions.len(), 1);
        assert!(matches!(
            contradictions[0].contradiction_type,
            ContradictionType::ConflictingAssertion { .. }
        ));
    }

    // ── Temporal contradictions ──────────────────────────────────────────

    #[test]
    fn detect_temporal_overlap() {
        let t1 = Utc::now() - chrono::Duration::days(10);
        let t2 = Utc::now() - chrono::Duration::days(5);
        let t3 = Utc::now();

        let existing = make_entry(temporal("CEO is Alice", t1, Some(t3)), 0.9, 0.9);
        let new = make_entry(temporal("CEO is Bob", t2, None), 0.9, 0.9);

        let contradictions = detect_contradictions(&[existing], &new);
        assert_eq!(contradictions.len(), 1);
        assert!(matches!(
            contradictions[0].contradiction_type,
            ContradictionType::TemporalOverlap { .. }
        ));
    }

    #[test]
    fn no_temporal_overlap_disjoint() {
        let t1 = Utc::now() - chrono::Duration::days(20);
        let t2 = Utc::now() - chrono::Duration::days(10);
        let t3 = Utc::now() - chrono::Duration::days(5);

        let existing = make_entry(temporal("CEO is Alice", t1, Some(t2)), 0.9, 0.9);
        let new = make_entry(temporal("CEO is Bob", t3, None), 0.9, 0.9);

        let contradictions = detect_contradictions(&[existing], &new);
        assert!(contradictions.is_empty());
    }

    // ── Resolution logic ─────────────────────────────────────────────────

    #[test]
    fn resolution_supersede_when_conditions_met() {
        let mut existing = make_entry(triple("X", "is", "old"), 0.3, 0.5);
        existing.last_accessed = Utc::now() - chrono::Duration::days(10);

        let new = make_entry(triple("X", "is", "new"), 0.9, 0.9);

        let ct = ContradictionType::ConflictingTriple {
            subject: "X".into(),
            predicate: "is".into(),
            existing_object: "old".into(),
            new_object: "new".into(),
        };

        let resolution = recommend_resolution(&existing, &new, &ct);
        assert!(
            matches!(resolution, ContradictionResolution::Supersede { .. }),
            "Expected Supersede, got {resolution:?}"
        );
    }

    #[test]
    fn resolution_coexist_when_neither_dominates() {
        let existing = make_entry(triple("X", "is", "A"), 0.5, 0.5);
        let new = make_entry(triple("X", "is", "B"), 0.5, 0.5);

        let ct = ContradictionType::ConflictingTriple {
            subject: "X".into(),
            predicate: "is".into(),
            existing_object: "A".into(),
            new_object: "B".into(),
        };

        let resolution = recommend_resolution(&existing, &new, &ct);
        assert!(matches!(
            resolution,
            ContradictionResolution::CoexistWithContext { .. }
        ));
    }

    #[test]
    fn resolution_flag_for_review_both_high_trust() {
        let existing = make_entry(triple("X", "is", "A"), 0.9, 0.9);
        let new = make_entry(triple("X", "is", "B"), 0.8, 0.8);

        let ct = ContradictionType::ConflictingTriple {
            subject: "X".into(),
            predicate: "is".into(),
            existing_object: "A".into(),
            new_object: "B".into(),
        };

        let resolution = recommend_resolution(&existing, &new, &ct);
        assert!(matches!(
            resolution,
            ContradictionResolution::FlagForReview { .. }
        ));
    }

    #[test]
    fn resolution_temporal_succession() {
        let t1 = Utc::now() - chrono::Duration::days(10);
        let t2 = Utc::now() - chrono::Duration::days(5);

        let mut existing = make_entry(temporal("CEO is Alice", t1, None), 0.5, 0.5);
        existing.valid_from = t1;
        let mut new = make_entry(temporal("CEO is Bob", t2, None), 0.5, 0.5);
        new.valid_from = t2;

        let ct = ContradictionType::TemporalOverlap {
            existing_statement: "CEO is Alice".into(),
            new_statement: "CEO is Bob".into(),
            overlap_start: t2,
            overlap_end: None,
        };

        let resolution = recommend_resolution(&existing, &new, &ct);
        assert!(matches!(
            resolution,
            ContradictionResolution::TemporalSuccession { .. }
        ));
    }

    #[test]
    fn skips_revoked_entries() {
        let mut existing = make_entry(triple("X", "is", "old"), 0.9, 0.9);
        existing.validation_state = ValidationState::Revoked;

        let new = make_entry(triple("X", "is", "new"), 0.9, 0.9);

        let contradictions = detect_contradictions(&[existing], &new);
        assert!(contradictions.is_empty());
    }
}
