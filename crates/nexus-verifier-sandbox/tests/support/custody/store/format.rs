//! The store's byte and text formats (design sections 7 and 12.3): the
//! journal container (header, closure seal and record blocks around the
//! unchanged version-1 record frames), the `PROVISION` and disposition text
//! grammars, the store's entry names, the incident bindings and the storage
//! identity record.
//!
//! Everything here is pure: bytes in, plain data out. Every assigned byte is
//! checked and every unassigned byte must be zero (full consumption); every
//! length and every arithmetic step is checked before it is used. Decoding
//! creates no authority of any kind: a parsed `PROVISION` is a set of claimed
//! fields that an opening compares with fresh observations; a parsed header,
//! seal or record is a description, not an acknowledgement, not proof of
//! durability and not proof of native cleanup; a parsed disposition is
//! accepted only by the disposition reader, after the scan has read it under
//! the store lock from a root-owned file. Nothing here can claim a journal,
//! admit storage, acknowledge a record, start maintenance or construct a
//! native owner.

use sha2::{Digest, Sha256};

use super::super::codec::{self, CodecError, RecordEvidence};
use super::super::{DispositionReason, RecordIntent};

/// One container block: 4096 bytes.
pub const BLOCK: usize = 4096;
/// `BLOCK` as a file offset unit.
pub const BLOCK_U64: u64 = BLOCK as u64;
/// The header magic, `NXCJ`.
pub const HEADER_MAGIC: [u8; 4] = *b"NXCJ";
/// The closure seal's magic, `NXCS`.
pub const SEAL_MAGIC: [u8; 4] = *b"NXCS";
/// Header bytes 4..8: container version, frame domain, frame version, log2
/// of the block size.
pub const HEADER_VERSIONS: [u8; 4] = [0x01, 0x52, 0x01, 0x0c];
/// The highest claim number: claims are 1 ..= 2^63 and never wrap.
pub const MAX_CLAIM: u64 = 1 << 63;
/// Applied dispositions a header lists at most.
pub const MAX_APPLIED: usize = 32;
/// Pool files a store has at most.
pub const MAX_POOL: u32 = 1024;
/// Record blocks a pool file has at most.
pub const MAX_C_POOL: u32 = 4096;
/// The version-1 record payload range (`codec.rs`: the largest is 83).
pub const MIN_RECORD_PAYLOAD: usize = 33;
pub const MAX_RECORD_PAYLOAD: usize = 83;
/// Read bounds of the text files.
pub const PROVISION_LIMIT: usize = 65_536;
pub const DISPOSITION_LIMIT: usize = 4096;
/// Entry bounds, counted from the names before any entry is examined.
pub const ARCHIVE_ENTRY_LIMIT: usize = 4096;
pub const DISPOSITIONS_ENTRY_LIMIT: usize = 4097;
pub const REVOKED_ENTRY_LIMIT: usize = 4096;
/// Incidents a report lists in detail; the rest only in its totals.
pub const REPORT_LIMIT: usize = 64;
/// Selections an opening makes at most before it refuses as
/// SelectionChanged (design section 10.5).
pub const SELECTION_ATTEMPTS: u32 = 3;
/// The prefix of every incident binding: the ASCII text, a zero byte and the
/// binding version `01`.
pub const BINDING_PREFIX: &[u8] = b"nexus-phase2-custody-incident\x00\x01";
/// The prefix of the storage identity record, with its version byte.
pub const STORAGE_IDENTITY_PREFIX: &[u8] = b"nexus-phase2-custody-storage\x00\x01";
/// The only admitted storage class.
pub const STORAGE_CLASS: &str = "nvme-pcie";
/// Each storage attribute is read with this bound.
pub const STORAGE_READ_LIMIT: usize = 4096;

/// A pool file's size for `c_pool` record blocks, with checked arithmetic.
pub fn pool_file_size(c_pool: u32) -> Option<u64> {
    u64::from(c_pool).checked_add(2)?.checked_mul(BLOCK_U64)
}

/// The SHA-256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn be16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn be64(bytes: &[u8]) -> u64 {
    let mut out = [0u8; 8];
    out.copy_from_slice(&bytes[..8]);
    u64::from_be_bytes(out)
}

fn zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

/// A disposition reason's header tag (`01` to `05`, design section 7.2).
pub fn reason_tag(reason: DispositionReason) -> u8 {
    match reason {
        DispositionReason::OwnerDestroyed => 0x01,
        DispositionReason::HostRebooted => 0x02,
        DispositionReason::RecordsMalformed => 0x03,
        DispositionReason::CompletionNotRecorded => 0x04,
        DispositionReason::Other => 0x05,
    }
}

/// The reason a header tag names, if any.
pub fn reason_of_tag(tag: u8) -> Option<DispositionReason> {
    match tag {
        0x01 => Some(DispositionReason::OwnerDestroyed),
        0x02 => Some(DispositionReason::HostRebooted),
        0x03 => Some(DispositionReason::RecordsMalformed),
        0x04 => Some(DispositionReason::CompletionNotRecorded),
        0x05 => Some(DispositionReason::Other),
        _ => None,
    }
}

/// A disposition reason's text form (design section 7.5).
pub fn reason_text(reason: DispositionReason) -> &'static str {
    match reason {
        DispositionReason::OwnerDestroyed => "owner-destroyed",
        DispositionReason::HostRebooted => "host-rebooted",
        DispositionReason::RecordsMalformed => "records-malformed",
        DispositionReason::CompletionNotRecorded => "completion-not-recorded",
        DispositionReason::Other => "other",
    }
}

fn reason_of_text(text: &str) -> Option<DispositionReason> {
    [
        DispositionReason::OwnerDestroyed,
        DispositionReason::HostRebooted,
        DispositionReason::RecordsMalformed,
        DispositionReason::CompletionNotRecorded,
        DispositionReason::Other,
    ]
    .into_iter()
    .find(|reason| reason_text(*reason) == text)
}

/// A structure an encoder refused to write: it would not parse as valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatError(pub &'static str);

// ---------------------------------------------------------------------------
// Header (block 0, design section 7.2)
// ---------------------------------------------------------------------------

/// The fields of a claim header, as the claim writes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderFields {
    pub root_id: [u8; 16],
    pub generation: [u8; 16],
    pub claim: u64,
    pub pool_index: u32,
    pub c_pool: u32,
    pub capacity: u32,
    /// Data only (Unix milliseconds); never interpreted.
    pub created_ms: u64,
    /// Data only; never interpreted.
    pub boot_id: [u8; 16],
    /// The applied dispositions: bindings strictly increasing, each with the
    /// reason of its disposition.
    pub applied: Vec<([u8; 32], DispositionReason)>,
}

