//! P3-C: the governed browser, below egress authority.
//!
//! A browser session is one commitment: a start URL and a bounded list of
//! typed steps (navigate, click, fill, press, wait, extract text). It runs
//! only under the owner's browser grant, which names the origins sessions
//! may reach and pins the browser executable's identity. The session is a
//! fresh, run-owned browser: the system Chrome (identity checked again
//! immediately before launch), headless, with a new profile in a private
//! scratch directory deleted when the session ends, driven over its
//! DevTools pipe (no debugging port), launched in its own process group
//! and killed with it. Every connection it makes goes through the session's
//! proxy ([`proxy`]), which admits only the granted origins and applies the
//! egress address policy, so pages, subresources, redirects and
//! page-initiated navigations cannot reach anything else; `file:` and
//! other local schemes are refused before launch; new windows are blocked
//! and closed; downloads are refused, or kept inside the session when the
//! grant allows them. Steps run fixed scripts with their arguments passed
//! as JSON data: no model-written script ever runs. Password and file
//! inputs are never filled. Navigation and reading are R1; clicking,
//! filling and pressing are R2.
//!
//! The proxy switch is only as strong as the browser's own configuration:
//! a machine policy (`ProxyMode`, a forced extension, cloud management)
//! takes precedence over it. So the browser does not run while any machine
//! policy is configured where Chrome or Chromium reads one
//! ([`POLICY_ROOTS`]): checked when a session is prepared and again
//! immediately before launch, a place that cannot be read counting as
//! configured. And a session whose browser did not go through its proxy
//! (no connection admitted once a page loaded) is refused, never reported
//! as done; that check only notices a bypass after it happened, so the
//! policy refusal is what keeps one from happening.

#[cfg(target_os = "linux")]
mod cdp;
#[cfg(target_os = "linux")]
mod proxy;

use crate::authority::commitment::{ExecutionGuard, FailureClass, PreparedAction, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{escaped, is_plain, quoted, wrapped};
use crate::authority::ids::{Digest, GrantId};
use crate::authority::policy::GrantScope;
use crate::authority::{Authority, AuthorityError};
use crate::control::{EffectOutput, PendingEffect, Preparation};
use crate::egress::destination::{Destination, Resolver, SystemResolver};
use crate::executable::{inspect, inspect_tree, ExecutableIdentity, Trust};
use crate::runtime_root::RuntimeRoot;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;

/// The browser Phase Three drives.
pub const CHROME: &str = "/opt/google/chrome/chrome";

/// Where Google Chrome, Chrome for Testing and Chromium read machine policy
/// on Linux. In each, `managed` and `recommended` hold policy files and
/// `enrollment` a cloud-management token (whose policies come from the
/// cloud); any of them can set the proxy over the session's own.
pub const POLICY_ROOTS: [&str; 4] = [
    "/etc/opt/chrome/policies",
    "/etc/opt/chrome_for_testing/policies",
    "/etc/chromium/policies",
    "/etc/chromium-browser/policies",
];
const POLICY_KINDS: [&str; 3] = ["managed", "recommended", "enrollment"];

const POLICY_CONFIGURED: &str =
    "a browser policy is configured on this machine (it can override the session's proxy)";

/// Fail closed while any machine policy is configured under `roots`: which
/// of a policy's settings could override the session's proxy is not judged
/// here. A root, or a policy directory, that cannot be read or is not a
/// plain directory counts as configured; an empty policy directory does
/// not.
fn no_machine_policy(roots: &[PathBuf]) -> Result<(), AuthorityError> {
    let configured = AuthorityError::Closed(POLICY_CONFIGURED);
    for root in roots {
        match std::fs::symlink_metadata(root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(meta) if meta.is_dir() => {}
            _ => return Err(configured),
        }
        for kind in POLICY_KINDS {
            let dir = root.join(kind);
            match std::fs::symlink_metadata(&dir) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Ok(meta) if meta.is_dir() => {}
                _ => return Err(configured),
            }
            let mut entries = std::fs::read_dir(&dir).map_err(|_| configured.clone())?;
            if entries.next().is_some() {
                return Err(configured);
            }
        }
    }
    Ok(())
}
const MAX_STEPS: usize = 32;
const MAX_SELECTOR: usize = 512;
const MAX_TEXT: usize = 4096;
const MAX_EXTRACT: usize = 64 * 1024;
/// The longest report a session returns.
const MAX_REPORT: usize = 256 * 1024;

