//! SB-3E S5 section U6a-1: s5_reg.rs
// SB3E-R120, R125: the three credited registration_record ingress rows (437,
// 439, 440) and their control, each one call of s5's `registered` with its
// row's literals; s5 holds the point, applies the value and collects.
use super::{
    s5::{registered, Facts},
    Fixture,
};
/// Row 437: `emergency()` twice while the record is held.
pub(super) fn record_repeated(f: &Fixture) -> Facts {
    registered(f, ("registration_record", "ingress"), "repeated")
}
/// Row 439: `fail()` while the record is held.
pub(super) fn record_fatal(f: &Fixture) -> Facts {
    registered(f, ("registration_record", "ingress"), "fatal")
}
/// Row 440: `accept(Quit, 1)` while the record is held.
pub(super) fn record_quit(f: &Fixture) -> Facts {
    registered(f, ("registration_record", "ingress"), "quit")
}
/// The control: held at the record and released with no value.
pub(super) fn ctl_registration_record(f: &Fixture) -> Facts {
    registered(f, ("registration_record", "ingress"), "none")
}
