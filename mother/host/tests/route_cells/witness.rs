//! SB3E-R86.4 (U5a; design 2.3-2.5; AM-2 A.4; mission Behaviour 9): a cell's
//! witness. In the child, `render` writes the runner's facts in the frozen
//! table's grammar, S, Q, RUN, INV and ATT bound and the table's location
//! tokens mapped by the reviewed `TOKENS`; in the parent, `record` checks the
//! child's one `CELL` record and `compare` checks the witness against its
//! expected text, field by field and byte for byte. W0b B1: `render` is
//! row-driven: the cell's frozen row gives its fields, their order and each
//! identity tag's keys; every value is rendered from the facts.
use super::s5::{Ack, Facts, Ran};
use nexus_governed_control::broker::route_seams as s1;
use nexus_mother::journal::Event;
use nexus_mother::lane::LaneRole;

/// The frozen expectation table: the one reviewed `include_str!` row.
const TABLE: &str = include_str!("../session/model-bridge-expect.tsv");
/// W0b B1: every witness field, in the one order every row keeps (the
/// table's 11 field sets are ordered subsets of it).
const ORDER: [&str; 13] = [
    "entry",
    "check",
    "value",
    "out",
    "branch",
    "ledger",
    "window",
    "peer",
    "grant",
    "locks",
    "store",
    "next",
    "deadline_fired",
];
/// W0b B1: an identity tag's keys, in their one order; a row's tag is an
/// ordered subset of them.
const KEYS: [&str; 7] = ["s", "q", "run", "inv", "att", "kind", "ord"];
/// Behaviour 9, W0b B1 (R88.1): the reviewed location tokens, each with the
/// point it reads and the predicate that binds it to one observed entry:
/// point, identity, ordinal, thread and order. Finding a string is never
/// enough. U5's two, unchanged in meaning; later units add rows (W0b A: the
/// predicate gets its row's point and the tag; `first_at` and `note_at` are
/// the admitted entry or note there).
type Tag<'t> = (Option<&'t str>, Option<u64>);
type Predicate = for<'a> fn(&View<'a>, &str, Tag) -> Option<Seen<'a>>;
type Token = (&'static str, &'static str, Predicate);
const TOKENS: [Token; 18] = [
    // model.call.observed, then on its thread exactly the note
    // refused:8.cas, with no ord-4 model.call.admitted.
    ("CAS:m:774", "model.call.observed", |v, _, _| {
        cas(v).map(|(o, _)| o)
    }),
    // exactly the note refused:8.cas (refused:8.capacity is not it).
    ("note:8", "refused:8.cas", |v, _, _| cas(v).map(|(_, n)| n)),
    // U9a (d_close): the acknowledged entry at the row's point (close.rs:271,
    // 232, 524, 49, 367-370), the next close.mother_end of S after it on its
    // thread (observe_mother_end, close.rs:499; Q11), and the seal's stop note
    // (close.rs:407).
    ("c:265-300", "close.obligations", acked),
    ("c:227-234", "close.delivery", acked),
    ("c:462-480", "close.mother_end", after_acked),
    ("c:490-505", "close.primary", acked),
    ("c:48-54", "producer.join", acked),
    ("c:372-429", "close.seal", acked),
    ("note:stop_obligations", "seal:stop", note_at),
    // U6a-1 (d_register): the acknowledged model.record.before, then on its
    // thread, later, the admitted model.install.before (model.rs:301, 309, 314).
    ("m:292-299", "model.record.before", recorded_write),
    // U9b (d_quit, d_stop): the acknowledged entry at the held sink's point
    // (host_session.rs:1695-1698, 1642).
    ("h:1690-1712", "ingress.accept", acked),
    ("CAS:h:1630-1633", "ingress.emergency", acked),
    // SEAM-1 (d_register, d_bootstrap): the acknowledged reg.reserve and
    // reg.ready (model.rs:122, host_session.rs:858) and their refusal notes
    // (model.rs:142, host_session.rs:865).
    ("m:126-141", "reg.reserve", acked),
    ("note:scope_admit", "reg:scope_admit", note_at),
    ("h:838", "reg.ready", acked),
    ("note:word_compare", "reg:word_compare", note_at),
    // U21a (SB3E-R199 Q-U21-1): close_report's check, its bound branch
    // records (entries, not notes) then its exit with the report; and its
    // exit point `string` (hands.rs `close_report`).
    ("note", "host.close_report.string", reported),
    ("string", "host.close_report.string", acked),
];
/// U21a (SB3E-R199 Q-U21-1): the admitted S7 `host.close_report` entry of
/// S, then on its thread, in order, the `resources`, `inventory` and `model`
/// branch records, each `Ok`, and the exit at `point` whose report holds no
/// `unknown` (so the stop and primary observations were known too). The
/// records are entries (`note: false`): `branch` stays `none`.
fn reported<'a>(v: &View<'a>, point: &str, tag: Tag) -> Option<Seen<'a>> {
    let at = |p: &str, e: &Seen| !e.s1 && !e.note && e.point == p && admitted(v, e, tag);
    let host = |e: &Seen| {
        v.f.host
            .iter()
            .find(|h| h.tick == e.tick && h.point == e.point)
    };
    let entry = *v.seen.iter().find(|e| at("host.close_report", e))?;
    let branches = [
        "host.close_report.resources",
        "host.close_report.inventory",
        "host.close_report.model",
    ];
    let mut last = entry;
    for p in branches.into_iter().chain([point]) {
        let after = |e: &&Seen| at(p, e) && e.thread == entry.thread && e.tick > last.tick;
        last = *v.seen.iter().find(after)?;
        let record = host(&last)?;
        let known = match p == point {
            true => record
                .text
                .as_deref()
                .is_some_and(|t| !t.contains("unknown")),
            false => record.known == Some(true),
        };
        if !known {
            return None;
        }
    }
    Some(last)
}
/// U9a: the admitted entry at `point` that is the cell's acknowledgement.
fn acked<'a>(v: &View<'a>, point: &str, tag: Tag) -> Option<Seen<'a>> {
    let ack = |e: &&Seen| v.f.acks.iter().any(|a| of(a) == **e);
    let at = |e: &&Seen| !e.note && e.point == point && admitted(v, e, tag);
    v.seen.iter().filter(at).find(ack).copied()
}
/// U6a-1: the record write returned (model.rs:309's `?` did not): the
/// acknowledged entry, then a later admitted `model.install.before` of S and
/// Q on its thread (model.rs:314).
fn recorded_write<'a>(v: &View<'a>, point: &str, tag: Tag) -> Option<Seen<'a>> {
    let a = acked(v, point, tag)?;
    let install = |e: &Seen| {
        let at = e.s1 && !e.note && e.point == "model.install.before";
        at && e.thread == a.thread && e.tick > a.tick && admitted(v, e, tag)
    };
    v.seen.iter().any(install).then_some(a)
}
/// U9a (Q11): the next admitted entry at `point` after the acknowledged one,
/// of S1 and on its thread.
fn after_acked<'a>(v: &View<'a>, point: &str, tag: Tag) -> Option<Seen<'a>> {
    let a = acked(v, point, tag)?;
    let later = |e: &&Seen| e.s1 && e.tick > a.tick && e.thread == a.thread;
    let at = |e: &&Seen| !e.note && e.point == point && admitted(v, e, tag);
    v.seen
        .iter()
        .filter(at)
        .filter(later)
        .min_by_key(|e| e.tick)
        .copied()
}
/// W0b B1: the ledger's closed list: each event the table names, as a
/// journal event (by invocation), a native record's phase (by run and
/// attempt, or by session) or, for the store's own unprojected end record
/// (evidence_store.rs:353-357), its S1 commit (SEAM-1 T2, `commit`: the
/// `store.commit.result` of that phase, evidence_store.rs:1382-1383); `all` is every journal entry and native record
/// of the session in the cell's window: since `start` (or `open_broker`)
/// captured the sources (s5.rs `start`'s `since`), or for `collect_close` and
/// `registered` since the hold's acknowledgement (U9a M3, SEAM-1; SB3E-R6.3's
/// empty delta is `+0:all[s=S]`).
const LEDGER: [(&str, &str); 7] = [
    ("ModelCallCharged", "journal"),
    ("LaneReturned", "journal"),
    ("SessionClosed", "journal"),
    ("model_call", "native"),
    ("model_registered", "native"),
    ("session_closed", "commit"),
    ("all", "all"),
];
/// W0b A1 (SB3E-R112): `point|keys|source`, one row per point: the exact
/// identity keys the route records there (`KEYS` order; `kind=`/`ord=` a
/// fixed value the frozen tag does not show) and the line that records them.
/// An entry is admitted only if its recorded keys are exactly its point's row
/// and each equals the bound value; a point with no row is never admitted.
const RECORDS: &str = "\
call.gate|s q run att kind ord|host_session/model.rs:893-899; broker/route_seams.rs:255-266
model.call.sampled|s q run att kind ord|host_session/model.rs:893-899
model.call.observed|s q run att kind ord|host_session/model.rs:893-899
model.call.admitted|s q run att kind ord|host_session/model.rs:893-899
refused:8.cas|s q run att kind ord|host_session/model.rs:849, 893-899
refused:1.expired|s q run att kind ord|host_session/model.rs:771, 893-899
model.attempt.record.before|s run att|host_session/model.rs:519-523
complete|s run att|host_session/model.rs:519-523
mother.charge.entry|s q inv|mother coordinator/model_calls.rs:203-206, 216
mother.settle|s q inv|mother coordinator/model_calls.rs:203-206, 307
attempt.slot|s run|host_session/model.rs:383
model.attempt.reserved|s run|host_session/model.rs:418-420
model.attempt.checked|run att|authority/run.rs:2144-2146
reg.reserve|s q|host_session/model.rs:122; host_session.rs:89-91
model.record.before|s q ord=4|host_session/model.rs:301 (after the 4 metadata gates)
model.install.before|s q ord=4|host_session/model.rs:314 (after the 4 metadata gates)
reg.ready|s q ord=4|host_session.rs:858 (after the 4 metadata gates)
reg:scope_admit|s q|host_session/model.rs:142; host_session.rs:89-91
reg:word_compare|s q ord=4|host_session.rs:865 (after the 4 metadata gates); 89-91
close.obligations|s|host_session/close.rs:271
close.delivery|s|host_session/close.rs:232
close.mother_end|s|host_session/close.rs:205, 499
close.primary|s|host_session/close.rs:524
producer.join|s|host_session/close.rs:49
close.seal|s q|host_session/close.rs:367-369
seal:stop|s|host_session/close.rs:407
ingress.accept|s q|host_session.rs:1695-1698
ingress.emergency|s|host_session.rs:1642
host.close_report|s|host hands.rs close_report entry; route_seams.rs report
host.close_report.resources|s|host hands.rs close_report resources branch; route_seams.rs report
host.close_report.inventory|s|host hands.rs close_report inventory branch; route_seams.rs report
host.close_report.model|s|host hands.rs close_report model branch; route_seams.rs report
host.close_report.string|s|host hands.rs close_report exit; route_seams.rs report
";
/// W0b A1b, A4: `key|points|carrier|source`: the only bindings of a tagged
/// key a point does not record, each through the observed entry `carrier`
/// names (as A.4's `inv`); `native` and `journal` are a ledger's records.
/// Each row has an inverse plant. Units add rows.
const CORRELATIONS: &str = "\
inv|call.gate model.call.sampled model.call.observed model.call.admitted|charge|AM-2 A.4: the call's mother.charge.entry of the Builder (model_calls.rs:216)
inv|attempt.slot model.attempt.reserved|prelock|the Builder's mother.lane.prelock of S and Q (mother coordinator.rs:1978-1983)
q|attempt.slot model.attempt.reserved|gate|the run's later call.gate of S and Q (host_session/model.rs:883, 893-899)
att|attempt.slot model.attempt.reserved|claim|credit's claim of the run (s5.rs credit, A.3)
run|mother.charge.entry mother.settle|sites|credit's claim on the Builder's attempt sites (s5.rs credit, A.3)
att|mother.charge.entry mother.settle|sites|credit's claim on the Builder's attempt sites (s5.rs credit, A.3)
s|native journal|fixture|the cell's own projection and journal in its window (s5.rs collect: since start; collect_close, registered: since the hold; kept, B1')
s|commit|primary|the end record's S1 commit on the acknowledged close.primary's thread, after it (close.rs:524, 538; evidence_store.rs:1382-1383)
q|native|bracket:model.record.before,model.install.before|the registration's own entries of S and Q either side of its record (host_session/model.rs:301-314)
";
/// The carriers a `CORRELATIONS` row may name, a closed list.
fn carried(name: &str, v: &View, e: Option<&Seen>) -> bool {
    let run = e.and_then(|e| e.run);
    let builder = e.is_some_and(|e| e.inv.is_some() && e.inv == v.b.inv);
    match name {
        "charge" => v.inv,
        "prelock" => prelocked(v),
        "gate" => gated(v, run),
        "claim" => v.b.att.is_some() && run.is_some() && run == v.b.run,
        "sites" => builder && v.b.att.is_some(),
        "fixture" => true,
        "primary" => acked(v, "close.primary", (None, None)).is_some(),
        _ => name
            .strip_prefix("bracket:")
            .is_some_and(|p| brackets(v, p)),
    }
}
/// A test's or a unit's tokens over the reviewed `RECORDS` and `CORRELATIONS`.
fn tables<'t>(tokens: &'t [Token]) -> Tables<'t> {
    Tables {
        tokens,
        records: RECORDS,
        correlations: CORRELATIONS,
    }
}
/// The tables a rendering reads (wave units add rows to each).
struct Tables<'t> {
    tokens: &'t [Token],
    records: &'t str,
    correlations: &'t str,
}
const TABLES: Tables<'static> = Tables {
    tokens: &TOKENS,
    records: RECORDS,
    correlations: CORRELATIONS,
};
/// W0b A1, A2: one observed point, of S1 or of S5, with the keys recorded.
#[derive(Clone, Copy, PartialEq)]
struct Seen<'a> {
    point: &'a str,
    note: bool,
    s1: bool,
    tick: u64,
    s: Option<nexus_governed_control::host_session::SessionIdentity>,
    q: Option<u64>,
    run: Option<nexus_governed_control::authority::ids::RunId>,
    inv: Option<nexus_mother::lane::InvocationId>,
    att: Option<u32>,
    kind: Option<&'a str>,
    ord: Option<u64>,
    /// An S1 invocation (the broker records none): never admitted.
    foreign: bool,
    thread: std::thread::ThreadId,
    instant: std::time::Instant,
}
fn of1(e: &s1::Entry) -> Seen<'_> {
    Seen {
        point: e.point,
        note: e.note,
        s1: true,
        tick: e.tick,
        s: e.session,
        q: e.request,
        run: e.run,
        inv: None,
        att: e.attempt,
        kind: e.kind,
        ord: e.request_ordinal,
        foreign: e.invocation.is_some(),
        thread: e.thread,
        instant: e.instant,
    }
}
fn of5(e: &nexus_mother::route_seams::Entry) -> Seen<'_> {
    Seen {
        point: e.point,
        note: false,
        s1: false,
        tick: e.tick,
        s: e.session,
        q: e.request,
        run: e.run,
        inv: e.invocation,
        att: None,
        kind: None,
        ord: None,
        foreign: false,
        thread: e.thread,
        instant: e.instant,
    }
}
/// U21a: an S7 `close_report` entry (its session only; never a note).
fn of7(e: &nexus_mother_host::route_seams::Entry) -> Seen<'_> {
    Seen {
        point: e.point,
        note: false,
        s1: false,
        tick: e.tick,
        s: e.session,
        q: None,
        run: None,
        inv: None,
        att: None,
        kind: None,
        ord: None,
        foreign: false,
        thread: e.thread,
        instant: e.instant,
    }
}
fn of(a: &Ack) -> Seen<'_> {
    match a {
        Ack::S1(e) => of1(e),
        Ack::S5(e) => of5(e),
        Ack::S7(e) => of7(e),
    }
}
/// Key `k` of `e`: whether it was recorded, and whether it equals the bound
/// value (`kind`/`ord`: `fixed`'s, else the tag's).
fn key(v: &View, e: &Seen, k: &str, tag: (Option<&str>, Option<u64>), fixed: &str) -> (bool, bool) {
    let b = &v.b;
    let pinned = |n: &str| {
        fixed
            .split(' ')
            .find_map(|x| x.strip_prefix(n))
            .and_then(|x| x.strip_prefix('='))
    };
    match k {
        "s" => (e.s.is_some(), e.s == Some(b.s)),
        "q" => (e.q.is_some(), e.q == Some(b.q)),
        "run" => (e.run.is_some(), e.run.is_some() && e.run == b.run),
        "inv" => (e.inv.is_some(), e.inv.is_some() && e.inv == b.inv),
        "att" => (e.att.is_some(), e.att.is_some() && e.att == b.att),
        "kind" => (e.kind.is_some(), e.kind == pinned("kind").or(tag.0)),
        _ => (
            e.ord.is_some(),
            e.ord.map(|o| o.to_string()).as_deref()
                == pinned("ord").or(tag.1.map(|o| o.to_string()).as_deref()),
        ),
    }
}
/// W0b A1: `e` admitted under the tag's kind and ordinal (its point's
/// `RECORDS` row, exactly).
fn admitted(v: &View, e: &Seen, tag: (Option<&str>, Option<u64>)) -> bool {
    let mut rows =
        v.t.records
            .lines()
            .map(|l| l.split('|').collect::<Vec<_>>());
    let Some(row) = rows.find(|r| r[0] == e.point) else {
        return false;
    };
    let keys: Vec<&str> = row[1]
        .split(' ')
        .map(|k| k.split('=').next().unwrap_or(k))
        .collect();
    !e.foreign
        && KEYS.iter().all(|k| {
            let (recorded, equal) = key(v, e, k, tag, row[1]);
            recorded == keys.contains(k) && (!recorded || equal)
        })
}
/// W0b A1b: a tag key bound on `e` (recorded, so equal), or by a correlation.
fn bound(v: &View, e: Option<&Seen>, point: &str, k: &str) -> bool {
    // W0b N-a: a recorded key binds only if it equals the bound value, and a
    // recorded, unequal one is never bound by a correlation either.
    if let Some((true, equal)) = e.map(|e| key(v, e, k, (None, None), "")) {
        return equal;
    }
    let rows =
        v.t.correlations
            .lines()
            .map(|l| l.split('|').collect::<Vec<_>>());
    let carries = |r: &Vec<&str>| r[0] == k && r[1].split(' ').any(|p| p == point);
    rows.filter(carries).any(|r| carried(r[2], v, e))
}
/// The Builder's `mother.lane.prelock` of S and Q (its invocation).
fn prelocked(v: &View) -> bool {
    let at = |m: &&Seen| !m.s1 && m.point == "mother.lane.prelock" && m.inv.is_some();
    v.seen
        .iter()
        .filter(at)
        .any(|m| m.inv == v.b.inv && m.s == Some(v.b.s) && m.q == Some(v.b.q))
}
/// A `call.gate` of S and Q on `e`'s run.
fn gated(v: &View, run: Option<nexus_governed_control::authority::ids::RunId>) -> bool {
    let gate = |g: &&Seen| g.s1 && g.point == "call.gate" && run.is_some() && g.run == run;
    let mut gates = v.seen.iter().filter(gate);
    gates.any(|g| g.s == Some(v.b.s) && g.q == Some(v.b.q))
}
/// W0b R1-1: q through the route's own request-carrying entries around the
/// record (`bracket:<point>,..`): each listed point is observed for S, and
/// every entry of S there is admitted, so carries Q. An entry of S at any
/// other point (an applier's `ingress.accept`) neither binds nor unbinds Q.
fn brackets(v: &View, points: &str) -> bool {
    points.split(',').all(|p| {
        let at = |e: &&Seen| e.s1 && !e.note && e.point == p && e.s == Some(v.b.s);
        let mut at = v.seen.iter().filter(at).peekable();
        at.peek().is_some() && at.all(|e| admitted(v, e, (None, None)))
    })
}
/// Behaviour 10: the controls' witnesses, by `(sink, dim)`, as measured at U5
/// Step 0 (R86).
const CONTROLS: [(&str, &str, &str); 13] = [
    ("call_chat", "ingress", CONTROL),
    ("close_obligations", "lifecycle", CLOSE_CONTROLS[0]),
    ("delivery", "lifecycle", CLOSE_CONTROLS[1]),
    ("mother_end", "lifecycle", CLOSE_CONTROLS[2]),
    ("primary_end", "lifecycle", CLOSE_CONTROLS[3]),
    ("producer_join", "lifecycle", CLOSE_CONTROLS[4]),
    ("seal", "lifecycle", CLOSE_CONTROLS[5]),
    ("registration_record", "ingress", REG_CONTROL),
    ("close_restrict", "lifecycle", HELD_CONTROLS[0]),
    ("emergency_restrict", "lifecycle", HELD_CONTROLS[1]),
    ("registration_reserve", "ingress", RESERVE_CONTROL),
    ("registration_readiness", "ingress", READY_CONTROL),
    ("close_report", "lifecycle", REPORT_CONTROL),
];
/// U21a: the close_report control, `quit` held at the entry with no value:
/// the credited rows' text with `none`.
const REPORT_CONTROL: &str = "entry=host.close_report[s=S];check=note[s=S];\
value=none@host.close_report ack=held won=route;out=passed@note;branch=none;\
ledger=+0:all[s=S];window=boundary;deadline_fired=none";
/// U9b (SB3E-R135.3 Q8): the held-sink controls as measured at U9b Step 0,
/// 20/20 each: the credited rows' text with no value.
const HELD_CONTROLS: [&str; 2] = [
    "entry=ingress.accept[s=S q=Q];check=h:1690-1712[s=S q=Q];value=none@ingress.accept ack=held won=route;\
out=passed@h:1690-1712;branch=none;ledger=+0:all[s=S];window=boundary;deadline_fired=none",
    "entry=ingress.emergency[s=S];check=CAS:h:1630-1633[s=S];value=none@ingress.emergency ack=held won=route;\
out=passed@CAS:h:1630-1633;branch=none;ledger=+0:all[s=S];window=boundary;deadline_fired=none",
];
/// SEAM-1 (SB3E-R139 Q-6): the reserve and readiness controls as measured at
/// SEAM-1 Step 0, 10/10 each: the refused rows' text with no value, the sink
/// passing (reserve then records its registration after the hold).
const RESERVE_CONTROL: &str = "entry=reg.reserve[s=S q=Q];check=m:126-141[s=S q=Q];\
value=none@reg.reserve ack=held won=route;out=passed@m:126-141;branch=none;\
ledger=+1:all[s=S];window=boundary;deadline_fired=none";
const READY_CONTROL: &str = "entry=reg.ready[s=S];check=h:838[s=S];\
value=none@reg.ready ack=held won=route;out=passed@h:838;branch=none;\
ledger=+0:all[s=S];window=boundary;deadline_fired=none";
/// U6a-1 (SB3E-R120.6 Q7): the registration_record control as measured at
/// U6a-1 Step 0, 10/10: the record rows' text with no value.
const REG_CONTROL: &str = "entry=model.record.before[s=S q=Q];check=m:292-299[s=S q=Q];\
value=none@model.record.before ack=held won=route;out=recorded@m:292-299;branch=none;\
ledger=+1:model_registered[s=S q=Q];window=boundary;deadline_fired=none";
/// U9a (SB3E-R117.3 Q10): the d_close controls as measured at U9a Step 0,
/// 20/20 each; `primary_end`'s `+1:session_closed` is the end record's S1
/// commit (SEAM-1 T2, measured 10/10: the record is not projected,
/// evidence_store.rs:992-1009).
const CLOSE_CONTROLS: [&str; 6] = [
    "entry=close.obligations[s=S];check=c:265-300[s=S];value=none@close.obligations ack=held won=route;\
out=passed@c:265-300;branch=none;ledger=+0:all[s=S];window=boundary;deadline_fired=none",
    "entry=close.delivery[s=S];check=c:227-234[s=S];value=none@close.delivery ack=held won=route;\
out=passed@c:227-234;branch=none;ledger=+0:all[s=S];window=boundary;deadline_fired=none",
    "entry=close.mother_end[s=S];check=c:462-480[s=S];value=none@close.mother_end ack=held won=route;\
out=passed@c:462-480;branch=none;ledger=+1:SessionClosed[s=S];window=boundary;deadline_fired=none",
    "entry=close.primary[s=S];check=c:490-505[s=S];value=none@close.primary ack=held won=route;\
out=recorded@c:490-505;branch=none;ledger=+1:session_closed[s=S];window=boundary;deadline_fired=none",
    "entry=producer.join[s=S];check=c:48-54[s=S];value=none@producer.join ack=held won=route;\
out=passed@c:48-54;branch=none;ledger=+0:all[s=S];window=boundary;deadline_fired=none",
    "entry=close.seal[s=S];check=c:372-429[s=S];value=none@close.seal ack=held won=route;\
out=passed@c:372-429;branch=none;ledger=+0:all[s=S];window=boundary;deadline_fired=none",
];
const CONTROL: &str = "entry=call.gate[s=S q=Q run=RUN inv=INV att=ATT kind=chat ord=4];\
check=model.call.observed[s=S q=Q run=RUN inv=INV att=ATT kind=chat ord=4];\
value=none@model.call.observed ack=held won=route;out=admitted@model.call.admitted;branch=none;\
ledger=+1:ModelCallCharged[inv=INV],+1:model_call[run=RUN att=ATT];window=completed_attempt;\
peer=/api/version,/api/status,/api/tags,/api/chat chat=1;next=lane_returned=1;deadline_fired=none";

