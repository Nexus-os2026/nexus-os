//! A3: independently selected work records, with before/after append ordering.
//! All durable prefixes are replayed, including the interrupted prefix.
use super::*;
use crate::oracles::lite_support::files_lanes;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::authority::file_session::HaltCell;
use std::cell::RefCell;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

const CASE: &str = "M2DD_SESSION_APPEND_CASE";
const TEST: &str =
    "coordinator::session_journal_oracles::session_r08_append_admission_and_every_prefix";
const SINKS: [&str; 18] = [
    "draft",
    "file_select",
    "revise",
    "submit",
    "consent_install",
    "start",
    "phase_done",
    "verify_pass",
    "approval_recognize",
    "dispatch_reserve",
    "dispatch_reconcile",
    "pending_install",
    "awaiting_approval",
    "lane_reserve",
    "lane_invoke",
    "consent_reserve",
    "phase_reserve",
    "paused_store",
];

/// Literal mapping of observed records. It is not a production transition
/// classifier and supplies no expected state or publication result.
fn work_key(event: &Event) -> Option<&'static str> {
    match event {
        Event::Drafted { .. } => Some("draft"),
        Event::FileModeSelected => Some("file_select"),
        Event::EffectDispatched { .. } => Some("dispatch_reserve"),
        Event::EffectSettled { .. } => Some("dispatch_reconcile"),
        Event::ApprovalPending { .. } => Some("pending_install"),
        Event::LaneInvoked { .. } => Some("lane_reserve"),
        Event::Transition { trigger } => match trigger {
            Trigger::Revise { .. } => Some("revise"),
            Trigger::Submit { .. } => Some("submit"),
            Trigger::Consent { .. } => Some("consent_install"),
            Trigger::Start => Some("start"),
            Trigger::PhaseDone => Some("phase_done"),
            Trigger::VerifyPass => Some("verify_pass"),
            Trigger::AwaitApproval { .. } => Some("awaiting_approval"),
            Trigger::ActionApproved { .. } => Some("approval_recognize"),
            _ => None,
        },
        _ => None,
    }
}

struct Probe {
    sink: String,
    when: String,
    count: u32,
    cell: HaltCell,
    boundary: Option<u64>,
    last_seq: u64,
    calls: usize,
}
thread_local! { static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) }; }
pub(super) fn at(when: &str, entry: &Entry) {
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let Some(p) = probe.as_mut() else {
            return;
        };
        if when == "after" {
            p.last_seq = entry.seq;
        }
        if p.boundary.is_none() && p.when == when && work_key(&entry.event) == Some(p.sink.as_str())
        {
            p.boundary = Some(entry.seq);
            for _ in 0..p.count {
                p.cell.engage_emergency();
            }
        }
    });
}

pub(super) fn call(when: &str, sink: &str) {
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let Some(p) = probe.as_mut() else { return };
        if p.sink != sink {
            return;
        }
        if when == "entered" {
            p.calls += 1;
        }
        if p.boundary.is_none() && p.when == when {
            // A non-journal sink has no entry of its own. Every later work
            // record starts at this next sequence, even in an after case.
            p.boundary = Some(p.last_seq + 1);
            for _ in 0..p.count {
                p.cell.engage_emergency();
            }
        }
    });
}

