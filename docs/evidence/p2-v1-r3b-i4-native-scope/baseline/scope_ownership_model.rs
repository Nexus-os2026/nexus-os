//! SOURCE-DERIVED MODEL (P2-V1-R3B-I4 baseline). Not the production D-Bus
//! implementation, and no real systemd, D-Bus or cgroup is touched.
//!
//! It replays, branch for branch, the ownership and control flow of the
//! base commit 3805204f941d9694b2c0c36549d32a5f4e6c157c:
//!   scope.rs   ScopeManager::start_with_fault (lines 158-214), prove
//!              (216-248), call (123-148: a timeout or transport error is
//!              ScopeError::Bus);
//!   execution.rs attempt (478-482: Err(error) => NotRun::Scope, owned.scope
//!              stays None), Owned { helper, scope: Option<Scope> } (336-347),
//!              finalize/end (531-582: end(scope: Option<&Scope>, ..)),
//!              RetainedBoundary { scope: Option<Scope>, helper } (154-158),
//!              classify (254-259: confirmed cleanup + NotRun::Scope =>
//!              SandboxUnavailable).
//! over a modelled remote world in which each remote operation's effect and
//! its reply are independent. For every scenario it prints what the base
//! returns, what Nexus still owns after finalization, what may remain
//! remotely, and whether that remote state is representable in the base's
//! ownership types (Option<Scope> + Option<Helper>).
//! Standard library only. Build: rustc --edition 2021 -O scope_ownership_model.rs

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartEffect {
    /// The manager never acted on the request.
    None,
    /// The unit was created and the helper moved into its cgroup.
    CreatedPlaced,
    /// The unit was created, but the helper never entered it.
    CreatedNotPlaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reply {
    Delivered,
    /// The call timed out (BUS_CALL_TIMEOUT) or the transport failed.
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Proof {
    /// Every manager property read succeeds and matches.
    Ok,
    /// A GetUnit or property read after placement times out.
    Uncertain,
}

#[derive(Clone, Copy, Debug)]
struct Scenario {
    name: &'static str,
    start_effect: StartEffect,
    start_reply: Reply,
    proof: Proof,
    /// Whether StopUnit takes effect, and whether its reply arrives.
    stop_effect: bool,
    stop_reply: Reply,
}

/// What the remote side holds.
#[derive(Clone, Copy, Debug, Default)]
struct Remote {
    unit_exists: bool,
    helper_in_cgroup: bool,
    /// The unit's cgroup held the helper, which finalization then reaped:
    /// whether the emptied unit lingers is the manager's decision, never
    /// observed by Nexus.
    emptied_by_reap: bool,
}

/// What the base returns from start_with_fault.
#[derive(Debug)]
#[allow(dead_code)] // the fields are printed through Debug
enum StartResult {
    Scope { descriptor: bool },
    Bus(&'static str),
    NotPlaced,
}

/// The base's ownership after `attempt` and `finalize`.
#[derive(Debug)]
#[allow(dead_code)] // the fields are printed through Debug
struct Ownership {
    scope: Option<&'static str>,
    helper: bool,
    cleanup_confirmed: bool,
    stop_result_used: bool,
    cgroup_observed: bool,
}

fn start_with_fault(s: &Scenario, remote: &mut Remote, stop_called: &mut bool) -> StartResult {
    // let _job = self.call(.., "StartTransientUnit", ..)?;
    match s.start_effect {
        StartEffect::None => {}
        StartEffect::CreatedPlaced => {
            remote.unit_exists = true;
            remote.helper_in_cgroup = true;
        }
        StartEffect::CreatedNotPlaced => remote.unit_exists = true,
    }
    if s.start_reply == Reply::Lost {
        // `?`: Err(ScopeError::Bus("StartTransientUnit timed out")); no
        // StopUnit, no retained object of any kind.
        return StartResult::Bus("StartTransientUnit timed out");
    }
    // let proven = catch_unwind(|| self.prove(..));
    let proven = if !remote.helper_in_cgroup {
        // wait_for_placement(..)? => Err(NotPlaced)
        Err(StartResult::NotPlaced)
    } else {
        // Scope::open: a descriptor exists only inside `prove`, then
        // verify_limits, then GetUnit / Get(RuntimeMaxUSec, OOMPolicy).
        match s.proof {
            Proof::Ok => Ok(()),
            // `?` on the property call: the local Scope (its descriptor)
            // is dropped as `prove` returns.
            Proof::Uncertain => Err(StartResult::Bus("Get timed out")),
        }
    };
    match proven {
        Ok(()) => StartResult::Scope { descriptor: true },
        Err(error) => {
            // if !matches!(proven, Ok(Ok(_))) { let _: Result<..> = self.call(.., "StopUnit", ..); }
            *stop_called = true;
            if s.stop_effect {
                remote.unit_exists = false;
                remote.helper_in_cgroup = false;
            }
            // The StopUnit reply (delivered or lost) is discarded either way.
            let _ = s.stop_reply;
            error
        }
    }
}

fn run(s: &Scenario) -> (StartResult, Ownership, Remote, bool) {
    let mut remote = Remote::default();
    let mut stop_called = false;
    // attempt: owned.helper = Some(helper) before the scope.
    let result = start_with_fault(s, &mut remote, &mut stop_called);
    // match .. { Ok(scope) => owned.scope = Some(scope), Err(e) => return NotRun::Scope(e) }
    let scope_owned = matches!(result, StartResult::Scope { .. });
    // finalize -> end(owned.scope.as_ref(), &mut owned.helper): kill the
    // helper, reap it; with no Scope, `None => true`.
    let mut ownership = Ownership {
        scope: if scope_owned { Some("proven Scope") } else { None },
        helper: false,
        cleanup_confirmed: true,
        stop_result_used: false,
        cgroup_observed: scope_owned,
    };
    if !scope_owned {
        // The helper is killed and reaped: whatever cgroup it was in is
        // now empty, but a remote unit, a pending start job or an empty
        // scope created for it are invisible to `end(None, ..)`.
        if remote.helper_in_cgroup {
            remote.helper_in_cgroup = false;
            remote.emptied_by_reap = true;
        }
    } else {
        // A proven scope: cgroup.kill, reap, wait_empty through the
        // descriptor (observed). Its emptied unit is the manager's to remove.
        remote.helper_in_cgroup = false;
        remote.unit_exists = false;
    }
    ownership.helper = false;
    let _ = stop_called;
    (result, ownership, remote, stop_called)
}

fn main() {
    use Reply::*;
    let scenarios = [
        Scenario { name: "control: start delivered, placed, proof ok", start_effect: StartEffect::CreatedPlaced, start_reply: Delivered, proof: Proof::Ok, stop_effect: true, stop_reply: Delivered },
        Scenario { name: "start had no effect, reply lost", start_effect: StartEffect::None, start_reply: Lost, proof: Proof::Ok, stop_effect: true, stop_reply: Delivered },
        Scenario { name: "start took effect (placed), reply lost", start_effect: StartEffect::CreatedPlaced, start_reply: Lost, proof: Proof::Ok, stop_effect: true, stop_reply: Delivered },
        Scenario { name: "start took effect (not placed), reply lost", start_effect: StartEffect::CreatedNotPlaced, start_reply: Lost, proof: Proof::Ok, stop_effect: true, stop_reply: Delivered },
        Scenario { name: "not placed; stop took effect, reply lost", start_effect: StartEffect::CreatedNotPlaced, start_reply: Delivered, proof: Proof::Ok, stop_effect: true, stop_reply: Lost },
        Scenario { name: "not placed; stop had no effect, reply lost", start_effect: StartEffect::CreatedNotPlaced, start_reply: Delivered, proof: Proof::Ok, stop_effect: false, stop_reply: Lost },
        Scenario { name: "placed; property proof uncertain; stop took effect, reply lost", start_effect: StartEffect::CreatedPlaced, start_reply: Delivered, proof: Proof::Uncertain, stop_effect: true, stop_reply: Lost },
        Scenario { name: "placed; property proof uncertain; stop had no effect, reply lost", start_effect: StartEffect::CreatedPlaced, start_reply: Delivered, proof: Proof::Uncertain, stop_effect: false, stop_reply: Lost },
    ];
    println!("SOURCE-DERIVED MODEL of base 3805204f (scope.rs, execution.rs): not the production D-Bus implementation.");
    println!("Representable ownership in the base: Owned/RetainedBoundary = (Option<Scope>, Option<Helper>); no pending state.");
    println!();
    let mut counterexamples = 0;
    for s in &scenarios {
        let (result, ownership, remote, stop_called) = run(s);
        let remote_effect = matches!(s.start_effect, StartEffect::CreatedPlaced | StartEffect::CreatedNotPlaced);
        // (a) A remote effect actually survives while Nexus owns nothing
        // and reports its cleanup confirmed: a unit the helper never
        // entered, which its manager does not stop on its own.
        let survives = remote.unit_exists && !remote.emptied_by_reap
            && ownership.scope.is_none() && ownership.cleanup_confirmed;
        // (b) Nexus gave up every owner of a possible remote effect and
        // confirmed without observing it: indistinguishable, to Nexus, from
        // (a). Option<Scope> has no state for "may exist, not proven".
        let unrepresentable = remote_effect && ownership.scope.is_none();
        let counter = survives || unrepresentable;
        counterexamples += usize::from(counter);
        println!("SCENARIO {}", s.name);
        println!("  remote: start effect {:?}, start reply {:?}, proof {:?}, stop effect {}, stop reply {:?}",
                 s.start_effect, s.start_reply, s.proof, s.stop_effect, s.stop_reply);
        println!("  base start result: {result:?}; StopUnit called: {stop_called}; its result used: {}", ownership.stop_result_used);
        println!("  base ownership after finalization: scope {:?}, helper retained {}, cleanup confirmed {}",
                 ownership.scope, ownership.helper, ownership.cleanup_confirmed);
        let left = if !remote.unit_exists {
            "none"
        } else if remote.emptied_by_reap {
            "a unit emptied only by the helper's reap: lingering or not is the manager's decision"
        } else {
            "a unit the helper never entered (its manager does not stop an empty scope)"
        };
        println!("  actual remote state left (unknown to Nexus): {left}");
        println!("  cgroup observed through a retained descriptor: {}", ownership.cgroup_observed);
        println!("  classification: {}", if ownership.scope.is_some() { "(proceeds to launch)" } else { "NotRun::Scope + Cleanup::Confirmed => SandboxUnavailable" });
        println!("  verdict: {}", if survives {
            "COUNTEREXAMPLE (a): the remote effect survives; Nexus owns nothing and reports Cleanup::Confirmed"
        } else if unrepresentable {
            "COUNTEREXAMPLE (b): a possible remote effect left without a retained owner and confirmed unobserved (unrepresentable in Option<Scope>)"
        } else {
            "representable"
        });
        println!();
    }
    println!("counterexamples: {counterexamples} of {}", scenarios.len());
}
