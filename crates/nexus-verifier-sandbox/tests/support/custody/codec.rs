//! The custody core's canonical evidence and request codec, version 1: the
//! exact bytes of one evidence record or one control request, the strict
//! decoding of exactly one such frame back into plain data, and the
//! domain-separated digests the core binds record acknowledgements and
//! request receipts to.
//!
//! What it is not: storage, a journal, recovery, a transport or
//! authentication. A decoded record is a description: not proof that it was
//! durably written, not an acknowledgement, and no authority over anything it
//! names. A decoded request is not an authenticated request. A matching
//! digest shows only that the bytes are internally consistent: whoever can
//! rewrite the bytes can rewrite the digest too. Decoding one frame proves
//! nothing about the frames around it (that a journal is complete, that no
//! suffix was deleted) or about native cleanup.
//!
//! # Format, version 1
//!
//! Every integer is unsigned, fixed width and big-endian. Nothing is padded,
//! nothing depends on the platform, and no tag is taken from a declaration
//! order, a memory layout or any formatting.
//!
//! | Offset | Size | Field |
//! |---|---|---|
//! | 0 | 4 | magic `NXCD` (`4e 58 43 44`) |
//! | 4 | 1 | domain: `52` (`R`) record, `51` (`Q`) request |
//! | 5 | 1 | version: `01` |
//! | 6 | 2 | payload length `L` (u16); `1 <= L <= 4056` |
//! | 8 | `L` | payload (below) |
//! | `8 + L` | 32 | digest: SHA-256 of bytes `0 .. 8 + L` (never of itself) |
//!
//! A frame is exactly `8 + L + 32` bytes, at most 4096.
//!
//! Fields: `u32` and `u64` big-endian; `bool` one byte, `00` false, `01`
//! true; `option<T>` one byte, `00` absent, `01` present and followed by
//! `T`; `generation` its 16 bytes as held; `identity` (record, action,
//! incident) its generation then its `u64` sequence; `instant` the `u64`
//! milliseconds supplied; `case` `u32`. In every tag position, `00` and any
//! value not listed are invalid.
//!
//! Record payload: record generation (16), record sequence (`u64`), instant
//! (`u64`), kind tag (`u8`), kind fields:
//!
//! | Tag | Kind | Fields, in order |
//! |---|---|---|
//! | `01` | `RunStarted` | dispositioned `u32` |
//! | `02` | `CaseStarted` | case `u32`, expectation |
//! | `03` | `ActionStarted` | action identity, slot kind |
//! | `04` | `ActionFailed` | action identity |
//! | `05` | `ActionSettled` | action identity, settlement |
//! | `06` | `CaseEnded` | case `u32`, passed `bool` |
//! | `07` | `Control` | control fact |
//! | `08` | `RecoveryRequired` | none |
//! | `09` | `RecoveryAttempt` | attempt `u32`, resolved `bool` |
//! | `0a` | `RunEnded` | verdict, resolved by `option<u32>` |
//! | `0b` | `IncidentOpened` | incident identity, action identity, owner kind `option<slot kind>` |
//! | `0c` | `IncidentSettled` | incident identity, settlement |
//!
//! Request payload: generation (16), sequence (`u64`), operation tag
//! (`u8`), operation fields: `01` `Retry`, epoch `u64`; `02` `Shutdown`,
//! none.
//!
//! Nested tags:
//!
//! | Field | Tags |
//! |---|---|
//! | expectation | `01` Clean, `02` RetainedBoundary, `03` OutputDetached |
//! | slot kind | `01` Process, `02` Workspace, `03` Fixture |
//! | settlement | `01` Confirmed, `02` OutputLost, `03` NothingCreated, `04` NotAdmitted |
//! | verdict | `01` Pending, `02` Passed, `03` Failed |
//! | control fact | `01` AdmissionClosed: closure reason, after `u64`; `02` ShutdownRefused: none |
//! | closure reason | `01` Cancelled: cancel reason; `02` Failed: failure class |
//! | cancel reason | `01` Requested, `02` LeaseLost, `03` Shutdown, `04` Stop, `05` RunBudget |
//! | failure class | `01` Assertion, `02` UnexpectedRetained, `03` UnexpectedCleanup, `04` ExpectedConditionUnmet, `05` UnexpectedOwner, `06` LateOwner, `07` UnknownOutcome, `08` OutputLost, `09` AuthorityLost, `0a` RecordFailed, `0b` RecorderFault, `0c` Cancelled |
//!
//! Record payloads are 33 to 83 bytes and request payloads 25 or 33, far
//! below the limit. A value that is representable but meaningless in some
//! lifecycle state (a terminal record carrying `Pending`, an embedded
//! identity of another generation) is encoded and decoded as it is: the
//! codec checks syntax, never lifecycle.
//!
//! # Use
//!
//! - The core computes the digest of a record it issues from the typed fields
//!   it made ([`record_digest`]), and a request's digest from the request
//!   ([`request_digest`]). Both hash exactly the bytes [`encode_record`] and
//!   [`encode_request`] write: one encoder.
//! - [`encode_record`] writes an issued [`RecordIntent`] only if the digest it
//!   carries is its fields' own; it never overwrites a mismatch.
//! - [`decode_record`] and [`decode_request`] accept exactly one complete,
//!   consistent frame of their domain and nothing else, check every length
//!   before reading, allocate nothing, and return plain data
//!   ([`RecordEvidence`], [`RequestEvidence`]). Nothing here can start,
//!   admit, acknowledge, disposition, clean up or reconstruct anything.