/// A valid header, as parsed: plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub root_id: [u8; 16],
    pub generation: [u8; 16],
    pub claim: u64,
    pub pool_index: u32,
    pub c_pool: u32,
    pub capacity: u32,
    pub created_ms: u64,
    pub boot_id: [u8; 16],
    pub applied: Vec<([u8; 32], DispositionReason)>,
    /// The header's own digest field.
    pub digest: [u8; 32],
}

/// What block 0 holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderParse {
    /// Every byte zero.
    Zero,
    /// Not checksum-valid: wrong magic, a count above 32 or a digest that does
    /// not match. In a pool file with every other block zero, an abandoned
    /// claim; otherwise Malformed.
    ChecksumInvalid(&'static str),
    /// Checksum-valid but breaking a rule: always Malformed, never an
    /// abandoned claim.
    Invalid(&'static str),
    Valid(Header),
}

/// The block of a claim header. Refused unless it would parse as valid.
pub fn encode_header(fields: &HeaderFields) -> Result<Box<[u8; BLOCK]>, FormatError> {
    if zero(&fields.generation) {
        return Err(FormatError("zero generation"));
    }
    if !(1..=MAX_CLAIM).contains(&fields.claim) {
        return Err(FormatError("claim range"));
    }
    if fields.pool_index >= MAX_POOL {
        return Err(FormatError("pool index"));
    }
    if !(1..=MAX_C_POOL).contains(&fields.c_pool) {
        return Err(FormatError("pool capacity"));
    }
    if !(1..=fields.c_pool).contains(&fields.capacity) {
        return Err(FormatError("capacity"));
    }
    if fields.applied.len() > MAX_APPLIED {
        return Err(FormatError("too many applied dispositions"));
    }
    if fields.applied.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(FormatError("bindings not strictly increasing"));
    }
    let mut block = Box::new([0u8; BLOCK]);
    block[0..4].copy_from_slice(&HEADER_MAGIC);
    block[4..8].copy_from_slice(&HEADER_VERSIONS);
    block[8..24].copy_from_slice(&fields.root_id);
    block[24..40].copy_from_slice(&fields.generation);
    block[40..48].copy_from_slice(&fields.claim.to_be_bytes());
    block[48..52].copy_from_slice(&fields.pool_index.to_be_bytes());
    block[52..56].copy_from_slice(&fields.c_pool.to_be_bytes());
    block[56..60].copy_from_slice(&fields.capacity.to_be_bytes());
    block[64..72].copy_from_slice(&fields.created_ms.to_be_bytes());
    block[72..88].copy_from_slice(&fields.boot_id);
    block[88] = fields.applied.len() as u8;
    let mut at = 92;
    for (binding, reason) in &fields.applied {
        block[at..at + 32].copy_from_slice(binding);
        block[at + 32] = reason_tag(*reason);
        at += 33;
    }
    let digest = sha256(&block[0..at]);
    block[at..at + 32].copy_from_slice(&digest);
    Ok(block)
}

/// Parse block 0 against the store's root id and, for a pool file, its
/// pool capacity and index (`None` for an archive entry: then the index must
/// be below 1024 and the capacity 1 to 4096).
pub fn parse_header(
    block: &[u8],
    root_id: &[u8; 16],
    c_pool: Option<u32>,
    index: Option<u32>,
) -> HeaderParse {
    if block.len() != BLOCK {
        return HeaderParse::ChecksumInvalid("block length");
    }
    if zero(block) {
        return HeaderParse::Zero;
    }
    let n = usize::from(block[88]);
    if block[0..4] != HEADER_MAGIC {
        return HeaderParse::ChecksumInvalid("magic");
    }
    if n > MAX_APPLIED {
        return HeaderParse::ChecksumInvalid("applied count");
    }
    let end = 92 + 33 * n;
    if sha256(&block[0..end]) != block[end..end + 32] {
        return HeaderParse::ChecksumInvalid("digest");
    }
    if block[4..8] != HEADER_VERSIONS {
        return HeaderParse::Invalid("version bytes");
    }
    if block[8..24] != root_id[..] {
        return HeaderParse::Invalid("root id");
    }
    let mut generation = [0u8; 16];
    generation.copy_from_slice(&block[24..40]);
    if zero(&generation) {
        return HeaderParse::Invalid("zero generation");
    }
    let claim = be64(&block[40..48]);
    if !(1..=MAX_CLAIM).contains(&claim) {
        return HeaderParse::Invalid("claim range");
    }
    let pool_index = be32(&block[48..52]);
    match index {
        Some(expected) if pool_index != expected => {
            return HeaderParse::Invalid("pool index");
        }
        None if pool_index >= MAX_POOL => return HeaderParse::Invalid("pool index"),
        _ => {}
    }
    let cp = be32(&block[52..56]);
    if !(1..=MAX_C_POOL).contains(&cp) || c_pool.is_some_and(|expected| expected != cp) {
        return HeaderParse::Invalid("pool capacity");
    }
    let capacity = be32(&block[56..60]);
    if !(1..=cp).contains(&capacity) {
        return HeaderParse::Invalid("capacity");
    }
    if !zero(&block[60..64]) || !zero(&block[89..92]) {
        return HeaderParse::Invalid("reserved bytes");
    }
    let mut applied = Vec::with_capacity(n);
    for k in 0..n {
        let at = 92 + 33 * k;
        let mut binding = [0u8; 32];
        binding.copy_from_slice(&block[at..at + 32]);
        let Some(reason) = reason_of_tag(block[at + 32]) else {
            return HeaderParse::Invalid("reason tag");
        };
        if applied
            .last()
            .is_some_and(|(previous, _): &([u8; 32], DispositionReason)| binding <= *previous)
        {
            return HeaderParse::Invalid("bindings not strictly increasing");
        }
        applied.push((binding, reason));
    }
    if !zero(&block[end + 32..]) {
        return HeaderParse::Invalid("bytes after the digest");
    }
    let mut boot_id = [0u8; 16];
    boot_id.copy_from_slice(&block[72..88]);
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&block[end..end + 32]);
    HeaderParse::Valid(Header {
        root_id: *root_id,
        generation,
        claim,
        pool_index,
        c_pool: cp,
        capacity,
        created_ms: be64(&block[64..72]),
        boot_id,
        applied,
        digest,
    })
}

