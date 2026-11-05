//! Session-only acknowledgement proof. No journal path, record hash or caller
//! boolean is accepted by ack. The original Home binding and retained native
//! action select the record; the backend verifies and synchronizes it itself.
use super::{SessionDoor, SessionIdentity, SEALED};
use crate::authority::commitment::{CommitmentState, PublicationClass};
use crate::authority::ids::{CommitmentId, RunId};
use crate::authority::run::GroupId;
use crate::authority::AuthorityError;
use crate::governed::Lookup;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) struct History {
    root: PathBuf,
    identity: Metadata,
}
impl History {
    pub(super) fn bind(root: &Path) -> Result<Self, AuthorityError> {
        let root = root.canonicalize().map_err(|_| unavailable())?;
        let identity = std::fs::symlink_metadata(&root).map_err(|_| unavailable())?;
        if !identity.is_dir() {
            return Err(unavailable());
        }
        Ok(Self { root, identity })
    }
    fn verify_root(&self) -> Result<(), AuthorityError> {
        let now = std::fs::symlink_metadata(&self.root).map_err(|_| unavailable())?;
        if !now.is_dir() || !same(&now, &self.identity) {
            return Err(unavailable());
        }
        Ok(())
    }
    fn read(&self) -> Result<(File, Vec<Line>), AuthorityError> {
        self.verify_root()?;
        match std::fs::symlink_metadata(self.root.join("mother-journal.jsonl.poisoned")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(unavailable()),
        }
        let path = self.root.join("mother-journal.jsonl");
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| unavailable())?;
        if !metadata.is_file() {
            return Err(unavailable());
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options.open(path).map_err(|_| unavailable())?;
        if !same(&metadata, &file.metadata().map_err(|_| unavailable())?) {
            return Err(unavailable());
        }
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(|_| unavailable())?;
        self.verify_root()?;
        if !text.is_empty() && !text.ends_with('\n') {
            return Err(unavailable());
        }
        let mut previous = "0".repeat(64);
        let mut lines = Vec::new();
        for (sequence, raw) in text.split_terminator('\n').enumerate() {
            let value: Value = serde_json::from_str(raw).map_err(|_| unavailable())?;
            let mission = value["mission"].as_u64().ok_or_else(unavailable)?;
            let hash = value["hash"].as_str().ok_or_else(unavailable)?;
            // Preserve the canonical event's field order in its hash input.
            // The outer canonical shape rules out duplicate top-level fields.
            let prefix = format!("{{\"format\":2,\"seq\":{sequence},\"prev\":\"{previous}\",\"mission\":{mission},\"event\":");
            let suffix = format!(",\"hash\":\"{hash}\"}}");
            let event = raw
                .strip_prefix(&prefix)
                .and_then(|s| s.strip_suffix(&suffix))
                .ok_or_else(unavailable)?;
            let mut digest = Sha256::new();
            digest.update(b"nexus-mother-journal-v2\n");
            for part in [
                previous.as_bytes(),
                &[2],
                &(sequence as u64).to_be_bytes(),
                &mission.to_be_bytes(),
                event.as_bytes(),
            ] {
                digest.update((part.len() as u64).to_be_bytes());
                digest.update(part);
            }
            if hex::encode(digest.finalize()) != hash {
                return Err(unavailable());
            }
            previous = hash.to_owned();
            lines.push(Line {
                mission,
                sequence: sequence as u64,
                hash: previous.clone(),
                event: value["event"].clone(),
            });
        }
        Ok((file, lines))
    }

