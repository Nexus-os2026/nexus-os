//! U4a S4 (design 4.2, 4.5; AM-2 B.2): each named broker lock poisoned by
//! `HostSession::route_poison`, then the same public probes in the same
//! order. Poisoning alone changes no observable state; each lock then fails
//! exactly its own probes, so each lock has its own row and a poison of any
//! other lock (or of none) cannot reproduce it.
use super::*;
use nexus_governed_control::governed::HaltReason;
use nexus_governed_control::host_session::RouteLock;

/// The probes after poisoning `lock`, each with the session's failure flag:
/// the store's health, the model slot, the close obligations, the run's
/// inference admission, its cancellation, then the door's halt.
fn poisoned(f: &Fixture, lock: RouteLock) -> Vec<String> {
    let o = f.open();
    o.session
        .register_local_model(&fake_ollama().0, spec())
        .unwrap();
    let (door, run, _) = o.door(&f.0);
    let ask = MissionConfirmation {
        contract_hash: "ab".repeat(32),
        request: "msn-000001/1/1".into(),
        summary: vec!["Route cell mission.".into()],
        page_facts: None,
    };
    door.confirm_mission(&ask).unwrap();
    let (hash, label) = (&ask.contract_hash, &ask.request);
    door.admit_files(run, hash, label).unwrap();
    let observed = || (o.ingress.failed(), o.session.state());
    let before = observed();
    o.session.route_poison(lock);
    assert_eq!(
        observed(),
        before,
        "poisoning alone changes nothing observed"
    );
    let identity = ModelIdentity::from_spec(&spec()).unwrap().digest();
    let probes: [&dyn Fn() -> String; 6] = [
        &|| format!("{:?}", o.evidence.try_health().map(|h| (h.latched, h.owed))),
        &|| format!("{}", door.model_identity().is_some()),
        &|| format!("{:?}", o.session.close_obligations().map(|_| ())),
        &|| format!("{:?}", door.admit_inference(run, hash, label, identity)),
        &|| format!("{:?}", door.cancel_run(run).map(|_| ())),
        &|| format!("{:?}", door.halt(HaltReason::JournalFailed).map(|_| ())),
    ];
    let probe = |p: &dyn Fn() -> String| format!("{} failed={}", p(), o.ingress.failed());
    probes.into_iter().map(probe).collect()
}
const FAILURE: &str = "Err(SessionFailure { error: ";

#[test]
fn route_s4_poisons_the_model_state_lock() {
    child("locks::route_s4_poisons_the_model_state_lock", |f| {
        let row = [
            "Some((false, 0)) failed=false",
            "false failed=true",
            "Err(Unavailable) failed=true",
            &format!("{FAILURE}Halted }}) failed=true"),
            "Ok(()) failed=true",
            "Ok(()) failed=true",
        ];
        assert_eq!(poisoned(f, RouteLock::ModelState), row);
    });
}
#[test]
fn route_s4_poisons_the_evidence_lock() {
    child("locks::route_s4_poisons_the_evidence_lock", |f| {
        let row = [
            "Some((true, 0)) failed=false",
            "true failed=false",
            "Ok(()) failed=false",
            "Ok(()) failed=false",
            "Ok(()) failed=false",
            "Ok(()) failed=false",
        ];
        assert_eq!(poisoned(f, RouteLock::EvidenceInner), row);
    });
}
#[test]
fn route_s4_poisons_the_run_table_lock() {
    child("locks::route_s4_poisons_the_run_table_lock", |f| {
        let row = [
            "Some((false, 0)) failed=false",
            "true failed=false",
            "Ok(()) failed=false",
            &format!("{FAILURE}Unavailable(\"model run hold unavailable\") }}) failed=true"),
            &format!("{FAILURE}Unavailable(\"the broker cannot read its runs\") }}) failed=true"),
            "Ok(()) failed=true",
        ];
        assert_eq!(poisoned(f, RouteLock::RunTable), row);
    });
}
#[test]
fn route_s4_poisons_the_pending_lock() {
    child("locks::route_s4_poisons_the_pending_lock", |f| {
        let row = [
            "Some((false, 0)) failed=false",
            "true failed=false",
            "Ok(()) failed=false",
            "Ok(()) failed=false",
            &format!("{FAILURE}Halted }}) failed=true"),
            "Ok(()) failed=true",
        ];
        assert_eq!(poisoned(f, RouteLock::Pending), row);
    });
}
#[test]
fn route_s4_poisons_the_producers_lock() {
    child("locks::route_s4_poisons_the_producers_lock", |f| {
        let closed = format!("{FAILURE}Closed(\"the session has ended\") }}) failed=true");
        let row = [
            "Some((false, 0)) failed=false",
            "true failed=false",
            "Err(Unavailable) failed=false",
            "Ok(()) failed=false",
            &closed,
            &closed,
        ];
        assert_eq!(poisoned(f, RouteLock::Producers), row);
    });
}
#[test]
fn route_s4_poisons_the_actions_lock() {
    child("locks::route_s4_poisons_the_actions_lock", |f| {
        let row = [
            "Some((false, 0)) failed=false",
            "true failed=false",
            "Err(Unavailable) failed=false",
            "Ok(()) failed=false",
            "Ok(()) failed=false",
            "Ok(()) failed=false",
        ];
        assert_eq!(poisoned(f, RouteLock::Actions), row);
    });
}
