//! PROVISIONAL. A test-only SQLite storage shell for the ledger's protocol
//! core, written against the unadopted draft P3-C11-D2A-FULL-CONTRACT-DRAFT-01
//! (mission P3-C11-48H-OPUS55-HIGH-CONTINUOUS-01). No contract for it has
//! been adopted or accepted. It is compiled only in test builds: its `mod`
//! line carries `#[cfg(test)]` and SQLite is a dev-dependency of this crate,
//! so no production build contains it and nothing in production can reach
//! it.
//!
//! It supplies the verbs of the core's traits one at a time, with the
//! meanings section 4.1 of CONTRACT-02-R1 gives them, and decides nothing the
//! core decides. What it adds is what the core leaves to the full contract
//! (CONTRACT-02-R1 section 13): each verb's SQL, the schema, the records'
//! builders and encodings, row verification, the barrier, the session's
//! opening and closing records, and the wait to enter the store. Storage
//! assumptions A-1 and A-2 are exercised on a real file for a process crash
//! (`crash_tests`); they are not discharged, and nothing here speaks to a
//! power loss.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use rusqlite::{params, Connection, ErrorCode, OpenFlags};
use sha2::{Digest, Sha256};

use super::protocol::*;

mod composition_tests;
mod concurrency_tests;
mod crash_tests;
mod fault_tests;
mod schema_tests;
mod session_tests;
mod verb_tests;

// ---- the schema ------------------------------------------------------------------------------

/// The schema's version, kept in `meta`.
const SCHEMA_VERSION: &[u8] = b"nexus.ledger.schema.v1";

/// The schema, statement by statement, exactly as `sqlite_master` keeps it.
/// `open` refuses a file whose schema differs in any byte. No constraint is
/// deferred and none can fail at COMMIT (section 4.4; `schema_tests`).
const SCHEMA: [&str; 8] = [
    "CREATE TABLE meta (k TEXT PRIMARY KEY NOT NULL, v BLOB NOT NULL) STRICT",
    "CREATE TABLE ledger (\
seq INTEGER PRIMARY KEY NOT NULL CHECK (seq > 0), \
family INTEGER NOT NULL CHECK (family BETWEEN 1 AND 7), \
id BLOB NOT NULL UNIQUE CHECK (length(id) = 32), \
digest BLOB NOT NULL CHECK (length(digest) = 32), \
commitment BLOB CHECK (commitment IS NULL OR length(commitment) = 32), \
role INTEGER CHECK (role IS NULL OR role BETWEEN 0 AND 2), \
about BLOB CHECK (about IS NULL OR length(about) = 64), \
acked INTEGER CHECK (acked IS NULL OR acked >= 0), \
needs_ack INTEGER NOT NULL CHECK (needs_ack IN (0, 1)), \
body BLOB NOT NULL, \
prev BLOB NOT NULL CHECK (length(prev) = 32), \
tag BLOB NOT NULL CHECK (length(tag) = 32), \
CHECK ((commitment IS NULL) = (role IS NULL))) STRICT",
    "CREATE UNIQUE INDEX ledger_role ON ledger (commitment, role) WHERE commitment IS NOT NULL",
    "CREATE INDEX ledger_acked ON ledger (seq) WHERE acked IS NOT NULL",
    "CREATE TRIGGER ledger_no_update BEFORE UPDATE ON ledger \
BEGIN SELECT RAISE(ABORT, 'the ledger is append-only'); END",
    "CREATE TRIGGER ledger_no_delete BEFORE DELETE ON ledger \
BEGIN SELECT RAISE(ABORT, 'the ledger is append-only'); END",
    "CREATE TRIGGER meta_no_update BEFORE UPDATE ON meta \
BEGIN SELECT RAISE(ABORT, 'meta is fixed'); END",
    "CREATE TRIGGER meta_no_delete BEFORE DELETE ON meta \
BEGIN SELECT RAISE(ABORT, 'meta is fixed'); END",
];

/// Every column of a ledger row, in the order `Stored` reads them.
const COLUMNS: &str =
    "seq, family, id, digest, commitment, role, about, acked, needs_ack, body, prev, tag";

// ---- families and their encodings --------------------------------------------------------------

/// What a row is. Each entry point writes one fixed family or builds its
/// record itself (section 5.2); no verb takes a family from a caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    Fact,
    Gating,
    Correction,
    Explanation,
    Acknowledgement,
    Opened,
    Closed,
}

impl Family {
    fn code(self) -> u8 {
        match self {
            Family::Fact => 1,
            Family::Gating => 2,
            Family::Correction => 3,
            Family::Explanation => 4,
            Family::Acknowledgement => 5,
            Family::Opened => 6,
            Family::Closed => 7,
        }
    }

    fn from_code(code: i64) -> Option<Family> {
        Some(match code {
            1 => Family::Fact,
            2 => Family::Gating,
            3 => Family::Correction,
            4 => Family::Explanation,
            5 => Family::Acknowledgement,
            6 => Family::Opened,
            7 => Family::Closed,
            _ => return None,
        })
    }
}

fn role_code(role: Role) -> u8 {
    match role {
        Role::Prepared => 0,
        Role::Started => 1,
        Role::End => 2,
    }
}

fn role_from(code: u8) -> Option<Role> {
    Some(match code {
        0 => Role::Prepared,
        1 => Role::Started,
        2 => Role::End,
        _ => return None,
    })
}

fn exit_code(exit: Exit) -> u8 {
    match exit {
        Exit::InCall => 0,
        Exit::ReturnedUncertain => 1,
        Exit::Unwound => 2,
    }
}

/// A canonical, length-prefixed encoding. Every variable part carries its
/// length, so two different values never encode alike.
#[derive(Default)]
struct Encoder(Vec<u8>);

impl Encoder {
    fn bytes(mut self, bytes: &[u8]) -> Encoder {
        let len = u32::try_from(bytes.len()).expect("a part fits in u32");
        self.0.extend_from_slice(&len.to_be_bytes());
        self.0.extend_from_slice(bytes);
        self
    }

    fn u64(mut self, value: u64) -> Encoder {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn u8(mut self, value: u8) -> Encoder {
        self.0.push(value);
        self
    }

    fn key(self, key: &RecordKey) -> Encoder {
        self.bytes(&key.id).bytes(&key.digest)
    }

    fn part(self, part: Option<Part>) -> Encoder {
        match part {
            None => self.u8(0),
            Some(part) => self.u8(1).bytes(&part.commitment).u8(role_code(part.role)),
        }
    }

    fn set(self, set: &SetFacts) -> Encoder {
        self.u64(set.frontier)
            .u64(set.head)
            .u64(set.top)
            .u64(set.count)
            .bytes(&set.digest)
    }

    fn done(self) -> Vec<u8> {
        self.0
    }
}

/// Reads what `Encoder` wrote. Any shortfall or excess is `None`.
struct Decoder<'a>(&'a [u8]);