use std::cmp::Ordering;

use sha2::{Digest, Sha256};

use super::model::*;

/// The frame magic, `NXCD`.
pub const MAGIC: [u8; 4] = *b"NXCD";
/// The only format version written or read.
pub const VERSION: u8 = 1;
/// Magic, domain, version and payload length.
pub const HEADER_LEN: usize = 8;
/// The digest trailer.
pub const DIGEST_LEN: usize = 32;
/// The largest frame read or written.
pub const MAX_FRAME: usize = 4096;
/// The largest payload length a header may declare.
pub const MAX_PAYLOAD: usize = MAX_FRAME - HEADER_LEN - DIGEST_LEN;

/// The largest version-1 payload: an opened incident with its owner's kind.
const LARGEST_PAYLOAD: usize = 83;
const _: () = assert!(LARGEST_PAYLOAD <= MAX_PAYLOAD);

/// A frame's domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// An evidence record: `52` (`R`).
    Record,
    /// A control request: `51` (`Q`).
    Request,
}

impl Domain {
    /// The domain's wire byte.
    pub fn byte(self) -> u8 {
        match self {
            Domain::Record => 0x52,
            Domain::Request => 0x51,
        }
    }
}

/// Why bytes are not exactly one valid version-1 frame of the expected
/// domain, or why an intent cannot be encoded. Plain data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// Longer than [`MAX_FRAME`]: refused before anything is read.
    TooLong {
        len: usize,
    },
    /// Shorter than the header, or than the frame its header declares.
    Truncated,
    BadMagic,
    /// Another domain's frame, or no known domain.
    WrongDomain {
        expected: Domain,
        found: u8,
    },
    UnsupportedVersion {
        found: u8,
    },
    /// A declared payload length of zero or above [`MAX_PAYLOAD`].
    BadLength {
        declared: u16,
    },
    /// Bytes after the one frame (a second frame included).
    TrailingBytes {
        extra: usize,
    },
    /// The digest trailer is not the SHA-256 of the bytes before it.
    DigestMismatch,
    /// A tag that names nothing.
    UnknownTag {
        field: &'static str,
        tag: u8,
    },
    /// A boolean byte other than `00` or `01`.
    InvalidBool {
        field: &'static str,
        value: u8,
    },
    /// An option presence byte other than `00` or `01`.
    InvalidPresence {
        field: &'static str,
        value: u8,
    },
    /// The fields run past the declared payload.
    ShortPayload,
    /// The declared payload is longer than its fields.
    LongPayload {
        unused: usize,
    },
    /// The intent's digest is not its fields' canonical digest.
    IntentDigestMismatch,
}

