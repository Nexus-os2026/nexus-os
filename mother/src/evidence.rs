//! P0 (privacy closure): what Mother's records and errors may carry.
//!
//! Records keep only evidence: ids, digests, sizes, counts, kinds, roles,
//! states, times, and the closed codes here (design §3 I1). Each code
//! serializes as `name` or `name:param` and is parsed only from its closed
//! set, so a journal that carries anything else is refused at replay, and
//! an untrusted string (a lane's reason, a port's class, a question id) can
//! never become one. [`check`] refuses an event whose digest or id field is
//! not one, before it is journaled and when it is replayed.
//!
//! The Owner reads the detail behind a code through [`WorkingDetail`]: held
//! in memory, never persisted, and with no `Debug`, `Display`, `Clone` or
//! `Serialize`, it is shown only through an [`OwnerView`] the host
//! implements.
//!
//! Non-claim: plain SHA-256 digests of low-entropy values (a short lane
//! name, a small request) can be guessed by trying candidates (design
//! O-P0-2). The digests are not keyed.

use crate::contract::{sha256, EffectKind};
use crate::journal::{Event, ReplayViolation};
use crate::lane::{LaneIdentity, LaneRole};
use crate::private::MAX_CONTENT_BYTES;
use crate::state::Trigger;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Serialized through its code, parsed only from its closed set.
macro_rules! by_code {
    ($name:ident) => {
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                Self::parse(&text).ok_or_else(|| de::Error::custom("not a code of its registry"))
            }
        }
    };
}

/// A closed set of codes with no parameter.
macro_rules! codes {
    ($(#[$doc:meta])* $name:ident { $($(#[$vdoc:meta])* $variant:ident => $code:literal,)* }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$vdoc])* $variant,)*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            pub fn as_str(&self) -> String {
                self.code().to_string()
            }

            pub fn code(&self) -> &'static str {
                match self {
                    $($name::$variant => $code,)*
                }
            }

            pub fn parse(text: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|value| value.code() == text)
            }

            /// Every value (the registry).
            pub fn every() -> Vec<Self> {
                Self::ALL.to_vec()
            }
        }

        by_code!($name);
    };
}

fn role(text: &str) -> Option<LaneRole> {
    LaneRole::ALL.iter().copied().find(|r| r.as_str() == text)
}

fn effect_kind(text: &str) -> Option<EffectKind> {
    EffectKind::ALL.iter().copied().find(|k| k.as_str() == text)
}

/// A broker or port class (design I1: every `AuthorityError::class()`, the
/// port's own classes and a lapsed action's end state, plus `Unknown` for
/// anything else a port says).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BrokerClass {
    UnknownCommitment,
    UnknownRun,
    UnknownGrant,
    UnknownLease,
    WrongAgent,
    WrongRun,
    NotPending,
    NotAuthorized,
    Expired,
    Stale,
    GrantNotLive,
    NoCoveringGrant,
    RunCancelled,
    RunNotActive,
    EmergencyStopped,
    ResumeSuperseded,
    ApprovalRequired,
    ApprovalNotApplicable,
    ApprovalMismatch,
    Declined,
    TargetChanged,
    ParametersChanged,
    EvidenceUnavailable,
    Capacity,
    NotMember,
    Halted,
    Closed,
    InvalidAction,
    Unavailable,
    /// The port's own: fenced after the coordinator broke.
    PortFenced,
    /// The port's own: the broker has no route for this effect.
    NoBrokerRoute,
    /// The port's own: the mission ended at the port.
    MissionCancelled,
    /// The port's own: a journaled action the broker still holds open.
    ActionNotHeld,
    /// The port's own: the port does not know the action's outcome.
    ApprovalOutcomeUnknown,
    /// A lapsed action's end state at the broker.
    Failed,
    Cancelled,
    Revoked,
    /// M2d: the broker's Files classes.
    NotAdmitted,
    NoConsent,
    AlreadyAdmitted,
    FileExists,
    ChangeTooLargeToShow,
    UnsupportedMountTopology,
    RecoveryRequired,
    /// M2d, the port's own: a perform of another admission generation.
    AdmissionStale,
    /// S1a-D+B (merged from main): the broker's page and facts classes.
    FactsInvalid,
    FileTypeNotAllowed,
    PageNotInert,
    PageFactMissing,
    /// Anything outside this registry.
    Unknown,
}

