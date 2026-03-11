use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const ALERT_THRESHOLDS: [u8; 3] = [70, 100, 120];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BudgetPeriodId(pub String);

impl BudgetPeriodId {
    pub fn new(value: impl Into<String>) -> Self {
        let raw = value.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Self("period.default".to_string());
        }
        Self(trimmed.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyBudget {
    pub cap_units: u64,
    pub spent_units: u64,
    pub period: BudgetPeriodId,
    pub alerts_emitted: Vec<u8>,
}

impl MonthlyBudget {
    pub fn new(cap_units: u64, period: BudgetPeriodId) -> Self {
        Self {
            cap_units: cap_units.max(1),
            spent_units: 0,
            period,
            alerts_emitted: Vec::new(),
        }
    }

    pub fn set_period(&mut self, period: BudgetPeriodId) -> bool {
        if self.period == period {
            return false;
        }

        self.period = period;
        self.spent_units = 0;
        self.alerts_emitted.clear();
        true
    }

    pub fn record_spend(&mut self, units: u64) -> Vec<u8> {
        self.spent_units = self.spent_units.saturating_add(units);

        let mut crossed = Vec::new();
        for threshold in ALERT_THRESHOLDS {
            if self.alerts_emitted.contains(&threshold) {
                continue;
            }

            let threshold_units = percent_ceiling(self.cap_units, u64::from(threshold));
            if self.spent_units >= threshold_units {
                self.alerts_emitted.push(threshold);
                crossed.push(threshold);
            }
        }

        crossed
    }

    pub fn exceeds_cap(&self) -> bool {
        self.spent_units > self.cap_units
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCost {
    pub cost_per_1k_input: u64,
    pub cost_per_1k_output: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FuelToTokenModel {
    pub models: HashMap<String, ModelCost>,
}

impl FuelToTokenModel {
    pub fn with_defaults() -> Self {
        let mut model = Self::default();
        model.models.insert(
            "mock-1".to_string(),
            ModelCost {
                cost_per_1k_input: 0,
                cost_per_1k_output: 0,
            },
        );
        model.models.insert(
            "deepseek-chat".to_string(),
            ModelCost {
                cost_per_1k_input: 140,
                cost_per_1k_output: 280,
            },
        );
        model.models.insert(
            "claude-sonnet-4-5".to_string(),
            ModelCost {
                cost_per_1k_input: 3_000,
                cost_per_1k_output: 15_000,
            },
        );
        model.models.insert(
            "ollama/llama3".to_string(),
            ModelCost {
                cost_per_1k_input: 0,
                cost_per_1k_output: 0,
            },
        );
        model
    }

    pub fn insert(&mut self, model: impl Into<String>, cost: ModelCost) {
        self.models.insert(model.into(), cost);
    }

    pub fn simulate_cost(&self, model: &str, input_tokens: u32, output_tokens: u32) -> u64 {
        match self.models.get(model) {
            Some(cost) => simulate_cost_units(cost, input_tokens, output_tokens),
            None => 0,
        }
    }

    pub fn simulate_cost_with_fallback(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        fallback: ModelCost,
    ) -> u64 {
        let cost = self.models.get(model).unwrap_or(&fallback);
        simulate_cost_units(cost, input_tokens, output_tokens)
    }
}

fn simulate_cost_units(cost: &ModelCost, input_tokens: u32, output_tokens: u32) -> u64 {
    let input = round_up_per_1000(u64::from(input_tokens), cost.cost_per_1k_input);
    let output = round_up_per_1000(u64::from(output_tokens), cost.cost_per_1k_output);
    input.saturating_add(output)
}

fn round_up_per_1000(tokens: u64, cost_per_1k: u64) -> u64 {
    if tokens == 0 || cost_per_1k == 0 {
        return 0;
    }

    tokens
        .saturating_mul(cost_per_1k)
        .saturating_add(999)
        .saturating_div(1_000)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyKind {
    Spike,
    Slope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub kind: AnomalyKind,
    pub cost_units: u64,
    pub baseline_cost_per_call: u64,
    pub spike_factor_x100: u64,
    pub window_total_units: u64,
    pub window_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BurnAnomalyDetector {
    pub baseline_cost_per_call: u64,
    pub spike_factor_x100: u64,
    pub slope_threshold: u64,
    pub window_calls: usize,
    history: VecDeque<u64>,
}

impl BurnAnomalyDetector {
    pub fn new(
        baseline_cost_per_call: u64,
        spike_factor_x100: u64,
        slope_threshold: u64,
        window_calls: usize,
    ) -> Self {
        Self {
            baseline_cost_per_call,
            spike_factor_x100,
            slope_threshold,
            window_calls,
            history: VecDeque::new(),
        }
    }

    pub fn observe(&mut self, cost_units: u64) -> Option<AnomalyEvent> {
        self.history.push_back(cost_units);
        while self.history.len() > self.window_calls {
            let _ = self.history.pop_front();
        }

        let window_total = self
            .history
            .iter()
            .fold(0_u64, |sum, value| sum.saturating_add(*value));

        let spike = self.baseline_cost_per_call > 0
            && self.spike_factor_x100 > 0
            && cost_units.saturating_mul(100)
                >= self
                    .baseline_cost_per_call
                    .saturating_mul(self.spike_factor_x100);
        if spike {
            return Some(AnomalyEvent {
                kind: AnomalyKind::Spike,
                cost_units,
                baseline_cost_per_call: self.baseline_cost_per_call,
                spike_factor_x100: self.spike_factor_x100,
                window_total_units: window_total,
                window_calls: self.history.len(),
            });
        }

        let slope = self.window_calls > 0
            && self.history.len() == self.window_calls
            && window_total >= self.slope_threshold;
        if slope {
            return Some(AnomalyEvent {
                kind: AnomalyKind::Slope,
                cost_units,
                baseline_cost_per_call: self.baseline_cost_per_call,
                spike_factor_x100: self.spike_factor_x100,
                window_total_units: window_total,
                window_calls: self.history.len(),
            });
        }

        None
    }
}

impl BurnAnomalyDetector {
    /// Update the baseline cost using a rolling average of the last 20 observations.
    ///
    /// Call this after each successful LLM spend to let the detector adapt to
    /// the agent's actual cost profile.  The first 20 calls build the initial
    /// average; subsequent calls maintain an exponentially-weighted rolling mean.
    pub fn update_baseline(&mut self, actual_cost: u64) {
        const ROLLING_WINDOW: u64 = 20;

        if self.baseline_cost_per_call == 0 {
            // Cold start: use the first observation directly.
            self.baseline_cost_per_call = actual_cost;
            return;
        }

        // Rolling average: new = old + (actual - old) / min(history_len, WINDOW)
        let n = (self.history.len() as u64).clamp(1, ROLLING_WINDOW);
        if actual_cost >= self.baseline_cost_per_call {
            self.baseline_cost_per_call += (actual_cost - self.baseline_cost_per_call) / n;
        } else {
            self.baseline_cost_per_call -= (self.baseline_cost_per_call - actual_cost) / n;
        }
    }
}

impl Default for BurnAnomalyDetector {
    fn default() -> Self {
        // baseline=100, spike_factor=500 (5×), slope_threshold=1200 (baseline × window × 3), window=4
        Self::new(100, 500, 1200, 4)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuelViolation {
    OverMonthlyCap,
    AnomalyDetected,
    ProviderTokenMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StakeAccount {
    pub staked_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlashReason {
    FuelAbuse,
    PolicyViolation,
    ReplayMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashEvent {
    pub amount_units: u64,
    pub reason: SlashReason,
    pub evidence_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSpend {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub spent_units: u64,
    pub calls: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuelAuditReport {
    pub agent_id: Uuid,
    pub period: BudgetPeriodId,
    pub cap_units: u64,
    pub spent_units: u64,
    pub anomalies: Vec<AnomalyEvent>,
    pub halts: u32,
    pub model_breakdown: Vec<ModelSpend>,
}

#[derive(Debug, Clone)]
pub struct AgentFuelLedger {
    budget: MonthlyBudget,
    detector: BurnAnomalyDetector,
    anomalies: Vec<AnomalyEvent>,
    halts: u32,
    model_breakdown: HashMap<String, ModelSpend>,
}

impl AgentFuelLedger {
    pub fn new(period: BudgetPeriodId, cap_units: u64, detector: BurnAnomalyDetector) -> Self {
        Self {
            budget: MonthlyBudget::new(cap_units, period),
            detector,
            anomalies: Vec::new(),
            halts: 0,
            model_breakdown: HashMap::new(),
        }
    }

    pub fn period(&self) -> &BudgetPeriodId {
        &self.budget.period
    }

    pub fn cap_units(&self) -> u64 {
        self.budget.cap_units
    }

    pub fn spent_units(&self) -> u64 {
        self.budget.spent_units
    }

    pub fn set_period(
        &mut self,
        agent_id: Uuid,
        period: BudgetPeriodId,
        audit: &mut AuditTrail,
    ) -> bool {
        let previous = self.budget.period.clone();
        let changed = self.budget.set_period(period.clone());
        if changed {
            self.anomalies.clear();
            self.halts = 0;
            self.model_breakdown.clear();
            audit
                .append_event(
                    agent_id,
                    EventType::StateChange,
                    json!({
                        "event_kind": "fuel.period_set",
                        "agent_id": agent_id,
                        "previous_period": previous.0,
                        "period": period.0,
                        "cap_units": self.budget.cap_units,
                        "spent_units": self.budget.spent_units,
                    }),
                )
                .expect("audit: fail-closed");
        }

        changed
    }

    pub fn set_cap_units(&mut self, agent_id: Uuid, cap_units: u64, audit: &mut AuditTrail) {
        self.budget.cap_units = cap_units.max(1);
        audit
            .append_event(
                agent_id,
                EventType::UserAction,
                json!({
                    "event_kind": "fuel.period_set",
                    "agent_id": agent_id,
                    "period": self.budget.period.0,
                    "cap_units": self.budget.cap_units,
                    "spent_units": self.budget.spent_units,
                }),
            )
            .expect("audit: fail-closed");
    }

    pub fn record_llm_spend(
        &mut self,
        agent_id: Uuid,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        cost_units: u64,
        audit: &mut AuditTrail,
    ) -> Result<(), FuelViolation> {
        let crossed_thresholds = self.budget.record_spend(cost_units);

        let entry = self
            .model_breakdown
            .entry(model.to_string())
            .or_insert_with(|| ModelSpend {
                model: model.to_string(),
                input_tokens: 0,
                output_tokens: 0,
                spent_units: 0,
                calls: 0,
            });
        entry.input_tokens = entry.input_tokens.saturating_add(u64::from(input_tokens));
        entry.output_tokens = entry.output_tokens.saturating_add(u64::from(output_tokens));
        entry.spent_units = entry.spent_units.saturating_add(cost_units);
        entry.calls = entry.calls.saturating_add(1);

        audit
            .append_event(
                agent_id,
                EventType::LlmCall,
                json!({
                    "event_kind": "fuel.spend_recorded",
                    "agent_id": agent_id,
                    "period": self.budget.period.0,
                    "model": model,
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                    "cost_units": cost_units,
                    "spent_units": self.budget.spent_units,
                    "cap_units": self.budget.cap_units,
                }),
            )
            .expect("audit: fail-closed");

        for threshold in crossed_thresholds {
            audit
                .append_event(
                    agent_id,
                    EventType::UserAction,
                    json!({
                        "event_kind": "fuel.alert_threshold_crossed",
                        "agent_id": agent_id,
                        "period": self.budget.period.0,
                        "threshold_percent": threshold,
                        "spent_units": self.budget.spent_units,
                        "cap_units": self.budget.cap_units,
                    }),
                )
                .expect("audit: fail-closed");
        }

        if let Some(anomaly) = self.detector.observe(cost_units) {
            self.anomalies.push(anomaly.clone());
            self.halts = self.halts.saturating_add(1);
            let evidence_hash = hash_evidence(
                agent_id,
                self.budget.period.0.as_str(),
                model,
                cost_units,
                self.budget.spent_units,
            );
            audit
                .append_event(
                    agent_id,
                    EventType::Error,
                    json!({
                        "event_kind": "fuel.anomaly_detected",
                        "agent_id": agent_id,
                        "period": self.budget.period.0,
                        "model": model,
                        "cost_units": cost_units,
                        "spent_units": self.budget.spent_units,
                        "cap_units": self.budget.cap_units,
                        "anomaly": anomaly,
                        "evidence_hash": evidence_hash,
                    }),
                )
                .expect("audit: fail-closed");
            return Err(FuelViolation::AnomalyDetected);
        }

        if self.budget.exceeds_cap() {
            self.halts = self.halts.saturating_add(1);
            let evidence_hash = hash_evidence(
                agent_id,
                self.budget.period.0.as_str(),
                model,
                cost_units,
                self.budget.spent_units,
            );
            audit
                .append_event(
                    agent_id,
                    EventType::Error,
                    json!({
                        "event_kind": "fuel.exhausted_halt",
                        "agent_id": agent_id,
                        "period": self.budget.period.0,
                        "model": model,
                        "cost_units": cost_units,
                        "spent_units": self.budget.spent_units,
                        "cap_units": self.budget.cap_units,
                        "violation": "OverMonthlyCap",
                        "evidence_hash": evidence_hash,
                    }),
                )
                .expect("audit: fail-closed");
            return Err(FuelViolation::OverMonthlyCap);
        }

        Ok(())
    }

    pub fn register_violation(
        &mut self,
        agent_id: Uuid,
        violation: FuelViolation,
        reason: &str,
        audit: &mut AuditTrail,
    ) {
        self.halts = self.halts.saturating_add(1);
        let event_kind = match violation {
            FuelViolation::AnomalyDetected => "fuel.anomaly_detected",
            FuelViolation::OverMonthlyCap | FuelViolation::ProviderTokenMismatch => {
                "fuel.exhausted_halt"
            }
        };
        let evidence_hash = hash_evidence(
            agent_id,
            self.budget.period.0.as_str(),
            reason,
            self.budget.spent_units,
            self.budget.cap_units,
        );

        audit
            .append_event(
                agent_id,
                EventType::Error,
                json!({
                    "event_kind": event_kind,
                    "agent_id": agent_id,
                    "period": self.budget.period.0,
                    "reason": reason,
                    "spent_units": self.budget.spent_units,
                    "cap_units": self.budget.cap_units,
                    "violation": violation,
                    "evidence_hash": evidence_hash,
                }),
            )
            .expect("audit: fail-closed");
    }

    pub fn snapshot(&self, agent_id: Uuid) -> FuelAuditReport {
        let mut breakdown = self.model_breakdown.values().cloned().collect::<Vec<_>>();
        breakdown.sort_by(|left, right| left.model.cmp(&right.model));

        FuelAuditReport {
            agent_id,
            period: self.budget.period.clone(),
            cap_units: self.budget.cap_units,
            spent_units: self.budget.spent_units,
            anomalies: self.anomalies.clone(),
            halts: self.halts,
            model_breakdown: breakdown,
        }
    }
}

fn percent_ceiling(value: u64, percent: u64) -> u64 {
    value
        .saturating_mul(percent)
        .saturating_add(99)
        .saturating_div(100)
}

fn hash_evidence(
    agent_id: Uuid,
    period: &str,
    model_or_reason: &str,
    primary_units: u64,
    secondary_units: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent_id.to_string().as_bytes());
    hasher.update(period.as_bytes());
    hasher.update(model_or_reason.as_bytes());
    hasher.update(primary_units.to_le_bytes());
    hasher.update(secondary_units.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

/// Thread-safe fuel context that tracks remaining fuel for an agent execution.
///
/// Supports both direct deduction (`deduct_fuel`) and two-phase reservation
/// (`reserve_fuel` → commit/cancel).  Reservations that are dropped without
/// being committed automatically return their fuel.
#[derive(Debug, Clone)]
pub struct FuelContext {
    inner: Arc<Mutex<FuelContextInner>>,
}

#[derive(Debug)]
struct FuelContextInner {
    fuel_remaining: u64,
}

impl FuelContext {
    /// Create a new fuel context with the given initial budget.
    pub fn new(initial_fuel: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(FuelContextInner {
                fuel_remaining: initial_fuel,
            })),
        }
    }

    /// Returns the current remaining fuel (excluding reserved-but-uncommitted fuel).
    pub fn fuel_remaining(&self) -> u64 {
        self.inner
            .lock()
            .expect("fuel lock poisoned")
            .fuel_remaining
    }

    /// Immediately deduct `amount` fuel.  Returns `Err(FuelExhausted)` if
    /// insufficient fuel is available.
    pub fn deduct_fuel(&self, amount: u64) -> Result<(), AgentError> {
        let mut inner = self.inner.lock().expect("fuel lock poisoned");
        if inner.fuel_remaining < amount {
            return Err(AgentError::FuelExhausted);
        }
        inner.fuel_remaining -= amount;
        Ok(())
    }

    /// Reserve `amount` fuel for later commit or cancel.
    ///
    /// The reserved fuel is immediately subtracted from the available balance.
    /// Call [`FuelReservation::commit`] to finalise or [`FuelReservation::cancel`]
    /// to return it.  Dropping the reservation without committing automatically
    /// cancels it (fuel is returned).
    pub fn reserve_fuel(&self, amount: u64) -> Result<FuelReservation, AgentError> {
        let mut inner = self.inner.lock().expect("fuel lock poisoned");
        if inner.fuel_remaining < amount {
            return Err(AgentError::FuelExhausted);
        }
        inner.fuel_remaining -= amount;
        Ok(FuelReservation {
            context: self.inner.clone(),
            amount,
            committed: false,
        })
    }
}

/// A pending fuel reservation.  Commit to finalise the deduction, cancel to
/// return the fuel, or simply drop to auto-cancel.
#[derive(Debug)]
pub struct FuelReservation {
    context: Arc<Mutex<FuelContextInner>>,
    amount: u64,
    committed: bool,
}

impl FuelReservation {
    /// Finalise the reservation — the fuel is permanently consumed.
    pub fn commit(mut self) {
        self.committed = true;
    }

    /// Cancel the reservation — the fuel is returned to the context.
    pub fn cancel(mut self) {
        if !self.committed {
            let mut inner = self.context.lock().expect("fuel lock poisoned");
            inner.fuel_remaining += self.amount;
            self.committed = true; // prevent double-return in Drop
        }
    }
}

impl Drop for FuelReservation {
    fn drop(&mut self) {
        if !self.committed {
            let mut inner = self.context.lock().expect("fuel lock poisoned");
            inner.fuel_remaining += self.amount;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelContext, FuelViolation};
    use crate::audit::AuditTrail;
    use uuid::Uuid;

    #[test]
    fn monthly_cap_violation_emits_halt_event() {
        let agent_id = Uuid::new_v4();
        let mut audit = AuditTrail::new();
        // Use a detector with anomaly detection disabled so we test the cap path.
        let mut ledger = AgentFuelLedger::new(
            BudgetPeriodId::new("2026-03"),
            1_000,
            BurnAnomalyDetector::new(0, 0, u64::MAX, 4),
        );

        let result = ledger.record_llm_spend(agent_id, "mock-1", 100, 100, 1_001, &mut audit);
        assert_eq!(result, Err(FuelViolation::OverMonthlyCap));

        let found = audit.events().iter().any(|event| {
            event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str())
                == Some("fuel.exhausted_halt")
        });
        assert!(found);
    }

    #[test]
    fn alert_thresholds_emit_once() {
        let agent_id = Uuid::new_v4();
        let mut audit = AuditTrail::new();
        // Use a detector with anomaly detection disabled so we test the threshold path.
        let mut ledger = AgentFuelLedger::new(
            BudgetPeriodId::new("2026-03"),
            1_000,
            BurnAnomalyDetector::new(0, 0, u64::MAX, 4),
        );

        let first = ledger.record_llm_spend(agent_id, "mock-1", 10, 10, 700, &mut audit);
        assert!(first.is_ok());

        let second = ledger.record_llm_spend(agent_id, "mock-1", 10, 10, 300, &mut audit);
        assert!(second.is_ok());

        let threshold_70_count = audit
            .events()
            .iter()
            .filter(|event| {
                event
                    .payload
                    .get("event_kind")
                    .and_then(|value| value.as_str())
                    == Some("fuel.alert_threshold_crossed")
                    && event
                        .payload
                        .get("threshold_percent")
                        .and_then(|value| value.as_u64())
                        == Some(70)
            })
            .count();

        let threshold_100_count = audit
            .events()
            .iter()
            .filter(|event| {
                event
                    .payload
                    .get("event_kind")
                    .and_then(|value| value.as_str())
                    == Some("fuel.alert_threshold_crossed")
                    && event
                        .payload
                        .get("threshold_percent")
                        .and_then(|value| value.as_u64())
                        == Some(100)
            })
            .count();

        assert_eq!(threshold_70_count, 1);
        assert_eq!(threshold_100_count, 1);
    }

    #[test]
    fn anomaly_detector_spike_triggers_event() {
        let agent_id = Uuid::new_v4();
        let mut audit = AuditTrail::new();
        let mut ledger = AgentFuelLedger::new(
            BudgetPeriodId::new("2026-03"),
            10_000,
            BurnAnomalyDetector::new(10, 300, u64::MAX, 4),
        );

        let violation = ledger.record_llm_spend(agent_id, "mock-1", 10, 10, 35, &mut audit);
        assert_eq!(violation, Err(FuelViolation::AnomalyDetected));

        let anomaly_logged = audit.events().iter().any(|event| {
            event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str())
                == Some("fuel.anomaly_detected")
        });
        assert!(anomaly_logged);
    }

    // ── Fuel reservation tests ──

    #[test]
    fn test_fuel_reservation_commit() {
        let ctx = FuelContext::new(500);
        let reservation = ctx.reserve_fuel(100).expect("reserve should succeed");
        assert_eq!(ctx.fuel_remaining(), 400);
        reservation.commit();
        assert_eq!(ctx.fuel_remaining(), 400);
    }

    #[test]
    fn test_fuel_reservation_cancel() {
        let ctx = FuelContext::new(500);
        let reservation = ctx.reserve_fuel(100).expect("reserve should succeed");
        assert_eq!(ctx.fuel_remaining(), 400);
        reservation.cancel();
        assert_eq!(ctx.fuel_remaining(), 500);
    }

    #[test]
    fn test_fuel_reservation_drop_returns_fuel() {
        let ctx = FuelContext::new(500);
        {
            let _reservation = ctx.reserve_fuel(100).expect("reserve should succeed");
            assert_eq!(ctx.fuel_remaining(), 400);
        } // _reservation dropped here without commit or cancel
        assert_eq!(ctx.fuel_remaining(), 500);
    }

    #[test]
    fn test_fuel_reservation_insufficient() {
        let ctx = FuelContext::new(500);
        let result = ctx.reserve_fuel(600);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            crate::errors::AgentError::FuelExhausted
        );
        assert_eq!(ctx.fuel_remaining(), 500);
    }

    #[test]
    fn test_concurrent_reservation_no_overdraw() {
        use std::sync::Arc;

        let ctx = Arc::new(FuelContext::new(500));
        let ctx1 = ctx.clone();
        let ctx2 = ctx.clone();

        let handle1 = std::thread::spawn(move || ctx1.reserve_fuel(400));
        let handle2 = std::thread::spawn(move || ctx2.reserve_fuel(400));

        let r1 = handle1.join().expect("thread 1 panicked");
        let r2 = handle2.join().expect("thread 2 panicked");

        // Exactly one should succeed, one should fail.
        let successes = [r1.is_ok(), r2.is_ok()].iter().filter(|&&ok| ok).count();
        assert_eq!(successes, 1, "exactly one reservation should succeed");

        // Commit the successful reservation.
        if let Ok(reservation) = r1 {
            reservation.commit();
        }
        if let Ok(reservation) = r2 {
            reservation.commit();
        }

        // Total fuel spent should never exceed initial balance.
        assert!(ctx.fuel_remaining() <= 500);
        assert_eq!(ctx.fuel_remaining(), 100);
    }

    #[test]
    fn default_detector_has_real_thresholds() {
        let detector = BurnAnomalyDetector::default();
        assert_eq!(detector.baseline_cost_per_call, 100);
        assert_eq!(detector.spike_factor_x100, 500);
        assert_eq!(detector.slope_threshold, 1200);
        assert_eq!(detector.window_calls, 4);
    }

    #[test]
    fn default_detector_catches_spike() {
        let mut detector = BurnAnomalyDetector::default();
        // Spike threshold: cost * 100 >= 100 * 500 → cost >= 500
        let normal = detector.observe(400);
        assert!(normal.is_none());

        let spike = detector.observe(500);
        assert!(spike.is_some());
        assert_eq!(spike.unwrap().kind, super::AnomalyKind::Spike);
    }

    #[test]
    fn default_detector_catches_slope() {
        let mut detector = BurnAnomalyDetector::default();
        // Slope threshold: 1200.  Window = 4 calls.
        // 4 calls of 300 each = 1200 total → triggers slope.
        assert!(detector.observe(300).is_none()); // window not full
        assert!(detector.observe(300).is_none()); // window not full
        assert!(detector.observe(300).is_none()); // window not full
        let slope = detector.observe(300); // window full, total=1200
        assert!(slope.is_some());
        assert_eq!(slope.unwrap().kind, super::AnomalyKind::Slope);
    }

    #[test]
    fn update_baseline_cold_start() {
        let mut detector = BurnAnomalyDetector::new(0, 500, 1200, 4);
        assert_eq!(detector.baseline_cost_per_call, 0);
        detector.update_baseline(200);
        assert_eq!(detector.baseline_cost_per_call, 200);
    }

    #[test]
    fn update_baseline_rolling_average() {
        let mut detector = BurnAnomalyDetector::default();
        // Baseline starts at 100.  Feed 20 observations of 200.
        for _ in 0..20 {
            detector.observe(200); // fill history for rolling window
            detector.update_baseline(200);
        }
        // After 20 observations of 200, baseline should converge toward 200.
        // Due to integer division it won't be exact, but should be close.
        assert!(
            detector.baseline_cost_per_call >= 190 && detector.baseline_cost_per_call <= 210,
            "baseline {} should be near 200",
            detector.baseline_cost_per_call
        );
    }

    #[test]
    fn test_deduct_fuel_backward_compatible() {
        let ctx = FuelContext::new(500);
        ctx.deduct_fuel(100).expect("deduct should succeed");
        assert_eq!(ctx.fuel_remaining(), 400);
    }
}
