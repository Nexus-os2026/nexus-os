//! U4a-2 S3 (design 4.2): through the `SessionPort` forwarder, the original
//! door's File grant is revoked by the production route (a `GrantRevoked`
//! record), and the grant is then refused. This test keeps the port, so it
//! is not an S5 module test (AM-2 A.2 item 1).
use super::s5::{boot, consented, lanes, Recorded};
use super::*;
use nexus_mother::adapter::{FileJournal, FilePrivate, SessionPort};
use nexus_mother::coordinator::Coordinator;

#[test]
fn route_s3_revokes_the_original_grant_through_its_port() {
    child(
        "grant::route_s3_revokes_the_original_grant_through_its_port",
        |f| {
            let o = f.open();
            let b = boot(f, &o);
            let port = Arc::new(SessionPort::new(b.door, b.name.clone()));
            let journal = FileJournal::open(&f.0.join("mother-journal.jsonl")).unwrap();
            let private = FilePrivate::open_session(&f.0.join("private"), b.context).unwrap();
            let (calls, recorded) = (Arc::default(), Recorded::default());
            let lanes = lanes(&calls, &recorded);
            let clock = Arc::new(SystemClock::default());
            let m = Coordinator::open(journal, port.clone(), lanes, clock, private);
            let m = m.unwrap().into_file_session().unwrap();
            let id = consented(&m, &b.name);
            let _trace = arm("reg.reserve", Action::Trace);
            drain_trace();
            assert!(
                port.route_revoke_original_grant(),
                "an original door's grant"
            );
            let trace = drain_trace();
            let phases = trace.iter().filter(|e| e.point == "store.commit");
            let phases: Vec<_> = phases.map(|e| e.phase.as_deref()).collect();
            assert_eq!(phases, [Some("grant_revoked")], "its GrantRevoked record");
            let mut states = Vec::new();
            for _ in 0..6 {
                let advanced = m.advance(id).map(|_| ());
                states.push(format!("{advanced:?}/{:?}", m.state(id)));
                if advanced.is_err() {
                    break;
                }
            }
            // The Builder's inference admission is refused before its one call;
            // unrevoked, the same fixture reaches Done (s5's recorded path).
            let refused = [
                "Ok(())/Ok(Some(Investigating))",
                "Ok(())/Ok(Some(Building))",
            ];
            let refused = [&refused[..], &["Ok(())/Ok(Some(Blocked))"]].concat();
            assert_eq!(&states[..3], refused, "{states:?}");
            assert_eq!(calls.load(Ordering::SeqCst), 0, "no model call was made");
            let reason = m.reason(id).unwrap();
            assert_eq!(reason.as_deref(), Some("model_admission_refused"));
        },
    );
}