// ---------------------------------------------------------------------------
// Closure seal (block 1, design section 7.3)
// ---------------------------------------------------------------------------

/// The seal's verdict byte: `00` with no terminal record, otherwise the
/// `RunEnded` verdict's codec tag (`02` Passed, `03` Failed).
pub const SEAL_NO_VERDICT: u8 = 0x00;
pub const SEAL_PASSED: u8 = 0x02;
pub const SEAL_FAILED: u8 = 0x03;

/// The fields of a closure seal, as the recorder writes them after `close()`
/// returned `Ok` with nothing latched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealFields {
    pub generation: [u8; 16],
    pub claim: u64,
    pub header_digest: [u8; 32],
    pub records: u64,
    pub last_digest: [u8; 32],
    pub terminal: u64,
    pub verdict: u8,
    pub resolved_by: Option<u32>,
    /// Data only (Unix milliseconds).
    pub closed_ms: u64,
}

/// The seal block.
pub fn encode_seal(fields: &SealFields) -> Box<[u8; BLOCK]> {
    let mut block = Box::new([0u8; BLOCK]);
    block[0..4].copy_from_slice(&SEAL_MAGIC);
    block[4] = 0x01;
    block[8..24].copy_from_slice(&fields.generation);
    block[24..32].copy_from_slice(&fields.claim.to_be_bytes());
    block[32..64].copy_from_slice(&fields.header_digest);
    block[64..72].copy_from_slice(&fields.records.to_be_bytes());
    block[72..104].copy_from_slice(&fields.last_digest);
    block[104..112].copy_from_slice(&fields.terminal.to_be_bytes());
    block[112] = fields.verdict;
    block[113] = u8::from(fields.resolved_by.is_some());
    block[114..118].copy_from_slice(&fields.resolved_by.unwrap_or(0).to_be_bytes());
    block[118..126].copy_from_slice(&fields.closed_ms.to_be_bytes());
    let digest = sha256(&block[0..128]);
    block[128..160].copy_from_slice(&digest);
    block
}

/// What the seal block says about the valid record prefix before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefixFacts {
    /// Records in the valid prefix.
    pub records: u64,
    /// The last record's digest, zero for an empty prefix.
    pub last_digest: [u8; 32],
    /// The `RunEnded` record: its sequence, verdict and `resolved_by`.
    pub run_ended: Option<(u64, super::super::Verdict, Option<u32>)>,
    pub run_started: bool,
    /// Started actions or opened incidents not settled in the prefix.
    pub unsettled: u64,
}

/// The seal block's classification (design section 7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealParse {
    /// Every byte zero.
    Unsealed,
    /// Not checksum-valid. In domain H a torn seal write; treated as unsealed,
    /// the more conservative state, and reported as unreadable.
    Unreadable,
    /// Checksum-valid but breaking a rule.
    Malformed(&'static str),
    Sealed,
}

/// The codec's verdict byte (the seal stores it for a terminal record).
fn verdict_byte(verdict: super::super::Verdict) -> u8 {
    match verdict {
        super::super::Verdict::Pending => 0x01,
        super::super::Verdict::Passed => SEAL_PASSED,
        super::super::Verdict::Failed => SEAL_FAILED,
    }
}

/// The seal the recorder writes for a prefix (design section 9.7).
pub fn seal_for(header: &Header, prefix: &PrefixFacts, closed_ms: u64) -> SealFields {
    let (terminal, verdict, resolved_by) = match prefix.run_ended {
        Some((seq, verdict, by)) => (seq, verdict_byte(verdict), by),
        None => (0, SEAL_NO_VERDICT, None),
    };
    SealFields {
        generation: header.generation,
        claim: header.claim,
        header_digest: header.digest,
        records: prefix.records,
        last_digest: prefix.last_digest,
        terminal,
        verdict,
        resolved_by,
        closed_ms,
    }
}

/// Classify block 1 against its header and the valid record prefix.
pub fn parse_seal(block: &[u8], header: &Header, prefix: &PrefixFacts) -> SealParse {
    if block.len() != BLOCK {
        return SealParse::Unreadable;
    }
    if zero(block) {
        return SealParse::Unsealed;
    }
    if block[0..4] != SEAL_MAGIC || sha256(&block[0..128]) != block[128..160] {
        return SealParse::Unreadable;
    }
    if block[4] != 0x01 {
        return SealParse::Malformed("version");
    }
    if !zero(&block[5..8]) || !zero(&block[126..128]) || !zero(&block[160..]) {
        return SealParse::Malformed("reserved or tail bytes");
    }
    if block[8..24] != header.generation || be64(&block[24..32]) != header.claim {
        return SealParse::Malformed("identity");
    }
    if block[32..64] != header.digest {
        return SealParse::Malformed("header digest");
    }
    let records = be64(&block[64..72]);
    if records != prefix.records || records > u64::from(header.capacity) {
        return SealParse::Malformed("record count");
    }
    if block[72..104] != prefix.last_digest {
        return SealParse::Malformed("last record digest");
    }
    let (terminal, verdict, by) = match prefix.run_ended {
        Some((seq, verdict, by)) => (seq, verdict_byte(verdict), by),
        None => (0, SEAL_NO_VERDICT, None),
    };
    if be64(&block[104..112]) != terminal {
        return SealParse::Malformed("terminal sequence");
    }
    if block[112] != verdict {
        return SealParse::Malformed("verdict");
    }
    if block[113] > 1 || (block[113] == 1) != by.is_some() {
        return SealParse::Malformed("resolved_by presence");
    }
    if be32(&block[114..118]) != by.unwrap_or(0) {
        return SealParse::Malformed("resolved_by");
    }
    if prefix.run_started && prefix.run_ended.is_none() {
        return SealParse::Malformed("a started run without RunEnded");
    }
    if prefix.unsettled > 0 {
        return SealParse::Malformed("recorded unsettled entries");
    }
    SealParse::Sealed
}

// ---------------------------------------------------------------------------
// Record blocks (block 1 + n, design section 7.4)
// ---------------------------------------------------------------------------

/// A record block: the intent's unchanged version-1 frame, then zeros. The
/// codec refuses an intent whose digest is not its fields' own.
pub fn encode_record_block(intent: &RecordIntent) -> Result<Box<[u8; BLOCK]>, CodecError> {
    let frame = codec::encode_record(intent)?;
    let mut block = Box::new([0u8; BLOCK]);
    if frame.len() > BLOCK {
        return Err(CodecError::TooLong { len: frame.len() });
    }
    block[..frame.len()].copy_from_slice(&frame);
    Ok(block)
}

