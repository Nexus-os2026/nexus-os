//! Oracle for Codex review 2 of PR #78 (M2d-C), HIGH 2 (the Architect's
//! decision B), against the real broker, pre-registered before the
//! repair; run inside `real_broker_m1_oracles` after `m2d_files_r1`.
//!
//! The port keeps the waited File's identity and outcome until the
//! coordinator acknowledges it: asking what became of it (`approval`) does
//! not drop it, so a Stop that comes right after still sees the published
//! File; once acknowledged, nothing is kept.

use super::m2c_support::{consent, port_for, Home};
use super::{mother_agent, FileJournal};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::{ApprovalStatus, EffectPort};
use crate::oracles::m2d_c::{files_lanes, FilesOwner};
use crate::oracles::{contract_with, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use std::sync::Arc;

pub(super) fn on_the_real_broker(home: &Home) {
    let folder = home.control.workspace().expect("the workspace").name;
    let port = Arc::new(port_for(&home.control, true).with_workspace(folder.clone()));
    let mother = Coordinator::open(
        FileJournal::open(&home.dir.join("m2d-c-r2.jsonl")).unwrap(),
        port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        Arc::new(SystemClock::default()),
        home.private(),
    )
    .unwrap();
    let mut contract = contract_with("MKFILE:r2-kept-a.txt MKFILE:r2-kept-b.txt");
    contract.requested_folder = Some(folder);
    let id = mother.draft(contract).unwrap();
    mother.submit(id).unwrap();
    assert_eq!(consent(&mother, id, &Yes), Ok(MissionState::Approved));
    for _ in 0..3 {
        mother.advance(id).unwrap();
    }
    assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
    let pending = port.pending(id).unwrap();
    let (run, commitment) = (
        RunId::parse(&pending.run).unwrap(),
        CommitmentId::parse(&pending.commitment).unwrap(),
    );
    let owner = FilesOwner::default();
    home.control
        .authorize(commitment, &mother_agent(), run, &owner)
        .unwrap();
    home.control
        .execute(commitment, &mother_agent(), run)
        .expect("published");

    assert_eq!(port.approval(id), ApprovalStatus::Approved);
    assert_eq!(
        port.publication(id),
        Ok(Some(ApprovalStatus::Approved)),
        "kept after the answer"
    );
    assert_eq!(
        port.approval(id),
        ApprovalStatus::Approved,
        "and asked again"
    );
    port.ack_publication(id);
    assert_eq!(port.publication(id), Ok(None), "dropped once acknowledged");
    mother.stop(id).unwrap();
}
