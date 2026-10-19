//! The evidence store (EL-1 §1–§5), on real files in a private scratch
//! directory. Nothing here opens the owner's state directory.

use super::*;
use std::cell::Cell;
use std::io::{BufRead, BufReader, Write as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

thread_local! {
    /// A synchronous level a test plants in place of `FULL` (this thread).
    static SYNCHRONOUS: Cell<Option<&'static str>> = const { Cell::new(None) };
}

pub(crate) fn planted_synchronous() -> Option<&'static str> {
    SYNCHRONOUS.with(Cell::get)
}

/// A private scratch directory, removed when dropped.
pub(crate) struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    pub(crate) fn new() -> Scratch {
        let mut bytes = [0u8; 8];
        getrandom::getrandom(&mut bytes).unwrap();
        let dir = std::env::temp_dir().join(format!("nexus-p3-evidence-{}", hex::encode(bytes)));
        std::fs::create_dir(&dir).expect("a scratch directory");
        Scratch { dir }
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The store file in its own (not yet existing) directory.
    pub(crate) fn store(&self) -> PathBuf {
        self.dir.join("p3-evidence").join("evidence.sqlite3")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn open(path: &Path) -> EvidenceStore {
    EvidenceStore::open(path).expect("the store qualifies")
}

fn refused(path: &Path) -> Unqualified {
    match EvidenceStore::open(path) {
        Ok(_) => panic!("the store qualified"),
        Err(why) => why,
    }
}

/// A raw connection to a store file the store has closed (fixtures only).
fn raw(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).expect("raw open")
}

fn trigger_sql(name: &str) -> &'static str {
    SCHEMA
        .iter()
        .find(|(_, n, _, _)| *n == name)
        .map(|(_, _, _, sql)| *sql)
        .expect("a trigger")
}

/// TEST FIXTURE: change the file behind the store's back: the trigger is
/// dropped, `sql` runs, the trigger is created again exactly.
fn tamper(path: &Path, trigger: &str, sql: &str) {
    let conn = raw(path);
    conn.execute_batch(&format!(
        "BEGIN; DROP TRIGGER {trigger}; {sql}; {}; COMMIT;",
        trigger_sql(trigger)
    ))
    .expect("the fixture applies");
}

const BODY: &str = r#"{"event_kind":"p3.action.evidence","phase":"finished"}"#;

/// §2: every setting, read back on the store's own connection.
#[test]
fn el1_n2_the_settings_are_read_back() {
    let scratch = Scratch::new();
    let store = open(&scratch.store());
    assert_eq!(store.pragma("journal_mode"), "wal");
    assert_eq!(store.pragma("synchronous"), "2");
    assert_eq!(store.pragma("locking_mode"), "exclusive");
    assert_eq!(store.pragma("wal_autocheckpoint"), "1000");
    assert_eq!(store.pragma("trusted_schema"), "0");
    assert_eq!(store.pragma("application_id"), APPLICATION_ID.to_string());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch.store().parent().unwrap().to_path_buf();
        let mode = std::fs::metadata(dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "the store directory is private");
    }
}

/// §2: SQL `load_extension` is refused on the store's connection.
#[test]
fn el1_n2_sql_load_extension_is_refused() {
    let scratch = Scratch::new();
    let store = open(&scratch.store());
    let error = store
        .probe_sql("SELECT load_extension('/nonexistent/extension')")
        .expect_err("load_extension ran");
    assert!(error.to_string().contains("not authorized"), "{error}");
}

/// §2 (D-30a/b of the lineage, kept): the linked SQLite is 3.45.0, this
/// exact source.
#[test]
fn el1_n2_the_linked_sqlite_is_pinned() {
    assert_eq!(rusqlite::version(), "3.45.0");
    let scratch = Scratch::new();
    let store = open(&scratch.store());
    let inner = store.inner();
    let source: String = inner
        .conn
        .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        source,
        "2024-01-15 17:01:13 1066602b2b1976fe58b5150777cced894af17c803e068f5918390d6915b46e1d"
    );
}

