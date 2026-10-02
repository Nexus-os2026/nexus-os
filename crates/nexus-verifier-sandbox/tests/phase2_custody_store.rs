//! P2-V1-R3B-I3-I1 fixture controls for the custody store
//! (`support/custody/store/`): formats and the record grammar; per-file and
//! pool-level classification and the conservative refusal; the simulated
//! storage's state model and its conformance to the cited kernel paths; the
//! opening (selection, revalidation, safe open, mount, profile and storage
//! checks, the opening-bound storage admission, durable activation, the
//! scan and the claim); the recorder (the exchange, the one fatal latch, the
//! worker, failure delivery and the store admission gate) driven through the
//! real custody core; maintenance sessions and every procedure's crash
//! points and protocol steps; composed recovery; and unprivileged native
//! primitives beneath `CARGO_TARGET_TMPDIR`.
//!
//! The custody module is included here exactly as the core and codec
//! targets include it, never copied: every run uses the real `Custody`, its
//! ledger, admission gate and codec. These are not live evidence. The store
//! runs against simulated storage and a fixture host; every host, profile
//! and storage value is a fixture, and a passing test qualifies no host,
//! filesystem or device. The real configured-store entry is closed, and no
//! test opens a real store. Native tests touch only their own files beneath
//! `CARGO_TARGET_TMPDIR`; they inject no real I/O error or power loss.
//!
//! Every assertion that guards a requirement carries a bracketed marker; the
//! negative controls of `docs/evidence/p2-v1-r3b-i3-i1/` restore one wrong
//! behaviour each and must fail the assertion carrying their marker.

#![cfg(target_os = "linux")]

#[path = "support/custody/mod.rs"]
mod custody;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use custody::codec::{self, RecordEvidence};
use custody::store::classify::{self, classify_bytes, FileClass, Rule};
use custody::store::exchange::{
    Cause, ClaimState, Delivery, GateRefused, Invalid, SealState, SealWithheld, WorkerExit,
};
use custody::store::format::{
    self, encode_header, encode_seal, parse_disposition, parse_header, parse_provision,
    parse_record_block, parse_seal, render_disposition, render_provision, seal_for, sha256,
    ArchiveClass, ArchiveName, Disposition, HeaderFields, HeaderParse, IncidentClass,
    IncidentFacts, IncidentKind, PrefixFacts, RecordBlock, SealParse, BLOCK,
};
use custody::store::open::{self, open_owner, verify_standalone, NoHooks, Refused};
use custody::store::recorder::{start_owner, OwnerStart, StartRefused, Step};
use custody::store::sim::{self, Fixture, HostFixture, SimIo, SimWorld};
use custody::*;
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

/// A stand-in native owner of one slot kind.
#[derive(Debug)]
struct Token(SlotKind);

impl Resource for Token {
    fn kind(&self) -> SlotKind {
        self.0
    }
}

/// A cleanup adapter that confirms every fact (complete output), fails
/// every one, or detaches the output.
#[derive(Clone, Copy)]
enum Clean {
    Confirm,
    Fail,
    Detach,
}

impl Cleanup<Token> for Clean {
    fn attempt(&mut self, _owner: &mut Token, _entry: &EntryView) -> CleanupReport {
        let (fact, output) = match self {
            Clean::Confirm => (Observed::Confirmed, OutputObserved::Complete),
            Clean::Fail => (Observed::StillPending, OutputObserved::StillPending),
            Clean::Detach => (Observed::Confirmed, OutputObserved::Detached),
        };
        CleanupReport {
            subtree: fact,
            reaped: fact,
            output,
            removed: fact,
        }
    }
}

const SMALL: Config = Config {
    automatic_attempts: 2,
    recovery_budget: 3,
    recovery_spacing_millis: 1,
    lease_millis: 1_000_000,
    record_capacity: 16,
    control_reserve: 2,
    receipt_limit: 4,
    late_limit: 1,
    failure_detail_limit: 8,
    detail_chars: 64,
    incident_limit: 4,
};

fn clock() -> Arc<dyn Fn() -> Tick + Send + Sync> {
    let counter = Arc::new(AtomicU64::new(0));
    Arc::new(move || Tick(counter.fetch_add(1, Ordering::SeqCst)))
}

fn hexs(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A labelled in-place edit of one encoded block.
type BlockEdit = (&'static str, Box<dyn Fn(&mut [u8])>);

/// A labelled edit of the fixture host and the refusal it must produce.
type HostCase<'a> = (
    &'static str,
    Box<dyn Fn(&mut HostFixture) + 'a>,
    &'static str,
);

// -- an independent record-frame encoder (from the version-1 table) -------

/// The generation of the grammar fixtures.
const GR: [u8; 16] = [0x47; 16];

/// A record kind with identities as sequence numbers (of `GR`), for building
/// frames from the version-1 table independently of the core.
#[derive(Debug, Clone, Copy)]
enum K {
    Rs(u32),
    Cs(u32, Expectation),
    As(u64, SlotKind),
    Af(u64),
    Aset(u64, Settlement),
    Ce(u32, bool),
    Ac(ClosureReason, u64),
    Sr,
    Rr,
    Ra(u32, bool),
    Re(Verdict, Option<u32>),
    Io(u64, u64, Option<SlotKind>),
    /// An incident whose action identity carries another generation.
    IoForeign(u64, [u8; 16], u64),
    Is(u64, Settlement),
}

fn slot_tag(kind: SlotKind) -> u8 {
    match kind {
        SlotKind::Process => 1,
        SlotKind::Workspace => 2,
        SlotKind::Fixture => 3,
    }
}

fn settlement_tag(how: Settlement) -> u8 {
    match how {
        Settlement::Confirmed => 1,
        Settlement::OutputLost => 2,
        Settlement::NothingCreated => 3,
        Settlement::NotAdmitted => 4,
    }
}

fn identity(out: &mut Vec<u8>, generation: [u8; 16], seq: u64) {
    out.extend_from_slice(&generation);
    out.extend_from_slice(&seq.to_be_bytes());
}

/// The frame of record `seq` of generation `generation`, at `at`.
fn frame(generation: [u8; 16], seq: u64, at: u64, kind: K) -> Vec<u8> {
    let mut payload = Vec::new();
    identity(&mut payload, generation, seq);
    payload.extend_from_slice(&at.to_be_bytes());
    match kind {
        K::Rs(n) => {
            payload.push(0x01);
            payload.extend_from_slice(&n.to_be_bytes());
        }
        K::Cs(case, expectation) => {
            payload.push(0x02);
            payload.extend_from_slice(&case.to_be_bytes());
            payload.push(match expectation {
                Expectation::Clean => 1,
                Expectation::RetainedBoundary => 2,
                Expectation::OutputDetached => 3,
            });
        }
        K::As(action, slot) => {
            payload.push(0x03);
            identity(&mut payload, generation, action);
            payload.push(slot_tag(slot));
        }
        K::Af(action) => {
            payload.push(0x04);
            identity(&mut payload, generation, action);
        }
        K::Aset(action, how) => {
            payload.push(0x05);
            identity(&mut payload, generation, action);
            payload.push(settlement_tag(how));
        }
        K::Ce(case, passed) => {
            payload.push(0x06);
            payload.extend_from_slice(&case.to_be_bytes());
            payload.push(u8::from(passed));
        }
        K::Ac(reason, after) => {
            payload.push(0x07);
            payload.push(0x01);
            match reason {
                ClosureReason::Cancelled(cancel) => {
                    payload.push(0x01);
                    payload.push(match cancel {
                        CancelReason::Requested => 1,
                        CancelReason::LeaseLost => 2,
                        CancelReason::Shutdown => 3,
                        CancelReason::Stop => 4,
                        CancelReason::RunBudget => 5,
                    });
                }
                ClosureReason::Failed(class) => {
                    payload.push(0x02);
                    payload.push(class.index() as u8 + 1);
                }
            }
            payload.extend_from_slice(&after.to_be_bytes());
        }
        K::Sr => {
            payload.push(0x07);
            payload.push(0x02);
        }
        K::Rr => payload.push(0x08),
        K::Ra(attempt, resolved) => {
            payload.push(0x09);
            payload.extend_from_slice(&attempt.to_be_bytes());
            payload.push(u8::from(resolved));
        }
        K::Re(verdict, by) => {
            payload.push(0x0a);
            payload.push(match verdict {
                Verdict::Pending => 1,
                Verdict::Passed => 2,
                Verdict::Failed => 3,
            });
            match by {
                None => payload.push(0),
                Some(n) => {
                    payload.push(1);
                    payload.extend_from_slice(&n.to_be_bytes());
                }
            }
        }
        K::Io(incident, action, slot) => {
            payload.push(0x0b);
            identity(&mut payload, generation, incident);
            identity(&mut payload, generation, action);
            match slot {
                None => payload.push(0),
                Some(slot) => {
                    payload.push(1);
                    payload.push(slot_tag(slot));
                }
            }
        }
        K::IoForeign(incident, other, action) => {
            payload.push(0x0b);
            identity(&mut payload, generation, incident);
            identity(&mut payload, other, action);
            payload.push(1);
            payload.push(1);
        }
        K::Is(incident, how) => {
            payload.push(0x0c);
            identity(&mut payload, generation, incident);
            payload.push(settlement_tag(how));
        }
    }
    let mut out = b"NXCD".to_vec();
    out.push(0x52);
    out.push(0x01);
    out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    out.extend_from_slice(&payload);
    let digest: [u8; 32] = Sha256::digest(&out).into();
    out.extend_from_slice(&digest);
    out
}

/// Records of `GR`, numbered from 1, at the given instants (default: the
/// sequence number).
fn records(kinds: &[K], ats: Option<&[u64]>) -> Vec<RecordEvidence> {
    kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            let at = ats.map_or(index as u64, |ats| ats[index]);
            match codec::decode_record(&frame(GR, index as u64 + 1, at, *kind)) {
                Ok(record) => record,
                Err(error) => panic!("fixture frame {index} does not decode: {error:?}"),
            }
        })
        .collect()
}

use self::K::*;

const CANCELLED: ClosureReason = ClosureReason::Cancelled(CancelReason::Requested);

fn failed(class: FailureClass) -> ClosureReason {
    ClosureReason::Failed(class)
}

/// The source-derived fixtures T1 to T15 of design section 11.5 (T8 and T12
/// are prefixes of T1).
fn traces() -> Vec<(&'static str, Vec<K>)> {
    let t1 = vec![
        Rs(0),
        Cs(1, Expectation::Clean),
        As(1, SlotKind::Process),
        Aset(1, Settlement::Confirmed),
        Ce(1, true),
        Re(Verdict::Passed, None),
    ];
    let mut t7 = t1.clone();
    t7.extend([
        Io(1, 1, Some(SlotKind::Process)),
        Is(1, Settlement::Confirmed),
        Ra(1, true),
    ]);
    vec![
        ("T1", t1),
        ("T2", vec![Ac(CANCELLED, 0), Sr]),
        (
            "T3",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Process),
                Ac(CANCELLED, 1),
                Aset(1, Settlement::Confirmed),
                Ce(1, true),
                Re(Verdict::Failed, None),
            ],
        ),
        (
            "T4",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Process),
                Af(1),
                Ac(failed(FailureClass::UnexpectedCleanup), 1),
                Ce(1, false),
                Rr,
                Aset(1, Settlement::Confirmed),
                Ra(1, true),
                Re(Verdict::Failed, Some(1)),
            ],
        ),
        (
            "T5",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Process),
                Af(1),
                Ac(failed(FailureClass::UnknownOutcome), 1),
                Ce(1, false),
                Rr,
                Aset(1, Settlement::NothingCreated),
                Ra(1, true),
                Re(Verdict::Failed, Some(1)),
            ],
        ),
        (
            "T6",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Process),
                Af(1),
                Ac(failed(FailureClass::AuthorityLost), 1),
                Ce(1, false),
                Rr,
            ],
        ),
        ("T7", t7),
        (
            "T9",
            vec![
                Rs(0),
                Cs(1, Expectation::RetainedBoundary),
                As(1, SlotKind::Process),
                Aset(1, Settlement::Confirmed),
                Ce(1, true),
                Re(Verdict::Passed, None),
            ],
        ),
        (
            "T10",
            vec![
                Rs(0),
                Cs(1, Expectation::OutputDetached),
                As(1, SlotKind::Process),
                Aset(1, Settlement::OutputLost),
                Ce(1, true),
                Re(Verdict::Passed, None),
            ],
        ),
        (
            "T11",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Process),
                Ac(ClosureReason::Cancelled(CancelReason::Shutdown), 1),
                Sr,
                Aset(1, Settlement::Confirmed),
                Ce(1, true),
                Re(Verdict::Failed, None),
            ],
        ),
        (
            "T13",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Process),
                Aset(1, Settlement::Confirmed),
                Ce(1, true),
                Io(1, 1, Some(SlotKind::Process)),
                Ac(failed(FailureClass::LateOwner), 1),
                Rr,
                Is(1, Settlement::Confirmed),
                Ra(1, true),
                Re(Verdict::Failed, Some(1)),
            ],
        ),
        (
            "T14",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Process),
                Ac(CANCELLED, 0),
                Aset(1, Settlement::NotAdmitted),
                Ce(1, true),
                Re(Verdict::Failed, None),
            ],
        ),
        (
            "T15",
            vec![
                Rs(0),
                Cs(1, Expectation::Clean),
                As(1, SlotKind::Workspace),
                Af(1),
                Ac(failed(FailureClass::UnexpectedOwner), 1),
                Aset(1, Settlement::Confirmed),
                Ce(1, false),
                Re(Verdict::Failed, None),
            ],
        ),
    ]
}

fn trace(name: &str) -> Vec<K> {
    traces()
        .into_iter()
        .find(|(label, _)| *label == name)
        .map(|(_, kinds)| kinds)
        .unwrap_or_default()
}

fn splice(base: &[K], keep: usize, insert: &[K], resume: usize) -> Vec<K> {
    let mut out = base[..keep].to_vec();
    out.extend_from_slice(insert);
    out.extend_from_slice(&base[resume.min(base.len())..]);
    out
}

/// The 31 single-rule mutations of the design model's C13, with the rule
/// each must break.
fn grammar_mutations() -> Vec<(&'static str, Vec<K>, Rule)> {
    let t1 = trace("T1");
    let t2 = trace("T2");
    let t3 = trace("T3");
    let t4 = trace("T4");
    let t5 = trace("T5");
    let t7 = trace("T7");
    let t11 = trace("T11");
    let t13 = trace("T13");
    let t15 = trace("T15");
    vec![
        (
            "RunEnded before CaseEnded",
            splice(&t1, 4, &[t1[5], t1[4]], 6),
            Rule::G15,
        ),
        ("ActionStarted removed", splice(&t1, 2, &[], 3), Rule::G8),
        ("second RunStarted", splice(&t1, 1, &[Rs(0)], 1), Rule::G2),
        (
            "ShutdownRefused before AdmissionClosed",
            vec![t2[1], t2[0]],
            Rule::G3,
        ),
        (
            "CaseStarted before RunStarted",
            splice(&t1, 0, &[t1[1], t1[0]], 2),
            Rule::G3,
        ),
        (
            "ActionStarted after AdmissionClosed",
            splice(&t3, 2, &[Ac(CANCELLED, 0), As(1, SlotKind::Process)], 5),
            Rule::G6,
        ),
        (
            "CaseEnded for another case",
            splice(&t1, 4, &[Ce(2, true)], 5),
            Rule::G5,
        ),
        (
            "RunEnded Pending",
            splice(&t1, 5, &[Re(Verdict::Pending, None)], 6),
            Rule::G15,
        ),
        (
            "Failed verdict without a closure",
            splice(&t1, 5, &[Re(Verdict::Failed, None)], 6),
            Rule::G16,
        ),
        (
            "Passed verdict after a closure",
            splice(&t3, 6, &[Re(Verdict::Passed, None)], 7),
            Rule::G16,
        ),
        (
            "recovery attempt numbering",
            splice(&t4, 8, &[Ra(2, true), Re(Verdict::Failed, Some(2))], 10),
            Rule::G12,
        ),
        (
            "recovery attempt without RecoveryRequired",
            splice(&t4, 6, &[], 7),
            Rule::G12,
        ),
        (
            "attempt after RunEnded without an incident",
            splice(&t7, 6, &[Ra(1, true)], 9),
            Rule::G12,
        ),
        (
            "IncidentSettled NotAdmitted",
            splice(&t7, 7, &[Is(1, Settlement::NotAdmitted)], 8),
            Rule::G14,
        ),
        (
            "IncidentOpened for an unstarted action",
            splice(&t7, 6, &[Io(1, 2, Some(SlotKind::Process))], 7),
            Rule::G13,
        ),
        (
            "incident numbering",
            splice(&t7, 6, &[Io(2, 1, Some(SlotKind::Process))], 7),
            Rule::G13,
        ),
        (
            "NotAdmitted after ActionFailed",
            splice(&t5, 7, &[Aset(1, Settlement::NotAdmitted)], 8),
            Rule::G8,
        ),
        (
            "ActionFailed after ActionSettled",
            splice(&t4, 3, &[Aset(1, Settlement::Confirmed), Af(1)], 5),
            Rule::G7,
        ),
        (
            "ActionFailed after RunEnded",
            splice(&t1, 6, &[Af(1)], 6),
            Rule::G17,
        ),
        (
            "second AdmissionClosed",
            vec![t2[0], Ac(CANCELLED, 0), t2[1]],
            Rule::G9,
        ),
        (
            "second ShutdownRefused",
            splice(&t11, 5, &[Sr], 5),
            Rule::G10,
        ),
        (
            "RecoveryRequired with a case open",
            splice(&t4, 5, &[Rr, t4[5]], 7),
            Rule::G11,
        ),
        (
            "dispositioned count differs from the header",
            splice(&t1, 0, &[Rs(1)], 1),
            Rule::G2,
        ),
        (
            "closure counts more admissions than starts",
            splice(&t3, 3, &[Ac(CANCELLED, 2)], 4),
            Rule::G9,
        ),
        (
            "CaseStarted after RecoveryRequired",
            splice(&t4, 7, &[Cs(2, Expectation::Clean)], 7),
            Rule::G4,
        ),
        (
            "passed case with a failed action",
            splice(&t15, 6, &[Ce(1, true)], 7),
            Rule::G5,
        ),
        (
            "resolved_by missing after recovery",
            splice(&t4, 9, &[Re(Verdict::Failed, None)], 10),
            Rule::G15,
        ),
        (
            "resolved_by without recovery",
            splice(&t1, 5, &[Re(Verdict::Passed, Some(1))], 6),
            Rule::G15,
        ),
        (
            "CaseStarted while a case is open",
            splice(&t1, 2, &[Cs(2, Expectation::Clean)], 2),
            Rule::G4,
        ),
        (
            "action numbering gap",
            splice(&t1, 2, &[As(2, SlotKind::Process)], 3),
            Rule::G6,
        ),
        (
            "identity of another generation",
            splice(&t13, 5, &[IoForeign(1, [0; 16], 1)], 6),
            Rule::GId,
        ),
    ]
}

// -- journal images ---------------------------------------------------------

const ROOT: [u8; 16] = sim::ROOT_ID;

fn header_fields(
    generation: [u8; 16],
    claim: u64,
    index: u32,
    c_pool: u32,
    capacity: u32,
) -> HeaderFields {
    HeaderFields {
        root_id: ROOT,
        generation,
        claim,
        pool_index: index,
        c_pool,
        capacity,
        created_ms: 7,
        boot_id: [9; 16],
        applied: Vec::new(),
    }
}

/// A pool file's bytes: a header, an optional seal block, and the record
/// frames in order (blocks 2, 3, ...).
fn journal_image(header: &HeaderFields, frames: &[Vec<u8>], seal: Option<&[u8; BLOCK]>) -> Vec<u8> {
    let c_pool = header.c_pool as usize;
    let mut data = vec![0u8; (c_pool + 2) * BLOCK];
    let block = match encode_header(header) {
        Ok(block) => block,
        Err(error) => panic!("fixture header: {error:?}"),
    };
    data[..BLOCK].copy_from_slice(&block[..]);
    if let Some(seal) = seal {
        data[BLOCK..2 * BLOCK].copy_from_slice(seal);
    }
    for (index, frame) in frames.iter().enumerate() {
        let at = (index + 2) * BLOCK;
        data[at..at + frame.len()].copy_from_slice(frame);
    }
    data
}

/// The frames of a list of kinds, for generation `generation`.
fn frames_of(generation: [u8; 16], kinds: &[K]) -> Vec<Vec<u8>> {
    kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| frame(generation, index as u64 + 1, index as u64, *kind))
        .collect()
}

/// The seal the recorder writes for an image's header and prefix.
fn seal_of(header: &HeaderFields, kinds: &[K]) -> Box<[u8; BLOCK]> {
    let block = match encode_header(header) {
        Ok(block) => block,
        Err(error) => panic!("fixture header: {error:?}"),
    };
    let HeaderParse::Valid(parsed) = parse_header(&block[..], &ROOT, None, None) else {
        panic!("fixture header does not parse");
    };
    let recs: Vec<RecordEvidence> = frames_of(header.generation, kinds)
        .iter()
        .map(|frame| codec::decode_record(frame).expect("fixture frame"))
        .collect();
    let (_, summary) = classify::check_grammar(
        &recs,
        Generation::new(header.generation),
        header.applied.len() as u32,
    );
    let facts = PrefixFacts {
        records: recs.len() as u64,
        last_digest: recs.last().map(|record| record.digest).unwrap_or([0; 32]),
        run_ended: summary.run_ended,
        run_started: summary.run_started,
        unsettled: summary.unsettled(),
    };
    encode_seal(&seal_for(&parsed, &facts, 0))
}

// ---------------------------------------------------------------------------
// F: formats (design section 7) and bindings (section 12.3)
// ---------------------------------------------------------------------------

fn valid_header() -> HeaderFields {
    let mut fields = header_fields([0x61; 16], 5, 1, 16, 12);
    fields.applied = vec![
        ([0x01; 32], DispositionReason::HostRebooted),
        ([0x02; 32], DispositionReason::Other),
    ];
    fields
}

fn redigest_header(block: &mut [u8]) {
    let n = usize::from(block[88]);
    let end = 92 + 33 * n;
    let digest = sha256(&block[..end]);
    block[end..end + 32].copy_from_slice(&digest);
}

/// Every header byte is assigned and checked; a checksum-valid header that
/// breaks a rule is Malformed, never an abandoned claim.
#[test]
fn f01_header_bytes_are_assigned_and_checked() {
    let fields = valid_header();
    let block = encode_header(&fields).expect("a valid header");
    let HeaderParse::Valid(parsed) = parse_header(&block[..], &ROOT, Some(16), Some(1)) else {
        panic!("[exact-bytes] the valid header must parse");
    };
    assert_eq!(parsed.applied, fields.applied, "[exact-bytes] applied list");
    assert_eq!((parsed.claim, parsed.capacity), (5, 12));
    // Every single-byte change is detected.
    for at in 0..BLOCK {
        let mut changed = block.clone();
        changed[at] ^= 0x01;
        assert!(
            !matches!(
                parse_header(&changed[..], &ROOT, Some(16), Some(1)),
                HeaderParse::Valid(_)
            ),
            "[exact-bytes] header byte {at} changed and still valid"
        );
    }
    // Checksum-valid but invalid: each rule, digest recomputed.
    let edits: Vec<BlockEdit> = vec![
        ("container version", Box::new(|b: &mut [u8]| b[4] = 2)),
        ("frame domain", Box::new(|b: &mut [u8]| b[5] = 0x51)),
        ("log2 block", Box::new(|b: &mut [u8]| b[7] = 11)),
        ("root id", Box::new(|b: &mut [u8]| b[8] ^= 1)),
        (
            "zero generation",
            Box::new(|b: &mut [u8]| b[24..40].fill(0)),
        ),
        ("claim zero", Box::new(|b: &mut [u8]| b[40..48].fill(0))),
        (
            "claim above 2^63",
            Box::new(|b: &mut [u8]| b[40..48].copy_from_slice(&((1u64 << 63) + 1).to_be_bytes())),
        ),
        (
            "pool index",
            Box::new(|b: &mut [u8]| b[48..52].copy_from_slice(&2u32.to_be_bytes())),
        ),
        (
            "pool capacity",
            Box::new(|b: &mut [u8]| b[52..56].copy_from_slice(&15u32.to_be_bytes())),
        ),
        ("capacity zero", Box::new(|b: &mut [u8]| b[56..60].fill(0))),
        (
            "capacity above the pool",
            Box::new(|b: &mut [u8]| b[56..60].copy_from_slice(&17u32.to_be_bytes())),
        ),
        ("reserved 60..64", Box::new(|b: &mut [u8]| b[61] = 1)),
        ("reserved 89..92", Box::new(|b: &mut [u8]| b[90] = 1)),
        ("reason tag 0", Box::new(|b: &mut [u8]| b[92 + 32] = 0)),
        ("reason tag 6", Box::new(|b: &mut [u8]| b[92 + 32] = 6)),
        (
            "bindings not increasing",
            Box::new(|b: &mut [u8]| {
                let first: Vec<u8> = b[92..124].to_vec();
                b[125..157].copy_from_slice(&first);
            }),
        ),
    ];
    for (label, edit) in &edits {
        let mut changed = block.clone();
        edit(&mut changed[..]);
        redigest_header(&mut changed[..]);
        assert!(
            matches!(
                parse_header(&changed[..], &ROOT, Some(16), Some(1)),
                HeaderParse::Invalid(_)
            ),
            "[exact-bytes] {label}: a checksum-valid header breaking a rule is invalid"
        );
        // In a pool file whose other blocks are zero, it is Malformed, never
        // an abandoned claim.
        let mut image = vec![0u8; 18 * BLOCK];
        image[..BLOCK].copy_from_slice(&changed[..]);
        let report = classify_bytes(&image, ROOT, 16, 1);
        assert_eq!(
            report.class,
            FileClass::MalformedPoolFile,
            "[exact-bytes] {label}: a checksum-valid invalid header taken as {:?}",
            report.class
        );
    }
    // Bytes after the digest are not covered by it: still invalid.
    let mut tail = block.clone();
    tail[4000] = 1;
    assert!(
        matches!(
            parse_header(&tail[..], &ROOT, Some(16), Some(1)),
            HeaderParse::Invalid(_)
        ),
        "[exact-bytes] bytes after the digest"
    );
    // A torn header with every other block zero is an abandoned claim.
    let mut torn = vec![0u8; 18 * BLOCK];
    torn[..2048].copy_from_slice(&block[..2048]);
    torn[100] ^= 0x40;
    let report = classify_bytes(&torn, ROOT, 16, 1);
    assert_eq!(
        report.class,
        FileClass::AbandonedClaim,
        "[exact-bytes] torn header"
    );
    assert!(!report.refused && !report.native_work_possible);
    assert!(report
        .notes
        .iter()
        .any(|note| note.contains("does not prove")));
    // An encoder refuses what would not parse.
    let mut bad = valid_header();
    bad.applied.reverse();
    assert!(
        encode_header(&bad).is_err(),
        "[exact-bytes] the encoder refuses unordered bindings"
    );
}

/// Seal classification: zero, unreadable (treated as unsealed), malformed by
/// any rule, sealed; reserved and tail bytes checked.
#[test]
fn f02_seal_bytes_are_assigned_and_checked() {
    let header = header_fields([0x62; 16], 3, 0, 16, 12);
    let kinds = trace("T1");
    let frames = frames_of(header.generation, &kinds);
    let seal = seal_of(&header, &kinds);
    let image = journal_image(&header, &frames, Some(&seal));
    let report = classify_bytes(&image, ROOT, 16, 0);
    assert_eq!(
        report.class,
        FileClass::Sealed,
        "[exact-bytes] the recorder's seal: {}",
        report.reason
    );
    assert!(report.evidence_complete && !report.refused);
    // Torn: not checksum-valid, so unreadable, treated as unsealed.
    let mut torn = image.clone();
    torn[BLOCK + 70] ^= 1;
    let report = classify_bytes(&torn, ROOT, 16, 0);
    assert_eq!(report.seal, Some(SealParse::Unreadable));
    assert_eq!(
        report.class,
        FileClass::UnsealedAction,
        "[exact-bytes] a torn seal is treated as unsealed"
    );
    assert!(
        report.refused,
        "[no-false-resolution] unsealed with an action start refuses"
    );
    let redigest = |block: &mut [u8]| {
        let digest = sha256(&block[..128]);
        block[128..160].copy_from_slice(&digest);
    };
    let edits: Vec<BlockEdit> = vec![
        ("version", Box::new(|b: &mut [u8]| b[4] = 2)),
        ("reserved 5..8", Box::new(|b: &mut [u8]| b[6] = 1)),
        ("reserved 126..128", Box::new(|b: &mut [u8]| b[127] = 1)),
        ("generation", Box::new(|b: &mut [u8]| b[8] ^= 1)),
        ("claim", Box::new(|b: &mut [u8]| b[31] ^= 1)),
        ("header digest", Box::new(|b: &mut [u8]| b[40] ^= 1)),
        ("record count", Box::new(|b: &mut [u8]| b[71] = 5)),
        ("last digest", Box::new(|b: &mut [u8]| b[80] ^= 1)),
        ("terminal sequence", Box::new(|b: &mut [u8]| b[111] = 5)),
        ("verdict", Box::new(|b: &mut [u8]| b[112] = 3)),
        ("presence byte", Box::new(|b: &mut [u8]| b[113] = 2)),
        ("resolved_by", Box::new(|b: &mut [u8]| b[117] = 1)),
    ];
    for (label, edit) in &edits {
        let mut changed = image.clone();
        edit(&mut changed[BLOCK..2 * BLOCK]);
        redigest(&mut changed[BLOCK..2 * BLOCK]);
        let report = classify_bytes(&changed, ROOT, 16, 0);
        assert!(
            matches!(report.seal, Some(SealParse::Malformed(_)))
                && report.class == FileClass::MalformedJournal,
            "[exact-bytes] seal {label}: {:?} {:?}",
            report.seal,
            report.class
        );
    }
    let mut tail = image.clone();
    tail[BLOCK + 3000] = 1;
    let report = classify_bytes(&tail, ROOT, 16, 0);
    assert!(
        matches!(report.seal, Some(SealParse::Malformed(_))),
        "[exact-bytes] seal tail bytes"
    );
    // A seal on a started run without RunEnded, or with unsettled entries.
    let partial: Vec<K> = kinds[..4].to_vec();
    let frames = frames_of(header.generation, &partial);
    let seal = seal_of(&header, &partial);
    let report = classify_bytes(&journal_image(&header, &frames, Some(&seal)), ROOT, 16, 0);
    assert_eq!(
        report.class,
        FileClass::MalformedJournal,
        "[exact-bytes] a sealed run must have ended"
    );
    let parsed = match parse_header(
        &encode_header(&header).expect("header")[..],
        &ROOT,
        None,
        None,
    ) {
        HeaderParse::Valid(parsed) => parsed,
        other => panic!("{other:?}"),
    };
    let facts = PrefixFacts {
        records: 0,
        last_digest: [0; 32],
        run_ended: None,
        run_started: false,
        unsettled: 1,
    };
    let seal = encode_seal(&seal_for(&parsed, &facts, 0));
    assert!(
        matches!(
            parse_seal(&seal[..], &parsed, &facts),
            SealParse::Malformed(_)
        ),
        "[exact-bytes] unsettled entries"
    );
}

/// Record blocks: the version-1 frame unchanged, a payload length in the
/// record range, exactly the bytes the codec accepts, then zeros.
#[test]
fn f03_record_blocks_take_exactly_one_frame() {
    let frame = frame(GR, 1, 0, Rs(0));
    let mut block = vec![0u8; BLOCK];
    block[..frame.len()].copy_from_slice(&frame);
    assert!(
        matches!(parse_record_block(&block), RecordBlock::Valid(_)),
        "[exact-bytes] valid block"
    );
    let mut padding = block.clone();
    padding[frame.len() + 5] = 1;
    assert!(
        matches!(parse_record_block(&padding), RecordBlock::Invalid(_)),
        "[exact-bytes] padding"
    );
    for length in [32u16, 84, 0, 4056] {
        let mut changed = block.clone();
        changed[6..8].copy_from_slice(&length.to_be_bytes());
        assert!(
            matches!(parse_record_block(&changed), RecordBlock::Invalid(_)),
            "[exact-bytes] length {length}"
        );
    }
    let mut digest = block.clone();
    digest[frame.len() - 1] ^= 1;
    assert!(
        matches!(parse_record_block(&digest), RecordBlock::Invalid(_)),
        "[exact-bytes] digest"
    );
    let mut domain = block.clone();
    domain[4] = 0x51;
    assert!(
        matches!(parse_record_block(&domain), RecordBlock::Invalid(_)),
        "[exact-bytes] domain"
    );
    assert_eq!(parse_record_block(&vec![0u8; BLOCK]), RecordBlock::Zero);
}

fn sample_provision() -> format::Provision {
    format::Provision {
        uid: 1001,
        gid: 1001,
        root_id: ROOT,
        state_root: "/var/lib/nexus-os/phase2-custody/roots/uid-1001-x".into(),
        root_inode: 11,
        lock_inode: 12,
        journals_inode: 13,
        dispositions_inode: 14,
        revoked_inode: 15,
        archive_inode: 16,
        mount_options: "rw,relatime".into(),
        super_options: "rw,errors=remount-ro".into(),
        device_logical_block_size: 512,
        device_physical_block_size: 4096,
        kernel: "6.17.0-fixture #1 SMP PREEMPT_DYNAMIC".into(),
        storage_pci_function: "0000:3d:00.0".into(),
        storage_partition: Some(2),
        storage_identity: [0xab; 32],
        storage_attestation: "fixture statement".into(),
        c_pool: 16,
        pool_inodes: vec![21, 22],
        retired_through: 0,
        predecessor: None,
        operator: "owner".into(),
        created: "2026-10-01T00:00:00Z".into(),
        revision: 1,
    }
}

fn redigest_provision(text: &str) -> Vec<u8> {
    let body = &text[..text.rfind("digest=").expect("digest line")];
    let digest = hexs(&sha256(body.as_bytes()));
    format!("{body}digest={digest}\n").into_bytes()
}

