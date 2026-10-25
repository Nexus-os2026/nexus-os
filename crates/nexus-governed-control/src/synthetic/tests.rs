//! P4-UAT05: the owner's synthetic approval probe, attacked through the
//! front door and the authority (T-C1 to T-C13, N-3b, and the Architect's
//! class/kind mismatch safeguard). These are probe-execution (PX) proofs
//! only: they establish nothing about the native window on screen, a real
//! agent's request or any real-world effect.

use super::{admit, capped, grant_scope, prepare, SyntheticIntent, SyntheticReceipt};
use super::{COMMITMENT_TTL, GRANT_TTL, OPERATION, TARGET_TEXT};
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation, OWNER_ONLY_USE, SYNTHETIC_EFFECT,
};
use crate::authority::clock::ManualClock;
use crate::authority::commitment::{
    CommitmentState, CommitmentView, PreparedAction, TargetIdentity, MAX_COMMITMENT_TTL,
};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{EvidencePhase, EvidenceRecord, MemoryEvidence};
use crate::authority::ids::{AgentId, CommitmentId, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::RunOrigin;
use crate::authority::{Authority, AuthorityError};
use crate::broker::Vault;
use crate::control::{Control, EffectOutput};
use crate::governed::{GovernedControl, GrantRequest, Intent, NeverAsk, MAX_GRANT};
use crate::harness_tests::{temp_root, TempRoot};
use crate::ingress::{understand, Attachments, CommandEnvelope, Understood};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const REFUSED_FOR_AGENTS: AuthorityError =
    AuthorityError::Closed("a synthetic approval test is an owner-session command only");
const NOT_R2: AuthorityError =
    AuthorityError::InvalidAction("a synthetic approval test is classed R2 only");

/// The owner's scripted answers; it keeps every window it was shown.
struct Owner {
    answer: bool,
    actions: Mutex<Vec<ActionConfirmation>>,
    grants: Mutex<Vec<GrantConfirmation>>,
}

impl Owner {
    fn new(answer: bool) -> Self {
        Self {
            answer,
            actions: Mutex::new(Vec::new()),
            grants: Mutex::new(Vec::new()),
        }
    }

    fn asked(&self) -> usize {
        self.actions.lock().unwrap().len()
    }
}

impl ControlConfirmer for Owner {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.actions.lock().unwrap().push(request.clone());
        self.answer
    }
    fn confirm_grant(&self, request: &GrantConfirmation) -> bool {
        self.grants.lock().unwrap().push(request.clone());
        self.answer
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.answer
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

/// An owner who, while the window is shown, does `work`, then allows.
struct During<'a> {
    work: &'a dyn Fn(),
    asked: AtomicU32,
}

impl<'a> During<'a> {
    fn new(work: &'a dyn Fn()) -> Self {
        Self {
            work,
            asked: AtomicU32::new(0),
        }
    }
}

impl ControlConfirmer for During<'_> {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        self.asked.fetch_add(1, Ordering::SeqCst);
        (self.work)();
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

/// The front door over in-memory evidence and a clock that moves only when
/// told to.
struct Probe {
    control: GovernedControl,
    evidence: Arc<MemoryEvidence>,
    clock: Arc<ManualClock>,
    _root: TempRoot,
}

fn probe() -> Probe {
    let root = temp_root("synthetic");
    let evidence = Arc::new(MemoryEvidence::new(10_000));
    let clock = Arc::new(ManualClock::default());
    let control = GovernedControl::with_secrets(
        root.0.path(),
        Vault::Disabled.source(),
        evidence.clone(),
        clock.clone(),
    )
    .unwrap();
    Probe {
        control,
        evidence,
        clock,
        _root: root,
    }
}

impl Probe {
    fn grant(&self, ttl: Duration) -> GrantId {
        self.control
            .request_grant(&GrantRequest::Synthetic {}, ttl, &Owner::new(true))
            .unwrap()
    }

    fn owner_run(&self) -> (AgentId, RunId) {
        self.control.open_owner_run(vec!["text".into()]).unwrap()
    }

    fn propose(&self, agent: &AgentId, run: RunId) -> Result<CommitmentView, AuthorityError> {
        self.control
            .propose(agent, run, &Intent::Synthetic(SyntheticIntent {}))
    }

    /// One owner command, committed.
    fn submit(&self) -> (AgentId, RunId, CommitmentView) {
        let (agent, run) = self.owner_run();
        let view = self.propose(&agent, run).unwrap();
        (agent, run, view)
    }

    fn records_of(&self, id: CommitmentId) -> Vec<EvidenceRecord> {
        self.evidence
            .records()
            .into_iter()
            .filter(|r| r.commitment.as_deref() == Some(id.to_string().as_str()))
            .collect()
    }

    fn phases_of(&self, id: CommitmentId) -> Vec<EvidencePhase> {
        self.records_of(id).iter().map(|r| r.phase).collect()
    }

    fn count(&self, id: CommitmentId, phase: EvidencePhase) -> usize {
        self.phases_of(id).iter().filter(|p| **p == phase).count()
    }

    /// Every commitment record of kind `synthetic`.
    fn synthetic_records(&self) -> Vec<EvidenceRecord> {
        self.evidence
            .records()
            .into_iter()
            .filter(|r| r.commitment.is_some() && r.kind == Some(CapabilityKind::Synthetic))
            .collect()
    }

    fn state(&self, id: CommitmentId) -> CommitmentState {
        self.control
            .authority()
            .commitments()
            .view(id)
            .unwrap()
            .state
    }
}

fn hex32(text: &str) -> bool {
    text.len() == 32
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The receipt a commitment's summary shows: `Receipt: rcpt-<32 hex>`.
fn receipt_of(summary: &[String]) -> String {
    let line = &summary[3];
    let receipt = line.strip_prefix("Receipt: ").expect("the receipt line");
    assert!(receipt.strip_prefix("rcpt-").is_some_and(hex32), "{line}");
    receipt.to_string()
}

/// The five lines the owner reads, exactly.
fn expected_summary(receipt: &str) -> Vec<String> {
    vec![
        "SYNTHETIC TEST: Nexus performs no external effect".to_string(),
        "Allow issues one test receipt; nothing is written outside Nexus's own records".to_string(),
        "No network, process, browser, connector, display or input action".to_string(),
        format!("Receipt: {receipt}"),
        "A receipt is never evidence that a real-world action happened".to_string(),
    ]
}

/// T-C1: no live synthetic grant, no commitment, whatever else is granted.
#[test]
fn t_c1_without_a_live_synthetic_grant_nothing_is_prepared() {
    let p = probe();
    let (agent, run) = p.owner_run();
    assert_eq!(
        p.propose(&agent, run).unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    // Other kinds' grants cover nothing synthetic.
    let grants = p.control.authority().grants();
    for scope in [
        GrantScope::Egress {
            scheme: "https".into(),
            host: "example.invalid".into(),
            port: 443,
            methods: vec!["GET".into()],
            allow_private: false,
        },
        GrantScope::Perception {
            display: "agent".into(),
        },
        GrantScope::Tool {
            tool: "text.sha256".into(),
            executable: "/usr/bin/sha256sum".into(),
            identity: Digest::of("test.identity", &[b"x"]),
        },
    ] {
        grants
            .request(scope, Duration::from_secs(600), &Owner::new(true))
            .unwrap();
    }
    let (agent, run) = p.owner_run();
    assert_eq!(
        p.propose(&agent, run).unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    assert!(p.synthetic_records().is_empty());
}

/// T-C2 and N-3b: an agent's run never prepares one, by its class and not
/// its name: an agent whose identity is the text `owner-session` is still
/// an agent. Nothing is recorded as prepared.
#[test]
fn t_c2_n3b_an_agent_run_never_prepares_a_synthetic_receipt() {
    let p = probe();
    p.grant(Duration::from_secs(600));
    for name in ["agent-a", "owner-session"] {
        let agent = AgentId::new(name).unwrap();
        for origin in [
            RunOrigin::AgentGoal,
            RunOrigin::Command {
                modalities: vec!["text".into()],
            },
        ] {
            let run = p.control.open_run(agent.clone(), origin).unwrap();
            assert!(!p.control.is_owner_run(run));
            assert_eq!(p.propose(&agent, run).unwrap_err(), REFUSED_FOR_AGENTS);
            assert_eq!(
                p.control
                    .agent_action(&agent, run, &Intent::Synthetic(SyntheticIntent {}))
                    .unwrap_err(),
                REFUSED_FOR_AGENTS
            );
            assert!(p
                .control
                .authority()
                .commitments()
                .views_of_run(run)
                .is_empty());
        }
    }
    // An agent's path cannot borrow an owner run either.
    let (owner, run) = p.owner_run();
    assert_eq!(
        p.control
            .agent_action(&owner, run, &Intent::Synthetic(SyntheticIntent {}))
            .unwrap_err(),
        AuthorityError::WrongRun
    );
    // Nor does a run the backend does not know.
    let unknown = RunId::parse("run-00000000000000000000000000000000").unwrap();
    let authority = p.control.authority();
    assert_eq!(
        prepare(authority, unknown, &SyntheticIntent {})
            .err()
            .unwrap(),
        REFUSED_FOR_AGENTS
    );
    assert!(p.synthetic_records().is_empty());
}

/// T-C3: the owner's Allow issues exactly one truthful receipt, bound to the
/// one shown, with one record of each phase, all labelled synthetic.
#[test]
fn t_c3_allow_issues_exactly_one_receipt_and_its_records() {
    let p = probe();
    let grant = p.grant(Duration::from_secs(600));
    let (agent, run, view) = p.submit();
    assert_eq!(agent.as_str(), "owner-session");
    assert_eq!(view.kind, CapabilityKind::Synthetic);
    assert_eq!(view.class, EffectClass::R2);
    assert!(view.requires_approval);
    assert_eq!(view.operation, OPERATION);
    assert_eq!(view.operation, "synthetic.receipt");
    assert_eq!(view.target, TARGET_TEXT);
    assert_eq!(
        view.target,
        "none (synthetic test receipt; no external target)"
    );
    assert_eq!(view.state, CommitmentState::Prepared);
    let receipt = receipt_of(&view.summary);
    assert_eq!(view.summary, expected_summary(&receipt));
    // Only the owner's native Allow authorizes it; the window named it.
    let owner = Owner::new(true);
    p.control.authorize(view.id, &agent, run, &owner).unwrap();
    assert_eq!(owner.asked(), 1);
    let seen = owner.actions.lock().unwrap()[0].clone();
    assert_eq!(seen.commitment, view.id.to_string());
    assert_eq!(seen.kind, CapabilityKind::Synthetic);
    assert_eq!(seen.class, EffectClass::R2);
    assert_eq!(seen.summary, view.summary);
    assert!(seen.expires_in_secs <= COMMITMENT_TTL.as_secs());
    let output: EffectOutput = p.control.execute(view.id, &agent, run).unwrap();
    assert_eq!(
        output.text.as_deref(),
        Some(
            format!(
                "SYNTHETIC TEST RECEIPT {receipt} for {}: no external effect was performed.",
                view.id
            )
            .as_str()
        )
    );
    assert_eq!(output.bytes, None);
    let meta = vec![
        ("synthetic".to_string(), "true".to_string()),
        ("external_effect".to_string(), "none".to_string()),
        ("receipt".to_string(), receipt.clone()),
    ];
    assert_eq!(output.meta, meta);
    assert!(output.evidence.is_some());
    assert_eq!(p.state(view.id), CommitmentState::Succeeded);
    assert_eq!(
        p.phases_of(view.id),
        [
            EvidencePhase::Prepared,
            EvidencePhase::Approved,
            EvidencePhase::Authorized,
            EvidencePhase::Started,
            EvidencePhase::Finished,
        ]
    );
    for record in p.records_of(view.id) {
        assert_eq!(record.kind, Some(CapabilityKind::Synthetic));
        assert_eq!(record.class, Some(EffectClass::R2));
        assert_eq!(record.operation, Some(OPERATION));
        assert_eq!(record.target.as_deref(), Some(TARGET_TEXT));
        assert_eq!(record.agent.as_deref(), Some("owner-session"));
        assert_eq!(record.run, Some(run.to_string()));
        let json = record.to_json();
        assert_eq!(json["kind"], "synthetic");
        assert_eq!(json["operation"], "synthetic.receipt");
    }
    let finished = p.records_of(view.id).pop().unwrap();
    assert_eq!(finished.outcome, Some("succeeded"));
    assert_eq!(finished.detail, meta);
    // The grant it rested on is the synthetic one.
    assert_eq!(
        p.control.authority().grants().live(grant).unwrap().scope,
        GrantScope::Synthetic
    );
}

/// T-C4: Cancel in the window (any answer but Allow) declines it: no
/// receipt, no start, the effect dropped.
#[test]
fn t_c4_cancel_in_the_window_declines_and_issues_nothing() {
    let p = probe();
    p.grant(Duration::from_secs(600));
    let (agent, run, view) = p.submit();
    let owner = Owner::new(false);
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(owner.asked(), 1);
    assert_eq!(p.state(view.id), CommitmentState::Denied);
    assert_eq!(
        p.phases_of(view.id),
        [EvidencePhase::Prepared, EvidencePhase::ApprovalDeclined]
    );
    // The pending effect was dropped: nothing is left to run.
    assert_eq!(
        p.control.execute(view.id, &agent, run).unwrap_err(),
        AuthorityError::UnknownCommitment
    );
    assert_eq!(p.count(view.id, EvidencePhase::Started), 0);
}

/// T-C5: Deny before review: no approval is asked or given, no receipt.
#[test]
fn t_c5_deny_before_review_issues_nothing() {
    let p = probe();
    p.grant(Duration::from_secs(600));
    let (agent, run, view) = p.submit();
    p.control.deny(view.id, &agent, run).unwrap();
    assert_eq!(p.state(view.id), CommitmentState::Denied);
    let owner = Owner::new(true);
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::NotPending
    );
    assert_eq!(owner.asked(), 0, "no window for a denied request");
    assert_eq!(
        p.control.execute(view.id, &agent, run).unwrap_err(),
        AuthorityError::UnknownCommitment
    );
    assert_eq!(
        p.phases_of(view.id),
        [EvidencePhase::Prepared, EvidencePhase::Denied]
    );
}

/// T-C6: a receipt is issued once, however often or concurrently it is
/// asked for.
#[test]
fn t_c6_replay_and_concurrent_approvals_issue_one_receipt() {
    let p = probe();
    p.grant(Duration::from_secs(600));
    let (agent, run, view) = p.submit();
    let owner = Owner::new(true);
    p.control.authorize(view.id, &agent, run, &owner).unwrap();
    p.control.execute(view.id, &agent, run).unwrap();
    // Replayed: no second window, no second receipt.
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::NotPending
    );
    assert_eq!(owner.asked(), 1);
    assert_eq!(
        p.control.execute(view.id, &agent, run).unwrap_err(),
        AuthorityError::UnknownCommitment
    );
    assert_eq!(p.count(view.id, EvidencePhase::Started), 1);

    // Sixteen concurrent approvals of a new request: one receipt.
    let (agent, run, view) = p.submit();
    let owner = Owner::new(true);
    let results: Vec<Result<EffectOutput, AuthorityError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    p.control
                        .authorize(view.id, &agent, run, &owner)
                        .and_then(|()| p.control.execute(view.id, &agent, run))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    // Overlapping windows may each be answered (each answer recorded as an
    // approval: the desktop shows one window at a time); only one approval
    // is ever consumed, and the receipt is issued once.
    assert!(p.count(view.id, EvidencePhase::Approved) >= 1);
    for phase in [
        EvidencePhase::Authorized,
        EvidencePhase::Started,
        EvidencePhase::Finished,
    ] {
        assert_eq!(p.count(view.id, phase), 1, "{phase:?}");
    }
    for refused in results.iter().filter_map(|r| r.as_ref().err()) {
        assert_eq!(*refused, AuthorityError::NotPending, "{results:?}");
    }
    assert_eq!(p.state(view.id), CommitmentState::Succeeded);
}

/// T-C7: expired requests cannot proceed: before the window, while it is
/// shown, and on the wall clock alone (a suspended machine).
#[test]
fn t_c7_an_expired_request_is_refused_wherever_it_expires() {
    let p = probe();
    p.grant(Duration::from_secs(900));
    // (a) Expired before review: no window at all.
    let (agent, run, view) = p.submit();
    p.clock.advance(COMMITMENT_TTL + Duration::from_secs(1));
    let owner = Owner::new(true);
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::Expired
    );
    assert_eq!(owner.asked(), 0);
    assert_eq!(p.state(view.id), CommitmentState::Expired);
    assert_eq!(p.count(view.id, EvidencePhase::Approved), 0);

    // (b) Expired while the window was shown: the answer approves nothing.
    let (agent, run, view) = p.submit();
    let clock = p.clock.clone();
    let late = move || clock.advance(COMMITMENT_TTL + Duration::from_secs(1));
    let during = During::new(&late);
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &during)
            .unwrap_err(),
        AuthorityError::Expired
    );
    assert_eq!(during.asked.load(Ordering::SeqCst), 1);
    assert_eq!(p.state(view.id), CommitmentState::Expired);
    assert_eq!(p.count(view.id, EvidencePhase::Approved), 0);

    // (c) Only the wall clock moved (the machine slept).
    let (agent, run, view) = p.submit();
    p.clock.suspend(COMMITMENT_TTL + Duration::from_secs(1));
    let owner = Owner::new(true);
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::Expired
    );
    assert_eq!(owner.asked(), 0);
    assert_eq!(p.count(view.id, EvidencePhase::Approved), 0);
    for record in p.synthetic_records() {
        assert!(!matches!(
            record.phase,
            EvidencePhase::Started | EvidencePhase::Finished
        ));
    }
}