impl BrokerClass {
    pub const ALL: &'static [BrokerClass] = &[
        BrokerClass::UnknownCommitment,
        BrokerClass::UnknownRun,
        BrokerClass::UnknownGrant,
        BrokerClass::UnknownLease,
        BrokerClass::WrongAgent,
        BrokerClass::WrongRun,
        BrokerClass::NotPending,
        BrokerClass::NotAuthorized,
        BrokerClass::Expired,
        BrokerClass::Stale,
        BrokerClass::GrantNotLive,
        BrokerClass::NoCoveringGrant,
        BrokerClass::RunCancelled,
        BrokerClass::RunNotActive,
        BrokerClass::EmergencyStopped,
        BrokerClass::ResumeSuperseded,
        BrokerClass::ApprovalRequired,
        BrokerClass::ApprovalNotApplicable,
        BrokerClass::ApprovalMismatch,
        BrokerClass::Declined,
        BrokerClass::TargetChanged,
        BrokerClass::ParametersChanged,
        BrokerClass::EvidenceUnavailable,
        BrokerClass::Capacity,
        BrokerClass::NotMember,
        BrokerClass::Halted,
        BrokerClass::Closed,
        BrokerClass::InvalidAction,
        BrokerClass::Unavailable,
        BrokerClass::PortFenced,
        BrokerClass::NoBrokerRoute,
        BrokerClass::MissionCancelled,
        BrokerClass::ActionNotHeld,
        BrokerClass::ApprovalOutcomeUnknown,
        BrokerClass::Failed,
        BrokerClass::Cancelled,
        BrokerClass::Revoked,
        BrokerClass::NotAdmitted,
        BrokerClass::NoConsent,
        BrokerClass::AlreadyAdmitted,
        BrokerClass::FileExists,
        BrokerClass::ChangeTooLargeToShow,
        BrokerClass::UnsupportedMountTopology,
        BrokerClass::RecoveryRequired,
        BrokerClass::AdmissionStale,
        BrokerClass::FactsInvalid,
        BrokerClass::FileTypeNotAllowed,
        BrokerClass::PageNotInert,
        BrokerClass::PageFactMissing,
        BrokerClass::Unknown,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            BrokerClass::UnknownCommitment => "unknown_commitment",
            BrokerClass::UnknownRun => "unknown_run",
            BrokerClass::UnknownGrant => "unknown_grant",
            BrokerClass::UnknownLease => "unknown_lease",
            BrokerClass::WrongAgent => "wrong_agent",
            BrokerClass::WrongRun => "wrong_run",
            BrokerClass::NotPending => "not_pending",
            BrokerClass::NotAuthorized => "not_authorized",
            BrokerClass::Expired => "expired",
            BrokerClass::Stale => "stale_policy_generation",
            BrokerClass::GrantNotLive => "grant_not_live",
            BrokerClass::NoCoveringGrant => "no_covering_grant",
            BrokerClass::RunCancelled => "run_cancelled",
            BrokerClass::RunNotActive => "run_not_active",
            BrokerClass::EmergencyStopped => "emergency_stopped",
            BrokerClass::ResumeSuperseded => "resume_superseded",
            BrokerClass::ApprovalRequired => "approval_required",
            BrokerClass::ApprovalNotApplicable => "approval_not_applicable",
            BrokerClass::ApprovalMismatch => "approval_mismatch",
            BrokerClass::Declined => "declined",
            BrokerClass::TargetChanged => "target_changed",
            BrokerClass::ParametersChanged => "parameters_changed",
            BrokerClass::EvidenceUnavailable => "evidence_unavailable",
            BrokerClass::Capacity => "capacity",
            BrokerClass::NotMember => "not_member",
            BrokerClass::Halted => "halted",
            BrokerClass::Closed => "closed",
            BrokerClass::InvalidAction => "invalid_action",
            BrokerClass::Unavailable => "unavailable",
            BrokerClass::PortFenced => "port_fenced",
            BrokerClass::NoBrokerRoute => "no_broker_route",
            BrokerClass::MissionCancelled => "mission_cancelled",
            BrokerClass::ActionNotHeld => "action_not_held",
            BrokerClass::ApprovalOutcomeUnknown => "approval_outcome_unknown",
            BrokerClass::Failed => "failed",
            BrokerClass::Cancelled => "cancelled",
            BrokerClass::Revoked => "revoked",
            BrokerClass::NotAdmitted => "not_admitted",
            BrokerClass::NoConsent => "no_consent",
            BrokerClass::AlreadyAdmitted => "already_admitted",
            BrokerClass::FileExists => "file_exists",
            BrokerClass::ChangeTooLargeToShow => "change_too_large_to_show",
            BrokerClass::UnsupportedMountTopology => "unsupported_mount_topology",
            BrokerClass::RecoveryRequired => "recovery_required",
            BrokerClass::AdmissionStale => "admission_stale",
            BrokerClass::FactsInvalid => "facts_invalid",
            BrokerClass::FileTypeNotAllowed => "file_type_not_allowed",
            BrokerClass::PageNotInert => "page_not_inert",
            BrokerClass::PageFactMissing => "page_fact_missing",
            BrokerClass::Unknown => "unknown",
        }
    }

    pub fn as_str(&self) -> String {
        self.code().to_string()
    }

    /// Only a code of this registry.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|class| class.code() == text)
    }

    /// What a port or the broker said, as a class: anything outside the
    /// registry is [`BrokerClass::Unknown`].
    pub fn of(class: &str) -> Self {
        Self::parse(class).unwrap_or(BrokerClass::Unknown)
    }

    pub fn every() -> Vec<Self> {
        Self::ALL.to_vec()
    }
}

