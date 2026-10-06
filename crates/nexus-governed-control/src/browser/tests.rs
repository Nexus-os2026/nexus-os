//! The governed browser, live: a fresh headless Chrome per session against
//! a loopback page server, reachable only through the session's proxy.

use super::{Browser, BrowserIntent, BrowserStep};
use crate::authority::commitment::CommitmentState;
use crate::authority::effect::EffectClass;
use crate::authority::ids::Digest;
use crate::authority::policy::GrantScope;
use crate::authority::AuthorityError;
use crate::control::EffectOutput;
use crate::egress::destination::Resolver;
use crate::harness_tests::{harness, temp_root, Harness, Reply, TempRoot, TestServer, Yes};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Every `*.nexus.invalid` name is the loopback test server.
struct Fixtures;
impl Resolver for Fixtures {
    fn resolve(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        if host.ends_with(".nexus.invalid") {
            Ok(vec![SocketAddr::new("127.0.0.1".parse().unwrap(), port)])
        } else {
            Err(std::io::Error::other("not a fixture"))
        }
    }
}

/// A browser to drive live, when this host has a Chrome it may launch.
///
/// A Chrome that someone other than root could replace is refused before
/// any session (the hosted runner image makes `/opt` world-writable until
/// ci.yml restores Chrome's modes). Where that happens the refusal is
/// checked, exactly, along with the reason for it, and the live sessions
/// are skipped. CI installs a root-only Chrome, so there a missing or
/// refused one fails the tests instead of skipping them.
fn browser() -> Option<(Browser, TempRoot)> {
    let in_ci = std::env::var_os("CI").is_some();
    if !std::path::Path::new(super::CHROME).exists() {
        assert!(
            !in_ci,
            "Chrome is missing: CI must exercise the live browser"
        );
        eprintln!("Chrome is not installed: live browser tests are skipped");
        return None;
    }
    let root = temp_root("browser");
    let browser = Browser::for_tests(root.0.clone(), Arc::new(Fixtures));
    match browser.grant_scope(&["https://example.com".into()], false) {
        Ok(_) => Some((browser, root)),
        Err(refused) => {
            let Some(replaceable) = replaceable_by_others(super::CHROME) else {
                panic!("Chrome was refused although only root can change it: {refused:?}");
            };
            assert_eq!(
                refused,
                AuthorityError::Closed("the executable is not in a trusted location")
            );
            assert!(
                !in_ci,
                "Chrome is refused ({replaceable}): CI must exercise the live browser"
            );
            eprintln!(
                "Chrome is refused, as it must be ({replaceable}): live browser tests are skipped"
            );
            None
        }
    }
}

/// The first of `path` and the directories above it that someone other than
/// root owns or may write, if any.
fn replaceable_by_others(path: &str) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    std::path::Path::new(path).ancestors().find_map(|current| {
        let meta = std::fs::symlink_metadata(current).ok()?;
        (meta.uid() != 0 || meta.mode() & 0o022 != 0).then(|| {
            format!(
                "{} has owner {} and mode {:o}",
                current.display(),
                meta.uid(),
                meta.mode() & 0o7777
            )
        })
    })
}

/// Why a refused Chrome is refused: a path someone other than root could
/// change is named, a root-only one is not.
#[test]
fn a_path_others_could_replace_is_named() {
    use std::os::unix::fs::PermissionsExt;
    let root = temp_root("replaceable");
    let file = root.0.path().join("chrome");
    std::fs::write(&file, b"").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o766)).unwrap();
    let named = replaceable_by_others(file.to_str().unwrap()).expect("writable by others");
    assert!(named.ends_with("mode 766"), "{named}");
    assert_eq!(replaceable_by_others("/usr/bin"), None);
}

/// Each click on `#count` takes about 40 ms and is reported to the server.
const CLICKS: &str = r#"<!doctype html><html><body>
<button id="count" onclick="const t=Date.now();while(Date.now()-t<40){};fetch('/tick')">+1</button>
<p class="twice">one</p><p class="twice">two</p>
<input id="sneaky" onfocus="this.type='password'">
</body></html>"#;

/// Enter on the link opens another window (a trusted key press), and the
/// page marks itself a moment later.
const POPUP: &str = r#"<!doctype html><html><body>
<a id="away" href="/" target="_blank" rel="opener" onclick="setTimeout(()=>{const p=document.createElement('p');p.id='later';document.body.append(p)},300)">away</a>
<h1 id="title">Opener</h1>
</body></html>"#;

const PAGE: &str = r#"<!doctype html><html><head><title>Fixture</title></head><body>
<h1 id="title">Hello governed</h1>
<img src="http://blocked.nexus.invalid:PORT/tracker.png">
<button id="go" onclick="document.getElementById('title').innerText='Clicked'">Go</button>
<input id="name"><input id="secret" type="password">
</body></html>"#;

/// A page that redefines what its scripts would see: every input claims to
/// be text, and `querySelector` always answers the password field.
const HOSTILE: &str = r#"<!doctype html><html><body>
<input id="name"><input id="secret" type="password"><div id="log"></div>
<script>
Object.defineProperty(HTMLInputElement.prototype, 'type', { get() { return 'text'; } });
const real = Document.prototype.querySelector;
document.querySelector = function () { return real.call(this, '#secret'); };
for (const id of ['name', 'secret']) {
  document.getElementById(id).addEventListener('input', (e) => {
    document.getElementById('log').innerText += ' ' + id + '=' + e.target.value;
  });
}
</script></body></html>"#;

