//! Lite controls use production eligibility and real kernel publication.
use super::m2c_support::{port_for, Home};
use super::{create_private, mother_agent, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError, FileSession, Lanes};
use crate::effect::EffectPort;
use crate::evidence::{SettlementOutcome, FILE_UNCERTAIN_NOTICE};
use crate::journal::{Event, MemoryJournal, MissionId};
use crate::oracles::lite_support::{content_of, files_lanes, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::governed::GrantRequest;
use std::sync::Arc;
use std::time::Duration;

const CASE: &str = "NEXUS_LITE_ORACLE_CASE";

#[test]
fn lite_real_session_scenarios() {
    for scenario in [
        "fresh",
        "draft",
        "awaiting",
        "held",
        "race",
        "same-port",
        "rebuilt",
        "emergency-history",
        "emergency-live",
        "stop",
        "close",
        "two-files",
        "no-recycle",
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "adapter::lite::lite_real_session_child",
                "--nocapture",
            ])
            .env(CASE, scenario)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{scenario}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(&format!("lite-receipt:{scenario}")),
            "child must execute the named scenario"
        );
    }
}

struct Fx {
    home: Home,
    folder: String,
    ws: std::path::PathBuf,
    port: Arc<GovernedPort>,
    journal: MemoryJournal,
    owner: FilesOwner,
}
impl Fx {
    fn new() -> Self {
        assert!(nexus_governed_control::files::make_non_dumpable());
        let home = Home::fresh_temp("lite").unwrap();
        let ws = home.dir.join("workspace");
        create_private(&ws).unwrap();
        let info = home.control.register_workspace(&ws, &[]).unwrap();
        let owner = FilesOwner::default();
        home.control
            .request_grant(
                &GrantRequest::Files { workspace: info.id },
                Duration::from_secs(86400),
                &owner,
            )
            .unwrap();
        let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
        Self {
            home,
            folder: info.name,
            ws,
            port,
            journal: MemoryJournal::default(),
            owner,
        }
    }
    fn open(&self, port: Arc<GovernedPort>) -> Result<Coordinator<GovernedPort>, CoordinatorError> {
        Coordinator::open(
            self.journal.clone(),
            port,
            Lanes::new(files_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
            self.home.private(),
        )
    }
    fn file(&self) -> FileSession<GovernedPort> {
        self.open(self.port.clone())
            .unwrap()
            .into_file_session()
            .unwrap()
    }
    fn contract(&self, markers: &str) -> crate::contract::MissionContract {
        let mut c = contract_with(markers);
        c.requested_folder = Some(self.folder.clone());
        c
    }
    fn waiting(&self, mother: &FileSession<GovernedPort>, markers: &str) -> MissionId {
        let id = mother.draft(self.contract(markers)).unwrap();
        mother.submit(id).unwrap();
        assert_eq!(
            answering(&Yes, || mother.request_consent(id)),
            Ok(MissionState::Approved)
        );
        for _ in 0..3 {
            mother.advance(id).unwrap();
        }
        assert_eq!(
            mother.state(id).unwrap(),
            Some(MissionState::AwaitingApproval)
        );
        id
    }
    fn publish(&self, id: MissionId) {
        let p = self.port.pending(id).unwrap();
        let (run, commitment) = (
            RunId::parse(&p.run).unwrap(),
            CommitmentId::parse(&p.commitment).unwrap(),
        );
        self.home
            .control
            .authorize(commitment, &mother_agent(), run, &self.owner)
            .unwrap();
        self.home
            .control
            .execute(commitment, &mother_agent(), run)
            .unwrap();
    }
}

#[test]
fn lite_real_session_child() {
    let Ok(scenario) = std::env::var(CASE) else {
        return;
    };
    let f = Fx::new();
    match scenario.as_str() {
        "fresh" | "two-files" => {
            let m = f.file();
            let second = scenario == "two-files";
            let id = f.waiting(
                &m,
                if second {
                    "MKFILE:one.txt MKFILE:two.txt"
                } else {
                    "MKFILE:one.txt"
                },
            );
            f.publish(id);
            m.advance(id).unwrap();
            assert_eq!(
                std::fs::read_to_string(f.ws.join("one.txt")).unwrap(),
                content_of("MKFILE", "one.txt").unwrap()
            );
            if second {
                assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
                f.publish(id);
                m.advance(id).unwrap();
            }
            for _ in 0..8 {
                if m.state(id).unwrap().unwrap().is_terminal() {
                    break;
                }
                m.advance(id).unwrap();
            }
            assert_eq!(m.state(id).unwrap(), Some(MissionState::Done));
            let successes = m
                .journal()
                .unwrap()
                .iter()
                .filter(|e| {
                    matches!(
                        e.event,
                        Event::Transition {
                            trigger: Trigger::ActionApproved { .. }
                        }
                    )
                })
                .count();
            assert_eq!(successes, if second { 2 } else { 1 });
            m.close().unwrap();
        }
        "draft" | "awaiting" | "held" => {
            let m = f.open(f.port.clone()).unwrap();
            let id = m.draft(f.contract("MKFILE:one.txt")).unwrap();
            if scenario == "awaiting" {
                m.submit(id).unwrap();
            }
            let m = if scenario == "held" {
                drop(m);
                f.open(f.port.clone()).unwrap()
            } else {
                m
            };
            assert!(
                matches!(m.into_file_session(), Err(CoordinatorError::FileExclusive)),
                "every unfinished state prevents admission"
            );
            assert!(!f.ws.join("one.txt").exists());
        }
        "race" => {
            let m = Arc::new(f.file());
            let gate = Arc::new(std::sync::Barrier::new(3));
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let m = m.clone();
                    let gate = gate.clone();
                    let c = f.contract("MKFILE:one.txt");
                    std::thread::spawn(move || {
                        gate.wait();
                        m.draft(c)
                    })
                })
                .collect();
            gate.wait();
            assert_eq!(
                handles
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .filter(Result::is_ok)
                    .count(),
                1
            );
        }
        "same-port" | "rebuilt" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt");
            drop(m);
            let port = if scenario == "rebuilt" {
                Arc::new(port_for(&f.home.control, true).with_workspace(f.folder.clone()))
            } else {
                f.port.clone()
            };
            assert!(
                f.open(port).is_err(),
                "replacement refuses before replay adoption"
            );
            let p = f.port.pending(id).unwrap();
            assert!(
                f.home
                    .control
                    .authorize(
                        CommitmentId::parse(&p.commitment).unwrap(),
                        &mother_agent(),
                        RunId::parse(&p.run).unwrap(),
                        &f.owner
                    )
                    .is_err(),
                "original cell was engaged"
            );
        }
        "emergency-history" => {
            let m = f.open(f.port.clone()).unwrap();
            f.home.control.emergency_stop();
            f.home.control.resume(&f.owner).unwrap();
            assert!(m.into_file_session().is_err());
        }
        "emergency-live" | "stop" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt");
            if scenario == "emergency-live" {
                f.home.control.emergency_stop();
                let _ = m.advance(id);
            } else {
                m.stop(id).unwrap();
            }
            assert!(m.state(id).unwrap().unwrap().is_terminal());
            assert!(!f.ws.join("one.txt").exists());
            assert_eq!(
                m.journal()
                    .unwrap()
                    .iter()
                    .filter(|e| matches!(
                        e.event,
                        Event::Transition {
                            trigger: Trigger::Settled { .. }
                        }
                    ))
                    .count(),
                1
            );
        }
        "close" => {
            let m = f.file();
            f.waiting(&m, "MKFILE:one.txt");
            assert!(m.close().is_err());
        }
        "no-recycle" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt");
            m.stop(id).unwrap();
            assert!(m.draft(f.contract("MKFILE:two.txt")).is_err());
        }
        _ => panic!("unknown scenario"),
    }
    assert_eq!(FILE_UNCERTAIN_NOTICE,"A file may exist in your folder. A write already handed off may still finish. Mother will not retry or continue this mission.");
    assert!(!SettlementOutcome::Uncertain.conclusive());
    println!("lite-receipt:{scenario}");
}

#[path = "lite_settlement.rs"]
mod settlement;
