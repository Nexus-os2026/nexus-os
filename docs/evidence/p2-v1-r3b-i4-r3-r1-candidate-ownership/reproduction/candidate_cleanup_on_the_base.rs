
// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R3-R1 reproduction (evidence only; never part of a candidate).
// Appended, in a scratch snapshot of 44a4fc6c2f5e81536c07119362c1507f00448cd7
// (the accepted R3 candidate), to crates/nexus-verifier-sandbox/src/
// execution/tests.rs, run once and removed. It uses only the base's own model
// and production code. A foreign unit's cgroup holding Nexus's retained
// helper beside a process of its own, with exactly the limits and manager
// properties Nexus expects (the model's foreign cgroups carry the requested
// limits, and its manager answers `Id`, `ControlGroup`, `RuntimeMaxUSec` and
// `OOMPolicy` exactly for whatever unit holds the name):
//
// A. the name is already held by that foreign unit: StartTransientUnit is
//    refused with exactly `UnitExists`, without effect, and that reply is
//    lost (Nexus sees `Started::Uncertain`);
// B. the start is accepted, Nexus's unit is unloaded, and a foreign unit is
//    loaded under the same name and holds Nexus's helper.
mod r3r1_reproduction {
    use super::*;
    use crate::scope::tests::Cgroup;

    /// The foreign units' own processes (model process ids: the model never
    /// signals a real process).
    const FOREIGN_A: u32 = 4_000_101;
    const FOREIGN_B: u32 = 4_000_102;

    /// What happened to the foreign cgroup (the last cgroup holding
    /// `member`) through the calls since `since`: opened, killed through a
    /// descriptor, its member ended. Printed and returned.
    fn foreign_fate(world: &World, member: u32, since: usize, label: &str) -> (usize, u32, bool) {
        let state = world.lock();
        let index = state
            .cgroups
            .iter()
            .rposition(|cgroup: &Cgroup| cgroup.members.contains(&member))
            .expect("the foreign cgroup");
        let cgroup = &state.cgroups[index];
        let opened = state.calls[since..]
            .iter()
            .filter(|call| matches!(call, Call::Open(path) if *path == cgroup.path))
            .count();
        let proof = state.calls[since..]
            .iter()
            .filter(|call| {
                matches!(
                    call,
                    Call::UnitId(_) | Call::ControlGroup(_) | Call::RuntimeMax(_) | Call::OomPolicy(_)
                )
            })
            .count();
        println!(
            "    {label}: foreign cgroup {} (index {index}): opened {opened} time(s); manager proof reads {proof}; \
             cgroup.kill through a descriptor of it: {}; killed: {}; removed: {}; its own process {member}: {}",
            cgroup.path,
            cgroup.kills,
            cgroup.killed,
            cgroup.removed,
            if cgroup.killed { "ENDED by Nexus's cgroup.kill" } else { "untouched" },
        );
        (opened, cgroup.kills, cgroup.killed)
    }

    /// The lost-`UnitExists` world: the foreign unit holds the name, the
    /// helper and a process of its own.
    fn lost_collision(state: &mut State) {
        state.collide = true;
        state.collision_holds_helper = true;
        state.foreign_member = Some(FOREIGN_A);
        state.collision_reply = Reply::Timeout;
    }

