//! Phase One end-to-end: the owner's user story through the backend
//! authority path, with a scripted local model standing in for Ollama and
//! stand-ins for the native picker and confirmation.

use super::p1_apply::{env, Env};
use super::*;
use std::sync::Mutex as StdMutex;

struct Model {
    pin: ModelPin,
    answers: StdMutex<Vec<String>>,
}

impl LocalModel for Model {
    fn pin(&self) -> &ModelPin {
        &self.pin
    }
    fn complete(
        &self,
        _messages: &[ModelMessage],
        _timeout: Duration,
        _max: u64,
    ) -> Result<String, ModelError> {
        let mut answers = self.answers.lock().unwrap();
        Ok(if answers.is_empty() {
            json!({"action": "finish", "summary": "done"}).to_string()
        } else {
            answers.remove(0)
        })
    }
}

struct Yes;

impl OwnerConfirmer for Yes {
    fn confirm(&self, _request: &ConfirmationRequest) -> bool {
        true
    }
}

#[test]
fn p1_e2e_01_select_run_review_approve_apply_and_restore() {
    // 1. The owner selected the project natively (see env()).
    let Env { f, projects, info } = env();
    let before = digest(&f.project);

    // 2–4. Scope, task and local model; a fresh read grant for this run.
    let binding = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    };
    let grant = projects.grant_for_run(info.id, binding).unwrap();
    let mut run = CodingRun::create_for_project(
        Arc::clone(&f.ledger) as Arc<dyn LedgerStore>,
        grant,
        default_scopes(),
    )
    .unwrap();
    run.grant(&parent(&f)).unwrap();
    run.snapshot().unwrap();
    let model = Model {
        pin: local_model::pin_for_test("qwen2.5-coder:7b"),
        answers: StdMutex::new(vec![
            json!({"action": "read", "paths": ["src/lib.rs", "AGENTS.md"]}).to_string(),
            json!({"action": "edit", "edits": [
                {"path": "src/lib.rs", "op": "replace", "content": "pub fn answer() -> u32 { 42 }\n"},
                {"path": "tests/check.rs", "op": "replace", "content": "// disabled\n"},
                {"path": "src/answer_test.rs", "op": "create", "content": "// covers answer()\n"}
            ]})
            .to_string(),
            json!({"action": "finish", "summary": "answer is now 42"}).to_string(),
        ]),
    };
    run.pin_model(model.pin().clone()).unwrap();

    // 5. Run: the worker edits staging only; the protected test is refused.
    let report = run_worker(&mut run, &model, "Make answer() return 42.").unwrap();
    assert_eq!(
        report.accepted,
        vec![rel("src/lib.rs"), rel("src/answer_test.rs")]
    );
    assert_eq!(report.rejected, 1);
    assert!(report.verification.as_ref().unwrap().passed());
    assert_eq!(
        digest(&f.project),
        before,
        "nothing reached the project yet"
    );

    // 6. Review changes.
    let review = run.review().unwrap();
    assert_eq!(review.changes.len(), 2);

    // 7. Approve natively, then apply the exact reviewed candidate.
    let approval = run.request_approval(&info.name, &Yes).unwrap();
    assert_eq!(approval.binding(), &review.binding);
    let write = projects.grant_for_apply(info.id, binding).unwrap();
    let applied = run.apply(approval, write, &parent(&f)).unwrap();
    assert_eq!(applied.files.len(), 2);

    // 8. Result.
    assert_eq!(
        std::fs::read_to_string(f.project.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> u32 { 42 }\n"
    );
    assert_eq!(
        std::fs::read_to_string(f.project.join("tests/check.rs")).unwrap(),
        "#[test] fn t() { assert!(true); }\n",
        "the protected test was never changed"
    );

    // 9. Restore this run.
    let restore = run.request_restore(&info.name, &Yes).unwrap();
    let write = projects.grant_for_apply(info.id, binding).unwrap();
    run.restore(restore, write).unwrap();
    assert_eq!(digest(&f.project), before);
    assert_eq!(run.apply_state(), ApplyState::Restored);

    // The ledger tells the whole story and verifies.
    let kinds = kinds(&f, &run);
    for kind in [
        "run.created",
        "run.granted",
        "run.model_pinned",
        "worker.started",
        "worker.read",
        "edit.rejected",
        "worker.finished",
        "verify.structural",
        "review.computed",
        "approval.granted",
        "apply.prepared",
        "apply.completed",
        "restore.approved",
        "restore.prepared",
        "restore.completed",
    ] {
        assert!(kinds.contains(&kind.to_string()), "{kind}");
    }
}