fn server() -> TestServer {
    TestServer::start(|request| {
        // The page names other origins on the server's own port.
        let port = request
            .header("host")
            .and_then(|host| host.rsplit(':').next())
            .unwrap_or("1")
            .to_string();
        let html = |body: &str| Reply {
            headers: vec![("Content-Type".into(), "text/html".into())],
            ..Reply::ok(&body.replace("PORT", &port))
        };
        match request.path.as_str() {
            "/" => html(PAGE),
            "/hostile" => html(HOSTILE),
            "/escape" => html("<script>location='http://evil.nexus.invalid:PORT/steal'</script>"),
            "/clicks" => html(CLICKS),
            "/popup" => html(POPUP),
            "/tick" => Reply::ok("tick"),
            "/slow" => Reply {
                delay: Duration::from_secs(20),
                ..Reply::ok("late")
            },
            _ => Reply::ok("other"),
        }
    })
}

fn origin(server: &TestServer) -> String {
    format!("http://page.nexus.invalid:{}", server.address.port())
}

fn grant(h: &Harness, browser: &Browser, origins: &[String]) {
    let scope = browser.grant_scope(origins, false).unwrap();
    h.control
        .authority()
        .grants()
        .request(scope, Duration::from_secs(600), &Yes::new(true))
        .unwrap();
}

fn run(
    h: &Harness,
    browser: &Browser,
    intent: BrowserIntent,
    approve: bool,
) -> Result<EffectOutput, AuthorityError> {
    let preparation = browser.prepare(h.control.authority(), &intent)?;
    let view = h.control.propose(&h.agent, h.run, preparation)?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(approve))?;
    let result = h.control.execute(view.id, &h.agent, h.run);
    if result.is_err() {
        // A failing test shows why the effect failed, as evidence has it.
        for record in h.evidence.records() {
            if record.failure.is_some() {
                eprintln!("failed: {:?} {:?}", record.failure, record.detail);
            }
        }
    }
    result
}

/// A read that fails while the page is changing is asked again; the end of
/// the browser, its silence and cancellation still end the wait.
#[test]
fn a_probe_failing_while_the_page_changes_is_asked_again() {
    use super::live::probed;
    use crate::authority::commitment::FailureClass;
    let changing = || {
        Err((
            FailureClass::Actuator,
            "Inspected target navigated or closed".to_string(),
        ))
    };
    assert_eq!(probed(changing(), false), Ok(false));
    assert_eq!(probed(Ok(true), false), Ok(true));
    assert_eq!(probed(Ok(false), false), Ok(false));
    assert!(probed(changing(), true).is_err(), "cancelled");
    for (class, why) in [
        (FailureClass::Unavailable, "the browser ended"),
        (FailureClass::Timeout, "the browser did not answer in time"),
    ] {
        assert_eq!(
            probed(Err((class, why.to_string())), false),
            Err((class, why.to_string()))
        );
    }
}

fn report(out: &EffectOutput) -> Value {
    serde_json::from_str(out.text.as_deref().unwrap()).unwrap()
}

fn meta<'a>(out: &'a EffectOutput, key: &str) -> &'a str {
    &out.meta.iter().find(|(k, _)| k == key).unwrap().1
}

#[test]
fn a_session_reaches_only_its_granted_origins_and_interacts_with_approval() {
    let Some((browser, root)) = browser() else {
        return;
    };
    let server = server();
    let h = harness();
    let page = origin(&server);
    grant(&h, &browser, std::slice::from_ref(&page));
    let intent = BrowserIntent {
        start_url: format!("{page}/"),
        steps: vec![
            BrowserStep::ExtractText {
                selector: "#title".into(),
            },
            BrowserStep::Click {
                selector: "#go".into(),
            },
            BrowserStep::ExtractText {
                selector: "#title".into(),
            },
            BrowserStep::Fill {
                selector: "#name".into(),
                text: "typed by the agent".into(),
            },
            BrowserStep::Fill {
                selector: "#secret".into(),
                text: "never".into(),
            },
        ],
    };
    let preparation = browser.prepare(h.control.authority(), &intent).unwrap();
    assert_eq!(
        preparation.action.class,
        EffectClass::R2,
        "it clicks and fills"
    );
    assert_eq!(
        run(&h, &browser, intent.clone(), false).unwrap_err(),
        AuthorityError::Declined
    );
    assert!(
        server.received().is_empty(),
        "a declined session never starts"
    );
    let out = run(&h, &browser, intent, true).unwrap();
    let report = report(&out);
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(steps[1]["text"], "Hello governed");
    assert_eq!(steps[2]["result"], "ok");
    assert_eq!(steps[3]["text"], "Clicked");
    assert_eq!(steps[4]["result"], "ok");
    assert_eq!(
        steps[5]["result"], "refused",
        "password fields are never filled"
    );
    assert_eq!(report["completed"], false);
    // The tracker on another origin never reached the server.
    assert!(meta(&out, "connections_refused").parse::<u32>().unwrap() >= 1);
    let paths: Vec<String> = server.received().iter().map(|r| r.path.clone()).collect();
    assert!(paths.iter().all(|p| p != "/tracker.png"), "{paths:?}");
    // The profile and everything of the session are gone.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(std::fs::read_dir(root.0.path()).unwrap().count(), 0);
}

#[test]
fn a_page_cannot_take_the_session_elsewhere() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let server = server();
    let h = harness();
    let page = origin(&server);
    grant(&h, &browser, std::slice::from_ref(&page));
    let out = run(
        &h,
        &browser,
        BrowserIntent {
            start_url: format!("{page}/escape"),
            steps: vec![BrowserStep::WaitFor {
                selector: None,
                timeout_ms: Some(1500),
            }],
        },
        true,
    )
    .unwrap();
    assert!(server.received().iter().all(|r| r.path != "/steal"));
    assert!(meta(&out, "connections_refused").parse::<u32>().unwrap() >= 1);
}