impl<'a> Decoder<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn bytes(&mut self) -> Option<&'a [u8]> {
        let len = u32::from_be_bytes(self.take(4)?.try_into().ok()?);
        self.take(usize::try_from(len).ok()?)
    }

    fn array(&mut self) -> Option<[u8; 32]> {
        self.bytes()?.try_into().ok()
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_be_bytes(self.take(8)?.try_into().ok()?))
    }

    fn u8(&mut self) -> Option<u8> {
        Some(*self.take(1)?.first()?)
    }

    fn key(&mut self) -> Option<RecordKey> {
        Some(RecordKey {
            id: self.array()?,
            digest: self.array()?,
        })
    }

    fn part(&mut self) -> Option<Option<Part>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(Part {
                commitment: self.array()?,
                role: role_from(self.u8()?)?,
            })),
            _ => None,
        }
    }

    fn set(&mut self) -> Option<SetFacts> {
        Some(SetFacts {
            frontier: self.u64()?,
            head: self.u64()?,
            top: self.u64()?,
            count: self.u64()?,
            digest: self.array()?,
        })
    }

    fn end(&self) -> Option<()> {
        self.0.is_empty().then_some(())
    }
}

// ---- hashing -----------------------------------------------------------------------------------

const ID_DOMAIN: &str = "nexus.ledger.id.v1";
const DIGEST_DOMAIN: &str = "nexus.ledger.digest.v1";
const TAG_DOMAIN: &str = "nexus.ledger.row.v1";
const SET_DOMAIN: &str = "nexus.ledger.set.v1";

/// SHA-256 over a domain and length-prefixed parts.
fn hash(domain: &str, parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn part_bytes(part: Option<Part>) -> Vec<u8> {
    Encoder::default().part(part).done()
}

/// A record's identity: its family and the identity material its builder
/// was given, under this store's identity.
fn record_id(store: &[u8; 32], family: Family, material: &[u8]) -> [u8; 32] {
    hash(ID_DOMAIN, &[store, &[family.code()], material])
}

/// A record's digest: everything a row says about its record.
fn record_digest(
    store: &[u8; 32],
    family: Family,
    id: &[u8; 32],
    part: Option<Part>,
    body: &[u8],
) -> [u8; 32] {
    hash(
        DIGEST_DOMAIN,
        &[store, &[family.code()], id, &part_bytes(part), body],
    )
}

// ---- rows --------------------------------------------------------------------------------------

/// One ledger row, as read and verified.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored {
    seq: Seq,
    family: Family,
    id: [u8; 32],
    digest: [u8; 32],
    part: Option<Part>,
    about: Option<RecordKey>,
    acked: Option<Seq>,
    needs_ack: bool,
    body: Vec<u8>,
    prev: [u8; 32],
    tag: [u8; 32],
}

/// A record ready to be written: everything of its row but the sequence,
/// the chain and the tag.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Draft {
    family: Family,
    id: [u8; 32],
    digest: [u8; 32],
    part: Option<Part>,
    about: Option<RecordKey>,
    acked: Option<Seq>,
    needs_ack: bool,
    body: Vec<u8>,
}

impl Draft {
    fn key(&self) -> RecordKey {
        RecordKey {
            id: self.id,
            digest: self.digest,
        }
    }
}

/// A row's tag: the store's identity, its sequence, every column, and the
/// tag of the row before it. A row copied from another store, edited, moved
/// or cut from the chain fails it.
fn row_tag(store: &[u8; 32], seq: Seq, draft: &Draft, prev: &[u8; 32]) -> [u8; 32] {
    let about = draft
        .about
        .map(|key| Encoder::default().key(&key).done())
        .unwrap_or_default();
    let acked = draft
        .acked
        .map(|n| n.to_be_bytes().to_vec())
        .unwrap_or_default();
    hash(
        TAG_DOMAIN,
        &[
            store,
            &seq.to_be_bytes(),
            &[draft.family.code()],
            &draft.id,
            &draft.digest,
            &part_bytes(draft.part),
            &about,
            &acked,
            &[u8::from(draft.needs_ack)],
            &draft.body,
            prev,
        ],
    )
}

impl Stored {
    fn draft(&self) -> Draft {
        Draft {
            family: self.family,
            id: self.id,
            digest: self.digest,
            part: self.part,
            about: self.about,
            acked: self.acked,
            needs_ack: self.needs_ack,
            body: self.body.clone(),
        }
    }

    fn row(&self) -> Row {
        Row {
            seq: self.seq,
            digest: self.digest,
        }
    }

    /// Whether this row is what this store wrote: its tag, its digest, and
    /// its family's own rules about its columns and its body.
    fn verified(&self, store: &[u8; 32]) -> bool {
        let draft = self.draft();
        row_tag(store, self.seq, &draft, &self.prev) == self.tag
            && record_digest(store, self.family, &self.id, self.part, &self.body) == self.digest
            && family_rules(store, &draft)
    }
}

/// The columns and body each family must have. A cell record's identity is
/// derived from its content, so it is recomputed here too.
fn family_rules(store: &[u8; 32], draft: &Draft) -> bool {
    let plain = draft.about.is_none() && draft.acked.is_none() && !draft.needs_ack;
    match draft.family {
        Family::Fact | Family::Gating => plain,
        Family::Opened | Family::Closed => {
            plain
                && draft.part.is_none()
                && draft.body.len() == 16
                && draft.id == record_id(store, draft.family, &draft.body)
        }
        Family::Correction => {
            let Some(about) = draft.about else {
                return false;
            };
            let mut d = Decoder(&draft.body);
            let decoded = (|| {
                let key = d.key()?;
                let _seq = d.u64()?;
                let exit = d.u8()?;
                d.end()?;
                Some((key, exit))
            })();
            match decoded {
                Some((key, exit)) => {
                    key == about
                        && (exit == exit_code(Exit::ReturnedUncertain)
                            || exit == exit_code(Exit::Unwound))
                        && draft.part.is_none()
                        && draft.acked.is_none()
                        && !draft.needs_ack
                        && draft.id == correction_id(store, &about, exit)
                }
                None => false,
            }
        }
        Family::Explanation => {
            let Some(about) = draft.about else {
                return false;
            };
            let mut d = Decoder(&draft.body);
            let original = d.key();
            original == Some(about)
                && draft.part.is_none()
                && draft.acked.is_none()
                && draft.needs_ack
                && draft.id == explanation_id(store, &about)
                && decode_cause(&mut d).is_some()
                && d.end().is_some()
        }
        Family::Acknowledgement => {
            let mut d = Decoder(&draft.body);
            let decoded = (|| {
                let set = d.set()?;
                let nonce: [u8; 16] = d.take(16)?.try_into().ok()?;
                d.end()?;
                Some((set, nonce))
            })();
            match decoded {
                Some((set, nonce)) => {
                    draft.acked == Some(set.top)
                        && draft.about.is_none()
                        && draft.part.is_none()
                        && !draft.needs_ack
                        && draft.id == ack_id(store, &set, &nonce)
                }
                None => false,
            }
        }
    }
}

