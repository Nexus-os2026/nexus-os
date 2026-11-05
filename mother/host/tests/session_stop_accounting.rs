//! Amendment B A9: a joined worker is not proof of covered Stop events.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use support::faults::Fault;
use support::{Fixture, Pty};

#[test]
fn session_each_accepted_ingress_requires_its_own_verified_primary_record() {
    let mut failures = Vec::new();
    for events in [1, 4, 5] {
        let f = Fixture::new();
        let fault = Fault::new(&f, "evidence");
        let mut command = f.command("no work authority\n");
        fault.attach(&f, &mut command);
        let mut p = Pty::start(command);
        p.ready();
        fault.arm();
        p.line("stop");
        let held = fault.reached();
        assert!(held.path.contains("/p3-evidence/evidence.sqlite3"));
        p.expect("STOP REQUEST RECEIVED epoch=1");
        for event in 2..=events {
            p.line("stop");
            p.expect(&format!("STOP REQUEST RECEIVED epoch={event}"));
        }
        if events > 4 {
            p.expect("STOP COALESCED");
        }
        held.release();
        p.expect("STOP WORKERS outstanding=0\n");
        p.quit();
        let [recorded, opened, closed] = support::evidence::counts(&f);
        assert_eq!((opened, closed), (1, 1), "normal idle completion control");
        let correct = recorded == events;
        println!("STOP RECORDS accepted={events} verified={recorded} conforms={correct}");
        if !correct {
            failures.push((events, recorded));
        }
    }
    assert!(
        failures.is_empty(),
        "coalescing cannot discharge uncovered ingress: {failures:?}"
    );
}
