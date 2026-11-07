//! File-only recognition, settlement and replay. Every uncertain outcome is
//! terminal and keeps its broker pin. A terminal outcome is ONE journal line.
use super::*;
use crate::effect::FileSnapshot;
use nexus_governed_control::authority::commitment::{CommitmentState, PublicationClass};
use nexus_governed_control::authority::file_session::{HaltCause, HaltTag};

pub(super) enum FileDecision {
    Status(ApprovalStatus),
    Ended(MissionState),
}

fn halt_cause(cause: SettleCause) -> HaltCause {
    match cause {
        SettleCause::Stop(StopReason::Owner) => HaltCause::Stop,
        SettleCause::Stop(StopReason::EmergencyStop) => HaltCause::Emergency,
        SettleCause::TimeBudget
        | SettleCause::Ended(BlockReason::BudgetTime)
        | SettleCause::Stop(StopReason::Ended(BlockReason::BudgetTime)) => HaltCause::TimeBudget,
        _ => HaltCause::Fence,
    }
}

/// Preserve the end reason through a transition legal both live and on replay.
/// Held and unsettled missions accept Stop, not an ordinary Block/lapse. An
/// unresolved File takes the separate settlement path before this is called.
fn plain_terminal(mission: &Mission, cause: SettleCause) -> Trigger {
    let reason = match cause {
        SettleCause::Stop(reason) => return Trigger::Stop { reason },
        SettleCause::TimeBudget => BlockReason::BudgetTime,
        SettleCause::Ended(reason) => reason,
    };
    if mission.held.is_some()
        || mission.unsettled.is_some()
        || matches!(
            mission.state,
            MissionState::Draft | MissionState::AwaitingConsent
        )
    {
        return Trigger::Stop {
            reason: StopReason::Ended(reason),
        };
    }
    if let (MissionState::AwaitingApproval, Some(waiting)) = (mission.state, &mission.waiting) {
        return Trigger::ApprovalLapsed {
            effect: waiting.effect,
            reason: LapseReason::Ended(reason),
        };
    }
    Trigger::Block { reason }
}
// The closed cause registry is shared by all callers in this process. Its
// one-based index is kept ONLY in the deny-only cell, never in a journal or
// across a restart. Journal records still use the existing stable codes.
fn cause_tag(cause: SettleCause) -> Option<std::num::NonZeroU16> {
    let index = SettleCause::every()
        .iter()
        .position(|known| *known == cause)?;
    std::num::NonZeroU16::new(u16::try_from(index.checked_add(1)?).ok()?)
}
fn first_cause(
    (first, tag): (HaltCause, Option<std::num::NonZeroU16>),
) -> Result<SettleCause, CoordinatorError> {
    let tag = tag.ok_or(CoordinatorError::Stopped)?.get();
    // Broker tags have fixed, exact meanings. Unknown or inconsistent tags
    // fail closed; no engaged cell can be guessed to mean DoorHalted.
    match (first, tag) {
        (HaltCause::Stop, u16::MAX) => Ok(SettleCause::Stop(StopReason::Owner)),
        (HaltCause::Emergency, tag) if tag == u16::MAX - 1 => {
            Ok(SettleCause::Stop(StopReason::EmergencyStop))
        }
        (HaltCause::TimeBudget, tag) if tag == u16::MAX - 2 => Ok(SettleCause::TimeBudget),
        (HaltCause::Fence, tag) if tag == u16::MAX - 3 => {
            Ok(SettleCause::Stop(StopReason::DoorHalted))
        }
        (_, tag) => SettleCause::every()
            .get(usize::from(tag) - 1)
            .copied()
            .filter(|cause| halt_cause(*cause) == first)
            .ok_or(CoordinatorError::Stopped),
    }
}

pub(super) fn outcome(publication: PublicationClass) -> SettlementOutcome {
    match publication {
        PublicationClass::Published {
            recovery_required,
            interrupted,
        } => SettlementOutcome::Published {
            recovery_required,
            interrupted: interrupted
                .into_iter()
                .map(crate::evidence::PublicationCause::of)
                .collect(),
        },
        PublicationClass::ConfirmedNotPublished => SettlementOutcome::NotPublished,
        PublicationClass::Pending | PublicationClass::Uncertain => SettlementOutcome::Uncertain,
    }
}

