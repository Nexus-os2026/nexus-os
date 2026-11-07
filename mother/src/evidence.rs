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
use crate::verifier::{CheckCode, CheckId};
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::inert_page::{FactCode, RefusalCode, POLICY_V1};
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
    /// S1a-D+B (merged from main): the broker's page and facts classes.
    FactsInvalid,
    FileTypeNotAllowed,
    PageNotInert,
    PageFactMissing,
    /// M2e: the broker's model classes: `model_not_registered`,
    /// `model_identity_mismatch` and `model_` with each transport code.
    ModelNotRegistered,
    ModelIdentityMismatch,
    ModelNotLocal,
    ModelInvalidAddress,
    ModelInvalidModelName,
    ModelCloudName,
    ModelCloudEnabled,
    ModelNotInstalled,
    ModelDigestMismatch,
    ModelRuntimeMismatch,
    ModelRemote,
    ModelConfigMismatch,
    ModelUnavailable,
    ModelTimeout,
    ModelRequestTooLarge,
    ModelResponseTooLarge,
    ModelMalformed,
    ModelTruncated,
    ModelToolCalls,
    ModelThinkingLeaked,
    ModelEmptyContent,
    ModelOverBudget,
    ModelStopped,
    ModelCancelled,
    ModelLate,
    ModelUnsafeDelimiter,
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
        BrokerClass::FactsInvalid,
        BrokerClass::FileTypeNotAllowed,
        BrokerClass::PageNotInert,
        BrokerClass::PageFactMissing,
        BrokerClass::ModelNotRegistered,
        BrokerClass::ModelIdentityMismatch,
        BrokerClass::ModelNotLocal,
        BrokerClass::ModelInvalidAddress,
        BrokerClass::ModelInvalidModelName,
        BrokerClass::ModelCloudName,
        BrokerClass::ModelCloudEnabled,
        BrokerClass::ModelNotInstalled,
        BrokerClass::ModelDigestMismatch,
        BrokerClass::ModelRuntimeMismatch,
        BrokerClass::ModelRemote,
        BrokerClass::ModelConfigMismatch,
        BrokerClass::ModelUnavailable,
        BrokerClass::ModelTimeout,
        BrokerClass::ModelRequestTooLarge,
        BrokerClass::ModelResponseTooLarge,
        BrokerClass::ModelMalformed,
        BrokerClass::ModelTruncated,
        BrokerClass::ModelToolCalls,
        BrokerClass::ModelThinkingLeaked,
        BrokerClass::ModelEmptyContent,
        BrokerClass::ModelOverBudget,
        BrokerClass::ModelStopped,
        BrokerClass::ModelCancelled,
        BrokerClass::ModelLate,
        BrokerClass::ModelUnsafeDelimiter,
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
            BrokerClass::FactsInvalid => "facts_invalid",
            BrokerClass::FileTypeNotAllowed => "file_type_not_allowed",
            BrokerClass::PageNotInert => "page_not_inert",
            BrokerClass::PageFactMissing => "page_fact_missing",
            BrokerClass::ModelNotRegistered => "model_not_registered",
            BrokerClass::ModelIdentityMismatch => "model_identity_mismatch",
            BrokerClass::ModelNotLocal => "model_not_local",
            BrokerClass::ModelInvalidAddress => "model_invalid_address",
            BrokerClass::ModelInvalidModelName => "model_invalid_model_name",
            BrokerClass::ModelCloudName => "model_cloud_name",
            BrokerClass::ModelCloudEnabled => "model_cloud_enabled",
            BrokerClass::ModelNotInstalled => "model_not_installed",
            BrokerClass::ModelDigestMismatch => "model_digest_mismatch",
            BrokerClass::ModelRuntimeMismatch => "model_runtime_mismatch",
            BrokerClass::ModelRemote => "model_remote",
            BrokerClass::ModelConfigMismatch => "model_config_mismatch",
            BrokerClass::ModelUnavailable => "model_unavailable",
            BrokerClass::ModelTimeout => "model_timeout",
            BrokerClass::ModelRequestTooLarge => "model_request_too_large",
            BrokerClass::ModelResponseTooLarge => "model_response_too_large",
            BrokerClass::ModelMalformed => "model_malformed",
            BrokerClass::ModelTruncated => "model_truncated",
            BrokerClass::ModelToolCalls => "model_tool_calls",
            BrokerClass::ModelThinkingLeaked => "model_thinking_leaked",
            BrokerClass::ModelEmptyContent => "model_empty_content",
            BrokerClass::ModelOverBudget => "model_over_budget",
            BrokerClass::ModelStopped => "model_stopped",
            BrokerClass::ModelCancelled => "model_cancelled",
            BrokerClass::ModelLate => "model_late",
            BrokerClass::ModelUnsafeDelimiter => "model_unsafe_delimiter",
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

