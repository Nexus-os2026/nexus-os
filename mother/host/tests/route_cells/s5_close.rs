//! SB-3E S5 section U9a: s5_close.rs
// SB3E-R117, R123: the eleven credited close-chain lifecycle rows and the six
// d_close controls, each through s5's `closed`, over the public API only.
// Each row's prefix, apply and setup are named in its own driver, so a plant
// can move or remove one in place (S2, S3).
use super::{
    s5::{closed, Close, Facts, Mode, Setup},
    Fixture,
};
use nexus_governed_control::host_session::close::CloseSeal;
use nexus_governed_control::host_session::SessionRequest;

/// The prepared Mother end (sc:56), recorded at the seal (sc:14-20); its
/// type is never named here (a section row imports nothing of the coordinator).
type End = Box<dyn FnOnce(&CloseSeal) -> Result<(), String> + Send>;
/// How far a prefix goes along D2's close chain (d2:185-214).
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Upto {
    Nothing,
    Prepare,
    Authority,
    Seal,
    Observe,
    Primary,
}
/// What the prefix made: the Quit, the Mother end and the seal.
struct Pre {
    quit: Option<SessionRequest>,
    end: Option<End>,
    seal: Option<CloseSeal>,
}
fn text<T, E: std::fmt::Debug>(r: Result<T, E>) -> Result<(), String> {
    r.map(|_| ()).map_err(|e| format!("{e:?}"))
}
fn through(c: &mut Close, upto: Upto) -> Pre {
    let mut p = Pre {
        quit: None,
        end: None,
        seal: None,
    };
    if upto > Upto::Nothing {
        p.quit = Some(c.accept_quit().expect("Quit"));
        let mother = c.mother.take().expect("the Mother");
        let end = mother.m.prepare_session_end().expect("the Mother end");
        p.end = Some(Box::new(move |seal: &CloseSeal| text(end.record(seal))));
    }
    let (s, quit) = (&c.o.session, p.quit.as_ref());
    if upto >= Upto::Authority {
        s.settle_close_resources(quit.unwrap()).expect("resources");
        s.settle_close_authority(quit.unwrap()).expect("authority");
    }
    if upto >= Upto::Seal {
        p.seal = Some(s.seal_close(quit.unwrap()).expect("the seal"));
    }
    if upto >= Upto::Observe {
        let seal = p.seal.as_ref().unwrap();
        (p.end.take().unwrap())(seal).expect("the Mother end recorded");
        s.observe_mother_end(seal).expect("the Mother end observed");
    }
    if upto >= Upto::Primary {
        s.close_primary(p.seal.as_ref().unwrap())
            .expect("the primary end");
    }
    p
}
fn nothing(_: &Close) {}
fn emergency(c: &Close) {
    c.o.ingress.emergency().expect("emergency");
}
fn fail(c: &Close) {
    c.o.ingress.fail();
}
fn quit_and_emergency(c: &Close) {
    c.accept_quit().expect("Quit");
    emergency(c);
}

/// `close_obligations` (close.rs:269-310), held at close.obligations.
fn obligations(
    f: &Fixture,
    setup: Setup,
    value: &'static str,
    mode: Mode,
    before: fn(&Close),
    apply: fn(&Close),
) -> Facts {
    let row = ("close_obligations", "lifecycle");
    let prefix = |c: &mut Close| before(c);
    let sink = |c: &Close, ()| (text(c.o.session.close_obligations()), ());
    closed(f, setup, row, value, mode, Ok(()), prefix, sink, apply).0
}
pub(super) fn obligations_normal(f: &Fixture) -> Facts {
    obligations(f, Setup::Mother, "Normal", Mode::Armed, nothing, nothing)
}
pub(super) fn obligations_bootstrap(f: &Fixture) -> Facts {
    obligations(
        f,
        Setup::Bootstrap,
        "Bootstrap",
        Mode::Paused,
        nothing,
        nothing,
    )
}
pub(super) fn obligations_emergency(f: &Fixture) -> Facts {
    obligations(
        f,
        Setup::Mother,
        "Emergency",
        Mode::Paused,
        nothing,
        emergency,
    )
}
pub(super) fn obligations_failed(f: &Fixture) -> Facts {
    obligations(f, Setup::Mother, "Failed", Mode::Paused, nothing, fail)
}
pub(super) fn obligations_closingrestricted(f: &Fixture) -> Facts {
    obligations(
        f,
        Setup::Mother,
        "ClosingRestricted",
        Mode::Paused,
        nothing,
        quit_and_emergency,
    )
}
pub(super) fn ctl_close_obligations(f: &Fixture) -> Facts {
    obligations(f, Setup::Mother, "none", Mode::Paused, nothing, nothing)
}

/// `finish_delivery` (close.rs:230-247), held at close.delivery.
fn delivery(f: &Fixture, value: &'static str, upto: Upto) -> Facts {
    let row = ("delivery", "lifecycle");
    let prefix = |c: &mut Close| through(c, upto).seal;
    let sink = |_: &Close, seal: Option<CloseSeal>| match seal.map(CloseSeal::finish_delivery) {
        Some(true) => (Ok(()), ()),
        delivered => (Err(format!("delivered {delivered:?}")), ()),
    };
    closed(
        f,
        Setup::Mother,
        row,
        value,
        Mode::Armed,
        Ok(()),
        prefix,
        sink,
        nothing,
    )
    .0
}
pub(super) fn delivery_sealedfinalizing(f: &Fixture) -> Facts {
    delivery(f, "SealedFinalizing", Upto::Primary)
}
pub(super) fn ctl_delivery(f: &Fixture) -> Facts {
    delivery(f, "none", Upto::Primary)
}

