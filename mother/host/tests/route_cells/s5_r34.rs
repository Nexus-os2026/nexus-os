//! SB-3E S5 section U6b-2: s5_r34.rs
// SEAM-2, U19d (SB3E-R242): the R3 attempt and R4 meter rows, harness
// composition until M-5b (F1). Each value is applied while the sink's own
// point is held: one call of s5's `attempted`, which names the advance's
// last step (C1), measured for every value; the controls pass.
use super::{
    s5::{attempted, Facts},
    Fixture,
};
const SLOT: (&str, &str) = ("call_slot_reserve", "ingress");
const TICKET: (&str, &str) = ("attempt_ticket", "ingress");
const CHARGE: (&str, &str) = ("charge_append", "ingress");
const SETTLE: (&str, &str) = ("meter_settle", "ingress");
const SLOT_L: (&str, &str) = ("call_slot_reserve", "lifecycle");
const TICKET_L: (&str, &str) = ("attempt_ticket", "lifecycle");
const CHARGE_L: (&str, &str) = ("charge_append", "lifecycle");
const SETTLE_L: (&str, &str) = ("meter_settle", "lifecycle");
const STOPPED: &str = "Err(Stopped)";
const PASSED: &str = "Ok(())";
pub(super) fn slot_repeated(f: &Fixture) -> Facts {
    attempted(f, SLOT, ("repeated", None), STOPPED)
}
pub(super) fn slot_quit(f: &Fixture) -> Facts {
    attempted(f, SLOT, ("quit", None), STOPPED)
}
pub(super) fn slot_emergency(f: &Fixture) -> Facts {
    attempted(f, SLOT_L, ("Emergency", None), STOPPED)
}
pub(super) fn slot_failed(f: &Fixture) -> Facts {
    attempted(f, SLOT_L, ("Failed", None), STOPPED)
}
pub(super) fn slot_closingnormal(f: &Fixture) -> Facts {
    attempted(f, SLOT_L, ("ClosingNormal", None), STOPPED)
}
pub(super) fn slot_closingrestricted(f: &Fixture) -> Facts {
    attempted(f, SLOT_L, ("ClosingRestricted", None), STOPPED)
}
pub(super) fn ctl_call_slot_reserve(f: &Fixture) -> Facts {
    attempted(f, SLOT, ("none", None), PASSED)
}
pub(super) fn ticket_repeated(f: &Fixture) -> Facts {
    attempted(f, TICKET, ("repeated", None), STOPPED)
}
pub(super) fn ticket_quit(f: &Fixture) -> Facts {
    attempted(f, TICKET, ("quit", None), STOPPED)
}
pub(super) fn ticket_emergency(f: &Fixture) -> Facts {
    attempted(f, TICKET_L, ("Emergency", None), STOPPED)
}
pub(super) fn ticket_failed(f: &Fixture) -> Facts {
    attempted(f, TICKET_L, ("Failed", None), STOPPED)
}
pub(super) fn ticket_closingnormal(f: &Fixture) -> Facts {
    attempted(f, TICKET_L, ("ClosingNormal", None), STOPPED)
}
pub(super) fn ticket_closingrestricted(f: &Fixture) -> Facts {
    attempted(f, TICKET_L, ("ClosingRestricted", None), STOPPED)
}
pub(super) fn ctl_attempt_ticket(f: &Fixture) -> Facts {
    attempted(f, TICKET, ("none", None), PASSED)
}
pub(super) fn charge_cr(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("CR", Some("stop0d")), STOPPED)
}
pub(super) fn charge_lf(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("LF", Some("stop0a")), STOPPED)
}
pub(super) fn charge_crlf(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("CRLF", Some("stop0d0a")), STOPPED)
}
pub(super) fn charge_eof(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("EOF", Some("04")), STOPPED)
}
pub(super) fn charge_partial_ctrl_d(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("partial_Ctrl_D", Some("partial+04")), STOPPED)
}
pub(super) fn charge_master_close(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("master_close", Some("master_close")), STOPPED)
}
pub(super) fn charge_repeated(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("repeated", None), STOPPED)
}
pub(super) fn charge_quit(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("quit", None), STOPPED)
}
pub(super) fn charge_emergency(f: &Fixture) -> Facts {
    attempted(f, CHARGE_L, ("Emergency", None), STOPPED)
}
pub(super) fn charge_failed(f: &Fixture) -> Facts {
    attempted(f, CHARGE_L, ("Failed", None), STOPPED)
}
pub(super) fn charge_closingnormal(f: &Fixture) -> Facts {
    attempted(f, CHARGE_L, ("ClosingNormal", None), STOPPED)
}
pub(super) fn charge_closingrestricted(f: &Fixture) -> Facts {
    attempted(f, CHARGE_L, ("ClosingRestricted", None), STOPPED)
}
pub(super) fn ctl_charge_append(f: &Fixture) -> Facts {
    attempted(f, CHARGE, ("none", None), PASSED)
}
pub(super) fn settle_repeated(f: &Fixture) -> Facts {
    attempted(f, SETTLE, ("repeated", None), STOPPED)
}
pub(super) fn settle_quit(f: &Fixture) -> Facts {
    attempted(f, SETTLE, ("quit", None), STOPPED)
}
pub(super) fn settle_emergency(f: &Fixture) -> Facts {
    attempted(f, SETTLE_L, ("Emergency", None), STOPPED)
}
pub(super) fn settle_failed(f: &Fixture) -> Facts {
    attempted(f, SETTLE_L, ("Failed", None), STOPPED)
}
pub(super) fn settle_closingnormal(f: &Fixture) -> Facts {
    attempted(f, SETTLE_L, ("ClosingNormal", None), STOPPED)
}
pub(super) fn settle_closingrestricted(f: &Fixture) -> Facts {
    attempted(f, SETTLE_L, ("ClosingRestricted", None), STOPPED)
}
pub(super) fn ctl_meter_settle(f: &Fixture) -> Facts {
    attempted(f, SETTLE, ("none", None), PASSED)
}
