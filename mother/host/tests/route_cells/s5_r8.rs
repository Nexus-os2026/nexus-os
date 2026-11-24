//! SB-3E S5 section U20c: s5_r8.rs
// SB3E-R243: the R8 rows, harness composition until M-5b (R216 F1; the
// prelock's production order is report.rs `admission_order`): the twelve
// lane_result_admission rows (x:316-321, 324, 327, 625-628), each value typed
// into production's reader or applied while the Builder's own prelock is held
// (`d_lane`), post_record_recheck repeated (x:371; credited only with the call
// result `Err(Invalidated)` bound, finding D) and record_first_classify
// repeated (x:399), and the three controls. C1 names the advance's last step,
// the mission's end and the Builder's own call result. Open for M-5b: the
// byte rows x:366-368, 394-396 (finding C); for GB4b: x:322, 369, 397.
use super::{
    s5::{after_chat, d_lane, Facts},
    Fixture,
};
const LANE: (&str, &str) = ("lane_result_admission", "ingress");
const LANE_L: (&str, &str) = ("lane_result_admission", "lifecycle");
const RECHECK: (&str, &str) = ("post_record_recheck", "ingress");
const CLASSIFY: (&str, &str) = ("record_first_classify", "ingress");
/// C1, measured (Step 0 section 6): the advance, the mission's end, the call.
/// The refused admission journals nothing, and the lane's call had returned.
const REFUSED: [&str; 3] = ["Err(Stopped)", "none", "Ok(())"];
const STOPPED: [&str; 3] = [
    "Err(Stopped)",
    "Stop { reason: EmergencyStop }",
    "Err(Invalidated)",
];
const PASSED: [&str; 3] = ["Ok(())", "none", "Ok(())"];
pub(super) fn lane_cr(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("CR", Some("stop0d")), REFUSED)
}
pub(super) fn lane_lf(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("LF", Some("stop0a")), REFUSED)
}
pub(super) fn lane_crlf(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("CRLF", Some("stop0d0a")), REFUSED)
}
pub(super) fn lane_eof(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("EOF", Some("04")), REFUSED)
}
pub(super) fn lane_partial_ctrl_d(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("partial_Ctrl_D", Some("partial+04")), REFUSED)
}
pub(super) fn lane_master_close(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("master_close", Some("master_close")), REFUSED)
}
pub(super) fn lane_repeated(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("repeated", None), REFUSED)
}
pub(super) fn lane_quit(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("quit", None), REFUSED)
}
pub(super) fn lane_emergency(f: &Fixture) -> Facts {
    d_lane(f, LANE_L, ("Emergency", None), REFUSED)
}
pub(super) fn lane_failed(f: &Fixture) -> Facts {
    d_lane(f, LANE_L, ("Failed", None), REFUSED)
}
pub(super) fn lane_closingnormal(f: &Fixture) -> Facts {
    d_lane(f, LANE_L, ("ClosingNormal", None), REFUSED)
}
pub(super) fn lane_closingrestricted(f: &Fixture) -> Facts {
    d_lane(f, LANE_L, ("ClosingRestricted", None), REFUSED)
}
pub(super) fn ctl_lane_result_admission(f: &Fixture) -> Facts {
    d_lane(f, LANE, ("none", None), PASSED)
}
pub(super) fn recheck_repeated(f: &Fixture) -> Facts {
    after_chat(f, RECHECK, "repeated", STOPPED)
}
pub(super) fn ctl_post_record_recheck(f: &Fixture) -> Facts {
    after_chat(f, RECHECK, "none", PASSED)
}
pub(super) fn classify_repeated(f: &Fixture) -> Facts {
    after_chat(f, CLASSIFY, "repeated", STOPPED)
}
pub(super) fn ctl_record_first_classify(f: &Fixture) -> Facts {
    after_chat(f, CLASSIFY, "none", PASSED)
}
