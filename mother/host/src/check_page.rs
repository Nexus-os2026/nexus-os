//! M3-E M-2 (S1a-E, design §5.5, rulings M3E-R1..R6): the Owner's
//! `nexus-mother check-page <file> [--facts <file>]`.
//!
//! A read-only checker of his own file: it never writes, never fixes, calls
//! no model, opens no network and touches no home. It reads the page
//! (at most 65,536 + 1 bytes) and the facts file through Mother's adapter
//! (`input_files`, the M-1 reader: one open, `O_NOFOLLOW | O_NONBLOCK`,
//! regular-file check, bounded read), parses the facts with the M-1 parser,
//! and reports only numbers, hashes and closed codes, never content.
//!
//! Bare, it runs S1a's `check` and V5 and says the facts were not checked.
//! With `--facts` it also runs V3 (`facts_report`) and V4 (the separate
//! reader) against the **supplied** snapshot: a PASS means agreement with
//! that file, not with any consent; acceptance compares the printed facts
//! hash with the consent's. The first failure is reported in V2 → V3 → V4 →
//! V5 order. Exit 0 PASS, 1 REFUSED, 2 usage or a read refusal.

use crate::binary_identity::{running_binary_sha256, SelfIdentityUnavailable};
use crate::inputs::{read_input, sha256_hex, InputError, InputKind, InputValue};
use nexus_governed_control::authority::evidence::escaped;
use nexus_governed_control::inert_page::{
    check, facts_report, policy_hash, ApprovedFacts, FactCode, Page, PageVerdict, MOTHER_CSS,
};
use nexus_mother::adapter::input_files::{self, ReadRefusal};
use nexus_mother::teaching::{t14_passed, t14_refused, LABEL_PASSED, OPEN_LINES};
use nexus_mother::verifier::{email_scan, read_visible_text, visible_facts};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The most page bytes read (design §5.5).
pub const PAGE_MAX_BYTES: usize = 65_536;
/// The one usage line (exit 2).
pub const USAGE: &str = "ERROR usage: nexus-mother check-page <file> [--facts <file>]";
/// The bare contract's facts line.
pub const FACTS_NOT_CHECKED: &str = "facts: not checked (no --facts given)";

/// `check-page`'s arguments (after `check-page`): exactly `<file>
/// [--facts <file>]`. Paths are taken as given (relative or not, UTF-8 or
/// not, M3E-R6 5); nothing is canonicalized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckArgs {
    pub page: PathBuf,
    pub facts: Option<PathBuf>,
}

/// Parse `<file> [--facts <file>]`; anything else is usage.
pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<CheckArgs, &'static str> {
    let args: Vec<OsString> = args.into_iter().collect();
    let path = |a: &OsString| {
        let b = a.as_encoded_bytes();
        (!b.is_empty() && !b.starts_with(b"--")).then(|| PathBuf::from(a))
    };
    match args.as_slice() {
        [page] => Ok(CheckArgs {
            page: path(page).ok_or("usage")?,
            facts: None,
        }),
        [page, flag, facts] if flag == "--facts" => Ok(CheckArgs {
            page: path(page).ok_or("usage")?,
            facts: Some(path(facts).ok_or("usage")?),
        }),
        _ => Err("usage"),
    }
}

/// The binary: run on the process's stdout; the exit code.
pub fn main_entry(args: impl IntoIterator<Item = OsString>) -> u8 {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    run(args, &mut out)
}

/// Check `args` and write the report to `out`; the exit code.
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write) -> u8 {
    run_with(running_binary_sha256, args, out)
}

fn run_with(
    binary: impl FnOnce() -> Result<[u8; 32], SelfIdentityUnavailable>,
    args: impl IntoIterator<Item = OsString>,
    out: &mut dyn Write,
) -> u8 {
    let (lines, code) = report(binary, args);
    for line in &lines {
        if writeln!(out, "{line}").is_err() {
            return 2;
        }
    }
    if out.flush().is_err() {
        return 2;
    }
    code
}

fn read_error(refusal: ReadRefusal, which: &str) -> (Vec<String>, u8) {
    let code = InputError::from(refusal).code();
    (vec![format!("ERROR {code} ({which})")], 2)
}

/// A refusal code, and the page location when it has one.
struct Failure {
    code: String,
    at: Option<(u64, u64)>,
}