/// §1, §4: a store reopened keeps its records and its identity, and its
/// next row continues the chain.
#[test]
fn el1_n2_a_reopened_store_keeps_its_records() {
    let scratch = Scratch::new();
    let path = scratch.store();
    let (rows, id) = {
        let store = open(&path);
        assert_eq!(
            store.append("run_opened", None, "{\"a\":1}"),
            Append::Committed { seq: 1 }
        );
        assert_eq!(
            store.append("prepared", Some("cmt-a"), "{\"a\":2}"),
            Append::Committed { seq: 2 }
        );
        assert_eq!(
            store.append("approved", Some("cmt-a"), "{\"a\":3}"),
            Append::Committed { seq: 3 }
        );
        (store.rows(), store.store_id())
    };
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].prev, GENESIS);
    assert_eq!(rows[1].prev, rows[0].tag);
    let store = open(&path);
    assert_eq!(store.rows(), rows);
    assert_eq!(store.store_id(), id);
    assert_ne!(
        store.session(),
        rows[0].session,
        "each open is a session of its own"
    );
    assert_eq!(
        store.append("approved", Some("cmt-a"), "{\"a\":4}"),
        Append::Committed { seq: 4 }
    );
    assert_eq!(store.rows()[3].prev, rows[2].tag);
}

fn three_rows(path: &Path) {
    let store = open(path);
    for (n, phase) in ["run_opened", "prepared", "started"].iter().enumerate() {
        assert!(matches!(
            store.append(phase, Some("cmt-t"), &format!("{{\"n\":{n}}}")),
            Append::Committed { .. }
        ));
    }
}

/// §3, §4: a changed body, tag or link, or a deleted row (each made behind
/// the store's back with its trigger dropped and created again, so the
/// schema is exact) fails the chain: the store is unqualified. (A deleted
/// row is one with a successor: removing the last rows leaves a chain that
/// verifies; the tag is not a MAC and such a writer is outside the threat
/// model, EL-1 §9.)
#[test]
fn el1_n2_a_tampered_chain_is_unqualified() {
    for (trigger, sql) in [
        (
            "record_no_update",
            "UPDATE record SET body = '{\"n\":9}' WHERE seq = 2",
        ),
        (
            "record_no_update",
            "UPDATE record SET tag = '00' || substr(tag, 3) WHERE seq = 3",
        ),
        (
            "record_no_update",
            "UPDATE record SET prev = tag WHERE seq = 2",
        ),
        ("record_no_delete", "DELETE FROM record WHERE seq = 2"),
        ("record_no_delete", "DELETE FROM record WHERE seq = 1"),
    ] {
        let scratch = Scratch::new();
        let path = scratch.store();
        three_rows(&path);
        tamper(&path, trigger, sql);
        assert_eq!(refused(&path), Unqualified::Chain, "{sql}");
    }
}

/// §3: the schema is exact: an extra index or a dropped trigger is refused.
#[test]
fn el1_n2_a_changed_schema_is_unqualified() {
    for sql in [
        "CREATE INDEX extra ON record (body)",
        "DROP TRIGGER record_no_delete",
        "CREATE TABLE other (x INTEGER)",
    ] {
        let scratch = Scratch::new();
        let path = scratch.store();
        three_rows(&path);
        raw(&path).execute_batch(sql).unwrap();
        assert_eq!(refused(&path), Unqualified::Schema, "{sql}");
    }
}

/// §3: the identity row is exact.
#[test]
fn el1_n2_a_changed_identity_is_unqualified() {
    for sql in [
        "UPDATE identity SET format = 'p3-evidence/2'",
        "UPDATE identity SET store = 'not-hex'",
    ] {
        let scratch = Scratch::new();
        let path = scratch.store();
        three_rows(&path);
        tamper(&path, "identity_no_update", sql);
        assert_eq!(refused(&path), Unqualified::Identity, "{sql}");
    }
}

/// §1: only an absolute path names a store; nothing is created otherwise.
#[test]
fn el1_n2_a_non_absolute_path_is_refused() {
    for name in [
        "",
        ":memory:",
        "file::memory:",
        "file:evidence.sqlite3",
        "evidence.sqlite3",
        "p3/x",
    ] {
        assert_eq!(refused(Path::new(name)), Unqualified::Path, "{name:?}");
    }
    let scratch = Scratch::new();
    let uri = format!("file:{}", scratch.store().display());
    assert_eq!(refused(Path::new(&uri)), Unqualified::Path);
    assert!(!scratch.store().parent().unwrap().exists());
}

