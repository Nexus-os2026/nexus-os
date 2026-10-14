#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 5: the source-bound branch vocabulary GENERATOR.

  vocab_generate.py [--source-dir DIR] [--out FILE]
  vocab_generate.py [--source-dir DIR] --compare VOCABULARY.json     the freeze check (exit 0 only if equal)

Method (method A; the two static checks use other methods): the manual source-site inventory is held below as SITES,
one entry per `obs.branch(...)` call of model_tests.rs at C, with every label-producing match arm transcribed. The
generator does NOT trust the transcription:

  1. every site's call text, every label-producing block and every arm is ANCHORED: its whitespace-normalised text
     must occur verbatim in the named function's body at C (a changed label, arm or call is a failure);
  2. every label-producing match is COUNTED: its number of top-level arms in the source must equal the inventory's
     (an added or removed arm is a failure);
  3. every helper match (reason_label, cause_label) must cover exactly the variants of its enum as declared at C;
  4. every interpolated `{x:?}` is EXPANDED from the enum declarations parsed from the source at C (Debug is derived
     for every such enum, so its rendering is the variant name, then the payload's rendering in parentheses); a
     variant whose payload is not a finite enum makes the domain unbounded and is a failure, unless an earlier arm of
     the same match is anchored as consuming it.

The accounting unit is (operation identity, exact emitted label). Rows are emitted in a fixed order (operation, then
source arm order, then declaration order of the expanded enums) and numbered B001.. in that order. Sources are read
from git objects of C, or, for negative controls only, from --source-dir.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402

GENERATOR_VERSION = "vocab_generate v4"


# ---- source text helpers (text level; independent of the token-level checker) ---------------------------------

def strip_comments(text):
    """Replace comments by spaces, keeping strings and char literals intact and positions stable."""
    out, i, n = list(text), 0, len(text)
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            for k in range(i, j):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif c == '"':
            i = skip_string(text, i)
        elif c == "'":
            i = skip_char(text, i)
        else:
            i += 1
    return "".join(out)


def skip_string(text, i):
    j = i + 1
    while text[j] != '"':
        j += 2 if text[j] == "\\" else 1
    return j + 1


def skip_char(text, i):
    if text[i + 1] == "\\":
        return text.index("'", i + 2) + 1
    if i + 2 < len(text) and text[i + 2] == "'":
        return i + 3
    return i + 1  # a lifetime


def match_brace(text, open_at):
    """Index just past the brace that closes text[open_at] ('{', '(' or '['), skipping strings and chars."""
    pairs = {"{": "}", "(": ")", "[": "]"}
    stack, i = [], open_at
    while i < len(text):
        c = text[i]
        if c == '"':
            i = skip_string(text, i)
            continue
        if c == "'":
            i = skip_char(text, i)
            continue
        if c in pairs:
            stack.append(pairs[c])
        elif c in ")]}":
            if not stack or stack.pop() != c:
                raise SystemExit(f"unbalanced {c} at {i}")
            if not stack:
                return i + 1
        i += 1
    raise SystemExit("unclosed brace")


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


def fn_body(text, name):
    """(start offset, body text) of the one `fn name(` in text (comments already stripped)."""
    hits = [m for m in re.finditer(r"\bfn\s+" + re.escape(name) + r"\s*[<(]", text)]
    if len(hits) != 1:
        raise SystemExit(f"fn {name}: {len(hits)} definitions")
    paren = text.index("(", hits[0].start())
    sig_end = match_brace(text, paren)
    brace = text.index("{", sig_end)
    end = match_brace(text, brace)
    return brace, text[brace:end]


def top_arms(block):
    """The number of top-level `=>` of a match block text that starts at its '{'."""
    depth, i, count = 0, 0, 0
    while i < len(block):
        c = block[i]
        if c == '"':
            i = skip_string(block, i)
            continue
        if c == "'":
            i = skip_char(block, i)
            continue
        if c in "{([":
            depth += 1
        elif c in "})]":
            depth -= 1
        elif block.startswith("=>", i) and depth == 1:
            count += 1
            i += 2
            continue
        i += 1
    return count


def parse_enums(text):
    """{name: [(variant, kind, [payload types])]} for every `enum` in text (comments stripped)."""
    enums = {}
    for m in re.finditer(r"\benum\s+(\w+)\s*(<[^>]*>)?\s*\{", text):
        start = m.end() - 1
        body = text[start + 1:match_brace(text, start) - 1]
        variants = []
        for part in split_top(body, ","):
            part = re.sub(r"#\[[^\]]*\]", "", part).strip()
            if not part:
                continue
            vm = re.match(r"(\w+)\s*(.*)$", part, re.S)
            name, rest = vm.group(1), vm.group(2).strip()
            if rest.startswith("("):
                variants.append((name, "tuple", [t.strip() for t in split_top(rest[1:-1], ",") if t.strip()]))
            elif rest.startswith("{"):
                fields = [f.strip() for f in split_top(rest[1:-1], ",") if f.strip()]
                variants.append((name, "struct", [f.split(":", 1)[1].strip() for f in fields]))
            elif rest == "" or rest.startswith("="):
                variants.append((name, "unit", []))
            else:
                raise SystemExit(f"enum {m.group(1)}: cannot read variant {part!r}")
        if m.group(1) in enums:
            raise SystemExit(f"enum {m.group(1)} declared twice")
        enums[m.group(1)] = variants
    return enums


def split_top(s, sep):
    out, depth, cur, i = [], 0, [], 0
    while i < len(s):
        c = s[i]
        if c in "([{<":
            depth += 1
        elif c in ")]}>":
            depth -= 1
        if c == sep and depth == 0:
            out.append("".join(cur))
            cur = []
        else:
            cur.append(c)
        i += 1
    out.append("".join(cur))
    return out


class Unbounded(Exception):
    pass


def debug_values(enums, ty, exclude=()):
    """[(rendering, derivation)] of Debug over the type's declared variants, in declaration order."""
    if ty not in enums:
        raise Unbounded(f"{ty} is not a finite enum")
    out = []
    for name, kind, payload in enums[ty]:
        if name in exclude:
            continue
        if kind == "unit":
            out.append((name, f"{ty}::{name}"))
        elif kind == "tuple" and len(payload) == 1:
            for s, d in debug_values(enums, payload[0]):
                out.append((f"{name}({s})", f"{ty}::{name}({d})"))
        else:
            raise Unbounded(f"{ty}::{name} has payload {payload}")
    for x in exclude:
        if x not in [v[0] for v in enums[ty]]:
            raise SystemExit(f"exclusion {ty}::{x} is not a variant")
    return out


# ---- the manual source-site inventory, in machine form --------------------------------------------------------
# Body kinds: ("lit", s) | ("debug", Type, [excluded variants]) | ("cat", [kinds]) | ("helper", name).

L = lambda s: ("lit", s)
HELPERS = {
    "reason_label": dict(
        fn="reason_label", enum="Reason", block="match reason {", arms=7,
        arms_spec=[
            ("Entry", 'Reason::Entry(entry) => format!("Entry({entry:?})"),',
             [("cat", [L("Entry("), ("debug", "Entry", []), L(")")])]),
            ("Conflict", 'Reason::Conflict { known, .. } => match known { Known::Cause(cause) => { format!("Conflict(new '
             'or retained cause {})", cause_label(cause)) } Known::Explained(_) => "Conflict(explained)".to_string(), },',
             [("cat", [L("Conflict(new or retained cause "), ("helper", "cause_label"), L(")")]),
              L("Conflict(explained)")]),
            ("SessionClosed", 'Reason::SessionClosed { .. } => "SessionClosed".to_string(),', [L("SessionClosed")]),
            ("Corrected", 'Reason::Corrected { .. } => "Corrected".to_string(),', [L("Corrected")]),
            ("TableFull", 'Reason::TableFull { .. } => "TableFull".to_string(),', [L("TableFull")]),
            ("Constraint", 'Reason::Constraint => "Constraint".to_string(),', [L("Constraint")]),
            ("Storage", 'Reason::Storage => "Storage".to_string(),', [L("Storage")]),
        ],
        nested=[("Known", 'match known { Known::Cause(cause) =>', 2, ["Cause", "Explained"])]),
    "cause_label": dict(
        fn="cause_label", enum="CauseKind", block="match cause.kind() {", arms=3,
        arms_spec=[
            ("IdentityContent", 'CauseKind::IdentityContent { .. } => "IdentityContent",', [L("IdentityContent")]),
            ("DuplicateRole", 'CauseKind::DuplicateRole { .. } => "DuplicateRole",', [L("DuplicateRole")]),
            ("MissingPrepared", 'CauseKind::MissingPrepared { .. } => "MissingPrepared",', [L("MissingPrepared")]),
        ], nested=[]),
}

SITES = [
    dict(site="S01", fn="fact_append", call='obs.branch(format!("append_facts: {label}"));',
         ops=[dict(op="append_facts", block="let label = match &outcome {", arms=6,
                   context=['let outcome = run_op(|| core.append_facts(ticket, &batch, &mut io));'],
                   arms_spec=[
                       ('Some(FactOutcome::Committed) if issued => "Committed (inserted)".to_string(),',
                        [L("Committed (inserted)")]),
                       ('Some(FactOutcome::Committed) => "Committed (already present)".to_string(),',
                        [L("Committed (already present)")]),
                       ('Some(FactOutcome::NotApplied { counted, reason }) => { format!( "NotApplied {:?} {}", '
                        'counted.persistence, reason_label(reason) ) }',
                        [("cat", [L("NotApplied "), ("debug", "Persistence", []), L(" "), ("helper", "reason_label")])]),
                       ('Some(FactOutcome::Rejected(fault)) => format!("Rejected({fault:?})"),',
                        [("cat", [L("Rejected("), ("debug", "TicketFault", []), L(")")])]),
                       ('None if issued => "unwound after an insert was issued".to_string(),',
                        [L("unwound after an insert was issued")]),
                       ('None => "unwound before any insert".to_string(),', [L("unwound before any insert")]),
                   ])]),
    dict(site="S02", fn="step_fw", call='obs.branch("expire: Armed -> Counted".into());',
         ops=[dict(op="expire", block=None, arms=None, context=['let outcome = ticket.expire();'],
                   arms_spec=[('obs.branch("expire: Armed -> Counted".into());', [L("Armed -> Counted")])])]),
    dict(site="S03", fn="step_fw", call='obs.branch("DROP: a Counted ticket dropped".into());',
         ops=[dict(op="DROP", block=None, arms=None,
                   context=['let options: &[bool] = if config.drop && counted_ticket { &[false, true, false] } else '
                            '{ &[false, true] };'],
                   arms_spec=[('obs.branch("DROP: a Counted ticket dropped".into());',
                               [L("a Counted ticket dropped")])])]),
    dict(site="S04", fn="settlement_read", call='obs.branch(format!("{name}: {label}"));',
         ops=[dict(op="lookup", block="let label = match outcome {", arms=5, block_after="core.lookup(&key, &mut io)",
                   context=['let outcome = run_op(|| core.lookup(&key, &mut io));',
                            '(outcome.map(SeHeld::Lookup), label, "lookup")'],
                   arms_spec=[
                       ('Some(Lookup::Conflict(Known::Cause(_))) => "Conflict(cause)".to_string(),',
                        [L("Conflict(cause)")]),
                       ('Some(Lookup::Conflict(Known::Explained(_))) => "Conflict(explained)".to_string(),',
                        [L("Conflict(explained)")]),
                       ('Some(Lookup::Present(_)) => "Present".to_string(),', [L("Present")]),
                       ('Some(other) => format!("{other:?}"),', [("debug", "Lookup", ["Conflict", "Present"])]),
                       ('None => "unwound".to_string(),', [L("unwound")]),
                   ]),
              dict(op="explain", block="let label = match outcome {", arms=3, block_after="core.explain(&key, &mut io)",
                   context=['let outcome = run_op(|| core.explain(&key, &mut io));',
                            '(outcome.map(SeHeld::Explain), label, "explain")'],
                   arms_spec=[
                       ('Some(Explain::Explained(_)) => "Explained".to_string(),', [L("Explained")]),
                       ('Some(other) => format!("{other:?}"),', [("debug", "Explain", ["Explained"])]),
                       ('None => "unwound".to_string(),', [L("unwound")]),
                   ])]),
    dict(site="S05", fn="worker_op", call="obs.branch(label.clone());",
         ops=[dict(op="barrier", block=None, arms=None,
                   context=['let outcome = run_op(|| core.barrier(worker, &mut io));'],
                   arms_spec=[('format!( "barrier: {}", outcome.map_or("unwound".into(), |o| format!("{o:?}")) ),',
                               [L("unwound"), ("debug", "BarrierOutcome", [])])]),
              dict(op="resolve", block="let label = match outcome {", arms=5,
                   block_after="core.resolve(worker, token, &mut io)",
                   context=['let outcome = run_op(|| core.resolve(worker, token, &mut io));',
                            '(format!("resolve: {label}"), stale_answer)'],
                   arms_spec=[
                       ('Some(Resolve::Corrected(_)) => "Corrected".to_string(),', [L("Corrected")]),
                       ('Some(Resolve::ConflictPlaced(_)) => "ConflictPlaced".to_string(),', [L("ConflictPlaced")]),
                       ('Some(Resolve::Acknowledged(_)) => "Acknowledged".to_string(),', [L("Acknowledged")]),
                       ('Some(other) => format!("{other:?}"),',
                        [("debug", "Resolve", ["Corrected", "ConflictPlaced", "Acknowledged"])]),
                       ('None => "unwound".to_string(),', [L("unwound")]),
                   ]),
              dict(op="append_cell {Cell}", block="let label = match outcome {", arms=5,
                   block_after="core.append_cell(worker, token, &mut io)",
                   context=['Action::AppendC(token) | Action::AppendX(token) => {', 'let cell = token.cell;',
                            'let outcome = run_op(|| core.append_cell(worker, token, &mut io));',
                            '(format!("append_cell {cell:?}: {label}"), stale_answer)'],
                   arms_spec=[
                       ('Some(CellAppend::Committed(_)) if inserted => "Committed (inserted)".to_string(),',
                        [L("Committed (inserted)")]),
                       ('Some(CellAppend::Committed(_)) if log.is_empty() => { "Committed (already marked)".to_string() }',
                        [L("Committed (already marked)")]),
                       ('Some(CellAppend::Committed(_)) => "Committed (present)".to_string(),',
                        [L("Committed (present)")]),
                       ('Some(other) => format!("{other:?}"),', [("debug", "CellAppend", ["Committed"])]),
                       ('None => "unwound".to_string(),', [L("unwound")]),
                   ]),
              dict(op="empty", block=None, arms=None,
                   context=['let outcome = run_op(|| core.empty(worker, token));'],
                   arms_spec=[('format!( "empty: {}", outcome.map_or("unwound".into(), |o| format!("{o:?}")) ),',
                               [L("unwound"), ("debug", "Emptying", [])])])],
         outer=dict(block="let (label, stale_answer) = match action {", arms=5)),
    dict(site="S06", fn="step_gc", call='obs.branch(format!("append_gating: {label}"));',
         ops=[dict(op="append_gating", block="let label = match &outcome {", arms=5,
                   context=['let outcome = run_op(|| core.append_gating(&record, &mut io));'],
                   arms_spec=[
                       ('Some(GatingOutcome::Committed(_)) if issued => "Committed (inserted)".to_string(),',
                        [L("Committed (inserted)")]),
                       ('Some(GatingOutcome::Committed(_)) => "Committed (already present)".to_string(),',
                        [L("Committed (already present)")]),
                       ('Some(GatingOutcome::Refused { persistence, reason, }) => { format!("Refused {persistence:?} {}", '
                        'reason_label(reason)) }',
                        [("cat", [L("Refused "), ("debug", "Persistence", []), L(" "), ("helper", "reason_label")])]),
                       ('None if u_holds_it => "unwound after an insert (U kept, Unwound)".to_string(),',
                        [L("unwound after an insert (U kept, Unwound)")]),
                       ('None => "unwound before any insert (U emptied)".to_string(),',
                        [L("unwound before any insert (U emptied)")]),
                   ])]),
    dict(site="S07", fn="step_ack", call='obs.branch(format!("snapshot: {label}"));',
         ops=[dict(op="snapshot", block="let label = match &outcome {", arms=3, block_after="core.snapshot(nonce, &mut io)",
                   context=['let outcome = run_op(|| core.snapshot(nonce, &mut io));'],
                   arms_spec=[
                       ('Some(SnapshotOutcome::Taken(_)) => "Taken".to_string(),', [L("Taken")]),
                       ('Some(other) => format!("{other:?}"),', [("debug", "SnapshotOutcome", ["Taken"])]),
                       ('None => "unwound".to_string(),', [L("unwound")]),
                   ])]),
    dict(site="S08", fn="step_ack", call='obs.branch(format!("malformed snapshot: {}", MALFORMATIONS[which]));',
         ops=[dict(op="malformed snapshot", block=None, arms=None,
                   module_context=['const MALFORMATIONS: [&str; 3] = ["future", "set", "empty"];'],
                   context=['let options = if config.malformed { 2 + MALFORMATIONS.len() } else { 2 };',
                            'which => { let which = which - 2;'],
                   arms_spec=[('obs.branch(format!("malformed snapshot: {}", MALFORMATIONS[which]));',
                               [("const_array", "MALFORMATIONS")])])]),
    dict(site="S09", fn="step_ack", call='obs.branch(format!("append_acknowledgement: {label}"));',
         ops=[dict(op="append_acknowledgement", block="let label = match &outcome {", arms=4,
                   block_after="core.append_acknowledgement(snapshot, &mut io)",
                   context=['let outcome = run_op(|| core.append_acknowledgement(snapshot, &mut io));'],
                   arms_spec=[
                       ('Some(AckOutcome::Committed(_)) => "Committed".to_string(),', [L("Committed")]),
                       ('Some(other) => format!("{other:?}"),', [("debug", "AckOutcome", ["Committed"])]),
                       ('None if u_kept => "unwound after an insert (U kept)".to_string(),',
                        [L("unwound after an insert (U kept)")]),
                       ('None => "unwound before any insert (U emptied)".to_string(),',
                        [L("unwound before any insert (U emptied)")]),
                   ])]),
    dict(site="S10", fn="step_adm",
         call='obs.branch(format!( "admission: {}", if admitted { "admitted" } else { "refused" } ));',
         ops=[dict(op="admission", block=None, arms=None,
                   context=['let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;'],
                   arms_spec=[('if admitted { "admitted" } else { "refused" }', [L("admitted"), L("refused")])])]),
    dict(site="S11", fn="admit_by", call='obs.branch(format!("admission: {label}"));',
         ops=[dict(op="admission", block=None, arms=None,
                   context=['let admitted = admits(w, view);'],
                   arms_spec=[('let label = if admitted { "admitted" } else { "refused" };',
                               [L("admitted"), L("refused")])])]),
]

# Operations in report order (the static BRANCHES order, then operations it does not list).
OP_ORDER = ["append_facts", "expire", "append_gating", "lookup", "explain", "snapshot", "append_acknowledgement",
            "barrier", "resolve", "append_cell C", "append_cell X", "append_cell U", "empty", "admission", "DROP",
            "malformed snapshot"]

# Sites proved to represent the same operation and semantic branch (mission 5): S10 and S11 (see SOURCE-AUDIT.md
# section "Collision analysis"). Any other label emitted by two sites is an unresolved collision.
EQUIVALENT_SITES = {frozenset({"S10", "S11"}): (
    "S10 (step_adm) and S11 (admit_by) are the two implementations of the AdmStep type ('An admission step: step_adm, "
    "or a deliberately defective one in a negative case'): both are ADM's one admission step, both spend one "
    "admission, both label 'admission: admitted' exactly when that step admitted and 'admission: refused' otherwise, "
    "and both then run admission_properties when admitted. They differ only in the admission predicate. admit_by is "
    "not reachable from any required configuration's test (vocab_check_sites.py, reachability), so every required "
    "receipt attributes these labels to S10.")}


def expand(enums, consts, body, helpers_out):
    """[(string, derivation)] of one body kind."""
    kind = body[0]
    if kind == "lit":
        return [(body[1], "literal")]
    if kind == "debug":
        return [(s, "Debug " + d) for s, d in debug_values(enums, body[1], body[2])]
    if kind == "helper":
        return helpers_out[body[1]]
    if kind == "const_array":
        return [(s, f"{body[1]}[{i}]") for i, s in enumerate(consts[body[1]])]
    if kind == "cat":
        acc = [("", "")]
        for part in body[1]:
            vals = expand(enums, consts, part, helpers_out)
            acc = [(a + s, (da + " · " if da else "") + d) for a, da in acc for s, d in vals]
        return acc
    raise SystemExit(f"unknown body kind {kind}")


def load_sources(source_dir):
    if source_dir:
        rd = lambda p: open(os.path.join(source_dir, os.path.basename(p)), encoding="utf-8").read()
        return rd(common.MODEL), rd(common.PROTOCOL)
    return common.source(common.MODEL), common.source(common.PROTOCOL)


def generate(source_dir=None):
    problems = []
    model_raw, protocol_raw = load_sources(source_dir)
    model, protocol = strip_comments(model_raw), strip_comments(protocol_raw)
    enums = parse_enums(protocol)
    for k, v in parse_enums(model).items():
        if k in enums and k in USED_ENUMS:
            problems.append(f"enum {k}, interpolated into a label, is declared in both files")
        if k in USED_ENUMS or k not in enums:
            enums[k] = v
    consts = {}
    cm = re.search(r"const MALFORMATIONS: \[&str; (\d+)\] = \[(.*?)\];", model, re.S)
    consts["MALFORMATIONS"] = re.findall(r'"([^"]*)"', cm.group(2))
    if len(consts["MALFORMATIONS"]) != int(cm.group(1)):
        problems.append("MALFORMATIONS length")
    bm = re.search(r"const BRANCHES: &\[&str\] = &\[(.*?)\n\];", model, re.S)
    static = re.findall(r'"((?:[^"\\]|\\.)*)"', bm.group(1))

    def anchored(fn, start, body_raw, anchors, what):
        nb = norm(body_raw)
        for a in anchors:
            if norm(a) not in nb:
                problems.append(f"{what}: anchor not found verbatim in fn {fn}: {a!r}")

    # Helpers, cause_label first (reason_label interpolates it).
    helpers_out = {}
    for name in ("cause_label", "reason_label"):
        h = HELPERS[name]
        start, body = fn_body(model, h["fn"])
        anchored(h["fn"], start, body, [a for _, a, _ in h["arms_spec"]] + [h["block"]], f"helper {name}")
        raw_at = re.search(r"\s+".join(map(re.escape, h["block"].split())), body)
        if not raw_at:
            problems.append(f"helper {name}: match block not found")
        else:
            blk = body[raw_at.end() - 1:match_brace(body, raw_at.end() - 1)]
            if top_arms(blk) != h["arms"]:
                problems.append(f"helper {name}: {top_arms(blk)} arms in source, inventory has {h['arms']}")
        declared = [v[0] for v in enums[h["enum"]]]
        if sorted(declared) != sorted(v for v, _, _ in h["arms_spec"]):
            problems.append(f"helper {name}: arms cover {[v for v, _, _ in h['arms_spec']]}, {h['enum']} declares "
                            f"{declared}")
        for enum, anchor, n, variants in h["nested"]:
            if [v[0] for v in enums[enum]] != variants:
                problems.append(f"helper {name}: nested match over {enum} covers {variants}, {enum} declares "
                                f"{[v[0] for v in enums[enum]]}")
            raw_at = re.search(r"\s+".join(map(re.escape, "match known {".split())), body)
            if raw_at:
                blk = body[raw_at.end() - 1:match_brace(body, raw_at.end() - 1)]
                if top_arms(blk) != n:
                    problems.append(f"helper {name}: nested match over {enum} has {top_arms(blk)} arms, not {n}")
        records = []
        for variant, anchor, bodies in h["arms_spec"]:
            for b in bodies:
                for s, d in expand(enums, consts, b, helpers_out):
                    records.append((s, f"{name} {h['enum']}::{variant} arm: {d}"))
        helpers_out[name] = records

    model_lines = model_raw.splitlines()
    rows = {}
    sites_out = []
    for site in SITES:
        start, body = fn_body(model, site["fn"])
        nb = norm(body)
        occurrences = nb.count(norm(site["call"]))
        if occurrences != 1:
            problems.append(f"{site['site']}: call {site['call']!r} occurs {occurrences} times in fn {site['fn']}")
        # Line of the call in the original file: search the raw text from the function's start.
        call_re = re.compile(r"\s*".join(map(re.escape, norm(site["call"]).replace(" ", ""))))
        m = call_re.search(model, start)
        line = model.count("\n", 0, m.start()) + 1 if m else None
        if site.get("outer"):
            o = site["outer"]
            anchored(site["fn"], start, body, [o["block"]], site["site"])
            raw_at = re.search(r"\s+".join(map(re.escape, o["block"].split())), body)
            if raw_at:
                blk = body[raw_at.end() - 1:match_brace(body, raw_at.end() - 1)]
                if top_arms(blk) != o["arms"]:
                    problems.append(f"{site['site']}: outer match has {top_arms(blk)} arms, inventory {o['arms']}")
        site_ops = []
        for op in site["ops"]:
            anchors = [a for a, _ in op["arms_spec"]] + op["context"] + ([op["block"]] if op["block"] else [])
            anchored(site["fn"], start, body, anchors, f"{site['site']} {op['op']}")
            for a in op.get("module_context", []):
                if norm(model).count(norm(a)) != 1:
                    problems.append(f"{site['site']} {op['op']}: module-level anchor not found once: {a!r}")
            if op["block"]:
                # The label match that follows the operation's own core call (several sites hold more than one).
                after = body.find(op["block_after"]) if op.get("block_after") else 0
                if after < 0:
                    problems.append(f"{site['site']} {op['op']}: {op['block_after']!r} not found")
                    after = 0
                raw_at = re.compile(r"\s+".join(map(re.escape, op["block"].split()))).search(body, after)
                if not raw_at:
                    problems.append(f"{site['site']} {op['op']}: label block not found")
                else:
                    blk = body[raw_at.end() - 1:match_brace(body, raw_at.end() - 1)]
                    if top_arms(blk) != op["arms"]:
                        problems.append(f"{site['site']} {op['op']}: label match has {top_arms(blk)} arms in source, "
                                        f"inventory has {op['arms']}")
                    for a, _ in op["arms_spec"]:
                        if norm(a) not in norm(blk):
                            problems.append(f"{site['site']} {op['op']}: arm not inside its label match: {a!r}")
            ops = ([(op["op"].replace("{Cell}", s), f"Debug Cell::{s}") for s, _ in debug_values(enums, "Cell")]
                   if "{Cell}" in op["op"] else [(op["op"], "")])
            for opname, opderiv in ops:
                site_ops.append(opname)
                for arm, bodies in op["arms_spec"]:
                    for b in bodies:
                        try:
                            vals = expand(enums, consts, b, helpers_out)
                        except Unbounded as e:
                            problems.append(f"{site['site']} {opname}: unbounded domain: {e}")
                            continue
                        for s, d in vals:
                            label = f"{opname}: {s}"
                            key = (opname, label)
                            rec = rows.setdefault(key, dict(operation=opname, label=label, sites=[], arms=[],
                                                            derivations=[]))
                            if site["site"] not in [x["site"] for x in rec["sites"]]:
                                rec["sites"].append(dict(site=site["site"], file=common.MODEL, function=site["fn"],
                                                         line=line, call=site["call"]))
                            rec["arms"].append(norm(arm))
                            rec["derivations"].append((opderiv + " · " if opderiv else "") + d)
        sites_out.append(dict(site=site["site"], function=site["fn"], line=line, call=site["call"],
                              operations=sorted(set(site_ops), key=OP_ORDER.index)))

    # The inventory's size against the source: every `obs.branch(` call of model_tests.rs (receiver named obs).
    calls = len(re.findall(r"\bobs\s*\.\s*branch\s*\(", model))
    if calls != len(SITES):
        problems.append(f"model_tests.rs has {calls} obs.branch( calls; the inventory has {len(SITES)} sites")

    # Collisions: one label from two sites.
    collisions = []
    for (opname, label), rec in rows.items():
        ids = frozenset(s["site"] for s in rec["sites"])
        if len(ids) > 1:
            proved = EQUIVALENT_SITES.get(ids)
            collisions.append(dict(label=label, sites=sorted(ids), resolved=bool(proved), proof=proved))
            if not proved:
                problems.append(f"UNRESOLVED COLLISION: {label!r} from sites {sorted(ids)}")
    labels = [k[1] for k in rows]
    if len(labels) != len(set(labels)):
        problems.append("one label under two operations")
    for k, rec in rows.items():
        if not rec["label"].startswith(rec["operation"] + ": "):
            problems.append(f"label {rec['label']!r} does not start with its operation")
        if any(rec["operation"] != o and rec["label"].startswith(o + ": ") for o in OP_ORDER):
            problems.append(f"label {rec['label']!r} is ambiguous between operations")
    unknown_ops = sorted({k[0] for k in rows} - set(OP_ORDER))
    if unknown_ops:
        problems.append(f"operations outside OP_ORDER: {unknown_ops}")

    ordered = sorted(rows.values(), key=lambda r: (OP_ORDER.index(r["operation"]) if r["operation"] in OP_ORDER
                                                    else 99, first_index(rows, r)))
    vocab = []
    for i, rec in enumerate(ordered, start=1):
        vocab.append(dict(
            id=f"B{i:03d}", operation=rec["operation"], label=rec["label"],
            source_file=common.MODEL, source_items=sorted({s["function"] for s in rec["sites"]}),
            sites=rec["sites"], source_expressions=sorted(set(rec["arms"]), key=rec["arms"].index),
            derivation=sorted(set(rec["derivations"]), key=rec["derivations"].index),
            in_static_branches=rec["label"] in static,
            static_index=static.index(rec["label"]) if rec["label"] in static else None,
            note=("type-domain row: the interpolated `{cell:?}` ranges over Cell's declared variants; no reachability "
                  "argument is used to remove it" if rec["operation"] == "append_cell U" else "")))
    static_missing = [s for s in static if s not in labels]
    if static_missing:
        problems.append(f"static BRANCHES labels not derivable from source: {static_missing}")
    if len(static) != len(set(static)):
        problems.append("static BRANCHES has a duplicate")
    return dict(generator=GENERATOR_VERSION, commit=common.C,
                source_blobs={p: common.BLOBS[p] for p in (common.MODEL, common.PROTOCOL)} if not source_dir else
                "OVERRIDDEN (negative control)",
                accounting_unit="(operation identity, exact emitted label)",
                rows=vocab, sites=sites_out, collisions=collisions, static_branches=static,
                static_missing_from_vocabulary=static_missing,
                vocabulary_missing_from_static=[r["label"] for r in vocab if not r["in_static_branches"]],
                operations=OP_ORDER, enums_used={k: [v[0] for v in enums[k]] for k in sorted(USED_ENUMS)
                                                 if k in enums},
                problems=problems)


USED_ENUMS = ["Persistence", "Reason", "Entry", "Known", "CauseKind", "FactOutcome", "TicketFault", "GatingOutcome",
              "Lookup", "Explain", "SnapshotOutcome", "AckRefusal", "AckOutcome", "BarrierOutcome", "Resolve",
              "CellAppend", "Emptying", "Cell", "Action"]


def first_index(rows, rec):
    return list(rows.values()).index(rec)


def main(argv):
    source_dir = out = None
    if "--source-dir" in argv:
        source_dir = argv[argv.index("--source-dir") + 1]
    if "--out" in argv:
        out = argv[argv.index("--out") + 1]
    v = generate(source_dir)
    if "--compare" in argv:
        # The freeze check: the vocabulary regenerated from source must equal the given one, row for row.
        other = json.load(open(argv[argv.index("--compare") + 1]))
        key = lambda r: (r["id"], r["operation"], r["label"], tuple((s["site"], s["function"], s["line"]) for s in
                                                                     r["sites"]), r["in_static_branches"])
        mine, theirs = [key(r) for r in v["rows"]], [key(r) for r in other["rows"]]
        diffs = [f"only regenerated: {x}" for x in mine if x not in theirs] + \
                [f"only in the given vocabulary: {x}" for x in theirs if x not in mine]
        same_sites = [(s["site"], s["function"], s["line"]) for s in v["sites"]] == \
                     [(s["site"], s["function"], s["line"]) for s in other["sites"]]
        same_static = v["static_branches"] == other["static_branches"]
        same_coll = [(c["label"], c["sites"], c["resolved"]) for c in v["collisions"]] == \
                    [(c["label"], c["sites"], c["resolved"]) for c in other["collisions"]]
        ok = not diffs and same_sites and same_static and same_coll and not v["problems"]
        print(f"COMPARE {GENERATOR_VERSION}: regenerated {len(mine)} rows, given {len(theirs)}; row differences "
              f"{len(diffs)}; sites equal {same_sites}; static BRANCHES equal {same_static}; collisions equal "
              f"{same_coll}; generator problems {len(v['problems'])}: {'EQUAL' if ok else 'DIFFERENT'}")
        for d in diffs[:20]:
            print("  ", d)
        for p in v["problems"]:
            print("PROBLEM:", p)
        return 0 if ok else 1
    text = json.dumps(v, indent=1, ensure_ascii=False)
    if out:
        with open(out, "w") as f:
            f.write(text + "\n")
    print(f"{GENERATOR_VERSION}: {len(v['rows'])} rows; {len(v['sites'])} sites; static BRANCHES {len(v['static_branches'])}"
          f"; not in static {len(v['vocabulary_missing_from_static'])}; collisions {len(v['collisions'])} "
          f"({sum(not c['resolved'] for c in v['collisions'])} unresolved); problems {len(v['problems'])}")
    for p in v["problems"]:
        print("PROBLEM:", p)
    return 0 if not v["problems"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
