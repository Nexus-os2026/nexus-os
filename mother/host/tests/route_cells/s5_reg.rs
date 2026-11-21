//! SB-3E S5 section U6a-1: s5_reg.rs
// SB3E-R120, R125: the three credited registration_record ingress rows (437,
// 439, 440) and their control, each one call of s5's `registered` with its
// row's literals; s5 holds the point, applies the value and collects.
// SEAM-1 (SB3E-R139): the registration_reserve rows 449, 451, 452 and the
// registration_readiness rows 425, 427, 428 and their controls, the same way.
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
/// Row 449: `emergency()` twice while `reg.reserve` is held.
pub(super) fn reserve_repeated(f: &Fixture) -> Facts {
    registered(f, ("registration_reserve", "ingress"), "repeated")
}
/// Row 451: `fail()` while `reg.reserve` is held.
pub(super) fn reserve_fatal(f: &Fixture) -> Facts {
    registered(f, ("registration_reserve", "ingress"), "fatal")
}
/// Row 452: `accept(Quit, 1)` while `reg.reserve` is held.
pub(super) fn reserve_quit(f: &Fixture) -> Facts {
    registered(f, ("registration_reserve", "ingress"), "quit")
}
/// The control: held at `reg.reserve` and released with no value.
pub(super) fn ctl_registration_reserve(f: &Fixture) -> Facts {
    registered(f, ("registration_reserve", "ingress"), "none")
}
/// Row 425: `emergency()` twice while `reg.ready` is held.
pub(super) fn readiness_repeated(f: &Fixture) -> Facts {
    registered(f, ("registration_readiness", "ingress"), "repeated")
}
/// Row 427: `fail()` while `reg.ready` is held.
pub(super) fn readiness_fatal(f: &Fixture) -> Facts {
    registered(f, ("registration_readiness", "ingress"), "fatal")
}
/// Row 428: `accept(Quit, 1)` while `reg.ready` is held.
pub(super) fn readiness_quit(f: &Fixture) -> Facts {
    registered(f, ("registration_readiness", "ingress"), "quit")
}
/// The control: held at `reg.ready` and released with no value.
pub(super) fn ctl_registration_readiness(f: &Fixture) -> Facts {
    registered(f, ("registration_readiness", "ingress"), "none")
}