fn correction_id(store: &[u8; 32], about: &RecordKey, exit: u8) -> [u8; 32] {
    let material = Encoder::default().key(about).u8(exit).done();
    record_id(store, Family::Correction, &material)
}

fn explanation_id(store: &[u8; 32], original: &RecordKey) -> [u8; 32] {
    let material = Encoder::default().key(original).done();
    record_id(store, Family::Explanation, &material)
}

fn ack_id(store: &[u8; 32], set: &SetFacts, nonce: &[u8; 16]) -> [u8; 32] {
    let material = Encoder::default().set(set).bytes(nonce).done();
    record_id(store, Family::Acknowledgement, &material)
}

fn encode_cause(encoder: Encoder, kind: CauseKind) -> Encoder {
    match kind {
        CauseKind::IdentityContent {
            id,
            existing,
            offered,
        } => encoder
            .u8(1)
            .bytes(&id)
            .u64(existing.seq)
            .bytes(&existing.digest)
            .bytes(&offered),
        CauseKind::DuplicateRole {
            part,
            holder,
            offered,
        } => encoder
            .u8(2)
            .part(Some(part))
            .bytes(&holder.id)
            .u64(holder.seq)
            .key(&offered),
        CauseKind::MissingPrepared { part, offered } => {
            encoder.u8(3).part(Some(part)).key(&offered)
        }
    }
}

/// Checks that a cause decodes; the core alone builds causes, so the shell
/// never makes one back.
fn decode_cause(d: &mut Decoder<'_>) -> Option<()> {
    match d.u8()? {
        1 => {
            d.array()?;
            d.u64()?;
            d.array()?;
            d.array()?;
        }
        2 => {
            d.part()??;
            d.array()?;
            d.u64()?;
            d.key()?;
        }
        3 => {
            d.part()??;
            d.key()?;
        }
        _ => return None,
    }
    Some(())
}

/// The record a cell occupant stands for, built by the ledger (section 5.1).
/// Only corrections and explanations are cell records.
fn cell_draft(store: &[u8; 32], occupant: &Occupant) -> Option<Draft> {
    match occupant {
        Occupant::Correction { about, seq, exit } => {
            let exit = exit_code(*exit);
            if exit == exit_code(Exit::InCall) {
                return None;
            }
            let body = Encoder::default().key(about).u64(*seq).u8(exit).done();
            let id = correction_id(store, about, exit);
            Some(Draft {
                family: Family::Correction,
                id,
                digest: record_digest(store, Family::Correction, &id, None, &body),
                part: None,
                about: Some(*about),
                acked: None,
                needs_ack: false,
                body,
            })
        }
        Occupant::Explanation { original, cause } => {
            let body = encode_cause(Encoder::default().key(original), cause.kind()).done();
            let id = explanation_id(store, original);
            Some(Draft {
                family: Family::Explanation,
                id,
                digest: record_digest(store, Family::Explanation, &id, None, &body),
                part: None,
                about: Some(*original),
                acked: None,
                needs_ack: true,
                body,
            })
        }
        Occupant::Gating { .. } | Occupant::Acknowledgement { .. } => None,
    }
}

fn ack_draft(store: &[u8; 32], set: &SetFacts, nonce: &[u8; 16]) -> Draft {
    let body = Encoder::default().set(set).done();
    let mut body = body;
    body.extend_from_slice(nonce);
    let id = ack_id(store, set, nonce);
    Draft {
        family: Family::Acknowledgement,
        id,
        digest: record_digest(store, Family::Acknowledgement, &id, None, &body),
        part: None,
        about: None,
        acked: Some(set.top),
        needs_ack: false,
        body,
    }
}

// ---- reading and writing rows ------------------------------------------------------------------

/// A failed read of a corrupt file is `Corrupt`; any other failure `Failed`.
fn read_error<T>(error: &rusqlite::Error) -> Read<T> {
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => Read::Corrupt,
        _ => Read::Failed,
    }
}

type Raw = (
    i64,
    i64,
    Vec<u8>,
    Vec<u8>,
    Option<Vec<u8>>,
    Option<i64>,
    Option<Vec<u8>>,
    Option<i64>,
    i64,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
);

fn raw(row: &rusqlite::Row<'_>) -> rusqlite::Result<Raw> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
    ))
}

/// A raw row as `Stored`, if every column has its shape. It is not yet
/// verified.
fn stored(raw: Raw) -> Option<Stored> {
    let (seq, family, id, digest, commitment, role, about, acked, needs_ack, body, prev, tag) = raw;
    let part = match (commitment, role) {
        (None, None) => None,
        (Some(commitment), Some(role)) => Some(Part {
            commitment: commitment.try_into().ok()?,
            role: role_from(u8::try_from(role).ok()?)?,
        }),
        _ => return None,
    };
    let about = match about {
        None => None,
        Some(about) => {
            let (id, digest) = about.split_at_checked(32)?;
            Some(RecordKey {
                id: id.try_into().ok()?,
                digest: digest.try_into().ok()?,
            })
        }
    };
    Some(Stored {
        seq: Seq::try_from(seq).ok()?,
        family: Family::from_code(family)?,
        id: id.try_into().ok()?,
        digest: digest.try_into().ok()?,
        part,
        about,
        acked: acked.map(Seq::try_from).transpose().ok()?,
        needs_ack: match needs_ack {
            0 => false,
            1 => true,
            _ => return None,
        },
        body,
        prev: prev.try_into().ok()?,
        tag: tag.try_into().ok()?,
    })
}

/// Reads rows by a query and verifies each. Any row that fails is `Corrupt`.
fn select(
    conn: &Connection,
    store: &[u8; 32],
    sql: &str,
    args: impl rusqlite::Params,
) -> Read<Vec<Stored>> {
    let mut statement = match conn.prepare_cached(sql) {
        Ok(statement) => statement,
        Err(error) => return read_error(&error),
    };
    let rows = match statement.query_map(args, raw) {
        Ok(rows) => rows,
        Err(error) => return read_error(&error),
    };
    let mut out = Vec::new();
    for row in rows {
        match row {
            Ok(raw) => match stored(raw) {
                Some(row) if row.verified(store) => out.push(row),
                _ => return Read::Corrupt,
            },
            Err(error) => return read_error(&error),
        }
    }
    Read::Got(out)
}

/// At most one row. Two where the schema allows one is `Corrupt`.
fn select_one(
    conn: &Connection,
    store: &[u8; 32],
    sql: &str,
    args: impl rusqlite::Params,
) -> Read<Option<Stored>> {
    match select(conn, store, sql, args) {
        Read::Got(mut rows) if rows.len() <= 1 => Read::Got(rows.pop()),
        Read::Got(_) => Read::Corrupt,
        Read::Failed => Read::Failed,
        Read::Corrupt => Read::Corrupt,
    }
}

fn by_id(conn: &Connection, store: &[u8; 32], id: &[u8; 32]) -> Read<Option<Stored>> {
    select_one(
        conn,
        store,
        &format!("SELECT {COLUMNS} FROM ledger WHERE id = ?1"),
        params![id.as_slice()],
    )
}