/// One record decoded from one frame: evidence data. Not proof that it was
/// durably written, not an acknowledgement, and no authority; its digest
/// shows only that the frame was internally consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordEvidence {
    pub id: RecordId,
    pub at: Tick,
    pub kind: RecordKind,
    pub digest: [u8; DIGEST_LEN],
}

impl RecordEvidence {
    /// Whether this evidence describes exactly `intent`, every field and the
    /// digest (equal data; it says nothing about durability).
    pub fn describes(&self, intent: &RecordIntent) -> bool {
        self.id == intent.id
            && self.at == intent.at
            && self.kind == intent.kind
            && self.digest == intent.digest
    }
}

/// One request decoded from one frame: a description. Parsing does not
/// authenticate it, and it earns nothing but the core's own checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestEvidence {
    pub request: Request,
    pub digest: [u8; DIGEST_LEN],
}

/// The digest of a record the core is issuing, from the typed fields it
/// made (before any intent exists): exactly the digest [`encode_record`]
/// writes and [`decode_record`] checks.
pub fn record_digest(id: RecordId, at: Tick, kind: &RecordKind) -> [u8; DIGEST_LEN] {
    digest_of(&record_covered(id, at, kind))
}

/// The frame of an issued record, for a recorder. Refused, never
/// overwritten, if the digest the intent carries is not its fields'.
pub fn encode_record(intent: &RecordIntent) -> Result<Vec<u8>, CodecError> {
    let covered = record_covered(intent.id, intent.at, &intent.kind);
    if digest_of(&covered) != intent.digest {
        return Err(CodecError::IntentDigestMismatch);
    }
    Ok(seal(covered))
}

/// Exactly one complete, consistent record frame, as evidence data.
pub fn decode_record(bytes: &[u8]) -> Result<RecordEvidence, CodecError> {
    let (payload, digest) = open(bytes, Domain::Record)?;
    let mut fields = Reader { rest: payload };
    let generation = fields.generation()?;
    let seq = fields.u64()?;
    let at = Tick(fields.u64()?);
    let kind = match fields.u8()? {
        0x01 => RecordKind::RunStarted {
            dispositioned: fields.u32()?,
        },
        0x02 => {
            let case = CaseId(fields.u32()?);
            let expectation = expectation(fields.u8()?)?;
            RecordKind::CaseStarted { case, expectation }
        }
        0x03 => {
            let action = fields.action()?;
            let kind = slot(fields.u8()?)?;
            RecordKind::ActionStarted { action, kind }
        }
        0x04 => RecordKind::ActionFailed {
            action: fields.action()?,
        },
        0x05 => {
            let action = fields.action()?;
            let how = settlement(fields.u8()?)?;
            RecordKind::ActionSettled { action, how }
        }
        0x06 => {
            let case = CaseId(fields.u32()?);
            let passed = fields.bool("case passed")?;
            RecordKind::CaseEnded { case, passed }
        }
        0x07 => RecordKind::Control {
            fact: control_fact(&mut fields)?,
        },
        0x08 => RecordKind::RecoveryRequired,
        0x09 => {
            let attempt = fields.u32()?;
            let resolved = fields.bool("recovery resolved")?;
            RecordKind::RecoveryAttempt { attempt, resolved }
        }
        0x0a => {
            let verdict = verdict(fields.u8()?)?;
            let resolved_by = match fields.present("resolved by")? {
                true => Some(fields.u32()?),
                false => None,
            };
            RecordKind::RunEnded {
                verdict,
                resolved_by,
            }
        }
        0x0b => {
            let incident = fields.incident()?;
            let action = fields.action()?;
            let kind = match fields.present("incident owner kind")? {
                true => Some(slot(fields.u8()?)?),
                false => None,
            };
            RecordKind::IncidentOpened {
                incident,
                action,
                kind,
            }
        }
        0x0c => {
            let incident = fields.incident()?;
            let how = settlement(fields.u8()?)?;
            RecordKind::IncidentSettled { incident, how }
        }
        tag => {
            return Err(CodecError::UnknownTag {
                field: "record kind",
                tag,
            })
        }
    };
    fields.finish()?;
    Ok(RecordEvidence {
        id: RecordId { generation, seq },
        at,
        kind,
        digest,
    })
}

