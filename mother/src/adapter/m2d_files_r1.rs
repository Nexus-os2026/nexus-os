//! Oracles for Codex review 1 of PR #78 (M2d-C), HIGH 1, against the real
//! broker, pre-registered before the repair. They run inside
//! `real_broker_m1_oracles`, after `m2d_files` (which registered the
//! process's one workspace and left a live Files grant), before its
//! emergency-stop finale.
//!
//! A File the mission waits on is published; then the emergency Stop, the
//! Owner's Stop, or the mission's time budget ends the mission. Mother
//! journals the publication as the success it is (`ActionApproved`, the
//! action closed), ends the mission with
//! `file_publication_interrupted:<cause>`, and runs nothing more: neither
//! the Builder's File B nor the Verifier. The cause is Mother's own: the
//! Stop or the budget she ended the mission for. The emergency Stop lands during
//! the publication, after the broker's final check (at the first reading
//! of the broker's clock once the name is there, the check after the
//! publication), and is lifted by the Owner afterwards.
//!
//! HIGH 2 (recovery required) has no real-broker route from here: the
//! publication's directory-sync failure seam is the kernel's own,
//! crate-private; `oracles::m2d_c_r1` and the broker's view oracle cover
//! it.

use super::m2c_support::{consent, port_for, Home};
use super::{mother_agent, FileJournal, GovernedPort};
use crate::coordinator::Coordinator;
use crate::coordinator::Lanes;
use crate::journal::{Event, MissionId};
use crate::lane::LaneRole;
use crate::oracles::m2c_owner::OwnerLiftsTheStop;
use crate::oracles::m2d_c::{files_lanes, FilesOwner};
use crate::oracles::{contract_with, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::{Clock, ManualClock, SystemClock};
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use std::sync::Arc;
use std::time::Duration;

struct Scenario<'a> {
    home: &'a Home,
    folder: String,
    port: Arc<GovernedPort>,
    mother: Coordinator<GovernedPort>,
    owner: FilesOwner,
}

impl<'a> Scenario<'a> {
    fn new(home: &'a Home, journal: &str, clock: Arc<dyn Clock>) -> Self {
        let folder = home.control.workspace().expect("the workspace").name;
        let port = Arc::new(port_for(&home.control, true).with_workspace(folder.clone()));
        let mother = Coordinator::open(
            FileJournal::open(&home.dir.join(journal)).unwrap(),
            port.clone(),
            Lanes::new(files_lanes()).unwrap(),
            clock,
            home.private(),
        )
        .unwrap();
        Self {
            home,
            folder,
            port,
            mother,
            owner: FilesOwner::default(),
        }
    }

    /// Approved and advanced to the Builder's File A, waiting.
    fn waiting(&self, tag: &str) -> MissionId {
        let mut contract = contract_with(&format!(
            "MKFILE:{tag}-a.txt MKFILE:{tag}-b.txt VFILE:{tag}-c.txt"
        ));
        contract.requested_folder = Some(self.folder.clone());
        let id = self.mother.draft(contract).unwrap();
        self.mother.submit(id).unwrap();
        assert_eq!(consent(&self.mother, id, &Yes), Ok(MissionState::Approved));
        for _ in 0..3 {
            self.mother.advance(id).unwrap();
        }
        assert_eq!(self.mother.state(id), Some(MissionState::AwaitingApproval));
        id
    }

    /// The Owner approves the waiting File natively; returns its identity.
    fn approve(&self, id: MissionId) -> (RunId, CommitmentId) {
        let pending = self.port.pending(id).expect("the waiting action");
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = CommitmentId::parse(&pending.commitment).unwrap();
        self.home
            .control
            .authorize(commitment, &mother_agent(), run, &self.owner)
            .unwrap();
        (run, commitment)
    }

    fn execute(&self, (run, commitment): (RunId, CommitmentId)) {
        self.home
            .control
            .execute(commitment, &mother_agent(), run)
            .expect("the one publication completes");
    }

    /// Journaled as a success, the action closed, then ended with `code`;
    /// File A at its name, File B never asked, the Verifier never run.
    fn truthful_then_ended(&self, id: MissionId, tag: &str, run: RunId, code: &str) {
        assert_eq!(self.mother.state(id), Some(MissionState::Blocked), "{tag}");
        assert_eq!(self.mother.reason(id).as_deref(), Some(code), "{tag}");
        let events: Vec<Event> = self
            .mother
            .journal()
            .unwrap()
            .into_iter()
            .filter(|e| e.mission == id)
            .map(|e| e.event)
            .collect();
        let tail: Vec<&Event> = events
            .iter()
            .skip_while(|e| {
                !matches!(
                    e,
                    Event::Transition {
                        trigger: Trigger::ActionApproved { .. }
                    }
                )
            })
            .collect();
        assert!(
            matches!(
                tail.as_slice(),
                [
                    Event::Transition {
                        trigger: Trigger::ActionApproved { .. }
                    },
                    Event::ApprovalClosed { .. },
                    Event::Transition {
                        trigger: Trigger::Block { .. }
                    },
                ]
            ),
            "{tag}: {tail:?}"
        );
        let commitments = self
            .home
            .control
            .authority()
            .commitments()
            .views_of_run(run)
            .len();
        assert_eq!(commitments, 1, "{tag}: File B never asked");
        let verifier = events.iter().any(|e| {
            matches!(
                e,
                Event::LaneInvoked {
                    role: LaneRole::Verifier,
                    ..
                }
            )
        });
        assert!(!verifier, "{tag}: no later lane");
    }
}

pub(super) fn on_the_real_broker(home: &Home) {
    // The workspace m2d_files registered: the canonical temporary
    // directory's entry of that name.
    let folder = home.control.workspace().expect("the workspace").name;
    let ws = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(folder);
    let ws = ws.as_path();
    // The emergency Stop after the final check, found on Mother's next
    // advance.
    let s = Scenario::new(
        home,
        "m2d-c-r1-emergency.jsonl",
        Arc::new(SystemClock::default()),
    );
    let id = s.waiting("r1-emergency");
    let action = s.approve(id);
    let control = home.control.clone();
    let name = ws.join("r1-emergency-a.txt");
    home.clock.arm(
        move || name.exists(),
        move || {
            control.emergency_stop();
        },
    );
    s.execute(action);
    assert!(!home.clock.disarm(), "the Stop landed");
    assert!(ws.join("r1-emergency-a.txt").exists());
    let _ = s.mother.advance(id);
    s.truthful_then_ended(
        id,
        "emergency",
        action.0,
        "file_publication_interrupted:emergency_stop",
    );
    home.control.resume(&OwnerLiftsTheStop).unwrap();

    // The Owner's Stop of a mission whose File already ran.
    let s = Scenario::new(
        home,
        "m2d-c-r1-owner.jsonl",
        Arc::new(SystemClock::default()),
    );
    let id = s.waiting("r1-owner");
    let action = s.approve(id);
    s.execute(action);
    let _ = s.mother.stop(id);
    s.truthful_then_ended(
        id,
        "owner",
        action.0,
        "file_publication_interrupted:owner_stop",
    );

    // The time budget, spent (Mother's own clock) once the File ran.
    let clock = Arc::new(ManualClock::default());
    let s = Scenario::new(home, "m2d-c-r1-budget.jsonl", clock.clone());
    let id = s.waiting("r1-budget");
    let action = s.approve(id);
    s.execute(action);
    clock.advance(Duration::from_millis(3_600_001));
    let _ = s.mother.advance(id);
    s.truthful_then_ended(
        id,
        "budget",
        action.0,
        "file_publication_interrupted:time_budget",
    );
}
