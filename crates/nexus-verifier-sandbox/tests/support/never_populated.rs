//! The live qualification of the never-populated scope lifecycle
//! (P2-V1-R3B-I4-Q3-R1): live case 18, `p2d_live_unmovable_process_fails_closed`.
//! Shared by the live suite (`phase2_live_sandbox.rs`) and its deterministic
//! fixture controls (`phase2_never_populated.rs`, which never reach a real
//! user manager); each uses a part.
//!
//! Live run 37163435032 showed the real lifecycle (systemd 255.4, Linux 6.17).
//! A placement of a helper that is killed but not reaped is not refused: the
//! manager still resolves the exiting process, the kernel's migration skips
//! it, and the start succeeds. Its scope runs never populated, so no
//! cgroup-empty notification ends it (the kernel notifies `cgroup.events`
//! only when `populated` changes); it stays loaded and running until its
//! runtime backstop (`RuntimeMaxUSec`) expires and the manager collects it.
//! Production never acts on a unit by its name, so its accepted operation
//! without a candidate is confirmed only by the manager's absence of the unit
//! with the helper outside it, after that collection, and its helper is
//! reaped only then.
//!
//! [`qualify`] proves that lifecycle, in order, every wait bounded:
//!
//! 1. the baseline of loaded verifier scopes;
//! 2. the helper is killed and observed exited by its own exit report, not
//!    reaped ([`exit_unreaped`]: never a reap, never a sleep taken as proof),
//!    so its process id stays reserved;
//! 3. the production placement proves no scope and retains the operation;
//! 4. exactly one new verifier scope is loaded: the retained operation's own
//!    unit (its name in the boundary's own description);
//! 5. every retry before the backstop leaves the operation unconfirmed, the
//!    scope loaded and the helper unreaped: expected, never a failure; a
//!    confirmation then is a failure;
//! 6. the scope is collected no earlier than its backstop and within a bound
//!    after it, the helper still unreaped;
//! 7. a bounded explicit retry then confirms the operation gone, and only
//!    then is the helper reaped (by production);
//! 8. the loaded scopes return to the baseline.
//!
//! A failure reports what was observed: the unit, the placement's error,
//! whether a boundary was retained, the retry, the elapsed time, whether the
//! cleanup was confirmed, and the loaded scopes. A boundary still held is
//! released explicitly ([`World::release`]), never as a confirmation.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::time::Duration;

use nexus_verifier_sandbox::policy::ResourcePolicy;

/// Loaded verifier scopes, by unit name.
pub type Scopes = BTreeSet<String>;

/// The case's runtime backstop, in seconds: short, so that the manager
/// itself ends the never-populated scope within the case, and long enough
/// that production's placement (its placement wait, then its own settling)
/// and at least one full explicit retry end before it ([`Plan::coherent`]).
pub const BACKSTOP_SECS: u64 = 45;
/// Room left before the backstop beyond production's own bounds.
pub const MARGIN: Duration = Duration::from_secs(5);
/// Bound on observing the killed helper's exit.
pub const EXIT_WITHIN: Duration = Duration::from_secs(10);
/// Bound, after the backstop, on the manager's collection of the scope.
pub const COLLECTED_WITHIN: Duration = Duration::from_secs(30);
/// The scope's backstop is armed when the manager starts it, after the case
/// has taken its starting time: a scope collected earlier than the backstop
/// less this tolerance ended some other way.
pub const EARLY_TOLERANCE: Duration = Duration::from_secs(1);
/// Bound on each observation, and on the scopes returning to the baseline.
pub const OBSERVED_WITHIN: Duration = Duration::from_secs(10);
/// Pacing of a poll: never proof of anything.
pub const POLL: Duration = Duration::from_millis(200);
/// Bound on the explicit retries after the collection.
pub const ATTEMPTS: usize = 3;

/// The case's resource policy: `base` with [`BACKSTOP_SECS`] as its runtime
/// backstop, nothing else changed. A live qualification fixture only; the
/// production policy and its default are untouched.
pub fn limits(base: ResourcePolicy) -> ResourcePolicy {
    ResourcePolicy {
        runtime_backstop_secs: BACKSTOP_SECS,
        ..base
    }
}