#[test]
fn origins_outside_the_grant_and_local_schemes_are_refused_before_launch() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let server = server();
    let h = harness();
    grant(&h, &browser, &[origin(&server)]);
    for url in [
        "http://other.nexus.invalid:1/".to_string(),
        "https://example.com/".to_string(),
    ] {
        assert_eq!(
            browser
                .prepare(
                    h.control.authority(),
                    &BrowserIntent {
                        start_url: url.clone(),
                        steps: vec![]
                    }
                )
                .err(),
            Some(AuthorityError::NoCoveringGrant),
            "{url}"
        );
    }
    for url in [
        "file:///etc/passwd",
        "chrome://settings",
        "javascript:alert(1)",
        "data:text/html,x",
    ] {
        assert!(
            matches!(
                browser.prepare(
                    h.control.authority(),
                    &BrowserIntent {
                        start_url: url.into(),
                        steps: vec![]
                    }
                ),
                Err(AuthorityError::InvalidAction(_))
            ),
            "{url}"
        );
    }
    assert!(server.received().is_empty());
}

#[test]
fn reading_is_r1_and_cancellation_ends_the_browser() {
    let Some((browser, root)) = browser() else {
        return;
    };
    let server = server();
    let h = harness();
    let page = origin(&server);
    grant(&h, &browser, std::slice::from_ref(&page));
    let intent = BrowserIntent {
        start_url: format!("{page}/slow"),
        steps: vec![],
    };
    let preparation = browser.prepare(h.control.authority(), &intent).unwrap();
    assert_eq!(preparation.action.class, EffectClass::R1);
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    let control = h.control.clone();
    let (agent, run) = (h.agent.clone(), h.run);
    let worker = std::thread::spawn(move || control.execute(view.id, &agent, run));
    // Cancel once the browser is really waiting on the page.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !server.received().iter().any(|r| r.path == "/slow") {
        assert!(
            Instant::now() < deadline,
            "the browser never asked for the page"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let cancelled_at = Instant::now();
    h.control.cancel_run(h.run).unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        AuthorityError::RunCancelled
    );
    assert!(
        cancelled_at.elapsed() < Duration::from_secs(5),
        "it did not wait for the page"
    );
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Cancelled
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        std::fs::read_dir(root.0.path()).unwrap().count(),
        0,
        "the session is gone"
    );
}

#[test]
fn the_browser_is_identity_pinned() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let h = harness();
    let mut scope = browser
        .grant_scope(&["https://example.com".into()], false)
        .unwrap();
    if let GrantScope::Browser { identity, .. } = &mut scope {
        *identity = Digest::of("forged", &[]);
    }
    h.control
        .authority()
        .grants()
        .request(scope, Duration::from_secs(60), &Yes::new(true))
        .unwrap();
    assert_eq!(
        browser
            .prepare(
                h.control.authority(),
                &BrowserIntent {
                    start_url: "https://example.com/".into(),
                    steps: vec![]
                }
            )
            .err(),
        Some(AuthorityError::Closed(
            "the browser changed since it was granted; grant it again"
        ))
    );
}

/// The browser inherits nothing of Nexus's environment: a home and a
/// temporary directory of the session's own and a fixed locale. Chrome keeps
/// its process-singleton socket in the temporary directory, which ends with
/// the session instead of staying in the system's.
#[test]
fn the_browser_environment_is_the_sessions_own() {
    let env = super::live::environment(
        "/session/home".into(),
        "/session/tmp".into(),
        "/session/bin".into(),
    );
    let env: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.to_str().unwrap()))
        .collect();
    assert_eq!(
        env,
        [
            ("HOME", "/session/home"),
            ("LANG", "C.UTF-8"),
            ("PATH", "/session/bin"),
            ("TMPDIR", "/session/tmp"),
        ]
    );
}

/// Every step of a session is shown, in full, however many there are (the
/// approval covers exactly what is listed); a selector list, which would
/// click whatever matches first, is refused.
#[test]
fn every_step_is_shown_in_full_and_selector_lists_are_refused() {
    use crate::authority::evidence::is_plain;
    let Some((browser, _root)) = browser() else {
        return;
    };
    let h = harness();
    let site = "http://page.nexus.invalid:8080".to_string();
    grant(&h, &browser, std::slice::from_ref(&site));
    let fill = format!("Dear owner\n{}", "w".repeat(600));
    let mut steps: Vec<BrowserStep> = (0..10)
        .map(|_| BrowserStep::WaitFor {
            selector: None,
            timeout_ms: None,
        })
        .collect();
    steps.push(BrowserStep::Fill {
        selector: "#to".into(),
        text: fill.clone(),
    });
    steps.push(BrowserStep::Click {
        selector: "#confirm".into(),
    });
    let intent = |steps: Vec<BrowserStep>| BrowserIntent {
        start_url: format!("{site}/"),
        steps,
    };
    let preparation = browser
        .prepare(h.control.authority(), &intent(steps))
        .unwrap();
    let summary = &preparation.action.summary;
    assert!(summary.iter().all(|l| is_plain(l)), "{summary:?}");
    assert!(summary.contains(&"11. Fill #to".to_string()), "{summary:?}");
    assert!(
        summary.contains(&"12. Click #confirm".to_string()),
        "{summary:?}"
    );
    let at = summary.iter().position(|l| l == "with the text:").unwrap();
    let mut rebuilt: Vec<String> = Vec::new();
    for line in &summary[at + 1..] {
        if let Some(more) = line.strip_prefix("│↳ ") {
            rebuilt.last_mut().unwrap().push_str(more);
        } else if let Some(text) = line.strip_prefix("│ ") {
            rebuilt.push(text.to_string());
        } else {
            break;
        }
    }
    assert_eq!(rebuilt.join("\n"), fill);
    for list in [
        "#safe, #delete-account",
        "a,b",
        "#x , #y",
        "/*)*/#a, #b",
        "#a/**/",
    ] {
        assert!(
            matches!(
                browser.prepare(
                    h.control.authority(),
                    &intent(vec![BrowserStep::Click {
                        selector: list.into()
                    }])
                ),
                Err(AuthorityError::InvalidAction(_))
            ),
            "{list}"
        );
    }
    for single in [":is(#a, #b)", "[data-x=\"a,b\"]", "a[title='x,y'] > b"] {
        assert!(
            browser
                .prepare(
                    h.control.authority(),
                    &intent(vec![BrowserStep::Click {
                        selector: single.into()
                    }])
                )
                .is_ok(),
            "{single}"
        );
    }
}