/// T-C8: a policy change, a revoked or a lapsed grant ends the request.
#[test]
fn t_c8_stale_revoked_and_lapsed_grants_refuse() {
    let p = probe();
    let synthetic = p.grant(Duration::from_secs(600));
    let other = p
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Perception {
                display: "agent".into(),
            },
            Duration::from_secs(600),
            &Owner::new(true),
        )
        .unwrap();
    // Another grant revoked in the authority: the generation moved.
    let (agent, run, view) = p.submit();
    p.control.authority().grants().revoke(other).unwrap();
    let owner = Owner::new(true);
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::Stale
    );
    assert_eq!(owner.asked(), 0);
    assert_eq!(p.state(view.id), CommitmentState::Revoked);
    // Its own grant revoked through the front door: ended at once, recorded.
    let (agent, run, view) = p.submit();
    p.control.revoke_grant(synthetic).unwrap();
    assert_eq!(p.state(view.id), CommitmentState::Revoked);
    let revoked = p.records_of(view.id).pop().unwrap();
    assert_eq!(revoked.phase, EvidencePhase::Revoked);
    assert!(p.control.authorize(view.id, &agent, run, &owner).is_err());
    assert_eq!(owner.asked(), 0);
    // And no further request rests on it, in this run or a new one.
    assert_eq!(
        p.propose(&agent, run).unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    let (agent, run) = p.owner_run();
    assert_eq!(
        p.propose(&agent, run).unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    // A 60 s grant under a 180 s request: it lapses first.
    p.grant(Duration::from_secs(60));
    let (agent, run, view) = p.submit();
    p.clock.advance(Duration::from_secs(61));
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::GrantNotLive
    );
    assert_eq!(owner.asked(), 0);
    assert_eq!(p.count(view.id, EvidencePhase::Approved), 0);
}