fn by_role(conn: &Connection, store: &[u8; 32], part: Part) -> Read<Option<Stored>> {
    select_one(
        conn,
        store,
        &format!("SELECT {COLUMNS} FROM ledger WHERE commitment = ?1 AND role = ?2"),
        params![part.commitment.as_slice(), role_code(part.role)],
    )
}

fn head_row(conn: &Connection, store: &[u8; 32]) -> Read<Option<Stored>> {
    select_one(
        conn,
        store,
        &format!("SELECT {COLUMNS} FROM ledger ORDER BY seq DESC LIMIT 1"),
        [],
    )
}

/// The durable acknowledgement frontier: the N of the last acknowledgement
/// row, or zero (section 4.8).
fn durable_frontier(conn: &Connection, store: &[u8; 32]) -> Read<Seq> {
    match select_one(
        conn,
        store,
        &format!("SELECT {COLUMNS} FROM ledger WHERE acked IS NOT NULL ORDER BY seq DESC LIMIT 1"),
        [],
    ) {
        Read::Got(Some(row)) => match (row.family, row.acked) {
            (Family::Acknowledgement, Some(n)) => Read::Got(n),
            _ => Read::Corrupt,
        },
        Read::Got(None) => Read::Got(0),
        Read::Failed => Read::Failed,
        Read::Corrupt => Read::Corrupt,
    }
}

/// The acknowledgement-requiring rows in `(above, upto]`: their highest
/// sequence, their count, and the digest of their identities in order.
/// Verify before filter (REVIEW-02 section 5.3): the range is read by the
/// primary key, every row in it is verified, and only then is the flag
/// that selects it trusted. A row whose flag was changed fails its tag
/// instead of dropping out of the set.
fn ack_range(
    conn: &Connection,
    store: &[u8; 32],
    above: Seq,
    upto: Seq,
) -> Read<(Seq, u64, [u8; 32])> {
    let (Ok(above_i), Ok(upto_i)) = (i64::try_from(above), i64::try_from(upto)) else {
        return Read::Corrupt;
    };
    let rows = match select(
        conn,
        store,
        &format!("SELECT {COLUMNS} FROM ledger WHERE seq > ?1 AND seq <= ?2 ORDER BY seq"),
        params![above_i, upto_i],
    ) {
        Read::Got(rows) => rows,
        Read::Failed => return Read::Failed,
        Read::Corrupt => return Read::Corrupt,
    };
    let mut top = 0;
    let mut ids: Vec<u8> = Vec::new();
    let mut count = 0u64;
    for row in rows.iter().filter(|row| row.needs_ack) {
        top = row.seq;
        count += 1;
        ids.extend_from_slice(&row.id);
    }
    Read::Got((top, count, hash(SET_DOMAIN, &[store, &ids])))
}

/// Appends one record at the head: its sequence is the head's plus one and
/// its chain the head's tag. A constraint failure is `Constraint`; anything
/// else that fails is `Failed`. A head that fails verification fails the
/// insert.
fn append(conn: &Connection, store: &[u8; 32], draft: &Draft) -> Insert {
    let (seq, prev) = match head_row(conn, store) {
        Read::Got(Some(head)) => match head.seq.checked_add(1) {
            Some(seq) => (seq, head.tag),
            None => return Insert::Failed,
        },
        Read::Got(None) => (1, [0; 32]),
        Read::Failed | Read::Corrupt => return Insert::Failed,
    };
    let Ok(seq_i) = i64::try_from(seq) else {
        return Insert::Failed;
    };
    let tag = row_tag(store, seq, draft, &prev);
    let about = draft.about.map(|key| {
        let mut bytes = key.id.to_vec();
        bytes.extend_from_slice(&key.digest);
        bytes
    });
    let Ok(acked) = draft.acked.map(i64::try_from).transpose() else {
        return Insert::Failed;
    };
    let result = conn.execute(
        &format!("INSERT INTO ledger ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"),
        params![
            seq_i,
            draft.family.code(),
            draft.id.as_slice(),
            draft.digest.as_slice(),
            draft.part.map(|part| part.commitment.to_vec()),
            draft.part.map(|part| role_code(part.role)),
            about,
            acked,
            u8::from(draft.needs_ack),
            draft.body,
            prev.as_slice(),
            tag.as_slice(),
        ],
    );
    match result {
        Ok(1) => Insert::Inserted(seq),
        Ok(_) => Insert::Failed,
        Err(error) if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) => {
            Insert::Constraint
        }
        Err(_) => Insert::Failed,
    }
}

/// Verb `open`: a write transaction, taken at once. It is refused when a
/// transaction is already open; the barrier closes that.
fn begin(conn: &Connection) -> Read<()> {
    if !conn.is_autocommit() {
        return Read::Failed;
    }
    match conn.execute_batch("BEGIN IMMEDIATE") {
        Ok(()) => Read::Got(()),
        Err(error) => read_error(&error),
    }
}

/// Verb `commit`: `Durable` only if COMMIT returned success (section 4.1).
fn commit(conn: &Connection) -> Commit {
    match conn.execute_batch("COMMIT") {
        Ok(()) => Commit::Durable,
        Err(_) => Commit::Unknown,
    }
}

/// Verb `rollback`: `Undone` only if ROLLBACK returned success and the
/// connection then reports that no transaction is open (section 4.1).
fn rollback(conn: &Connection) -> Rollback {
    match conn.execute_batch("ROLLBACK") {
        Ok(()) if conn.is_autocommit() => Rollback::Undone,
        _ => Rollback::Unknown,
    }
}

/// The whole chain, from the first row: each sequence follows the one before
/// it, each row's chain is the tag before it, every row verifies, and
/// SQLite's own integrity check (which compares every index with its table)
/// passes. Returns the head.
fn verify_all(conn: &Connection, store: &[u8; 32]) -> Read<Seq> {
    match conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)) {
        Ok(answer) if answer == "ok" => {}
        Ok(_) => return Read::Corrupt,
        Err(error) => return read_error(&error),
    }
    let rows = match select(
        conn,
        store,
        &format!("SELECT {COLUMNS} FROM ledger ORDER BY seq"),
        [],
    ) {
        Read::Got(rows) => rows,
        Read::Failed => return Read::Failed,
        Read::Corrupt => return Read::Corrupt,
    };
    let mut expected_seq = 1;
    let mut prev = [0; 32];
    for row in &rows {
        if row.seq != expected_seq || row.prev != prev {
            return Read::Corrupt;
        }
        prev = row.tag;
        expected_seq += 1;
    }
    Read::Got(expected_seq - 1)
}

// ---- the verbs ----------------------------------------------------------------------------------

/// A record staged for one append: the core names it by position and by its
/// member; the body was built by its family's builder.
#[derive(Clone, Debug)]
struct Staged {
    member: Member,
    draft: Draft,
}

