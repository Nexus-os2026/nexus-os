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
/// any session: the hosted CI runner image makes `/opt` world-writable.
/// There the refusal is checked, exactly, along with the reason for it,
/// and the live sessions are skipped.
fn browser() -> Option<(Browser, TempRoot)> {
    if !std::path::Path::new(super::CHROME).exists() {
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
    h.control.execute(view.id, &h.agent, h.run)
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
    let env = super::live::environment("/session/home".into(), "/session/tmp".into());
    let env: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.to_str().unwrap()))
        .collect();
    assert_eq!(
        env,
        [
            ("HOME", "/session/home"),
            ("LANG", "C.UTF-8"),
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
    for list in ["#safe, #delete-account", "a,b", "#x , #y"] {
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