fn report(
    binary: impl FnOnce() -> Result<[u8; 32], SelfIdentityUnavailable>,
    args: impl IntoIterator<Item = OsString>,
) -> (Vec<String>, u8) {
    let Ok(args) = parse_args(args) else {
        return (vec![USAGE.to_string()], 2);
    };
    let binary = match binary() {
        Ok(digest) => digest,
        Err(e) => return (vec![format!("ERROR {}", e.code())], 2),
    };
    // Both inputs are read, once each, before anything is checked.
    let page = match input_files::open(&args.page, PAGE_MAX_BYTES).and_then(|f| f.read()) {
        Ok(bytes) => Some(bytes),
        Err(ReadRefusal::TooLarge) => None,
        Err(refusal) => return read_error(refusal, "page"),
    };
    let facts = match &args.facts {
        None => None,
        Some(path) => match read_facts(path) {
            Ok(facts) => Some(facts),
            Err(e) => return (vec![format!("ERROR {} (facts file)", e.code())], 2),
        },
    };
    let mut lines = vec![
        format!("binary sha256 {}", sha256_hex(&binary)),
        format!(
            "policy {}  css {}",
            sha256_hex(&policy_hash()),
            sha256_hex(&Sha256::digest(MOTHER_CSS.as_bytes()).into())
        ),
    ];
    let page = page.as_ref().map(|b| (b.bytes(), b.sha256()));
    let code = assess(&mut lines, page, facts.as_ref(), args.facts.as_deref(), v3);
    (lines, code)
}

/// The V3 check: the policy's facts line, or the first missing fact.
type V3 = fn(&[u8], &Page, &ApprovedFacts) -> Result<String, String>;

/// Every check on inputs already read (`page` is `None` over the cap),
/// appended to `lines`; the exit code.
fn assess(
    lines: &mut Vec<String>,
    page: Option<(&[u8], [u8; 32])>,
    facts: Option<&Facts>,
    facts_path: Option<&Path>,
    v3: V3,
) -> u8 {
    let mut failures: Vec<Failure> = Vec::new();
    let checked = match page {
        None => {
            lines.push(format!(
                "file bytes unavailable (over {PAGE_MAX_BYTES}) sha256 unavailable"
            ));
            lines.push("refused: input_too_large".into());
            failures.push(Failure {
                code: "input_too_large".into(),
                at: None,
            });
            None
        }
        Some((bytes, sha256)) => {
            lines.push(format!(
                "file bytes {} sha256 {}",
                bytes.len(),
                sha256_hex(&sha256)
            ));
            match check(bytes) {
                Ok(tree) => {
                    lines.push(LABEL_PASSED.into());
                    Some((bytes, tree))
                }
                Err(r) => {
                    lines.push(format!(
                        "refused: {} at line {}, column {}",
                        r.code.as_str(),
                        r.line,
                        r.column
                    ));
                    failures.push(Failure {
                        code: r.code.as_str().into(),
                        at: Some((u64::from(r.line), u64::from(r.column))),
                    });
                    None
                }
            }
        }
    };
    lines.push(OPEN_LINES[0].into());
    let mut facts_hash = None;
    match facts {
        None => lines.push(FACTS_NOT_CHECKED.into()),
        Some(Facts { sha256, parsed }) => {
            let approved = parsed.as_ref().ok();
            facts_hash = approved.map(|a| sha256_hex(&a.hash()));
            lines.push(format!(
                "facts file sha256 {}  facts hash {}",
                sha256_hex(sha256),
                facts_hash.as_deref().unwrap_or("unavailable")
            ));
            // A refused page runs neither key, whatever the facts file holds.
            let (v3, v4) = match (&checked, parsed) {
                (None, _) => (None, None),
                (Some(_), Err(e)) => (Some(Err(facts_code(e))), None),
                (Some((bytes, tree)), Ok(a)) => (Some(v3(bytes, tree, a)), Some(v4(bytes, a))),
            };
            lines.push(match &v3 {
                None => "facts present as given: not run".into(),
                Some(Ok(line)) => line.clone(),
                Some(Err(code)) => format!("facts present as given: refused {code}"),
            });
            lines.push(match &v4 {
                None => "visible text: not run".into(),
                Some(None) => "visible text: pass".into(),
                Some(Some(code)) => format!("visible text: refused {code}"),
            });
            for code in [v3.and_then(Result::err), v4.flatten()]
                .into_iter()
                .flatten()
            {
                failures.push(Failure { code, at: None });
            }
        }
    }
    if let Some((_, tree)) = &checked {
        if email_scan(tree).is_some() {
            failures.push(Failure {
                code: "email_not_supported".into(),
                at: None,
            });
        }
    }
    match failures.first() {
        None => {
            lines.push("result: PASS".into());
            if let (Some(path), Some(hash)) = (facts_path, &facts_hash) {
                lines.push(t14_passed(&shown_path(path), hash));
            }
            0
        }
        Some(first) => {
            lines.push(format!("result: REFUSED {}", first.code));
            lines.push(t14_refused(&first.code, first.at));
            1
        }
    }
}

