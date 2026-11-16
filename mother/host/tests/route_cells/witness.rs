//! SB3E-R86.4 (U5a; design 2.3-2.5; AM-2 A.4; mission Behaviour 9): a cell's
//! witness. In the child, `render` writes the runner's facts in the frozen
//! table's grammar, S, Q, RUN, INV and ATT bound and the table's location
//! tokens mapped by the reviewed `TOKENS`; in the parent, `record` checks the
//! child's one `CELL` record and `compare` checks the witness against its
//! expected text, field by field and byte for byte. W0b B1: `render` is
//! row-driven: the cell's frozen row gives its fields, their order and each
//! identity tag's keys; every value is rendered from the facts.
use super::s5::{Facts, Ran};
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
/// enough. U5's two, unchanged in meaning; later units add rows.
type Predicate = for<'a> fn(&View<'a>) -> Option<&'a s1::Entry>;
const TOKENS: [(&str, &str, Predicate); 2] = [
    // model.call.observed, then on its thread exactly the note
    // refused:8.cas, with no ord-4 model.call.admitted.
    ("CAS:m:774", "model.call.observed", |v| {
        cas(v).map(|(o, _)| o)
    }),
    // exactly the note refused:8.cas (refused:8.capacity is not it).
    ("note:8", "refused:8.cas", |v| cas(v).map(|(_, n)| n)),
];
/// W0b B1: the ledger's closed list: each event the table names, as a
/// journal event (by invocation) or a native record's `op` (by run and
/// attempt, or by session); `all` is every journal entry and native record
/// of the session since the hold (SB3E-R6.3's empty delta is `+0:all[s=S]`).
const LEDGER: [(&str, &str); 7] = [
    ("ModelCallCharged", "journal"),
    ("LaneReturned", "journal"),
    ("SessionClosed", "journal"),
    ("model_call", "native"),
    ("model_registered", "native"),
    ("session_closed", "native"),
    ("all", "all"),
];
/// Behaviour 10: the controls' witnesses, by `(sink, dim)`, as measured at U5
/// Step 0 (R86).
const CONTROLS: [(&str, &str, &str); 1] = [("call_chat", "ingress", CONTROL)];
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
    Ok(Shape { fields, group })
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
/// An S1 entry of the bound attempt: session, request, run and attempt.
fn mine(e: &s1::Entry, b: &Bound) -> bool {
    b.run.is_some()
        && e.session == Some(b.s)
        && e.request == Some(b.q)
        && e.run == b.run
        && e.attempt == b.att
}
/// What `render` reads: the facts, the binding, whether INV is correlated
/// (A.4) and the placeholders' group suffix.
struct View<'a> {
    f: &'a Facts,
    b: Bound,
    inv: bool,
    group: String,
}
/// The first entry at `point` of the bound attempt, of the tag's kind
/// and ordinal (binding point, identity, ordinal and order).
fn first_at<'a>(
    v: &View<'a>,
    point: &str,
    kind: Option<&str>,
    ord: Option<u64>,
) -> Option<&'a s1::Entry> {
    let bound = |e: &&s1::Entry| mine(e, &v.b) && !e.note && e.point == point;
    let tagged = |e: &&s1::Entry| e.kind == kind && e.request_ordinal == ord;
    v.f.broker.iter().filter(bound).find(tagged)
}
/// Ord 4's chat notes of the bound attempt.
fn notes<'a>(v: &View<'a>) -> Vec<&'a s1::Entry> {
    let four = |e: &&s1::Entry| mine(e, &v.b) && e.request_ordinal == Some(4);
    v.f.broker.iter().filter(four).filter(|e| e.note).collect()
}
/// `TOKENS[0]` and `[1]`: the observed entry, then on its thread, later,
/// exactly one note of this identity and ordinal, `refused:8.cas`, of the
/// chat (kind `chat`, R88.1); no `model.call.admitted` of it.
fn cas<'a>(v: &View<'a>) -> Option<(&'a s1::Entry, &'a s1::Entry)> {
    let o = first_at(v, "model.call.observed", Some("chat"), Some(4))?;
    let admitted = first_at(v, "model.call.admitted", Some("chat"), Some(4));
    let [note] = notes(v)[..] else {
        return None;
    };
    let ok = note.point == "refused:8.cas"
        && note.kind == Some("chat")
        && note.thread == o.thread
        && note.tick > o.tick
        && admitted.is_none();
    ok.then_some((o, note))
}
/// A location token's entry: a `TOKENS` row's predicate, or a plain
/// point of the tag's kind and ordinal.
fn token<'a>(
    v: &View<'a>,
    token: &str,
    kind: Option<&str>,
    ord: Option<u64>,
) -> Option<&'a s1::Entry> {
    match TOKENS.iter().find(|t| t.0 == token) {
        Some((_, _, predicate)) => predicate(v),
        None => first_at(v, token, kind, ord),
    }
}
/// A placeholder, the bound value's (with the group's suffix), or else the
/// observed value itv.
fn hold<'a>(v: &View<'a>, name: &str, bound: bool, seen: String) -> String {
    match bound {
        true => format!("{name}{}", v.group),
        false => seen,
    }
}
/// `e`'s identity under `keys` (the row's tag keys, in `KEYS`'s order):
/// each bound value as its placeholder, any other as itself; `inv=INV`
/// is A.4's correlation, not the broker entry's.
fn identity<'a>(v: &View<'a>, e: &s1::Entry, keys: &[(&str, &str)]) -> String {
    let b = &v.b;
    let one = |k: &str| match k {
        "s" => hold(v, "S", e.session == Some(b.s), format!("{:?}", e.session)),
        "q" => hold(v, "Q", e.request == Some(b.q), format!("{:?}", e.request)),
        "run" => hold(
            v,
            "RUN",
            b.run.is_some() && e.run == b.run,
            format!("{:?}", e.run),
        ),
        "inv" => hold(v, "INV", v.inv, "none".into()),
        "att" => hold(
            v,
            "ATT",
            b.att.is_some() && e.attempt == b.att,
            format!("{:?}", e.attempt),
        ),
        "kind" => e.kind.unwrap_or("none").to_string(),
        _ => e.request_ordinal.map_or("none".into(), |n| n.to_string()),
    };
    let shown = keys.iter().map(|(k, _)| format!("{k}={}", one(k)));
    shown.collect::<Vec<_>>().join(" ")
}
/// A ledger tag: each key's placeholder (a ledger names no entry).
fn ledger_tag<'a>(v: &View<'a>, keys: &[(&str, &str)]) -> String {
    let shown = keys.iter().map(|(k, _)| {
        let name = match *k {
            "s" => "S",
            "q" => "Q",
            "run" => "RUN",
            "inv" => "INV",
            _ => "ATT",
        };
        format!("{k}={name}{}", v.group)
    });
    shown.collect::<Vec<_>>().join(" ")
}
/// A field's `<token>[..]`: its token, its tag's keys, and the kind and
/// ordinal the tag reads.
type Located<'a> = (
    &'a str,
    Vec<(&'a str, &'a str)>,
    Option<&'a str>,
    Option<u64>,
);
fn located(text: &str) -> Located<'_> {
    let token = text.split('[').next().unwrap_or(text);
    let keys = tag_keys(text).unwrap_or_default();
    let get = |k: &str| keys.iter().find(|x| x.0 == k).map(|x| x.1);
    let (kind, ord) = (get("kind"), get("ord").and_then(|o| o.parse().ok()));
    (token, keys, kind, ord)
}

