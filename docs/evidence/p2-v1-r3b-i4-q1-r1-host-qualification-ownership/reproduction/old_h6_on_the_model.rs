
// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-Q1-R1 reproduction (evidence only; never part of a
// candidate). Appended, in a scratch snapshot of
// 3d46b4dbe6b920dfcc2a1fa0c2d3b055f86c1823, to
// crates/nexus-verifier-sandbox/src/scope/tests.rs, run once and removed.
//
// It reproduces the Architect's finding on the deterministic model of the
// user manager and the kernel above: the H6 owner of 3d46b4d
// (tests/phase2_live_sandbox.rs, `struct Existing`, its `Drop` and
// `h6_unit_exists`, lines 1734-1818) is transliterated statement for
// statement, with the model's manager in place of the probe:
// `start_transient_scope(unit, pid)` is `start_scope` for the same unit and
// the same helper (`Answer::Returned` is `Started::Accepted`,
// `Answer::Refused { UnitExists }` is `Started::Collision`, a probe error is
// `Started::Uncertain`), and `stop_unit(unit)` is `stop_unit(unit)`. The
// helper is a live stand-in (`/bin/cat`), as in every model test.
//
// Then the same worlds are run through the repaired H6's first start,
// `execution::place` (production code, unchanged since ea475eac), for
// contrast.
mod q1r1_reproduction {
    use super::*;
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// A process of the foreign unit (a model process id: the model never
    /// signals a real process).
    const FOREIGN: u32 = 4_000_001;

    /// Old H6's owner (3d46b4d, lines 1736-1754), the probe replaced by the
    /// model's manager.
    struct Existing {
        probe: Box<dyn Manager>,
        helper: Option<Helper>,
        unit: String,
        done: bool,
    }

    impl Drop for Existing {
        fn drop(&mut self) {
            if self.done {
                return;
            }
            if let Some(mut helper) = self.helper.take() {
                let _ = helper.kill();
                let _ = helper.reap();
            }
            let _ = self.probe.stop_unit(&self.unit);
        }
    }

    /// Old H6 (3d46b4d, lines 1761-1780) up to its first answer: the owner
    /// is built before the first start, and any answer but a delivered
    /// reply panics with it alive. `pid` records the helper's process id.
    fn old_h6(world: &World, unit: &str, pid: &Cell<u32>) {
        let helper = super::helper();
        pid.set(helper.pid());
        let existing = Existing {
            probe: world.controller().manager,
            helper: Some(helper),
            unit: unit.to_string(),
            done: false,
        };
        let unit = existing.unit.clone();
        match existing.probe.start_scope(&ScopeRequest {
            unit: &unit,
            helper_pid: pid.get(),
            limits: &LIMITS,
        }) {
            Started::Accepted => {}
            other => panic!("h6: StartTransientUnit of the fresh name: {other:?}"),
        }
        // (The rest of old H6 is not reached in these worlds.)
        unreachable!("the first start was accepted");
    }

