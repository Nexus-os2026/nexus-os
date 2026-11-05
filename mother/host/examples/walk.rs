//! Mother M1, M2a and M2c, walked through in plain language.
//!
//! Run 1 walks one sample mission through every state with deterministic
//! test lanes. Run 2 shows Stop and Deny refusing effects. Run 3 shows an
//! unclean exit: the mission comes back held, and only your yes resumes it.
//! Mother acts through the real governed-control broker, in a fresh home
//! under the system's temporary directory, and only through a door the
//! broker issues after your yes, each time she starts.
//!
//! This example uses the shared terminal's fresh code, complete display,
//! acknowledged input boundary and deadline for door, mission and held
//! resume dialogs. It declines all other confirmations.
//!
//! Anything else is a no. The broker records each answer in its evidence
//! store; Mother acts only on the broker's recorded yes.
//!
//! ```text
//! cargo run -p nexus-mother-host --example walk
//! ```

#[cfg(target_os = "linux")]
mod walk {
    use nexus_governed_control::authority::approval::{
        AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
        GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
    };
    use nexus_governed_control::authority::clock::SystemClock;
    use nexus_governed_control::authority::AuthorityError;
    use nexus_governed_control::egress::EgressIntent;
    use nexus_governed_control::governed::GovernedControl;
    use nexus_mother::adapter::{FileJournal, GovernedPort, MOTHER_AGENT};
    use nexus_mother::contract::{Budget, EffectKind, MissionContract, PassCriterion, Question};
    use nexus_mother::coordinator::{Advance, Coordinator, CoordinatorError, Lanes};
    use nexus_mother::effect::EffectRequest;
    use nexus_mother::journal::{JournalStore, MissionId};
    use nexus_mother::lane::{Lane, LaneRole, ScriptedLane, Verdict};
    use nexus_mother_host::Home;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    // Terminal::open disables OPOST. Every progress line owns its CRLF,
    // including errors after opening; escape data just like the renderer.
    macro_rules! progress {
        () => { progress!("") };
        ($($arg:tt)*) => {
            print!("{}\r\n", nexus_governed_control::authority::evidence::escaped(&format!($($arg)*)))
        };
    }

    /// A stand-in for the broker's native dialogs, at this terminal (the walk
    /// only): it asks the Owner about a door, a mission and a held mission, and
    /// declines everything else.
    struct OwnerAtTheTerminal(Arc<nexus_mother_host::legacy_terminal::WalkConfirmer>);

