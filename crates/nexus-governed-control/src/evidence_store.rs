//! The Phase Three evidence store (contract EL-1 §1–§5): the evidence of
//! record of governed real-world control, in one SQLite file that Phase
//! Three owns.
//!
//! - One file, at a path the desktop builds from its own state directory
//!   (never from a caller's text): absolute, in a private (0700) directory;
//!   a symbolic link at the directory or the file is refused. The file is
//!   created only when it does not exist, its schema and one identity row in
//!   one transaction, then the directory is synced. An existing file that is
//!   not a qualified store is refused and never initialised in place: its
//!   header is read first, and SQLite opens it only when the header names
//!   this format.
//! - One connection, owned behind one mutex (a leaf: nothing else is called
//!   while it is held), opened with its settings read back: WAL,
//!   `synchronous = FULL`, an exclusive lock taken at open, the automatic
//!   checkpoint every 1000 pages, `trusted_schema = OFF`. Extensions are
//!   never enabled.
//! - Qualified before any append: the settings, `integrity_check`, the exact
//!   schema and identity row, then the whole hash chain.
//! - Append-only: triggers refuse every UPDATE and DELETE. Each append is
//!   one `BEGIN IMMEDIATE … COMMIT`. A record of a once-only phase of one
//!   commitment written again with the same body is already present; with
//!   another body it conflicts.
//! - A COMMIT that itself fails (its outcome cannot be known), a conflict,
//!   and a row whose tag does not verify latch the store: from then on this
//!   process appends, reads and qualifies nothing through it. Availability is
//!   the cost; nothing falls back.
//!
//! - [`PrimaryEvidence`] is the production sink (§6): the store's result
//!   decides; the audit trail and the recent history are projections,
//!   written only after the store committed. A gating record (written
//!   before the action it permits) first writes every owed record, in
//!   order, and is refused while one is still owed or the store is latched.
//!   An end record (a transition that has already taken effect) never fails
//!   and never waits for the owner: written, owed (kept, bounded, with its
//!   original content), or uncertain (latched).
//!
//! - Sessions (§7): each qualification appends `session_opened`; a clean
//!   shutdown appends `session_closed`. Before anything is admitted, once,
//!   over the whole verified chain, every commitment that started and has no
//!   end and no `unsettled` statement gets one ("outcome not recorded; the
//!   effect may have occurred"), and every earlier session opened and never
//!   closed gets an `unclean_session` statement. Recovery is required while
//!   an `unsettled` statement is newer than what the owner acknowledged:
//!   until then every gating record is refused. The acknowledgement covers
//!   exactly the statements the owner was shown, grants nothing and
//!   changes no outcome; it is the one record written for an action while
//!   recovery is required.
//!
//! The tag is a SHA-256 chain over each row's fields, not a MAC: a writer
//! running as the same OS user is outside the threat model (EL-1 §9), and
//! tail truncation by such a writer is not detected (no external anchor).

use crate::authority::evidence::{
    EndStatus, EvidenceProjection, EvidenceRecord, EvidenceSink, EvidenceUnavailable,
};
use rusqlite::{params, Connection, ErrorCode, OpenFlags};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// The format the identity row names.
pub(crate) const FORMAT: &str = "p3-evidence/1";

/// The header's application id (`P3EV`): a file whose header does not carry
/// it is not opened by SQLite at all, so it cannot be changed.
const APPLICATION_ID: i32 = 0x5033_4556;

/// `prev` of the first row.
const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The schema, exactly: (type, name, table, SQL). Created in this order;
/// compared, by name, with `sqlite_schema` at every open. The unique index
/// covers the phases one commitment writes at most once (inventory: receipt
/// 002-N0, section C); `approved` and `credential_released` can repeat and
/// stay out of it.
const SCHEMA: [(&str, &str, &str, &str); 7] = [
    (
        "table",
        "identity",
        "identity",
        "CREATE TABLE identity (one INTEGER PRIMARY KEY CHECK (one = 1), \
         store TEXT NOT NULL, format TEXT NOT NULL) STRICT",
    ),
    (
        "table",
        "record",
        "record",
        "CREATE TABLE record (seq INTEGER PRIMARY KEY, session TEXT NOT NULL, \
         stored_ms INTEGER NOT NULL, phase TEXT NOT NULL, commitment TEXT, \
         body TEXT NOT NULL, prev TEXT NOT NULL, tag TEXT NOT NULL) STRICT",
    ),
    (
        "index",
        "record_once",
        "record",
        "CREATE UNIQUE INDEX record_once ON record (commitment, phase) \
         WHERE commitment IS NOT NULL AND phase IN ('prepared', 'authorized', \
         'started', 'finished', 'expired', 'revoked', 'denied', \
         'approval_declined', 'unsettled')",
    ),
    (
        "trigger",
        "identity_no_delete",
        "identity",
        "CREATE TRIGGER identity_no_delete BEFORE DELETE ON identity \
         BEGIN SELECT RAISE(ABORT, 'the evidence store is append-only'); END",
    ),
    (
        "trigger",
        "identity_no_update",
        "identity",
        "CREATE TRIGGER identity_no_update BEFORE UPDATE ON identity \
         BEGIN SELECT RAISE(ABORT, 'the evidence store is append-only'); END",
    ),
    (
        "trigger",
        "record_no_delete",
        "record",
        "CREATE TRIGGER record_no_delete BEFORE DELETE ON record \
         BEGIN SELECT RAISE(ABORT, 'the evidence store is append-only'); END",
    ),
    (
        "trigger",
        "record_no_update",
        "record",
        "CREATE TRIGGER record_no_update BEFORE UPDATE ON record \
         BEGIN SELECT RAISE(ABORT, 'the evidence store is append-only'); END",
    ),
];

