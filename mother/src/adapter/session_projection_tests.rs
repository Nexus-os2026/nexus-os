//! Observe each adapter projection after its actual owning hold is acquired.
use super::*;
use crate::coordinator::{Coordinator, Lanes};
use crate::oracles::{contract_with, lite_support::files_lanes, m2c_confirmers::answering, Yes};
use std::cell::RefCell;
struct Probe {
    sink: &'static str,
    when: &'static str,
    ingresses: usize,
    cell: HaltCell,
    hit: usize,
    entered: usize,
}
thread_local! { static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) }; }
pub(in crate::adapter) fn at(sink: &str, when: &str) {
    PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let Some(p) = probe.as_mut() else {
            return;
        };
        if sink != p.sink {
            return;
        }
        if when == "after" {
            p.entered += 1;
        }
        if p.hit == 0 && when == p.when {
            p.hit += 1;
            for _ in 0..p.ingresses {
                p.cell.engage_emergency();
            }
        }
    });
}
#[test]
fn session_adapter_each_projection_checks_after_the_owning_hold() {
    assert!(nexus_governed_control::files::make_non_dumpable());
    let selected = std::env::var("M2DD_PORT_FINAL").ok();
    if selected.is_none() {
        let mut failures = Vec::new();
        for sink in [
            "run_project",
            "file_binding_project",
            "file_effect_select",
            "effect_dispatch",
        ] {
            for when in ["before", "after"] {
                for count in 0..=3 {
                    let key = format!("{sink}/{when}/{count}");
                    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                        .args(["--exact", "adapter::session_port::session_projection_tests::session_adapter_each_projection_checks_after_the_owning_hold", "--nocapture"])
                        .env("M2DD_PORT_FINAL", &key)
                        .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
                    let mut stdout = child.stdout.take().unwrap();
                    let mut stderr = child.stderr.take().unwrap();
                    let (send, receive) = std::sync::mpsc::sync_channel(1);
                    let reader = std::thread::spawn(move || {
                        use std::io::Read;
                        let mut out = String::new();
                        let mut err = String::new();
                        stdout.read_to_string(&mut out).unwrap();
                        stderr.read_to_string(&mut err).unwrap();
                        send.send((out, err)).ok();
                    });
                    let output = receive.recv_timeout(std::time::Duration::from_secs(30));
                    if output.is_err() {
                        child.kill().ok();
                    }
                    let status = child.wait().unwrap();
                    reader.join().unwrap();
                    if let Ok((out, err)) = output {
                        println!("{out}");
                        if !status.success() || !out.contains("conforms=true") {
                            eprintln!("{err}");
                            failures.push(key);
                        }
                    } else {
                        failures.push(key);
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "32 actual adapter schedules: {failures:?}"
        );
        return;
    }
    let mut failures = Vec::new();
    let mut executed = 0;
    for sink in [
        "run_project",
        "file_binding_project",
        "file_effect_select",
        "effect_dispatch",
    ] {
        for when in ["before", "after"] {
            for ingresses in 0..=3 {
                let key = format!("{sink}/{when}/{ingresses}");
                if selected.as_ref().is_some_and(|value| value != &key) {
                    continue;
                }
                let home = crate::adapter::session_support::Fixture::new(&format!(
                    "projection-{sink}-{when}-{ingresses}"
                ));
                let m = Coordinator::open(
                    home.journal(),
                    home.port.clone(),
                    Lanes::new(files_lanes()).unwrap(),
                    home.clock.clone(),
                    home.private(),
                )
                .unwrap()
                .into_file_session()
                .unwrap();
                PROBE.with(|probe| {
                    *probe.borrow_mut() = Some(Probe {
                        sink,
                        when,
                        ingresses,
                        cell: m.cell_for_test(),
                        hit: 0,
                        entered: 0,
                    })
                });
                let mut contract = contract_with("MKFILE:projection.txt");
                contract.requested_folder = Some(home.session.workspace().unwrap().name);
                let drive = (|| -> Result<(), crate::coordinator::CoordinatorError> {
                    let id = m.draft(contract)?;
                    m.submit(id)?;
                    answering(&Yes, || m.request_consent(id))?;
                    for _ in 0..8 {
                        if m.state(id)? == Some(crate::state::MissionState::AwaitingApproval) {
                            return Ok(());
                        }
                        m.advance(id)?;
                    }
                    panic!("finite projection driver did not reach approval");
                })();
                let p = PROBE.with(|probe| probe.borrow_mut().take().unwrap());
                let allowed = ingresses == 0 || when == "after";
                let conforms = p.hit == 1
                    && p.entered == usize::from(allowed)
                    && (ingresses != 0 || drive.is_ok());
                assert!(!home.session.stop_snapshot().broker_stopped);
                assert!(!home.session.stop_snapshot().kernel_stopped);
                assert!(!home.workspace.join("projection.txt").exists());
                println!(
                    "PORT-FINAL/{key}: entered={} native_workers=0 conforms={conforms}",
                    p.entered
                );
                executed += 1;
                if !conforms {
                    failures.push(key);
                }
            }
        }
    }
    assert_eq!(executed, if selected.is_some() { 1 } else { 32 });
    assert!(
        failures.is_empty(),
        "{executed} actual adapter schedules: {failures:?}"
    );
}
