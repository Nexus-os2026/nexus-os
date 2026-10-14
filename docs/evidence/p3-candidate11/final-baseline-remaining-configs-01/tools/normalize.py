#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission sections 6 and 10: branch accounting of one exploration report
over the frozen source-bound vocabulary.

  normalize.py stage1 OUTDIR            the ten preserved Stage-1 unchanged-core reports, read from the Stage-1
                                        evidence commit's git objects (authenticated against its SHA256SUMS)
  normalize.py run CFG RAW OUTDIR       one report of this mission (CFG e.g. F-G; RAW its raw-output.txt)
  normalize.py file CFG RAW OUTDIR      the same, for a synthetic or historical input (development and controls)

For each report: every vocabulary row is TAKEN, NOT TAKEN, or NOT APPLICABLE (only where the row's whole operation is
absent by configuration, with the source-anchored reason of configs.py). A row that is TAKEN in an operation the
configuration makes absent is a contradiction. The explorer prints every BRANCHES label, taken or not, in order, then
every other taken label once, sorted, marked '(not in the listed set)' (vocab_check_sites.py A4); so a vocabulary row
that is neither listed nor printed as taken was not taken.

STOP (exit 2, no table written as valid) when: the branch list is malformed (BRANCHES not printed exactly, in order;
an extra line not of the printed form, unsorted or duplicated); a raw label (listed or extra) is outside the
vocabulary; an NOT APPLICABLE operation is taken; the vocabulary or configuration does not verify.
The original raw text is never edited: the table carries the raw branch lines verbatim and the raw file's sha256.
"""
import hashlib
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402
import configs  # noqa: E402

NORMALIZER_VERSION = "normalize v1"
VOCAB = os.path.join(common.EV, "vocabulary/VOCABULARY.json")
LISTED = re.compile(r"^    \[(taken    |NOT TAKEN)\] (.*)$")
EXTRA_SUFFIX = " (not in the listed set)"
# Stage-1 receipts: configuration -> (path in the Stage-1 packet, how to read it).
STAGE1 = {
    "X-1": ("receipts/30-suite/raw-output.txt", "x1_two_facts_two_writers_one_admission"),
    "X-2": ("receipts/30-suite/raw-output.txt", "x2_one_fact_two_gating_callers"),
    "X-3": ("receipts/30-suite/raw-output.txt", "x3_one_conflicting_fact_with_env"),
    "X-4": ("receipts/30-suite/raw-output.txt", "x4_one_conflicting_fact_one_acknowledgement"),
    "X-5": ("receipts/30-suite/raw-output.txt", "x5_one_clean_batch_of_two"),
    "F-1": ("receipts/45-full-F-1/raw-output.txt", None),
    "F-2": ("receipts/41-full-F-2/raw-output.txt", None),
    "F-3": ("receipts/43-full-F-3/raw-output.txt", None),
    "F-PT": ("receipts/44-full-F-PT/raw-output.txt", None),
    "F-B": ("receipts/42-full-F-B/raw-output.txt", None),
}
STATUS = ("TAKEN", "NOT TAKEN", "NOT APPLICABLE")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load_vocab(path=VOCAB):
    v = json.load(open(path))
    return v, sha(open(path, "rb").read())


def report_block(text):
    """The lines of the one EXPLORATION report in text, from its header to its key-fields line (inclusive)."""
    lines = text.splitlines()
    heads = [i for i, l in enumerate(lines) if "EXPLORATION " in l]
    if len(heads) != 1:
        return None, f"{len(heads)} EXPLORATION headers"
    end = next((j for j in range(heads[0], len(lines)) if lines[j].startswith("  state key fields: ")), None)
    if end is None:
        return None, "no state key fields line"
    return lines[heads[0]:end + 1], None


def suite_segment(text, test):
    """The output lines of one test of the unfiltered suite (as Stage 1's check_suite.py segments it)."""
    start = re.compile(r"^test (\S+)( - should panic)? \.\.\. ?(.*)$")
    result = re.compile(r"^test result: ")
    out, cur = None, None
    for l in text.splitlines():
        m = start.match(l)
        if m:
            if cur is not None and out is None and cur[0] == common.TEST_PREFIX + test:
                out = cur
            cur = [m.group(1), [m.group(3)]]
            continue
        if result.match(l) or l == "failures:":
            if cur is not None and out is None and cur[0] == common.TEST_PREFIX + test:
                out = cur
            cur = None
            continue
        if cur is not None:
            cur[1].append(l)
    if out is None and cur is not None and cur[0] == common.TEST_PREFIX + test:
        out = cur
    if out is None:
        return None, f"test {test} not found in the suite output"
    body = [l for l in out[1]]
    while body and body[-1].strip() == "":
        body.pop()
    if not body or body[-1] != "ok":
        return None, f"test {test}: not 'ok' ({body[-1] if body else 'empty'})"
    return "\n".join(body[:-1]), None


def parse_branches(block, static):
    """(listed [(mark, label)], extras [label], raw branch lines, problems)."""
    problems = []
    try:
        b0 = block.index("  branches:")
    except ValueError:
        return [], [], [], ["no '  branches:' line"]
    raw = block[b0 + 1:-1]  # up to the key-fields line, exclusive
    listed, extras = [], []
    for i, l in enumerate(raw):
        m = LISTED.match(l)
        if not m:
            problems.append(f"branch line {i + 1} is not of the printed form: {l!r}")
            continue
        mark, label = m.group(1), m.group(2)
        if i < len(static):
            listed.append((mark, label))
        else:
            if mark != "taken    " or not label.endswith(EXTRA_SUFFIX):
                problems.append(f"line {i + 1} after the listed set is not '[taken    ] … (not in the listed set)': {l!r}")
                continue
            extras.append(label[:-len(EXTRA_SUFFIX)])
    if [lab for _, lab in listed] != static:
        problems.append(f"the listed section is not exactly the static BRANCHES list in order "
                        f"({len(listed)} lines; first difference at "
                        f"{next((k for k, (a, b) in enumerate(zip([x for _, x in listed], static)) if a != b), 'length')})")
    if extras != sorted(extras):
        problems.append("the extra labels are not in sorted (BTreeSet) order")
    if len(set(extras)) != len(extras):
        problems.append("an extra label is printed twice")
    dup = [x for x in extras if x in static]
    if dup:
        problems.append(f"an extra label is a BRANCHES label: {dup}")
    return listed, extras, raw, problems


def normalize(text, cfg, vocab, derived, source_desc):
    """(table dict, problems). problems non-empty means STOP."""
    problems = []
    static = vocab["static_branches"]
    block, why = report_block(text)
    if block is None:
        return None, [f"report not found: {why}"]
    listed, extras, raw_lines, p = parse_branches(block, static)
    problems += p
    req = derived["required"].get(cfg)
    if req is None:
        return None, problems + [f"configuration {cfg} is not a required configuration"]
    head = block[0][block[0].index("EXPLORATION "):]
    if head.split()[1] != cfg:
        problems.append(f"the report's header names {head.split()[1]}, not {cfg}")
    na = req["na"]
    vrows = {r["label"]: r for r in vocab["rows"]}
    taken_listed = {lab for mark, lab in listed if mark == "taken    "}
    notaken_listed = {lab for mark, lab in listed if mark == "NOT TAKEN"}
    outside = sorted((set(lab for _, lab in listed) | set(extras)) - set(vrows))
    if outside:
        problems.append(f"raw label(s) outside the frozen vocabulary: {outside}")
    rows = []
    for r in vocab["rows"]:
        lab = r["label"]
        if lab in taken_listed:
            ev, st = "listed taken", "TAKEN"
        elif lab in extras:
            ev, st = "printed taken (not in the listed set)", "TAKEN"
        elif lab in notaken_listed:
            ev, st = "listed NOT TAKEN", "NOT TAKEN"
        else:
            ev, st = "not printed (not a BRANCHES label, not taken)", "NOT TAKEN"
        reason = ""
        if r["operation"] in na:
            if st == "TAKEN":
                problems.append(f"{r['id']} {lab!r} is TAKEN but its operation is NOT APPLICABLE here: "
                                f"{na[r['operation']]}")
            else:
                st, reason = "NOT APPLICABLE", na[r["operation"]]
        rows.append(dict(id=r["id"], operation=r["operation"], label=lab, status=st, raw_evidence=ev,
                         na_reason=reason, in_static_branches=r["in_static_branches"]))
    ops = {}
    for row in rows:
        o = ops.setdefault(row["operation"], {"rows": 0, "TAKEN": 0, "NOT TAKEN": 0, "NOT APPLICABLE": 0})
        o["rows"] += 1
        o[row["status"]] += 1
    table = dict(
        normalizer=NORMALIZER_VERSION, configuration=cfg, test=req["test"], explorer=req["explorer"],
        source=source_desc, report_header=head,
        vocabulary_rows=len(rows),
        totals={s: sum(r["status"] == s for r in rows) for s in STATUS},
        taken_outside_static=[r["label"] for r in rows if r["status"] == "TAKEN" and not r["in_static_branches"]],
        not_applicable_operations=na,
        operations=ops, rows=rows,
        raw_branch_lines=raw_lines, raw_listed_taken=len(taken_listed), raw_listed_not_taken=len(notaken_listed),
        raw_extra_taken=extras, problems=problems,
        verdict="NORMALIZED" if not problems else "STOP")
    return table, problems


def write(table, outdir, name):
    os.makedirs(outdir, exist_ok=True)
    with open(os.path.join(outdir, name + ".json"), "w") as f:
        json.dump(table, f, indent=1, ensure_ascii=False)
        f.write("\n")
    md = [f"# Branch accounting — {table['configuration']} ({table['explorer']} explorer)", "",
          f"- test: `{table['test']}`",
          f"- source: {table['source']}",
          f"- header: `{table['report_header']}`",
          f"- vocabulary rows: {table['vocabulary_rows']}; TAKEN {table['totals']['TAKEN']}, NOT TAKEN "
          f"{table['totals']['NOT TAKEN']}, NOT APPLICABLE {table['totals']['NOT APPLICABLE']}",
          f"- raw: {table['raw_listed_taken']} listed taken, {table['raw_listed_not_taken']} listed NOT TAKEN, "
          f"{len(table['raw_extra_taken'])} printed taken outside the listed set",
          f"- TAKEN labels missing from the static BRANCHES constant: "
          f"{', '.join('`' + x + '`' for x in table['taken_outside_static']) or 'none'}",
          f"- verdict: **{table['verdict']}**" + (f" — {table['problems']}" if table['problems'] else ""), "",
          "## NOT APPLICABLE operations (absent by configuration)", ""]
    md += [f"- `{op}`: {why}" for op, why in table["not_applicable_operations"].items()] or ["- none"]
    md += ["", "## Operation totals", "", "| operation | rows | TAKEN | NOT TAKEN | NOT APPLICABLE |", "|---|---|---|---|---|"]
    for op, o in table["operations"].items():
        md.append(f"| {op} | {o['rows']} | {o['TAKEN']} | {o['NOT TAKEN']} | {o['NOT APPLICABLE']} |")
    md += ["", "## Rows", "", "| id | operation | label | status | raw evidence | static |", "|---|---|---|---|---|---|"]
    for r in table["rows"]:
        md.append(f"| {r['id']} | {r['operation']} | `{r['label']}` | {r['status']} | {r['raw_evidence']} | "
                  f"{'yes' if r['in_static_branches'] else 'NO'} |")
    md += ["", "## Raw branch lines (verbatim)", "", "```"] + table["raw_branch_lines"] + ["```", ""]
    with open(os.path.join(outdir, name + ".md"), "w") as f:
        f.write("\n".join(md))


def stage1(outdir, vocab_path=VOCAB):
    vocab, vsha = load_vocab(vocab_path)
    derived = configs.derive()
    if derived["problems"]:
        raise SystemExit(f"STOP: configuration derivation: {derived['problems']}")
    sums = {}
    for l in common.blob(common.STAGE1, common.STAGE1_PACKET + "/SHA256SUMS").decode().splitlines():
        d, rel = l.split("  ", 1)
        sums[rel] = d
    summary, stop = [], False
    for cfg, (rel, test) in STAGE1.items():
        data = common.blob(common.STAGE1, f"{common.STAGE1_PACKET}/{rel}")
        blob_id = common.git("rev-parse", f"{common.STAGE1}:{common.STAGE1_PACKET}/{rel}").decode().strip()
        if sha(data) != sums.get(rel):
            raise SystemExit(f"STOP: Stage-1 {rel} does not match its SHA256SUMS")
        text = data.decode()
        desc = (f"Stage-1 commit {common.STAGE1}, path {common.STAGE1_PACKET}/{rel}, blob {blob_id}, sha256 "
                f"{sha(data)} (equals the Stage-1 SHA256SUMS entry)")
        if test:
            seg, why = suite_segment(text, test)
            if seg is None:
                raise SystemExit(f"STOP: {cfg}: {why}")
            desc += f"; the output segment of test {common.TEST_PREFIX}{test} (segment sha256 {sha(seg.encode())})"
            text = seg
        table, problems = normalize(text, cfg, vocab, derived, desc)
        table = table or dict(configuration=cfg, verdict="STOP", problems=problems)
        table["vocabulary_sha256"] = vsha
        write(table, outdir, cfg)
        summary.append(f"{cfg:5} {table['verdict']}: TAKEN {table.get('totals', {}).get('TAKEN')}, NOT TAKEN "
                       f"{table.get('totals', {}).get('NOT TAKEN')}, NOT APPLICABLE "
                       f"{table.get('totals', {}).get('NOT APPLICABLE')}; taken outside static "
                       f"{table.get('taken_outside_static')}" + (f"; PROBLEMS {problems}" if problems else ""))
        stop |= bool(problems)
    summary.append(f"STAGE-1 NORMALIZATION ({NORMALIZER_VERSION}, vocabulary sha256 {vsha}): "
                   f"{'ALL NORMALIZED' if not stop else 'STOP'}")
    with open(os.path.join(outdir, "SUMMARY.txt"), "w") as f:
        f.write("\n".join(summary) + "\n")
    print("\n".join(summary))
    return 0 if not stop else 2


def one(cfg, raw, outdir, vocab_path=VOCAB, label="run"):
    vocab, vsha = load_vocab(vocab_path)
    derived = configs.derive()
    if derived["problems"]:
        raise SystemExit(f"STOP: configuration derivation: {derived['problems']}")
    data = open(raw, "rb").read()
    table, problems = normalize(data.decode("utf-8", errors="replace"), cfg, vocab, derived,
                                f"{label} {raw}, sha256 {sha(data)}")
    table = table or dict(configuration=cfg, verdict="STOP", problems=problems)
    table["vocabulary_sha256"] = vsha
    write(table, outdir, f"BRANCH-ACCOUNTING-{cfg}")
    print(f"{cfg}: {table['verdict']}; totals {table.get('totals')}; taken outside static "
          f"{table.get('taken_outside_static')}" + (f"; PROBLEMS {problems}" if problems else ""))
    return 0 if not problems else 2


if __name__ == "__main__":
    a = sys.argv[1:]
    vp = a[a.index("--vocab") + 1] if "--vocab" in a else VOCAB
    if a[0] == "stage1":
        sys.exit(stage1(a[1], vp))
    if a[0] in ("run", "file"):
        sys.exit(one(a[1], a[2], a[3], vp, label="this mission's raw output" if a[0] == "run" else "input file"))
    raise SystemExit("usage")
