#!/usr/bin/env python3
"""Checker B — an independent lexical and control-flow analysis (mission
section 20, Checker B).

Shares no code with Checker A. It masks comments and literals with its own
character state machine, finds items with regular expressions over the
masked text, scans the same target occurrences (definition: NAMING.md),
derives read/write status on its own, and checks the source facts the proof
rests on: the projection (key layout, actor order, initial values), the
REPLAY step's exact form, the stale-token paths of the three worker
operations, the publication guard, the X cell's writers, and the absence of
replay values from every property, terminality and branch function.

  checker_b.py OUTDIR [--override REL=FILE ...]

Exit status 0 only if every check holds.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
COMMIT = "d5964ee5326c2733177307db6f507328023d361a"
PREFIX = "crates/nexus-governed-control/src/"
PINNED = {
    "ledger/protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
    "ledger/protocol/model_tests.rs": "28085ce232827301104b5d757af042ec933976f8",
    "ledger/protocol/unit_tests.rs": "55dc66409eaccdaa93e8f0a56f2ccb14be81e114",
}
MT = "ledger/protocol/model_tests.rs"
PR = "ledger/protocol.rs"


def sh(*a):
    return subprocess.run(["git", "-C", REPO, *a], check=True, capture_output=True).stdout


def load(overrides):
    files = {}
    for line in sh("ls-tree", "-r", COMMIT, PREFIX).decode().splitlines():
        meta, path = line.split("\t", 1)
        if not path.endswith(".rs"):
            continue
        blob = meta.split()[2]
        rel = path[len(PREFIX):]
        if rel in overrides:
            raw = open(overrides[rel], "rb").read()
        else:
            raw = sh("cat-file", "blob", blob)
            if rel in PINNED and PINNED[rel] != blob:
                raise SystemExit(f"STOP: {rel}: {blob} is not the pinned blob")
        files[rel] = (blob, raw.decode("utf-8"))
    return files


# ---- masking --------------------------------------------------------------------------------

def mask(text):
    """(masked, region): comments and literals blanked (newlines kept);
    region[i] in 'c' code, 'm' comment, 's' literal."""
    out = list(text)
    reg = ["c"] * len(text)
    n = len(text)
    i = 0

    def blank(a, b, r):
        for k in range(a, b):
            reg[k] = r
            if out[k] != "\n":
                out[k] = " "

    def ident_char(ch):
        return ch.isalnum() or ch == "_"

    while i < n:
        ch = text[i]
        two = text[i:i + 2]
        if two == "//":
            j = text.find("\n", i)
            j = n if j == -1 else j
            blank(i, j, "m")
            i = j
            continue
        if two == "/*":
            d, j = 0, i
            while j < n:
                if text[j:j + 2] == "/*":
                    d += 1
                    j += 2
                elif text[j:j + 2] == "*/":
                    d -= 1
                    j += 2
                    if d == 0:
                        break
                else:
                    j += 1
            blank(i, j, "m")
            i = j
            continue
        prev_ok = i == 0 or not ident_char(text[i - 1])
        if prev_ok and (ch == "r" or text[i:i + 2] == "br"):
            k = i + (2 if text[i:i + 2] == "br" else 1)
            h = 0
            while k < n and text[k] == "#":
                h += 1
                k += 1
            if k < n and text[k] == '"' and (h > 0 or k == i + 1 or k == i + 2):
                end = text.find('"' + "#" * h, k + 1)
                if end != -1:
                    blank(i, end + 1 + h, "s")
                    i = end + 1 + h
                    continue
        if ch == '"' or (prev_ok and text[i:i + 2] == 'b"'):
            j = i + (2 if ch == "b" else 1)
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    break
                j += 1
            blank(i, j + 1, "s")
            i = j + 1
            continue
        if ch == "'" or (prev_ok and text[i:i + 2] == "b'"):
            q = i + (1 if ch == "b" else 0)
            if text[q + 1] == "\\":
                j = q + 2
                if text[j] == "u":
                    j = text.index("}", j)
                j += 1
                if text[j] == "'":
                    blank(i, j + 1, "s")
                    i = j + 1
                    continue
            elif q + 2 < n and text[q + 2] == "'":
                blank(i, q + 3, "s")
                i = q + 3
                continue
            # a lifetime: code
            i += 1
            continue
        i += 1
    return "".join(out), reg


# ---- items ----------------------------------------------------------------------------------

def bracket_map(m):
    pairs = {}
    stack = []
    for i, ch in enumerate(m):
        if ch in "([{":
            stack.append(i)
        elif ch in ")]}":
            o = stack.pop()
            if {"(": ")", "[": "]", "{": "}"}[m[o]] != ch:
                raise SystemExit(f"bracket mismatch at {i}")
            pairs[o] = i
            pairs[i] = o
    if stack:
        raise SystemExit("unbalanced brackets")
    return pairs


HEADER = re.compile(
    r"\b(?:(fn)\s+([A-Za-z_]\w*)|(struct|enum|trait|union)\s+([A-Za-z_]\w*)|(?<!')(const|static)\s+(?:mut\s+)?([A-Za-z_]\w*)\s*:"
    r"|(type)\s+([A-Za-z_]\w*)\s*[=<]|(mod)\s+([A-Za-z_]\w*)(?=\s*\{)|(impl)\b|(macro_rules)!\s*([A-Za-z_]\w*))")


class It:
    def __init__(self, kind, raw, hstart, start, end):
        self.kind, self.raw, self.hstart, self.start, self.end = kind, raw, hstart, start, end
        self.name = None
        self.body = None


def leading(text, m, reg, pairs, pos):
    """Start of an item whose qualifiers begin at `pos`."""
    # qualifiers
    q = pos
    while True:
        k = q - 1
        while k >= 0 and m[k] in " \t\r\n":
            k -= 1
        mm = re.search(r"(pub\s*\([^()]*\)|pub|const|unsafe|async|extern)\s*$", m[:k + 1])
        if mm and mm.end() == k + 1 and (mm.start() == 0 or not (m[mm.start() - 1].isalnum() or m[mm.start() - 1] == "_")):
            q = mm.start()
            continue
        break
    start = q
    while True:
        k = start - 1
        while k >= 0 and text[k] in " \t\r\n":
            k -= 1
        if k < 0:
            break
        if reg[k] == "m":
            a = k
            while a > 0 and reg[a - 1] == "m" and text[a - 1] != "\n":
                a -= 1
            ctext = text[a:k + 1]
            if (ctext.startswith("///") and not ctext.startswith("////")) or ctext.startswith("/**"):
                start = a
                continue
            break
        if m[k] == "]":
            o = pairs[k]
            p = o - 1
            if p >= 0 and m[p] == "#":
                start = p
                continue
            break
        break
    return start


def find_items(text):
    m, reg = mask(text)
    pairs = bracket_map(m)
    items = []
    for h in HEADER.finditer(m):
        g = h.groups()
        if g[0]:
            kind, nm = "fn", g[1]
        elif g[2]:
            kind, nm = g[2], g[3]
        elif g[4]:
            kind, nm = g[4], g[5]
        elif g[6]:
            kind, nm = "type", g[7]
        elif g[8]:
            kind, nm = "mod", g[9]
        elif g[10]:
            kind, nm = "impl", None
        else:
            kind, nm = "macro_rules!", g[12]
        hs = h.start()
        # impl in type position is not an item
        if kind == "impl":
            k = hs - 1
            while k >= 0 and m[k] in " \t\r\n":
                k -= 1
            if not (k < 0 or m[k] in "};{]" or m[k - 5:k + 1] in ("unsafe",)):
                continue
        # find body
        k = h.end()
        body_open = None
        end = None
        depth = 0
        while k < len(m):
            ch = m[k]
            if ch in "([":
                k = pairs[k] + 1
                continue
            if ch == "{":
                if kind in ("const", "static", "type"):
                    k = pairs[k] + 1
                    continue
                body_open, end = k, pairs[k] + 1
                break
            if ch == ";":
                end = k + 1
                break
            if kind == "impl" and ch in ",)":
                end = None
                break
            k += 1
        if end is None:
            continue
        start = leading(text, m, reg, pairs, hs)
        it = It(kind, nm, hs, start, end)
        it.body = (body_open, end) if body_open is not None else None
        if kind == "impl":
            hdr = m[h.end():body_open]
            it.raw = hdr
        items.append(it)
    # names, by containment
    containers = [i for i in items if i.kind in ("fn", "impl", "mod", "trait") and i.body]

    def parent_of(it):
        best = None
        for c in containers:
            if c is it:
                continue
            if c.body[0] < it.hstart < c.body[1]:
                if best is None or c.body[0] > best.body[0]:
                    best = c
        return best

    def impl_name(hdr):
        s = hdr.strip()
        if s.startswith("<"):
            d = 0
            for x, ch in enumerate(s):
                d += ch == "<"
                d -= ch == ">"
                if d == 0:
                    s = s[x + 1:]
                    break
        parts = re.split(r"\bfor\b", s)

        def tname(part):
            d, last = 0, None
            for tok in re.finditer(r"[A-Za-z_]\w*|<|>", part):
                t = tok.group(0)
                if t == "<":
                    d += 1
                elif t == ">":
                    d -= 1
                elif d == 0 and t not in ("mut", "dyn"):
                    last = t
            return last
        if len(parts) == 2:
            return f"<{tname(parts[1])} as {tname(parts[0])}>"
        return tname(parts[0])

    def name(it):
        if it.name is not None:
            return it.name
        p = parent_of(it)
        if it.kind == "impl":
            base = impl_name(it.raw)
            it.name = f"{name(p)}::{base}" if p is not None and p.kind == "mod" else base
            return it.name
        if it.kind == "mod":
            it.name = it.raw if p is None or p.kind != "mod" else f"{name(p)}::{it.raw}"
            return it.name
        own = it.raw if it.kind == "fn" else f"{it.kind} {it.raw}"
        if p is None:
            it.name = own
        elif p.kind == "impl" or p.kind == "trait":
            it.name = f"{name(p)}::{it.raw}"
        elif p.kind == "fn":
            it.name = f"{name(p)}/{own}"
        else:
            it.name = f"{name(p)}::{own}"
        return it.name

    for it in items:
        name(it)
    return items, m, reg


def enclosing(items, off):
    best = None
    for it in items:
        if it.start <= off < it.end and (best is None or it.end - it.start < best.end - best.start):
            best = it
    return best.name if best else "<module>"


# ---- occurrences ----------------------------------------------------------------------------

EXACT = {"replays", "Replay", "step_replay", "last"}
RUN = re.compile(r"[A-Za-z0-9_]*replay[A-Za-z0-9_]*", re.I)
ACTION = re.compile(r"\bAction\s*::\s*(Nothing|Barrier|Resolve|AppendC|AppendX|EmptyX)\b")


def line_col(text, off):
    line = text.count("\n", 0, off) + 1
    col = off - (text.rfind("\n", 0, off) + 1) + 1
    return line, col


def occurrences(rel, text, items, m, reg):
    occ = []

    def add(token, region, group, off):
        ln, col = line_col(text, off)
        occ.append({"file": rel, "line": ln, "col": col, "token": token, "region": region, "group": group,
                    "item": enclosing(items, off), "off": off})

    for t in re.finditer(r"[A-Za-z_][A-Za-z0-9_]*", m):
        w = t.group(0)
        s = t.start()
        if s > 0 and (m[s - 1].isalnum() or m[s - 1] == "_"):
            continue
        if w in EXACT:
            add(w, "code", "exact", s)
            continue
        if w == "name":
            k = s - 1
            while k >= 0 and m[k] in " \t\r\n":
                k -= 1
            if k >= 0 and m[k] == ".":
                add(".name", "code", "name", s)
                continue
        if w == "Action":
            a = ACTION.match(m, s)
            if a:
                add("Action::" + a.group(1), "code", "action", s)
                continue
        if RUN.fullmatch(w):
            add(w, "code", "lexical", s)
    # comment and literal runs
    k = 0
    n = len(text)
    while k < n:
        r = reg[k]
        if r == "c":
            k += 1
            continue
        j = k
        while j < n and reg[j] == r and not (r == "m" and text[j] == "\n"):
            j += 1
        seg = text[k:j]
        for x in RUN.finditer(seg):
            add(x.group(0), "comment" if r == "m" else "string", "lexical", k + x.start())
        k = j if j > k else k + 1
    occ.sort(key=lambda o: o["off"])
    return occ


def rw_of(o, text, m, items):
    """B's own read/write derivation for `replays`, `last` and `.name`."""
    s = o["off"]
    tok = o["token"]
    word = "name" if tok == ".name" else tok
    e = s + len(word)
    before = m[:s].rstrip()
    after = m[e:].lstrip()
    item = o["item"]
    if before.endswith("."):
        if re.match(r"(-=|\+=|=(?!=))", after):
            return "WRITE"
        return "READ"
    if item.startswith("struct ") and re.match(r":", after):
        return "DECL"
    if re.match(r":\s*_\b", after):
        return "PATTERN"
    if re.search(r"\blet\s+(mut\s+)?$", before) or re.search(r"\blet\s+Some\(\s*$", before):
        return "WRITE"
    if re.match(r"=(?!=)", after) and not before.endswith("&"):
        return "WRITE"
    if before.endswith("&"):
        return "READ"
    if re.match(r":", after):
        return "WRITE"  # a field initialised in a struct literal
    if re.match(r"[,}]", after):
        # shorthand only inside a struct literal: innermost '{' preceded by a path
        depth = 0
        k = s - 1
        while k >= 0:
            ch = m[k]
            if ch in ")]}":
                depth += 1
            elif ch in "([{":
                if depth == 0:
                    if ch == "{":
                        pre = m[:k].rstrip()
                        return "WRITE" if re.search(r"[A-Za-z_]\w*$", pre) else "READ"
                    return "READ"
                depth -= 1
            k -= 1
        return "READ"
    return "READ"