/// T-C9: a changed request refuses: another request's approval, another
/// agent or run, another target, other parameters.
#[test]
fn t_c9_changed_requests_refuse() {
    let p = probe();
    p.grant(Duration::from_secs(600));
    let commitments = p.control.authority().commitments();
    let (agent, run, a) = p.submit();
    let (agent_b, run_b, b) = p.submit();
    let owner = Owner::new(true);
    let approval_b = commitments
        .request_approval(b.id, &agent_b, run_b, &owner)
        .unwrap();
    assert_eq!(
        commitments.authorize(a.id, &agent, run, Some(approval_b)),
        Err(AuthorityError::ApprovalMismatch)
    );
    // Another agent or run, anywhere along the way.
    let intruder = AgentId::new("intruder").unwrap();
    assert_eq!(
        p.control
            .authorize(a.id, &intruder, run, &owner)
            .unwrap_err(),
        AuthorityError::WrongAgent
    );
    assert_eq!(
        p.control
            .authorize(a.id, &agent, run_b, &owner)
            .unwrap_err(),
        AuthorityError::WrongRun
    );
    assert_eq!(
        p.control.deny(a.id, &intruder, run).unwrap_err(),
        AuthorityError::WrongAgent
    );
    assert_eq!(
        p.control.execute(a.id, &agent, run_b).unwrap_err(),
        AuthorityError::WrongRun
    );
    assert_eq!(owner.asked(), 1, "only B's window was shown");

    // Below the front door: an effect whose target or parameters differ
    // from what was committed never issues a receipt.
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let control = Control::new(Authority::new(
        evidence.clone(),
        Arc::new(ManualClock::default()),
    ));
    let authority = control.authority();
    authority
        .grants()
        .request(grant_scope(), GRANT_TTL, &Owner::new(true))
        .unwrap();
    let (agent, run) = authority.open_owner_run(vec!["text".into()]).unwrap();
    let moved = Digest::of("moved", &[b"elsewhere"]);
    let mut preparation = prepare(authority, run, &SyntheticIntent {}).unwrap();
    preparation.effect = Box::new(SyntheticReceipt {
        receipt: "rcpt-00000000000000000000000000000000".into(),
        target: moved,
        parameters: preparation.action.parameters,
    });
    let view = control.propose(&agent, run, preparation).unwrap();
    control.authorize(view.id, &agent, run, &owner).unwrap();
    assert_eq!(
        control.execute(view.id, &agent, run).unwrap_err(),
        AuthorityError::TargetChanged
    );
    assert_eq!(
        authority.commitments().view(view.id).unwrap().state,
        CommitmentState::Failed
    );
    assert!(!evidence
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::Started));
    let mut preparation = prepare(authority, run, &SyntheticIntent {}).unwrap();
    preparation.effect = Box::new(SyntheticReceipt {
        receipt: "rcpt-00000000000000000000000000000000".into(),
        target: preparation.action.target.digest,
        parameters: Digest::of("other", &[b"parameters"]),
    });
    assert_eq!(
        control.propose(&agent, run, preparation).unwrap_err(),
        AuthorityError::InvalidAction("the prepared parameters do not match the effect")
    );
}

