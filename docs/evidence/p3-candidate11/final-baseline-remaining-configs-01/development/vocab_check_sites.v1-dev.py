#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 5: static CHECK A — site, sink and printer closure.

  vocab_check_sites.py VOCABULARY.json [--source-dir DIR]

Independent of the generator: it imports nothing from it, reads every Rust file of the crate at C (git objects; or
--source-dir for negative controls, which then replaces the two protocol files only), masks comments and literals
with its own scanner, and works on lines and regular expressions. It establishes:

  A1 every `.branch(` method call in all 49 crate files, with its receiver resolved to a type (Obs or Oracle) from
     the enclosing function's parameters, its `let` bindings or the enclosing impl's struct fields; an unresolved
     receiver fails;
  A2 the Obs-typed call sites equal the vocabulary's sites exactly (function, line and call text): an omitted or an
     extra site fails;
  A3 the sink closure: Obs::branch is the only writer of an Obs's branches; every other mention of a `branches`
     field only reads it, or initialises a record's field from another record's or an Obs's `branches`; the only
     `branches` fields of another type are witness expectations (static string lists), and every label in them is a
     vocabulary label;
  A4 the printer closure: every report line `    [taken    ]`/`    [{mark}]` is printed from the static BRANCHES list
     or from the explorer's `taken` set, and `taken` is extended only from `.branches` of a successor/item/observation;
  A5 reachability: the conservative identifier-reference call graph of model_tests.rs from the required tests'
     entry points; every vocabulary site is reported reachable or not, and a label shared by two sites must be shared
     with a site no required test reaches (the proved S10/S11 equivalence) — otherwise an unresolved collision;
  A6 Debug is derived (never implemented by hand anywhere in the crate) for every enum a label interpolates;
  A7 every complete branch-label string literal in model_tests.rs (one that starts with a vocabulary operation and
     ': ' and has no format hole) is a vocabulary label;
  A8 every vocabulary row names at least one site, and its label starts with its operation.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402

CHECKER_VERSION = "vocab_check_sites v1"
LABEL_ENUMS = ["Persistence", "Reason", "Entry", "Known", "CauseKind", "FactOutcome", "TicketFault", "GatingOutcome",
               "Lookup", "Explain", "SnapshotOutcome", "AckRefusal", "AckOutcome", "BarrierOutcome", "Resolve",
               "CellAppend", "Emptying", "Cell"]
# Entry points of every required unchanged-core run: the five default explorations (X-1..X-5), every full_* test,
# and the composed-quotient tests of Stage 1 (F-3, F-PT, F-B).
REQUIRED_TESTS = (["x1_two_facts_two_writers_one_admission", "x2_one_fact_two_gating_callers",
                   "x3_one_conflicting_fact_with_env", "x4_one_conflicting_fact_one_acknowledgement",
                   "x5_one_clean_batch_of_two"] +
                  ["full_" + k for k in ("f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "fpt", "fb", "fb2", "fg",
                                         "fw", "fd", "fr")] +
                  ["gc_symmetry_f3", "gc_symmetry_fpt", "gc_symmetry_fb"])


def mask(text):
    """Comments → spaces; string and char literal contents → 'x' (quotes kept); newlines kept everywhere."""
    out = []
    i, n = 0, len(text)
    state = None
    depth = 0
    while i < n:
        c = text[i]
        if state is None:
            if c == "/" and i + 1 < n and text[i + 1] == "/":
                state = "line"
                out.append("  ")
                i += 2
                continue
            if c == "/" and i + 1 < n and text[i + 1] == "*":
                state, depth = "block", 1
                out.append("  ")
                i += 2
                continue
            if c == "r" and re.match(r'r#*"', text[i:i + 8]) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
                hashes = re.match(r'r(#*)"', text[i:]).group(1)
                end = text.index('"' + hashes, i + 2 + len(hashes))
                seg = text[i:end + 1 + len(hashes)]
                out.append(re.sub(r"[^\n]", "x", seg))
                i = end + 1 + len(hashes)
                continue
            if c == '"':
                j = i + 1
                while text[j] != '"':
                    j += 2 if text[j] == "\\" else 1
                out.append('"' + re.sub(r"[^\n]", "x", text[i + 1:j]) + '"')
                i = j + 1
                continue
            if c == "'":
                m = re.match(r"'(\\.[^']*|[^\\'])'", text[i:])
                if m:
                    out.append("'" + "x" * (len(m.group(0)) - 2) + "'")
                    i += len(m.group(0))
                    continue
            out.append(c)
            i += 1
        elif state == "line":
            if c == "\n":
                state = None
                out.append("\n")
            else:
                out.append(" ")
            i += 1
        else:
            if text.startswith("/*", i):
                depth += 1
                out.append("  ")
                i += 2
            elif text.startswith("*/", i):
                depth -= 1
                out.append("  ")
                i += 2
                if depth == 0:
                    state = None
            else:
                out.append("\n" if c == "\n" else " ")
                i += 1
    s = "".join(out)
    assert len(s) == len(text), "mask changed the length"
    return s