impl<P: EffectPort> Coordinator<P> {
    /// Existing legacy calls retain their record behavior. File calls have a
    /// typed poison/recording result across the entire lock/owed-write chain.
    pub(super) fn step<T>(
        &self,
        body: impl FnOnce(&mut Inner) -> Result<T, CoordinatorError>,
    ) -> Result<T, CoordinatorError> {
        if self.file.is_some() {
            self.try_with_lock(body)
        } else {
            self.with_lock(body)
        }
    }
    pub(super) fn try_with_lock<T>(
        &self,
        body: impl FnOnce(&mut Inner) -> Result<T, CoordinatorError>,
    ) -> Result<T, CoordinatorError> {
        let (result, owed) = {
            let mut inner = self.inner.lock().map_err(|poison| {
                drop(poison);
                if let Some(file) = &self.file {
                    file.cell.engage_tagged(HaltTag::DoorHalted);
                    #[cfg(test)]
                    self.hooks.file_engagement.fire("lock-poison");
                }
                CoordinatorError::Poisoned
            })?;
            #[cfg(test)]
            let _held = crate::effect::held::Held::coordinator();
            let result = body(&mut inner);
            (result, std::mem::take(&mut inner.owed))
        };
        #[cfg(test)]
        self.hooks.before_owed_write.fire();
        let written = owed.write_checked();
        #[cfg(test)]
        self.hooks.after_owed_write.fire();
        if written.is_err() {
            if let Some(file) = &self.file {
                file.cell.engage_tagged(HaltTag::DoorHalted);
                #[cfg(test)]
                self.hooks.file_engagement.fire("owed-failure");
            }
        }
        let result = result?;
        written.map_err(|_| CoordinatorError::Recording)?;
        Ok(result)
    }

