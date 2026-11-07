//! M3-E M-2 oracles (S1a-E, register docs/security/m3-e-oracles.md,
//! rulings M3E-R1..R6): `check-page`, bare and with `--facts`, through the
//! real entry on real files. Expected values are independent: hashes from
//! sha2 and the image file, refusals from the kernel's own `check`, texts
//! from the frozen constants.
#![cfg(target_os = "linux")]
#![deny(unsafe_code)]

use nexus_governed_control::authority::evidence::escaped;
use nexus_governed_control::inert_page::{check, policy_hash, ApprovedFacts, MOTHER_CSS};
use nexus_mother::teaching::{t14_passed, t14_refused, LABEL_PASSED, OPEN_LINES};
use nexus_mother_host::check_page::{run, FACTS_NOT_CHECKED, USAGE};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

const BAKERY: &str = include_str!("../../src/oracles/m3a_bakery.html");
const REFERENCE: &str = include_str!("../../../kernel/src/inert_page/fixtures/reference.html");
const OLD_TEMPLATE: &str = include_str!("../../../kernel/tests/fixtures/local_business.html");
const F_BAKERY: &str = "name: Rosa's Bakery\nhours: Tuesday to Sunday, 7am to 3pm\nhours: Closed on Mondays\nphone: 555-0142\n";
const BREAD: &str = "<p>We bake sourdough, rye and seeded loaves before the doors open.</p>";
const CAKE: &str = "<p>Lemon drizzle and carrot cake are on the counter every day.</p>";
const CANARY: &str = "Zqcanary";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("m3e-m2-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Dir(path)
    }
    fn file(&self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sha(bytes: impl AsRef<[u8]>) -> String {
    hex(&Sha256::digest(bytes.as_ref()).into())
}
fn hex(d: &[u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}
fn image() -> &'static str {
    static IMAGE: OnceLock<String> = OnceLock::new();
    IMAGE.get_or_init(|| sha(std::fs::read(std::env::current_exe().unwrap()).unwrap()))
}
fn facts_hash(name: &str, hours: &[&str], phone: &str) -> String {
    hex(&ApprovedFacts::new(name, hours, phone).unwrap().hash())
}
fn bakery_hash() -> String {
    facts_hash(
        "Rosa's Bakery",
        &["Tuesday to Sunday, 7am to 3pm", "Closed on Mondays"],
        "555-0142",
    )
}

/// One run: exit code and stdout lines.
fn cli(args: &[&Path]) -> (u8, Vec<String>) {
    cli_os(args.iter().map(|p| p.as_os_str().to_os_string()).collect())
}
fn cli_os(args: Vec<OsString>) -> (u8, Vec<String>) {
    let mut out = Vec::new();
    let code = run(args, &mut out);
    let text = String::from_utf8(out).expect("output is UTF-8");
    (code, text.lines().map(str::to_string).collect())
}
fn with_facts(page: &Path, facts: &Path) -> (u8, Vec<String>) {
    cli(&[page, Path::new("--facts"), facts])
}
fn head(page: &[u8]) -> Vec<String> {
    vec![
        format!("binary sha256 {}", image()),
        format!("policy {}  css {}", hex(&policy_hash()), sha(MOTHER_CSS)),
        format!("file bytes {} sha256 {}", page.len(), sha(page)),
    ]
}
fn lines(head: Vec<String>, tail: &[&str]) -> Vec<String> {
    head.into_iter()
        .chain(tail.iter().map(|s| s.to_string()))
        .collect()
}
fn facts_line(file: &str, hash: &str) -> String {
    format!("facts file sha256 {}  facts hash {hash}", sha(file))
}

#[test]
fn s1ae_bare_receipt_has_no_facts_claim() {
    let d = Dir::new();
    for page in [BAKERY, REFERENCE] {
        let path = d.file("index.html", page);
        let expected = lines(
            head(page.as_bytes()),
            &[
                LABEL_PASSED,
                OPEN_LINES[0],
                FACTS_NOT_CHECKED,
                "result: PASS",
            ],
        );
        assert_eq!(cli(&[&path]), (0, expected), "bare canonical page");
    }
    let (_, out) = cli(&[&d.file("index.html", BAKERY)]);
    for claim in [
        "facts present as given",
        "visible text",
        "facts hash",
        "Your edit passed",
    ] {
        assert!(!out.iter().any(|l| l.contains(claim)), "{claim}");
    }
    // V5 runs bare: an email-shaped token refuses.
    let email = BAKERY.replace(CAKE, "<p>Write to rosa@example.com for cake orders.</p>");
    let path = d.file("email.html", &email);
    let expected = lines(
        head(email.as_bytes()),
        &[
            LABEL_PASSED,
            OPEN_LINES[0],
            FACTS_NOT_CHECKED,
            "result: REFUSED email_not_supported",
            &t14_refused("email_not_supported", None),
        ],
    );
    assert_eq!(cli(&[&path]), (1, expected), "bare email page");
}

#[test]
fn s1ae_supplied_facts_two_keys() {
    let d = Dir::new();
    let facts = d.file("facts.txt", F_BAKERY);
    let shown = escaped(&facts.to_string_lossy());
    let pass = |page: &str| {
        lines(
            head(page.as_bytes()),
            &[
                LABEL_PASSED,
                OPEN_LINES[0],
                &facts_line(F_BAKERY, &bakery_hash()),
                "facts present as given: name, hours (2 lines), phone",
                "visible text: pass",
                "result: PASS",
                &t14_passed(&shown, &bakery_hash()),
            ],
        )
    };
    let page = d.file("index.html", BAKERY);
    assert_eq!(
        with_facts(&page, &facts),
        (0, pass(BAKERY)),
        "matching facts"
    );
    // A non-fact edit passes both contracts.
    let edited = BAKERY.replace(
        BREAD,
        &format!("<p>We bake {CANARY} rye before the doors open.</p>"),
    );
    let page = d.file("edited.html", &edited);
    assert_eq!(
        with_facts(&page, &facts),
        (0, pass(&edited)),
        "non-fact edit"
    );
    assert_eq!(cli(&[&page]).0, 0);

    // A changed facts file: its own hash, and the result follows it. Both
    // keys run and print; the first failure is V3's.
    let page = d.file("index.html", BAKERY);
    let hours = ["Tuesday to Sunday, 7am to 3pm", "Closed on Mondays"];
    for (file, hash, v3, v4) in [
        (
            F_BAKERY.replace("Rosa's Bakery", "Rosa's Cakes"),
            facts_hash("Rosa's Cakes", &hours, "555-0142"),
            "name_not_heading",
            "name_not_visible",
        ),
        (
            F_BAKERY.replace("Closed on Mondays", "Closed on Sundays"),
            facts_hash(
                "Rosa's Bakery",
                &[hours[0], "Closed on Sundays"],
                "555-0142",
            ),
            "hours_line_missing:1",
            "hours_line_not_visible:1",
        ),
        (
            F_BAKERY.replace("555-0142", "555-0143"),
            facts_hash("Rosa's Bakery", &hours, "555-0143"),
            "phone_missing",
            "phone_not_visible",
        ),
    ] {
        assert_ne!(hash, bakery_hash());
        let changed = d.file("changed.txt", &file);
        let expected = lines(
            head(BAKERY.as_bytes()),
            &[
                LABEL_PASSED,
                OPEN_LINES[0],
                &facts_line(&file, &hash),
                &format!("facts present as given: refused {v3}"),
                &format!("visible text: refused {v4}"),
                &format!("result: REFUSED {v3}"),
                &t14_refused(v3, None),
            ],
        );
        assert_eq!(with_facts(&page, &changed), (1, expected), "{v3}");
    }
    // V3 alone: a foreign phone-like number refuses although V4 sees every fact.
    let foreign = BAKERY.replace(CAKE, "<p>Order line 555-0199 opens at noon.</p>");
    let page = d.file("foreign.html", &foreign);
    let expected = lines(
        head(foreign.as_bytes()),
        &[
            LABEL_PASSED,
            OPEN_LINES[0],
            &facts_line(F_BAKERY, &bakery_hash()),
            "facts present as given: refused foreign_number",
            "visible text: pass",
            "result: REFUSED foreign_number",
            &t14_refused("foreign_number", None),
        ],
    );
    assert_eq!(with_facts(&page, &facts), (1, expected), "foreign number");
}

#[test]
fn s1ae_refusal_location_and_exit_contract() {
    let d = Dir::new();
    let facts = d.file("facts.txt", F_BAKERY);
    // Structural refusals: the kernel's code and location, nothing else.
    let href = BAKERY.replace(BREAD, "<p>We bake <a href=\"x\">bread</a> daily.</p>");
    let href_line = 1 + BAKERY[..BAKERY.find(BREAD).unwrap()].matches('\n').count();
    let script = BAKERY.replace("<main>\n", "<main>\n<script>alert(1)</script>\n");
    for (name, page, line) in [
        ("href", href.as_str(), Some(href_line)),
        ("script", script.as_str(), None),
        ("old template", OLD_TEMPLATE, None),
    ] {
        let r = check(page.as_bytes()).expect_err(name);
        if let Some(line) = line {
            assert_eq!(r.line as usize, line, "{name}: the planted line");
        }
        let (code, at) = (r.code.as_str(), (u64::from(r.line), u64::from(r.column)));
        let refused = format!("refused: {code} at line {}, column {}", r.line, r.column);
        let path = d.file("page.html", page);
        let bare = lines(
            head(page.as_bytes()),
            &[
                &refused,
                OPEN_LINES[0],
                FACTS_NOT_CHECKED,
                &format!("result: REFUSED {code}"),
                &t14_refused(code, Some(at)),
            ],
        );
        assert_eq!(cli(&[&path]), (1, bare), "{name} bare");
        let facts_run = lines(
            head(page.as_bytes()),
            &[
                &refused,
                OPEN_LINES[0],
                &facts_line(F_BAKERY, &bakery_hash()),
                "facts present as given: not run",
                "visible text: not run",
                &format!("result: REFUSED {code}"),
                &t14_refused(code, Some(at)),
            ],
        );
        assert_eq!(
            with_facts(&path, &facts),
            (1, facts_run),
            "{name} with facts"
        );
    }
    // Invalid facts syntax: the facts line only, never a page location.
    let page = d.file("index.html", BAKERY);
    let bad = "name: Rosa's Bakery\nopen: daily\n";
    let code = "facts_file_invalid (facts file line 2)";
    let expected = lines(
        head(BAKERY.as_bytes()),
        &[
            LABEL_PASSED,
            OPEN_LINES[0],
            &format!("facts file sha256 {}  facts hash unavailable", sha(bad)),
            &format!("facts present as given: refused {code}"),
            "visible text: not run",
            &format!("result: REFUSED {code}"),
            &t14_refused(code, None),
        ],
    );
    assert_eq!(with_facts(&page, &d.file("bad.txt", bad)), (1, expected));
    let phone = F_BAKERY.replace("555-0142", "12345");
    let (exit, out) = with_facts(&page, &d.file("phone.txt", &phone));
    assert_eq!(
        (exit, out.last().cloned()),
        (1, Some(t14_refused("phone_format", None)))
    );

    // Over the read cap: refused, no hash of the prefix.
    let over = vec![b'a'; 65_537];
    let path = d.file("over.html", &over);
    let expected = vec![
        format!("binary sha256 {}", image()),
        format!("policy {}  css {}", hex(&policy_hash()), sha(MOTHER_CSS)),
        "file bytes unavailable (over 65536) sha256 unavailable".to_string(),
        "refused: input_too_large".to_string(),
        OPEN_LINES[0].to_string(),
        FACTS_NOT_CHECKED.to_string(),
        "result: REFUSED input_too_large".to_string(),
        t14_refused("input_too_large", None),
    ];
    let (exit, out) = cli(&[&path]);
    assert_eq!((exit, &out), (1, &expected), "cap + 1");
    assert!(!out.iter().any(|l| l.contains(&sha(&over[..65_536]))));
    let at_cap = d.file("cap.html", vec![b'a'; 65_536]);
    let r = check(&[b'a'; 65_536]).unwrap_err();
    let (exit, out) = cli(&[&at_cap]);
    assert_eq!(
        (exit, out[2].clone()),
        (
            1,
            format!("file bytes 65536 sha256 {}", sha([b'a'; 65_536]))
        )
    );
    assert_eq!(
        out[3],
        format!(
            "refused: {} at line {}, column {}",
            r.code.as_str(),
            r.line,
            r.column
        )
    );

    // Read refusals and usage: exit 2, a fixed line, no OS text.
    let link = d.0.join("link.html");
    std::os::unix::fs::symlink(&page, &link).unwrap();
    let missing = d.0.join("missing.html");
    let big_facts = d.file("big.txt", vec![b'a'; 2049]);
    for (args, line) in [
        (vec![missing.clone()], "ERROR input_unavailable (page)"),
        (vec![link.clone()], "ERROR input_link (page)"),
        (vec![d.0.clone()], "ERROR input_not_regular (page)"),
        (
            vec![PathBuf::from("/proc/self/mem")],
            "ERROR input_unreadable (page)",
        ),
        (
            vec![missing.clone(), "--facts".into(), missing.clone()],
            "ERROR input_unavailable (page)",
        ),
        (
            vec![page.clone(), "--facts".into(), missing.clone()],
            "ERROR input_unavailable (facts file)",
        ),
        (
            vec![page.clone(), "--facts".into(), link.clone()],
            "ERROR input_link (facts file)",
        ),
        (
            vec![page.clone(), "--facts".into(), big_facts.clone()],
            "ERROR input_too_large (facts file)",
        ),
    ] {
        let refs: Vec<&Path> = args.iter().map(PathBuf::as_path).collect();
        assert_eq!(cli(&refs), (2, vec![line.to_string()]), "{args:?}");
    }
    let usage: Vec<Vec<&str>> = vec![
        vec![],
        vec!["a.html", "b.html"],
        vec!["a.html", "--facts"],
        vec!["a.html", "--facts", "f.txt", "x"],
        vec!["--facts", "f.txt", "a.html"],
        vec!["a.html", "--fact", "f.txt"],
        vec!["--x"],
        vec![""],
        vec!["a.html", "--facts", "--facts"],
    ];
    for args in usage {
        let os: Vec<OsString> = args.iter().map(OsString::from).collect();
        assert_eq!(cli_os(os), (2, vec![USAGE.to_string()]), "{args:?}");
    }
    // Relative and non-UTF-8 paths are taken as given.
    use std::os::unix::ffi::OsStringExt;
    let odd = d.0.join(OsString::from_vec(b"page-\xff.html".to_vec()));
    std::fs::write(&odd, BAKERY).unwrap();
    assert_eq!(cli(&[&odd]).0, 0, "non-UTF-8 path");
    let relative = Path::new("../src/oracles/m3a_bakery.html");
    let here = std::env::current_dir().unwrap();
    assert!(
        here.join(relative).is_file(),
        "cargo runs tests in the package dir"
    );
    assert_eq!(cli(&[relative]).0, 0, "relative path");
    // No content of the page or facts ever appears in the output.
    let edited = BAKERY.replace(BREAD, &format!("<p>We bake {CANARY} rye daily.</p>"));
    let canary_facts = F_BAKERY.replace("Rosa's Bakery", CANARY);
    let (_, out) = with_facts(&d.file("c.html", &edited), &d.file("c.txt", &canary_facts));
    assert!(!out.iter().any(|l| l.contains(CANARY)), "{out:?}");
    let (_, out) = cli(&[&d.file("c2.html", &edited)]);
    assert!(!out.iter().any(|l| l.contains(CANARY)) && !out.iter().any(|l| l.contains("Rosa")));
}
