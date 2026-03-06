use crate::audit::{AuditTrail, EventType};
use crate::safety_supervisor::KpiKind;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KillGateConfig {
    pub screen_poster_freeze_threshold: f64,
    pub screen_poster_halt_threshold: f64,
    pub mutation_freeze_threshold: f64,
    pub mutation_halt_threshold: f64,
    pub cluster_freeze_threshold: f64,
    pub cluster_halt_threshold: f64,
    pub bft_freeze_threshold: f64,
    pub bft_halt_threshold: f64,
}

impl Default for KillGateConfig {
    fn default() -> Self {
        Self {
            screen_poster_freeze_threshold: 2.0,
            screen_poster_halt_threshold: 5.0,
            mutation_freeze_threshold: 1.0,
            mutation_halt_threshold: f64::INFINITY,
            cluster_freeze_threshold: 1.0,
            cluster_halt_threshold: f64::INFINITY,
            bft_freeze_threshold: f64::INFINITY,
            bft_halt_threshold: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KillGate {
    pub subsystem: String,
    pub metric_kind: KpiKind,
    pub freeze_threshold: f64,
    pub halt_threshold: f64,
    pub frozen: bool,
    pub halted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    Open,
    Frozen,
    Halted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationLevel {
    Warn,
    Degrade,
    Freeze,
    Halt,
    Incident,
}

impl EscalationLevel {
    pub fn next(self) -> Self {
        match self {
            EscalationLevel::Warn => EscalationLevel::Degrade,
            EscalationLevel::Degrade => EscalationLevel::Freeze,
            EscalationLevel::Freeze => EscalationLevel::Halt,
            EscalationLevel::Halt => EscalationLevel::Incident,
            EscalationLevel::Incident => EscalationLevel::Incident,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            EscalationLevel::Warn => "warn",
            EscalationLevel::Degrade => "degrade",
            EscalationLevel::Freeze => "freeze",
            EscalationLevel::Halt => "halt",
            EscalationLevel::Incident => "incident",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KillGateError {
    #[error("unknown subsystem '{0}'")]
    UnknownSubsystem(String),
    #[error("HITL Tier3 is required to unfreeze")]
    Tier3Required,
}

#[derive(Debug, Clone)]
pub struct KillGateRegistry {
    pub gates: HashMap<String, KillGate>,
    escalation_levels: HashMap<String, EscalationLevel>,
}

impl Default for KillGateRegistry {
    fn default() -> Self {
        Self::from_config(&KillGateConfig::default())
    }
}

impl KillGateRegistry {
    pub fn from_config(config: &KillGateConfig) -> Self {
        let mut gates = HashMap::new();

        gates.insert(
            "screen_poster".to_string(),
            KillGate {
                subsystem: "screen_poster".to_string(),
                metric_kind: KpiKind::BanRate,
                freeze_threshold: config.screen_poster_freeze_threshold,
                halt_threshold: config.screen_poster_halt_threshold,
                frozen: false,
                halted: false,
            },
        );
        gates.insert(
            "mutation".to_string(),
            KillGate {
                subsystem: "mutation".to_string(),
                metric_kind: KpiKind::ReplayMismatch,
                freeze_threshold: config.mutation_freeze_threshold,
                halt_threshold: config.mutation_halt_threshold,
                frozen: false,
                halted: false,
            },
        );
        gates.insert(
            "cluster".to_string(),
            KillGate {
                subsystem: "cluster".to_string(),
                metric_kind: KpiKind::Divergence,
                freeze_threshold: config.cluster_freeze_threshold,
                halt_threshold: config.cluster_halt_threshold,
                frozen: false,
                halted: false,
            },
        );
        gates.insert(
            "bft".to_string(),
            KillGate {
                subsystem: "bft".to_string(),
                metric_kind: KpiKind::QuorumInvariant,
                freeze_threshold: config.bft_freeze_threshold,
                halt_threshold: config.bft_halt_threshold,
                frozen: false,
                halted: false,
            },
        );

        Self {
            gates,
            escalation_levels: HashMap::new(),
        }
    }

    pub fn gate_status(&self, subsystem: &str) -> Option<GateStatus> {
        self.gates.get(subsystem).map(|gate| {
            if gate.halted {
                GateStatus::Halted
            } else if gate.frozen {
                GateStatus::Frozen
            } else {
                GateStatus::Open
            }
        })
    }

    pub fn check_metric(
        &mut self,
        kind: KpiKind,
        metric_value: f64,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) -> Vec<(String, GateStatus)> {
        let subsystems = self
            .gates
            .iter()
            .filter_map(|(name, gate)| {
                if gate.metric_kind == kind {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let mut results = Vec::with_capacity(subsystems.len());
        for subsystem in subsystems {
            let status = self.check_gate(subsystem.as_str(), metric_value, agent_id, audit);
            results.push((subsystem, status));
        }
        results
    }

    pub fn check_gate(
        &mut self,
        subsystem: &str,
        metric_value: f64,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) -> GateStatus {
        let Some(gate) = self.gates.get_mut(subsystem) else {
            return GateStatus::Open;
        };

        let mut status = if gate.halted {
            GateStatus::Halted
        } else if gate.frozen {
            GateStatus::Frozen
        } else {
            GateStatus::Open
        };

        if !gate.halted && metric_value >= gate.halt_threshold {
            gate.halted = true;
            gate.frozen = true;
            status = GateStatus::Halted;

            let _ = audit.append_event(
                agent_id,
                EventType::Error,
                json!({
                    "event_kind": "killgate.halted",
                    "subsystem": subsystem,
                    "reason": format!("metric {} exceeded halt threshold {}", metric_value, gate.halt_threshold),
                }),
            );
            self.escalate_to(subsystem, EscalationLevel::Incident, agent_id, audit);
        } else if !gate.frozen && metric_value >= gate.freeze_threshold {
            gate.frozen = true;
            status = GateStatus::Frozen;

            let _ = audit.append_event(
                agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "killgate.frozen",
                    "subsystem": subsystem,
                    "reason": format!("metric {} exceeded freeze threshold {}", metric_value, gate.freeze_threshold),
                    "by": "auto",
                }),
            );
            self.escalate_to(subsystem, EscalationLevel::Freeze, agent_id, audit);
        } else if metric_value > 0.0 && status == GateStatus::Open {
            let _ = self.escalate_once(subsystem, agent_id, audit);
        }

        let _ = audit.append_event(
            agent_id,
            EventType::StateChange,
            json!({
                "event_kind": "killgate.checked",
                "subsystem": subsystem,
                "metric": metric_value,
                "status": match status {
                    GateStatus::Open => "open",
                    GateStatus::Frozen => "frozen",
                    GateStatus::Halted => "halted",
                },
            }),
        );

        status
    }

    pub fn manual_freeze(
        &mut self,
        subsystem: &str,
        operator_id: &str,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) -> Result<GateStatus, KillGateError> {
        let gate = self
            .gates
            .get_mut(subsystem)
            .ok_or_else(|| KillGateError::UnknownSubsystem(subsystem.to_string()))?;

        gate.frozen = true;
        let _ = audit.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "killgate.frozen",
                "subsystem": subsystem,
                "reason": "manual freeze",
                "by": operator_id,
            }),
        );

        self.escalate_to(subsystem, EscalationLevel::Freeze, agent_id, audit);
        Ok(GateStatus::Frozen)
    }

    pub fn manual_halt(
        &mut self,
        subsystem: &str,
        operator_id: &str,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) -> Result<GateStatus, KillGateError> {
        let gate = self
            .gates
            .get_mut(subsystem)
            .ok_or_else(|| KillGateError::UnknownSubsystem(subsystem.to_string()))?;

        gate.halted = true;
        gate.frozen = true;

        let _ = audit.append_event(
            agent_id,
            EventType::Error,
            json!({
                "event_kind": "killgate.halted",
                "subsystem": subsystem,
                "reason": format!("manual halt by {}", operator_id),
            }),
        );

        self.escalate_to(subsystem, EscalationLevel::Incident, agent_id, audit);
        Ok(GateStatus::Halted)
    }

    pub fn manual_unfreeze(
        &mut self,
        subsystem: &str,
        operator_id: &str,
        hitl_tier: u8,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) -> Result<GateStatus, KillGateError> {
        if hitl_tier < 3 {
            return Err(KillGateError::Tier3Required);
        }

        let gate = self
            .gates
            .get_mut(subsystem)
            .ok_or_else(|| KillGateError::UnknownSubsystem(subsystem.to_string()))?;

        gate.frozen = false;
        gate.halted = false;
        self.escalation_levels.remove(subsystem);

        let _ = audit.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "killgate.unfrozen",
                "subsystem": subsystem,
                "operator_id": operator_id,
                "hitl_tier": hitl_tier,
            }),
        );
        Ok(GateStatus::Open)
    }

    pub fn escalate(
        &mut self,
        subsystem: &str,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) -> Result<EscalationLevel, KillGateError> {
        if !self.gates.contains_key(subsystem) {
            return Err(KillGateError::UnknownSubsystem(subsystem.to_string()));
        }

        let next = self.escalate_once(subsystem, agent_id, audit);
        Ok(next)
    }

    fn escalate_once(
        &mut self,
        subsystem: &str,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) -> EscalationLevel {
        let current = self
            .escalation_levels
            .get(subsystem)
            .copied()
            .unwrap_or(EscalationLevel::Warn);
        let next = current.next();
        self.escalation_levels.insert(subsystem.to_string(), next);

        let _ = audit.append_event(
            agent_id,
            EventType::UserAction,
            json!({
                "event_kind": "killgate.escalated",
                "subsystem": subsystem,
                "from_level": current.as_str(),
                "to_level": next.as_str(),
            }),
        );

        if next == EscalationLevel::Incident {
            let _ = audit.append_event(
                agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "killgate.notification_sent",
                    "subsystem": subsystem,
                    "via": "messaging_bridge",
                }),
            );
        }

        next
    }