/// A report cut to `MAX_REPORT` bytes on a character boundary (page text
/// is any UTF-8).
fn bounded_report(mut text: String) -> String {
    if text.len() > MAX_REPORT {
        let mut end = MAX_REPORT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}
const MAX_ORIGINS: usize = 16;
const SESSION_LIMIT: Duration = Duration::from_secs(120);
const STEP_TIMEOUT: Duration = Duration::from_secs(20);
const SESSION_TTL: Duration = Duration::from_secs(10 * 60);
/// Browser sessions one control runs at once; another is refused.
const MAX_SESSIONS: usize = 4;

/// One step, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserStep {
    Navigate {
        url: String,
    },
    Click {
        selector: String,
    },
    Fill {
        selector: String,
        text: String,
    },
    Press {
        selector: String,
        key: String,
    },
    WaitFor {
        #[serde(default)]
        selector: Option<String>,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    ExtractText {
        selector: String,
    },
}

impl BrowserStep {
    fn interacts(&self) -> bool {
        matches!(
            self,
            BrowserStep::Click { .. } | BrowserStep::Fill { .. } | BrowserStep::Press { .. }
        )
    }
}

/// A browser session request, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BrowserIntent {
    pub start_url: String,
    #[serde(default)]
    pub steps: Vec<BrowserStep>,
}

/// The browser actuator.
pub struct Browser {
    root: RuntimeRoot,
    resolver: Arc<dyn Resolver>,
    executable: &'static str,
    trust: Trust,
    allow_private: bool,
    /// Where machine policy is looked for (the browser refuses to run while
    /// one is configured).
    policies: Vec<PathBuf>,
    /// Tests only: launch the browser without its proxy, as a machine
    /// policy forcing direct connections would.
    #[cfg(test)]
    direct: bool,
    /// Sessions running now.
    sessions: Arc<AtomicUsize>,
}

/// Keys a step may press: (name, DOM key, virtual key code).
const KEYS: [(&str, &str, u32); 13] = [
    ("enter", "Enter", 13),
    ("tab", "Tab", 9),
    ("escape", "Escape", 27),
    ("backspace", "Backspace", 8),
    ("delete", "Delete", 46),
    ("space", " ", 32),
    ("arrowup", "ArrowUp", 38),
    ("arrowdown", "ArrowDown", 40),
    ("arrowleft", "ArrowLeft", 37),
    ("arrowright", "ArrowRight", 39),
    ("home", "Home", 36),
    ("end", "End", 35),
    ("pagedown", "PageDown", 34),
];

fn key(name: &str) -> Option<(&'static str, u32)> {
    let lower = name.to_ascii_lowercase();
    KEYS.iter()
        .find(|(n, _, _)| *n == lower)
        .map(|(_, key, code)| (*key, *code))
}

fn plain(text: &str, max: usize) -> bool {
    !text.is_empty()
        && text.chars().count() <= max
        && !text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
}

/// A selector naming one thing: plain, bounded, not a list and without
/// comments. A top-level `,` makes a selector list, and a comment can hide
/// one from this check. The page scripts also act only when the selector
/// matches exactly one element, whatever it is written as.
fn selector(text: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, c) {
            (_, '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth -= 1,
            (None, ',') if depth == 0 => return false,
            _ => {}
        }
    }
    plain(text, MAX_SELECTOR) && !text.contains('\n') && !text.contains("/*")
}

/// The browser's identity as it is now. A system browser's whole
/// installation must also be root-only: what it loads or runs from its own
/// directory cannot be replaced by another user either.
fn pinned(executable: &Path, trust: Trust) -> Result<ExecutableIdentity, AuthorityError> {
    let identity = inspect(executable, trust)?;
    if trust == Trust::System {
        if let Some(installation) = identity.path.parent() {
            inspect_tree(installation)?;
        }
    }
    Ok(identity)
}

fn origin_of(url: &str) -> Result<Destination, AuthorityError> {
    Destination::parse(url).map_err(|e| AuthorityError::InvalidAction(e.as_str()))
}