/// T-C10: Stop (the run), a stop during the window and the emergency stop
/// all refuse continuation; only the owner's native Resume lifts the stop.
#[test]
fn t_c10_stops_refuse_continuation() {
    let p = probe();
    p.grant(Duration::from_secs(600));
    // Stop this run before review.
    let (agent, run, view) = p.submit();
    p.control.cancel_run(run).unwrap();
    assert_eq!(p.state(view.id), CommitmentState::Revoked);
    let owner = Owner::new(true);
    assert!(p.control.authorize(view.id, &agent, run, &owner).is_err());
    assert_eq!(owner.asked(), 0);
    let revoked = p.records_of(view.id).pop().unwrap();
    assert_eq!(revoked.failure, Some("run_cancelled"));
    // Stop while the window is shown: the Allow approves nothing.
    let (agent, run, view) = p.submit();
    let control = &p.control;
    let stop = || control.cancel_run(run).unwrap();
    let during = During::new(&stop);
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &during)
            .unwrap_err(),
        AuthorityError::NotPending
    );
    assert_eq!(p.state(view.id), CommitmentState::Revoked);
    assert_eq!(p.count(view.id, EvidencePhase::Approved), 0);
    // The emergency stop: revoked, no new command until a native Resume.
    let (agent, run, view) = p.submit();
    p.control.emergency_stop();
    assert_eq!(p.state(view.id), CommitmentState::Revoked);
    assert!(p.control.authorize(view.id, &agent, run, &owner).is_err());
    assert_eq!(
        p.control.open_owner_run(vec!["text".into()]).unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert_eq!(
        p.control.resume(&Owner::new(false)).unwrap_err(),
        AuthorityError::Declined
    );
    assert!(p.control.authority().runs().is_stopped());
    p.control.resume(&Owner::new(true)).unwrap();
    let (_, _, view) = p.submit();
    assert_eq!(view.state, CommitmentState::Prepared);
    assert_eq!(owner.asked(), 0);
    for record in p.synthetic_records() {
        assert_ne!(record.phase, EvidencePhase::Started);
    }
}

