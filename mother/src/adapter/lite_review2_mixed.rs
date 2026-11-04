//! Review 2: approval and retirement follow the pending effect, not File mode.
//! Real broker, Owner approval, loopback POST and kernel File publication.
use super::*;
use crate::effect::EffectRequest;
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, Verdict};
use crate::oracles::lite_support::file;
use nexus_governed_control::authority::commitment::CommitmentState;
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::egress::EgressIntent;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::Ordering;
use std::time::Instant;

struct MixedBuilder(String);
impl Lane for MixedBuilder {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Builder,
            name: "review2-mixed".into(),
            version: "1".into(),
        }
    }

    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        LaneOutput {
            text: "File A, Owner-approved POST, File B".into(),
            effects: vec![
                file("one.txt", &content_of("MKFILE", "one.txt").unwrap()),
                EffectRequest::Network(EgressIntent {
                    method: "POST".into(),
                    url: format!("{}/review2", self.0),
                    headers: vec![],
                    body: None,
                }),
                file("two.txt", &content_of("MKFILE", "two.txt").unwrap()),
            ],
            verdict: Verdict::Continue,
        }
    }
}

fn child(scenario: &str) -> bool {
    const ENV: &str = "NEXUS_LITE_REVIEW2_MIXED";
    if std::env::var(ENV).as_deref() == Ok(scenario) {
        return true;
    }
    let name = format!("adapter::lite::review2_mixed::lite_review2_{scenario}");
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(ENV, scenario)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{scenario}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("review2:{scenario}")));
    false
}

fn setup(retain_a_ack: bool) -> (Fx, FileSession<GovernedPort>, MissionId, TcpListener) {
    let f = Fx::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    f.home
        .control
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            Duration::from_secs(600),
            &f.owner,
        )
        .unwrap();
    let mut lanes = files_lanes();
    lanes.retain(|lane| lane.identity().role != LaneRole::Builder);
    lanes.push(Arc::new(MixedBuilder(origin)));
    let m = Coordinator::open(
        f.journal.clone(),
        f.port.clone(),
        Lanes::new(lanes).unwrap(),
        Arc::new(SystemClock::default()),
        f.home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let id = f.waiting(&m, "mixed effects");
    f.publish(id);
    if retain_a_ack {
        f.port.fail_file_ack_once.store(true, Ordering::SeqCst);
    }
    assert_eq!(m.advance(id).unwrap().to, MissionState::AwaitingApproval);
    assert_eq!(f.port.acks_tried(), 1);
    assert_eq!(
        std::fs::read_to_string(f.ws.join("one.txt")).unwrap(),
        content_of("MKFILE", "one.txt").unwrap()
    );
    assert!(!f.ws.join("two.txt").exists());
    let entries = m.journal().unwrap();
    assert!(entries.iter().any(|e| matches!(
        e.event,
        Event::Transition {
            trigger: Trigger::ActionApproved { effect: 0, .. }
        }
    )));
    assert!(entries
        .iter()
        .any(|e| matches!(e.event, Event::ApprovalClosed { effect: 0 })));
    let p = f.port.pending(id).unwrap();
    let view = f
        .home
        .control
        .authority()
        .commitments()
        .view(CommitmentId::parse(&p.commitment).unwrap())
        .unwrap();
    assert_eq!(view.kind, CapabilityKind::Egress);
    assert_eq!(view.class, EffectClass::R2);
    assert!(view.requires_approval);
    assert_eq!(view.state, CommitmentState::Prepared);
    no_post(&listener);
    (f, m, id, listener)
}

fn no_post(listener: &TcpListener) {
    assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
}

fn execute_post(f: &Fx, id: MissionId, listener: &TcpListener) {
    let p = f.port.pending(id).unwrap();
    let commitment = CommitmentId::parse(&p.commitment).unwrap();
    let run = RunId::parse(&p.run).unwrap();
    f.home
        .control
        .authorize(commitment, &mother_agent(), run, &f.owner)
        .unwrap();
    let shown = f.owner.0.lock().unwrap().last().unwrap().clone();
    assert_eq!(shown.commitment, p.commitment);
    assert_eq!(shown.kind, CapabilityKind::Egress);
    assert_eq!(shown.class, EffectClass::R2);
    let control = f.home.control.clone();
    let executed = std::thread::spawn(move || control.execute(commitment, &mother_agent(), run));
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::yield_now();
            }
            result => panic!("POST must reach its granted listener: {result:?}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut request = Vec::new();
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        let mut buf = [0; 1024];
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0 && request.len() + n <= 8192);
        request.extend_from_slice(&buf[..n]);
    }
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
        .unwrap();
    assert!(executed.join().unwrap().is_ok());
    assert_eq!(
        String::from_utf8(request).unwrap().lines().next(),
        Some("POST /review2 HTTP/1.1")
    );
    assert_eq!(
        f.home
            .control
            .authority()
            .commitments()
            .view(commitment)
            .unwrap()
            .state,
        CommitmentState::Succeeded
    );
    no_post(listener);
}