impl Browser {
    pub fn new(root: RuntimeRoot) -> Self {
        Self {
            root,
            resolver: Arc::new(SystemResolver),
            executable: CHROME,
            trust: Trust::System,
            allow_private: false,
            policies: POLICY_ROOTS.iter().map(PathBuf::from).collect(),
            #[cfg(test)]
            direct: false,
            sessions: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Tests only: a resolver of fixtures and loopback test servers, and no
    /// machine policy looked for (see `with_policies`).
    #[cfg(test)]
    pub(crate) fn for_tests(root: RuntimeRoot, resolver: Arc<dyn Resolver>) -> Self {
        Self {
            resolver,
            allow_private: true,
            policies: Vec::new(),
            ..Self::new(root)
        }
    }

    /// Tests only: look for machine policy under `roots`.
    #[cfg(test)]
    pub(crate) fn with_policies(mut self, roots: Vec<PathBuf>) -> Self {
        self.policies = roots;
        self
    }

    /// Tests only: launch without the session's proxy (as a policy forcing
    /// direct connections would).
    #[cfg(test)]
    pub(crate) fn direct(mut self) -> Self {
        self.direct = true;
        self
    }

    /// The scope the owner may grant: sessions limited to `origins`, with
    /// the browser pinned as it is now.
    pub fn grant_scope(
        &self,
        origins: &[String],
        downloads: bool,
    ) -> Result<GrantScope, AuthorityError> {
        if origins.is_empty() || origins.len() > MAX_ORIGINS {
            return Err(AuthorityError::InvalidAction("1 to 16 origins"));
        }
        let mut granted: Vec<String> = Vec::new();
        for origin in origins {
            let destination = origin_of(origin)?;
            if destination.url().path() != "/" || destination.url().query().is_some() {
                return Err(AuthorityError::InvalidAction(
                    "an origin has no path or query",
                ));
            }
            let text = destination.origin_text();
            if !granted.contains(&text) {
                granted.push(text);
            }
        }
        granted.sort();
        let identity = pinned(Path::new(self.executable), self.trust)?;
        Ok(GrantScope::Browser {
            origins: granted,
            downloads,
            executable: identity.path.display().to_string(),
            identity: identity.digest,
        })
    }

    /// Prepare a browser session.
    pub(crate) fn prepare(
        &self,
        authority: &Authority,
        intent: &BrowserIntent,
    ) -> Result<Preparation, AuthorityError> {
        if intent.steps.len() > MAX_STEPS {
            return Err(AuthorityError::InvalidAction("at most 32 browser steps"));
        }
        let start = origin_of(&intent.start_url)?;
        let mut needed = vec![start.origin_text()];
        for step in &intent.steps {
            match step {
                BrowserStep::Navigate { url } => needed.push(origin_of(url)?.origin_text()),
                BrowserStep::Click { selector: s } | BrowserStep::ExtractText { selector: s } => {
                    if !selector(s) {
                        return Err(AuthorityError::InvalidAction(
                            "a selector is plain bounded text",
                        ));
                    }
                }
                BrowserStep::Fill { selector: s, text } => {
                    if !selector(s) || !plain(text, MAX_TEXT) {
                        return Err(AuthorityError::InvalidAction(
                            "fill takes plain bounded text",
                        ));
                    }
                }
                BrowserStep::Press {
                    selector: s,
                    key: name,
                } => {
                    if !selector(s) || key(name).is_none() {
                        return Err(AuthorityError::InvalidAction(
                            "press takes a selector and a known key",
                        ));
                    }
                }
                BrowserStep::WaitFor {
                    selector: s,
                    timeout_ms,
                } => {
                    if s.as_deref().is_some_and(|s| !selector(s))
                        || timeout_ms.is_some_and(|t| t > 10_000)
                    {
                        return Err(AuthorityError::InvalidAction(
                            "wait for a plain selector, at most 10 s",
                        ));
                    }
                }
            }
        }
        let identity = pinned(Path::new(self.executable), self.trust)?;
        no_machine_policy(&self.policies)?;
        let (grant, origins, downloads) = covering_grant(authority, &needed, &identity.digest)?;
        let class = if intent.steps.iter().any(BrowserStep::interacts) {
            EffectClass::R2
        } else {
            EffectClass::R1
        };
        let target = Digest::of(
            "nexus.p3.browser.target.v1",
            &[identity.digest.as_bytes(), origins.join(" ").as_bytes()],
        );
        let canonical =
            serde_json::to_vec(intent).map_err(|_| AuthorityError::InvalidAction("intent"))?;
        let parameters = Digest::of("nexus.p3.browser.session.v1", &[&canonical]);
        // Every step, in full: an approval covers exactly what is listed.
        let mut summary = wrapped(&format!(
            "Browse {} ({} steps)",
            start.url(),
            intent.steps.len()
        ));
        summary.extend(wrapped(&format!("Reachable: {}", origins.join(", "))));
        for (index, step) in intent.steps.iter().enumerate() {
            let number = index + 1;
            match step {
                BrowserStep::Navigate { url } => {
                    summary.extend(wrapped(&format!("{number}. Go to {url}")))
                }
                BrowserStep::Click { selector } => {
                    summary.extend(wrapped(&format!("{number}. Click {selector}")))
                }
                BrowserStep::Fill { selector, text } => {
                    summary.extend(wrapped(&format!("{number}. Fill {selector}")));
                    summary.extend(quoted("with the text", text));
                }
                BrowserStep::Press { selector, key } => {
                    summary.extend(wrapped(&format!("{number}. Press {key} in {selector}")))
                }
                BrowserStep::WaitFor { selector, .. } => summary.extend(wrapped(&format!(
                    "{number}. Wait for {}",
                    selector.as_deref().unwrap_or("the page to load")
                ))),
                BrowserStep::ExtractText { selector } => {
                    summary.extend(wrapped(&format!("{number}. Read the text of {selector}")))
                }
            }
        }
        let shown_target = escaped(&format!("Browser session to {}", origins.join(", ")));
        let display = if is_plain(&shown_target) {
            shown_target
        } else {
            format!(
                "Browser session to {} origins (listed below)",
                origins.len()
            )
        };
        let action = PreparedAction {
            kind: CapabilityKind::Browser,
            class,
            operation: "browser.session",
            target: TargetIdentity {
                display,
                digest: target,
            },
            parameters,
            grants: vec![grant],
            leases: vec![],
            summary,
        };
        let effect = Session {
            executable: identity.path,
            identity: identity.digest,
            trust: self.trust,
            start: start.url().to_string(),
            steps: intent.steps.clone(),
            origins,
            downloads,
            allow_private: self.allow_private,
            resolver: self.resolver.clone(),
            policies: self.policies.clone(),
            #[cfg(test)]
            direct: self.direct,
            root: self.root.clone(),
            target,
            parameters,
            sessions: self.sessions.clone(),
        };
        Ok(Preparation {
            action,
            effect: Box::new(effect),
            ttl: SESSION_TTL,
        })
    }
}

/// The live browser grant covering every needed origin with the browser's
/// identity as it is now.
fn covering_grant(
    authority: &Authority,
    needed: &[String],
    identity: &Digest,
) -> Result<(GrantId, Vec<String>, bool), AuthorityError> {
    let mut stale = false;
    for grant in authority.grants().live_of(CapabilityKind::Browser) {
        if let GrantScope::Browser {
            origins,
            downloads,
            identity: pinned,
            ..
        } = &grant.scope
        {
            if needed.iter().all(|origin| origins.contains(origin)) {
                if pinned == identity {
                    return Ok((grant.id, origins.clone(), *downloads));
                }
                stale = true;
            }
        }
    }
    Err(if stale {
        AuthorityError::Closed("the browser changed since it was granted; grant it again")
    } else {
        AuthorityError::NoCoveringGrant
    })
}

struct Session {
    executable: PathBuf,
    identity: Digest,
    trust: Trust,
    start: String,
    steps: Vec<BrowserStep>,
    origins: Vec<String>,
    downloads: bool,
    allow_private: bool,
    resolver: Arc<dyn Resolver>,
    policies: Vec<PathBuf>,
    #[cfg(test)]
    direct: bool,
    root: RuntimeRoot,
    target: Digest,
    parameters: Digest,
    sessions: Arc<AtomicUsize>,
}

impl PendingEffect for Session {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        let now =
            pinned(&self.executable, self.trust).map_err(|_| AuthorityError::TargetChanged)?;
        if now.digest != self.identity {
            return Err(AuthorityError::TargetChanged);
        }
        Ok(self.target)
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        live::run(&self, guard)
    }
}

/// Fixed page scripts; their arguments are passed as JSON data.
#[cfg(target_os = "linux")]
mod scripts {
    pub const READY: &str = "()=>document.readyState";
    pub const LOCATION: &str = "()=>location.origin";
    pub const EXISTS: &str = "(s)=>document.querySelectorAll(s).length===1";
    pub const CLICK: &str = "(s)=>{const a=document.querySelectorAll(s);if(a.length!==1)return a.length?'ambiguous':'missing';const e=a[0];e.scrollIntoView({block:'center'});e.click();return'ok'}";
    pub const FILL: &str = "(s,t)=>{const a=document.querySelectorAll(s);if(a.length!==1)return a.length?'ambiguous':'missing';const e=a[0];e.focus();const k=(e.type||'').toLowerCase();if(k==='password'||k==='file')return'refused';e.value=t;e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));return'ok'}";
    pub const FOCUS: &str = "(s)=>{const a=document.querySelectorAll(s);if(a.length!==1)return a.length?'ambiguous':'missing';a[0].focus();return'ok'}";
    pub const TEXT: &str = "(s,n)=>{const a=document.querySelectorAll(s);return a.length===1?String(a[0].innerText).slice(0,n):null}";
}

#[cfg(target_os = "linux")]
mod live {
    use super::cdp::{Cdp, CdpError};
    use super::proxy::{BrowserProxy, OriginPolicy};
    use super::{
        key, no_machine_policy, scripts, BrowserStep, Session, MAX_EXTRACT, MAX_SESSIONS,
        POLICY_CONFIGURED, SESSION_LIMIT, STEP_TIMEOUT,
    };
    use crate::authority::commitment::{ExecutionGuard, FailureClass};
    use crate::authority::run::CancelToken;
    use crate::control::EffectOutput;
    use crate::launcher::{SessionProcess, SessionSpec};
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    type Failure = (FailureClass, String);

