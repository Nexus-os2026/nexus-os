//! SB-3E S5 section U9b: s5_r6.rs
// SB3E-R130.1, R135: the five credited R6 lifecycle rows (584, 585, 599, 600, 601)
// and their two controls, each one call of s5's `closed` with its row's
// literals; the sink only calls the held route, the apply runs while held.
use super::{
    s5::{closed, Close, Facts, Mode, Setup},
    Fixture,
};

fn text<T, E: std::fmt::Debug>(r: Result<T, E>) -> Result<(), String> {
    r.map(|_| ()).map_err(|e| format!("{e:?}"))
}
fn nothing(_: &Close) {}
fn emergency(c: &Close) {
    c.o.ingress.emergency().expect("emergency");
}
fn fail(c: &Close) {
    c.o.ingress.fail();
}
/// `accept(Quit, 2)` (h:1693-1766), held at ingress.accept (h:1695-1698).
fn quit_at(f: &Fixture, value: &'static str, before: fn(&Close), apply: fn(&Close)) -> Facts {
    let row = ("close_restrict", "lifecycle");
    let prefix = |c: &mut Close| before(c);
    let sink = |c: &Close, ()| (text(c.accept_quit()), ());
    let mode = Mode::Paused;
    closed(
        f,
        Setup::Mother,
        row,
        value,
        mode,
        Ok(()),
        prefix,
        sink,
        apply,
    )
    .0
}
/// `emergency()` (h:1640-1663), held at ingress.emergency (h:1642).
fn emergency_at(
    f: &Fixture,
    setup: Setup,
    value: &'static str,
    before: fn(&Close),
    apply: fn(&Close),
) -> Facts {
    let row = ("emergency_restrict", "lifecycle");
    let prefix = |c: &mut Close| before(c);
    let sink = |c: &Close, ()| (text(c.o.ingress.emergency()), ());
    closed(
        f,
        setup,
        row,
        value,
        Mode::Paused,
        Ok(()),
        prefix,
        sink,
        apply,
    )
    .0
}
pub(super) fn quit_emergency(f: &Fixture) -> Facts {
    quit_at(f, "Emergency", nothing, emergency)
}
pub(super) fn quit_failed(f: &Fixture) -> Facts {
    quit_at(f, "Failed", nothing, fail)
}
pub(super) fn ctl_close_restrict(f: &Fixture) -> Facts {
    quit_at(f, "none", nothing, nothing)
}
pub(super) fn emergency_bootstrap(f: &Fixture) -> Facts {
    emergency_at(f, Setup::Bootstrap, "Bootstrap", nothing, nothing)
}
pub(super) fn emergency_emergency(f: &Fixture) -> Facts {
    emergency_at(f, Setup::Mother, "Emergency", nothing, emergency)
}
pub(super) fn emergency_failed(f: &Fixture) -> Facts {
    emergency_at(f, Setup::Mother, "Failed", nothing, fail)
}
pub(super) fn ctl_emergency_restrict(f: &Fixture) -> Facts {
    emergency_at(f, Setup::Mother, "none", nothing, nothing)
}