/// The verbs of one fact or gating append.
struct AppendIo<'a> {
    conn: &'a Connection,
    store: [u8; 32],
    staged: &'a [Staged],
}

impl Txn for AppendIo<'_> {
    fn open(&mut self) -> Read<()> {
        begin(self.conn)
    }

    /// One record's facts, in the open transaction (section 4.1).
    fn facts(&mut self, index: u16, member: &Member) -> Read<MemberFacts> {
        if self.staged.get(usize::from(index)).map(|s| &s.member) != Some(member) {
            return Read::Failed;
        }
        member_facts(self.conn, &self.store, member)
    }

    fn insert(&mut self, index: u16, member: &Member) -> Insert {
        match self.staged.get(usize::from(index)) {
            Some(staged) if staged.member == *member => {
                append(self.conn, &self.store, &staged.draft)
            }
            _ => Insert::Failed,
        }
    }

    fn commit(&mut self) -> Commit {
        commit(self.conn)
    }

    fn rollback(&mut self) -> Rollback {
        rollback(self.conn)
    }
}

/// The five facts of one record. `corrected` comes only from a verified
/// correction row bound to the record's exact identity and digest, whose
/// sequence is the record's own row (REVIEW-02, condition 7 of the
/// acceptance); a correction whose record is not in the ledger is a
/// contradiction and reads as corrupt.
fn member_facts(conn: &Connection, store: &[u8; 32], member: &Member) -> Read<MemberFacts> {
    macro_rules! got {
        ($read:expr) => {
            match $read {
                Read::Got(value) => value,
                Read::Failed => return Read::Failed,
                Read::Corrupt => return Read::Corrupt,
            }
        };
    }
    let key = member.key;
    let row = got!(by_id(conn, store, &key.id)).map(|row| row.row());
    let explained = got!(by_id(conn, store, &explanation_id(store, &key))).map(|row| row.seq);
    let mut corrected = None;
    for exit in [Exit::ReturnedUncertain, Exit::Unwound] {
        if let Some(correction) = got!(by_id(
            conn,
            store,
            &correction_id(store, &key, exit_code(exit))
        )) {
            let mut d = Decoder(&correction.body);
            let seq = d.key().and_then(|_| d.u64());
            match (row, seq) {
                (Some(row), Some(seq)) if row.digest == key.digest && row.seq == seq => {
                    corrected = Some(correction.seq);
                }
                _ => return Read::Corrupt,
            }
        }
    }
    let (holder, prepared) = match member.part {
        None => (None, false),
        Some(part) => {
            let holder = got!(by_role(conn, store, part)).map(|row| Holder {
                id: row.id,
                seq: row.seq,
            });
            let prepared = got!(by_role(
                conn,
                store,
                Part {
                    commitment: part.commitment,
                    role: Role::Prepared,
                }
            ))
            .is_some();
            (holder, prepared)
        }
    };
    Read::Got(MemberFacts {
        row,
        explained,
        corrected,
        holder,
        prepared,
    })
}

/// The reads outside a transaction, and the barrier.
struct ReadIo<'a> {
    conn: &'a Connection,
    store: [u8; 32],
}

impl Reads for ReadIo<'_> {
    fn row(&mut self, id: &[u8; ID_LEN]) -> Read<Option<Row>> {
        match by_id(self.conn, &self.store, id) {
            Read::Got(row) => Read::Got(row.map(|row| row.row())),
            Read::Failed => Read::Failed,
            Read::Corrupt => Read::Corrupt,
        }
    }

    fn explained(&mut self, key: &RecordKey) -> Read<Option<Seq>> {
        match by_id(self.conn, &self.store, &explanation_id(&self.store, key)) {
            Read::Got(row) => Read::Got(row.map(|row| row.seq)),
            Read::Failed => Read::Failed,
            Read::Corrupt => Read::Corrupt,
        }
    }

    fn acknowledgement(&mut self, set: &SetFacts, nonce: &[u8; 16]) -> Read<Option<Seq>> {
        let draft = ack_draft(&self.store, set, nonce);
        match by_id(self.conn, &self.store, &draft.id) {
            Read::Got(Some(row)) if row.digest == draft.digest => Read::Got(Some(row.seq)),
            Read::Got(Some(_)) => Read::Corrupt,
            Read::Got(None) => Read::Got(None),
            Read::Failed => Read::Failed,
            Read::Corrupt => Read::Corrupt,
        }
    }

    fn frontier(&mut self) -> Read<Seq> {
        durable_frontier(self.conn, &self.store)
    }
}

impl BarrierRun for ReadIo<'_> {
    /// The durability barrier: the connection back to a known state (a
    /// transaction an unwind left open is rolled back), then the whole chain
    /// and every index verified. True only if all of it passed.
    fn run(&mut self) -> bool {
        if !self.conn.is_autocommit() {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
        self.conn.is_autocommit() && matches!(verify_all(self.conn, &self.store), Read::Got(_))
    }
}

/// The acknowledgement's verbs.
struct AckIo<'a> {
    conn: &'a Connection,
    store: [u8; 32],
}

impl AckIo<'_> {
    fn set(&self, above: Seq, upto: Option<Seq>) -> Read<SetFacts> {
        let frontier = match durable_frontier(self.conn, &self.store) {
            Read::Got(frontier) => frontier,
            Read::Failed => return Read::Failed,
            Read::Corrupt => return Read::Corrupt,
        };
        let head = match head_row(self.conn, &self.store) {
            Read::Got(head) => head.map_or(0, |row| row.seq),
            Read::Failed => return Read::Failed,
            Read::Corrupt => return Read::Corrupt,
        };
        if frontier > head {
            return Read::Corrupt;
        }
        let (above, upto) = match upto {
            Some(upto) => (above, upto),
            None => (frontier, head),
        };
        match ack_range(self.conn, &self.store, above, upto) {
            Read::Got((top, count, digest)) => Read::Got(SetFacts {
                frontier,
                head,
                top,
                count,
                digest,
            }),
            Read::Failed => Read::Failed,
            Read::Corrupt => Read::Corrupt,
        }
    }
}

impl AckTxn for AckIo<'_> {
    fn survey(&mut self) -> Read<SetFacts> {
        self.set(0, None)
    }

    fn open(&mut self) -> Read<()> {
        begin(self.conn)
    }

    fn recheck(&mut self, above: Seq, upto: Seq) -> Read<SetFacts> {
        self.set(above, Some(upto))
    }

    fn insert(&mut self, snapshot: &Snapshot) -> Insert {
        append(
            self.conn,
            &self.store,
            &ack_draft(&self.store, &snapshot.set(), &snapshot.nonce()),
        )
    }

    fn commit(&mut self) -> Commit {
        commit(self.conn)
    }

    fn rollback(&mut self) -> Rollback {
        rollback(self.conn)
    }
}

/// A cell's own occupant, appended.
struct CellIo<'a> {
    conn: &'a Connection,
    store: [u8; 32],
}