/// The fixed scripts run in an isolated world: a page that redefines what
/// an input's type is, or what `querySelector` answers, neither gets its
/// password field filled nor turns a fill toward another field.
#[test]
fn a_page_cannot_redefine_what_the_fixed_scripts_see() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let server = server();
    let h = harness();
    grant(&h, &browser, &[origin(&server)]);
    let out = run(
        &h,
        &browser,
        BrowserIntent {
            start_url: format!("{}/hostile", origin(&server)),
            steps: vec![
                BrowserStep::Fill {
                    selector: "#name".into(),
                    text: "hello".into(),
                },
                BrowserStep::ExtractText {
                    selector: "#log".into(),
                },
                BrowserStep::Fill {
                    selector: "#secret".into(),
                    text: "pw".into(),
                },
            ],
        },
        true,
    )
    .unwrap();
    let report = report(&out);
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(steps[1]["result"], "ok");
    assert_eq!(steps[2]["text"].as_str().unwrap().trim(), "name=hello");
    assert_eq!(steps[3]["result"], "refused");
}

/// A long report is cut on a character boundary, whatever the page text.
#[test]
fn a_report_is_cut_on_a_character_boundary() {
    let text = "語".repeat(100_000);
    let out = super::bounded_report(text);
    assert!(out.len() <= 256 * 1024);
    assert!(out.chars().all(|c| c == '語'));
}