    fn run_old(world: &World, unit: &str) -> u32 {
        let pid = Cell::new(0);
        let outcome = catch_unwind(AssertUnwindSafe(|| old_h6(world, unit, &pid)));
        let message = outcome
            .err()
            .and_then(|panic| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        println!("    old H6 panicked: {message}");
        pid.get()
    }

    const UNIT: &str = "nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope";

    #[test]
    fn q1r1_reproduction_a_old_h6_stops_a_foreign_unit_that_answered_unit_exists() {
        // A foreign unit is already loaded under the name old H6 generated
        // (improbable for a random name; never impossible), holding a
        // process that is not the harness's.
        let world = World::new(|state| {
            state.cgroups.push(Cgroup {
                path: format!("{SLICE}/{UNIT}"),
                v2: true,
                removed: false,
                members: BTreeSet::from([FOREIGN]),
                listed: true,
                killed: false,
                phantom: Phantom::None,
                limits: LIMITS,
                limit_mismatch: None,
                kills: 0,
            });
            state.units.insert(
                UNIT.to_string(),
                Unit {
                    ours: false,
                    cgroup: Some(0),
                },
            );
            state.membership.insert(FOREIGN, format!("{SLICE}/{UNIT}"));
        });
        let pid = run_old(&world, UNIT);
        let state = world.lock();
        println!("    calls: {:?}", state.calls);
        println!(
            "    foreign unit loaded after old H6: {}; its cgroup killed: {}, removed: {}; its process now in: {:?}",
            state.units.contains_key(UNIT),
            state.cgroups[0].killed,
            state.cgroups[0].removed,
            state.membership.get(&FOREIGN)
        );
        // The defect: the first start was refused with UnitExists (the
        // model's collision), and old H6's owner then stopped that foreign
        // unit by its name alone: its process ended, its cgroup removed,
        // the unit unloaded.
        assert_eq!(
            state.count(|call| matches!(call, Call::Stop(unit) if unit == UNIT)),
            1,
            "old H6 did not stop the foreign unit"
        );
        assert!(!state.units.contains_key(UNIT));
        assert!(state.cgroups[0].killed && state.cgroups[0].removed);
        drop(state);
        assert!(!unreaped(pid));
        println!("REPRODUCED (A): a first start answered UnitExists made old H6 stop the foreign unit by name");
    }

    fn uncertain_world(panic: bool) -> World {
        World::new(|state| {
            // The request created the unit; its start job has not attached
            // the helper yet when the reply is lost (or the call panics).
            state.start_effect = true;
            state.start_reply = Reply::Timeout;
            state.placement = Placement::AfterReads(2);
            // Old H6's StopUnit, in the same failure, is just as uncertain.
            state.stop = Script::always((false, Reply::Timeout));
            if panic {
                state.panic_at = Some(Op::Start);
            }
        })
    }

    fn reaped_while_unresolved(world: &World, pid: u32, case: &str) {
        let state = world.lock();
        println!("    calls: {:?}", state.calls);
        let unit = state.requested().unwrap();
        println!(
            "    after old H6: helper {pid} unreaped: {}; unit {unit} loaded: {}; its start job still to attach: {:?}",
            unreaped(pid),
            state.units.contains_key(&unit),
            state.placing
        );
        // The defect: the helper was reaped while the request that may
        // still attach it is unresolved: the unit is loaded and its job
        // still targets the process id this process no longer holds (free
        // for reuse by any process), and nothing owns the operation.
        assert!(!unreaped(pid), "{case}: old H6 kept the helper unreaped");
        assert!(state.units.get(&unit).is_some_and(|unit| unit.ours));
        assert!(matches!(state.placing, Some((target, _, _)) if target == pid));
        assert_eq!(state.count(|call| matches!(call, Call::Stop(_))), 1);
        println!("REPRODUCED ({case}): old H6 reaped the helper while its first start could still act");
    }

    #[test]
    fn q1r1_reproduction_b_old_h6_reaps_its_helper_after_an_uncertain_first_start() {
        let world = uncertain_world(false);
        let pid = run_old(&world, UNIT);
        reaped_while_unresolved(&world, pid, "B, the reply lost");
    }

    #[test]
    fn q1r1_reproduction_c_old_h6_reaps_its_helper_after_a_panic_in_the_first_start() {
        let world = uncertain_world(true);
        let pid = run_old(&world, UNIT);
        reaped_while_unresolved(&world, pid, "C, a panic after dispatch");
    }

    #[test]
    fn q1r1_reproduction_d_the_repaired_first_start_keeps_ownership_in_the_same_worlds() {
        // (A) through `execution::place`: the name is refused as loaded by a
        // foreign unit; nothing is stopped, killed or claimed; only the
        // harness's helper is ended.
        let world = World::new(|state| state.collide = true);
        let helper = super::helper();
        let pid = helper.pid();
        let failed = place(&world.scopes(), helper, &LIMITS).expect_err("a collision was proven");
        let state = world.lock();
        let unit = state.requested().unwrap();
        println!("    place, collision: error {:?}, cleanup confirmed: {}, calls: {:?}", failed.error, failed.cleanup.is_confirmed(), state.calls);
        assert_eq!(state.count(|call| matches!(call, Call::Stop(_) | Call::Kill(_) | Call::Open(_))), 0);
        assert!(state.units.get(&unit).is_some_and(|unit| !unit.ours));
        drop(state);
        assert!(failed.cleanup.is_confirmed() && !unreaped(pid));
        println!("REPAIRED (A): the foreign unit was never stopped, killed or claimed; the harness's own helper was ended");
        // (B) and (C) through `execution::place`: whatever the request did,
        // the helper stays this process's unreaped child until the
        // operation is proven (the job attached it) or settled (its cgroup
        // ended and observed empty), and nothing crosses the owner.
        for (case, panic) in [("B", false), ("C", true)] {
            let world = uncertain_world(panic);
            let helper = super::helper();
            let pid = helper.pid();
            let placed = catch_unwind(AssertUnwindSafe(|| place(&world.scopes(), helper, &LIMITS)))
                .expect("a panic crossed place");
            let outcome = match placed {
                Ok(placed) => format!(
                    "discovered and proven, then ended by its owner: confirmed {}",
                    placed.end().is_confirmed()
                ),
                Err(failed) => match failed.cleanup {
                    Cleanup::Confirmed => {
                        format!("not proven ({:?}); settled, then confirmed", failed.error)
                    }
                    Cleanup::Failed(boundary) => {
                        let retained = format!(
                            "retained: scope {} helper {} unreaped {}",
                            boundary.holds_scope(),
                            boundary.holds_helper(),
                            unreaped(pid)
                        );
                        abandon(boundary, pid);
                        retained
                    }
                },
            };
            let state = world.lock();
            println!("    place, {case}: {outcome}; calls: {:?}", state.calls);
            assert!(
                state
                    .calls
                    .iter()
                    .all(|call| !matches!(call, Call::Membership(_, false))),
                "{case}: the helper was observed after it was reaped"
            );
            assert!(state.cgroups.iter().all(|cgroup| !cgroup.populated()), "{case}");
            drop(state);
            assert!(!unreaped(pid));
            println!("REPAIRED ({case}): the helper was reaped only after the operation was proven or settled");
        }
    }

    #[test]
    fn q1r1_reproduction_e_residual_a_collision_whose_reply_is_lost_is_an_uncertain_start() {
        // Not part of the finding: a residual of the accepted production
        // semantics that the repaired H6 inherits (its first start is the
        // production owner's). Scratch-only model extension (applied by
        // reproduce_old_h6.sh, never in a candidate): a collision whose
        // UnitExists reply is lost. To the backend it is an uncertain start
        // without a candidate, so the accepted settling attempts StopUnit
        // for its own generated name, which here a foreign unit holds; the
        // reply is never taken as proof, and the operation stays retained
        // with its helper unreaped.
        let world = World::new(|state| {
            state.collide = true;
            state.start_reply = Reply::Timeout;
        });
        let helper = super::helper();
        let pid = helper.pid();
        let failed = place(&world.scopes(), helper, &LIMITS).expect_err("a collision was proven");
        let state = world.lock();
        let unit = state.requested().unwrap();
        let stops = state.count(|call| matches!(call, Call::Stop(stopped) if *stopped == unit));
        let foreign_loaded = state.units.contains_key(&unit);
        println!("    place, collision with its reply lost: error {:?}; calls: {:?}", failed.error, state.calls);
        drop(state);
        let Cleanup::Failed(boundary) = failed.cleanup else {
            panic!("an uncertain start was confirmed without proof");
        };
        assert!(boundary.holds_scope() && boundary.holds_helper() && unreaped(pid));
        assert!(stops >= 1 && !foreign_loaded);
        println!("RESIDUAL (E): StopUnit attempts for the generated name: {stops}; the foreign unit is still loaded: {foreign_loaded}; the operation stays retained with its helper unreaped");
        abandon(boundary, pid);
    }
}
