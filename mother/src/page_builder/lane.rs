//! Private direct lane; production host/session selection remains disabled.
use crate::lane::{Lane, LaneContext, LaneIdentity, LaneInput, LaneOutput, LaneRole, Verdict};
use nexus_governed_control::authority::ids::MarkerUnavailable;

pub(crate) struct PageBuilderLane;
impl Lane for PageBuilderLane {
    fn max_model_calls(&self) -> u32 {
        0
    }
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Builder,
            name: "inert".into(),
            version: "inert".into(),
        }
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        failure("assembler_fault")
    }
    fn invoke_in(&self, _: &LaneInput, _: &LaneContext<'_>) -> LaneOutput {
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
    }
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