/// What one record block holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordBlock {
    Zero,
    Invalid(String),
    Valid(RecordEvidence),
}

/// Parse a record block: one version-1 record frame whose payload length is
/// in the record range, exactly the bytes the codec accepts, then zeros.
pub fn parse_record_block(block: &[u8]) -> RecordBlock {
    if block.len() != BLOCK {
        return RecordBlock::Invalid("block length".into());
    }
    if zero(block) {
        return RecordBlock::Zero;
    }
    let length = usize::from(be16(&block[6..8]));
    if !(MIN_RECORD_PAYLOAD..=MAX_RECORD_PAYLOAD).contains(&length) {
        return RecordBlock::Invalid("payload length".into());
    }
    let end = codec::HEADER_LEN + length + codec::DIGEST_LEN;
    match codec::decode_record(&block[..end]) {
        Err(error) => RecordBlock::Invalid(format!("{error:?}")),
        Ok(_) if !zero(&block[end..]) => RecordBlock::Invalid("padding".into()),
        Ok(record) => RecordBlock::Valid(record),
    }
}

// ---------------------------------------------------------------------------
// Bindings (design section 12.3) and the storage identity (section 6.4 step 12)
// ---------------------------------------------------------------------------

/// The class a journal binding carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IncidentClass {
    Unresolved,
    Malformed,
}

impl IncidentClass {
    fn byte(self) -> u8 {
        match self {
            IncidentClass::Unresolved => 0x01,
            IncidentClass::Malformed => 0x02,
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            IncidentClass::Unresolved => "unresolved",
            IncidentClass::Malformed => "malformed",
        }
    }
}

/// A pool journal's binding: content-addressed, independent of where the
/// bytes lie, so an archive copy keeps it and any byte change makes another.
pub fn bind_journal(
    root_id: &[u8; 16],
    claim: u64,
    generation: &[u8; 16],
    content: &[u8; 32],
    class: IncidentClass,
) -> [u8; 32] {
    let mut data = Vec::with_capacity(BINDING_PREFIX.len() + 1 + 16 + 8 + 16 + 32 + 1);
    data.extend_from_slice(BINDING_PREFIX);
    data.push(0x01);
    data.extend_from_slice(root_id);
    data.extend_from_slice(&claim.to_be_bytes());
    data.extend_from_slice(generation);
    data.extend_from_slice(content);
    data.push(class.byte());
    sha256(&data)
}

/// A claim gap's binding: no file.
pub fn bind_gap(root_id: &[u8; 16], claim: u64) -> [u8; 32] {
    let mut data = Vec::with_capacity(BINDING_PREFIX.len() + 1 + 16 + 8);
    data.extend_from_slice(BINDING_PREFIX);
    data.push(0x02);
    data.extend_from_slice(root_id);
    data.extend_from_slice(&claim.to_be_bytes());
    sha256(&data)
}

/// A pool file without a valid header: bound to its index and content.
pub fn bind_pool(root_id: &[u8; 16], index: u32, content: &[u8; 32]) -> [u8; 32] {
    let mut data = Vec::with_capacity(BINDING_PREFIX.len() + 1 + 16 + 4 + 32);
    data.extend_from_slice(BINDING_PREFIX);
    data.push(0x03);
    data.extend_from_slice(root_id);
    data.extend_from_slice(&index.to_be_bytes());
    data.extend_from_slice(content);
    sha256(&data)
}

/// The storage identity record's SHA-256: the prefix, then each of the
/// controller's `model`, `serial` and `firmware_rev` and the namespace's
/// `wwid`, as read (final newline and trailing spaces included), each after
/// its length as a big-endian `u16`. `None` if an attribute exceeds the read
/// bound.
pub fn storage_identity(attributes: [&[u8]; 4]) -> Option<[u8; 32]> {
    let mut record = Vec::with_capacity(STORAGE_IDENTITY_PREFIX.len() + 4 * (2 + 64));
    record.extend_from_slice(STORAGE_IDENTITY_PREFIX);
    for value in attributes {
        if value.len() > STORAGE_READ_LIMIT {
            return None;
        }
        record.extend_from_slice(&(value.len() as u16).to_be_bytes());
        record.extend_from_slice(value);
    }
    Some(sha256(&record))
}

// ---------------------------------------------------------------------------
// Text values (design section 7.5)
// ---------------------------------------------------------------------------

/// Why text did not parse. Plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

fn fail<T>(why: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(why.into()))
}

/// A canonical decimal: `0` or `[1-9][0-9]*`, at most 20 digits, in range.
pub fn decimal(value: &str, lo: u64, hi: u64) -> Result<u64, ParseError> {
    let bytes = value.as_bytes();
    let canonical = !bytes.is_empty()
        && bytes.len() <= 20
        && bytes.iter().all(u8::is_ascii_digit)
        && (bytes.len() == 1 || bytes[0] != b'0');
    if !canonical {
        return fail("non-canonical decimal");
    }
    let mut number: u64 = 0;
    for digit in bytes {
        number = match number
            .checked_mul(10)
            .and_then(|n| n.checked_add(u64::from(digit - b'0')))
        {
            Some(n) => n,
            None => return fail("decimal out of range"),
        };
    }
    if !(lo..=hi).contains(&number) {
        return fail("decimal out of range");
    }
    Ok(number)
}

/// Lowercase hex of exactly `N` bytes.
pub fn hex_exact<const N: usize>(value: &str) -> Result<[u8; N], ParseError> {
    let bytes = value.as_bytes();
    if bytes.len() != 2 * N
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return fail("hex");
    }
    let mut out = [0u8; N];
    for (k, pair) in bytes.chunks(2).enumerate() {
        let high = (pair[0] as char).to_digit(16).unwrap_or(0) as u8;
        let low = (pair[1] as char).to_digit(16).unwrap_or(0) as u8;
        out[k] = (high << 4) | low;
    }
    Ok(out)
}

/// Lowercase hex text of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}

fn digits(text: &[u8]) -> Option<u32> {
    if text.is_empty() || !text.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(
        text.iter()
            .fold(0u32, |acc, digit| acc * 10 + u32::from(digit - b'0')),
    )
}

fn date_time(year: &[u8], rest: [&[u8]; 5]) -> Result<(), ParseError> {
    let year = digits(year).ok_or_else(|| ParseError("bad timestamp".into()))?;
    let [month, day, hour, minute, second] = rest.map(|field| digits(field).unwrap_or(u32::MAX));
    if !(1970..=9999).contains(&year) {
        return fail("timestamp range");
    }
    if !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return fail("bad timestamp");
    }
    Ok(())
}