/// `PROVISION`: exact keys in exact order, canonical values, the storage
/// fields required, the digest of every preceding byte.
#[test]
fn f04_provision_grammar_is_exact() {
    let provision = sample_provision();
    let bytes = render_provision(&provision).expect("renders");
    assert_eq!(
        parse_provision(&bytes).expect("parses"),
        provision,
        "[exact-bytes] round trip"
    );
    let text = String::from_utf8(bytes.clone()).expect("ascii");
    let keys = format::provision_keys(2);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), keys.len() + 1);
    for (line, key) in lines[1..].iter().zip(&keys) {
        assert!(
            line.starts_with(&format!("{key}=")),
            "[exact-bytes] key order: {line}"
        );
    }
    let mutations: Vec<(&str, String)> = vec![
        ("leading zero", text.replace("uid=1001", "uid=01001")),
        ("plus sign", text.replace("gid=1001", "gid=+1001")),
        (
            "uppercase hex",
            text.replace(&hexs(&ROOT), &hexs(&ROOT).to_uppercase()),
        ),
        (
            "fs type",
            text.replace("mount-fstype=ext4", "mount-fstype=ext3"),
        ),
        (
            "block size",
            text.replace("fs-block-size=4096", "fs-block-size=1024"),
        ),
        (
            "device block size",
            text.replace(
                "device-logical-block-size=512",
                "device-logical-block-size=3000",
            ),
        ),
        (
            "storage class",
            text.replace("storage-class=nvme-pcie", "storage-class=sata"),
        ),
        ("PCI function", text.replace("0000:3d:00.0", "0000:3d:00.8")),
        (
            "partition zero",
            text.replace("storage-partition=2", "storage-partition=0"),
        ),
        (
            "partition 256",
            text.replace("storage-partition=2", "storage-partition=256"),
        ),
        (
            "identity length",
            text.replace(&hexs(&[0xab; 32]), &hexs(&[0xab; 31])),
        ),
        ("path component", text.replace("/roots/", "/../")),
        (
            "statement space",
            text.replace(
                "storage-attestation=fixture statement",
                "storage-attestation= fixture",
            ),
        ),
        (
            "timestamp",
            text.replace("2026-10-01T00:00:00Z", "2026-02-30T00:00:00Z"),
        ),
        (
            "timestamp range",
            text.replace("2026-10-01T00:00:00Z", "1969-12-31T23:59:59Z"),
        ),
        ("revision zero", text.replace("revision=1", "revision=0")),
        (
            "predecessor statement without predecessor",
            text.replace("predecessor-statement=none", "predecessor-statement=x"),
        ),
        ("blank line", text.replacen("\n", "\n\n", 1)),
        ("CR", text.replacen("\n", "\r\n", 1)),
        (
            "key order",
            text.replacen("uid=1001\ngid=1001", "gid=1001\nuid=1001", 1),
        ),
        (
            "unknown key",
            text.replacen("uid=1001", "uid=1001\nextra=1", 1),
        ),
        (
            "missing storage fields (an R4 PROVISION)",
            text.lines()
                .filter(|line| !line.starts_with("storage-"))
                .map(|line| format!("{line}\n"))
                .collect(),
        ),
    ];
    for (label, mutated) in mutations {
        let redigested = redigest_provision(&mutated);
        assert!(
            parse_provision(&redigested).is_err(),
            "[exact-bytes] PROVISION {label} accepted"
        );
    }
    let mut digest = bytes.clone();
    let at = digest.len() - 5;
    digest[at] = if digest[at] == b'0' { b'1' } else { b'0' };
    assert!(
        parse_provision(&digest).is_err(),
        "[exact-bytes] digest mismatch"
    );
    let mut missing_newline = bytes.clone();
    missing_newline.pop();
    assert!(
        parse_provision(&missing_newline).is_err(),
        "[exact-bytes] final newline"
    );
    let mut nul = bytes.clone();
    nul[10] = 0;
    assert!(parse_provision(&nul).is_err(), "[exact-bytes] NUL");
    let oversize = vec![b'a'; format::PROVISION_LIMIT + 1];
    assert!(
        parse_provision(&oversize).is_err(),
        "[exact-bytes] oversize"
    );
    let mut trailing = bytes.clone();
    trailing.extend_from_slice(b"x\n");
    assert!(
        parse_provision(&trailing).is_err(),
        "[exact-bytes] trailing data"
    );
    for (value, accepted) in [
        ("0", true),
        ("10", true),
        ("01", false),
        ("00", false),
        ("+1", false),
    ] {
        assert_eq!(
            format::decimal(value, 0, 100).is_ok(),
            accepted,
            "[exact-bytes] canonical decimal {value:?}"
        );
    }
}

fn sample_disposition(kind: IncidentKind) -> Disposition {
    let facts = match kind {
        IncidentKind::Journal => IncidentFacts {
            kind,
            claim: Some(3),
            generation: Some([0x63; 16]),
            pool_index: None,
            content: Some([0x64; 32]),
            class: IncidentClass::Unresolved,
            recorded_unsettled: 2,
        },
        IncidentKind::ClaimGap => IncidentFacts {
            kind,
            claim: Some(4),
            generation: None,
            pool_index: None,
            content: None,
            class: IncidentClass::Malformed,
            recorded_unsettled: 0,
        },
        IncidentKind::PoolFile => IncidentFacts {
            kind,
            claim: None,
            generation: None,
            pool_index: Some(1),
            content: Some([0x65; 32]),
            class: IncidentClass::Malformed,
            recorded_unsettled: 0,
        },
    };
    Disposition {
        root: ROOT,
        binding: facts.binding(&ROOT).expect("binding"),
        facts,
        reason: DispositionReason::OwnerDestroyed,
        statement: "the owner process was destroyed".into(),
        operator: "owner".into(),
        at: "2026-10-01T12:00:00Z".into(),
    }
}

/// Dispositions: exact keys, kind-dependent field shapes, and a binding that
/// must recompute from the fields.
#[test]
fn f05_disposition_grammar_is_exact_and_bound() {
    for kind in [
        IncidentKind::Journal,
        IncidentKind::ClaimGap,
        IncidentKind::PoolFile,
    ] {
        let disposition = sample_disposition(kind);
        let bytes = render_disposition(&disposition).expect("renders");
        assert_eq!(
            parse_disposition(&bytes).expect("parses"),
            disposition,
            "[exact-bytes] {kind:?}"
        );
    }
    let text = String::from_utf8(
        render_disposition(&sample_disposition(IncidentKind::Journal)).expect("renders"),
    )
    .expect("ascii");
    let gap = String::from_utf8(
        render_disposition(&sample_disposition(IncidentKind::ClaimGap)).expect("renders"),
    )
    .expect("ascii");
    for (label, mutated) in [
        (
            "binding does not recompute",
            text.replace("claim=3", "claim=4"),
        ),
        (
            "journal pool-index",
            text.replace("pool-index=none", "pool-index=1"),
        ),
        (
            "gap generation",
            gap.replace("generation=none", &format!("generation={}", hexs(&[1; 16]))),
        ),
        (
            "gap unsettled",
            gap.replace("recorded-unsettled=0", "recorded-unsettled=1"),
        ),
        (
            "gap class",
            gap.replace("class=malformed", "class=unresolved"),
        ),
        (
            "reason",
            text.replace("reason=owner-destroyed", "reason=destroyed"),
        ),
        ("kind", text.replace("kind=journal", "kind=journals")),
        (
            "operator too long",
            text.replace("operator=owner", &format!("operator={}", "o".repeat(65))),
        ),
    ] {
        assert!(
            parse_disposition(mutated.as_bytes()).is_err(),
            "[exact-bytes] disposition {label} accepted"
        );
    }
    assert!(parse_disposition(&vec![b'a'; format::DISPOSITION_LIMIT + 1]).is_err());
    // The recorded-unsettled count is not part of the binding: it parses,
    // and it is the validator that requires it to restate the incident.
    let restated = parse_disposition(
        text.replace("recorded-unsettled=2", "recorded-unsettled=3")
            .as_bytes(),
    )
    .expect("the count is a field, not part of the binding");
    let original = sample_disposition(IncidentKind::Journal);
    let incident = Incident {
        facts: original.facts.clone(),
        outcome: PriorOutcome::Unresolved { outstanding: 2 },
    };
    assert!(classify::disposition_matches(Some(&original), &incident));
    assert!(
        !classify::disposition_matches(Some(&restated), &incident),
        "[disposition-exact] another count does not restate the incident"
    );
}

/// Entry names and archive names (design section 7.5).
#[test]
fn f06_entry_names_are_exact() {
    assert_eq!(format::pool_name(7), "j00007.journal");
    assert_eq!(format::parse_pool_name("j00007.journal", 8), Some(7));
    for name in [
        "j00008.journal",
        "j0007.journal",
        "j000007.journal",
        "j00007.journal.tmp",
        "J00007.journal",
    ] {
        assert_eq!(
            format::parse_pool_name(name, 8),
            None,
            "[exact-bytes] {name}"
        );
    }
    let binding = [0x5c; 32];
    assert_eq!(
        format::parse_disposition_name(&format::disposition_name(&binding)),
        Some(binding)
    );
    assert_eq!(format::parse_disposition_name(".tmp-x.disposition"), None);
    let revoked = format::revoked_name(&binding, "20261001T120000Z");
    assert!(format::parse_revoked_name(&revoked).is_some());
    assert!(
        format::parse_revoked_name(&format::revoked_name(&binding, "20261301T120000Z")).is_none(),
        "[exact-bytes] month 13"
    );
    let journal = ArchiveName::Journal {
        claim: 9,
        generation: [1; 16],
        content: [2; 32],
        class: ArchiveClass::Unresolved,
    };
    assert_eq!(ArchiveName::parse(&journal.render()), Some(journal.clone()));
    let pool = ArchiveName::PoolFile {
        index: 3,
        content: [4; 32],
    };
    assert_eq!(ArchiveName::parse(&pool.render()), Some(pool));
    for name in [
        journal.render().replace("j-9-", "j-09-"),
        journal.render().replace("j-9-", "j-0-"),
        journal.render().replace("-u.journal", "-x.journal"),
        format!("p-01024-{}.journal", hexs(&[4; 32])),
        format!("p-1-{}.journal", hexs(&[4; 32])),
    ] {
        assert_eq!(
            ArchiveName::parse(&name),
            None,
            "[exact-bytes] archive name {name}"
        );
    }
}

/// Bindings are content-addressed and independent of location; any byte
/// change makes a new binding (INV-7); the kinds never collide.
#[test]
fn f07_bindings_are_content_addressed() {
    let independent = |kind: u8, fields: &[&[u8]]| -> [u8; 32] {
        let mut data = b"nexus-phase2-custody-incident\x00\x01".to_vec();
        data.push(kind);
        for field in fields {
            data.extend_from_slice(field);
        }
        Sha256::digest(&data).into()
    };
    let generation = [0x66; 16];
    let content = [0x67; 32];
    assert_eq!(
        format::bind_journal(&ROOT, 3, &generation, &content, IncidentClass::Unresolved),
        independent(
            1,
            &[&ROOT, &3u64.to_be_bytes(), &generation, &content, &[1]]
        ),
        "[archive-binding] journal binding bytes"
    );
    assert_eq!(
        format::bind_gap(&ROOT, 4),
        independent(2, &[&ROOT, &4u64.to_be_bytes()]),
        "[archive-binding] gap"
    );
    assert_eq!(
        format::bind_pool(&ROOT, 5, &content),
        independent(3, &[&ROOT, &5u32.to_be_bytes(), &content]),
        "[archive-binding] pool"
    );
    // The same bytes in another pool file keep their journal binding.
    let header0 = header_fields(generation, 3, 0, 16, 12);
    let header1 = header_fields(generation, 3, 1, 16, 12);
    let kinds = trace("T3")[..3].to_vec();
    let a = classify_bytes(
        &journal_image(&header0, &frames_of(generation, &kinds), None),
        ROOT,
        16,
        0,
    );
    let b = classify_bytes(
        &journal_image(&header1, &frames_of(generation, &kinds), None),
        ROOT,
        16,
        1,
    );
    assert_eq!(a.class, FileClass::UnsealedAction);
    let bind = |report: &classify::FileReport| {
        let header = report.header.as_ref().expect("header");
        format::bind_journal(
            &ROOT,
            header.claim,
            &header.generation,
            &report.content,
            IncidentClass::Unresolved,
        )
    };
    assert_ne!(
        a.content, b.content,
        "the pool index is in the header bytes"
    );
    let copy = classify_bytes(
        &journal_image(&header0, &frames_of(generation, &kinds), None),
        ROOT,
        16,
        0,
    );
    assert_eq!(
        bind(&a),
        bind(&copy),
        "[archive-binding] byte-identical copies keep the binding"
    );
    let mut header_changed = header0.clone();
    header_changed.created_ms += 1;
    let other = classify_bytes(
        &journal_image(&header_changed, &frames_of(generation, &kinds), None),
        ROOT,
        16,
        0,
    );
    assert_eq!(other.class, FileClass::UnsealedAction);
    assert_ne!(
        bind(&other),
        bind(&a),
        "[archive-binding] any byte change makes a new binding"
    );
    let all = [
        format::bind_journal(&ROOT, 1, &generation, &content, IncidentClass::Unresolved),
        format::bind_journal(&ROOT, 1, &generation, &content, IncidentClass::Malformed),
        format::bind_gap(&ROOT, 1),
        format::bind_pool(&ROOT, 1, &content),
    ];
    assert_eq!(
        all.iter().collect::<BTreeSet<_>>().len(),
        4,
        "[archive-binding] kinds never collide"
    );
}

/// The storage identity record (design section 6.4 step 12).
#[test]
fn f08_storage_identity_record_is_exact() {
    let attributes: [&[u8]; 4] = [b"Model  \n", b"SERIAL\n", b"FW1 \n", b"eui.1\n"];
    let mut record = b"nexus-phase2-custody-storage\x00\x01".to_vec();
    for value in attributes {
        record.extend_from_slice(&(value.len() as u16).to_be_bytes());
        record.extend_from_slice(value);
    }
    let expected: [u8; 32] = Sha256::digest(&record).into();
    assert_eq!(
        format::storage_identity(attributes),
        Some(expected),
        "[storage-qualification] identity record"
    );
    let changed: [&[u8]; 4] = [b"Model \n", b"SERIAL\n", b"FW1 \n", b"eui.1\n"];
    assert_ne!(
        format::storage_identity(changed),
        Some(expected),
        "[storage-qualification] trailing spaces are kept"
    );
    let big = vec![b'a'; format::STORAGE_READ_LIMIT + 1];
    assert_eq!(format::storage_identity([&big, b"", b"", b""]), None);
}

/// The store's disposition validator (design section 13.4) accepts only a
/// root-owned disposition that restates exactly the incident the store
/// computed, under that incident's binding, and returns that file's reason:
/// another count or another root is no disposition.
#[test]
fn f09_the_validator_accepts_only_an_exact_restatement() {
    use custody::store::disposition::StoreValidator;
    let sample = sample_disposition(IncidentKind::Journal);
    let binding = sample.facts.binding(&ROOT).expect("a journal binding");
    let incident = Incident {
        facts: sample.facts.clone(),
        outcome: PriorOutcome::Unresolved { outstanding: 2 },
    };
    let scan_with = |presented: Disposition| {
        let mut level = classify::PoolLevel::default();
        level.incidents.insert(binding, incident.clone());
        let mut dispositions = BTreeMap::new();
        dispositions.insert(binding, presented);
        open::ScanResult {
            files: Vec::new(),
            archive: Vec::new(),
            dispositions,
            revoked: Vec::new(),
            report: classify::bounded_report(&level),
            level,
            generations: BTreeSet::new(),
            preservation_sync_returned_zero: false,
        }
    };
    let key = IncidentBinding::new(binding);
    let exact = Disposition {
        root: ROOT,
        binding,
        ..sample
    };
    assert_eq!(
        StoreValidator::new(&ROOT, &scan_with(exact.clone())).validate(&key),
        Some(ValidatedDisposition::new(key, exact.reason)),
        "the exact restatement"
    );
    let mut recount = exact.clone();
    recount.facts.recorded_unsettled = 3;
    let mut other_root = exact.clone();
    other_root.root = [0x99; 16];
    for (label, presented) in [("another count", recount), ("another root", other_root)] {
        assert_eq!(
            StoreValidator::new(&ROOT, &scan_with(presented)).validate(&key),
            None,
            "[disposition-exact] {label}"
        );
    }
}

// ---------------------------------------------------------------------------
// G: the record grammar (design section 11.4)
// ---------------------------------------------------------------------------

/// Every source-derived fixture and every prefix of each is accepted; an
/// instant regression and 31 single-rule mutations are each rejected by the
/// rule they break.
#[test]
fn g01_grammar_fixtures_prefixes_and_mutations() {
    let mut prefixes = 0;
    for (name, kinds) in traces() {
        let recs = records(&kinds, None);
        for end in 0..=recs.len() {
            let (violation, _) = classify::check_grammar(&recs[..end], Generation::new(GR), 0);
            assert_eq!(
                violation, None,
                "[grammar-conformance] {name} prefix {end} rejected"
            );
            prefixes += 1;
        }
    }
    assert!(prefixes > 100);
    let regress = records(&trace("T1"), Some(&[0, 1, 2, 3, 2, 5]));
    assert_eq!(
        classify::check_grammar(&regress, Generation::new(GR), 0)
            .0
            .map(|(rule, _)| rule),
        Some(Rule::G1),
        "[grammar-conformance] instants"
    );
    let mutations = grammar_mutations();
    assert_eq!(mutations.len(), 31);
    for (label, kinds, rule) in mutations {
        let recs = records(&kinds, None);
        let (violation, _) = classify::check_grammar(&recs, Generation::new(GR), 0);
        assert_eq!(
            violation.map(|(found, _)| found),
            Some(rule),
            "[grammar-conformance] mutation '{label}'"
        );
    }
}

// ---------------------------------------------------------------------------
// C: classification, the report and the refusal (design sections 11.1-11.3)
// ---------------------------------------------------------------------------

/// Each class from its bytes, and the refusal rule: an unsealed generation
/// with an action start refuses whatever its recorded-unsettled count, and a
/// visible `RunEnded` is never resolution.
#[test]
fn c01_classes_and_the_conservative_refusal() {
    let generation = [0x71; 16];
    let header = header_fields(generation, 2, 0, 16, 12);
    let unused = classify_bytes(&vec![0u8; 18 * BLOCK], ROOT, 16, 0);
    assert_eq!(unused.class, FileClass::Unused);
    assert!(!unused.refused && !unused.native_work_possible);
    let empty = classify_bytes(&journal_image(&header, &[], None), ROOT, 16, 0);
    assert_eq!(
        empty.class,
        FileClass::UnsealedNoAction,
        "[no-false-resolution] header only"
    );
    assert!(!empty.refused && !empty.native_work_possible);
    let control_only = classify_bytes(
        &journal_image(&header, &frames_of(generation, &trace("T2")), None),
        ROOT,
        16,
        0,
    );
    assert_eq!(control_only.class, FileClass::UnsealedNoAction);
    // A complete, passed, unsealed run: RunEnded visible, nothing unsettled.
    let t1 = trace("T1");
    let ended = classify_bytes(
        &journal_image(&header, &frames_of(generation, &t1), None),
        ROOT,
        16,
        0,
    );
    assert_eq!(
        ended.class,
        FileClass::UnsealedAction,
        "[no-false-resolution] a visible RunEnded is not resolution"
    );
    assert!(ended.recorded_unsettled.is_empty());
    assert!(
        ended.refused,
        "[no-false-resolution] a visible RunEnded is not resolution"
    );
    assert!(
        ended
            .notes
            .iter()
            .any(|note| note.contains("no durable record")),
        "[late-uncertain] the late-owner note"
    );
    // An action started, nothing unsettled recorded beyond it.
    let started = classify_bytes(
        &journal_image(&header, &frames_of(generation, &t1[..3]), None),
        ROOT,
        16,
        0,
    );
    assert_eq!(
        started.recorded_unsettled,
        vec![classify::Unsettled::Action(1)]
    );
    assert!(started.refused && started.native_work_possible && !started.evidence_complete);
    // A hole, a block beyond the capacity, an invalid block, an identity mismatch.
    let frames = frames_of(generation, &t1);
    let mut hole = journal_image(&header, &frames, None);
    hole[3 * BLOCK..4 * BLOCK].fill(0);
    assert_eq!(
        classify_bytes(&hole, ROOT, 16, 0).class,
        FileClass::MalformedJournal,
        "[exact-bytes] hole"
    );
    let mut beyond = journal_image(&header, &frames, None);
    beyond[15 * BLOCK] = 1;
    let report = classify_bytes(&beyond, ROOT, 16, 0);
    assert_eq!(
        report.class,
        FileClass::MalformedJournal,
        "[exact-bytes] beyond capacity"
    );
    assert!(report.reason.contains("beyond capacity"));
    let mut torn = journal_image(&header, &frames, None);
    torn[2 * BLOCK + 50] ^= 1;
    assert_eq!(
        classify_bytes(&torn, ROOT, 16, 0).class,
        FileClass::MalformedJournal,
        "[tear-containment] a torn record is never benign"
    );
    let foreign = journal_image(&header, &frames_of([0x72; 16], &t1), None);
    assert_eq!(
        classify_bytes(&foreign, ROOT, 16, 0).class,
        FileClass::MalformedJournal,
        "[exact-bytes] foreign records"
    );
    let swapped = {
        let mut frames = frames.clone();
        frames.swap(0, 1);
        journal_image(&header, &frames, None)
    };
    assert_eq!(
        classify_bytes(&swapped, ROOT, 16, 0).class,
        FileClass::MalformedJournal,
        "[exact-bytes] sequence"
    );
    let grammar = journal_image(
        &header,
        &frames_of(generation, &grammar_mutations()[0].1),
        None,
    );
    let report = classify_bytes(&grammar, ROOT, 16, 0);
    assert_eq!(report.class, FileClass::MalformedJournal);
    assert!(report.reason.starts_with("grammar"));
    // Malformed pool files: a zero header with records, a wrong root.
    let mut headless = journal_image(&header, &frames, None);
    headless[..BLOCK].fill(0);
    assert_eq!(
        classify_bytes(&headless, ROOT, 16, 0).class,
        FileClass::MalformedPoolFile
    );
    let wrong_root = classify_bytes(&journal_image(&header, &frames, None), [0x99; 16], 16, 0);
    assert_eq!(
        wrong_root.class,
        FileClass::MalformedPoolFile,
        "[exact-bytes] another root's header"
    );
    for class in [FileClass::MalformedJournal, FileClass::MalformedPoolFile] {
        assert!(
            classify::refused(class),
            "[no-false-resolution] malformed refuses"
        );
    }
    // Size.
    assert_eq!(
        classify_bytes(&vec![0u8; 17 * BLOCK], ROOT, 16, 0).class,
        FileClass::SizeInvalid
    );
}

/// Determinism: equal bytes give equal classes, reports and bindings.
#[test]
fn c02_classification_is_deterministic() {
    let generation = [0x73; 16];
    let header = header_fields(generation, 2, 0, 16, 12);
    for (_, kinds) in traces() {
        let image = journal_image(&header, &frames_of(generation, &kinds), None);
        let a = classify_bytes(&image, ROOT, 16, 0);
        let b = classify_bytes(&image.clone(), ROOT, 16, 0);
        assert_eq!(a, b, "[archive-binding] determinism");
    }
}

fn level_of(
    files: &[classify::FileReport],
    archive: &[classify::ArchiveEntry],
    dispositions: &BTreeMap<[u8; 32], Disposition>,
    retired: u64,
    limit: usize,
) -> Result<classify::PoolLevel, classify::PoolRefusal> {
    classify::pool_level(
        classify::PoolFacts {
            root_id: ROOT,
            c_pool: 16,
            retired_through: retired,
        },
        files,
        archive,
        dispositions,
        limit,
    )
}

/// Pool-level checks: claim gaps, duplicates, retirement, the arithmetic
/// gap bound before enumeration, history by applied dispositions, and the
/// bounded report that never decides.
#[test]
fn c03_pool_level_checks_and_bounded_reports() {
    let image = |generation: [u8; 16], claim: u64, index: u32, kinds: &[K]| {
        classify_bytes(
            &journal_image(
                &header_fields(generation, claim, index, 16, 12),
                &frames_of(generation, kinds),
                None,
            ),
            ROOT,
            16,
            index,
        )
    };
    let t1 = trace("T1");
    let a = image([1; 16], 1, 0, &t1);
    let c = image([3; 16], 3, 1, &t1);
    let level = level_of(&[a.clone(), c.clone()], &[], &BTreeMap::new(), 0, 4).expect("level");
    let gaps: Vec<&Incident> = level
        .incidents
        .values()
        .filter(|incident| incident.facts.kind == IncidentKind::ClaimGap)
        .collect();
    assert_eq!(gaps.len(), 1, "[arith-bounds] claim 2 is a gap");
    assert_eq!(gaps[0].facts.claim, Some(2));
    assert_eq!(level.max_claim, 3);
    let duplicate = image([4; 16], 1, 1, &t1);
    assert!(
        matches!(
            level_of(&[a.clone(), duplicate], &[], &BTreeMap::new(), 0, 4),
            Err(classify::PoolRefusal::Invalid(_))
        ),
        "[arith-bounds] duplicate claim"
    );
    let same_generation = image([1; 16], 2, 1, &t1);
    assert!(
        matches!(
            level_of(&[a.clone(), same_generation], &[], &BTreeMap::new(), 0, 4),
            Err(classify::PoolRefusal::Invalid(_))
        ),
        "duplicate generation"
    );
    assert!(
        matches!(
            level_of(std::slice::from_ref(&a), &[], &BTreeMap::new(), 1, 4),
            Err(classify::PoolRefusal::Invalid(_))
        ),
        "a pool claim at or below retired-through"
    );
    // A huge claim refuses as Capacity, arithmetically, without enumerating.
    let huge = image([5; 16], 1 << 62, 0, &t1);
    let started = std::time::Instant::now();
    assert!(
        matches!(
            level_of(&[huge], &[], &BTreeMap::new(), 0, 4),
            Err(classify::PoolRefusal::Capacity(_))
        ),
        "[arith-bounds] gap capacity"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "[arith-bounds] the gap count was enumerated"
    );
    // History: a header that applied a disposition still in force.
    let binding = format::bind_journal(&ROOT, 1, &[1; 16], &a.content, IncidentClass::Unresolved);
    let incident_facts = level
        .incidents
        .get(&binding)
        .expect("a's incident")
        .facts
        .clone();
    let mut dispositions = BTreeMap::new();
    dispositions.insert(
        binding,
        Disposition {
            root: ROOT,
            binding,
            facts: incident_facts,
            reason: DispositionReason::HostRebooted,
            statement: "x".into(),
            operator: "o".into(),
            at: "2026-10-01T00:00:00Z".into(),
        },
    );
    let mut applied_header = header_fields([6; 16], 4, 2, 16, 12);
    applied_header.applied = vec![(binding, DispositionReason::HostRebooted)];
    let applier = classify_bytes(&journal_image(&applied_header, &[], None), ROOT, 16, 2);
    let level =
        level_of(&[a.clone(), c.clone(), applier], &[], &dispositions, 0, 4).expect("level");
    assert!(
        level.history.contains(&binding),
        "applied and still dispositioned: history"
    );
    let current = classify::current_incidents(&level);
    assert!(current.iter().all(|(b, _)| *b != binding));
    // The bounded report: 70 incidents, 64 listed, partial, exact totals.
    let mut many = classify::PoolLevel::default();
    for claim in 1..=70u64 {
        many.incidents.insert(
            format::bind_gap(&ROOT, claim),
            Incident {
                facts: custody::store::maintenance::gap_facts(claim),
                outcome: PriorOutcome::Malformed,
            },
        );
    }
    let report = classify::bounded_report(&many);
    assert_eq!(
        (
            report.detail.len(),
            report.total,
            report.current,
            report.partial
        ),
        (64, 70, 70, true),
        "[aggregate-bounds] report"
    );
    assert!(
        matches!(
            classify::decide(&many, &BTreeMap::new(), 32),
            Err(classify::PoolRefusal::Capacity(_))
        ),
        "[aggregate-bounds] decision from the complete set"
    );
    // Dispositions for exactly the 64 incidents the report details: the
    // decision, over the complete set, still blocks on the other six.
    let dispositions: BTreeMap<[u8; 32], Disposition> = report
        .detail
        .iter()
        .map(|(binding, incident)| {
            (
                *binding,
                Disposition {
                    root: ROOT,
                    binding: *binding,
                    facts: incident.facts.clone(),
                    reason: DispositionReason::Other,
                    statement: "x".into(),
                    operator: "o".into(),
                    at: "2026-10-01T00:00:00Z".into(),
                },
            )
        })
        .collect();
    let decision = classify::decide(&many, &dispositions, 100).expect("a decision");
    assert_eq!(
        (decision.dispositioned.len(), decision.blocking.len()),
        (64, 6),
        "[aggregate-bounds] decision from the complete set, never from the report"
    );
}

/// Claims are 1 to 2^63 and never wrap (design section 6.6): a store whose
/// highest claim is 2^63 (with every lower claim retired) refuses the next
/// claim before writing anything.
#[test]
fn c04_the_claim_counter_never_wraps() {
    let fixture = Fixture::provisioned(2, 16);
    let session = session(&fixture, false);
    let mut procedure = maint::rewrite_provision(Rc::clone(&session), |provision| {
        provision.retired_through = format::MAX_CLAIM - 1;
    });
    procedure.run_all(&mut quiet()).expect("rewritten");
    drop(procedure);
    end(session);
    let t1 = trace("T1");
    let fields = header_fields([0x63; 16], format::MAX_CLAIM, 0, 16, 12);
    let seal = seal_of(&fields, &t1);
    let image = journal_image(&fields, &frames_of(fields.generation, &t1), Some(&seal));
    assert_eq!(
        classify_bytes(&image, ROOT, 16, 0).class,
        FileClass::Sealed,
        "a sealed generation at claim 2^63"
    );
    fixture
        .world
        .install(fixture.pool_ino(0).expect("pool"), &image);
    let refused = refused_store(
        Run::start(&fixture, "next", SMALL),
        "[arith-bounds] the claim after 2^63",
    );
    assert_eq!(
        refused,
        Refused::ClaimExhausted,
        "[arith-bounds] the claim after 2^63"
    );
    let other = fixture.pool_ino(1).expect("pool");
    assert!(
        fixture.world.visible(other).iter().all(|byte| *byte == 0),
        "[arith-bounds] nothing was claimed"
    );
}

use classify::Incident;

// ---------------------------------------------------------------------------
// S: the simulated storage's state model (design sections 4 and 4.8)
// ---------------------------------------------------------------------------

/// A plain file in a fresh world, written by `writer` (not synced).
fn scratch_file(
    world: &SimWorld,
    writer: &SimIo,
    name: &str,
    blocks: usize,
) -> (sim::Ino, sim::SimFile) {
    let root = writer.root_dir().expect("root");
    let file = writer.create_exclusive(&root, name, 0o600).expect("create");
    writer
        .allocate(&file, (blocks * BLOCK) as u64)
        .expect("allocate");
    writer.fdatasync(&file).expect("sync zeros");
    let ino = writer.file_ino(&file).expect("ino");
    let _ = world;
    (ino, file)
}

/// Process death makes nothing durable; power loss makes the durable bytes
/// visible; a sync after restart (through a new description) and before the
/// power loss changes the outcome.
#[test]
fn s01_process_death_and_power_loss_keep_states_apart() {
    let world = SimWorld::new(HostFixture::qualified());
    let writer = world.process("writer", 0, 0);
    let (ino, file) = scratch_file(&world, &writer, "f", 4);
    writer
        .pwrite(&file, 2 * BLOCK as u64, &[7u8; BLOCK])
        .expect("write");
    world.kill(&writer);
    assert_eq!(
        world.visible(ino)[2 * BLOCK],
        7,
        "[F1-volatile] visible after process death"
    );
    assert_eq!(
        world.durable(ino)[2 * BLOCK],
        0,
        "[F1-volatile] process death made the write durable"
    );
    let lost = world.fork();
    lost.power_loss(None, sim::Tear::Old, &BTreeMap::new());
    assert_eq!(
        lost.visible(ino)[2 * BLOCK],
        0,
        "[F1-F2-sync] F1, restart, F2 before any sync loses it"
    );
    let restart = world.process("restart", 0, 0);
    let root = restart.root_dir().expect("root");
    let again = restart.open_read(&root, "f").expect("open");
    restart.fdatasync(&again).expect("the preservation sync");
    world.power_loss(None, sim::Tear::Old, &BTreeMap::new());
    assert_eq!(
        world.visible(ino)[2 * BLOCK],
        7,
        "[F1-F2-sync] a sync after restart keeps it"
    );
}

/// A writeback error is reported once per open description: a reopened
/// description's sync returns 0 though the durable bytes lack the write,
/// and inode eviction drops the visible bytes too; an error nobody saw is
/// reported to a new description.
#[test]
fn s02_writeback_errors_are_not_repaired_by_reopening() {
    let world = SimWorld::new(HostFixture::qualified());
    let writer = world.process("writer", 0, 0);
    let (ino, file) = scratch_file(&world, &writer, "f", 4);
    world.fail_writeback(ino, 2);
    writer
        .pwrite(&file, 2 * BLOCK as u64, &[7u8; BLOCK])
        .expect("write");
    let error = writer
        .fdatasync(&file)
        .expect_err("[errseq-reopen] the writer sees EIO");
    assert_eq!(error.errno, custody::store::io::Errno::Io);
    world.kill(&writer);
    let reader = world.process("reader", 0, 0);
    let root = reader.root_dir().expect("root");
    let fresh = reader.open_read(&root, "f").expect("open");
    assert!(
        reader.fdatasync(&fresh).is_ok(),
        "[errseq-reopen] a new description does not see an error already seen"
    );
    assert_eq!(
        world.visible(ino)[2 * BLOCK],
        7,
        "the failed page is clean but still visible"
    );
    assert_eq!(
        world.durable(ino)[2 * BLOCK],
        0,
        "[errseq-reopen] the durable bytes lack the write"
    );
    drop(fresh);
    world.kill(&reader);
    assert!(
        world.evict_inode(ino),
        "no description open, nothing pending"
    );
    assert_eq!(
        world.visible(ino)[2 * BLOCK],
        0,
        "[errseq-reopen] eviction drops the visible bytes"
    );
    // An error nobody saw: the writer dies first.
    let world = SimWorld::new(HostFixture::qualified());
    let writer = world.process("writer", 0, 0);
    let (ino, file) = scratch_file(&world, &writer, "f", 4);
    world.fail_writeback(ino, 1);
    writer
        .pwrite(&file, BLOCK as u64, &[3u8; BLOCK])
        .expect("write");
    world.background_writeback(ino);
    world.kill(&writer);
    let reader = world.process("reader", 0, 0);
    let root = reader.root_dir().expect("root");
    let fresh = reader.open_read(&root, "f").expect("open");
    assert!(
        reader.fdatasync(&fresh).is_err(),
        "[errseq-reopen] an unseen error is reported to the new description"
    );
}

/// Page reclaim may drop a clean page while descriptors are open: bytes
/// only the page cache held disappear, the inode and its error stay.
#[test]
fn s03_page_reclaim_with_a_descriptor_open() {
    let world = SimWorld::new(HostFixture::qualified());
    let writer = world.process("writer", 0, 0);
    let (ino, file) = scratch_file(&world, &writer, "f", 4);
    world.fail_writeback(ino, 2);
    writer
        .pwrite(&file, 2 * BLOCK as u64, &[9u8; BLOCK])
        .expect("write");
    world.background_writeback(ino);
    assert_eq!(world.visible(ino)[2 * BLOCK], 9);
    world.reclaim_pages(ino);
    assert_eq!(
        world.visible(ino)[2 * BLOCK],
        0,
        "[metadata-schedules] reclaim with the writer's descriptor open"
    );
    assert!(world.errseq(ino).0 != 0, "the inode and its error stay");
    assert!(
        writer.fdatasync(&file).is_err(),
        "the writer still sees the error"
    );
}

