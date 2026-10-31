//! The contract: the Owner's words verbatim, every bound checked, and a
//! hash that binds every field.

use super::*;
use crate::contract::{ContractError, MAX_ITEMS, MAX_OWNER_WORDS_CHARS};

#[test]
fn contract_the_hash_binds_every_field() {
    let base = contract();
    assert_eq!(base.hash(), base.clone().hash());
    assert_eq!(base.hash().to_string().len(), "sha256:".len() + 64);
    let mut variants: Vec<(&str, MissionContract)> = Vec::new();
    let mut v = |name, f: &dyn Fn(&mut MissionContract)| {
        let mut c = base.clone();
        f(&mut c);
        variants.push((name, c));
    };
    v("owner words", &|c| c.owner_words.push('!'));
    v("owner words' trailing space", &|c| c.owner_words.push(' '));
    v("restatement", &|c| c.restatement.push('!'));
    v("folder", &|c| {
        c.requested_folder = Some("~/Sites/other".into())
    });
    v("no folder", &|c| c.requested_folder = None);
    v("duration", &|c| c.budget.max_duration_ms += 1);
    v("model calls", &|c| c.budget.max_model_calls += 1);
    v("money", &|c| c.budget.max_money_minor = 1);
    v("forbidden", &|c| {
        c.forbidden_effects.insert(EffectKind::Process);
    });
    v("question", &|c| c.questions[0].text.push('?'));
    v("answer", &|c| c.questions[0].answer = Some("Red".into()));
    v("unanswered", &|c| c.questions[0].answer = None);
    v("question id", &|c| c.questions[0].id = "q2".into());
    v("criterion", &|c| c.pass_criteria[0].text.push('.'));
    v("criterion id", &|c| c.pass_criteria[0].id = "p2".into());
    v("criteria order", &|c| {
        c.pass_criteria.push(PassCriterion {
            id: "p0".into(),
            text: "Loads offline".into(),
        });
        c.pass_criteria.swap(0, 1);
    });
    let mut seen = BTreeSet::from([base.hash()]);
    for (name, c) in variants {
        assert!(seen.insert(c.hash()), "{name} did not change the hash");
    }
}

#[test]
fn contract_the_owner_s_words_are_kept_verbatim() {
    let w = world();
    let mut c = contract();
    c.owner_words = "  two  spaces,\ttab,\nnewline, ünïcödé, \u{202E}bidi, trailing   ".into();
    let id = w.mother.draft(c.clone()).unwrap();
    assert_eq!(w.mother.contract(id).unwrap().owner_words, c.owner_words);
    let w = w.restart();
    assert_eq!(w.mother.contract(id).unwrap().owner_words, c.owner_words);
    w.mother.submit(id).unwrap();
    assert_eq!(
        w.mother.confirmation(id).unwrap().owner_words,
        c.owner_words
    );
    assert_eq!(w.mother.contract(id).unwrap().hash(), c.hash());
}

#[test]
fn contract_every_bound_is_checked() {
    let check = |f: &dyn Fn(&mut MissionContract), expected: ContractError| {
        let mut c = contract();
        f(&mut c);
        assert_eq!(c.validate(), Err(expected.clone()), "{expected:?}");
        let w = world();
        assert_eq!(w.mother.draft(c), Err(CoordinatorError::Contract(expected)),);
        assert_eq!(w.mother.journal().unwrap(), vec![]);
    };
    check(
        &|c| c.owner_words = String::new(),
        ContractError::EmptyOwnerWords,
    );
    check(
        &|c| c.owner_words = " \n ".into(),
        ContractError::EmptyOwnerWords,
    );
    check(
        &|c| c.owner_words = "w".repeat(MAX_OWNER_WORDS_CHARS + 1),
        ContractError::OwnerWordsTooLong,
    );
    check(
        &|c| c.restatement = "  ".into(),
        ContractError::EmptyRestatement,
    );
    for hidden in [
        "line\nbreak",
        "bidi \u{202E}x",
        "zero\u{200B}width",
        "nbsp\u{00A0}",
    ] {
        check(
            &|c| c.restatement = hidden.into(),
            ContractError::RestatementNotPlain,
        );
    }
    check(
        &|c| c.requested_folder = Some(String::new()),
        ContractError::FolderNotPlain,
    );
    check(
        &|c| c.requested_folder = Some("a\u{0}b".into()),
        ContractError::FolderNotPlain,
    );
    check(&|c| c.pass_criteria.clear(), ContractError::NoPassCriteria);
    check(
        &|c| c.pass_criteria[0].text = "  ".into(),
        ContractError::ItemNotPlain,
    );
    check(
        &|c| c.questions[0].id = "q\n1".into(),
        ContractError::ItemNotPlain,
    );
    check(
        &|c| {
            let again = c.pass_criteria[0].clone();
            c.pass_criteria.push(again);
        },
        ContractError::DuplicateId,
    );
    check(
        &|c| {
            c.pass_criteria = (0..=MAX_ITEMS)
                .map(|i| PassCriterion {
                    id: format!("p{i}"),
                    text: "ok".into(),
                })
                .collect()
        },
        ContractError::TooManyItems,
    );
    check(
        &|c| c.budget.max_duration_ms = 0,
        ContractError::DurationOutOfRange,
    );
    check(
        &|c| c.budget.max_duration_ms = crate::contract::MAX_DURATION_MS + 1,
        ContractError::DurationOutOfRange,
    );
    check(
        &|c| c.budget.max_model_calls = 0,
        ContractError::ModelCallsOutOfRange,
    );
    check(
        &|c| c.budget.max_model_calls = crate::contract::MAX_MODEL_CALLS + 1,
        ContractError::ModelCallsOutOfRange,
    );
    check(
        &|c| c.budget.max_money_minor = 1,
        ContractError::MoneyNotZero,
    );
}

#[test]
fn contract_an_unanswered_question_keeps_the_mission_in_draft() {
    let w = world();
    let mut c = contract();
    c.questions[0].answer = None;
    let id = w.mother.draft(c.clone()).unwrap();
    assert_eq!(
        w.mother.submit(id),
        Err(CoordinatorError::Contract(
            ContractError::UnansweredQuestion { id: "q1".into() }
        ))
    );
    assert_eq!(w.mother.state(id), Some(MissionState::Draft));
    c.questions[0].answer = Some("Green".into());
    w.mother.revise(id, c).unwrap();
    assert_eq!(w.mother.submit(id), Ok(MissionState::AwaitingConsent));
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn contract_the_requested_folder_is_only_a_request() {
    // Any folder text is kept as the Owner's request; Mother never opens,
    // resolves or checks it on the host (the source scan pins that there
    // is no filesystem code outside the adapter).
    let w = world();
    for folder in [
        "~/Sites/rosas",
        "/etc",
        "../../somewhere",
        "C:\\Users\\rosa",
    ] {
        let mut c = contract();
        c.requested_folder = Some(folder.into());
        let id = w.mother.draft(c).unwrap();
        assert_eq!(
            w.mother.contract(id).unwrap().requested_folder.as_deref(),
            Some(folder)
        );
    }
    assert_eq!(w.port.calls(), vec![]);
}

#[test]
fn contract_the_coordinator_keeps_a_bounded_number_of_missions() {
    let w = world();
    for _ in 0..crate::coordinator::MAX_MISSIONS {
        w.mother.draft(contract()).unwrap();
    }
    assert_eq!(w.mother.draft(contract()), Err(CoordinatorError::Capacity));
}
