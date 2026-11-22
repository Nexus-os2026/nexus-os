//! SB-3E S5 section U6a-1: s5_reg.rs
// SB3E-R120, R125: the three credited registration_record ingress rows (437,
// 439, 440) and their control, each one call of s5's `registered` with its
// row's literals; s5 holds the point, applies the value and collects.
// SEAM-1 (SB3E-R139): the registration_reserve rows 449, 451, 452 and the
// registration_readiness rows 425, 427, 428 and their controls, the same way.
// U19a (SB3E-R196, R198): each call names its sink result (C1); lifecycle rows
// 673-676, 667-670, 665, 659-661 held; byte rows 429-434, 441-446, 417-422 typed.
// U19b-1 (SB3E-R216): registration_install rows 413, 416, 653-656 held and
// 405-410 typed; the metadata-gate rows 453-500 (B1 and B2i); their controls.
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
const INSTALL: (&str, &str) = ("registration_install", "ingress");
const INSTALL_L: (&str, &str) = ("registration_install", "lifecycle");
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
pub(super) fn install_repeated(f: &Fixture) -> Facts {
    registered(f, INSTALL, "repeated", CANCELLED)
}
pub(super) fn install_quit(f: &Fixture) -> Facts {
    registered(f, INSTALL, "quit", CANCELLED)
}
pub(super) fn install_emergency(f: &Fixture) -> Facts {
    registered(f, INSTALL_L, "Emergency", CANCELLED)
}
pub(super) fn install_failed(f: &Fixture) -> Facts {
    registered(f, INSTALL_L, "Failed", CANCELLED)
}
pub(super) fn install_closingnormal(f: &Fixture) -> Facts {
    registered(f, INSTALL_L, "ClosingNormal", CANCELLED)
}
pub(super) fn install_closingrestricted(f: &Fixture) -> Facts {
    registered(f, INSTALL_L, "ClosingRestricted", CANCELLED)
}
/// The control: held at the install and released with no value.
pub(super) fn ctl_registration_install(f: &Fixture) -> Facts {
    registered(f, INSTALL, "none", PASSED)
}
pub(super) fn install_cr(f: &Fixture) -> Facts {
    read_registered(f, INSTALL, ("CR", "stop0d"), STOPPED)
}
pub(super) fn install_lf(f: &Fixture) -> Facts {
    read_registered(f, INSTALL, ("LF", "stop0a"), STOPPED)
}
pub(super) fn install_crlf(f: &Fixture) -> Facts {
    read_registered(f, INSTALL, ("CRLF", "stop0d0a"), STOPPED)
}
pub(super) fn install_eof(f: &Fixture) -> Facts {
    read_registered(f, INSTALL, ("EOF", "04"), STOPPED)
}
pub(super) fn install_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, INSTALL, ("partial_Ctrl_D", "partial+04"), STOPPED)
}
pub(super) fn install_master_close(f: &Fixture) -> Facts {
    read_registered(f, INSTALL, ("master_close", "master_close"), STOPPED)
}
// U19b-1 (SB3E-R216): the metadata-gate rows of registration_version,
// _status, _tags and _show, each held at its own ord of `model.gate.checked`
// (s5 `REG_GATES`); the CAS refuses with `model_cancelled` (model.rs:244-248).
const VERSION: (&str, &str) = ("registration_version", "ingress");
const STATUS: (&str, &str) = ("registration_status", "ingress");
const TAGS: (&str, &str) = ("registration_tags", "ingress");
const SHOW: (&str, &str) = ("registration_show", "ingress");
pub(super) fn version_cr(f: &Fixture) -> Facts {
    read_registered(f, VERSION, ("CR", "stop0d"), CANCELLED)
}
pub(super) fn version_lf(f: &Fixture) -> Facts {
    read_registered(f, VERSION, ("LF", "stop0a"), CANCELLED)
}
pub(super) fn version_crlf(f: &Fixture) -> Facts {
    read_registered(f, VERSION, ("CRLF", "stop0d0a"), CANCELLED)
}
pub(super) fn version_eof(f: &Fixture) -> Facts {
    read_registered(f, VERSION, ("EOF", "04"), CANCELLED)
}
pub(super) fn version_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, VERSION, ("partial_Ctrl_D", "partial+04"), CANCELLED)
}
pub(super) fn version_master_close(f: &Fixture) -> Facts {
    read_registered(f, VERSION, ("master_close", "master_close"), CANCELLED)
}
pub(super) fn version_repeated(f: &Fixture) -> Facts {
    registered(f, VERSION, "repeated", CANCELLED)
}
pub(super) fn version_quit(f: &Fixture) -> Facts {
    registered(f, VERSION, "quit", CANCELLED)
}
pub(super) fn ctl_registration_version(f: &Fixture) -> Facts {
    registered(f, VERSION, "none", PASSED)
}
pub(super) fn status_cr(f: &Fixture) -> Facts {
    read_registered(f, STATUS, ("CR", "stop0d"), CANCELLED)
}
pub(super) fn status_lf(f: &Fixture) -> Facts {
    read_registered(f, STATUS, ("LF", "stop0a"), CANCELLED)
}
pub(super) fn status_crlf(f: &Fixture) -> Facts {
    read_registered(f, STATUS, ("CRLF", "stop0d0a"), CANCELLED)
}
pub(super) fn status_eof(f: &Fixture) -> Facts {
    read_registered(f, STATUS, ("EOF", "04"), CANCELLED)
}
pub(super) fn status_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, STATUS, ("partial_Ctrl_D", "partial+04"), CANCELLED)
}
pub(super) fn status_master_close(f: &Fixture) -> Facts {
    read_registered(f, STATUS, ("master_close", "master_close"), CANCELLED)
}
pub(super) fn status_repeated(f: &Fixture) -> Facts {
    registered(f, STATUS, "repeated", CANCELLED)
}
pub(super) fn status_quit(f: &Fixture) -> Facts {
    registered(f, STATUS, "quit", CANCELLED)
}
pub(super) fn ctl_registration_status(f: &Fixture) -> Facts {
    registered(f, STATUS, "none", PASSED)
}
pub(super) fn tags_cr(f: &Fixture) -> Facts {
    read_registered(f, TAGS, ("CR", "stop0d"), CANCELLED)
}
pub(super) fn tags_lf(f: &Fixture) -> Facts {
    read_registered(f, TAGS, ("LF", "stop0a"), CANCELLED)
}
pub(super) fn tags_crlf(f: &Fixture) -> Facts {
    read_registered(f, TAGS, ("CRLF", "stop0d0a"), CANCELLED)
}
pub(super) fn tags_eof(f: &Fixture) -> Facts {
    read_registered(f, TAGS, ("EOF", "04"), CANCELLED)
}
pub(super) fn tags_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, TAGS, ("partial_Ctrl_D", "partial+04"), CANCELLED)
}
pub(super) fn tags_master_close(f: &Fixture) -> Facts {
    read_registered(f, TAGS, ("master_close", "master_close"), CANCELLED)
}
pub(super) fn tags_repeated(f: &Fixture) -> Facts {
    registered(f, TAGS, "repeated", CANCELLED)
}
pub(super) fn tags_quit(f: &Fixture) -> Facts {
    registered(f, TAGS, "quit", CANCELLED)
}
pub(super) fn ctl_registration_tags(f: &Fixture) -> Facts {
    registered(f, TAGS, "none", PASSED)
}
pub(super) fn show_cr(f: &Fixture) -> Facts {
    read_registered(f, SHOW, ("CR", "stop0d"), CANCELLED)
}
pub(super) fn show_lf(f: &Fixture) -> Facts {
    read_registered(f, SHOW, ("LF", "stop0a"), CANCELLED)
}
pub(super) fn show_crlf(f: &Fixture) -> Facts {
    read_registered(f, SHOW, ("CRLF", "stop0d0a"), CANCELLED)
}
pub(super) fn show_eof(f: &Fixture) -> Facts {
    read_registered(f, SHOW, ("EOF", "04"), CANCELLED)
}
pub(super) fn show_partial_ctrl_d(f: &Fixture) -> Facts {
    read_registered(f, SHOW, ("partial_Ctrl_D", "partial+04"), CANCELLED)
}
pub(super) fn show_master_close(f: &Fixture) -> Facts {
    read_registered(f, SHOW, ("master_close", "master_close"), CANCELLED)
}
pub(super) fn show_repeated(f: &Fixture) -> Facts {
    registered(f, SHOW, "repeated", CANCELLED)
}
pub(super) fn show_quit(f: &Fixture) -> Facts {
    registered(f, SHOW, "quit", CANCELLED)
}
pub(super) fn ctl_registration_show(f: &Fixture) -> Facts {
    registered(f, SHOW, "none", PASSED)
}
