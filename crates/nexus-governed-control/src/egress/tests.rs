//! Egress, end to end against a loopback server (granted explicitly as a
//! private destination, as only the owner can).

use super::destination::{Resolver, SystemResolver};
use super::transport::{HttpResponse, HttpTransport, PinnedRequest, Transport, TransportError};
use super::{Egress, EgressIntent, EgressLimits};
use crate::authority::commitment::CommitmentState;
use crate::authority::effect::EffectClass;
use crate::authority::run::CancelToken;
use crate::authority::AuthorityError;
use crate::control::EffectOutput;
use crate::harness_tests::{harness, Harness, Reply, TestServer, Yes};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn grant(h: &Harness, origin: &str, methods: &[&str], allow_private: bool) {
    let methods: Vec<String> = methods.iter().map(|m| m.to_string()).collect();
    let scope = Egress::grant_scope(origin, &methods, allow_private).unwrap();
    h.control
        .authority()
        .grants()
        .request(scope, Duration::from_secs(600), &Yes::new(true))
        .unwrap();
}

fn intent(method: &str, url: &str) -> EgressIntent {
    EgressIntent {
        method: method.into(),
        url: url.into(),
        headers: vec![],
        body: None,
    }
}

fn run(
    h: &Harness,
    egress: &Egress,
    intent: EgressIntent,
    approve: bool,
) -> Result<EffectOutput, AuthorityError> {
    let preparation = egress.prepare(h.control.authority(), &intent)?;
    let view = h.control.propose(&h.agent, h.run, preparation)?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(approve))?;
    h.control.execute(view.id, &h.agent, h.run)
}

fn meta<'a>(out: &'a EffectOutput, key: &str) -> Option<&'a str> {
    out.meta
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Resolves every name to fixed addresses (the first answer, then the
/// second for every later lookup).
struct Answers(Vec<&'static str>, Vec<&'static str>, AtomicU32);

impl Resolver for Answers {
    fn resolve(&self, _host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        let answer = if self.2.fetch_add(1, Ordering::SeqCst) == 0 {
            &self.0
        } else {
            &self.1
        };
        Ok(answer
            .iter()
            .map(|ip| SocketAddr::new(ip.parse().unwrap(), port))
            .collect())
    }
}

/// Counts exchanges and then performs them for real.
struct Counting(AtomicU32);

impl Transport for Counting {
    fn exchange(
        &self,
        request: PinnedRequest,
        cancel: &CancelToken,
    ) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        HttpTransport.exchange(request, cancel)
    }
}

fn egress() -> Egress {
    Egress::new(
        Arc::new(SystemResolver),
        Arc::new(HttpTransport),
        EgressLimits::default(),
    )
}

#[test]
fn a_granted_request_reaches_the_destination_with_nothing_ambient() {
    let server = TestServer::start(|_| Reply::ok("hello"));
    let h = harness();
    grant(&h, &server.origin(), &["GET"], true);
    let out = run(
        &h,
        &egress(),
        intent("GET", &format!("{}/a?b=1", server.origin())),
        false,
    )
    .unwrap();
    assert_eq!(out.text.as_deref(), Some("hello"));
    assert_eq!(meta(&out, "status"), Some("200"));
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "GET");
    assert_eq!(received[0].path, "/a?b=1");
    assert_eq!(
        received[0].header("user-agent"),
        Some(super::transport::USER_AGENT)
    );
    for ambient in ["authorization", "cookie", "proxy-authorization", "referer"] {
        assert!(received[0].header(ambient).is_none(), "{ambient}");
    }
    // Evidence: prepared, authorized, started, finished; no body in it.
    let records = h.evidence.records();
    let finished = records.iter().rev().find(|r| r.outcome.is_some()).unwrap();
    assert_eq!(finished.outcome, Some("succeeded"));
    assert!(!format!("{:?}", records).contains("hello"));
}

