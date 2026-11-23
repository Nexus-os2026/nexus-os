//! SB-3E S5 section U20c: s5_r7.rs
// SB3E-R243: the R7 rows credited past the chat, harness composition until
// M-5b (R216 F1): attempt_record (x:60, 65, 524-527), attempt_pin_release
// (x:43, 48, 519-522) and operation_complete (x:357, 362, 636-638), each
// value applied by the harness while the sink's own point is held, and the
// three controls, held and released. Each is one call of s5's `after_chat`,
// which names the advance's last step, the mission's end and the Builder's
// own call result (C1). Open for M-5b: the byte rows x:52-57 (finding H) and
// x:35-40, 349-354 (finding C); for GB4b: read_failure x:41, 58, 355.
use super::{
    s5::{after_chat, Facts},
    Fixture,
};
const RECORD: (&str, &str) = ("attempt_record", "ingress");
const RECORD_L: (&str, &str) = ("attempt_record", "lifecycle");
const PIN: (&str, &str) = ("attempt_pin_release", "ingress");
const PIN_L: (&str, &str) = ("attempt_pin_release", "lifecycle");
const COMPLETE: (&str, &str) = ("operation_complete", "ingress");
const COMPLETE_L: (&str, &str) = ("operation_complete", "lifecycle");
/// C1, measured (Step 0 section 6): the advance, the mission's end, the call.
const INVALID: &str = "Err(Invalidated)";
const STOPPED: [&str; 3] = ["Err(Stopped)", "Stop { reason: EmergencyStop }", INVALID];
const OWNER: [&str; 3] = ["Err(Stopped)", "Stop { reason: Owner }", INVALID];
const PASSED: [&str; 3] = ["Ok(())", "none", "Ok(())"];
pub(super) fn record_repeated(f: &Fixture) -> Facts {
    after_chat(f, RECORD, "repeated", STOPPED)
}
pub(super) fn record_quit(f: &Fixture) -> Facts {
    after_chat(f, RECORD, "quit", OWNER)
}
pub(super) fn record_emergency(f: &Fixture) -> Facts {
    after_chat(f, RECORD_L, "Emergency", STOPPED)
}
pub(super) fn record_failed(f: &Fixture) -> Facts {
    after_chat(f, RECORD_L, "Failed", OWNER)
}
pub(super) fn record_closingnormal(f: &Fixture) -> Facts {
    after_chat(f, RECORD_L, "ClosingNormal", OWNER)
}
pub(super) fn record_closingrestricted(f: &Fixture) -> Facts {
    after_chat(f, RECORD_L, "ClosingRestricted", STOPPED)
}
pub(super) fn ctl_attempt_record(f: &Fixture) -> Facts {
    after_chat(f, RECORD, "none", PASSED)
}
pub(super) fn pin_repeated(f: &Fixture) -> Facts {
    after_chat(f, PIN, "repeated", STOPPED)
}
pub(super) fn pin_quit(f: &Fixture) -> Facts {
    after_chat(f, PIN, "quit", OWNER)
}
pub(super) fn pin_emergency(f: &Fixture) -> Facts {
    after_chat(f, PIN_L, "Emergency", STOPPED)
}
pub(super) fn pin_failed(f: &Fixture) -> Facts {
    after_chat(f, PIN_L, "Failed", OWNER)
}
pub(super) fn pin_closingnormal(f: &Fixture) -> Facts {
    after_chat(f, PIN_L, "ClosingNormal", OWNER)
}
pub(super) fn pin_closingrestricted(f: &Fixture) -> Facts {
    after_chat(f, PIN_L, "ClosingRestricted", STOPPED)
}
pub(super) fn ctl_attempt_pin_release(f: &Fixture) -> Facts {
    after_chat(f, PIN, "none", PASSED)
}
pub(super) fn complete_repeated(f: &Fixture) -> Facts {
    after_chat(f, COMPLETE, "repeated", STOPPED)
}
pub(super) fn complete_quit(f: &Fixture) -> Facts {
    after_chat(f, COMPLETE, "quit", OWNER)
}
pub(super) fn complete_emergency(f: &Fixture) -> Facts {
    after_chat(f, COMPLETE_L, "Emergency", STOPPED)
}
pub(super) fn complete_failed(f: &Fixture) -> Facts {
    after_chat(f, COMPLETE_L, "Failed", OWNER)
}
pub(super) fn complete_closingrestricted(f: &Fixture) -> Facts {
    after_chat(f, COMPLETE_L, "ClosingRestricted", STOPPED)
}
pub(super) fn ctl_operation_complete(f: &Fixture) -> Facts {
    after_chat(f, COMPLETE, "none", PASSED)
}
