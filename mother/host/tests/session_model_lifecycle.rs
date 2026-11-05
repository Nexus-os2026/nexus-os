//! Literal lifecycle/resource oracle, from §§3/6 and B A1/A6/A7/A8/A9.
//! These are reference-machine checks, never reported as product coverage.
#![deny(unsafe_code)]
#[path = "session/model.rs"]
#[allow(dead_code)]
mod model;
use model::*;

#[test]
fn all_nine_states_keep_denial_and_terminal_lifecycle_independent() {
    use Lifecycle::*;
    let stop = [
        Emergency,
        Emergency,
        Emergency,
        Failed,
        ClosingRestricted,
        ClosingRestricted,
        SealedFinalizing,
        ClosedClean,
        ClosedUnclean,
    ];
    let quit = [
        ClosingNormal,
        ClosingNormal,
        ClosingRestricted,
        ClosingRestricted,
        ClosingNormal,
        ClosingRestricted,
        SealedFinalizing,
        ClosedClean,
        ClosedUnclean,
    ];
    for (i, initial) in LIFECYCLES.into_iter().enumerate() {
        for event in ["stop", "quit"] {
            let mut m = Model::normal();
            m.lifecycle = initial;
            m.emergency = matches!(initial, Emergency | ClosingRestricted);
            m.fatal = initial == Failed;
            if matches!(initial, ClosingNormal | ClosingRestricted) {
                m.close_generation = Some(0);
            }
            let prior = (m.generation, m.emergency, m.fatal);
            match event {
                "stop" => {
                    assert_eq!(m.stop(), i < 6);
                    assert_eq!(m.lifecycle, stop[i]);
                }
                "quit" => {
                    m.quit();
                    assert_eq!(m.lifecycle, quit[i]);
                }
                _ => unreachable!(),
            }
            if i >= 6 {
                assert_eq!((m.generation, m.emergency, m.fatal), prior);
            }
            if prior.1 {
                assert!(m.emergency);
            }
            if prior.2 {
                assert!(m.fatal);
            }
        }
    }
    println!("reference lifecycle: 18 event/state cases");
}

#[test]
fn exact_ack_identity_retains_every_field_and_never_discharges_resources() {
    let exact = [1, 2, 3, 4, 5, 6];
    for retained in [false, true] {
        for conclusive in [false, true] {
            for durable in [false, true] {
                let mut m = Model::normal();
                m.reserve(1);
                m.abandon(1);
                m.ack_owed = true;
                let before = m.resources.clone();
                assert_eq!(
                    m.ack(exact, retained, conclusive, durable),
                    retained && conclusive && durable
                );
                assert_eq!(m.resources, before);
                assert_eq!(m.ack_owed, !(retained && conclusive && durable));
                if retained && conclusive && durable {
                    assert!(
                        m.ack(exact, false, false, false),
                        "only exact retained duplicate receipt suffices"
                    );
                    for field in 0..6 {
                        let mut wrong = exact;
                        wrong[field] += 1;
                        assert!(!m.ack(wrong, true, true, true), "identity field {field}");
                    }
                }
            }
        }
    }
}

#[test]
fn inventory_capacity_and_every_discharge_fact_are_independent() {
    for count in [0, 1, 255, 256] {
        let mut m = Model::normal();
        for id in 0..count {
            assert!(m.reserve(id));
        }
        assert_eq!(m.resources.len(), usize::from(count));
        assert_eq!(m.reserve(count), count < 256);
        assert!(m.conserved());
    }
    let mut cases = 0;
    for state in [
        ResourceState::Reserved,
        ResourceState::Live,
        ResourceState::Borrowed,
        ResourceState::Retained,
        ResourceState::Released,
    ] {
        for bits in 0u16..256 {
            let flags: Vec<_> = (0..8).map(|bit| bits & (1 << bit) != 0).collect();
            let r = Resource {
                state,
                observed_absent: flags[0],
                removal_durable: flags[1],
                authority_closed: flags[2],
                intentional_retention: flags[3],
                records: [4, 5, 6].map(|bit| {
                    if flags[bit] {
                        Record::Recorded
                    } else if flags[7] {
                        Record::Unknown
                    } else {
                        Record::Failed
                    }
                }),
            };
            let expected = state == ResourceState::Released
                && flags[2]
                && flags[4]
                && flags[5]
                && flags[6]
                && (flags[3] || (flags[0] && flags[1]));
            assert_eq!(
                r.discharged(),
                expected,
                "state={state:?}, facts={bits:08b}"
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 1280);
    println!("reference discharge: {cases} cases; capacities 0/1/255/256");
}

#[test]
fn close_requires_all_sources_and_does_not_cover_coalesced_ingress() {
    for missing in 0..9 {
        let mut m = Model::normal();
        m.stop();
        m.stop();
        m.stop();
        m.stop();
        m.stop();
        m.recorded_ingress = m.ingress.clone();
        m.quit();
        m.quiescent = true;
        match missing {
            0 => {
                m.recorded_ingress.remove(&5);
            }
            1 => {
                m.workers.insert(1);
            }
            2 => {
                m.quiescent = false;
            }
            3 => {
                m.ack_owed = true;
            }
            4 => {
                m.terminal_record = Record::Unknown;
            }
            5 => {
                m.publication = Publication::Uncertain;
            }
            6 => {
                m.reserve(1);
                m.abandon(1);
            }
            7 => {
                m.fatal = true;
            }
            8 => {}
            _ => unreachable!(),
        }
        assert_eq!(m.seal(), missing == 8, "missing proof {missing}");
    }
    for mother in [
        Record::Owed,
        Record::Recorded,
        Record::Failed,
        Record::Unknown,
    ] {
        for primary in [
            Record::Owed,
            Record::Recorded,
            Record::Failed,
            Record::Unknown,
        ] {
            for delivered in [false, true] {
                for within_deadline in [false, true] {
                    let mut m = Model::normal();
                    m.quit();
                    m.quiescent = true;
                    assert!(m.seal());
                    m.end_records = [mother, primary];
                    m.delivered = delivered;
                    let expected = mother == Record::Recorded
                        && primary == Record::Recorded
                        && delivered
                        && within_deadline;
                    assert_eq!(m.finish(within_deadline), expected);
                    assert_eq!(
                        m.end_records,
                        [mother, primary],
                        "unclean does not erase a durable end"
                    );
                }
            }
        }
    }
}