# ---- shape helpers --------------------------------------------------------------------------

def norm(s):
    s = re.sub(r"\s+", " ", s)
    s = re.sub(r" ?([(){}\[\],;.&!|<>=+*:-]) ?", r"\1", s)
    return s.strip()


def body_of(items, m, name):
    hits = [i for i in items if i.name == name]
    if len(hits) != 1:
        return None
    it = hits[0]
    if it.body is None:
        return None
    return m[it.body[0] + 1: it.body[1] - 1]


def item_text(items, m, name):
    hits = [i for i in items if i.name == name]
    return m[hits[0].start:hits[0].end] if len(hits) == 1 else None


class Report:
    def __init__(self):
        self.lines = []
        self.fail = 0

    def check(self, cid, ok, what):
        self.lines.append(f"{'PASS' if ok else 'FAIL'}  {cid}  {what}")
        if not ok:
            self.fail += 1
        return ok


def call_closure(items, m, rel_of, roots):
    ledger = ("ledger/protocol.rs", "ledger/protocol/model_tests.rs", "ledger/protocol/unit_tests.rs")
    fns = [(rel, it) for rel, its in items.items() if rel in ledger for it in its if it.kind == "fn" and it.body]
    by_leaf = {}
    for rel, it in fns:
        leaf = re.split(r"::|/", it.name)[-1]
        by_leaf.setdefault(leaf, []).append((rel, it))
    seen = set()
    work = [(rel, it) for rel, it in fns if it.name in roots and rel == MT]
    while work:
        rel, it = work.pop()
        key = (rel, it.name)
        if key in seen:
            continue
        seen.add(key)
        body = m[rel][it.body[0]:it.body[1]]
        refs = set(re.findall(r"\b([A-Za-z_]\w*)\s*(?:\(|::\s*<)", body)) | set(re.findall(r"::\s*([A-Za-z_]\w*)", body))
        for r in refs:
            for cand in by_leaf.get(r, []):
                if (cand[0], cand[1].name) not in seen:
                    work.append(cand)
    return seen