/// The witness of `f`, in its frozen row's grammar (Behaviour 9; W0b B1).
pub(super) fn render(f: &Facts) -> String {
    let (sink, dim) = f.row;
    let control = CONTROLS
        .iter()
        .find(|c| (c.0, c.1) == (sink, dim) && f.value == "none");
    let found = match control {
        Some(c) => Some((c.2, "own")),
        None => row(sink, dim, f.value).map(|c| (c[13], c[10])),
    };
    let Some((witness, exec)) = found else {
        return format!("no row {sink}/{dim}/{}", f.value);
    };
    match shape(witness, exec) {
        Ok(shape) => rendered(f, &shape),
        Err(e) => format!("unrenderable row {sink}/{dim}/{}: {e}", f.value),
    }
}
/// `f` rendered field by field in `shape`'s order.
fn rendered(f: &Facts, shape: &Shape) -> String {
    let b = bind(f);
    let correlated = |m: &&nexus_mother::route_seams::Entry| {
        m.point == "mother.charge.entry" && b.inv.is_some() && m.invocation == b.inv
    };
    let inv = f.mother.iter().any(|m| correlated(&m));
    let group = shape.group.map_or(String::new(), |g| format!(".{g}"));
    let v = &View { f, b, inv, group };
    let text = |k: &str| shape.fields.iter().find(|x| x.0 == k).map_or("", |x| x.1);
    let (entry_token, entry_keys, kind, ord) = located(text("entry"));
    let (check_token, check_keys, _, _) = located(text("check"));
    let entry = token(v, entry_token, kind, ord);
    let check = token(v, check_token, kind, ord);
    let noted: Vec<&str> = notes(v).iter().map(|n| n.point).collect();
    // The branch note: its token's predicate held, or what was noted.
    let branch_text = text("branch");
    let note_token = branch_text.split('@').next().unwrap_or("");
    let note = note_token
        .starts_with("note:")
        .then(|| token(v, note_token, kind, ord))
        .flatten();
    let branch = match (note, noted.is_empty()) {
        (Some(_), _) => branch_text.to_string(),
        (None, true) => "none".to_string(),
        (None, false) => format!("notes{noted:?}"),
    };
    // Generic outcome classes: `refused:<n>@<loc>` from the location's
    // predicate and its note `note:<n>`; `admitted@`, `passed@` and
    // `recorded@<loc>` from the location's predicate with no note.
    let out_text = text("out");
    let (class, loc) = out_text.split_once('@').unwrap_or((out_text, ""));
    let located_out = token(v, loc, kind, ord);
    let refused = class
        .strip_prefix("refused:")
        .is_some_and(|n| note.is_some() && note_token == format!("note:{n}"));
    let quiet = !class.starts_with("refused:") && noted.is_empty();
    let out = match located_out.is_some() && (refused || quiet) {
        true => out_text.to_string(),
        false if noted.is_empty() => "none".to_string(),
        false => format!("notes{noted:?}"),
    };
    let won = if out.starts_with("refused:") {
        "value"
    } else {
        "route"
    };
    // R88.1: each acknowledgement answers its own held point, of the
    // entry tag's kind and ordinal, of the bound attempt, the very trace
    // entry; `held` if the value came while every one stood, `armed` if it
    // was set before the first.
    let held = f.points.last().copied().unwrap_or("none");
    let answers = |(a, point): (&s1::Entry, &&str)| {
        a.point == *point
            && !a.note
            && a.kind == kind
            && a.request_ordinal == ord
            && mine(a, &v.b)
            && f.broker.contains(a)
    };
    let pairs = f.acks.iter().zip(&f.points);
    let answered =
        !f.acks.is_empty() && f.acks.len() == f.points.len() && pairs.clone().all(answers);
    let first = f.acks.iter().map(|a| a.instant).min();
    let ack = match (answered, f.applied.zip(first)) {
        (true, Some((at, first))) if at < first => "armed".to_string(),
        (true, _) => "held".to_string(),
        (false, _) => {
            let shown = f.acks.iter().map(|a| (a.point, a.kind, a.request_ordinal));
            format!("unanswered{:?}", shown.collect::<Vec<_>>())
        }
    };
    let values: Vec<String> = shape
        .fields
        .iter()
        .map(|(k, t)| match *k {
            "entry" => entry.map_or("none".into(), |e| {
                format!("{entry_token}[{}]", identity(v, e, &entry_keys))
            }),
            "check" => match check {
                Some(c) => format!("{check_token}[{}]", identity(v, c, &check_keys)),
                None => {
                    let point = TOKENS
                        .iter()
                        .find(|x| x.0 == check_token)
                        .map_or(check_token, |x| x.1);
                    let seen = first_at(v, point, kind, ord);
                    seen.map_or("none".into(), |e| {
                        format!("{point}[{}]", identity(v, e, &check_keys))
                    })
                }
            },
            "value" => format!("{}@{held} ack={ack} won={won}", f.value),
            "out" => out.clone(),
            "branch" => branch.clone(),
            "ledger" => ledger(v, t),
            "window" => window(v).to_string(),
            "peer" => {
                let chat = f.peer.iter().filter(|p| *p == "/api/chat").count();
                let paths = if f.peer.is_empty() {
                    "none".to_string()
                } else {
                    f.peer.join(",")
                };
                format!("{paths} chat={chat}")
            }
            "next" => next(v, t),
            "deadline_fired" => deadline(v, &noted),
            other => format!("unbuilt:{other}"),
        })
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
/// W0b B1: the ledger from `LEDGER`'s closed list, each item `+N:<event>[..]`
/// counted from the facts under its tag's identity; an event outside the
/// list is shown as such (a named difference).
fn ledger(v: &View, text: &str) -> String {
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
        let tag = ledger_tag(v, &keys);
        let by_inv = |e: &&nexus_mother::journal::Entry| {
            let (name, invocation) = match e.event {
                Event::ModelCallCharged { invocation, .. } => {
                    ("ModelCallCharged", Some(invocation))
                }
                Event::LaneReturned { invocation, .. } => ("LaneReturned", Some(invocation)),
                Event::SessionClosed => ("SessionClosed", None),
                _ => ("", None),
            };
            name == event && (invocation.is_none() || invocation == v.b.inv)
        };
        let native = |r: &&nexus_governed_control::authority::evidence::EvidenceRecord| {
            detail(r, "op").as_deref() == Some(event) && (event != "model_call" || attempt(v, r))
        };
        let n = match LEDGER.iter().find(|l| l.0 == event).map(|l| l.1) {
            Some("journal") => v.f.journal.iter().filter(by_inv).count(),
            Some("native") => v.f.native.iter().filter(native).count(),
            Some(_) => v.f.journal.len() + v.f.native.len(),
            None => return format!("unlisted:{event}"),
        };
        format!("+{n}:{event}[{tag}]")
    });
    shown.collect::<Vec<_>>().join(",")
}
/// An attempt site carries the session, run and attempt, no request (A.1).
fn window(v: &View) -> &'static str {
    let site = |point: &str| {
        let at = |e: &&s1::Entry| e.point == point && e.session == Some(v.b.s);
        let run = v.b.run;
        v.f.broker
            .iter()
            .filter(at)
            .any(|e| run.is_some() && e.run == run && e.attempt == v.b.att)
    };
    match site("model.attempt.record.before") && site("complete") {
        true => "completed_attempt",
        false => "boundary",
    }
}
/// W0b B1: `next`'s keys from the facts; `lane_returned` is built (U5), any
/// other key is left to the unit that adds it.
fn next(v: &View, text: &str) -> String {
    let returned = |e: &&nexus_mother::journal::Entry| matches!(e.event, Event::LaneReturned { invocation, .. } if Some(invocation) == v.b.inv);
    let keys = text.split(',').map(|kv| kv.split('=').next().unwrap_or(""));
    let shown = keys.map(|k| match k {
        "lane_returned" => format!(
            "lane_returned={}",
            v.f.journal.iter().filter(returned).count()
        ),
        other => format!("{other}=unbuilt"),
    });
    shown.collect::<Vec<_>>().join(",")
}
/// `deadline_fired`: an expiry note or a native timeout of the attempt.
fn deadline(v: &View, noted: &[&str]) -> String {
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
    match (expired.is_empty(), timed_out) {
        (true, false) => "none".to_string(),
        _ => format!("expired{expired:?} timeout={timed_out}"),
    }
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

/// Behaviour 10: the control's record, and its witness against `CONTROL`.
pub(super) fn control(r: &Ran) {
    let witness = match record(r) {
        Ok(w) => w,
        Err(e) => panic!("{}: {e}\n{}{}", r.id, r.result, r.output),
    };
    let diffs = compare(CONTROL, witness);
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
        let fields: Vec<&str> = shape(witness, "own")
            .unwrap()
            .fields
            .iter()
            .map(|x| x.0)
            .collect();
        assert_eq!(keys, fields, "{value}");
        assert_eq!(keys.len(), 10, "{value}");
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
            b: bind(&f),
            inv: true,
            group,
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
                    "ledger" => ledger_tag(v, &keys),
                    _ => identity(v, &e, &keys),
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
    assert_eq!(rows, 794, "every row with a witness");
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
            .map(|a| a.instant - std::time::Duration::from_millis(1));
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
                rendered(&f, &shape(untagged, "own").unwrap()),
                "entry",
            ),
            ("ack=armed for a held value", render(&armed), "value"),
            (
                "an unlisted ledger event",
                rendered(&f, &shape(unlisted, "own").unwrap()),
                "ledger",
            ),
        ] {
            let diffs = compare(&base, &observed);
            let named = diffs.iter().any(|d| d.starts_with(&format!("{field}: ")));
            assert!(named, "{control}: {diffs:?}");
            println!("WITNESS B1 control {control:?}: {diffs:?}");
        }
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
                (a.kind, a.request_ordinal) = (Some("metadata"), Some(3));
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
        acks: vec![broker[1].clone()],
        value: "quit",
        row: ("call_chat", "ingress"),
        applied: Some(std::time::Instant::now()),
        broker,
        mother: vec![charge],
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
    let Some(r) = super::s5::cell(name, id, drive, render) else {
        return;
    };
    let want = frozen("call_chat", "ingress", "quit").unwrap();
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