/// The case's bounds: production's own (its placement wait and its settling
/// bound) and the case's.
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    pub backstop: Duration,
    /// Production's placement wait.
    pub placement: Duration,
    /// Production's settling bound: one retry takes up to this long.
    pub settle: Duration,
    pub exit_within: Duration,
    pub collected_within: Duration,
    pub early_tolerance: Duration,
    pub observed_within: Duration,
    pub poll: Duration,
    pub attempts: usize,
}

impl Plan {
    /// The case's plan over production's `placement` wait and `settle`
    /// bound.
    pub fn new(placement: Duration, settle: Duration) -> Self {
        Self {
            backstop: Duration::from_secs(BACKSTOP_SECS),
            placement,
            settle,
            exit_within: EXIT_WITHIN,
            collected_within: COLLECTED_WITHIN,
            early_tolerance: EARLY_TOLERANCE,
            observed_within: OBSERVED_WITHIN,
            poll: POLL,
            attempts: ATTEMPTS,
        }
    }

    /// The live case's plan: production's own bounds.
    pub fn live() -> Self {
        Self::new(
            nexus_verifier_sandbox::scope::PLACEMENT_TIMEOUT,
            nexus_verifier_sandbox::scope::SETTLE_TIMEOUT,
        )
    }

    /// The backstop leaves room for production's placement (its wait, then
    /// its settling) and for at least one full retry before it.
    pub fn coherent(&self) -> bool {
        self.backstop > self.placement + self.settle + self.settle + MARGIN
            && self.attempts > 0
            && !self.poll.is_zero()
    }
}

/// The helper's exit as its parent observes it without reaping it: the
/// `waitid` report's `si_code` and `si_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    pub code: i32,
    pub status: i32,
}

impl Exit {
    /// Ended by `SIGKILL`.
    pub fn killed(&self) -> bool {
        self.code == libc::CLD_KILLED && self.status == libc::SIGKILL
    }
}

/// The helper's exit report, without reaping it: `waitid(P_PID, pid,
/// WEXITED | WNOHANG | WNOWAIT)`. `None` while it has not exited; `ECHILD`
/// once it is no longer this process's unreaped child. It never blocks and
/// never reaps: the child stays waitable, its process id reserved.
pub fn exit_unreaped(pid: u32) -> io::Result<Option<Exit>> {
    // SAFETY: siginfo_t is plain data; waitid fills it.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: WNOWAIT leaves the child waitable (nothing is reaped) and
    // WNOHANG never blocks.
    let rc = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: waitid filled these for P_PID (all zero: not exited yet).
    let (reported, code, status) = unsafe { (info.si_pid(), info.si_code, info.si_status()) };
    if reported == 0 {
        return Ok(None);
    }
    if reported as u32 != pid {
        return Err(io::Error::other(format!(
            "waitid reported process {reported}, not {pid}"
        )));
    }
    Ok(Some(Exit { code, status }))
}

/// Whether `pid` is still this process's unreaped child, running or not
/// (see [`exit_unreaped`]).
pub fn unreaped(pid: u32) -> io::Result<bool> {
    match exit_unreaped(pid) {
        Ok(_) => Ok(true),
        Err(error) if error.raw_os_error() == Some(libc::ECHILD) => Ok(false),
        Err(error) => Err(error),
    }
}

/// A placement that proved no scope: production's error, and the boundary it
/// retained (`None`: its cleanup was confirmed at once).
#[derive(Debug)]
pub struct Refused<B> {
    pub error: String,
    pub retained: Option<B>,
}