/// The row holding `value` for `sink` and `dim`, all 18 columns.
fn row(sink: &str, dim: &str, value: &str) -> Option<Vec<&'static str>> {
    let rows = TABLE.lines().map(|l| l.split('\t').collect::<Vec<_>>());
    let mut rows = rows.filter(|c| c.len() == 18 && c[1] == sink && c[2] == dim);
    rows.find(|c| c[3].split(',').any(|v| v == value))
}
/// The frozen witness of the row holding `value` for `sink` and `dim`.
fn frozen(sink: &str, dim: &str, value: &str) -> Option<&'static str> {
    row(sink, dim, value).map(|c| c[13])
}
/// W0b B1: a witness's shape: each field's name and value text, in order,
/// and the exec group whose placeholders a group runner binds (`S.<group>`).
struct Shape {
    fields: Vec<(&'static str, &'static str)>,
    group: Option<&'static str>,
    /// U21a: a `before@` row with value `none`: nothing is applied at its
    /// acknowledged point, so `ack` renders `none`.
    armed_none: bool,
}
/// The shape of `witness` (fields split at `;`; a tag holds none), or the
/// first field that is not in `ORDER`'s order.
fn shape(witness: &'static str, exec: &'static str) -> Result<Shape, String> {
    let fields: Vec<(&str, &str)> = witness
        .split(';')
        .map(|kv| kv.split_once('=').unwrap_or((kv, "")))
        .collect();
    let mut at = 0;
    for (k, _) in &fields {
        match ORDER[at..].iter().position(|o| o == k) {
            Some(n) => at += n + 1,
            None => return Err(format!("field {k} out of ORDER")),
        }
    }
    let group = exec.strip_prefix("g:");
    let armed_none = false;
    Ok(Shape {
        fields,
        group,
        armed_none,
    })
}
/// The keys of the first identity tag in `text` (`[s=S q=Q ..]`), checked to
/// be an ordered subset of `KEYS`, each with its row value.
fn tag_keys(text: &str) -> Option<Vec<(&str, &str)>> {
    let inner = text.split_once('[')?.1.split_once(']')?.0;
    let keys: Vec<(&str, &str)> = inner
        .split(' ')
        .filter_map(|kv| kv.split_once('='))
        .collect();
    let mut at = 0;
    for (k, _) in &keys {
        at += KEYS[at..].iter().position(|o| o == k)? + 1;
    }
    Some(keys)
}

