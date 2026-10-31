//! Mother M1 and M2a, walked through in plain language.
//!
//! Run 1 walks one sample mission through every state with deterministic
//! test lanes. Run 2 shows Stop and Deny refusing effects. Mother acts
//! through the real governed-control broker, in a fresh home under the
//! system's temporary directory. You consent through the broker's mission
//! dialog, which this example stands in for at the terminal: you type the
//! first 8 characters of the contract's hash; anything else is a no. The
//! broker records your answer in its evidence store, and Mother's consent
//! is made only from the broker's recorded yes.
//!
//! ```text
//! cargo run -p nexus-mother --example walk
//! ```

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    MissionConfirmation, ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::egress::EgressIntent;
use nexus_mother::adapter::{FileJournal, GovernedPort, Home, MOTHER_AGENT};
use nexus_mother::contract::{Budget, EffectKind, MissionContract, PassCriterion, Question};
use nexus_mother::coordinator::{Advance, Coordinator, CoordinatorError, Lanes};
use nexus_mother::effect::EffectRequest;
use nexus_mother::journal::{JournalStore, MissionId};
use nexus_mother::lane::{Lane, LaneRole, ScriptedLane, Verdict};
use std::collections::BTreeSet;
use std::io::Write;
use std::sync::Arc;

/// A stand-in for the broker's native dialogs, at this terminal (the walk
/// only): it asks the Owner about a mission and declines everything else.
struct OwnerAtTheTerminal;

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
        show_confirmation(request);
        let short = request.contract_hash.get(..8).unwrap_or_default();
        print!(
            "  To consent, type the first 8 characters of the hash ({}…); anything else means no: ",
            request.contract_hash.get(..4).unwrap_or_default()
        );
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return false;
        }
        short.len() == 8 && line.trim() == short
    }
}

/// The broker's mission dialog, as text.
fn show_confirmation(r: &MissionConfirmation) {
    let text = r.text();
    println!();
    println!("  ┌ {} (the broker asks)", r.title());
    for (label, value) in &text.header {
        println!("  │ {label}: {value}");
    }
    for line in &text.details {
        println!("  │   {line}");
    }
    println!("  └");
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

fn open(
    journal: &FileJournal,
    port: &Arc<GovernedPort>,
    lanes: Lanes,
) -> Coordinator<GovernedPort> {
    let store = FileJournal::open(journal.path()).expect("journal");
    match Coordinator::open(store, port.clone(), lanes, Arc::new(SystemClock::default())) {
        Ok(c) => c,
        Err(e) => panic!("Mother refused her journal: {e:?}"),
    }
}

/// An orderly shutdown: journals the clean close that lets the next start
/// carry on every mission. Without it (a crash), the next start holds every
/// unfinished mission until the Owner resumes or stops it.
fn shut_down(mother: Coordinator<GovernedPort>) {
    if let Err(e) = mother.close() {
        println!("  Mother could not shut down cleanly ({e:?}): her next start holds every unfinished mission.");
    }
}

fn say_advance(a: &Advance) {
    for lane in &a.lanes {
        println!(
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
        println!(
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
    println!("  {} → {}", a.from.as_str(), a.to.as_str());
}

fn mothers_runs(port: &GovernedPort) -> usize {
    port.control()
        .authority()
        .runs()
        .views()
        .iter()
        .filter(|run| run.agent.to_string() == MOTHER_AGENT)
        .count()
}

fn draft_and_ask(mother: &Coordinator<GovernedPort>, c: MissionContract) -> (MissionId, bool) {
    let id = mother.draft(c).expect("a valid contract");
    println!("  {id}: {}", mother.state(id).unwrap().as_str());
    mother.submit(id).expect("every question answered");
    println!("  {id}: {}", mother.state(id).unwrap().as_str());
    let consented = mother.request_consent(id, &OwnerAtTheTerminal).is_ok();
    println!("  {id}: {}", mother.state(id).unwrap().as_str());
    (id, consented)
}

fn main() {
    let home = match Home::fresh_temp("walk") {
        Ok(home) => home,
        Err(e) => {
            eprintln!("Could not open Mother's home: {e:?}");
            return;
        }
    };
    let port = Arc::new(GovernedPort::new(home.control.clone()));
    let journal = home.journal().expect("journal");
    println!("Mother's home for this walk: {}", home.dir.display());
    println!("No grants are given in this walk, so the broker allows no effect at all.");

    // ------------------------------------------------------------ run 1
    println!();
    println!("RUN 1 — one mission, every state, test lanes (no model is called)");
    let mother = open(&journal, &port, lanes(vec![]));
    let (id, consented) = draft_and_ask(
        &mother,
        contract("A one-page site for Rosa's bakery: name, hours and phone."),
    );
    if consented {
        while !mother.state(id).unwrap().is_terminal() {
            let a = mother.advance(id).expect("the mission moves");
            say_advance(&a);
            if let Some(text) = &a.explanation {
                println!();
                for line in text.lines() {
                    println!("  Mother: {line}");
                }
            }
        }
    } else {
        println!("  You said no, so nothing ran.");
    }
    shut_down(mother);
    let restarted = open(&journal, &port, lanes(vec![]));
    println!(
        "  Mother restarted from her journal: {id} is {}.",
        restarted.state(id).unwrap().as_str()
    );
    println!(
        "  Effects sent to the broker in run 1: {}. Broker runs Mother opened: {}.",
        port.performed(),
        mothers_runs(&port)
    );
    shut_down(restarted);

    // ------------------------------------------------------------ run 2
    println!();
    println!("RUN 2 — Stop and Deny refuse effects");
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
    let run2 = FileJournal::open(&home.dir.join("run2-journal.jsonl")).expect("journal");
    let mother = open(&run2, &port, lanes(asks.clone()));
    let before = (port.performed(), mothers_runs(&port));

    println!();
    println!("  Stop: consent to this one; Mother stops it before its Builder may fetch a page and write a file.");
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
    println!("  Stop → {}", mother.stop(stopped).unwrap().as_str());
    match mother.advance(stopped) {
        Err(CoordinatorError::Stopped) | Err(CoordinatorError::Illegal(_)) => {
            println!("  Asked to carry on: refused. Nothing new is dispatched.")
        }
        other => println!("  Unexpected: {other:?}"),
    }

    println!();
    println!("  Deny: this time answer no (type anything but the code).");
    let (denied, consented) = draft_and_ask(
        &mother,
        contract("The same site, published to the web for everyone."),
    );
    if consented {
        println!("  You consented, so this part cannot show a denial; stopping it.");
        mother.stop(denied).unwrap();
    } else {
        match mother.advance(denied) {
            Err(e) => println!("  Asked to carry on: refused ({e:?})."),
            Ok(_) => println!("  Unexpected: a denied mission moved"),
        }
    }
    shut_down(mother);

    let restarted = open(&run2, &port, lanes(asks));
    println!();
    println!(
        "  After a restart: {stopped} is {}, {denied} is {}.",
        restarted.state(stopped).unwrap().as_str(),
        restarted.state(denied).unwrap().as_str()
    );
    println!(
        "  Effects sent to the broker in run 2: {}. Broker runs Mother opened in run 2: {}.",
        port.performed() - before.0,
        mothers_runs(&port) - before.1
    );
    let entries = run2.load().map(|l| l.len()).unwrap_or(0);
    println!(
        "  Journals (hash-chained, verified on every open): {} and {} ({entries} entries).",
        journal.path().display(),
        run2.path().display()
    );
}