/// The digest the core binds a request's receipt to: exactly the digest
/// [`encode_request`] writes and [`decode_request`] checks.
pub fn request_digest(request: &Request) -> [u8; DIGEST_LEN] {
    digest_of(&request_covered(request))
}

/// The frame of a request.
pub fn encode_request(request: &Request) -> Vec<u8> {
    seal(request_covered(request))
}

/// Exactly one complete, consistent request frame, as a description.
pub fn decode_request(bytes: &[u8]) -> Result<RequestEvidence, CodecError> {
    let (payload, digest) = open(bytes, Domain::Request)?;
    let mut fields = Reader { rest: payload };
    let generation = fields.generation()?;
    let seq = fields.u64()?;
    let op = match fields.u8()? {
        0x01 => RequestOp::Retry {
            epoch: fields.u64()?,
        },
        0x02 => RequestOp::Shutdown,
        tag => {
            return Err(CodecError::UnknownTag {
                field: "request operation",
                tag,
            })
        }
    };
    fields.finish()?;
    Ok(RequestEvidence {
        request: Request {
            generation,
            seq,
            op,
        },
        digest,
    })
}

fn digest_of(covered: &[u8]) -> [u8; DIGEST_LEN] {
    Sha256::digest(covered).into()
}

/// The frame: the covered bytes and their digest.
fn seal(covered: Vec<u8>) -> Vec<u8> {
    let digest = digest_of(&covered);
    let mut frame = covered;
    frame.extend_from_slice(&digest);
    frame
}

/// A record's covered bytes (header and payload), from typed fields.
fn record_covered(id: RecordId, at: Tick, kind: &RecordKind) -> Vec<u8> {
    let mut out = Writer::new(Domain::Record);
    out.generation(id.generation());
    out.u64(id.seq());
    out.u64(at.0);
    match *kind {
        RecordKind::RunStarted { dispositioned } => {
            out.u8(0x01);
            out.u32(dispositioned);
        }
        RecordKind::CaseStarted { case, expectation } => {
            out.u8(0x02);
            out.u32(case.0);
            out.u8(expectation_tag(expectation));
        }
        RecordKind::ActionStarted { action, kind } => {
            out.u8(0x03);
            out.action(action);
            out.u8(slot_tag(kind));
        }
        RecordKind::ActionFailed { action } => {
            out.u8(0x04);
            out.action(action);
        }
        RecordKind::ActionSettled { action, how } => {
            out.u8(0x05);
            out.action(action);
            out.u8(settlement_tag(how));
        }
        RecordKind::CaseEnded { case, passed } => {
            out.u8(0x06);
            out.u32(case.0);
            out.bool(passed);
        }
        RecordKind::Control { fact } => {
            out.u8(0x07);
            match fact {
                ControlFact::AdmissionClosed { reason, after } => {
                    out.u8(0x01);
                    match reason {
                        ClosureReason::Cancelled(cancel) => {
                            out.u8(0x01);
                            out.u8(cancel_tag(cancel));
                        }
                        ClosureReason::Failed(class) => {
                            out.u8(0x02);
                            out.u8(failure_tag(class));
                        }
                    }
                    out.u64(after);
                }
                ControlFact::ShutdownRefused => out.u8(0x02),
            }
        }
        RecordKind::RecoveryRequired => out.u8(0x08),
        RecordKind::RecoveryAttempt { attempt, resolved } => {
            out.u8(0x09);
            out.u32(attempt);
            out.bool(resolved);
        }
        RecordKind::RunEnded {
            verdict,
            resolved_by,
        } => {
            out.u8(0x0a);
            out.u8(verdict_tag(verdict));
            match resolved_by {
                Some(attempt) => {
                    out.u8(0x01);
                    out.u32(attempt);
                }
                None => out.u8(0x00),
            }
        }
        RecordKind::IncidentOpened {
            incident,
            action,
            kind,
        } => {
            out.u8(0x0b);
            out.incident(incident);
            out.action(action);
            match kind {
                Some(kind) => {
                    out.u8(0x01);
                    out.u8(slot_tag(kind));
                }
                None => out.u8(0x00),
            }
        }
        RecordKind::IncidentSettled { incident, how } => {
            out.u8(0x0c);
            out.incident(incident);
            out.u8(settlement_tag(how));
        }
    }
    out.covered()
}