impl CellTxn for CellIo<'_> {
    fn open(&mut self) -> Read<()> {
        begin(self.conn)
    }

    /// The occupant's row, if present. A row under its identity with other
    /// content contradicts the identity's derivation and reads as corrupt.
    fn present(&mut self, occupant: &Occupant) -> Read<Option<Seq>> {
        let Some(draft) = cell_draft(&self.store, occupant) else {
            return Read::Failed;
        };
        match by_id(self.conn, &self.store, &draft.id) {
            Read::Got(Some(row)) if row.digest == draft.digest => Read::Got(Some(row.seq)),
            Read::Got(Some(_)) => Read::Corrupt,
            Read::Got(None) => Read::Got(None),
            Read::Failed => Read::Failed,
            Read::Corrupt => Read::Corrupt,
        }
    }

    fn insert(&mut self, occupant: &Occupant) -> Insert {
        match cell_draft(&self.store, occupant) {
            Some(draft) => append(self.conn, &self.store, &draft),
            None => Insert::Failed,
        }
    }

    fn commit(&mut self) -> Commit {
        commit(self.conn)
    }

    fn rollback(&mut self) -> Rollback {
        rollback(self.conn)
    }
}

// ---- records and their builders ------------------------------------------------------------------

/// What a store's records are bound to: one `Store` value, by allocation (as
/// the core binds its tickets), and its persistent random identity, which
/// every identity, digest and row tag covers.
#[derive(Debug)]
struct Binding {
    identity: [u8; 32],
}

/// A record built by this store's builder for its family. It can be offered
/// only to the store that built it.
#[derive(Clone, Debug)]
pub(super) struct Built<R> {
    binding: Arc<Binding>,
    record: R,
    draft: Draft,
}

impl<R> Built<R> {
    pub(super) fn record(&self) -> &R {
        &self.record
    }

    pub(super) fn key(&self) -> RecordKey {
        self.draft.key()
    }
}

/// Refused before the core is asked: a record another store built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OtherStore;

#[derive(Debug)]
pub(super) enum StoreError {
    /// The file exists (create) or does not (open).
    Path,
    /// SQLite refused to open, lock or configure the file. The message is
    /// read through `Debug` when a test fails.
    Sqlite(#[allow(dead_code)] String),
    /// The file's schema, identity or rows are not what this shell wrote.
    Corrupt,
    /// Another connection holds the file.
    Busy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PrivateWrite {
    /// COMMIT returned success, or the record was found present.
    Confirmed,
    NotWritten,
    Uncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Opening {
    Opened,
    NotBootstrap,
    NotWritten,
    Uncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Sealing {
    /// Sealed, and the close record is known durable.
    Closed,
    /// Sealed; the close record's append was attempted and is not known
    /// durable. Sealed stays sealed.
    SealedCloseNotWritten,
    SealedCloseUncertain,
    /// Not quiescent: nothing changed.
    NotQuiescent,
}

/// The store: one connection, one core, and the session's three facts.
pub(super) struct Store {
    conn: Connection,
    binding: Arc<Binding>,
    core: Core,
    health: Health,
    session_nonce: [u8; 16],
    sealed: bool,
    close_attempted: bool,
    close_confirmed: bool,
}

fn sqlite(error: rusqlite::Error) -> StoreError {
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => StoreError::Busy,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => StoreError::Corrupt,
        _ => StoreError::Sqlite(error.to_string()),
    }
}

/// Opens a connection to an existing file, never creating one and never
/// through a symbolic link; WAL with a full sync at each commit; one
/// exclusive lock, taken now and held for the connection's life, so no other
/// connection can open the store.
fn connect(path: &Path) -> Result<Connection, StoreError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(sqlite)?;
    let mode: String = conn
        .query_row("PRAGMA locking_mode = EXCLUSIVE", [], |row| row.get(0))
        .map_err(sqlite)?;
    if mode != "exclusive" {
        return Err(StoreError::Sqlite(format!("locking mode {mode}")));
    }
    let journal: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(sqlite)?;
    if journal != "wal" {
        return Err(StoreError::Busy);
    }
    conn.execute_batch(
        "PRAGMA synchronous = FULL; PRAGMA foreign_keys = OFF; PRAGMA trusted_schema = OFF",
    )
    .map_err(sqlite)?;
    conn.execute_batch("BEGIN IMMEDIATE; COMMIT")
        .map_err(sqlite)?;
    Ok(conn)
}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes).expect("the system random source");
    bytes
}

