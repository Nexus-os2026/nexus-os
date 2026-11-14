//! SB3E-R86.4 (U5a; design 2.3-2.5; AM-2 A.4; mission Behaviour 9): a cell's
//! witness. In the child, `render` writes the runner's facts in the frozen
//! table's grammar, S, Q, RUN, INV and ATT bound and the table's location
//! tokens mapped by the reviewed `TOKENS`; in the parent, `record` checks the
//! child's one `CELL` record and `compare` checks the witness against its
//! expected text, field by field and byte for byte.
use super::s5::{Facts, Ran};
use nexus_governed_control::broker::route_seams as s1;
use nexus_mother::journal::Event;
use nexus_mother::lane::LaneRole;

/// The frozen expectation table: the one reviewed `include_str!` row.
const TABLE: &str = include_str!("../session/model-bridge-expect.tsv");
/// The witness fields of rows 93-100, in the table's order.
const FIELDS: [&str; 10] = [
    "entry",
    "check",
    "value",
    "out",
    "branch",
    "ledger",
    "window",
    "peer",
    "next",
    "deadline_fired",
];
/// Behaviour 9: the reviewed token map, each table location token and the S1
/// observation it stands for. Both need the full identity (`s`, `q`, `run`,
/// `att`, `kind=chat`), the ordinal, the thread and the order; finding both
/// strings is not enough (`cas`).
const TOKENS: [(&str, &str); 2] = [
    (
        "CAS:m:774",
        "model.call.observed, then on its thread exactly the note refused:8.cas, with no ord-4 model.call.admitted",
    ),
    ("note:8", "exactly the note refused:8.cas (refused:8.capacity is not it)"),
];
/// Behaviour 10: the control's witness, as measured at U5 Step 0 (R86).
const CONTROL: &str = "entry=call.gate[s=S q=Q run=RUN inv=INV att=ATT kind=chat ord=4];\
check=model.call.observed[s=S q=Q run=RUN inv=INV att=ATT kind=chat ord=4];\
value=none@model.call.observed ack=held won=route;out=admitted@model.call.admitted;branch=none;\
ledger=+1:ModelCallCharged[inv=INV],+1:model_call[run=RUN att=ATT];window=completed_attempt;\
peer=/api/version,/api/status,/api/tags,/api/chat chat=1;next=lane_returned=1;deadline_fired=none";

/// The frozen witness of the row holding `value` for `sink` and `dim`.
fn frozen(sink: &str, dim: &str, value: &str) -> Option<&'static str> {
    let rows = TABLE.lines().map(|l| l.split('\t').collect::<Vec<_>>());
    let mut rows = rows.filter(|c| c.len() == 18 && c[1] == sink && c[2] == dim);
    let row = rows.find(|c| c[3].split(',').any(|v| v == value))?;
    Some(row[13])
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
/// An entry's identity, each bound value as its placeholder and any other
/// value as itself. `inv=INV` is A.4's correlation, not the broker entry's.
fn tag(e: &s1::Entry, b: &Bound, inv: bool) -> String {
    let s = match e.session == Some(b.s) {
        true => "S".to_string(),
        false => format!("{:?}", e.session),
    };
    let q = match e.request == Some(b.q) {
        true => "Q".to_string(),
        false => format!("{:?}", e.request),
    };
    let run = match b.run.is_some() && e.run == b.run {
        true => "RUN".to_string(),
        false => format!("{:?}", e.run),
    };
    let att = match b.att.is_some() && e.attempt == b.att {
        true => "ATT".to_string(),
        false => format!("{:?}", e.attempt),
    };
    let inv = if inv { "INV" } else { "none" };
    let kind = e.kind.unwrap_or("none");
    let ord = e
        .request_ordinal
        .map_or("none".to_string(), |n| n.to_string());
    format!("s={s} q={q} run={run} inv={inv} att={att} kind={kind} ord={ord}")
}
/// `TOKENS[0]` and `[1]`: the observed entry, then on its thread, later,
/// exactly one note of this identity and ordinal, `refused:8.cas`; no
/// `model.call.admitted` of it.
fn cas(observed: Option<&s1::Entry>, notes: &[&s1::Entry], admitted: bool) -> bool {
    observed.is_some_and(|o| {
        let [note] = notes else { return false };
        note.point == "refused:8.cas" && note.thread == o.thread && note.tick > o.tick && !admitted
    })
}