/// A request's covered bytes (header and payload).
fn request_covered(request: &Request) -> Vec<u8> {
    let mut out = Writer::new(Domain::Request);
    out.generation(request.generation);
    out.u64(request.seq);
    match request.op {
        RequestOp::Retry { epoch } => {
            out.u8(0x01);
            out.u64(epoch);
        }
        RequestOp::Shutdown => out.u8(0x02),
    }
    out.covered()
}

/// The envelope of exactly one frame of `domain`, checked before any field
/// is read: size, header, exact length and digest. Returns the payload and
/// the digest. Nothing is allocated, and every slice is checked.
fn open(bytes: &[u8], domain: Domain) -> Result<(&[u8], [u8; DIGEST_LEN]), CodecError> {
    if bytes.len() > MAX_FRAME {
        return Err(CodecError::TooLong { len: bytes.len() });
    }
    let header: [u8; HEADER_LEN] = *bytes.first_chunk().ok_or(CodecError::Truncated)?;
    let [m0, m1, m2, m3, found, version, l0, l1] = header;
    if [m0, m1, m2, m3] != MAGIC {
        return Err(CodecError::BadMagic);
    }
    if found != domain.byte() {
        return Err(CodecError::WrongDomain {
            expected: domain,
            found,
        });
    }
    if version != VERSION {
        return Err(CodecError::UnsupportedVersion { found: version });
    }
    let declared = u16::from_be_bytes([l0, l1]);
    let length = usize::from(declared);
    if length == 0 || length > MAX_PAYLOAD {
        return Err(CodecError::BadLength { declared });
    }
    let covered_len = HEADER_LEN
        .checked_add(length)
        .ok_or(CodecError::BadLength { declared })?;
    let frame_len = covered_len
        .checked_add(DIGEST_LEN)
        .ok_or(CodecError::BadLength { declared })?;
    match bytes.len().cmp(&frame_len) {
        Ordering::Less => return Err(CodecError::Truncated),
        Ordering::Greater => {
            return Err(CodecError::TrailingBytes {
                extra: bytes.len().saturating_sub(frame_len),
            })
        }
        Ordering::Equal => {}
    }
    let covered = bytes.get(..covered_len).ok_or(CodecError::Truncated)?;
    let trailer = bytes
        .get(covered_len..frame_len)
        .ok_or(CodecError::Truncated)?;
    let digest: [u8; DIGEST_LEN] = trailer.try_into().map_err(|_| CodecError::Truncated)?;
    if digest_of(covered) != digest {
        return Err(CodecError::DigestMismatch);
    }
    let payload = covered.get(HEADER_LEN..).ok_or(CodecError::Truncated)?;
    Ok((payload, digest))
}

/// The one canonical encoder: fields in table order, big-endian; the
/// payload length is filled in last.
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new(domain: Domain) -> Self {
        let mut bytes = Vec::with_capacity(HEADER_LEN + LARGEST_PAYLOAD + DIGEST_LEN);
        bytes.extend_from_slice(&MAGIC);
        bytes.push(domain.byte());
        bytes.push(VERSION);
        bytes.extend_from_slice(&[0, 0]);
        Self { bytes }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn generation(&mut self, generation: Generation) {
        self.bytes.extend_from_slice(&generation.bytes());
    }

    fn action(&mut self, action: ActionId) {
        self.generation(action.generation());
        self.u64(action.seq());
    }

    fn incident(&mut self, incident: IncidentId) {
        self.generation(incident.generation());
        self.u64(incident.seq());
    }

    /// The covered bytes, with the payload length filled in (every
    /// version-1 payload is at most `LARGEST_PAYLOAD` bytes, so it fits).
    fn covered(mut self) -> Vec<u8> {
        let payload = self.bytes.len().saturating_sub(HEADER_LEN);
        let declared = u16::try_from(payload).unwrap_or(u16::MAX).to_be_bytes();
        if let Some(field) = self.bytes.get_mut(6..HEADER_LEN) {
            field.copy_from_slice(&declared);
        }
        self.bytes
    }
}

