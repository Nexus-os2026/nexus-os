//! M2d-B1 unit oracles for the Files domain (design v2 §6.1 steps 3-4,
//! §6.10 D-O12, D-O13 at the broker, D-O17; §6.11 MD-6), pre-registered
//! before the domain exists. Each refusal table has its accepted twin, so a
//! rule that refuses everything fails.

use super::{
    content_ok, display, line_count, name_ok, parameters_digest, Components, FileCreate,
    WorkspaceInfo, MAX_FILE_BYTES, MAX_FILE_LINES,
};
use crate::authority::commitment::MAX_SUMMARY_LINES;
use crate::authority::evidence::{is_plain, quoted, shown};
use crate::authority::ids::{Digest, GrantId, RunId};
use crate::authority::run::{AdmittedWorkspace, ConsentKind, FilesAdmission};
use crate::authority::AuthorityError;
use std::collections::BTreeSet;

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn admission() -> FilesAdmission {
    FilesAdmission {
        contract_hash: HASH.into(),
        request: "msn-000001/1/1".into(),
        evidence_seq: 42,
        kind: ConsentKind::Mission,
        consented_wall_ms: 1_793_750_400_000,
        workspace: AdmittedWorkspace {
            id: "2f1c3e4d-5a6b-4c7d-8e9f-0a1b2c3d4e5f".into(),
            identity: Digest::of("m2d.b1.identity", &[b"ws"]),
        },
    }
}