by_code!(BrokerClass);

codes! {
    /// Why a mission was stopped.
    StopReason {
        Owner => "owner",
        EmergencyStop => "emergency_stop",
        /// P0: its private content is missing or does not verify.
        PrivateContentUnavailable => "private_content_unavailable",
    }
}

codes! {
    /// Why a mission is held.
    HeldReason {
        UncleanShutdown => "unclean_shutdown",
    }
}

codes! {
    /// M2d (§11 M4′): what ended a Files publication's authority after the
    /// broker's final check (the broker's registry; anything else is
    /// `unknown`).
    PublicationCause {
        EmergencyStop => "emergency_stop",
        Cancelled => "cancelled",
        Revoked => "revoked",
        Expired => "expired",
        CommitmentExpired => "commitment_expired",
        WriteGrantRevoked => "write_grant_revoked",
        WriteGrantExpired => "write_grant_expired",
        Unknown => "unknown",
    }
}

impl PublicationCause {
    /// The broker's code, as a cause: outside the registry, `Unknown`.
    pub fn of(code: &str) -> Self {
        Self::parse(code).unwrap_or(PublicationCause::Unknown)
    }
}

codes! {
    /// Why the Verifier failed a mission.
    VerifyFailReason {
        /// The Verifier judged a criterion does not hold (its reason is
        /// working detail).
        VerifierFailed => "verifier_fail",
        VerifierGaveNoVerdict => "verifier_gave_no_verdict",
    }
}

