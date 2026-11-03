//! M2d-C oracles (design v2 §3 I7, §6.1 steps 2-3, §6.8 M2d-C, §6.10 D-O12
//! and D-O15 at Mother; §11 M3′ and M4′; the Architect's decisions of
//! 2026-11-03), pre-registered before the code: Mother's Files admission,
//! her create requests and her own refusals, against the recording port.
//! Their real-broker twins are in `adapter/m2d_files.rs`, which shares the
//! lanes and the Owner here (test confirmers live in `oracles/`).

use super::{contract_with, test_private, world, world_with, Recording, RecordingPort};
use crate::adapter::m2c_support::{consent, resume_with, HeldOwner};
use crate::adapter::p0_support::{action, canaries};
use crate::adapter::status_of;
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::{Admission, ApprovalStatus, EffectOutcome, EffectRequest};
use crate::evidence::{BlockReason, BrokerClass, MotherRefusal, PublicationCause};
use crate::journal::{Event, MissionId};
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, ScriptedLane, Verdict};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::files::FileCreate;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------- shared

/// The Owner at the broker: yes to every action (each one's dialog kept)
/// and every grant; no to anything else.
#[derive(Default)]
pub(crate) struct FilesOwner(pub Mutex<Vec<ActionConfirmation>>);

impl ControlConfirmer for FilesOwner {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.0.lock().unwrap().push(request.clone());
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

/// The content of a `MK<KIND>:<name>` marker.
pub(crate) fn content_of(kind: &str, name: &str) -> Option<String> {
    Some(match kind {
        "MKFILE" | "VFILE" => format!("{name}: made by Mother\n"),
        "MKBIG" => "x".repeat(8 * 1024 + 1),
        "MKLINES" => "l\n".repeat(151),
        "MKNUL" => "a\0b\n".into(),
        // Within every limit, but more than 192 lines to show.
        "MKWIDE" => format!(
            "{}{}",
            format!("{}\n", "w".repeat(120)).repeat(60),
            "x\n".repeat(90)
        ),
        _ => return None,
    })
}

/// A lane that asks for one File create per `MK<KIND>:<name>` word of the
/// restatement (the Builder), or per `VFILE:<name>` word (the Verifier,
/// which then passes).
pub(crate) struct FileLane(pub LaneRole);

impl Lane for FileLane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: self.0,
            name: format!("m2d-c-files-{}", self.0.as_str()),
            version: "m2d-c".into(),
        }
    }

    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        let verifier = self.0 == LaneRole::Verifier;
        let effects = input
            .restatement
            .split(' ')
            .filter_map(|word| {
                let (kind, name) = word.split_once(':')?;
                if verifier != (kind == "VFILE") {
                    return None;
                }
                Some(file(name, &content_of(kind, name)?))
            })
            .collect();
        LaneOutput {
            text: "the lane wrote".into(),
            effects,
            verdict: if verifier {
                Verdict::Pass
            } else {
                Verdict::Continue
            },
        }
    }
}

/// The keyed lanes, with the Builder and the Verifier asking for files.
pub(crate) fn files_lanes() -> Vec<Arc<dyn Lane>> {
    let mut lanes: Vec<Arc<dyn Lane>> = super::keyed_lanes()
        .into_iter()
        .filter(|l| !matches!(l.identity().role, LaneRole::Builder | LaneRole::Verifier))
        .collect();
    lanes.push(Arc::new(FileLane(LaneRole::Builder)));
    lanes.push(Arc::new(FileLane(LaneRole::Verifier)));
    lanes
}

pub(crate) fn file(name: &str, content: &str) -> EffectRequest {
    EffectRequest::File(FileCreate {
        name: name.into(),
        content: content.into(),
    })
}

/// The journaled events of `id`, with their sequence numbers.
fn events(mother: &Coordinator<RecordingPort>, id: MissionId) -> Vec<(u64, Event)> {
    mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .map(|e| (e.seq, e.event))
        .collect()
}

fn seq_of(entries: &[(u64, Event)], pick: impl Fn(&Event) -> bool) -> u64 {
    entries
        .iter()
        .find(|(_, e)| pick(e))
        .map(|(seq, _)| *seq)
        .expect("the entry")
}

