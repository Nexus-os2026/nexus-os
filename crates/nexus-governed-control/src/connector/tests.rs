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
            },
            ConnectorOperation {
                id: "fixture.notes.create",
                class: EffectClass::R2,
                method: Method::Post,
                credential: Some(SPEC),
                build: notes_create,
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
    let preparation = connectors.prepare(h.control.authority(), &h.agent, h.run, &intent)?;
    let view = h.control.propose(&h.agent, h.run, preparation)?;
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
        .prepare(h.control.authority(), &h.agent, h.run, &intent)
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
            },
            ConnectorOperation {
                id: "fixture.port",
                class: EffectClass::R1,
                method: Method::Get,
                credential: None,
                build: escape_to_another_port,
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
    assert_eq!(ids.len(), 13);
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