/// `YYYY-MM-DDTHH:MM:SSZ`: a valid Gregorian date and time, 1970 to 9999.
pub fn timestamp(value: &str) -> Result<&str, ParseError> {
    let b = value.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return fail("bad timestamp");
    }
    date_time(
        &b[0..4],
        [&b[5..7], &b[8..10], &b[11..13], &b[14..16], &b[17..19]],
    )?;
    Ok(value)
}

/// `YYYYMMDDTHHMMSSZ`, the compact form a revoked entry's name carries.
pub fn compact_timestamp(value: &str) -> Result<&str, ParseError> {
    let b = value.as_bytes();
    if b.len() != 16 || b[8] != b'T' || b[15] != b'Z' {
        return fail("bad timestamp");
    }
    date_time(
        &b[0..4],
        [&b[4..6], &b[6..8], &b[9..11], &b[11..13], &b[13..15]],
    )?;
    Ok(value)
}

/// An absolute path: `/`-separated components of `[A-Za-z0-9._-]`, 1 to 64
/// bytes each, neither `.` nor `..`, at most 255 bytes in all.
pub fn path(value: &str) -> Result<&str, ParseError> {
    if value.len() > 255 || !value.starts_with('/') || value.len() < 2 {
        return fail("bad path");
    }
    for component in value[1..].split('/') {
        let valid = (1..=64).contains(&component.len())
            && component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            && component != "."
            && component != "..";
        if !valid {
            return fail("bad path component");
        }
    }
    Ok(value)
}

/// The components of a valid absolute path.
pub fn path_components(value: &str) -> Vec<&str> {
    value[1..].split('/').collect()
}

/// An option string: printable ASCII without spaces, 1 to 1024 bytes.
pub fn option_string(value: &str) -> Result<&str, ParseError> {
    if !(1..=1024).contains(&value.len())
        || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return fail("option string");
    }
    Ok(value)
}

/// A statement: printable ASCII `0x20`-`0x7e`, 1 to `limit` bytes, with no
/// leading or trailing space.
pub fn statement(value: &str, limit: usize) -> Result<&str, ParseError> {
    if !(1..=limit).contains(&value.len())
        || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        || value.starts_with(' ')
        || value.ends_with(' ')
    {
        return fail("bad statement");
    }
    Ok(value)
}

/// A PCI function as Linux names it: `dddd:bb:dd.f` in lowercase hex, the
/// function 0 to 7.
pub fn pci_function(value: &str) -> Result<&str, ParseError> {
    let b = value.as_bytes();
    let hexdigit = |byte: &u8| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte);
    let valid = b.len() == 12
        && b[0..4].iter().all(hexdigit)
        && b[4] == b':'
        && b[5..7].iter().all(hexdigit)
        && b[7] == b':'
        && b[8..10].iter().all(hexdigit)
        && b[10] == b'.'
        && (b'0'..=b'7').contains(&b[11]);
    if !valid {
        return fail("PCI function");
    }
    Ok(value)
}

/// The lines of a text file: at most `limit` bytes, ASCII, LF line endings,
/// no CR or NUL, a final newline, and no empty line (every line must be a
/// field).
fn text_lines(data: &[u8], limit: usize) -> Result<(&str, Vec<&str>), ParseError> {
    if data.len() > limit {
        return fail("too large");
    }
    if !data.is_ascii() {
        return fail("not ASCII");
    }
    let text = std::str::from_utf8(data).map_err(|_| ParseError("not ASCII".into()))?;
    if !text.ends_with('\n') || text.contains('\r') || text.contains('\0') {
        return fail("line endings");
    }
    Ok((text, text[..text.len() - 1].split('\n').collect()))
}

/// The values of `keys`, in order, each exactly once: `key=value` lines.
fn fields<'a>(lines: &[&'a str], keys: &[&str]) -> Result<Vec<&'a str>, ParseError> {
    if lines.len() != keys.len() {
        return fail("field count");
    }
    let mut out = Vec::with_capacity(keys.len());
    for (line, key) in lines.iter().zip(keys) {
        match line
            .strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('='))
        {
            Some(value) => out.push(value),
            None => return fail(format!("expected {key}")),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// PROVISION (design section 7.5)
// ---------------------------------------------------------------------------

pub const PROVISION_VERSION_LINE: &str = "nexus-phase2-custody-provision 1";

const PROVISION_HEAD: [&str; 25] = [
    "uid",
    "gid",
    "root-id",
    "state-root",
    "root-inode",
    "lock-inode",
    "journals-inode",
    "dispositions-inode",
    "revoked-inode",
    "archive-inode",
    "mount-fstype",
    "mount-options",
    "super-options",
    "fs-block-size",
    "page-size",
    "device-logical-block-size",
    "device-physical-block-size",
    "kernel",
    "storage-class",
    "storage-pci-function",
    "storage-partition",
    "storage-identity",
    "storage-attestation",
    "pool",
    "pool-capacity",
];

const PROVISION_TAIL: [&str; 6] = [
    "retired-through",
    "predecessor",
    "predecessor-statement",
    "operator",
    "created",
    "revision",
];

/// The keys of `PROVISION`, in order, for `pool` pool files (the reference
/// list of design section 7.5).
pub fn provision_keys(pool: u32) -> Vec<String> {
    let mut keys: Vec<String> = PROVISION_HEAD.iter().map(|key| key.to_string()).collect();
    keys.extend((0..pool).map(|index| format!("pool-{index:05}-inode")));
    keys.extend(PROVISION_TAIL.iter().map(|key| key.to_string()));
    keys.push("digest".into());
    keys
}

/// The fields of `PROVISION`. As parsed, these are the Owner's claims: an
/// opening compares each with a fresh observation, and none of them admits
/// anything by being decoded (design section 10.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provision {
    pub uid: u32,
    pub gid: u32,
    pub root_id: [u8; 16],
    pub state_root: String,
    pub root_inode: u64,
    pub lock_inode: u64,
    pub journals_inode: u64,
    pub dispositions_inode: u64,
    pub revoked_inode: u64,
    pub archive_inode: u64,
    pub mount_options: String,
    pub super_options: String,
    pub device_logical_block_size: u32,
    pub device_physical_block_size: u32,
    pub kernel: String,
    pub storage_pci_function: String,
    pub storage_partition: Option<u32>,
    pub storage_identity: [u8; 32],
    pub storage_attestation: String,
    pub c_pool: u32,
    pub pool_inodes: Vec<u64>,
    pub retired_through: u64,
    pub predecessor: Option<([u8; 16], String)>,
    pub operator: String,
    pub created: String,
    pub revision: u64,
}