/// Mother's own refusal of a lane's effect, before any port call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MotherRefusal {
    ForbiddenEffect(EffectKind),
    BudgetMoneyZero,
    SpendNotSupported,
    NoRequestedFolder,
    FileRequestNotRelative,
    /// M2d: not one top-level name of the allowed characters.
    FileNameInvalid,
    /// M2d: the content holds a NUL.
    FileNotText,
    /// M2d: more than 8 KiB.
    FileTooLarge,
    /// M2d: more than 150 lines.
    FileTooManyLines,
    /// M2d: the requested folder is not the workspace the host gave.
    FileFolderNotWorkspace,
}

const FILE_REFUSALS: [(MotherRefusal, &str); 5] = [
    (MotherRefusal::FileNameInvalid, "file_name_invalid"),
    (MotherRefusal::FileNotText, "file_not_text"),
    (MotherRefusal::FileTooLarge, "file_too_large"),
    (MotherRefusal::FileTooManyLines, "file_too_many_lines"),
    (
        MotherRefusal::FileFolderNotWorkspace,
        "file_folder_not_workspace",
    ),
];

impl MotherRefusal {
    pub fn as_str(&self) -> String {
        match self {
            MotherRefusal::ForbiddenEffect(kind) => format!("forbidden_effect:{}", kind.as_str()),
            MotherRefusal::BudgetMoneyZero => "budget_money_zero".into(),
            MotherRefusal::SpendNotSupported => "spend_not_supported".into(),
            MotherRefusal::NoRequestedFolder => "no_requested_folder".into(),
            MotherRefusal::FileRequestNotRelative => "file_request_not_relative".into(),
            file => FILE_REFUSALS
                .iter()
                .find(|(refusal, _)| refusal == file)
                .map_or_else(String::new, |(_, code)| (*code).to_string()),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        if let Some(kind) = text.strip_prefix("forbidden_effect:") {
            return effect_kind(kind).map(MotherRefusal::ForbiddenEffect);
        }
        match text {
            "budget_money_zero" => Some(MotherRefusal::BudgetMoneyZero),
            "spend_not_supported" => Some(MotherRefusal::SpendNotSupported),
            "no_requested_folder" => Some(MotherRefusal::NoRequestedFolder),
            "file_request_not_relative" => Some(MotherRefusal::FileRequestNotRelative),
            _ => FILE_REFUSALS
                .iter()
                .find(|(_, code)| *code == text)
                .map(|(refusal, _)| *refusal),
        }
    }

    pub fn every() -> Vec<Self> {
        let mut every: Vec<Self> = EffectKind::ALL
            .iter()
            .map(|k| MotherRefusal::ForbiddenEffect(*k))
            .collect();
        every.extend([
            MotherRefusal::BudgetMoneyZero,
            MotherRefusal::SpendNotSupported,
            MotherRefusal::NoRequestedFolder,
            MotherRefusal::FileRequestNotRelative,
        ]);
        every.extend(FILE_REFUSALS.iter().map(|(refusal, _)| *refusal));
        every
    }
}

by_code!(MotherRefusal);

/// Why a mission was blocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlockReason {
    WaitUnknown,
    ApprovalNotClosed,
    WaitedRestUnreadable,
    BudgetTime,
    BudgetModelCalls,
    LaneIdentityChanged,
    TooManyEffects,
    JournalFailed,
    AwaitingOwnerApproval,
    EffectOutcomeUnknown,
    PhaseInterrupted,
    /// A lane answered Halt (its reason is working detail).
    LaneHalt(LaneRole),
    /// Mother refused a lane's effect.
    Refused(MotherRefusal),
    /// The port or the broker refused an effect.
    BrokerRefused(BrokerClass),
    /// M2d: no consent of this process binds the mission's Files (after a
    /// clean restart, or the door holds no such yes).
    FilesConsentUnverifiable,
    /// M2d (§11 M4′): a File was published, and its authority ended after
    /// the broker's final check.
    FilePublicationInterrupted(PublicationCause),
    /// M2d (Codex review 1 of PR #78, HIGH 1): a File the mission waited on
    /// ran, and Mother then ended the mission for its time budget, or for
    /// the Owner's Stop (an emergency Stop is the broker registry's own
    /// `emergency_stop`).
    FilePublicationTimeBudget,
    FilePublicationOwnerStop,
    /// M2d (HIGH 2): a File was published with recovery required.
    FilePublicationRecoveryRequired,
}

