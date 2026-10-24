//! Native owner confirmation and the R2 approval it produces.
//!
//! The desktop's native dialogs are the only production `ControlConfirmer`.
//! An `R2Approval` exists only after the owner confirmed one exact
//! commitment: it cannot be constructed, cloned or deserialized outside this
//! crate, it names the commitment and its binding digest, and authorizing
//! the commitment consumes it.

use super::effect::{CapabilityKind, EffectClass};
use super::evidence::DIALOG_COLUMNS;
use super::ids::{CommitmentId, Digest};

/// The lines of a confirmation's details, each at most `DIALOG_COLUMNS`
/// characters: a longer one continues on `↳ ` lines. The lines are already
/// plain; the confirmation window shows each one whole and never wraps.
fn fitted_lines(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let first = chars.len().min(DIALOG_COLUMNS);
        out.push(chars[..first].iter().collect());
        for chunk in chars[first..].chunks(DIALOG_COLUMNS - 2) {
            out.push(format!("↳ {}", chunk.iter().collect::<String>()));
        }
    }
    out
}

/// What the owner's confirmation window shows, in two parts it keeps
/// apart. The header is the security identity of what is asked: rows of a
/// label the window draws itself and the backend's value, shown whole and
/// never scrolled, in view whenever the answer can be given. The details
/// are what the request carries (bodies, typed text, steps), each line
/// plain and at most `DIALOG_COLUMNS` characters (a longer one continues on
/// `↳ ` lines), shown in a scrolled region of their own that the window
/// marks as the request's content: nothing in them shares the header's
/// place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmationText {
    pub header: Vec<(&'static str, String)>,
    pub details: Vec<String>,
}

impl ConfirmationText {
    /// The same as plain text (logs and tests): the header rows, then the
    /// details.
    pub fn plain(&self) -> String {
        let mut lines: Vec<String> = self
            .header
            .iter()
            .map(|(label, value)| format!("{label}: {value}"))
            .collect();
        lines.extend(self.details.iter().cloned());
        lines.join("\n")
    }
}

/// The Effect row of the owner's synthetic approval probe (P4-UAT05): it
/// takes the R2 approval path, has no external effect, and says so. Only the
/// exact pair `(CapabilityKind::Synthetic, EffectClass::R2)` reads so.
pub const SYNTHETIC_EFFECT: &str = "Synthetic R2-approval test: no external effect";

/// Who may use a grant. Only the synthetic probe's is narrower: it refuses
/// every run the backend did not open as an owner-session command's.
pub const ANY_AGENT_USE: &str = "any agent, until it expires or you revoke it";
pub const OWNER_ONLY_USE: &str =
    "Owner-session commands only; agent runs are refused; every use requires separate native approval.";

/// The Effect row: the class and what it means, except for the synthetic
/// probe. A synthetic request of any class but R2 is refused before it is
/// committed (`synthetic::admit`), so no window shows one; were one ever
/// shown, it would not read as harmless. Exhaustive on purpose: a new kind
/// or class must choose its text.
fn effect_row(class: EffectClass, kind: CapabilityKind) -> String {
    match (kind, class) {
        (CapabilityKind::Synthetic, EffectClass::R2) => SYNTHETIC_EFFECT.to_string(),
        (CapabilityKind::Synthetic, EffectClass::R0 | EffectClass::R1) => format!(
            "{} ({}); not a valid synthetic test: refuse it",
            class,
            class.meaning()
        ),
        (
            CapabilityKind::Tool
            | CapabilityKind::Egress
            | CapabilityKind::Browser
            | CapabilityKind::Perception
            | CapabilityKind::Input
            | CapabilityKind::Connector,
            EffectClass::R0 | EffectClass::R1 | EffectClass::R2,
        ) => format!("{} ({})", class, class.meaning()),
    }
}

/// Who may use a grant of `kind`. Exhaustive on purpose.
fn who_may_use(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Synthetic => OWNER_ONLY_USE,
        CapabilityKind::Tool
        | CapabilityKind::Egress
        | CapabilityKind::Browser
        | CapabilityKind::Perception
        | CapabilityKind::Input
        | CapabilityKind::Connector => ANY_AGENT_USE,
    }
}

/// What the owner is asked to approve: one R2 commitment. Every line is
/// computed by the backend from the commitment it approves; nothing here is
/// a secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionConfirmation {
    pub commitment: String,
    pub class: EffectClass,
    pub kind: CapabilityKind,
    pub operation: String,
    pub target: String,
    pub agent: String,
    pub run: String,
    pub summary: Vec<String>,
    pub expires_in_secs: u64,
    /// The first digits of the binding digest, so that two look-alike
    /// requests are visibly different.
    pub binding_short: String,
}

impl ActionConfirmation {
    pub fn title(&self) -> &'static str {
        "Allow this action?"
    }

    /// The text of the native confirmation: the effect class, operation,
    /// canonical target, acting agent, run, commitment and binding in the
    /// header; the action's summary in the details.
    pub fn text(&self) -> ConfirmationText {
        ConfirmationText {
            header: vec![
                ("Effect", effect_row(self.class, self.kind)),
                ("Operation", self.operation.clone()),
                ("Target", self.target.clone()),
                ("Acting for", self.agent.clone()),
                ("Run", self.run.clone()),
                ("Commitment", self.commitment.clone()),
                ("Binding", self.binding_short.clone()),
                ("Expires in", format!("{} s", self.expires_in_secs)),
            ],
            details: fitted_lines(&self.summary),
        }
    }

    /// The same as plain text.
    pub fn message(&self) -> String {
        self.text().plain()
    }
}