impl Provision {
    pub fn pool(&self) -> u32 {
        self.pool_inodes.len() as u32
    }

    /// The state root's parent path and its last component.
    pub fn state_root_split(&self) -> (&str, &str) {
        match self.state_root.rfind('/') {
            Some(0) => ("/", &self.state_root[1..]),
            Some(at) => (&self.state_root[..at], &self.state_root[at + 1..]),
            None => ("/", self.state_root.as_str()),
        }
    }
}

/// `PROVISION`'s exact bytes, its digest line last. Refused unless they would
/// parse back to the same fields.
pub fn render_provision(p: &Provision) -> Result<Vec<u8>, FormatError> {
    let partition = match p.storage_partition {
        None => "none".to_string(),
        Some(number) => number.to_string(),
    };
    let head: [String; 25] = [
        p.uid.to_string(),
        p.gid.to_string(),
        hex(&p.root_id),
        p.state_root.clone(),
        p.root_inode.to_string(),
        p.lock_inode.to_string(),
        p.journals_inode.to_string(),
        p.dispositions_inode.to_string(),
        p.revoked_inode.to_string(),
        p.archive_inode.to_string(),
        "ext4".into(),
        p.mount_options.clone(),
        p.super_options.clone(),
        "4096".into(),
        "4096".into(),
        p.device_logical_block_size.to_string(),
        p.device_physical_block_size.to_string(),
        p.kernel.clone(),
        STORAGE_CLASS.into(),
        p.storage_pci_function.clone(),
        partition,
        hex(&p.storage_identity),
        p.storage_attestation.clone(),
        p.pool_inodes.len().to_string(),
        p.c_pool.to_string(),
    ];
    let mut body = String::new();
    body.push_str(PROVISION_VERSION_LINE);
    body.push('\n');
    for (key, value) in PROVISION_HEAD.iter().zip(head.iter()) {
        body.push_str(&format!("{key}={value}\n"));
    }
    for (index, inode) in p.pool_inodes.iter().enumerate() {
        body.push_str(&format!("pool-{index:05}-inode={inode}\n"));
    }
    let (predecessor, statement) = match &p.predecessor {
        None => ("none".to_string(), "none".to_string()),
        Some((id, statement)) => (hex(id), statement.clone()),
    };
    let tail: [String; 6] = [
        p.retired_through.to_string(),
        predecessor,
        statement,
        p.operator.clone(),
        p.created.clone(),
        p.revision.to_string(),
    ];
    for (key, value) in PROVISION_TAIL.iter().zip(tail.iter()) {
        body.push_str(&format!("{key}={value}\n"));
    }
    let digest = hex(&sha256(body.as_bytes()));
    body.push_str(&format!("digest={digest}\n"));
    let bytes = body.into_bytes();
    match parse_provision(&bytes) {
        Ok(parsed) if parsed == *p => Ok(bytes),
        _ => Err(FormatError("PROVISION would not parse back to its fields")),
    }
}

/// Parse `PROVISION` exactly: every key in order, once; canonical values; the
/// digest of every preceding byte. A `PROVISION` without the storage fields,
/// or with another storage class, is invalid.
pub fn parse_provision(data: &[u8]) -> Result<Provision, ParseError> {
    let (text, lines) = text_lines(data, PROVISION_LIMIT)?;
    if lines.first() != Some(&PROVISION_VERSION_LINE) {
        return fail("version line");
    }
    let head_end = 1 + PROVISION_HEAD.len();
    if lines.len() < head_end {
        return fail("field count");
    }
    let head = fields(&lines[1..head_end], &PROVISION_HEAD)?;
    let pool = decimal(head[23], 1, u64::from(MAX_POOL))? as usize;
    let c_pool = decimal(head[24], 1, u64::from(MAX_C_POOL))? as u32;
    let pool_keys: Vec<String> = (0..pool)
        .map(|index| format!("pool-{index:05}-inode"))
        .collect();
    let pool_key_refs: Vec<&str> = pool_keys.iter().map(String::as_str).collect();
    let pools_end = head_end + pool;
    let tail_end = pools_end + PROVISION_TAIL.len();
    if lines.len() != tail_end + 1 {
        return fail("field count");
    }
    let pools = fields(&lines[head_end..pools_end], &pool_key_refs)?;
    let tail = fields(&lines[pools_end..tail_end], &PROVISION_TAIL)?;
    let digest_line = lines[tail_end];
    let Some(digest_text) = digest_line.strip_prefix("digest=") else {
        return fail("digest line");
    };
    let body_len = text.len() - digest_line.len() - 1;
    if hex_exact::<32>(digest_text)? != sha256(&data[..body_len]) {
        return fail("digest mismatch");
    }
    let root_id = hex_exact::<16>(head[2]).map_err(|_| ParseError("root id".into()))?;
    let state_root = path(head[3])?.to_string();
    if head[10] != "ext4" || head[13] != "4096" || head[14] != "4096" {
        return fail("filesystem profile");
    }
    let mount_options = option_string(head[11])?.to_string();
    let super_options = option_string(head[12])?.to_string();
    let logical = decimal(head[15], 1, 4096)? as u32;
    let physical = decimal(head[16], 1, 4096)? as u32;
    if 4096 % logical != 0 || 4096 % physical != 0 {
        return fail("device block size");
    }
    let kernel = statement(head[17], 512)?.to_string();
    if head[18] != STORAGE_CLASS {
        return fail("storage class");
    }
    let storage_pci_function = pci_function(head[19])?.to_string();
    let storage_partition = match head[20] {
        "none" => None,
        value => Some(decimal(value, 1, 255)? as u32),
    };
    let storage_identity =
        hex_exact::<32>(head[21]).map_err(|_| ParseError("storage identity".into()))?;
    let storage_attestation = statement(head[22], 512)?.to_string();
    let mut pool_inodes = Vec::with_capacity(pool);
    for value in pools {
        pool_inodes.push(decimal(value, 0, u64::MAX)?);
    }
    let retired_through = decimal(tail[0], 0, MAX_CLAIM)?;
    let predecessor = match (tail[1], tail[2]) {
        ("none", "none") => None,
        ("none", _) => return fail("predecessor statement without predecessor"),
        (id, text) => Some((
            hex_exact::<16>(id).map_err(|_| ParseError("predecessor".into()))?,
            statement(text, 512)?.to_string(),
        )),
    };
    if predecessor.as_ref().is_some_and(|(_, text)| text == "none") {
        return fail("predecessor without statement");
    }
    let operator = statement(tail[3], 64)?.to_string();
    let created = timestamp(tail[4])?.to_string();
    let revision = decimal(tail[5], 1, u64::MAX)?;
    Ok(Provision {
        uid: decimal(head[0], 0, u64::from(u32::MAX))? as u32,
        gid: decimal(head[1], 0, u64::from(u32::MAX))? as u32,
        root_id,
        state_root,
        root_inode: decimal(head[4], 0, u64::MAX)?,
        lock_inode: decimal(head[5], 0, u64::MAX)?,
        journals_inode: decimal(head[6], 0, u64::MAX)?,
        dispositions_inode: decimal(head[7], 0, u64::MAX)?,
        revoked_inode: decimal(head[8], 0, u64::MAX)?,
        archive_inode: decimal(head[9], 0, u64::MAX)?,
        mount_options,
        super_options,
        device_logical_block_size: logical,
        device_physical_block_size: physical,
        kernel,
        storage_pci_function,
        storage_partition,
        storage_identity,
        storage_attestation,
        c_pool,
        pool_inodes,
        retired_through,
        predecessor,
        operator,
        created,
        revision,
    })
}

