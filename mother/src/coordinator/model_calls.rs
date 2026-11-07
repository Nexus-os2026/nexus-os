//! Replay accounting never restores the borrowed live caller or its deny latch.
use super::*;
use nexus_governed_control::inference::{CallMeter, Charged, MeterRefusal};
use std::sync::atomic::{AtomicBool, Ordering};

/// Memory-only deny latch for one invocation; never replayed or reset.
/// This observation seam grants no permission of its own.
#[derive(Clone)]
pub(crate) struct InvocationLiveness(Arc<AtomicBool>);
impl InvocationLiveness {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    pub(crate) fn is_revoked(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(super) struct CallAccounting {
    invocation: InvocationId,
    cap: u32,
    budget: u32,
    pub(super) next: u32,
    pub(super) prepaid: bool,
}
impl CallAccounting {
    pub(super) fn reserved(
        m: &Mission,
        invocation: InvocationId,
        cap: u32,
        budget: u32,
    ) -> Result<Option<Self>, ReplayViolation> {
        if cap == 0 && budget == 0 {
            return Ok(None);
        }
        if !(1..=4).contains(&cap)
            || !(1..=crate::contract::MAX_MODEL_CALLS).contains(&budget)
            || m.model_calls.checked_add(cap).is_none_or(|n| n > budget)
        {
            return Err(ReplayViolation::ModelChargeOutOfPlace);
        }
        Ok(Some(Self {
            invocation,
            cap,
            budget,
            next: 1,
            prepaid: true,
        }))
    }
    pub(super) fn charge(
        &mut self,
        invocation: InvocationId,
        call: u32,
        total: &mut u32,
    ) -> Result<(), ReplayViolation> {
        if invocation != self.invocation
            || call != self.next
            || call > self.cap
            || (call == 1 && !self.prepaid)
            || (call > 1 && *total >= self.budget)
        {
            return Err(ReplayViolation::ModelChargeOutOfPlace);
        }
        if call == 1 {
            self.prepaid = false;
        } else {
            *total += 1;
        }
        self.next += 1;
        Ok(())
    }
}

pub(super) struct LaneCalls {
    pub(super) liveness: InvocationLiveness,
    invocation: InvocationId,
    generation: u64,
    in_flight: bool,
    at_mono_ms: u64,
    at_wall_ms: u64,
}

// Dropping the live invocation denies every retained observer, including
// coordinator teardown. The atomic has no route back to false.
impl Drop for LaneCalls {
    fn drop(&mut self) {
        self.liveness.0.store(true, Ordering::Release);
    }
}

/// The sole live reservation step, also used by the in-process oracle caller.
pub(super) fn reserve_lane(
    inner: &mut Inner,
    id: MissionId,
    generation: u64,
    identity: &LaneIdentity,
    cap: u32,
    clock: &dyn Clock,
) -> Result<Then<LaneInput>, CoordinatorError> {
    inner.usable()?;
    let seq = inner.next_seq;
    let m = inner.mission(id)?;
    m.not_held()?;
    if m.generation != generation
        || !m.state.is_working()
        || !m.consented()
        || m.terminal_record.is_some()
        || m.settlement.is_some()
    {
        return Err(CoordinatorError::Stopped);
    }
    if m.pending_lane.is_some() {
        return Err(CoordinatorError::Busy);
    }
    if cap > 4 {
        return Err(CoordinatorError::Contract(
            ContractError::ModelCapOutOfRange,
        ));
    }
    let c = m.contract()?;
    let budget = c.budget.max_model_calls;
    if m.model_calls
        .checked_add(cap.max(1))
        .is_none_or(|n| n > budget)
    {
        return Ok(Then::End(BlockReason::BudgetModelCalls));
    }
    if m.out_of_time(clock) {
        return Ok(Then::End(BlockReason::BudgetTime));
    }
    let invocation = InvocationId { mission: id, seq };
    let declared = if cap == 0 { 0 } else { budget };
    let accounting = CallAccounting::reserved(m, invocation, cap, declared)
        .map_err(|_| CoordinatorError::Contract(ContractError::ModelCapOutOfRange))?;
    let live = (cap > 0).then(|| LaneCalls {
        liveness: InvocationLiveness::new(),
        invocation,
        generation,
        in_flight: false,
        at_mono_ms: m
            .mono_ref
            .saturating_add(c.budget.max_duration_ms.saturating_sub(m.spent_at_ref)),
        at_wall_ms: m
            .approved_wall_ms
            .unwrap_or(0)
            .saturating_add(c.budget.max_duration_ms),
    });
    let input = LaneInput {
        invocation,
        role: identity.role,
        contract_hash: m.contract_ref.contract_hash,
        owner_words: c.owner_words.clone(),
        restatement: c.restatement.clone(),
        pass_criteria: c.pass_criteria.clone(),
        prior_outputs: m.prior_outputs.clone(),
        builder_input: crate::page_inputs::prepare(
            c,
            m.contract_ref.contract_hash,
            invocation,
            identity.role,
        )
        .map_err(CoordinatorError::Contract)?,
        verification: None,
    };
    inner.append(
        id,
        Event::LaneInvoked {
            invocation,
            role: identity.role,
            identity_sha256: lane_identity_digest(identity),
            input_hash: input_hash(&input),
            model_cap: cap,
            model_budget: declared,
        },
    )?;
    let m = inner.mission(id)?;
    m.pending_lane = Some(invocation);
    m.lane_calls = live;
    m.call_accounting = accounting;
    if cap > 0 {
        m.declared_model_budgets.push(declared);
    }
    m.model_calls += 1;
    Ok(Then::Go(input))
}

pub(super) struct BorrowedMeter<'a, P: EffectPort> {
    coordinator: &'a Coordinator<P>,
    invocation: InvocationId,
    generation: u64,
}
impl<P: EffectPort> CallMeter for BorrowedMeter<'_, P> {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let c = self.coordinator;
        if c.port.emergency_stopped() {
            return Err(MeterRefusal::Stale);
        }
        let mut result = Err(MeterRefusal::Stale);
        c.try_with_lock(|inner| {
            if inner.usable().is_err()
                || inner
                    .file_cell
                    .as_ref()
                    .is_some_and(|cell| cell.is_engaged())
            {
                return Ok(());
            }
            let m = inner.mission(self.invocation.mission)?;
            if m.generation != self.generation
                || m.pending_lane != Some(self.invocation)
                || !m.state.is_working()
                || m.held.is_some()
                || !m.consented()
                || m.settlement.is_some()
                || m.terminal_record.is_some()
            {
                return Ok(());
            }
            let Some(live) = m.lane_calls.as_ref().filter(|l| {
                l.invocation == self.invocation
                    && l.generation == self.generation
                    && !l.liveness.is_revoked()
            }) else {
                return Ok(());
            };
            let Some(a) = m.call_accounting.as_ref() else {
                return Ok(());
            };
            result = if live.in_flight {
                Err(MeterRefusal::InFlight)
            } else if a.next > a.cap
                || (a.next == 1 && !a.prepaid)
                || (a.next > 1 && m.model_calls >= a.budget)
            {
                Err(MeterRefusal::Exhausted)
            } else if m.out_of_time(&*c.clock)
                || c.clock.monotonic_ms() >= live.at_mono_ms
                || c.clock.wall_ms() >= live.at_wall_ms
            {
                Err(MeterRefusal::OutOfTime)
            } else {
                Ok(Charged { call: a.next })
            };
            let Ok(charged) = result else {
                return Ok(());
            };
            result = Err(MeterRefusal::NotRecorded);
            inner.append(
                self.invocation.mission,
                Event::ModelCallCharged {
                    invocation: self.invocation,
                    call: charged.call,
                },
            )?;
            let m = inner.mission(self.invocation.mission)?;
            m.call_accounting
                .as_mut()
                .expect("checked accounting")
                .charge(self.invocation, charged.call, &mut m.model_calls)
                .map_err(|code| {
                    CoordinatorError::Journal(JournalError::Inconsistent {
                        line: self.invocation.seq as usize + 1,
                        code,
                    })
                })?;
            m.lane_calls
                .as_mut()
                .expect("checked live invocation")
                .in_flight = true;
            result = Ok(charged);
            Ok(())
        })
        .map_err(|_| {
            if result == Err(MeterRefusal::NotRecorded) {
                MeterRefusal::NotRecorded
            } else {
                MeterRefusal::Stale
            }
        })?;
        result
    }
    fn settle(&self, call: u32) {
        let c = self.coordinator;
        let _ =
            c.try_with_lock(|i| {
                if i.usable().is_err() {
                    return Ok(());
                }
                let m = i.mission(self.invocation.mission)?;
                if m.generation == self.generation
                    && m.pending_lane == Some(self.invocation)
                    && m.call_accounting
                        .as_ref()
                        .is_some_and(|a| a.next == call.saturating_add(1))
                {
                    if let Some(l) = m.lane_calls.as_mut().filter(|l| {
                        l.invocation == self.invocation && l.generation == self.generation
                    }) {
                        l.in_flight = false;
                    }
                }
                Ok(())
            });
    }
}