    pub(super) fn report_lines(&self) -> Result<(bool, Vec<Line>), AuthorityError> {
        self.verify_root()?;
        match std::fs::symlink_metadata(self.root.join("mother-journal.jsonl")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::symlink_metadata(self.root.join("mother-journal.jsonl.poisoned")) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Ok((true, Vec::new()))
                    }
                    _ => Err(unavailable()),
                }
            }
            Err(_) => Err(unavailable()),
            Ok(_) => self.read().map(|(_, lines)| (false, lines)),
        }
    }
    pub(super) fn waiting(&self, action: &Action) -> Result<(), AuthorityError> {
        let (file, lines) = self.read()?;
        let mission = action.mission.ok_or_else(unavailable)?;
        let effect = action.effect.ok_or_else(unavailable)?;
        let run = action.run.to_string();
        let commitment = action.commitment.ok_or_else(unavailable)?.to_string();
        let mut pending = false;
        let mut waiting = false;
        for line in lines.iter().filter(|line| line.mission == mission) {
            let e = &line.event;
            if e["event"] == "approval_pending" && e["effect"] == effect {
                if pending || e["pending"]["run"] != run || e["pending"]["commitment"] != commitment
                {
                    return Err(unavailable());
                }
                pending = true;
            }
            if e["event"] == "settlement_started" || e["event"] == "held" {
                return Err(unavailable());
            }
            if e["event"] == "transition" {
                if waiting {
                    return Err(unavailable());
                }
                if e["trigger"]["trigger"] == "await_approval" && e["trigger"]["effect"] == effect {
                    if !pending {
                        return Err(unavailable());
                    }
                    waiting = true;
                }
            }
        }
        if !waiting {
            return Err(unavailable());
        }
        // Same opened file and fixed Home identity; journal data merely
        // corroborates the retained native action, never creates authority.
        file.sync_all().map_err(|_| unavailable())?;
        self.verify_root()?;
        if !same(
            &file.metadata().map_err(|_| unavailable())?,
            &std::fs::symlink_metadata(self.root.join("mother-journal.jsonl"))
                .map_err(|_| unavailable())?,
        ) {
            return Err(unavailable());
        }
        let parent = File::open(&self.root).map_err(|_| unavailable())?;
        if !same(
            &parent.metadata().map_err(|_| unavailable())?,
            &self.identity,
        ) {
            return Err(unavailable());
        }
        parent.sync_all().map_err(|_| unavailable())?;
        self.verify_root()
    }

    pub(super) fn verify_end(&self, count: usize, previous: &str) -> Result<(), AuthorityError> {
        let (file, lines) = self.read()?;
        if lines.len() != count + 1
            || count == 0
            || lines[count - 1].hash != previous
            || lines[count].mission != 0
            || lines[count].event["event"] != "session_closed"
        {
            return Err(unavailable());
        }
        file.sync_all().map_err(|_| unavailable())?;
        self.verify_root()?;
        if !same(
            &file.metadata().map_err(|_| unavailable())?,
            &std::fs::symlink_metadata(self.root.join("mother-journal.jsonl"))
                .map_err(|_| unavailable())?,
        ) {
            return Err(unavailable());
        }
        let parent = File::open(&self.root).map_err(|_| unavailable())?;
        if !same(
            &parent.metadata().map_err(|_| unavailable())?,
            &self.identity,
        ) {
            return Err(unavailable());
        }
        parent.sync_all().map_err(|_| unavailable())?;
        self.verify_root()
    }

    fn terminal(
        &self,
        action: &Action,
        publication: &PublicationClass,
        state: CommitmentState,
        #[cfg(test)] cell: &crate::authority::file_session::HaltCell,
    ) -> Result<Terminal, AuthorityError> {
        let (file, lines) = self.read()?;
        let mission = action.mission.ok_or_else(unavailable)?;
        let effect = action.effect.ok_or_else(unavailable)?;
        let commitment = action.commitment.ok_or_else(unavailable)?.to_string();
        let run = action.run.to_string();
        let mut pending = false;
        let mut terminal = None;
        for line in lines {
            if line.mission != mission {
                continue;
            }
            let e = &line.event;
            if e["event"] == "approval_pending" && e["effect"] == effect {
                if pending || e["pending"]["run"] != run || e["pending"]["commitment"] != commitment
                {
                    return Err(unavailable());
                }
                pending = true;
            }
            let t = &e["trigger"];
            if e["event"] != "transition" || t["effect"] != effect {
                continue;
            }
            let kind = t["trigger"].as_str().unwrap_or("");
            let matches = match kind {
                "action_approved" => {
                    pending
                        && matches!(publication, PublicationClass::Published { recovery_required: false, interrupted } if interrupted.is_empty())
                }
                "action_denied" => {
                    pending
                        && state == CommitmentState::Denied
                        && *publication == PublicationClass::ConfirmedNotPublished
                }
                "approval_lapsed" => {
                    pending && *publication == PublicationClass::ConfirmedNotPublished
                }
                "settled" => {
                    let exact =
                        t["action"]["run"] == run && t["action"]["commitment"] == commitment;
                    exact && outcome_matches(&t["outcome"], publication)
                }
                _ => continue,
            };
            if !matches || terminal.is_some() {
                return Err(unavailable());
            }
            terminal = Some(Terminal {
                sequence: line.sequence,
                hash: hex::decode(line.hash)
                    .map_err(|_| unavailable())?
                    .try_into()
                    .map_err(|_| unavailable())?,
            });
        }
        let terminal = terminal.ok_or_else(unavailable)?;
        // Reading a line is not its durability proof. The same opened file and
        // its identity-checked parent are synchronized before receipt install.
        #[cfg(test)]
        cell.1.fire("ack.terminal.file_sync");
        file.sync_all().map_err(|_| unavailable())?;
        self.verify_root()?;
        if !same(
            &file.metadata().map_err(|_| unavailable())?,
            &std::fs::symlink_metadata(self.root.join("mother-journal.jsonl"))
                .map_err(|_| unavailable())?,
        ) {
            return Err(unavailable());
        }
        let parent = File::open(&self.root).map_err(|_| unavailable())?;
        if !same(
            &parent.metadata().map_err(|_| unavailable())?,
            &self.identity,
        ) {
            return Err(unavailable());
        }
        #[cfg(test)]
        cell.1.fire("ack.terminal.parent_sync");
        parent.sync_all().map_err(|_| unavailable())?;
        self.verify_root()?;
        Ok(terminal)
    }
}
fn outcome_matches(value: &Value, publication: &PublicationClass) -> bool {
    match publication {
        PublicationClass::ConfirmedNotPublished => value["outcome"] == "not_published",
        PublicationClass::Published {
            recovery_required,
            interrupted,
        } => {
            value["outcome"] == "published"
                && value["recovery_required"] == *recovery_required
                && value["interrupted"].as_array().is_some_and(|causes| {
                    causes.len() == interrupted.len()
                        && causes
                            .iter()
                            .zip(interrupted)
                            .all(|(a, b)| a.as_str() == Some(*b))
                })
        }
        _ => false,
    }
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        false
    }
}
fn unavailable() -> AuthorityError {
    AuthorityError::EvidenceUnavailable
}
pub(super) struct Line {
    pub(super) mission: u64,
    sequence: u64,
    pub(super) hash: String,
    pub(super) event: Value,
}
#[derive(Clone)]
struct Terminal {
    sequence: u64,
    hash: [u8; 32],
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Conclusive {
    NotPublished,
    Published {
        recovery: bool,
        interrupted: [Option<&'static str>; 8],
    },
}
impl Conclusive {
    fn of(publication: &PublicationClass) -> Result<Self, AuthorityError> {
        match publication {
            PublicationClass::ConfirmedNotPublished => Ok(Self::NotPublished),
            PublicationClass::Published {
                recovery_required,
                interrupted,
            } if interrupted.len() <= 8 => {
                let mut causes = [None; 8];
                for (slot, cause) in causes.iter_mut().zip(interrupted) {
                    *slot = Some(*cause);
                }
                Ok(Self::Published {
                    recovery: *recovery_required,
                    interrupted: causes,
                })
            }
            _ => Err(unavailable()),
        }
    }
}
#[derive(Clone)]
struct Receipt {
    outcome: Conclusive,
    terminal: Terminal,
    released: bool,
}
#[derive(Clone)]
pub(super) struct Action {
    pub(super) session: SessionIdentity,
    pub(super) request: u64,
    pub(super) door_request: u64,
    pub(super) preparation: u64,
    pub(super) door: GroupId,
    pub(super) run: RunId,
    pub(super) mission: Option<u64>,
    pub(super) effect: Option<u64>,
    pub(super) commitment: Option<CommitmentId>,
    receipt: Option<Receipt>,
    ack_claimed: bool,
}
impl Action {
    pub(super) fn observation(
        &self,
        native: Option<PublicationClass>,
    ) -> super::outcome::ActionObservation {
        use super::outcome::{Acknowledgement, ActionObservation, RecordingStatus};
        let publication = self
            .receipt
            .as_ref()
            .map(|receipt| match &receipt.outcome {
                Conclusive::NotPublished => PublicationClass::ConfirmedNotPublished,
                Conclusive::Published {
                    recovery,
                    interrupted,
                } => PublicationClass::Published {
                    recovery_required: *recovery,
                    interrupted: interrupted.iter().copied().flatten().collect(),
                },
            })
            .or(native)
            .unwrap_or(PublicationClass::Uncertain);
        ActionObservation {
            target: self.preparation,
            run: self.run,
            mission: self.mission,
            effect: self.effect,
            commitment: self.commitment,
            publication,
            mother: if self.receipt.is_some() {
                RecordingStatus::Recorded
            } else {
                RecordingStatus::Unknown
            },
            mother_sequence: self.receipt.as_ref().map(|r| r.terminal.sequence),
            mother_hash: self.receipt.as_ref().map(|r| r.terminal.hash),
            acknowledgement: if self.acknowledged() {
                Acknowledgement::Verified
            } else {
                Acknowledgement::Pending
            },
        }
    }
    pub(super) fn acknowledged(&self) -> bool {
        self.receipt.as_ref().is_some_and(|receipt| {
            receipt.released && receipt.terminal.hash != [0; 32] && receipt.terminal.sequence > 0
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn reserved(
        session: SessionIdentity,
        request: u64,
        door_request: u64,
        preparation: u64,
        door: GroupId,
        run: RunId,
        consent: String,
        effect: Option<u64>,
    ) -> Self {
        let mission = consent
            .split('/')
            .next()
            .and_then(|s| s.strip_prefix("msn-"))
            .and_then(|s| s.parse().ok());
        Self {
            session,
            request,
            door_request,
            preparation,
            door,
            run,
            mission,
            effect,
            commitment: None,
            receipt: None,
            ack_claimed: false,
        }
    }
}
pub(super) fn ack(door: &SessionDoor, id: CommitmentId) -> Result<(), AuthorityError> {
    if door.core.cell.session_word() & SEALED != 0 {
        return Err(AuthorityError::Halted);
    }
    let lock = || {
        door.core.actions.lock().map_err(|_| {
            door.context.fail();
            unavailable()
        })
    };
    let (slot, action) = {
        let mut actions = lock()?;
        let (slot, action) = actions
            .iter_mut()
            .enumerate()
            .find(|(_, a)| a.commitment == Some(id))
            .ok_or(AuthorityError::UnknownCommitment)?;
        if action.session != door.core.identity
            || action.door != door.door.origin()
            || action.door_request != door.origin_request
            || action.preparation == 0
        {
            return Err(AuthorityError::NotMember);
        }
        if action.acknowledged() {
            return Ok(());
        }
        if action.ack_claimed {
            return Err(unavailable());
        }
        action.ack_claimed = true;
        (slot, action.clone())
    };
    // The exact claim and pin remain owed while verification and native ack
    // run outside the session-wide metadata hold. Unwind retains the claim.
    let unchanged = |current: &Action| {
        current.ack_claimed
            && current.session == action.session
            && current.request == action.request
            && current.door_request == action.door_request
            && current.preparation == action.preparation
            && current.door == action.door
            && current.run == action.run
            && current.mission == action.mission
            && current.effect == action.effect
            && current.commitment == action.commitment
    };
    let result = (|| {
        let Lookup::Mine((view, publication)) = door.door.file_observation(id)? else {
            return Err(AuthorityError::UnknownCommitment);
        };
        if view.run != action.run
            || !view.state.is_final()
            || matches!(
                publication,
                PublicationClass::Pending | PublicationClass::Uncertain
            )
        {
            return Err(unavailable());
        }
        let outcome = Conclusive::of(&publication)?;
        let receipt = match action.receipt {
            Some(ref receipt) if receipt.outcome == outcome => receipt.clone(),
            Some(_) => return Err(unavailable()),
            None => Receipt {
                outcome,
                terminal: door.core.history.get().ok_or_else(unavailable)?.terminal(
                    &action,
                    &publication,
                    view.state,
                    #[cfg(test)]
                    &door.context.cell,
                )?,
                released: false,
            },
        };
        {
            let mut actions = lock()?;
            let current = actions
                .get_mut(slot)
                .filter(|a| unchanged(a))
                .ok_or_else(unavailable)?;
            current.receipt = Some(receipt);
        }
        // Install durable proof before releasing its exact native pin. The
        // receipt survives failure, pruning or a lost return; retries are exact.
        door.door.ack_commitment(id)?;
        let mut actions = lock()?;
        let current = actions
            .get_mut(slot)
            .filter(|a| unchanged(a))
            .ok_or_else(unavailable)?;
        current.receipt.as_mut().ok_or_else(unavailable)?.released = true;
        Ok(())
    })();
    let mut actions = lock()?;
    let current = actions
        .get_mut(slot)
        .filter(|a| unchanged(a))
        .ok_or_else(unavailable)?;
    current.ack_claimed = false;
    result
}
