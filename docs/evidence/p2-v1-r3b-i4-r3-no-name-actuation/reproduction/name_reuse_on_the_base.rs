
// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R3 reproduction (evidence only; never part of a candidate).
// Appended, in a scratch snapshot of 2438f47f1726ae4075a52878bec005b6047691f0
// (the accepted R2 candidate), to crates/nexus-verifier-sandbox/src/
// execution/tests.rs, run once and removed. It uses only the base's own model
// and production code.
//
// The accepted path's name authority: a delivered and recorded
// StartTransientUnit reply (`accepted`) lets settling stop the unit by its
// name. The scenario: the start is accepted, the helper never enters the
// unit's cgroup (no candidate); Nexus's unit is later unloaded, and a foreign
// unit is loaded under the same name, holding a process of its own. A later
// settling attempt (a retry of the retained boundary) runs.
mod r3_reproduction {
    use super::*;
    use crate::scope::tests::{Cgroup, Unit};

    /// The foreign replacement's process (a model process id: the model never
    /// signals a real process).
    const FOREIGN: u32 = 4_000_003;

    #[test]
    fn r3_reproduction_an_accepted_operation_stops_a_foreign_unit_loaded_under_its_name() {
        // Accepted, never placed; the first settling's stop has no effect (so
        // the operation is retained, its unit still loaded).
        let world = World::new(|state| {
            state.placement = Placement::Never;
            state.stop = Script::always((false, Reply::Timeout));
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        println!("    after the execution: {observed:?}");
        assert!(observed.accepted && !observed.candidate && !observed.settled);
        // Nexus's unit is unloaded; a foreign unit is loaded under the same
        // name, holding its own process.
        let unit = {
            let mut state = world.lock();
            let unit = state.requested().unwrap();
            state.units.remove(&unit);
            let index = state.cgroups.len();
            state.cgroups.push(Cgroup {
                path: format!("{SLICE}/{unit}"),
                v2: true,
                removed: false,
                members: Default::default(),
                listed: true,
                killed: false,
                phantom: Phantom::None,
                limits: LIMITS,
                limit_mismatch: None,
                kills: 0,
            });
            state.place(FOREIGN, index);
            state.units.insert(
                unit.clone(),
                Unit {
                    ours: false,
                    cgroup: Some(index),
                },
            );
            unit
        };
        let stops_before = stops(&world);
        // The manager answers again and acts on StopUnit: a retry settles.
        world.release();
        let retried = boundary.retry();
        let state = world.lock();
        let stops_after = state.count(|call| matches!(call, Call::Stop(stopped) if *stopped == unit));
        let cgroup = state.cgroups.last().unwrap();
        println!(
            "    retry confirmed: {}; StopUnit({unit}) calls during the retry: {}; the foreign unit still loaded: {}; its cgroup killed: {}, removed: {}; its process now in: {:?}",
            retried.is_ok(),
            stops_after - stops_before.min(stops_after),
            state.units.contains_key(&unit),
            cgroup.killed,
            cgroup.removed,
            state.membership.get(&FOREIGN)
        );
        println!("    calls: {:?}", state.calls);
        // Reproduced: the historical accepted reply authorized a stop by name,
        // and it reached the foreign replacement.
        assert!(stops_after > stops_before, "no stop by name reached the replacement");
        assert!(!state.units.contains_key(&unit) && cgroup.killed && cgroup.removed);
        drop(state);
        if let Err(boundary) = retried {
            abandon(boundary, pid);
        }
        println!("REPRODUCED: an accepted operation stopped the foreign unit loaded under its name");
    }
}
