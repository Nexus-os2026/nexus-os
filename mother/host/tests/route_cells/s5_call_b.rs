//! SB-3E S5 section U19b-2: s5_call_b.rs
// SB3E-R216 F1, F6 and R217: the R5 call rows refused at ords 1-3
// (call_version 1, call_status 2, call_tags 3), harness composition until
// M-5b (F1: production makes no model call before it). Byte values typed and
// repeated and quit applied while the row's ord of `model.call.observed` is
// held; the four states reached while its `call.gate` is held; the controls
// held and released. Each is one call of s5's `called` or `read_called`,
// which names the advance's last step (C1).
use super::{
    s5::{called, read_called, Facts},
    Fixture,
};
const VERSION: (&str, &str) = ("call_version", "ingress");
const STATUS: (&str, &str) = ("call_status", "ingress");
const TAGS: (&str, &str) = ("call_tags", "ingress");
const VERSION_L: (&str, &str) = ("call_version", "lifecycle");
const STATUS_L: (&str, &str) = ("call_status", "lifecycle");
const TAGS_L: (&str, &str) = ("call_tags", "lifecycle");
/// C1: the advance's last step, measured for every value (byte, held and
/// state; a bare terminator gives `Ok`, Step 0 §5); the controls' passes.
const STOPPED: &[&str] = &["Err(Stopped)"];
const PASSED: &[&str] = &["Ok(())"];
pub(super) fn version_cr(f: &Fixture) -> Facts {
    read_called(f, VERSION, ("CR", "stop0d"), STOPPED)
}
pub(super) fn version_lf(f: &Fixture) -> Facts {
    read_called(f, VERSION, ("LF", "stop0a"), STOPPED)
}
pub(super) fn version_crlf(f: &Fixture) -> Facts {
    read_called(f, VERSION, ("CRLF", "stop0d0a"), STOPPED)
}
pub(super) fn version_eof(f: &Fixture) -> Facts {
    read_called(f, VERSION, ("EOF", "04"), STOPPED)
}
pub(super) fn version_partial_ctrl_d(f: &Fixture) -> Facts {
    read_called(f, VERSION, ("partial_Ctrl_D", "partial+04"), STOPPED)
}
pub(super) fn version_master_close(f: &Fixture) -> Facts {
    read_called(f, VERSION, ("master_close", "master_close"), STOPPED)
}
pub(super) fn version_repeated(f: &Fixture) -> Facts {
    called(f, VERSION, "repeated", STOPPED)
}
pub(super) fn version_quit(f: &Fixture) -> Facts {
    called(f, VERSION, "quit", STOPPED)
}
pub(super) fn version_emergency(f: &Fixture) -> Facts {
    called(f, VERSION_L, "Emergency", STOPPED)
}
pub(super) fn version_failed(f: &Fixture) -> Facts {
    called(f, VERSION_L, "Failed", STOPPED)
}
pub(super) fn version_closingnormal(f: &Fixture) -> Facts {
    called(f, VERSION_L, "ClosingNormal", STOPPED)
}
pub(super) fn version_closingrestricted(f: &Fixture) -> Facts {
    called(f, VERSION_L, "ClosingRestricted", STOPPED)
}
pub(super) fn ctl_call_version(f: &Fixture) -> Facts {
    called(f, VERSION, "none", PASSED)
}
pub(super) fn status_cr(f: &Fixture) -> Facts {
    read_called(f, STATUS, ("CR", "stop0d"), STOPPED)
}
pub(super) fn status_lf(f: &Fixture) -> Facts {
    read_called(f, STATUS, ("LF", "stop0a"), STOPPED)
}
pub(super) fn status_crlf(f: &Fixture) -> Facts {
    read_called(f, STATUS, ("CRLF", "stop0d0a"), STOPPED)
}
pub(super) fn status_eof(f: &Fixture) -> Facts {
    read_called(f, STATUS, ("EOF", "04"), STOPPED)
}
pub(super) fn status_partial_ctrl_d(f: &Fixture) -> Facts {
    read_called(f, STATUS, ("partial_Ctrl_D", "partial+04"), STOPPED)
}
pub(super) fn status_master_close(f: &Fixture) -> Facts {
    read_called(f, STATUS, ("master_close", "master_close"), STOPPED)
}
pub(super) fn status_repeated(f: &Fixture) -> Facts {
    called(f, STATUS, "repeated", STOPPED)
}
pub(super) fn status_quit(f: &Fixture) -> Facts {
    called(f, STATUS, "quit", STOPPED)
}
pub(super) fn status_emergency(f: &Fixture) -> Facts {
    called(f, STATUS_L, "Emergency", STOPPED)
}
pub(super) fn status_failed(f: &Fixture) -> Facts {
    called(f, STATUS_L, "Failed", STOPPED)
}
pub(super) fn status_closingnormal(f: &Fixture) -> Facts {
    called(f, STATUS_L, "ClosingNormal", STOPPED)
}
pub(super) fn status_closingrestricted(f: &Fixture) -> Facts {
    called(f, STATUS_L, "ClosingRestricted", STOPPED)
}
pub(super) fn ctl_call_status(f: &Fixture) -> Facts {
    called(f, STATUS, "none", PASSED)
}
pub(super) fn tags_cr(f: &Fixture) -> Facts {
    read_called(f, TAGS, ("CR", "stop0d"), STOPPED)
}
pub(super) fn tags_lf(f: &Fixture) -> Facts {
    read_called(f, TAGS, ("LF", "stop0a"), STOPPED)
}
pub(super) fn tags_crlf(f: &Fixture) -> Facts {
    read_called(f, TAGS, ("CRLF", "stop0d0a"), STOPPED)
}
pub(super) fn tags_eof(f: &Fixture) -> Facts {
    read_called(f, TAGS, ("EOF", "04"), STOPPED)
}
pub(super) fn tags_partial_ctrl_d(f: &Fixture) -> Facts {
    read_called(f, TAGS, ("partial_Ctrl_D", "partial+04"), STOPPED)
}
pub(super) fn tags_master_close(f: &Fixture) -> Facts {
    read_called(f, TAGS, ("master_close", "master_close"), STOPPED)
}
pub(super) fn tags_repeated(f: &Fixture) -> Facts {
    called(f, TAGS, "repeated", STOPPED)
}
pub(super) fn tags_quit(f: &Fixture) -> Facts {
    called(f, TAGS, "quit", STOPPED)
}
pub(super) fn tags_emergency(f: &Fixture) -> Facts {
    called(f, TAGS_L, "Emergency", STOPPED)
}
pub(super) fn tags_failed(f: &Fixture) -> Facts {
    called(f, TAGS_L, "Failed", STOPPED)
}
pub(super) fn tags_closingnormal(f: &Fixture) -> Facts {
    called(f, TAGS_L, "ClosingNormal", STOPPED)
}
pub(super) fn tags_closingrestricted(f: &Fixture) -> Facts {
    called(f, TAGS_L, "ClosingRestricted", STOPPED)
}
pub(super) fn ctl_call_tags(f: &Fixture) -> Facts {
    called(f, TAGS, "none", PASSED)
}
