//! The file journal: lines survive a reopen, a torn last line and a link
//! are refused, and a coordinator over a tampered file opens nothing.

use super::FileJournal;
use crate::journal::{JournalError, JournalStore};
use std::path::PathBuf;

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dir(name: &str) -> Dir {
    let dir = std::env::temp_dir().join(format!("nexus-mother-m1-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Dir(dir)
}

#[test]
fn file_journal_lines_survive_a_reopen() {
    let d = dir("file-reopen");
    let path = d.0.join("journal.jsonl");
    let mut journal = FileJournal::open(&path).unwrap();
    assert_eq!(journal.load().unwrap(), Vec::<String>::new());
    journal.append("{\"a\":1}").unwrap();
    journal.append("{\"b\":2}").unwrap();
    drop(journal);
    let journal = FileJournal::open(&path).unwrap();
    assert_eq!(journal.load().unwrap(), vec!["{\"a\":1}", "{\"b\":2}"]);
}

#[test]
fn file_journal_a_torn_last_line_is_refused() {
    let d = dir("file-torn");
    let path = d.0.join("journal.jsonl");
    let mut journal = FileJournal::open(&path).unwrap();
    journal.append("{\"a\":1}").unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(b"{\"b\":");
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(
        FileJournal::open(&path).unwrap().load(),
        Err(JournalError::Malformed { line: 2 })
    ));
}

#[test]
fn file_journal_a_line_with_a_newline_is_refused() {
    let d = dir("file-newline");
    let mut journal = FileJournal::open(&d.0.join("journal.jsonl")).unwrap();
    assert!(journal.append("one\ntwo").is_err());
}

#[cfg(unix)]
#[test]
fn file_journal_a_link_is_refused() {
    let d = dir("file-link");
    let target = d.0.join("elsewhere.jsonl");
    std::fs::write(&target, b"").unwrap();
    let link = d.0.join("journal.jsonl");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(FileJournal::open(&link).is_err());
}

#[test]
fn file_journal_a_poisoned_journal_is_refused() {
    let d = dir("file-poison");
    let path = d.0.join("journal.jsonl");
    let mut journal = FileJournal::open(&path).unwrap();
    journal.append("{\"a\":1}").unwrap();
    journal.poison().unwrap();
    assert!(d.0.join("journal.jsonl.poisoned").exists());
    let reopened = FileJournal::open(&path).unwrap();
    assert_eq!(reopened.load(), Err(JournalError::Poisoned));
}