/// A stop ends a session between its steps: once the run is cancelled no
/// further step reaches the page (each click here takes about 40 ms and is
/// counted by the server), the session ends cancelled, and the browser and
/// everything of the session are gone.
#[test]
fn a_stop_ends_the_session_between_its_steps() {
    let Some((browser, root)) = browser() else {
        return;
    };
    let server = server();
    let h = harness();
    let page = origin(&server);
    grant(&h, &browser, std::slice::from_ref(&page));
    let intent = BrowserIntent {
        start_url: format!("{page}/clicks"),
        steps: (0..20)
            .map(|_| BrowserStep::Click {
                selector: "#count".into(),
            })
            .collect(),
    };
    let preparation = browser.prepare(h.control.authority(), &intent).unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    let control = h.control.clone();
    let (agent, run) = (h.agent.clone(), h.run);
    let worker = std::thread::spawn(move || control.execute(view.id, &agent, run));
    let ticks = || {
        server
            .received()
            .iter()
            .filter(|r| r.path == "/tick")
            .count()
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    while ticks() == 0 {
        assert!(Instant::now() < deadline, "the page was never clicked");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.control.cancel_run(h.run).unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        AuthorityError::RunCancelled
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(ticks() <= 4, "{} clicks reached the page", ticks());
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Cancelled
    );
    // The cancelled session's record says how far it got.
    let records = h.evidence.records();
    let finished = records.iter().rev().find(|r| r.outcome.is_some()).unwrap();
    assert_eq!(finished.outcome, Some("cancelled"));
    let detail = &finished
        .detail
        .iter()
        .find(|(k, _)| k == "detail")
        .unwrap()
        .1;
    assert!(
        detail.contains(" (at step ") && detail.ends_with(" of 20)"),
        "{detail}"
    );
    assert_eq!(
        std::fs::read_dir(root.0.path()).unwrap().count(),
        0,
        "the session is gone"
    );
    // No process of the session outlives it, not even one that left its
    // process group: Chrome's crash handlers daemonize, and end with the
    // browser.
    let session = root.0.path().display().to_string();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = processes_mentioning(&session);
        if left.is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "left behind: {left:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The processes whose command line mentions `text`.
fn processes_mentioning(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let cmdline = String::from_utf8_lossy(&cmdline).replace('\0', " ");
        if cmdline.contains(text) {
            found.push(format!(
                "{}: {cmdline}",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    found
}

/// Revoking the session's grant ends it before its next step: no further
/// click reaches the page, and the failure says how far it got.
#[test]
fn a_revoked_grant_ends_the_session_before_its_next_step() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let server = server();
    let h = harness();
    let page = origin(&server);
    let grant = h
        .control
        .authority()
        .grants()
        .request(
            browser
                .grant_scope(std::slice::from_ref(&page), false)
                .unwrap(),
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    let intent = BrowserIntent {
        start_url: format!("{page}/clicks"),
        steps: (0..20)
            .map(|_| BrowserStep::Click {
                selector: "#count".into(),
            })
            .collect(),
    };
    let preparation = browser.prepare(h.control.authority(), &intent).unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    let control = h.control.clone();
    let (agent, run) = (h.agent.clone(), h.run);
    let worker = std::thread::spawn(move || control.execute(view.id, &agent, run));
    let ticks = || {
        server
            .received()
            .iter()
            .filter(|r| r.path == "/tick")
            .count()
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    while ticks() == 0 {
        assert!(Instant::now() < deadline, "the page was never clicked");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.control.authority().grants().revoke(grant).unwrap();
    let at_revocation = ticks();
    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        AuthorityError::Unavailable("the effect failed")
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        ticks() <= at_revocation + 1,
        "{} clicks reached the page, {at_revocation} before the revocation",
        ticks()
    );
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Failed
    );
    let records = h.evidence.records();
    let finished = records.iter().rev().find(|r| r.outcome.is_some()).unwrap();
    assert_eq!(finished.failure, Some("refused"));
    let detail = &finished
        .detail
        .iter()
        .find(|(k, _)| k == "detail")
        .unwrap()
        .1;
    assert!(
        detail.starts_with("its grant was revoked or expired (at step ")
            && detail.ends_with(" of 20)"),
        "{detail}"
    );
}

/// A selector acts only when it matches exactly one element, and a field
/// that turns into a password field when focused is not filled.
#[test]
fn a_step_acts_on_exactly_one_element_and_never_on_a_password_field() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let server = server();
    let page = origin(&server);
    for (step, refused) in [
        (
            BrowserStep::Click {
                selector: ".twice".into(),
            },
            "ambiguous",
        ),
        (
            BrowserStep::Fill {
                selector: "#sneaky".into(),
                text: "hunter2".into(),
            },
            "refused",
        ),
    ] {
        let h = harness();
        grant(&h, &browser, std::slice::from_ref(&page));
        let out = run(
            &h,
            &browser,
            BrowserIntent {
                start_url: format!("{page}/clicks"),
                steps: vec![step],
            },
            true,
        )
        .unwrap();
        let report: Value = serde_json::from_str(out.text.as_deref().unwrap()).unwrap();
        assert_eq!(report["steps"][1]["result"], refused, "{report}");
        assert_eq!(report["completed"], false);
    }
}

/// A window a step opens is closed before the next step, found by listing
/// the browser's pages rather than from events a page could crowd out.
#[test]
fn a_window_a_step_opens_is_closed_before_the_next_step() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let server = server();
    let page = origin(&server);
    let h = harness();
    grant(&h, &browser, std::slice::from_ref(&page));
    let out = run(
        &h,
        &browser,
        BrowserIntent {
            start_url: format!("{page}/popup"),
            steps: vec![
                BrowserStep::Press {
                    selector: "#away".into(),
                    key: "enter".into(),
                },
                BrowserStep::WaitFor {
                    selector: Some("#later".into()),
                    timeout_ms: Some(5_000),
                },
                BrowserStep::ExtractText {
                    selector: "#title".into(),
                },
            ],
        },
        true,
    )
    .unwrap();
    let report: Value = serde_json::from_str(out.text.as_deref().unwrap()).unwrap();
    assert_eq!(report["completed"], true, "{report}");
    assert_eq!(report["popups_closed"], 1, "{report}");
    assert_eq!(report["steps"][3]["text"], "Opener", "{report}");
}

/// The session's proxy serves at most `MAX_CONNECTIONS` clients at once and
/// ends every connection soon after the session (and so the proxy) ends.
#[test]
fn the_proxy_caps_its_connections_and_ends_them_with_the_session() {
    use super::proxy::{BrowserProxy, OriginPolicy};
    use std::io::Read;
    let proxy = BrowserProxy::start(OriginPolicy {
        origins: vec![],
        allow_private: true,
        resolver: Arc::new(Fixtures),
        live: Arc::new(|| true),
    })
    .unwrap();
    let address: SocketAddr = format!("127.0.0.1:{}", proxy.port()).parse().unwrap();
    let mut held: Vec<std::net::TcpStream> = (0..64)
        .map(|_| std::net::TcpStream::connect(address).unwrap())
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let mut over = std::net::TcpStream::connect(address).unwrap();
    over.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut byte = [0u8; 1];
    // The 65th is closed at once and counted as refused: neither served nor
    // left waiting (a read that times out means it was held open).
    let started = Instant::now();
    match over.read(&mut byte) {
        Ok(0) => {}
        other => panic!("the 65th connection was not closed at once: {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "closed only after {:?}",
        started.elapsed()
    );
    assert_eq!(proxy.refused(), 1);
    drop(proxy);
    // Each waiting client is answered 403 or closed, never left open.
    for client in &mut held {
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut answer = Vec::new();
        client
            .read_to_end(&mut answer)
            .expect("the connection ends with the proxy");
        assert!(
            answer.is_empty() || answer.starts_with(b"HTTP/1.1 403"),
            "{}",
            String::from_utf8_lossy(&answer)
        );
    }
}

/// A loopback upstream that reports every byte that reaches it: one
/// message per connection, when the connection ends (or after two quiet
/// seconds).
fn recording_upstream() -> (u16, std::sync::mpsc::Receiver<Vec<u8>>) {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                break;
            };
            let sender = sender.clone();
            std::thread::spawn(move || {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut seen = Vec::new();
                let mut chunk = [0u8; 1024];
                while let Ok(n) = stream.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    seen.extend_from_slice(&chunk[..n]);
                }
                let _ = sender.send(seen);
            });
        }
    });
    (port, receiver)
}