fn is_consent(event: &Event) -> bool {
    matches!(
        event,
        Event::Transition {
            trigger: Trigger::Consent { .. }
        }
    )
}

// ---------------------------------------------------------------- I1

/// I1 (appended variants only): the new codes serialize as themselves and
/// parse back from their closed sets; nothing else parses; `BrokerClass`
/// mirrors the broker's seven Files classes and the port's own
/// `admission_stale`.
#[test]
fn m2d_c_i1_the_new_codes_round_trip_and_nothing_else_parses() {
    for (refusal, code) in [
        (MotherRefusal::FileNameInvalid, "file_name_invalid"),
        (MotherRefusal::FileNotText, "file_not_text"),
        (MotherRefusal::FileTooLarge, "file_too_large"),
        (MotherRefusal::FileTooManyLines, "file_too_many_lines"),
        (
            MotherRefusal::FileFolderNotWorkspace,
            "file_folder_not_workspace",
        ),
    ] {
        assert_eq!(refusal.to_string(), code);
        assert_eq!(MotherRefusal::parse(code), Some(refusal));
        assert!(MotherRefusal::every().contains(&refusal));
        assert_eq!(
            BlockReason::parse(code),
            Some(BlockReason::Refused(refusal))
        );
    }
    let unverifiable = BlockReason::FilesConsentUnverifiable;
    assert_eq!(unverifiable.to_string(), "files_consent_unverifiable");
    assert_eq!(
        BlockReason::parse("files_consent_unverifiable"),
        Some(unverifiable)
    );
    assert!(BlockReason::every().contains(&unverifiable));
    let causes = [
        "emergency_stop",
        "cancelled",
        "revoked",
        "expired",
        "commitment_expired",
        "write_grant_revoked",
        "write_grant_expired",
        "unknown",
    ];
    assert_eq!(PublicationCause::every().len(), causes.len());
    for code in causes {
        let cause = PublicationCause::parse(code).expect(code);
        assert_eq!(PublicationCause::of(code), cause);
        let reason = BlockReason::FilePublicationInterrupted(cause);
        let text = format!("file_publication_interrupted:{code}");
        assert_eq!(reason.to_string(), text);
        assert_eq!(BlockReason::parse(&text), Some(reason));
        assert!(BlockReason::every().contains(&reason));
    }
    for error in [
        AuthorityError::NotAdmitted,
        AuthorityError::NoConsent,
        AuthorityError::AlreadyAdmitted,
        AuthorityError::FileExists,
        AuthorityError::ChangeTooLargeToShow,
        AuthorityError::UnsupportedMountTopology,
        AuthorityError::RecoveryRequired,
    ] {
        let class = BrokerClass::of(error.class());
        assert_ne!(class, BrokerClass::Unknown, "{}", error.class());
        assert_eq!(class.to_string(), error.class());
    }
    assert_eq!(BrokerClass::AdmissionStale.to_string(), "admission_stale");
    assert_eq!(
        BrokerClass::parse("admission_stale"),
        Some(BrokerClass::AdmissionStale)
    );
    for (_, canary) in canaries().both() {
        assert_eq!(PublicationCause::parse(canary), None);
        assert_eq!(PublicationCause::of(canary), PublicationCause::Unknown);
        let text = format!("file_publication_interrupted:{canary}");
        assert_eq!(BlockReason::parse(&text), None);
    }
}

// ---------------------------------------------------------------- refusals

/// A request's name and content, the contract's folder, the port's
/// workspace, and Mother's code (none: it reaches the port).
type Case<'a> = (&'a str, String, &'a str, Option<&'a str>, Option<&'a str>);