/// Why the store is not qualified. Names no path and no content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unqualified {
    /// The path is not absolute, or names no file.
    Path,
    /// A symbolic link at the directory or the file.
    Link,
    /// The directory cannot be created, made private or synced.
    Directory,
    /// An existing file that is not a store of this format.
    NotAStore,
    /// Another connection holds the file.
    Busy,
    /// A setting did not take (read back).
    Settings(&'static str),
    /// `integrity_check` did not answer exactly `ok`.
    Integrity,
    /// The schema is not exactly this format's.
    Schema,
    /// The identity row is missing or not exact.
    Identity,
    /// A row's sequence, link or tag does not verify.
    Chain,
    /// SQLite failed otherwise.
    Sqlite,
    /// The store latched in this process.
    Latched,
}

impl Unqualified {
    pub fn as_str(self) -> &'static str {
        match self {
            Unqualified::Path => "the store path is not an absolute file path",
            Unqualified::Link => "the store path is a symbolic link",
            Unqualified::Directory => "the store directory cannot be made private",
            Unqualified::NotAStore => "an existing file is not an evidence store",
            Unqualified::Busy => "the store is held by another connection",
            Unqualified::Settings(_) => "a store setting did not take",
            Unqualified::Integrity => "the store failed its integrity check",
            Unqualified::Schema => "the store schema is not exact",
            Unqualified::Identity => "the store identity is not exact",
            Unqualified::Chain => "the store chain does not verify",
            Unqualified::Sqlite => "the store cannot be read",
            Unqualified::Latched => "the store is latched",
        }
    }
}

impl std::fmt::Display for Unqualified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evidence store unqualified: {}", self.as_str())
    }
}

impl std::error::Error for Unqualified {}

/// What one append came to (EL-1 §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Append {
    /// COMMIT returned success: the row is `seq`.
    Committed { seq: u64 },
    /// The same once-only record, byte for byte, is already row `seq`.
    AlreadyPresent { seq: u64 },
    /// The failure came before COMMIT was issued and the transaction rolled
    /// back: nothing was written.
    NotCommitted,
    /// COMMIT itself failed, or the outcome cannot otherwise be known: the
    /// store latched.
    Uncertain,
    /// A once-only record of the same commitment and phase with another
    /// body: the store latched.
    Conflict,
    /// The store is latched: nothing was attempted.
    Latched,
}

/// A row as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) seq: u64,
    pub(crate) session: String,
    pub(crate) stored_ms: u64,
    pub(crate) phase: String,
    pub(crate) commitment: Option<String>,
    pub(crate) body: String,
    pub(crate) prev: String,
    pub(crate) tag: String,
}

/// The most records owed at once: at least the commitments that can be live
/// at once (each ends with one record), twice over for the runs, grants and
/// stops recorded beside them. One more latches the store.
pub(crate) const OWED_CAPACITY: usize = 2 * crate::authority::commitment::CAPACITY;

/// The text of every `unsettled` statement (EL-1 §7).
pub(crate) const UNSETTLED_TEXT: &str = "outcome not recorded; the effect may have occurred";

/// What the store knows of its sessions and statements (EL-1 §7), read from
/// the verified chain and kept as rows are appended.
#[derive(Default)]
struct Known {
    /// Commitments with a `started` row: its sequence number.
    started: BTreeMap<String, u64>,
    /// Commitments with a `finished` row (the one end of a started one).
    finished: BTreeSet<String>,
    /// `unsettled` statements: commitment → statement sequence number.
    unsettled: BTreeMap<String, u64>,
    /// Sessions opened, with the sequence number of `session_opened`.
    opened: Vec<(String, u64)>,
    closed: BTreeSet<String>,
    /// `unclean_session` statements: session → statement sequence number.
    unclean: BTreeMap<String, u64>,
    /// The highest `covers_through` acknowledged.
    acknowledged: u64,
}

impl Known {
    /// Take one verified row into account.
    fn take(&mut self, row: &Row) {
        let json = || serde_json::from_str::<serde_json::Value>(&row.body).ok();
        match (row.phase.as_str(), row.commitment.as_ref()) {
            ("started", Some(commitment)) => {
                self.started.insert(commitment.clone(), row.seq);
            }
            ("finished", Some(commitment)) => {
                self.finished.insert(commitment.clone());
            }
            ("unsettled", Some(commitment)) => {
                self.unsettled.insert(commitment.clone(), row.seq);
            }
            ("session_opened", None) => {
                if let Some(session) =
                    json().and_then(|v| v["session"].as_str().map(str::to_string))
                {
                    self.opened.push((session, row.seq));
                }
            }
            ("session_closed", None) => {
                if let Some(session) =
                    json().and_then(|v| v["session"].as_str().map(str::to_string))
                {
                    self.closed.insert(session);
                }
            }
            ("unclean_session", None) => {
                if let Some(session) =
                    json().and_then(|v| v["session"].as_str().map(str::to_string))
                {
                    self.unclean.insert(session, row.seq);
                }
            }
            ("acknowledged", None) => {
                if let Some(through) = json().and_then(|v| v["covers_through"].as_u64()) {
                    self.acknowledged = self.acknowledged.max(through);
                }
            }
            _ => {}
        }
    }

