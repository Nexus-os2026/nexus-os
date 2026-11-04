//! The governed coding worker (Phase One): a local model proposes bounded
//! edits into a run's private staging snapshot.
//!
//! The worker owns no authority. It holds no grant, no directory handle, no
//! process, no network destination and no credential; it can only call the
//! backend operations below on the run it was given:
//!
//! - list the staged paths;
//! - read a staged file inside the read scope;
//! - submit a typed edit proposal (applied to staging by [`CodingRun::edit`],
//!   which enforces the frozen scopes);
//! - ask for another model turn;
//! - finish.
//!
//! Model output is untrusted data. Each answer is parsed into the closed
//! action structure of the kernel's one worker grammar
//! ([`crate::inference::grammar`]); anything else (an unknown action or
//! field, a shell command, a URL, an approval claim, a provider switch) ends
//! the run: it is recorded by digest and size, and grants nothing because no
//! such operation exists. An accepted answer is kept, and sent back to the
//! model, in its retained form (`retained-answer-v1`, M2e N1).
//! Project content — `AGENTS.md`, `CLAUDE.md`, READMEs, comments, tests — is
//! shown to the model as delimited data; the instruction that it cannot
//! change Nexus authority is a courtesy, not the enforcement.
//!
//! Bounds are backend constants ([`WORKER_LIMITS`]); no caller can raise
//! them. After the model finishes, the candidate is structurally verified;
//! the verified candidate hash is what any later review binds to. The worker
//! never writes to the owner's project.

use std::time::{Duration, Instant};

use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::ledger::EventKind;
use super::local_model::{LocalModel, ModelError, ModelMessage, ModelRole};
use super::scope::{RelPath, ScopeError};
use super::structural::StructuralProfile;
use super::{
    CandidateEdit, CodingRun, EditRejection, FailureReason, RunError, RunState,
    StructuralVerification,
};
use crate::inference::grammar::{self, EditAction, RawEdit};
use crate::inference::RetainedAnswer;

/// Frozen worker bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerLimits {
    pub max_turns: u32,
    pub max_files_read: u32,
    pub max_bytes_read: u64,
    pub max_candidate_files: u32,
    pub max_candidate_bytes: u64,
    pub max_proposals: u32,
    pub max_response_bytes: u64,
    pub max_task_bytes: usize,
    pub deadline: Duration,
}

/// The Phase One worker bounds. Not configurable by any caller.
pub const WORKER_LIMITS: WorkerLimits = WorkerLimits {
    max_turns: 16,
    max_files_read: 64,
    max_bytes_read: 2 * 1024 * 1024,
    max_candidate_files: 32,
    max_candidate_bytes: 1024 * 1024,
    max_proposals: 64,
    max_response_bytes: 512 * 1024,
    max_task_bytes: 16 * 1024,
    deadline: Duration::from_secs(15 * 60),
};

/// The longest path spelling echoed back to the model in feedback.
const MAX_RECORDED_PATH: usize = 256;
/// The most paths listed for the model.
const MAX_LISTED_PATHS: usize = 2000;

/// The name of an answer's action, as the turn record names it.
fn action_name(action: &EditAction) -> &'static str {
    match action {
        EditAction::Read { .. } => "read",
        EditAction::Edit { .. } => "edit",
        EditAction::Finish { .. } => "finish",
    }
}

/// A typed edit proposal: the only form in which model output can reach
/// staging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditProposal {
    pub path: RelPath,
    pub op: ProposalOp,
    pub content: Vec<u8>,
}

/// Phase One proposal operations. Deletion is not supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalOp {
    Create,
    Replace,
}

impl EditProposal {
    fn into_edit(self) -> CandidateEdit {
        match self.op {
            ProposalOp::Create => CandidateEdit::Create {
                path: self.path,
                content: self.content,
            },
            ProposalOp::Replace => CandidateEdit::Replace {
                path: self.path,
                content: self.content,
            },
        }
    }
}