    fn failure(error: CdpError) -> Failure {
        match error {
            CdpError::Cancelled => (FailureClass::Actuator, "cancelled".into()),
            CdpError::Timeout => (
                FailureClass::Timeout,
                "the browser did not answer in time".into(),
            ),
            CdpError::Closed => (FailureClass::Unavailable, "the browser ended".into()),
            CdpError::Protocol(why) => (FailureClass::Actuator, why),
        }
    }

    /// One answer of a read-only probe in a wait. A probe that failed
    /// while the page was changing is no answer yet and is asked again: when
    /// the page navigates (by itself too), its document and the world the
    /// probe ran in go away, or the page moves to another process (a failed
    /// navigation's error page does), and the browser answers a call in
    /// flight with an error. The end of the browser, its silence and
    /// cancellation still end the wait. Only reads are asked again: an
    /// action is never repeated.
    pub(super) fn probed(answer: Result<bool, Failure>, cancelled: bool) -> Result<bool, Failure> {
        match answer {
            Err((FailureClass::Actuator, _)) if !cancelled => Ok(false),
            other => other,
        }
    }

    struct Page<'a> {
        cdp: &'a Cdp,
        session: String,
        target: String,
        cancel: &'a CancelToken,
        popups_closed: u32,
    }

    impl Page<'_> {
        fn call(&self, method: &str, params: Value) -> Result<Value, Failure> {
            self.cdp
                .call(
                    method,
                    params,
                    Some(&self.session),
                    STEP_TIMEOUT,
                    self.cancel,
                )
                .map_err(failure)
        }