    /// Recovery is required while an `unsettled` statement is newer than
    /// the highest acknowledgement (an `unclean_session` alone does not
    /// hold it).
    fn recovery_required(&self) -> bool {
        self.unsettled.values().any(|seq| *seq > self.acknowledged)
    }
}

/// A statement's body (canonical JSON).
fn statement(kind: &str, mut fields: serde_json::Value) -> String {
    fields["event_kind"] = serde_json::Value::from("p3.evidence.statement");
    fields["statement"] = serde_json::Value::from(kind);
    fields.to_string()
}

/// What the owner is shown before acknowledging (EL-1 §7): the statements
/// not yet acknowledged, and the highest of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Recovery {
    pub(crate) unsettled: Vec<String>,
    pub(crate) unclean_sessions: Vec<String>,
    pub(crate) covers_through: u64,
}

/// A record owed: its original content, written late.
struct Owed {
    record: EvidenceRecord,
    phase: &'static str,
    commitment: Option<String>,
    body: String,
}

impl Owed {
    fn of(record: &EvidenceRecord) -> Self {
        Self {
            record: record.clone(),
            phase: record.phase.as_str(),
            commitment: record.commitment.clone(),
            body: body_of(record),
        }
    }
}

/// A record's body: its canonical JSON (`to_json`, keys in order).
pub(crate) fn body_of(record: &EvidenceRecord) -> String {
    record.to_json().to_string()
}

/// A fault the tests plant at COMMIT.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    /// COMMIT is not issued; it is reported failed.
    FailCommit,
    /// COMMIT is issued and succeeds; it is reported failed.
    CommitThenFail,
}

struct Inner {
    conn: Connection,
    store: String,
    session: String,
    last_seq: u64,
    last_tag: String,
    latched: bool,
    /// Sessions and statements (EL-1 §7).
    known: Known,
    /// End records not yet written, in order.
    owed: VecDeque<Owed>,
    #[cfg(test)]
    fault: Option<Fault>,
    /// Every insert fails before COMMIT (a full disk), until cleared.
    #[cfg(test)]
    refusing: bool,
    /// The next insert alone fails before COMMIT (once), then inserts succeed.
    #[cfg(test)]
    refuse_next: bool,
}

/// The evidence store: one file, one connection.
pub(crate) struct EvidenceStore {
    inner: Mutex<Inner>,
}

/// The tag of one row: SHA-256 over its fields, each length-prefixed, after
/// a domain label (`commitment` is marked present or absent).
#[allow(clippy::too_many_arguments)]
fn tag_of(
    store: &str,
    prev: &str,
    seq: u64,
    session: &str,
    stored_ms: u64,
    phase: &str,
    commitment: Option<&str>,
    body: &str,
) -> String {
    let mut hash = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    };
    field(b"nexus.p3.evidence.tag.v1");
    field(store.as_bytes());
    field(prev.as_bytes());
    field(&seq.to_be_bytes());
    field(session.as_bytes());
    field(&stored_ms.to_be_bytes());
    field(phase.as_bytes());
    match commitment {
        Some(commitment) => {
            field(&[1]);
            field(commitment.as_bytes());
        }
        None => field(&[0]),
    }
    field(body.as_bytes());
    hex::encode(hash.finalize())
}

impl Row {
    fn verifies(&self, store: &str) -> bool {
        tag_of(
            store,
            &self.prev,
            self.seq,
            &self.session,
            self.stored_ms,
            &self.phase,
            self.commitment.as_deref(),
            &self.body,
        ) == self.tag
    }
}

fn random_hex() -> Result<String, Unqualified> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|_| Unqualified::Sqlite)?;
    Ok(hex::encode(bytes))
}

fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn is_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The store's directory: created private if missing; a link is refused;
/// it must be a directory, ours (we make it 0700).
fn private_directory(dir: &Path) -> Result<(), Unqualified> {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_symlink() => return Err(Unqualified::Link),
        Ok(meta) if !meta.is_dir() => return Err(Unqualified::Directory),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private(dir).map_err(|_| Unqualified::Directory)?;
        }
        Err(_) => return Err(Unqualified::Directory),
    }
    // Created by someone else meanwhile as a link: refused all the same.
    let meta = std::fs::symlink_metadata(dir).map_err(|_| Unqualified::Directory)?;
    if meta.file_type().is_symlink() {
        return Err(Unqualified::Link);
    }
    if !meta.is_dir() {
        return Err(Unqualified::Directory);
    }
    make_private(dir)
}

#[cfg(unix)]
fn create_private(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn create_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn make_private(dir: &Path) -> Result<(), Unqualified> {
    use std::os::unix::fs::PermissionsExt;
    // Only the owner can change the mode: this also proves it is ours.
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| Unqualified::Directory)
}

#[cfg(not(unix))]
fn make_private(_dir: &Path) -> Result<(), Unqualified> {
    Ok(())
}

fn sync_directory(dir: &Path) -> Result<(), Unqualified> {
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|_| Unqualified::Directory)
}

/// Create the file, empty, only if it does not exist (never truncating).
fn create_new(path: &Path) -> Result<bool, Unqualified> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => Err(Unqualified::Path),
    }
}

/// Whether the file's header names this format: the SQLite magic, the WAL
/// read and write versions, and the application id. Read with plain file
/// reads, before SQLite opens anything.
fn header_is_ours(path: &Path) -> Result<bool, Unqualified> {
    use std::io::Read;
    let mut header = [0u8; 100];
    let mut file = std::fs::File::open(path).map_err(|_| Unqualified::NotAStore)?;
    if file.read_exact(&mut header).is_err() {
        return Ok(false);
    }
    let application_id = i32::from_be_bytes([header[68], header[69], header[70], header[71]]);
    Ok(&header[..16] == b"SQLite format 3\0"
        && header[18] == 2
        && header[19] == 2
        && application_id == APPLICATION_ID)
}