/// A bounded cursor over one payload: every read is checked, nothing is
/// allocated, and reading past the payload is an error, never a panic.
struct Reader<'a> {
    rest: &'a [u8],
}

impl Reader<'_> {
    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let (head, rest) = self
            .rest
            .split_first_chunk::<N>()
            .ok_or(CodecError::ShortPayload)?;
        self.rest = rest;
        Ok(*head)
    }

    fn u8(&mut self) -> Result<u8, CodecError> {
        let [byte] = self.array::<1>()?;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn bool(&mut self, field: &'static str) -> Result<bool, CodecError> {
        match self.u8()? {
            0x00 => Ok(false),
            0x01 => Ok(true),
            value => Err(CodecError::InvalidBool { field, value }),
        }
    }

    fn present(&mut self, field: &'static str) -> Result<bool, CodecError> {
        match self.u8()? {
            0x00 => Ok(false),
            0x01 => Ok(true),
            value => Err(CodecError::InvalidPresence { field, value }),
        }
    }

    fn generation(&mut self) -> Result<Generation, CodecError> {
        Ok(Generation::new(self.array()?))
    }

    fn action(&mut self) -> Result<ActionId, CodecError> {
        let generation = self.generation()?;
        let seq = self.u64()?;
        Ok(ActionId { generation, seq })
    }

    fn incident(&mut self) -> Result<IncidentId, CodecError> {
        let generation = self.generation()?;
        let seq = self.u64()?;
        Ok(IncidentId { generation, seq })
    }

    /// The payload must be used exactly.
    fn finish(self) -> Result<(), CodecError> {
        match self.rest.len() {
            0 => Ok(()),
            unused => Err(CodecError::LongPayload { unused }),
        }
    }
}

fn control_fact(fields: &mut Reader<'_>) -> Result<ControlFact, CodecError> {
    match fields.u8()? {
        0x01 => {
            let reason = match fields.u8()? {
                0x01 => ClosureReason::Cancelled(cancel_reason(fields.u8()?)?),
                0x02 => ClosureReason::Failed(failure_class(fields.u8()?)?),
                tag => {
                    return Err(CodecError::UnknownTag {
                        field: "closure reason",
                        tag,
                    })
                }
            };
            let after = fields.u64()?;
            Ok(ControlFact::AdmissionClosed { reason, after })
        }
        0x02 => Ok(ControlFact::ShutdownRefused),
        tag => Err(CodecError::UnknownTag {
            field: "control fact",
            tag,
        }),
    }
}

fn expectation_tag(value: Expectation) -> u8 {
    match value {
        Expectation::Clean => 0x01,
        Expectation::RetainedBoundary => 0x02,
        Expectation::OutputDetached => 0x03,
    }
}

fn expectation(tag: u8) -> Result<Expectation, CodecError> {
    match tag {
        0x01 => Ok(Expectation::Clean),
        0x02 => Ok(Expectation::RetainedBoundary),
        0x03 => Ok(Expectation::OutputDetached),
        tag => Err(CodecError::UnknownTag {
            field: "expectation",
            tag,
        }),
    }
}

fn slot_tag(value: SlotKind) -> u8 {
    match value {
        SlotKind::Process => 0x01,
        SlotKind::Workspace => 0x02,
        SlotKind::Fixture => 0x03,
    }
}

