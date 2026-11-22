//! SB-3E S5 section U21a: s5_report.rs
//! U21a (SB3E-R196, R197, R199): the five set-B `close_report` rows and
//! their control. Each runs production `hands::run` through `report.rs` in
//! its lane child, Owner lines only, held at an S7 `close_report` point,
//! and is collected by `s5::collect_report`.
use super::{
    report::{reached, Hold, Owner},
    s5::{collect_report, Facts},
    Fixture,
};

/// `quit`: the clean close, so the state at the hold and the report's stage.
const QUIT: (&str, &str) = ("SealedFinalizing", "delivery_pending");
/// `run`, `stop`, `quit`: the seal is refused (Step 0, measured).
const SEAL_REFUSED: (&str, &str) = ("ClosingRestricted", "seal");
/// The entry point; the exit point and its table name (`paused@string`).
const ENTRY: (&str, &str) = ("host.close_report", "host.close_report");
const EXIT: (&str, &str) = ("host.close_report.string", "string");

/// One cell: `hands::run` held at `point`, then collected under `label`.
fn held(
    f: &Fixture,
    owner: Owner,
    (point, label): (&'static str, &'static str),
    row: (&'static str, &'static str),
    value: &'static str,
    want: (&'static str, &'static str),
) -> Facts {
    let r = reached(f, owner, point, Hold::Held);
    let journal = (r.journal.0.as_path(), r.journal.1);
    let threads = (r.run, r.test);
    collect_report(
        label,
        r.ack,
        threads,
        journal,
        (row, value),
        want,
        r.findings,
    )
}
/// x:581: the clean close reaches close_report SealedFinalizing.
pub(super) fn sealedfinalizing(f: &Fixture) -> Facts {
    let row = ("close_report", "lifecycle");
    held(f, Owner::Quit, ENTRY, row, "SealedFinalizing", QUIT)
}
/// x:580: a refused seal reaches close_report ClosingRestricted.
pub(super) fn closingrestricted(f: &Fixture) -> Facts {
    let row = ("close_report", "lifecycle");
    let value = "ClosingRestricted";
    held(f, Owner::RunStopQuit, ENTRY, row, value, SEAL_REFUSED)
}
/// x:761: no fault; the point held only to open the window (`ack=none`).
pub(super) fn fault_none(f: &Fixture) -> Facts {
    held(
        f,
        Owner::Quit,
        ENTRY,
        ("close_report", "fault"),
        "none",
        QUIT,
    )
}
/// x:868: held at the entry.
pub(super) fn schedule_before(f: &Fixture) -> Facts {
    held(
        f,
        Owner::Quit,
        ENTRY,
        ("close_report", "schedule"),
        "before",
        QUIT,
    )
}
/// x:869: held at the exit, after the report is built.
pub(super) fn schedule_after(f: &Fixture) -> Facts {
    held(
        f,
        Owner::Quit,
        EXIT,
        ("close_report", "schedule"),
        "after",
        QUIT,
    )
}
/// The control: `quit` held at the entry, no value.
pub(super) fn ctl_close_report(f: &Fixture) -> Facts {
    held(
        f,
        Owner::Quit,
        ENTRY,
        ("close_report", "lifecycle"),
        "none",
        QUIT,
    )
}