/// The synchronous level the settings ask for (a test may plant another to
/// prove it is read back).
fn synchronous_setting() -> &'static str {
    #[cfg(test)]
    if let Some(level) = tests::planted_synchronous() {
        return level;
    }
    "FULL"
}

/// The one connection, with every setting read back and the exclusive lock
/// taken.
fn connect(path: &Path) -> Result<Connection, Unqualified> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(|_| Unqualified::Sqlite)?;
    let text = |sql: &str| -> Result<String, Unqualified> {
        conn.query_row(sql, [], |row| row.get::<_, String>(0))
            .map_err(|_| Unqualified::Sqlite)
    };
    let number = |sql: &str| -> Result<i64, Unqualified> {
        conn.query_row(sql, [], |row| row.get::<_, i64>(0))
            .map_err(|_| Unqualified::Sqlite)
    };
    if text("PRAGMA locking_mode = EXCLUSIVE")? != "exclusive" {
        return Err(Unqualified::Settings("locking_mode"));
    }
    match conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(mode) if mode == "wal" => {}
        Ok(_) => return Err(Unqualified::Settings("journal_mode")),
        Err(error) if error.sqlite_error_code() == Some(ErrorCode::DatabaseBusy) => {
            return Err(Unqualified::Busy)
        }
        Err(_) => return Err(Unqualified::Sqlite),
    }
    conn.execute_batch(&format!(
        "PRAGMA synchronous = {}; PRAGMA trusted_schema = OFF; PRAGMA wal_autocheckpoint = 1000;",
        synchronous_setting()
    ))
    .map_err(|_| Unqualified::Sqlite)?;
    if number("PRAGMA synchronous")? != 2 {
        return Err(Unqualified::Settings("synchronous"));
    }
    if number("PRAGMA wal_autocheckpoint")? != 1000 {
        return Err(Unqualified::Settings("wal_autocheckpoint"));
    }
    if number("PRAGMA trusted_schema")? != 0 {
        return Err(Unqualified::Settings("trusted_schema"));
    }
    // The exclusive lock is taken now, not at the first append.
    match conn.execute_batch("BEGIN IMMEDIATE; COMMIT;") {
        Ok(()) => Ok(conn),
        Err(error) if error.sqlite_error_code() == Some(ErrorCode::DatabaseBusy) => {
            Err(Unqualified::Busy)
        }
        Err(_) => Err(Unqualified::Sqlite),
    }
}

/// Whether the connection's transaction ended (rolled back by SQLite or by
/// us): only then did nothing commit.
fn rolled_back(conn: &Connection) -> bool {
    if conn.is_autocommit() {
        return true;
    }
    conn.execute_batch("ROLLBACK").is_ok() && conn.is_autocommit()
}