/// D-O12 and D-O15 (h) at Mother: a request outside the rules, or for a
/// folder that is not the Owner's workspace, ends the mission with Mother's
/// own code before the port is asked anything (no admission, no perform).
/// The limits themselves pass.
#[test]
fn m2d_c_mother_refuses_a_bad_file_request_before_the_port() {
    const FOLDER: &str = "~/Sites/rosas-bakery";
    let ok = "<h1>Rosa's</h1>\n";
    let limit = format!(
        "{}{}",
        format!("{}\n", "x".repeat(53)).repeat(149),
        "x".repeat(146)
    );
    assert_eq!(
        (limit.len(), limit.split_inclusive('\n').count()),
        (8192, 150)
    );
    let (long, max) = ("n".repeat(65), "m".repeat(64));
    let cases: Vec<Case> = vec![
        (
            ".x",
            ok.into(),
            FOLDER,
            Some(FOLDER),
            Some("file_name_invalid"),
        ),
        (
            ".nexus-coding-run-tmp-x",
            ok.into(),
            FOLDER,
            Some(FOLDER),
            Some("file_name_invalid"),
        ),
        (
            "a/b",
            ok.into(),
            FOLDER,
            Some(FOLDER),
            Some("file_name_invalid"),
        ),
        (
            "..",
            ok.into(),
            FOLDER,
            Some(FOLDER),
            Some("file_request_not_relative"),
        ),
        (
            long.as_str(),
            ok.into(),
            FOLDER,
            Some(FOLDER),
            Some("file_name_invalid"),
        ),
        (
            "é.txt",
            ok.into(),
            FOLDER,
            Some(FOLDER),
            Some("file_name_invalid"),
        ),
        (
            "big.txt",
            "x".repeat(8193),
            FOLDER,
            Some(FOLDER),
            Some("file_too_large"),
        ),
        (
            "lines.txt",
            "l\n".repeat(151),
            FOLDER,
            Some(FOLDER),
            Some("file_too_many_lines"),
        ),
        (
            "nul.txt",
            "a\0b\n".into(),
            FOLDER,
            Some(FOLDER),
            Some("file_not_text"),
        ),
        (
            "ok.txt",
            ok.into(),
            "rosas-bakery",
            Some(FOLDER),
            Some("file_folder_not_workspace"),
        ),
        (
            "ok.txt",
            ok.into(),
            FOLDER,
            None,
            Some("file_folder_not_workspace"),
        ),
        (
            "ok.txt",
            ok.into(),
            FOLDER,
            Some("other"),
            Some("file_folder_not_workspace"),
        ),
        ("ok.txt", limit.clone(), FOLDER, Some(FOLDER), None),
        (max.as_str(), ok.into(), FOLDER, Some(FOLDER), None),
    ];
    for (name, content, folder, workspace, code) in cases {
        let what = format!("{name:.12}/{}/{folder}/{workspace:?}", content.len());
        let builder = ScriptedLane::new(LaneRole::Builder, "writes", Verdict::Continue)
            .asking(vec![file(name, &content)]);
        let mut lanes: Vec<Arc<dyn Lane>> = vec![Arc::new(builder)];
        lanes.extend(
            super::keyed_lanes()
                .into_iter()
                .filter(|l| l.identity().role != LaneRole::Builder),
        );
        let w = world_with(lanes);
        w.port.set_workspace(workspace);
        let mut c = contract_with("");
        c.requested_folder = Some(folder.into());
        let id = w.approved(c);
        w.mother.advance(id).unwrap();
        w.mother.advance(id).unwrap();
        let step = w.mother.advance(id).unwrap();
        match code {
            Some(code) => {
                assert_eq!(step.to, MissionState::Blocked, "{what}");
                assert_eq!(w.mother.reason(id).as_deref(), Some(code), "{what}");
                assert!(w.port.effects().is_empty(), "{what}");
                assert!(w.port.admissions().is_empty(), "{what}");
            }
            None => {
                assert_eq!(w.port.effects().len(), 1, "{what}: the limits pass");
                assert_eq!(w.port.admissions().len(), 1, "{what}");
            }
        }
    }
}

// ---------------------------------------------------------------- I7