/// §1: a symbolic link at the directory or at the file is refused, and
/// what it points to is not changed.
#[cfg(unix)]
#[test]
fn el1_n2_a_symbolic_link_is_refused() {
    let scratch = Scratch::new();
    let real = scratch.store();
    three_rows(&real);
    let before = std::fs::read(&real).unwrap();
    // The file.
    let linked_file = scratch.path("linked-file").join("evidence.sqlite3");
    std::fs::create_dir(linked_file.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &linked_file).unwrap();
    assert_eq!(refused(&linked_file), Unqualified::Link);
    // The directory.
    let linked_dir = scratch.path("linked-dir");
    std::os::unix::fs::symlink(real.parent().unwrap(), &linked_dir).unwrap();
    assert_eq!(
        refused(&linked_dir.join("evidence.sqlite3")),
        Unqualified::Link
    );
    // A dangling link is not followed to create a file.
    let dangling = scratch.path("dangling-dir").join("evidence.sqlite3");
    std::fs::create_dir(dangling.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(scratch.path("nowhere"), &dangling).unwrap();
    assert_eq!(refused(&dangling), Unqualified::Link);
    assert!(!scratch.path("nowhere").exists());
    assert_eq!(std::fs::read(&real).unwrap(), before);
}

/// §1 (mutant 9): an existing file that is not a store is refused and never
/// initialised in place: its bytes are unchanged and no other file appears.
#[test]
fn el1_n2_an_existing_unidentified_file_is_refused_and_not_modified() {
    let foreign_sqlite = |path: &Path| {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("CREATE TABLE notes (x TEXT); INSERT INTO notes VALUES ('kept');")
            .unwrap();
    };
    let foreign_wal = |path: &Path| {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; CREATE TABLE notes (x TEXT); \
             INSERT INTO notes VALUES ('kept'); PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .unwrap();
    };
    type Case<'a> = (&'a str, &'a dyn Fn(&Path));
    let cases: [Case; 5] = [
        ("empty", &|path| std::fs::write(path, b"").unwrap()),
        ("bytes", &|path| {
            std::fs::write(path, b"not a database at all").unwrap()
        }),
        ("header only", &|path| {
            std::fs::write(path, b"SQLite format 3\0").unwrap()
        }),
        ("sqlite", &foreign_sqlite),
        ("sqlite wal", &foreign_wal),
    ];
    for (name, make) in cases {
        let scratch = Scratch::new();
        let path = scratch.store();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        make(&path);
        let before = std::fs::read(&path).unwrap();
        let listed = || {
            let mut names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            names.sort();
            names
        };
        let files = listed();
        assert_eq!(refused(&path), Unqualified::NotAStore, "{name}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "{name}: the file changed"
        );
        assert_eq!(listed(), files, "{name}: a file appeared");
    }
}

/// §1: the file is created only when it does not exist; a store that
/// exists is opened, never created again or truncated.
#[test]
fn el1_n2_create_new_never_truncates() {
    let scratch = Scratch::new();
    let path = scratch.store();
    three_rows(&path);
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 0);
    assert!(
        !create_new(&path).unwrap(),
        "an existing file is not created"
    );
    assert_eq!(std::fs::metadata(&path).unwrap().len(), size);
    let store = open(&path);
    assert_eq!(store.rows().len(), 3);
}

/// §5: a once-only record written again with the same body is already
/// present; with another body it conflicts and latches the store. A phase
/// that can repeat is appended each time.
#[test]
fn el1_n2_committed_already_present_and_conflict() {
    let scratch = Scratch::new();
    let store = open(&scratch.store());
    assert_eq!(
        store.append("finished", Some("cmt-1"), BODY),
        Append::Committed { seq: 1 }
    );
    assert_eq!(
        store.append("finished", Some("cmt-1"), BODY),
        Append::AlreadyPresent { seq: 1 }
    );
    assert_eq!(
        store.append("finished", Some("cmt-2"), BODY),
        Append::Committed { seq: 2 }
    );
    assert_eq!(
        store.append("approved", Some("cmt-1"), BODY),
        Append::Committed { seq: 3 }
    );
    assert_eq!(
        store.append("approved", Some("cmt-1"), BODY),
        Append::Committed { seq: 4 }
    );
    assert!(!store.is_latched());
    assert_eq!(
        store.append("finished", Some("cmt-1"), "{\"other\":1}"),
        Append::Conflict
    );
    assert!(store.is_latched());
    assert_eq!(store.append("run_opened", None, BODY), Append::Latched);
    assert_eq!(store.rows().len(), 4);
}