fn child(case: &str) {
    let parts: Vec<_> = case.split('/').collect();
    let (sink, when, ingresses) = (parts[0], parts[1], parts[2].parse::<u32>().unwrap());
    assert!(SINKS.contains(&sink));
    assert!(matches!(when, "before" | "after"));
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = crate::adapter::session_support::Fixture::new("append");
    let workspace = home.workspace.clone();
    let info = home.session.workspace().unwrap();
    let port = home.port.clone();
    let m = Coordinator::open(
        home.journal(),
        port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    PROBE.with(|probe| {
        *probe.borrow_mut() = Some(Probe {
            sink: sink.into(),
            when: when.into(),
            count: ingresses,
            cell: m.cell_for_test(),
            boundary: None,
            last_seq: 0,
            calls: 0,
        })
    });
    let mut contract = contract_with("MKFILE:session.txt");
    contract.requested_folder = Some(info.name);
    let drive = (|| -> Result<(), CoordinatorError> {
        let id = m.draft(contract.clone())?;
        m.revise(id, contract)?;
        m.submit(id)?;
        answering(&Yes, || m.request_consent(id))?;
        let mut approved = false;
        for _ in 0..8 {
            if m.state(id)?.is_some_and(|s| s.is_terminal()) {
                return Ok(());
            }
            if m.state(id)? == Some(MissionState::AwaitingApproval) && !approved {
                home.approve().map_err(|_| CoordinatorError::Stopped)?;
                approved = true;
            }
            m.advance(id)?;
        }
        panic!("finite driver did not reach terminal completion");
    })();
    let observation = PROBE.with(|probe| probe.borrow_mut().take().unwrap());
    let boundary = observation
        .boundary
        .expect("real coordinator must reach the named work append");
    println!(
        "COORDINATOR OBSERVED {case} boundary_seq={boundary} calls={}",
        observation.calls
    );
    let call_sink = matches!(
        sink,
        "lane_invoke" | "consent_reserve" | "phase_reserve" | "paused_store"
    );
    if call_sink {
        if ingresses == 0 {
            assert!(observation.calls > 0);
        } else {
            assert_eq!(
                observation.calls,
                usize::from(when == "after"),
                "{case}: actual mutation/invocation count"
            );
        }
    }
    assert!(
        !home.session.stop_snapshot().broker_stopped,
        "native Stop worker unscheduled"
    );
    assert!(
        !home.session.stop_snapshot().kernel_stopped,
        "native Stop worker unscheduled"
    );
    let lines = home.lines();
    let entries = verify(&lines).unwrap();
    for count in 0..=entries.len() {
        let observed = crate::journal::MemoryJournal::new();
        observed.set_lines(lines[..count].to_vec());
        assert_eq!(
            crate::coordinator::inspect_history(&observed)
                .unwrap()
                .records,
            count
        );
        assert_eq!(
            observed.lines(),
            lines[..count],
            "report-only inspection never appends"
        );
        let mut missions = BTreeMap::new();
        let mut next = 1;
        let mut session = Session::Never;
        for entry in &entries[..count] {
            replay(&mut missions, &mut next, &mut session, entry)
                .unwrap_or_else(|e| panic!("prefix {count}: {e:?}"));
        }
    }
    if ingresses == 0 {
        assert!(drive.is_ok(), "positive control must complete: {drive:?}");
        assert_eq!(
            std::fs::read(workspace.join("session.txt")).unwrap(),
            b"session.txt: made by Mother\n"
        );
        assert!(entries.iter().any(|e| matches!(
            &e.event,
            Event::Transition {
                trigger: Trigger::ActionApproved { .. }
            }
        )));
    } else {
        let forbidden: Vec<_> = entries
            .iter()
            .filter(|entry| {
                (entry.seq > boundary || ((when == "before" || call_sink) && entry.seq == boundary))
                    && work_key(&entry.event).is_some()
            })
            .collect();
        assert!(
            forbidden.is_empty(),
            "{case}: authoritative work append after ingress: {forbidden:?}"
        );
    }
    println!(
        "SESSION APPEND VERIFIED {case} prefixes={}",
        entries.len() + 1
    );
}

#[test]
fn session_r08_append_admission_and_every_prefix() {
    if let Ok(case) = std::env::var(CASE) {
        child(&case);
        return;
    }
    let mut failures = Vec::new();
    let mut count = 0;
    for sink in SINKS {
        for when in ["before", "after"] {
            for ingresses in 0..=3 {
                let case = format!("{sink}/{when}/{ingresses}");
                let mut child = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", TEST, "--nocapture"])
                    .env(CASE, &case)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let mut stdout = child.stdout.take().unwrap();
                let mut stderr = child.stderr.take().unwrap();
                let (send, receive) = std::sync::mpsc::sync_channel(1);
                let reader = std::thread::spawn(move || {
                    let mut text = String::new();
                    stdout.read_to_string(&mut text).unwrap();
                    let mut errors = String::new();
                    stderr.read_to_string(&mut errors).unwrap();
                    send.send((text, errors)).ok();
                });
                let output = receive.recv_timeout(Duration::from_secs(30));
                if output.is_err() {
                    child.kill().ok();
                }
                let status = child.wait().unwrap();
                reader.join().unwrap();
                match output {
                    Ok((out, _err))
                        if status.success()
                            && out.contains(&format!("SESSION APPEND VERIFIED {case}")) =>
                    {
                        assert!(out.contains(&format!("COORDINATOR OBSERVED {case} ")));
                        println!("COORDINATOR/{case}: reached=true conforms=true");
                    }
                    Ok((out, err)) => {
                        let reached = out.contains(&format!("COORDINATOR OBSERVED {case} "));
                        println!("COORDINATOR/{case}: reached={reached} conforms=false");
                        failures.push(format!("{case}: {out}{err}"));
                    }
                    Err(error) => failures.push(format!("{case}: bounded child failed {error}")),
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 144);
    assert!(
        failures.is_empty(),
        "executed {count} real append schedules; {}",
        failures.join("\n")
    );
}