/// Why a model answer or proposal was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalRejection {
    /// Not one closed JSON action (unknown action or field, not JSON).
    MalformedResponse,
    /// An operation other than create or replace.
    UnsupportedOperation,
    InvalidPath(ScopeError),
    NotText,
    TooLarge,
    /// A worker bound (files, bytes or proposals) was reached.
    LimitReached,
    /// The run refused the edit or read (scope, protection, existence).
    Refused(EditRejection),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkerError {
    #[error("the run has no pinned model, or a different one")]
    ModelNotPinned,
    #[error("task description is empty or too long")]
    InvalidTask,
    #[error("local model: {0}")]
    Model(ModelError),
    #[error("the worker exceeded its turn limit")]
    TurnLimit,
    #[error("the worker exceeded its deadline")]
    Deadline,
    #[error("run: {0}")]
    Run(RunError),
}

/// What the worker did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerReport {
    pub turns: u32,
    pub files_read: u32,
    pub bytes_read: u64,
    pub accepted: Vec<RelPath>,
    pub rejected: u32,
    /// The structural result of the finished candidate; `None` if the model
    /// finished without changing anything (the run is then cancelled).
    pub verification: Option<StructuralVerification>,
}

struct Budget {
    limits: WorkerLimits,
    started: Instant,
    turns: u32,
    files_read: u32,
    bytes_read: u64,
    proposals: u32,
    candidate: std::collections::BTreeMap<RelPath, u64>,
    rejected: u32,
}

impl Budget {
    fn candidate_bytes(&self) -> u64 {
        self.candidate.values().sum()
    }

    fn remaining(&self) -> Option<Duration> {
        self.limits.deadline.checked_sub(self.started.elapsed())
    }
}

/// Run the worker on a staged run with its pinned local model, then verify
/// the candidate. Uses the frozen [`WORKER_LIMITS`].
pub fn run_worker(
    run: &mut CodingRun,
    model: &dyn LocalModel,
    task: &str,
) -> Result<WorkerReport, WorkerError> {
    drive(run, model, task, WORKER_LIMITS)
}

#[cfg(test)]
pub(crate) fn run_worker_with_limits(
    run: &mut CodingRun,
    model: &dyn LocalModel,
    task: &str,
    limits: WorkerLimits,
) -> Result<WorkerReport, WorkerError> {
    drive(run, model, task, limits)
}