/// What the case acts on: the live suite's real helper, production's
/// placement and retry and the checked observation, or a fixture's model of
/// them. Times are elapsed times since the world was made, monotonic.
pub trait World {
    /// Production's retained boundary of the unconfirmed operation.
    type Boundary;
    /// A proven placement: a failure of the case.
    type Placed: fmt::Debug;
    fn now(&self) -> Duration;
    /// Pace a poll: never proof of anything.
    fn pause(&mut self, interval: Duration);
    /// Kill the helper (`SIGKILL`) without reaping it.
    fn kill(&mut self) -> Result<(), String>;
    /// The helper's exit report without reaping it (see [`exit_unreaped`]).
    fn exit(&mut self) -> Result<Option<Exit>, String>;
    /// Whether the helper is still this process's unreaped child.
    fn unreaped(&mut self) -> Result<bool, String>;
    /// Production's placement of the helper (consumed).
    fn place(&mut self) -> Result<Self::Placed, Refused<Self::Boundary>>;
    /// One explicit retry of the retained boundary (production's); a failed
    /// retry returns the boundary, still held.
    fn retry(&mut self, boundary: Self::Boundary) -> Result<(), Self::Boundary>;
    /// The boundary's own description: it names its unit.
    fn describe(&self, boundary: &Self::Boundary) -> String;
    /// The loaded verifier scopes, observed by `deadline`; a failed
    /// observation is never "no scopes".
    fn scopes(&mut self, deadline: Duration) -> Result<Scopes, String>;
    /// Release a boundary still unconfirmed after `report`: its drop
    /// backstop, never a confirmation.
    fn release(&mut self, boundary: Self::Boundary, report: &str);
}

/// What the case qualified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qualified {
    /// The never-populated scope's unit.
    pub unit: String,
    /// Production's placement error.
    pub error: String,
    /// Retries that ended before the backstop, every one unconfirmed.
    pub unconfirmed_before_backstop: usize,
    /// When the scope was observed collected, since the placement began.
    pub collected_at: Duration,
    /// The explicit retry after the collection that confirmed the operation.
    pub confirmed_on: usize,
}

/// Why the case failed, with what was observed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Failure {
    pub what: String,
    pub unit: Option<String>,
    pub error: Option<String>,
    pub retained: bool,
    pub retries: usize,
    /// Since the placement began (`None`: before it).
    pub elapsed: Option<Duration>,
    pub confirmed: bool,
    pub scopes: Option<Result<Scopes, String>>,
    pub boundary: Option<String>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [unit {:?}; placement error {:?}; boundary retained {}; retries {}; elapsed {:?}; \
             cleanup confirmed {}; loaded scopes {:?}; boundary {:?}]",
            self.what,
            self.unit,
            self.error,
            self.retained,
            self.retries,
            self.elapsed,
            self.confirmed,
            self.scopes,
            self.boundary
        )
    }
}

/// The facts gathered so far, for a failure's report.
#[derive(Default)]
struct Facts {
    failure: Failure,
    started: Option<Duration>,
}

impl Facts {
    fn fail(&self, now: Duration, what: impl Into<String>) -> Box<Failure> {
        let mut failure = self.failure.clone();
        failure.what = what.into();
        failure.elapsed = self.started.map(|started| now.saturating_sub(started));
        Box::new(failure)
    }
}

/// Release `boundary` explicitly once the failure is reported, and return
/// the failure.
fn give_up<W: World>(world: &mut W, boundary: W::Boundary, failure: Box<Failure>) -> Box<Failure> {
    world.release(boundary, &failure.to_string());
    failure
}