/// Why a mission was stopped. A File end in a pre-consent or held state
/// uses the universally legal Stop, retaining the exact closed end reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StopReason {
    Owner,
    EmergencyStop,
    PrivateContentUnavailable,
    DoorHalted,
    Ended(BlockReason),
}

impl StopReason {
    /// The unparameterized reasons; `every` also enumerates Ended's registry.
    pub const ALL: &'static [Self] = &[
        Self::Owner,
        Self::EmergencyStop,
        Self::PrivateContentUnavailable,
        Self::DoorHalted,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::EmergencyStop => "emergency_stop",
            Self::PrivateContentUnavailable => "private_content_unavailable",
            Self::DoorHalted => "door_halted",
            Self::Ended(_) => "ended",
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::Ended(reason) => format!("ended:{reason}"),
            reason => reason.code().into(),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        if let Some(reason) = text.strip_prefix("ended:") {
            return BlockReason::parse(reason).map(Self::Ended);
        }
        Self::ALL.iter().copied().find(|value| value.code() == text)
    }

    pub fn every() -> Vec<Self> {
        let mut reasons = Self::ALL.to_vec();
        reasons.extend(BlockReason::every().into_iter().map(Self::Ended));
        reasons
    }
}

by_code!(StopReason);

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

/// Why the Verifier failed a mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerifyFailReason {
    /// The Verifier judged a criterion does not hold (its reason is
    /// working detail).
    VerifierFailed,
    VerifierGaveNoVerdict,
    /// M3-Dc1: a page check's closed code, never a lane's text.
    Check(VerifyCode),
}

/// M3-Dc1: a page verification's failure: a V0-V5 code, or a payload that
/// is not its declared identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerifyCode {
    Identity,
    Check(CheckCode),
}

impl VerifyCode {
    pub fn as_str(&self) -> String {
        match self {
            VerifyCode::Identity => "verification_identity".into(),
            VerifyCode::Check(code) => code.to_string(),
        }
    }
}

/// Every V0-V5 failure code, with its check: the closed registry.
pub fn check_codes() -> Vec<(CheckId, CheckCode)> {
    use CheckCode as C;
    use CheckId::{V0, V1, V2, V3, V4, V5};
    let hours = 0..POLICY_V1.facts_hours_max_lines as u8;
    let fact = |code| (V3, C::FactMissing(code));
    let mut v = vec![(V0, C::NoPublication), (V1, C::PublishedMismatch)];
    v.extend(RefusalCode::CODES.map(|code| (V2, C::PageNotInert(code))));
    v.push((V2, C::PolicyChanged));
    v.extend([fact(FactCode::NameNotHeading), fact(FactCode::NameNotTitle)]);
    v.extend(
        hours
            .clone()
            .map(|index| fact(FactCode::HoursLineMissing { index })),
    );
    v.extend([fact(FactCode::PhoneMissing), fact(FactCode::ForeignNumber)]);
    v.push((V4, C::NameNotVisible));
    v.extend(hours.map(|index| (V4, C::HoursLineNotVisible(index))));
    v.extend([(V4, C::PhoneNotVisible), (V4, C::ReaderUnsupported)]);
    v.push((V5, C::EmailNotSupported));
    v
}

