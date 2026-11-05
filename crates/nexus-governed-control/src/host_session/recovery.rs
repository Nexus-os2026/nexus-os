//! Report-only bootstrap. Persistent names never become live capabilities.
use super::{receipt::History, SessionError};
use crate::evidence_store::EvidenceHealth;
use crate::files::{RecordedResource, RecoveryEntry};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFailure {
    KernelLedger,
    MotherHistory,
    PrimaryEvidence,
}
/// Report-only integration with Mother's canonical typed replay. Implementors
/// inspect these exact verified bytes; they receive no operational handles.
pub trait HistoryInspector: Send + Sync {
    fn inspect(&self, lines: &[String]) -> Result<MotherHistory, SourceFailure>;
}
#[derive(Clone, Debug)]
pub struct MotherHistory {
    pub newly_absent: bool,
    pub records: usize,
    pub terminal_missions: usize,
    pub unresolved_missions: usize,
    pub session_end_recorded: bool,
    pub missions: Vec<RecordedMission>,
}
/// Canonical report-only identifiers, never adopted as live handles.
#[derive(Clone, Debug)]
pub struct RecordedMission {
    pub mission: String,
    pub state: String,
    pub terminal: Option<(&'static str, u64, Option<u64>)>,
    pub actions: Vec<RecordedAction>,
}
#[derive(Clone, Debug)]
pub struct RecordedAction {
    pub effect: u64,
    pub run: String,
    pub commitment: String,
}
#[derive(Clone, Debug)]
pub struct StartupReport {
    pub kernel: Result<Vec<RecoveryEntry>, SourceFailure>,
    pub resources: Result<Vec<RecordedResource>, SourceFailure>,
    pub mother: Result<MotherHistory, SourceFailure>,
    pub primary: EvidenceHealth,
}
impl StartupReport {
    pub fn ready(&self) -> bool {
        !self.primary.latched
            && self.primary.owed == 0
            && !self.primary.recovery_required
            && self.primary.unsettled == 0
            && self.primary.unclean_sessions == 0
            && self.kernel.as_ref().is_ok_and(Vec::is_empty)
            && self
                .resources
                .as_ref()
                .is_ok_and(|rows| rows.iter().all(|r| r.discard_recorded))
            && self.mother.as_ref().is_ok_and(|m| {
                m.unresolved_missions == 0 && (m.records == 0 || m.session_end_recorded)
            })
    }
    pub(super) fn require_ready(&self) -> Result<(), SessionError> {
        if self.ready() {
            Ok(())
        } else {
            Err(SessionError::Restricted)
        }
    }
}
struct Mission {
    state: &'static str,
    waiting_phase: Option<&'static str>,
    effect: Option<u64>,
    open: Option<u64>,
    lane: bool,
    uncertain: bool,
}
impl Default for Mission {
    fn default() -> Self {
        Self {
            state: "draft",
            waiting_phase: None,
            effect: None,
            open: None,
            lane: false,
            uncertain: false,
        }
    }
}
impl Mission {
    fn terminal(&self) -> bool {
        matches!(
            self.state,
            "done" | "stopped" | "denied" | "failed" | "blocked"
        )
    }
    fn transition(&mut self, t: &serde_json::Value) -> Result<(), SourceFailure> {
        let kind = t["trigger"].as_str().ok_or(SourceFailure::MotherHistory)?;
        let working = matches!(
            self.state,
            "investigating" | "building" | "verifying" | "explaining" | "awaiting_approval"
        );
        let may_wait = matches!(self.state, "investigating" | "building" | "verifying");
        self.state = match (kind, self.state) {
            ("revise", "draft" | "awaiting_consent" | "approved") => "draft",
            ("submit", "draft") => "awaiting_consent",
            ("consent", "awaiting_consent") => "approved",
            ("deny", "awaiting_consent") => "denied",
            ("start", "approved") => "investigating",
            ("phase_done", "investigating") => "building",
            ("phase_done", "building") => "verifying",
            ("phase_done", "explaining") => "done",
            ("verify_pass", "verifying") => "explaining",
            ("verify_fail", "verifying") => "failed",
            ("block", "approved") => "blocked",
            ("block", _) if working => "blocked",
            ("fail", _) if working => "failed",
            ("stop", _) if !self.terminal() => "stopped",
            ("await_approval", _) if may_wait => {
                self.waiting_phase = Some(self.state);
                "awaiting_approval"
            }
            ("action_approved", "awaiting_approval")
                if t["phase"].as_str() == self.waiting_phase =>
            {
                self.waiting_phase
                    .take()
                    .ok_or(SourceFailure::MotherHistory)?
            }
            ("action_denied", "awaiting_approval") => "denied",
            ("approval_lapsed", "awaiting_approval") => "blocked",
            ("settled", _) if may_wait || self.state == "awaiting_approval" => {
                match t["outcome"]["outcome"].as_str() {
                    Some("uncertain") => self.uncertain = true,
                    Some("published" | "not_published") => {}
                    _ => return Err(SourceFailure::MotherHistory),
                }
                self.effect = None;
                if t["end"].get("block").is_some() {
                    "blocked"
                } else if t["end"].get("stop").is_some() {
                    "stopped"
                } else {
                    return Err(SourceFailure::MotherHistory);
                }
            }
            _ => return Err(SourceFailure::MotherHistory),
        };
        Ok(())
    }
}
pub(super) fn startup_history(history: &History) -> Result<MotherHistory, SourceFailure> {
    // A broker-only client cannot validate Mother missions by approximating
    // canonical replay. Without the adapter, only empty/session-only history
    // can establish readiness. No fallback grants authority for mission rows.
    if history.inspector.get().is_none() {
        let (_, lines) = history
            .report_lines()
            .map_err(|_| SourceFailure::MotherHistory)?;
        if lines.iter().any(|line| line.mission != 0) {
            return Err(SourceFailure::MotherHistory);
        }
    }
    self::history(history)
}
pub(super) fn history(history: &History) -> Result<MotherHistory, SourceFailure> {
    let (newly_absent, lines) = history
        .report_lines()
        .map_err(|_| SourceFailure::MotherHistory)?;
    if let Some(inspector) = history.inspector.get() {
        let raw: Vec<_> = lines.iter().map(|line| line.raw.clone()).collect();
        let mut report = inspector.inspect(&raw)?;
        let (absent_after, after) = history
            .report_lines()
            .map_err(|_| SourceFailure::MotherHistory)?;
        if absent_after != newly_absent
            || after.len() != lines.len()
            || after.iter().zip(&lines).any(|(a, b)| a.hash != b.hash)
        {
            return Err(SourceFailure::MotherHistory);
        }
        report.newly_absent = newly_absent;
        return Ok(report);
    }
    let mut missions = BTreeMap::<u64, Mission>::new();
    let mut session_end = false;
    let mut session_open = false;
    for line in &lines {
        let e = &line.event;
        let event = e["event"].as_str().ok_or(SourceFailure::MotherHistory)?;
        match event {
            "session_opened" | "session_closed" => {
                if line.mission != 0 {
                    return Err(SourceFailure::MotherHistory);
                }
                if event == "session_closed" && !session_open {
                    return Err(SourceFailure::MotherHistory);
                }
                session_open = event == "session_opened";
                session_end = event == "session_closed";
                continue;
            }
            "drafted" => {
                if line.mission == 0 || line.mission != missions.len() as u64 + 1 || session_end {
                    return Err(SourceFailure::MotherHistory);
                }
                missions.insert(line.mission, Mission::default());
                continue;
            }
            _ if session_end => return Err(SourceFailure::MotherHistory),
            _ => {}
        }
        let m = missions
            .get_mut(&line.mission)
            .ok_or(SourceFailure::MotherHistory)?;
        match event {
            "effect_dispatched" => {
                if m.terminal() || m.effect.is_some() {
                    return Err(SourceFailure::MotherHistory);
                }
                m.effect = Some(e["effect"].as_u64().ok_or(SourceFailure::MotherHistory)?);
            }
            "effect_settled" => {
                let effect = e["effect"].as_u64().ok_or(SourceFailure::MotherHistory)?;
                if m.effect != Some(effect) {
                    return Err(SourceFailure::MotherHistory);
                }
                m.effect = None;
            }
            "approval_pending" => {
                if m.open.is_some() {
                    return Err(SourceFailure::MotherHistory);
                }
                m.open = Some(e["effect"].as_u64().ok_or(SourceFailure::MotherHistory)?);
            }
            "approval_closed" => {
                if m.open != e["effect"].as_u64() || m.open.is_none() {
                    return Err(SourceFailure::MotherHistory);
                }
                m.open = None;
            }
            "lane_invoked" => {
                if m.lane {
                    return Err(SourceFailure::MotherHistory);
                }
                m.lane = true;
            }
            "lane_returned" | "lane_overtaken" => {
                if !m.lane {
                    return Err(SourceFailure::MotherHistory);
                }
                m.lane = false;
            }
            "transition" => m.transition(&e["trigger"])?,
            "file_mode_selected" | "consent_refused" | "effect_refused" | "settlement_started"
            | "held" | "resumed" => {}
            _ => return Err(SourceFailure::MotherHistory),
        }
    }
    Ok(MotherHistory {
        newly_absent,
        records: lines.len(),
        terminal_missions: missions.values().filter(|m| m.terminal()).count(),
        unresolved_missions: missions
            .values()
            .filter(|m| {
                !m.terminal() || m.effect.is_some() || m.open.is_some() || m.lane || m.uncertain
            })
            .count(),
        session_end_recorded: session_end,
        missions: Vec::new(),
    })
}