/// AM-2 A.3/A.4: S and Q are the driver's, RUN and ATT `credit`'s claim, INV
/// the Builder lane's recorded invocation.
struct Bound {
    s: nexus_governed_control::host_session::SessionIdentity,
    q: u64,
    run: Option<nexus_governed_control::authority::ids::RunId>,
    att: Option<u32>,
    inv: Option<nexus_mother::lane::InvocationId>,
}
fn bind(f: &Facts) -> Bound {
    let builder = f.recorded.iter().find(|r| r.0 == LaneRole::Builder);
    Bound {
        s: f.session,
        q: f.request,
        run: f.claim.map(|c| c.0),
        att: f.claim.map(|c| c.1),
        inv: builder.map(|r| r.1),
    }
}
/// What `render` reads: the facts, every observed point (S1, then S5), the
/// binding, whether INV is correlated (A.4), the group suffix and the tables.
struct View<'a> {
    f: &'a Facts,
    seen: Vec<Seen<'a>>,
    b: Bound,
    inv: bool,
    group: String,
    t: &'a Tables<'a>,
}
/// The rendering's view of `f`: every observed point, the binding, A.4's INV.
fn view<'a>(f: &'a Facts, group: String, t: &'a Tables<'a>) -> View<'a> {
    let b = bind(f);
    let correlated = |m: &&nexus_mother::route_seams::Entry| {
        m.point == "mother.charge.entry" && b.inv.is_some() && m.invocation == b.inv
    };
    let inv = f.mother.iter().any(|m| correlated(&m));
    let seen = f
        .broker
        .iter()
        .map(of1)
        .chain(f.mother.iter().map(of5))
        .chain(f.host.iter().map(of7))
        .collect();
    View {
        f,
        seen,
        b,
        inv,
        group,
        t,
    }
}
/// The first admitted entry at `point` under the tag's kind and ordinal.
fn first_at<'a>(v: &View<'a>, point: &str, tag: (Option<&str>, Option<u64>)) -> Option<Seen<'a>> {
    let at = |e: &&Seen| !e.note && e.point == point && admitted(v, e, tag);
    v.seen.iter().find(at).copied()
}
/// The admitted note at `point` under the tag's kind and ordinal.
fn note_at<'a>(v: &View<'a>, point: &str, tag: Tag) -> Option<Seen<'a>> {
    notes(v, tag).into_iter().find(|n| n.point == point)
}
/// The admitted notes under the tag's kind and ordinal.
fn notes<'a>(v: &View<'a>, tag: (Option<&str>, Option<u64>)) -> Vec<Seen<'a>> {
    v.seen
        .iter()
        .filter(|e| e.note && admitted(v, e, tag))
        .copied()
        .collect()
}
/// `TOKENS[0]` and `[1]`: the observed entry, then on its thread, later,
/// exactly one note of this identity and ordinal, `refused:8.cas`, of the
/// chat (kind `chat`, R88.1); no `model.call.admitted` of it.
fn cas<'a>(v: &View<'a>) -> Option<(Seen<'a>, Seen<'a>)> {
    let chat = (Some("chat"), Some(4));
    let o = first_at(v, "model.call.observed", chat)?;
    let admitted = first_at(v, "model.call.admitted", chat);
    let [note] = notes(v, chat)[..] else {
        return None;
    };
    let ok = note.point == "refused:8.cas"
        && note.thread == o.thread
        && note.tick > o.tick
        && admitted.is_none();
    ok.then_some((o, note))
}
/// A location token's entry: a `TOKENS` row's predicate, or a plain point.
fn token<'a>(v: &View<'a>, token: &str, tag: (Option<&str>, Option<u64>)) -> Option<Seen<'a>> {
    match v.t.tokens.iter().find(|t| t.0 == token) {
        Some((_, point, predicate)) => predicate(v, point, tag),
        None => first_at(v, token, tag),
    }
}
/// `e`'s identity under the row's tag keys: each bound key (recorded and
/// equal, or correlated, W0b A1b) as its placeholder with the group suffix;
/// any other key leaves it `Unbound` (R1-1); kind and ordinal as recorded.
fn identity(v: &View, e: &Seen, keys: &[(&str, &str)]) -> Obs {
    let one = |k: &str| match k {
        "kind" => Ok(e.kind.unwrap_or("none").to_string()),
        "ord" => Ok(e.ord.map_or("none".into(), |n| n.to_string())),
        _ if bound(v, Some(e), e.point, k) => Ok(format!("{}{}", placeholder(k), v.group)),
        _ => Err(Unbound),
    };
    let shown: Result<Vec<String>, Unbound> = keys
        .iter()
        .map(|(k, _)| Ok(format!("{k}={}", one(k)?)))
        .collect();
    Ok(shown?.join(" "))
}
/// W0b R1-1: an observation, or `Unbound` when a key its value depends on is
/// not bound. Every field renders through `obs`, the one rule: `Unbound` is
/// `unobserved`, which equals no frozen value.
#[derive(Debug, PartialEq)]
struct Unbound;
type Obs = Result<String, Unbound>;
fn obs(o: Obs) -> String {
    o.unwrap_or_else(|Unbound| "unobserved".into())
}
fn placeholder(k: &str) -> &'static str {
    match k {
        "s" => "S",
        "q" => "Q",
        "run" => "RUN",
        "inv" => "INV",
        _ => "ATT",
    }
}
/// A field's `<token>[..]`: its token, its tag's keys, and the kind and
/// ordinal the tag reads.
type Located<'a> = (
    &'a str,
    Vec<(&'a str, &'a str)>,
    (Option<&'a str>, Option<u64>),
);
fn located(text: &str) -> Located<'_> {
    let token = text.split('[').next().unwrap_or(text);
    let keys = tag_keys(text).unwrap_or_default();
    let get = |k: &str| keys.iter().find(|x| x.0 == k).map(|x| x.1);
    let tag = (get("kind"), get("ord").and_then(|o| o.parse().ok()));
    (token, keys, tag)
}