#[test]
fn without_a_grant_nothing_is_sent() {
    let server = TestServer::start(|_| Reply::ok("hello"));
    let h = harness();
    // A grant for another port, another method, another scheme.
    grant(&h, "http://127.0.0.1:9", &["GET"], true);
    grant(&h, &server.origin(), &["HEAD"], true);
    assert_eq!(
        egress()
            .prepare(
                h.control.authority(),
                &intent("GET", &format!("{}/", server.origin()))
            )
            .err(),
        Some(AuthorityError::NoCoveringGrant)
    );
    assert!(server.received().is_empty());
}

#[test]
fn private_destinations_need_a_grant_that_names_them() {
    let server = TestServer::start(|_| Reply::ok("hello"));
    let h = harness();
    grant(&h, &server.origin(), &["GET"], false);
    assert_eq!(
        egress()
            .prepare(
                h.control.authority(),
                &intent("GET", &format!("{}/", server.origin()))
            )
            .err(),
        Some(AuthorityError::Closed("destination address not permitted"))
    );
    assert!(server.received().is_empty());
}

#[test]
fn the_connection_goes_only_to_the_checked_addresses() {
    // `pinned.invalid` exists in no DNS: the request still reaches the
    // server, so the transport used the checked address and resolved
    // nothing itself.
    let server = TestServer::start(|_| Reply::ok("pinned"));
    let h = harness();
    let port = server.address.port();
    grant(&h, &format!("http://pinned.invalid:{port}"), &["GET"], true);
    let resolver = Arc::new(Answers(
        vec!["127.0.0.1"],
        vec!["127.0.0.1"],
        AtomicU32::new(0),
    ));
    let egress = Egress::new(resolver, Arc::new(HttpTransport), EgressLimits::default());
    let out = run(
        &h,
        &egress,
        intent("GET", &format!("http://pinned.invalid:{port}/x")),
        false,
    )
    .unwrap();
    assert_eq!(out.text.as_deref(), Some("pinned"));
    assert_eq!(
        server.received()[0].header("host"),
        Some(format!("pinned.invalid:{port}").as_str())
    );
}

#[test]
fn rebinding_between_preparation_and_execution_fails_before_any_connection() {
    let h = harness();
    grant(&h, "http://rebind.invalid:8080", &["GET"], false);
    let transport = Arc::new(Counting(AtomicU32::new(0)));
    let egress = Egress::new(
        Arc::new(Answers(
            vec!["93.184.216.34"],
            vec!["127.0.0.1"],
            AtomicU32::new(0),
        )),
        transport.clone(),
        EgressLimits::default(),
    );
    let preparation = egress
        .prepare(
            h.control.authority(),
            &intent("GET", "http://rebind.invalid:8080/"),
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(false))
        .unwrap();
    assert_eq!(
        h.control.execute(view.id, &h.agent, h.run).unwrap_err(),
        AuthorityError::TargetChanged
    );
    assert_eq!(transport.0.load(Ordering::SeqCst), 0);
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Failed
    );
}

#[test]
fn same_origin_redirects_are_followed_and_checked_others_are_returned() {
    let server = TestServer::start(|request| match request.path.as_str() {
        "/start" => Reply::redirect(302, "/next"),
        "/next" => Reply::ok("arrived"),
        "/away" => Reply::redirect(302, "https://elsewhere.example/landing?code=secret"),
        _ => Reply::ok("other"),
    });
    let h = harness();
    grant(&h, &server.origin(), &["GET"], true);
    let out = run(
        &h,
        &egress(),
        intent("GET", &format!("{}/start", server.origin())),
        false,
    )
    .unwrap();
    assert_eq!(out.text.as_deref(), Some("arrived"));
    assert_eq!(meta(&out, "redirects"), Some("1"));
    // Neither a path nor a query reaches the (permanent) record.
    assert!(meta(&out, "path_digest").is_none());
    assert!(meta(&out, "path").is_none());
    let out = run(
        &h,
        &egress(),
        intent("GET", &format!("{}/away", server.origin())),
        false,
    )
    .unwrap();
    assert_eq!(meta(&out, "status"), Some("302"));
    assert_eq!(
        meta(&out, "redirect_not_followed"),
        Some("https://elsewhere.example:443"),
        "neither the path nor its query is kept in evidence"
    );
    assert!(out
        .meta
        .iter()
        .all(|(_, value)| !value.contains("landing") && !value.contains("secret")));
    assert_eq!(server.received().len(), 3);
}