/// Directory entries may become durable before a directory sync: under the
/// ordered family a prefix of the log persists; the per-directory
/// over-approximation also splits a rename.
#[test]
fn s04_metadata_schedules_and_split_renames() {
    let world = SimWorld::new(HostFixture::qualified());
    let admin = world.process("admin", 0, 0);
    let root = admin.root_dir().expect("root");
    admin.make_dir(&root, "a", 0o755).expect("mkdir a");
    admin.make_dir(&root, "b", 0o755).expect("mkdir b");
    let sync_root = admin.open_dir_for_sync(&root, ".").err();
    assert!(sync_root.is_some(), "a dot name is refused");
    let world_root = world.root();
    // Guarantee a and b (sync /).
    {
        let handle = {
            // `/` has no parent: sync it through its own entry in the
            // fixture's view of the log: the root's halves.
            world.pending_metadata()
        };
        assert_eq!(handle.len(), 2);
    }
    let a = admin.open_dir(&root, "a").expect("a");
    let b = admin.open_dir(&root, "b").expect("b");
    let file = admin.create_exclusive(&a, "x", 0o644).expect("create");
    drop(file);
    admin.rename(&a, "x", &b, "y").expect("rename");
    let ordered = world.schedules(true);
    let perdir = world.schedules(false);
    assert!(
        ordered.len() >= 4,
        "[metadata-schedules] ordered prefixes: {}",
        ordered.len()
    );
    let mut split = false;
    for schedule in &perdir {
        let fork = world.fork();
        fork.power_loss(Some(schedule), sim::Tear::Old, &BTreeMap::new());
        let a_ino = fork.entries(world_root).get("a").copied();
        let b_ino = fork.entries(world_root).get("b").copied();
        if let (Some(a_ino), Some(b_ino)) = (a_ino, b_ino) {
            let in_a = fork.entries(a_ino).contains_key("x");
            let in_b = fork.entries(b_ino).contains_key("y");
            if in_a == in_b && in_a {
                split = true;
            }
        }
    }
    assert!(
        split,
        "[metadata-schedules] the per-directory family splits a rename (both names)"
    );
    for schedule in &ordered {
        let fork = world.fork();
        fork.power_loss(Some(schedule), sim::Tear::Old, &BTreeMap::new());
        let a_ino = fork.entries(world_root).get("a").copied();
        let b_ino = fork.entries(world_root).get("b").copied();
        if let (Some(a_ino), Some(b_ino)) = (a_ino, b_ino) {
            let both =
                fork.entries(a_ino).contains_key("x") && fork.entries(b_ino).contains_key("y");
            assert!(
                !both,
                "[metadata-schedules] an ordered schedule never splits a rename"
            );
        }
    }
}

/// A block being written may end old, new or torn, but only within its
/// own 4096-byte block: no other block's durable content changes.
#[test]
fn s05_tears_stay_within_their_block() {
    let world = SimWorld::new(HostFixture::qualified());
    let writer = world.process("writer", 0, 0);
    let (ino, file) = scratch_file(&world, &writer, "f", 4);
    writer
        .pwrite(&file, BLOCK as u64, &[1u8; BLOCK])
        .expect("write 1");
    writer.fdatasync(&file).expect("sync 1");
    writer
        .pwrite(&file, 2 * BLOCK as u64, &[2u8; BLOCK])
        .expect("write 2");
    for tear in [
        sim::Tear::Old,
        sim::Tear::New,
        sim::Tear::Prefix(8),
        sim::Tear::Prefix(2048),
        sim::Tear::Garbage,
    ] {
        let fork = world.fork();
        let mut tears = BTreeMap::new();
        tears.insert((ino, 2u64), tear);
        fork.power_loss(None, sim::Tear::Old, &tears);
        let durable = fork.durable(ino);
        assert!(
            durable[BLOCK..2 * BLOCK].iter().all(|byte| *byte == 1),
            "[tear-containment] {tear:?} altered another block"
        );
        assert!(
            durable[3 * BLOCK..].iter().all(|byte| *byte == 0),
            "[tear-containment] {tear:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// J: the journal model's conformance to the cited kernel paths (design
// sections 5.1, 5.5 to 5.8 and 10.7) and the activation proof's windows
// ---------------------------------------------------------------------------

use custody::store::sim::{Branch, Device, Jbd2, JournalEvent as E, JournalTrace};

fn committed_dependency(device: Device, emulated: bool) -> Jbd2 {
    let mut journal = Jbd2::new(false, false, device, emulated);
    assert!(journal.op("d1").is_ok());
    assert_eq!(journal.dir_fsync(), Ok(()));
    assert!(journal.committed("d1"));
    journal
}

/// The completed-transaction return (`journal.c:800-805`): once the probe's
/// transaction completed, its `fsync` returns 0 without testing the abort
/// flag, even with the journal aborted by a later commit; while it runs, the
/// `fsync` commits it and then tests the flag.
#[test]
fn j01_the_completed_transaction_return() {
    let mut journal = Jbd2::admitted();
    assert_eq!(journal.probe_touch(false), Ok(()));
    assert!(journal.start_commit());
    assert!(journal.finish_commit(true));
    assert!(journal.op("u").is_ok());
    assert!(journal.start_commit());
    assert!(journal.finish_commit(false), "a later commit fails");
    assert!(journal.aborted);
    assert_eq!(
        journal.probe_fsync(),
        Ok(()),
        "[journal-conformance] the completed branch returns 0 untested"
    );
    assert!(journal
        .trace
        .contains(&JournalTrace::CompletedBranch { aborted: true }));
    let mut running = Jbd2::admitted();
    assert_eq!(running.probe_touch(false), Ok(()));
    running.outcomes.push_back(false);
    assert_eq!(
        running.probe_fsync(),
        Err(custody::store::io::Errno::Io),
        "[journal-conformance] the running branch tests the flag"
    );
    assert!(running
        .trace
        .contains(&JournalTrace::FsyncBranch(Branch::Running)));
    // An abort before the probe's handle test: emergency read-only first.
    let mut early = Jbd2::admitted();
    early.abort("an ext4 error elsewhere");
    assert_eq!(
        early.probe_touch(false),
        Ok(()),
        "futimens itself returns 0"
    );
    assert_eq!(
        early.probe_fsync(),
        Err(custody::store::io::Errno::Rofs),
        "[journal-conformance] emergency read-only first"
    );
    // A directory sync with nothing to commit returns 0 untested.
    let mut silent = Jbd2::admitted();
    assert!(silent.op("d").is_ok());
    assert!(silent.start_commit());
    assert!(silent.finish_commit(false));
    assert_eq!(
        silent.dir_fsync(),
        Ok(()),
        "[journal-conformance] nothing to commit: 0 untested"
    );
}

/// Discarded flush statuses (`checkpoint.c:338-339`, `commit.c:775-778`,
/// `commit.c:883-886`): on a volatile write-back cache each leaves the
/// journal unaborted and can lose a committed operation at a power loss;
/// no abort is invented where the source discards the status. On the
/// supported profile the two `commit.c` sites never run.
#[test]
fn j02_discarded_flushes_lose_history_on_a_volatile_cache() {
    // R4's witness, retained on a cache that is not admitted.
    let mut journal = committed_dependency(Device::Volatile, false);
    journal.flushes.push_back(false);
    assert_eq!(
        journal.checkpoint(),
        Ok(()),
        "[journal-conformance] the discarded status changes nothing"
    );
    assert!(
        !journal.aborted,
        "[journal-conformance] no abort is invented at a discarded flush"
    );
    assert!(
        !journal.survives("d1"),
        "[journal-conformance] R4's counterexample: history lost on a volatile cache"
    );
    // The external journal's and the asynchronous commit's discarded flushes.
    for (external, asynchronous, site) in [
        (true, false, "commit.c:775-778"),
        (false, true, "commit.c:883-886"),
    ] {
        let mut journal = Jbd2::new(external, asynchronous, Device::Volatile, false);
        assert!(journal.op("d1").is_ok());
        journal.flushes.push_back(false);
        assert_eq!(journal.dir_fsync(), Ok(()), "{site}: the sync succeeds");
        assert!(
            !journal.aborted,
            "[journal-conformance] {site}: no abort invented"
        );
        assert!(journal.sites.contains_key(&(site, "fail")));
    }
    // The supported profile: neither commit.c site runs.
    let mut profile = Jbd2::admitted();
    for name in ["d1", "d2", "u"] {
        assert!(profile.op(name).is_ok());
        assert_eq!(profile.dir_fsync(), Ok(()));
    }
    let _ = profile.checkpoint();
    for site in ["commit.c:775-778", "commit.c:883-886"] {
        assert!(
            !profile.sites.keys().any(|(name, _)| *name == site),
            "[journal-conformance] {site} ran on the profile"
        );
    }
}

/// Stable completion on admitted storage (design section 5.7): no flush is
/// sent; a failed home write aborts before any new tail; a failed
/// superblock write leaves either tail keeping the history; a write in
/// flight holds the tail. A-S1 is necessary: on a cache the kernel
/// registered as absent the history is lost with nothing reported.
#[test]
fn j03_the_stable_completion_checkpoint() {
    let cases: Vec<(&str, Vec<E>)> = vec![
        ("clean checkpoint", vec![E::Checkpoint]),
        (
            "a flush failure the device is never sent",
            vec![E::FlushFails(1), E::Checkpoint],
        ),
        (
            "a failed home write",
            vec![E::HomeWriteFails, E::Checkpoint],
        ),
        (
            "a failed tail write, the rewrite succeeding",
            vec![E::SuperblockFails, E::Checkpoint],
        ),
        (
            "both superblock writes failing",
            vec![E::SuperblockFailsTwice, E::Checkpoint],
        ),
        (
            "a home write in flight",
            vec![E::HomeInFlight, E::Checkpoint],
        ),
        (
            "in flight, then completed",
            vec![
                E::HomeInFlight,
                E::Checkpoint,
                E::InFlightCompletes,
                E::U2,
                E::BackgroundOk,
                E::Checkpoint,
            ],
        ),
    ];
    for (label, events) in cases {
        let mut journal = committed_dependency(Device::Stable, false);
        for event in events {
            journal.event(event, false);
        }
        assert!(
            journal.survives("d1"),
            "[stable-completion] {label}: a committed dependency lost on admitted storage"
        );
        assert!(
            !journal
                .sites
                .keys()
                .any(|(_, outcome)| *outcome != "not sent"),
            "[stable-completion] {label}: a flush reached admitted storage"
        );
    }
    let mut failed = committed_dependency(Device::Stable, false);
    failed.event(E::HomeWriteFails, false);
    assert_eq!(
        failed.checkpoint(),
        Err(custody::store::io::Errno::Io),
        "[stable-completion] a failed home write is reported"
    );
    assert!(
        failed.aborted,
        "[stable-completion] a failed home write aborts before a new tail"
    );
    // A-S1 is necessary.
    let mut witness = committed_dependency(Device::Unflushed, false);
    assert_eq!(witness.checkpoint(), Ok(()));
    assert!(
        !witness.aborted && !witness.survives("d1"),
        "[stable-completion] the A-S1 witness: lost with nothing reported"
    );
}

/// The paths of design section 5.8 on a volatile write-back cache, each
/// with its own outcome: (a) a discarded flush failure loses history;
/// (b) a checked superblock failure after (a) can still lose it; (c) native
/// FUA keeps it only after a successful flush; (d) emulated FUA's later
/// flush keeps it; (e) a later successful flush keeps it.
#[test]
fn j04_volatile_paths_keep_their_own_outcomes() {
    let run = |emulated: bool, events: &[E]| {
        let mut journal = committed_dependency(Device::Volatile, emulated);
        for event in events {
            journal.event(*event, false);
        }
        (journal.survives("d1"), journal.aborted)
    };
    assert_eq!(
        run(false, &[E::FlushFails(1), E::Checkpoint]),
        (false, false),
        "[journal-conformance] (a)"
    );
    let (kept, aborted) = run(
        false,
        &[E::FlushFails(1), E::SuperblockFails, E::Checkpoint],
    );
    assert!(
        !kept && aborted,
        "[journal-conformance] (b) detected, not prevented"
    );
    assert!(
        run(false, &[E::Checkpoint]).0,
        "[journal-conformance] (c) after a successful flush"
    );
    assert!(
        run(true, &[E::FlushFails(1), E::Checkpoint]).0,
        "[journal-conformance] (d) the emulated FUA's flush"
    );
    assert!(
        !run(true, &[E::FlushFails(3), E::Checkpoint]).0,
        "[journal-conformance] (d) every flush failing"
    );
    assert!(
        run(
            false,
            &[E::FlushFails(1), E::Checkpoint, E::U2, E::BackgroundOk]
        )
        .0,
        "[journal-conformance] (e) a later successful flush"
    );
}

const JOURNAL_PRE: [E; 6] = [E::D1, E::D2, E::U, E::Start, E::CommitOk, E::CommitFail];
const JOURNAL_GAPS: [Option<E>; 6] = [
    None,
    Some(E::Start),
    Some(E::BackgroundOk),
    Some(E::BackgroundFail),
    Some(E::U),
    Some(E::Abort),
];

fn journal_after() -> Vec<Vec<E>> {
    vec![
        vec![],
        vec![E::U2, E::BackgroundFail],
        vec![E::Abort],
        vec![E::U2, E::BackgroundOk, E::Checkpoint],
        vec![E::U2, E::BackgroundFail, E::Checkpoint],
        vec![E::Checkpoint],
        vec![E::FlushFails(1), E::Checkpoint],
        vec![E::HomeWriteFails, E::Checkpoint],
        vec![E::U2, E::BackgroundOk, E::HomeWriteFails, E::Checkpoint],
        vec![E::HomeInFlight, E::Checkpoint],
        vec![E::SuperblockFails, E::Checkpoint],
        vec![E::SuperblockFailsTwice, E::Checkpoint],
    ]
}

fn journal_key(journal: &Jbd2) -> String {
    let txns: Vec<String> = journal
        .txns
        .values()
        .map(|txn| {
            format!(
                "{}:{:?}:{:?}:{:?}",
                txn.tid, txn.state, txn.outcome, txn.ops
            )
        })
        .collect();
    format!(
        "{txns:?}|{:?}|{:?}|{}",
        journal.running, journal.committing, journal.aborted
    )
}

fn journal_initial_states() -> Vec<Jbd2> {
    let mut seen: BTreeMap<String, Jbd2> = BTreeMap::new();
    for length in 2..=4usize {
        let total = 6usize.pow(length as u32);
        for code in 0..total {
            let mut sequence = Vec::with_capacity(length);
            let mut rest = code;
            for _ in 0..length {
                sequence.push(JOURNAL_PRE[rest % 6]);
                rest /= 6;
            }
            sequence.reverse();
            if sequence.iter().filter(|event| **event == E::D1).count() != 1
                || sequence.iter().filter(|event| **event == E::D2).count() != 1
            {
                continue;
            }
            let mut journal = Jbd2::admitted();
            if !sequence.iter().all(|event| journal.event(*event, true)) {
                continue;
            }
            seen.entry(journal_key(&journal)).or_insert(journal);
        }
    }
    seen.into_values().collect()
}

/// The activation proof's safety (design section 10.7): in every enumerated
/// initial state, with any background event before each of A2's two syncs,
/// A3's touch and A3's fsync, an abort inside the probe's handle start or
/// not, and the forced commits succeeding or failing, a certified activation
/// has both dependencies committed, and they survive every continuation
/// (failures the storage reports included) and a power loss on admitted
/// storage. A refusal happens only after a failure. The counts are the
/// design's (section 10.7).
#[test]
fn j05_activation_windows_on_admitted_storage() {
    let initial = journal_initial_states();
    let after = journal_after();
    let (mut runs, mut certified, mut refused, mut conservative, mut late_abort, mut continuations) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for start in &initial {
        for gaps_code in 0..6usize.pow(4) {
            let gaps: Vec<Option<E>> = (0..4)
                .map(|k| JOURNAL_GAPS[(gaps_code / 6usize.pow(3 - k)) % 6])
                .collect();
            for window in [false, true] {
                for wait in [true, false] {
                    runs += 1;
                    let mut journal = start.clone();
                    journal.outcomes = std::iter::repeat_n(wait, 6).collect();
                    let mut ok = true;
                    for (step, gap) in gaps.iter().enumerate() {
                        if let Some(event) = gap {
                            journal.event(*event, false);
                        }
                        let result = match step {
                            0 | 1 => journal.dir_fsync(),
                            2 => journal.probe_touch(window),
                            _ => journal.probe_fsync(),
                        };
                        if result.is_err() {
                            ok = false;
                            break;
                        }
                        if step == 1 {
                            journal.trace.push(JournalTrace::SyncsReturned);
                        }
                    }
                    if !ok {
                        refused += 1;
                        assert!(
                            journal.aborted || journal.emergency,
                            "[activation-proof] refused although nothing failed"
                        );
                        if journal.committed("d1") && journal.committed("d2") {
                            conservative += 1;
                        }
                        continue;
                    }
                    certified += 1;
                    assert!(
                        journal.committed("d1") && journal.committed("d2"),
                        "[activation-proof] certified with a dependency not committed"
                    );
                    if journal
                        .trace
                        .contains(&JournalTrace::CompletedBranch { aborted: true })
                    {
                        late_abort += 1;
                    }
                    for events in &after {
                        let mut later = journal.clone();
                        for event in events {
                            later.event(*event, false);
                        }
                        continuations += 1;
                        assert!(
                            later.survives("d1") && later.survives("d2"),
                            "[activation-proof] a certified dependency lost after {events:?}"
                        );
                    }
                }
            }
        }
    }
    let counts = (
        initial.len() as u64,
        runs,
        certified,
        refused,
        conservative,
        late_abort,
        continuations,
    );
    println!("activation windows: {counts:?}");
    assert_eq!(
        counts,
        (50, 259_200, 51_820, 207_380, 48_404, 32_460, 621_840),
        "[activation-proof] the enumeration's counts"
    );
}

/// The finite errseq counter (`errseq.c:22-23`, `errseq.c:36-46`): after
/// 2^19 errors, each seen by another reader, the counter returns to a
/// sample taken earlier, and a check against that sample reports nothing.
/// The store does not detect this and does not claim to; it documents the
/// limitation (design section 5.6, domain S). A sample of zero never recurs.
#[test]
fn j06_the_errseq_counter_limitation_is_documented_not_detected() {
    use custody::store::sim::Errseq;
    let mut device = Errseq::default();
    let zero_sample = device.sample();
    device.set(sim::EIO);
    let mut reader = 0u32;
    assert_eq!(device.check_and_advance(&mut reader), sim::EIO);
    let journal_sample = device.sample();
    assert_ne!(journal_sample, 0);
    for _ in 0..(1u32 << 19) {
        device.set(sim::EIO);
        assert_ne!(
            device.check(journal_sample),
            0,
            "[errseq-limit] detected before the counter wraps"
        );
        let mut other = 0u32;
        device.check_and_advance(&mut other);
        assert_ne!(
            device.check(zero_sample),
            0,
            "[errseq-limit] a zero sample always sees an error"
        );
    }
    assert_eq!(
        device.check(journal_sample),
        0,
        "[errseq-limit] after 2^19 seen errors the counter collides: no detection"
    );
    let mut fresh = Errseq::default();
    fresh.set(0);
    fresh.set(5000);
    assert_eq!(
        fresh,
        Errseq::default(),
        "zero and out-of-range errors are not recorded"
    );
}

/// The whole model's checkpoint agrees with the transaction model on the
/// storage paths both express: whether a committed, synced directory entry
/// survives a power loss.
#[test]
fn j07_whole_model_checkpoint_agrees_with_the_transaction_model() {
    use custody::store::sim::{Checkpoint, Home};
    let paths: Vec<(&str, Device, Checkpoint, Vec<E>)> = vec![
        (
            "admitted, clean",
            Device::Stable,
            Checkpoint::CLEAN,
            vec![E::Checkpoint],
        ),
        (
            "admitted, flush fails (not sent)",
            Device::Stable,
            Checkpoint {
                flush_ok: false,
                ..Checkpoint::CLEAN
            },
            vec![E::FlushFails(1), E::Checkpoint],
        ),
        (
            "admitted, home write fails",
            Device::Stable,
            Checkpoint {
                home: Home::Fail,
                ..Checkpoint::CLEAN
            },
            vec![E::HomeWriteFails, E::Checkpoint],
        ),
        (
            "admitted, tail write fails",
            Device::Stable,
            Checkpoint {
                superblock_ok: false,
                ..Checkpoint::CLEAN
            },
            vec![E::SuperblockFails, E::Checkpoint],
        ),
        (
            "admitted, both superblock writes fail",
            Device::Stable,
            Checkpoint {
                superblock_ok: false,
                rewrite_ok: false,
                ..Checkpoint::CLEAN
            },
            vec![E::SuperblockFailsTwice, E::Checkpoint],
        ),
        (
            "admitted, in flight",
            Device::Stable,
            Checkpoint {
                home: Home::InFlight,
                ..Checkpoint::CLEAN
            },
            vec![E::HomeInFlight, E::Checkpoint],
        ),
        (
            "volatile, clean",
            Device::Volatile,
            Checkpoint::CLEAN,
            vec![E::Checkpoint],
        ),
        (
            "volatile, flush fails",
            Device::Volatile,
            Checkpoint {
                flush_ok: false,
                ..Checkpoint::CLEAN
            },
            vec![E::FlushFails(1), E::Checkpoint],
        ),
        (
            "volatile, home write fails",
            Device::Volatile,
            Checkpoint {
                home: Home::Fail,
                ..Checkpoint::CLEAN
            },
            vec![E::HomeWriteFails, E::Checkpoint],
        ),
        (
            "volatile, flush fails, tail write fails",
            Device::Volatile,
            Checkpoint {
                flush_ok: false,
                superblock_ok: false,
                ..Checkpoint::CLEAN
            },
            vec![E::FlushFails(1), E::SuperblockFails, E::Checkpoint],
        ),
        (
            "volatile, in flight",
            Device::Volatile,
            Checkpoint {
                home: Home::InFlight,
                ..Checkpoint::CLEAN
            },
            vec![E::HomeInFlight, E::Checkpoint],
        ),
        (
            "unflushed, clean",
            Device::Unflushed,
            Checkpoint::CLEAN,
            vec![E::Checkpoint],
        ),
    ];
    for (label, device, checkpoint, events) in paths {
        let mut journal = committed_dependency(device, false);
        for event in &events {
            journal.event(*event, false);
        }
        let world = SimWorld::new(HostFixture::qualified());
        world.set_device(device);
        let admin = world.process("admin", 0, 0);
        let root = admin.root_dir().expect("root");
        admin.make_dir(&root, "d", 0o755).expect("mkdir");
        let d = admin.open_dir(&root, "d").expect("d");
        drop(admin.create_exclusive(&d, "entry", 0o644).expect("create"));
        let synced = admin.open_dir_for_sync(&root, "d").expect("open for sync");
        admin.fsync(&synced).expect("sync");
        let _ = world.checkpoint(checkpoint);
        world.power_loss(None, sim::Tear::Old, &BTreeMap::new());
        let d_ino = world.entries(world.root()).get("d").copied();
        let kept = d_ino.is_some_and(|ino| world.entries(ino).contains_key("entry"));
        assert_eq!(
            kept,
            journal.survives("d1"),
            "[stable-completion] {label}: the two models disagree"
        );
    }
}

// ---------------------------------------------------------------------------
// The owner harness: the real core, the store's recorder and the worker
// ---------------------------------------------------------------------------

use custody::store::exchange::Applied;
use custody::store::io::{Errno, StoreIo};
use custody::store::open::{ActivationDir, ActivationPoint, OpeningHooks};
use custody::store::sim::Interaction;

/// One owner: its started store (custody, recorder, guard, worker), driven
/// step by step on this thread.
struct Run {
    started: OwnerStart<Token, SimIo>,
    owner: SimIo,
    now: u64,
    seq: u64,
}

impl Run {
    fn start(fixture: &Fixture, name: &str, config: Config) -> Result<Run, StartRefused> {
        Self::start_with(fixture, name, config, &mut NoHooks)
    }

    fn start_with(
        fixture: &Fixture,
        name: &str,
        config: Config,
        hooks: &mut dyn OpeningHooks,
    ) -> Result<Run, StartRefused> {
        let owner = fixture.store_process(name);
        let started =
            start_owner::<Token, _>(&owner, &fixture.path, &config, Tick(1), clock(), hooks)?;
        let mut run = Run {
            started,
            owner,
            now: 10,
            seq: 0,
        };
        run.started.worker.run_until_idle();
        Ok(run)
    }

    fn claimed(fixture: &Fixture, name: &str, config: Config) -> Run {
        match Self::start(fixture, name, config) {
            Ok(run) => {
                assert_eq!(
                    run.started.recorder.claim_state(),
                    ClaimState::Claimed,
                    "the claim"
                );
                run
            }
            Err(refused) => panic!("start refused: {refused:?}"),
        }
    }

    fn tick(&mut self) -> Tick {
        self.now += 1;
        Tick(self.now)
    }

    /// Flush, run the worker until idle, apply: until nothing changes.
    fn pump(&mut self) -> Applied {
        let mut last = None;
        for _ in 0..64 {
            let now = self.tick();
            let applied = match self.started.recorder.flush(&mut self.started.custody, now) {
                Ok(applied) => applied,
                Err(error) => panic!("flush: {error:?}"),
            };
            self.started.worker.run_until_idle();
            let now = self.tick();
            let applied_again = self.started.recorder.apply(&mut self.started.custody, now);
            if last == Some(applied_again) && applied == applied_again {
                return applied_again;
            }
            last = Some(applied_again);
        }
        last.expect("pumped")
    }

    fn custody(&mut self) -> &mut Custody<Token> {
        &mut self.started.custody
    }

    fn start_run(&mut self) {
        let now = self.tick();
        self.custody().start_run(now).expect("the run starts");
        self.pump();
    }

    fn begin(&mut self, case: u32, expectation: Expectation) {
        let now = self.tick();
        self.custody()
            .begin_case(CaseId(case), expectation, now)
            .expect("the case begins");
        self.pump();
    }

    /// Reserve an action, make its start record durable, and admit it
    /// through the store gate.
    fn admit(&mut self, kind: SlotKind) -> Result<OpTicket, GateRefused> {
        let now = self.tick();
        let reservation = self.custody().reserve(kind, now).expect("reserved");
        self.pump();
        let now = self.tick();
        self.started
            .recorder
            .admit(&mut self.started.custody, reservation, now)
    }

    fn complete(
        &mut self,
        ticket: &OpTicket,
        outcome: NativeOutcome<Token>,
    ) -> Result<Completion, Rejected<Token>> {
        let now = self.tick();
        self.custody().complete(ticket, outcome, now)
    }

    fn end_case(&mut self, clean: Clean) -> CaseOutcome {
        let now = self.tick();
        let mut clean = clean;
        let outcome = self
            .custody()
            .end_case(&mut clean, now)
            .expect("the case ends");
        self.custody().take_released();
        self.pump();
        outcome
    }

    fn finish(&mut self) {
        let now = self.tick();
        let _ = self.custody().finish_run(now);
        self.pump();
    }

    fn snapshot(&self) -> Arc<Snapshot> {
        self.started.custody.snapshot()
    }

    /// Submit a request through the control side and serve it, past any
    /// recovery spacing, then pump.
    fn serve(&mut self, op: RequestOp, clean: Clean) -> RequestOutcome {
        self.seq += 1;
        let request = Request {
            generation: self.started.custody.generation(),
            seq: self.seq,
            op,
        };
        self.started
            .control
            .submit(request)
            .expect("the queue is empty");
        self.now += 10;
        let now = Tick(self.now);
        let mut clean = clean;
        let (_, outcome) = self.started.custody.serve(&mut clean, now).expect("served");
        self.custody().take_released();
        self.pump();
        outcome
    }

    /// The claimed pool file's visible and durable bytes.
    fn journal(&self, fixture: &Fixture) -> (Vec<u8>, Vec<u8>) {
        let pool = fixture.pool_ino(self.started.index).expect("pool");
        (fixture.world.visible(pool), fixture.world.durable(pool))
    }

    /// The kinds the journal's valid prefix records.
    fn recorded(&self, fixture: &Fixture) -> Vec<RecordKind> {
        let (visible, _) = self.journal(fixture);
        let report = classify_bytes(&visible, ROOT, fixture.layout.c_pool, self.started.index);
        report.records.iter().map(|record| record.kind).collect()
    }

    /// A clean, passed, finalized run with one admitted process action.
    fn clean_pass(&mut self) {
        self.start_run();
        self.begin(1, Expectation::Clean);
        let ticket = self.admit(SlotKind::Process).expect("admitted");
        assert!(matches!(
            self.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
            Ok(Completion::Deposited(_))
        ));
        let outcome = self.end_case(Clean::Confirm);
        assert!(outcome.passed && outcome.resolved);
        self.finish();
        assert_eq!(self.snapshot().phase, RunPhase::Finalized);
    }

    /// Close and seal.
    fn close_and_seal(self) -> Result<SealState, String> {
        let Run {
            mut started, now, ..
        } = self;
        let closed = match started.custody.close(Tick(now + 1)) {
            Ok(closed) => closed,
            Err(custody) => {
                return Err(format!("close refused: {:?}", custody.shutdown_decision()))
            }
        };
        started
            .recorder
            .request_seal(&started.header, &closed, 0)
            .map_err(|withheld| format!("{withheld:?}"))?;
        started.worker.run_until_idle();
        Ok(started.recorder.seal_state())
    }
}

/// The inode of `/var/lib/nexus-os/phase2-custody/...` components.
fn ino_of(fixture: &Fixture, path: &str) -> sim::Ino {
    fixture
        .world
        .lookup(path)
        .unwrap_or_else(|| panic!("no {path}"))
}

fn state_path(fixture: &Fixture, rest: &str) -> String {
    format!("/{}/{rest}", fixture.state_root().join("/"))
}

const PROVDIR: &str = "/var/lib/nexus-os/phase2-custody/provision";
const ROOTS_DIR: &str = "/var/lib/nexus-os/phase2-custody/roots";

/// The store's refusal of an owner's start; `context` (the caller's marker
/// and label) names any other outcome.
fn refused_store(result: Result<Run, StartRefused>, context: &str) -> Refused {
    match result {
        Err(StartRefused::Store(refusal)) => refusal.refused,
        Err(other) => panic!("{context}: not a store refusal: {other:?}"),
        Ok(_) => panic!("{context}: the opening must refuse"),
    }
}

fn interactions_of(fixture: &Fixture, pid: u32) -> Vec<Interaction> {
    fixture
        .world
        .take_interactions()
        .into_iter()
        .filter(|interaction| interaction.pid == pid)
        .collect()
}

// ---------------------------------------------------------------------------
// O: opening, activation, the profile and the storage (design sections 6.4,
// 10 and 10.8)
// ---------------------------------------------------------------------------

/// The real configured-store entry is unconditionally unavailable: it
/// refuses before touching anything, its success type is uninhabited, and no
/// environment variable, feature or configuration can enable it.
#[test]
fn o01_the_real_store_entry_is_closed() {
    for path in [sim::PROVISION_PATH, "/", "/nonexistent", ""] {
        let request = open::ConfiguredStoreRequest {
            provision_path: path.into(),
            config: Config::LIVE,
        };
        match open::open_configured_store(&request) {
            Ok(never) => match never {},
            Err(refusal) => assert_eq!(
                refusal,
                open::IntegrationUnavailable::DeploymentNotAuthorized,
                "[real-entry-closed]"
            ),
        }
    }
    let sources = [
        include_str!("support/custody/store/open.rs"),
        include_str!("support/custody/store/mod.rs"),
        include_str!("support/custody/store/recorder.rs"),
        include_str!("support/custody/store/exchange.rs"),
        include_str!("support/custody/store/maintenance.rs"),
        include_str!("support/custody/store/io.rs"),
        include_str!("support/custody/store/format.rs"),
        include_str!("support/custody/store/classify.rs"),
        include_str!("support/custody/store/disposition.rs"),
    ];
    for source in sources {
        for pattern in [
            "std::env",
            "env::var",
            "option_env!",
            "env!(",
            "cfg(feature",
            "assume_safe",
            "flush_always_succeeds",
        ] {
            assert!(
                !source.contains(pattern),
                "[real-entry-closed] a switch: {pattern}"
            );
        }
    }
    let open_rs = include_str!("support/custody/store/open.rs");
    let entry = &open_rs[open_rs
        .find("pub fn open_configured_store(")
        .expect("the entry")..];
    let body =
        &entry[entry.find("{\n").expect("its body") + 2..entry.find("\n}\n").expect("its end")];
    assert_eq!(
        body, "    let _ = request;\n    Err(IntegrationUnavailable::DeploymentNotAuthorized)",
        "[real-entry-closed] the entry's whole body is its refusal: it calls nothing"
    );
    // Only the simulator implements the host view.
    let platform_impls: usize = sources
        .iter()
        .map(|source| source.matches("impl Platform for").count())
        .sum();
    let sim_rs = include_str!("support/custody/store/sim.rs");
    assert_eq!(platform_impls, 0, "[real-entry-closed] no other host view");
    assert_eq!(sim_rs.matches("impl Platform for SimIo").count(), 1);
}

/// A fresh store opens: the store lock first, then activation's seven
/// directory syncs in order, the `PROVISION` sync and the probe, all before
/// any pool file is synced or read; nothing is written before the claim.
#[test]
fn o02_activation_precedes_every_decision_and_the_claim() {
    let fixture = Fixture::provisioned(2, 16);
    fixture.world.take_interactions();
    let owner = fixture.store_process("owner");
    let lock = ino_of(&fixture, &state_path(&fixture, "LOCK"));
    let touched = fixture.world.touched(lock);
    let opened = open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks).expect("opens");
    assert_eq!(
        fixture.world.touched(lock),
        touched + 1,
        "[durable-activation] the probe"
    );
    let log = interactions_of(&fixture, owner.pid());
    let first = |op: &str, ino: Option<sim::Ino>| {
        log.iter()
            .position(|entry| entry.op == op && (ino.is_none() || entry.ino == ino))
    };
    let pools: BTreeSet<sim::Ino> = [fixture.pool_ino(0), fixture.pool_ino(1)]
        .into_iter()
        .flatten()
        .collect();
    let first_pool = log
        .iter()
        .position(|entry| {
            entry.ino.is_some_and(|ino| pools.contains(&ino))
                && matches!(entry.op, "pread" | "fdatasync" | "openat2 read")
        })
        .expect("the scan reads the pool");
    let locked = first("flock-ex", Some(lock)).expect("the store lock");
    let expected_dirs = [
        state_path(&fixture, "journals"),
        state_path(&fixture, "dispositions/revoked"),
        state_path(&fixture, "dispositions"),
        state_path(&fixture, "archive"),
        format!("/{}", fixture.state_root().join("/")),
        ROOTS_DIR.to_string(),
        PROVDIR.to_string(),
    ];
    let synced: Vec<sim::Ino> = log
        .iter()
        .filter(|entry| entry.op == "fsync dir")
        .filter_map(|entry| entry.ino)
        .collect();
    let expected: Vec<sim::Ino> = expected_dirs
        .iter()
        .map(|path| ino_of(&fixture, path))
        .collect();
    assert_eq!(
        synced, expected,
        "[durable-activation] the seven directory syncs, in order"
    );
    let provision = ino_of(&fixture, sim::PROVISION_PATH);
    let order = [
        locked,
        log.iter()
            .position(|entry| entry.op == "fsync dir")
            .expect("syncs"),
        first("fdatasync", Some(provision)).expect("PROVISION sync"),
        first("futimens", Some(lock)).expect("probe"),
        first("fsync", Some(lock)).expect("probe sync"),
        first_pool,
    ];
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "[durable-activation] order {order:?}"
    );
    assert!(
        !log.iter()
            .any(|entry| entry.op == "pwrite" || entry.op == "create"),
        "nothing written before the claim"
    );
    assert!(opened.report().activated && opened.report().preservation_sync_returned_zero);
    assert!(opened.decision.blocking.is_empty());
}

/// A second owner while the first holds the store lock is Busy, before it
/// syncs or reads anything of the store.
#[test]
fn o03_busy_before_any_sync_or_read() {
    let fixture = Fixture::provisioned(2, 16);
    let first = Run::claimed(&fixture, "first", SMALL);
    fixture.world.take_interactions();
    let second = fixture.store_process("second");
    let refused = open_owner(&second, &fixture.path, &SMALL, &mut NoHooks)
        .expect_err("[busy-before-sync] a second owner opened");
    assert_eq!(refused.refused, Refused::Busy, "[busy-before-sync]");
    let log = interactions_of(&fixture, second.pid());
    assert!(
        !log.iter().any(|entry| matches!(
            entry.op,
            "fdatasync" | "fsync" | "fsync dir" | "pread" | "futimens"
        ) && entry.ino != Some(ino_of(&fixture, sim::PROVISION_PATH))),
        "[busy-before-sync] the refused owner synced or read the store: {log:?}"
    );
    let verifier = fixture.store_process("verifier");
    assert_eq!(
        verify_standalone(&verifier, &fixture.path, &SMALL)
            .expect_err("[session-verify] Busy is never success")
            .refused,
        Refused::Busy,
        "[session-verify] Busy is never success"
    );
    drop(first);
}

/// A hook that rewrites `PROVISION` (a new revision, through a root
/// session) at the given attempts, between an owner's read and its lock.
struct Rewriter<'a> {
    fixture: &'a Fixture,
    attempts: BTreeSet<u32>,
}