impl Store {
    /// Creates a new store file. Refuses a path that exists.
    pub(super) fn create(path: &Path, limits: Limits) -> Result<Store, StoreError> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| StoreError::Path)?;
        let conn = connect(path)?;
        let identity: [u8; 32] = random();
        conn.execute_batch("BEGIN IMMEDIATE").map_err(sqlite)?;
        for statement in SCHEMA {
            conn.execute_batch(statement).map_err(sqlite)?;
        }
        conn.execute(
            "INSERT INTO meta (k, v) VALUES ('schema', ?1), ('store', ?2)",
            params![SCHEMA_VERSION, identity.as_slice()],
        )
        .map_err(sqlite)?;
        conn.execute_batch("COMMIT").map_err(sqlite)?;
        Ok(Store::with(conn, identity, limits))
    }

    /// Opens an existing store file: its schema exactly, its identity, its
    /// whole chain and every index, before anything else reads it.
    pub(super) fn open(path: &Path, limits: Limits) -> Result<Store, StoreError> {
        if !path.is_file() {
            return Err(StoreError::Path);
        }
        let conn = connect(path)?;
        let schema: Vec<String> = {
            let mut statement = conn
                .prepare("SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY rowid")
                .map_err(sqlite)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(sqlite)?;
            rows.collect::<Result<_, _>>().map_err(sqlite)?
        };
        if schema != SCHEMA {
            return Err(StoreError::Corrupt);
        }
        let meta: Vec<(String, Vec<u8>)> = {
            let mut statement = conn
                .prepare("SELECT k, v FROM meta ORDER BY k")
                .map_err(sqlite)?;
            let rows = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(sqlite)?;
            rows.collect::<Result<_, _>>().map_err(sqlite)?
        };
        let identity = match meta.as_slice() {
            [(schema_k, schema_v), (store_k, store_v)]
                if schema_k == "schema" && schema_v == SCHEMA_VERSION && store_k == "store" =>
            {
                <[u8; 32]>::try_from(store_v.as_slice()).map_err(|_| StoreError::Corrupt)?
            }
            _ => return Err(StoreError::Corrupt),
        };
        match verify_all(&conn, &identity) {
            Read::Got(_) => Ok(Store::with(conn, identity, limits)),
            Read::Failed => Err(StoreError::Sqlite("verification failed".into())),
            Read::Corrupt => Err(StoreError::Corrupt),
        }
    }

    fn with(conn: Connection, identity: [u8; 32], limits: Limits) -> Store {
        let (core, health) = Core::new(limits);
        Store {
            conn,
            binding: Arc::new(Binding { identity }),
            core,
            health,
            session_nonce: random(),
            sealed: false,
            close_attempted: false,
            close_confirmed: false,
        }
    }

    fn identity(&self) -> [u8; 32] {
        self.binding.identity
    }

    pub(super) fn health(&self) -> &Health {
        &self.health
    }

    pub(super) fn agenda(&self) -> Agenda {
        self.core.agenda()
    }

    pub(super) fn take_worker(&mut self) -> Option<Worker> {
        self.core.take_worker()
    }

    // ---- builders ----

    fn build(&self, family: Family, identity: &[u8], part: Option<Part>, content: &[u8]) -> Draft {
        let store = self.identity();
        let id = record_id(&store, family, identity);
        let body = content.to_vec();
        Draft {
            family,
            id,
            digest: record_digest(&store, family, &id, part, &body),
            part,
            about: None,
            acked: None,
            needs_ack: false,
            body,
        }
    }

    /// A fact record: its identity from `identity` under this store, its
    /// digest over its family, identity, part and content.
    pub(super) fn fact(
        &self,
        identity: &[u8],
        part: Option<Part>,
        content: &[u8],
    ) -> Built<FactRecord> {
        let draft = self.build(Family::Fact, identity, part, content);
        Built {
            binding: Arc::clone(&self.binding),
            record: FactRecord::new(Member {
                key: draft.key(),
                part,
            }),
            draft,
        }
    }

    pub(super) fn gating(
        &self,
        identity: &[u8],
        part: Option<Part>,
        content: &[u8],
    ) -> Built<GatingRecord> {
        let draft = self.build(Family::Gating, identity, part, content);
        Built {
            binding: Arc::clone(&self.binding),
            record: GatingRecord::new(Member {
                key: draft.key(),
                part,
            }),
            draft,
        }
    }

    fn mine<R>(&self, built: &Built<R>) -> bool {
        Arc::ptr_eq(&built.binding, &self.binding)
    }

    // ---- the core's operations, with this store's verbs ----

    pub(super) fn append_facts(
        &mut self,
        ticket: &mut Ticket,
        batch: &[Built<FactRecord>],
    ) -> Result<FactOutcome, OtherStore> {
        if !batch.iter().all(|built| self.mine(built)) {
            return Err(OtherStore);
        }
        let staged: Vec<Staged> = batch
            .iter()
            .map(|built| Staged {
                member: *built.record.member(),
                draft: built.draft.clone(),
            })
            .collect();
        let records: Vec<FactRecord> = batch.iter().map(|built| built.record.clone()).collect();
        let mut io = AppendIo {
            conn: &self.conn,
            store: self.binding.identity,
            staged: &staged,
        };
        Ok(self.core.append_facts(ticket, &records, &mut io))
    }

    pub(super) fn append_gating(
        &mut self,
        built: &Built<GatingRecord>,
    ) -> Result<GatingOutcome, OtherStore> {
        if !self.mine(built) {
            return Err(OtherStore);
        }
        let staged = [Staged {
            member: *built.record.member(),
            draft: built.draft.clone(),
        }];
        let mut io = AppendIo {
            conn: &self.conn,
            store: self.binding.identity,
            staged: &staged,
        };
        Ok(self.core.append_gating(&built.record, &mut io))
    }

    pub(super) fn lookup(&mut self, key: &RecordKey) -> Lookup {
        let mut io = ReadIo {
            conn: &self.conn,
            store: self.binding.identity,
        };
        self.core.lookup(key, &mut io)
    }

    pub(super) fn explain(&mut self, key: &RecordKey) -> Explain {
        let mut io = ReadIo {
            conn: &self.conn,
            store: self.binding.identity,
        };
        self.core.explain(key, &mut io)
    }

    /// The nonce comes from the shell's random source (section 4.8).
    pub(super) fn snapshot(&mut self) -> SnapshotOutcome {
        let mut io = AckIo {
            conn: &self.conn,
            store: self.binding.identity,
        };
        self.core.snapshot(random(), &mut io)
    }

    pub(super) fn append_acknowledgement(&mut self, snapshot: Snapshot) -> AckOutcome {
        let mut io = AckIo {
            conn: &self.conn,
            store: self.binding.identity,
        };
        self.core.append_acknowledgement(snapshot, &mut io)
    }

    pub(super) fn barrier(&mut self, worker: &mut Worker) -> BarrierOutcome {
        let mut io = ReadIo {
            conn: &self.conn,
            store: self.binding.identity,
        };
        self.core.barrier(worker, &mut io)
    }

    pub(super) fn resolve(&mut self, worker: &mut Worker, token: CellToken) -> Resolve {
        let mut io = ReadIo {
            conn: &self.conn,
            store: self.binding.identity,
        };
        self.core.resolve(worker, token, &mut io)
    }

    pub(super) fn append_cell(&mut self, worker: &mut Worker, token: CellToken) -> CellAppend {
        let mut io = CellIo {
            conn: &self.conn,
            store: self.binding.identity,
        };
        self.core.append_cell(worker, token, &mut io)
    }

    pub(super) fn empty(&mut self, worker: &mut Worker, token: CellToken) -> Emptying {
        self.core.empty(worker, token)
    }

    // ---- the session ----

    fn session_draft(&self, family: Family) -> Draft {
        let store = self.identity();
        let body = self.session_nonce.to_vec();
        let id = record_id(&store, family, &body);
        Draft {
            family,
            id,
            digest: record_digest(&store, family, &id, None, &body),
            part: None,
            about: None,
            acked: None,
            needs_ack: false,
            body,
        }
    }

    /// One private record, outside the core: used only while the core runs
    /// no transaction (bootstrap, and after the seal). Found present counts
    /// as confirmed; a transaction left open is rolled back first.
    fn write_private(&self, draft: &Draft) -> PrivateWrite {
        if !self.conn.is_autocommit() && rollback(&self.conn) != Rollback::Undone {
            return PrivateWrite::NotWritten;
        }
        if begin(&self.conn) != Read::Got(()) {
            return PrivateWrite::NotWritten;
        }
        let store = self.identity();
        match by_id(&self.conn, &store, &draft.id) {
            Read::Got(Some(row)) if row.digest == draft.digest => {
                rollback(&self.conn);
                return PrivateWrite::Confirmed;
            }
            Read::Got(None) => {}
            Read::Got(Some(_)) | Read::Failed | Read::Corrupt => {
                rollback(&self.conn);
                return PrivateWrite::NotWritten;
            }
        }
        match append(&self.conn, &store, draft) {
            Insert::Inserted(_) => match commit(&self.conn) {
                Commit::Durable => PrivateWrite::Confirmed,
                Commit::Unknown => {
                    rollback(&self.conn);
                    PrivateWrite::Uncertain
                }
            },
            Insert::Constraint | Insert::Failed => match rollback(&self.conn) {
                Rollback::Undone => PrivateWrite::NotWritten,
                Rollback::Unknown => PrivateWrite::Uncertain,
            },
        }
    }

    /// Opening: the session is open only once its opening record is
    /// confirmed durable (section 4.9; REVIEW-02 section 5.1).
    pub(super) fn open_session(&mut self) -> Opening {
        if self.core.agenda().session != Session::Bootstrap {
            return Opening::NotBootstrap;
        }
        match self.write_private(&self.session_draft(Family::Opened)) {
            PrivateWrite::Confirmed => {
                self.core.session_opened();
                Opening::Opened
            }
            PrivateWrite::NotWritten => Opening::NotWritten,
            PrivateWrite::Uncertain => Opening::Uncertain,
        }
    }

    /// Sealing comes first, does not depend on any record and is never
    /// undone; the close record is a separate attempt afterwards, which may
    /// be repeated (section 4.9, RF-01).
    pub(super) fn seal_and_close(&mut self) -> Sealing {
        if !self.sealed {
            if !self.core.session_sealed() {
                return Sealing::NotQuiescent;
            }
            self.sealed = true;
        }
        self.close_attempted = true;
        match self.write_private(&self.session_draft(Family::Closed)) {
            PrivateWrite::Confirmed => {
                self.close_confirmed = true;
                Sealing::Closed
            }
            PrivateWrite::NotWritten => Sealing::SealedCloseNotWritten,
            PrivateWrite::Uncertain => Sealing::SealedCloseUncertain,
        }
    }

    /// The three facts of the session's end: sealed, close attempted, close
    /// confirmed.
    pub(super) fn session_end(&self) -> (bool, bool, bool) {
        (self.sealed, self.close_attempted, self.close_confirmed)
    }
}

