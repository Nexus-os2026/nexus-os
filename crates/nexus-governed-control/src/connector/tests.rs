//! Connectors end to end: a fixture connector against a loopback server
//! (read R1, write-like R2), and the migrated production read path proven
//! through a capturing transport, all without real credentials.

use super::catalog::{self, CONNECTOR_SCOPE};
use super::{
    only_fields, text_field, Connector, ConnectorIntent, ConnectorOperation, Connectors,
    OperationRequest,
};
use crate::authority::commitment::CommitmentState;
use crate::authority::effect::EffectClass;
use crate::authority::run::CancelToken;
use crate::authority::AuthorityError;
use crate::broker::tests::{FakeVault, SECRET, SPEC};
use crate::broker::{CredentialBroker, NoVault, SecretSource};
use crate::control::EffectOutput;
use crate::egress::destination::{Destination, Resolver, SystemResolver};
use crate::egress::transport::{
    HttpResponse, HttpTransport, Method, PinnedRequest, Transport, TransportError,
};
use crate::egress::{Egress, EgressLimits};
use crate::harness_tests::{harness, Harness, Reply, TestServer, Yes};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn notes_list(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &[])?;
    Ok(OperationRequest {
        path: "/notes?limit=5".into(),
        body: None,
        content_type: None,
        summary: vec!["List notes".into()],
    })
}

fn notes_create(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["text"])?;
    let text = text_field(input, "text", 200)?;
    Ok(OperationRequest {
        path: "/notes".into(),
        body: Some(json!({ "text": text }).to_string()),
        content_type: Some("application/json"),
        summary: vec![format!("Create a note: {text}")],
    })
}

fn escape_to_another_host(_: &Value) -> Result<OperationRequest, AuthorityError> {
    Ok(OperationRequest {
        path: ".evil.example/steal".into(),
        body: None,
        content_type: None,
        summary: vec![],
    })
}

fn escape_to_another_port(_: &Value) -> Result<OperationRequest, AuthorityError> {
    Ok(OperationRequest {
        path: ":8443/steal".into(),
        body: None,
        content_type: None,
        summary: vec![],
    })
}

/// A request over the egress body bound (it builds, then fails to prepare).
fn notes_import(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &[])?;
    Ok(OperationRequest {
        path: "/notes/import".into(),
        body: Some("x".repeat(70 * 1024)),
        content_type: Some("text/plain"),
        summary: vec!["Import notes".into()],
    })
}

fn fixture(server: &TestServer) -> Connector {
    Connector {
        id: "fixture",
        origin: server.origin(),
        allow_private: true,
        operations: vec![
            ConnectorOperation {
                id: "fixture.notes.list",
                class: EffectClass::R1,
                method: Method::Get,
                credential: Some(SPEC),
                build: notes_list,
                destination: None,
            },
            ConnectorOperation {
                id: "fixture.notes.create",
                class: EffectClass::R2,
                method: Method::Post,
                credential: Some(SPEC),
                build: notes_create,
                destination: None,
            },
            ConnectorOperation {
                id: "fixture.notes.import",
                class: EffectClass::R2,
                method: Method::Post,
                credential: Some(SPEC),
                build: notes_import,
                destination: None,
            },
        ],
    }
}

fn connectors(h: &Harness, connector: Connector, vault: Arc<dyn SecretSource>) -> Connectors {
    let broker = CredentialBroker::new(h.control.authority(), vault);
    Connectors::new(vec![connector], Arc::new(Egress::system()), broker)
}

fn grant(h: &Harness, connectors: &Connectors, connector: &str, operations: &[&str]) {
    let operations: Vec<String> = operations.iter().map(|o| o.to_string()).collect();
    let scope = connectors
        .grant_scope(connector, "owner@example.com", &operations)
        .unwrap();
    h.control
        .authority()
        .grants()
        .request(scope, Duration::from_secs(600), &Yes::new(true))
        .unwrap();
}

fn run(
    h: &Harness,
    connectors: &Connectors,
    operation: &str,
    input: Value,
    approve: bool,
) -> Result<EffectOutput, AuthorityError> {
    let intent = ConnectorIntent {
        operation: operation.into(),
        input,
    };
    let view = connectors.propose(&h.control, &h.agent, h.run, &intent)?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(approve))?;
    h.control.execute(view.id, &h.agent, h.run)
}