fn drive(
    run: &mut CodingRun,
    model: &dyn LocalModel,
    task: &str,
    limits: WorkerLimits,
) -> Result<WorkerReport, WorkerError> {
    if run.state() != RunState::Staged {
        return Err(WorkerError::Run(RunError::InvalidState {
            operation: "worker",
            state: run.state(),
        }));
    }
    if run.model_pin() != Some(model.pin()) {
        return Err(WorkerError::ModelNotPinned);
    }
    if task.trim().is_empty() || task.len() > limits.max_task_bytes {
        return Err(WorkerError::InvalidTask);
    }
    let marker = uuid::Uuid::new_v4().simple().to_string();
    run.record_worker(
        EventKind::WorkerStarted,
        &json!({
            "task_sha256": hex::encode(Sha256::digest(task.as_bytes())),
            "task_bytes": task.len(),
            "limits": {
                "turns": limits.max_turns,
                "files_read": limits.max_files_read,
                "bytes_read": limits.max_bytes_read,
                "candidate_files": limits.max_candidate_files,
                "candidate_bytes": limits.max_candidate_bytes,
                "proposals": limits.max_proposals,
                "response_bytes": limits.max_response_bytes,
                "deadline_secs": limits.deadline.as_secs(),
            },
        }),
    )
    .map_err(|error| abort(run, error))?;

    let mut messages = vec![
        ModelMessage {
            role: ModelRole::System,
            content: grammar::WORKER_SYSTEM_INSTRUCTION.to_string(),
        },
        ModelMessage {
            role: ModelRole::User,
            content: opening_message(run, task, &marker),
        },
    ];
    let mut budget = Budget {
        limits,
        started: Instant::now(),
        turns: 0,
        files_read: 0,
        bytes_read: 0,
        proposals: 0,
        candidate: Default::default(),
        rejected: 0,
    };
    let mut accepted: Vec<RelPath> = Vec::new();
    let summary;

    loop {
        if budget.turns >= limits.max_turns {
            return Err(stop(run, WorkerError::TurnLimit));
        }
        let Some(remaining) = budget.remaining() else {
            return Err(stop(run, WorkerError::Deadline));
        };
        // M0-FIX F5: no turn starts after the owner's emergency stop.
        run.halt_if_stopped().map_err(WorkerError::Run)?;
        budget.turns += 1;
        let answer = match model.complete(&messages, remaining, limits.max_response_bytes) {
            Ok(answer) => answer,
            Err(ModelError::Timeout) if budget.remaining().is_none() => {
                return Err(stop(run, WorkerError::Deadline))
            }
            Err(ModelError::Stopped) => {
                run.halt_if_stopped().map_err(WorkerError::Run)?;
                return Err(stop(run, WorkerError::Model(ModelError::Stopped)));
            }
            Err(error) => return Err(stop(run, WorkerError::Model(error))),
        };
        // An answer that returns after the stop is never used.
        run.halt_if_stopped().map_err(WorkerError::Run)?;
        if budget.remaining().is_none() {
            return Err(stop(run, WorkerError::Deadline));
        }
        if answer.len() as u64 > limits.max_response_bytes {
            return Err(stop(run, WorkerError::Model(ModelError::ResponseTooLarge)));
        }
        // An accepted answer is retained in its retained form (N1 spec
        // §2.2): those are the bytes recorded and sent back. An answer that
        // is not exactly one closed action ends the run (fail closed): it is
        // recorded by digest and size, and nothing fabricated is ever sent.
        let accepted_answer = grammar::parse(&answer)
            .ok()
            .and_then(|action| RetainedAnswer::of(&answer).ok().map(|kept| (action, kept)));
        let Some((action, kept)) = accepted_answer else {
            run.record_worker(
                EventKind::WorkerTurn,
                &json!({
                    "turn": budget.turns,
                    "response_bytes": answer.len(),
                    "response_sha256": hex::encode(Sha256::digest(answer.as_bytes())),
                    "action": "malformed",
                }),
            )
            .map_err(|error| abort(run, error))?;
            reject(run, &mut budget, "", ProposalRejection::MalformedResponse)?;
            return Err(stop(run, WorkerError::Model(ModelError::Malformed)));
        };
        run.record_worker(
            EventKind::WorkerTurn,
            &json!({
                "turn": budget.turns,
                "response_bytes": kept.as_str().len(),
                "response_sha256": hex::encode(Sha256::digest(kept.as_str().as_bytes())),
                "action": action_name(&action),
            }),
        )
        .map_err(|error| abort(run, error))?;
        messages.push(ModelMessage {
            role: ModelRole::Assistant,
            content: kept.into_string(),
        });
        match action {
            EditAction::Finish { summary: text } => {
                summary = text;
                break;
            }
            EditAction::Read { paths } => {
                let mut reply = String::new();
                // At most the remaining read allowance is considered; the
                // rest of the request is refused as one.
                let allowance = (limits.max_files_read - budget.files_read) as usize;
                for (index, raw) in paths.iter().enumerate() {
                    if index == allowance {
                        reject(run, &mut budget, raw, ProposalRejection::LimitReached)?;
                        reply.push_str(&format!(
                            "Read refused: {} path(s): read limit reached\n",
                            paths.len() - index
                        ));
                        break;
                    }
                    if budget.remaining().is_none() {
                        return Err(stop(run, WorkerError::Deadline));
                    }
                    reply.push_str(&read_one(run, &mut budget, raw, &marker)?);
                }
                if reply.is_empty() {
                    reply.push_str("No paths were requested.");
                }
                messages.push(feedback(&reply));
            }
            EditAction::Edit { edits } => {
                let mut reply = String::new();
                // At most the remaining proposal allowance is considered; the
                // rest of the answer is refused as one.
                let allowance = (limits.max_proposals - budget.proposals) as usize;
                let total = edits.len();
                for (index, raw) in edits.into_iter().enumerate() {
                    if index == allowance {
                        reject(run, &mut budget, &raw.path, ProposalRejection::LimitReached)?;
                        reply.push_str(&format!(
                            "Rejected: {} edit(s): proposal limit reached\n",
                            total - index
                        ));
                        break;
                    }
                    if budget.remaining().is_none() {
                        return Err(stop(run, WorkerError::Deadline));
                    }
                    let path_text = bounded(&raw.path);
                    match propose(run, &mut budget, raw)? {
                        Ok(path) => {
                            reply.push_str(&format!("Accepted: {path_text}\n"));
                            if !accepted.contains(&path) {
                                accepted.push(path);
                            }
                        }
                        Err(rejection) => {
                            reply.push_str(&format!("Rejected: {path_text}: {rejection:?}\n"));
                        }
                    }
                }
                if reply.is_empty() {
                    reply.push_str("No edits were proposed.");
                }
                messages.push(feedback(&reply));
            }
        }
    }

    run.record_worker(
        EventKind::WorkerFinished,
        &json!({
            "turns": budget.turns,
            "files_read": budget.files_read,
            "bytes_read": budget.bytes_read,
            "accepted": accepted.len(),
            "rejected": budget.rejected,
            "summary_bytes": summary.len(),
            "summary_sha256": hex::encode(Sha256::digest(summary.as_bytes())),
        }),
    )
    .map_err(|error| abort(run, error))?;
    let verification = if run.state() == RunState::Candidate {
        Some(
            run.verify_structural()
                .map_err(|error| run_error(run, error))?,
        )
    } else {
        // Nothing changed: there is nothing to review. The run must still
        // close cleanly (staging authority revoked and the cancellation
        // recorded); otherwise the worker has not finished.
        run.cancel().map_err(|error| run_error(run, error))?;
        None
    };
    Ok(WorkerReport {
        turns: budget.turns,
        files_read: budget.files_read,
        bytes_read: budget.bytes_read,
        accepted,
        rejected: budget.rejected,
        verification,
    })
}