/// I7 and M3′: before a mission's first File effect, once per consent
/// lifetime, the port is asked to admit it with the consented hash, the
/// consent's own request and that lifetime (the consent's journal
/// sequence); every perform carries the same lifetime, through File A →
/// approval wait → approved → File B and a File in a later phase. A
/// mission with no File effect is never admitted.
#[test]
fn m2d_c_one_admission_per_consent_lifetime_before_the_first_file() {
    let w = world();
    let id = w.submitted(contract_with(
        "ASK-BUILDER-FILE ASK-BUILDER-FILE ASK-VERIFIER-FILE",
    ));
    let asked = Recording::default();
    assert_eq!(consent(&w.mother, id, &asked), Ok(MissionState::Approved));
    let request = asked.0.lock().unwrap()[0].request.clone();
    let lifetime = seq_of(&events(&w.mother, id), is_consent);
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.set_pending(Some(action(1)));
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    assert_eq!(
        w.mother.advance(id).unwrap().to,
        MissionState::AwaitingApproval
    );
    w.port.approval_reply(ApprovalStatus::Approved);
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Verifying);
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Explaining);
    let hash = w.mother.consented_hash(id).unwrap().to_hex();
    assert_eq!(
        w.port.admissions(),
        vec![(
            id,
            Admission {
                contract_hash: hash,
                consent_request: request,
                generation: lifetime,
            }
        )]
    );
    assert_eq!(w.port.generations(), vec![(id, lifetime); 3]);

    let network = w.approved(contract_with("ASK-BUILDER-NETWORK"));
    for _ in 0..3 {
        w.mother.advance(network).unwrap();
    }
    assert_eq!(w.port.admissions().len(), 1, "no admission without a File");
}

/// D-O15 (e): after a clean restart the coordinator holds no consent of
/// this process, so a File effect ends the mission
/// `files_consent_unverifiable`, asking the port nothing.
#[test]
fn m2d_c_a_clean_restart_leaves_the_files_consent_unverifiable() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-FILE"));
    let w = w.restart();
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    assert_eq!(
        w.mother.reason(id).as_deref(),
        Some("files_consent_unverifiable")
    );
    assert!(w.port.admissions().is_empty());
    assert!(w.port.effects().is_empty());
}

/// D-O15 (f): a mission held after an unclean shutdown, resumed by the
/// Owner's yes, is admitted with that resume's request and lifetime.
#[test]
fn m2d_c_a_held_resume_yes_admits_with_its_own_request() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-FILE"));
    let mother = Coordinator::open(
        w.journal.clone(),
        w.port.clone(),
        Lanes::new(w.lanes.clone()).unwrap(),
        w.clock.clone(),
        test_private(),
    )
    .unwrap();
    assert!(mother.held(id).is_some(), "an unclean shutdown holds it");
    let owner = HeldOwner::new(true);
    assert_eq!(resume_with(&mother, id, &owner), Ok(MissionState::Approved));
    let lifetime = seq_of(&events(&mother, id), |e| matches!(e, Event::Resumed));
    for _ in 0..3 {
        mother.advance(id).unwrap();
    }
    let admissions = w.port.admissions();
    assert_eq!(admissions.len(), 1, "{admissions:?}");
    assert_eq!(admissions[0].1.consent_request, owner.shown()[0].request);
    assert_eq!(admissions[0].1.generation, lifetime);
    assert_eq!(w.port.effects().len(), 1);
}

/// D-O15 at Mother: an admission the port refuses ends the mission before
/// any perform: no consent at the door is `files_consent_unverifiable`;
/// any other class is the broker's refusal.
#[test]
fn m2d_c_a_refused_admission_ends_the_mission_with_its_code() {
    for (class, reason) in [
        ("no_consent", "files_consent_unverifiable"),
        ("already_admitted", "broker_refused:already_admitted"),
        ("unavailable", "broker_refused:unavailable"),
    ] {
        let w = world();
        w.port.refuse_admission(class);
        let id = w.approved(contract_with("ASK-BUILDER-FILE"));
        w.mother.advance(id).unwrap();
        w.mother.advance(id).unwrap();
        assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
        assert_eq!(w.mother.reason(id).as_deref(), Some(reason), "{class}");
        assert!(w.port.effects().is_empty(), "{class}");
    }
}

// ---------------------------------------------------------------- M4′