/// T-C11: the class is fixed by the backend: nothing the caller sends
/// chooses it, an R2 request needs the native approval, and an agent's
/// path (which never asks) is declined.
#[test]
fn t_c11_the_class_is_fixed_and_needs_the_native_approval() {
    assert_eq!(
        serde_json::from_value::<Intent>(json!({ "domain": "synthetic", "intent": {} })).unwrap(),
        Intent::Synthetic(SyntheticIntent {})
    );
    for forged in [
        json!({ "domain": "synthetic", "intent": { "class": "R1" } }),
        json!({ "domain": "synthetic", "intent": { "x": 1 } }),
        json!({ "domain": "synthetic", "intent": { "receipt": "rcpt-0" } }),
        json!({ "domain": "synthetic", "intent": { "target": "https://example.com/" } }),
        json!({ "domain": "synthetic", "intent": {}, "class": "R0" }),
    ] {
        assert!(
            serde_json::from_value::<Intent>(forged.clone()).is_err(),
            "{forged}"
        );
    }
    let p = probe();
    p.grant(Duration::from_secs(600));
    let (agent, run, view) = p.submit();
    assert_eq!(view.class, EffectClass::R2);
    assert!(view.requires_approval);
    assert_eq!(
        p.control
            .authority()
            .commitments()
            .authorize(view.id, &agent, run, None),
        Err(AuthorityError::ApprovalRequired)
    );
    // The confirmer of every non-owner path declines.
    assert_eq!(
        p.control
            .authorize(view.id, &agent, run, &NeverAsk)
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(p.state(view.id), CommitmentState::Denied);
}

/// T-C12: a synthetic grant covers nothing else and nothing else covers a
/// synthetic request; it lasts at most 15 minutes, and the grant request
/// carries nothing the caller chooses.
#[test]
fn t_c12_the_grant_is_isolated_and_short() {
    let p = probe();
    let owner = Owner::new(true);
    let id = p
        .control
        .request_grant(
            &GrantRequest::Synthetic {},
            Duration::from_secs(86_400),
            &owner,
        )
        .unwrap();
    let grant = p.control.authority().grants().live(id).unwrap();
    assert_eq!(
        grant.expires_wall_ms - grant.created_wall_ms,
        u64::try_from(GRANT_TTL.as_millis()).unwrap()
    );
    assert_eq!(GRANT_TTL, Duration::from_secs(900));
    let shown = owner.grants.lock().unwrap()[0].clone();
    assert_eq!(shown.kind, CapabilityKind::Synthetic);
    assert_eq!(shown.expires_in_secs, 900);
    // The cap is the synthetic kind's alone.
    assert_eq!(
        capped(
            &GrantScope::Perception {
                display: "agent".into()
            },
            MAX_GRANT
        ),
        MAX_GRANT
    );
    assert_eq!(capped(&GrantScope::Synthetic, MAX_GRANT), GRANT_TTL);
    assert!(GRANT_TTL <= MAX_GRANT);
    assert!(COMMITMENT_TTL <= MAX_COMMITMENT_TTL);
    assert_eq!(COMMITMENT_TTL, Duration::from_secs(180));
    // Grants cover only their own kind, both ways.
    let commitments = p.control.authority().commitments();
    let (agent, run) = p.owner_run();
    let egress = PreparedAction {
        kind: CapabilityKind::Egress,
        class: EffectClass::R1,
        operation: "egress.fetch",
        target: TargetIdentity {
            display: "https://example.invalid/".into(),
            digest: Digest::of("t", &[b"t"]),
        },
        parameters: Digest::of("p", &[b"p"]),
        grants: vec![id],
        leases: vec![],
        summary: vec!["GET https://example.invalid/".into()],
    };
    assert_eq!(
        commitments
            .prepare(&agent, run, egress, Duration::from_secs(60))
            .unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    let perception = p
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Perception {
                display: "agent".into(),
            },
            Duration::from_secs(600),
            &Owner::new(true),
        )
        .unwrap();
    let mut synthetic = prepare(p.control.authority(), run, &SyntheticIntent {})
        .unwrap()
        .action;
    synthetic.grants = vec![perception];
    assert_eq!(
        commitments
            .prepare(&agent, run, synthetic, COMMITMENT_TTL)
            .unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    // The request is data with nothing to choose.
    assert_eq!(
        serde_json::from_value::<GrantRequest>(json!({ "kind": "synthetic" })).unwrap(),
        GrantRequest::Synthetic {}
    );
    for forged in [
        json!({ "kind": "synthetic", "x": 1 }),
        json!({ "kind": "synthetic", "ttl": 86400 }),
        json!({ "kind": "synthetic", "scope": "tool" }),
    ] {
        assert!(
            serde_json::from_value::<GrantRequest>(forged.clone()).is_err(),
            "{forged}"
        );
    }
    // A declined grant creates nothing.
    assert_eq!(
        p.control
            .request_grant(
                &GrantRequest::Synthetic {},
                Duration::from_secs(60),
                &Owner::new(false)
            )
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(
        p.control
            .authority()
            .grants()
            .live_of(CapabilityKind::Synthetic)
            .len(),
        1
    );
}

/// T-C13: the command is one word that takes nothing; spoken or typed, it is
/// only data.
#[test]
fn t_c13_the_command_is_strict() {
    let attachments = Attachments::default();
    let read = |envelope: CommandEnvelope| understand(&envelope, &attachments).unwrap();
    let text = |text: &str| {
        read(CommandEnvelope {
            text: Some(text.into()),
            ..Default::default()
        })
    };
    let probe = Understood::Intent(Intent::Synthetic(SyntheticIntent {}));
    for command in [
        "synthetic-receipt",
        "SYNTHETIC-RECEIPT",
        "  synthetic-receipt\n",
    ] {
        assert_eq!(text(command), probe, "{command:?}");
    }
    for command in [
        "synthetic-receipt now",
        "synthetic-receipt R1",
        "synthetic-receipt {\"class\":\"R1\"}",
        "synthetic-receipt https://example.com/",
    ] {
        assert_eq!(
            text(command),
            Understood::NotUnderstood("synthetic-receipt takes nothing"),
            "{command:?}"
        );
    }
    for command in [
        "synthetic",
        "synthetic receipt",
        "receipt",
        "synthetic-receipts",
    ] {
        assert!(
            matches!(text(command), Understood::NotUnderstood(_)),
            "{command:?}"
        );
    }
    assert_eq!(
        read(CommandEnvelope {
            voice_transcript: Some("synthetic-receipt".into()),
            ..Default::default()
        }),
        probe
    );
    assert_eq!(
        text("please"),
        Understood::NotUnderstood(
            "not a governed command: fetch, browse, observe, hash, say, connector or synthetic-receipt"
        )
    );
}

/// The Architect's safeguard: the harmless wording belongs to the exact pair
/// (`synthetic`, R2). A synthetic action of any other class is refused
/// before it is committed, and the window would never read it as harmless.
#[test]
fn a_synthetic_kind_with_any_class_but_r2_is_refused_and_never_reads_harmless() {
    let action = |kind: CapabilityKind, class: EffectClass| PreparedAction {
        kind,
        class,
        operation: OPERATION,
        target: TargetIdentity {
            display: TARGET_TEXT.into(),
            digest: Digest::of("t", &[b"t"]),
        },
        parameters: Digest::of("p", &[b"p"]),
        grants: vec![],
        leases: vec![],
        summary: vec![],
    };
    let classes = [EffectClass::R0, EffectClass::R1, EffectClass::R2];
    for class in classes {
        let admitted = admit(&action(CapabilityKind::Synthetic, class));
        if class == EffectClass::R2 {
            assert_eq!(admitted, Ok(()));
        } else {
            assert_eq!(admitted, Err(NOT_R2), "{class:?}");
        }
        for kind in [
            CapabilityKind::Tool,
            CapabilityKind::Egress,
            CapabilityKind::Browser,
            CapabilityKind::Perception,
            CapabilityKind::Input,
            CapabilityKind::Connector,
        ] {
            assert_eq!(admit(&action(kind, class)), Ok(()), "{kind:?} {class:?}");
        }
        let shown = ActionConfirmation {
            commitment: "cmt-00000000000000000000000000000000".into(),
            class,
            kind: CapabilityKind::Synthetic,
            operation: OPERATION.into(),
            target: TARGET_TEXT.into(),
            agent: "owner-session".into(),
            run: "run-00000000000000000000000000000000".into(),
            summary: vec![],
            expires_in_secs: 60,
            binding_short: "000000000000".into(),
        };
        let effect = shown.text().header[0].1.clone();
        if class == EffectClass::R2 {
            assert_eq!(effect, SYNTHETIC_EFFECT);
        } else {
            assert_ne!(effect, SYNTHETIC_EFFECT);
            assert!(!effect.contains("no external effect"), "{effect}");
            assert!(effect.starts_with(&format!("{class} (")), "{effect}");
            assert!(effect.contains("not a valid synthetic test"), "{effect}");
        }
    }
    // Below the front door, which no production path takes: the authority
    // shows no window for an R0 or R1 commitment, and would let one run
    // without the owner's native approval. That is why the front door
    // refuses a synthetic action of any class but R2 first.
    let control = Control::new(Authority::new(
        Arc::new(MemoryEvidence::new(1_000)),
        Arc::new(ManualClock::default()),
    ));
    let authority = control.authority();
    authority
        .grants()
        .request(grant_scope(), GRANT_TTL, &Owner::new(true))
        .unwrap();
    let (agent, run) = authority.open_owner_run(vec!["text".into()]).unwrap();
    let mut mismatched = prepare(authority, run, &SyntheticIntent {}).unwrap();
    mismatched.action.class = EffectClass::R1;
    assert_eq!(admit(&mismatched.action), Err(NOT_R2));
    let view = control.propose(&agent, run, mismatched).unwrap();
    let owner = Owner::new(true);
    assert_eq!(
        authority
            .commitments()
            .request_approval(view.id, &agent, run, &owner)
            .unwrap_err(),
        AuthorityError::ApprovalNotApplicable
    );
    assert_eq!(owner.asked(), 0, "no native window for a non-R2 request");
}

/// The same safeguard through the front door: every synthetic action it
/// commits is R2 and needs the native approval; one that is not R2 is
/// refused there, before any record (a class changed in `prepare` is caught
/// here, with or without `admit`; see the negative controls).
#[test]
fn a_synthetic_action_is_r2_or_refused_before_any_commitment() {
    let p = probe();
    p.grant(Duration::from_secs(600));
    let (agent, run) = p.owner_run();
    let before = p.synthetic_records().len();
    match p.propose(&agent, run) {
        Ok(view) => {
            assert_eq!(
                view.class,
                EffectClass::R2,
                "a non-R2 synthetic request was committed"
            );
            assert!(view.requires_approval);
            assert_eq!(
                p.control
                    .authorize(view.id, &agent, run, &NeverAsk)
                    .unwrap_err(),
                AuthorityError::Declined,
                "a synthetic request ran without the native approval"
            );
        }
        Err(error) => {
            assert_eq!(error, NOT_R2);
            assert_eq!(p.synthetic_records().len(), before);
        }
    }
}

/// The native grant window's header for the synthetic kind (its exact text
/// is pinned in the authority tests, N-3), and its four lines, in order.
#[test]
fn n3_the_grant_window_names_owner_session_commands_and_the_four_lines() {
    let p = probe();
    let owner = Owner::new(true);
    p.control
        .request_grant(
            &GrantRequest::Synthetic {},
            Duration::from_secs(86_400),
            &owner,
        )
        .unwrap();
    let shown = owner.grants.lock().unwrap()[0].clone();
    let text = shown.text();
    assert_eq!(
        text.header,
        vec![
            ("Capability", "synthetic".to_string()),
            ("Who may use it", OWNER_ONLY_USE.to_string()),
            ("Expires in", "900 s".to_string()),
        ]
    );
    assert_eq!(
        text.details,
        vec![
            "Synthetic test receipts only: Nexus performs no external effect",
            "Usable only by your own commands, never by an agent",
            "Each receipt also needs your approval in a separate Nexus approval window",
            "A receipt is never evidence that a real-world action happened",
        ]
    );
    assert!(!shown.message().contains("any agent"));
    // The grant's own record carries its kind and first line.
    let issued = p
        .evidence
        .records()
        .into_iter()
        .find(|r| r.phase == EvidencePhase::GrantIssued)
        .unwrap();
    assert_eq!(issued.kind, Some(CapabilityKind::Synthetic));
    assert_eq!(
        issued.target.as_deref(),
        Some("Synthetic test receipts only: Nexus performs no external effect")
    );
}