/// What the owner is asked to grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantConfirmation {
    pub kind: CapabilityKind,
    pub lines: Vec<String>,
    pub expires_in_secs: u64,
}

impl GrantConfirmation {
    pub fn title(&self) -> &'static str {
        "Grant this capability?"
    }

    /// The capability and who may use it in the header; its scope, line by
    /// line, in the details.
    pub fn text(&self) -> ConfirmationText {
        ConfirmationText {
            header: vec![
                ("Capability", self.kind.as_str().to_string()),
                ("Who may use it", who_may_use(self.kind).to_string()),
                ("Expires in", format!("{} s", self.expires_in_secs)),
            ],
            details: fitted_lines(&self.lines),
        }
    }

    pub fn message(&self) -> String {
        self.text().plain()
    }
}

/// The backend's native confirmation. `true` only if the owner confirmed.
/// Everything that raises authority asks through it: a grant, an R2
/// approval, and lifting an emergency stop; and the owner's acknowledgement
/// of actions whose outcome was not recorded (which grants nothing).
pub trait ControlConfirmer {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool;
    fn confirm_grant(&self, request: &GrantConfirmation) -> bool;
    fn confirm_resume(&self, request: &ResumeConfirmation) -> bool;
    fn confirm_acknowledgement(&self, request: &AcknowledgementConfirmation) -> bool;
}

/// What the owner is shown before acknowledging unsettled actions (EL-1
/// §7): built by the backend from the evidence store; it grants nothing.
/// `covers_through` is the highest statement shown: the acknowledgement
/// covers exactly what was shown, never a statement written after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcknowledgementConfirmation {
    /// Each unsettled commitment's short id.
    pub unsettled: Vec<String>,
    /// Each unclean session's short id.
    pub unclean_sessions: Vec<String>,
    pub covers_through: u64,
}

/// What an acknowledgement says it is, verbatim (EL-1 §7).
pub const ACKNOWLEDGEMENT_MEANING: [&str; 2] = [
    "These actions may or may not have happened.",
    "Acknowledging records that you were shown this; it grants nothing and changes no outcome.",
];

impl AcknowledgementConfirmation {
    pub fn title(&self) -> &'static str {
        "Acknowledge actions whose outcome was not recorded?"
    }

    /// The counts and what they cover in the header; each unsettled
    /// commitment and unclean session, then what acknowledging means, in
    /// the details.
    pub fn text(&self) -> ConfirmationText {
        let mut details: Vec<String> = self
            .unsettled
            .iter()
            .map(|id| format!("Unsettled action: {id}"))
            .collect();
        details.extend(
            self.unclean_sessions
                .iter()
                .map(|id| format!("Unclean session: {id}")),
        );
        details.extend(ACKNOWLEDGEMENT_MEANING.iter().map(|line| line.to_string()));
        ConfirmationText {
            header: vec![
                (
                    "Unsettled actions",
                    format!(
                        "{} (outcome not recorded; the effect may have occurred)",
                        self.unsettled.len()
                    ),
                ),
                ("Unclean sessions", self.unclean_sessions.len().to_string()),
                ("Covers records", format!("through {}", self.covers_through)),
            ],
            details: fitted_lines(&details),
        }
    }

    pub fn message(&self) -> String {
        self.text().plain()
    }
}

/// What the owner is asked before an emergency stop is lifted: which stop,
/// by its number in this run of the desktop, and the runs cancelled by
/// emergency stops since the last lift. A resume lifts only that stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumeConfirmation {
    pub stop_number: u64,
    pub runs_cancelled_by_the_stop: usize,
}

impl ResumeConfirmation {
    pub fn title(&self) -> &'static str {
        "Resume governed control?"
    }

    pub fn text(&self) -> ConfirmationText {
        ConfirmationText {
            header: vec![
                ("Lifts", format!("emergency stop {}", self.stop_number)),
                (
                    "Runs it cancelled",
                    format!("{} (they stay cancelled)", self.runs_cancelled_by_the_stop),
                ),
            ],
            details: fitted_lines(&[
                "Resuming lets agents open new runs and request actions again;".to_string(),
                "each still needs its grants and, for sensitive actions, your approval."
                    .to_string(),
            ]),
        }
    }

    pub fn message(&self) -> String {
        self.text().plain()
    }
}

/// The owner's native approval of exactly one R2 commitment.
///
/// It has no `Clone`, `Copy`, `Default` or deserializer, and its fields are
/// private; `CommitmentRegistry::request_approval` is its only constructor,
/// and `CommitmentRegistry::authorize` consumes it.
#[derive(Debug)]
pub struct R2Approval {
    commitment: CommitmentId,
    binding: Digest,
}

impl R2Approval {
    /// Only after the owner confirmed the commitment natively.
    pub(crate) fn confirmed(commitment: CommitmentId, binding: Digest) -> Self {
        Self {
            commitment,
            binding,
        }
    }

    pub fn commitment(&self) -> CommitmentId {
        self.commitment
    }

    pub fn binding(&self) -> &Digest {
        &self.binding
    }
}