// ---------------------------------------------------------------------------
// Dispositions (design sections 7.5 and 13.4)
// ---------------------------------------------------------------------------

pub const DISPOSITION_VERSION_LINE: &str = "nexus-phase2-custody-disposition 1";

const DISPOSITION_KEYS: [&str; 13] = [
    "root",
    "binding",
    "kind",
    "claim",
    "generation",
    "pool-index",
    "content",
    "class",
    "recorded-unsettled",
    "reason",
    "statement",
    "operator",
    "at",
];

/// The kind of an incident (and of the disposition bound to it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IncidentKind {
    Journal,
    ClaimGap,
    PoolFile,
}

impl IncidentKind {
    pub fn text(self) -> &'static str {
        match self {
            IncidentKind::Journal => "journal",
            IncidentKind::ClaimGap => "claim-gap",
            IncidentKind::PoolFile => "pool-file",
        }
    }
}

/// One incident's identity, as the store computes it and as a disposition
/// restates it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IncidentFacts {
    pub kind: IncidentKind,
    pub claim: Option<u64>,
    pub generation: Option<[u8; 16]>,
    pub pool_index: Option<u32>,
    pub content: Option<[u8; 32]>,
    pub class: IncidentClass,
    pub recorded_unsettled: u64,
}

impl IncidentFacts {
    /// The binding of these facts under `root_id`, or `None` if the facts
    /// are not of their kind's shape.
    pub fn binding(&self, root_id: &[u8; 16]) -> Option<[u8; 32]> {
        match (
            self.kind,
            self.claim,
            self.generation,
            self.pool_index,
            self.content,
        ) {
            (IncidentKind::Journal, Some(claim), Some(generation), None, Some(content)) => Some(
                bind_journal(root_id, claim, &generation, &content, self.class),
            ),
            (IncidentKind::ClaimGap, Some(claim), None, None, None)
                if self.class == IncidentClass::Malformed && self.recorded_unsettled == 0 =>
            {
                Some(bind_gap(root_id, claim))
            }
            (IncidentKind::PoolFile, None, None, Some(index), Some(content))
                if self.class == IncidentClass::Malformed && self.recorded_unsettled == 0 =>
            {
                Some(bind_pool(root_id, index, &content))
            }
            _ => None,
        }
    }
}

/// A disposition file's fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disposition {
    pub root: [u8; 16],
    pub binding: [u8; 32],
    pub facts: IncidentFacts,
    pub reason: DispositionReason,
    pub statement: String,
    pub operator: String,
    pub at: String,
}

/// A disposition file's exact bytes. Refused unless they parse back to the
/// same fields (the binding must recompute).
pub fn render_disposition(d: &Disposition) -> Result<Vec<u8>, FormatError> {
    let none = || "none".to_string();
    let f = &d.facts;
    let values: [String; 13] = [
        hex(&d.root),
        hex(&d.binding),
        f.kind.text().into(),
        f.claim.map_or_else(none, |claim| claim.to_string()),
        f.generation
            .map_or_else(none, |generation| hex(&generation)),
        f.pool_index.map_or_else(none, |index| index.to_string()),
        f.content.map_or_else(none, |content| hex(&content)),
        f.class.text().into(),
        f.recorded_unsettled.to_string(),
        reason_text(d.reason).into(),
        d.statement.clone(),
        d.operator.clone(),
        d.at.clone(),
    ];
    let mut text = String::new();
    text.push_str(DISPOSITION_VERSION_LINE);
    text.push('\n');
    for (key, value) in DISPOSITION_KEYS.iter().zip(values.iter()) {
        text.push_str(&format!("{key}={value}\n"));
    }
    let bytes = text.into_bytes();
    match parse_disposition(&bytes) {
        Ok(parsed) if parsed == *d => Ok(bytes),
        _ => Err(FormatError(
            "disposition would not parse back to its fields",
        )),
    }
}