impl OpeningHooks for Rewriter<'_> {
    fn before_lock(&mut self, attempt: u32) {
        if !self.attempts.contains(&attempt) {
            return;
        }
        let root = self.fixture.root_process("rewriter");
        let session =
            custody::store::maintenance::begin(root, &self.fixture.path, false).expect("a session");
        let session = std::rc::Rc::new(std::cell::RefCell::new(session));
        let mut procedure = custody::store::maintenance::rewrite_provision(
            std::rc::Rc::clone(&session),
            |provision| {
                provision.operator = "rewriter".into();
            },
        );
        procedure.run_all(&mut |_| {}).expect("the rewrite");
        drop(procedure);
        if let Ok(session) = std::rc::Rc::try_unwrap(session) {
            session.into_inner().end();
        }
    }
}

/// A `PROVISION` replaced between an owner's read and its lock never
/// regains authority: revalidation by fresh lookups fails and the owner
/// selects again, at most three times (design section 10.5).
#[test]
fn o04_a_replaced_selection_never_regains_authority() {
    let fixture = Fixture::provisioned(2, 16);
    let owner = fixture.store_process("owner");
    let mut once = Rewriter {
        fixture: &fixture,
        attempts: BTreeSet::from([1]),
    };
    let opened = open_owner(&owner, &fixture.path, &SMALL, &mut once).expect("re-selects");
    assert_eq!(
        opened.selection.revision(),
        2,
        "[provision-selection] the new revision"
    );
    drop(opened);
    let owner = fixture.store_process("owner-2");
    let mut always = Rewriter {
        fixture: &fixture,
        attempts: BTreeSet::from([1, 2, 3]),
    };
    let refused = open_owner(&owner, &fixture.path, &SMALL, &mut always)
        .expect_err("[provision-selection] bounded re-selection");
    assert!(
        matches!(refused.refused, Refused::SelectionChanged(_)),
        "[provision-selection] bounded re-selection: {refused:?}"
    );
    assert_eq!(
        refused.revision,
        Some(4),
        "the refusal names the revision it last read"
    );
}

/// Special files and links refuse without being opened; ownership, link
/// count and identity are checked before any open (design section 10.1).
#[test]
fn o05_safe_open_refuses_before_opening() {
    use custody::store::io::FileType;
    type Case = (&'static str, Box<dyn Fn(&Fixture)>, fn(&Refused) -> bool);
    let cases: Vec<Case> = vec![
        (
            "a FIFO at a pool name",
            Box::new(|f: &Fixture| {
                let journals = ino_of(f, &state_path(f, "journals"));
                f.world.fixture_remove(journals, "j00001.journal");
                f.world.fixture_entry(
                    journals,
                    "j00001.journal",
                    FileType::Fifo,
                    (sim::STORE_UID, sim::STORE_GID, 0o600),
                );
            }),
            |r| matches!(r, Refused::Invalid(_)),
        ),
        (
            "a symbolic link at a pool name",
            Box::new(|f: &Fixture| {
                let journals = ino_of(f, &state_path(f, "journals"));
                f.world.fixture_remove(journals, "j00001.journal");
                f.world.fixture_entry(
                    journals,
                    "j00001.journal",
                    FileType::Symlink,
                    (sim::STORE_UID, sim::STORE_GID, 0o777),
                );
            }),
            |r| matches!(r, Refused::Invalid(_)),
        ),
        (
            "a hard link to a pool file",
            Box::new(|f: &Fixture| {
                let archive = ino_of(f, &state_path(f, "archive"));
                let pool = f.pool_ino(1).expect("pool");
                f.world.fixture_link(archive, "extra", pool);
            }),
            |r| matches!(r, Refused::Invalid(_)),
        ),
        (
            "another owner",
            Box::new(|f: &Fixture| {
                f.world
                    .fixture_owner(f.pool_ino(1).expect("pool"), 1002, sim::STORE_GID, 0o600);
            }),
            |r| matches!(r, Refused::Invalid(_)),
        ),
        (
            "another mode",
            Box::new(|f: &Fixture| {
                f.world.fixture_owner(
                    f.pool_ino(1).expect("pool"),
                    sim::STORE_UID,
                    sim::STORE_GID,
                    0o640,
                );
            }),
            |r| matches!(r, Refused::Invalid(_)),
        ),
        (
            "a replaced pool file",
            Box::new(|f: &Fixture| {
                let journals = ino_of(f, &state_path(f, "journals"));
                f.world.fixture_remove(journals, "j00001.journal");
                let replacement = f.world.fixture_entry(
                    journals,
                    "j00001.journal",
                    FileType::Regular,
                    (sim::STORE_UID, sim::STORE_GID, 0o600),
                );
                f.world.install(replacement, &vec![0u8; 18 * BLOCK]);
            }),
            |r| matches!(r, Refused::Lost(_)),
        ),
        (
            "a symbolic link as an ancestor",
            Box::new(|f: &Fixture| {
                let custody_dir = ino_of(f, "/var/lib/nexus-os/phase2-custody");
                f.world.fixture_remove(custody_dir, "provision");
                f.world
                    .fixture_entry(custody_dir, "provision", FileType::Symlink, (0, 0, 0o777));
            }),
            |r| matches!(r, Refused::Invalid(_)),
        ),
        (
            "a writable ancestor",
            Box::new(|f: &Fixture| {
                f.world
                    .fixture_owner(ino_of(f, "/var/lib/nexus-os"), 0, 0, 0o777);
            }),
            |r| matches!(r, Refused::Invalid(_)),
        ),
    ];
    for (label, edit, expected) in cases {
        let fixture = Fixture::provisioned(2, 16);
        let target = fixture.pool_ino(1);
        edit(&fixture);
        fixture.world.take_interactions();
        let owner = fixture.store_process("owner");
        let refused = open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks)
            .expect_err(&format!("[safe-open] {label}"));
        assert!(
            expected(&refused.refused),
            "[safe-open] {label}: {refused:?}"
        );
        let opened_special = interactions_of(&fixture, owner.pid()).iter().any(|entry| {
            entry.op.starts_with("openat2")
                && entry.name.as_deref() == Some("j00001.journal")
                && entry.ino != target
        });
        assert!(!opened_special, "[safe-open] {label}: the entry was opened");
    }
}

fn fixture_with_host(edit: impl Fn(&mut HostFixture)) -> Fixture {
    let fixture = Fixture::provisioned(2, 16);
    fixture.world.with_host(edit);
    fixture
}

/// ext4 only, identified through the retained root descriptor's mount
/// (never the shared magic), with the pinned options, sizes, link
/// protection and one filesystem (design section 6.4 steps 1 to 8).
#[test]
fn o06_the_mount_is_identified_through_the_descriptor() {
    let replace_line = |host: &mut HostFixture, line: &str| {
        let lines: Vec<String> = host
            .mountinfo
            .lines()
            .map(|old| {
                if old.starts_with("31 ") {
                    line.to_string()
                } else {
                    old.to_string()
                }
            })
            .collect();
        host.mountinfo = lines.iter().map(|l| format!("{l}\n")).collect();
    };
    let cases: Vec<HostCase<'_>> = vec![
        (
            "ext3 with the shared magic",
            Box::new(move |h: &mut HostFixture| {
                replace_line(h, "31 22 259:3 / /var/lib rw,relatime shared:30 - ext3 /dev/nvme0n1p2 rw,errors=remount-ro")
            }),
            "filesystem ext3",
        ),
        (
            "ext2 with the shared magic",
            Box::new(move |h: &mut HostFixture| {
                replace_line(h, "31 22 259:3 / /var/lib rw,relatime shared:30 - ext2 /dev/nvme0n1p2 rw,errors=remount-ro")
            }),
            "filesystem ext2",
        ),
        (
            "two records with the mount ID",
            Box::new(|h: &mut HostFixture| {
                h.mountinfo.push_str(
                    "31 22 259:3 / /mnt rw,relatime - ext4 /dev/nvme0n1p2 rw,errors=remount-ro\n",
                )
            }),
            "mount id not unique",
        ),
        (
            "no record with the mount ID",
            Box::new(move |h: &mut HostFixture| {
                replace_line(h, "32 22 259:3 / /var/lib rw,relatime shared:30 - ext4 /dev/nvme0n1p2 rw,errors=remount-ro")
            }),
            "mount id not unique",
        ),
        (
            "device mismatch",
            Box::new(move |h: &mut HostFixture| {
                replace_line(h, "31 22 8:1 / /var/lib rw,relatime shared:30 - ext4 /dev/sda1 rw,errors=remount-ro")
            }),
            "device mismatch",
        ),
        (
            "an unparsable line",
            Box::new(|h: &mut HostFixture| h.mountinfo.push_str("garbage line\n")),
            "separator",
        ),
        (
            "options changed",
            Box::new(move |h: &mut HostFixture| {
                replace_line(h, "31 22 259:3 / /var/lib rw,relatime,noatime shared:30 - ext4 /dev/nvme0n1p2 rw,errors=remount-ro")
            }),
            "options changed",
        ),
        (
            "block size",
            Box::new(|h: &mut HostFixture| h.fs_block_size = 1024),
            "block or page size",
        ),
        (
            "page size",
            Box::new(|h: &mut HostFixture| h.page_size = 16384),
            "block or page size",
        ),
        (
            "link protection off",
            Box::new(|h: &mut HostFixture| h.protected_hardlinks = 0),
            "link protection",
        ),
    ];
    for (label, edit, reason) in cases {
        let fixture = fixture_with_host(edit);
        let owner = fixture.store_process("owner");
        let refused = open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks)
            .expect_err(&format!("[safe-open] {label}"));
        assert!(
            matches!(&refused.refused, Refused::Unsupported(why) if why.contains(reason)),
            "[safe-open] {label}: {refused:?}"
        );
    }
    // One filesystem: the PROVISION directory on another device.
    let fixture = Fixture::provisioned(2, 16);
    fixture
        .world
        .fixture_device(ino_of(&fixture, PROVDIR), sim::makedev(259, 4), 31);
    let owner = fixture.store_process("owner");
    let refused = open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks)
        .expect_err("[durable-activation] one filesystem");
    assert!(
        matches!(&refused.refused, Refused::Unsupported(why) if why.contains("different filesystems")),
        "[durable-activation] one filesystem: {refused:?}"
    );
}

/// The effective profile (design section 6.4 steps 9 to 11), read where the
/// kernel shows it, never inferred from an absent `mountinfo` string: every
/// unsupported case refuses before any claim, for owners, the verifier and
/// sessions.
#[test]
fn o07_the_effective_profile_is_read_not_inferred() {
    let listing = |extra: &[&str], remove: &[&str]| {
        let mut lines: Vec<String> = sim::QUALIFIED_LISTING
            .iter()
            .map(|line| line.to_string())
            .collect();
        lines.retain(|line| !remove.contains(&line.as_str()));
        lines.extend(extra.iter().map(|line| line.to_string()));
        lines
    };
    let set = |lines: Vec<String>| {
        move |host: &mut HostFixture| {
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            host.set_listing(&refs);
        }
    };
    let cases: Vec<HostCase<'_>> = vec![
        (
            "an external journal",
            Box::new(|h: &mut HostFixture| {
                h.jbd2.clear();
                h.jbd2.insert("nvme1n1-8".into());
            }),
            "journal not internal",
        ),
        (
            "data=writeback in effect, absent from mountinfo",
            Box::new(set(listing(&["data=writeback"], &["data=ordered"]))),
            "effective profile",
        ),
        (
            "nobarrier in effect",
            Box::new(set(listing(&["nobarrier"], &["barrier"]))),
            "effective profile",
        ),
        (
            "data=journal",
            Box::new(set(listing(&["data=journal"], &["data=ordered"]))),
            "effective profile",
        ),
        (
            "journal_async_commit",
            Box::new(set(listing(&["journal_async_commit"], &[]))),
            "effective profile",
        ),
        (
            "norecovery",
            Box::new(set(listing(&["norecovery"], &[]))),
            "effective profile",
        ),
        (
            "fc_debug_force",
            Box::new(set(listing(&["fc_debug_force"], &[]))),
            "effective profile",
        ),
        (
            "emergency read-only",
            Box::new(set(listing(&["emergency_ro"], &[]))),
            "effective profile",
        ),
        (
            "read-only",
            Box::new(|h: &mut HostFixture| {
                let mut lines = listing(&[], &["rw"]);
                lines.insert(0, "ro".into());
                let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
                h.set_listing(&refs);
            }),
            "read-only",
        ),
        (
            "another kernel",
            Box::new(|h: &mut HostFixture| h.kernel_version = "#2 SMP PREEMPT_DYNAMIC".into()),
            "kernel not qualified",
        ),
        (
            "an unresolved name",
            Box::new(|h: &mut HostFixture| {
                h.device_links.clear();
            }),
            "device name unresolved",
        ),
        (
            "an unreadable listing",
            Box::new(|h: &mut HostFixture| {
                h.ext4_options.clear();
            }),
            "unreadable",
        ),
        (
            "an oversized listing",
            Box::new(|h: &mut HostFixture| {
                let mut lines = listing(&[], &[]);
                lines.push("x".repeat(5000));
                let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
                h.set_listing(&refs);
            }),
            "unreadable",
        ),
        (
            "a contradiction with mountinfo",
            Box::new(|h: &mut HostFixture| {
                let (mount, _) = h.store_mount_options();
                h.set_store_mount_options(&mount, "rw,errors=remount-ro,journal_async_commit");
            }),
            "options changed",
        ),
    ];
    for (label, edit, reason) in cases {
        let fixture = fixture_with_host(edit);
        for who in ["owner", "verifier", "session"] {
            let refused = match who {
                "owner" => open_owner(
                    &fixture.store_process("owner"),
                    &fixture.path,
                    &SMALL,
                    &mut NoHooks,
                )
                .expect_err(&format!("[supported-profile] {label} ({who})")),
                "verifier" => {
                    verify_standalone(&fixture.store_process("verifier"), &fixture.path, &SMALL)
                        .expect_err(&format!("[supported-profile] {label} ({who})"))
                }
                _ => match custody::store::maintenance::begin(
                    fixture.root_process("session"),
                    &fixture.path,
                    false,
                ) {
                    Ok(_) => panic!("[supported-profile] {label}: a session began"),
                    Err(refusal) => refusal,
                },
            };
            assert!(
                matches!(&refused.refused, Refused::Unsupported(why) if why.contains(reason)),
                "[supported-profile] {label} ({who}): {refused:?}"
            );
        }
    }
    // The superblock's own options, contradicting the listing.
    let fixture = fixture_with_host(|h| {
        let (mount, _) = h.store_mount_options();
        h.set_store_mount_options(&mount, "rw,nobarrier");
    });
    let refused = open_owner(
        &fixture.store_process("owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect_err("[supported-profile] the superblock's own options");
    assert!(
        matches!(refused.refused, Refused::Unsupported(_)),
        "[supported-profile] {refused:?}"
    );
}

/// A test hook that changes the host at one activation point.
struct ChangeAt<F: FnMut()> {
    point: ActivationPoint,
    change: F,
}

impl<F: FnMut()> OpeningHooks for ChangeAt<F> {
    fn activation(&mut self, point: ActivationPoint) {
        if point == self.point {
            (self.change)();
        }
    }
}

/// The profile changed during activation is refused at A4.
#[test]
fn o08_a_profile_changed_during_activation_refuses_at_a4() {
    let fixture = Fixture::provisioned(2, 16);
    let world = fixture.world.clone();
    let mut hook = ChangeAt {
        point: ActivationPoint::Revalidate,
        change: move || world.with_host(|h| h.kernel_version = "#9 SMP".into()),
    };
    let owner = fixture.store_process("owner");
    let refused =
        open_owner(&owner, &fixture.path, &SMALL, &mut hook).expect_err("[supported-profile] A4");
    assert!(
        matches!(&refused.refused, Refused::Unsupported(why) if why.contains("kernel")),
        "[supported-profile] A4: {refused:?}"
    );
}

/// Storage (design sections 5.5, 6.4 step 12 and 10.8): sixteen
/// configurations that are not admitted storage each refuse before any
/// claim, for owners, the verifier and sessions, with no admission and no
/// fallback.
#[test]
fn o09_storage_that_is_not_admitted_refuses_before_any_claim() {
    let variants = sim::storage_variants();
    assert_eq!(variants.len(), 16);
    for variant in variants {
        let fixture = Fixture::provisioned(2, 16);
        fixture.world.with_host(|host| *host = variant.host.clone());
        for who in ["owner", "verifier", "session"] {
            let refused = match who {
                "owner" => refused_store(
                    Run::start(&fixture, "owner", SMALL),
                    &format!("[storage-qualification] {} (owner)", variant.name),
                ),
                "verifier" => {
                    verify_standalone(&fixture.store_process("verifier"), &fixture.path, &SMALL)
                        .expect_err(&format!(
                            "[storage-qualification] {} (verifier)",
                            variant.name
                        ))
                        .refused
                }
                _ => match custody::store::maintenance::begin(
                    fixture.root_process("session"),
                    &fixture.path,
                    false,
                ) {
                    Ok(_) => panic!("[storage-qualification] {}: a session began", variant.name),
                    Err(refusal) => refusal.refused,
                },
            };
            assert_eq!(
                refused,
                Refused::Unsupported(variant.refusal.into()),
                "[storage-qualification] {} ({who})",
                variant.name
            );
        }
        let pool = fixture.pool_ino(0).expect("pool");
        assert!(
            fixture.world.visible(pool).iter().all(|byte| *byte == 0),
            "[storage-qualification] {}: a claim was written",
            variant.name
        );
    }
}

/// The admission belongs to one opening and its selection: none, a stale
/// one from an earlier opening of the same selection, or another store's,
/// refuses at the claim; it has no `Clone` or `Default`; decoding
/// `PROVISION` makes none (design section 10.8).
#[test]
fn o10_a_storage_admission_is_bound_to_its_opening() {
    // Autoref specialization: the impl on `Probe<T>` applies only when `T`
    // is `Clone` (or `Default`); otherwise method lookup falls through to the
    // impl on `&Probe<T>`.
    struct Probe<T>(std::marker::PhantomData<T>);
    trait Clones {
        fn is_clone(&self) -> bool {
            true
        }
    }
    impl<T: Clone> Clones for Probe<T> {}
    trait NotClone {
        fn is_clone(&self) -> bool {
            false
        }
    }
    impl<T> NotClone for &Probe<T> {}
    trait Defaults {
        fn is_default(&self) -> bool {
            true
        }
    }
    impl<T: Default> Defaults for Probe<T> {}
    trait NotDefault {
        fn is_default(&self) -> bool {
            false
        }
    }
    impl<T> NotDefault for &Probe<T> {}
    let control = Probe::<u8>(std::marker::PhantomData);
    let control = &control;
    assert!(
        control.is_clone() && control.is_default(),
        "the probe detects Clone and Default"
    );
    let admission = Probe::<open::StorageAdmission>(std::marker::PhantomData);
    let admission = &admission;
    assert!(
        !admission.is_clone() && !admission.is_default(),
        "[storage-qualification] no Clone or Default"
    );
    let open_rs = include_str!("support/custody/store/open.rs");
    assert_eq!(
        open_rs
            .matches("StorageAdmission {\n        opening,")
            .count(),
        1,
        "[storage-qualification] one constructor"
    );
    let fixture = Fixture::provisioned(2, 16);
    // An earlier opening of the same store and selection, on a copy of the
    // world: the same PROVISION digest, another opening.
    let earlier_world = fixture.world.fork();
    let earlier_owner = earlier_world.process("earlier", sim::STORE_UID, sim::STORE_GID);
    let earlier = open_owner(&earlier_owner, &fixture.path, &SMALL, &mut NoHooks).expect("opens");
    let other_fixture = Fixture::provisioned(2, 16);
    let other = open_owner(
        &other_fixture.store_process("other"),
        &other_fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect("opens");
    let not_verified = Refused::Unsupported("storage admission not verified".into());
    for (label, presented) in [
        ("no admission", None),
        ("an earlier opening's admission", Some(earlier.admission())),
        ("another store's admission", Some(other.admission())),
    ] {
        let owner = fixture.store_process(label);
        let opened = open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks).expect("opens");
        assert_eq!(opened.selection.digest, earlier.selection.digest);
        let generation = opened.draw_generation(&owner).expect("generation");
        let refused = opened
            .claim_presenting(&owner, presented, &SMALL, generation, &[])
            .err()
            .unwrap_or_else(|| panic!("[storage-qualification] {label}: the claim was accepted"));
        assert_eq!(
            refused.refused, not_verified,
            "[storage-qualification] {label}"
        );
    }
    let owner = fixture.store_process("own");
    let opened = open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks).expect("opens");
    let generation = opened.draw_generation(&owner).expect("generation");
    assert!(
        opened.claim(&owner, &SMALL, generation, &[]).is_ok(),
        "its own admission"
    );
}

/// Activation's refusals and its harmless late errors, through the whole
/// store (design sections 10.6 and 10.7): any failed or uncertain step
/// refuses before any claim; an abort after the probe's test certifies.
#[test]
fn o11_activation_refuses_on_failure_and_certifies_late_errors() {
    let unreliable = |refused: &Refused| matches!(refused, Refused::Unreliable(_));
    // Each of the seven directory syncs failing.
    for k in 0..7 {
        let fixture = Fixture::provisioned(2, 16);
        fixture
            .world
            .plan_dir_syncs(std::iter::repeat_n(None, k).chain([Some(Errno::Io)]));
        let refused = refused_store(
            Run::start(&fixture, "owner", SMALL),
            &format!("[durable-activation] sync {k} failing"),
        );
        assert!(
            unreliable(&refused),
            "[durable-activation] sync {k} failing: {refused:?}"
        );
        assert!(
            fixture
                .world
                .visible(fixture.pool_ino(0).expect("pool"))
                .iter()
                .all(|byte| *byte == 0),
            "[durable-activation] a claim after a failed activation"
        );
    }
    // EINTR: three retries, then a refusal.
    let fixture = Fixture::provisioned(2, 16);
    fixture
        .world
        .plan_dir_syncs(std::iter::repeat_n(Some(Errno::Intr), 3));
    assert!(
        Run::start(&fixture, "owner", SMALL).is_ok(),
        "[durable-activation] EINTR retried three times"
    );
    let fixture = Fixture::provisioned(2, 16);
    fixture
        .world
        .plan_dir_syncs(std::iter::repeat_n(Some(Errno::Intr), 4));
    assert!(
        unreliable(&refused_store(
            Run::start(&fixture, "owner", SMALL),
            "[durable-activation] a fourth EINTR"
        )),
        "[durable-activation] a fourth EINTR"
    );
    // PROVISION's sync, then the probe's fsync.
    for (k, label) in [(0usize, "PROVISION sync"), (1, "probe fsync")] {
        let fixture = Fixture::provisioned(2, 16);
        fixture
            .world
            .plan_file_syncs(std::iter::repeat_n(None, k).chain([Some(Errno::Io)]));
        assert!(
            unreliable(&refused_store(
                Run::start(&fixture, "owner", SMALL),
                &format!("[durable-activation] {label}")
            )),
            "[durable-activation] {label}"
        );
    }
    // A commit that failed silently before activation: the syncs find
    // nothing to commit, the probe's handle start notices the abort.
    let fixture = Fixture::provisioned(2, 16);
    fixture.world.abort_journal();
    let refused = refused_store(
        Run::start(&fixture, "owner", SMALL),
        "[activation-proof] a silent abort",
    );
    assert!(
        matches!(&refused, Refused::Unreliable(why) if why.contains("probe")),
        "[activation-proof] a silent abort: {refused:?}"
    );
    // Windows of design section 10.7, through the whole store.
    type Case = (&'static str, ActivationPoint, Box<dyn Fn(&SimWorld)>, bool);
    let cases: Vec<Case> = vec![
        (
            "an abort between the probe's two handle tests",
            ActivationPoint::Probe,
            Box::new(|w: &SimWorld| w.abort_inside_probe_handle()),
            true,
        ),
        (
            "the probe's transaction committed, then a later commit failed",
            ActivationPoint::ProbeFsync,
            Box::new(|w: &SimWorld| {
                w.commit_probe(false);
                w.abort_journal();
            }),
            true,
        ),
        (
            "the probe's own commit failed in the background",
            ActivationPoint::ProbeFsync,
            Box::new(|w: &SimWorld| w.commit_probe(true)),
            true,
        ),
        (
            "a commit failed while the probe's transaction ran",
            ActivationPoint::ProbeFsync,
            Box::new(|w: &SimWorld| w.abort_journal()),
            false,
        ),
        (
            "a read-only remount during the syncs",
            ActivationPoint::BeforeSync(ActivationDir::Journals),
            Box::new(|w: &SimWorld| w.with_host(|h| h.read_only = true)),
            false,
        ),
    ];
    for (label, point, change, certifies) in cases {
        let fixture = Fixture::provisioned(2, 16);
        let world = fixture.world.clone();
        let mut hook = ChangeAt {
            point,
            change: move || change(&world),
        };
        let result = Run::start_with(&fixture, "owner", SMALL, &mut hook);
        if certifies {
            assert!(
                result.is_ok(),
                "[activation-proof] {label}: a harmless late error refused"
            );
        } else {
            assert!(
                unreliable(&refused_store(
                    result,
                    &format!("[activation-proof] {label}")
                )),
                "[activation-proof] {label}"
            );
        }
    }
    // A directory swapped during activation, and a PROVISION replaced.
    let fixture = Fixture::provisioned(2, 16);
    let world = fixture.world.clone();
    let root_ino = ino_of(&fixture, state_path(&fixture, "").trim_end_matches('/'));
    let mut swap = ChangeAt {
        point: ActivationPoint::BeforeSync(ActivationDir::Archive),
        change: move || {
            world.fixture_remove(root_ino, "archive");
            world.fixture_entry(
                root_ino,
                "archive",
                custody::store::io::FileType::Directory,
                (0, 0, 0o755),
            );
        },
    };
    let refused = refused_store(
        Run::start_with(&fixture, "owner", SMALL, &mut swap),
        "[durable-activation] identity",
    );
    assert!(
        matches!(&refused, Refused::Invalid(why) if why.contains("archive")),
        "[durable-activation] identity: {refused:?}"
    );
    let fixture = Fixture::provisioned(2, 16);
    let world = fixture.world.clone();
    let provdir = ino_of(&fixture, PROVDIR);
    let bytes = world.visible(ino_of(&fixture, sim::PROVISION_PATH));
    let mut replace = ChangeAt {
        point: ActivationPoint::Revalidate,
        change: move || {
            world.fixture_remove(provdir, "uid-1001.provision");
            let ino = world.fixture_entry(
                provdir,
                "uid-1001.provision",
                custody::store::io::FileType::Regular,
                (0, 0, 0o444),
            );
            world.install(ino, &bytes);
        },
    };
    let refused = refused_store(
        Run::start_with(&fixture, "owner", SMALL, &mut replace),
        "[provision-selection] a replacement racing activation",
    );
    assert!(
        matches!(refused, Refused::SelectionChanged(_)),
        "[provision-selection] a replacement racing activation: {refused:?}"
    );
    // Process death and power loss during activation leave no claim; the
    // next owner activates and proceeds.
    for power in [false, true] {
        let fixture = Fixture::provisioned(2, 16);
        let world = fixture.world.clone();
        let owner = fixture.store_process("dying");
        let victim = owner.clone();
        let mut die = ChangeAt {
            point: ActivationPoint::Probe,
            change: move || {
                if power {
                    world.power_loss(None, sim::Tear::Old, &BTreeMap::new());
                } else {
                    world.kill(&victim);
                }
            },
        };
        let result =
            start_owner::<Token, _>(&owner, &fixture.path, &SMALL, Tick(1), clock(), &mut die);
        assert!(
            result.is_err(),
            "[durable-activation] the dying owner claimed"
        );
        assert!(
            fixture
                .world
                .visible(fixture.pool_ino(0).expect("pool"))
                .iter()
                .all(|byte| *byte == 0),
            "[durable-activation] a claim"
        );
        let next = Run::claimed(&fixture, "next", SMALL);
        drop(next);
    }
    // The standalone verifier never activates.
    let fixture = Fixture::provisioned(2, 16);
    fixture.world.take_interactions();
    let verifier = fixture.store_process("verifier");
    let verified = verify_standalone(&verifier, &fixture.path, &SMALL).expect("verifies");
    assert_eq!(verified.durability, open::VERIFIER_DURABILITY_NOTE);
    let log = interactions_of(&fixture, verifier.pid());
    assert!(
        !log.iter()
            .any(|entry| matches!(entry.op, "fsync dir" | "futimens" | "fdatasync" | "fsync")),
        "[durable-activation] the verifier activated or synced"
    );
}

/// A loss of qualification (design sections 5.9 and 10.8): the running
/// owner keeps its custody and its durable records; every later opening
/// refuses with the history kept; re-qualification restores the decision;
/// it never admits a volatile cache.
#[test]
fn o12_a_loss_of_qualification_refuses_later_openings_and_keeps_history() {
    let mut fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let ticket = run.admit(SlotKind::Process).expect("admitted");
    fixture
        .world
        .with_host(|h| h.set_storage_attribute("firmware_rev", Some("FW-0002 \n")));
    assert!(matches!(
        run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
        Ok(Completion::Deposited(_))
    ));
    let applied = run.pump();
    assert!(
        applied.fatal.is_none() && applied.applied_through >= 3,
        "[storage-qualification] the running owner keeps recording"
    );
    let pool = fixture.pool_ino(0).expect("pool");
    let history = fixture.world.visible(pool);
    // The owner process ends (F1): its locks are released by its exit.
    fixture.world.kill(&run.owner);
    drop(run);
    let refused = refused_store(
        Run::start(&fixture, "later", SMALL),
        "[storage-qualification] a later opening",
    );
    assert_eq!(
        refused,
        Refused::Unsupported("storage not qualified".into()),
        "[storage-qualification] a later opening"
    );
    assert_eq!(
        fixture.world.visible(pool),
        history,
        "[storage-qualification] the history is kept"
    );
    // Re-qualification never admits a volatile cache.
    let volatile = fixture.world.fork();
    volatile.with_host(|h| {
        h.set_storage_attribute("queue/write_cache", Some("write back\n"));
        h.set_storage_attribute("queue/fua", Some("1\n"));
    });
    let root = volatile.process("owner-root", 0, 0);
    match custody::store::maintenance::begin(root, &fixture.path, true) {
        Ok(_) => panic!("[storage-qualification] a re-qualification admitted a volatile cache"),
        Err(refusal) => assert_eq!(
            refusal.refused,
            Refused::Unsupported("volatile write cache".into())
        ),
    }
    // Re-qualification restores the decision: the history blocks.
    let qualification = fixture.qualify().expect("re-qualified");
    let session =
        custody::store::maintenance::begin(fixture.root_process("session"), &fixture.path, true)
            .expect("a re-qualification session");
    let session = std::rc::Rc::new(std::cell::RefCell::new(session));
    let mut procedure =
        custody::store::maintenance::requalify(std::rc::Rc::clone(&session), qualification)
            .expect("procedure");
    procedure.run_all(&mut |_| {}).expect("rewritten");
    drop(procedure);
    if let Ok(session) = std::rc::Rc::try_unwrap(session) {
        session.into_inner().end();
    }
    match Run::start(&fixture, "after", SMALL) {
        Err(StartRefused::PriorUnresolved(blocking)) => assert_eq!(
            blocking.len(),
            1,
            "[storage-qualification] the generation is in the decision"
        ),
        Err(other) => panic!("[storage-qualification] {other:?}"),
        Ok(_) => panic!("[storage-qualification] the history was dropped"),
    }
}

/// The owner's evidence-preservation syncs (design section 4.6): what an
/// owner's startup report saw of an earlier generation (here a record that
/// generation wrote but never made durable) is durable before anything rests
/// on it, so a power loss after the report keeps the bytes it reported. The
/// report says only that the syncs returned 0.
#[test]
fn o13_the_preservation_sync_makes_what_the_report_saw_durable() {
    let fixture = Fixture::provisioned(2, 16);
    let mut first = Run::claimed(&fixture, "first", SMALL);
    first.start_run();
    // The second record's sync fails before any writeback: the record is
    // visible, not durable, and never acknowledged.
    fixture.world.plan_file_syncs([Some(Errno::Io)]);
    let now = first.tick();
    first
        .custody()
        .begin_case(CaseId(1), Expectation::Clean, now)
        .expect("the case begins");
    let now = first.tick();
    let _ = first
        .started
        .recorder
        .flush(&mut first.started.custody, now);
    first.started.worker.run_until_idle();
    assert_eq!(
        first.started.recorder.latched(),
        Some((Cause::Sync(2, Errno::Io), 1))
    );
    let index = first.started.index;
    let pool = fixture.pool_ino(index).expect("pool");
    assert_ne!(
        fixture.world.visible(pool),
        fixture.world.durable(pool),
        "a record visible, not durable"
    );
    fixture.world.kill(&first.owner);
    drop(first);
    let next = Run::claimed(&fixture, "next", SMALL);
    assert!(next.started.report.preservation_sync_returned_zero);
    let reported = next
        .started
        .report
        .files
        .iter()
        .find(|file| file.index == index)
        .expect("the earlier generation is reported")
        .content;
    assert_eq!(
        reported,
        sha256(&fixture.world.visible(pool)),
        "the report read the visible bytes"
    );
    fixture
        .world
        .power_loss(None, sim::Tear::Old, &BTreeMap::new());
    assert_eq!(
        sha256(&fixture.world.durable(pool)),
        reported,
        "[F1-F2-sync] the bytes the startup report saw did not survive a power loss"
    );
}

// ---------------------------------------------------------------------------
// R: the recorder, the exchange and the admission gate (design section 9),
// through the real core
// ---------------------------------------------------------------------------

/// Every acknowledged record's block is durable and equal to its visible
/// bytes when the core applies it (INV-1); nothing beyond P is ever
/// acknowledged (INV-4).
fn assert_acked_durable(run: &Run, fixture: &Fixture, label: &str) {
    let pool = fixture.pool_ino(run.started.index).expect("pool");
    let durable = fixture.world.durable(pool);
    let acknowledged = run.snapshot().evidence.acknowledged;
    for seq in 1..=acknowledged {
        let at = ((seq + 1) as usize) * BLOCK;
        let block = &durable[at..at + BLOCK];
        let intent = &run.started.recorder.retained()[(seq - 1) as usize];
        let frame = codec::encode_record(intent).expect("frame");
        assert_eq!(
            &block[..frame.len()],
            &frame[..],
            "[ack-durable] {label}: record {seq} acknowledged before it was durable"
        );
    }
    if let Some((_, p)) = run.started.recorder.latched() {
        assert!(
            acknowledged <= p,
            "[ack-durable] {label}: acknowledged {acknowledged} beyond P = {p}"
        );
    }
}

/// A clean run, and runs whose recorder's sync or write fails at each
/// record: nothing is acknowledged at or after the failed record, the
/// failure lands at it, and a failed sync is never retried into success.
#[test]
fn r01_acknowledged_implies_durable_under_write_and_sync_faults() {
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.clean_pass();
    assert_acked_durable(&run, &fixture, "clean");
    for failing in 1..=6u64 {
        for write in [false, true] {
            let fixture = Fixture::provisioned(2, 16);
            let mut run = Run::claimed(&fixture, "owner", SMALL);
            // The claim's sync was the last file sync before the run: plan
            // the worker's record syncs (or writes) from here.
            if write {
                fixture.world.plan_writes(
                    std::iter::repeat_n(sim::WritePlan::Ok, (failing - 1) as usize)
                        .chain([sim::WritePlan::Fail(Errno::Io)]),
                );
            } else {
                fixture.world.plan_file_syncs(
                    std::iter::repeat_n(None, (failing - 1) as usize).chain([Some(Errno::Io)]),
                );
            }
            // The clean run's calls, each refused or not as the core decides
            // once recording has failed.
            let now = run.tick();
            let _ = run.custody().start_run(now);
            run.pump();
            let now = run.tick();
            let _ = run.custody().begin_case(CaseId(1), Expectation::Clean, now);
            run.pump();
            let now = run.tick();
            if let Ok(reservation) = run.custody().reserve(SlotKind::Process, now) {
                run.pump();
                let now = run.tick();
                if let Ok(ticket) =
                    run.started
                        .recorder
                        .admit(&mut run.started.custody, reservation, now)
                {
                    let _ = run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process)));
                }
            }
            if run.snapshot().case.is_some() {
                run.end_case(Clean::Confirm);
            }
            run.finish();
            let label = format!(
                "{} fails at record {failing}",
                if write { "write" } else { "sync" }
            );
            let latched = run.started.recorder.latched();
            let expected = if write {
                Cause::Write(failing)
            } else {
                Cause::Sync(failing, Errno::Io)
            };
            assert_eq!(
                latched,
                Some((expected, failing - 1)),
                "[ack-durable] {label}: the latch and P"
            );
            assert_eq!(
                run.snapshot().evidence.acknowledged,
                failing - 1,
                "[ack-durable] {label}"
            );
            assert_eq!(
                run.snapshot().evidence.failed.map(|(id, _)| id.seq()),
                Some(failing),
                "[ack-durable] {label}: delivered at the failed record"
            );
            assert_acked_durable(&run, &fixture, &label);
            assert_ne!(
                run.snapshot().phase,
                RunPhase::Finalized,
                "[ack-durable] {label}: finalized over a failure"
            );
        }
    }
    // A writeback error the storage reports for record 2's block: the sync
    // returns EIO through the error sequence, the recorder latches, and
    // nothing at or after record 2 is acknowledged.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    let pool = fixture.pool_ino(run.started.index).expect("pool");
    fixture.world.fail_writeback(pool, 3);
    run.start_run();
    run.begin(1, Expectation::Clean);
    assert_eq!(
        run.started.recorder.latched(),
        Some((Cause::Sync(2, Errno::Io), 1)),
        "[ack-durable] a reported write error is never an acknowledgement"
    );
    assert_acked_durable(&run, &fixture, "a reported write error");
    // Short writes and interruptions complete the block; a fourth stall fails.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    fixture.world.plan_writes([
        sim::WritePlan::Short(100),
        sim::WritePlan::Zero,
        sim::WritePlan::Interrupted,
        sim::WritePlan::Short(1000),
    ]);
    run.start_run();
    assert_eq!(
        run.snapshot().evidence.acknowledged,
        1,
        "[ack-durable] short writes looped to a whole block"
    );
    assert_acked_durable(&run, &fixture, "short writes");
    fixture
        .world
        .plan_writes(std::iter::repeat_n(sim::WritePlan::Zero, 4));
    run.begin(1, Expectation::Clean);
    assert_eq!(
        run.started.recorder.latched(),
        Some((Cause::Write(2), 1)),
        "[ack-durable] zero progress four times"
    );
    // A sync's EINTR is retried at most three times; its EIO never.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    fixture.world.plan_file_syncs([
        Some(Errno::Intr),
        Some(Errno::Intr),
        Some(Errno::Intr),
        Some(Errno::Io),
        None,
    ]);
    run.start_run();
    assert_eq!(
        run.started.recorder.latched(),
        Some((Cause::Sync(1, Errno::Io), 0)),
        "[ack-durable] EIO after three EINTRs is final; the next sync would have succeeded"
    );
    assert_eq!(
        run.snapshot().evidence.acknowledged,
        0,
        "[ack-durable] a failed sync retried into success"
    );
}

