//! Governed egress never inherits a proxy from the environment.
//!
//! This file holds exactly one test, so setting process-wide proxy
//! variables cannot race with another test in the same process.

use nexus_governed_control::authority::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::authority::ids::AgentId;
use nexus_governed_control::authority::run::RunOrigin;
use nexus_governed_control::authority::Authority;
use nexus_governed_control::control::Control;
use nexus_governed_control::egress::{Egress, EgressIntent};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

struct Owner;
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
}

#[test]
fn proxy_variables_in_the_environment_are_ignored() {
    // A "proxy" that would capture the request if it were used.
    let trap = TcpListener::bind("127.0.0.1:0").unwrap();
    let trap_url = format!("http://{}", trap.local_addr().unwrap());
    trap.set_nonblocking(true).unwrap();
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        std::env::set_var(name, &trap_url);
    }
    std::env::remove_var("NO_PROXY");
    std::env::remove_var("no_proxy");

    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.local_addr().unwrap());
    let served = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = [0u8; 4096];
        let n = stream.read(&mut request).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ndirect")
            .unwrap();
        String::from_utf8_lossy(&request[..n]).into_owned()
    });

    let control = Control::new(Authority::new(
        Arc::new(MemoryEvidence::new(64)),
        Arc::new(SystemClock::default()),
    ));
    let agent = AgentId::new("agent-proxy-test").unwrap();
    let run = control
        .authority()
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let scope = Egress::grant_scope(&origin, &["GET".into()], true).unwrap();
    control
        .authority()
        .grants()
        .request(scope, Duration::from_secs(60), &Owner)
        .unwrap();
    let egress = Egress::system();
    let preparation = egress
        .prepare(
            control.authority(),
            &EgressIntent {
                method: "GET".into(),
                url: format!("{origin}/through"),
                headers: vec![],
                body: None,
            },
        )
        .unwrap();
    let view = control.propose(&agent, run, preparation).unwrap();
    control.authorize(view.id, &agent, run, &Owner).unwrap();
    let out = control.execute(view.id, &agent, run).unwrap();
    assert_eq!(out.text.as_deref(), Some("direct"));
    let request = served.join().unwrap();
    assert!(request.starts_with("GET /through HTTP/1.1"), "{request}");
    assert!(
        trap.accept().is_err(),
        "nothing ever connected to the environment's proxy"
    );
}