/// The witness of `f`, in its frozen row's grammar (Behaviour 9; W0b B1).
pub(super) fn render(f: &Facts) -> String {
    render_in(f, &TABLES, &CONTROLS)
}
/// `render` over the given tables (W0b: a wave's rows, or a test's).
fn render_in(f: &Facts, t: &Tables, controls: &[(&str, &str, &'static str)]) -> String {
    let (sink, dim) = f.row;
    let control = controls
        .iter()
        .find(|c| (c.0, c.1) == (sink, dim) && f.value == "none");
    let found = match control {
        Some(c) => Some((c.2, "own", "")),
        None => row(sink, dim, f.value).map(|c| (c[13], c[10], c[11])),
    };
    let Some((witness, exec, apply)) = found else {
        return format!("no row {sink}/{dim}/{}", f.value);
    };
    match shape(witness, exec) {
        Ok(mut shape) => {
            shape.armed_none = f.value == "none" && apply.starts_with("before@");
            rendered(f, &shape, t)
        }
        Err(e) => format!("unrenderable row {sink}/{dim}/{}: {e}", f.value),
    }
}
/// `f` rendered field by field in `shape`'s order.
fn rendered(f: &Facts, shape: &Shape, t: &Tables) -> String {
    let v = &view(f, shape.group.map_or(String::new(), |g| format!(".{g}")), t);
    let text = |k: &str| shape.fields.iter().find(|x| x.0 == k).map_or("", |x| x.1);
    let (entry_token, entry_keys, tag) = located(text("entry"));
    let (check_token, check_keys, _) = located(text("check"));
    let entry = token(v, entry_token, tag);
    let check = token(v, check_token, tag);
    let noted: Vec<&str> = notes(v, tag).iter().map(|n| n.point).collect();
    // W0b A3: a note of the session that no `RECORDS` row admits is shown.
    let stray = v
        .seen
        .iter()
        .filter(|e| e.note && e.s == Some(v.b.s) && !admitted(v, e, tag));
    let stray: Vec<&str> = stray.map(|e| e.point).collect();
    // The branch note: its token's predicate held, or what was noted.
    let branch_text = text("branch");
    let note_token = branch_text.split('@').next().unwrap_or("");
    let note = note_token
        .starts_with("note:")
        .then(|| token(v, note_token, tag))
        .flatten();
    let observed = |shown: String| match (entry.is_some(), stray.is_empty()) {
        (false, _) => Err(Unbound),
        (true, false) => Ok(format!("unadmitted{stray:?}")),
        (true, true) => Ok(shown),
    };
    let branch = observed(match (note, noted.is_empty()) {
        (Some(_), _) => branch_text.to_string(),
        (None, true) => "none".to_string(),
        (None, false) => format!("notes{noted:?}"),
    });
    // Generic outcome classes: `refused:<n>@<loc>` from the location's
    // predicate and its note `note:<n>`; `admitted@`, `passed@` and
    // `recorded@<loc>` from the location's predicate with no note.
    let out_text = text("out");
    let (class, loc) = out_text.split_once('@').unwrap_or((out_text, ""));
    let located_out = token(v, loc, tag);
    let refused = class
        .strip_prefix("refused:")
        .is_some_and(|n| note.is_some() && note_token == format!("note:{n}"));
    let quiet = !class.starts_with("refused:") && noted.is_empty();
    // Otherwise the outcome as observed (U5's): the chat's admission, or what
    // was noted.
    let admission = first_at(v, "model.call.admitted", tag).is_some();
    let out = observed(
        match (located_out.is_some() && (refused || quiet), admission) {
            (true, _) => out_text.to_string(),
            (false, true) => "admitted@model.call.admitted".to_string(),
            (false, false) => format!("notes{noted:?}"),
        },
    );
    let won = |out: &String| match out.starts_with("refused:") {
        true => "value",
        false => "route",
    };
    // R88.1, W0b A1/A2: each acknowledgement (S1 or S5) answers its own held
    // point, admitted under the entry tag, the very trace entry; `held` if
    // the value came while every one stood, `armed` if set before the first.
    let held = f.points.last().copied().unwrap_or("none");
    // U21a: a held point named by its `TOKENS` row (`string`) answers there.
    let answers = |(a, point): (&Ack, &&str)| {
        let a = of(a);
        let named = v.t.tokens.iter().find(|x| x.0 == *point);
        let point: &str = named.map_or(point, |x| x.1);
        a.point == point && !a.note && admitted(v, &a, tag) && v.seen.contains(&a)
    };
    let pairs = f.acks.iter().zip(&f.points);
    let answered =
        !f.acks.is_empty() && f.acks.len() == f.points.len() && pairs.clone().all(answers);
    let first = f.acks.iter().map(|a| of(a).instant).min();
    let ack = match (answered, f.applied.zip(first)) {
        (true, _) if shape.armed_none => "none".to_string(),
        (true, Some((at, first))) if at < first => "armed".to_string(),
        (true, _) => "held".to_string(),
        (false, _) => {
            let shown = f.acks.iter().map(of).map(|a| (a.point, a.kind, a.ord));
            format!("unanswered{:?}", shown.collect::<Vec<_>>())
        }
    };
    // W0b R1-1: every field is an `Obs`, and renders only through `obs`.
    let at = |token: &str, e: Seen, keys: &[(&str, &str)]| -> Obs {
        Ok(format!("{token}[{}]", identity(v, &e, keys)?))
    };
    let values: Vec<String> = shape
        .fields
        .iter()
        .map(|(k, t)| -> Obs {
            match *k {
                "entry" => entry
                    .ok_or(Unbound)
                    .and_then(|e| at(entry_token, e, &entry_keys)),
                "check" => match check {
                    Some(c) => at(check_token, c, &check_keys),
                    None => {
                        let row = v.t.tokens.iter().find(|x| x.0 == check_token);
                        let point = row.map_or(check_token, |x| x.1);
                        let seen = first_at(v, point, tag).ok_or(Unbound);
                        seen.and_then(|e| at(point, e, &check_keys))
                    }
                },
                "value" => {
                    let out = out.as_ref().map_err(|_| Unbound)?;
                    Ok(format!("{}@{held} ack={ack} won={}", f.value, won(out)))
                }
                "out" => out.as_ref().map(String::clone).map_err(|_| Unbound),
                "branch" => branch.as_ref().map(String::clone).map_err(|_| Unbound),
                "ledger" => ledger(v, t),
                "window" => window(v),
                "peer" => {
                    let chat = f.peer.iter().filter(|p| *p == "/api/chat").count();
                    let paths = match f.peer.is_empty() {
                        true => "none".to_string(),
                        false => f.peer.join(","),
                    };
                    Ok(format!("{paths} chat={chat}"))
                }
                "next" => next(v, t),
                "deadline_fired" => deadline(v, &noted),
                other => Ok(format!("unbuilt:{other}")),
            }
        })
        .map(obs)
        .collect();
    let fields = shape
        .fields
        .iter()
        .zip(values)
        .map(|((k, _), x)| format!("{k}={x}"));
    fields.collect::<Vec<_>>().join(";")
}
/// A native record's detail `k`.
fn detail(
    r: &nexus_governed_control::authority::evidence::EvidenceRecord,
    k: &str,
) -> Option<String> {
    let found = r.detail.iter().find(|(key, _)| key == k);
    found.map(|(_, x)| x.clone())
}
/// A native record of the bound attempt (run and `call`).
fn attempt(v: &View, r: &nexus_governed_control::authority::evidence::EvidenceRecord) -> bool {
    let run = v.b.run.map(|r| r.to_string());
    detail(r, "op").as_deref() == Some("model_call")
        && run.is_some()
        && r.run == run
        && detail(r, "call") == v.b.att.map(|a| a.to_string())
}
/// W0b B1, A4: the ledger from `LEDGER`'s closed list, each item `+N:<event>[..]`
/// counted from the facts under its tag's identity: a key the record itself
/// carries (a journal event's invocation, `model_call`'s run and call), or a
/// key bound by a correlation (`native`, `journal`); a record with any tag
/// key unbound is not counted. An event outside the list is shown as such.
fn ledger(v: &View, text: &str) -> Obs {
    let items = text.split("],").map(|i| i.trim_end_matches(']'));
    let shown = items.map(|item| {
        let event = item
            .split_once(':')
            .map_or("", |x| x.1)
            .split('[')
            .next()
            .unwrap_or("");
        let closed = format!("{item}]");
        let keys = tag_keys(&closed).unwrap_or_default();
        let tag = keys
            .iter()
            .map(|(k, _)| format!("{k}={}{}", placeholder(k), v.group));
        let tag = tag.collect::<Vec<_>>().join(" ");
        let named = |e: &nexus_mother::journal::Entry| match e.event {
            Event::ModelCallCharged { invocation, .. } => ("ModelCallCharged", Some(invocation)),
            Event::LaneReturned { invocation, .. } => ("LaneReturned", Some(invocation)),
            Event::SessionClosed => ("SessionClosed", None),
            _ => ("", None),
        };
        let by_inv = |e: &&nexus_mother::journal::Entry| {
            let (name, invocation) = named(e);
            name == event && (invocation.is_none() || invocation == v.b.inv)
        };
        let native = |r: &&nexus_governed_control::authority::evidence::EvidenceRecord| {
            r.phase.as_str() == event && (event != "model_call" || attempt(v, r))
        };
        let Some(source) = LEDGER.iter().find(|l| l.0 == event).map(|l| l.1) else {
            return Ok(format!("unlisted:{event}"));
        };
        let carried: &[&str] = match event {
            "ModelCallCharged" | "LaneReturned" => &["inv"],
            "model_call" => &["run", "att"],
            _ => &[],
        };
        // SEAM-1 T2: the end record's S1 commits of `event`; counted only on
        // the acknowledged close.primary's thread, after it.
        let primary = acked(v, "close.primary", (None, None));
        let commits = |after: bool| {
            let commit = |e: &&s1::Entry| {
                e.point == "store.commit.result" && !e.note && e.phase.as_deref() == Some(event)
            };
            let later =
                |e: &&s1::Entry| primary.is_some_and(|a| a.thread == e.thread && e.tick > a.tick);
            let found = v.f.broker.iter().filter(commit);
            found.filter(|e| !after || later(e)).count()
        };
        let sources = if source == "all" {
            "native journal"
        } else {
            source
        };
        let unbound = keys.iter().any(|(k, _)| {
            !carried.contains(k) && sources.split(' ').any(|s| !bound(v, None, s, k))
        });
        // R1-1: a candidate record with any tag key unbound (by the record or
        // a correlation; the Builder's invocation; the claim) is unobserved,
        // never a count. With no candidate, nothing is to bind: `+0` is seen.
        let unbound = unbound
            || carried.contains(&"inv") && v.b.inv.is_none()
            || carried.contains(&"run") && (v.b.run.is_none() || v.b.att.is_none());
        let candidates = match source {
            "journal" => v.f.journal.iter().filter(|e| named(e).0 == event).count(),
            "native" => {
                v.f.native
                    .iter()
                    .filter(|r| r.phase.as_str() == event)
                    .count()
            }
            "commit" => commits(false),
            _ => v.f.journal.len() + v.f.native.len() + host_commits(v),
        };
        if unbound && candidates > 0 {
            return Err(Unbound);
        }
        let n = match source {
            "journal" => v.f.journal.iter().filter(by_inv).count(),
            "native" => v.f.native.iter().filter(native).count(),
            "commit" => commits(true),
            _ => v.f.journal.len() + v.f.native.len() + host_commits(v),
        };
        Ok(format!("+{n}:{event}[{tag}]"))
    });
    Ok(shown.collect::<Result<Vec<_>, _>>()?.join(","))
}
/// U21a: a `close_report` cell's primary records in its window, with no
/// projection in a `hands::run` child: its S1 commits after its S7
/// acknowledgement (0 for every other cell).
fn host_commits(v: &View) -> usize {
    let s7 = |a: &Ack| match a {
        Ack::S7(e) => Some(e.instant),
        _ => None,
    };
    let Some(first) = v.f.acks.iter().find_map(s7) else {
        return 0;
    };
    let commit = |e: &&s1::Entry| e.point == "store.commit" && !e.note && e.instant > first;
    v.f.broker.iter().filter(commit).count()
}
/// An attempt site of the bound attempt, admitted (A.1: session, run and
/// attempt, no request). R1-1: a site of S with the attempt unbound is
/// unobserved; with no site of S, the window is a boundary.
fn window(v: &View) -> Obs {
    let sites = ["model.attempt.record.before", "complete"];
    let of_s = v
        .seen
        .iter()
        .any(|e| e.s1 && sites.contains(&e.point) && e.s == Some(v.b.s));
    match sites.iter().all(|p| first_at(v, p, (None, None)).is_some()) {
        true => Ok("completed_attempt".into()),
        false if of_s && (v.b.run.is_none() || v.b.att.is_none()) => Err(Unbound),
        false => Ok("boundary".into()),
    }
}
/// W0b B1: `next`'s keys from the facts; `lane_returned` is built (U5), any
/// other key is left to the unit that adds it. R1-1: a `LaneReturned` with no
/// Builder invocation bound is unobserved.
fn next(v: &View, text: &str) -> Obs {
    let lane = |e: &&nexus_mother::journal::Entry| matches!(e.event, Event::LaneReturned { .. });
    let returned = |e: &&nexus_mother::journal::Entry| matches!(e.event, Event::LaneReturned { invocation, .. } if Some(invocation) == v.b.inv);
    if v.b.inv.is_none() && v.f.journal.iter().any(|e| lane(&e)) {
        return Err(Unbound);
    }
    let keys = text.split(',').map(|kv| kv.split('=').next().unwrap_or(""));
    let shown = keys.map(|k| match k {
        "lane_returned" => format!(
            "lane_returned={}",
            v.f.journal.iter().filter(returned).count()
        ),
        other => format!("{other}=unbuilt"),
    });
    Ok(shown.collect::<Vec<_>>().join(","))
}
/// `deadline_fired`: an expiry note or a native timeout of the attempt.
fn deadline(v: &View, noted: &[&str]) -> Obs {
    // R1-1: a model call record with the attempt unbound is unobserved.
    let call = |r: &&nexus_governed_control::authority::evidence::EvidenceRecord| {
        r.phase.as_str() == "model_call"
    };
    if (v.b.run.is_none() || v.b.att.is_none()) && v.f.native.iter().any(|r| call(&r)) {
        return Err(Unbound);
    }
    let expired: Vec<&str> = noted
        .iter()
        .copied()
        .filter(|n| n.starts_with("refused:1"))
        .collect();
    let attempt = |r: &&nexus_governed_control::authority::evidence::EvidenceRecord| attempt(v, r);
    let mut outcomes =
        v.f.native
            .iter()
            .filter(attempt)
            .map(|r| detail(r, "outcome"));
    let timed_out = outcomes.any(|o| o.as_deref() == Some("timeout"));
    let shown =
        v.f.native
            .iter()
            .filter(attempt)
            .map(|r| detail(r, "outcome"));
    println!("WITNESS native outcome {:?}", shown.collect::<Vec<_>>());
    Ok(match (expired.is_empty(), timed_out) {
        (true, false) => "none".to_string(),
        _ => format!("expired{expired:?} timeout={timed_out}"),
    })
}

/// Design 3.3: the child's result file must hold exactly one `CELL <nonce>
/// <id> <witness>` line, of this run's nonce and this cell, and nothing else
/// (a `TIMEOUT` line included). The witness, or the refusal's name.
fn record(r: &Ran) -> Result<&str, &'static str> {
    if !r.passed {
        return Err("record: the child failed");
    }
    written(r)
}
/// `record`'s checks of the record itself, whatever the child's exit (a
/// driver's finding fails the child after its record is written).
fn written(r: &Ran) -> Result<&str, &'static str> {
    let lines: Vec<&str> = r.result.lines().collect();
    if lines.iter().any(|l| !l.starts_with("CELL ")) {
        return Err("record: a line that is not a CELL record");
    }
    let [line] = lines[..] else {
        return Err(match lines.len() {
            0 => "record: missing",
            _ => "record: duplicate",
        });
    };
    let mut parts = line.splitn(4, ' ').skip(1);
    let (nonce, id) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if nonce != r.nonce {
        return Err("record: not this run's nonce (stale or wrong)");
    }
    if id != r.id {
        return Err("record: another cell's id");
    }
    parts.next().ok_or("record: no witness")
}