// ---- the wait to enter the store ----------------------------------------------------------------

/// The store's one lock, with a time limit on the wait to enter it (CONTRACT-02-R1
/// section 13). A store whose operation unwinds is dropped with it, so every
/// later wait runs out: the store fails closed.
pub(super) struct Gate {
    slot: Mutex<Option<Store>>,
    freed: Condvar,
}

impl Gate {
    pub(super) fn new(store: Store) -> Gate {
        Gate {
            slot: Mutex::new(Some(store)),
            freed: Condvar::new(),
        }
    }

    /// Runs `op` with the store if it can be entered by `deadline`.
    pub(super) fn with<T>(&self, deadline: Instant, op: impl FnOnce(&mut Store) -> T) -> Option<T> {
        let mut store = {
            let mut slot = self
                .slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            loop {
                if let Some(store) = slot.take() {
                    break store;
                }
                let now = Instant::now();
                if now >= deadline {
                    return None;
                }
                slot = self
                    .freed
                    .wait_timeout(slot, deadline - now)
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .0;
            }
        };
        let result = op(&mut store);
        *self
            .slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(store);
        self.freed.notify_one();
        Some(result)
    }

    /// A fact append that waits at most until `deadline`; when the wait runs
    /// out, the ticket is expired (section 4.3).
    pub(super) fn append_facts(
        &self,
        deadline: Instant,
        ticket: &mut Ticket,
        batch: &[Built<FactRecord>],
    ) -> Result<FactOutcome, OtherStore> {
        match self.with(deadline, |store| store.append_facts(ticket, batch)) {
            Some(outcome) => outcome,
            None => Ok(ticket.expire()),
        }
    }
}

// ---- test support ----------------------------------------------------------------------------------

/// A directory of its own under the system's temporary directory, removed
/// when dropped.
pub(super) struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    pub(super) fn new() -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("nexus-ledger-shell-{}", hex::encode(random::<8>())));
        std::fs::create_dir(&dir).expect("a scratch directory");
        Scratch { dir }
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(super) const LIMITS: Limits = Limits {
    tickets: 8,
    refusals: 8,
};

/// A commitment's id, for tests.
pub(super) fn commitment(n: u8) -> [u8; ID_LEN] {
    [n; ID_LEN]
}

pub(super) fn part(n: u8, role: Role) -> Option<Part> {
    Some(Part {
        commitment: commitment(n),
        role,
    })
}

/// Every row of a store file, read through a fresh connection after the
/// store is gone: what the disk holds. Unverified.
pub(super) fn rows_on_disk(path: &Path) -> Vec<(Seq, [u8; 32], [u8; 32])> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("the store file opens read-only");
    let mut statement = conn
        .prepare("SELECT seq, id, digest FROM ledger ORDER BY seq")
        .expect("a select");
    let rows = statement
        .query_map([], |row| {
            let seq: i64 = row.get(0)?;
            let id: Vec<u8> = row.get(1)?;
            let digest: Vec<u8> = row.get(2)?;
            Ok((
                seq as Seq,
                id.try_into().unwrap(),
                digest.try_into().unwrap(),
            ))
        })
        .expect("rows");
    rows.map(|row| row.expect("a row")).collect()
}

/// Whether the record is on disk, by its key.
pub(super) fn on_disk(path: &Path, key: &RecordKey) -> bool {
    rows_on_disk(path)
        .iter()
        .any(|(_, id, digest)| *id == key.id && *digest == key.digest)
}

/// Opens a store's session or fails the test.
pub(super) fn opened(path: &Path) -> Store {
    let mut store = Store::create(path, LIMITS).expect("a new store");
    assert_eq!(store.open_session(), Opening::Opened);
    store
}

/// Plays the layer above's worker (B2's code; the tests play it, as in
/// CONTRACT-02-R1 section 6.3): barrier, resolve, append C, append and empty
/// X, until the store is Healthy with empty cells or a step fails. On a
/// latched store only the barrier runs. At most `steps` steps.
pub(super) fn drive(store: &mut Store, worker: &mut Worker, steps: usize) -> Agenda {
    for _ in 0..steps {
        let agenda = store.agenda();
        if agenda.latch.is_some() {
            // The barrier's entry admits any latch (CONTRACT-02-R1 5.3): it
            // still returns the connection to a known state. Nothing else
            // runs on a latched store.
            if agenda.phase == Phase::NeedBarrier {
                store.barrier(worker);
            }
            return store.agenda();
        }
        match agenda.phase {
            Phase::NeedBarrier => {
                if store.barrier(worker) != BarrierOutcome::Passed {
                    return store.agenda();
                }
            }
            Phase::Resolving => match agenda.u {
                Some(token) => {
                    if matches!(
                        store.resolve(worker, token),
                        Resolve::Retained | Resolve::Refused(_) | Resolve::Inconsistent
                    ) {
                        return store.agenda();
                    }
                }
                None => return agenda,
            },
            Phase::Healthy => {
                if let Some(token) = agenda.c {
                    if !matches!(store.append_cell(worker, token), CellAppend::Committed(_)) {
                        return store.agenda();
                    }
                } else if let Some((token, stage)) = agenda.x {
                    match stage {
                        Stage::Built => {
                            if !matches!(store.append_cell(worker, token), CellAppend::Committed(_))
                            {
                                return store.agenda();
                            }
                        }
                        Stage::Committed(_) => {
                            store.empty(worker, token);
                        }
                    }
                } else {
                    return agenda;
                }
            }
        }
    }
    store.agenda()
}