/// An intent as data (fields are public): the record `id` of a frame.
fn evidence_id(generation: [u8; 16], seq: u64) -> RecordId {
    codec::decode_record(&frame(generation, seq, 0, Rs(0)))
        .expect("frame")
        .id
}

/// Every invalid or conflicting submission latches with its cause and P;
/// true acknowledgements stay; admission is refused at once; the failure
/// lands at the core's next unacknowledged record once it is issued.
#[test]
fn r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record() {
    type Case = (
        &'static str,
        Box<dyn Fn(&RecordIntent, Generation) -> RecordIntent>,
        fn(u64) -> Cause,
        u64,
    );
    let cases: Vec<Case> = vec![
        (
            "a conflict on an acknowledged record",
            Box::new(|first: &RecordIntent, _| RecordIntent {
                digest: [0xee; 32],
                ..first.clone()
            }),
            |_| Cause::Conflict(1),
            1,
        ),
        (
            "a foreign record",
            Box::new(|first: &RecordIntent, _| RecordIntent {
                id: evidence_id([0x99; 16], 4),
                ..first.clone()
            }),
            |_| Cause::InvalidSubmission(4, Invalid::Foreign),
            4,
        ),
        (
            "sequence zero",
            Box::new(|first: &RecordIntent, g: Generation| RecordIntent {
                id: evidence_id(g.bytes(), 0),
                ..first.clone()
            }),
            |_| Cause::InvalidSubmission(0, Invalid::OutOfRange),
            0,
        ),
        (
            "beyond the capacity",
            Box::new(|first: &RecordIntent, g: Generation| RecordIntent {
                id: evidence_id(g.bytes(), 17),
                ..first.clone()
            }),
            |_| Cause::InvalidSubmission(17, Invalid::OutOfRange),
            17,
        ),
        (
            "a future sequence",
            Box::new(|first: &RecordIntent, g: Generation| RecordIntent {
                id: evidence_id(g.bytes(), 6),
                ..first.clone()
            }),
            |_| Cause::InvalidSubmission(6, Invalid::Future),
            6,
        ),
    ];
    for (label, make, cause, _) in cases {
        let fixture = Fixture::provisioned(2, 16);
        let mut run = Run::claimed(&fixture, "owner", SMALL);
        run.start_run();
        run.begin(1, Expectation::Clean);
        let now = run.tick();
        let reservation = run
            .custody()
            .reserve(SlotKind::Process, now)
            .expect("reserved");
        run.pump();
        assert_eq!(run.snapshot().evidence.acknowledged, 3);
        let first = run.started.recorder.retained()[0].clone();
        let generation = run.custody().generation();
        let bad = make(&first, generation);
        {
            use custody::RecordSink;
            run.started.recorder.sink().submit(&bad);
        }
        assert_eq!(
            run.started.recorder.latched(),
            Some((cause(0), 3)),
            "[fatal-total] {label}: cause and P"
        );
        let now = run.tick();
        match run
            .started
            .recorder
            .admit(&mut run.started.custody, reservation, now)
        {
            Err(GateRefused::RecorderFatal { .. }) => {}
            other => panic!("[admission-fence] {label}: admitted after the latch: {other:?}"),
        }
        assert_eq!(
            run.started
                .custody
                .control()
                .status()
                .control
                .admission
                .admitted,
            0,
            "[admission-fence] {label}: the core's admission ran"
        );
        assert_eq!(
            run.snapshot().evidence.acknowledged,
            3,
            "[fatal-total] {label}: true acknowledgements stay"
        );
        let applied = run.pump();
        assert_eq!(
            applied.delivery,
            Delivery::Waiting,
            "[fatal-total] {label}: nothing issued yet, nothing reported"
        );
        run.end_case(Clean::Confirm);
        let applied = run.pump();
        assert_eq!(
            applied.delivery,
            Delivery::Delivered { target: 4 },
            "[fatal-total] {label}: delivered at the next issued record"
        );
        assert_eq!(
            run.snapshot().evidence.failed.map(|(id, _)| id.seq()),
            Some(4),
            "[fatal-total] {label}: the core recorded the failure at record 4"
        );
        assert_eq!(
            run.snapshot().failure_counts[FailureClass::RecordFailed.index()],
            1,
            "[fatal-total] {label}: one recording failure"
        );
    }
    // Before the claim: an exchange not yet claimed.
    let exchange = custody::store::exchange::Exchange::new(Generation::new([5; 16]), 16, clock());
    let mut recorder = custody::store::exchange::Recorder::new(exchange);
    let (mut custody, _control) =
        Custody::<Token>::new(SMALL, Generation::new([5; 16]), Vec::new(), Tick(1))
            .expect("custody");
    custody.start_run(Tick(2)).expect("starts");
    let _ = recorder.flush(&mut custody, Tick(3));
    assert_eq!(
        recorder.latched(),
        Some((Cause::InvalidSubmission(1, Invalid::BeforeClaim), 0)),
        "[fatal-total] before the claim"
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ev {
    Take,
    Io,
    Publish,
    Apply,
    Admit,
    Latch,
}

fn permutations(events: &[Ev]) -> Vec<Vec<Ev>> {
    if events.len() <= 1 {
        return vec![events.to_vec()];
    }
    let mut out = Vec::new();
    for at in 0..events.len() {
        let mut rest = events.to_vec();
        let head = rest.remove(at);
        for mut tail in permutations(&rest) {
            tail.insert(0, head);
            if !out.contains(&tail) {
                out.push(tail);
            }
        }
    }
    out
}

/// Every interleaving of the admission gate, the worker's W1, its I/O and
/// W6, the owner's apply step and a latch (design section 9.8, INV-13 and
/// INV-4): nothing is admitted or published after the latch, and admission
/// needs the start record's acknowledgement.
#[test]
fn r03_every_interleaving_of_gate_worker_apply_and_latch() {
    let orders: Vec<Vec<Ev>> = permutations(&[
        Ev::Take,
        Ev::Io,
        Ev::Publish,
        Ev::Apply,
        Ev::Admit,
        Ev::Latch,
    ])
    .into_iter()
    .filter(|order| {
        let at = |ev| order.iter().position(|e| *e == ev).unwrap_or(0);
        at(Ev::Take) < at(Ev::Io) && at(Ev::Io) < at(Ev::Publish)
    })
    .collect();
    assert_eq!(orders.len(), 120);
    let mut admitted = 0;
    for order in orders {
        let fixture = Fixture::provisioned(2, 16);
        let mut run = Run::claimed(&fixture, "owner", SMALL);
        run.start_run();
        run.begin(1, Expectation::Clean);
        let now = run.tick();
        let mut reservation = Some(
            run.custody()
                .reserve(SlotKind::Process, now)
                .expect("reserved"),
        );
        let now = run.tick();
        let _ = run.started.recorder.flush(&mut run.started.custody, now);
        let mut ticket = None;
        let mut latched_at = None;
        let mut published_after_latch = false;
        for (position, event) in order.iter().enumerate() {
            match event {
                Ev::Take | Ev::Io => {
                    run.started.worker.step();
                }
                Ev::Publish => {
                    let before = run.started.recorder.status().durable_through;
                    run.started.worker.step();
                    let after = run.started.recorder.status().durable_through;
                    if latched_at.is_some() && after > before {
                        published_after_latch = true;
                    }
                }
                Ev::Apply => {
                    let now = run.tick();
                    run.started.recorder.apply(&mut run.started.custody, now);
                }
                Ev::Admit => {
                    let now = run.tick();
                    let taken = reservation.take().expect("one admission");
                    match run
                        .started
                        .recorder
                        .admit(&mut run.started.custody, taken, now)
                    {
                        Ok(granted) => ticket = Some((granted, position)),
                        Err(GateRefused::RecorderFatal {
                            reservation: back, ..
                        }) => reservation = Some(back),
                        Err(GateRefused::Core(refused)) => reservation = refused.reservation,
                    }
                }
                Ev::Latch => {
                    run.started.recorder.exchange().latch(Cause::Conflict(1));
                    latched_at = Some(position);
                }
            }
        }
        assert!(
            !published_after_latch,
            "[admission-fence] {order:?}: published after the latch"
        );
        if let Some((_, position)) = &ticket {
            admitted += 1;
            assert!(
                latched_at.is_some_and(|latch| latch > *position),
                "[admission-fence] {order:?}: admitted after the latch"
            );
            let at = |ev| order.iter().position(|e| *e == ev).unwrap_or(0);
            assert!(
                at(Ev::Publish) < *position && at(Ev::Apply) < *position,
                "[admission-fence] {order:?}: admitted before the start record was acknowledged"
            );
        }
        let (_, p) = run.started.recorder.latched().expect("latched");
        assert!(
            run.snapshot().evidence.acknowledged <= p,
            "[ack-durable] {order:?}: acknowledged beyond P"
        );
        assert_acked_durable(&run, &fixture, &format!("{order:?}"));
    }
    assert!(admitted > 0, "some orders admit before the latch");
}

/// A worker hook for the threaded runner: blocks at one point until
/// released, or panics there.
struct Gate {
    point: custody::store::recorder::WorkerPoint,
    panic: bool,
    reached: std::sync::Mutex<Option<std::sync::mpsc::SyncSender<()>>>,
    release: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
}

impl custody::store::recorder::WorkerHooks for Gate {
    fn reached(&self, point: custody::store::recorder::WorkerPoint) {
        if point != self.point {
            return;
        }
        if self.panic {
            panic!("fixture: the worker panics at {point:?}");
        }
        if let Some(reached) = self.reached.lock().ok().and_then(|mut slot| slot.take()) {
            let _ = reached.send(());
            if let Some(release) = self.release.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = release.recv_timeout(std::time::Duration::from_secs(30));
            }
        }
    }
}

/// Worker loss (its drop guard), a vanished worker thread and a poisoned
/// exchange each latch; admission is refused at once; nothing is reported
/// until the core issues the next record; true acknowledgements stay.
#[test]
fn r04_worker_loss_vanishing_and_poisoning() {
    use custody::store::recorder::{spawn_worker, WorkerPoint};
    // A: the worker unwinds with three records acknowledged and none issued
    // since.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let now = run.tick();
    let reservation = run
        .custody()
        .reserve(SlotKind::Process, now)
        .expect("reserved");
    run.pump();
    let OwnerStart {
        custody: mut core,
        mut recorder,
        worker,
        guard,
        ..
    } = run.started;
    let hook = Arc::new(Gate {
        point: WorkerPoint::Take,
        panic: true,
        reached: std::sync::Mutex::new(None),
        release: std::sync::Mutex::new(None),
    });
    let handle = spawn_worker(worker, Some(hook));
    recorder.attach_worker(handle);
    assert_eq!(recorder.wait_durable(u64::MAX, 500), 3);
    assert_eq!(
        recorder.latched(),
        Some((Cause::WorkerLost, 3)),
        "[fatal-total] worker loss latched by its drop guard"
    );
    match recorder.admit(&mut core, reservation, Tick(100)) {
        Err(GateRefused::RecorderFatal { cause, .. }) => {
            assert_eq!(cause, Cause::WorkerLost);
        }
        other => panic!("[admission-fence] after worker loss: {other:?}"),
    }
    assert_eq!(
        recorder.apply(&mut core, Tick(101)).delivery,
        Delivery::Waiting,
        "[fatal-total] no record to fail yet"
    );
    let mut clean = Clean::Confirm;
    core.end_case(&mut clean, Tick(102)).expect("the case ends");
    let applied = recorder.flush(&mut core, Tick(103)).expect("flush");
    assert_eq!(
        applied.delivery,
        Delivery::Delivered { target: 4 },
        "[fatal-total] delivered at record 4"
    );
    assert_eq!(
        core.snapshot().evidence.acknowledged,
        3,
        "[fatal-total] true acknowledgements stay"
    );
    drop(guard);
    // A thread that ends without its exit recorded: vanished.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.started
        .recorder
        .attach_worker(custody::store::exchange::WorkerHandle::new(
            std::thread::spawn(|| {}),
        ));
    std::thread::sleep(std::time::Duration::from_millis(50));
    let now = run.tick();
    run.started.recorder.apply(&mut run.started.custody, now);
    assert_eq!(
        run.started.recorder.latched().map(|(cause, _)| cause),
        Some(Cause::WorkerVanished),
        "[fatal-total] a vanished worker"
    );
    // A poisoned exchange.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.start_run();
    run.started.recorder.exchange().fixture_poison();
    run.begin(1, Expectation::Clean);
    assert_eq!(
        run.started.recorder.latched(),
        Some((Cause::Poisoned, 1)),
        "[fatal-total] a poisoned mutex"
    );
    assert_eq!(
        run.snapshot().evidence.acknowledged,
        1,
        "[fatal-total] nothing published after poisoning"
    );
}

/// An append in flight when another cause latches is never published, and
/// it is the record failed (design section 9.10, case E); an unexpected
/// acknowledgement outcome latches and stops application.
#[test]
fn r05_in_flight_appends_and_unexpected_acknowledgement_outcomes() {
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.start_run();
    let now = run.tick();
    run.custody()
        .begin_case(CaseId(1), Expectation::Clean, now)
        .expect("begins");
    let now = run.tick();
    let _ = run.started.recorder.flush(&mut run.started.custody, now);
    assert_eq!(run.started.worker.step(), Step::Progress, "W1");
    assert_eq!(run.started.worker.step(), Step::Progress, "W2 to W5");
    run.started.recorder.exchange().latch(Cause::Conflict(1));
    assert!(
        matches!(run.started.worker.step(), Step::Exit(WorkerExit::Latched)),
        "W6 refuses"
    );
    assert_eq!(
        run.started.recorder.status().durable_through,
        1,
        "[fatal-total] the record in flight was published"
    );
    let applied = run.pump();
    assert_eq!(
        applied.delivery,
        Delivery::Delivered { target: 2 },
        "[fatal-total] the record in flight is the one failed"
    );
    // An acknowledgement the core answers with Duplicate.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    let now = run.tick();
    run.custody().start_run(now).expect("starts");
    let now = run.tick();
    let _ = run.started.recorder.flush(&mut run.started.custody, now);
    run.started.worker.run_until_idle();
    let intent = run.started.recorder.retained()[0].clone();
    let now = run.tick();
    assert_eq!(
        run.custody().acknowledge(&RecordAck::of(&intent), now),
        AckOutcome::Acknowledged
    );
    let now = run.tick();
    run.started.recorder.apply(&mut run.started.custody, now);
    assert_eq!(
        run.started.recorder.latched(),
        Some((Cause::UnexpectedAck(1, AckOutcome::Duplicate), 1)),
        "[fatal-total] an unexpected outcome latches"
    );
    let now = run.tick();
    run.custody()
        .begin_case(CaseId(1), Expectation::Clean, now)
        .expect("begins");
    let applied = run.pump();
    assert_eq!(
        applied.delivery,
        Delivery::Delivered { target: 2 },
        "[fatal-total] and lands at the next issued record"
    );
}

/// The two ends `Blocking` holds one write with: it reports reaching the
/// write on the first and waits on the second for its release.
type Handoff = (
    std::sync::mpsc::SyncSender<()>,
    std::sync::mpsc::Receiver<()>,
);

/// A test I/O that blocks one `pwrite` until released: the worker is held
/// inside I/O with no mutex held.
#[derive(Clone)]
struct Blocking {
    inner: SimIo,
    gate: Arc<std::sync::Mutex<Option<Handoff>>>,
}

impl StoreIo for Blocking {
    type Dir = sim::SimDir;
    type File = sim::SimFile;
    fn root_dir(&self) -> Result<Self::Dir, custody::store::io::IoError> {
        self.inner.root_dir()
    }
    fn open_dir(
        &self,
        at: &Self::Dir,
        name: &str,
    ) -> Result<Self::Dir, custody::store::io::IoError> {
        self.inner.open_dir(at, name)
    }
    fn stat_at(
        &self,
        at: &Self::Dir,
        name: &str,
    ) -> Result<custody::store::io::Stat, custody::store::io::IoError> {
        self.inner.stat_at(at, name)
    }
    fn stat_dir(
        &self,
        dir: &Self::Dir,
    ) -> Result<custody::store::io::Stat, custody::store::io::IoError> {
        self.inner.stat_dir(dir)
    }
    fn stat_file(
        &self,
        file: &Self::File,
    ) -> Result<custody::store::io::Stat, custody::store::io::IoError> {
        self.inner.stat_file(file)
    }
    fn list_dir(
        &self,
        dir: &Self::Dir,
        limit: usize,
    ) -> Result<custody::store::io::Listing, custody::store::io::IoError> {
        self.inner.list_dir(dir, limit)
    }
    fn open_read(
        &self,
        at: &Self::Dir,
        name: &str,
    ) -> Result<Self::File, custody::store::io::IoError> {
        self.inner.open_read(at, name)
    }
    fn open_write(
        &self,
        at: &Self::Dir,
        name: &str,
    ) -> Result<Self::File, custody::store::io::IoError> {
        self.inner.open_write(at, name)
    }
    fn open_dir_for_sync(
        &self,
        at: &Self::Dir,
        name: &str,
    ) -> Result<Self::File, custody::store::io::IoError> {
        self.inner.open_dir_for_sync(at, name)
    }
    fn flock(
        &self,
        file: &Self::File,
        request: custody::store::io::LockRequest,
    ) -> Result<(), custody::store::io::IoError> {
        self.inner.flock(file, request)
    }
    fn pread(
        &self,
        file: &Self::File,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize, custody::store::io::IoError> {
        self.inner.pread(file, offset, buf)
    }
    fn pwrite(
        &self,
        file: &Self::File,
        offset: u64,
        buf: &[u8],
    ) -> Result<usize, custody::store::io::IoError> {
        let gate = self.gate.lock().ok().and_then(|mut slot| slot.take());
        if let Some((reached, release)) = gate {
            let _ = reached.send(());
            let _ = release.recv_timeout(std::time::Duration::from_secs(30));
        }
        self.inner.pwrite(file, offset, buf)
    }
    fn fdatasync(&self, file: &Self::File) -> Result<(), custody::store::io::IoError> {
        self.inner.fdatasync(file)
    }
    fn fsync(&self, file: &Self::File) -> Result<(), custody::store::io::IoError> {
        self.inner.fsync(file)
    }
    fn touch(&self, file: &Self::File) -> Result<(), custody::store::io::IoError> {
        self.inner.touch(file)
    }
    fn random(&self, buf: &mut [u8]) -> Result<(), custody::store::io::IoError> {
        self.inner.random(buf)
    }
    fn create_exclusive(
        &self,
        at: &Self::Dir,
        name: &str,
        mode: u32,
    ) -> Result<Self::File, custody::store::io::IoError> {
        self.inner.create_exclusive(at, name, mode)
    }
    fn make_dir(
        &self,
        at: &Self::Dir,
        name: &str,
        mode: u32,
    ) -> Result<(), custody::store::io::IoError> {
        self.inner.make_dir(at, name, mode)
    }
    fn link(
        &self,
        from: &Self::Dir,
        from_name: &str,
        to: &Self::Dir,
        to_name: &str,
    ) -> Result<(), custody::store::io::IoError> {
        self.inner.link(from, from_name, to, to_name)
    }
    fn rename(
        &self,
        from: &Self::Dir,
        from_name: &str,
        to: &Self::Dir,
        to_name: &str,
    ) -> Result<(), custody::store::io::IoError> {
        self.inner.rename(from, from_name, to, to_name)
    }
    fn unlink(&self, at: &Self::Dir, name: &str) -> Result<(), custody::store::io::IoError> {
        self.inner.unlink(at, name)
    }
    fn allocate(&self, file: &Self::File, len: u64) -> Result<(), custody::store::io::IoError> {
        self.inner.allocate(file, len)
    }
    fn set_owner(
        &self,
        file: &Self::File,
        uid: u32,
        gid: u32,
    ) -> Result<(), custody::store::io::IoError> {
        self.inner.set_owner(file, uid, gid)
    }
}

/// While storage blocks, the owner and the control side stay responsive: no
/// mutex is held across I/O; a stall is reported, never failed and never
/// acknowledged, and a timeout changes nothing; the late completion is
/// acknowledged late (design sections 9.6 and 14.1).
#[test]
fn r06_a_stall_is_reported_never_failed_and_nothing_waits_on_storage() {
    use custody::store::recorder::{spawn_worker, Worker};
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    let OwnerStart {
        custody: mut core,
        recorder: mut rec,
        guard,
        worker,
        header,
        ..
    } = {
        // Swap the stepped worker for a threaded one on a blocking I/O.
        let started = run.started;
        run.started = Run::claimed(&Fixture::provisioned(1, 16), "spare", SMALL).started;
        started
    };
    drop(worker);
    let _ = header;
    let (reached_tx, reached_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let blocking = Blocking {
        inner: run.owner.clone(),
        gate: Arc::new(std::sync::Mutex::new(None)),
    };
    // A fresh worker on the claimed journal: its own I/O description.
    let journals = Arc::clone(guard.journals());
    let name = format::pool_name(0);
    let file = blocking
        .open_write(&journals, &name)
        .expect("I/O description");
    let identity = blocking.stat_file(&file).expect("identity");
    let worker = Worker::new(
        blocking.clone(),
        file,
        journals,
        name,
        identity,
        Arc::clone(rec.exchange()),
    );
    core.start_run(Tick(50)).expect("starts");
    *blocking.gate.lock().expect("gate") = Some((reached_tx, release_rx));
    rec.attach_worker(spawn_worker(worker, None));
    let _ = rec.flush(&mut core, Tick(51));
    reached_rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the worker is inside pwrite");
    // The worker is inside I/O. Everything else completes now.
    let status = rec.status();
    assert!(
        status.stalled() && status.pending.map(|(seq, _)| seq) == Some(1),
        "[fatal-latched] a stall is reported: {status:?}"
    );
    assert!(
        status.fatal.is_none(),
        "[fatal-latched] a stall is not a failure"
    );
    let control = core.control();
    let _ = control.status();
    assert_eq!(rec.wait_durable(1, 3), 0, "a bounded wait times out");
    assert!(
        rec.latched().is_none(),
        "[fatal-latched] a timeout is never a failure"
    );
    let applied = rec.apply(&mut core, Tick(52));
    assert_eq!(
        (applied.applied_through, applied.fatal),
        (0, None),
        "[fatal-latched] a timeout is never an acknowledgement"
    );
    assert_eq!(
        core.begin_case(CaseId(1), Expectation::Clean, Tick(53)),
        Err(Refusal::EvidencePending),
        "admission waits for the acknowledgement"
    );
    release_tx.send(()).expect("release");
    assert_eq!(rec.wait_durable(1, 500), 1, "the late completion");
    let applied = rec.apply(&mut core, Tick(54));
    assert_eq!(
        applied.applied_through, 1,
        "[fatal-latched] acknowledged late"
    );
    rec.stop_worker();
    for _ in 0..500 {
        if rec.status().worker_exit.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    rec.join_worker();
    drop(guard);
}

/// A recorder failure changes neither native custody nor exclusion: owners
/// stay held and are cleaned up, the store lock stays held (another owner is
/// Busy), and the custody never closes (design section 9.9 concern 8).
#[test]
fn r07_a_recorder_failure_keeps_owners_and_exclusion() {
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let ticket = run.admit(SlotKind::Process).expect("admitted");
    assert!(matches!(
        run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
        Ok(Completion::Deposited(_))
    ));
    run.started.recorder.exchange().latch(Cause::WorkerLost);
    assert_eq!(
        run.started.worker.run_until_idle(),
        Step::Exit(WorkerExit::Latched),
        "the worker stops writing at the latch"
    );
    assert_eq!(
        run.snapshot().entries.len(),
        1,
        "[lock-retained] the owner is still held"
    );
    assert_eq!(
        refused_store(
            Run::start(&fixture, "other", SMALL),
            "[lock-retained] exclusion stays"
        ),
        Refused::Busy,
        "[lock-retained] exclusion stays"
    );
    let outcome = run.end_case(Clean::Confirm);
    assert!(outcome.resolved, "cleanup runs");
    assert!(
        run.snapshot().evidence.failed.is_some(),
        "[fatal-total] the failure was delivered"
    );
    let Run { started, .. } = run;
    match started.custody.close(Tick(1_000)) {
        Ok(_) => panic!("[evidence-close] closed with failed evidence"),
        Err(custody) => assert!(matches!(
            custody.shutdown_decision(),
            ShutdownDecision::Refused(_)
        )),
    }
}

/// The claim and the seal (design sections 9.7 and 9.9): a claim whose write
/// fails latches and leaves an untouched NotStarted custody that closes; a
/// latch withholds the seal; a seal write failure fails the seal; a seal
/// that became visible though its sync failed is still true.
#[test]
fn r08_claim_and_seal_boundaries() {
    use custody::store::open::NoHooks as Quiet;
    // The claim's write fails.
    let fixture = Fixture::provisioned(2, 16);
    let owner = fixture.store_process("owner");
    let mut started =
        start_owner::<Token, _>(&owner, &fixture.path, &SMALL, Tick(1), clock(), &mut Quiet)
            .expect("started");
    fixture.world.plan_writes([sim::WritePlan::Fail(Errno::Io)]);
    started.worker.run_until_idle();
    assert_eq!(
        started.recorder.claim_state(),
        ClaimState::Failed,
        "[fatal-total] the claim failed"
    );
    assert_eq!(started.recorder.latched(), Some((Cause::ClaimWrite, 0)));
    assert!(
        started.custody.close(Tick(2)).is_ok(),
        "the untouched NotStarted custody closes"
    );
    // A latch withholds the seal.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.clean_pass();
    run.started.recorder.exchange().latch(Cause::WorkerLost);
    let Run {
        started,
        now,
        owner,
        ..
    } = run;
    let closed = started
        .custody
        .close(Tick(now + 1))
        .expect("closes: nothing held or pending");
    assert_eq!(
        started.recorder.request_seal(&started.header, &closed, 0),
        Err(SealWithheld::Latched(Cause::WorkerLost)),
        "[fatal-total] the seal is withheld"
    );
    // The owner exits; a restart refuses the unsealed generation that
    // recorded an action start (a disclosed false positive).
    fixture.world.kill(&owner);
    drop(started.guard);
    match Run::start(&fixture, "next", SMALL) {
        Err(StartRefused::PriorUnresolved(blocking)) => assert_eq!(
            blocking.len(),
            1,
            "[fatal-total] refused after a withheld seal"
        ),
        other => panic!("[fatal-total] a withheld seal: {:?}", other.err()),
    }
    // A seal whose sync fails: Failed; still visible and true until evicted.
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.clean_pass();
    let pool = fixture.pool_ino(run.started.index).expect("pool");
    fixture.world.plan_file_syncs([Some(Errno::Io)]);
    let state = run.close_and_seal().expect("requested");
    assert_eq!(state, SealState::Failed, "[fatal-total] a failed seal sync");
    let report = classify_bytes(&fixture.world.visible(pool), ROOT, 16, 0);
    assert_eq!(
        report.class,
        FileClass::Sealed,
        "a visible seal is still true: close() returned Ok"
    );
    let lost = classify_bytes(&fixture.world.durable(pool), ROOT, 16, 0);
    assert_eq!(
        lost.class,
        FileClass::UnsealedAction,
        "[no-false-resolution] once it is gone, the generation is refused"
    );
    assert!(lost.refused);
}

/// Re-submission after a sink panic is idempotent, and repeated
/// submissions change nothing and grow nothing (design section 9.3).
#[test]
fn r09_resubmission_is_idempotent_and_bounded() {
    struct Panicking<'a, S: custody::RecordSink> {
        inner: &'a mut S,
        armed: bool,
    }
    impl<S: custody::RecordSink> custody::RecordSink for Panicking<'_, S> {
        fn submit(&mut self, intent: &RecordIntent) {
            self.inner.submit(intent);
            if self.armed {
                self.armed = false;
                panic!("fixture: the sink panics after storing");
            }
        }
    }
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    let now = run.tick();
    run.custody().start_run(now).expect("starts");
    {
        let started = &mut run.started;
        let mut sink = started.recorder.sink();
        let mut panicking = Panicking {
            inner: &mut sink,
            armed: true,
        };
        assert_eq!(
            started
                .custody
                .flush_records(&mut panicking, Tick(run.now + 1)),
            Err(FlushError::SinkPanicked { sent: 0 })
        );
    }
    run.now += 1;
    assert_eq!(
        run.started.recorder.status().submitted_through,
        1,
        "stored before the panic"
    );
    for _ in 0..50 {
        let now = run.tick();
        let _ = run.started.recorder.flush(&mut run.started.custody, now);
    }
    assert!(
        run.started.recorder.latched().is_none(),
        "[dup-bounded] a duplicate is not a conflict"
    );
    // The panic was a failure of the run (RecorderFault): the core issued
    // its closure record, record 2. Fifty flushes later the store holds
    // exactly what the core issued, once each.
    let issued = run.snapshot().evidence.issued;
    assert_eq!(issued, 2, "the closure record");
    assert_eq!(
        run.started.recorder.status().submitted_through,
        issued,
        "[dup-bounded] nothing grew beyond what was issued"
    );
    assert_eq!(
        run.started.recorder.retained().len() as u64,
        issued,
        "[dup-bounded] one retained copy each"
    );
    run.pump();
    // Acknowledged, the failed run finalizes: its terminal record is issued
    // and acknowledged too.
    let evidence = run.snapshot().evidence.clone();
    assert_eq!(
        evidence.acknowledged, evidence.issued,
        "[dup-bounded] acknowledged once each"
    );
    assert_eq!(run.snapshot().phase, RunPhase::Finalized);
    assert_eq!(run.snapshot().verdict, Verdict::Failed);
    assert_eq!(
        run.snapshot().failure_counts[FailureClass::RecorderFault.index()],
        1,
        "the panic is a failure"
    );
}

/// The worker's recheck (design section 9.4, W5): a journal whose name was
/// replaced during the run latches Identity before the record is published,
/// so nothing written to the unlinked inode is acknowledged.
#[test]
fn r10_a_replaced_journal_latches_before_publication() {
    let fixture = Fixture::provisioned(2, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    let journals = ino_of(&fixture, &state_path(&fixture, "journals"));
    let name = format::pool_name(run.started.index);
    fixture.world.fixture_remove(journals, &name);
    let replacement = fixture.world.fixture_entry(
        journals,
        &name,
        custody::store::io::FileType::Regular,
        (sim::STORE_UID, sim::STORE_GID, 0o600),
    );
    fixture.world.install(replacement, &vec![0u8; 18 * BLOCK]);
    run.start_run();
    assert_eq!(
        run.started.recorder.latched(),
        Some((Cause::Identity(1), 0)),
        "[ack-durable] a replaced journal latches before publication"
    );
    assert_eq!(
        run.snapshot().evidence.acknowledged,
        0,
        "[ack-durable] nothing written to a replaced journal is acknowledged"
    );
}

// ---------------------------------------------------------------------------
// K: real-core conformance (design sections 11.4 and 16.3, I9)
// ---------------------------------------------------------------------------

const WIDE: Config = Config {
    automatic_attempts: 2,
    recovery_budget: 4,
    recovery_spacing_millis: 1,
    lease_millis: 1_000_000,
    record_capacity: 64,
    control_reserve: 2,
    receipt_limit: 4,
    late_limit: 1,
    failure_detail_limit: 8,
    detail_chars: 64,
    incident_limit: 4,
};

fn kind_name(kind: &RecordKind) -> String {
    match kind {
        RecordKind::Control {
            fact: ControlFact::AdmissionClosed { reason, .. },
        } => format!("AdmissionClosed {reason:?}"),
        RecordKind::Control {
            fact: ControlFact::ShutdownRefused,
        } => "ShutdownRefused".into(),
        other => format!("{other:?}")
            .split([' ', '{'])
            .next()
            .unwrap_or_default()
            .to_string(),
    }
}

/// Every prefix of a journal image (its record blocks after `k` zeroed)
/// classifies without a malformed class.
fn assert_prefixes_not_malformed(image: &[u8], index: u32, c_pool: u32, label: &str) {
    let report = classify_bytes(image, ROOT, c_pool, index);
    assert!(
        !report.class.malformed(),
        "[grammar-conformance] {label}: {:?} {}",
        report.class,
        report.reason
    );
    for keep in 0..report.records.len() {
        let mut prefix = image.to_vec();
        for block in (keep + 2)..(report.records.len() + 2) {
            prefix[block * BLOCK..(block + 1) * BLOCK].fill(0);
        }
        prefix[BLOCK..2 * BLOCK].fill(0);
        let cut = classify_bytes(&prefix, ROOT, c_pool, index);
        assert!(
            !cut.class.malformed(),
            "[grammar-conformance] {label}, prefix {keep}: {:?} {}",
            cut.class,
            cut.reason
        );
    }
}

/// A run through the store that issues every record kind (the codec tests'
/// `every_kind`): a failed cleanup and its recovery, the terminal record, a
/// late owner after it, a refused shutdown and the incident's own recovery.
/// Every prefix of its journal classifies; sealed, it is Sealed.
#[test]
fn k01_every_record_kind_through_the_store_classifies() {
    let fixture = Fixture::provisioned(2, 64);
    let mut run = Run::claimed(&fixture, "owner", WIDE);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let ticket = run.admit(SlotKind::Process).expect("admitted");
    assert!(matches!(
        run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
        Ok(Completion::Deposited(_))
    ));
    let outcome = run.end_case(Clean::Fail);
    assert!(outcome.stopped && !outcome.resolved);
    assert!(matches!(
        run.serve(RequestOp::Retry { epoch: 0 }, Clean::Confirm),
        RequestOutcome::Executed(Response::Retried { resolved: true, .. })
    ));
    assert_eq!(run.snapshot().phase, RunPhase::Finalized);
    assert!(matches!(
        run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
        Ok(Completion::Late { .. })
    ));
    run.pump();
    assert!(matches!(
        run.serve(RequestOp::Shutdown, Clean::Confirm),
        RequestOutcome::Executed(Response::Shutdown(ShutdownDecision::Refused(_)))
    ));
    let before = run.recorded(&fixture).len();
    assert_eq!(
        run.serve(RequestOp::Retry { epoch: 0 }, Clean::Confirm),
        RequestOutcome::Executed(Response::RetryRefused(Refusal::StaleEpoch)),
        "[grammar-conformance] a stale recovery epoch"
    );
    assert_eq!(
        run.recorded(&fixture).len(),
        before,
        "[grammar-conformance] a refused retry records nothing"
    );
    assert!(matches!(
        run.serve(RequestOp::Retry { epoch: 1 }, Clean::Confirm),
        RequestOutcome::Executed(Response::Retried { resolved: true, .. })
    ));
    let sequence: Vec<String> = run.recorded(&fixture).iter().map(kind_name).collect();
    println!("real-core trace every kind: {sequence:?}");
    let kinds: BTreeSet<String> = sequence.into_iter().collect();
    for wanted in [
        "RunStarted",
        "CaseStarted",
        "ActionStarted",
        "ActionFailed",
        "ActionSettled",
        "CaseEnded",
        "AdmissionClosed Failed(UnexpectedCleanup)",
        "ShutdownRefused",
        "RecoveryRequired",
        "RecoveryAttempt",
        "RunEnded",
        "IncidentOpened",
        "IncidentSettled",
    ] {
        assert!(
            kinds.contains(wanted),
            "[grammar-conformance] {wanted} not recorded: {kinds:?}"
        );
    }
    let (visible, durable) = run.journal(&fixture);
    assert_eq!(visible, durable, "every acknowledged block is durable");
    assert_prefixes_not_malformed(&visible, run.started.index, 64, "every kind");
    let report = classify_bytes(&visible, ROOT, 64, run.started.index);
    assert_eq!(report.class, FileClass::UnsealedAction);
    assert!(report.recorded_unsettled.is_empty());
    let index = run.started.index;
    assert_eq!(run.close_and_seal().expect("sealed"), SealState::Sealed);
    let pool = fixture.pool_ino(index).expect("pool");
    assert_eq!(
        classify_bytes(&fixture.world.visible(pool), ROOT, 64, index).class,
        FileClass::Sealed,
        "[grammar-conformance] sealed"
    );
}

/// The design's source-derived scenarios, run on the real core through the
/// store, record what their fixtures say (design section 11.5): the names
/// of the recorded kinds in order.
#[test]
fn k02_real_runs_record_the_source_derived_shapes() {
    let names = |run: &Run, fixture: &Fixture| -> Vec<String> {
        run.recorded(fixture).iter().map(kind_name).collect()
    };
    let fixture = Fixture::provisioned(2, 64);
    // T1: a clean pass. A cancellation after the terminal commitment is
    // late: it closes nothing, records nothing and the verdict stands.
    let mut run = Run::claimed(&fixture, "t1", WIDE);
    run.clean_pass();
    let now = run.tick();
    assert!(
        matches!(
            run.started.control.cancel(CancelReason::Requested, now),
            CancelReceipt::Late(_)
        ),
        "[grammar-conformance] a cancellation after the commitment is late"
    );
    run.pump();
    assert_eq!(
        run.snapshot().verdict,
        Verdict::Passed,
        "[grammar-conformance] a late cancellation changes no verdict"
    );
    println!("real-core trace T1: {:?}", names(&run, &fixture));
    assert_eq!(
        names(&run, &fixture),
        [
            "RunStarted",
            "CaseStarted",
            "ActionStarted",
            "ActionSettled",
            "CaseEnded",
            "RunEnded"
        ],
        "[grammar-conformance] T1"
    );
    // T3: cancelled after admission.
    let fixture = Fixture::provisioned(2, 64);
    let mut run = Run::claimed(&fixture, "t3", WIDE);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let ticket = run.admit(SlotKind::Process).expect("admitted");
    let now = run.tick();
    assert!(matches!(
        run.started.control.cancel(CancelReason::Requested, now),
        CancelReceipt::Accepted(_)
    ));
    assert!(matches!(
        run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
        Ok(Completion::Deposited(_))
    ));
    run.end_case(Clean::Confirm);
    run.pump();
    println!("real-core trace T3: {:?}", names(&run, &fixture));
    assert_eq!(
        names(&run, &fixture),
        [
            "RunStarted",
            "CaseStarted",
            "ActionStarted",
            "AdmissionClosed Cancelled(Requested)",
            "ActionSettled",
            "CaseEnded",
            "RunEnded"
        ],
        "[grammar-conformance] T3"
    );
    assert_eq!(run.snapshot().verdict, Verdict::Failed);
    // T14: cancelled before admission (the reservation settles NotAdmitted).
    let fixture = Fixture::provisioned(2, 64);
    let mut run = Run::claimed(&fixture, "t14", WIDE);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let now = run.tick();
    let reservation = run
        .custody()
        .reserve(SlotKind::Process, now)
        .expect("reserved");
    run.pump();
    let now = run.tick();
    run.started.control.cancel(CancelReason::Requested, now);
    let now = run.tick();
    assert!(matches!(
        run.started
            .recorder
            .admit(&mut run.started.custody, reservation, now),
        Err(GateRefused::Core(_))
    ));
    run.end_case(Clean::Confirm);
    run.pump();
    let recorded = run.recorded(&fixture);
    println!("real-core trace T14: {:?}", names(&run, &fixture));
    assert!(
        recorded.iter().any(|kind| matches!(
            kind,
            RecordKind::ActionSettled {
                how: Settlement::NotAdmitted,
                ..
            }
        )),
        "[grammar-conformance] T14"
    );
    // T10: declared detached output passes; T6-like authority loss never resolves.
    let fixture = Fixture::provisioned(2, 64);
    let mut run = Run::claimed(&fixture, "t10", WIDE);
    run.start_run();
    run.begin(1, Expectation::OutputDetached);
    let ticket = run.admit(SlotKind::Process).expect("admitted");
    let end = EndFacts {
        subtree: true,
        reaped: true,
        output: OutputObserved::Detached,
        removed: false,
    };
    assert!(matches!(
        run.complete(&ticket, NativeOutcome::Ended(end)),
        Ok(Completion::OutputLost)
    ));
    run.end_case(Clean::Confirm);
    run.finish();
    assert_eq!(
        run.snapshot().verdict,
        Verdict::Passed,
        "[grammar-conformance] T10 declared detachment passes"
    );
    let fixture = Fixture::provisioned(2, 64);
    let mut run = Run::claimed(&fixture, "t6", WIDE);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let ticket = run.admit(SlotKind::Process).expect("admitted");
    let end = EndFacts {
        subtree: false,
        reaped: false,
        output: OutputObserved::NotLooked,
        removed: false,
    };
    assert!(matches!(
        run.complete(&ticket, NativeOutcome::Ended(end)),
        Ok(Completion::AuthorityLost)
    ));
    run.end_case(Clean::Confirm);
    let report = classify_bytes(&run.journal(&fixture).0, ROOT, 64, run.started.index);
    assert_eq!(report.class, FileClass::UnsealedAction);
    assert!(
        report.refused && report.recorded_unsettled == vec![classify::Unsettled::Action(1)],
        "[no-false-resolution] authority lost: {report:?}"
    );
    // Foreign, gap, duplicate, conflicting and stale (replayed) requests
    // execute nothing and record nothing; the next request in sequence
    // executes, and only it changes the journal.
    let fixture = Fixture::provisioned(2, 64);
    let mut run = Run::claimed(&fixture, "requests", WIDE);
    run.start_run();
    let generation = run.started.custody.generation();
    let before = run.recorded(&fixture).len();
    let send = |run: &mut Run, generation: Generation, seq: u64, op: RequestOp| {
        run.started
            .control
            .submit(Request {
                generation,
                seq,
                op,
            })
            .expect("queued");
        let now = run.tick();
        let mut clean = Clean::Confirm;
        let (_, outcome) = run.started.custody.serve(&mut clean, now).expect("served");
        run.pump();
        outcome
    };
    let retry = RequestOp::Retry { epoch: 0 };
    let refused = Response::RetryRefused(Refusal::NotRecoveryRequired);
    assert_eq!(
        send(
            &mut run,
            Generation::new([0xab; 16]),
            1,
            RequestOp::Shutdown
        ),
        RequestOutcome::Foreign,
        "[grammar-conformance] a foreign request"
    );
    assert_eq!(
        send(&mut run, generation, 5, retry),
        RequestOutcome::Gap,
        "[grammar-conformance] a gap"
    );
    assert_eq!(
        send(&mut run, generation, 1, retry),
        RequestOutcome::Executed(refused.clone())
    );
    assert_eq!(
        send(&mut run, generation, 1, retry),
        RequestOutcome::Duplicate(refused.clone()),
        "[grammar-conformance] a duplicate answers its first response"
    );
    assert_eq!(
        send(&mut run, generation, 1, RequestOp::Shutdown),
        RequestOutcome::Conflict,
        "[grammar-conformance] a conflicting payload"
    );
    for seq in 2..=5 {
        assert_eq!(
            send(&mut run, generation, seq, retry),
            RequestOutcome::Executed(refused.clone())
        );
    }
    assert_eq!(
        send(&mut run, generation, 1, RequestOp::Shutdown),
        RequestOutcome::Replayed,
        "[grammar-conformance] a stale sequence number, its receipt gone"
    );
    assert_eq!(
        run.started.control.status().control.admission.closure,
        None,
        "[grammar-conformance] no refused shutdown closed admission"
    );
    assert_eq!(
        run.recorded(&fixture).len(),
        before,
        "[grammar-conformance] requests that execute nothing record nothing"
    );
    assert!(matches!(
        send(&mut run, generation, 6, RequestOp::Shutdown),
        RequestOutcome::Executed(Response::Shutdown(_))
    ));
    let after = names(&run, &fixture);
    assert!(
        after[before..]
            .iter()
            .any(|name| name == "AdmissionClosed Cancelled(Shutdown)"),
        "[grammar-conformance] the executed shutdown's closure is recorded: {after:?}"
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum A {
    Admit,
    AdmitRetained,
    Created,
    Retained,
    Late,
    Timeout,
    EndOk,
    EndFail,
    EndDetached,
    Cancel,
    Assert,
    Retry,
    Shutdown,
    Finish,
    Latch,
}

const ADVERSARY: [A; 15] = [
    A::Admit,
    A::AdmitRetained,
    A::Created,
    A::Retained,
    A::Late,
    A::Timeout,
    A::EndOk,
    A::EndFail,
    A::EndDetached,
    A::Cancel,
    A::Assert,
    A::Retry,
    A::Shutdown,
    A::Finish,
    A::Latch,
];

/// The adversarial runs' configuration: room for every sequence's records
/// in a 24-block pool file.
const ADVERSARIAL: Config = Config {
    record_capacity: 24,
    ..WIDE
};

/// One adversarial run through the store on the real core.
struct Adversary {
    run: Run,
    tickets: Vec<OpTicket>,
    case: u32,
}

impl Adversary {
    fn new(fixture: &Fixture) -> Adversary {
        let mut run = Run::claimed(fixture, "adversary", ADVERSARIAL);
        run.start_run();
        Adversary {
            run,
            tickets: Vec::new(),
            case: 0,
        }
    }

    fn apply(&mut self, event: A) {
        let run = &mut self.run;
        match event {
            A::Admit | A::AdmitRetained => {
                if run.snapshot().case.is_none() {
                    self.case += 1;
                    let now = run.tick();
                    let expectation = if event == A::AdmitRetained {
                        Expectation::RetainedBoundary
                    } else {
                        Expectation::Clean
                    };
                    let _ = run
                        .custody()
                        .begin_case(CaseId(self.case), expectation, now);
                    run.pump();
                }
                let now = run.tick();
                if let Ok(reservation) = run.custody().reserve(SlotKind::Process, now) {
                    run.pump();
                    let now = run.tick();
                    if let Ok(ticket) =
                        run.started
                            .recorder
                            .admit(&mut run.started.custody, reservation, now)
                    {
                        self.tickets.push(ticket);
                    }
                }
            }
            A::Created | A::Retained | A::Late => {
                let outcome = |kind| match event {
                    A::Retained => NativeOutcome::Retained(Token(kind)),
                    _ => NativeOutcome::Created(Token(kind)),
                };
                let ticket = if event == A::Late {
                    self.tickets.first()
                } else {
                    self.tickets.last()
                };
                if let Some(ticket) = ticket {
                    let now = run.tick();
                    if let Err(rejected) =
                        run.custody()
                            .complete(ticket, outcome(SlotKind::Process), now)
                    {
                        let _ = rejected.into_owner();
                    }
                }
            }
            A::Timeout => {
                if let Some(ticket) = self.tickets.last() {
                    let now = run.tick();
                    let _ = run.custody().complete(ticket, NativeOutcome::Unknown, now);
                }
            }
            A::EndOk | A::EndFail | A::EndDetached => {
                if run.snapshot().case.is_some() {
                    let clean = match event {
                        A::EndOk => Clean::Confirm,
                        A::EndFail => Clean::Fail,
                        _ => Clean::Detach,
                    };
                    run.end_case(clean);
                }
            }
            A::Cancel => {
                let now = run.tick();
                run.started.control.cancel(CancelReason::Requested, now);
                let now = run.tick();
                let _ = run.custody().observe_control(now);
            }
            A::Assert => {
                let now = run.tick();
                let _ = run.custody().record_assertion_failure("adversary", now);
            }
            A::Retry => {
                let epoch = run.snapshot().recovery.epoch;
                if run.started.control.status().control.queued {
                    return;
                }
                let _ = run.serve(RequestOp::Retry { epoch }, Clean::Confirm);
            }
            A::Shutdown => {
                if !run.started.control.status().control.queued {
                    let _ = run.serve(RequestOp::Shutdown, Clean::Confirm);
                }
            }
            A::Finish => {
                let now = run.tick();
                let _ = run.custody().finish_run(now);
            }
            A::Latch => {
                run.started.recorder.exchange().latch(Cause::WorkerLost);
            }
        }
        run.custody().take_released();
        run.pump();
    }
}

/// Adversarial event sequences on the real core, through the store: after
/// every event, the visible and the durable journal classify without a
/// malformed class (I9), and whenever the custody holds, has open or has
/// lost a native owner or operation, the durable journal is refused (INV-3).
#[test]
fn k03_adversarial_real_core_sequences_classify_and_refuse() {
    let base = Fixture::provisioned(2, 24);
    let mut sequences: Vec<Vec<A>> = Vec::new();
    for a in ADVERSARY {
        for b in ADVERSARY {
            for c in ADVERSARY {
                sequences.push(vec![a, b, c]);
            }
        }
    }
    let witnesses: Vec<Vec<A>> = vec![
        vec![
            A::Admit,
            A::Created,
            A::EndFail,
            A::Retry,
            A::Late,
            A::Retry,
        ],
        vec![
            A::Admit,
            A::Created,
            A::Cancel,
            A::EndOk,
            A::Finish,
            A::Late,
        ],
        vec![A::AdmitRetained, A::Retained, A::EndOk, A::Finish],
        vec![A::Admit, A::Timeout, A::EndOk, A::Retry, A::Late, A::Retry],
        vec![A::Admit, A::Created, A::Latch, A::EndOk, A::Finish],
        vec![A::Admit, A::Created, A::EndDetached, A::Finish, A::Shutdown],
        vec![
            A::Admit,
            A::Created,
            A::EndOk,
            A::Finish,
            A::Late,
            A::Shutdown,
            A::Retry,
        ],
    ];
    sequences.extend(witnesses);
    let mut checked = 0u64;
    // Equal bytes classify equally (c02): an image already classified is
    // not hashed again.
    let mut seen: BTreeMap<Vec<u8>, classify::FileReport> = BTreeMap::new();
    let mut classify_once = |image: &Vec<u8>, index: u32| -> classify::FileReport {
        if let Some(report) = seen.get(image) {
            return report.clone();
        }
        let report = classify_bytes(image, ROOT, 24, index);
        if seen.len() < 4096 {
            seen.insert(image.clone(), report.clone());
        }
        report
    };
    for sequence in &sequences {
        let fixture = Fixture {
            world: base.world.fork(),
            root: base.root.clone(),
            path: base.path.clone(),
            layout: base.layout.clone(),
            qualification: base.qualification.clone(),
        };
        let mut adversary = Adversary::new(&fixture);
        for (step, event) in sequence.iter().enumerate() {
            adversary.apply(*event);
            let run = &adversary.run;
            let (visible, durable) = run.journal(&fixture);
            let label = format!("{:?} after {}", sequence, step + 1);
            let index = run.started.index;
            for (name, image) in [("visible", &visible), ("durable", &durable)] {
                let report = classify_once(image, index);
                assert!(
                    !report.class.malformed(),
                    "[grammar-conformance] {label} ({name}): {:?} {}",
                    report.class,
                    report.reason
                );
            }
            let snapshot = run.snapshot();
            let holds = !snapshot.entries.is_empty()
                || snapshot.operation.is_some()
                || !snapshot.lost.is_empty();
            if holds {
                let report = classify_once(&durable, index);
                assert!(
                    report.refused,
                    "[no-false-resolution] {label}: native state held, yet {:?} not refused",
                    report.class
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 10_000, "{checked}");
}

/// T8 through the real core (design sections 11.5 and 12.1, INV-3): a
/// passed, finalized run is delivered a late owner after its terminal
/// record, and the owner dies before that incident's record is
/// acknowledged: never submitted, taken by the worker but not written, or
/// durable but not published. Whatever the journal shows (T1's records
/// only, or T1's and the incident's), it stays a current incident, carries
/// the late-owner note, and the next owner is refused.
#[test]
fn k04_a_late_owner_without_an_acknowledged_record_still_blocks() {
    let t1 = [
        "RunStarted",
        "CaseStarted",
        "ActionStarted",
        "ActionSettled",
        "CaseEnded",
        "RunEnded",
    ];
    // How many worker steps the incident's record got before the owner died.
    for (label, steps, shows_incident) in [
        ("never submitted", None, false),
        ("taken, not written", Some(1), false),
        ("durable, not published", Some(2), true),
    ] {
        let fixture = Fixture::provisioned(2, 64);
        let mut run = Run::claimed(&fixture, "t8", WIDE);
        run.start_run();
        run.begin(1, Expectation::Clean);
        let ticket = run.admit(SlotKind::Process).expect("admitted");
        assert!(matches!(
            run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
            Ok(Completion::Deposited(_))
        ));
        assert!(run.end_case(Clean::Confirm).passed);
        run.finish();
        assert_eq!(run.snapshot().phase, RunPhase::Finalized);
        let acknowledged = run.snapshot().evidence.acknowledged;
        assert!(matches!(
            run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process))),
            Ok(Completion::Late { .. })
        ));
        if let Some(steps) = steps {
            let now = run.tick();
            run.started
                .recorder
                .flush(&mut run.started.custody, now)
                .expect("flushed");
            for _ in 0..steps {
                assert_eq!(run.started.worker.step(), Step::Progress, "{label}");
            }
        }
        assert_eq!(
            run.snapshot().evidence.acknowledged,
            acknowledged,
            "{label}: the incident's record is not acknowledged"
        );
        let index = run.started.index;
        fixture.world.kill(&run.owner);
        drop(run);
        fixture
            .world
            .power_loss(None, sim::Tear::Old, &BTreeMap::new());
        let pool = fixture.pool_ino(index).expect("pool");
        let report = classify_bytes(&fixture.world.durable(pool), ROOT, 64, index);
        let names: Vec<String> = report
            .records
            .iter()
            .map(|record| kind_name(&record.kind))
            .collect();
        let mut expected: Vec<String> = t1.iter().map(|name| name.to_string()).collect();
        if shows_incident {
            expected.push("IncidentOpened".into());
        }
        println!(
            "real-core trace T8 ({label}): {names:?}, {:?}, refused {}",
            report.class, report.refused
        );
        assert_eq!(names, expected, "[late-uncertain] {label}: the journal");
        assert_eq!(report.class, FileClass::UnsealedAction, "{label}");
        assert!(
            report.refused,
            "[no-false-resolution] {label}: {:?}",
            report.class
        );
        assert!(
            report.notes.contains(&classify::NOTE_LATE),
            "[late-uncertain] {label}: the late-owner note: {:?}",
            report.notes
        );
        let unsettled = if shows_incident {
            vec![classify::Unsettled::Incident(1)]
        } else {
            Vec::new()
        };
        assert_eq!(report.recorded_unsettled, unsettled, "{label}");
        match Run::start(&fixture, "next", WIDE) {
            Err(StartRefused::PriorUnresolved(blocking)) => assert_eq!(
                blocking.len(),
                1,
                "[no-false-resolution] {label}: the generation blocks"
            ),
            Err(other) => panic!("[no-false-resolution] {label}: {other:?}"),
            Ok(_) => panic!("[no-false-resolution] {label}: the next owner ran"),
        }
    }
}

// ---------------------------------------------------------------------------
// M: maintenance sessions and procedures (design section 13), their crash
// matrix (section 15.2) and their protocol steps (section 15.3)
// ---------------------------------------------------------------------------

use custody::store::maintenance::{self as maint, Session, SessionReport};
use std::cell::RefCell;
use std::rc::Rc;

const DESIGN: &str =
    include_str!("../../../docs/architecture/p2-custody-durable-recorder-design.md");

/// A store whose pool file 0 holds an unsealed generation that recorded an
/// action start (its owner died): one current, blocking incident.
fn store_with_incident(pool: u32) -> Fixture {
    let fixture = Fixture::provisioned(pool, 16);
    let mut run = Run::claimed(&fixture, "owner", SMALL);
    run.start_run();
    run.begin(1, Expectation::Clean);
    let ticket = run.admit(SlotKind::Process).expect("admitted");
    let _ = run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process)));
    fixture.world.kill(&run.owner);
    drop(run);
    fixture
}

/// A maintenance session shared by a procedure's steps and its test.
type SharedSession = Rc<RefCell<Session<SimIo>>>;

/// A staged procedure and, when it verifies in its session first, that
/// verification's report.
type Built = (maint::Procedure<'static>, Option<SessionReport>);

fn session(fixture: &Fixture, requalify: bool) -> SharedSession {
    let io = fixture.root_process("session");
    match maint::begin(io, &fixture.path, requalify) {
        Ok(session) => Rc::new(RefCell::new(session)),
        Err(refusal) => panic!("a session: {refusal:?}"),
    }
}

fn end(session: SharedSession) {
    match Rc::try_unwrap(session) {
        Ok(session) => session.into_inner().end(),
        Err(_) => panic!("the session is still shared"),
    }
}

fn verify(session: &SharedSession) -> SessionReport {
    session
        .borrow_mut()
        .verify(&SMALL)
        .expect("[session-verify] in-session verification")
}

fn quiet() -> impl FnMut(Option<&'static str>) {
    |_| {}
}

/// Publish a disposition for the one current incident, in its own session.
fn publish_only_incident(fixture: &Fixture) -> [u8; 32] {
    let session = session(fixture, false);
    let report = verify(&session);
    let (binding, incident) = report.current.first().cloned().expect("one incident");
    let disposition = maint::disposition_for(
        &ROOT,
        binding,
        &incident,
        DispositionReason::OwnerDestroyed,
        "the owner process was destroyed",
        "owner",
        "2026-10-01T12:00:00Z",
    );
    let mut procedure = maint::publish_disposition(&session.borrow(), disposition)
        .expect("[archive-binding] a disposition for the binding the store computed");
    procedure.run_all(&mut quiet()).expect("published");
    drop(procedure);
    let after = verify(&session);
    assert!(
        after.dispositioned.contains(&binding),
        "[admin-crash] verify-after shows the disposition"
    );
    end(session);
    binding
}

/// Verification inside a session (design sections 13.1 and 13.11): no lock
/// request on the store lock, competitors excluded for the whole session,
/// nested procedures in one lock lifetime, authority only from the retained
/// lock (a dead session is NotAuthorized).
#[test]
fn m01_sessions_verify_through_their_retained_lock() {
    let fixture = store_with_incident(2);
    let session = session(&fixture, false);
    let lock = ino_of(&fixture, &state_path(&fixture, "LOCK"));
    fixture.world.take_interactions();
    let pid = session.borrow().io().pid();
    let report = verify(&session);
    assert_eq!(report.blocking.len(), 1);
    let log = interactions_of(&fixture, pid);
    assert!(
        !log.iter()
            .any(|entry| entry.op.starts_with("flock") && entry.ino == Some(lock)),
        "[session-verify] the verification requested the store lock"
    );
    assert_eq!(
        fixture.world.lock_holders(lock),
        Some((true, 1)),
        "[session-verify] the session's exclusive lock held throughout"
    );
    assert_eq!(
        refused_store(
            Run::start(&fixture, "competitor", SMALL),
            "[session-verify] an owner is excluded"
        ),
        Refused::Busy,
        "[session-verify] an owner is excluded"
    );
    assert_eq!(
        verify_standalone(&fixture.store_process("verifier"), &fixture.path, &SMALL)
            .expect_err("[session-verify] the standalone verifier is Busy, never success")
            .refused,
        Refused::Busy,
        "[session-verify] the standalone verifier is Busy, never success"
    );
    // Nested: a disposition published and verified again, in one lifetime.
    let (binding, incident) = report.current[0].clone();
    let disposition = maint::disposition_for(
        &ROOT,
        binding,
        &incident,
        DispositionReason::Other,
        "x",
        "owner",
        "2026-10-01T12:00:00Z",
    );
    let mut procedure =
        maint::publish_disposition(&session.borrow(), disposition).expect("procedure");
    procedure.run_all(&mut quiet()).expect("published");
    drop(procedure);
    let again = verify(&session);
    assert!(
        again.blocking.is_empty(),
        "[session-verify] verify-after scans afresh"
    );
    assert!(
        session
            .borrow()
            .events
            .iter()
            .filter(|event| matches!(event, maint::SessionEvent::Verify(_)))
            .count()
            >= 2
    );
    // A session whose process died holds nothing: NotAuthorized.
    let io = session.borrow().io().clone();
    fixture.world.kill(&io);
    let refused = session
        .borrow_mut()
        .verify(&SMALL)
        .expect_err("[session-verify] authority only from the retained lock");
    assert_eq!(
        refused.refused,
        Refused::NotAuthorized,
        "[session-verify] authority only from the retained lock"
    );
}

/// Dispositions (design section 13.4): a published disposition lets the
/// next owner run, with the incident applied in its header and counted in
/// its `RunStarted`; a byte change of the journal makes a new binding the
/// old disposition does not cover (never silently honoured); revocation
/// blocks again.
#[test]
fn m02_dispositions_apply_and_are_never_silently_stale() {
    let fixture = store_with_incident(2);
    match Run::start(&fixture, "blocked", SMALL) {
        Err(StartRefused::PriorUnresolved(blocking)) => {
            assert_eq!(blocking.len(), 1, "[admin-crash] the incident blocks")
        }
        other => panic!("{:?}", other.err()),
    }
    let binding = publish_only_incident(&fixture);
    let mut run = Run::claimed(&fixture, "next", SMALL);
    assert_eq!(
        run.started.header.applied,
        vec![(binding, DispositionReason::OwnerDestroyed)],
        "[admin-crash] the header lists the applied binding"
    );
    run.start_run();
    assert!(
        matches!(
            run.recorded(&fixture).first(),
            Some(RecordKind::RunStarted { dispositioned: 1 })
        ),
        "[admin-crash] RunStarted counts it"
    );
    let index = run.started.index;
    let finished = {
        run.begin(1, Expectation::Clean);
        let ticket = run.admit(SlotKind::Process).expect("admitted");
        let _ = run.complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process)));
        run.end_case(Clean::Confirm);
        run.finish();
        run
    };
    assert_eq!(
        finished.close_and_seal().expect("sealed"),
        SealState::Sealed
    );
    assert_ne!(index, 0);
    // The incident is history now (applied, dispositioned).
    let opened = open_owner(
        &fixture.store_process("check"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect("opens");
    assert!(
        opened.scan.level.history.contains(&binding),
        "[admin-crash] applied and dispositioned: history"
    );
    drop(opened);
    // Domain A: one byte of the old journal's first record changes. A new
    // binding (a malformed journal), which no disposition covers: it blocks.
    fixture
        .world
        .overwrite_visible(fixture.pool_ino(0).expect("pool"), 2 * BLOCK + 20, &[0x55]);
    let opened = open_owner(
        &fixture.store_process("after-change"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect("opens");
    assert_eq!(
        opened.decision.blocking.len(),
        1,
        "[archive-binding] a changed binding is never silently honoured"
    );
    assert!(!opened.decision.blocking.contains(&binding));
}

/// Revocation (design section 13.4): the incident blocks again; revoked
/// files stay as evidence.
#[test]
fn m03_revocation_blocks_again() {
    let fixture = store_with_incident(2);
    let binding = publish_only_incident(&fixture);
    let session = session(&fixture, false);
    let mut procedure = maint::revoke(&session.borrow(), binding, "20261001T130000Z");
    procedure.run_all(&mut quiet()).expect("revoked");
    drop(procedure);
    let report = verify(&session);
    assert_eq!(
        report.blocking,
        vec![binding],
        "[admin-crash] revoked: blocks again"
    );
    assert_eq!(
        report.scan.revoked.len(),
        1,
        "revoked files stay as evidence"
    );
    end(session);
}

/// Archival keeps the binding; recycling makes the pool file fresh;
/// retirement needs its preconditions over the complete set; a successor
/// keeps the predecessor's `PROVISION` and root (design sections 13.6 to
/// 13.9).
#[test]
fn m04_archival_recycling_retirement_and_succession() {
    let fixture = store_with_incident(2);
    let binding = publish_only_incident(&fixture);
    // Retirement refused while claim 1 is only dispositioned in the pool.
    let session_a = session(&fixture, false);
    let report = verify(&session_a);
    assert!(
        maint::retire(Rc::clone(&session_a), 1, &report).is_none(),
        "[admin-crash] retirement without its preconditions: claim 1 is still in the pool"
    );
    let name = maint::archival_allowed(&report, 0).expect("archivable");
    let mut procedure = maint::archive(&mut session_a.borrow_mut(), 0, &name).expect("procedure");
    procedure.run_all(&mut quiet()).expect("archived");
    drop(procedure);
    let after = verify(&session_a);
    assert!(
        after.scan.level.pending_recycle.contains(&0),
        "[archive-binding] archived, pending recycling"
    );
    assert!(
        after.scan.level.history.contains(&binding),
        "[archive-binding] the archive keeps the binding"
    );
    let mut procedure = maint::recycle(Rc::clone(&session_a), 0, &after).expect("procedure");
    procedure.run_all(&mut quiet()).expect("recycled");
    drop(procedure);
    let recycled = verify(&session_a);
    assert_eq!(
        recycled.scan.files[0].class,
        FileClass::Unused,
        "[admin-crash] recycled"
    );
    assert_eq!(recycled.revision, 2);
    let mut procedure =
        maint::retire(Rc::clone(&session_a), 1, &recycled).expect("retirement allowed");
    procedure.run_all(&mut quiet()).expect("retired");
    drop(procedure);
    let retired = verify(&session_a);
    assert_eq!(session_a.borrow().selection().provision.retired_through, 1);
    assert!(
        retired.current.is_empty(),
        "[admin-crash] retired history leaves the decision"
    );
    end(session_a);
    // Succession.
    let mut successor_layout = fixture.layout.clone();
    successor_layout.root_id = sim::SUCCESSOR_ID;
    successor_layout.state_name = format!("uid-{}-{}", sim::STORE_UID, hexs(&sim::SUCCESSOR_ID));
    let predecessor_root = fixture.store_ino(&[]);
    let session_b = session(&fixture, false);
    let mut procedure = maint::successor(
        Rc::clone(&session_b),
        &successor_layout,
        "predecessor fully dispositioned",
    )
    .expect("procedure");
    procedure.run_all(&mut quiet()).expect("succession");
    drop(procedure);
    assert_eq!(
        session_b.borrow().selection().provision.root_id,
        sim::SUCCESSOR_ID
    );
    end(session_b);
    let opened = open_owner(
        &fixture.store_process("successor-owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect("opens the successor");
    assert_eq!(
        opened.selection.provision.root_id,
        sim::SUCCESSOR_ID,
        "[provision-selection] the successor is selected"
    );
    assert_eq!(
        opened
            .selection
            .provision
            .predecessor
            .as_ref()
            .map(|(id, _)| *id),
        Some(ROOT)
    );
    let kept = fixture.world.lookup(&format!(
        "{PROVDIR}/uid-1001.provision.predecessor-{}",
        hexs(&ROOT)
    ));
    assert!(
        kept.is_some(),
        "[provision-selection] the predecessor's PROVISION is kept"
    );
    assert_eq!(
        fixture.store_ino(&[]),
        predecessor_root,
        "the predecessor's root is unchanged"
    );
}

/// A leftover temporary refuses as MaintenanceIncomplete until a session
/// removes it; a mount option change refuses until re-qualification
/// (design sections 13.1 and 13.3).
#[test]
fn m05_leftovers_and_requalification() {
    use custody::store::io::FileType;
    let fixture = Fixture::provisioned(2, 16);
    let dispositions = ino_of(&fixture, &state_path(&fixture, "dispositions"));
    fixture
        .world
        .fixture_entry(dispositions, ".tmp-0000", FileType::Regular, (0, 0, 0o444));
    let refused = open_owner(
        &fixture.store_process("owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect_err("[admin-crash] a leftover temporary");
    assert!(
        matches!(refused.refused, Refused::MaintenanceIncomplete(_)),
        "[admin-crash] {refused:?}"
    );
    let session_a = session(&fixture, false);
    let mut procedure = maint::leftover(&session_a.borrow()).expect("procedure");
    assert_eq!(procedure.len(), 2);
    procedure.run_all(&mut quiet()).expect("removed");
    drop(procedure);
    verify(&session_a);
    end(session_a);
    assert!(open_owner(
        &fixture.store_process("owner-2"),
        &fixture.path,
        &SMALL,
        &mut NoHooks
    )
    .is_ok());
    // Options changed: Unsupported until re-qualification.
    let mut fixture = Fixture::provisioned(2, 16);
    fixture.world.with_host(|host| {
        let (_, superblock) = host.store_mount_options();
        host.set_store_mount_options("rw,relatime,noatime", &superblock);
    });
    let refused = open_owner(
        &fixture.store_process("owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect_err("[storage-qualification] options changed");
    assert_eq!(
        refused.refused,
        Refused::Unsupported("options changed".into()),
        "[storage-qualification] options changed"
    );
    let qualification = fixture.qualify().expect("re-qualified");
    let session_b = session(&fixture, true);
    let mut procedure = maint::requalify(Rc::clone(&session_b), qualification).expect("procedure");
    procedure.run_all(&mut quiet()).expect("rewritten");
    drop(procedure);
    end(session_b);
    let opened = open_owner(
        &fixture.store_process("owner-3"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect("re-qualified");
    assert_eq!(opened.selection.revision(), 2);
}

/// After Unprovisioned, a root that may hold history is re-published, never
/// replaced by a fresh one (design section 13.2, recovery).
#[test]
fn m06_an_unprovisioned_root_with_history_is_republished() {
    let fixture = store_with_incident(2);
    let provdir = ino_of(&fixture, PROVDIR);
    fixture.world.fixture_remove(provdir, "uid-1001.provision");
    let refused = open_owner(
        &fixture.store_process("owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    )
    .expect_err("[composed-recovery] unprovisioned");
    assert_eq!(refused.refused, Refused::Unprovisioned);
    let roots: Vec<String> = sim::ROOTS.iter().map(|part| part.to_string()).collect();
    let found = maint::roots_with_history(&fixture.root, &roots).expect("listed");
    assert_eq!(
        found,
        vec![fixture.layout.state_name.clone()],
        "[composed-recovery] the root with history"
    );
    let qualification = fixture.qualification.clone().expect("qualified");
    let mut procedure =
        maint::republish(&fixture.root, &fixture.layout, &qualification).expect("procedure");
    procedure.run_all(&mut quiet()).expect("republished");
    match Run::start(&fixture, "after", SMALL) {
        Err(StartRefused::PriorUnresolved(blocking)) => assert_eq!(
            blocking.len(),
            1,
            "[composed-recovery] the history is in the decision"
        ),
        other => panic!("[composed-recovery] {:?}", other.err()),
    }
}

/// Directory counts are refused before any entry is examined; reports
/// beyond their bound say so (design sections 6.6, 7.7 and 11.2).
#[test]
fn m07_aggregate_bounds_refuse_before_any_entry_is_examined() {
    use custody::store::io::FileType;
    for (dir, limit) in [
        ("dispositions", format::DISPOSITIONS_ENTRY_LIMIT),
        ("dispositions/revoked", format::REVOKED_ENTRY_LIMIT),
        ("archive", format::ARCHIVE_ENTRY_LIMIT),
    ] {
        let fixture = Fixture::provisioned(2, 16);
        let target = ino_of(&fixture, &state_path(&fixture, dir));
        for k in 0..=limit {
            fixture.world.fixture_entry(
                target,
                &format!("{}.disposition", hexs(&sha256(&k.to_be_bytes()))),
                FileType::Regular,
                (0, 0, 0o444),
            );
        }
        fixture.world.take_interactions();
        let owner = fixture.store_process("owner");
        let refused = open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks)
            .expect_err(&format!("[aggregate-bounds] {dir}"));
        assert!(
            matches!(&refused.refused, Refused::Invalid(why) if why.contains("enumeration bound")),
            "[aggregate-bounds] {dir}: {refused:?}"
        );
        let opened = interactions_of(&fixture, owner.pid())
            .iter()
            .filter(|entry| {
                entry.op == "openat2 read"
                    && entry
                        .name
                        .as_deref()
                        .is_some_and(|name| name.ends_with(".disposition"))
            })
            .count();
        assert_eq!(opened, 0, "[aggregate-bounds] {dir}: an entry was examined");
    }
}

/// Retirement's preconditions (design section 13.9) are evaluated over the
/// complete set of incidents, never the bounded report: of 70 claim gaps,
/// the 64 the report details are history and the other six are current, so
/// retirement through 70 is refused and through 64 allowed.
#[test]
fn m10_retirement_is_decided_from_the_complete_set() {
    let mut level = classify::PoolLevel::default();
    for claim in 1..=70u64 {
        level.incidents.insert(
            format::bind_gap(&ROOT, claim),
            Incident {
                facts: maint::gap_facts(claim),
                outcome: PriorOutcome::Malformed,
            },
        );
    }
    let detailed = classify::bounded_report(&level);
    assert!(detailed.partial);
    for (binding, _) in &detailed.detail {
        level.history.insert(*binding);
    }
    let report = classify::bounded_report(&level);
    let current = classify::current_incidents(&level);
    assert_eq!(current.len(), 6);
    let session_report = SessionReport {
        revision: 1,
        scan: open::ScanResult {
            files: Vec::new(),
            archive: Vec::new(),
            dispositions: BTreeMap::new(),
            revoked: Vec::new(),
            level,
            report,
            generations: BTreeSet::new(),
            preservation_sync_returned_zero: false,
        },
        current,
        blocking: Vec::new(),
        dispositioned: Vec::new(),
    };
    assert!(
        !maint::retirement_allowed(&session_report, 70),
        "[aggregate-bounds] retirement decided from the truncated report"
    );
    assert!(
        maint::retirement_allowed(&session_report, 64),
        "retirement through the history the complete set shows"
    );
}

// -- protocol steps (design section 15.3) ------------------------------------

/// The operation table of design section 15.3, parsed from the document.
fn design_operations() -> BTreeMap<String, Vec<String>> {
    let start = DESIGN.find("### 15.3").expect("15.3");
    let end = DESIGN.find("### 15.4").expect("15.4");
    let mut table = BTreeMap::new();
    for line in DESIGN[start..end].lines() {
        let cells: Vec<&str> = line.split(" | ").collect();
        if cells.len() == 3
            && (cells[0] == "| P-PROV"
                || cells[0].starts_with("| P-")
                || cells[0].starts_with("| R-"))
        {
            let name = cells[0].trim_start_matches("| ").to_string();
            let ops: Vec<String> = cells[2]
                .trim_end_matches(" |")
                .split("; ")
                .map(|op| {
                    op.split_once(' ')
                        .map(|(_, rest)| rest.to_string())
                        .unwrap_or_default()
                })
                .collect();
            table.insert(name, ops);
        }
    }
    table
}

/// Label the traced operations the way design section 15.3 names them.
fn label_trace(fixture: &Fixture, trace: &[sim::TraceOp], roots: &[&str]) -> Vec<String> {
    let mut dirs: BTreeMap<sim::Ino, &str> = BTreeMap::new();
    dirs.insert(ino_of(fixture, ROOTS_DIR), "parent");
    dirs.insert(ino_of(fixture, PROVDIR), "provdir");
    for root in roots {
        let base = format!("{ROOTS_DIR}/{root}");
        for (path, label) in [
            ("", "root"),
            ("/journals", "journals"),
            ("/dispositions", "dispositions"),
            ("/dispositions/revoked", "revoked"),
            ("/archive", "archive"),
        ] {
            if let Some(ino) = fixture.world.lookup(&format!("{base}{path}")) {
                dirs.insert(ino, label);
            }
        }
    }
    let file_label = |dir: &str, name: &str| -> String {
        let label = match dir {
            "root" => name.to_string(),
            "journals" if name.starts_with(".tmp-") => "tmp".into(),
            "journals" => "pool".into(),
            "dispositions" if name == "revoked" => "revoked".into(),
            "dispositions" | "archive" if name.starts_with(".tmp-") => "tmp".into(),
            "dispositions" | "archive" => "final".into(),
            "revoked" => "entry".into(),
            "provdir" if name.ends_with(".tmp") => "tmp".into(),
            "provdir" if name.contains(".predecessor-") => "predecessor".into(),
            "provdir" => "PROVISION".into(),
            "parent" => "root".into(),
            other => format!("{other}?"),
        };
        format!("{dir}/{label}")
    };
    let mut files: BTreeMap<sim::Ino, String> = BTreeMap::new();
    let mut out = Vec::new();
    for op in trace {
        let step = op.step.clone().unwrap_or_default();
        let dir = op
            .dir
            .and_then(|ino| dirs.get(&ino).copied())
            .unwrap_or("?");
        let object = match op.op {
            "mkdir" | "create" | "link" | "unlink" | "rename" => {
                let object = file_label(dir, op.name.as_deref().unwrap_or(""));
                if let Some(ino) = op.ino {
                    files.insert(ino, object.clone());
                }
                object
            }
            "write" | "fsync" => op
                .ino
                .and_then(|ino| files.get(&ino).cloned())
                .unwrap_or_else(|| "?".into()),
            "fsync_dir" => dir.to_string(),
            other => other.to_string(),
        };
        out.push(format!("{} `{object}` ({step})", op.op));
    }
    out
}

fn traced(fixture: &Fixture, mut procedure: maint::Procedure<'static>) -> Vec<sim::TraceOp> {
    let world = fixture.world.clone();
    world.trace_start();
    procedure
        .run_all(&mut |label| world.set_step(label))
        .expect("the procedure");
    drop(procedure);
    world.trace_take()
}

/// Every durability operation each Rust procedure performs is the one
/// design section 15.3 lists, in order, with its step label, and no other
/// (INV-17).
#[test]
fn m08_procedures_perform_exactly_the_documented_steps() {
    let table = design_operations();
    assert_eq!(table.len(), 11, "the table's rows");
    let state = |fixture: &Fixture| fixture.layout.state_name.clone();
    let check = |name: &str, fixture: &Fixture, trace: &[sim::TraceOp], roots: &[&str]| {
        let labelled = label_trace(fixture, trace, roots);
        assert_eq!(
            &labelled,
            table.get(name).expect(name),
            "[step-parity] {name}"
        );
    };
    // P-PROV.
    let mut fixture = Fixture::host_sized(HostFixture::qualified(), 1, 16);
    let qualification = fixture.qualify().expect("qualified");
    let (procedure, _) = maint::provision(&fixture.root, &fixture.layout, &qualification);
    let trace = traced(&fixture, procedure);
    check("P-PROV", &fixture, &trace, &[&state(&fixture)]);
    // P-DISP, P-REVOKE, P-ARCH, P-RECYCLE, P-RETIRE.
    let fixture = store_with_incident(1);
    let s = session(&fixture, false);
    let report = verify(&s);
    let (binding, incident) = report.current[0].clone();
    let disposition = maint::disposition_for(
        &ROOT,
        binding,
        &incident,
        DispositionReason::Other,
        "x",
        "owner",
        "2026-10-01T12:00:00Z",
    );
    let trace = traced(
        &fixture,
        maint::publish_disposition(&s.borrow(), disposition.clone()).expect("procedure"),
    );
    check("P-DISP", &fixture, &trace, &[&state(&fixture)]);
    let trace = traced(
        &fixture,
        maint::revoke(&s.borrow(), binding, "20261001T130000Z"),
    );
    check("P-REVOKE", &fixture, &trace, &[&state(&fixture)]);
    let mut disposition_again = disposition;
    disposition_again.at = "2026-10-01T14:00:00Z".into();
    traced(
        &fixture,
        maint::publish_disposition(&s.borrow(), disposition_again).expect("procedure"),
    );
    let report = verify(&s);
    let name = maint::archival_allowed(&report, 0).expect("archivable");
    let procedure = maint::archive(&mut s.borrow_mut(), 0, &name).expect("procedure");
    let trace = traced(&fixture, procedure);
    check("P-ARCH", &fixture, &trace, &[&state(&fixture)]);
    let report = verify(&s);
    let trace = traced(
        &fixture,
        maint::recycle(Rc::clone(&s), 0, &report).expect("procedure"),
    );
    check("P-RECYCLE", &fixture, &trace, &[&state(&fixture)]);
    let report = verify(&s);
    let trace = traced(
        &fixture,
        maint::retire(Rc::clone(&s), 1, &report).expect("allowed"),
    );
    check("P-RETIRE", &fixture, &trace, &[&state(&fixture)]);
    end(s);
    // P-REQUALIFY.
    let mut fixture = Fixture::provisioned(1, 16);
    fixture.world.with_host(|host| {
        let (_, superblock) = host.store_mount_options();
        host.set_store_mount_options("rw,relatime,noatime", &superblock);
    });
    let qualification = fixture.qualify().expect("qualified");
    let s = session(&fixture, true);
    let trace = traced(
        &fixture,
        maint::requalify(Rc::clone(&s), qualification).expect("procedure"),
    );
    check("P-REQUALIFY", &fixture, &trace, &[&state(&fixture)]);
    end(s);
    // P-SUCCESSOR.
    let fixture = Fixture::provisioned(1, 16);
    let mut successor_layout = fixture.layout.clone();
    successor_layout.root_id = sim::SUCCESSOR_ID;
    successor_layout.state_name = format!("uid-{}-{}", sim::STORE_UID, hexs(&sim::SUCCESSOR_ID));
    let s = session(&fixture, false);
    let trace = traced(
        &fixture,
        maint::successor(Rc::clone(&s), &successor_layout, "statement").expect("procedure"),
    );
    check(
        "P-SUCCESSOR",
        &fixture,
        &trace,
        &[&state(&fixture), &successor_layout.state_name],
    );
    end(s);
    // R-LEFTOVER, R-RESUME, R-REPUBLISH.
    let fixture = Fixture::provisioned(1, 16);
    let dispositions = ino_of(&fixture, &state_path(&fixture, "dispositions"));
    fixture.world.fixture_entry(
        dispositions,
        ".tmp-0000",
        custody::store::io::FileType::Regular,
        (0, 0, 0o444),
    );
    let s = session(&fixture, false);
    let trace = traced(&fixture, maint::leftover(&s.borrow()).expect("procedure"));
    check("R-LEFTOVER", &fixture, &trace, &[&state(&fixture)]);
    end(s);
    let fixture = store_with_incident(1);
    let provdir = ino_of(&fixture, PROVDIR);
    fixture.world.fixture_remove(provdir, "uid-1001.provision");
    fixture.world.fixture_entry(
        provdir,
        "uid-1001.provision.tmp",
        custody::store::io::FileType::Regular,
        (0, 0, 0o444),
    );
    let qualification = fixture.qualification.clone().expect("qualified");
    let trace = traced(
        &fixture,
        maint::republish(&fixture.root, &fixture.layout, &qualification).expect("procedure"),
    );
    check("R-REPUBLISH", &fixture, &trace, &[&state(&fixture)]);
}

// -- the crash matrix (design section 15.2) ----------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Proc {
    Prov,
    Disp,
    Revoke,
    Arch,
    Recycle,
    Retire,
    Requalify,
    Successor,
}

impl Proc {
    fn name(self) -> &'static str {
        match self {
            Proc::Prov => "P-PROV",
            Proc::Disp => "P-DISP",
            Proc::Revoke => "P-REVOKE",
            Proc::Arch => "P-ARCH",
            Proc::Recycle => "P-RECYCLE",
            Proc::Retire => "P-RETIRE",
            Proc::Requalify => "P-REQUALIFY",
            Proc::Successor => "P-SUCCESSOR",
        }
    }
}

/// One procedure, staged on its own fresh store, ready to run.
struct Staged {
    fixture: Fixture,
    procedure: maint::Procedure<'static>,
    actor: SimIo,
    saved: Option<SessionReport>,
    _session: Option<SharedSession>,
}

fn successor_layout(fixture: &Fixture) -> maint::Layout {
    let mut layout = fixture.layout.clone();
    layout.root_id = sim::SUCCESSOR_ID;
    layout.state_name = format!("uid-{}-{}", sim::STORE_UID, hexs(&sim::SUCCESSOR_ID));
    layout
}

fn stage(proc: Proc) -> Staged {
    let with_session =
        |fixture: Fixture, requalify: bool, build: &dyn Fn(&SharedSession) -> Built| {
            let s = session(&fixture, requalify);
            let (procedure, saved) = build(&s);
            let actor = s.borrow().io().clone();
            Staged {
                fixture,
                procedure,
                actor,
                saved,
                _session: Some(s),
            }
        };
    match proc {
        Proc::Prov => {
            let mut fixture = Fixture::host_sized(HostFixture::qualified(), 1, 16);
            let qualification = fixture.qualify().expect("qualified");
            let (procedure, _) = maint::provision(&fixture.root, &fixture.layout, &qualification);
            let actor = fixture.root.clone();
            Staged {
                fixture,
                procedure,
                actor,
                saved: None,
                _session: None,
            }
        }
        Proc::Disp => with_session(store_with_incident(1), false, &|s| {
            let report = verify(s);
            let (binding, incident) = report.current[0].clone();
            let disposition = maint::disposition_for(
                &ROOT,
                binding,
                &incident,
                DispositionReason::Other,
                "x",
                "owner",
                "2026-10-01T12:00:00Z",
            );
            (
                maint::publish_disposition(&s.borrow(), disposition).expect("procedure"),
                None,
            )
        }),
        Proc::Revoke => {
            let fixture = store_with_incident(1);
            let binding = publish_only_incident(&fixture);
            with_session(fixture, false, &|s| {
                (
                    maint::revoke(&s.borrow(), binding, "20261001T130000Z"),
                    None,
                )
            })
        }
        Proc::Arch => {
            let fixture = store_with_incident(1);
            publish_only_incident(&fixture);
            with_session(fixture, false, &|s| {
                let report = verify(s);
                let name = maint::archival_allowed(&report, 0).expect("archivable");
                let procedure = maint::archive(&mut s.borrow_mut(), 0, &name).expect("procedure");
                (procedure, None)
            })
        }
        Proc::Recycle | Proc::Retire => {
            let fixture = store_with_incident(1);
            publish_only_incident(&fixture);
            let s = session(&fixture, false);
            let report = verify(&s);
            let name = maint::archival_allowed(&report, 0).expect("archivable");
            maint::archive(&mut s.borrow_mut(), 0, &name)
                .expect("procedure")
                .run_all(&mut quiet())
                .expect("archived");
            end(s);
            if proc == Proc::Recycle {
                return with_session(fixture, false, &|s| {
                    let report = verify(s);
                    (
                        maint::recycle(Rc::clone(s), 0, &report).expect("procedure"),
                        Some(report),
                    )
                });
            }
            let s = session(&fixture, false);
            let report = verify(&s);
            maint::recycle(Rc::clone(&s), 0, &report)
                .expect("procedure")
                .run_all(&mut quiet())
                .expect("recycled");
            end(s);
            with_session(fixture, false, &|s| {
                let report = verify(s);
                (
                    maint::retire(Rc::clone(s), 1, &report).expect("allowed"),
                    None,
                )
            })
        }
        Proc::Requalify => {
            let mut fixture = Fixture::provisioned(1, 16);
            fixture.world.with_host(|host| {
                let (_, superblock) = host.store_mount_options();
                host.set_store_mount_options("rw,relatime,noatime", &superblock);
            });
            let qualification = fixture.qualify().expect("qualified");
            with_session(fixture, true, &|s| {
                (
                    maint::requalify(Rc::clone(s), qualification.clone()).expect("procedure"),
                    None,
                )
            })
        }
        Proc::Successor => {
            let fixture = Fixture::provisioned(1, 16);
            let layout = successor_layout(&fixture);
            with_session(fixture, false, &|s| {
                (
                    maint::successor(Rc::clone(s), &layout, "statement").expect("procedure"),
                    None,
                )
            })
        }
    }
}

fn view(fixture: &Fixture, world: SimWorld) -> Fixture {
    let root = world.process("evaluator-root", 0, 0);
    Fixture {
        world,
        root,
        path: fixture.path.clone(),
        layout: fixture.layout.clone(),
        qualification: fixture.qualification.clone(),
    }
}

/// What the next opening reports, in the vocabulary of design section 15.2.
fn outcome(fixture: &Fixture, proc: Proc) -> String {
    let owner = fixture.store_process("evaluator");
    match open_owner(&owner, &fixture.path, &SMALL, &mut NoHooks) {
        Err(refusal) => match refusal.refused {
            Refused::Unprovisioned => "unprovisioned".into(),
            Refused::MaintenanceIncomplete(_) => "maintenance".into(),
            Refused::Lost(_) => "lost".into(),
            Refused::Invalid(_) => "invalid".into(),
            Refused::Unsupported(_) => "unsupported".into(),
            other => format!("{other:?}"),
        },
        Ok(opened) => match proc {
            Proc::Prov => {
                let fresh = opened.decision.current.is_empty()
                    && opened
                        .scan
                        .files
                        .iter()
                        .all(|file| file.class == FileClass::Unused);
                if fresh {
                    "fresh".into()
                } else {
                    "not fresh".into()
                }
            }
            Proc::Disp | Proc::Revoke => {
                if !opened.decision.blocking.is_empty() {
                    "blocks".into()
                } else if !opened.decision.dispositioned.is_empty() {
                    "published".into()
                } else {
                    "neither".into()
                }
            }
            Proc::Arch | Proc::Recycle => {
                if opened.scan.files[0].class == FileClass::Unused {
                    "recycled".into()
                } else if opened.scan.level.pending_recycle.contains(&0)
                    && !opened.scan.archive.is_empty()
                {
                    "archived".into()
                } else if !opened.decision.dispositioned.is_empty() {
                    "published".into()
                } else {
                    "neither".into()
                }
            }
            Proc::Retire => format!("bound {}", opened.selection.provision.retired_through),
            Proc::Requalify => format!("revision {}", opened.selection.revision()),
            Proc::Successor => {
                if opened.selection.provision.root_id == ROOT {
                    "predecessor".into()
                } else {
                    "successor".into()
                }
            }
        },
    }
}

/// One row of the first table of design section 15.2: the procedure, the
/// first and last crash point it covers and its three outcome cells.
type DesignRow = (
    String,
    u64,
    u64,
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<String>,
);

/// The first table of design section 15.2, parsed: per procedure, the
/// crash points each row covers and its three outcome cells.
fn design_matrix() -> Vec<DesignRow> {
    let start = DESIGN.find("### 15.2").expect("15.2");
    let end = DESIGN[start..]
        .find("| Outcome |")
        .map(|at| start + at)
        .expect("end of the table");
    let set = |cell: &str| -> BTreeSet<String> {
        if cell.trim() == "\u{2014}" {
            BTreeSet::new()
        } else {
            cell.split(", ")
                .map(|item| item.trim().to_string())
                .collect()
        }
    };
    DESIGN[start..end]
        .lines()
        .filter(|line| line.starts_with("| P-"))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split(" | ").map(str::trim).collect();
            let (lo, hi) = match cells[1].split_once('\u{2013}') {
                Some((lo, hi)) => (lo.parse().expect("range"), hi.parse().expect("range")),
                None => {
                    let k: u64 = cells[1].parse().expect("point");
                    (k, k)
                }
            };
            (
                cells[0].to_string(),
                lo,
                hi,
                set(cells[2]),
                set(cells[3]),
                set(cells[4]),
            )
        })
        .collect()
}

/// Recovery from a refusing outcome (design sections 13.1, 13.8 and 15.2):
/// a session removes leftovers, then completes or repeats the procedure, or
/// resumes an interrupted recycling. Returns the final outcome.
fn recover(fixture: &Fixture, proc: Proc, saved: Option<&SessionReport>) -> String {
    let s = session(fixture, false);
    let leftovers = maint::leftover(&s.borrow()).expect("procedure");
    let mut leftovers = leftovers;
    leftovers.run_all(&mut quiet()).expect("leftovers removed");
    drop(leftovers);
    let report = s.borrow_mut().verify(&SMALL);
    match (proc, report) {
        (Proc::Recycle, Err(refusal)) => {
            let saved = saved.expect("the interrupted session's report");
            let mut resume =
                maint::resume_recycle(Rc::clone(&s), 0, &refusal, saved).expect("resumable");
            resume.run_all(&mut quiet()).expect("resumed");
        }
        (Proc::Recycle, Ok(report)) => {
            if report.scan.level.pending_recycle.contains(&0) {
                maint::recycle(Rc::clone(&s), 0, &report)
                    .expect("procedure")
                    .run_all(&mut quiet())
                    .expect("recycled");
            }
        }
        (Proc::Arch, Ok(report)) => {
            if !report.scan.level.pending_recycle.contains(&0) {
                let name = maint::archival_allowed(&report, 0).expect("archivable");
                maint::archive(&mut s.borrow_mut(), 0, &name)
                    .expect("procedure")
                    .run_all(&mut quiet())
                    .expect("archived");
            }
        }
        (Proc::Disp, Ok(report)) => {
            if let Some((binding, incident)) = report
                .current
                .iter()
                .find(|(binding, _)| report.blocking.contains(binding))
                .cloned()
            {
                let disposition = maint::disposition_for(
                    &ROOT,
                    binding,
                    &incident,
                    DispositionReason::Other,
                    "x",
                    "owner",
                    "2026-10-01T12:00:00Z",
                );
                maint::publish_disposition(&s.borrow(), disposition)
                    .expect("procedure")
                    .run_all(&mut quiet())
                    .expect("published");
            }
        }
        (proc, report) => panic!("[admin-crash] {proc:?}: unrecoverable {:?}", report.err()),
    }
    end(s);
    outcome(fixture, proc)
}

/// The crash matrix (design section 15.2), recomputed from the Rust
/// procedures on simulated storage: every crash point of every procedure,
/// after F1, and after F2 under every ordered schedule and every
/// per-directory schedule with pending data old and new. Each outcome set
/// equals the document's; every refusing outcome of dispositions,
/// archival and recycling recovers; the coverage counts are the
/// document's.
#[test]
fn m09_the_crash_matrix_is_the_documented_one() {
    let expected = design_matrix();
    assert_eq!(expected.len(), 30, "the table's rows");
    let procedures = [
        Proc::Prov,
        Proc::Disp,
        Proc::Revoke,
        Proc::Arch,
        Proc::Recycle,
        Proc::Retire,
        Proc::Requalify,
        Proc::Successor,
    ];
    let mut coverage = Vec::new();
    let mut resume_traced = false;
    for proc in procedures {
        let ops = stage(proc).procedure.len() as u64;
        let (mut ordered_total, mut perdir_total, mut outcomes_checked, mut recoveries) =
            (0u64, 0u64, 0u64, 0u64);
        for k in 0..=ops {
            let mut staged = stage(proc);
            staged
                .procedure
                .run(k as usize, &mut quiet())
                .expect("steps");
            let world = staged.fixture.world.clone();
            let ordered = world.schedules(true);
            let perdir = world.schedules(false);
            ordered_total += ordered.len() as u64;
            perdir_total += perdir.len() as u64;
            // F1.
            let f1_world = world.fork();
            f1_world.kill(&staged.actor);
            let f1 = view(&staged.fixture, f1_world);
            let f1_outcome = outcome(&f1, proc);
            outcomes_checked += 1;
            let mut f2_ordered = BTreeSet::new();
            let mut f2_perdir = BTreeSet::new();
            let mut refusing: Vec<Fixture> = Vec::new();
            if matches!(f1_outcome.as_str(), "maintenance" | "lost") {
                refusing.push(view(&staged.fixture, f1.world.fork()));
            }
            for (family, schedules) in [(true, &ordered), (false, &perdir)] {
                for schedule in schedules.iter() {
                    for data in [sim::Tear::Old, sim::Tear::New] {
                        let fork = world.fork();
                        fork.power_loss(Some(schedule), data, &BTreeMap::new());
                        let after = view(&staged.fixture, fork);
                        let result = outcome(&after, proc);
                        outcomes_checked += 1;
                        if matches!(result.as_str(), "maintenance" | "lost") {
                            refusing.push(view(&staged.fixture, after.world.fork()));
                        }
                        if family {
                            f2_ordered.insert(result);
                        } else {
                            f2_perdir.insert(result);
                        }
                    }
                }
            }
            let row = expected
                .iter()
                .find(|(name, lo, hi, ..)| name == proc.name() && *lo <= k && k <= *hi)
                .unwrap_or_else(|| panic!("no row for {} at {k}", proc.name()));
            let added: BTreeSet<String> = f2_perdir.difference(&f2_ordered).cloned().collect();
            assert_eq!(
                BTreeSet::from([f1_outcome.clone()]),
                row.3,
                "[admin-crash] {} point {k}: after F1",
                proc.name()
            );
            assert_eq!(
                f2_ordered,
                row.4,
                "[metadata-schedules] {} point {k}: after F2, ordered",
                proc.name()
            );
            assert_eq!(
                added,
                row.5,
                "[metadata-schedules] {} point {k}: added by the per-directory family",
                proc.name()
            );
            if matches!(proc, Proc::Disp | Proc::Arch | Proc::Recycle) {
                for state in refusing {
                    let saved = staged.saved.clone();
                    if proc == Proc::Recycle && !resume_traced && outcome(&state, proc) == "lost" {
                        // R-RESUME's steps (design section 15.3).
                        let probe = view(&state, state.world.fork());
                        let s = session(&probe, false);
                        let refusal = s
                            .borrow_mut()
                            .verify(&SMALL)
                            .expect_err("[admin-crash] the interrupted recycling refuses");
                        let resume = maint::resume_recycle(
                            Rc::clone(&s),
                            0,
                            &refusal,
                            saved.as_ref().expect("saved"),
                        )
                        .expect("[step-parity] R-RESUME applies");
                        let trace = traced(&probe, resume);
                        let labelled = label_trace(&probe, &trace, &[&probe.layout.state_name]);
                        assert_eq!(
                            &labelled,
                            design_operations().get("R-RESUME").expect("R-RESUME"),
                            "[step-parity] R-RESUME"
                        );
                        resume_traced = true;
                        end(s);
                    }
                    let recovered = recover(&state, proc, saved.as_ref());
                    let wanted = match proc {
                        Proc::Disp => "published",
                        Proc::Arch => "archived",
                        _ => "recycled",
                    };
                    assert_eq!(
                        recovered,
                        wanted,
                        "[admin-crash] {} point {k}: recovery",
                        proc.name()
                    );
                    recoveries += 1;
                }
            }
        }
        coverage.push((
            proc.name(),
            ops,
            ops + 1,
            ordered_total,
            perdir_total,
            outcomes_checked,
            recoveries,
        ));
    }
    assert!(resume_traced, "R-RESUME was traced");
    println!("crash matrix coverage: {coverage:?}");
    let design: Vec<(&str, u64, u64, u64, u64, u64, u64)> = vec![
        ("P-PROV", 23, 24, 54, 103, 338, 0),
        ("P-DISP", 6, 7, 15, 15, 67, 32),
        ("P-REVOKE", 3, 4, 5, 8, 30, 0),
        ("P-ARCH", 6, 7, 15, 15, 67, 32),
        ("P-RECYCLE", 10, 11, 21, 23, 99, 66),
        ("P-RETIRE", 5, 6, 11, 12, 52, 0),
        ("P-REQUALIFY", 5, 6, 11, 12, 52, 0),
        ("P-SUCCESSOR", 29, 30, 68, 117, 400, 0),
    ];
    assert_eq!(
        coverage, design,
        "[metadata-schedules] the coverage counts of design section 15.2"
    );
}

// ---------------------------------------------------------------------------
// X: composed recovery (design section 15.4)
// ---------------------------------------------------------------------------

/// Stage a procedure on a store with two pool files (so an owner can claim
/// after it): the composed scenarios' form.
fn stage_composed(proc: Proc) -> Staged {
    let with_session =
        |fixture: Fixture, build: &dyn Fn(&SharedSession) -> maint::Procedure<'static>| {
            let s = session(&fixture, false);
            let procedure = build(&s);
            let actor = s.borrow().io().clone();
            Staged {
                fixture,
                procedure,
                actor,
                saved: None,
                _session: Some(s),
            }
        };
    match proc {
        Proc::Prov => {
            let mut fixture = Fixture::host_sized(HostFixture::qualified(), 2, 16);
            let qualification = fixture.qualify().expect("qualified");
            let (procedure, _) = maint::provision(&fixture.root, &fixture.layout, &qualification);
            let actor = fixture.root.clone();
            Staged {
                fixture,
                procedure,
                actor,
                saved: None,
                _session: None,
            }
        }
        Proc::Successor => {
            let fixture = Fixture::provisioned(2, 16);
            let layout = successor_layout(&fixture);
            with_session(fixture, &|s| {
                maint::successor(Rc::clone(s), &layout, "statement").expect("procedure")
            })
        }
        Proc::Disp => with_session(store_with_incident(2), &|s| {
            let report = verify(s);
            let (binding, incident) = report.current[0].clone();
            let disposition = maint::disposition_for(
                &ROOT,
                binding,
                &incident,
                DispositionReason::Other,
                "x",
                "owner",
                "2026-10-01T12:00:00Z",
            );
            maint::publish_disposition(&s.borrow(), disposition).expect("procedure")
        }),
        other => panic!("not staged for composition: {other:?}"),
    }
}

/// An owner's dependent work: claim, start, a case, an acknowledged
/// `ActionStarted` and an admission through the store gate. Returns the
/// generation, or the refusal before any claim.
fn dependent_work(fixture: &Fixture) -> Result<(Generation, u32, SimIo), Refused> {
    let mut run = match Run::start(fixture, "worker", SMALL) {
        Ok(run) => run,
        Err(StartRefused::Store(refusal)) => return Err(refusal.refused),
        Err(StartRefused::PriorUnresolved(_)) => return Err(Refused::PriorUnresolved(Vec::new())),
        Err(StartRefused::Core(refusal)) => panic!("core refused: {refusal:?}"),
    };
    if run.started.recorder.claim_state() != ClaimState::Claimed {
        return Err(Refused::ClaimFailed("claim".into()));
    }
    run.start_run();
    run.begin(1, Expectation::Clean);
    let ticket = run
        .admit(SlotKind::Process)
        .expect("admitted through the gate");
    let _ = ticket;
    assert!(
        run.snapshot().evidence.acknowledged >= 3,
        "the start record is acknowledged"
    );
    let generation = run.custody().generation();
    let index = run.started.index;
    let owner = run.owner.clone();
    std::mem::forget(run);
    Ok((generation, index, owner))
}

/// The composed oracle (design section 15.4): after a second crash, the
/// generation that acquired an acknowledged `ActionStarted` and was never
/// sealed is in the fresh owner's decision, or the opening refuses and the
/// specified recovery brings it into the decision or ends in a refusal that
/// keeps its bytes. A fresh owner Ready without it is a failure.
fn oracle(state: &Fixture, generation: Generation, label: &str) -> &'static str {
    let in_decision = |opened: &open::Opened<SimIo>| {
        opened
            .scan
            .level
            .incidents
            .values()
            .any(|incident| incident.facts.generation == Some(generation.bytes()))
            && opened.decision.blocking.iter().any(|binding| {
                opened
                    .scan
                    .level
                    .incidents
                    .get(binding)
                    .is_some_and(|incident| incident.facts.generation == Some(generation.bytes()))
            })
    };
    let kept = |world: &SimWorld| {
        // The generation's header is still on a pool file of some root.
        let roots = world
            .lookup(ROOTS_DIR)
            .map(|ino| world.entries(ino))
            .unwrap_or_default();
        roots.values().any(|root| {
            let journals = world.entries(*root).get("journals").copied();
            journals.is_some_and(|journals| {
                world
                    .entries(journals)
                    .values()
                    .any(|pool| world.visible(*pool).get(24..40) == Some(&generation.bytes()[..]))
            })
        })
    };
    match open_owner(
        &state.store_process("fresh"),
        &state.path,
        &SMALL,
        &mut NoHooks,
    ) {
        Ok(opened) => {
            assert!(
                in_decision(&opened),
                "[composed-recovery] {label}: Ready without the generation"
            );
            "in the decision"
        }
        Err(refusal) if refusal.refused == Refused::Unprovisioned => {
            let roots: Vec<String> = sim::ROOTS.iter().map(|part| part.to_string()).collect();
            let found = maint::roots_with_history(&state.root, &roots).expect("listed");
            assert_eq!(
                found.len(),
                1,
                "[composed-recovery] {label}: the root with history"
            );
            let mut layout = state.layout.clone();
            layout.state_name = found[0].clone();
            if layout.state_name != state.layout.state_name {
                layout.root_id = sim::SUCCESSOR_ID;
            }
            let qualification = state.qualification.clone().expect("qualified");
            maint::republish(&state.root, &layout, &qualification)
                .expect("procedure")
                .run_all(&mut quiet())
                .expect("republished");
            let opened = open_owner(
                &state.store_process("after-republish"),
                &state.path,
                &SMALL,
                &mut NoHooks,
            )
            .expect("opens");
            assert!(
                in_decision(&opened),
                "[composed-recovery] {label}: republished without the generation"
            );
            "republished"
        }
        Err(refusal) => {
            assert!(
                kept(&state.world),
                "[composed-recovery] {label}: refused ({:?}) and the bytes lost",
                refusal.refused
            );
            "refused, kept"
        }
    }
}

/// Every first-crash state of every prefix of first provisioning, of
/// succession and of a disposition publication; an ordinary owner then
/// activates, claims and acknowledges an `ActionStarted`; then every second
/// crash; the oracle never finds the generation lost. Forbidden effects are
/// absent: no claim on a store that was not activated, no Ready owner
/// without the generation.
#[test]
fn x01_composed_recovery_keeps_every_acknowledged_generation() {
    let (mut first, mut worked, mut second) = (0u64, 0u64, 0u64);
    let mut outcomes: BTreeMap<&'static str, (u64, String)> = BTreeMap::new();
    let mut refused_before_work: BTreeMap<String, u64> = BTreeMap::new();
    for proc in [Proc::Prov, Proc::Successor, Proc::Disp] {
        let ops = stage_composed(proc).procedure.len();
        for k in 0..=ops {
            let mut staged = stage_composed(proc);
            staged.procedure.run(k, &mut quiet()).expect("steps");
            let world = staged.fixture.world.clone();
            let mut states: Vec<(String, SimWorld)> = Vec::new();
            let f1 = world.fork();
            f1.kill(&staged.actor);
            states.push(("F1".into(), f1));
            for schedule in world
                .schedules(true)
                .iter()
                .chain(world.schedules(false).iter())
            {
                for data in [sim::Tear::Old, sim::Tear::New] {
                    let fork = world.fork();
                    fork.power_loss(Some(schedule), data, &BTreeMap::new());
                    states.push((format!("F2 {schedule:?} {data:?}"), fork));
                }
            }
            for (crash, state_world) in states {
                first += 1;
                let state = view(&staged.fixture, state_world);
                let (generation, _, owner) = match dependent_work(&state) {
                    Ok(work) => work,
                    Err(refused) => {
                        *refused_before_work
                            .entry(
                                format!("{refused:?}")
                                    .split('(')
                                    .next()
                                    .unwrap_or_default()
                                    .to_string(),
                            )
                            .or_default() += 1;
                        continue;
                    }
                };
                worked += 1;
                let after = state.world.clone();
                let mut seconds: Vec<(String, SimWorld)> = Vec::new();
                let f1 = after.fork();
                f1.kill(&owner);
                seconds.push(("F1".into(), f1));
                for schedule in after
                    .schedules(true)
                    .iter()
                    .chain(after.schedules(false).iter())
                {
                    for data in [sim::Tear::Old, sim::Tear::New] {
                        let fork = after.fork();
                        fork.power_loss(Some(schedule), data, &BTreeMap::new());
                        seconds.push((format!("F2 {schedule:?} {data:?}"), fork));
                    }
                }
                for (second_crash, world2) in seconds {
                    second += 1;
                    let label = format!("{} prefix {k}, {crash}, then {second_crash}", proc.name());
                    let outcome = oracle(&view(&staged.fixture, world2), generation, &label);
                    outcomes.entry(outcome).or_insert((0, label)).0 += 1;
                }
            }
        }
    }
    println!("composed: {first} first crashes, {worked} runs with dependent work, {second} second crashes; refused before work: {refused_before_work:?}");
    for (outcome, (count, example)) in &outcomes {
        println!("composed outcome {outcome:?}: {count}, for example {example}");
    }
    assert!(
        worked > 100 && second > worked,
        "[composed-recovery] the enumeration reached dependent work"
    );
}

/// The positive path: a generation dispositioned in a session survives
/// either crash, and the fresh owner is Ready with it dispositioned.
#[test]
fn x02_a_dispositioned_generation_lets_the_next_owner_run() {
    let fixture = store_with_incident(2);
    let binding = publish_only_incident(&fixture);
    for crash in ["F1", "F2"] {
        let world = fixture.world.fork();
        if crash == "F2" {
            world.power_loss(None, sim::Tear::Old, &BTreeMap::new());
        }
        let state = view(&fixture, world);
        let opened = open_owner(
            &state.store_process("fresh"),
            &state.path,
            &SMALL,
            &mut NoHooks,
        )
        .expect("opens");
        assert!(
            opened.decision.blocking.is_empty() && opened.decision.dispositioned == vec![binding],
            "[composed-recovery] {crash}: the positive path"
        );
    }
}

/// Storage events composed (design section 15.4, C32): after every prefix
/// of first provisioning and of succession and process death, an opening on
/// each of four hosts; on admitted storage dependent work, then a storage
/// event (a flush the device is never sent, a failed home write, a failed
/// tail write rewritten, both superblock writes failing with the new tail,
/// a write in flight), then every second crash: the generation is
/// discoverable or explicitly refused. Storage that is not admitted never
/// reaches dependent work; a loss of qualification refuses with the history
/// kept.
#[test]
fn x03_storage_events_composed() {
    use custody::store::sim::{Checkpoint, Home};
    let hosts: Vec<(&str, HostFixture, Device)> = {
        let variants = sim::storage_variants();
        let pick = |name: &str| {
            variants
                .iter()
                .find(|variant| variant.name == name)
                .map(|variant| variant.host.clone())
                .expect(name)
        };
        vec![
            ("admitted", HostFixture::qualified(), Device::Stable),
            (
                "volatile write-back",
                pick("a volatile write-back cache"),
                Device::Volatile,
            ),
            (
                "volatile hidden by write_cache",
                pick("a volatile cache hidden by queue/write_cache"),
                Device::Unflushed,
            ),
            (
                "device-mapper",
                pick("a device-mapper device"),
                Device::Volatile,
            ),
        ]
    };
    let events = [
        (
            "a flush never sent",
            Checkpoint {
                flush_ok: false,
                ..Checkpoint::CLEAN
            },
        ),
        (
            "a failed home write",
            Checkpoint {
                home: Home::Fail,
                ..Checkpoint::CLEAN
            },
        ),
        (
            "a failed tail write, rewritten",
            Checkpoint {
                superblock_ok: false,
                ..Checkpoint::CLEAN
            },
        ),
        (
            "both superblock writes failing",
            Checkpoint {
                superblock_ok: false,
                rewrite_ok: false,
                ..Checkpoint::CLEAN
            },
        ),
        (
            "a home write in flight",
            Checkpoint {
                home: Home::InFlight,
                ..Checkpoint::CLEAN
            },
        ),
    ];
    let (mut openings, mut worked, mut seconds, mut not_admitted, mut lost_qualification) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    for proc in [Proc::Prov, Proc::Successor] {
        let ops = stage_composed(proc).procedure.len();
        for k in 0..=ops {
            let mut staged = stage_composed(proc);
            staged.procedure.run(k, &mut quiet()).expect("steps");
            staged.fixture.world.kill(&staged.actor);
            for (host_name, host, device) in &hosts {
                openings += 1;
                let world = staged.fixture.world.fork();
                world.with_host(|h| *h = host.clone());
                world.set_device(*device);
                let state = view(&staged.fixture, world);
                let work = dependent_work(&state);
                if *host_name != "admitted" {
                    assert!(work.is_err(), "[storage-composed] {host_name}: storage that is not admitted reached dependent work");
                    not_admitted += 1;
                    continue;
                }
                let Ok((generation, _, owner)) = work else {
                    continue;
                };
                worked += 1;
                for (event_name, event) in &events {
                    let after = state.world.fork();
                    let _ = after.checkpoint(*event);
                    let mut crashes: Vec<SimWorld> = Vec::new();
                    let f1 = after.fork();
                    f1.kill(&owner);
                    crashes.push(f1);
                    for schedule in after
                        .schedules(true)
                        .iter()
                        .chain(after.schedules(false).iter())
                    {
                        for data in [sim::Tear::Old, sim::Tear::New] {
                            let fork = after.fork();
                            fork.power_loss(Some(schedule), data, &BTreeMap::new());
                            crashes.push(fork);
                        }
                    }
                    for crashed in crashes {
                        seconds += 1;
                        oracle(
                            &view(&staged.fixture, crashed),
                            generation,
                            &format!("{} prefix {k}, {event_name}", proc.name()),
                        );
                    }
                }
                // A loss of qualification after the work: every later
                // opening refuses, the history kept.
                if proc == Proc::Successor {
                    let changed = state.world.fork();
                    changed.kill(&owner);
                    changed
                        .with_host(|h| h.set_storage_attribute("firmware_rev", Some("FW-0002 \n")));
                    let checked = view(&staged.fixture, changed);
                    let refusal = open_owner(
                        &checked.store_process("later"),
                        &checked.path,
                        &SMALL,
                        &mut NoHooks,
                    )
                    .expect_err("[storage-composed] loss of qualification");
                    assert_eq!(
                        refusal.refused,
                        Refused::Unsupported("storage not qualified".into()),
                        "[storage-composed] loss of qualification"
                    );
                    assert_eq!(
                        oracle(&checked, generation, "loss of qualification"),
                        "refused, kept"
                    );
                    lost_qualification += 1;
                }
            }
        }
    }
    println!("storage composed: {openings} openings, {worked} runs on admitted storage, {seconds} second crashes, {not_admitted} openings on storage that is not admitted, {lost_qualification} losses of qualification");
    assert!(worked > 10 && not_admitted > 100 && lost_qualification > 0);
}

// ---------------------------------------------------------------------------
// A: the authority surface, pinned in the source
// ---------------------------------------------------------------------------

const IO_SOURCE: &str = include_str!("support/custody/store/io.rs");
const OPEN_SOURCE: &str = include_str!("support/custody/store/open.rs");
const EXCHANGE_SOURCE: &str = include_str!("support/custody/store/exchange.rs");
const RECORDER_SOURCE: &str = include_str!("support/custody/store/recorder.rs");

/// The text of the first item that starts with `head`, through the closing
/// brace at the head's own indentation.
fn source_item(source: &str, head: &str) -> String {
    let start = source.find(head).unwrap_or_else(|| panic!("no {head}"));
    let indent = source[..start].rsplit('\n').next().unwrap_or("").len();
    let close = format!("\n{}}}", " ".repeat(indent));
    let end = source[start..]
        .find(&close)
        .map(|at| start + at + close.len())
        .unwrap_or_else(|| panic!("no end of {head}"));
    source[start..end].to_string()
}

/// The members a struct or an enum declares: each name, and the type after
/// its colon (empty for an enum variant). Comments and attributes are
/// skipped.
fn source_members(item: &str) -> Vec<(String, String)> {
    item.lines()
        .skip(1)
        .map(str::trim)
        .filter(|line| {
            !line.is_empty() && *line != "}" && !line.starts_with("//") && !line.starts_with('#')
        })
        .map(|line| {
            let line = line
                .strip_prefix("pub(super) ")
                .or_else(|| line.strip_prefix("pub "))
                .unwrap_or(line);
            let name: String = line
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let kind = line
                .split_once(':')
                .map(|(_, kind)| kind.trim().trim_end_matches(',').to_string())
                .unwrap_or_default();
            (name, kind)
        })
        .collect()
}

/// The authority the store's types can express, fixed in the source (design
/// sections 8.1, 8.2, 9.8 and 10.5): the I/O trait can take a lock but never
/// release one, and only the fixture primitive does; the worker holds no
/// lock description; the admission gate holds the exchange mutex across the
/// core's admission; a selection keeps no open description, and revalidation
/// walks afresh. Source facts, not behaviour: the behaviours are r07, n04,
/// r03 and o04; these pin what no deterministic test can interleave.
#[test]
fn a01_the_authority_surface_is_fixed_in_the_source() {
    let names = |members: Vec<(String, String)>| -> Vec<String> {
        members.into_iter().map(|(name, _)| name).collect()
    };
    assert_eq!(
        names(source_members(&source_item(
            IO_SOURCE,
            "pub enum LockRequest {"
        ))),
        ["Exclusive", "Shared"],
        "[lock-retained] a lock request only takes a lock"
    );
    let trait_item = source_item(IO_SOURCE, "pub trait StoreIo: Send + Sync {");
    assert!(
        !trait_item.to_lowercase().contains("unlock") && !trait_item.contains("LOCK_UN"),
        "[lock-retained] the I/O trait can release a lock"
    );
    let fixture_unlock = source_item(IO_SOURCE, "pub fn fixture_unlock(");
    assert_eq!(
        (
            IO_SOURCE.matches("libc::LOCK_UN").count(),
            fixture_unlock.matches("libc::LOCK_UN").count()
        ),
        (1, 1),
        "[lock-retained] LOCK_UN outside the fixture primitive"
    );
    assert_eq!(
        names(source_members(&source_item(
            RECORDER_SOURCE,
            "pub struct Worker<P: StoreIo> {"
        ))),
        ["io", "file", "journals", "name", "identity", "exchange", "phase"],
        "[lock-retained] the worker holds a lock description"
    );
    let gate = source_item(EXCHANGE_SOURCE, "pub fn admit<R: Resource>(");
    let at = |needle: &str| {
        gate.find(needle)
            .unwrap_or_else(|| panic!("[admission-fence] the gate lacks {needle}"))
    };
    let order = [
        at("let held = self.exchange.acquire();"),
        at("held.state.fatal"),
        at("custody.admit(reservation, now)"),
        at("drop(held)"),
    ];
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "[admission-fence] the gate's health check and admission are not one critical section: {order:?}"
    );
    assert_eq!(
        gate.matches("drop(held)").count(),
        1,
        "[admission-fence] the gate releases the mutex more than once"
    );
    let selection = source_members(&source_item(OPEN_SOURCE, "pub struct Selection {"));
    assert!(
        selection.iter().all(|(_, kind)| !kind.contains("File")
            && !kind.contains("Dir")
            && !kind.contains("Fd")),
        "[provision-selection] a selection keeps an open description: {selection:?}"
    );
    let revalidation = source_item(OPEN_SOURCE, "pub fn revalidate<P: Platform>(");
    assert!(
        revalidation.contains("walk(io, &components(&path.directory))")
            && revalidation.contains("walk(io, &components(&selection.parent))"),
        "[provision-selection] revalidation does not walk afresh"
    );
}

// ---------------------------------------------------------------------------
// N: native primitives (Linux), beneath CARGO_TARGET_TMPDIR only
// ---------------------------------------------------------------------------

use custody::store::io::linux::{LinuxDir, LinuxFile, LinuxIo};
use custody::store::io::{Fault, Faulty, FileType, LockRequest};

static NATIVE: AtomicU64 = AtomicU64::new(0);

/// A fixture directory the test created exclusively beneath
/// `CARGO_TARGET_TMPDIR`, with every entry it made recorded with its
/// identity. Cleanup removes only those entries, each only while it is
/// still the inode the test made; nothing is deleted recursively or by a
/// caller's path.
struct Native {
    base: LinuxDir,
    name: String,
    ino: u64,
    dir: LinuxDir,
    made: Vec<(String, bool, u64)>,
}

impl Native {
    fn new() -> Native {
        let io = LinuxIo;
        let base = LinuxIo::open_base(std::path::Path::new(env!("CARGO_TARGET_TMPDIR")))
            .expect("the test's tmp directory");
        let name = format!(
            "custody-store-{}-{}",
            std::process::id(),
            NATIVE.fetch_add(1, Ordering::SeqCst)
        );
        io.make_dir(&base, &name, 0o700)
            .expect("an exclusive fixture directory");
        let ino = io.stat_at(&base, &name).expect("stat").ino;
        let dir = io.open_dir(&base, &name).expect("open");
        assert_eq!(
            io.stat_dir(&dir).expect("fstat").ino,
            ino,
            "the fixture's identity"
        );
        Native {
            base,
            name,
            ino,
            dir,
            made: Vec::new(),
        }
    }

    fn file(&mut self, name: &str, data: &[u8]) -> LinuxFile {
        let io = LinuxIo;
        let file = io.create_exclusive(&self.dir, name, 0o600).expect("create");
        if !data.is_empty() {
            custody::store::io::write_fully(&io, &file, 0, data).expect("write");
        }
        self.made
            .push((name.into(), false, io.stat_file(&file).expect("fstat").ino));
        file
    }

    fn record(&mut self, name: &str, directory: bool) {
        let ino = LinuxIo.stat_at(&self.dir, name).expect("stat").ino;
        self.made.push((name.into(), directory, ino));
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(&self.name)
            .join(name)
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        let io = LinuxIo;
        for (name, directory, ino) in self.made.iter().rev() {
            if io.stat_at(&self.dir, name).map(|stat| stat.ino) == Ok(*ino) {
                let _ = LinuxIo::fixture_remove(&self.dir, name, *directory);
            }
        }
        if io.stat_at(&self.base, &self.name).map(|stat| stat.ino) == Ok(self.ino) {
            let _ = LinuxIo::fixture_remove(&self.base, &self.name, true);
        }
    }
}

/// Opening is descriptor-relative and no-follow: a symbolic link refuses
/// (ELOOP) and is typed before any open; names with separators or dots
/// never reach the kernel.
#[test]
fn n01_native_opening_is_descriptor_relative_and_no_follow() {
    let mut fixture = Native::new();
    let io = LinuxIo;
    fixture.file("target", b"x");
    LinuxIo::fixture_symlink(&fixture.dir, "link", "target").expect("symlink");
    fixture.record("link", false);
    assert_eq!(
        io.stat_at(&fixture.dir, "link").expect("lstat").file_type,
        FileType::Symlink,
        "[safe-open] typed without being followed"
    );
    assert_eq!(
        io.open_read(&fixture.dir, "link")
            .expect_err("[safe-open] native: a link refuses")
            .errno,
        Errno::Loop,
        "[safe-open] native: a link refuses"
    );
    let as_dir = io
        .open_dir(&fixture.dir, "link")
        .expect_err("[safe-open] native: a link opened as a directory refuses")
        .errno;
    assert!(
        matches!(as_dir, Errno::Loop | Errno::NotDir),
        "[safe-open] native: a link opened as a directory refuses ({as_dir:?})"
    );
    for name in ["../escape", "a/b", "", "."] {
        assert!(
            io.open_read(&fixture.dir, name).is_err(),
            "[safe-open] native: {name:?}"
        );
    }
}

/// A hard link is visible as a link count of two on the same inode.
#[test]
fn n02_native_hard_links_are_visible() {
    let mut fixture = Native::new();
    let io = LinuxIo;
    fixture.file("one", b"x");
    io.link(&fixture.dir, "one", &fixture.dir, "two")
        .expect("link");
    fixture.record("two", false);
    let one = io.stat_at(&fixture.dir, "one").expect("stat");
    let two = io.stat_at(&fixture.dir, "two").expect("stat");
    assert!(
        one.same_inode(&two) && one.nlink == 2,
        "[safe-open] native: the link count shows the hard link"
    );
}

/// A FIFO is typed before opening, and a read-only non-blocking open of it
/// returns at once instead of waiting for a writer.
#[test]
fn n03_native_fifo_opens_without_blocking() {
    let mut fixture = Native::new();
    LinuxIo::fixture_fifo(&fixture.dir, "fifo", 0o600).expect("mkfifo");
    fixture.record("fifo", false);
    assert_eq!(
        LinuxIo
            .stat_at(&fixture.dir, "fifo")
            .expect("stat")
            .file_type,
        FileType::Fifo,
        "[safe-open] typed before opening"
    );
    let dir = LinuxIo.open_dir(&fixture.base, &fixture.name).expect("dir");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let opened = LinuxIo.open_read(&dir, "fifo").map(drop);
        let _ = tx.send(opened.is_ok());
    });
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(true),
        "[safe-open] native: the FIFO open blocked or failed"
    );
}

/// `flock` belongs to the open file description: independent descriptions
/// conflict, even in one process; a duplicate shares the lock, and
/// `LOCK_UN` on any duplicate releases it (design section 8.2 rule 1);
/// closing one duplicate does not.
#[test]
fn n04_native_flock_descriptions_and_duplicates() {
    let mut fixture = Native::new();
    let io = LinuxIo;
    fixture.file("LOCK", b"");
    let a = io.open_read(&fixture.dir, "LOCK").expect("a");
    let b = io.open_read(&fixture.dir, "LOCK").expect("b");
    io.flock(&a, LockRequest::Exclusive).expect("a locks");
    assert_eq!(
        io.flock(&b, LockRequest::Exclusive)
            .expect_err("[lock-retained] independent descriptions conflict")
            .errno,
        Errno::Again,
        "[lock-retained] independent descriptions conflict"
    );
    assert_eq!(
        io.flock(&b, LockRequest::Shared)
            .expect_err("[lock-retained] a shared request conflicts")
            .errno,
        Errno::Again
    );
    let duplicate = LinuxIo::fixture_duplicate(&a).expect("dup");
    io.flock(&duplicate, LockRequest::Exclusive)
        .expect("the same description");
    LinuxIo::fixture_unlock(&duplicate).expect("LOCK_UN");
    io.flock(&b, LockRequest::Exclusive)
        .expect("[lock-retained] LOCK_UN on a duplicate released the original's lock");
    drop(b);
    let c = io.open_read(&fixture.dir, "LOCK").expect("c");
    io.flock(&c, LockRequest::Exclusive).expect("c locks");
    let d = LinuxIo::fixture_duplicate(&c).expect("dup");
    drop(d);
    let e = io.open_read(&fixture.dir, "LOCK").expect("e");
    assert_eq!(
        io.flock(&e, LockRequest::Exclusive)
            .expect_err("[lock-retained] closing one duplicate releases nothing")
            .errno,
        Errno::Again,
        "[lock-retained] closing one duplicate releases nothing"
    );
    drop(c);
    io.flock(&e, LockRequest::Exclusive)
        .expect("released when every duplicate closed");
    drop((a, duplicate, e));
}

/// Whole-block writes through the store's own write loop: short counts are
/// looped, zero progress and EINTR retried at most three times; a sync's
/// EINTR likewise, any other error final (design section 9.4).
#[test]
fn n05_native_writes_complete_through_a_controlled_adapter() {
    let mut fixture = Native::new();
    let file = fixture.file("journal", &[]);
    let faulty = Faulty::new(LinuxIo);
    let block: Vec<u8> = (0..BLOCK).map(|k| (k % 251) as u8).collect();
    faulty.plan_writes([
        Fault::Short(100),
        Fault::Zero,
        Fault::Interrupted,
        Fault::Short(1000),
    ]);
    custody::store::io::write_fully(&faulty, &file, BLOCK as u64, &block)
        .expect("[ack-durable] native: the block completes");
    let mut back = vec![0u8; BLOCK];
    assert_eq!(LinuxIo.pread(&file, BLOCK as u64, &mut back), Ok(BLOCK));
    assert_eq!(back, block, "[ack-durable] native: every byte written");
    faulty.plan_writes([
        Fault::Zero,
        Fault::Interrupted,
        Fault::Zero,
        Fault::Interrupted,
    ]);
    assert_eq!(
        custody::store::io::write_fully(&faulty, &file, 0, &block),
        Err(custody::store::io::WriteFailure::NoProgress),
        "[ack-durable] native: a fourth stall fails"
    );
    faulty.plan_writes([Fault::Fail(Errno::Io)]);
    assert!(matches!(
        custody::store::io::write_fully(&faulty, &file, 0, &block),
        Err(custody::store::io::WriteFailure::Error(_))
    ));
    faulty.plan_syncs([Fault::Interrupted, Fault::Interrupted, Fault::Interrupted]);
    custody::store::io::sync_with_retries(&faulty, &file, true).expect("three EINTRs retried");
    faulty.plan_syncs([
        Fault::Interrupted,
        Fault::Interrupted,
        Fault::Interrupted,
        Fault::Interrupted,
    ]);
    assert_eq!(
        custody::store::io::sync_with_retries(&faulty, &file, true)
            .expect_err("[ack-durable] native: a fourth EINTR fails")
            .errno,
        Errno::Intr
    );
    faulty.plan_syncs([Fault::Fail(Errno::Io), Fault::Interrupted]);
    assert_eq!(
        custody::store::io::sync_with_retries(&faulty, &file, true)
            .expect_err("[ack-durable] native: a failed sync is never retried")
            .errno,
        Errno::Io,
        "[ack-durable] native: a failed sync is never retried"
    );
}

/// File and directory syncs on the test's own files: a directory opened
/// `O_RDONLY | O_DIRECTORY` syncs, an `O_PATH` handle cannot; `futimens`
/// on an owned file sets its timestamps.
#[test]
fn n06_native_syncs_and_futimens_on_owned_files() {
    let mut fixture = Native::new();
    let io = LinuxIo;
    let file = fixture.file("data", &[7u8; 64]);
    io.fdatasync(&file).expect("fdatasync");
    io.fsync(&file).expect("fsync");
    let directory = io
        .open_dir_for_sync(&fixture.base, &fixture.name)
        .expect("O_RDONLY | O_DIRECTORY");
    io.fsync(&directory)
        .expect("[durable-activation] native: a directory description syncs");
    assert_eq!(
        LinuxIo::fixture_fsync_path_handle(&fixture.dir)
            .expect_err("[durable-activation] native: an O_PATH handle cannot be synced")
            .errno,
        Errno::BadF,
        "[durable-activation] native: an O_PATH handle cannot be synced"
    );
    fixture.file("LOCK", b"");
    let before = std::fs::metadata(fixture.path("LOCK"))
        .and_then(|meta| meta.modified())
        .expect("mtime");
    std::thread::sleep(std::time::Duration::from_millis(20));
    let lock = io.open_read(&fixture.dir, "LOCK").expect("open");
    io.touch(&lock)
        .expect("[durable-activation] native: futimens by the owner");
    io.fsync(&lock).expect("the probe's fsync");
    let after = std::fs::metadata(fixture.path("LOCK"))
        .and_then(|meta| meta.modified())
        .expect("mtime");
    assert!(
        after > before,
        "[durable-activation] native: the timestamps moved"
    );
}

/// Descriptor identity and the one-filesystem comparison: a description
/// keeps its inode when the name is replaced, which the recheck detects;
/// the fixture's directories share one device.
#[test]
fn n07_native_identity_and_one_filesystem() {
    let mut fixture = Native::new();
    let io = LinuxIo;
    let first = fixture.file("journal", b"one");
    let opened = io.stat_file(&first).expect("fstat");
    assert!(
        opened.same_inode(&io.stat_at(&fixture.dir, "journal").expect("stat")),
        "[safe-open] native: identity"
    );
    fixture.file("replacement", b"two");
    io.rename(&fixture.dir, "replacement", &fixture.dir, "journal")
        .expect("rename");
    fixture.record("journal", false);
    assert!(
        !opened.same_inode(&io.stat_at(&fixture.dir, "journal").expect("stat")),
        "[safe-open] native: the recheck sees a replaced name"
    );
    io.make_dir(&fixture.dir, "sub", 0o700).expect("mkdir");
    fixture.record("sub", true);
    let sub = io.open_dir(&fixture.dir, "sub").expect("open");
    assert_eq!(
        io.stat_dir(&sub).expect("fstat").dev,
        io.stat_dir(&fixture.dir).expect("fstat").dev,
        "[durable-activation] native: one filesystem"
    );
}

/// The worker's write and sync wiring on a native file (design section
/// 16.5): the claim header and records are written whole, synced on the
/// worker's own description and published only then. This opens no store:
/// there is no `PROVISION`, activation, lock or opening, only the worker's
/// I/O path on a zero-filled file of the test's own.
#[test]
fn n08_native_worker_write_and_sync_wiring() {
    use custody::store::exchange::{Exchange, Recorder};
    use custody::store::recorder::Worker;
    let mut fixture = Native::new();
    let generation = Generation::new([0x7a; 16]);
    let file = fixture.file("j00000.journal", &vec![0u8; 18 * BLOCK]);
    drop(file);
    let faulty = Faulty::new(LinuxIo);
    let io_file = faulty
        .open_write(&fixture.dir, "j00000.journal")
        .expect("I/O description");
    let identity = faulty.stat_file(&io_file).expect("identity");
    let journals = Arc::new(LinuxIo.open_dir(&fixture.base, &fixture.name).expect("dir"));
    let exchange = Exchange::new(generation, 16, clock());
    let mut recorder = Recorder::new(Arc::clone(&exchange));
    let mut worker = Worker::new(
        faulty,
        io_file,
        journals,
        "j00000.journal".into(),
        identity,
        exchange,
    );
    let header = header_fields(generation.bytes(), 1, 0, 16, 16);
    recorder.request_claim(encode_header(&header).expect("header"));
    worker.run_until_idle();
    assert_eq!(
        recorder.claim_state(),
        ClaimState::Claimed,
        "[ack-durable] native: the claim"
    );
    let (mut custody, _control) =
        Custody::<Token>::new(SMALL, generation, Vec::new(), Tick(1)).expect("custody");
    custody.start_run(Tick(2)).expect("starts");
    custody
        .begin_case(CaseId(1), Expectation::Clean, Tick(3))
        .ok();
    recorder.flush(&mut custody, Tick(4)).expect("flush");
    worker.run_until_idle();
    recorder.apply(&mut custody, Tick(5));
    custody
        .begin_case(CaseId(1), Expectation::Clean, Tick(6))
        .expect("begins");
    recorder.flush(&mut custody, Tick(7)).expect("flush");
    worker.run_until_idle();
    let applied = recorder.apply(&mut custody, Tick(8));
    assert_eq!(
        applied.applied_through, 2,
        "[ack-durable] native: published after the sync"
    );
    let bytes = std::fs::read(fixture.path("j00000.journal")).expect("read back");
    let report = classify_bytes(&bytes, ROOT, 16, 0);
    assert_eq!(
        report.class,
        FileClass::UnsealedNoAction,
        "[ack-durable] native: the file holds the claim and two records: {}",
        report.reason
    );
    assert_eq!(report.records.len(), 2);
}
