//! P2-V1-R3B-I4-Q3-R1 fixture controls for live case 18's qualification of
//! the never-populated scope lifecycle (`support/never_populated.rs`).
//!
//! These are not live evidence. No user manager, bus or cgroup is reached:
//! [`qualify`] runs over a model of the lifecycle live run 37163435032
//! showed (a refused placement whose never-populated scope stays loaded until
//! its runtime backstop, then is collected), with virtual time, and over
//! deviations from it that the case must report. The helper's exit
//! observation is proved on real child processes of this test (killed,
//! observed, then reaped here); and the live case's own source is checked to
//! drive exactly that procedure through production's placement and retry.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/never_populated.rs"]
mod never_populated;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod controls {
    use super::never_populated::*;
    use std::time::Duration;

    use nexus_verifier_sandbox::policy::ResourcePolicy;

    /// The placement the model's production makes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Placing {
        /// What live run 37163435032 showed: no scope proven, the operation
        /// retained, its never-populated scope loaded until the backstop.
        Refused,
        /// A scope proven for the helper.
        Proven,
        /// No scope proven and the cleanup confirmed at once.
        ConfirmedAtOnce,
    }

    /// A model of the case's world, in virtual time.
    struct Model {
        clock: Duration,
        /// Virtual time past which a wait is unbounded: the case is stopped.
        cap: Duration,
        killed_at: Option<Duration>,
        /// How long after its kill the helper exits (`None`: never).
        exits_after: Option<Duration>,
        exit_reported: bool,
        reaped: bool,
        placing: Placing,
        /// How long production's placement takes (its wait, then its
        /// settling).
        place_takes: Duration,
        settle: Duration,
        unit: &'static str,
        loaded: bool,
        /// When the manager collects the scope, after the placement began
        /// (`None`: never).
        collected_after: Option<Duration>,
        collect_at: Option<Duration>,
        /// Something reaps the helper during the first retry.
        reaps_on_first_retry: bool,
        /// Retries never confirm, even once the scope is collected.
        never_confirms: bool,
        /// Another verifier scope remains once the operation is confirmed.
        residue: bool,
        confirmed: bool,
        /// Observations fail.
        unobservable: bool,
        baseline: Scopes,
        retries: usize,
        released: Vec<String>,
    }

    const UNIT: &str = "nexus-verifier-0123456789abcdef0123456789abcdef.scope";
    const RESIDUE: &str = "nexus-verifier-residue.scope";
    const GC: Duration = Duration::from_millis(100);

    impl Model {
        /// The real lifecycle: the helper exits at once, the placement is
        /// refused after its wait and its settling, and the manager collects
        /// the scope just after its backstop.
        fn real() -> Self {
            Self {
                clock: Duration::ZERO,
                cap: Duration::from_secs(600),
                killed_at: None,
                exits_after: Some(Duration::from_millis(30)),
                exit_reported: false,
                reaped: false,
                placing: Placing::Refused,
                place_takes: Duration::from_secs(20),
                settle: Duration::from_secs(10),
                unit: UNIT,
                loaded: false,
                collected_after: Some(Duration::from_secs(BACKSTOP_SECS) + GC),
                collect_at: None,
                reaps_on_first_retry: false,
                never_confirms: false,
                residue: false,
                confirmed: false,
                unobservable: false,
                baseline: Scopes::new(),
                retries: 0,
                released: Vec::new(),
            }
        }

        fn plan() -> Plan {
            Plan::new(Duration::from_secs(10), Duration::from_secs(10))
        }

        fn exited(&self) -> bool {
            matches!((self.killed_at, self.exits_after), (Some(at), Some(after)) if self.clock >= at + after)
        }

        /// The manager collects the scope once its time has come.
        fn tick(&mut self) {
            if self.collect_at.is_some_and(|at| self.clock >= at) {
                self.loaded = false;
            }
        }

        fn advance(&mut self, by: Duration) {
            self.clock += by;
            assert!(
                self.clock <= self.cap,
                "an unbounded wait: the case's virtual time passed {:?}",
                self.cap
            );
            self.tick();
        }
    }

    #[derive(Debug)]
    struct Boundary(&'static str);

    impl World for Model {
        type Boundary = Boundary;
        type Placed = &'static str;

        fn now(&self) -> Duration {
            self.clock
        }

        fn pause(&mut self, interval: Duration) {
            self.advance(interval);
        }

        fn kill(&mut self) -> Result<(), String> {
            self.killed_at = Some(self.clock);
            Ok(())
        }

        fn exit(&mut self) -> Result<Option<Exit>, String> {
            if self.reaped {
                return Err("no such child (ECHILD)".into());
            }
            if !self.exited() {
                return Ok(None);
            }
            self.exit_reported = true;
            Ok(Some(Exit {
                code: libc::CLD_KILLED,
                status: libc::SIGKILL,
            }))
        }

        fn unreaped(&mut self) -> Result<bool, String> {
            Ok(!self.reaped)
        }

        fn place(&mut self) -> Result<Self::Placed, Refused<Self::Boundary>> {
            // A process still running is moved, and its scope proven; so is
            // one whose exit was never reported, however long the case slept.
            if !self.exited() || !self.exit_reported {
                return Ok("placed before the helper's exit was reported");
            }
            match self.placing {
                Placing::Proven => Ok("a proven scope"),
                Placing::ConfirmedAtOnce => {
                    self.advance(self.place_takes);
                    self.reaped = true;
                    Err(Refused {
                        error: "Some(NotPlaced)".into(),
                        retained: None,
                    })
                }
                Placing::Refused => {
                    let started = self.clock;
                    self.loaded = true;
                    self.collect_at = self.collected_after.map(|after| started + after);
                    self.advance(self.place_takes);
                    Err(Refused {
                        error: "Some(NotPlaced)".into(),
                        retained: Some(Boundary(self.unit)),
                    })
                }
            }
        }

        fn retry(&mut self, boundary: Boundary) -> Result<(), Boundary> {
            self.retries += 1;
            if self.reaps_on_first_retry && self.retries == 1 {
                self.reaped = true;
            }
            self.tick();
            if !self.loaded && !self.never_confirms {
                self.advance(Duration::from_millis(5));
                self.reaped = true;
                self.confirmed = true;
                return Ok(());
            }
            // Production observes until its settling bound: a collection
            // within it confirms the operation then.
            if let Some(at) = self.collect_at.filter(|at| *at <= self.clock + self.settle) {
                if !self.never_confirms {
                    self.advance(at.saturating_sub(self.clock) + Duration::from_millis(5));
                    self.reaped = true;
                    self.confirmed = true;
                    return Ok(());
                }
            }
            self.advance(self.settle);
            Err(boundary)
        }

        fn describe(&self, boundary: &Boundary) -> String {
            format!(
                "RetainedBoundary {{ scope: Pending(PendingScope {{ unit: {:?} }}) }}",
                boundary.0
            )
        }

        fn scopes(&mut self, _deadline: Duration) -> Result<Scopes, String> {
            self.advance(Duration::from_millis(2));
            if self.unobservable {
                return Err("the observation timed out".into());
            }
            let mut scopes = self.baseline.clone();
            if self.loaded {
                scopes.insert(self.unit.to_string());
            }
            if self.residue && self.confirmed {
                scopes.insert(RESIDUE.to_string());
            }
            Ok(scopes)
        }

        fn release(&mut self, _boundary: Boundary, report: &str) {
            self.released.push(report.to_string());
        }
    }

    fn run(model: &mut Model) -> Result<Qualified, Box<Failure>> {
        qualify(model, &Model::plan())
    }

    fn failed(model: &mut Model) -> Box<Failure> {
        match run(model) {
            Ok(qualified) => {
                panic!("qualified a world that deviates from the lifecycle: {qualified:?}")
            }
            Err(failure) => failure,
        }
    }

    #[test]
    fn np_01_the_real_lifecycle_qualifies() {
        let mut model = Model::real();
        let qualified = run(&mut model).unwrap_or_else(|failure| panic!("{failure}"));
        assert_eq!(qualified.unit, UNIT);
        assert_eq!(qualified.error, "Some(NotPlaced)");
        assert!(qualified.unconfirmed_before_backstop >= 1, "{qualified:?}");
        assert!(
            qualified.collected_at >= Duration::from_secs(BACKSTOP_SECS),
            "{qualified:?}"
        );
        assert_eq!(qualified.confirmed_on, 1);
        assert!(model.reaped && model.confirmed && model.released.is_empty());
    }

    #[test]
    fn np_02_a_capture_failure_s_faster_refusal_qualifies_too() {
        // An accepted start whose identity was not captured is refused after
        // its settling only: more retries end before the backstop.
        let mut model = Model::real();
        model.place_takes = Duration::from_secs(10);
        let qualified = run(&mut model).unwrap_or_else(|failure| panic!("{failure}"));
        assert!(qualified.unconfirmed_before_backstop >= 2, "{qualified:?}");
    }

    #[test]
    fn np_03_a_confirmation_before_the_backstop_is_a_failure() {
        // The manager collects the scope during the first retry, long before
        // its backstop: the scope did not stay loaded.
        let mut model = Model::real();
        model.collected_after = Some(Duration::from_secs(25));
        let failure = failed(&mut model);
        assert!(
            failure
                .what
                .contains("confirmed before the runtime backstop"),
            "a confirmation before the backstop was taken as the lifecycle: {failure}"
        );
        assert!(failure.confirmed);
    }

    #[test]
    fn np_04_a_scope_collected_before_its_backstop_is_a_failure() {
        // Collected between the last retry before the backstop and the
        // backstop itself.
        let mut model = Model::real();
        model.collected_after = Some(Duration::from_secs(33));
        let failure = failed(&mut model);
        assert!(
            failure.what.contains("collected before its backstop"),
            "a collection before the backstop was taken as the lifecycle: {failure}"
        );
        assert_eq!(
            model.released.len(),
            1,
            "the retained boundary was not released"
        );
    }

    #[test]
    fn np_05_a_proven_placement_is_a_failure() {
        let mut model = Model::real();
        model.placing = Placing::Proven;
        let failure = failed(&mut model);
        assert!(
            failure
                .what
                .contains("a scope was proven for an unmovable process"),
            "a proven placement was accepted: {failure}"
        );
    }

    #[test]
    fn np_06_a_cleanup_confirmed_at_once_is_a_failure() {
        let mut model = Model::real();
        model.placing = Placing::ConfirmedAtOnce;
        let failure = failed(&mut model);
        assert!(failure.what.contains("confirmed at once"), "{failure}");
        assert!(failure.confirmed && !failure.retained);
    }

    #[test]
    fn np_07_a_helper_reaped_before_the_confirmation_is_a_failure() {
        let mut model = Model::real();
        model.reaps_on_first_retry = true;
        let failure = failed(&mut model);
        assert!(
            failure
                .what
                .contains("reaped before the operation was confirmed"),
            "a helper reaped before the confirmation was accepted: {failure}"
        );
        assert_eq!(model.released.len(), 1);
    }

    #[test]
    fn np_08_a_helper_that_never_exits_fails_within_its_bound() {
        let mut model = Model::real();
        model.exits_after = None;
        let failure = failed(&mut model);
        assert!(
            failure.what.contains("did not exit within its bound"),
            "{failure}"
        );
        assert!(
            model.clock <= EXIT_WITHIN + Duration::from_secs(1),
            "the exit was waited for beyond its bound: {:?}",
            model.clock
        );
    }

    #[test]
    fn np_09_a_late_exit_is_waited_for_by_its_own_report() {
        // The helper exits nine seconds after its kill: only its exit report
        // proves it, and the placement comes only after it.
        let mut model = Model::real();
        model.exits_after = Some(Duration::from_secs(9));
        let qualified = run(&mut model).unwrap_or_else(|failure| {
            panic!("the exit was not waited for by its report: {failure}")
        });
        assert_eq!(qualified.confirmed_on, 1);
    }

    #[test]
    fn np_10_a_scope_never_collected_fails_after_its_bound() {
        let mut model = Model::real();
        model.collected_after = None;
        let failure = failed(&mut model);
        assert!(
            failure.what.contains("not collected within its bound"),
            "{failure}"
        );
        assert!(
            model.clock
                <= Duration::from_secs(BACKSTOP_SECS) + COLLECTED_WITHIN + Duration::from_secs(1)
        );
        assert_eq!(model.released.len(), 1);
    }

    #[test]
    fn np_11_scopes_not_back_to_the_baseline_are_a_failure() {
        let mut model = Model::real();
        model.residue = true;
        let failure = failed(&mut model);
        assert!(
            failure.what.contains("did not return to the baseline"),
            "a scope left after the confirmation was not detected: {failure}"
        );
    }

    #[test]
    fn np_12_unconfirmed_after_the_collection_fails_and_releases() {
        let mut model = Model::real();
        model.never_confirms = true;
        let failure = failed(&mut model);
        assert!(
            failure
                .what
                .contains("unconfirmed after 3 explicit retries"),
            "{failure}"
        );
        assert!(!failure.confirmed && failure.retained);
        assert_eq!(model.released.len(), 1);
        assert!(!model.reaped);
    }

    #[test]
    fn np_13_an_unobservable_manager_is_never_no_scopes() {
        let mut model = Model::real();
        model.unobservable = true;
        let failure = failed(&mut model);
        assert!(failure.what.contains("the loaded scopes"), "{failure}");
    }

    #[test]
    fn np_14_a_failure_reports_what_was_observed() {
        let mut model = Model::real();
        model.collected_after = None;
        let failure = failed(&mut model).to_string();
        for needle in [
            UNIT,
            "Some(NotPlaced)",
            "boundary retained true",
            "retries 1",
            "elapsed Some(",
            "cleanup confirmed false",
            "loaded scopes Some(Ok(",
        ] {
            assert!(failure.contains(needle), "{needle} not in: {failure}");
        }
    }

    #[test]
    fn np_15_the_case_s_backstop_is_its_own_and_leaves_room_for_a_retry() {
        let base = ResourcePolicy::RUST_OFFLINE_V1;
        let case = limits(base);
        assert_eq!(
            case.runtime_backstop_secs, BACKSTOP_SECS,
            "the case does not set its own runtime backstop"
        );
        assert_ne!(
            case.runtime_backstop_secs, base.runtime_backstop_secs,
            "the case's runtime backstop is the production default"
        );
        assert_eq!(
            ResourcePolicy {
                runtime_backstop_secs: base.runtime_backstop_secs,
                ..case
            },
            base,
            "the case changed more than its runtime backstop"
        );
        let live = Plan::live();
        assert!(live.coherent(), "{live:?}");
        assert_eq!(live.backstop, Duration::from_secs(BACKSTOP_SECS));
        assert_eq!(
            live.placement,
            nexus_verifier_sandbox::scope::PLACEMENT_TIMEOUT
        );
        assert_eq!(live.settle, nexus_verifier_sandbox::scope::SETTLE_TIMEOUT);
        // A backstop that leaves no room for the placement and a retry is
        // refused before anything is done.
        let mut tight = Model::plan();
        tight.backstop = tight.placement + tight.settle + tight.settle;
        let mut model = Model::real();
        let failure = qualify(&mut model, &tight).unwrap_err();
        assert!(failure.what.contains("leaves no room"), "{failure}");
        assert!(model.killed_at.is_none());
    }

    /// A real child of this test that only waits on its stdin: its process
    /// id, and the child (reaped here).
    fn child() -> std::process::Child {
        std::process::Command::new("/bin/cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }

    fn state(pid: u32) -> Option<char> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat.rsplit_once(") ")?.1.chars().next()
    }

    #[test]
    fn np_16_the_exit_observation_never_reaps() {
        let mut child = child();
        let pid = child.id();
        child.kill().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let exit = loop {
            if let Some(exit) = exit_unreaped(pid).unwrap() {
                break exit;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the killed child did not exit"
            );
            std::thread::sleep(Duration::from_millis(1));
        };
        assert!(exit.killed(), "{exit:?}");
        assert!(
            unreaped(pid).unwrap(),
            "the exit observation reaped the helper"
        );
        assert_eq!(state(pid), Some('Z'), "the observed child is not a zombie");
        assert_eq!(
            exit_unreaped(pid).unwrap(),
            Some(exit),
            "observed twice, the same exit"
        );
        // Reaped here, only now: no longer this process's child.
        child.wait().unwrap();
        assert!(!unreaped(pid).unwrap(), "a reaped child reported unreaped");
        assert_eq!(
            exit_unreaped(pid).unwrap_err().raw_os_error(),
            Some(libc::ECHILD)
        );
    }

    #[test]
    fn np_17_the_exit_observation_never_blocks_on_a_running_helper() {
        let mut child = child();
        let pid = child.id();
        let started = std::time::Instant::now();
        assert_eq!(
            exit_unreaped(pid).unwrap(),
            None,
            "a running child reported exited"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the exit observation blocked"
        );
        assert!(unreaped(pid).unwrap() && state(pid).is_some_and(|state| state != 'Z'));
        child.kill().unwrap();
        child.wait().unwrap();
    }

    /// Rust source without line comments.
    fn code(text: &str) -> String {
        text.lines()
            .map(|line| line.find("//").map_or(line, |at| &line[..at]))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The item of `source` that starts at `head`, up to its closing brace at
    /// the head's own indentation.
    fn item<'a>(source: &'a str, head: &str) -> &'a str {
        let start = source
            .find(head)
            .unwrap_or_else(|| panic!("not found: {head}"));
        let indent = &head[..head.len() - head.trim_start().len()];
        let end = format!("\n{indent}}}\n");
        let rest = &source[start..];
        &rest[..rest.find(&end).map_or(rest.len(), |at| at + end.len())]
    }

    #[test]
    fn np_18_live_case_18_drives_exactly_this_qualification() {
        let harness = code(include_str!("phase2_live_sandbox.rs"));
        let support = code(include_str!("support/never_populated.rs"));
        assert_eq!(
            harness
                .matches("#[path = \"support/never_populated.rs\"]")
                .count(),
            1
        );
        // The case: its own backstop over the harness's limits, production's
        // own bounds, the qualification, and nothing else of its own.
        let case = item(&harness, "        pub fn unmovable_process(");
        for (needle, count) in [
            ("never_populated::limits(limits())", 1),
            ("never_populated::Plan::live()", 1),
            ("never_populated::qualify(&mut world, &plan)", 1),
            ("Helper::spawn(&HelperProgram::at(HELPER))", 1),
        ] {
            assert_eq!(case.matches(needle).count(), count, "{needle}");
        }
        for needle in [
            ".reap(",
            "try_reap",
            "waitpid",
            "waitid",
            "sleep",
            "kill(",
            "settle(",
            "retry",
            "loaded_scopes",
            "wait_for(",
            "runtime_backstop_secs",
        ] {
            assert!(!case.contains(needle), "the case does {needle} itself");
        }
        // Its world: production's placement and retry, the helper observed
        // through the support's own exit observation, the checked scope
        // observation, an explicit release; nothing reaps the helper.
        let world = item(
            &harness,
            "        impl never_populated::World for NeverPopulated<'_> {",
        );
        for needle in [
            "execution::place(self.scopes, helper, &self.limits)",
            "execution::RetainedBoundary::retry(boundary)",
            "never_populated::exit_unreaped(self.pid)",
            "never_populated::unreaped(self.pid)",
            "loaded_scopes_by(self.start + deadline)",
            "release(boundary, \"unmovable_process\", &[report.to_string()])",
            "Some(helper) => helper.kill()",
        ] {
            assert_eq!(world.matches(needle).count(), 1, "{needle}");
        }
        for needle in [
            ".reap(",
            "try_reap",
            "waitpid",
            "try_wait",
            ".wait(",
            "Cleanup::Confirmed => Some",
        ] {
            assert!(!world.contains(needle), "the case's world does {needle}");
        }
        // The support: one exit observation, which never reaps and never
        // blocks, and nothing that acts on a unit or a process otherwise.
        assert_eq!(support.matches("libc::waitid(").count(), 1);
        assert_eq!(
            support
                .matches("libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,")
                .count(),
            1
        );
        for needle in [
            "waitpid",
            "try_wait",
            ".wait(",
            "libc::kill",
            "Command",
            "StopUnit",
            "KillUnit",
            "systemctl",
            "thread::sleep",
            "Instant::now",
        ] {
            assert!(!support.contains(needle), "the support does {needle}");
        }
        // Every wait of the qualification is bounded, and a confirmation
        // before the backstop, a proven placement or scopes not back to the
        // baseline is a failure.
        let qualify = item(&support, "pub fn qualify<W: World>(");
        for needle in [
            "Ok(None) if world.now() >= deadline => {",
            "Ok(_) if at >= plan.backstop + plan.collected_within => {",
            "if at + plan.early_tolerance < plan.backstop || unconfirmed == 0 {",
            "\"a scope was proven for an unmovable process: {placed:?}\"",
            "if collected_at + plan.early_tolerance < plan.backstop {",
            "for attempt in 1..=plan.attempts {",
        ] {
            assert!(qualify.contains(needle), "{needle}");
        }
        let confirmed = item(&support, "fn confirmed<W: World>(");
        assert!(confirmed.contains(
            "Ok(loaded) if loaded.len() <= before.len() && !loaded.contains(&unit) => break,"
        ));
        assert!(confirmed.contains("Ok(_) if world.now() >= deadline => {"));
    }
}