/// §5: a failed tag check of the row a once-only append decides on latches
/// the store.
#[test]
fn el1_n2_a_row_whose_tag_fails_latches_the_store() {
    let scratch = Scratch::new();
    let store = open(&scratch.store());
    assert_eq!(
        store.append("finished", Some("cmt-1"), BODY),
        Append::Committed { seq: 1 }
    );
    {
        let inner = store.inner();
        inner
            .conn
            .execute_batch(&format!(
                "DROP TRIGGER record_no_update; \
                 UPDATE record SET stored_ms = stored_ms + 1 WHERE seq = 1; {};",
                trigger_sql("record_no_update")
            ))
            .unwrap();
    }
    assert_eq!(
        store.append("finished", Some("cmt-1"), BODY),
        Append::Latched
    );
    assert!(store.is_latched());
}

/// §5: a full database fails the insert before COMMIT; the transaction
/// rolls back, nothing is written, the store is not latched, and the next
/// append continues the chain.
#[test]
fn el1_n2_a_full_database_is_not_committed() {
    let scratch = Scratch::new();
    let store = open(&scratch.store());
    assert_eq!(
        store.append("run_opened", None, BODY),
        Append::Committed { seq: 1 }
    );
    store.set_max_page_count(store.page_count());
    let big = format!("{{\"pad\":\"{}\"}}", "x".repeat(64 * 1024));
    assert_eq!(store.append("run_opened", None, &big), Append::NotCommitted);
    assert!(!store.is_latched());
    assert_eq!(store.rows().len(), 1);
    store.set_max_page_count(1 << 30);
    assert_eq!(
        store.append("run_opened", None, &big),
        Append::Committed { seq: 2 }
    );
    assert_eq!(store.rows()[1].prev, store.rows()[0].tag);
}

/// §5: a COMMIT that fails latches the store (Uncertain), whether or not the
/// row reached the file; after a restart the file qualifies and holds
/// exactly what committed.
#[test]
fn el1_n2_a_failed_commit_is_uncertain_and_latches() {
    for (fault, kept) in [(Fault::FailCommit, false), (Fault::CommitThenFail, true)] {
        let scratch = Scratch::new();
        let path = scratch.store();
        {
            let store = open(&path);
            assert_eq!(
                store.append("run_opened", None, BODY),
                Append::Committed { seq: 1 }
            );
            store.plant_fault(fault);
            assert_eq!(
                store.append("started", Some("cmt-u"), BODY),
                Append::Uncertain
            );
            assert!(store.is_latched());
            assert_eq!(store.append("run_opened", None, BODY), Append::Latched);
        }
        let store = open(&path);
        let rows = store.rows();
        assert_eq!(rows.len(), if kept { 2 } else { 1 }, "{fault:?}");
    }
}

/// §4: the unique index covers exactly the once-only phases of the N0
/// inventory (receipt 002, section C).
#[test]
fn el1_n2_the_unique_index_covers_the_once_only_phases() {
    let (_, _, _, sql) = SCHEMA
        .iter()
        .find(|(kind, _, _, _)| *kind == "index")
        .unwrap();
    let listed = sql
        .split_once("phase IN (")
        .unwrap()
        .1
        .trim_end_matches(')');
    let phases: Vec<&str> = listed.split(", ").map(|p| p.trim_matches('\'')).collect();
    assert_eq!(
        phases,
        [
            "prepared",
            "authorized",
            "started",
            "finished",
            "expired",
            "revoked",
            "denied",
            "approval_declined",
            "unsettled"
        ]
    );
    for repeating in ["approved", "credential_released"] {
        assert!(!phases.contains(&repeating));
    }
}

