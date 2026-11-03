//! The code-owned question registry (design §1, §6).

use crate::ident::{Label, QuestionId, SetId};

/// A probability in parts per million, `0..=1_000_000`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ppm(pub(crate) u32);

impl Ppm {
    pub const MAX: u32 = 1_000_000;

    pub(crate) const fn new(value: u32) -> Self {
        assert!(value <= Self::MAX, "ppm over one million");
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Risk: Yes means the bad thing is present. Positive: Yes means the
/// required property holds (§2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polarity {
    Risk,
    Positive,
}

/// What selecting a Choice option means for its lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionDisposition {
    Clear,
    NeedsOwner,
    Restrict,
}

/// A question's kind. Noul and Score carry their polarity; a Choice
/// registers a disposition per option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Noul {
        polarity: Polarity,
    },
    Choice {
        options: &'static [(Label, OptionDisposition)],
    },
    Score {
        levels: &'static [Label],
        at_least: u8,
        polarity: Polarity,
    },
}

/// `0 <= no_below < yes_above <= 1_000_000` (§7, `td-read-v1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadPolicy {
    pub(crate) no_below: Ppm,
    pub(crate) yes_above: Ppm,
}

impl ReadPolicy {
    /// Whether the thresholds form a valid policy.
    pub const fn is_valid(no_below: u32, yes_above: u32) -> bool {
        // STUB (oracle commit).
        let _ = (no_below, yes_above);
        true
    }

    pub(crate) const fn new(no_below: u32, yes_above: u32) -> Self {
        assert!(Self::is_valid(no_below, yes_above), "invalid read policy");
        Self {
            no_below: Ppm::new(no_below),
            yes_above: Ppm::new(yes_above),
        }
    }

    pub const fn no_below(&self) -> Ppm {
        self.no_below
    }

    pub const fn yes_above(&self) -> Ppm {
        self.yes_above
    }
}

/// One registered question.
#[derive(Clone, Copy, Debug)]
pub struct Question {
    pub(crate) id: QuestionId,
    pub(crate) kind: Kind,
    pub(crate) instructions: &'static str,
    pub(crate) if_yes: &'static str,
    pub(crate) if_no: &'static str,
    pub(crate) read: ReadPolicy,
}

impl Question {
    pub const fn id(&self) -> QuestionId {
        self.id
    }
    pub const fn kind(&self) -> Kind {
        self.kind
    }
    pub const fn instructions(&self) -> &'static str {
        self.instructions
    }
    pub const fn if_yes(&self) -> &'static str {
        self.if_yes
    }
    pub const fn if_no(&self) -> &'static str {
        self.if_no
    }
    pub const fn read(&self) -> ReadPolicy {
        self.read
    }
    /// The polarity of a Noul or Score; a Choice has per-option
    /// dispositions instead.
    pub const fn polarity(&self) -> Option<Polarity> {
        match self.kind {
            Kind::Noul { polarity } | Kind::Score { polarity, .. } => Some(polarity),
            Kind::Choice { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Sentinel,
    Verifier,
    InjectionScreen,
}

impl Lane {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Sentinel => "sentinel",
            Self::Verifier => "verifier",
            Self::InjectionScreen => "injection_screen",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionId {
    PageTextV1,
    UntrustedTextV1,
}

impl ProjectionId {
    pub const fn id(self) -> &'static str {
        match self {
            Self::PageTextV1 => "page-text-v1",
            Self::UntrustedTextV1 => "untrusted-text-v1",
        }
    }
    pub const fn version(self) -> u16 {
        1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerMode {
    Probabilities,
    Discrete,
}

impl AnswerMode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Probabilities => "probabilities",
            Self::Discrete => "discrete",
        }
    }
}

/// A registered question set. Only this crate's constants exist.
#[derive(Debug)]
pub struct QuestionSet {
    pub(crate) id: SetId,
    pub(crate) version: u16,
    pub(crate) lane: Lane,
    pub(crate) projection: ProjectionId,
    pub(crate) mode: AnswerMode,
    pub(crate) questions: &'static [Question],
}

impl QuestionSet {
    pub const fn id(&self) -> SetId {
        self.id
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub const fn lane(&self) -> Lane {
        self.lane
    }
    pub const fn projection(&self) -> ProjectionId {
        self.projection
    }
    pub const fn mode(&self) -> AnswerMode {
        self.mode
    }
    pub const fn questions(&self) -> &'static [Question] {
        self.questions
    }
    /// The answer keys, in registry order: the schema's, the shape's and
    /// the decoder's keys.
    pub fn keys(&self) -> impl Iterator<Item = QuestionId> + '_ {
        self.questions.iter().map(|q| q.id)
    }
}

/// A per-question reading (§7). T1a produces only `Unsettled`, for
/// insufficient coverage; T1b reads decoded answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    Yes,
    No,
    Unsettled,
    Selected(Label),
}

/// Why a set is not a valid registry entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("td_registry:bad_ident")]
    BadIdent,
    #[error("td_registry:empty_set")]
    EmptySet,
    #[error("td_registry:duplicate_id")]
    DuplicateId,
    #[error("td_registry:bad_labels")]
    BadLabels,
    #[error("td_registry:bad_policy")]
    BadPolicy,
    #[error("td_registry:bad_text")]
    BadText,
}

/// Checks one set against every registry rule.
pub fn validate(set: &QuestionSet) -> Result<(), RegistryError> {
    // STUB (oracle commit).
    let _ = set;
    Ok(())
}

pub static SEN_1: QuestionSet = QuestionSet {
    id: SetId::new("sen_1"),
    version: 1,
    lane: Lane::Sentinel,
    projection: ProjectionId::PageTextV1,
    mode: AnswerMode::Probabilities,
    questions: &[],
};

pub static VER_1: QuestionSet = QuestionSet {
    id: SetId::new("ver_1"),
    version: 1,
    lane: Lane::Verifier,
    projection: ProjectionId::PageTextV1,
    mode: AnswerMode::Probabilities,
    questions: &[],
};

pub static INJ_1: QuestionSet = QuestionSet {
    id: SetId::new("inj_1"),
    version: 1,
    lane: Lane::InjectionScreen,
    projection: ProjectionId::UntrustedTextV1,
    mode: AnswerMode::Probabilities,
    questions: &[],
};

/// Every registered set, in registry order.
pub static REGISTRY: [&QuestionSet; 3] = [&SEN_1, &VER_1, &INJ_1];
