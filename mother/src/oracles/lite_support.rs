//! Lite real-broker oracle helpers; reused from the audited #78 fixtures.
use crate::effect::EffectRequest;
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, Verdict};
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use nexus_governed_control::files::FileCreate;
use std::sync::{Arc, Mutex};
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
        true
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
    EffectRequest::CreateFile(FileCreate {
        name: name.into(),
        content: content.into(),
    })
}