fn workspace() -> WorkspaceInfo {
    WorkspaceInfo {
        id: "2f1c3e4d-5a6b-4c7d-8e9f-0a1b2c3d4e5f".into(),
        name: "rosa-site".into(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    hex::encode(Sha256::digest(bytes))
}

// ---------------------------------------------------------------- names

/// §6.1 step 3: `[A-Za-z0-9_-][A-Za-z0-9._-]{0,63}`, not starting with `.`
/// or the kernel's temp prefix, one root-level component (D-O12's names).
#[test]
fn m2d_b1_d_o12_a_file_names() {
    let longest = "x".repeat(64);
    for name in [
        "a",
        "0",
        "-x",
        "_x",
        "index.html",
        "style.css",
        "A_b-c.d.txt",
        "a..b",
        longest.as_str(),
    ] {
        assert!(name_ok(name), "{name:?} is a valid name");
    }
    let too_long = "x".repeat(65);
    for name in [
        "",
        ".",
        "..",
        ".x",
        ".nexus-coding-run-tmp-0",
        "a/b",
        "/a",
        "a\\b",
        "a b",
        "a:b",
        "é.txt",
        "a\u{202e}b",
        "a\nb",
        "a\0b",
        too_long.as_str(),
    ] {
        assert!(!name_ok(name), "{name:?} is refused");
    }
}

// ---------------------------------------------------------------- content

/// §6.1 step 3: UTF-8 (a `String`), no NUL, at most 8 KiB and 150 lines.
#[test]
fn m2d_b1_d_o12_b_file_content() {
    assert_eq!(MAX_FILE_BYTES, 8 * 1024);
    assert_eq!(MAX_FILE_LINES, 150);
    assert_eq!(line_count(""), 0);
    assert_eq!(line_count("a"), 1);
    assert_eq!(line_count("a\n"), 1);
    assert_eq!(line_count("a\nb"), 2);
    assert_eq!(line_count("a\n\n"), 2);

    for content in [
        String::new(),
        "<h1>Rosa's</h1>\n".to_string(),
        "x".repeat(MAX_FILE_BYTES),
        "line\n".repeat(MAX_FILE_LINES),
        "line\n".repeat(MAX_FILE_LINES - 1) + "last",
        "tab\there\r\n".to_string(),
    ] {
        assert_eq!(content_ok(&content), Ok(()), "{} bytes", content.len());
    }
    for content in [
        "x".repeat(MAX_FILE_BYTES + 1),
        "line\n".repeat(MAX_FILE_LINES + 1),
        "line\n".repeat(MAX_FILE_LINES) + "one more",
        "nul \0 inside".to_string(),
    ] {
        assert!(
            matches!(content_ok(&content), Err(AuthorityError::InvalidAction(_))),
            "{} bytes refused",
            content.len()
        );
    }
}

// ---------------------------------------------------------------- display

/// §6.1 step 4: the broker's facts and the complete file, escaped: the
/// contract hash in full, the consent's record, the workspace's name and
/// id, the name, the exact bytes and lines, the full sha256, then exactly
/// `quoted("New file, complete", content)`; every line plain and at most
/// `MAX_SUMMARY_LINES` of them.
#[test]
fn m2d_b1_display_shows_every_fact_and_the_complete_file() {
    let content = "<!doctype html>\n<h1>Rosa's bakery</h1>\n\tFresh bread\n";
    let lines = display(&admission(), &workspace(), "index.html", content).unwrap();

    assert_eq!(lines[0], format!("Mission contract: {HASH}"));
    let text = lines.join("\n");
    for fact in [
        "record 42".to_string(),
        "msn-000001/1/1".to_string(),
        "rosa-site".to_string(),
        "2f1c3e4d-5a6b-4c7d-8e9f-0a1b2c3d4e5f".to_string(),
        "File: index.html".to_string(),
        format!("{} bytes", content.len()),
        format!("{} lines", line_count(content)),
        "ends with a newline: yes".to_string(),
        format!("SHA-256: {}", sha256_hex(content.as_bytes())),
    ] {
        assert!(text.contains(&fact), "{fact:?} is shown in\n{text}");
    }
    let tail = quoted("New file, complete", content);
    assert!(
        lines.ends_with(&tail),
        "the complete file ends the display:\n{text}"
    );
    assert!(lines.len() <= MAX_SUMMARY_LINES);
    assert!(lines.iter().all(|line| is_plain(line)));

    let unterminated = display(&admission(), &workspace(), "a.txt", "no newline").unwrap();
    assert!(unterminated.join("\n").contains("ends with a newline: no"));
}

/// D-O13 at the broker: CSI, OSC 52, C1, CR, BEL, bidi, zero-width and a
/// combining stack in the content are escaped: no raw control and no
/// character that is not shown; no user-perceived character carries more
/// than three code points; every content line is `│`-prefixed, so a forged
/// header row is never a row of the dialog's own.
#[test]
fn m2d_b1_d_o13_display_escapes_everything_hidden() {
    let content = concat!(
        "\u{1b}[31mred\u{1b}[0m\n",
        "\u{1b}]52;c;bm90IGEgY29kZQ==\u{7}\n",
        "c1 \u{9b}31m\n",
        "carriage\rreturn bell\u{7}\n",
        "bidi \u{202e}txt.exe\n",
        "zero\u{200b}width\n",
        "e\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\n",
        "Binding: 0123456789ab\n",
        "Expires in: 999 s\n",
    );
    assert_eq!(content_ok(content), Ok(()));
    let lines = display(&admission(), &workspace(), "page.html", content).unwrap();
    use unicode_segmentation::UnicodeSegmentation;
    for line in &lines {
        assert!(line.chars().all(shown), "a hidden character in {line:?}");
        for cluster in line.graphemes(true) {
            assert!(cluster.chars().count() <= 3, "a stack in {line:?}");
        }
    }
    let start = lines
        .iter()
        .position(|line| line == "New file, complete:")
        .expect("the content's label");
    assert!(lines[start + 1..]
        .iter()
        .all(|line| line.starts_with("│ ") || line.starts_with("│↳ ")));
    for header in [
        "Effect:",
        "Operation:",
        "Target:",
        "Acting for:",
        "Run:",
        "Commitment:",
        "Binding:",
        "Expires in:",
    ] {
        assert!(
            lines.iter().all(|line| !line.starts_with(header)),
            "{header} is never a row of the summary"
        );
    }
}

/// §6.1 step 4: more than `MAX_SUMMARY_LINES` lines, as escaped, is
/// refused whole (`change_too_large_to_show`), never cut; the largest
/// ordinary file is shown.
#[test]
fn m2d_b1_d_o12_c_too_large_to_show_is_refused() {
    // 8,000 DEL bytes: valid text, one line, about 517 lines escaped.
    let hidden = "\u{7f}".repeat(8_000);
    assert_eq!(content_ok(&hidden), Ok(()));
    assert_eq!(
        display(&admission(), &workspace(), "a.txt", &hidden),
        Err(AuthorityError::ChangeTooLargeToShow)
    );
    assert_eq!(
        AuthorityError::ChangeTooLargeToShow.class(),
        "change_too_large_to_show"
    );

    let largest = format!("{}\n", "x".repeat(50)).repeat(MAX_FILE_LINES);
    assert_eq!(content_ok(&largest), Ok(()));
    let lines = display(&admission(), &workspace(), "a.txt", &largest).unwrap();
    assert!(lines.len() <= MAX_SUMMARY_LINES);
    assert!(lines.ends_with(&quoted("New file, complete", &largest)));
}

/// The intent is data with exactly two fields, and its `Debug` shows
/// neither the name nor the content (only sizes).
#[test]
fn m2d_b1_file_create_is_closed_data() {
    let create: FileCreate =
        serde_json::from_str(r#"{"name":"index.html","content":"NXCANARY-in-debug"}"#).unwrap();
    assert_eq!(create.name, "index.html");
    assert!(
        serde_json::from_str::<FileCreate>(r#"{"name":"a","content":"b","path":"../x"}"#).is_err()
    );
    let debug = format!("{create:?}");
    assert!(!debug.contains("NXCANARY"), "{debug}");
    assert!(!debug.contains("index.html"), "{debug}");
}

// ---------------------------------------------------------------- digest

fn components_with(admission: &FilesAdmission, name: &str, vary: usize) -> Digest {
    let mut broker_run = [1u8; 16];
    let mut coding_run = [2u8; 16];
    let mut grant = [3u8; 16];
    let mut binding_agent = [4u8; 16];
    let mut binding_run = [5u8; 16];
    let mut review_hash = [6u8; 32];
    match vary {
        1 => broker_run[15] ^= 1,
        2 => coding_run[0] ^= 1,
        3 => grant[7] ^= 1,
        4 => binding_agent[3] ^= 1,
        5 => binding_run[9] ^= 1,
        6 => review_hash[31] ^= 1,
        7 => review_hash[0] ^= 1,
        _ => {}
    }
    let broker_run = RunId::parse(&format!("run-{}", hex::encode(broker_run))).unwrap();
    let grant = GrantId::parse(&format!("grant-{}", hex::encode(grant))).unwrap();
    parameters_digest(&Components {
        admission,
        broker_run,
        coding_run,
        grant,
        binding_agent,
        binding_run,
        review_hash,
        name,
    })
}

/// D-O17 (MD-6): `parameters_digest` is a pure function of its components,
/// and changing only one of them (each admission field, the broker run, the
/// coding run, the Files grant, either half of the kernel binding, the name,
/// and the review hash at either end) changes it.
#[test]
fn m2d_b1_d_o17_every_component_moves_the_parameters_digest() {
    let base = admission();
    let same = components_with(&base, "index.html", 0);
    assert_eq!(same, components_with(&admission(), "index.html", 0));

    let mut digests = vec![same];
    for vary in 1..=7 {
        digests.push(components_with(&base, "index.html", vary));
    }
    digests.push(components_with(&base, "index.htm", 0));
    let mut edits: Vec<FilesAdmission> = Vec::new();
    let mut a = admission();
    a.contract_hash = HASH.replace('0', "f");
    edits.push(a);
    let mut a = admission();
    a.request = "msn-000001/1/2".into();
    edits.push(a);
    let mut a = admission();
    a.evidence_seq = 43;
    edits.push(a);
    let mut a = admission();
    a.kind = ConsentKind::Resume;
    edits.push(a);
    let mut a = admission();
    a.consented_wall_ms += 1;
    edits.push(a);
    let mut a = admission();
    a.workspace.id = "2f1c3e4d-5a6b-4c7d-8e9f-0a1b2c3d4e50".into();
    edits.push(a);
    let mut a = admission();
    a.workspace.identity = Digest::of("m2d.b1.identity", &[b"other"]);
    edits.push(a);
    for edited in &edits {
        digests.push(components_with(edited, "index.html", 0));
    }
    let distinct: BTreeSet<Digest> = digests.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        digests.len(),
        "each component alone moves the digest"
    );
}