def close_of(masked, open_at):
    depth = 0
    for i in range(open_at, len(masked)):
        if masked[i] == "{":
            depth += 1
        elif masked[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    raise SystemExit("unbalanced")


def functions(masked):
    """[(name, sig_start, body_open, body_close)] for every fn with a body."""
    out = []
    for m in re.finditer(r"\bfn\s+(\w+)\b", masked):
        j, depth = m.end(), 0
        while j < len(masked):
            ch = masked[j]
            if ch in "(<[":
                depth += 1
            elif ch in ")>]":
                depth -= 1
            elif ch == ";" and depth <= 0:
                j = None
                break
            elif ch == "{" and depth <= 0:
                break
            j += 1
        if j is None or j >= len(masked):
            continue
        out.append((m.group(1), m.start(), j, close_of(masked, j)))
    return out


def enclosing_fn(fns, pos):
    best = None
    for f in fns:
        if f[2] <= pos <= f[3] and (best is None or f[2] > best[2]):
            best = f
    return best


def impls(masked):
    out = []
    for m in re.finditer(r"\bimpl\b(<[^>]*>)?\s+([^{;]*?)\s*\{", masked):
        target = m.group(2)
        name = re.findall(r"(\w+)\s*(?:<[^>]*>)?\s*$", target.split(" for ")[-1])
        out.append((name[0] if name else target, m.end() - 1, close_of(masked, m.end() - 1)))
    return out


def struct_fields(masked, name):
    m = re.search(r"\bstruct\s+" + name + r"\b[^{;(]*\{", masked)
    if not m:
        return {}
    body = masked[m.end():close_of(masked, m.end() - 1)]
    return {f: t.strip() for f, t in re.findall(r"(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:\s*([^,]+),", body)}


def line_of(text, pos):
    return text.count("\n", 0, pos) + 1


def load(source_dir):
    files = [p for p in common.git("ls-tree", "-r", "--name-only", common.C, common.CRATE_SRC + "/").decode().split()
             if p.endswith(".rs")]
    texts = {}
    for p in files:
        if source_dir and p in (common.MODEL, common.PROTOCOL):
            texts[p] = open(os.path.join(source_dir, os.path.basename(p)), encoding="utf-8").read()
        else:
            texts[p] = common.source(p)
    return texts


def receiver_type(masked, fns, pos, recv):
    f = enclosing_fn(fns, pos)
    if f is None:
        return None, "no enclosing fn"
    name, sig, bo, bc = f
    sig_text = masked[sig:bo]
    body = masked[bo:pos]
    if recv.startswith("self."):
        field = recv[5:]
        for iname, io_, ic in IMPLS_CACHE:
            if io_ <= pos <= ic:
                t = struct_fields(MASKED_CACHE, iname).get(field)
                if t:
                    return ("Oracle" if re.search(r"\bOracle\b", t) else "Obs" if re.search(r"\bObs\b", t) else t), \
                        f"field {iname}.{field}: {t}"
        return None, f"field {recv} unresolved"
    if recv == "self":
        for iname, io_, ic in IMPLS_CACHE:
            if io_ <= pos <= ic:
                return iname, f"impl {iname}"
        return None, "self outside impl"
    p = re.search(r"\b" + re.escape(recv) + r"\s*:\s*&\s*mut\s+(\w+)", sig_text)
    if p:
        return p.group(1), f"parameter {recv}: &mut {p.group(1)} of fn {name}"
    lets = list(re.finditer(r"\blet\s+(?:mut\s+)?" + re.escape(recv) + r"\b\s*(?::\s*(\w+))?\s*=\s*(\w+)", body))
    if lets:
        m = lets[-1]
        t = m.group(1) or m.group(2)
        return t, f"let {recv} = {m.group(2)}… in fn {name}"
    return None, f"{recv} unresolved in fn {name}"


IMPLS_CACHE, MASKED_CACHE = [], ""


def main(argv):
    global IMPLS_CACHE, MASKED_CACHE
    vocab = json.load(open(argv[0]))
    source_dir = argv[argv.index("--source-dir") + 1] if "--source-dir" in argv else None
    texts = load(source_dir)
    lines, fails = [], []

    def check(label, ok, detail=""):
        lines.append(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
        if not ok:
            fails.append(label)

    check("crate sources read", len(texts) == 49, f"{len(texts)} .rs files under {common.CRATE_SRC} at C"
          + (" (protocol.rs and model_tests.rs OVERRIDDEN for a negative control)" if source_dir else ""))
    masked = {p: mask(t) for p, t in texts.items()}

    # A1, A2.
    obs_sites, other_sites, unresolved = [], [], []
    for p, mt in masked.items():
        fns = functions(mt)
        IMPLS_CACHE, MASKED_CACHE = impls(mt), mt
        for m in re.finditer(r"([\w.]+)\s*\.\s*branch\s*\(", mt):
            recv = m.group(1)
            t, why = receiver_type(mt, fns, m.start(), recv)
            f = enclosing_fn(fns, m.start())
            ln = line_of(mt, m.start())
            call_end = texts[p].index(";", m.start()) + 1
            call = re.sub(r"\s+", " ", texts[p][m.start():call_end]).strip()
            rec = dict(file=p, line=ln, function=f[0] if f else None, receiver=recv, type=t, why=why, call=call)
            if t == "Obs":
                obs_sites.append(rec)
            elif t == "Oracle":
                other_sites.append(rec)
            else:
                unresolved.append(rec)
    check("A1 every .branch( call's receiver resolves to Obs or Oracle", not unresolved,
          f"{len(obs_sites)} Obs, {len(other_sites)} Oracle, unresolved {unresolved}")
    for r in other_sites:
        lines.append(f"     Oracle::branch (a choice, emits no label): {r['file'].split('/')[-1]}:{r['line']} fn "
                     f"{r['function']}: {r['receiver']}.branch — {r['why']}")
    norm = lambda s: re.sub(r"\s+", "", s)
    vsites = {(s["function"], s["line"], norm(s["call"])): s["site"] for s in vocab["sites"]}
    found = {(r["function"], r["line"], norm(r["call"]).split(".branch(", 1)[0].split()[-1] if False else
              norm(r["call"])): r for r in obs_sites}
    found_keys = {(r["function"], r["line"], norm(r["call"])) for r in obs_sites}
    missing = sorted(set(vsites) - found_keys)
    extra = sorted(found_keys - set(vsites))
    check("A2 Obs::branch call sites equal the vocabulary's sites (function, line, call text)", not missing and not
          extra, f"{len(found_keys)} in source, {len(vsites)} in the vocabulary; in the vocabulary only {missing}; in "
          f"the source only {extra}")
    for r in sorted(obs_sites, key=lambda r: (r["file"], r["line"])):
        lines.append(f"     Obs::branch site {vsites.get((r['function'], r['line'], norm(r['call'])), 'UNLISTED')}: "
                     f"{r['file'].split('/')[-1]}:{r['line']} fn {r['function']} — {r['why']}")
    check("A2 Obs::branch sites lie only in model_tests.rs", all(r["file"] == common.MODEL for r in obs_sites),
          sorted({r["file"] for r in obs_sites}))

    # A3: sink closure.
    writes, reads, inits, decls, problems_a3 = [], [], [], [], []
    witness_labels = []
    for p, mt in masked.items():
        fns = functions(mt)
        for m in re.finditer(r"\bbranches\b", mt):
            ln = line_of(mt, m.start())
            src_line = texts[p].splitlines()[ln - 1].strip()
            before = mt[max(0, m.start() - 80):m.start()]
            after = mt[m.end():m.end() + 80]
            f = enclosing_fn(fns, m.start())
            where = f"{p.split('/')[-1]}:{ln} fn {f[0] if f else '-'}: {src_line}"
            if re.match(r"\s*\.\s*(push|extend|insert|append|retain|clear|drain|truncate|sort|dedup|swap|pop)\b", after) \
                    or re.match(r"\s*=[^=]", after) or re.search(r"&\s*mut\s+[\w.]*$", before) \
                    or re.search(r"mem::(take|replace|swap)\s*\(\s*&\s*mut\s+[\w.]*$", before):
                writes.append((where, f[0] if f else None, before.strip()[-20:], after.strip()[:20]))
            elif re.match(r"\s*:", after) and not re.match(r"\s*::", after):
                # A field declaration (`branches: Type,` in a struct) or an initialiser (`branches: expr,`).
                init = re.match(r"\s*:\s*([^,\n}]+)", after).group(1).strip()
                if f is None or re.match(r"^(Vec<String>|&'static \[&'static str\])$", texts[p][m.end():m.end() + 80]
                                          .split(":", 1)[1].split(",")[0].strip()) and f is None:
                    decls.append((where, texts[p][m.end():m.end() + 80].split(":", 1)[1].split(",")[0].strip()))
                else:
                    inits.append((where, init))
            else:
                reads.append(where)
    ok_writes = [w for w in writes if w[1] == "branch" and "self" in w[2] and w[3].startswith(".push(label")]
    bad_writes = [w for w in writes if w not in ok_writes]
    check("A3 the only write to a branches field is Obs::branch's self.branches.push(label)",
          len(ok_writes) == 1 and not bad_writes, f"Obs::branch writes {len(ok_writes)}; other writes {bad_writes}")
    check("A3 branches fields are declared only as Vec<String> (records) or &'static [&'static str] (witness "
          "expectations)", all(t in ("Vec<String>", "&'static [&'static str]") for _, t in decls),
          [d for d in decls if d[1] not in ("Vec<String>", "&'static [&'static str]")])
    good_init = re.compile(r"^(obs|s|item|observed|successor|record|e|g)\.branches(\.clone\(\))?$")
    bad_inits = [i for i in inits if not good_init.match(re.sub(r"\s+", "", i[1]))]
    check("A3 every record's branches field is initialised from an Obs's or another record's branches",
          not bad_inits, f"{len(inits)} initialisers; others {bad_inits}")
    for i in inits:
        lines.append(f"     initialiser: {i[0]}")
    # Witness expectation lists: every literal label they hold must be a vocabulary label.
    vlabels = {r["label"] for r in vocab["rows"]}
    model = texts[common.MODEL]
    for m in re.finditer(r"branches:\s*&\[(.*?)\]", model, re.S):
        witness_labels += re.findall(r'"([^"]*)"', m.group(1))
    for m in re.finditer(r"branch:\s*\"([^\"]*)\"", model):
        witness_labels.append(m.group(1))
    bad_w = sorted(set(witness_labels) - vlabels)
    check("A3 every label in a witness expectation (branches:/branch:) is a vocabulary label", not bad_w,
          f"{len(set(witness_labels))} distinct; outside {bad_w}")

    # A4: printer closure.
    mm = masked[common.MODEL]
    prints = [(line_of(mm, m.start()), model.splitlines()[line_of(mm, m.start()) - 1].strip())
              for m in re.finditer(r"writeln!\s*\(\s*text\s*,\s*\"", mm)
              if re.match(r'\s*"    \[', model[m.end() - 1:m.end() + 8])]
    allowed = {'let _ = writeln!(text, "    [{mark}] {label}");',
               'let _ = writeln!(text, "    [taken    ] {label} (not in the listed set)");'}
    check("A4 report branch lines are printed only as '[{mark}] {label}' over BRANCHES or '[taken    ] {label} (not in "
          "the listed set)' over taken", prints and all(t in allowed for _, t in prints),
          f"{len(prints)} printing statements: {[l for l, _ in prints]}")
    for ln, t in prints:
        ctx = "\n".join(model.splitlines()[max(0, ln - 9):ln])
        ok = ("for label in BRANCHES" in ctx) if "{mark}" in t else (
            "for label in taken" in ctx and "!BRANCHES.contains(&label.as_str())" in ctx)
        check(f"A4 printer at line {ln} iterates {'BRANCHES' if '{mark}' in t else 'taken minus BRANCHES'}", ok)
    takes = [(line_of(mm, m.start()), re.sub(r"\s+", "", model[m.start():model.index(";", m.start())]))
             for m in re.finditer(r"\btaken\s*\.\s*(extend|insert|append)\s*\(", mm)]
    bad_t = [t for t in takes if not re.match(r"^taken\.extend\((successor|item|observed)\.branches\)$", t[1])]
    check("A4 the explorers' taken sets are extended only from .branches", takes and not bad_t,
          f"{len(takes)} extensions at lines {[l for l, _ in takes]}; others {bad_t}")

    # A5: reachability over model_tests.rs (conservative: any identifier naming a fn is an edge).
    fns_m = functions(mm)
    names = {}
    for name, sig, bo, bc in fns_m:
        names.setdefault(name, []).append((bo, bc))
    edges = {}
    for name, sig, bo, bc in fns_m:
        body_ids = set(re.findall(r"\b([A-Za-z_]\w*)\b", mm[bo:bc + 1]))
        edges.setdefault(name, set()).update(i for i in body_ids if i in names and i != name)
    # Constants are configurations; a const's initialiser can name a fn only through these identifiers.
    reach, todo = set(), [t for t in REQUIRED_TESTS if t in names]
    absent = [t for t in REQUIRED_TESTS if t not in names]
    check("A5 every required test entry point exists in model_tests.rs", not absent, absent)
    while todo:
        n = todo.pop()
        if n in reach:
            continue
        reach.add(n)
        todo.extend(edges.get(n, ()))
    site_reach = {}
    for s in vocab["sites"]:
        site_reach[s["site"]] = s["function"] in reach
        lines.append(f"     site {s['site']} fn {s['function']}: {'REACHABLE' if s['function'] in reach else 'not reachable'}"
                     f" from the required tests")
    shared = {}
    for r in vocab["rows"]:
        if len(r["sites"]) > 1:
            shared[r["label"]] = [s["site"] for s in r["sites"]]
    unresolved_c = {l: s for l, s in shared.items() if sum(site_reach.get(x, True) for x in s) > 1}
    check("A5 a label emitted by two sites is emitted by at most one site any required test reaches", not unresolved_c,
          f"shared labels {shared}; unresolved {unresolved_c}")
    check("A5 admit_by (S11) is not reachable from any required test", "admit_by" not in reach)

    # A6: Debug derived, never hand-written.
    for e in LABEL_ENUMS:
        decl = [(p, m) for p, mt in masked.items() for m in re.finditer(r"\benum\s+" + e + r"\b", mt)]
        derived = False
        if len(decl) == 1:
            p, m = decl[0]
            pre = texts[p][:m.start()].rstrip().splitlines()
            attrs = []
            for l in reversed(pre):
                l = l.strip()
                if l.startswith("#[") or l.startswith("///") or l.startswith("pub"):
                    attrs.append(l)
                    continue
                break
            derived = any(re.search(r"#\[derive\([^)]*\bDebug\b", a) for a in attrs)
        manual = [p for p, mt in masked.items() if re.search(r"\bimpl\b[^{]*\bDebug\s+for\s+" + e + r"\b", mt)]
        if len(decl) != 1 or not derived or manual:
            check(f"A6 enum {e}: declared once, #[derive(..Debug..)], no hand-written Debug", False,
                  f"declarations {len(decl)}, derived {derived}, manual {manual}")
    check(f"A6 all {len(LABEL_ENUMS)} label enums declared once with derived Debug and no hand-written Debug",
          not any(f.startswith("A6 enum") for f in fails))

    # A7: complete label literals.
    ops = sorted({r["operation"] for r in vocab["rows"]}, key=len, reverse=True)
    lits = set()
    for m in re.finditer(r'"((?:[^"\\]|\\.)*)"', model):
        s = m.group(1)
        if "{" in s:
            continue
        if any(s.startswith(o + ": ") for o in ops):
            lits.add(s)
    outside = sorted(lits - vlabels)
    check("A7 every complete label literal in model_tests.rs is a vocabulary label", not outside,
          f"{len(lits)} distinct literals; outside {outside}")

    # A8.
    bad_rows = [r["id"] for r in vocab["rows"] if not r["sites"] or not r["label"].startswith(r["operation"] + ": ")]
    check("A8 every vocabulary row names a site and starts with its operation", not bad_rows, bad_rows)

    lines.append(f"CHECK A ({CHECKER_VERSION}): {'PASS' if not fails else 'FAIL: ' + '; '.join(fails)}")
    print("\n".join(lines))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