/// Parse a disposition exactly; its binding must recompute from its fields.
pub fn parse_disposition(data: &[u8]) -> Result<Disposition, ParseError> {
    let (_, lines) = text_lines(data, DISPOSITION_LIMIT)?;
    if lines.first() != Some(&DISPOSITION_VERSION_LINE) {
        return fail("version line");
    }
    let f = fields(&lines[1..], &DISPOSITION_KEYS)?;
    let root = hex_exact::<16>(f[0]).map_err(|_| ParseError("root".into()))?;
    let binding = hex_exact::<32>(f[1]).map_err(|_| ParseError("binding".into()))?;
    let kind = match f[2] {
        "journal" => IncidentKind::Journal,
        "claim-gap" => IncidentKind::ClaimGap,
        "pool-file" => IncidentKind::PoolFile,
        _ => return fail("kind"),
    };
    let none = |key: &str, value: &str| -> Result<(), ParseError> {
        if value == "none" {
            Ok(())
        } else {
            fail(format!("{key} must be none"))
        }
    };
    let (claim, generation, pool_index, content) = match kind {
        IncidentKind::Journal => {
            none("pool-index", f[5])?;
            (
                Some(decimal(f[3], 1, MAX_CLAIM)?),
                Some(hex_exact::<16>(f[4]).map_err(|_| ParseError("generation".into()))?),
                None,
                Some(hex_exact::<32>(f[6]).map_err(|_| ParseError("content".into()))?),
            )
        }
        IncidentKind::ClaimGap => {
            none("generation", f[4])?;
            none("pool-index", f[5])?;
            none("content", f[6])?;
            (Some(decimal(f[3], 1, MAX_CLAIM)?), None, None, None)
        }
        IncidentKind::PoolFile => {
            none("claim", f[3])?;
            none("generation", f[4])?;
            (
                None,
                None,
                Some(decimal(f[5], 0, u64::from(MAX_POOL) - 1)? as u32),
                Some(hex_exact::<32>(f[6]).map_err(|_| ParseError("content".into()))?),
            )
        }
    };
    let class = match f[7] {
        "unresolved" if kind == IncidentKind::Journal => IncidentClass::Unresolved,
        "malformed" => IncidentClass::Malformed,
        _ => return fail("class"),
    };
    let recorded_unsettled = decimal(f[8], 0, u64::MAX)?;
    if kind != IncidentKind::Journal && recorded_unsettled != 0 {
        return fail("recorded-unsettled");
    }
    let reason = reason_of_text(f[9]).ok_or_else(|| ParseError("reason".into()))?;
    let statement_text = statement(f[10], 512)?.to_string();
    let operator = statement(f[11], 64)?.to_string();
    let at = timestamp(f[12])?.to_string();
    let facts = IncidentFacts {
        kind,
        claim,
        generation,
        pool_index,
        content,
        class,
        recorded_unsettled,
    };
    if facts.binding(&root) != Some(binding) {
        return fail("binding does not recompute");
    }
    Ok(Disposition {
        root,
        binding,
        facts,
        reason,
        statement: statement_text,
        operator,
        at,
    })
}

// ---------------------------------------------------------------------------
// Entry names (design section 7.5)
// ---------------------------------------------------------------------------

/// The entries of `<STATE_ROOT>/`.
pub const ROOT_ENTRIES: [&str; 4] = ["LOCK", "journals", "dispositions", "archive"];
/// The temporary-name prefix of every interrupted procedure in a store
/// directory.
pub const TMP_PREFIX: &str = ".tmp-";

/// A pool file's name: `j<5-digit index>.journal`.
pub fn pool_name(index: u32) -> String {
    format!("j{index:05}.journal")
}

/// The pool index a name gives, for indices below `pool`.
pub fn parse_pool_name(name: &str, pool: u32) -> Option<u32> {
    let digits_text = name.strip_prefix('j')?.strip_suffix(".journal")?;
    if digits_text.len() != 5 || !digits_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let index: u32 = digits_text.parse().ok()?;
    (index < pool).then_some(index)
}

/// A disposition's name: `<64 hex>.disposition`.
pub fn disposition_name(binding: &[u8; 32]) -> String {
    format!("{}.disposition", hex(binding))
}

/// The binding a disposition's name gives.
pub fn parse_disposition_name(name: &str) -> Option<[u8; 32]> {
    hex_exact::<32>(name.strip_suffix(".disposition")?).ok()
}

/// A revoked disposition's name: `<64 hex>-<YYYYMMDDTHHMMSSZ>.disposition`.
pub fn revoked_name(binding: &[u8; 32], compact: &str) -> String {
    format!("{}-{compact}.disposition", hex(binding))
}

/// The binding and time a revoked entry's name gives.
pub fn parse_revoked_name(name: &str) -> Option<([u8; 32], String)> {
    let stem = name.strip_suffix(".disposition")?;
    let (binding, time) = stem.split_once('-')?;
    let binding = hex_exact::<32>(binding).ok()?;
    compact_timestamp(time).ok()?;
    Some((binding, time.to_string()))
}

/// An archive entry's class letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveClass {
    Sealed,
    NoNativeWork,
    Unresolved,
    Malformed,
}

impl ArchiveClass {
    pub fn letter(self) -> char {
        match self {
            ArchiveClass::Sealed => 's',
            ArchiveClass::NoNativeWork => 'n',
            ArchiveClass::Unresolved => 'u',
            ArchiveClass::Malformed => 'm',
        }
    }

    fn of_letter(letter: &str) -> Option<ArchiveClass> {
        match letter {
            "s" => Some(ArchiveClass::Sealed),
            "n" => Some(ArchiveClass::NoNativeWork),
            "u" => Some(ArchiveClass::Unresolved),
            "m" => Some(ArchiveClass::Malformed),
            _ => None,
        }
    }
}

/// What an archive entry's name says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveName {
    /// `j-<claim>-<32 hex>-<64 hex>-<class>.journal`: a pool journal with a
    /// valid header.
    Journal {
        claim: u64,
        generation: [u8; 16],
        content: [u8; 32],
        class: ArchiveClass,
    },
    /// `p-<5-digit index>-<64 hex>.journal`: a pool file without a valid
    /// header.
    PoolFile { index: u32, content: [u8; 32] },
}

impl ArchiveName {
    pub fn render(&self) -> String {
        match self {
            ArchiveName::Journal {
                claim,
                generation,
                content,
                class,
            } => format!(
                "j-{claim}-{}-{}-{}.journal",
                hex(generation),
                hex(content),
                class.letter()
            ),
            ArchiveName::PoolFile { index, content } => {
                format!("p-{index:05}-{}.journal", hex(content))
            }
        }
    }

    pub fn parse(name: &str) -> Option<ArchiveName> {
        let stem = name.strip_suffix(".journal")?;
        if let Some(rest) = stem.strip_prefix("j-") {
            let parts: Vec<&str> = rest.split('-').collect();
            if parts.len() != 4 {
                return None;
            }
            let claim = decimal(parts[0], 1, MAX_CLAIM).ok()?;
            return Some(ArchiveName::Journal {
                claim,
                generation: hex_exact::<16>(parts[1]).ok()?,
                content: hex_exact::<32>(parts[2]).ok()?,
                class: ArchiveClass::of_letter(parts[3])?,
            });
        }
        let rest = stem.strip_prefix("p-")?;
        let (index, content) = rest.split_once('-')?;
        if index.len() != 5 || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let index: u32 = index.parse().ok()?;
        if index >= MAX_POOL {
            return None;
        }
        Some(ArchiveName::PoolFile {
            index,
            content: hex_exact::<32>(content).ok()?,
        })
    }
}