        /// Run a fixed script with JSON arguments; its value. It runs in a
        /// fresh isolated world of the page's main frame: it shares the DOM,
        /// but nothing the page's own scripts redefine (prototypes,
        /// `document.querySelector`) changes what it sees or does.
        fn script(&self, function: &str, args: &[Value]) -> Result<Value, Failure> {
            let missing = |what: &str| (FailureClass::Actuator, what.to_string());
            let tree = self.call("Page.getFrameTree", json!({}))?;
            let frame = tree["frameTree"]["frame"]["id"]
                .as_str()
                .ok_or_else(|| missing("no page frame"))?
                .to_string();
            let world = self.call(
                "Page.createIsolatedWorld",
                json!({ "frameId": frame, "worldName": "nexus-governed" }),
            )?;
            let context = world["executionContextId"]
                .as_i64()
                .ok_or_else(|| missing("no isolated world"))?;
            let args: Vec<String> = args.iter().map(Value::to_string).collect();
            let result = self.call(
                "Runtime.evaluate",
                json!({
                    "expression": format!("({function})({})", args.join(",")),
                    "contextId": context,
                    "returnByValue": true,
                }),
            )?;
            if result.get("exceptionDetails").is_some() {
                return Err((FailureClass::Actuator, "a page script failed".into()));
            }
            Ok(result["result"]["value"].clone())
        }