const BLOCK_CODES: [(BlockReason, &str); 15] = [
    (BlockReason::WaitUnknown, "wait_unknown"),
    (BlockReason::ApprovalNotClosed, "approval_not_closed"),
    (BlockReason::WaitedRestUnreadable, "waited_rest_unreadable"),
    (BlockReason::BudgetTime, "budget_time"),
    (BlockReason::BudgetModelCalls, "budget_model_calls"),
    (BlockReason::LaneIdentityChanged, "lane_identity_changed"),
    (BlockReason::TooManyEffects, "too_many_effects"),
    (BlockReason::JournalFailed, "journal_failed"),
    (
        BlockReason::AwaitingOwnerApproval,
        "awaiting_owner_approval",
    ),
    (BlockReason::EffectOutcomeUnknown, "effect_outcome_unknown"),
    (BlockReason::PhaseInterrupted, "phase_interrupted"),
    (
        BlockReason::FilesConsentUnverifiable,
        "files_consent_unverifiable",
    ),
    (
        BlockReason::FilePublicationTimeBudget,
        "file_publication_interrupted:time_budget",
    ),
    (
        BlockReason::FilePublicationOwnerStop,
        "file_publication_interrupted:owner_stop",
    ),
    (
        BlockReason::FilePublicationRecoveryRequired,
        "file_publication_recovery_required",
    ),
];

impl BlockReason {
    pub fn as_str(&self) -> String {
        match self {
            BlockReason::LaneHalt(role) => format!("lane_halt:{}", role.as_str()),
            BlockReason::Refused(refusal) => refusal.as_str(),
            BlockReason::BrokerRefused(class) => format!("broker_refused:{}", class.code()),
            BlockReason::FilePublicationInterrupted(cause) => {
                format!("file_publication_interrupted:{}", cause.code())
            }
            simple => BLOCK_CODES
                .iter()
                .find(|(reason, _)| reason == simple)
                .map_or_else(String::new, |(_, code)| (*code).to_string()),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        if let Some(found) = BLOCK_CODES.iter().find(|(_, code)| *code == text) {
            return Some(found.0);
        }
        if let Some(r) = text.strip_prefix("lane_halt:") {
            return role(r).map(BlockReason::LaneHalt);
        }
        if let Some(class) = text.strip_prefix("broker_refused:") {
            return BrokerClass::parse(class).map(BlockReason::BrokerRefused);
        }
        if let Some(cause) = text.strip_prefix("file_publication_interrupted:") {
            return PublicationCause::parse(cause).map(BlockReason::FilePublicationInterrupted);
        }
        MotherRefusal::parse(text).map(BlockReason::Refused)
    }

    pub fn every() -> Vec<Self> {
        let mut every: Vec<Self> = BLOCK_CODES.iter().map(|(reason, _)| *reason).collect();
        every.extend(LaneRole::ALL.iter().map(|r| BlockReason::LaneHalt(*r)));
        every.extend(MotherRefusal::every().into_iter().map(BlockReason::Refused));
        every.extend(
            BrokerClass::ALL
                .iter()
                .map(|c| BlockReason::BrokerRefused(*c)),
        );
        every.extend(
            PublicationCause::ALL
                .iter()
                .map(|c| BlockReason::FilePublicationInterrupted(*c)),
        );
        every
    }
}

by_code!(BlockReason);

/// Why a lane failed a mission (its reason is working detail).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailReason {
    LaneFail(LaneRole),
}

