//! The page verdict (design v2 §6): which candidate bytes were checked,
//! under which policy, with which approved facts, and the outcome, bound
//! together by [`PageVerdict::binding_hash`] for the run binding (S1a-C) and
//! M2d's review.
//!
//! The label is exactly [`LABEL_PASSED`] whenever the inert check passed,
//! whatever the facts: a missing fact never changes it. A facts pass adds a
//! separate line ([`PageVerdict::facts_line`]); only a full pass is
//! [`PageVerdict::passed`].

use sha2::{Digest, Sha256};

use super::facts::{check_facts, ApprovedFacts, FactCode};
use super::policy::{policy_hash, LABEL_PASSED, POLICY_V1};
use super::{check, work, RefusalCode};

/// Which approved facts a verdict was evaluated with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FactsMarker {
    NoFacts,
    Facts([u8; 32]),
}

impl FactsMarker {
    /// The marker of `facts`: their hash, or none.
    pub fn of(facts: Option<&ApprovedFacts>) -> Self {
        facts.map_or(FactsMarker::NoFacts, |f| FactsMarker::Facts(f.hash()))
    }
}

/// What the check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageOutcome {
    /// The inert check passed, and so did the facts check if facts were
    /// given.
    Passed,
    /// The inert check refused the candidate (S1 to S4).
    Refused(RefusalCode),
    /// The inert check passed, and the facts check found this fact missing.
    FactMissing(FactCode),
}

impl PageOutcome {
    /// The outcome as bound: a u16 code (0 passed; 0x0100 plus the refusal
    /// position; 0x0200 plus the fact position) and a u32 detail (an hours
    /// line's index, else 0).
    fn code(self) -> (u16, u32) {
        match self {
            PageOutcome::Passed => (0, 0),
            PageOutcome::Refused(c) => (0x0100 + c.position(), 0),
            PageOutcome::FactMissing(f) => (
                0x0200 + f.position(),
                match f {
                    FactCode::HoursLineMissing { index } => u32::from(index),
                    _ => 0,
                },
            ),
        }
    }
}

/// The verdict on one candidate. Equal verdicts name the same bytes,
/// policy, facts and outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageVerdict {
    pub candidate_sha256: [u8; 32],
    pub policy_hash: [u8; 32],
    pub facts: FactsMarker,
    pub outcome: PageOutcome,
    /// The number of approved hours lines (0 without facts), for the facts
    /// line. The facts hash already covers it.
    hours_lines: u8,
}

const VERDICT_DOMAIN: &[u8] = b"nexus.inert_page.verdict.v1";

impl PageVerdict {
    /// Checks `candidate` (S1 to S4), then, if it passed and `facts` are
    /// given, the facts (FC-1 to FC-4).
    pub fn evaluate(candidate: &[u8], facts: Option<&ApprovedFacts>) -> Self {
        work::add(candidate.len());
        let candidate_sha256: [u8; 32] = Sha256::digest(candidate).into();
        let outcome = match check(candidate) {
            Err(refusal) => PageOutcome::Refused(refusal.code),
            Ok(page) => match facts.map(|f| check_facts(&page, f)) {
                Some(Err(code)) => PageOutcome::FactMissing(code),
                Some(Ok(())) | None => PageOutcome::Passed,
            },
        };
        PageVerdict {
            candidate_sha256,
            policy_hash: policy_hash(),
            facts: FactsMarker::of(facts),
            outcome,
            hours_lines: facts.map_or(0, |f| u8::try_from(f.hours_lines()).unwrap_or(u8::MAX)),
        }
    }

    /// True only for a full pass: the inert check and, if given, the facts.
    pub fn passed(&self) -> bool {
        self.outcome == PageOutcome::Passed
    }

    /// SHA-256 over `put("nexus.inert_page.verdict.v1")` (`put(b)` is `b`'s
    /// length as a u64, big-endian, then `b`), the candidate's SHA-256, the
    /// policy hash, the facts tag (0, or 1 then the facts hash), and the
    /// outcome's u16 code and u32 detail, big-endian.
    pub fn binding_hash(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update((VERDICT_DOMAIN.len() as u64).to_be_bytes());
        h.update(VERDICT_DOMAIN);
        h.update(self.candidate_sha256);
        h.update(self.policy_hash);
        match self.facts {
            FactsMarker::NoFacts => h.update([0u8]),
            FactsMarker::Facts(f) => {
                h.update([1u8]);
                h.update(f);
            }
        }
        let (code, detail) = self.outcome.code();
        h.update(code.to_be_bytes());
        h.update(detail.to_be_bytes());
        h.finalize().into()
    }

    /// Whether this verdict is for exactly `candidate`, `policy` and
    /// `facts`.
    pub fn is_for(&self, candidate: &[u8], policy: &[u8; 32], facts: &FactsMarker) -> bool {
        work::add(candidate.len());
        let sha: [u8; 32] = Sha256::digest(candidate).into();
        self.candidate_sha256 == sha && self.policy_hash == *policy && self.facts == *facts
    }

    /// [`LABEL_PASSED`] whenever the inert check passed (a missing fact
    /// included); otherwise `POLICY_V1.label_refused_prefix`, the code being
    /// the outcome's.
    pub fn label(&self) -> &'static str {
        match self.outcome {
            PageOutcome::Passed | PageOutcome::FactMissing(_) => LABEL_PASSED,
            PageOutcome::Refused(_) => POLICY_V1.label_refused_prefix,
        }
    }

    /// The separate facts line, `POLICY_V1.facts_line_template` with `{n}`
    /// the number of hours lines: only for a full pass with facts.
    pub fn facts_line(&self) -> Option<String> {
        match (self.outcome, self.facts) {
            (PageOutcome::Passed, FactsMarker::Facts(_)) => Some(
                POLICY_V1
                    .facts_line_template
                    .replace("{n}", &self.hours_lines.to_string()),
            ),
            _ => None,
        }
    }
}