/// Whether `id` may journal `code`: `pass`, `not_run` or `fail:<its code>`.
pub fn is_check_result(id: CheckId, code: &str) -> bool {
    matches!(code, "pass" | "not_run")
        || code.strip_prefix("fail:").is_some_and(|code| {
            check_codes()
                .iter()
                .any(|(of, c)| *of == id && c.to_string() == code)
        })
}

impl VerifyFailReason {
    pub fn as_str(&self) -> String {
        match self {
            VerifyFailReason::VerifierFailed => "verifier_fail".into(),
            VerifyFailReason::VerifierGaveNoVerdict => "verifier_gave_no_verdict".into(),
            VerifyFailReason::Check(code) => format!("check:{}", code.as_str()),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::every().into_iter().find(|r| r.as_str() == text)
    }

    /// Every value (the registry).
    pub fn every() -> Vec<Self> {
        let mut every = vec![
            VerifyFailReason::VerifierFailed,
            VerifyFailReason::VerifierGaveNoVerdict,
            VerifyFailReason::Check(VerifyCode::Identity),
        ];
        every.extend(
            check_codes()
                .into_iter()
                .map(|(_, code)| VerifyFailReason::Check(VerifyCode::Check(code))),
        );
        every
    }
}

by_code!(VerifyFailReason);

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
    /// M2d (Codex review 2 of PR #78, decision C): a Stop or the time
    /// budget ended a mission waiting on a File the broker could not say
    /// anything of: a file may exist.
    FilePublicationUncertain,
    /// M2d-C round 4 (addendum 1): a File was published, and another
    /// mission's Stop then halted the door this mission shares.
    FilePublicationDoorHalted,
    /// M2d-C round 5 (D3): a File was published while Mother's own
    /// deadline for its approval had passed (read after the withdrawal).
    FilePublicationApprovalDeadline,
    /// M2e-C1: Mother's model codes (a contract pins no model or no
    /// deliverable; the consented model is not the port's; the broker
    /// refused the native inference admission).
    ModelNotPinned,
    ModelIdentityChanged,
    ModelAdmissionRefused,
    /// M3-Db (Db-R1 M3): a page's observed publication settled without the
    /// broker's receipt.
    ReceiptMissing,
}

const BLOCK_CODES: [(BlockReason, &str); 22] = [
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
    (
        BlockReason::FilePublicationUncertain,
        "file_publication_uncertain",
    ),
    (
        BlockReason::FilePublicationDoorHalted,
        "file_publication_interrupted:door_halted",
    ),
    (
        BlockReason::FilePublicationApprovalDeadline,
        "file_publication_interrupted:approval_deadline",
    ),
    (BlockReason::ModelNotPinned, "model_not_pinned"),
    (BlockReason::ModelIdentityChanged, "model_identity_changed"),
    (
        BlockReason::ModelAdmissionRefused,
        "model_admission_refused",
    ),
    (BlockReason::ReceiptMissing, "receipt_missing"),
];

/// M2d-C round 4 (addendum 5): what the Owner is told of a mission ended
/// `file_publication_uncertain`, exactly.
pub const FILE_UNCERTAIN_NOTICE: &str = "A file may exist in your folder. A write already handed off may still finish. Mother will not retry or continue this mission.";

/// M2d-C round 4 (settlement design v2, rule 4): why a File's settlement
/// began, kept as its first cause: a Stop, the time budget, or another end
/// of the mission (a block's reason).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettleCause {
    Stop(StopReason),
    TimeBudget,
    Ended(BlockReason),
}