/// The witness of `f`, in the table's grammar (Behaviour 9).
pub(super) fn render(f: &Facts) -> String {
    let b = bind(f);
    let correlated = |m: &&nexus_mother::route_seams::Entry| {
        m.point == "mother.charge.entry" && b.inv.is_some() && m.invocation == b.inv
    };
    let inv = f.mother.iter().any(|m| correlated(&m));
    let four: Vec<&s1::Entry> = f
        .broker
        .iter()
        .filter(|e| mine(e, &b) && e.request_ordinal == Some(4))
        .collect();
    let first = |point: &str| {
        four.iter()
            .find(|e| e.point == point && !e.note && e.kind == Some("chat"))
            .copied()
    };
    let (gate, observed) = (first("call.gate"), first("model.call.observed"));
    let admitted = first("model.call.admitted").is_some();
    let notes: Vec<&s1::Entry> = four.iter().filter(|e| e.note).copied().collect();
    let noted: Vec<&str> = notes.iter().map(|n| n.point).collect();
    let refused = cas(observed, &notes, admitted);
    let entry = gate.map_or("none".into(), |g| format!("call.gate[{}]", tag(g, &b, inv)));
    let check = match (refused, observed) {
        (true, Some(o)) => format!("CAS:m:774[{}]", tag(o, &b, inv)),
        (false, Some(o)) => format!("model.call.observed[{}]", tag(o, &b, inv)),
        (_, None) => "none".into(),
    };
    let held = f.acks.last().map_or("none", |a| a.point);
    let ack = if f.acks.iter().all(|a| mine(a, &b)) && !f.acks.is_empty() {
        "held"
    } else {
        "foreign"
    };
    let won = if refused { "value" } else { "route" };
    let out = match (refused, admitted) {
        (true, _) => "refused:8@CAS:m:774".to_string(),
        (false, true) => "admitted@model.call.admitted".to_string(),
        (false, false) => format!("notes{noted:?}"),
    };
    let branch = match (refused, noted.is_empty()) {
        (true, _) => "note:8@m:774-783".to_string(),
        (false, true) => "none".to_string(),
        (false, false) => format!("notes{noted:?}"),
    };
    let charged = |e: &&nexus_mother::journal::Entry| matches!(e.event, Event::ModelCallCharged { invocation, .. } if Some(invocation) == b.inv);
    let returned = |e: &&nexus_mother::journal::Entry| matches!(e.event, Event::LaneReturned { invocation, .. } if Some(invocation) == b.inv);
    let detail = |r: &nexus_governed_control::authority::evidence::EvidenceRecord, k: &str| {
        r.detail
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
    };
    let run = b.run.map(|r| r.to_string());
    let attempt = |r: &&nexus_governed_control::authority::evidence::EvidenceRecord| {
        detail(r, "op").as_deref() == Some("model_call")
            && run.is_some()
            && r.run == run
            && detail(r, "call") == b.att.map(|a| a.to_string())
    };
    let ledger = format!(
        "+{}:ModelCallCharged[inv=INV],+{}:model_call[run=RUN att=ATT]",
        f.journal.iter().filter(charged).count(),
        f.native.iter().filter(attempt).count()
    );
    // An attempt site carries the session, run and attempt, no request (A.1).
    let site = |point: &str| {
        let at = |e: &&s1::Entry| e.point == point && e.session == Some(b.s);
        f.broker
            .iter()
            .filter(at)
            .any(|e| b.run.is_some() && e.run == b.run && e.attempt == b.att)
    };
    let window = match site("model.attempt.record.before") && site("complete") {
        true => "completed_attempt",
        false => "boundary",
    };
    let chat = f.peer.iter().filter(|p| *p == "/api/chat").count();
    let paths = if f.peer.is_empty() {
        "none".to_string()
    } else {
        f.peer.join(",")
    };
    let expired: Vec<&str> = noted
        .iter()
        .copied()
        .filter(|n| n.starts_with("refused:1"))
        .collect();
    let timed_out = f
        .native
        .iter()
        .filter(attempt)
        .any(|r| detail(r, "outcome").as_deref() == Some("timeout"));
    let deadline = match (expired.is_empty(), timed_out) {
        (true, false) => "none".to_string(),
        _ => format!("expired{expired:?} timeout={timed_out}"),
    };
    let values = [
        entry,
        check,
        format!("{}@{held} ack={ack} won={won}", f.value),
        out,
        branch,
        ledger,
        window.to_string(),
        format!("{paths} chat={chat}"),
        format!(
            "lane_returned={}",
            f.journal.iter().filter(returned).count()
        ),
        deadline,
    ];
    let fields = FIELDS.iter().zip(values).map(|(k, v)| format!("{k}={v}"));
    fields.collect::<Vec<_>>().join(";")
}

/// Design 3.3: the child's result file must hold exactly one `CELL <nonce>
/// <id> <witness>` line, of this run's nonce and this cell, and nothing else
/// (a `TIMEOUT` line included). The witness, or the refusal's name.
fn record(r: &Ran) -> Result<&str, &'static str> {
    if !r.passed {
        return Err("record: the child failed");
    }
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
    for key in FIELDS {
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

/// Rows 93-100 (the `call_chat` ingress cells) hold exactly the rendered
/// fields; every location token they use is one of `TOKENS`, or is the
/// budget row's (`model.call.sampled`, `note:1`), which U5 does not map.
#[test]
fn witness_rows_hold_the_rendered_fields() {
    let values = [
        "original_cell_unscheduled",
        "repeated",
        "mission_stop",
        "stop_all",
        "door_fence",
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
        assert_eq!(keys, FIELDS, "{value}");
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
        acks: vec![entry(2, "model.call.observed", false)],
        value: "quit",
        broker: vec![
            entry(1, "call.gate", false),
            entry(2, "model.call.observed", false),
            entry(3, "refused:8.cas", true),
        ],
        mother: vec![charge],
        journal: Vec::new(),
        native: Vec::new(),
        peer: Vec::new(),
    }
}
