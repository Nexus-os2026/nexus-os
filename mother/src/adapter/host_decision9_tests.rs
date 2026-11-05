//! A host-gated port has no replay-adoption route, even before File selection.
use super::*;
use crate::oracles::m2c_owner::OwnerGrants as Grants;
use nexus_governed_control::authority::run::RunOrigin;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::governed::{AgentOutcome, Intent};

#[test]
fn decision9_host_port_does_not_adopt_legacy_replay() {
    const CHILD: &str = "M2DD_DECISION9_PORT_CHILD";
    const TEST: &str =
        "adapter::host_decision9_tests::decision9_host_port_does_not_adopt_legacy_replay";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("decision9-host-port-checked"));
        return;
    }
    let home = crate::test_home::TestHome::fresh_temp("decision9").unwrap();
    let control = &home.control;
    control
        .request_grant(
            &nexus_governed_control::governed::GrantRequest::Egress {
                origin: "https://example.org".into(),
                methods: vec!["POST".into()],
                allow_private: false,
            },
            std::time::Duration::from_secs(60),
            &Grants,
        )
        .unwrap();
    let run = control
        .open_run(mother_agent(), RunOrigin::AgentGoal)
        .unwrap();
    // Preparation only: this test makes no network request or model attempt.
    let AgentOutcome::AwaitingApproval(view) = control
        .agent_action(
            &mother_agent(),
            run,
            &Intent::Request(EgressIntent {
                method: "POST".into(),
                url: "https://example.org/decision9".into(),
                headers: vec![],
                body: None,
            }),
        )
        .unwrap()
    else {
        panic!("POST must wait for approval")
    };
    let pending = PendingAction {
        run: run.to_string(),
        commitment: view.id.to_string(),
        deadline_mono_ms: u64::MAX,
    };
    let port = super::m2c_support::port_for(control, true)
        .with_admission_gate(Arc::new(AdmissionGate::default()));
    let id = MissionId(9009);
    port.adopt(id, &pending);
    assert_eq!(
        port.run_of(id),
        None,
        "host replay must not adopt a surviving legacy run"
    );
    assert_eq!(port.pending(id), None);
    // Keep handed-back evidence for truthful withdrawal failure; no fake success.
    assert!(port.withdraw(id).is_err());
    let legacy = super::m2c_support::port_for(control, true);
    legacy.adopt(id, &pending);
    assert_eq!(legacy.run_of(id), Some(run));
    assert!(legacy.pending(id).is_some());
    control.cancel_run(run).unwrap();
    println!("decision9-host-port-checked");
}
