//! M3-E M-2 (S1a-E, design §5.5, rulings M3E-R1..R6): the Owner's
//! `nexus-mother check-page <file> [--facts <file>]`. ORACLE STUB: inert
//! until the implementation commit; it checks nothing and exits 2.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

/// The most page bytes read (design §5.5).
pub const PAGE_MAX_BYTES: usize = 65_536;
/// The one usage line (exit 2).
pub const USAGE: &str = "ERROR usage: nexus-mother check-page <file> [--facts <file>]";
/// The bare contract's facts line.
pub const FACTS_NOT_CHECKED: &str = "facts: not checked (no --facts given)";

/// `check-page`'s arguments (after `check-page`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckArgs {
    pub page: PathBuf,
    pub facts: Option<PathBuf>,
}

/// Parse `<file> [--facts <file>]` (stub: always usage).
pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<CheckArgs, &'static str> {
    let _ = args.into_iter();
    Err("usage")
}

/// The binary: run on the process's stdout; the exit code.
pub fn main_entry(args: impl IntoIterator<Item = OsString>) -> u8 {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    run(args, &mut out)
}

/// Check `args` and write the report to `out` (stub: refuses).
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write) -> u8 {
    run_with(|| Ok([0; 32]), args, out)
}

fn run_with(
    binary: impl FnOnce() -> Result<[u8; 32], crate::binary_identity::SelfIdentityUnavailable>,
    args: impl IntoIterator<Item = OsString>,
    out: &mut dyn Write,
) -> u8 {
    let _ = (binary, args.into_iter());
    let _ = writeln!(out, "ERROR not_implemented");
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary_identity::SelfIdentityUnavailable;

    #[test]
    fn s1ae_self_identity_unavailable_refuses_before_any_read() {
        let mut out = Vec::new();
        let args = ["/nonexistent/m2-page.html"].map(OsString::from);
        let code = run_with(|| Err(SelfIdentityUnavailable), args, &mut out);
        assert_eq!(
            (code, String::from_utf8(out).unwrap()),
            (2, "ERROR self_identity_unavailable\n".to_string())
        );
    }
}