fn approve_post(f: &Fx, m: &FileSession<GovernedPort>, id: MissionId, listener: &TcpListener) {
    let waiting = m.advance(id);
    no_post(listener);
    execute_post(f, id, listener);
    assert_eq!(
        m.advance(id).map(|a| a.to),
        Ok(MissionState::AwaitingApproval),
        "executed POST must be recognized and File B must reach its approval wait"
    );
    assert_eq!(
        waiting.map(|a| a.to),
        Ok(MissionState::AwaitingApproval),
        "unapproved POST remains pollable"
    );
    let entries = m.journal().unwrap();
    assert!(entries
        .iter()
        .any(|e| matches!(e.event, Event::ApprovalClosed { effect: 1 })));
    assert!(entries.iter().any(|e| matches!(
        e.event,
        Event::Transition {
            trigger: Trigger::ActionApproved { effect: 1, .. }
        }
    )));
    assert_eq!(
        f.port.acks_tried(),
        1,
        "POST must not enter File acknowledgements"
    );
}

#[test]
fn lite_review2_continue() {
    if !child("continue") {
        return;
    }
    let (f, m, id, listener) = setup(false);
    approve_post(&f, &m, id, &listener);
    f.publish(id);
    m.advance(id).unwrap();
    for _ in 0..8 {
        if m.state(id).unwrap().unwrap().is_terminal() {
            break;
        }
        m.advance(id).unwrap();
    }
    assert_eq!(m.state(id).unwrap(), Some(MissionState::Done));
    assert_eq!(
        std::fs::read_to_string(f.ws.join("two.txt")).unwrap(),
        content_of("MKFILE", "two.txt").unwrap()
    );
    let entries = m.journal().unwrap();
    let approved: Vec<_> = entries
        .iter()
        .filter_map(|e| match e.event {
            Event::Transition {
                trigger: Trigger::ActionApproved { effect, .. },
            } => Some(effect),
            _ => None,
        })
        .collect();
    assert_eq!(approved, vec![0, 1, 2]);
    assert_eq!(f.port.acks_tried(), 2);
    no_post(&listener);
    m.close().unwrap();
    println!("review2:continue");
}

#[test]
fn lite_review2_stop_post() {
    if !child("stop_post") {
        return;
    }
    let (f, m, id, listener) = setup(true);
    let p = f.port.pending(id).unwrap();
    assert_eq!(m.stop(id), Ok(MissionState::Stopped));
    assert!(f
        .home
        .control
        .authorize(
            CommitmentId::parse(&p.commitment).unwrap(),
            &mother_agent(),
            RunId::parse(&p.run).unwrap(),
            &f.owner
        )
        .is_err());
    assert!(!f.ws.join("two.txt").exists());
    no_post(&listener);
    let entries = m.journal().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| matches!(e.event, Event::ApprovalClosed { effect: 1 })),
        "Stop must retire the non-File pending action"
    );
    assert!(
        !entries.iter().any(|e| matches!(
            e.event,
            Event::SettlementStarted { .. }
                | Event::Transition {
                    trigger: Trigger::Settled { .. }
                }
                | Event::EffectDispatched { effect: 2, .. }
        )),
        "POST cannot borrow A's File settlement or dispatch B"
    );
    assert_eq!(
        f.port.acks_tried(),
        2,
        "Stop must discharge A's retained acknowledgement"
    );
    assert_eq!(m.stop(id), Ok(MissionState::Stopped));
    assert_eq!(f.port.acks_tried(), 2);
    m.close().unwrap();
    println!("review2:stop_post");
}

#[test]
fn lite_review2_deny_post() {
    if !child("deny_post") {
        return;
    }
    let (f, m, id, listener) = setup(false);
    let p = f.port.pending(id).unwrap();
    f.home
        .control
        .deny(
            CommitmentId::parse(&p.commitment).unwrap(),
            &mother_agent(),
            RunId::parse(&p.run).unwrap(),
        )
        .unwrap();
    assert_eq!(
        m.advance(id).map(|a| a.to),
        Ok(MissionState::Denied),
        "non-File denial keeps the legacy disposition"
    );
    assert!(!f.ws.join("two.txt").exists());
    no_post(&listener);
    assert!(m
        .journal()
        .unwrap()
        .iter()
        .any(|e| matches!(e.event, Event::ApprovalClosed { effect: 1 })));
    assert_eq!(f.port.acks_tried(), 1);
    m.close().unwrap();
    println!("review2:deny_post");
}

#[test]
fn lite_review2_stop_file_b() {
    if !child("stop_file_b") {
        return;
    }
    let (f, m, id, listener) = setup(false);
    approve_post(&f, &m, id, &listener);
    assert_eq!(m.stop(id), Ok(MissionState::Stopped));
    let entries = m.journal().unwrap();
    let settled: Vec<_> = entries
        .iter()
        .filter_map(|e| match &e.event {
            Event::Transition {
                trigger:
                    Trigger::Settled {
                        effect, outcome, ..
                    },
            } => Some((*effect, outcome.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(settled, vec![(2, SettlementOutcome::NotPublished)]);
    assert_eq!(f.port.acks_tried(), 2);
    assert!(!f.ws.join("two.txt").exists());
    no_post(&listener);
    m.close().unwrap();
    println!("review2:stop_file_b");
}