def cite_mode(docs, overrides):
    """B's own resolution of every citation: item span, lines and hash from
    B's item finder (not A's)."""
    files = load(overrides)
    short = {"protocol.rs": PR, "model_tests.rs": MT, "unit_tests.rs": "ledger/protocol/unit_tests.rs"}
    index = {}
    for sh_, rel in short.items():
        blob, text = files[rel]
        its, m, reg = find_items(text)
        for it in its:
            l1 = text.count("\n", 0, it.start) + 1
            l2 = text.count("\n", 0, it.end - 1) + 1
            h = hashlib.sha256(text[it.start:it.end].encode("utf-8")).hexdigest()
            index[(sh_, it.name)] = (l1, l2, blob, h)
    pat = re.compile(r"\[(protocol\.rs|model_tests\.rs|unit_tests\.rs) · ([^·\]]+?) · L(\d+)(?:–(\d+))? · blob ([0-9a-f]{8}) · item ([0-9a-f]{16})\]")
    total, bad = 0, []
    for doc in docs:
        for mm_ in pat.finditer(open(doc, encoding="utf-8").read()):
            total += 1
            sh_, item, a, b, blob8, h16 = mm_.groups()
            a, b = int(a), int(b or a)
            rec = index.get((sh_, item.strip()))
            if rec is None or not (rec[0] <= a <= b <= rec[1]) or not rec[2].startswith(blob8) or not rec[3].startswith(h16):
                bad.append(f"{os.path.basename(doc)}: {mm_.group(0)} -> {rec}")
    print(f"CHECKER B citations: {total} citations in {len(docs)} documents; {len(bad)} unresolved")
    for x in bad:
        print("  PROBLEM:", x)
    return 0 if total and not bad else 1


