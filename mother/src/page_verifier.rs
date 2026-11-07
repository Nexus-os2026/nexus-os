//! M3-Dc1: the page Verifier lane, its identity-bound input and its typed
//! report. No model is called and nothing is judged by a model: the lane
//! runs M3-A's pure `verify_page` on the retained candidate, the consented
//! facts and the journaled broker receipt. The input hash commits to their
//! digests only; the report carries closed codes only, and can be made only
//! here, from an input whose content is its declared identity.
use crate::evidence::VerifyCode;
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, Verdict};
use crate::page_inputs::PageFacts;
use crate::verifier::{verify_page, CheckId, CheckResult, Receipt};
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt;

pub const NAME: &str = "page-verifier";
pub const VERSION: &str = "m3-1";
const REPORT_DOMAIN: &[u8] = b"nexus-m3/verification-report/1\0";

/// What the lane input commits to (§4 order): digests, sizes and a flag.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VerificationIdentity {
    pub candidate_sha256: String,
    pub candidate_bytes: u64,
    pub facts_hash: String,
    pub receipt_sha256: String,
    pub receipt_bytes: u64,
    pub receipt_policy_hash: String,
    pub receipt_overtaken: bool,
}

/// The journaled broker receipt's fields, as the coordinator read them.
pub(crate) struct ReceiptFields {
    pub sha256: String,
    pub bytes: u64,
    pub policy_hash: String,
    pub overtaken: bool,
}

/// The candidate and facts (never serialized) and their identity.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct VerificationInput {
    #[serde(skip)]
    candidate: Vec<u8>,
    #[serde(skip)]
    facts: PageFacts,
    identity: VerificationIdentity,
}

impl fmt::Debug for VerificationInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerificationInput")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// The candidate and facts parts of an identity, computed from content.
fn content_identity(candidate: &[u8], facts: &PageFacts) -> Option<(String, u64, String)> {
    let facts = facts.approved().ok()?;
    let sha = hex::encode(Sha256::digest(candidate));
    Some((sha, candidate.len() as u64, hex::encode(facts.hash())))
}

impl VerificationInput {
    /// The candidate and facts parts recomputed here; the receipt's from
    /// the journal. Facts that cannot be approved give no input.
    pub(crate) fn new(candidate: Vec<u8>, facts: &PageFacts, r: &ReceiptFields) -> Option<Self> {
        let (candidate_sha256, candidate_bytes, facts_hash) = content_identity(&candidate, facts)?;
        let identity = VerificationIdentity {
            candidate_sha256,
            candidate_bytes,
            facts_hash,
            receipt_sha256: r.sha256.clone(),
            receipt_bytes: r.bytes,
            receipt_policy_hash: r.policy_hash.clone(),
            receipt_overtaken: r.overtaken,
        };
        Some(Self {
            candidate,
            facts: facts.clone(),
            identity,
        })
    }

    pub fn identity(&self) -> &VerificationIdentity {
        &self.identity
    }

    #[cfg(test)]
    pub(crate) fn identity_mut(&mut self) -> &mut VerificationIdentity {
        &mut self.identity
    }

    #[cfg(test)]
    pub(crate) fn payload_mut(&mut self) -> (&mut Vec<u8>, &mut PageFacts) {
        (&mut self.candidate, &mut self.facts)
    }

    /// The broker receipt, unless the publication was overtaken.
    fn receipt(&self) -> Option<Receipt> {
        let i = &self.identity;
        let digest = |hex: &str| -> Option<[u8; 32]> { hex::decode(hex).ok()?.try_into().ok() };
        (!i.receipt_overtaken).then_some(())?;
        Some(Receipt {
            sha256: digest(&i.receipt_sha256)?,
            bytes: i.receipt_bytes,
            policy_hash: digest(&i.receipt_policy_hash)?,
        })
    }
}

/// The page Verifier's typed result: the identity it judged and its six
/// checks, or none when the payload was not that identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationReport {
    identity: VerificationIdentity,
    checks: Option<[CheckResult; 6]>,
}

#[derive(Serialize)]
struct Wire<'a> {
    identity: &'a VerificationIdentity,
    checks: Option<Vec<(String, String)>>,
}

impl Serialize for VerificationReport {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Wire {
            identity: &self.identity,
            checks: self.entries(),
        }
        .serialize(s)
    }
}

impl VerificationReport {
    fn of(input: &VerificationInput) -> Self {
        let own = content_identity(&input.candidate, &input.facts);
        let i = &input.identity;
        let checks = own
            .filter(|own| {
                *own == (
                    i.candidate_sha256.clone(),
                    i.candidate_bytes,
                    i.facts_hash.clone(),
                )
            })
            .and_then(|_| input.facts.approved().ok())
            .map(|facts| {
                let receipt = input.receipt();
                let report = verify_page(&input.candidate, &facts, receipt.as_ref());
                report.checks().map(|(_, result)| result)
            });
        Self {
            identity: input.identity.clone(),
            checks,
        }
    }

    pub fn identity(&self) -> &VerificationIdentity {
        &self.identity
    }

    /// The ordered `(id, result)` codes, if the checks ran.
    pub fn entries(&self) -> Option<Vec<(String, String)>> {
        let checks = self.checks?;
        Some(
            CheckId::ALL
                .iter()
                .zip(checks)
                .map(|(id, result)| (id.as_str().to_string(), result.to_string()))
                .collect(),
        )
    }

    pub fn passed(&self) -> bool {
        self.checks
            .is_some_and(|c| c.iter().all(|r| *r == CheckResult::Pass))
    }

    /// The first failing code in table order, if not every check passed.
    pub fn failure(&self) -> Option<VerifyCode> {
        if self.passed() {
            return None;
        }
        let first = self.checks.into_iter().flatten().find_map(|r| match r {
            CheckResult::Fail(code) => Some(code),
            _ => None,
        });
        Some(first.map_or(VerifyCode::Identity, VerifyCode::Check))
    }
}

/// The frozen report digest: SHA-256 of the domain and the compact JSON of
/// the ordered `(id, code)` checks.
pub fn report_digest(entries: &[(String, String)]) -> String {
    let json = serde_json::to_vec(entries).expect("checks serialize");
    let mut h = Sha256::new();
    h.update(REPORT_DOMAIN);
    h.update(json);
    hex::encode(h.finalize())
}

/// `page-verifier`/`m3-1`: zero model calls, no effects.
pub struct PageVerifierLane;

impl Lane for PageVerifierLane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Verifier,
            name: NAME.into(),
            version: VERSION.into(),
        }
    }

    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        let report = input.verification.as_ref().map(VerificationReport::of);
        let verdict = match report.as_ref().map(|r| (r.passed(), r.failure())) {
            Some((true, _)) => Verdict::Pass,
            Some((false, Some(code))) => Verdict::Fail {
                reason: code.as_str(),
            },
            _ => Verdict::Continue,
        };
        LaneOutput {
            text: "page checks: codes recorded".into(),
            effects: vec![],
            verdict,
            verification: report,
        }
    }
}