/// A connection goes no further once its session may not: wherever the
/// session stops being live (before the request is admitted, between
/// resolving and connecting, after connecting, before the body), nothing
/// more reaches the upstream.
#[test]
fn a_proxy_connection_goes_no_further_once_the_session_may_not() {
    use super::proxy::{serve, OriginPolicy};
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    // `live` answers yes `yes` times, then no.
    let policy = |port: u16, yes: usize| {
        let asked = Arc::new(AtomicUsize::new(0));
        Arc::new(OriginPolicy {
            origins: vec![
                format!("http://fixture.nexus.invalid:{port}"),
                format!("https://fixture.nexus.invalid:{port}"),
            ],
            allow_private: true,
            resolver: Arc::new(Fixtures),
            live: Arc::new(move || asked.fetch_add(1, Ordering::SeqCst) < yes),
        })
    };
    // Send `request` through a proxy connection; what the client got back,
    // and every connection's bytes the upstream saw.
    let run = |request: &dyn Fn(u16) -> Vec<u8>, yes: usize| {
        let (port, upstream) = recording_upstream();
        let policy = policy(port, yes);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        client.write_all(&request(port)).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let served = std::thread::spawn(move || {
            serve(
                accepted,
                &policy,
                &stop,
                &std::sync::atomic::AtomicU32::new(0),
            )
        });
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut answer = Vec::new();
        let _ = client.read_to_end(&mut answer);
        assert!(!served.join().unwrap(), "refused");
        let mut seen = Vec::new();
        while let Ok(bytes) = upstream.recv_timeout(Duration::from_secs(1)) {
            seen.push(bytes);
        }
        (answer, seen)
    };
    let post = |port: u16| {
        format!(
            "POST http://fixture.nexus.invalid:{port}/submit HTTP/1.1\r\n\
             Host: fixture.nexus.invalid:{port}\r\nContent-Length: 6\r\n\r\nsecret"
        )
        .into_bytes()
    };
    let tunnel = |port: u16| {
        format!(
            "CONNECT fixture.nexus.invalid:{port} HTTP/1.1\r\n\
             Host: fixture.nexus.invalid:{port}\r\n\r\nsecret"
        )
        .into_bytes()
    };
    for yes in 0..3 {
        let (answer, seen) = run(&post, yes);
        assert!(answer.starts_with(b"HTTP/1.1 403"), "{yes}: {answer:?}");
        assert!(seen.iter().all(Vec::is_empty), "{yes}: {seen:?}");
    }
    // Live until the head is forwarded: the body is not.
    let (_, seen) = run(&post, 3);
    let seen: Vec<u8> = seen.concat();
    assert!(seen.starts_with(b"POST /submit HTTP/1.1\r\n"), "{seen:?}");
    assert!(!seen.windows(6).any(|w| w == b"secret"), "{seen:?}");
    for yes in 0..4 {
        let (answer, seen) = run(&tunnel, yes);
        assert!(
            answer.is_empty() || answer.starts_with(b"HTTP/1.1 "),
            "{yes}: {answer:?}"
        );
        assert!(seen.iter().all(Vec::is_empty), "{yes}: {seen:?}");
    }
}