/// The Mother end recorded (sc:14-20; close.mother_end's first arrival,
/// close.rs:205, held), then observed (close.rs:497-521).
fn mother_end(f: &Fixture, value: &'static str, upto: Upto) -> Facts {
    let row = ("mother_end", "lifecycle");
    let prefix = |c: &mut Close| {
        let p = through(c, upto);
        (p.end, p.seal)
    };
    let sink = |c: &Close, (end, seal): (Option<End>, Option<CloseSeal>)| {
        let (Some(end), Some(seal)) = (end, seal) else {
            return (Err("no seal".into()), None);
        };
        let recorded = end(&seal);
        (
            recorded.and(text(c.o.session.observe_mother_end(&seal))),
            Some(seal),
        )
    };
    closed(
        f,
        Setup::Mother,
        row,
        value,
        Mode::Armed,
        Ok(()),
        prefix,
        sink,
        nothing,
    )
    .0
}
pub(super) fn mother_end_sealedfinalizing(f: &Fixture) -> Facts {
    mother_end(f, "SealedFinalizing", Upto::Seal)
}
pub(super) fn ctl_mother_end(f: &Fixture) -> Facts {
    mother_end(f, "none", Upto::Seal)
}

/// `close_primary` (close.rs:522-549), held at close.primary: row 642
/// (SEAM-1 T2, SB3E-R139) and the control.
fn primary_end(f: &Fixture, value: &'static str) -> Facts {
    let row = ("primary_end", "lifecycle");
    let prefix = |c: &mut Close| through(c, Upto::Observe).seal;
    let sink = |c: &Close, seal: Option<CloseSeal>| match seal {
        Some(seal) => (text(c.o.session.close_primary(&seal)), Some(seal)),
        None => (Err("no seal".into()), None),
    };
    closed(
        f,
        Setup::Mother,
        row,
        value,
        Mode::Armed,
        Ok(()),
        prefix,
        sink,
        nothing,
    )
    .0
}
pub(super) fn primary_end_sealedfinalizing(f: &Fixture) -> Facts {
    primary_end(f, "SealedFinalizing")
}
pub(super) fn ctl_primary_end(f: &Fixture) -> Facts {
    primary_end(f, "none")
}

/// `settle_close_resources` (h:1017-1038): its close operation's join
/// (h:1036, producer.join's first arrival, close.rs:49) held.
fn join(
    f: &Fixture,
    value: &'static str,
    upto: Upto,
    before: fn(&Close),
    apply: fn(&Close),
) -> Facts {
    let row = ("producer_join", "lifecycle");
    let prefix = |c: &mut Close| {
        let quit = through(c, upto).quit;
        before(c);
        quit
    };
    let sink = |c: &Close, quit: Option<SessionRequest>| match quit {
        Some(quit) => (text(c.o.session.settle_close_resources(&quit)), ()),
        None => (Err("no Quit".into()), ()),
    };
    closed(
        f,
        Setup::Mother,
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
pub(super) fn join_closingrestricted(f: &Fixture) -> Facts {
    join(f, "ClosingRestricted", Upto::Prepare, nothing, emergency)
}
pub(super) fn join_closingnormal(f: &Fixture) -> Facts {
    join(f, "ClosingNormal", Upto::Prepare, nothing, nothing)
}
pub(super) fn ctl_producer_join(f: &Fixture) -> Facts {
    join(f, "none", Upto::Prepare, nothing, nothing)
}

/// `seal_close` (close.rs:365-496), held at close.seal (Q, close.rs:367-370).
type Want = Result<(), &'static str>;
fn seal(
    f: &Fixture,
    value: &'static str,
    mode: Mode,
    want: Want,
    upto: Upto,
    before: fn(&Close),
    apply: fn(&Close),
) -> Facts {
    let row = ("seal", "lifecycle");
    let prefix = |c: &mut Close| {
        let quit = through(c, upto).quit;
        before(c);
        quit
    };
    let sink = |c: &Close, quit: Option<SessionRequest>| match quit {
        Some(quit) => {
            let sealed = c.o.session.seal_close(&quit);
            (text(sealed.as_ref()), sealed.ok())
        }
        None => (Err("no Quit".into()), None),
    };
    closed(
        f,
        Setup::Mother,
        row,
        value,
        mode,
        want,
        prefix,
        sink,
        apply,
    )
    .0
}
pub(super) fn seal_closingnormal(f: &Fixture) -> Facts {
    seal(
        f,
        "ClosingNormal",
        Mode::Armed,
        Ok(()),
        Upto::Authority,
        nothing,
        nothing,
    )
}
pub(super) fn seal_closingrestricted(f: &Fixture) -> Facts {
    let refused = Err("Unavailable");
    seal(
        f,
        "ClosingRestricted",
        Mode::Paused,
        refused,
        Upto::Authority,
        nothing,
        emergency,
    )
}
pub(super) fn ctl_seal(f: &Fixture) -> Facts {
    seal(
        f,
        "none",
        Mode::Paused,
        Ok(()),
        Upto::Authority,
        nothing,
        nothing,
    )
}
