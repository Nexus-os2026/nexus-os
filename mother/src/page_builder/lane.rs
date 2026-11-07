//! Private direct lane; production host/session selection remains disabled.
use super::{prompt, BuildInputCode, BuildStep, Builder};
use crate::contract::ContractError;
use crate::lane::{
    Lane, LaneContext, LaneIdentity, LaneInput, LaneOutput, LaneRole, ModelCallError, Verdict,
};
use nexus_governed_control::{
    authority::ids::MarkerUnavailable,
    inference::{ModelError, ModelRequest, ResponseSchema},
};

pub(crate) struct PageBuilderLane;
impl Lane for PageBuilderLane {
    fn max_model_calls(&self) -> u32 {
        2
    }
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Builder,
            name: "page-builder".into(),
            version: "m3-1".into(),
        }
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        if input.role != LaneRole::Builder {
            return failure("page_input_identity");
        }
        failure("model_caller_unavailable")
    }
    fn invoke_in(&self, input: &LaneInput, context: &LaneContext<'_>) -> LaneOutput {
        if input.role != LaneRole::Builder {
            return failure("page_input_identity");
        }
        let Some(caller) = context.model else {
            return failure("model_caller_unavailable");
        };
        let (Some(contract), Some(payload)) = (context.contract, input.builder_input.as_ref())
        else {
            return failure("page_input_identity");
        };
        if let Err(error) = payload.validate(contract, input.contract_hash, input.invocation) {
            return failure(match error {
                ContractError::PageInputIdentity => "page_input_identity",
                ContractError::FactsNotPageReady => "facts_not_page_ready",
                _ => "page_input_invalid",
            });
        }
        let (note, values) = payload.borrowed();
        let facts = match values.approved() {
            Ok(facts) if super::facts_page_ready(&facts) => facts,
            _ => return failure("facts_not_page_ready"),
        };
        let mut builder = Builder::default();
        let mut previous = None;
        let mut correction = String::new();
        for _ in 0..2 {
            let current = match marker() {
                Ok(m) if marker_valid(&m) && previous.as_ref() != Some(&m) => m,
                _ => return failure("model_marker_unavailable"),
            };
            let mut user = match prompt::render_user(&facts, note, &current, None) {
                Ok(user) => user,
                Err(e) => return failure(input_code(e)),
            };
            user.push_str(&correction);
            #[cfg(test)]
            if let Some(bytes) = SIZE.with(|s| s.take()) {
                user = "x".repeat(bytes.saturating_sub(prompt::SYSTEM_M3.len()));
            }
            let schema = match ResponseSchema::new(prompt::SCHEMA) {
                Ok(schema) => schema,
                Err(_) => return failure("assembler_fault"),
            };
            let request = match ModelRequest::new(prompt::SYSTEM_M3.into(), user, schema) {
                Ok(request) => request,
                Err(ModelError::RequestTooLarge) => return failure("model_request_too_large"),
                Err(_) => return failure("assembler_fault"),
            };
            let reply = match caller.call(&request) {
                Ok(reply) => reply,
                Err(error) => {
                    return failure(match error {
                        ModelCallError::Invalidated => "model_invalidated",
                        ModelCallError::RecordUnavailable => "model_record_unavailable",
                        ModelCallError::Refused(c) | ModelCallError::Failed(c) => c.code(),
                    })
                }
            };
            match builder.answer(reply.content(), &facts) {
                BuildStep::File(effects) => {
                    return LaneOutput {
                        verdict: Verdict::Continue,
                        text: String::new(),
                        effects,
                        verification: None,
                    }
                }
                BuildStep::Failed(code) => return failure(code.failure()),
                BuildStep::Closed => return failure("assembler_fault"),
                BuildStep::Correction(block) => {
                    previous = Some(current);
                    correction = block;
                }
            }
        }
        failure("assembler_fault")
    }
}
fn failure(code: &str) -> LaneOutput {
    LaneOutput {
        verdict: Verdict::Fail {
            reason: code.into(),
        },
        text: String::new(),
        effects: vec![],
        verification: None,
    }
}
fn input_code(code: BuildInputCode) -> &'static str {
    match code {
        BuildInputCode::FactsNotPageReady => "facts_not_page_ready",
        BuildInputCode::InvalidMarker => "model_marker_unavailable",
        BuildInputCode::InvalidNote | BuildInputCode::NotesAndFactsTooLarge => "page_input_invalid",
        BuildInputCode::RequestTooLarge => "model_request_too_large",
        BuildInputCode::Internal => "assembler_fault",
    }
}
fn marker_valid(m: &str) -> bool {
    m.len() == 32
        && m.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn marker() -> Result<String, MarkerUnavailable> {
    #[cfg(test)]
    if let Some(result) = MARKERS.with(|m| m.borrow_mut().pop_front()) {
        return result;
    }
    nexus_governed_control::authority::ids::fresh_page_marker()
}
#[cfg(test)]
thread_local! {
    static MARKERS: std::cell::RefCell<std::collections::VecDeque<Result<String, MarkerUnavailable>>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
    static SIZE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(crate) fn markers_for_test(values: Vec<Result<String, MarkerUnavailable>>) {
    MARKERS.with(|m| *m.borrow_mut() = values.into());
}
#[cfg(test)]
pub(crate) fn size_for_test(bytes: usize) {
    SIZE.with(|s| s.set(Some(bytes)));
}