#[test]
fn changing_requests_are_r2_carry_their_body_and_never_follow_redirects() {
    let server = TestServer::start(|request| match request.path.as_str() {
        "/submit" => Reply::redirect(307, "/elsewhere"),
        _ => Reply::ok("unexpected"),
    });
    let h = harness();
    grant(&h, &server.origin(), &["POST"], true);
    let mut post = intent("POST", &format!("{}/submit", server.origin()));
    post.body = Some("{\"a\":1}".into());
    post.headers = vec![("Content-Type".into(), "application/json".into())];
    let preparation = egress().prepare(h.control.authority(), &post).unwrap();
    assert_eq!(preparation.action.class, EffectClass::R2);
    // Declined natively: nothing is sent.
    assert_eq!(
        run(&h, &egress(), post.clone(), false).unwrap_err(),
        AuthorityError::Declined
    );
    assert!(server.received().is_empty());
    // Approved natively, exactly once, for exactly this request.
    let preparation = egress().prepare(h.control.authority(), &post).unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    let owner = Yes::new(true);
    h.control
        .authorize(view.id, &h.agent, h.run, &owner)
        .unwrap();
    assert_eq!(owner.asked(), 1);
    let out = h.control.execute(view.id, &h.agent, h.run).unwrap();
    assert_eq!(meta(&out, "status"), Some("307"));
    let received = server.received();
    assert_eq!(received.len(), 1, "the redirect was not followed");
    assert_eq!(received[0].body, b"{\"a\":1}");
    assert_eq!(received[0].header("content-type"), Some("application/json"));
}

#[test]
fn only_allow_listed_plain_headers_may_be_sent() {
    let h = harness();
    grant(&h, "http://127.0.0.1:9", &["GET", "POST"], true);
    for (name, value) in [
        ("Authorization", "Bearer x"),
        ("Cookie", "a=b"),
        ("Proxy-Authorization", "Basic x"),
        ("Host", "evil.example"),
        ("X-Api-Key", "k"),
        ("Accept", "text/html\r\nCookie: a=b"),
        ("Accept", "caf\u{e9}"),
    ] {
        let mut request = intent("GET", "http://127.0.0.1:9/");
        request.headers = vec![(name.into(), value.into())];
        assert!(
            matches!(
                egress().prepare(h.control.authority(), &request).err(),
                Some(AuthorityError::InvalidAction(_))
            ),
            "{name}: {value:?}"
        );
    }
    let mut request = intent("GET", "http://127.0.0.1:9/");
    request.body = Some("x".into());
    assert_eq!(
        egress().prepare(h.control.authority(), &request).err(),
        Some(AuthorityError::InvalidAction("a safe request has no body"))
    );
    let mut request = intent("POST", "http://127.0.0.1:9/");
    request.body = Some("x".repeat(EgressLimits::default().max_request_body + 1));
    assert!(egress().prepare(h.control.authority(), &request).is_err());
}

#[test]
fn responses_are_bounded() {
    let server = TestServer::start(|_| Reply::ok(&"z".repeat(4096)));
    let h = harness();
    grant(&h, &server.origin(), &["GET"], true);
    let limits = EgressLimits {
        max_response: 1024,
        ..EgressLimits::default()
    };
    let egress = Egress::new(Arc::new(SystemResolver), Arc::new(HttpTransport), limits);
    assert!(run(
        &h,
        &egress,
        intent("GET", &format!("{}/", server.origin())),
        false
    )
    .is_err());
    let failed = h
        .evidence
        .records()
        .into_iter()
        .rev()
        .find(|r| r.outcome.is_some())
        .unwrap();
    assert_eq!(failed.outcome, Some("failed"));
    assert_eq!(failed.failure, Some("bounds"));
}