    impl ControlConfirmer for OwnerAtTheTerminal {
        fn confirm_action(&self, _: &ActionConfirmation) -> bool {
            false
        }
        fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
            false
        }
        fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
            false
        }
        fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
            false
        }
        fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
            self.0.confirm_mission(request)
        }
        fn confirm_agent_door(&self, request: &AgentDoorConfirmation) -> bool {
            self.0.confirm_agent_door(request)
        }
        fn confirm_held_mission(&self, request: &MissionResumeConfirmation) -> bool {
            self.0.confirm_held_mission(request)
        }
    }

    fn lanes(builder_asks: Vec<EffectRequest>) -> Lanes {
        let lane = |role, text: &str, verdict| Arc::new(ScriptedLane::new(role, text, verdict));
        let all: Vec<Arc<dyn Lane>> = vec![
            lane(
                LaneRole::Sentinel,
                "Nothing unsafe in this step.",
                Verdict::Continue,
            ),
            lane(
                LaneRole::ResearcherA,
                "Bakery sites usually show the name, hours and a phone number.",
                Verdict::Continue,
            ),
            lane(
                LaneRole::ResearcherB,
                "Independently: name, opening hours, where to find it.",
                Verdict::Continue,
            ),
            lane(
                LaneRole::Skeptic,
                "Both researchers agree; no claim needs the web.",
                Verdict::Continue,
            ),
            Arc::new(
                ScriptedLane::new(
                    LaneRole::Builder,
                    "Drafted one page: name, hours, phone.",
                    Verdict::Continue,
                )
                .asking(builder_asks),
            ),
            lane(
                LaneRole::Verifier,
                "The page names the bakery: holds.",
                Verdict::Pass,
            ),
        ];
        Lanes::new(all).expect("one lane per role")
    }

    fn contract(restatement: &str) -> MissionContract {
        MissionContract {
            model: None,
            notes: vec![],
            deliverable: None,
            owner_words: "Make me a small website for my bakery, Rosa's. Keep it simple!".into(),
            restatement: restatement.into(),
            requested_folder: Some("~/Sites/rosas-bakery".into()),
            budget: Budget {
                max_duration_ms: 30 * 60_000,
                max_model_calls: 20,
                max_money_minor: 0,
            },
            forbidden_effects: BTreeSet::from([EffectKind::Spend, EffectKind::Process]),
            questions: vec![Question {
                id: "q1".into(),
                text: "Which colours do you like?".into(),
                answer: Some("Blue and cream".into()),
            }],
            pass_criteria: vec![PassCriterion {
                id: "p1".into(),
                text: "The page names the bakery".into(),
            }],
        }
    }

    /// A port for Mother: the broker asks the Owner for her door first. A no
    /// gives no port, and Mother cannot act.
    fn door_port(home: &Home) -> Option<Arc<GovernedPort>> {
        match home.door() {
            Ok(door) => Some(Arc::new(GovernedPort::new(door))),
            Err(AuthorityError::Declined) => {
                progress!("  You said no to the door: Mother cannot act, so this part stops here.");
                None
            }
            Err(e) => {
                progress!("  The broker issued no door ({e}).");
                None
            }
        }
    }

    /// Mother over `journal` and her private store (P0: the journal keeps only
    /// references and codes; the contracts are in `private/` in her home).
    fn open(
        home: &Home,
        journal: &FileJournal,
        port: &Arc<GovernedPort>,
        lanes: Lanes,
    ) -> Coordinator<GovernedPort> {
        let store = FileJournal::open(journal.path()).expect("journal");
        let private = home.private().expect("private store");
        match Coordinator::open(
            store,
            port.clone(),
            lanes,
            Arc::new(SystemClock::default()),
            private,
        ) {
            Ok(c) => c,
            Err(e) => panic!("Mother refused her journal: {e:?}"),
        }
    }

    /// An orderly shutdown: journals the clean close that lets the next start
    /// carry on every mission. Without it (a crash), the next start holds every
    /// unfinished mission until the Owner resumes or stops it.
    fn shut_down(mother: Coordinator<GovernedPort>) {
        if let Err(e) = mother.close() {
            progress!("  Mother could not shut down cleanly ({e:?}): her next start holds every unfinished mission.");
        }
    }

    fn say_advance(a: &Advance) {
        for lane in &a.lanes {
            progress!(
                "    {} [{} {} {}] in {}… out {}…: {}",
                lane.identity.role.as_str(),
                lane.identity.name,
                lane.identity.version,
                lane.invocation,
                &lane.input_hash[..8],
                &lane.output_hash[..8],
                lane.text
            );
        }
        for effect in &a.effects {
            progress!(
                "    effect {} {}: {}",
                effect.kind.as_str(),
                if effect.dispatched {
                    "sent to the broker"
                } else {
                    "refused by Mother"
                },
                effect.outcome
            );
        }
        progress!("  {} → {}", a.from.as_str(), a.to.as_str());
    }

    /// Read on the Owner's side of the broker, never through Mother's port.
    fn mothers_runs(owner: &GovernedControl) -> usize {
        owner
            .authority()
            .runs()
            .views()
            .iter()
            .filter(|run| run.agent.to_string() == MOTHER_AGENT)
            .count()
    }

    fn draft_and_ask(mother: &Coordinator<GovernedPort>, c: MissionContract) -> (MissionId, bool) {
        let id = mother.draft(c).expect("a valid contract");
        progress!("  {id}: {}", mother.state(id).unwrap().as_str());
        mother.submit(id).expect("every question answered");
        progress!("  {id}: {}", mother.state(id).unwrap().as_str());
        let consented = mother.request_consent(id).is_ok();
        progress!("  {id}: {}", mother.state(id).unwrap().as_str());
        (id, consented)
    }

    /// Advance until the mission ends, printing each step and Mother's
    /// explanation.
    fn finish(mother: &Coordinator<GovernedPort>, id: MissionId) {
        while !mother.state(id).unwrap().is_terminal() {
            let a = mother.advance(id).expect("the mission moves");
            say_advance(&a);
            if let Some(text) = &a.explanation {
                progress!();
                for line in text.lines() {
                    progress!("  Mother: {line}");
                }
            }
        }
    }

    pub fn run() {
        let terminal = match nexus_mother_host::legacy_terminal::Terminal::open() {
            Ok(terminal) => terminal,
            Err(error) => {
                eprintln!("{}", error.code());
                return;
            }
        };
        let home = match Home::fresh_temp(
            "walk",
            Arc::new(OwnerAtTheTerminal(terminal.confirmer.clone())),
        ) {
            Ok(home) => home,
            Err(e) => {
                progress!("Could not open Mother's home: {e:?}");
                return;
            }
        };
        if !terminal.bind_stop(home.owner().clone()) {
            return;
        }
        let journal = home.journal().expect("journal");
        progress!("Mother's home for this walk: {}", home.dir().display());
        progress!("No grants are given in this walk, so the broker allows no effect at all.");
        progress!("Every time Mother starts, the broker asks you for her door.");

        // ------------------------------------------------------------ run 1
        progress!();
        progress!("RUN 1 — one mission, every state, test lanes (no model is called)");
        let Some(port1) = door_port(&home) else {
            return;
        };
        let mother = open(&home, &journal, &port1, lanes(vec![]));
        let (id, consented) = draft_and_ask(
            &mother,
            contract("A one-page site for Rosa's bakery: name, hours and phone."),
        );
        if consented {
            finish(&mother, id);
        } else {
            progress!("  You said no, so nothing ran.");
        }
        shut_down(mother);
        progress!();
        progress!("  Mother restarts from her journal: a new door, so the broker asks again.");
        let Some(port) = door_port(&home) else {
            return;
        };
        let restarted = open(&home, &journal, &port, lanes(vec![]));
        progress!("  {id} is {}.", restarted.state(id).unwrap().as_str());
        progress!(
            "  Effects sent to the broker in run 1: {}. Broker runs Mother opened: {}.",
            port1.performed(),
            mothers_runs(home.owner())
        );
        shut_down(restarted);

        // ------------------------------------------------------------ run 2
        progress!();
        progress!("RUN 2 — Stop and Deny refuse effects");
        let asks = vec![
            EffectRequest::Network(EgressIntent {
                method: "GET".into(),
                url: "https://example.org/".into(),
                headers: vec![],
                body: None,
            }),
            EffectRequest::File {
                relative_path: "index.html".into(),
                write: true,
            },
        ];
        // Run 2 has its own journal (its lanes differ), so its missions are
        // numbered from 1 again.
        let run2 = FileJournal::open(&home.dir().join("run2-journal.jsonl")).expect("journal");
        let Some(port2) = door_port(&home) else {
            return;
        };
        let mother = open(&home, &run2, &port2, lanes(asks.clone()));
        let runs_before = mothers_runs(home.owner());

        progress!();
        progress!("  Stop: consent to this one; Mother stops it before its Builder may fetch a page and write a file.");
        let (stopped, consented) = draft_and_ask(
            &mother,
            contract("The same site, and fetch an example page to copy its layout."),
        );
        if consented {
            // Approved → Investigating, then the Investigating lanes run; the
            // Builder (who would ask for the fetch and the file) is next.
            for _ in 0..2 {
                say_advance(&mother.advance(stopped).expect("the mission moves"));
            }
        }
        progress!("  Stop → {}", mother.stop(stopped).unwrap().as_str());
        match mother.advance(stopped) {
            Err(CoordinatorError::Stopped) | Err(CoordinatorError::Illegal(_)) => {
                progress!("  Asked to carry on: refused. Nothing new is dispatched.")
            }
            other => progress!("  Unexpected: {other:?}"),
        }

        progress!();
        progress!("  Deny: this time answer no (type anything but the code).");
        let (denied, consented) = draft_and_ask(
            &mother,
            contract("The same site, published to the web for everyone."),
        );
        if consented {
            progress!("  You consented, so this part cannot show a denial; stopping it.");
            mother.stop(denied).unwrap();
        } else {
            match mother.advance(denied) {
                Err(e) => progress!("  Asked to carry on: refused ({e:?})."),
                Ok(_) => progress!("  Unexpected: a denied mission moved"),
            }
        }
        shut_down(mother);

        progress!();
        progress!("  Mother restarts: a new door, so the broker asks again.");
        let Some(port) = door_port(&home) else {
            return;
        };
        let restarted = open(&home, &run2, &port, lanes(asks));
        progress!(
            "  After the restart: {stopped} is {}, {denied} is {}.",
            restarted.state(stopped).unwrap().as_str(),
            restarted.state(denied).unwrap().as_str()
        );
        progress!(
            "  Effects sent to the broker in run 2: {}. Broker runs Mother opened in run 2: {}.",
            port2.performed(),
            mothers_runs(home.owner()) - runs_before
        );
        shut_down(restarted);

        // ------------------------------------------------------------ run 3
        progress!();
        progress!("RUN 3 — an unclean exit, then resume");
        let run3 = FileJournal::open(&home.dir().join("run3-journal.jsonl")).expect("journal");
        let Some(port3) = door_port(&home) else {
            return;
        };
        let mother = open(&home, &run3, &port3, lanes(vec![]));
        let (held, consented) = draft_and_ask(
            &mother,
            contract("A one-page site for Rosa's bakery, with the weekly specials."),
        );
        if !consented {
            progress!("  You said no, so there is nothing to hold.");
            return;
        }
        say_advance(&mother.advance(held).expect("the mission moves"));
        progress!("  Mother exits without shutting down (as if the power went out).");
        drop(mother);
        drop(port3);
        progress!();
        progress!("  Mother restarts: a new door, so the broker asks again.");
        let Some(port) = door_port(&home) else {
            return;
        };
        let mother = open(&home, &run3, &port, lanes(vec![]));
        match mother.held(held) {
            Some(reason) => progress!(
                "  {held} is {} and held ({reason}): it does nothing until you resume or stop it.",
                mother.state(held).unwrap().as_str()
            ),
            None => progress!("  Unexpected: {held} is not held."),
        }
        match mother.resume(held) {
            Ok(state) => {
                progress!("  You said yes: {held} resumes from {}.", state.as_str());
                finish(&mother, held);
            }
            Err(CoordinatorError::Declined) => {
                progress!("  You said no: {held} stays held. Nothing of it runs.")
            }
            Err(e) => progress!("  The resume was refused ({e:?}): {held} stays held."),
        }
        shut_down(mother);

        let entries: usize = [&journal, &run2, &run3]
            .iter()
            .map(|j| j.load().map(|l| l.len()).unwrap_or(0))
            .sum();
        progress!();
        progress!(
            "  Journals (hash-chained, verified on every open): {}, {} and {} ({entries} entries).",
            journal.path().display(),
            run2.path().display(),
            run3.path().display()
        );
    }
}

#[cfg(target_os = "linux")]
fn main() {
    walk::run();
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("hands_unavailable: unsupported_platform");
}