/// Qualify the never-populated scope lifecycle over `world` (see the module
/// documentation): the operation is unconfirmed before the backstop, and
/// confirmed, its helper reaped, only after the manager collected the scope.
pub fn qualify<W: World>(world: &mut W, plan: &Plan) -> Result<Qualified, Box<Failure>> {
    let mut facts = Facts::default();
    if !plan.coherent() {
        return Err(facts.fail(
            world.now(),
            "the backstop leaves no room for the placement and a retry before it",
        ));
    }
    // 1. The baseline.
    let deadline = world.now() + plan.observed_within;
    let before = match world.scopes(deadline) {
        Ok(before) => before,
        Err(error) => return Err(facts.fail(world.now(), format!("the loaded scopes: {error}"))),
    };
    facts.failure.scopes = Some(Ok(before.clone()));
    // 2. Killed, observed exited by its own report, not reaped.
    if let Err(error) = world.kill() {
        return Err(facts.fail(world.now(), format!("the helper was not killed: {error}")));
    }
    let deadline = world.now() + plan.exit_within;
    let exit = loop {
        match world.exit() {
            Ok(Some(exit)) => break exit,
            Ok(None) if world.now() >= deadline => {
                return Err(facts.fail(
                    world.now(),
                    "the killed helper did not exit within its bound",
                ))
            }
            Ok(None) => world.pause(plan.poll),
            Err(error) => {
                return Err(facts.fail(world.now(), format!("the helper's exit: {error}")))
            }
        }
    };
    if !exit.killed() {
        return Err(facts.fail(
            world.now(),
            format!("the helper exited otherwise: {exit:?}"),
        ));
    }
    match world.unreaped() {
        Ok(true) => {}
        Ok(false) => {
            return Err(facts.fail(world.now(), "the helper was reaped before its placement"))
        }
        Err(error) => return Err(facts.fail(world.now(), format!("the helper's state: {error}"))),
    }
    // 3. The production placement proves no scope and retains the operation.
    facts.started = Some(world.now());
    let started = world.now();
    let since = |world: &W| world.now().saturating_sub(started);
    let mut boundary = match world.place() {
        Ok(placed) => {
            return Err(facts.fail(
                world.now(),
                format!("a scope was proven for an unmovable process: {placed:?}"),
            ))
        }
        Err(Refused { error, retained }) => {
            facts.failure.error = Some(error);
            match retained {
                Some(boundary) => boundary,
                None => {
                    facts.failure.confirmed = true;
                    return Err(facts.fail(
                        world.now(),
                        "the cleanup was confirmed at once: no never-populated scope was retained",
                    ));
                }
            }
        }
    };
    facts.failure.retained = true;
    facts.failure.boundary = Some(world.describe(&boundary));
    if since(world) >= plan.backstop {
        let failure = facts.fail(world.now(), "the placement returned after the backstop");
        return Err(give_up(world, boundary, failure));
    }
    // 4. Exactly one new verifier scope: this operation's own unit.
    let deadline = world.now() + plan.observed_within;
    let loaded = world.scopes(deadline);
    facts.failure.scopes = Some(loaded.clone());
    let unit = match &loaded {
        Ok(loaded) => {
            let new: Vec<&String> = loaded.difference(&before).collect();
            match new.as_slice() {
                [unit]
                    if facts
                        .failure
                        .boundary
                        .as_deref()
                        .is_some_and(|described| described.contains(unit.as_str())) =>
                {
                    (*unit).clone()
                }
                _ => {
                    let failure = facts.fail(
                        world.now(),
                        "not exactly one new verifier scope, the retained operation's own",
                    );
                    return Err(give_up(world, boundary, failure));
                }
            }
        }
        Err(error) => {
            let failure = facts.fail(world.now(), format!("the loaded scopes: {error}"));
            return Err(give_up(world, boundary, failure));
        }
    };
    facts.failure.unit = Some(unit.clone());
    // 5. Before the backstop: every retry unconfirmed, the scope loaded and
    // the helper unreaped. A retry starts only if its bound ends before the
    // backstop; one that still overran it may confirm the operation, but
    // only after the backstop.
    let mut unconfirmed = 0;
    while since(world) + plan.settle + MARGIN < plan.backstop {
        facts.failure.retries += 1;
        boundary = match world.retry(boundary) {
            Ok(()) => {
                facts.failure.confirmed = true;
                let at = since(world);
                if at + plan.early_tolerance < plan.backstop || unconfirmed == 0 {
                    return Err(facts.fail(
                        world.now(),
                        "the operation was confirmed before the runtime backstop: the \
                         never-populated scope did not stay loaded",
                    ));
                }
                return confirmed(world, &mut facts, plan, &before, unit, unconfirmed, at, 0);
            }
            Err(boundary) => boundary,
        };
        let at = since(world);
        match world.unreaped() {
            Ok(true) => {}
            outcome => {
                let failure = facts.fail(
                    world.now(),
                    format!(
                        "the helper was reaped before the operation was confirmed: {outcome:?}"
                    ),
                );
                return Err(give_up(world, boundary, failure));
            }
        }
        let deadline = world.now() + plan.observed_within;
        let loaded = world.scopes(deadline);
        facts.failure.scopes = Some(loaded.clone());
        match loaded {
            Ok(loaded) if loaded.contains(&unit) => {}
            _ => {
                let failure = facts.fail(world.now(), "the scope was gone before its backstop");
                return Err(give_up(world, boundary, failure));
            }
        }
        if at < plan.backstop {
            unconfirmed += 1;
        }
    }
    if unconfirmed == 0 {
        let failure = facts.fail(world.now(), "no retry ended before the backstop");
        return Err(give_up(world, boundary, failure));
    }
    // 6. Collected no earlier than the backstop, within a bound after it,
    // the helper still unreaped.
    let collected_at = loop {
        let deadline = world.now() + plan.observed_within;
        let loaded = world.scopes(deadline);
        facts.failure.scopes = Some(loaded.clone());
        let at = since(world);
        match loaded {
            Ok(loaded) if !loaded.contains(&unit) => break at,
            Ok(_) if at >= plan.backstop + plan.collected_within => {
                let failure = facts.fail(
                    world.now(),
                    "the scope was not collected within its bound after the backstop",
                );
                return Err(give_up(world, boundary, failure));
            }
            Ok(_) => world.pause(plan.poll),
            Err(error) => {
                let failure = facts.fail(world.now(), format!("the loaded scopes: {error}"));
                return Err(give_up(world, boundary, failure));
            }
        }
    };
    if collected_at + plan.early_tolerance < plan.backstop {
        let failure = facts.fail(world.now(), "the scope was collected before its backstop");
        return Err(give_up(world, boundary, failure));
    }
    match world.unreaped() {
        Ok(true) => {}
        outcome => {
            let failure = facts.fail(
                world.now(),
                format!("the helper was reaped before the operation was confirmed: {outcome:?}"),
            );
            return Err(give_up(world, boundary, failure));
        }
    }
    // 7. A bounded explicit retry confirms it.
    for attempt in 1..=plan.attempts {
        facts.failure.retries += 1;
        boundary = match world.retry(boundary) {
            Ok(()) => {
                facts.failure.confirmed = true;
                return confirmed(
                    world,
                    &mut facts,
                    plan,
                    &before,
                    unit,
                    unconfirmed,
                    collected_at,
                    attempt,
                );
            }
            Err(boundary) => boundary,
        };
    }
    let failure = facts.fail(
        world.now(),
        format!(
            "the operation is unconfirmed after {} explicit retries once its scope was collected",
            plan.attempts
        ),
    );
    Err(give_up(world, boundary, failure))
}