#[test]
fn cancelling_the_run_drops_an_exchange_in_flight() {
    let server = TestServer::start(|_| Reply {
        delay: Duration::from_secs(3),
        ..Reply::ok("late")
    });
    let h = harness();
    grant(&h, &server.origin(), &["GET"], true);
    let preparation = egress()
        .prepare(
            h.control.authority(),
            &intent("GET", &format!("{}/", server.origin())),
        )
        .unwrap();
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(false))
        .unwrap();
    let control = h.control.clone();
    let (agent, run) = (h.agent.clone(), h.run);
    let started = std::time::Instant::now();
    let worker = std::thread::spawn(move || control.execute(view.id, &agent, run));
    std::thread::sleep(Duration::from_millis(200));
    h.control.cancel_run(h.run).unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        AuthorityError::RunCancelled
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "it did not wait for the reply"
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
}

#[test]
fn grants_name_exactly_one_origin() {
    assert!(Egress::grant_scope("https://example.com/path", &["GET".into()], false).is_err());
    assert!(Egress::grant_scope("https://example.com/?q=1", &["GET".into()], false).is_err());
    assert!(Egress::grant_scope("https://example.com", &[], false).is_err());
    assert!(Egress::grant_scope("https://example.com", &["get".into()], false).is_err());
    assert!(Egress::grant_scope("file:///etc", &["GET".into()], false).is_err());
    let scope =
        Egress::grant_scope("https://Example.com", &["GET".into(), "GET".into()], false).unwrap();
    assert_eq!(
        scope.describe(),
        vec![
            "Network requests to https://example.com:443".to_string(),
            "Methods: GET".to_string(),
            "Private, local or loopback addresses: refused".to_string(),
        ]
    );
}

/// A wrapped value rebuilt from its first line and `↳ ` continuations.
fn unwrapped(lines: &[String], first: usize) -> String {
    let mut value = lines[first].clone();
    for line in &lines[first + 1..] {
        match line.strip_prefix("↳ ") {
            Some(more) => value.push_str(more),
            None => break,
        }
    }
    value
}

/// Quoted content rebuilt from the lines after its `label:` line.
fn unquoted(lines: &[String], label: &str) -> String {
    let at = lines
        .iter()
        .position(|l| l == &format!("{label}:"))
        .unwrap();
    let mut out: Vec<String> = Vec::new();
    for line in &lines[at + 1..] {
        if let Some(more) = line.strip_prefix("│↳ ") {
            out.last_mut().unwrap().push_str(more);
        } else if let Some(text) = line.strip_prefix("│ ") {
            out.push(text.to_string());
        } else {
            break;
        }
    }
    out.join("\n")
}

/// The owner approves the whole request: a long URL on as many lines as it
/// needs, and every line of the body, marked so that none can pass for the
/// dialog's own text. Nothing is shortened.
#[test]
fn the_owner_sees_the_whole_request() {
    use crate::authority::evidence::is_plain;
    let server = TestServer::start(|_| Reply::ok("ok"));
    let h = harness();
    grant(&h, &server.origin(), &["POST"], true);
    let url = format!(
        "{}/transfer?note={}&to=attacker",
        server.origin(),
        "q".repeat(600)
    );
    let body = format!(
        "{{\"amount\": 5}}\nTarget: https://safe.example\n{}",
        "b".repeat(400)
    );
    let mut post = intent("POST", &url);
    post.body = Some(body.clone());
    // A header's value changes how the body is read: it is shown too.
    post.headers = vec![(
        "content-type".into(),
        "application/x-www-form-urlencoded".into(),
    )];
    let preparation = egress().prepare(h.control.authority(), &post).unwrap();
    let summary = &preparation.action.summary;
    assert!(summary.iter().all(|l| is_plain(l)), "{summary:?}");
    assert_eq!(unwrapped(summary, 0), format!("POST {url}"));
    assert!(
        summary.contains(&"Header content-type: application/x-www-form-urlencoded".to_string()),
        "{summary:?}"
    );
    assert_eq!(unquoted(summary, "Body"), body);
    assert!(summary.contains(&"│ Target: https://safe.example".to_string()));
}

