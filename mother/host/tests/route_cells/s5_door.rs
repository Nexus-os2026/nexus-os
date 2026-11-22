//! SB-3E S5 section U20b: s5_door.rs
// SB3E-R226: the twelve credited R6 cancel_run rows (x:171-176, 179, 184, 562-565)
// and their control, each one call of s5's `d_cancel` with its row's literals.
use super::{
    s5::{d_cancel, Facts},
    Fixture,
};

const INGRESS: (&str, &str) = ("cancel_run", "ingress");
const LIFECYCLE: (&str, &str) = ("cancel_run", "lifecycle");
pub(super) fn cancel_cr(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "CR", Some("stop0d"))
}
pub(super) fn cancel_lf(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "LF", Some("stop0a"))
}
pub(super) fn cancel_crlf(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "CRLF", Some("stop0d0a"))
}
pub(super) fn cancel_eof(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "EOF", Some("04"))
}
pub(super) fn cancel_partial_ctrl_d(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "partial_Ctrl_D", Some("partial+04"))
}
pub(super) fn cancel_master_close(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "master_close", Some("master_close"))
}
pub(super) fn cancel_repeated(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "repeated", None)
}
pub(super) fn cancel_quit(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "quit", None)
}
pub(super) fn cancel_emergency(f: &Fixture) -> Facts {
    d_cancel(f, LIFECYCLE, "Emergency", None)
}
pub(super) fn cancel_failed(f: &Fixture) -> Facts {
    d_cancel(f, LIFECYCLE, "Failed", None)
}
pub(super) fn cancel_closingnormal(f: &Fixture) -> Facts {
    d_cancel(f, LIFECYCLE, "ClosingNormal", None)
}
pub(super) fn cancel_closingrestricted(f: &Fixture) -> Facts {
    d_cancel(f, LIFECYCLE, "ClosingRestricted", None)
}
pub(super) fn ctl_cancel_run(f: &Fixture) -> Facts {
    d_cancel(f, INGRESS, "none", None)
}