impl SettleCause {
    pub fn as_str(&self) -> String {
        match self {
            SettleCause::Stop(reason) => format!("stop:{reason}"),
            SettleCause::TimeBudget => "time_budget".into(),
            SettleCause::Ended(reason) => format!("ended:{}", reason.as_str()),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        if text == "time_budget" {
            return Some(SettleCause::TimeBudget);
        }
        if let Some(reason) = text.strip_prefix("stop:") {
            return StopReason::parse(reason).map(SettleCause::Stop);
        }
        BlockReason::parse(text.strip_prefix("ended:")?).map(SettleCause::Ended)
    }

    pub fn every() -> Vec<Self> {
        let mut every: Vec<Self> = StopReason::every()
            .into_iter()
            .map(SettleCause::Stop)
            .collect();
        every.push(SettleCause::TimeBudget);
        every.extend(BlockReason::every().into_iter().map(SettleCause::Ended));
        every
    }

    /// How a settlement with this first cause ends, given its outcome (rule
    /// 5): a published File blocks with the cause's interruption code (or
    /// recovery required); one confirmed not published ends as the cause
    /// would have; an uncertain one blocks `file_publication_uncertain`.
    pub fn end(&self, outcome: &SettlementOutcome) -> SettlementEnd {
        match outcome {
            SettlementOutcome::Uncertain => {
                SettlementEnd::Block(BlockReason::FilePublicationUncertain)
            }
            SettlementOutcome::Published {
                recovery_required: true,
                ..
            } => SettlementEnd::Block(BlockReason::FilePublicationRecoveryRequired),
            SettlementOutcome::Published { .. } => SettlementEnd::Block(match self {
                SettleCause::Stop(StopReason::Owner) => BlockReason::FilePublicationOwnerStop,
                SettleCause::Stop(StopReason::EmergencyStop) => {
                    BlockReason::FilePublicationInterrupted(PublicationCause::EmergencyStop)
                }
                SettleCause::Stop(StopReason::DoorHalted) => BlockReason::FilePublicationDoorHalted,
                SettleCause::Stop(StopReason::PrivateContentUnavailable) => {
                    BlockReason::FilePublicationInterrupted(PublicationCause::Unknown)
                }
                SettleCause::TimeBudget
                | SettleCause::Stop(StopReason::Ended(BlockReason::BudgetTime)) => {
                    BlockReason::FilePublicationTimeBudget
                }
                SettleCause::Ended(reason) | SettleCause::Stop(StopReason::Ended(reason)) => {
                    *reason
                }
            }),
            SettlementOutcome::NotPublished => match self {
                SettleCause::Stop(reason) => SettlementEnd::Stop(*reason),
                SettleCause::TimeBudget => SettlementEnd::Block(BlockReason::BudgetTime),
                SettleCause::Ended(reason) => SettlementEnd::Block(*reason),
            },
        }
    }
}

by_code!(SettleCause);

/// M2d-C round 4 (rule 5): what a File's settlement found, journaled in the
/// same line as the mission's end. `Published` is the truthful success
/// record (it never makes the mission runnable).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettlementOutcome {
    Published {
        recovery_required: bool,
        interrupted: Vec<PublicationCause>,
    },
    NotPublished,
    Uncertain,
}

impl SettlementOutcome {
    /// Published, or confirmed not published: the broker may drop it.
    pub fn conclusive(&self) -> bool {
        !matches!(self, SettlementOutcome::Uncertain)
    }

    /// On the ordinary path (no Stop: the broker's own answer), how a File
    /// that may not simply continue ends: recovery required, its first
    /// interruption, or uncertain. `None`: a clean publication (the
    /// ordinary approval) or one not published (the ordinary denial or
    /// lapse) take no settlement.
    pub fn ordinary_end(&self) -> Option<SettlementEnd> {
        match self {
            SettlementOutcome::Uncertain => {
                Some(SettlementEnd::Block(BlockReason::FilePublicationUncertain))
            }
            SettlementOutcome::Published {
                recovery_required: true,
                ..
            } => Some(SettlementEnd::Block(
                BlockReason::FilePublicationRecoveryRequired,
            )),
            SettlementOutcome::Published { interrupted, .. } => interrupted
                .first()
                .map(|cause| SettlementEnd::Block(BlockReason::FilePublicationInterrupted(*cause))),
            SettlementOutcome::NotPublished => None,
        }
    }