        /// Ask a read-only `probe` until it answers yes or `limit` passes.
        fn wait(
            &self,
            probe: impl Fn(&Self) -> Result<bool, Failure>,
            limit: Duration,
        ) -> Result<bool, Failure> {
            let deadline = Instant::now() + limit;
            loop {
                if self.cancel.is_cancelled() {
                    return Err((FailureClass::Actuator, "cancelled".into()));
                }
                if probed(probe(self), self.cancel.is_cancelled())? {
                    return Ok(true);
                }
                if Instant::now() >= deadline {
                    return Ok(false);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }

        fn navigate(&self, url: &str) -> Result<Value, Failure> {
            let result = self.call("Page.navigate", json!({ "url": url }))?;
            if let Some(error) = result.get("errorText").and_then(Value::as_str) {
                return Ok(json!({ "navigate": url, "refused": error }));
            }
            let loaded = self.wait(
                |page| Ok(page.script(scripts::READY, &[])? == json!("complete")),
                STEP_TIMEOUT,
            )?;
            Ok(json!({ "navigate": url, "loaded": loaded }))
        }

        /// Close every page but the session's own, as the browser lists
        /// them now (no event a page could crowd out is relied on). Pages a
        /// page opened are counted as popups.
        fn close_popups(&mut self) -> Result<(), Failure> {
            drop(self.cdp.drain_events());
            let targets = self
                .cdp
                .call(
                    "Target.getTargets",
                    json!({}),
                    None,
                    STEP_TIMEOUT,
                    self.cancel,
                )
                .map_err(failure)?;
            for info in targets["targetInfos"].as_array().into_iter().flatten() {
                if info["type"] == "page" && info["targetId"] != json!(self.target) {
                    let _ = self.cdp.call(
                        "Target.closeTarget",
                        json!({ "targetId": info["targetId"] }),
                        None,
                        STEP_TIMEOUT,
                        self.cancel,
                    );
                    if info.get("openerId").is_some_and(|opener| !opener.is_null()) {
                        self.popups_closed += 1;
                    }
                }
            }
            Ok(())
        }

        fn step(&self, step: &BrowserStep) -> Result<(Value, bool), Failure> {
            Ok(match step {
                BrowserStep::Navigate { url } => {
                    let result = self.navigate(url)?;
                    let ok = result.get("refused").is_none();
                    (result, ok)
                }
                BrowserStep::Click { selector } => {
                    let result = self.script(scripts::CLICK, &[json!(selector)])?;
                    (
                        json!({ "click": selector, "result": result }),
                        result == json!("ok"),
                    )
                }
                BrowserStep::Fill { selector, text } => {
                    let result = self.script(scripts::FILL, &[json!(selector), json!(text)])?;
                    (
                        json!({ "fill": selector, "result": result }),
                        result == json!("ok"),
                    )
                }
                BrowserStep::Press {
                    selector,
                    key: name,
                } => {
                    let focused = self.script(scripts::FOCUS, &[json!(selector)])?;
                    if focused != json!("ok") {
                        return Ok((json!({ "press": name, "result": focused }), false));
                    }
                    let (dom_key, code) = key(name).expect("validated");
                    for kind in ["keyDown", "keyUp"] {
                        let mut event = json!({
                            "type": kind,
                            "key": dom_key,
                            "windowsVirtualKeyCode": code,
                        });
                        if kind == "keyDown" && dom_key == "Enter" {
                            event["text"] = json!("\r");
                        }
                        self.call("Input.dispatchKeyEvent", event)?;
                    }
                    (json!({ "press": name, "result": "ok" }), true)
                }
                BrowserStep::WaitFor {
                    selector,
                    timeout_ms,
                } => {
                    let limit = Duration::from_millis(timeout_ms.unwrap_or(5_000));
                    let found =
                        match selector {
                            Some(selector) => self.wait(
                                |page| {
                                    Ok(page.script(scripts::EXISTS, &[json!(selector)])?
                                        == json!(true))
                                },
                                limit,
                            )?,
                            None => self.wait(
                                |page| Ok(page.script(scripts::READY, &[])? == json!("complete")),
                                limit,
                            )?,
                        };
                    (json!({ "wait": selector, "found": found }), found)
                }
                BrowserStep::ExtractText { selector } => {
                    let text =
                        self.script(scripts::TEXT, &[json!(selector), json!(MAX_EXTRACT)])?;
                    (json!({ "text": text, "selector": selector }), true)
                }
            })
        }
    }

    /// A private directory for one session's temporary files, under the
    /// system's temporary directory so that socket paths in it stay short:
    /// created exclusively (mode 0700, random name), removed when dropped.
    struct SessionTemp(std::path::PathBuf);

    impl SessionTemp {
        fn create() -> std::io::Result<Self> {
            use std::os::unix::fs::DirBuilderExt;
            let mut bytes = [0u8; 8];
            getrandom::getrandom(&mut bytes).map_err(|_| std::io::Error::other("no randomness"))?;
            let path =
                std::path::PathBuf::from(format!("/tmp/nexus-browser-{}", hex::encode(bytes)));
            std::fs::DirBuilder::new().mode(0o700).create(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for SessionTemp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The browser's whole environment: a home and a temporary directory
    /// of the session's own (Chrome keeps its process-singleton socket in
    /// the temporary one), a fixed locale, and a `PATH` that is an empty
    /// directory of the session, so nothing a page opens (an external
    /// protocol handler) can find a program to start.
    pub(super) fn environment(
        home: std::path::PathBuf,
        temp: std::path::PathBuf,
        path: std::path::PathBuf,
    ) -> Vec<(String, std::ffi::OsString)> {
        vec![
            ("HOME".into(), home.into_os_string()),
            ("LANG".into(), "C.UTF-8".into()),
            ("PATH".into(), path.into_os_string()),
            ("TMPDIR".into(), temp.into_os_string()),
        ]
    }

    /// One running session of a control, counted while it lives.
    pub(super) struct SessionSlot(Arc<AtomicUsize>);

    impl SessionSlot {
        pub(super) fn take(sessions: &Arc<AtomicUsize>) -> Option<Self> {
            let running = sessions.fetch_add(1, Ordering::SeqCst);
            let slot = Self(sessions.clone());
            (running < MAX_SESSIONS).then_some(slot)
        }
    }

    impl Drop for SessionSlot {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// The switches that send every connection of the browser through the
    /// session's proxy, with no implicit bypass (not even loopback).
    fn proxy_switches(session: &Session, port: u16) -> Vec<String> {
        #[cfg(test)]
        if session.direct {
            return vec!["--no-proxy-server".into()];
        }
        let _ = session;
        vec![
            format!("--proxy-server=http://127.0.0.1:{port}"),
            "--proxy-bypass-list=<-loopback>".into(),
        ]
    }

    pub(super) fn run(session: &Session, guard: &ExecutionGuard) -> Result<EffectOutput, Failure> {
        let unavailable = |what: &str| (FailureClass::Unavailable, what.to_string());
        let cancelled = || (FailureClass::Actuator, "cancelled".to_string());
        if guard.is_cancelled() {
            return Err(cancelled());
        }
        let Some(_slot) = SessionSlot::take(&session.sessions) else {
            return Err(unavailable("too many browser sessions at once"));
        };
        let started = Instant::now();
        let scratch = session
            .root
            .scratch("browser")
            .map_err(|_| unavailable("no session directory"))?;
        let profile = scratch
            .subdir("profile")
            .map_err(|_| unavailable("no profile directory"))?;
        let home = scratch
            .subdir("home")
            .map_err(|_| unavailable("no home directory"))?;
        let no_programs = scratch
            .subdir("bin")
            .map_err(|_| unavailable("no session directory"))?;
        // Chrome's own temporary files (its process-singleton socket, whose
        // path must stay short) go in a private directory of this session,
        // removed when it ends (declared before the process, dropped after).
        let temp = SessionTemp::create().map_err(|_| unavailable("no temporary directory"))?;
        let downloads = if session.downloads {
            Some(
                scratch
                    .subdir("downloads")
                    .map_err(|_| unavailable("no download directory"))?,
            )
        } else {
            None
        };
        let proxy = BrowserProxy::start(OriginPolicy {
            origins: session.origins.clone(),
            allow_private: session.allow_private,
            resolver: session.resolver.clone(),
            live: guard.liveness(),
        })
        .map_err(|_| unavailable("the browser proxy did not start"))?;
        let (command_reader, command_writer) =
            std::io::pipe().map_err(|_| unavailable("no pipe"))?;
        let (reply_reader, reply_writer) = std::io::pipe().map_err(|_| unavailable("no pipe"))?;
        let mut args: Vec<String> = vec![
            "--headless".into(),
            "--remote-debugging-pipe".into(),
            format!("--user-data-dir={}", profile.display()),
        ];
        args.extend(proxy_switches(session, proxy.port()));
        args.extend([
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            "--disable-background-networking".into(),
            "--disable-component-update".into(),
            "--disable-default-apps".into(),
            "--disable-extensions".into(),
            "--disable-sync".into(),
            "--disable-client-side-phishing-detection".into(),
            "--disable-domain-reliability".into(),
            "--disable-breakpad".into(),
            "--no-pings".into(),
            "--mute-audio".into(),
            "--disable-gpu".into(),
            "--disable-quic".into(),
            "--block-new-web-contents".into(),
            "--force-webrtc-ip-handling-policy=disable_non_proxied_udp".into(),
            "--dns-prefetch-disable".into(),
            "--password-store=basic".into(),
            "--use-mock-keychain".into(),
            "--deny-permission-prompts".into(),
            "--disable-features=Translate,MediaRouter,OptimizationHints,AutofillServerCommunication".into(),
            "about:blank".into(),
        ]);
        // Immediately before launch: no machine policy can override the
        // proxy it is given.
        no_machine_policy(&session.policies).map_err(|_| {
            (
                FailureClass::Refused,
                format!("{POLICY_CONFIGURED}: the browser was not started"),
            )
        })?;
        let _process = SessionProcess::launch(SessionSpec {
            program: session.executable.clone(),
            args: args.into_iter().map(Into::into).collect(),
            env: environment(home, temp.0.clone(), no_programs),
            current_dir: scratch.path().to_path_buf(),
            stop_grace: None,
            inherit: vec![(command_reader.into(), 3), (reply_writer.into(), 4)],
        })
        .map_err(|_| unavailable("the browser could not start"))?;
        let cdp = Cdp::new(command_writer, reply_reader);
        let cancel = guard.cancel_token();
        let browser = |method: &str, params: Value| {
            cdp.call(method, params, None, Duration::from_secs(30), cancel)
                .map_err(failure)
        };
        browser("Browser.getVersion", json!({}))?;
        browser("Target.setDiscoverTargets", json!({ "discover": true }))?;
        let target = browser("Target.createTarget", json!({ "url": "about:blank" }))?["targetId"]
            .as_str()
            .ok_or((FailureClass::Actuator, "no page".to_string()))?
            .to_string();
        let attached = browser(
            "Target.attachToTarget",
            json!({ "targetId": target, "flatten": true }),
        )?;
        let page_session = attached["sessionId"]
            .as_str()
            .ok_or((FailureClass::Actuator, "no page session".to_string()))?
            .to_string();
        match &downloads {
            Some(dir) => browser(
                "Browser.setDownloadBehavior",
                json!({ "behavior": "allow", "downloadPath": dir.display().to_string() }),
            )?,
            None => browser("Browser.setDownloadBehavior", json!({ "behavior": "deny" }))?,
        };
        let mut page = Page {
            cdp: &cdp,
            session: page_session,
            target,
            cancel,
            popups_closed: 0,
        };
        page.call("Page.enable", json!({}))?;
        let mut results = Vec::new();
        let mut completed = true;
        let total = session.steps.len();
        // A failure says how far the session got, wherever it happens.
        let before = |(class, detail): (FailureClass, String)| {
            let at = if total == 0 {
                "at the start page".to_string()
            } else {
                format!("at the start page, before step 1 of {total}")
            };
            (class, format!("{detail} ({at})"))
        };
        if let Some(reason) = guard.lapse() {
            return Err(before((FailureClass::Refused, reason.into())));
        }
        let start = page.navigate(&session.start).map_err(before)?;
        let started_ok = start.get("refused").is_none();
        // The start page needed the network: a browser that did not come to
        // its proxy for it (or that loaded it with no connection admitted)
        // went around it. Nothing more runs, and the session is refused.
        if proxy.seen() == 0 || (started_ok && proxy.opened() == 0) {
            return Err(before((
                FailureClass::Refused,
                "the browser did not go through the session's proxy".into(),
            )));
        }
        results.push(start);
        if started_ok {
            for (index, step) in session.steps.iter().enumerate() {
                // A failure says how far the session got: the steps before
                // this one ran.
                let at = |(class, detail): (FailureClass, String)| {
                    (
                        class,
                        format!("{detail} (at step {} of {total})", index + 1),
                    )
                };
                if cancel.is_cancelled() {
                    return Err(at(cancelled()));
                }
                // A grant revoked or expired, or a policy change, ends the
                // session before its next step, and says which.
                if let Some(reason) = guard.lapse() {
                    return Err(at((FailureClass::Refused, reason.into())));
                }
                if started.elapsed() > SESSION_LIMIT {
                    return Err(at((
                        FailureClass::Timeout,
                        "the session ran out of time".into(),
                    )));
                }
                page.close_popups().map_err(at)?;
                let (result, ok) = page.step(step).map_err(at)?;
                results.push(result);
                if !ok {
                    completed = false;
                    break;
                }
            }
        } else {
            completed = false;
        }
        // The steps that ran (the start page is the first result).
        let ran = results.len() - 1;
        let after = |(class, detail): (FailureClass, String)| {
            let at = if ran == 0 {
                "at the start page".to_string()
            } else {
                format!("after step {ran} of {total}")
            };
            (class, format!("{detail} ({at})"))
        };
        page.close_popups().map_err(after)?;
        // A grant that ended during the last step may have cut its traffic:
        // the session is not reported as done.
        if let Some(reason) = guard.lapse() {
            return Err(after((FailureClass::Refused, reason.into())));
        }
        let origin = page.script(scripts::LOCATION, &[]).unwrap_or(Value::Null);
        let mut downloaded = Vec::new();
        if let Some(dir) = &downloads {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten().take(32) {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    downloaded.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "bytes": size,
                    }));
                }
            }
        }
        let report = json!({
            "steps": results,
            "completed": completed,
            "final_origin": origin,
            "popups_closed": page.popups_closed,
            "downloads": downloaded,
        });
        let text = super::bounded_report(report.to_string());
        Ok(EffectOutput {
            text: Some(text),
            bytes: None,
            meta: vec![
                ("steps".into(), session.steps.len().to_string()),
                ("completed".into(), completed.to_string()),
                ("connections_opened".into(), proxy.opened().to_string()),
                ("connections_admitted".into(), proxy.admitted().to_string()),
                ("connections_refused".into(), proxy.refused().to_string()),
                ("popups_closed".into(), page.popups_closed.to_string()),
                ("downloads".into(), downloaded.len().to_string()),
            ],
        })
    }
}

#[cfg(not(target_os = "linux"))]
mod live {
    use super::Session;
    use crate::authority::commitment::{ExecutionGuard, FailureClass};
    use crate::control::EffectOutput;

    pub(super) fn run(
        _session: &Session,
        _guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        Err((
            FailureClass::Unavailable,
            "the governed browser is available on Linux only".into(),
        ))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
