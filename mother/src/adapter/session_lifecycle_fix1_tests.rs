//! Review 1 F02: a real pending File, exact Mother terminal ack, and both sealed states.
use super::*;
#[test]
fn fix1_b_real_file_close_refuses_retained_settlement_after_both_sealed_states() {
    const NAME: &str = "coordinator::lite::session_close_oracles::lifecycle_fix1::fix1_b_real_file_close_refuses_retained_settlement_after_both_sealed_states";
    if std::env::var("M2DD_FIX1_B_MOTHER").as_deref() != Ok("child") {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture"])
            .env("M2DD_FIX1_B_MOTHER", "child")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("bounded lifecycle child");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = Fixture::new("fix1-b-close");
    let file = Coordinator::open(
        home.journal(),
        home.port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("MKFILE:never.txt");
    contract.requested_folder = Some(home.session.workspace().unwrap().name);
    let id = file.draft(contract).unwrap();
    file.submit(id).unwrap();
    answering(&Yes, || file.request_consent(id)).unwrap();
    for _ in 0..8 {
        if file.state(id).unwrap() == Some(MissionState::AwaitingApproval) {
            break;
        }
        file.advance(id).unwrap();
    }
    assert_eq!(
        file.state(id).unwrap(),
        Some(MissionState::AwaitingApproval)
    );
    let quit = home.session.ingress().accept(CommandKind::Quit, 2).unwrap();
    let ending = file.prepare_session_end().unwrap();
    let inventory = home.session.settle_close_resources(&quit).unwrap();
    assert!(
        !inventory.entries.is_empty(),
        "a real retained native File entry is required"
    );
    assert!(inventory.entries.iter().all(|e| e.discharged));
    home.session.settle_close_authority(&quit).unwrap();
    let seal = home.session.seal_close(&quit).unwrap();
    assert_eq!(home.session.state(), SessionState::SealedFinalizing);
    let sealed_resources = home.session.settle_close_resources(&quit);
    let sealed_authority = home.session.settle_close_authority(&quit);
    ending.record(&seal).unwrap();
    home.session.observe_mother_end(&seal).unwrap();
    home.session.close_primary(&seal).unwrap();
    assert!(seal.finish_delivery());
    assert_eq!(home.session.state(), SessionState::ClosedClean);
    let closed_resources = home.session.settle_close_resources(&quit);
    let closed_authority = home.session.settle_close_authority(&quit);
    assert!(
        sealed_resources.is_err(),
        "Sealed-finalizing must refuse retained native resource settlement"
    );
    assert!(sealed_authority.is_err());
    assert!(
        closed_resources.is_err(),
        "Closed-clean must refuse retained native resource settlement"
    );
    assert!(closed_authority.is_err());
    assert!(!home.workspace.join("never.txt").exists());
    println!("FIX1 B real-file exact-ack=true sealed-refused=true closed-clean-refused=true");
}