    /// Called before blocking preparation and immediately before invocation or
    /// dispatch. The budget and cell reads precede the coordinator lock.
    pub(super) fn file_boundary(&self, id: MissionId) -> Result<(), CoordinatorError> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        file.check_budget(&*self.clock);
        if file.cell.is_engaged() {
            #[cfg(test)]
            self.hooks.file_after_halt.fire();
            self.file_stop(id, SettleCause::Stop(StopReason::Owner))?;
            return Err(CoordinatorError::Stopped);
        }
        Ok(())
    }

    pub(super) fn file_stop(
        &self,
        id: MissionId,
        asked: SettleCause,
    ) -> Result<MissionState, CoordinatorError> {
        let cause = if let Some(file) = &self.file {
            let tag = cause_tag(asked);
            #[cfg(test)]
            let tag = if self
                .hooks
                .file_missing_cause
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                None
            } else {
                tag
            };
            let Some(tag) = tag else {
                file.cell.engage_tagged(HaltTag::DoorHalted);
                #[cfg(test)]
                self.hooks.file_engagement.fire("unknown-cause");
                return Err(CoordinatorError::Stopped);
            };
            file.cell.engage_tagged(HaltTag::Client {
                cause: halt_cause(asked),
                reason: tag,
            });
            #[cfg(test)]
            self.hooks.file_engagement.fire("file-stop");
            #[cfg(test)]
            self.hooks.file_after_halt.fire();
            let first = file.cell.reason().ok_or(CoordinatorError::Stopped)?;
            if super::lite_intent::is_terminal_intent(first) {
                return self.file_finish_intent(id);
            }
            first_cause(first)?
        } else {
            asked
        };
        self.file_record_end(id, cause, None)
    }

    /// The original cell is already engaged. Unresolved File settlement wins;
    /// otherwise preserve an ordinary failure exactly, including on replay.
    pub(super) fn file_record_end(
        &self,
        id: MissionId,
        cause: SettleCause,
        failure: Option<Trigger>,
    ) -> Result<MissionState, CoordinatorError> {
        enum Step {
            Done(MissionState),
            Plain(Result<MissionState, CoordinatorError>),
            Settle(u64, Option<PendingAction>, Result<(), CoordinatorError>),
        }
        let step = self.try_with_lock(|inner| {
            inner.usable()?;
            let m = inner.mission(id)?;
            if m.state.is_terminal() {
                return Ok(Step::Done(m.state));
            }
            if let Some((effect, _)) = m.settlement {
                let pending = m
                    .open_action
                    .as_ref()
                    .filter(|(e, _)| *e == effect)
                    .map(|(_, p)| p.clone());
                return Ok(Step::Settle(effect, pending, Ok(())));
            }
            if let Some(effect) = m.unresolved_file {
                let started = inner
                    .append(id, Event::SettlementStarted { effect, cause })
                    .map(|_| ());
                let m = inner.mission(id)?;
                m.settlement = Some((effect, cause));
                m.generation += 1;
                let pending = m
                    .open_action
                    .as_ref()
                    .filter(|(e, _)| *e == effect)
                    .map(|(_, p)| p.clone());
                return Ok(Step::Settle(effect, pending, started));
            }
            let trigger = failure.unwrap_or_else(|| plain_terminal(m, cause));
            let result = inner.fire(id, trigger.clone(), self.clock.monotonic_ms());
            if result.is_err() {
                let m = inner.mission(id)?;
                if let Ok(to) = transition(m.state, &trigger) {
                    m.apply(&trigger, to, self.clock.monotonic_ms());
                }
            }
            Ok(Step::Plain(result))
        });
        match step {
            Ok(Step::Done(state)) => {
                self.file_ack_all(id)?;
                Ok(state)
            }
            Ok(Step::Plain(result)) => {
                let withdrawn = self
                    .port
                    .withdraw(id)
                    .map_err(|r| CoordinatorError::Withdrawal(r.class));
                let closed = self.close_action(id);
                let acked = self.file_ack_all(id);
                let state = result?;
                withdrawn?;
                closed?;
                acked?;
                Ok(state)
            }
            Ok(Step::Settle(effect, pending, started)) => {
                let withdrawn = self
                    .port
                    .withdraw(id)
                    .map_err(|r| CoordinatorError::Withdrawal(r.class));
                // Exactly one snapshot after withdrawal, including when it failed.
                let snapshot = self.port.file_publication_for(id, effect);
                let (action, read) = match snapshot {
                    Ok(Some(s)) if pending.as_ref().is_none_or(|p| p == &s.action) => {
                        (Some(s.action), outcome(s.publication))
                    }
                    // The engaged cell prevents this not-yet-proposed effect
                    // from entering the broker when its dispatch resumes.
                    Ok(None) if pending.is_none() => (None, SettlementOutcome::NotPublished),
                    _ => (pending, SettlementOutcome::Uncertain),
                };
                let ended = self.file_terminal(id, effect, action, read, None);
                started?;
                let state = ended?;
                withdrawn?;
                Ok(state)
            }
            Err(error) => {
                // The original cell already stopped admission, even if the
                // registry or journal cannot be read. Narrowing cleanup is tried.
                let _withdrawal = self.port.withdraw(id);
                Err(error)
            }
        }
    }

    /// One terminal envelope after a durable start, or the ordinary poll's
    /// truthful terminal classification. No conclusive ack precedes durability.
    fn file_terminal(
        &self,
        id: MissionId,
        observed_effect: u64,
        action: Option<PendingAction>,
        read: SettlementOutcome,
        reserved: Option<u64>,
    ) -> Result<MissionState, CoordinatorError> {
        if let Some(generation) = reserved.filter(|_| self.file.is_some()) {
            self.file_latch_terminal(super::lite_intent::FileTerminal::Publication {
                id,
                effect: observed_effect,
                action,
                read,
                generation,
            })?;
            #[cfg(test)]
            self.hooks.file_engagement.fire("publication");
            return self.file_stop(id, SettleCause::Stop(StopReason::Owner));
        }
        // file_stop already engaged its exact cause before its first lock.
        // Recovery has no live File cell; it uses the durable settlement cause.
        self.file_record_terminal(id, observed_effect, action, read, reserved)
    }

    pub(super) fn file_record_terminal(
        &self,
        id: MissionId,
        observed_effect: u64,
        action: Option<PendingAction>,
        read: SettlementOutcome,
        reserved: Option<u64>,
    ) -> Result<MissionState, CoordinatorError> {
        let result = self.try_with_lock(|inner| {
            inner.usable()?;
            let m = inner.mission(id)?;
            if m.state.is_terminal() {
                return Ok(m.state);
            }
            let (effect, end) = match (m.settlement, reserved) {
                (Some((effect, cause)), None) => (effect, cause.end(&read)),
                (None, Some(generation)) if m.generation == generation => {
                    let effect = m
                        .waiting
                        .as_ref()
                        .map(|w| w.effect)
                        .filter(|e| m.unresolved_file == Some(*e))
                        .ok_or(CoordinatorError::Stopped)?;
                    let end = read
                        .ordinary_end()
                        .or_else(|| read.deadline_end())
                        .ok_or(CoordinatorError::Stopped)?;
                    (effect, end)
                }
                _ => return Err(CoordinatorError::Stopped),
            };
            if effect != observed_effect || m.unresolved_file != Some(effect) {
                return Err(CoordinatorError::Stopped);
            }
            let action = match &m.open_action {
                Some((e, pending)) if *e == effect => {
                    // A snapshot's outcome may never be relabelled with a
                    // different commitment. Lost crash evidence is uncertain.
                    if action.as_ref() != Some(pending)
                        && !(action.is_none() && read == SettlementOutcome::Uncertain)
                    {
                        return Err(CoordinatorError::Stopped);
                    }
                    Some(pending.clone())
                }
                Some(_) => return Err(CoordinatorError::Stopped),
                None => action,
            };
            let trigger = Trigger::Settled {
                effect,
                action,
                outcome: read.clone(),
                end,
            };
            inner.fire_settling(id, trigger, self.clock.monotonic_ms())
        });
        // No ack after a failed terminal append. An uncertainty never enters acks.
        let state = result?;
        let withdrawn = if reserved.is_some() {
            self.port
                .withdraw(id)
                .map_err(|r| CoordinatorError::Withdrawal(r.class))
        } else {
            Ok(())
        };
        let closed = self.file_close_action(id);
        let acked = self.file_ack_all(id);
        closed?;
        acked?;
        withdrawn?;
        Ok(state)
    }

    pub(super) fn file_notice(
        &self,
        id: MissionId,
    ) -> Result<Option<&'static str>, CoordinatorError> {
        self.try_with_lock(|inner| {
            Ok(inner
                .mission(id)?
                .terminal_uncertain
                .then_some(FILE_UNCERTAIN_NOTICE))
        })
    }

    /// This path is reachable only from a ConfirmedNotPublished snapshot.
    /// The proof survives cancellation; first-cause Stop still takes precedence.
    pub(super) fn file_nonpublication_end(
        &self,
        id: MissionId,
        generation: u64,
        trigger: Trigger,
    ) -> Result<MissionState, CoordinatorError> {
        self.file_latch_terminal(super::lite_intent::FileTerminal::Nonpublication {
            id,
            generation,
            trigger,
        })?;
        #[cfg(test)]
        self.hooks.file_engagement.fire("nonpublication");
        self.file_stop(id, SettleCause::Stop(StopReason::Owner))
    }

    pub(super) fn file_record_nonpublication(
        &self,
        id: MissionId,
        generation: u64,
        trigger: Trigger,
    ) -> Result<MissionState, CoordinatorError> {
        let state = self.try_with_lock(|inner| {
            inner.usable()?;
            let m = inner.mission(id)?;
            if m.state.is_terminal() {
                return Ok(m.state);
            }
            if m.generation != generation || m.settlement.is_some() {
                return Err(CoordinatorError::Stopped);
            }
            inner.fire(id, trigger, self.clock.monotonic_ms())
        })?;
        let withdrawn = self
            .port
            .withdraw(id)
            .map_err(|r| CoordinatorError::Withdrawal(r.class));
        let closed = self.file_close_action(id);
        let acked = self.file_ack_all(id);
        closed?;
        acked?;
        withdrawn?;
        Ok(state)
    }

    pub(super) fn file_ack_all(&self, id: MissionId) -> Result<(), CoordinatorError> {
        let acks = self.try_with_lock(|inner| {
            inner.usable()?;
            Ok(inner.mission(id)?.acks.clone())
        })?;
        let mut first = None;
        for action in acks {
            #[cfg(test)]
            self.hooks.file_before_ack.fire();
            match self.port.file_ack(&action) {
                Err(refusal) => {
                    first.get_or_insert(CoordinatorError::Broker(refusal.class));
                }
                Ok(()) => {
                    // Retire only this successfully acknowledged obligation in
                    // memory. Replay reconstructs it from the durable outcome,
                    // so a crash here repeats an exact, idempotent ack.
                    self.try_with_lock(|inner| {
                        inner.mission(id)?.acks.retain(|pending| pending != &action);
                        Ok(())
                    })?;
                }
            }
        }
        first.map_or(Ok(()), Err)
    }

    pub(super) fn file_close_action(&self, id: MissionId) -> Result<bool, CoordinatorError> {
        self.try_with_lock(|inner| {
            inner.usable()?;
            let m = inner.mission(id)?;
            if m.terminal_uncertain || m.unresolved_file.is_some() {
                return Ok(false);
            }
            let Some((effect, pending)) = m.open_action.clone() else {
                return Ok(true);
            };
            if !m.acks.contains(&pending) {
                return Ok(false);
            }
            inner.append(id, Event::ApprovalClosed { effect })?;
            inner.mission(id)?.open_action = None;
            Ok(true)
        })
    }

    pub(super) fn file_approval(
        &self,
        id: MissionId,
        generation: u64,
    ) -> Result<FileDecision, CoordinatorError> {
        let (effect, pending) = self.try_with_lock(|inner| {
            let m = inner.mission(id)?;
            let (effect, pending) = m.open_action.clone().ok_or(CoordinatorError::Stopped)?;
            if m.generation != generation
                || m.unresolved_file != Some(effect)
                || m.waiting.as_ref().is_none_or(|w| w.effect != effect)
            {
                return Err(CoordinatorError::Stopped);
            }
            Ok((effect, pending))
        })?;
        let elapsed = self.port.file_deadline_elapsed(&pending);
        if elapsed {
            self.file_latch_terminal(super::lite_intent::FileTerminal::Deadline {
                id,
                effect,
                pending,
                generation,
                resolved: std::sync::OnceLock::new(),
            })?;
            #[cfg(test)]
            self.hooks.file_engagement.fire("deadline");
            return self
                .file_stop(id, SettleCause::Stop(StopReason::Owner))
                .map(FileDecision::Ended);
        }
        // D3: never read the publication before withdrawing an elapsed wait.
        let snapshot = self
            .port
            .file_publication_for(id, effect)
            .ok()
            .flatten()
            .filter(|s| s.action == pending);
        let FileSnapshot {
            action,
            publication,
            state,
            ..
        } = snapshot.unwrap_or(FileSnapshot {
            action: pending,
            publication: PublicationClass::Uncertain,
            state: None,
            receipt: None,
        });
        let status = match &publication {
            PublicationClass::Pending if !elapsed => Some(ApprovalStatus::Waiting),
            PublicationClass::Published {
                recovery_required: false,
                interrupted,
            } if interrupted.is_empty() && !elapsed => Some(ApprovalStatus::Approved),
            PublicationClass::ConfirmedNotPublished => Some(match state {
                Some(CommitmentState::Denied) => ApprovalStatus::Denied,
                Some(CommitmentState::Expired) => ApprovalStatus::Expired,
                _ if elapsed => ApprovalStatus::Expired,
                _ => ApprovalStatus::Lapsed {
                    class: "cancelled".into(),
                },
            }),
            _ => None,
        };
        if let Some(status) = status {
            return Ok(FileDecision::Status(status));
        }
        let action = (!action.commitment.is_empty()).then_some(action);
        let state =
            self.file_terminal(id, effect, action, outcome(publication), Some(generation))?;
        Ok(FileDecision::Ended(state))
    }

    /// A new broker cannot restore the lost File execution state. Settle every
    /// unresolved journaled File before any generic adoption or held resume.
    pub(super) fn recover_files(&self) -> Result<(), CoordinatorError> {
        let ids = self.try_with_lock(|inner| {
            Ok(inner
                .missions
                .iter()
                .filter(|(_, m)| m.file_mode)
                .map(|(id, _)| *id)
                .collect::<Vec<_>>())
        })?;
        for id in ids {
            let unresolved = self.try_with_lock(|inner| {
                let m = inner.mission(id)?;
                if m.state.is_terminal() {
                    return Ok(false);
                }
                if let Some(effect) = m.unresolved_file {
                    if m.settlement.is_none() {
                        let cause = SettleCause::Stop(StopReason::DoorHalted);
                        inner.append(id, Event::SettlementStarted { effect, cause })?;
                        inner.mission(id)?.settlement = Some((effect, cause));
                    }
                    Ok(true)
                } else {
                    inner.fire(
                        id,
                        Trigger::Stop {
                            reason: StopReason::DoorHalted,
                        },
                        self.clock.monotonic_ms(),
                    )?;
                    Ok(false)
                }
            })?;
            if unresolved {
                let effect = self.try_with_lock(|inner| {
                    inner
                        .mission(id)?
                        .unresolved_file
                        .ok_or(CoordinatorError::Stopped)
                })?;
                self.file_terminal(id, effect, None, SettlementOutcome::Uncertain, None)?;
            } else {
                self.file_close_action(id)?;
                self.file_ack_all(id)?;
            }
        }
        Ok(())
    }
}