/// One connection owns the file: a second store cannot open it while the
/// first is alive.
#[test]
fn el1_n2_a_held_store_is_busy() {
    let scratch = Scratch::new();
    let path = scratch.store();
    let store = open(&path);
    assert_eq!(refused(&path), Unqualified::Busy);
    drop(store);
    open(&path);
}

/// §3 (mutant 7): `integrity_check` runs at open: a corrupted index (the
/// table, the schema and the chain intact) is refused.
#[test]
fn el1_n2_integrity_check_runs_at_open() {
    let scratch = Scratch::new();
    let path = scratch.store();
    let marker = "cmt-integrity-marker-0123456789";
    {
        let store = open(&path);
        assert!(matches!(
            store.append("finished", Some(marker), BODY),
            Append::Committed { .. }
        ));
    }
    let (page_size, root): (i64, i64) = {
        let conn = raw(&path);
        let page_size = conn
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .unwrap();
        let root = conn
            .query_row(
                "SELECT rootpage FROM sqlite_schema WHERE name = 'record_once'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        (page_size, root)
    };
    let mut bytes = std::fs::read(&path).unwrap();
    let start = usize::try_from((root - 1) * page_size).unwrap();
    let page = &mut bytes[start..start + usize::try_from(page_size).unwrap()];
    let at = page
        .windows(marker.len())
        .position(|w| w == marker.as_bytes())
        .expect("the index entry");
    page[at + marker.len() - 1] = b'X';
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(refused(&path), Unqualified::Integrity);
}

/// §2 (mutant 8): `synchronous` is read back: a level that did not take
/// refuses the store.
#[test]
fn el1_n2_synchronous_is_read_back() {
    let scratch = Scratch::new();
    SYNCHRONOUS.with(|s| s.set(Some("NORMAL")));
    let opened = EvidenceStore::open(&scratch.store());
    SYNCHRONOUS.with(|s| s.set(None));
    assert_eq!(opened.err(), Some(Unqualified::Settings("synchronous")));
}

const CHILD: &str = "NEXUS_P3_EVIDENCE_STORE_CHILD";

/// The child of the crash test: appends in a loop, reporting each committed
/// row; it is killed by the parent. Without its task it does nothing.
#[test]
#[ignore = "run by el1_n2_a_killed_process_keeps_every_committed_row as a child process"]
fn el1_n2_crash_child() {
    let Ok(path) = std::env::var(CHILD) else {
        return;
    };
    let store = open(Path::new(&path));
    let mut out = std::io::stdout().lock();
    for n in 0..100_000u64 {
        let body = format!("{{\"n\":{n},\"pad\":\"{}\"}}", "p".repeat(512));
        match store.append("approved", Some("cmt-crash"), &body) {
            Append::Committed { seq } => {
                writeln!(out, "COMMITTED {seq} {n}").unwrap();
                out.flush().unwrap();
            }
            other => panic!("{other:?}"),
        }
    }
}

/// §5, EL-1 §9 A-2: a process killed (SIGKILL) in the middle of a loop of
/// appends leaves a file that qualifies on reopen and holds every row it
/// reported committed, with its body.
#[test]
fn el1_n2_a_killed_process_keeps_every_committed_row() {
    let scratch = Scratch::new();
    let path = scratch.store();
    drop(open(&path));
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "evidence_store::tests::el1_n2_crash_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, &path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(at) = line.find("COMMITTED ") {
                let mut parts = line[at + "COMMITTED ".len()..].split(' ');
                let seq: u64 = parts.next().unwrap().parse().unwrap();
                let n: u64 = parts.next().unwrap().trim().parse().unwrap();
                if tx.send((seq, n)).is_err() {
                    break;
                }
            }
        }
    });
    let mut committed = Vec::new();
    while committed.len() < 200 {
        committed.push(
            rx.recv_timeout(Duration::from_secs(120))
                .expect("the child commits"),
        );
    }
    // Killed (SIGKILL) through its own handle, mid-loop, then reaped.
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success(), "the child was killed, not finished");
    drop(rx);
    let _ = reader.join();
    let store = open(&path);
    let rows = store.rows();
    for (seq, n) in committed {
        let row = &rows[usize::try_from(seq - 1).unwrap()];
        assert_eq!(row.seq, seq);
        assert!(row.body.starts_with(&format!("{{\"n\":{n},")), "{seq}");
    }
}