def main(argv):
    if argv[1] == "cite":
        ov = {}
        docs = []
        k = 2
        while k < len(argv):
            if argv[k] == "--override":
                a_, b_ = argv[k + 1].split("=", 1)
                ov[a_] = b_
                k += 2
            else:
                docs.append(argv[k])
                k += 1
        return cite_mode(docs, ov)
    out = argv[1]
    overrides = {}
    k = 2
    while k < len(argv):
        if argv[k] == "--override":
            a, b = argv[k + 1].split("=", 1)
            overrides[a] = b
            k += 2
        else:
            k += 1
    files = load(overrides)
    rep = Report()
    allitems, masks, regs, allocc = {}, {}, {}, []
    for rel, (blob, text) in sorted(files.items()):
        its, m, reg = find_items(text)
        allitems[rel], masks[rel], regs[rel] = its, m, reg
        allocc.extend(occurrences(rel, text, its, m, reg))
    rep.check("B0", len(files) == 49, f"49 Rust inputs read from git objects of {COMMIT[:12]} ({len(files)})")
    # ordinals and the declared classification (independent matching)
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import occurrence_spec as spec
    cnt = {}
    for o in allocc:
        key = (o["file"], o["item"], o["region"], o["token"])
        cnt[key] = cnt.get(key, 0) + 1
        o["ordinal"] = cnt[key]
    hits = {}
    unclassified = []
    rw_disagree = []
    for o in allocc:
        # B derives read/write for every code occurrence of replays, last and .name in
        # model_tests.rs, classified or not, so that B87/B88 see unclassified writes too
        if o["file"] == MT and o["region"] == "code" and o["token"] in ("replays", "last", ".name"):
            o["rw_b"] = rw_of(o, files[MT][1], masks[MT], allitems[MT])
    for o in allocc:
        rs = [r for r in spec.RULES if r[0] == o["file"] and r[1] == o["item"] and r[2] == o["region"]
              and r[3] == o["token"] and (r[4] == "*" or r[4] == o["ordinal"])]
        if len(rs) != 1:
            unclassified.append(o)
            continue
        hits[rs[0]] = hits.get(rs[0], 0) + 1
        o["status"], o["rw_spec"] = rs[0][6], rs[0][7]
        if o["file"] == MT and o["region"] == "code" and o["token"] in ("replays", "last", ".name") and rs[0][7] != "NONE":
            mine = o["rw_b"]
            if mine != rs[0][7]:
                rw_disagree.append(f"{o['line']}:{o['col']} {o['token']} [{o['item']}] spec {rs[0][7]} B {mine}")
    bad_counts = [r for r in spec.RULES if hits.get(r, 0) != r[5]]
    rep.check("B1", not unclassified, f"every target occurrence classified by exactly one rule ({len(allocc)} occurrences; "
              f"unclassified: {[(o['file'], o['line'], o['token'], o['item']) for o in unclassified]})")
    rep.check("B2", not bad_counts, f"every rule matches exactly its count ({len(spec.RULES)} rules; mismatched: "
              f"{[(r[0], r[1], r[3], r[4], r[5], hits.get(r, 0)) for r in bad_counts]})")
    rep.check("B3", not rw_disagree, f"B's read/write derivation agrees with the declared status for every "
              f"replays/last/.name code occurrence in model_tests.rs ({rw_disagree})")

    mt, pr = masks[MT], masks[PR]
    mti, pri = allitems[MT], allitems[PR]
    N = lambda name, items=mti, mm=mt: norm(body_of(items, mm, name) or "")  # noqa: E731

    # ---- the projection: actor list, initial values, key layout ----
    rep.check("B10", N("actors") == norm("""let writers = if config.batch { 1 } else { config.facts };
        let mut all: Vec<Actor> = (0..writers).map(Actor::Fw).collect(); all.push(Actor::Se); all.push(Actor::Wk);
        all.extend((0..config.gating).map(Actor::Gc)); all.extend((0..config.acks).map(Actor::Ack)); all.push(Actor::Adm);
        if config.env { all.push(Actor::Env); } if config.replays > 0 { all.push(Actor::Replay); } all"""),
              "actors(): Replay is pushed last, after Adm and Env, only when config.replays > 0 (FR-6)")
    enum_actor = norm(body_of(mti, mt, "enum Actor") or "")
    rep.check("B11", enum_actor == norm("Fw(u8), Se, Wk, Gc(u8), Ack(u8), Adm, Env, Replay,"),
              "enum Actor: Replay is the last variant (FR-6)")
    ini = N("initial")
    rep.check("B12", "wk:Wk{worker,pos:WkPos::Idle,last:None,}" in ini and "replays:config.replays," in ini,
              "initial(): Wk.last = None and World.replays = config.replays (FR-1)")
    enc = N("encode")
    seg_enc = re.search(r"match&w\.wk\.pos\{.*?\}\}match&w\.wk\.last\{None=>e\.byte\(0\),Some\(action\)=>\{e\.byte\(1\);"
                        r"e\.action\(shared,action\);\}\}for gc in&w\.gc\{", enc)
    rep.check("B13", seg_enc is not None, "encode(): segment L (wk.last: 0, or 1 then the action) directly after wk.pos and "
              "before the gating positions (PROJ)")
    rep.check("B14", "e.b(w.env_done);e.byte(w.replays);//" in norm(item_text(mti, files[MT][1], "encode") or "").replace(" ", "") or
              "e.b(w.env_done);e.byte(w.replays);for class in w.table.class{" in enc,
              "encode(): byte r (w.replays) directly after env_done and before the table (PROJ)")
    dec = N("decode")
    rep.check("B15", re.search(r"let wk_pos=match d\.byte\(\)\{.*?\};let last=d\.b\(\)\.then\(\|\|d\.action\(&shared\)\);let gc=", dec)
              is not None, "decode(): segment L read directly after wk_pos and before gc (PROJ)")
    rep.check("B16", "let env_done=d.b();let replays=d.byte();let class=" in dec,
              "decode(): byte r read directly after env_done and before the classes (PROJ)")
    rep.check("B17", "wk:Wk{worker,pos:wk_pos,last,}" in dec and "replays,table," in dec,
              "decode(): the world is built with exactly the decoded last and replays (PROJ)")
    rep.check("B18", "assert_eq!(d.at,key.len()," in dec, "decode(): the key is read whole (PROJ)")
    ea = N("Enc::action")
    da = N("Dec::action")
    rep.check("B19", ea == norm("""match action { Action::Nothing => self.byte(0), Action::Barrier => self.byte(1),
        Action::Resolve(token) => { self.byte(2); self.token(shared, token); } Action::AppendC(token) => { self.byte(3); self.token(shared, token); }
        Action::AppendX(token) => { self.byte(4); self.token(shared, token); } Action::EmptyX(token, seq) => { self.byte(5); self.token(shared, token); self.u(*seq); } }""")
              and da == norm("""match self.byte() { 0 => Action::Nothing, 1 => Action::Barrier, 2 => Action::Resolve(self.token(shared)),
        3 => Action::AppendC(self.token(shared)), 4 => Action::AppendX(self.token(shared)), _ => { let token = self.token(shared); Action::EmptyX(token, self.u()) } }"""),
              "the action codec mirrors: tags 0..5, token, then the sequence for EmptyX (PROJ, KEY)")
    et, dt = N("Enc::token"), N("Dec::token")
    rep.check("B20", et == norm('assert!( Arc::ptr_eq(&token.ledger, shared), ); self.byte(cell_code(token.cell)); self.u(token.occupancy);')
              and dt == norm("let cell = [Cell::U, Cell::C, Cell::X][self.byte() as usize]; CellToken { ledger: Arc::clone(shared), cell, occupancy: self.u(), }"),
              "the token codec mirrors (cell code, occupancy) and a decoded token refers to the world's own allocation (INV-T)")
    rep.check("B21", N("cell_code") == norm("match cell { Cell::U => 0, Cell::C => 1, Cell::X => 2, }"),
              "cell codes U 0, C 1, X 2 match Dec::token's table (KEY)")
    # ---- step_replay's exact form ----
    rep.check("B30", N("step_replay") == norm("""if w.replays == 0 || !matches!(w.wk.pos, WkPos::Idle) { return false; }
        let Some(last) = w.wk.last.clone() else { return false; };
        let token = match &last { Action::Resolve(t) | Action::AppendC(t) | Action::AppendX(t) | Action::EmptyX(t, _) => t, _ => return false, };
        if present_token(&w.core, token) { return false; } w.replays -= 1; obs.note(|| .into());
        worker_op(config, w, &last, o, obs); true"""),
              "step_replay: guards (replays, Idle, last, a token action, stale), decrement by one, the stale operation; "
              "no other effect, no Oracle use, no write of Wk.last (FR-RS)")
    rep.check("B31", N("present_token") == norm("Arc::ptr_eq(&token.ledger, &core.shared) && core.occupancy(token.cell) == Some(token.occupancy)"),
              "present_token: same allocation and the cell's present occupancy (FR-RS)")
    rep.check("B32", "worker_op(config,w,&action,o,obs);if config.replays>0&&!matches!(action,Action::Barrier|Action::Nothing){w.wk.last=Some(action);}w.wk.pos=WkPos::Idle;"
              in N("step_wk"), "step_wk: Wk.last is saved only after worker_op, only when config.replays > 0, and never for Barrier or Nothing (FR-4, INV-AC)")
    al = N("allowed")
    rep.check("B33", "steps.push(Action::Resolve(u.clone()));" in al and "if let Some(u)=&agenda.u{" in al
              and "if let Some(c)=&agenda.c{steps.push(Action::AppendC(c.clone()));}" in al
              and "if let Some((x,Stage::Built))=&agenda.x{steps.push(Action::AppendX(x.clone()));}" in al
              and "if let Some((x,Stage::Committed(seq)))=&agenda.x{steps.push(Action::EmptyX(x.clone(),*seq));}" in al,
              "allowed(): Resolve takes the U token, AppendC the C token, AppendX and EmptyX the X token (INV-AC)")
    ag = norm(body_of(pri, pr, "Core::agenda") or "")
    rep.check("B34", "u:self.u.as_ref().map(|(n,_)|self.token(Cell::U,*n))," in ag and "c:self.c.as_ref().map(|(n,_)|self.token(Cell::C,*n))," in ag
              and "map(|(n,_,stage)|(self.token(Cell::X,*n),*stage))" in ag,
              "Core::agenda: each token names its own cell at its occupancy (INV-AC)")
    # ---- worker_op on a stale token ----
    wo = N("worker_op")
    rep.check("B40", "let stale=token.as_ref().is_some_and(|t|!present_token(&w.core,t));" in wo
              and "let Some(worker)=w.wk.worker.as_mut()else{return;};" in wo
              and "let mut io=Io::new(&mut w.rows,o,config.worker_unwinds,config.cell_constraint,);" in wo,
              "worker_op: stale computed from present_token; the worker taken by reference; Io built without a choice (FR-RS)")
    rep.check("B41", "if let Some((before,ledger))=before_core{if!stale_answer||core_key(&w.core)!=before||Arc::as_ptr(&w.core.shared)!=ledger||word_of(&w.core.shared)!=before_word{obs.fail(\"E8\"" in wo.replace(" ", "")
              or "if let Some((before,ledger))=before_core{if!stale_answer||core_key(&w.core)!=before||Arc::as_ptr(&w.core.shared)!=ledger||word_of(&w.core.shared)!=before_word{obs.fail(" in wo,
              "worker_op: E8 is checked for every stale token: stale answer, core key, allocation and word unchanged (FR-10)")
    rep.check("B42", 'label.ends_with("NotWritten")||label.ends_with("Stale")||label.contains("Refused");'.replace('"NotWritten"', '').replace('"Stale"', '').replace('"Refused"', '') in wo.replace('"', '')
              or "let unchanged=label.ends_with()||label.ends_with()||label.contains();if unchanged&&w.rows!=before_rows{obs.fail(" in wo,
              "worker_op: E4 checks that a Stale answer leaves the simulated ledger unchanged (FR-10)")
    rep.check("B43", "if!stale{if let Err(order)=verb_order(&log,1){" in wo,
              "worker_op: E5's verb order is not checked for a stale cell append (no verb is issued) (FR-10)")
    rep.check("B44", "obs.store_ops+=1;obs.branch(label.clone());" in wo,
              "worker_op: one store operation and exactly one branch label for every operation that reaches the core (FR-17)")
    # the worker is never replaced
    rep.check("B45", re.search(r"wk\.worker\s*=(?!=)", files[MT][1]) is None and re.search(r"\.wk\.worker\s*=(?!=)", mt) is None,
              "no step assigns Wk.worker; a decoded world's worker refers to its own allocation (INV-W)")
    # ---- protocol.rs: the stale returns precede every verb and every mutation ----
    P = lambda name: norm(body_of(pri, pr, name) or "")  # noqa: E731
    rep.check("B50", P("Publish::resolve").startswith(norm("if !self.worker_belongs(worker) { return Resolve::OtherLedger; } let core = &mut *self.core; if !core.names(token, Cell::U) { return Resolve::Stale; }")),
              "Publish::resolve: OtherLedger, then Stale when the token does not name U, before any read or mutation (FR-RS)")
    rep.check("B51", P("Publish::cell").startswith(norm("if !self.worker_belongs(worker) { return CellAppend::OtherLedger; } let core = &mut *self.core; if !(core.names(token, Cell::C) || core.names(token, Cell::X)) { return CellAppend::Stale; }")),
              "Publish::cell: OtherLedger, then Stale when the token names neither C nor X, before io.open or any mutation (FR-RS)")
    rep.check("B52", P("Publish::empty").startswith(norm("if !self.worker_belongs(worker) { return Emptying::OtherLedger; } if !self.core.names(token, Cell::X) { return Emptying::Stale; }")),
              "Publish::empty: OtherLedger, then Stale when the token does not name X, before X is touched (FR-RS)")
    for cid, nm, op in (("B53", "Core::resolve", "op.resolve(worker, &token, io)"), ("B54", "Core::append_cell", "op.cell(worker, &token, io)"),
                        ("B55", "Core::empty", "op.empty(worker, &token)")):
        rep.check(cid, P(nm) == norm(f"let mut op = Publish::new(self); let outcome = {op}; op.done = true; outcome"),
                  f"{nm}: guard, operation, done = true, return (FR-RS)")
    rep.check("B56", P("Publish::new") == norm("let guess = core.flags(); Publish { core, guess, ticket: None, count: None, issued: false, conflict: None, hold: Hold::Nothing, keep: false, done: false, }"),
              "Publish::new: reads core.flags() only; no ticket, nothing held (FR-RS)")
    rep.check("B57", P("Core::names") == norm("Arc::ptr_eq(&token.ledger, &self.shared) && token.cell == cell && self.occupancy(cell) == Some(token.occupancy)"),
              "Core::names: same allocation, same cell, present occupancy (FR-RS)")
    rep.check("B58", P("Publish::worker_belongs") == norm("Arc::ptr_eq(&worker.ledger, &self.core.shared)"),
              "Publish::worker_belongs: the worker's allocation is the core's (FR-RS, INV-W)")
    dr = P("<Publish as Drop>::drop")
    rep.check("B59", dr.startswith("if!self.done{") and "if self.hold!=Hold::Nothing{" in dr
              and "let flags=self.core.flags();let counted=self.ticket.is_some()&&self.count.is_some();" in dr
              and "exchange(&self.core.shared,self.guess,|word|{let carried=(word&!FLAGS)|(word&HANDOVER_FAULT)|flags;if counted{count_step(carried,UNAPPLIED_SHIFT,true)}else{carried}});" in dr
              and dr.endswith("if let(Some(ticket),Some(counted))=(self.ticket.as_mut(),self.count){ticket.state=TicketState::Counted(counted);}"),
              "Drop for Publish: with done, nothing held and no ticket, it makes one exchange publishing (word & !FLAGS) | (word & HANDOVER_FAULT) | core.flags() (FR-RS)")
    fl = P("Core::flags")
    rep.check("B60", re.search(r"self\.\w+\s*=(?!=)", fl) is None and "|bit(self.latch.is_some(),LATCHED)" in fl,
              "Core::flags: pure; phase, session, latch and cell bits (FR-RS)")
    consts = {c: re.search(rf"const {c}: u64 = (0x[0-9A-Fa-f]+);", files[PR][1]) for c in
              ("PHASE_NEED_BARRIER", "PHASE_RESOLVING", "SESSION_OPEN", "SESSION_SEALED", "LATCHED", "U_OCCUPIED", "C_OCCUPIED", "X_OCCUPIED", "HANDOVER_FAULT", "FLAGS")}
    vals = {c: int(v.group(1), 16) if v else None for c, v in consts.items()}
    rep.check("B61", vals == {"PHASE_NEED_BARRIER": 1, "PHASE_RESOLVING": 2, "SESSION_OPEN": 4, "SESSION_SEALED": 8, "LATCHED": 0x10,
                              "U_OCCUPIED": 0x20, "C_OCCUPIED": 0x40, "X_OCCUPIED": 0x80, "HANDOVER_FAULT": 0x100, "FLAGS": 0xFFFF},
              f"word constants: core.flags() and the test's expected_flags() use the same bits; HANDOVER_FAULT inside FLAGS ({vals})")
    ef = N("expected_flags")
    rep.check("B62", "Phase::NeedBarrier=>0x1,Phase::Resolving=>0x2," in ef and "Session::Open=>0x4,Session::Sealed=>0x8," in ef
              and "(core.latch.is_some(),0x10),(core.u.is_some(),0x20),(core.c.is_some(),0x40),(core.x.is_some(),0x80)," in ef,
              "expected_flags (E1) equals core.flags() bit for bit, so E1 at a state makes the stale publication rewrite the same word (FR-RS)")
    sp = N("state_properties")
    rep.check("B63", "if word&FLAGS&!HANDOVER_FAULT!=expected_flags(&w.core){" in sp,
              "E1: the word's flag bits other than the handover fault equal expected_flags(core) (FR-RS)")
    ex = P("exchange")
    rep.check("B64", "let mut present=guess;loop{let next=step(present);match shared.word.compare_exchange(present,next,Ordering::SeqCst,Ordering::SeqCst){" in ex,
              "exchange: one compare-and-exchange loop; a single thread's second attempt succeeds (FR-RS, E2)")
    # ---- the X cell's writers (FR-17.X) ----
    xw = []
    for mm_ in re.finditer(r"(?:\bself\.core\.x|\bself\.x|\bcore\.x)\s*(=(?!=)|\.as_mut\(\)|\.take\(\)|\.replace\()", pr):
        xw.append((enclosing(pri, mm_.start()), norm(pr[mm_.start():pr.find(";", mm_.start()) + 1])[:60]))
    xw_lit = [(enclosing(pri, mm_.start())) for mm_ in re.finditer(r"\bx:\s*", pr)]
    rep.check("B70", sorted(x[0] for x in xw) == ["Core::place", "Publish::empty", "Publish::settle"],
              f"protocol.rs: X is written only by Core::place (a new occupancy), Publish::settle (its stage) and Publish::empty (None) ({xw})")
    rep.check("B71", "Cell::X=>self.x=Some((occupancy,occupant,Stage::Built))," in P("Core::place")
              and P("Core::place").startswith("let Some(occupancy)=self.next_occupancy(cell)else{return false;};"),
              "Core::place: X gets a fresh occupancy number, Built (FR-17.X)")
    st = P("Publish::settle")
    rep.check("B72", "if let Some((_,occupant,stage))=self.core.x.as_mut(){*stage=Stage::Committed(seq);" in st,
              "Publish::settle: X's stage changes; its occupancy number does not (FR-17.X)")
    callers = [enclosing(pri, mm_.start()) for mm_ in re.finditer(r"\.explanation\(|\bself\.place\(Cell::X|\.place\(Cell::X", pr)]
    rep.check("B73", sorted(callers) == ["Core::classify", "Core::classify", "Core::explanation", "Publish::explain", "Publish::resolve_gating"],
              f"X is filled only through Core::explanation, called from Core::classify (2), Publish::explain and Publish::resolve_gating ({sorted(callers)})")
    cl = P("Core::classify")
    rep.check("B74", cl.count("if self.x.is_none(){self.explanation(member.key,cause);}") == 2,
              "Core::classify: both fills are guarded by X empty (FR-17.X)")
    exb = P("Publish::explain")
    rep.check("B75", "Some(_)=>Explain::Busy,None=>match core.refusals.get(key).copied(){Some(cause)=>{if core.explanation(*key,cause){" in exb,
              "Publish::explain: fills X only in the arm where X is empty (FR-17.X)")
    rr = P("Publish::resolve")
    rep.check("B76", "let room=core.c.is_none()&&core.x.is_none();" in rr and "Some(Occupant::Gating{record,exit})if room&&exit!=Exit::InCall=>{self.resolve_gating(&record,exit,io)}" in rr
              and len(re.findall(r"(?<!fn )\bresolve_gating\(", pr)) == 1,
              "Publish::resolve_gating runs only from Publish::resolve with room (C and X empty) (FR-17.X)")
    rep.check("B77", re.findall(r"\bcore\.empty\(", mt) and all(enclosing(mti, x.start()) in ("worker_op",) for x in re.finditer(r"\.empty\(\s*worker", mt)),
              "in the model, Core::empty is called only by worker_op (an EmptyX action) (FR-17.X)")
    # ---- property, terminality and branch functions hold no replay value ----
    uses = re.findall(r"(?m)^use\s+([^;]+);", files[PR][1])
    rep.check("B79", uses and all(u.startswith("std::") for u in uses),
              f"protocol.rs imports only std ({uses}); the model is `use super::*` of it: the explore closure lies in the ledger files")
    clos = call_closure(allitems, masks, None, {"explore"})
    pure = ["state_properties", "transition_properties", "admission_properties", "terminal", "worker_op", "step_fw",
            "step_se", "step_gc", "step_adm", "step_ack", "step_env", "fact_append", "settlement_read", "allowed",
            "present_token", "core_key", "encode_core", "successors", "record_counted", "record_dropped",
            "note_fact_result", "set_class", "claimable", "verb_order", "expected_flags"]
    leak = []
    for name in pure:
        b = body_of(mti, mt, name) or ""
        for tok in ("replays", "Replay", "step_replay"):
            if re.search(rf"\b{tok}\b", b):
                leak.append((name, tok))
        if re.search(r"\bwk\s*\.\s*last\b", b) or (name != "verb_order" and re.search(r"\blast\b", b)):
            leak.append((name, "last"))
    rep.check("B80", not leak, f"no property, terminality, branch or ordinary-step function names replays, Replay or Wk.last ({leak})")
    tb = N("terminal")
    wf = sorted(set(re.findall(r"\bw\.([a-z_]+(?:\.[a-z_]+)?)", tb)))
    cf = sorted(set(re.findall(r"\bconfig\.([a-z_]+)", tb)))
    rep.check("B81", set(wf) <= {"fw.iter", "se.pos", "wk.pos", "gc.iter", "ack.iter", "adm", "env_done", "se.ticket", "core.latch",
                                 "core.shared", "core.u", "core.c", "core.x", "core.phase", "table.class"} and set(cf) <= {"env", "facts"},
              f"terminal() reads world fields {wf} and config fields {cf}: none is replay-only (FR-12)")
    spf = sorted(set(re.findall(r"\bconfig\.([a-z_]+)", sp)))
    rep.check("B82", set(spf) <= {"facts", "gating"} and not re.search(r"\b(replays|last)\b", sp),
              f"state_properties reads config fields {spf} and no replay value (FR-9)")
    tp = N("transition_properties")
    rep.check("B83", not re.search(r"\b(replays|last|wk|config)\b", tp),
              "transition_properties reads neither replay values, the worker nor the configuration (FR-9, FR-10)")
    su = N("successors")
    rep.check("B84", "for(index,&actor)in all.iter().enumerate(){" in su and "edge:pack(index,&oracle.sequence())," in su
              and "w.faults=oracle.faults;transition_properties(&base,&w,&mut obs,counts);" in su
              and "fault:oracle.fault," in su and "hook::take_counts();let enabled=step(config,&mut w,actor,&mut oracle,&mut obs);let counts=hook::take_counts();if!enabled{break;}" in su,
              "successors: edges packed by actor index and choices; faults and hook counts per step; disabled steps add no edge (FR-6, FR-5)")
    ex_body = N("explore")
    rep.check("B85", "if next.is_empty(){no_step+=1;if!terminal(config,&decode(config,&key)){" in ex_body and "if successor.fault{fault_edges+=1;}else{nonfault.push(to);}" in ex_body,
              "explore: D1 judged at states with no successor; L1 over non-fault edges (FR-12, FR-14)")
    # config.name inside the explore closure
    name_in = [o for o in allocc if o["token"] == ".name" and o["file"] == MT and (MT, o["item"]) in clos]
    def in_format_macro(off):
        mm_ = masks[MT]
        depth, k = 0, off - 1
        while k >= 0:
            ch = mm_[k]
            if ch in ")]}":
                depth += 1
            elif ch in "([{":
                if depth == 0:
                    return ch == "(" and re.search(r"\b(writeln|write|format|println|print|eprintln|assert|assert_eq|assert_ne|panic)!\s*$", mm_[:k]) is not None
                depth -= 1
            k -= 1
        return False
    ok_name = all(in_format_macro(o["off"]) for o in name_in)
    rep.check("B86", [o["item"] for o in name_in] == ["explore"] and ok_name,
              f"in the explore closure config.name is read once, by the report header's writeln! ({[(o['line'], o['item']) for o in name_in]})")
    # writes of World.replays and Wk.last
    w_rep = sorted({(o["item"]) for o in allocc if o["file"] == MT and o["token"] == "replays" and o.get("rw_b") == "WRITE"})
    rep.check("B87", w_rep == ["const BASE", "const FR", "decode", "initial", "step_replay"],
              f"replays is written only in BASE/FR (the configurations), initial, decode and step_replay (-= 1) ({w_rep}) (FR-13)")
    w_last = sorted({(o["item"]) for o in allocc if o["file"] == MT and o["token"] == "last" and o.get("rw_b") == "WRITE"
                     and o["item"] not in ("last_step_violations",)})
    rep.check("B88", w_last == ["decode", "initial", "step_replay", "step_wk"],
              f"Wk.last is written only in initial, decode and step_wk; step_replay binds a local ({w_last}) (FR-RS)")
    sr = N("step_replay")
    rep.check("B89", re.search(r"\bo\.", sr) is None and "w.wk.last=" not in sr and "w.core" not in sr.replace("present_token(&w.core,token)", "")
              and "w.rows" not in sr and "w.faults" not in sr,
              "step_replay touches no Oracle, no core, no ledger rows, no fault budget and does not write Wk.last itself (FR-RS)")
    # FR config equals F2 but for name and replays
    frc = norm(item_text(mti, mt, "const FR") or "")
    rep.check("B90", frc == norm('const FR: Config = Config { name: , replays: 3, ..F2 };'),
              "const FR = Config { name, replays: 3, ..F2 }: every other field is F2's (FR-1)")
    # ---- outputs ----
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "B-occurrences.json"), "w") as fh:
        json.dump([{k: v for k, v in o.items() if k != "off"} for o in allocc], fh, indent=1, sort_keys=True)
    with open(os.path.join(out, "B-explore-closure.txt"), "w") as fh:
        names = sorted(f"{r.split('/')[-1]}::{n}" for r, n in clos)
        fh.write(f"# call closure of explore (Checker B): {len(names)} fns\n" + "\n".join(names) + "\n")
    with open(os.path.join(out, "B-checks.txt"), "w") as fh:
        fh.write("\n".join(rep.lines) + "\n")
    for line in rep.lines:
        print(line)
    print(f"CHECKER B: {len(rep.lines)} checks, {rep.fail} failed; {len(allocc)} target occurrences")
    return 0 if rep.fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