    /// D3: how a File read after a denial, expiry or lapse ends when it is
    /// not confirmed not published: as on the ordinary path, a clean
    /// publication `approval_deadline`.
    pub fn deadline_end(&self) -> Option<SettlementEnd> {
        match self {
            SettlementOutcome::Published {
                recovery_required: false,
                interrupted,
            } if interrupted.is_empty() => Some(SettlementEnd::Block(
                BlockReason::FilePublicationApprovalDeadline,
            )),
            other => other.ordinary_end(),
        }
    }
}

/// M2d-C round 4: the mission's end in a settlement's terminal line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SettlementEnd {
    Block(BlockReason),
    Stop(StopReason),
}

impl SettlementEnd {
    pub fn as_str(&self) -> String {
        match self {
            SettlementEnd::Block(reason) => reason.as_str(),
            SettlementEnd::Stop(reason) => reason.as_str(),
        }
    }
}

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

/// Why a lane failed a mission (its reason is working detail), or why an
/// M3 page's publication proof could not be recorded (M3-Db, terminal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailReason {
    LaneFail(LaneRole),
    ReceiptMissing,
    ReceiptPolicyMissing,
    DeliverableInvalid,
}

const PROOF_FAILS: [(FailReason, &str); 3] = [
    (FailReason::ReceiptMissing, "receipt_missing"),
    (FailReason::ReceiptPolicyMissing, "receipt_policy_missing"),
    (FailReason::DeliverableInvalid, "deliverable_invalid"),
];

impl FailReason {
    pub fn as_str(&self) -> String {
        match self {
            FailReason::LaneFail(role) => format!("lane_fail:{}", role.as_str()),
            proof => PROOF_FAILS
                .iter()
                .find(|(reason, _)| reason == proof)
                .map_or_else(String::new, |(_, code)| (*code).to_string()),
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        if let Some((reason, _)) = PROOF_FAILS.iter().find(|(_, code)| *code == text) {
            return Some(*reason);
        }
        role(text.strip_prefix("lane_fail:")?).map(FailReason::LaneFail)
    }

    pub fn every() -> Vec<Self> {
        let mut every: Vec<Self> = LaneRole::ALL
            .iter()
            .map(|r| FailReason::LaneFail(*r))
            .collect();
        every.extend(PROOF_FAILS.iter().map(|(reason, _)| *reason));
        every
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
            Trigger::Settled { action, .. } => action.as_ref().is_none_or(|pending| {
                RunId::parse(&pending.run).is_some()
                    && CommitmentId::parse(&pending.commitment).is_some()
            }),
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
        Event::ModelCallCharged { .. }
        | Event::ConsentRefused { .. }
        | Event::LaneOvertaken { .. }
        | Event::EffectRefused { .. }
        | Event::EffectSettled { .. }
        | Event::ApprovalClosed { .. }
        | Event::SessionOpened
        | Event::SessionClosed
        | Event::Held { .. }
        | Event::Resumed
        | Event::SettlementStarted { .. }
        | Event::FileModeSelected => true,
        Event::DeliverableRetained { sha256, bytes, .. } => is_digest(sha256) && is_size(*bytes),
        Event::PublicationReceipt {
            action,
            sha256,
            bytes,
            policy_hash,
            ..
        } => {
            is_digest(sha256)
                && policy_hash.as_deref().is_none_or(is_digest)
                && is_size(*bytes)
                && RunId::parse(&action.run).is_some()
                && CommitmentId::parse(&action.commitment).is_some()
        }
        Event::VerificationRecorded {
            report_digest,
            checks,
            ..
        } => {
            checks.len() == CheckId::ALL.len()
                && CheckId::ALL
                    .iter()
                    .zip(checks)
                    .all(|(id, (name, code))| name == id.as_str() && is_check_result(*id, code))
                && *report_digest == crate::page_verifier::report_digest(checks)
        }
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