    #[test]
    fn r3r1_reproduction_a_lost_collision_holding_the_helper() {
        println!("A1: execution (execution::run's path): lost UnitExists, the foreign unit holds the helper and its own process");
        let world = World::new(lost_collision);
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        let proven = launched(&report);
        println!(
            "    not_run: {:?}; the launch reached the helper (only a Proven scope is launched): {proven}; cleanup confirmed: {}",
            report.not_run,
            report.cleanup.is_confirmed()
        );
        let (opened, kills, killed) = foreign_fate(&world, FOREIGN_A, 0, "A1");
        assert!(!is_child(pid) || !report.cleanup.is_confirmed());
        if let Cleanup::Failed(boundary) = report.cleanup {
            abandon(boundary, pid);
        }

        println!("A2: the harness's direct owner (execution::place): the same world");
        let world = World::new(lost_collision);
        let (placed, pid) = place_cat(&world);
        let a2_proven = match &placed {
            Ok(scoped) => {
                let unit = scoped.scope().map(|scope| scope.unit().to_string());
                println!("    placed: PROVEN scope {unit:?} (a ScopedHelper owning the foreign cgroup)");
                true
            }
            Err(failed) => {
                println!("    placement failed: {:?}", failed.error);
                false
            }
        };
        let cleanup = match placed {
            Ok(scoped) => scoped.end(),
            Err(failed) => failed.cleanup,
        };
        println!("    the owner's end: confirmed {}", cleanup.is_confirmed());
        let (a2_opened, a2_kills, a2_killed) = foreign_fate(&world, FOREIGN_A, 0, "A2");
        if let Cleanup::Failed(boundary) = cleanup {
            abandon(boundary, pid);
        }

        println!("A3: the same world, one manager property not as expected (OOMPolicy): no proof, then settling");
        let world = World::new(|state| {
            lost_collision(state);
            state.oom_policy = Property::Wrong;
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        println!(
            "    not_run: {:?}; launched: {}; cleanup confirmed: {}",
            report.not_run,
            launched(&report),
            report.cleanup.is_confirmed()
        );
        let (a3_opened, a3_kills, a3_killed) = foreign_fate(&world, FOREIGN_A, 0, "A3");
        if let Cleanup::Failed(boundary) = report.cleanup {
            abandon(boundary, pid);
        }

        // Reproduced: the uncertain start took the foreign cgroup as its own.
        assert!(proven && opened > 0 && kills > 0 && killed, "A1 not reproduced");
        assert!(a2_proven && a2_opened > 0 && a2_kills > 0 && a2_killed, "A2 not reproduced");
        assert!(a3_opened > 0 && a3_kills > 0 && a3_killed, "A3 not reproduced");
        println!("REPRODUCED A: a lost UnitExists with the helper in the foreign cgroup: that cgroup is opened, \
                  proven and launched into (A1, A2), and its own process ended by Nexus's cgroup.kill (A1, A2, A3)");
    }

    #[test]
    fn r3r1_reproduction_b_a_same_name_replacement_holding_the_helper() {
        println!("B1: accepted start, the helper never placed; Nexus's unit unloaded; a foreign unit loaded under \
                  the same name holds the helper and its own process; a retry settles");
        let world = World::new(|state| state.placement = Placement::Never);
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        println!("    after the execution: {observed:?}");
        let since = world.lock().calls.len();
        {
            let mut state = world.lock();
            let unit = state.requested().unwrap();
            state.unload(&unit);
            let index = state.load_foreign(&unit, FOREIGN_B);
            state.place(pid, index);
        }
        let retried = boundary.retry();
        println!("    retry confirmed: {}", retried.is_ok());
        let (b1_opened, b1_kills, b1_killed) = foreign_fate(&world, FOREIGN_B, since, "B1");
        match retried {
            Ok(()) => {}
            Err(boundary) => abandon(boundary, pid),
        }

        println!("B2: accepted start; while the proof waits for the helper, Nexus's unit is unloaded and a foreign \
                  unit loaded under the same name takes the helper (another thread, as another client would)");
        let world = World::new(|state| state.placement = Placement::Never);
        let attacker = {
            let world = world.clone();
            std::thread::spawn(move || {
                let started = std::time::Instant::now();
                loop {
                    {
                        let mut state = world.lock();
                        let start = state.calls.iter().find_map(|call| match call {
                            Call::Start(unit, pid) => Some((unit.clone(), *pid)),
                            _ => None,
                        });
                        if let Some((unit, pid)) = start {
                            state.unload(&unit);
                            let index = state.load_foreign(&unit, FOREIGN_B);
                            state.place(pid, index);
                            return true;
                        }
                    }
                    if started.elapsed() > Duration::from_secs(10) {
                        return false;
                    }
                    std::thread::sleep(Duration::from_micros(200));
                }
            })
        };
        let report = run_in(&world, None);
        let replaced = attacker.join().unwrap();
        let pid = requested_pid(&world);
        let b2_proven = launched(&report);
        println!(
            "    replaced during the proof: {replaced}; not_run: {:?}; the launch reached the helper (Proven): {b2_proven}; cleanup confirmed: {}",
            report.not_run,
            report.cleanup.is_confirmed()
        );
        let (b2_opened, b2_kills, b2_killed) = foreign_fate(&world, FOREIGN_B, 0, "B2");
        if let Cleanup::Failed(boundary) = report.cleanup {
            abandon(boundary, pid);
        }

        assert!(b1_opened > 0 && b1_kills > 0 && b1_killed, "B1 not reproduced");
        assert!(replaced && b2_proven && b2_opened > 0 && b2_killed, "B2 not reproduced");
        println!("REPRODUCED B: a same-name replacement holding the helper: settling opens it and ends its own \
                  process through cgroup.kill (B1); during the proof it is proven, launched into and ended (B2)");
    }
}