#[test]
fn a_read_carries_the_leased_credential_and_never_leaks_it() {
    // The server echoes the Authorization header back.
    let server = TestServer::start(|request| {
        Reply::ok(&format!(
            "{{\"notes\":[],\"seen\":\"{}\"}}",
            request.header("authorization").unwrap_or_default()
        ))
    });
    let h = harness();
    let connectors = connectors(&h, fixture(&server), Arc::new(FakeVault(AtomicU32::new(0))));
    grant(&h, &connectors, "fixture", &["fixture.notes.list"]);
    let out = run(&h, &connectors, "fixture.notes.list", json!({}), false).unwrap();
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].path, "/notes?limit=5");
    assert_eq!(
        received[0].header("authorization"),
        Some(format!("Bearer {SECRET}").as_str()),
        "the secret reached the destination, in its header"
    );
    let text = out.text.unwrap();
    assert!(!text.contains(SECRET), "the echo was redacted: {text}");
    assert!(text.contains("[redacted]"));
    assert!(out.meta.iter().any(|(k, _)| k == "credential_redacted"));
    let everything = format!("{:?}", h.evidence.records());
    assert!(!everything.contains(SECRET));
    // Its commitment was a connector operation, R1, listing one lease.
    let views = h.control.authority().commitments().views_of_run(h.run);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].operation, "fixture.notes.list");
    assert_eq!(views[0].class, EffectClass::R1);
    assert!(!format!("{views:?}").contains(SECRET));
}

#[test]
fn a_write_is_r2_and_sends_exactly_what_the_owner_approved() {
    let server = TestServer::start(|_| Reply::ok("{\"id\":1}"));
    let h = harness();
    let connectors = connectors(&h, fixture(&server), Arc::new(FakeVault(AtomicU32::new(0))));
    grant(&h, &connectors, "fixture", &["fixture.notes.create"]);
    let intent = ConnectorIntent {
        operation: "fixture.notes.create".into(),
        input: json!({ "text": "buy milk" }),
    };
    let preparation = connectors
        .prepare(h.control.authority(), &h.agent, h.run, &intent, None)
        .unwrap();
    assert_eq!(preparation.action.class, EffectClass::R2);
    assert!(preparation
        .action
        .summary
        .iter()
        .any(|line| line.contains("Create a note: buy milk")));
    // Declined: nothing is sent and the lease ends.
    assert_eq!(
        run(
            &h,
            &connectors,
            "fixture.notes.create",
            json!({ "text": "buy milk" }),
            false
        )
        .unwrap_err(),
        AuthorityError::Declined
    );
    assert!(server.received().is_empty());
    // Approved: exactly that body.
    run(
        &h,
        &connectors,
        "fixture.notes.create",
        json!({ "text": "buy milk" }),
        true,
    )
    .unwrap();
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "POST");
    assert_eq!(received[0].body, br#"{"text":"buy milk"}"#);
}