impl FailReason {
    pub fn as_str(&self) -> String {
        match self {
            FailReason::LaneFail(role) => format!("lane_fail:{}", role.as_str()),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        role(text.strip_prefix("lane_fail:")?).map(FailReason::LaneFail)
    }

    pub fn every() -> Vec<Self> {
        LaneRole::ALL
            .iter()
            .map(|r| FailReason::LaneFail(*r))
            .collect()
    }
}

by_code!(FailReason);

/// Why a waited action will not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LapseReason {
    ApprovalExpired,
    /// The broker ended it otherwise (its class).
    ApprovalLapsed(BrokerClass),
    /// The mission ended while it waited.
    Ended(BlockReason),
}

impl LapseReason {
    pub fn as_str(&self) -> String {
        match self {
            LapseReason::ApprovalExpired => "approval_expired".into(),
            LapseReason::ApprovalLapsed(class) => format!("approval_lapsed:{}", class.code()),
            LapseReason::Ended(reason) => reason.as_str(),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        if text == "approval_expired" {
            return Some(LapseReason::ApprovalExpired);
        }
        if let Some(class) = text.strip_prefix("approval_lapsed:") {
            return BrokerClass::parse(class).map(LapseReason::ApprovalLapsed);
        }
        BlockReason::parse(text).map(LapseReason::Ended)
    }

    pub fn every() -> Vec<Self> {
        let mut every = vec![LapseReason::ApprovalExpired];
        every.extend(
            BrokerClass::ALL
                .iter()
                .map(|c| LapseReason::ApprovalLapsed(*c)),
        );
        every.extend(BlockReason::every().into_iter().map(LapseReason::Ended));
        every
    }
}

by_code!(LapseReason);

/// Why a consent answer was not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsentRefusedReason {
    /// The mission changed while the Owner was asked.
    MissionChanged,
    /// The broker's answer was for another request.
    AnotherRequest,
    /// The broker refused to ask, or to record the answer.
    BrokerRefused(BrokerClass),
}

impl ConsentRefusedReason {
    pub fn as_str(&self) -> String {
        match self {
            ConsentRefusedReason::MissionChanged => "mission_changed".into(),
            ConsentRefusedReason::AnotherRequest => "another_request".into(),
            ConsentRefusedReason::BrokerRefused(class) => {
                format!("broker_refused:{}", class.code())
            }
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "mission_changed" => Some(ConsentRefusedReason::MissionChanged),
            "another_request" => Some(ConsentRefusedReason::AnotherRequest),
            _ => BrokerClass::parse(text.strip_prefix("broker_refused:")?)
                .map(ConsentRefusedReason::BrokerRefused),
        }
    }

    pub fn every() -> Vec<Self> {
        let mut every = vec![
            ConsentRefusedReason::MissionChanged,
            ConsentRefusedReason::AnotherRequest,
        ];
        every.extend(
            BrokerClass::ALL
                .iter()
                .map(|c| ConsentRefusedReason::BrokerRefused(*c)),
        );
        every
    }
}

by_code!(ConsentRefusedReason);

/// What the port said of a dispatched effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettledOutcome {
    Done,
    AwaitingApproval,
    Refused(BrokerClass),
}

impl SettledOutcome {
    pub fn as_str(&self) -> String {
        match self {
            SettledOutcome::Done => "done".into(),
            SettledOutcome::AwaitingApproval => "awaiting_approval".into(),
            SettledOutcome::Refused(class) => format!("refused:{}", class.code()),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "done" => Some(SettledOutcome::Done),
            "awaiting_approval" => Some(SettledOutcome::AwaitingApproval),
            _ => BrokerClass::parse(text.strip_prefix("refused:")?).map(SettledOutcome::Refused),
        }
    }

    pub fn every() -> Vec<Self> {
        let mut every = vec![SettledOutcome::Done, SettledOutcome::AwaitingApproval];
        every.extend(BrokerClass::ALL.iter().map(|c| SettledOutcome::Refused(*c)));
        every
    }
}