#[cfg(test)]
pub(super) struct ModelMeter<P: EffectPort> {
    pub(super) coordinator: std::sync::Weak<Coordinator<P>>,
    pub(super) invocation: InvocationId,
    pub(super) generation: u64,
}
#[cfg(test)]
impl<P: EffectPort> CallMeter for ModelMeter<P> {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let c = self.coordinator.upgrade().ok_or(MeterRefusal::Stale)?;
        BorrowedMeter {
            coordinator: &c,
            invocation: self.invocation,
            generation: self.generation,
        }
        .charge()
    }
    fn settle(&self, call: u32) {
        if let Some(c) = self.coordinator.upgrade() {
            BorrowedMeter {
                coordinator: &c,
                invocation: self.invocation,
                generation: self.generation,
            }
            .settle(call);
        }
    }
}

use crate::effect::ModelAttempt;
use crate::lane::{ModelCallError, ModelCaller};
use nexus_governed_control::inference::{CallDeadline, ModelReply, ModelRequest};

/// Absolute values copied from the reservation, never reconstructed per call.
struct MissionDeadline {
    clock: Arc<dyn Clock>,
    mono: u64,
    wall: u64,
}
impl CallDeadline for MissionDeadline {
    fn expired(&self) -> bool {
        self.clock.monotonic_ms() >= self.mono || self.clock.wall_ms() >= self.wall
    }
}
pub(super) struct Caller<'a, P: EffectPort> {
    meter: BorrowedMeter<'a, P>,
    liveness: InvocationLiveness,
    deadline: MissionDeadline,
    hash: String,
    request: String,
    identity: [u8; 32],
}
impl<'a, P: EffectPort> Caller<'a, P> {
    pub(super) fn reserved(
        c: &'a Coordinator<P>,
        inner: &mut Inner,
        invocation: InvocationId,
        generation: u64,
    ) -> Result<Self, CoordinatorError> {
        let m = inner.mission(invocation.mission)?;
        let live = m.lane_calls.as_ref().ok_or(CoordinatorError::Stopped)?;
        let identity = hex::decode(
            &m.contract()?
                .model
                .as_ref()
                .ok_or(CoordinatorError::NoConsent)?
                .identity,
        )
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or(CoordinatorError::NoConsent)?;
        let caller = Self {
            meter: BorrowedMeter {
                coordinator: c,
                invocation,
                generation,
            },
            liveness: live.liveness.clone(),
            deadline: MissionDeadline {
                clock: c.clock.clone(),
                mono: live.at_mono_ms,
                wall: live.at_wall_ms,
            },
            hash: m.contract_ref.contract_hash.to_hex(),
            request: m
                .consent_request
                .clone()
                .ok_or(CoordinatorError::NoConsent)?,
            identity,
        };
        c.file
            .as_ref()
            .ok_or(CoordinatorError::Stopped)?
            .model_active
            .store(true, Ordering::Release);
        Ok(caller)
    }
    pub(super) fn admit(&self) -> Result<(), crate::effect::EffectRefusal> {
        self.meter.coordinator.port.admit_inference(
            self.meter.invocation.mission,
            &self.hash,
            &self.request,
            self.identity,
        )
    }
    fn live(&self) -> bool {
        !self.liveness.is_revoked()
            && self
                .meter
                .coordinator
                .file
                .as_ref()
                .is_some_and(|f| !f.cell.is_engaged())
            && !self.meter.coordinator.port.emergency_stopped()
    }
    fn deny(&self, reason: BlockReason) {
        let c = self.meter.coordinator;
        let _ = c.file_latch_terminal(super::lite_intent::FileTerminal::Failure {
            id: self.meter.invocation.mission,
            trigger: Trigger::Block { reason },
        });
    }
}
impl<P: EffectPort> Drop for Caller<'_, P> {
    fn drop(&mut self) {
        // A panic/lost result is an outstanding obligation, never a joined call.
        if !std::thread::panicking() {
            if let Some(f) = &self.meter.coordinator.file {
                f.model_active.store(false, Ordering::Release);
            }
        }
    }
}
impl<P: EffectPort> ModelCaller for Caller<'_, P> {
    fn call(&self, request: &ModelRequest) -> Result<ModelReply, ModelCallError> {
        if !self.live() {
            return Err(ModelCallError::Invalidated);
        }
        let c = self.meter.coordinator;
        let attempt = c
            .port
            .model_call(self.meter.invocation.mission, request, self, self);
        // Recording is independent of liveness and the first terminal cause.
        if matches!(attempt, Ok(ModelAttempt::Unrecorded(_))) {
            c.file
                .as_ref()
                .expect("File caller")
                .model_debt
                .store(true, Ordering::Release);
            self.deny(BlockReason::BrokerRefused(BrokerClass::EvidenceUnavailable));
            return Err(ModelCallError::RecordUnavailable);
        }
        if self.live() && self.deadline.expired() {
            self.deny(BlockReason::BudgetTime);
        }
        #[cfg(test)]
        if matches!(attempt, Ok(ModelAttempt::Replied(_))) {
            c.hooks.model_replied.fire();
        }
        match attempt {
            Ok(ModelAttempt::Failed(class)) => Err(ModelCallError::Failed(class)),
            Err(refusal) => Err(ModelCallError::Refused(refusal.class)),
            Ok(ModelAttempt::Replied(reply)) if self.live() => Ok(reply),
            _ => Err(ModelCallError::Invalidated),
        }
    }
}

impl<P: EffectPort> Coordinator<P> {
    pub(super) fn model_obligations(&self) -> Result<(), CoordinatorError> {
        if let Some(f) = &self.file {
            if f.model_debt.load(Ordering::Acquire) {
                return Err(CoordinatorError::Recording);
            }
            if f.model_active.load(Ordering::Acquire) {
                return Err(CoordinatorError::Busy);
            }
        }
        Ok(())
    }
}

// Publish denial at the callback, before the broker's attempt recording.
impl<P: EffectPort> CallMeter for Caller<'_, P> {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let result = self.meter.charge();
        match result {
            Err(MeterRefusal::Exhausted | MeterRefusal::InFlight) => {
                self.deny(BlockReason::BudgetModelCalls)
            }
            Err(MeterRefusal::OutOfTime) => self.deny(BlockReason::BudgetTime),
            _ => {}
        }
        result
    }
    fn settle(&self, call: u32) {
        self.meter.settle(call);
    }
}
impl<P: EffectPort> CallDeadline for Caller<'_, P> {
    fn expired(&self) -> bool {
        let expired = self.deadline.expired();
        if expired && self.live() {
            self.deny(BlockReason::BudgetTime);
        }
        expired
    }
}