/// Every address of an answer is classified before any is set aside: a
/// refused address past the sixteenth still refuses the answer.
#[test]
fn a_refused_address_anywhere_in_an_answer_refuses_it() {
    use super::destination::{resolve_checked, Destination, DestinationError};
    struct Many;
    impl Resolver for Many {
        fn resolve(&self, _: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
            let mut answer: Vec<SocketAddr> = (1..=16)
                .map(|i| SocketAddr::from(([93, 184, 216, i], port)))
                .collect();
            answer.push(SocketAddr::from(([192, 168, 1, 1], port)));
            Ok(answer)
        }
    }
    let destination = Destination::parse("https://many.example/").unwrap();
    assert_eq!(
        resolve_checked(&destination, false, &Many).unwrap_err(),
        DestinationError::NotPermitted
    );
}

/// A fixed answer and a fixed view of this machine's network (`None`: the
/// view cannot be read). Answers change from the second lookup on when
/// `later` is given.
struct Local {
    answer: Vec<std::net::IpAddr>,
    later: Option<Vec<std::net::IpAddr>>,
    local: Option<super::destination::LocalNetworks>,
    lookups: AtomicU32,
}

impl Local {
    fn new(answer: &[&str], local: Option<&[(&str, u8)]>) -> Self {
        Self {
            answer: answer.iter().map(|ip| ip.parse().unwrap()).collect(),
            later: None,
            local: local.map(|networks| {
                super::destination::LocalNetworks(
                    networks
                        .iter()
                        .map(|(ip, length)| (ip.parse().unwrap(), *length))
                        .collect(),
                )
            }),
            lookups: AtomicU32::new(0),
        }
    }
}

impl Resolver for Local {
    fn resolve(&self, _host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        let first = self.lookups.fetch_add(1, Ordering::SeqCst) == 0;
        let answer = match (&self.later, first) {
            (Some(later), false) => later,
            _ => &self.answer,
        };
        Ok(answer.iter().map(|ip| SocketAddr::new(*ip, port)).collect())
    }

    fn local_networks(&self) -> std::io::Result<super::destination::LocalNetworks> {
        self.local
            .clone()
            .ok_or_else(|| std::io::Error::other("no view of the local network"))
    }
}