/// The fields of `expected` and `observed` that differ, each with both
/// values; `observed` must also equal `expected` byte for byte.
fn compare(expected: &str, observed: &str) -> Vec<String> {
    let split = |w: &str| -> Vec<(String, String)> {
        let fields = w
            .split(';')
            .map(|kv| kv.split_once('=').unwrap_or((kv, "")));
        fields
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let (want, seen) = (split(expected), split(observed));
    let mut diffs = Vec::new();
    let mut keys: Vec<&str> = want.iter().map(|(k, _)| k.as_str()).collect();
    let extra = seen.iter().map(|(k, _)| k.as_str());
    let extra: Vec<&str> = extra.filter(|k| !keys.contains(k)).collect();
    keys.extend(extra);
    for key in keys {
        let value = |of: &[(String, String)]| {
            let found = of.iter().find(|(k, _)| k == key);
            found.map_or("<absent>".to_string(), |(_, v)| v.clone())
        };
        let (w, s) = (value(&want), value(&seen));
        if w != s {
            diffs.push(format!("{key}: expected {w:?}, observed {s:?}"));
        }
    }
    if diffs.is_empty() && expected != observed {
        diffs.push(format!(
            "bytes: expected {expected:?}, observed {observed:?}"
        ));
    }
    diffs
}

/// Behaviour 10, W0b C2: the control's record, and its witness against its
/// `(sink, dim)` row of `CONTROLS`; no row is a named failure.
pub(super) fn control(r: &Ran, sink: &str, dim: &str) {
    control_in(r, (sink, dim), &CONTROLS)
}
fn control_in(r: &Ran, (sink, dim): (&str, &str), controls: &[(&str, &str, &'static str)]) {
    let row = controls.iter().find(|c| (c.0, c.1) == (sink, dim));
    let want = row
        .unwrap_or_else(|| panic!("{}: no CONTROLS row {sink}/{dim}", r.id))
        .2;
    let witness = match record(r) {
        Ok(w) => w,
        Err(e) => panic!("{}: {e}\n{}{}", r.id, r.result, r.output),
    };
    let diffs = compare(want, witness);
    assert!(diffs.is_empty(), "{}: {}", r.id, diffs.join("; "));
}

/// SB3E-R86.4 (U5b; Behaviour 1): a credited cell's record and witness
/// against its frozen row, field by field and byte for byte, then the
/// child's own exit; every difference is named, the witness's first.
pub(super) fn credited(r: &Ran, sink: &str, dim: &str, value: &str) {
    let want = frozen(sink, dim, value);
    let want = want.unwrap_or_else(|| panic!("{}: no row {sink}/{dim}/{value}", r.id));
    let mut found = match written(r) {
        Ok(witness) => compare(want, witness),
        Err(e) => vec![e.to_string()],
    };
    if !r.passed {
        found.push("record: the child failed".into());
    }
    for line in r.output.lines().filter_map(|l| l.split_once("WITNESS ")) {
        println!("{}: {}", r.id, line.1); // Measured evidence (--nocapture).
    }
    let shown = (&r.result, &r.output);
    assert!(
        found.is_empty(),
        "{}: {}\n{}{}",
        r.id,
        found.join("; "),
        shown.0,
        shown.1
    );
}

/// Rows 93-96 and 98-100 (the `call_chat` ingress cells but the held row
/// 97) hold exactly the rendered fields; every location token they use is one of `TOKENS`, or is the
/// budget row's (`model.call.sampled`, `note:1`), which U5 does not map.
/// W0b B1: and every row of the table renders: its fields an ordered subset of
/// `ORDER`, each identity tag an ordered subset of `KEYS` and exactly what
/// `identity` gives an entry of the bound identity with the tag's kind and
/// ordinal (no token predicate).
#[test]
fn witness_rows_hold_the_rendered_fields() {
    super::child("witness::witness_rows_hold_the_rendered_fields", |_| {
        coverage()
    });
    let values = [
        "original_cell_unscheduled",
        "repeated",
        "mission_stop",
        "stop_all",
        "fatal",
        "quit",
        "budget",
    ];
    for value in values {
        let witness = frozen("call_chat", "ingress", value).expect(value);
        let keys: Vec<&str> = witness
            .split(';')
            .filter_map(|kv| kv.split_once('='))
            .map(|kv| kv.0)
            .collect();
        // W0b N8: U5's exact field list, not the witness against itself.
        let u5 = "entry check value out branch ledger window peer next deadline_fired";
        assert_eq!(keys.join(" "), u5, "{value}");
        let check = witness.split(";check=").nth(1).unwrap_or("");
        let token = check.split('[').next().unwrap_or("");
        let branch = witness.split(";branch=").nth(1).unwrap_or("");
        let note = branch.split('@').next().unwrap_or("");
        let mapped = |t: &str| TOKENS.iter().any(|m| m.0 == t);
        let budget = value == "budget" && token == "model.call.sampled" && note == "note:1";
        assert!(
            budget || (mapped(token) && mapped(note)),
            "{value}: {token} {note}"
        );
    }
    assert_eq!(frozen("call_chat", "ingress", "no_such_value"), None);
    // SB3E-R49.1 (U5b): row 97 is held; it states no witness.
    assert_eq!(frozen("call_chat", "ingress", "door_fence"), Some("hold"));
}

/// W0b B1: the coverage over every row (`witness_rows_hold_the_rendered_fields`).
fn coverage() {
    let f = refused();
    let (mut sets, mut tags, mut flags, mut rows) = (Vec::new(), Vec::new(), Vec::new(), 0);
    for line in TABLE.lines().skip(1) {
        let c: Vec<&'static str> = line.split('\t').collect();
        if c[13] == "hold" {
            continue;
        }
        rows += 1;
        let s = shape(c[13], c[10]).unwrap_or_else(|e| panic!("{}: {e}", c[..4].join(" ")));
        let keys: Vec<&str> = s.fields.iter().map(|x| x.0).collect();
        if !sets.contains(&keys) {
            sets.push(keys);
        }
        let group = s.group.map_or(String::new(), |g| format!(".{g}"));
        let v = &View {
            f: &f,
            seen: Vec::new(),
            b: bind(&f),
            inv: true,
            group,
            t: &TABLES,
        };
        for (field, text) in &s.fields {
            for tag in text
                .split('[')
                .skip(1)
                .map(|t| t.split(']').next().unwrap_or(""))
            {
                if !tag.contains('=') {
                    flags.push(format!("{field}[{tag}]"));
                    continue;
                }
                let bracketed = format!("[{tag}]");
                let keys = tag_keys(&bracketed).unwrap_or_else(|| panic!("{tag}"));
                let get = |k: &str| keys.iter().find(|x| x.0 == k).map(|x| x.1);
                let mut e = f.broker[0].clone();
                e.kind = get("kind").map(|k| &*Box::leak(k.to_string().into_boxed_str()));
                e.request_ordinal = get("ord").and_then(|o| o.parse().ok());
                let shown = match *field {
                    "ledger" => {
                        let shown = keys
                            .iter()
                            .map(|(k, _)| format!("{k}={}{}", placeholder(k), v.group));
                        shown.collect::<Vec<_>>().join(" ")
                    }
                    _ => obs(identity(v, &of1(&e), &keys)),
                };
                assert_eq!(shown, tag, "{}: {field}", c[..4].join(" "));
                let form: Vec<&str> = keys.iter().map(|x| x.0).collect();
                let form = (form.join(" "), s.group.is_some());
                if !tags.contains(&form) {
                    tags.push(form);
                }
            }
        }
    }
    flags.sort();
    flags.dedup();
    println!(
        "WITNESS B1 coverage: {rows} rows, {} field sets, {} tag sets, flags {flags:?}",
        sets.len(),
        tags.len()
    );
    for s in &sets {
        println!("WITNESS B1 field set {}", s.join(";"));
    }
    for t in &tags {
        println!("WITNESS B1 tag set [{}] group={}", t.0, t.1);
    }
    // U21a (SB3E-R197 TR-1): six close_report rows are held (no witness).
    assert_eq!(rows, 788, "every row with a witness");
    // W0b B2', U6a-1: `collect_reg`'s sinks are exactly the no-Mother drivers' rows.
    let rows = TABLE.lines().map(|l| l.split('\t').collect::<Vec<_>>());
    let no_mother = |c: &Vec<&str>| ["d_register", "d_bootstrap"].contains(&c[6]);
    let mut sinks: Vec<&str> = rows.filter(no_mother).map(|c| c[1]).collect();
    sinks.sort();
    sinks.dedup();
    assert_eq!(sinks, super::s5::BROKER_SINKS);
    // U9a M1: `collect_close`'s sinks are exactly the `d_close` rows'.
    let rows = TABLE.lines().map(|l| l.split('\t').collect::<Vec<_>>());
    let mut close: Vec<&str> = rows.filter(|c| c[6] == "d_close").map(|c| c[1]).collect();
    close.sort();
    close.dedup();
    assert_eq!(close, super::s5::CLOSE_SINKS);
    // U9b: `HELD_SINKS` are exactly the `d_quit` and `d_stop` rows' sinks.
    let rows = TABLE.lines().map(|l| l.split('\t').collect::<Vec<_>>());
    let quit_stop = |c: &Vec<&str>| ["d_quit", "d_stop"].contains(&c[6]);
    let mut held: Vec<&str> = rows.filter(quit_stop).map(|c| c[1]).collect();
    held.sort();
    held.dedup();
    assert_eq!(held, super::s5::HELD_SINKS);
}

/// W0b B1 (Behaviour 4): a renderer control per new capability, each a named
/// difference: a row's field missing from the rendering, an extra field, a
/// wrong identity tag set, `ack=armed` rendered for a held value, a ledger
/// event outside the closed list.
#[test]
fn witness_renderer_controls_each_differ() {
    super::child("witness::witness_renderer_controls_each_differ", |_| {
        let f = refused();
        let base = render(&f);
        let want = frozen("call_chat", "ingress", "quit").unwrap();
        let mut armed = refused();
        armed.applied = armed
            .acks
            .first()
            .map(|a| of(a).instant - std::time::Duration::from_millis(1));
        let leak = |w: String| -> &'static str { Box::leak(w.into_boxed_str()) };
        let untagged = leak(want.replacen(" inv=INV", "", 1));
        let unlisted = leak(want.replace("+1:model_call[run=RUN att=ATT]", "+1:Foo[s=S]"));
        let dropped = base
            .split(';')
            .filter(|kv| !kv.starts_with("window="))
            .collect::<Vec<_>>();
        for (control, observed, field) in [
            ("a field missing", dropped.join(";"), "window"),
            ("an extra field", format!("{base};extra=x"), "extra"),
            (
                "a wrong tag set",
                rendered(&f, &shape(untagged, "own").unwrap(), &TABLES),
                "entry",
            ),
            ("ack=armed for a held value", render(&armed), "value"),
            (
                "an unlisted ledger event",
                rendered(&f, &shape(unlisted, "own").unwrap(), &TABLES),
                "ledger",
            ),
        ] {
            let diffs = compare(&base, &observed);
            let named = diffs.iter().any(|d| d.starts_with(&format!("{field}: ")));
            assert!(named, "{control}: {diffs:?}");
            println!("WITNESS B1 control {control:?}: {diffs:?}");
        }
        // W0b R1-1: the one rule, by structure. With the claim and the lanes
        // unbound and a candidate for each field, every U5 field but `peer`
        // (the fixture's) and `deadline_fired` (no native candidate can be
        // built here) is `unobserved`, and none renders its frozen value.
        let (mut unbound, inv) = (refused(), refused().recorded[0].1);
        (unbound.claim, unbound.recorded) = (None, Vec::new());
        let charged = Event::ModelCallCharged {
            invocation: inv,
            call: 1,
        };
        let returned = Event::LaneReturned {
            invocation: inv,
            output_hash: "h".into(),
        };
        for event in [charged, returned] {
            let mission = nexus_mother::journal::MissionId(1);
            unbound
                .journal
                .push(nexus_mother::journal::Entry::chained(0, "", mission, event));
        }
        let mut site = unbound.broker[0].clone();
        site.point = "model.attempt.record.before";
        unbound.broker.push(site);
        for value in
            "original_cell_unscheduled repeated mission_stop stop_all fatal quit".split(' ')
        {
            unbound.value = value;
            let (shown, want) = (
                render(&unbound),
                frozen("call_chat", "ingress", value).unwrap(),
            );
            let open = |kv: &&str| kv.starts_with("peer=") || kv.starts_with("deadline_fired=");
            let mut pairs = shown
                .split(';')
                .zip(want.split(';'))
                .filter(|(kv, _)| !open(kv));
            let rule = pairs.all(|(kv, w)| kv.ends_with("=unobserved") && kv != w);
            assert!(rule, "R1-1 {value}: {shown}");
        }
        // W0b N-a: an entry a token returns unadmitted, of another session
        // (its s recorded but unequal), renders no identity.
        let other = nexus_governed_control::host_session::SessionSeed::new()
            .unwrap()
            .identity();
        let foreign = s1::Entry {
            session: Some(other),
            ..f.broker[0].clone()
        };
        let v = &view(&f, String::new(), &TABLES);
        assert_eq!(
            identity(v, &of1(&foreign), &[("s", "S")]),
            Err(Unbound),
            "N-a"
        );
    });
}

/// Design 3.3: a missing, a duplicate, a stale and a wrong-nonce `CELL`
/// record, another cell's and a `TIMEOUT` line are each refused; the one
/// record of this run is read.
#[test]
fn witness_result_records_each_refused() {
    let ran = |result: &str| Ran {
        id: "ctl_call_chat",
        nonce: "n2".into(),
        result: result.into(),
        passed: true,
        output: String::new(),
    };
    let good = "CELL n2 ctl_call_chat entry=x;check=y\n";
    assert_eq!(record(&ran(good)), Ok("entry=x;check=y"));
    for (plant, result, reason) in [
        ("missing", "", "record: missing"),
        ("duplicate", &*format!("{good}{good}"), "record: duplicate"),
        (
            "stale",
            "CELL n1 ctl_call_chat entry=x;check=y\n",
            "record: not this run's nonce (stale or wrong)",
        ),
        (
            "wrong nonce",
            "CELL m7 ctl_call_chat entry=x;check=y\n",
            "record: not this run's nonce (stale or wrong)",
        ),
        (
            "another cell",
            "CELL n2 rc_r5_call_chat_ingress_quit entry=x\n",
            "record: another cell's id",
        ),
        (
            "a timeout",
            &*format!("{good}TIMEOUT u5 call.gate\n"),
            "record: a line that is not a CELL record",
        ),
    ] {
        assert_eq!(record(&ran(result)), Err(reason), "{plant}");
    }
    let failed = Ran {
        passed: false,
        ..ran(good)
    };
    assert_eq!(record(&failed), Err("record: the child failed"));
    // W0b C2: a non-U5 control against its own `CONTROLS` row (U9a: a pair
    // with no control, now that seal/lifecycle has one); another row's
    // witness, or no row, fails (the missing one by name).
    let fails = |controls: &[(&str, &str, &'static str)]| {
        let run = std::panic::catch_unwind(|| {
            control_in(&ran(good), ("call_chat", "lifecycle"), controls)
        });
        run.err()
            .map(|p| p.downcast::<String>().map_or(String::new(), |m| *m))
    };
    assert_eq!(
        fails(&[("call_chat", "lifecycle", "entry=x;check=y")]),
        None
    );
    assert!(fails(&[("call_chat", "lifecycle", "entry=z;check=y")]).is_some());
    let missing = fails(&CONTROLS).unwrap_or_default();
    assert!(
        missing.contains("no CONTROLS row call_chat/lifecycle"),
        "{missing}"
    );
}

/// W0b renderer-level check (SB3E-R111-R115; not a readiness proof): wave-1
/// rows (`u9a`, `u6a1`, `u6b2`) whose points exist in the route today, one per
/// identity tag set, trace (S1/S5) and acknowledgement kind, each rendered byte
/// for byte from synthetic facts that record exactly the route's identity at each
/// point (close.rs:49, 232, 271, 368, 407; model.rs:383; S5's `route_ctx`),
/// under test-local tokens only. `label|sink dim value|flag|entries`; an
/// entry is `[!][5]point[:keys]` (keys after the last `:`): `!` a note, `5`
/// S5's (of S, Q and the Builder; `inv'` the Sentinel's), S1 keys `s` (`s'`
/// another session), `q` (`q2` another request), `run`, `ordN`; flag `armed`
/// (applied before the acknowledgement) or `noclaim`. A `not` row must not
/// render. Whether a real driver and `collect` can produce these facts is not
/// shown here: readiness-ledger.md holds every such row; 115 fails closed.
const RENDERED: &str = "\
573|close_obligations lifecycle Normal|armed|close.obligations:s
574|close_obligations lifecycle Bootstrap|-|close.obligations:s
700|seal lifecycle ClosingNormal|armed|close.seal:s q
705|seal lifecycle ClosingRestricted|-|close.seal:s q,!seal:stop:s
110|call_slot_reserve ingress original_cell_unscheduled|-|attempt.slot:s run,call.gate:s q run ord1,5mother.lane.prelock
338|meter_settle ingress original_cell_unscheduled|-|5mother.settle
not 573 another session|close_obligations lifecycle Normal|armed|close.obligations:s'
not 700 close.seal of another request|seal lifecycle ClosingNormal|armed|close.seal:s q2
not 700 with 705's note|seal lifecycle ClosingNormal|armed|close.seal:s q,!seal:stop:s
not 705 the note removed|seal lifecycle ClosingRestricted|-|close.seal:s q
not 110 no call.gate (q)|call_slot_reserve ingress original_cell_unscheduled|-|attempt.slot:s run,5mother.lane.prelock
not 110 another lane's prelock (inv)|call_slot_reserve ingress original_cell_unscheduled|-|attempt.slot:s run,call.gate:s q run ord1,5mother.lane.prelock:inv'
not 110 no claim (att, run)|call_slot_reserve ingress original_cell_unscheduled|noclaim|attempt.slot:s run,call.gate:s q run ord1,5mother.lane.prelock
not 338 no claim (run, att)|meter_settle ingress original_cell_unscheduled|noclaim|5mother.settle
not 338 another builder|meter_settle ingress original_cell_unscheduled|-|5mother.settle:inv'
not 115 no sound binding (q, att, run)|call_slot_reserve ingress fatal|noclaim|attempt.slot:s run
";
fn renderer_level() {
    let tokens: [Token; 5] = [
        ("c:265-300", "close.obligations", first_at),
        ("c:372-429", "close.seal", first_at),
        ("note:stop_obligations", "seal:stop", note_at),
        ("m:371-381", "attempt.slot", first_at),
        ("mc:296-300", "mother.settle", first_at),
    ];
    let t = &tables(&tokens);
    let base = refused();
    let other = nexus_governed_control::host_session::SessionSeed::new()
        .unwrap()
        .identity();
    for line in RENDERED.lines() {
        let [label, row, flag, entries] = line.split('|').collect::<Vec<_>>()[..] else {
            panic!("{line}")
        };
        let [sink, dim, value] = row.split(' ').collect::<Vec<_>>()[..] else {
            panic!("{line}")
        };
        let (mut f, mut acks) = (base.clone(), Vec::new());
        (f.row, f.value, f.broker, f.mother) = ((sink, dim), value, Vec::new(), Vec::new());
        f.claim = f.claim.filter(|_| flag != "noclaim");
        for (tick, text) in (1..).zip(entries.split(',')) {
            let (note, text) = text.strip_prefix('!').map_or((false, text), |t| (true, t));
            let (s5, text) = text.strip_prefix('5').map_or((false, text), |t| (true, t));
            let (point, keys) = text.rsplit_once(':').unwrap_or((text, ""));
            let has = |k: &str| keys.split(' ').any(|x| x == k);
            if s5 {
                let mut e = base.mother[0].clone();
                (e.tick, e.point) = (tick, point);
                e.invocation = Some(base.recorded[usize::from(has("inv'"))].1);
                acks.push(Ack::S5(e.clone()));
                f.mother.push(e);
                continue;
            }
            let mut e = base.broker[0].clone();
            (e.tick, e.point, e.note, e.attempt, e.kind) = (tick, point, note, None, None);
            e.session =
                (has("s") || has("s'")).then_some(if has("s'") { other } else { f.session });
            e.request = [("q", 1), ("q2", 2)]
                .into_iter()
                .find(|k| has(k.0))
                .map(|k| k.1);
            e.run = e.run.filter(|_| has("run"));
            e.request_ordinal = (1..5).find(|n| has(&format!("ord{n}")));
            acks.push(Ack::S1(e.clone()));
            f.broker.push(e);
        }
        f.acks = acks.into_iter().take(1).collect();
        f.points = f
            .acks
            .iter()
            .map(|a| match a {
                Ack::S1(e) => e.point,
                Ack::S5(e) => e.point,
                Ack::S7(e) => e.point,
            })
            .collect();
        let first = f.acks.first().map(|a| of(a).instant);
        f.applied = first
            .filter(|_| flag == "armed")
            .and_then(|i| i.checked_sub(std::time::Duration::from_millis(1)));
        let want = frozen(sink, dim, value).unwrap_or_else(|| panic!("{label}: no row"));
        let shown = render_in(&f, t, &[]);
        match label.starts_with("not ") {
            false => assert_eq!(shown, want, "renderer-level {label}"),
            true => assert_ne!(shown, want, "renderer-level {label}"),
        }
        println!(
            "WITNESS renderer-level {label}: {}",
            compare(want, &shown).join("; ")
        );
    }
}
#[test]
fn witness_wave_rows_render_from_recorded_identity() {
    super::child(
        "witness::witness_wave_rows_render_from_recorded_identity",
        |_| renderer_level(),
    );
}
/// W0b A4, R1-1, N-c: `model_registered` of a real registration through
/// `open_broker`, counted from the opening's own projection since it opened,
/// its q bound by the registration's own `model.record.before` and
/// `model.install.before` of S and Q (model.rs:301-314). A bracketing entry of another request leaves q unbound, so the
/// field is `unobserved`, never `+0`; an applier's `ingress.accept` of another
/// request (U5's `accept(Quit, 2)`) changes nothing. The window holds only
/// the registration's record (N-c i: `witness_collects_after_the_mother_is_released`).
/// Row 440 renders at the renderer level: no value was applied (R2-4).
#[test]
fn witness_native_ledger_binds_by_fixture_and_request() {
    super::child(
        "witness::witness_native_ledger_binds_by_fixture_and_request",
        native,
    );
}
fn native(f: &super::Fixture) {
    let b = super::s5::open_broker(f);
    b.o.session
        .register_local_model(&b.endpoint, super::spec())
        .unwrap();
    let row = (("registration_record", "ingress"), "quit");
    let facts = super::s5::collect_broker(&b, (Vec::new(), Vec::new()), row);
    let text = "+1:model_registered[s=S q=Q]";
    let led = |f: &Facts| obs(ledger(&view(f, String::new(), &TABLES), text));
    let with = |point, request| {
        let (mut planted, mut e) = (facts.clone(), facts.broker[0].clone());
        (e.point, e.request) = (point, Some(request));
        planted.broker.push(e);
        led(&planted)
    };
    assert_eq!(led(&facts), text, "A4: {}", led(&facts));
    let q = with("model.install.before", 7);
    assert_eq!(q, "unobserved", "A4 q: {q}");
    assert_eq!(with("ingress.accept", 2), text, "A4 accept");
    let phases: Vec<&str> = facts.native.iter().map(|r| r.phase.as_str()).collect();
    assert_eq!(phases, ["model_registered"], "A4 window: {phases:?}");
    // Row 440 (u6a1 `registration_record quit`) at the renderer level: the facts
    // of a real registration, with no value applied (R2-4).
    let (point, mut row440) = ("model.record.before", facts.clone());
    let ack = facts.broker.iter().find(|e| e.point == point);
    (row440.points, row440.acks) = (vec![point], ack.cloned().map(Ack::S1).into_iter().collect());
    let shown = render_in(&row440, &tables(&[("m:292-299", point, first_at)]), &[]);
    assert_eq!(
        shown,
        frozen("registration_record", "ingress", "quit").unwrap()
    );
}
/// W0b R1-2: a close-chain cell. `ctl_call_chat`'s path, then the Mother's
/// `FileSession` released by value, then the collection (`s5::released`):
/// credit and T6 hold, and the witness is `CONTROL`, byte for byte.
#[test]
fn witness_collects_after_the_mother_is_released() {
    super::child(
        "witness::witness_collects_after_the_mother_is_released",
        |f| {
            let facts = super::s5::released(f);
            assert!(facts.findings.is_empty(), "{:?}", facts.findings);
            assert_eq!(compare(CONTROL, &render(&facts)), Vec::<String>::new());
            // W0b N-c (i): the boot's records (consent, the door) precede the
            // window; only the cell's own are in it.
            let phases: Vec<&str> = facts.native.iter().map(|r| r.phase.as_str()).collect();
            assert_eq!(
                phases,
                ["run_opened", "model_call", "run_cancelled"],
                "window: {phases:?}"
            );
        },
    );
}
/// Behaviour 9 (Other plants): over one refusing attempt built from S1's own
/// entry type, `CAS:m:774` and `note:8` map, and each plant fails: the note
/// `refused:8.capacity`, INV bound to a scripted lane's invocation, the
/// `refused:8.cas` note from another thread, an ord-3 `model.call.observed`.
#[test]
fn witness_token_map_plants_each_fail() {
    super::child("witness::witness_token_map_plants_each_fail", |_| {
        let f = refused();
        let fields = |w: &str| -> Vec<String> {
            let parts = w.split(';').filter(|kv| {
                ["check", "out", "branch"]
                    .iter()
                    .any(|k| kv.starts_with(&format!("{k}=")))
            });
            parts.map(String::from).collect()
        };
        let mapped = fields(&render(&f));
        let tags = "s=S q=Q run=RUN inv=INV att=ATT kind=chat ord=4";
        let want = [
            format!("check=CAS:m:774[{tags}]"),
            "out=refused:8@CAS:m:774".into(),
            "branch=note:8@m:774-783".into(),
        ];
        assert_eq!(mapped, want);
        type Plant = fn(&mut Facts);
        let plants: [(&str, Plant); 4] = [
            ("refused:8.capacity", |f| {
                f.broker[2].point = "refused:8.capacity"
            }),
            ("INV bound to a scripted lane", |f| {
                f.recorded[0].1 = f.recorded[1].1
            }),
            ("the note from another thread", |f| {
                f.broker[2].thread = std::thread::spawn(|| std::thread::current().id())
                    .join()
                    .unwrap()
            }),
            ("an ord-3 model.call.observed", |f| {
                f.broker[1].request_ordinal = Some(3)
            }),
        ];
        for (plant, edit) in plants {
            let mut f = refused();
            edit(&mut f);
            let planted = fields(&render(&f));
            assert_ne!(planted, mapped, "{plant}: still mapped");
            println!("WITNESS token-map plant {plant:?}: {planted:?}");
        }
    });
}
/// SB3E-R88.1: over the same refusing attempt, an acknowledgement rewritten
/// to `kind=metadata, ord=3`, and the `refused:8.cas` note made
/// `kind=metadata`, each leave a named difference (on 7d27cde5, measured RED,
/// both compared equal).
#[test]
fn witness_acknowledgement_and_note_plants_each_differ() {
    super::child(
        "witness::witness_acknowledgement_and_note_plants_each_differ",
        |_| {
            let base = render(&refused());
            let mut acks = refused();
            for a in acks.acks.iter_mut() {
                if let Ack::S1(a) = a {
                    (a.kind, a.request_ordinal) = (Some("metadata"), Some(3));
                }
            }
            let mut note = refused();
            note.broker[2].kind = Some("metadata");
            for (plant, f, fields) in [
                ("acks kind=metadata ord=3", acks, &["value"][..]),
                ("note kind=metadata", note, &["check", "out", "branch"][..]),
            ] {
                let diffs = compare(&base, &render(&f));
                let named = |k: &&str| diffs.iter().any(|d| d.starts_with(&format!("{k}: ")));
                assert!(fields.iter().all(named), "{plant}: {diffs:?}");
                println!("WITNESS R88 plant {plant:?}: {diffs:?}");
            }
        },
    );
}
/// One refused ord-4 chat (`call.gate`, `model.call.observed`, then the note
/// `refused:8.cas` on its thread), its charge entry under the Builder's
/// invocation; the scripted Sentinel's invocation is recorded too.
fn refused() -> Facts {
    use nexus_governed_control::authority::ids::RunId;
    use nexus_mother::journal::MissionId;
    use nexus_mother::lane::InvocationId;
    let session = nexus_governed_control::host_session::SessionSeed::new()
        .unwrap()
        .identity();
    let run = RunId::parse(&format!("run-{}", "7".repeat(32))).unwrap();
    let (builder, sentinel) = (
        InvocationId {
            mission: MissionId(1),
            seq: 17,
        },
        InvocationId {
            mission: MissionId(1),
            seq: 6,
        },
    );
    let thread = std::thread::current().id();
    let entry = |tick: u64, point: &'static str, note: bool| s1::Entry {
        tick,
        point,
        note,
        session: Some(session),
        request: Some(1),
        run: Some(run),
        invocation: None,
        attempt: Some(1),
        kind: Some("chat"),
        request_ordinal: Some(4),
        phase: None,
        commitment: None,
        thread,
        instant: std::time::Instant::now(),
    };
    let broker = vec![
        entry(1, "call.gate", false),
        entry(2, "model.call.observed", false),
        entry(3, "refused:8.cas", true),
    ];
    let charge = nexus_mother::route_seams::Entry {
        tick: 1,
        point: "mother.charge.entry",
        session: Some(session),
        request: Some(1),
        run: None,
        invocation: Some(builder),
        thread,
        instant: std::time::Instant::now(),
    };
    Facts {
        session,
        request: 1,
        recorded: vec![(LaneRole::Builder, builder), (LaneRole::Sentinel, sentinel)],
        claim: Some((run, 1)),
        points: vec!["model.call.observed"],
        acks: vec![Ack::S1(broker[1].clone())],
        value: "quit",
        row: ("call_chat", "ingress"),
        applied: Some(std::time::Instant::now()),
        broker,
        mother: vec![charge],
        host: Vec::new(),
        journal: Vec::new(),
        native: Vec::new(),
        peer: Vec::new(),
        findings: Vec::new(),
    }
}

/// Design 7.2's harness controls, each its own test, each against the frozen
/// `quit` row (row 99): its record read whatever the child's exit, and each
/// named field must differ.
fn differs(name: &str, id: &'static str, drive: fn(&super::Fixture) -> Facts, fields: &[&str]) {
    differs_at(name, id, drive, ("call_chat", "ingress", "quit"), fields);
}
/// `differs` against the frozen row of `(sink, dim, value)` (U9a).
type Drive = fn(&super::Fixture) -> Facts;
fn differs_at(
    name: &str,
    id: &'static str,
    drive: Drive,
    row: (&str, &str, &str),
    fields: &[&str],
) {
    let Some(r) = super::s5::cell(name, id, drive, render) else {
        return;
    };
    let want = frozen(row.0, row.1, row.2).unwrap();
    let witness = written(&r).unwrap_or_else(|e| panic!("{id}: {e}\n{}", r.output));
    let diffs = compare(want, witness);
    let named = |k: &&str| diffs.iter().any(|d| d.starts_with(&format!("{k}: ")));
    assert!(fields.iter().all(named), "{id}: {fields:?} in {diffs:?}");
    println!("WITNESS {id}: {diffs:?}");
}
#[test]
fn witness_control_ord3_fails_in_entry() {
    let name = "witness::witness_control_ord3_fails_in_entry";
    differs(name, "ctl_ord3", super::s5::ord3_quit, &["entry"]);
}
#[test]
fn witness_control_other_attempt_fails_in_identity() {
    let name = "witness::witness_control_other_attempt_fails_in_identity";
    differs(
        name,
        "ctl_other_attempt",
        super::s5::quit_other_attempt,
        &["entry", "check"],
    );
}
#[test]
fn witness_control_earlier_refusal_fails_in_check_and_out() {
    let name = "witness::witness_control_earlier_refusal_fails_in_check_and_out";
    differs(
        name,
        "ctl_earlier_refusal",
        super::s5::early_quit,
        &["check", "out"],
    );
}
/// U9a harness control (labelled a control, over a real run's facts): row
/// 573's d_close cell with S rebound to another session; `entry` and `check`
/// must differ.
#[test]
fn witness_control_close_other_session_fails_in_entry_and_check() {
    let name = "witness::witness_control_close_other_session_fails_in_entry_and_check";
    let row = ("close_obligations", "lifecycle", "Normal");
    differs_at(
        name,
        "ctl_close_other_session",
        other_session,
        row,
        &["entry", "check"],
    );
}
fn other_session(f: &super::Fixture) -> Facts {
    use super::s5::{closed, Close, Mode, Setup};
    let sink = |c: &Close, ()| {
        let seen = c.o.session.close_obligations().map(|_| ());
        (seen.map_err(|e| format!("{e:?}")), ())
    };
    let (row, mode) = (("close_obligations", "lifecycle"), Mode::Armed);
    let (mut facts, ()) = closed(
        f,
        Setup::Mother,
        row,
        "Normal",
        mode,
        Ok(()),
        |_| (),
        sink,
        |_| {},
    );
    facts.session = nexus_governed_control::host_session::SessionSeed::new()
        .unwrap()
        .identity();
    facts
}
/// U6a-1 harness controls (labelled controls over a real `registered` run of
/// row 440): S rebound to another session fails in `entry` and `check`; and
/// another request's `model.record.before` in the trace leaves Q unbound, so
/// `ledger` is `unobserved` (as the A4 inverse).
#[test]
fn witness_control_registration_other_session_fails_in_entry_and_check() {
    let name = "witness::witness_control_registration_other_session_fails_in_entry_and_check";
    let row = ("registration_record", "ingress", "quit");
    let fields = ["entry", "check"];
    differs_at(
        name,
        "ctl_reg_other_session",
        reg_other_session,
        row,
        &fields,
    );
}
fn reg_other_session(f: &super::Fixture) -> Facts {
    let mut facts = super::s5::registered(f, ("registration_record", "ingress"), "quit");
    let other = nexus_governed_control::host_session::SessionSeed::new().unwrap();
    facts.session = other.identity();
    facts
}
#[test]
fn witness_control_registration_other_request_unbinds_the_ledger() {
    let name = "witness::witness_control_registration_other_request_unbinds_the_ledger";
    let row = ("registration_record", "ingress", "quit");
    differs_at(
        name,
        "ctl_reg_other_request",
        reg_other_request,
        row,
        &["ledger"],
    );
}
fn reg_other_request(f: &super::Fixture) -> Facts {
    let mut facts = super::s5::registered(f, ("registration_record", "ingress"), "quit");
    let record = facts
        .broker
        .iter()
        .find(|e| e.point == "model.record.before");
    let mut planted = record.cloned().expect("the record's entry");
    planted.request = Some(7);
    facts.broker.push(planted);
    facts
}

/// U9b D6 (SB3E-R127.3, R135.2; rule level over a real run): row 584 driven
/// through `closed` with the value applied in the sink closure, before the
/// point. D5 refuses it; its witness is otherwise the frozen row's.
#[test]
fn route_s5_closed_refuses_a_value_applied_before_the_hold() {
    let name = "witness::route_s5_closed_refuses_a_value_applied_before_the_hold";
    super::child(name, |f| {
        use super::s5::{closed, Close, Mode, Setup};
        let sink = |c: &Close, ()| {
            let early = c.o.ingress.emergency().map(|_| ());
            let early = early.map_err(|e| format!("{e:?}"));
            (early.and(c.accept_quit().map(|_| ())), ())
        };
        let (row, mode) = (("close_restrict", "lifecycle"), Mode::Paused);
        let none = |_: &mut Close| ();
        let (facts, ()) = closed(
            f,
            Setup::Mother,
            row,
            "Emergency",
            mode,
            Ok(()),
            none,
            sink,
            |_| {},
        );
        let want = frozen("close_restrict", "lifecycle", "Emergency").unwrap();
        println!("WITNESS D6: {:?}, {}", facts.findings, render(&facts));
        assert_eq!(
            compare(want, &render(&facts)),
            Vec::<String>::new(),
            "D6: the witness"
        );
        let d5 = "value: state Emergency at the hold, before the apply";
        assert_eq!(facts.findings, [d5], "D6: the D5 finding");
    });
}
/// U9b harness control (over a real run): row 584 with S rebound to another
/// session; `entry` and `check` must differ.
#[test]
fn witness_control_quit_other_session_fails_in_entry_and_check() {
    let name = "witness::witness_control_quit_other_session_fails_in_entry_and_check";
    let row = ("close_restrict", "lifecycle", "Emergency");
    differs_at(
        name,
        "ctl_quit_other_session",
        quit_other_session,
        row,
        &["entry", "check"],
    );
}
fn quit_other_session(f: &super::Fixture) -> Facts {
    use super::s5::{closed, Close, Mode, Setup};
    let sink = |c: &Close, ()| (c.accept_quit().map(|_| ()), ());
    let emergency = |c: &Close| {
        c.o.ingress.emergency().expect("emergency");
    };
    let row = ("close_restrict", "lifecycle");
    let (mut facts, ()) = closed(
        f,
        Setup::Mother,
        row,
        "Emergency",
        Mode::Paused,
        Ok(()),
        |_| (),
        sink,
        emergency,
    );
    facts.session = nexus_governed_control::host_session::SessionSeed::new()
        .unwrap()
        .identity();
    facts
}
/// SEAM-1 T2 harness controls (labelled controls over a real run of row
/// 642): the end record's S1 commit moved to another thread, or before the
/// acknowledgement, is not counted, so `ledger` must differ.
#[test]
fn witness_control_end_commit_on_another_thread_is_not_counted() {
    let name = "witness::witness_control_end_commit_on_another_thread_is_not_counted";
    let row = ("primary_end", "lifecycle", "SealedFinalizing");
    differs_at(
        name,
        "ctl_end_other_thread",
        end_elsewhere,
        row,
        &["ledger"],
    );
}
fn end_elsewhere(f: &super::Fixture) -> Facts {
    let mut facts = end_run(f);
    let other = std::thread::spawn(|| std::thread::current().id());
    let other = other.join().unwrap();
    let end = |e: &&mut s1::Entry| e.phase.as_deref() == Some("session_closed");
    facts
        .broker
        .iter_mut()
        .filter(end)
        .for_each(|e| e.thread = other);
    facts
}
#[test]
fn witness_control_end_commit_before_the_hold_is_not_counted() {
    let name = "witness::witness_control_end_commit_before_the_hold_is_not_counted";
    let row = ("primary_end", "lifecycle", "SealedFinalizing");
    differs_at(name, "ctl_end_before_hold", end_before, row, &["ledger"]);
}
/// Row 642's real run: the close chain through observe_mother_end, then
/// close_primary held at close.primary (as s5_close.rs's row driver).
fn end_run(f: &super::Fixture) -> Facts {
    use super::s5::{closed, Close, Mode, Setup};
    use nexus_governed_control::host_session::close::CloseSeal;
    let prefix = |c: &mut Close| {
        let quit = c.accept_quit().expect("Quit");
        let mother = c.mother.take().expect("the Mother");
        let end = mother.m.prepare_session_end().expect("the Mother end");
        let s = &c.o.session;
        s.settle_close_resources(&quit).expect("resources");
        s.settle_close_authority(&quit).expect("authority");
        let seal = s.seal_close(&quit).expect("the seal");
        end.record(&seal).expect("the Mother end recorded");
        s.observe_mother_end(&seal)
            .expect("the Mother end observed");
        seal
    };
    let sink = |c: &Close, seal: CloseSeal| {
        let end = c.o.session.close_primary(&seal);
        (end.map_err(|e| format!("{e:?}")), seal)
    };
    let row = ("primary_end", "lifecycle");
    let (mode, value) = (Mode::Armed, "SealedFinalizing");
    closed(
        f,
        Setup::Mother,
        row,
        value,
        mode,
        Ok(()),
        prefix,
        sink,
        |_| {},
    )
    .0
}
fn end_before(f: &super::Fixture) -> Facts {
    let mut facts = end_run(f);
    let end = |e: &&mut s1::Entry| e.phase.as_deref() == Some("session_closed");
    facts.broker.iter_mut().filter(end).for_each(|e| e.tick = 0);
    facts
}