#[test]
fn operations_need_a_grant_naming_them_and_exact_inputs() {
    let server = TestServer::start(|_| Reply::ok("{}"));
    let h = harness();
    let connectors = connectors(&h, fixture(&server), Arc::new(FakeVault(AtomicU32::new(0))));
    assert_eq!(
        run(&h, &connectors, "fixture.notes.list", json!({}), false).unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    grant(&h, &connectors, "fixture", &["fixture.notes.list"]);
    assert_eq!(
        run(
            &h,
            &connectors,
            "fixture.notes.create",
            json!({ "text": "x" }),
            true
        )
        .unwrap_err(),
        AuthorityError::NoCoveringGrant,
        "a grant for reads does not cover writes"
    );
    assert!(matches!(
        run(
            &h,
            &connectors,
            "fixture.notes.list",
            json!({ "extra": 1 }),
            false
        )
        .unwrap_err(),
        AuthorityError::InvalidAction(_)
    ));
    assert_eq!(
        run(&h, &connectors, "telegram.send", json!({}), true).unwrap_err(),
        AuthorityError::Closed("no such connector operation")
    );
    assert!(server.received().is_empty());
    // Grant scopes are checked against the catalog.
    assert!(connectors
        .grant_scope("fixture", "a b", &["fixture.notes.list".into()])
        .is_err());
    assert!(connectors
        .grant_scope("nope", "a", &["fixture.notes.list".into()])
        .is_err());
    assert!(connectors
        .grant_scope("fixture", "a", &["fixture.nope".into()])
        .is_err());
}

#[test]
fn an_operation_never_leaves_its_connectors_origin() {
    let h = harness();
    let connector = Connector {
        id: "fixture",
        origin: "https://fixture.invalid".into(),
        allow_private: false,
        operations: vec![
            ConnectorOperation {
                id: "fixture.host",
                class: EffectClass::R1,
                method: Method::Get,
                credential: None,
                build: escape_to_another_host,
                destination: None,
            },
            ConnectorOperation {
                id: "fixture.port",
                class: EffectClass::R1,
                method: Method::Get,
                credential: None,
                build: escape_to_another_port,
                destination: None,
            },
        ],
    };
    let connectors = connectors(&h, connector, Arc::new(NoVault));
    grant(
        &h,
        &connectors,
        "fixture",
        &["fixture.host", "fixture.port"],
    );
    for operation in ["fixture.host", "fixture.port"] {
        assert_eq!(
            run(&h, &connectors, operation, json!({}), false).unwrap_err(),
            AuthorityError::InvalidAction("an operation stays on its connector's origin"),
            "{operation}"
        );
    }
}

#[test]
fn without_a_vault_a_credentialed_operation_fails_closed_and_sends_nothing() {
    let server = TestServer::start(|_| Reply::ok("{}"));
    let h = harness();
    let connectors = connectors(&h, fixture(&server), Arc::new(NoVault));
    grant(&h, &connectors, "fixture", &["fixture.notes.list"]);
    assert!(run(&h, &connectors, "fixture.notes.list", json!({}), false).is_err());
    assert!(server.received().is_empty());
    let views = h.control.authority().commitments().views_of_run(h.run);
    assert_eq!(views[0].state, CommitmentState::Failed);
}

/// What a captured request was: method, url, credential header, and
/// whether it was pinned to the checked address.
type Captured = (String, String, Option<String>, bool);

/// Captures the request instead of sending it.
struct Capture(Mutex<Vec<Captured>>);

impl Transport for Capture {
    fn exchange(
        &self,
        request: PinnedRequest,
        _cancel: &CancelToken,
    ) -> Result<HttpResponse, TransportError> {
        let secret = request
            .secret
            .as_ref()
            .map(|s| format!("{}: {}", s.name, *s.value));
        self.0.lock().unwrap().push((
            request.method.as_str().to_string(),
            request.url.to_string(),
            secret,
            request
                .addresses
                .iter()
                .all(|a| a.ip().to_string() == "142.250.0.1"),
        ));
        Ok(HttpResponse {
            status: 200,
            peer: None,
            content_type: Some("application/json".into()),
            location: None,
            body: br#"{"messages":[]}"#.to_vec(),
            redacted: false,
        })
    }
}

struct Public;
impl Resolver for Public {
    fn resolve(&self, _host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        Ok(vec![SocketAddr::new("142.250.0.1".parse().unwrap(), port)])
    }
}

struct GmailVault;
impl SecretSource for GmailVault {
    fn read(
        &self,
        scope: &str,
        name: &str,
    ) -> Result<zeroize::Zeroizing<String>, crate::broker::SecretUnavailable> {
        assert_eq!((scope, name), (CONNECTOR_SCOPE, "gmail.access_token"));
        Ok(zeroize::Zeroizing::new("ya29.fixture-token".into()))
    }
}

#[test]
fn the_migrated_gmail_read_path_runs_through_the_pipeline() {
    let h = harness();
    let capture = Arc::new(Capture(Mutex::new(Vec::new())));
    let egress = Arc::new(Egress::new(
        Arc::new(Public),
        capture.clone(),
        EgressLimits::default(),
    ));
    let broker = CredentialBroker::new(h.control.authority(), Arc::new(GmailVault));
    let connectors = Connectors::production(egress, broker);
    grant(&h, &connectors, "gmail", &["gmail.messages.list"]);
    let out = run(
        &h,
        &connectors,
        "gmail.messages.list",
        json!({ "folder": "sent" }),
        false,
    )
    .unwrap();
    assert_eq!(out.text.as_deref(), Some(r#"{"messages":[]}"#));
    let sent = capture.0.lock().unwrap().clone();
    assert_eq!(
        sent,
        vec![(
            "GET".to_string(),
            "https://gmail.googleapis.com/gmail/v1/users/me/messages?labelIds=SENT&maxResults=20"
                .to_string(),
            Some("authorization: Bearer ya29.fixture-token".to_string()),
            true,
        )]
    );
    assert!(!format!("{:?}", h.evidence.records()).contains("ya29"));
}

#[test]
fn the_production_catalog_is_sound() {
    let mut ids = std::collections::BTreeSet::new();
    for connector in catalog::production() {
        let origin = Destination::parse(&connector.origin).unwrap();
        assert_eq!(origin.scheme(), "https", "{}", connector.id);
        assert!(!connector.allow_private, "{}", connector.id);
        assert!(
            crate::egress::destination::resolve_checked(&origin, false, &Public).is_ok(),
            "{}",
            connector.id
        );
        for operation in &connector.operations {
            assert!(ids.insert(operation.id), "{} is unique", operation.id);
            assert!(operation.id.starts_with(&format!("{}.", connector.id)));
            let credential = operation
                .credential
                .expect("every production operation is credentialed");
            assert_eq!(
                credential.scope, CONNECTOR_SCOPE,
                "the verified vault scope"
            );
            // Writes are R2, reads are R1, and nothing is classed below its method.
            match operation.method {
                Method::Get => assert_eq!(operation.class, EffectClass::R1, "{}", operation.id),
                _ => assert_eq!(operation.class, EffectClass::R2, "{}", operation.id),
            }
            // Unknown input fields are refused by every builder.
            assert!(
                (operation.build)(&json!({ "unexpected": true })).is_err(),
                "{}",
                operation.id
            );
        }
    }
    assert!(
        !ids.iter().any(|id| id.starts_with("telegram")),
        "Telegram stays closed"
    );
    assert_eq!(ids.len(), 15);
}

#[test]
fn migrated_sends_are_exact_and_refuse_header_injection() {
    let connectors: Vec<Connector> = catalog::production();
    let gmail_send = connectors
        .iter()
        .flat_map(|c| c.operations.iter())
        .find(|op| op.id == "gmail.messages.send")
        .unwrap();
    let request = (gmail_send.build)(&json!({
        "to": "friend@example.com",
        "subject": "Hello",
        "body": "Line one\nLine two"
    }))
    .unwrap();
    let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
    let raw = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        body["raw"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(raw).unwrap(),
        "To: friend@example.com\r\nSubject: Hello\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\nLine one\nLine two"
    );
    for bad in [
        json!({ "to": "a@b.com\r\nBcc: x@y.com", "subject": "s", "body": "b" }),
        json!({ "to": "a@b.com", "subject": "s\nBcc: x@y.com", "body": "b" }),
        json!({ "to": "not-an-address", "subject": "s", "body": "b" }),
        json!({ "to": "a@b.com", "subject": "s", "body": "b", "bcc": "x@y.com" }),
    ] {
        assert!((gmail_send.build)(&bad).is_err(), "{bad}");
    }
    // The resolver and transport used above are the real ones elsewhere.
    let _ = (SystemResolver, HttpTransport);
}

/// A send names one bare recipient, shown whole on its own line, and shows
/// the message's every line; nothing can hide where it goes.
#[test]
fn a_send_shows_one_bare_recipient_and_its_whole_text() {
    use crate::authority::evidence::is_plain;
    let catalog: Vec<Connector> = catalog::production();
    let text = format!(
        "Hello\nTo: someone-else@example.com\n{}\nBye",
        "z".repeat(700)
    );
    let domain = format!(
        "{}.{}.{}.example",
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(49)
    );
    let longest = format!("{}@{domain}", "a".repeat(64));
    assert_eq!(longest.len(), 250);
    for id in ["gmail.messages.send", "outlook.messages.send"] {
        let send = catalog
            .iter()
            .flat_map(|c| c.operations.iter())
            .find(|op| op.id == id)
            .unwrap();
        for bad in [
            "\"Accounts Payable\" <a@evil.example>".to_string(),
            "<a@evil.example>".into(),
            "a@ok.example, b@evil.example".into(),
            "a@ok.example;b@evil.example".into(),
            "a b@ok.example".into(),
            "a@ok.example (note)".into(),
            "\"a\"@ok.example".into(),
            "a@[127.0.0.1]".into(),
            "a..b@ok.example".into(),
            "a@-ok.example".into(),
            "a@localhost".into(),
            format!("{}@ok.example", "a".repeat(65)),
            format!("{longest}x"),
        ] {
            assert!(
                (send.build)(&json!({ "to": bad, "subject": "s", "body": "b" })).is_err(),
                "{id}: {bad}"
            );
        }
        let request =
            (send.build)(&json!({ "to": longest, "subject": "Hi", "body": text })).unwrap();
        let summary = &request.summary;
        assert!(summary.iter().all(|l| is_plain(l)), "{id}: {summary:?}");
        assert!(summary.contains(&format!("To: {longest}")), "{id}");
        let at = summary
            .iter()
            .position(|l| l.starts_with("Message ("))
            .unwrap();
        let mut rebuilt: Vec<String> = Vec::new();
        for line in &summary[at + 1..] {
            if let Some(more) = line.strip_prefix("│↳ ") {
                rebuilt.last_mut().unwrap().push_str(more);
            } else {
                rebuilt.push(line.strip_prefix("│ ").unwrap().to_string());
            }
        }
        assert_eq!(rebuilt.join("\n"), text, "{id}");
    }
}

/// An operation whose preparation fails after its lease was issued (here a
/// body over the request bound) ends that lease: nothing is left to hold.
#[test]
fn a_failed_preparation_ends_its_lease() {
    let server = TestServer::start(|_| Reply::ok("{}"));
    let h = harness();
    let broker = CredentialBroker::new(
        h.control.authority(),
        Arc::new(FakeVault(AtomicU32::new(0))),
    );
    let connectors = Connectors::new(
        vec![fixture(&server)],
        Arc::new(Egress::system()),
        broker.clone(),
    );
    grant(&h, &connectors, "fixture", &["fixture.notes.import"]);
    let issued_before = broker.issued();
    assert_eq!(
        run(&h, &connectors, "fixture.notes.import", json!({}), true).unwrap_err(),
        AuthorityError::InvalidAction("request body too large")
    );
    let issued = broker.issued();
    assert_eq!(
        issued.len(),
        issued_before.len() + 1,
        "one lease was issued"
    );
    assert!(issued.iter().all(|lease| !broker.is_live(*lease)));
    assert!(server.received().is_empty());
}

/// A long search query is shown on as many lines as it needs, never
/// refused for its length up to the operation's own bound.
#[test]
fn a_long_search_query_is_shown_whole() {
    use crate::authority::evidence::is_plain;
    let connectors: Vec<Connector> = catalog::production();
    for id in ["gmail.messages.search", "outlook.messages.search"] {
        let search = connectors
            .iter()
            .flat_map(|c| c.operations.iter())
            .find(|op| op.id == id)
            .unwrap();
        let query = "invoice ".repeat(31);
        let request = (search.build)(&json!({ "query": query.trim_end() })).unwrap();
        assert!(request.summary.len() > 1, "{:?}", request.summary);
        assert!(request.summary.iter().all(|l| is_plain(l)), "{id}");
        let shown: String = request
            .summary
            .iter()
            .map(|l| l.strip_prefix("↳ ").unwrap_or(l))
            .collect();
        assert_eq!(shown, format!("Search messages for: {}", query.trim_end()));
    }
}

/// A Slack and Discord API stand-in: answers by path (the first entry whose
/// path starts the request's) from a table the test can change, and records
/// every request: method, URL, credential header.
struct Api {
    answers: Mutex<Vec<(&'static str, u16, String)>>,
    sent: Mutex<Vec<(String, String, Option<String>)>>,
}

impl Api {
    fn new(answers: &[(&'static str, Value)]) -> Arc<Self> {
        let api = Arc::new(Self {
            answers: Mutex::new(Vec::new()),
            sent: Mutex::new(Vec::new()),
        });
        for (path, body) in answers {
            api.answer(path, body.clone());
        }
        api
    }

    fn answer(&self, path: &'static str, body: Value) {
        self.answer_with(path, 200, body.to_string());
    }

    fn answer_with(&self, path: &'static str, status: u16, body: String) {
        let mut answers = self.answers.lock().unwrap();
        answers.retain(|(p, _, _)| *p != path);
        answers.push((path, status, body));
    }

    fn sent(&self) -> Vec<(String, String, Option<String>)> {
        self.sent.lock().unwrap().clone()
    }

    fn posts(&self) -> usize {
        self.sent().iter().filter(|(m, _, _)| m == "POST").count()
    }
}

impl Transport for Api {
    fn exchange(
        &self,
        request: PinnedRequest,
        _cancel: &CancelToken,
    ) -> Result<HttpResponse, TransportError> {
        let path = match request.url.query() {
            Some(query) => format!("{}?{query}", request.url.path()),
            None => request.url.path().to_string(),
        };
        self.sent.lock().unwrap().push((
            request.method.as_str().to_string(),
            request.url.to_string(),
            request
                .secret
                .as_ref()
                .map(|s| format!("{}: {}", s.name, *s.value)),
        ));
        let (status, body) = self
            .answers
            .lock()
            .unwrap()
            .iter()
            .find(|(p, _, _)| path.starts_with(p))
            .map(|(_, status, body)| (*status, body.clone()))
            .unwrap_or((404, "{}".into()));
        Ok(HttpResponse {
            status,
            peer: None,
            content_type: Some("application/json".into()),
            location: (status == 302).then(|| "https://elsewhere.example/".to_string()),
            body: body.into_bytes(),
            redacted: false,
        })
    }
}

/// The Slack and Discord tokens.
struct ChatVault;
impl SecretSource for ChatVault {
    fn read(
        &self,
        scope: &str,
        name: &str,
    ) -> Result<zeroize::Zeroizing<String>, crate::broker::SecretUnavailable> {
        assert_eq!(scope, CONNECTOR_SCOPE);
        match name {
            "slack.bot_token" => Ok(zeroize::Zeroizing::new("xoxb-fixture".into())),
            "discord.bot_token" => Ok(zeroize::Zeroizing::new("discord-fixture".into())),
            _ => Err(crate::broker::SecretUnavailable::NotFound),
        }
    }
}

/// Records what the owner was asked to approve.
struct Shown(Mutex<Option<crate::authority::approval::ActionConfirmation>>);
impl crate::authority::approval::ControlConfirmer for Shown {
    fn confirm_action(&self, request: &crate::authority::approval::ActionConfirmation) -> bool {
        *self.0.lock().unwrap() = Some(request.clone());
        true
    }
    fn confirm_grant(&self, _: &crate::authority::approval::GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &crate::authority::approval::ResumeConfirmation) -> bool {
        false
    }
}

fn chat(h: &Harness, api: &Arc<Api>) -> Connectors {
    let egress = Arc::new(Egress::new(
        Arc::new(Public),
        api.clone(),
        EgressLimits::default(),
    ));
    let broker = CredentialBroker::new(h.control.authority(), Arc::new(ChatVault));
    Connectors::production(egress, broker)
}

fn slack_answers(name: &str) -> Vec<(&'static str, Value)> {
    vec![
        (
            "/api/auth.test",
            json!({ "ok": true, "team": "Acme", "team_id": "T0123", "user_id": "U9" }),
        ),
        (
            "/api/conversations.info",
            json!({ "ok": true, "channel": {
                "id": "C0456", "name": name, "is_channel": true, "is_private": false
            } }),
        ),
        ("/api/chat.postMessage", json!({ "ok": true })),
    ]
}

fn post(
    h: &Harness,
    connectors: &Connectors,
    operation: &str,
    input: Value,
) -> Result<crate::authority::commitment::CommitmentView, AuthorityError> {
    connectors.propose(
        &h.control,
        &h.agent,
        h.run,
        &ConnectorIntent {
            operation: operation.into(),
            input,
        },
    )
}

/// A Slack post is approved knowing where it goes: before it is proposed,
/// its destination is identified by Slack itself (the workspace and the
/// conversation: ids, name, kind), through governed reads of the run under
/// the post's own grant, to Slack's origin with its token; the owner sees
/// both the readable destination and its immutable ids; immediately before
/// it is sent the same reads run again under its commitment.
#[test]
fn a_slack_post_is_approved_and_sent_to_the_destination_slack_identified() {
    let h = harness();
    let api = Api::new(&slack_answers("general"));
    let connectors = chat(&h, &api);
    grant(&h, &connectors, "slack", &["slack.chat.post"]);
    let view = post(
        &h,
        &connectors,
        "slack.chat.post",
        json!({ "channel": "C0456", "text": "hello" }),
    )
    .unwrap();
    assert_eq!(view.class, EffectClass::R2);
    assert_eq!(
        view.target,
        "Slack public channel #general (C0456) in Acme (T0123)"
    );
    for line in [
        "Workspace: Acme (T0123)",
        "Conversation: public channel #general (C0456)",
    ] {
        assert!(
            view.summary.contains(&line.to_string()),
            "{:?}",
            view.summary
        );
    }
    // The reads that identified it: governed R1 commitments of the run,
    // done; to Slack, with its token; nothing posted.
    let views = h.control.authority().commitments().views_of_run(h.run);
    let mut reads: Vec<(&str, CommitmentState)> = views
        .iter()
        .filter(|v| v.id != view.id)
        .map(|v| (v.operation, v.state))
        .collect();
    reads.sort_by_key(|(operation, _)| *operation);
    assert_eq!(
        reads,
        [
            ("slack.auth.test", CommitmentState::Succeeded),
            ("slack.conversations.info", CommitmentState::Succeeded)
        ]
    );
    let token = Some("authorization: Bearer xoxb-fixture".to_string());
    assert_eq!(
        api.sent(),
        [
            (
                "GET".to_string(),
                "https://slack.com/api/auth.test".to_string(),
                token.clone()
            ),
            (
                "GET".to_string(),
                "https://slack.com/api/conversations.info?channel=C0456".to_string(),
                token.clone()
            ),
        ]
    );
    // The owner's approval shows it.
    let shown = Shown(Mutex::new(None));
    h.control
        .authorize(view.id, &h.agent, h.run, &shown)
        .unwrap();
    let asked = shown.0.lock().unwrap().clone().unwrap();
    assert_eq!(asked.target, view.target);
    assert!(asked
        .summary
        .contains(&"Conversation: public channel #general (C0456)".to_string()));
    h.control.execute(view.id, &h.agent, h.run).unwrap();
    let sent = api.sent();
    assert_eq!(sent.len(), 5);
    assert_eq!(
        sent[2..]
            .iter()
            .map(|(m, u, _)| format!("{m} {u}"))
            .collect::<Vec<_>>(),
        [
            "GET https://slack.com/api/auth.test",
            "GET https://slack.com/api/conversations.info?channel=C0456",
            "POST https://slack.com/api/chat.postMessage"
        ]
    );
    assert!(sent.iter().all(|(_, _, secret)| *secret == token));
}

/// Whatever about the destination changes between the approval and the
/// send (its name, its kind, its workspace or server, its id), the post
/// fails as `target_changed` and nothing is sent.
#[test]
fn a_destination_that_changed_after_approval_fails_the_post_and_sends_nothing() {
    let slack_changes: Vec<(&'static str, Value)> = vec![
        (
            "/api/conversations.info",
            json!({ "ok": true, "channel": {
                "id": "C0456", "name": "ceo-private", "is_channel": true, "is_private": false
            } }),
        ),
        (
            "/api/conversations.info",
            json!({ "ok": true, "channel": {
                "id": "C0456", "name": "general", "is_channel": true, "is_private": true
            } }),
        ),
        (
            "/api/conversations.info",
            json!({ "ok": true, "channel": {
                "id": "C0999", "name": "general", "is_channel": true, "is_private": false
            } }),
        ),
        (
            "/api/auth.test",
            json!({ "ok": true, "team": "Acme", "team_id": "T0999", "user_id": "U9" }),
        ),
        (
            "/api/conversations.info",
            json!({ "ok": true, "channel": { "id": "C0456", "is_im": true, "user": "U777" } }),
        ),
    ];
    for (path, changed) in slack_changes {
        let h = harness();
        let api = Api::new(&slack_answers("general"));
        let connectors = chat(&h, &api);
        grant(&h, &connectors, "slack", &["slack.chat.post"]);
        let view = post(
            &h,
            &connectors,
            "slack.chat.post",
            json!({ "channel": "C0456", "text": "hello" }),
        )
        .unwrap();
        h.control
            .authorize(view.id, &h.agent, h.run, &Yes::new(true))
            .unwrap();
        api.answer(path, changed.clone());
        assert!(h.control.execute(view.id, &h.agent, h.run).is_err());
        assert_eq!(api.posts(), 0, "{changed}");
        let finished = h
            .evidence
            .records()
            .into_iter()
            .rev()
            .find(|r| r.commitment.as_deref() == Some(&view.id.to_string()) && r.outcome.is_some())
            .unwrap();
        assert_eq!(finished.failure, Some("target_changed"), "{changed}");
    }
    let discord = |name: &str, kind: u64, guild: &str| json!({ "id": "1234", "type": kind, "name": name, "guild_id": guild });
    for changed in [
        discord("random", 0, "77"),
        discord("general", 5, "77"),
        discord("general", 0, "78"),
    ] {
        let h = harness();
        let api = Api::new(&[
            ("/api/v10/channels/1234/messages", json!({ "id": "1" })),
            ("/api/v10/channels/1234", discord("general", 0, "77")),
        ]);
        let connectors = chat(&h, &api);
        grant(&h, &connectors, "discord", &["discord.channel.post"]);
        let view = post(
            &h,
            &connectors,
            "discord.channel.post",
            json!({ "channel": "1234", "text": "hello" }),
        )
        .unwrap();
        assert_eq!(
            view.target,
            "Discord text channel #general (1234) in server 77"
        );
        h.control
            .authorize(view.id, &h.agent, h.run, &Yes::new(true))
            .unwrap();
        api.answer("/api/v10/channels/1234", changed.clone());
        assert!(h.control.execute(view.id, &h.agent, h.run).is_err());
        assert_eq!(api.posts(), 0, "{changed}");
    }
}

/// The destination is named by its own id, and what it is comes from the
/// API: a name in place of an id, an answer for another id, a kind that is
/// not a place to post, or an unidentified destination is refused before
/// anything is proposed; a direct message shows as one, with its peer.
#[test]
fn a_destination_is_its_id_and_what_the_api_says_it_is() {
    let refused = |answers: Vec<(&'static str, Value)>, operation: &str, input: Value| {
        let h = harness();
        let api = Api::new(&answers);
        let connectors = chat(&h, &api);
        grant(&h, &connectors, "slack", &["slack.chat.post"]);
        grant(&h, &connectors, "discord", &["discord.channel.post"]);
        let error = post(&h, &connectors, operation, input).unwrap_err();
        assert_eq!(api.posts(), 0);
        assert!(h
            .control
            .authority()
            .commitments()
            .views_of_run(h.run)
            .iter()
            .all(|v| v.class == EffectClass::R1));
        error
    };
    // A channel name where its id belongs: Slack does not identify it.
    let mut by_name = slack_answers("general");
    by_name[1] = (
        "/api/conversations.info",
        json!({ "ok": false, "error": "channel_not_found" }),
    );
    assert_eq!(
        refused(
            by_name,
            "slack.chat.post",
            json!({ "channel": "general", "text": "x" })
        ),
        AuthorityError::Closed("Slack did not identify the destination")
    );
    // An answer for another id.
    let mut other = slack_answers("general");
    other[1] = (
        "/api/conversations.info",
        json!({ "ok": true, "channel": { "id": "C0999", "name": "general", "is_channel": true } }),
    );
    assert_eq!(
        refused(
            other,
            "slack.chat.post",
            json!({ "channel": "C0456", "text": "x" })
        ),
        AuthorityError::Closed("the destination is named by its own id")
    );
    // A conversation Slack does not say the kind of.
    let mut unknown = slack_answers("general");
    unknown[1] = (
        "/api/conversations.info",
        json!({ "ok": true, "channel": { "id": "C0456", "name": "general" } }),
    );
    assert_eq!(
        refused(
            unknown,
            "slack.chat.post",
            json!({ "channel": "C0456", "text": "x" })
        ),
        AuthorityError::Closed("Slack did not say what kind of conversation it is")
    );
    // A Discord category is not a place to post.
    assert_eq!(
        refused(
            vec![(
                "/api/v10/channels/1234",
                json!({ "id": "1234", "type": 4, "name": "stuff", "guild_id": "77" })
            )],
            "discord.channel.post",
            json!({ "channel": "1234", "text": "x" })
        ),
        AuthorityError::Closed("Discord did not say it is a channel a message is posted to")
    );
    // A redirect is not followed and identifies nothing.
    let h = harness();
    let api = Api::new(&[]);
    api.answer_with("/api/v10/channels/1234", 302, String::new());
    let connectors = chat(&h, &api);
    grant(&h, &connectors, "discord", &["discord.channel.post"]);
    assert_eq!(
        post(
            &h,
            &connectors,
            "discord.channel.post",
            json!({ "channel": "1234", "text": "x" })
        )
        .unwrap_err(),
        AuthorityError::Unavailable("the destination could not be identified")
    );
    assert_eq!(api.sent().len(), 1, "the redirect was not followed");
    // Direct messages show as such, with their peer.
    let h = harness();
    let mut im = slack_answers("general");
    im[1] = (
        "/api/conversations.info",
        json!({ "ok": true, "channel": { "id": "D0123", "is_im": true, "user": "U777" } }),
    );
    let api = Api::new(&im);
    let connectors = chat(&h, &api);
    grant(&h, &connectors, "slack", &["slack.chat.post"]);
    let view = post(
        &h,
        &connectors,
        "slack.chat.post",
        json!({ "channel": "D0123", "text": "x" }),
    )
    .unwrap();
    assert_eq!(
        view.target,
        "Slack direct message with user U777 (D0123) in Acme (T0123)"
    );
    let h = harness();
    let api = Api::new(&[(
        "/api/v10/channels/555",
        json!({ "id": "555", "type": 1, "recipients": [{ "id": "42", "username": "alice" }] }),
    )]);
    let connectors = chat(&h, &api);
    grant(&h, &connectors, "discord", &["discord.channel.post"]);
    let view = post(
        &h,
        &connectors,
        "discord.channel.post",
        json!({ "channel": "555", "text": "x" }),
    )
    .unwrap();
    assert_eq!(
        view.target,
        "Discord direct message with alice (user 42) (555)"
    );
    // A post cannot be prepared without its destination identified.
    assert_eq!(
        connectors
            .prepare(
                h.control.authority(),
                &h.agent,
                h.run,
                &ConnectorIntent {
                    operation: "discord.channel.post".into(),
                    input: json!({ "channel": "555", "text": "x" }),
                },
                None
            )
            .err()
            .unwrap(),
        AuthorityError::Closed("a post's destination is identified before it is prepared")
    );
}
