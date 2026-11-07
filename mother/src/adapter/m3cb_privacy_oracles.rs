//! Cb2 privacy composition: each lifecycle schedule (with its own judgments)
//! reruns with canary notes/facts, then every evidence surface is scanned.
use super::{
    c1b_port_oracles::children,
    m3cb_lifecycle_oracles::{run as schedule, staged, FACT, LATE},
    m3cb_support::Scenario,
};
use crate::effect::EffectRequest;
use std::path::Path;

const VAR: &str = "NEXUS_M3CB2_PRIVACY";
const REJECT: &str = "CB_REJECT_CANARY";
const TRANSPORT: &str = "CB_TRANSPORT_CANARY";
fn cases(test: &str, list: &[&str]) {
    children("adapter::m3cb_privacy_oracles", VAR, test, list, check);
}
/// Every regular file below `dir`: (count, lossy text).
fn tree(dir: &Path) -> (usize, String) {
    let (mut n, mut text, mut pending) = (0, String::new(), vec![dir.to_path_buf()]);
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                n += 1;
                text.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
            }
        }
    }
    (n, text)
}
fn absent(what: &str, text: &str, canaries: &[&str]) {
    for canary in canaries {
        assert!(!text.contains(canary), "{canary} leaked into {what}");
        let hex = hex::encode(canary);
        assert!(!text.contains(&hex), "{canary} leaked (hex) into {what}");
        assert!(
            !text.contains(&hex.to_uppercase()),
            "{canary} leaked (HEX) into {what}"
        );
    }
}
fn check(case: &str) {
    let (kind, rest) = case.split_once(':').unwrap();
    let (s, diag) = schedule(rest, true);
    // The canary contract was the one consented and lent (positive control).
    assert!(s.c.notes[0].text.starts_with("CB_NOTE_CANARY"));
    assert_eq!(s.c.page_facts.as_ref().unwrap().hours, [FACT]);
    if kind == "records" {
        records(s, &diag, rest);
    } else {
        drafts(&s, rest);
    }
}
fn records(owned: Scenario, diag: &str, case: &str) {
    let s = &owned;
    let all = ["CB_NOTE_CANARY", "面包", FACT, REJECT, LATE, TRANSPORT];
    let o = s.output();
    let observed = s.seen.lock().unwrap().results.join(" ");
    let surfaces = [
        ("returned errors", diag.to_string()),
        ("caller results", observed),
        ("lane verdict/text", format!("{:?} {:?}", o.verdict, o.text)),
        ("reason", format!("{:?}", s.m.reason(s.id))),
        ("close", format!("{:?}", s.m.close())),
    ];
    for (what, text) in &surfaces {
        absent(what, text, &all);
    }
    // The page (facts + accepted prose) is the only legitimate effect content.
    absent("lane effects", &format!("{:?}", o.effects), &all[..2]);
    absent("lane effects", &format!("{:?}", o.effects), &all[3..]);
    let journal = s.fx.journal.lines();
    assert!(
        !journal.is_empty() && s.fx.model_invocations() == 1,
        "journal created"
    );
    absent("Mother journal", &journal.concat(), &all);
    let broker = s.fx.home.dir.join("broker");
    assert!(
        broker.join("p3-evidence/evidence.sqlite3").is_file(),
        "evidence store"
    );
    let success = case == "plain/one" || case == "plain/two";
    assert_eq!(
        staged(s).len(),
        usize::from(success),
        "a success stages its File"
    );
    let ledger = broker.join("files/ledger.db").is_file();
    assert_eq!(ledger, success, "the file ledger exists with a File");
    let (n, mut stores) = tree(&broker);
    assert!(n > 1, "broker ledger/evidence files: {n}");
    // The staged File (facts + accepted prose) is the deliverable, not a record.
    for page in staged(s) {
        let page = std::fs::read_to_string(page).unwrap();
        assert!(page.contains(FACT) && page.contains("Welcome to our bakery."));
        absent("staged File", &page, &all[..2]);
        absent("staged File", &page, &all[3..]);
        stores = stores.replacen(&page, "", 1);
    }
    absent("broker ledger/evidence", &stores, &all);
    let (attempts, charges, home) = (s.fx.attempts(), s.fx.charges(), s.fx.home.dir.clone());
    // Release the native stores' locks after all live judgments.
    drop(owned);
    native(&home, attempts, charges, success);
}
/// Run `cmd` with a bounded lifetime: on expiry the child is killed and
/// reaped, and the test fails. Its small output is read after exit (Cb2-R2).
fn bounded(cmd: &mut std::process::Command) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill the owned python3 child");
            child.wait().expect("reap the owned python3 child");
            panic!("native SQLite check exceeded its 10 s bound: python3 killed and reaped");
        }
        // A short poll of the owned child, not a synchronization assumption.
        std::thread::sleep(Duration::from_millis(10));
    };
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    std::process::Output {
        status,
        stdout,
        stderr,
    }
}
/// Positively identify the native rows: read-only SQLite (existing Linux
/// test convention: `python3`), once the scenario released its stores.
fn native(home: &Path, attempts: usize, charges: usize, success: bool) {
    let script = r#"
import json, sqlite3, sys
def q(db, sql):
    with sqlite3.connect('file:' + sys.argv[1] + db + '?mode=ro', uri=True, timeout=1) as c:
        return c.execute(sql).fetchall()
calls = [json.loads(b)['detail'] for (b,) in q('/broker/p3-evidence/evidence.sqlite3', "SELECT body FROM record WHERE phase='model_call'")]
consent = q('/broker/p3-evidence/evidence.sqlite3', "SELECT count(*) FROM record WHERE phase='mission_consented'")[0][0]
tables, ledger = [], 0
if sys.argv[2] == '1':
    names = q('/broker/files/ledger.db', "SELECT name FROM sqlite_master WHERE type='table'")
    tables = [n for (n,) in names if not n.startswith('sqlite_')]
    ledger = sum(q('/broker/files/ledger.db', 'SELECT count(*) FROM "%s"' % n.replace('"', '""'))[0][0] for n in tables)
print(json.dumps({'calls': len(calls), 'charged': sum('call' in d for d in calls), 'ops': sorted({d['op'] for d in calls}), 'consent': consent, 'tables': len(tables), 'ledger': ledger}))
"#;
    let out = bounded(
        std::process::Command::new("python3")
            .args(["-I", "-c", script])
            .arg(home)
            .arg(if success { "1" } else { "0" }),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["consent"], 1, "the consent record");
    assert_eq!(
        v["calls"], attempts,
        "one model_call record per native attempt"
    );
    assert_eq!(
        v["charged"], charges,
        "charged records match ModelCallCharged"
    );
    assert!(attempts == 0 || v["ops"] == serde_json::json!(["model_call"]));
    // Generic file-ledger check: every non-internal table's rows (Cb2-R3).
    assert_eq!(
        v["tables"].as_u64().unwrap() > 0,
        success,
        "file ledger tables"
    );
    assert_eq!(
        v["ledger"].as_u64().unwrap() > 0,
        success,
        "file ledger rows"
    );
}
fn drafts(s: &Scenario, case: &str) {
    let o = s.output();
    // Post-answer invalidation: the candidate is request data (§5.6), judged on disk.
    let returned = if case.starts_with("post/") {
        &[][..]
    } else {
        &o.effects[..]
    };
    for effect in returned {
        let EffectRequest::CreateFile(f) = effect else {
            panic!("only the File request")
        };
        absent("returned File content", &f.content, &[REJECT, LATE]);
        assert!(f.content.contains("Welcome to our bakery.") && f.content.contains(FACT));
    }
    let home = &s.fx.home.dir;
    let (n, private) = tree(&home.join("private"));
    assert!(n > 0, "the private store holds the consented contract");
    absent("private store", &private, &[REJECT, LATE]);
    let (_, everything) = tree(home);
    absent("test home", &everything, &[REJECT, LATE]);
    let page = home.join("workspace/index.html");
    if case == "plain/two" {
        assert_eq!(o.effects.len(), 1, "accepted correction");
        let pages = staged(s);
        assert_eq!(pages.len(), 1, "the one staged File");
        let written = std::fs::read_to_string(&pages[0]).unwrap();
        assert_eq!(
            o.effects,
            [EffectRequest::CreateFile(
                nexus_governed_control::files::FileCreate {
                    name: "index.html".into(),
                    content: written
                }
            )]
        );
    } else {
        assert!(staged(s).is_empty(), "no deliverable");
    }
    assert!(!page.exists(), "nothing published without the Owner");
}

#[test]
fn m3cb_canaries_absent_from_records_and_diagnostics() {
    cases(
        "m3cb_canaries_absent_from_records_and_diagnostics",
        &[
            "records:plain/one",
            "records:plain/two",
            "records:plain/bad",
            "records:plain/transport",
            "records:invalidated",
            "records:between/stop",
            "records:during/held",
            "records:during/late",
            "records:journal",
            "records:debt/owed",
        ],
    );
}
#[test]
fn m3cb_rejected_and_late_drafts_are_not_deliverables() {
    cases(
        "m3cb_rejected_and_late_drafts_are_not_deliverables",
        &[
            "drafts:plain/two",
            "drafts:plain/bad",
            "drafts:invalidated",
            "drafts:during/late",
            "drafts:post/invalidate",
        ],
    );
}
