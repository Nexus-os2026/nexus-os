//! The schema, the file's settings and the store's ownership of its file.

use super::*;

fn sql_master(conn: &Connection) -> Vec<String> {
    let mut statement = conn
        .prepare("SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY rowid")
        .unwrap();
    let rows = statement.query_map([], |row| row.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

#[test]
fn the_schema_is_exactly_the_one_open_expects() {
    let scratch = Scratch::new();
    let store = Store::create(&scratch.path("l.db"), LIMITS).unwrap();
    assert_eq!(sql_master(&store.conn), SCHEMA);
}

/// No constraint can fail at COMMIT (section 4.4): nothing is declared
/// deferrable, there is no foreign key to defer, and deferral is off.
#[test]
fn no_constraint_is_deferred() {
    let scratch = Scratch::new();
    let store = Store::create(&scratch.path("l.db"), LIMITS).unwrap();
    for statement in sql_master(&store.conn) {
        let upper = statement.to_ascii_uppercase();
        assert!(!upper.contains("DEFERR"), "{statement}");
        assert!(!upper.contains("REFERENCES"), "{statement}");
    }
    for table in ["ledger", "meta"] {
        let count: i64 = store
            .conn
            .query_row(
                &format!("SELECT count(*) FROM pragma_foreign_key_list('{table}')"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
    let defer: i64 = store
        .conn
        .query_row("PRAGMA defer_foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(defer, 0);
}

#[test]
fn the_file_is_wal_with_a_full_sync_and_an_exclusive_lock() {
    let scratch = Scratch::new();
    let store = Store::create(&scratch.path("l.db"), LIMITS).unwrap();
    let get = |pragma: &str| -> String {
        store
            .conn
            .query_row(&format!("PRAGMA {pragma}"), [], |row| {
                row.get::<_, rusqlite::types::Value>(0)
            })
            .map(|value| match value {
                rusqlite::types::Value::Integer(n) => n.to_string(),
                rusqlite::types::Value::Text(text) => text,
                other => format!("{other:?}"),
            })
            .unwrap()
    };
    assert_eq!(get("journal_mode"), "wal");
    assert_eq!(get("synchronous"), "2", "FULL");
    assert_eq!(get("locking_mode"), "exclusive");
    assert_eq!(get("fullfsync"), "1");
    assert_eq!(get("checkpoint_fullfsync"), "1");
    assert_eq!(get("trusted_schema"), "0");
    assert_eq!(get("foreign_keys"), "0");
}

#[test]
fn the_ledger_and_meta_are_append_only() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let mut ticket = store.health().arm().unwrap();
    let record = store.fact(b"f", None, b"content");
    assert_eq!(
        store.append_facts(&mut ticket, &[record]).unwrap(),
        FactOutcome::Committed
    );
    ticket.confirm();
    for sql in [
        "UPDATE ledger SET body = x'00'",
        "DELETE FROM ledger",
        "UPDATE meta SET v = x'00'",
        "DELETE FROM meta",
    ] {
        let error = store.conn.execute(sql, []).unwrap_err();
        assert_eq!(
            error.sqlite_error_code(),
            Some(ErrorCode::ConstraintViolation),
            "{sql}"
        );
    }
}

#[test]
fn create_refuses_a_path_that_exists() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    std::fs::write(&path, b"").unwrap();
    assert!(matches!(
        Store::create(&path, LIMITS),
        Err(StoreError::Path)
    ));
}

#[test]
fn open_refuses_a_missing_file_and_creates_none() {
    let scratch = Scratch::new();
    let path = scratch.path("missing.db");
    assert!(matches!(Store::open(&path, LIMITS), Err(StoreError::Path)));
    assert!(!path.exists());
}

/// D-30d: only an absolute path is a store path. A `file:` name, an
/// in-memory name, an empty name and any other relative name are refused
/// before anything is opened or created.
#[test]
fn only_an_absolute_path_is_a_store_path() {
    let scratch = Scratch::new();
    let inside = scratch.path("l.db");
    let uri = format!("file:{}", inside.display());
    for name in [
        "",
        ":memory:",
        "file::memory:",
        "file:l.db",
        uri.as_str(),
        "l.db",
    ] {
        assert!(
            matches!(absolute(Path::new(name)), Err(StoreError::Path)),
            "{name:?}"
        );
    }
    assert!(absolute(&inside).is_ok());
    assert!(matches!(
        Store::create(Path::new(&uri), LIMITS),
        Err(StoreError::Path)
    ));
    assert!(matches!(
        Store::open(Path::new(&uri), LIMITS),
        Err(StoreError::Path)
    ));
    assert!(!inside.exists());
}

/// D-30d's control: this build parses a `file:` name as a URI although the
/// open flag is not passed, so the flag is no safeguard and the name check
/// above is the one that counts.
#[test]
fn this_build_parses_a_file_name_as_a_uri() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    drop(Store::create(&path, LIMITS).unwrap());
    let conn = Connection::open_with_flags(
        format!("file:{}?mode=ro", path.display()),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("the name is parsed as a URI naming the store file");
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM meta", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 2);
}

/// One connection owns the file: a second store cannot open it while the
/// first is alive, and can once it is gone.
#[test]
fn a_second_store_cannot_open_a_held_file() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let store = Store::create(&path, LIMITS).unwrap();
    assert!(matches!(Store::open(&path, LIMITS), Err(StoreError::Busy)));
    drop(store);
    Store::open(&path, LIMITS).unwrap();
}

#[test]
fn a_changed_schema_is_refused() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    drop(Store::create(&path, LIMITS).unwrap());
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("CREATE INDEX extra ON ledger (body)")
        .unwrap();
    drop(raw);
    assert!(matches!(
        Store::open(&path, LIMITS),
        Err(StoreError::Corrupt)
    ));
}

#[test]
fn a_dropped_trigger_is_refused() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    drop(Store::create(&path, LIMITS).unwrap());
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("DROP TRIGGER ledger_no_update").unwrap();
    drop(raw);
    assert!(matches!(
        Store::open(&path, LIMITS),
        Err(StoreError::Corrupt)
    ));
}

#[test]
fn a_file_that_is_not_a_database_is_refused() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    std::fs::write(&path, vec![0x5a; 8192]).unwrap();
    assert!(matches!(
        Store::open(&path, LIMITS),
        Err(StoreError::Corrupt)
    ));
}

/// A symbolic link at the store's path is refused, on open and on create:
/// the store is the file at the path, never one a link leads to.
#[cfg(unix)]
#[test]
fn a_symbolic_link_is_refused() {
    let scratch = Scratch::new();
    let real = scratch.path("real.db");
    drop(Store::create(&real, LIMITS).unwrap());
    let link = scratch.path("link.db");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(Store::open(&link, LIMITS).is_err());
    assert!(matches!(
        Store::create(&link, LIMITS),
        Err(StoreError::Path)
    ));
    let dangling = scratch.path("dangling.db");
    std::os::unix::fs::symlink(scratch.path("nowhere.db"), &dangling).unwrap();
    assert!(Store::create(&dangling, LIMITS).is_err());
    assert!(!scratch.path("nowhere.db").exists());
}