fn slot(tag: u8) -> Result<SlotKind, CodecError> {
    match tag {
        0x01 => Ok(SlotKind::Process),
        0x02 => Ok(SlotKind::Workspace),
        0x03 => Ok(SlotKind::Fixture),
        tag => Err(CodecError::UnknownTag {
            field: "slot kind",
            tag,
        }),
    }
}

fn settlement_tag(value: Settlement) -> u8 {
    match value {
        Settlement::Confirmed => 0x01,
        Settlement::OutputLost => 0x02,
        Settlement::NothingCreated => 0x03,
        Settlement::NotAdmitted => 0x04,
    }
}

fn settlement(tag: u8) -> Result<Settlement, CodecError> {
    match tag {
        0x01 => Ok(Settlement::Confirmed),
        0x02 => Ok(Settlement::OutputLost),
        0x03 => Ok(Settlement::NothingCreated),
        0x04 => Ok(Settlement::NotAdmitted),
        tag => Err(CodecError::UnknownTag {
            field: "settlement",
            tag,
        }),
    }
}

fn verdict_tag(value: Verdict) -> u8 {
    match value {
        Verdict::Pending => 0x01,
        Verdict::Passed => 0x02,
        Verdict::Failed => 0x03,
    }
}

fn verdict(tag: u8) -> Result<Verdict, CodecError> {
    match tag {
        0x01 => Ok(Verdict::Pending),
        0x02 => Ok(Verdict::Passed),
        0x03 => Ok(Verdict::Failed),
        tag => Err(CodecError::UnknownTag {
            field: "verdict",
            tag,
        }),
    }
}

fn cancel_tag(value: CancelReason) -> u8 {
    match value {
        CancelReason::Requested => 0x01,
        CancelReason::LeaseLost => 0x02,
        CancelReason::Shutdown => 0x03,
        CancelReason::Stop => 0x04,
        CancelReason::RunBudget => 0x05,
    }
}

fn cancel_reason(tag: u8) -> Result<CancelReason, CodecError> {
    match tag {
        0x01 => Ok(CancelReason::Requested),
        0x02 => Ok(CancelReason::LeaseLost),
        0x03 => Ok(CancelReason::Shutdown),
        0x04 => Ok(CancelReason::Stop),
        0x05 => Ok(CancelReason::RunBudget),
        tag => Err(CodecError::UnknownTag {
            field: "cancel reason",
            tag,
        }),
    }
}

fn failure_tag(value: FailureClass) -> u8 {
    match value {
        FailureClass::Assertion => 0x01,
        FailureClass::UnexpectedRetained => 0x02,
        FailureClass::UnexpectedCleanup => 0x03,
        FailureClass::ExpectedConditionUnmet => 0x04,
        FailureClass::UnexpectedOwner => 0x05,
        FailureClass::LateOwner => 0x06,
        FailureClass::UnknownOutcome => 0x07,
        FailureClass::OutputLost => 0x08,
        FailureClass::AuthorityLost => 0x09,
        FailureClass::RecordFailed => 0x0a,
        FailureClass::RecorderFault => 0x0b,
        FailureClass::Cancelled => 0x0c,
    }
}

fn failure_class(tag: u8) -> Result<FailureClass, CodecError> {
    match tag {
        0x01 => Ok(FailureClass::Assertion),
        0x02 => Ok(FailureClass::UnexpectedRetained),
        0x03 => Ok(FailureClass::UnexpectedCleanup),
        0x04 => Ok(FailureClass::ExpectedConditionUnmet),
        0x05 => Ok(FailureClass::UnexpectedOwner),
        0x06 => Ok(FailureClass::LateOwner),
        0x07 => Ok(FailureClass::UnknownOutcome),
        0x08 => Ok(FailureClass::OutputLost),
        0x09 => Ok(FailureClass::AuthorityLost),
        0x0a => Ok(FailureClass::RecordFailed),
        0x0b => Ok(FailureClass::RecorderFault),
        0x0c => Ok(FailureClass::Cancelled),
        tag => Err(CodecError::UnknownTag {
            field: "failure class",
            tag,
        }),
    }
}
