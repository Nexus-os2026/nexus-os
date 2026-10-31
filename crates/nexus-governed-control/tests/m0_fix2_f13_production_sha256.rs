//! M0-FIX2 F13 oracle (host, public API): the production SHA-256 tool can
//! be granted and run through the front door on this host, whether its
//! `/usr/bin` entry is a regular file (GNU coreutils) or a link into the
//! rust-coreutils multicall binary. The grant pins a canonical path: the
//! executable actually launched.
#![cfg(target_os = "linux")]

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::AgentId;
use nexus_governed_control::authority::policy::GrantScope;
use nexus_governed_control::authority::run::RunOrigin;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::{GovernedControl, GrantRequest, Intent};
use nexus_governed_control::tool::ToolIntent;
use std::path::Path;
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
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

#[test]
fn m0_fix2_f13_the_production_sha256_tool_is_granted_and_runs() {
    let root = std::env::temp_dir().join(format!("nexus-m0-fix2-f13-{}", std::process::id()));
    let evidence = PrimaryEvidence::open(
        &root.join("p3-evidence").join("evidence.sqlite3"),
        Vec::new(),
    )
    .unwrap();
    let control = GovernedControl::new(
        &root,
        Vault::Disabled,
        evidence,
        Arc::new(SystemClock::default()),
    )
    .unwrap();
    let request = GrantRequest::Tool {
        tool: "text.sha256".into(),
    };
    let scope = control
        .grant_scope(&request)
        .expect("the production sha256 tool resolves on this host");
    let GrantScope::Tool { executable, .. } = &scope else {
        panic!("{scope:?}");
    };
    let pinned = Path::new(executable);
    assert_eq!(
        std::fs::canonicalize(pinned).unwrap(),
        pinned,
        "the grant pins a non-canonical path"
    );
    control
        .request_grant(&request, Duration::from_secs(60), &Owner)
        .unwrap();
    let agent = AgentId::new("agent-f13-sha256").unwrap();
    let run = control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let intent = Intent::Tool(ToolIntent {
        tool: "text.sha256".into(),
        input: serde_json::json!({ "text": "abc" }),
    });
    let view = control.propose(&agent, run, &intent).unwrap();
    control.authorize(view.id, &agent, run, &Owner).unwrap();
    let out = control.execute(view.id, &agent, run).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(
        out.text.as_deref(),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  input.txt\n")
    );
}