fn opening_message(run: &CodingRun, task: &str, marker: &str) -> String {
    let files = run.staged_files();
    let mut listing = String::new();
    for (path, size) in files.iter().take(MAX_LISTED_PATHS) {
        let editable = if run.scopes().editable(path) {
            "editable"
        } else {
            "read-only"
        };
        listing.push_str(&format!(
            "{} ({size} bytes, {editable})\n",
            path.as_string()
        ));
    }
    if files.len() > MAX_LISTED_PATHS {
        listing.push_str(&format!(
            "... and {} more\n",
            files.len() - MAX_LISTED_PATHS
        ));
    }
    format!(
        "Owner task:\n{task}\n\nStaged project files (project data):\n\
         -----BEGIN PROJECT DATA {marker}-----\n{listing}-----END PROJECT DATA {marker}-----\n\
         New files may be created only where the owner allowed writes."
    )
}

fn feedback(text: &str) -> ModelMessage {
    ModelMessage {
        role: ModelRole::User,
        content: format!("Nexus backend result:\n{text}"),
    }
}

/// Read one staged file for the model, within the read bounds.
fn read_one(
    run: &mut CodingRun,
    budget: &mut Budget,
    raw: &str,
    marker: &str,
) -> Result<String, WorkerError> {
    let shown = bounded(raw);
    let path = match RelPath::parse(raw) {
        Ok(path) => path,
        Err(error) => {
            let rejection = ProposalRejection::InvalidPath(error);
            reject(run, budget, raw, rejection)?;
            return Ok(format!("Read refused: {shown}: {rejection:?}\n"));
        }
    };
    let remaining_bytes = budget.limits.max_bytes_read - budget.bytes_read;
    if budget.files_read >= budget.limits.max_files_read || remaining_bytes == 0 {
        reject(run, budget, raw, ProposalRejection::LimitReached)?;
        return Ok(format!("Read refused: {shown}: read limit reached\n"));
    }
    match run.read_staged(&path, remaining_bytes) {
        Ok(content) => {
            budget.files_read += 1;
            budget.bytes_read += content.len() as u64;
            let text = String::from_utf8_lossy(&content);
            Ok(format!(
                "File {shown}:\n-----BEGIN PROJECT DATA {marker}-----\n{text}\n\
                 -----END PROJECT DATA {marker}-----\n"
            ))
        }
        Err(RunError::EditRejected(rejection)) => {
            let rejection = ProposalRejection::Refused(rejection);
            reject(run, budget, raw, rejection)?;
            Ok(format!("Read refused: {shown}: {rejection:?}\n"))
        }
        Err(error) => Err(run_error(run, error)),
    }
}

