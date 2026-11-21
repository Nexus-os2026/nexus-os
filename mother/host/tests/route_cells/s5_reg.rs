//! SB-3E S5 section U6a-1: s5_reg.rs
// SB3E-R120, R125: the three credited registration_record ingress rows (437,
// 439, 440) and their control, each one call of s5's `registered` with its
// row's literals; s5 holds the point, applies the value and collects.
// SEAM-1 (SB3E-R139): the registration_reserve rows 449, 451, 452 and the
// registration_readiness rows 425, 427, 428 and their controls, the same way.
// U19a (SB3E-R196, R198): each call names the sink result it must return
// (C1, SB3E-R194); the lifecycle rows reached while held; the byte rows typed
// into production's reader through `read_registered`.
use super::{
    s5::{read_registered, registered, Facts},
    Fixture,
};
const RESERVE: (&str, &str) = ("registration_reserve", "ingress");
const RECORD: (&str, &str) = ("registration_record", "ingress");
const READY: (&str, &str) = ("registration_readiness", "ingress");
const RESERVE_L: (&str, &str) = ("registration_reserve", "lifecycle");
const RECORD_L: (&str, &str) = ("registration_record", "lifecycle");
const READY_L: (&str, &str) = ("registration_readiness", "lifecycle");
/// The refused registrations (model.rs:142, host_session.rs:865).
const REFUSED: &[Result<(), &str>] = &[Err("Restricted")];
/// The record written, then the install refused (model.rs:315).
const CANCELLED: &[Result<(), &str>] = &[Err("Backend(\"model_cancelled\")")];
/// Step 0 F-S0-3: the reader's bound Stop races the install: either refusal.
const STOPPED: &[Result<(), &str>] = &[
    Err("Backend(\"model_stopped\")"),
    Err("Backend(\"model_cancelled\")"),
];
const PASSED: &[Result<(), &str>] = &[Ok(())];
/// Row 437: `emergency()` twice while the record is held.
pub(super) fn record_repeated(f: &Fixture) -> Facts {
    registered(f, RECORD, "repeated", CANCELLED)
}
/// Row 439: `fail()` while the record is held.
pub(super) fn record_fatal(f: &Fixture) -> Facts {
    registered(f, RECORD, "fatal", CANCELLED)
}
/// Row 440: `accept(Quit, 1)` while the record is held.
pub(super) fn record_quit(f: &Fixture) -> Facts {
    registered(f, RECORD, "quit", CANCELLED)
}
/// The control: held at the record and released with no value.
pub(super) fn ctl_registration_record(f: &Fixture) -> Facts {
    registered(f, RECORD, "none", PASSED)
}
/// Row 449: `emergency()` twice while `reg.reserve` is held.
pub(super) fn reserve_repeated(f: &Fixture) -> Facts {
    registered(f, RESERVE, "repeated", REFUSED)
}
/// Row 451: `fail()` while `reg.reserve` is held.
pub(super) fn reserve_fatal(f: &Fixture) -> Facts {
    registered(f, RESERVE, "fatal", REFUSED)
}
/// Row 452: `accept(Quit, 1)` while `reg.reserve` is held.
pub(super) fn reserve_quit(f: &Fixture) -> Facts {
    registered(f, RESERVE, "quit", REFUSED)
}
/// The control: held at `reg.reserve` and released with no value.
pub(super) fn ctl_registration_reserve(f: &Fixture) -> Facts {
    registered(f, RESERVE, "none", PASSED)
}
/// Row 425: `emergency()` twice while `reg.ready` is held.
pub(super) fn readiness_repeated(f: &Fixture) -> Facts {
    registered(f, READY, "repeated", REFUSED)
}
/// Row 427: `fail()` while `reg.ready` is held.
pub(super) fn readiness_fatal(f: &Fixture) -> Facts {
    registered(f, READY, "fatal", REFUSED)
}
/// Row 428: `accept(Quit, 1)` while `reg.ready` is held.
pub(super) fn readiness_quit(f: &Fixture) -> Facts {
    registered(f, READY, "quit", REFUSED)
}
/// The control: held at `reg.ready` and released with no value.
pub(super) fn ctl_registration_readiness(f: &Fixture) -> Facts {
    registered(f, READY, "none", PASSED)
}
/// Rows 673-676: each state reached while `reg.reserve` is held.
pub(super) fn reserve_emergency(f: &Fixture) -> Facts {
    registered(f, RESERVE_L, "Emergency", REFUSED)
}
pub(super) fn reserve_failed(f: &Fixture) -> Facts {
    registered(f, RESERVE_L, "Failed", REFUSED)
}
pub(super) fn reserve_closingnormal(f: &Fixture) -> Facts {
    registered(f, RESERVE_L, "ClosingNormal", REFUSED)
}
pub(super) fn reserve_closingrestricted(f: &Fixture) -> Facts {
    registered(f, RESERVE_L, "ClosingRestricted", REFUSED)
}
/// Rows 667-670: each state reached while the record is held.
pub(super) fn record_emergency(f: &Fixture) -> Facts {
    registered(f, RECORD_L, "Emergency", CANCELLED)
}
pub(super) fn record_failed(f: &Fixture) -> Facts {
    registered(f, RECORD_L, "Failed", CANCELLED)
}
pub(super) fn record_closingnormal(f: &Fixture) -> Facts {
    registered(f, RECORD_L, "ClosingNormal", CANCELLED)
}
pub(super) fn record_closingrestricted(f: &Fixture) -> Facts {
    registered(f, RECORD_L, "ClosingRestricted", CANCELLED)
}
/// Rows 665, 659-661: each state reached while `reg.ready` is held.
pub(super) fn readiness_emergency(f: &Fixture) -> Facts {
    registered(f, READY_L, "Emergency", REFUSED)
}
pub(super) fn readiness_failed(f: &Fixture) -> Facts {
    registered(f, READY_L, "Failed", REFUSED)
}
pub(super) fn readiness_closingnormal(f: &Fixture) -> Facts {
    registered(f, READY_L, "ClosingNormal", REFUSED)
}
pub(super) fn readiness_closingrestricted(f: &Fixture) -> Facts {
    registered(f, READY_L, "ClosingRestricted", REFUSED)
}
/// Rows 429-434: each byte typed while the record is held (`stop` and a
/// terminator for CR, LF and CRLF, SB3E-R196.2).
pub(super) fn record_cr(f: &Fixture) -> Facts {
    read_registered(f, RECORD, ("CR", "stop0d"), STOPPED)
}
pub(super) fn record_lf(f: &Fixture) -> Facts {
    read_registered(f, RECORD, ("LF", "stop0a"), STOPPED)
}
pub(super) fn record_crlf(f: &Fixture) -> Facts {
    read_registered(f, RECORD, ("CRLF", "stop0d0a"), STOPPED)
}
pub(super) fn record_eof(f: &Fixture) -> Facts {
    read_registered(f, RECORD, ("EOF", "04"), STOPPED)
}
pub(super) fn record_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, RECORD, ("partial_Ctrl_D", "partial+04"), STOPPED)
}
pub(super) fn record_master_close(f: &Fixture) -> Facts {
    read_registered(f, RECORD, ("master_close", "master_close"), STOPPED)
}
/// TR-1 rows 441-446 (SB3E-R198): each byte typed while `reg.reserve` is held.
pub(super) fn reserve_cr(f: &Fixture) -> Facts {
    read_registered(f, RESERVE, ("CR", "stop0d"), REFUSED)
}
pub(super) fn reserve_lf(f: &Fixture) -> Facts {
    read_registered(f, RESERVE, ("LF", "stop0a"), REFUSED)
}
pub(super) fn reserve_crlf(f: &Fixture) -> Facts {
    read_registered(f, RESERVE, ("CRLF", "stop0d0a"), REFUSED)
}
pub(super) fn reserve_eof(f: &Fixture) -> Facts {
    read_registered(f, RESERVE, ("EOF", "04"), REFUSED)
}
pub(super) fn reserve_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, RESERVE, ("partial_Ctrl_D", "partial+04"), REFUSED)
}
pub(super) fn reserve_master_close(f: &Fixture) -> Facts {
    read_registered(f, RESERVE, ("master_close", "master_close"), REFUSED)
}
/// TR-1 rows 417-422 (SB3E-R198): each byte typed while `reg.ready` is held.
pub(super) fn readiness_cr(f: &Fixture) -> Facts {
    read_registered(f, READY, ("CR", "stop0d"), REFUSED)
}
pub(super) fn readiness_lf(f: &Fixture) -> Facts {
    read_registered(f, READY, ("LF", "stop0a"), REFUSED)
}
pub(super) fn readiness_crlf(f: &Fixture) -> Facts {
    read_registered(f, READY, ("CRLF", "stop0d0a"), REFUSED)
}
pub(super) fn readiness_eof(f: &Fixture) -> Facts {
    read_registered(f, READY, ("EOF", "04"), REFUSED)
}
pub(super) fn readiness_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, READY, ("partial_Ctrl_D", "partial+04"), REFUSED)
}
pub(super) fn readiness_master_close(f: &Fixture) -> Facts {
    read_registered(f, READY, ("master_close", "master_close"), REFUSED)
}
