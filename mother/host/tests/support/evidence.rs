use super::{Fixture, Pty};
use std::process::Command;

/// This reader opens only the uniquely owned test database, after the host
/// process has exited and released its exclusive store lock.
pub fn counts(f: &Fixture) -> [u64; 3] {
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/evidence.py"
        ))
        .arg(f.home.join("broker/p3-evidence/evidence.sqlite3"));
    let mut reader = Pty::start(command);
    reader.expect("PRIMARY CHAIN VERIFIED ");
    assert!(
        reader.finish().success(),
        "independent primary chain validation"
    );
    let row = reader
        .output
        .lines()
        .find_map(|line| line.strip_prefix("PRIMARY CHAIN VERIFIED "))
        .unwrap();
    let numbers: Vec<u64> = row
        .split_whitespace()
        .map(|word| word.parse().unwrap())
        .collect();
    numbers.try_into().expect("Stop, opened and closed counts")
}