by_code!(SettledOutcome);

// ---------------------------------------------------------------- digests

/// The lowercase hex SHA-256 of `bytes` under `domain`.
pub fn digest(domain: &str, bytes: &[u8]) -> String {
    hex::encode(sha256(domain.as_bytes(), &[bytes]))
}

/// The digest a lane's identity (role, name, version) is journaled as.
pub fn lane_identity_digest(identity: &LaneIdentity) -> String {
    let json = serde_json::to_vec(identity).expect("a lane identity serialises");
    digest("nexus.mother.lane-identity.v1", &json)
}

fn is_digest(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_size(bytes: u64) -> bool {
    bytes <= MAX_CONTENT_BYTES
}

/// Whether `event` is evidence: every digest field a lowercase SHA-256 hex
/// digest, every broker id a broker id, every size within the private
/// store's bound. Every other field is a closed type. Refused events are
/// never journaled, and refused at replay.
pub fn check(event: &Event) -> Result<(), ReplayViolation> {
    let ok = match event {
        Event::Drafted { contract } => is_size(contract.bytes),
        Event::Transition { trigger } => match trigger {
            Trigger::Revise { contract } => is_size(contract.bytes),
            Trigger::AwaitApproval { rest, .. } => is_size(rest.bytes),
            Trigger::Submit { .. }
            | Trigger::Consent { .. }
            | Trigger::Deny { .. }
            | Trigger::Start
            | Trigger::PhaseDone
            | Trigger::VerifyPass
            | Trigger::VerifyFail { .. }
            | Trigger::Block { .. }
            | Trigger::Fail { .. }
            | Trigger::Stop { .. }
            | Trigger::ActionApproved { .. }
            | Trigger::ActionDenied { .. }
            | Trigger::ApprovalLapsed { .. } => true,
        },
        Event::LaneInvoked {
            identity_sha256,
            input_hash,
            ..
        } => is_digest(identity_sha256) && is_digest(input_hash),
        Event::LaneReturned { output_hash, .. } => is_digest(output_hash),
        Event::EffectDispatched { request_hash, .. } => is_digest(request_hash),
        Event::ApprovalPending { pending, .. } => {
            RunId::parse(&pending.run).is_some()
                && CommitmentId::parse(&pending.commitment).is_some()
        }
        Event::ConsentRefused { .. }
        | Event::LaneOvertaken { .. }
        | Event::EffectRefused { .. }
        | Event::EffectSettled { .. }
        | Event::ApprovalClosed { .. }
        | Event::SessionOpened
        | Event::SessionClosed
        | Event::Held { .. }
        | Event::Resumed => true,
    };
    if ok {
        Ok(())
    } else {
        Err(ReplayViolation::NotEvidence)
    }
}

// ---------------------------------------------------------------- detail

/// Where the Owner reads working detail: the host's own view (a terminal,
/// a window). Never a log, a record or a model.
pub trait OwnerView {
    fn line(&mut self, label: &str, value: &str);
}

/// The detail behind a code, for the Owner only (design I4): held in
/// memory by the coordinator, taken once, and shown only through an
/// [`OwnerView`]. It has no `Debug`, `Display`, `Clone` or `Serialize`.
pub struct WorkingDetail {
    lines: Vec<(&'static str, String)>,
}

impl WorkingDetail {
    pub(crate) fn new(label: &'static str, value: &str) -> Self {
        Self {
            lines: vec![(label, value.to_string())],
        }
    }

    pub(crate) fn and(mut self, label: &'static str, value: &str) -> Self {
        self.lines.push((label, value.to_string()));
        self
    }

    /// Show the detail to the Owner; it is consumed.
    pub fn show(self, owner: &mut dyn OwnerView) {
        for (label, value) in self.lines {
            owner.line(label, &value);
        }
    }
}
