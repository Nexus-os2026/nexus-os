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
    s5::{after_chat, d_lane, Facts},
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
// CELL-R-A (SB3E-R265): x:35-40, 52-57 and 349-354, typed while held (`d_lane`),
// under the Stop-first barrier (sched.rs).
pub(super) fn pin_cr(f: &Fixture) -> Facts {
    d_lane(f, PIN, ("CR", Some("stop0d")), STOPPED)
}
pub(super) fn pin_lf(f: &Fixture) -> Facts {
    d_lane(f, PIN, ("LF", Some("stop0a")), STOPPED)
}
pub(super) fn pin_crlf(f: &Fixture) -> Facts {
    d_lane(f, PIN, ("CRLF", Some("stop0d0a")), STOPPED)
}
pub(super) fn pin_eof(f: &Fixture) -> Facts {
    d_lane(f, PIN, ("EOF", Some("04")), STOPPED)
}
pub(super) fn pin_partial_ctrl_d(f: &Fixture) -> Facts {
    d_lane(f, PIN, ("partial_Ctrl_D", Some("partial+04")), STOPPED)
}
pub(super) fn pin_master_close(f: &Fixture) -> Facts {
    d_lane(f, PIN, ("master_close", Some("master_close")), STOPPED)
}
pub(super) fn record_cr(f: &Fixture) -> Facts {
    d_lane(f, RECORD, ("CR", Some("stop0d")), STOPPED)
}
pub(super) fn record_lf(f: &Fixture) -> Facts {
    d_lane(f, RECORD, ("LF", Some("stop0a")), STOPPED)
}
pub(super) fn record_crlf(f: &Fixture) -> Facts {
    d_lane(f, RECORD, ("CRLF", Some("stop0d0a")), STOPPED)
}
pub(super) fn record_eof(f: &Fixture) -> Facts {
    d_lane(f, RECORD, ("EOF", Some("04")), STOPPED)
}
pub(super) fn record_partial_ctrl_d(f: &Fixture) -> Facts {
    d_lane(f, RECORD, ("partial_Ctrl_D", Some("partial+04")), STOPPED)
}
pub(super) fn record_master_close(f: &Fixture) -> Facts {
    d_lane(f, RECORD, ("master_close", Some("master_close")), STOPPED)
}
pub(super) fn complete_cr(f: &Fixture) -> Facts {
    d_lane(f, COMPLETE, ("CR", Some("stop0d")), STOPPED)
}
pub(super) fn complete_lf(f: &Fixture) -> Facts {
    d_lane(f, COMPLETE, ("LF", Some("stop0a")), STOPPED)
}
pub(super) fn complete_crlf(f: &Fixture) -> Facts {
    d_lane(f, COMPLETE, ("CRLF", Some("stop0d0a")), STOPPED)
}
pub(super) fn complete_eof(f: &Fixture) -> Facts {
    d_lane(f, COMPLETE, ("EOF", Some("04")), STOPPED)
}
pub(super) fn complete_partial_ctrl_d(f: &Fixture) -> Facts {
    d_lane(f, COMPLETE, ("partial_Ctrl_D", Some("partial+04")), STOPPED)
}
pub(super) fn complete_master_close(f: &Fixture) -> Facts {
    d_lane(f, COMPLETE, ("master_close", Some("master_close")), STOPPED)
}