    fn escalate_to(
        &mut self,
        subsystem: &str,
        target: EscalationLevel,
        agent_id: Uuid,
        audit: &mut AuditTrail,
    ) {
        while self
            .escalation_levels
            .get(subsystem)
            .copied()
            .unwrap_or(EscalationLevel::Warn)
            != target
        {
            let next = self.escalate_once(subsystem, agent_id, audit);
            if next == EscalationLevel::Incident || next == target {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EscalationLevel, GateStatus, KillGateRegistry};
    use crate::audit::AuditTrail;
    use crate::safety_supervisor::KpiKind;
    use uuid::Uuid;

    #[test]
    fn test_screen_poster_freeze() {
        let mut registry = KillGateRegistry::default();
        let mut audit = AuditTrail::new();
        let status = registry.check_gate("screen_poster", 3.0, Uuid::new_v4(), &mut audit);
        assert_eq!(status, GateStatus::Frozen);
    }

    #[test]
    fn test_mutation_freeze_on_mismatch() {
        let mut registry = KillGateRegistry::default();
        let mut audit = AuditTrail::new();

        let results =
            registry.check_metric(KpiKind::ReplayMismatch, 1.0, Uuid::new_v4(), &mut audit);
        let frozen = results
            .iter()
            .any(|(subsystem, status)| subsystem == "mutation" && *status == GateStatus::Frozen);
        assert!(frozen);
    }

    #[test]
    fn test_manual_freeze() {
        let mut registry = KillGateRegistry::default();
        let mut audit = AuditTrail::new();
        let status = registry.manual_freeze("cluster", "operator-1", Uuid::new_v4(), &mut audit);

        assert_eq!(status, Ok(GateStatus::Frozen));
        let logged = audit.events().iter().any(|event| {
            event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str())
                == Some("killgate.frozen")
        });
        assert!(logged);
    }

    #[test]
    fn test_unfreeze_requires_tier3() {
        let mut registry = KillGateRegistry::default();
        let mut audit = AuditTrail::new();

        let freeze = registry.manual_freeze("cluster", "operator-1", Uuid::new_v4(), &mut audit);
        assert_eq!(freeze, Ok(GateStatus::Frozen));

        let denied =
            registry.manual_unfreeze("cluster", "operator-1", 2, Uuid::new_v4(), &mut audit);
        assert!(denied.is_err());

        let allowed =
            registry.manual_unfreeze("cluster", "operator-1", 3, Uuid::new_v4(), &mut audit);
        assert_eq!(allowed, Ok(GateStatus::Open));
    }

    #[test]
    fn test_escalation_chain() {
        let mut registry = KillGateRegistry::default();
        let mut audit = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        let first = registry.escalate("screen_poster", agent_id, &mut audit);
        let second = registry.escalate("screen_poster", agent_id, &mut audit);
        let third = registry.escalate("screen_poster", agent_id, &mut audit);
        let fourth = registry.escalate("screen_poster", agent_id, &mut audit);

        assert_eq!(first, Ok(EscalationLevel::Degrade));
        assert_eq!(second, Ok(EscalationLevel::Freeze));
        assert_eq!(third, Ok(EscalationLevel::Halt));
        assert_eq!(fourth, Ok(EscalationLevel::Incident));
    }
}