struct Facts {
    sha256: [u8; 32],
    parsed: Result<ApprovedFacts, InputError>,
}

// A read refusal is the caller's `Err` (exit 2); a parse refusal is kept
// for the report (exit 1).
fn read_facts(path: &Path) -> Result<Facts, InputError> {
    let snapshot = read_input(InputKind::Facts, path)?;
    let parsed = snapshot.parse().and_then(|value| match value {
        InputValue::Facts(f) => {
            let hours: Vec<&str> = f.hours.iter().map(String::as_str).collect();
            ApprovedFacts::new(&f.name, &hours, &f.phone).map_err(InputError::Fact)
        }
        _ => Err(InputError::NotUtf8),
    });
    Ok(Facts {
        sha256: snapshot.sha256(),
        parsed,
    })
}

fn facts_code(e: &InputError) -> String {
    match (e, e.line()) {
        (InputError::Fact(code), _) => fact_code(*code),
        (_, Some(line)) => format!("{} (facts file line {line})", e.code()),
        (_, None) => e.code().into(),
    }
}

fn fact_code(code: FactCode) -> String {
    match code {
        FactCode::HoursLineMissing { index } => format!("hours_line_missing:{index}"),
        other => other.as_str().into(),
    }
}

// V3: the policy's facts line on a pass, else the first missing fact.
fn v3(bytes: &[u8], tree: &Page, facts: &ApprovedFacts) -> Result<String, String> {
    facts_report(tree, facts).first_error().map_err(fact_code)?;
    PageVerdict::evaluate(bytes, Some(facts))
        .facts_line()
        .ok_or_else(|| "facts_line_unavailable".to_string())
}

// V4: the separate reader; `None` when every fact is visible.
fn v4(bytes: &[u8], facts: &ApprovedFacts) -> Option<String> {
    let Ok(text) = read_visible_text(bytes) else {
        return Some("reader_unsupported".into());
    };
    let seen = visible_facts(&text, facts);
    if !seen.supported {
        return Some("reader_unsupported".into());
    }
    if !seen.name {
        return Some("name_not_visible".into());
    }
    if let Some(i) = seen.hours.iter().position(|ok| !ok) {
        return Some(format!("hours_line_not_visible:{i}"));
    }
    (!seen.phone).then(|| "phone_not_visible".into())
}

fn shown_path(path: &Path) -> String {
    escaped(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAKERY: &str = include_str!("../../src/oracles/m3a_bakery.html");

    // M3E-R9 (3): V4 vetoes independently. With V3 forced to pass, a page
    // whose name is not the facts' still refuses with V4's code.
    #[test]
    fn s1ae_v4_vetoes_when_v3_is_forced_to_pass() {
        let hours = ["Tuesday to Sunday, 7am to 3pm", "Closed on Mondays"];
        let facts = Facts {
            sha256: [0; 32],
            parsed: Ok(ApprovedFacts::new("Rosa's Cakes", &hours, "555-0142").unwrap()),
        };
        let forced: V3 = |_, _, _| Ok("facts present as given: forced".into());
        let mut lines = Vec::new();
        let page = Some((BAKERY.as_bytes(), [0; 32]));
        let code = assess(
            &mut lines,
            page,
            Some(&facts),
            Some(Path::new("f.txt")),
            forced,
        );
        let tail: Vec<&str> = lines
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(code, 1, "{lines:?}");
        assert_eq!(
            tail,
            [
                "facts present as given: forced",
                "visible text: refused name_not_visible",
                "result: REFUSED name_not_visible",
                t14_refused("name_not_visible", None).as_str(),
            ]
        );
    }

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
