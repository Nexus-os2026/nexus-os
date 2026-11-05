//! Accounting is replayable; the private live entitlement is not. C1a has
//! no production meter constructor or model lane admission.
use super::*;
use nexus_governed_control::inference::{CallMeter, Charged, MeterRefusal};
use std::sync::Weak;

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
    invocation: InvocationId,
    generation: u64,
    in_flight: bool,
    at_mono_ms: u64,
    at_wall_ms: u64,
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

// Intentionally no production construction route until C1b. Keeping the
// private implementation compiled lets Clippy check the future adapter seam.
#[allow(dead_code)]
pub(super) struct ModelMeter<P: EffectPort> {
    pub(super) coordinator: Weak<Coordinator<P>>,
    pub(super) invocation: InvocationId,
    pub(super) generation: u64,
}
impl<P: EffectPort> CallMeter for ModelMeter<P> {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let c = self.coordinator.upgrade().ok_or(MeterRefusal::Stale)?;
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
            let Some(live) = m
                .lane_calls
                .as_ref()
                .filter(|l| l.invocation == self.invocation && l.generation == self.generation)
            else {
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
        let Some(c) = self.coordinator.upgrade() else {
            return;
        };
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