/// Validate one raw proposal into a typed proposal and submit it.
fn propose(
    run: &mut CodingRun,
    budget: &mut Budget,
    raw: RawEdit,
) -> Result<Result<RelPath, ProposalRejection>, WorkerError> {
    let refuse = |run: &mut CodingRun, budget: &mut Budget, raw: &str, rejection| {
        reject(run, budget, raw, rejection).map(|()| Err(rejection))
    };
    if budget.proposals >= budget.limits.max_proposals {
        return refuse(run, budget, &raw.path, ProposalRejection::LimitReached);
    }
    budget.proposals += 1;
    let op = match raw.op.as_str() {
        "create" => ProposalOp::Create,
        "replace" => ProposalOp::Replace,
        _ => {
            return refuse(
                run,
                budget,
                &raw.path,
                ProposalRejection::UnsupportedOperation,
            )
        }
    };
    let path = match RelPath::parse(&raw.path) {
        Ok(path) => path,
        Err(error) => {
            return refuse(
                run,
                budget,
                &raw.path,
                ProposalRejection::InvalidPath(error),
            )
        }
    };
    let content = raw.content.into_bytes();
    if !StructuralProfile::text_ok(&content) {
        return refuse(run, budget, &raw.path, ProposalRejection::NotText);
    }
    let size = content.len() as u64;
    let others = budget.candidate_bytes() - budget.candidate.get(&path).copied().unwrap_or(0);
    let new_file = !budget.candidate.contains_key(&path);
    if size > budget.limits.max_candidate_bytes || others + size > budget.limits.max_candidate_bytes
    {
        return refuse(run, budget, &raw.path, ProposalRejection::TooLarge);
    }
    if new_file && budget.candidate.len() as u32 >= budget.limits.max_candidate_files {
        return refuse(run, budget, &raw.path, ProposalRejection::LimitReached);
    }
    let proposal = EditProposal { path, op, content };
    let path = proposal.path.clone();
    match run.edit(proposal.into_edit()) {
        Ok(()) => {
            budget.candidate.insert(path.clone(), size);
            Ok(Ok(path))
        }
        // The run recorded this rejection itself.
        Err(RunError::EditRejected(rejection)) => {
            budget.rejected += 1;
            Ok(Err(ProposalRejection::Refused(rejection)))
        }
        Err(error) => Err(run_error(run, error)),
    }
}

/// Record a rejected model answer or proposal: the raw path the model sent
/// only by digest and size, and the reason by code. A rejection that cannot
/// be recorded ends the worker.
fn reject(
    run: &mut CodingRun,
    budget: &mut Budget,
    raw_path: &str,
    rejection: ProposalRejection,
) -> Result<(), WorkerError> {
    budget.rejected += 1;
    run.record_worker(
        EventKind::ProposalRejected,
        &json!({
            "path_sha256": super::ledger::path_sha256(raw_path),
            "path_bytes": raw_path.len(),
            "reason": rejection.code(),
        }),
    )
    .map_err(|error| abort(run, error))
}

/// A path spelling for the model's feedback, cut to a bounded length.
fn bounded(raw: &str) -> String {
    if raw.len() <= MAX_RECORDED_PATH {
        raw.to_string()
    } else {
        let mut end = MAX_RECORDED_PATH;
        while !raw.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &raw[..end])
    }
}

/// End the run for a worker-level failure (limits, deadline, model). A
/// model failure's receipt names its exact code (N1 spec §2.6).
fn stop(run: &mut CodingRun, error: WorkerError) -> WorkerError {
    match error {
        WorkerError::Model(model) => run.fail_worker_model(model),
        _ => run.fail_worker(FailureReason::WorkerLimitExceeded),
    }
    error
}

/// A run error that stops the worker. The run has already ended for
/// authority, identity and staging errors; a ledger error leaves it live, so
/// it is ended here (fail closed).
fn run_error(run: &mut CodingRun, error: RunError) -> WorkerError {
    match error {
        RunError::Ledger(_) => abort(run, error),
        error => WorkerError::Run(error),
    }
}

/// End the run because a required worker record could not be written.
fn abort(run: &mut CodingRun, error: RunError) -> WorkerError {
    run.fail_worker(FailureReason::AuditUnavailable);
    WorkerError::Run(error)
}