fn read_rows(conn: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Option<Vec<Row>> {
    let mut statement = conn.prepare(sql).ok()?;
    let rows = statement
        .query_map(args, |row| {
            Ok(Row {
                seq: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
                session: row.get(1)?,
                stored_ms: u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
                phase: row.get(3)?,
                commitment: row.get(4)?,
                body: row.get(5)?,
                prev: row.get(6)?,
                tag: row.get(7)?,
            })
        })
        .ok()?;
    rows.collect::<Result<Vec<Row>, _>>().ok()
}

const ROW_COLUMNS: &str = "seq, session, stored_ms, phase, commitment, body, prev, tag";

impl EvidenceStore {
    /// Open the store at `path` (absolute; the desktop builds it), creating
    /// it only if it does not exist, and qualify it (§1–§3).
    pub(crate) fn open(path: &Path) -> Result<Self, Unqualified> {
        if !path.is_absolute() || path.file_name().is_none() {
            return Err(Unqualified::Path);
        }
        let dir = path.parent().ok_or(Unqualified::Path)?;
        private_directory(dir)?;
        let created = match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(Unqualified::Link),
            Ok(meta) if !meta.is_file() => return Err(Unqualified::NotAStore),
            Ok(_) => false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => create_new(path)?,
            Err(_) => return Err(Unqualified::Path),
        };
        let conn = if created {
            let conn = connect(path)?;
            Self::initialise(&conn)?;
            sync_directory(dir)?;
            conn
        } else {
            // Read before SQLite opens it: a file that is not ours is
            // never opened read-write, so never changed.
            if !header_is_ours(path)? {
                return Err(Unqualified::NotAStore);
            }
            connect(path)?
        };
        Self::qualify(conn)
    }

    /// A new file's schema and identity row, in one transaction; then a
    /// checkpoint, so that the file's own header names the format.
    fn initialise(conn: &Connection) -> Result<(), Unqualified> {
        let store = random_hex()?;
        let created = (|| -> rusqlite::Result<()> {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(&format!("PRAGMA application_id = {APPLICATION_ID}"))?;
            for (_, _, _, sql) in SCHEMA {
                conn.execute_batch(sql)?;
            }
            conn.execute(
                "INSERT INTO identity (one, store, format) VALUES (1, ?1, ?2)",
                params![store, FORMAT],
            )?;
            conn.execute_batch("COMMIT")
        })();
        // A creation that failed leaves a file that is not a store: it is
        // refused from now on, never initialised again in place (EL-1 §9).
        created.map_err(|_| Unqualified::Sqlite)?;
        let checkpoint: rusqlite::Result<(i64, i64, i64)> =
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            });
        match checkpoint {
            Ok((0, _, _)) => Ok(()),
            _ => Err(Unqualified::Sqlite),
        }
    }

    /// §3, in order: the settings were read back by `connect`; then
    /// `integrity_check`, the schema and identity row, the whole chain.
    fn qualify(conn: Connection) -> Result<Self, Unqualified> {
        let application_id: i64 = conn
            .query_row("PRAGMA application_id", [], |row| row.get(0))
            .map_err(|_| Unqualified::Sqlite)?;
        if application_id != i64::from(APPLICATION_ID) {
            return Err(Unqualified::NotAStore);
        }
        if !integrity_ok(&conn) {
            return Err(Unqualified::Integrity);
        }
        if !schema_exact(&conn)? {
            return Err(Unqualified::Schema);
        }
        let store = identity(&conn)?;
        let rows = read_rows(
            &conn,
            &format!("SELECT {ROW_COLUMNS} FROM record ORDER BY seq"),
            &[],
        )
        .ok_or(Unqualified::Chain)?;
        let (last_seq, last_tag) = verify_chain(&store, &rows).ok_or(Unqualified::Chain)?;
        let mut known = Known::default();
        for row in &rows {
            known.take(row);
        }
        let mut inner = Inner {
            conn,
            store,
            session: random_hex()?,
            last_seq,
            last_tag,
            latched: false,
            known,
            owed: VecDeque::new(),
            #[cfg(test)]
            fault: None,
            #[cfg(test)]
            refusing: false,
            #[cfg(test)]
            refuse_next: false,
        };
        // Only now, the whole chain verified: this session, then the
        // reconciliation, before anything is admitted.
        inner.open_session()?;
        inner.reconcile()?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    /// The store's state, under its mutex. A panic while it was held (it
    /// was poisoned) latches it: what that append did cannot be known.
    fn inner(&self) -> MutexGuard<'_, Inner> {
        match self.inner.lock() {
            Ok(inner) => inner,
            Err(poisoned) => {
                let mut inner = poisoned.into_inner();
                inner.latched = true;
                inner
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn is_latched(&self) -> bool {
        self.inner().latched
    }

    /// Append one row (§5): `body` is the record's canonical JSON (tests;
    /// production appends through `gate` and `end`).
    #[cfg(test)]
    pub(crate) fn append(&self, phase: &str, commitment: Option<&str>, body: &str) -> Append {
        let mut inner = self.inner();
        inner.append(phase, commitment, body)
    }

    /// A gating record (§6): every owed record first, in order; refused
    /// while one is still owed or the store is latched. Returns what it
    /// came to and every record written (for the projections).
    pub(crate) fn gate(
        &self,
        record: &EvidenceRecord,
    ) -> (Result<u64, EvidenceUnavailable>, Vec<EvidenceRecord>) {
        let mut inner = self.inner();
        let mut written = Vec::new();
        // Nothing new is admitted while recovery is required (EL-1 §7).
        if inner.latched || inner.known.recovery_required() || !inner.pay_owed(&mut written) {
            return (Err(EvidenceUnavailable), written);
        }
        let owed = Owed::of(record);
        match inner.append(owed.phase, owed.commitment.as_deref(), &owed.body) {
            Append::Committed { seq } | Append::AlreadyPresent { seq } => {
                written.push(owed.record);
                (Ok(seq), written)
            }
            // Not written: the action it permits does not happen (nothing
            // is owed for it).
            _ => (Err(EvidenceUnavailable), written),
        }
    }

    /// An end record (§6): never fails. Every owed record first, in order;
    /// then this one: written, owed, or uncertain.
    pub(crate) fn end(&self, record: &EvidenceRecord) -> (EndStatus, Vec<EvidenceRecord>) {
        let mut inner = self.inner();
        let mut written = Vec::new();
        let owed = Owed::of(record);
        if inner.latched || !inner.pay_owed(&mut written) {
            return (inner.owe(owed), written);
        }
        match inner.append(owed.phase, owed.commitment.as_deref(), &owed.body) {
            Append::Committed { seq } | Append::AlreadyPresent { seq } => {
                written.push(owed.record);
                (EndStatus::Recorded { seq }, written)
            }
            Append::NotCommitted => (inner.owe(owed), written),
            // Latched.
            Append::Uncertain | Append::Conflict | Append::Latched => {
                (EndStatus::Uncertain, written)
            }
        }
    }

    /// What the interface shows (EL-1 §8).
    pub(crate) fn health(&self) -> EvidenceHealth {
        let inner = self.inner();
        EvidenceHealth {
            latched: inner.latched,
            owed: inner.owed.len(),
            recovery_required: inner.known.recovery_required(),
            unsettled: inner.known.unsettled.len(),
            unclean_sessions: inner.known.unclean.len(),
        }
    }

    /// What the owner must acknowledge, if recovery is required: read before
    /// the dialog (EL-1 §7).
    pub(crate) fn recovery(&self) -> Option<Recovery> {
        let inner = self.inner();
        let known = &inner.known;
        if inner.latched || !known.recovery_required() {
            return None;
        }
        let short = |id: &str| id.chars().take(12).collect::<String>();
        let unsettled: Vec<(u64, String)> = known
            .unsettled
            .iter()
            .filter(|(_, seq)| **seq > known.acknowledged)
            .map(|(commitment, seq)| (*seq, short(commitment)))
            .collect();
        let unclean: Vec<(u64, String)> = known
            .unclean
            .iter()
            .filter(|(_, seq)| **seq > known.acknowledged)
            .map(|(session, seq)| (*seq, short(session)))
            .collect();
        let covers_through = unsettled
            .iter()
            .chain(unclean.iter())
            .map(|(seq, _)| *seq)
            .max()?;
        let mut unsettled = unsettled;
        let mut unclean = unclean;
        unsettled.sort();
        unclean.sort();
        Some(Recovery {
            unsettled: unsettled.into_iter().map(|(_, id)| id).collect(),
            unclean_sessions: unclean.into_iter().map(|(_, id)| id).collect(),
            covers_through,
        })
    }

    /// The owner's acknowledgement of everything through `covers_through`
    /// (the statements shown): written after every owed record; the one
    /// record written for an action while recovery is required. It grants
    /// nothing and changes no outcome.
    pub(crate) fn acknowledge(&self, covers_through: u64) -> Result<u64, EvidenceUnavailable> {
        let mut inner = self.inner();
        let mut written = Vec::new();
        if inner.latched || !inner.pay_owed(&mut written) {
            return Err(EvidenceUnavailable);
        }
        let body = statement(
            "acknowledged",
            serde_json::json!({ "covers_through": covers_through }),
        );
        match inner.append("acknowledged", None, &body) {
            Append::Committed { seq } => Ok(seq),
            _ => Err(EvidenceUnavailable),
        }
    }

    /// A clean shutdown's `session_closed` (an end record: it never fails).
    pub(crate) fn close_session(&self) -> EndStatus {
        let mut inner = self.inner();
        let session = inner.session.clone();
        let body = statement("session_closed", serde_json::json!({ "session": session }));
        if inner.latched || !inner.pay_owed(&mut Vec::new()) {
            return EndStatus::Owed;
        }
        match inner.append("session_closed", None, &body) {
            Append::Committed { seq } | Append::AlreadyPresent { seq } => {
                EndStatus::Recorded { seq }
            }
            Append::NotCommitted => EndStatus::Owed,
            _ => EndStatus::Uncertain,
        }
    }
}

/// What the interface shows about the evidence store (EL-1 §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidenceHealth {
    /// The store latched in this process: nothing is admitted until restart.
    pub latched: bool,
    /// End records not yet written.
    pub owed: usize,
    /// An unsettled commitment has not been acknowledged (§7).
    pub recovery_required: bool,
    /// Unsettled statements in the store.
    pub unsettled: usize,
    /// Unclean-session statements in the store.
    pub unclean_sessions: usize,
}

/// The production evidence sink (EL-1 §6): the store's result decides; the
/// projections (the audit trail, the interface's recent history) are
/// written only after it committed, best effort, and change nothing.
pub struct PrimaryEvidence {
    store: EvidenceStore,
    projections: Vec<Arc<dyn EvidenceProjection>>,
}

impl PrimaryEvidence {
    /// Open and qualify the store at `path` (absolute; built by the desktop
    /// from its own state directory, never from a caller's text). An
    /// unqualified store is an error: nothing falls back.
    pub fn open(
        path: &Path,
        projections: Vec<Arc<dyn EvidenceProjection>>,
    ) -> Result<Arc<Self>, Unqualified> {
        Ok(Arc::new(Self {
            store: EvidenceStore::open(path)?,
            projections,
        }))
    }

    pub fn health(&self) -> EvidenceHealth {
        self.store.health()
    }

    /// A clean shutdown: this session's `session_closed` (EL-1 §7).
    pub fn close_session(&self) -> EndStatus {
        self.store.close_session()
    }

    /// What the owner must acknowledge, if recovery is required (the
    /// snapshot, read before the dialog).
    pub(crate) fn recovery(&self) -> Option<Recovery> {
        self.store.recovery()
    }

    /// Only after the owner's native confirmation of exactly `recovery`
    /// (`Authority::resume`).
    pub(crate) fn acknowledge(&self, covers_through: u64) -> Result<u64, EvidenceUnavailable> {
        self.store.acknowledge(covers_through)
    }

    /// Copy what the store wrote, in order, outside its lock.
    fn project(&self, written: &[EvidenceRecord]) {
        for record in written {
            for projection in &self.projections {
                projection.project(record);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn store(&self) -> &EvidenceStore {
        &self.store
    }
}

impl EvidenceSink for PrimaryEvidence {
    fn record(&self, record: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
        let (gated, written) = self.store.gate(record);
        self.project(&written);
        gated
    }

    fn record_end(&self, record: &EvidenceRecord) -> EndStatus {
        let (status, written) = self.store.end(record);
        self.project(&written);
        status
    }

    fn primary(&self) -> Option<&PrimaryEvidence> {
        Some(self)
    }
}

/// Exactly one row, `ok`.
fn integrity_ok(conn: &Connection) -> bool {
    let rows = conn
        .prepare("PRAGMA integrity_check")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<String>, _>>()
        });
    matches!(rows.as_deref(), Ok([only]) if only == "ok")
}

fn schema_exact(conn: &Connection) -> Result<bool, Unqualified> {
    let mut statement = conn
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY name")
        .map_err(|_| Unqualified::Sqlite)?;
    let found = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
        .map_err(|_| Unqualified::Schema)?;
    let mut expected: Vec<(String, String, String, Option<String>)> = SCHEMA
        .iter()
        .map(|(kind, name, table, sql)| {
            (
                kind.to_string(),
                name.to_string(),
                table.to_string(),
                Some(sql.to_string()),
            )
        })
        .collect();
    expected.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(found == expected)
}

/// The identity row: exactly one, this format, a well-formed store id.
fn identity(conn: &Connection) -> Result<String, Unqualified> {
    let mut statement = conn
        .prepare("SELECT one, store, format FROM identity")
        .map_err(|_| Unqualified::Identity)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
        .map_err(|_| Unqualified::Identity)?;
    match rows.as_slice() {
        [(1, store, format)] if format == FORMAT && is_lower_hex(store, 32) => Ok(store.clone()),
        _ => Err(Unqualified::Identity),
    }
}

/// Every row in order: its sequence the next, its `prev` the tag before it,
/// its tag its own. Returns the last sequence and tag.
fn verify_chain(store: &str, rows: &[Row]) -> Option<(u64, String)> {
    let mut last_seq = 0u64;
    let mut last_tag = GENESIS.to_string();
    for row in rows {
        if row.seq != last_seq.checked_add(1)? || row.prev != last_tag || !row.verifies(store) {
            return None;
        }
        last_seq = row.seq;
        last_tag = row.tag.clone();
    }
    Some((last_seq, last_tag))
}

impl Inner {
    fn latch(&mut self) {
        self.latched = true;
    }

    /// Write every owed record, in order. False when one is still owed (it
    /// was not committed, or the store latched).
    fn pay_owed(&mut self, written: &mut Vec<EvidenceRecord>) -> bool {
        while let Some(owed) = self.owed.front() {
            let (phase, body) = (owed.phase, owed.body.clone());
            let commitment = owed.commitment.clone();
            match self.append(phase, commitment.as_deref(), &body) {
                Append::Committed { .. } | Append::AlreadyPresent { .. } => {
                    if let Some(paid) = self.owed.pop_front() {
                        written.push(paid.record);
                    }
                }
                _ => return false,
            }
        }
        true
    }

    /// Keep an end record owed; one more than the list holds latches the
    /// store, and that record is neither written nor kept: uncertain.
    fn owe(&mut self, owed: Owed) -> EndStatus {
        if self.owed.len() >= OWED_CAPACITY {
            self.latch();
            return EndStatus::Uncertain;
        }
        self.owed.push_back(owed);
        EndStatus::Owed
    }

    fn append(&mut self, phase: &str, commitment: Option<&str>, body: &str) -> Append {
        if self.latched {
            return Append::Latched;
        }
        let Some(seq) = self.last_seq.checked_add(1) else {
            self.latch();
            return Append::Uncertain;
        };
        let stored_ms = wall_ms();
        let tag = tag_of(
            &self.store,
            &self.last_tag,
            seq,
            &self.session,
            stored_ms,
            phase,
            commitment,
            body,
        );
        if self.conn.execute_batch("BEGIN IMMEDIATE").is_err() {
            // No transaction began: nothing was written (unless the
            // connection is now somewhere it cannot tell).
            if self.conn.is_autocommit() {
                return Append::NotCommitted;
            }
            self.latch();
            return Append::Uncertain;
        }
        #[cfg(test)]
        if self.refusing || std::mem::take(&mut self.refuse_next) {
            if rolled_back(&self.conn) {
                return Append::NotCommitted;
            }
            self.latch();
            return Append::Uncertain;
        }
        let inserted = self.conn.execute(
            "INSERT INTO record (seq, session, stored_ms, phase, commitment, body, prev, tag) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                i64::try_from(seq).unwrap_or(i64::MAX),
                self.session,
                i64::try_from(stored_ms).unwrap_or(i64::MAX),
                phase,
                commitment,
                body,
                self.last_tag,
                tag,
            ],
        );
        match inserted {
            Ok(1) => {}
            Err(error) if is_unique_violation(&error) => {
                if !rolled_back(&self.conn) {
                    self.latch();
                    return Append::Uncertain;
                }
                return self.already(phase, commitment, body);
            }
            _ => {
                if rolled_back(&self.conn) {
                    return Append::NotCommitted;
                }
                self.latch();
                return Append::Uncertain;
            }
        }
        if self.commit().is_err() {
            self.latch();
            return Append::Uncertain;
        }
        self.known.take(&Row {
            seq,
            session: self.session.clone(),
            stored_ms,
            phase: phase.to_string(),
            commitment: commitment.map(str::to_string),
            body: body.to_string(),
            prev: self.last_tag.clone(),
            tag: tag.clone(),
        });
        self.last_seq = seq;
        self.last_tag = tag;
        Append::Committed { seq }
    }

    /// This process's `session_opened` (§7).
    fn open_session(&mut self) -> Result<(), Unqualified> {
        let body = statement(
            "session_opened",
            serde_json::json!({ "session": self.session }),
        );
        self.statement("session_opened", None, &body)
    }

    /// The one reconciliation (§7), over the whole verified chain, before
    /// anything is admitted: an `unsettled` statement for every commitment
    /// that started and has no end and none yet; an `unclean_session`
    /// statement for every earlier session opened and never closed, and none
    /// yet. A second start appends nothing new.
    fn reconcile(&mut self) -> Result<(), Unqualified> {
        let unsettled: Vec<(String, u64)> = self
            .known
            .started
            .iter()
            .filter(|(commitment, _)| {
                !self.known.finished.contains(*commitment)
                    && !self.known.unsettled.contains_key(*commitment)
            })
            .map(|(commitment, seq)| (commitment.clone(), *seq))
            .collect();
        for (commitment, started) in unsettled {
            let body = statement(
                "unsettled",
                serde_json::json!({
                    "commitment": commitment,
                    "started_seq": started,
                    "text": UNSETTLED_TEXT,
                }),
            );
            self.statement("unsettled", Some(&commitment), &body)?;
        }
        let unclean: Vec<String> = self
            .known
            .opened
            .iter()
            .map(|(session, _)| session.clone())
            .filter(|session| {
                *session != self.session
                    && !self.known.closed.contains(session)
                    && !self.known.unclean.contains_key(session)
            })
            .collect();
        for session in unclean {
            let body = statement("unclean_session", serde_json::json!({ "session": session }));
            self.statement("unclean_session", None, &body)?;
        }
        Ok(())
    }

    /// A statement the qualification needs: written, or the store is not
    /// qualified.
    fn statement(
        &mut self,
        kind: &str,
        commitment: Option<&str>,
        body: &str,
    ) -> Result<(), Unqualified> {
        match self.append(kind, commitment, body) {
            Append::Committed { .. } => Ok(()),
            Append::NotCommitted => Err(Unqualified::Sqlite),
            _ => Err(Unqualified::Latched),
        }
    }

    fn commit(&mut self) -> Result<(), ()> {
        #[cfg(test)]
        match self.fault.take() {
            Some(Fault::FailCommit) => return Err(()),
            Some(Fault::CommitThenFail) => {
                let _committed = self.conn.execute_batch("COMMIT");
                return Err(());
            }
            None => {}
        }
        self.conn.execute_batch("COMMIT").map_err(|_| ())
    }

    /// A once-only record already written: the same body is already present;
    /// another body conflicts (and latches). Its tag is checked first.
    fn already(&mut self, phase: &str, commitment: Option<&str>, body: &str) -> Append {
        let found = read_rows(
            &self.conn,
            &format!("SELECT {ROW_COLUMNS} FROM record WHERE commitment = ?1 AND phase = ?2"),
            &[&commitment, &phase],
        );
        match found.as_deref() {
            Some([row]) if !row.verifies(&self.store) => {
                self.latch();
                Append::Latched
            }
            Some([row]) if row.body == body => Append::AlreadyPresent { seq: row.seq },
            Some([_]) => {
                self.latch();
                Append::Conflict
            }
            _ => {
                self.latch();
                Append::Uncertain
            }
        }
    }
}

fn is_unique_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

#[cfg(test)]
impl EvidenceStore {
    pub(crate) fn plant_fault(&self, fault: Fault) {
        self.inner().fault = Some(fault);
    }

    /// The bodies of the records owed, in order.
    pub(crate) fn owed_bodies(&self) -> Vec<String> {
        self.inner().owed.iter().map(|o| o.body.clone()).collect()
    }

    /// Every insert fails before COMMIT (a full disk) until `false`.
    pub(crate) fn refuse_inserts(&self, refusing: bool) {
        self.inner().refusing = refusing;
    }

    /// Only the next insert fails before COMMIT; the one after succeeds.
    pub(crate) fn refuse_next_insert(&self) {
        self.inner().refuse_next = true;
    }

    /// Hold the store's mutex (a slow store) until the guard is dropped.
    pub(crate) fn hold(&self) -> MutexGuard<'_, impl Sized> {
        self.inner()
    }

    /// Limit the file's size (a full database), on this connection only.
    pub(crate) fn set_max_page_count(&self, pages: u64) {
        let inner = self.inner();
        let _set: i64 = inner
            .conn
            .query_row(&format!("PRAGMA max_page_count = {pages}"), [], |row| {
                row.get(0)
            })
            .expect("max_page_count");
    }

    pub(crate) fn page_count(&self) -> u64 {
        let inner = self.inner();
        let pages: i64 = inner
            .conn
            .query_row("PRAGMA page_count", [], |row| row.get(0))
            .expect("page_count");
        u64::try_from(pages).expect("pages")
    }

    /// One pragma's value, read on the store's own connection.
    pub(crate) fn pragma(&self, name: &str) -> String {
        let inner = self.inner();
        inner
            .conn
            .query_row(&format!("PRAGMA {name}"), [], |row| {
                row.get::<_, rusqlite::types::Value>(0)
            })
            .map(|value| match value {
                rusqlite::types::Value::Integer(n) => n.to_string(),
                rusqlite::types::Value::Text(t) => t,
                other => format!("{other:?}"),
            })
            .expect("pragma")
    }

    /// Run SQL on the store's own connection (a test's probe).
    pub(crate) fn probe_sql(&self, sql: &str) -> rusqlite::Result<()> {
        self.inner()
            .conn
            .query_row(sql, [], |_| Ok(()))
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(()),
                other => Err(other),
            })
    }

    pub(crate) fn rows(&self) -> Vec<Row> {
        let inner = self.inner();
        read_rows(
            &inner.conn,
            &format!("SELECT {ROW_COLUMNS} FROM record ORDER BY seq"),
            &[],
        )
        .expect("rows")
    }

    pub(crate) fn store_id(&self) -> String {
        self.inner().store.clone()
    }

    pub(crate) fn session(&self) -> String {
        self.inner().session.clone()
    }
}

#[cfg(test)]
#[path = "evidence_store/tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "evidence_store/sink_tests.rs"]
mod sink_tests;

#[cfg(test)]
#[path = "evidence_store/recovery_tests.rs"]
mod recovery_tests;