/// Not being in a private range is not enough: an address of this machine,
/// or in a network directly connected to it, is refused where private
/// addresses are not granted, IPv4 and IPv6 alike, as is any answer that
/// holds one; an ordinary remote address passes; a boundary that cannot be
/// read refuses everything; a grant of private addresses admits them, as
/// it says.
#[test]
fn this_machines_addresses_and_link_networks_are_refused_however_public() {
    use super::destination::{resolve_checked, Destination, DestinationError};
    let host = Destination::parse("https://rebound.example/").unwrap();
    let literal = Destination::parse("https://93.184.216.34/").unwrap();
    // This machine: a public IPv4 address on a /24, a global IPv6 address
    // on a /64, a point-to-point peer.
    let machine: &[(&str, u8)] = &[
        ("93.184.216.34", 24),
        ("2606:2800:220:1::5", 64),
        ("198.18.0.1", 32),
        ("45.33.32.156", 32),
    ];
    let refused = Err(DestinationError::NotPermitted);
    for (answer, expected) in [
        // Its own public IPv4 address, by name and as a literal below.
        (&["93.184.216.34"][..], refused),
        // Another host of its directly connected public subnet.
        (&["93.184.216.99"][..], refused),
        // Its own global IPv6 address, and another of its /64.
        (&["2606:2800:220:1::5"][..], refused),
        (&["2606:2800:220:1:abcd::1"][..], refused),
        // The point-to-point peer.
        (&["45.33.32.156"][..], refused),
        // A mapped IPv6 form of its subnet.
        (&["::ffff:93.184.216.99"][..], refused),
        // A mixed answer, one address local: the whole answer.
        (&["8.8.8.8", "93.184.216.99"][..], refused),
        (&["2001:4860:4860::8888", "2606:2800:220:1::9"][..], refused),
    ] {
        let resolver = Local::new(answer, Some(machine));
        assert_eq!(
            resolve_checked(&host, false, &resolver).map(|_| ()),
            expected,
            "{answer:?}"
        );
    }
    assert_eq!(
        resolve_checked(&literal, false, &Local::new(&[], Some(machine))).map(|_| ()),
        refused,
        "an IP literal"
    );
    // Ordinary remote addresses, just outside the local networks.
    for remote in [
        &["8.8.8.8"][..],
        &["93.184.217.1"][..],
        &["2606:2800:220:2::1"][..],
        &["2001:4860:4860::8888", "1.1.1.1"][..],
    ] {
        let resolver = Local::new(remote, Some(machine));
        assert!(
            resolve_checked(&host, false, &resolver).is_ok(),
            "{remote:?}"
        );
    }
    // No view of the boundary: nothing passes where it is needed.
    assert_eq!(
        resolve_checked(&host, false, &Local::new(&["8.8.8.8"], None)).unwrap_err(),
        DestinationError::NoLocalBoundary
    );
    // A network of length zero covers every address.
    assert_eq!(
        resolve_checked(
            &host,
            false,
            &Local::new(&["8.8.8.8"], Some(&[("10.0.0.1", 0)]))
        )
        .unwrap_err(),
        DestinationError::NotPermitted
    );
    // A grant of private addresses admits this machine's, explicitly, and
    // needs no view of the boundary.
    for answer in [&["93.184.216.99"][..], &["2606:2800:220:1::5"][..]] {
        assert!(resolve_checked(&host, true, &Local::new(answer, None)).is_ok());
    }
}

/// The system's view of this machine's network is read for real: it holds
/// the loopback interface's address and network.
#[cfg(target_os = "linux")]
#[test]
fn the_system_view_of_the_local_network_is_read() {
    let view = SystemResolver.local_networks().unwrap();
    assert!(view.contains("127.0.0.1".parse().unwrap()));
    assert!(view.contains("127.1.2.3".parse().unwrap()));
    assert!(!view.contains("8.8.8.8".parse().unwrap()) || view.0.iter().any(|(_, l)| *l == 0));
}

/// Every resolution is checked against the boundary: a name granted as a
/// public origin that resolves, when the request is about to be sent, to
/// this machine's directly connected network is refused before any
/// socket is opened.
#[test]
fn a_name_rebound_to_this_machines_network_is_refused_before_any_socket() {
    let mut resolver = Local::new(&["8.8.8.8"], Some(&[("93.184.216.34", 24)]));
    resolver.later = Some(vec!["93.184.216.99".parse().unwrap()]);
    let transport = Arc::new(Counting(AtomicU32::new(0)));
    let egress = Egress::new(
        Arc::new(resolver),
        transport.clone(),
        EgressLimits::default(),
    );
    let h = harness();
    grant(&h, "https://rebound.example", &["GET"], false);
    let error = run(
        &h,
        &egress,
        intent("GET", "https://rebound.example/"),
        false,
    )
    .unwrap_err();
    assert_eq!(error, AuthorityError::TargetChanged);
    assert_eq!(transport.0.load(Ordering::SeqCst), 0, "nothing was sent");
}
