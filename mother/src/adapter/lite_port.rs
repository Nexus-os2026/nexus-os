//! The typed File-only port path. Legacy adoption and Stop remain in mod.rs.
use super::*;

impl GovernedPort {
    /// Non-File retirement retains the legacy final-state rules. Unlike
    /// admission, this read must work after the File session's halt engages.
    /// Files themselves still require publication proof and exact ack.
    pub(super) fn non_file_settled_lite(&self, pending: &PendingAction) -> bool {
        if !self.file_runs().is_ok_and(|runs| !runs.fenced) {
            return false;
        }
        let Some(commitment) = CommitmentId::parse(&pending.commitment) else {
            return false;
        };
        match self.door.file_observation(commitment) {
            Ok(Lookup::Unknown) => true,
            Ok(Lookup::NotMine { settled }) => settled,
            Ok(Lookup::Mine((view, _))) => {
                view.kind != nexus_governed_control::authority::effect::CapabilityKind::Files
                    && RunId::parse(&pending.run) == Some(view.run)
                    && view.agent == mother_agent()
                    && !matches!(
                        view.state,
                        CommitmentState::Prepared
                            | CommitmentState::Authorized
                            | CommitmentState::Executing
                    )
            }
            Err(_) => false,
        }
    }

    pub(super) fn file_runs(&self) -> Result<std::sync::MutexGuard<'_, Runs>, EffectRefusal> {
        self.runs.lock().map_err(|poison| {
            drop(poison);
            if let Some((_, cell)) = self.file_session.get() {
                cell.engage_tagged(HaltTag::DoorHalted);
                #[cfg(test)]
                self.file_engagement.fire("port-poison");
            }
            refused(BrokerClass::PortFenced)
        })
    }

    pub(super) fn file_enter(&self) -> Result<(), EffectRefusal> {
        self.admit_host()?;
        let Some((_, cell)) = self.file_session.get() else {
            return Err(refused(BrokerClass::NotAdmitted));
        };
        if cell.is_engaged() {
            return Err(refused(BrokerClass::Halted));
        }
        if self.file_runs()?.fenced {
            return Err(refused(BrokerClass::PortFenced));
        }
        Ok(())
    }

    pub(super) fn admit_lite(
        &self,
        mission: MissionId,
        admission: &Admission,
    ) -> Result<(), EffectRefusal> {
        self.file_enter()?;
        if self.workspace.as_ref() != Some(&admission.folder) {
            return Err(refused(BrokerClass::NotAdmitted));
        }
        {
            let runs = self.file_runs()?;
            if runs.file_admissions.keys().any(|id| *id != mission) {
                return Err(refused(BrokerClass::AlreadyAdmitted));
            }
            if let Some(known) = runs.file_admissions.get(&mission) {
                return if known == admission {
                    Ok(())
                } else {
                    Err(refused(BrokerClass::AlreadyAdmitted))
                };
            }
        }
        let run = self.run_for(mission)?;
        self.admit_host()?;
        self.door
            .admit_files(run, &admission.contract_hash, &admission.consent_request)
            .map_err(|e| refused_by(e.class()))?;
        self.file_enter()?;
        let mut runs = self.file_runs()?;
        #[cfg(test)]
        session_port::session_projection_tests::at("file_binding_project", "before");
        self.admit_host()?;
        runs.file_admissions.insert(mission, admission.clone());
        #[cfg(test)]
        session_port::session_projection_tests::at("file_binding_project", "after");
        Ok(())
    }

    /// One immutable snapshot. Unknown/unreadable never means no publication.
    pub(super) fn publication_lite(
        &self,
        mission: MissionId,
        effect: u64,
    ) -> Result<Option<FileSnapshot>, EffectRefusal> {
        let pending = {
            let runs = self.file_runs()?;
            match runs.file_effects.get(&mission) {
                None => return Ok(None),
                Some(known) if *known < effect => return Ok(None),
                Some(known) if *known != effect => return Err(refused(BrokerClass::ActionNotHeld)),
                Some(_) => {}
            }
            match (runs.open.get(&mission), runs.waiting.get(&mission)) {
                (Some(run), Some((id, deadline))) => PendingAction {
                    run: run.to_string(),
                    commitment: id.to_string(),
                    deadline_mono_ms: *deadline,
                },
                // The proposal began but has not yielded an exact commitment,
                // or its evidence was lost: absence is not nonpublication.
                _ => return Err(refused(BrokerClass::ActionNotHeld)),
            }
        };
        let id = CommitmentId::parse(&pending.commitment)
            .ok_or_else(|| refused(BrokerClass::ActionNotHeld))?;
        let (publication, state) = match self
            .door
            .file_observation(id)
            .map_err(|e| refused_by(e.class()))?
        {
            Lookup::Mine((view, read)) if view.run.to_string() == pending.run => {
                (read, Some(view.state))
            }
            _ => (PublicationClass::Uncertain, None),
        };
        Ok(Some(FileSnapshot {
            action: pending,
            publication,
            state,
        }))
    }

    /// Narrowing cancellation works after cell engagement. All record outcomes
    /// are returned, with no port lock held while they are written.
    pub(super) fn withdraw_lite(&self, mission: MissionId) -> Result<(), EffectRefusal> {
        let run = {
            let mut runs = self.file_runs()?;
            runs.ended.insert(mission);
            runs.open.get(&mission).copied()
        };
        if let Some(run) = run {
            let batch = self
                .door
                .cancel_run(run)
                .map_err(|e| refused_by(e.class()))?;
            let mut owed = Owed::default();
            owed.push(batch);
            owed.write_checked().map_err(|e| refused_by(e.class()))?;
        }
        Ok(())
    }
}