/// Once the operation is confirmed (on explicit retry `confirmed_on` after
/// the collection; 0: a retry begun before the backstop that confirmed it
/// after): the helper reaped only now, by production, and the loaded scopes
/// back to the baseline.
#[allow(clippy::too_many_arguments)]
fn confirmed<W: World>(
    world: &mut W,
    facts: &mut Facts,
    plan: &Plan,
    before: &Scopes,
    unit: String,
    unconfirmed: usize,
    collected_at: Duration,
    confirmed_on: usize,
) -> Result<Qualified, Box<Failure>> {
    match world.unreaped() {
        Ok(false) => {}
        outcome => {
            return Err(facts.fail(
                world.now(),
                format!("the helper was not reaped once the operation was confirmed: {outcome:?}"),
            ))
        }
    }
    // 8. The loaded scopes return to the baseline.
    let deadline = world.now() + plan.observed_within;
    loop {
        let loaded = world.scopes(deadline);
        facts.failure.scopes = Some(loaded.clone());
        match loaded {
            Ok(loaded) if loaded.len() <= before.len() && !loaded.contains(&unit) => break,
            Ok(_) if world.now() >= deadline => {
                return Err(facts.fail(
                    world.now(),
                    "the loaded scopes did not return to the baseline",
                ))
            }
            Ok(_) => world.pause(plan.poll),
            Err(error) => {
                return Err(facts.fail(world.now(), format!("the loaded scopes: {error}")))
            }
        }
    }
    Ok(Qualified {
        unit,
        error: facts.failure.error.clone().unwrap_or_default(),
        unconfirmed_before_backstop: unconfirmed,
        collected_at,
        confirmed_on,
    })
}