/// M4′ at the coordinator, for every cause (the kernel write grant's
/// included, which no outside test can drive at the broker): an overtaken
/// publication is journaled as the success it is (`ActionApproved`, the
/// action closed), then the mission ends with
/// `file_publication_interrupted:<first cause>`; neither the rest of the
/// lane's answer nor any later lane runs. A cause outside the registry is
/// `unknown`, never journaled as text.
#[test]
fn m2d_c_an_overtaken_publication_is_a_success_then_the_end() {
    let cl: &'static str = Box::leak(canaries().cl.into_boxed_str());
    for (causes, code) in [
        (vec!["emergency_stop"], "emergency_stop"),
        (vec!["cancelled"], "cancelled"),
        (vec!["revoked", "write_grant_revoked"], "revoked"),
        (vec!["expired"], "expired"),
        (vec!["commitment_expired"], "commitment_expired"),
        (vec!["write_grant_revoked"], "write_grant_revoked"),
        (vec!["write_grant_expired"], "write_grant_expired"),
        (vec![cl], "unknown"),
    ] {
        let w = world();
        let id = w.approved(contract_with("ASK-BUILDER-FILE ASK-BUILDER-FILE"));
        w.mother.advance(id).unwrap();
        w.mother.advance(id).unwrap();
        w.port.set_pending(Some(action(1)));
        w.port.reply(Ok(EffectOutcome::AwaitingApproval));
        w.mother.advance(id).unwrap();
        let calls = w.keyed_calls();
        w.port.approval_reply(ApprovalStatus::Overtaken {
            causes: causes.clone(),
        });
        let step = w.mother.advance(id).unwrap();
        assert_eq!(step.to, MissionState::Blocked, "{code}");
        let reason = format!("file_publication_interrupted:{code}");
        assert_eq!(w.mother.reason(id), Some(reason), "{code}");
        let tail: Vec<Event> = events(&w.mother, id)
            .into_iter()
            .map(|(_, e)| e)
            .skip_while(|e| {
                !matches!(
                    e,
                    Event::Transition {
                        trigger: Trigger::ActionApproved { .. }
                    }
                )
            })
            .collect();
        assert!(
            matches!(
                tail.as_slice(),
                [
                    Event::Transition {
                        trigger: Trigger::ActionApproved { .. }
                    },
                    Event::ApprovalClosed { .. },
                    Event::Transition {
                        trigger: Trigger::Block {
                            reason: BlockReason::FilePublicationInterrupted(_)
                        }
                    },
                ]
            ),
            "{code}: {tail:?}"
        );
        assert_eq!(w.port.effects().len(), 1, "{code}: the rest never sent");
        assert_eq!(w.keyed_calls(), calls, "{code}: no later lane");
        let journal = format!("{:?}", w.mother.journal().unwrap());
        assert!(!journal.contains(cl), "{code}");
    }
}

/// The port's reading of the broker's record: a Succeeded commitment with
/// interruption codes is Overtaken (its codes, in order); without, it is
/// Approved.
#[test]
fn m2d_c_the_port_reads_an_interrupted_success_as_overtaken() {
    let view = |interrupted: Vec<&'static str>| CommitmentView {
        id: CommitmentId::parse("cmt-00000000000000000000000000000001").unwrap(),
        agent: AgentId::new("mother").unwrap(),
        run: RunId::parse("run-00000000000000000000000000000001").unwrap(),
        kind: CapabilityKind::Files,
        class: EffectClass::R2,
        operation: "create",
        target: "index.html in workspace wsp-0".into(),
        summary: vec![],
        state: CommitmentState::Succeeded,
        requires_approval: true,
        created_wall_ms: 1_000,
        expires_wall_ms: 601_000,
        binding_short: "0000".into(),
        interrupted,
        recovery_required: false,
    };
    assert_eq!(
        status_of(&view(vec![]), 2_000, 2_000, 600_000),
        ApprovalStatus::Approved
    );
    assert_eq!(
        status_of(
            &view(vec!["revoked", "write_grant_revoked"]),
            2_000,
            2_000,
            600_000
        ),
        ApprovalStatus::Overtaken {
            causes: vec!["revoked", "write_grant_revoked"]
        }
    );
}