/// A session that may no longer go out stops its proxy: new connections
/// are refused and an open tunnel carries nothing more.
#[test]
fn the_proxy_stops_when_its_session_may_no_longer_go_out() {
    use super::proxy::{BrowserProxy, OriginPolicy};
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    let (port, upstream) = recording_upstream();
    let live = Arc::new(AtomicBool::new(true));
    let proxy = {
        let live = live.clone();
        BrowserProxy::start(OriginPolicy {
            origins: vec![format!("https://fixture.nexus.invalid:{port}")],
            allow_private: true,
            resolver: Arc::new(Fixtures),
            live: Arc::new(move || live.load(Ordering::SeqCst)),
        })
        .unwrap()
    };
    let address: SocketAddr = format!("127.0.0.1:{}", proxy.port()).parse().unwrap();
    let mut client = std::net::TcpStream::connect(address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .write_all(
            format!(
                "CONNECT fixture.nexus.invalid:{port} HTTP/1.1\r\n\
                 Host: fixture.nexus.invalid:{port}\r\n\r\n"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut established = [0u8; 39];
    client.read_exact(&mut established).unwrap();
    assert_eq!(&established, b"HTTP/1.1 200 Connection Established\r\n\r\n");
    client.write_all(b"before").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    live.store(false, Ordering::SeqCst);
    client.write_all(b"after").unwrap();
    // The tunnel ends.
    let mut rest = Vec::new();
    let _ = client.read_to_end(&mut rest);
    assert_eq!(
        upstream.recv_timeout(Duration::from_secs(5)).unwrap(),
        b"before"
    );
    // The proxy keeps its port while the session lasts (no other process
    // can take it from the browser) and closes every new connection at
    // once, unserved: long after it saw the session end, it still does.
    std::thread::sleep(Duration::from_millis(300));
    let refused = proxy.refused();
    let mut late = std::net::TcpStream::connect(address).unwrap();
    late.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    late.write_all(format!("CONNECT fixture.nexus.invalid:{port} HTTP/1.1\r\n\r\n").as_bytes())
        .ok();
    let mut answer = Vec::new();
    let _ = late.read_to_end(&mut answer);
    assert!(answer.is_empty(), "{}", String::from_utf8_lossy(&answer));
    assert_eq!(proxy.refused(), refused + 1);
    // The port is released when the session ends.
    drop(proxy);
    let deadline = Instant::now() + Duration::from_secs(2);
    while std::net::TcpStream::connect(address).is_ok() {
        assert!(Instant::now() < deadline, "the proxy still listens");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(upstream.recv_timeout(Duration::from_secs(1)).is_err());
}

/// At most four sessions of one control run at once (the number the
/// design record states).
#[test]
fn sessions_are_counted_and_capped() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let counter = Arc::new(AtomicUsize::new(0));
    let slots: Vec<_> = (0..4)
        .map(|_| super::live::SessionSlot::take(&counter).expect("a slot"))
        .collect();
    assert!(
        super::live::SessionSlot::take(&counter).is_none(),
        "a fifth session"
    );
    assert_eq!(counter.load(Ordering::SeqCst), 4);
    drop(slots);
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    assert!(super::live::SessionSlot::take(&counter).is_some());
}

/// A machine browser policy, anywhere Chrome or Chromium reads one, refuses
/// the browser: a policy (`ProxyMode: direct`, a forced extension, cloud
/// enrollment) takes precedence over the session's proxy. An empty policy
/// directory is no policy; one that cannot be read, or is not a plain
/// directory, counts as one.
#[test]
fn a_machine_browser_policy_anywhere_the_browser_reads_one_refuses_it() {
    use super::{no_machine_policy, POLICY_CONFIGURED, POLICY_ROOTS};
    use std::os::unix::fs::PermissionsExt;
    let configured = Err(AuthorityError::Closed(POLICY_CONFIGURED));
    let root = temp_root("policies");
    let chrome = root.0.path().join("chrome");
    let chromium = root.0.path().join("chromium");
    let roots = vec![chrome.clone(), chromium.clone()];
    assert_eq!(no_machine_policy(&[]), Ok(()));
    assert_eq!(no_machine_policy(&roots), Ok(()), "no root exists");
    for kind in ["managed", "recommended", "enrollment"] {
        std::fs::create_dir_all(chromium.join(kind)).unwrap();
    }
    assert_eq!(
        no_machine_policy(&roots),
        Ok(()),
        "empty policy directories"
    );
    for (kind, file, text) in [
        ("managed", "proxy.json", r#"{"ProxyMode": "direct"}"#),
        ("recommended", "proxy.json", r#"{"ProxyMode": "direct"}"#),
        ("enrollment", "CloudManagementEnrollmentToken", "token"),
        (
            "managed",
            "harmless.json",
            r#"{"EncryptedClientHelloEnabled": false}"#,
        ),
    ] {
        let path = chromium.join(kind).join(file);
        std::fs::write(&path, text).unwrap();
        assert_eq!(no_machine_policy(&roots), configured, "{kind}/{file}");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(no_machine_policy(&roots), Ok(()));
    }
    // A policy place that is not a plain directory.
    std::fs::remove_dir(chromium.join("managed")).unwrap();
    std::fs::write(chromium.join("managed"), "{}").unwrap();
    assert_eq!(no_machine_policy(&roots), configured, "a file");
    std::fs::remove_file(chromium.join("managed")).unwrap();
    let elsewhere = root.0.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, chromium.join("managed")).unwrap();
    assert_eq!(no_machine_policy(&roots), configured, "a link");
    std::fs::remove_file(chromium.join("managed")).unwrap();
    std::fs::write(&chrome, "").unwrap();
    assert_eq!(
        no_machine_policy(&roots),
        configured,
        "a root that is a file"
    );
    std::fs::remove_file(&chrome).unwrap();
    // A policy directory that cannot be read (root reads anything: then
    // there is nothing to show).
    std::fs::create_dir(chromium.join("managed")).unwrap();
    std::fs::set_permissions(
        chromium.join("managed"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    if std::fs::read_dir(chromium.join("managed")).is_err() {
        assert_eq!(no_machine_policy(&roots), configured, "unreadable");
    }
    std::fs::set_permissions(
        chromium.join("managed"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_eq!(no_machine_policy(&roots), Ok(()));
    // The production browser looks in every place Chrome and Chromium read,
    // and on this host its answer is what those places hold.
    let production = Browser::new(root.0.clone());
    let expected: Vec<std::path::PathBuf> =
        POLICY_ROOTS.iter().map(std::path::PathBuf::from).collect();
    assert_eq!(production.policies, expected);
    let held = POLICY_ROOTS.iter().any(|root| {
        ["managed", "recommended", "enrollment"].iter().any(|kind| {
            let dir = std::path::Path::new(root).join(kind);
            std::fs::read_dir(&dir).map_or(dir.exists(), |mut entries| entries.next().is_some())
        })
    });
    assert_eq!(
        no_machine_policy(&production.policies).is_err(),
        held,
        "this host's machine policy"
    );
}

/// The production browser reaches no private or loopback address: its
/// sessions' proxy, with the production browser's own address policy and
/// resolver, refuses a granted loopback origin and sends nothing there.
#[test]
fn a_production_browser_session_reaches_no_private_address() {
    use super::proxy::{BrowserProxy, OriginPolicy};
    use std::io::{Read, Write};
    let root = temp_root("production-browser");
    let production = Browser::new(root.0.clone());
    let (port, upstream) = recording_upstream();
    let proxy = BrowserProxy::start(OriginPolicy {
        origins: vec![
            format!("https://127.0.0.1:{port}"),
            format!("http://127.0.0.1:{port}"),
        ],
        allow_private: production.allow_private,
        resolver: production.resolver.clone(),
        live: Arc::new(|| true),
    })
    .unwrap();
    for request in [
        format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
        format!("GET http://127.0.0.1:{port}/ HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"),
    ] {
        let mut client = std::net::TcpStream::connect(("127.0.0.1", proxy.port())).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        client.write_all(request.as_bytes()).unwrap();
        let mut answer = Vec::new();
        let _ = client.read_to_end(&mut answer);
        assert!(answer.starts_with(b"HTTP/1.1 403"), "{request}");
    }
    assert_eq!(proxy.opened(), 0);
    assert!(upstream.recv_timeout(Duration::from_secs(1)).is_err());
}

/// When the session ends (its proxy dropped) while its grant is still live,
/// an established tunnel ends within the proxy's poll bound and carries
/// nothing more: the proxy's own stop, not only the session's liveness,
/// ends it.
#[test]
fn an_open_tunnel_ends_with_its_session_within_the_poll_bound() {
    use super::proxy::{BrowserProxy, OriginPolicy};
    use std::io::{Read, Write};
    let (port, upstream) = recording_upstream();
    let proxy = BrowserProxy::start(OriginPolicy {
        origins: vec![format!("https://fixture.nexus.invalid:{port}")],
        allow_private: true,
        resolver: Arc::new(Fixtures),
        // The grant stays live throughout.
        live: Arc::new(|| true),
    })
    .unwrap();
    let mut client = std::net::TcpStream::connect(("127.0.0.1", proxy.port())).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    client
        .write_all(
            format!(
                "CONNECT fixture.nexus.invalid:{port} HTTP/1.1\r\n\
                 Host: fixture.nexus.invalid:{port}\r\n\r\n"
            )
            .as_bytes(),
        )
        .unwrap();
    let mut established = [0u8; 39];
    client.read_exact(&mut established).unwrap();
    assert_eq!(&established, b"HTTP/1.1 200 Connection Established\r\n\r\n");
    assert_eq!(proxy.opened(), 1);
    client.write_all(b"before").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let ended = Instant::now();
    drop(proxy);
    // The documented bound: every connection ends within one poll (250
    // ms) of the end; allowed here twice that and a margin.
    let mut rest = Vec::new();
    let _ = client.read_to_end(&mut rest);
    assert!(
        ended.elapsed() < Duration::from_millis(1500),
        "the tunnel outlived its session by {:?}",
        ended.elapsed()
    );
    let _ = client.write_all(b"after");
    assert_eq!(
        upstream.recv_timeout(Duration::from_secs(5)).unwrap(),
        b"before"
    );
}

/// A machine policy that could force direct connections, configured where
/// the browser reads one, refuses the session: when it is prepared, and
/// when it appears between the approval and the launch (the browser is
/// then never started). Nothing reaches the granted origin or any other.
#[test]
fn a_direct_mode_policy_refuses_the_session_before_any_connection() {
    use super::POLICY_CONFIGURED;
    let Some((browser, root)) = browser() else {
        return;
    };
    let policies = temp_root("policy-roots");
    let chrome = policies.0.path().join("chrome");
    std::fs::create_dir_all(chrome.join("managed")).unwrap();
    let browser = browser.with_policies(vec![chrome.clone()]);
    let server = server();
    let h = harness();
    let page = origin(&server);
    grant(&h, &browser, std::slice::from_ref(&page));
    let intent = BrowserIntent {
        start_url: format!("{page}/"),
        steps: vec![],
    };
    let direct = chrome.join("managed").join("proxy.json");
    std::fs::write(&direct, r#"{"ProxyMode": "direct"}"#).unwrap();
    assert_eq!(
        browser
            .prepare(h.control.authority(), &intent)
            .err()
            .unwrap(),
        AuthorityError::Closed(POLICY_CONFIGURED)
    );
    // Prepared and approved with no policy; the policy appears before the
    // launch.
    std::fs::remove_file(&direct).unwrap();
    let preparation = browser.prepare(h.control.authority(), &intent).unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    std::fs::write(&direct, r#"{"ProxyMode": "direct"}"#).unwrap();
    assert!(h.control.execute(view.id, &h.agent, h.run).is_err());
    let finished = h
        .evidence
        .records()
        .into_iter()
        .rev()
        .find(|r| r.commitment.as_deref() == Some(&view.id.to_string()) && r.outcome.is_some())
        .unwrap();
    assert_eq!(finished.failure, Some("refused"));
    assert!(
        format!("{:?}", finished.detail).contains("the browser was not started"),
        "{:?}",
        finished.detail
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(server.received().is_empty(), "nothing was reached");
    assert_eq!(std::fs::read_dir(root.0.path()).unwrap().count(), 0);
}

/// A browser that goes around its session's proxy (as one under a policy
/// forcing direct connections would) is refused, never reported as done,
/// and runs no step: the proxy saw none of its traffic. (It did reach the
/// page directly first, which is why the policy refusal must keep it from
/// starting at all.)
#[test]
fn a_browser_that_goes_around_its_proxy_is_refused() {
    let Some((browser, _root)) = browser() else {
        return;
    };
    let browser = browser.direct();
    let server = server();
    let h = harness();
    let page = format!("http://127.0.0.1:{}", server.address.port());
    grant(&h, &browser, std::slice::from_ref(&page));
    let result = run(
        &h,
        &browser,
        BrowserIntent {
            start_url: format!("{page}/"),
            steps: vec![BrowserStep::ExtractText {
                selector: "#title".into(),
            }],
        },
        true,
    );
    assert!(result.is_err(), "{result:?}");
    let finished = h
        .evidence
        .records()
        .into_iter()
        .rev()
        .find(|r| r.outcome.is_some())
        .unwrap();
    assert_eq!(finished.outcome, Some("failed"));
    assert_eq!(finished.failure, Some("refused"));
    assert!(
        format!("{:?}", finished.detail).contains("did not go through the session's proxy"),
        "{:?}",
        finished.detail
    );
    // The page was reached directly; no step ran.
    assert!(server.received().iter().any(|r| r.path == "/"));
}
