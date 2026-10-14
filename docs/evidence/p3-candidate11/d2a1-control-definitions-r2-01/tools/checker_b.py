#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01, mission sections 6, 9 and 10: static checker B (read-only; no build,
no test, no rustc, no Cargo).

    checker_b.py ROOT [CATDIR]

Checker B is implemented independently of checker A and of the deriver. It shares no code with them and uses other
techniques:
  - blobs are read with `git ls-tree` + `git cat-file blob`, not `git show`;
  - patches are parsed and applied by its own strict unified-diff engine (no fuzz, no offset: every context and
    removed line must match at the stated line), forward and in reverse; no GNU patch, no git apply;
  - Rust source is read by its own tokenizer (comments and whitespace dropped); items are found structurally
    (top-level fn/struct/enum/impl, and methods inside an impl block), not by header text;
  - each of the 21 Architect definitions is checked as a token-level transformation of the HEALTHY item that B
    derives from its own reading of section 6 (removal, insertion or replacement of token sequences), plus
    hunk-level confinement of every changed line to the named items;
  - the contract is read as a markdown table by column split; configurations and named tests are found by token
    patterns;
  - R2 (mission R2 sections 4, 6, 9): the R1 catalog and patches are read straight from the R1 evidence commit's
    git objects (`ls-tree` + `cat-file`), not from ROOT/inputs; entries are compared as canonical JSON; A6's patch is
    compared with R1's hunk by hunk through B's own diff parser (hunk header numbers, function context and body);
    KEY_FIELDS is found as a top-level `const` item by the tokenizer and its string token is evaluated by a
    character-by-character reading of Rust escapes; the state key's fields are read from tokens.
The last line is the per-id verdict map as JSON. Exit 0 only if every check holds.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

GIT = "/home/nexus/NEXUS/nexus-os"
CAND = "d5964ee5326c2733177307db6f507328023d361a"
EVID = "59df21036f7c0183fa17d76b6e0886f8f5784685"
STAGE_ONE = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
P = "crates/nexus-governed-control/src/ledger/protocol.rs"
MT = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
UT = "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs"
EXPECT_BLOB = {P: "a2df2841b99cad5d452008a6d4150e9369d07e66", MT: "28085ce232827301104b5d757af042ec933976f8",
               UT: "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}
HASHES = {"CONTRACT-02-R1.md": "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c",
          "AD1-R4.md": "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693",
          "AD1-R5.md": "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473",
          "blocked/catalog.json": "a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747"}
FOURTEEN = {"C1", "C2", "C4", "C9", "C10", "C12", "C14", "C19", "C21", "C22", "C24", "C25", "C27", "C28"}
# Mission section 6, re-read independently: id -> (file, property list, configuration).
MISSION6 = {"C3a": (P, "P13 P15", "F-7"), "C3b": (P, "P13 P12", "F-6"), "C3c": (P, "P12", "F-6"),
            "C5": (P, "P13", "F-5"), "C6": (P, "P6", "F-PT"), "C7": (P, "P6", "F-PT"), "C8": (P, "E5", "F-B"),
            "C11": (P, "E4", "F-B2"), "C15": (P, "E8", "F-R"), "C16": (P, "E3", "F-D"), "C17": (P, "E4", "F-B2"),
            "C18a": (P, "E7", "F-G"), "C18b": (P, "E7", "F-G"), "C20": (P, "L1", "F-2"), "C23": (P, "E1", "F-2"),
            "C26": (P, "E9", "F-4"), "A1": (MT, "P1", "F-1"), "A2": (MT, "P1", "F-1"), "A3": (MT, "P12", "F-5"),
            "A5": (MT, "L1", "F-2"), "A6": (MT, "P12 P15", "F-5")}
PUB = "impl < 'a > Publish < 'a >"
ITEMS = {  # B's structural item keys: (container, name)
    "C3a": [(PUB, "acknowledgement")], "C3b": [(PUB, "acknowledgement")], "C3c": [(PUB, "acknowledgement")],
    "C5": [(PUB, "resolve_ack")], "C6": [("impl Core", "classify")], "C7": [(PUB, "explain")],
    "C8": [(PUB, "facts")], "C11": [(PUB, "facts")], "C15": [("impl Core", "names")],
    "C16": [("impl Drop for Ticket", None)], "C17": [("impl Drop for Publish < '_ >", None)],
    "C18a": [(PUB, "gating")], "C18b": [("impl Core", "classify")], "C20": [(PUB, "cell")],
    "C23": [("impl Core", "flags")], "C26": [(PUB, "barrier")], "A1": [("", "step_fw")],
    "A2": [("", "step_adm")], "A3": [("", "step_wk")], "A5": [("", "claimable")],
    "A6": [("struct", "Table"), ("const", "KEY_FIELDS"), ("", "initial"), ("", "encode"), ("", "decode"),
           ("", "step_wk"), ("", "step_ack"), ("", "step_adm")]}
R1_EVIDENCE = "f81159046bf0b62666851bc1185992318302d31f"
R1_PACKET = "docs/evidence/p3-candidate11/d2a1-control-definitions-r1-01/"
R1_CAT_SHA = "a73d5721ec59a5d14abf5c31db72d1da1d34962d1db7294347797dd677ae6479"
R2_VERSION = "d2a1-controls-v1-r2-candidate"
# Mission R2 section 4, re-read independently.
WAS = "the test's table (classes, debt counter, H, A);"
NOW = "the test's table (classes, debt counter, H, A, recovery_required);"
# Mission R2 section 6's list for the 34 unchanged controls, as B reads it.
SAME_34 = ["patch", "patch_sha256", "mutant_blob", "mutant_sha256", "mutation_source_file", "named_property",
           "test", "configuration", "receipt_type", "restoration", "restored_receipt", "special_attribution_rules"]
A6_DERIVED = {"edits", "patch", "patch_sha256", "mutant_blob", "mutant_sha256", "patch_lines_removed_added",
              "definition_origin", "patch_origin", "one_named_change", "preserved_behaviour",
              "execution_dependent_uncertainty", "receipt_metadata_item", "r2_correction"}
STR = ("str", "*")  # wildcard: any one string literal
log = []


def out(cid, cls, label, ok, detail=""):
    log.append((cid, bool(ok)))
    print(f"{'pass' if ok else 'FAIL'} {cid} <{cls}> {label}{(' :: ' + str(detail)) if detail != '' else ''}")


def g(*a):
    return subprocess.run(["git", "-C", GIT, *a], capture_output=True, check=True).stdout


def blob_of(commit, path):
    line = g("ls-tree", commit, "--", path).decode().split()
    return line[2]


def oid(data):
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def h256(b):
    return hashlib.sha256(b).hexdigest()


# ---- tokenizer -------------------------------------------------------------------------------------------------
PUNCT = sorted([">>=", "<<=", "...", "..=", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=",
                "/=", "%=", "^=", "&=", "|=", "<<", ">>", ".."], key=len, reverse=True)


def tokens(src):
    toks, i, line, n = [], 0, 1, len(src)
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
        elif c.isspace():
            i += 1
        elif src.startswith("//", i):
            while i < n and src[i] != "\n":
                i += 1
        elif src.startswith("/*", i):
            depth = 0
            while i < n:
                if src.startswith("/*", i):
                    depth += 1
                    i += 2
                elif src.startswith("*/", i):
                    depth -= 1
                    i += 2
                    if depth == 0:
                        break
                else:
                    line += src[i] == "\n"
                    i += 1
        elif re.match(r'(b?r#*")', src[i:i + 8]) and (src[i] == "r" or src.startswith("br", i)):
            m = re.match(r'(b?r)(#*)"', src[i:])
            close = '"' + m.group(2)
            j = src.index(close, i + len(m.group(0))) + len(close)
            toks.append(("str", src[i:j], line))
            line += src[i:j].count("\n")
            i = j
        elif c == '"' or src.startswith('b"', i):
            j = i + (2 if c == "b" else 1)
            while src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            toks.append(("str", src[i:j + 1], line))
            line += src[i:j + 1].count("\n")
            i = j + 1
        elif c == "'":
            m = re.match(r"'(\\(x..|u\{[0-9a-fA-F]+\}|.)|[^\\'])'", src[i:])
            if m:
                toks.append(("chr", m.group(0), line))
                i += len(m.group(0))
            else:
                m = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", src[i:])
                toks.append(("life", m.group(0), line))
                i += len(m.group(0))
        elif c.isalpha() or c == "_":
            m = re.match(r"[A-Za-z_][A-Za-z0-9_]*", src[i:])
            toks.append(("id", m.group(0), line))
            i += len(m.group(0))
        elif c.isdigit():
            m = re.match(r"[0-9][0-9A-Za-z_]*(\.[0-9][0-9_]*)?", src[i:])
            toks.append(("num", m.group(0), line))
            i += len(m.group(0))
        else:
            for p in PUNCT:
                if src.startswith(p, i):
                    toks.append(("p", p, line))
                    i += len(p)
                    break
            else:
                toks.append(("p", c, line))
                i += 1
    return toks


def T(snippet):
    return [(k, v) for k, v, _ in tokens(snippet)]


def kv(toks):
    return [(k, v) for k, v, *_ in toks]


def match_at(seq, sub, i):
    if i + len(sub) > len(seq):
        return False
    return all(s == STR and q[0] == "str" or q == s for q, s in zip(seq[i:i + len(sub)], sub))


def find_all(seq, sub):
    return [i for i in range(len(seq) - len(sub) + 1) if match_at(seq, sub, i)]


def replace_once(seq, old, new):
    at = find_all(seq, old)
    if len(at) != 1:
        return None
    return seq[:at[0]] + new + seq[at[0] + len(old):]


def same(a, b):
    return len(a) == len(b) and all(y == STR and x[0] == "str" or x == y for x, y in zip(a, b))


# ---- structural items ------------------------------------------------------------------------------------------
def close_of(toks, i):
    depth = 0
    for j in range(i, len(toks)):
        if toks[j][1] == "{":
            depth += 1
        elif toks[j][1] == "}":
            depth -= 1
            if depth == 0:
                return j
    return None


def body_start(toks, i):
    """From a `fn` token: the index of its body's `{`, or None for a declaration ending in `;`."""
    depth = 0
    for j in range(i, len(toks)):
        v = toks[j][1]
        if v in "([":
            depth += 1
        elif v in ")]":
            depth -= 1
        elif depth == 0 and v == ";":
            return None
        elif depth == 0 and v == "{":
            return j
    return None


def items(toks):
    """{(container, name): [(start, end)]} for top-level fn/struct/enum/const, impl blocks and methods in them."""
    found, i = {}, 0
    while i < len(toks):
        k, v = toks[i][0], toks[i][1]
        if k == "id" and v == "fn" and i + 1 < len(toks):
            b = body_start(toks, i)
            if b is None:
                i += 1
                continue
            e = close_of(toks, b)
            found.setdefault(("", toks[i + 1][1]), []).append((i, e))
            i = e + 1
        elif k == "id" and v == "const" and i + 2 < len(toks) and toks[i + 1][0] == "id" and toks[i + 2][1] == ":":
            # R2: a top-level constant item, `const NAME: T = ...;` (to its `;` at bracket depth 0)
            depth, j = 0, i
            while j < len(toks):
                t = toks[j][1]
                if t in ("(", "[", "{"):
                    depth += 1
                elif t in (")", "]", "}"):
                    depth -= 1
                elif t == ";" and depth == 0:
                    break
                j += 1
            found.setdefault(("const", toks[i + 1][1]), []).append((i, j))
            i = j + 1
        elif k == "id" and v in ("struct", "enum") and i + 2 < len(toks) and toks[i + 2][1] == "{":
            e = close_of(toks, i + 2)
            found.setdefault((v, toks[i + 1][1]), []).append((i, e))
            i = e + 1
        elif k == "id" and v == "impl":
            b = next(j for j in range(i, len(toks)) if toks[j][1] == "{")
            e = close_of(toks, b)
            key = " ".join(t[1] for t in toks[i:b])
            found.setdefault((key, None), []).append((i, e))
            j = b + 1
            while j < e:
                if toks[j][1] == "fn" and toks[j][0] == "id":
                    bb = body_start(toks, j)
                    if bb is None:
                        j += 1
                        continue
                    ee = close_of(toks, bb)
                    found.setdefault((key, toks[j + 1][1]), []).append((j, ee))
                    j = ee + 1
                else:
                    j += 1
            i = e + 1
        else:
            i += 1
    return found


def get(toks, its, key):
    r = its.get(key)
    if not r or len(r) != 1:
        return None
    s, e = r[0]
    return kv(toks[s:e + 1])


# ---- strict unified diff -----------------------------------------------------------------------------------------
def parse_patch(data):
    text = data.decode()
    files, cur = [], None
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    k = 0
    while k < len(lines):
        l = lines[k]
        if l.startswith("diff --git "):
            cur = {"diff": l, "old": None, "new": None, "hunks": []}
            files.append(cur)
        elif l.startswith("--- ") and cur is not None and cur["old"] is None:
            cur["old"] = l[4:]
        elif l.startswith("+++ ") and cur is not None and cur["new"] is None:
            cur["new"] = l[4:]
        elif l.startswith("@@ "):
            m = re.match(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", l)
            hk = {"os": int(m.group(1)), "ol": int(m.group(2) or 1), "ns": int(m.group(3)),
                  "nl": int(m.group(4) or 1), "body": [], "head": l}
            cur["hunks"].append(hk)
            k += 1
            while k < len(lines) and (lines[k][:1] in (" ", "-", "+", "\\") or lines[k] == ""):
                if lines[k].startswith("\\"):
                    raise ValueError("no-newline markers are not expected")
                if sum(1 for b in hk["body"] if b[0] in " -") == hk["ol"] and \
                        sum(1 for b in hk["body"] if b[0] in " +") == hk["nl"]:
                    break
                hk["body"].append(lines[k] if lines[k] else " ")
                k += 1
            continue
        k += 1
    return files


def apply_strict(base, hunks, reverse=False):
    """Exact application: each hunk's old side must equal the file at its stated start. Returns bytes or None."""
    src = base.decode().split("\n")
    trailing = src[-1] == ""
    if trailing:
        src.pop()
    res, pos = [], 0
    for hk in hunks:
        old = [b[1:] for b in hk["body"] if b[0] in (" ", "+" if reverse else "-")]
        new = [b[1:] for b in hk["body"] if b[0] in (" ", "-" if reverse else "+")]
        start = (hk["ns"] if reverse else hk["os"]) - 1
        if len(old) == 0:
            start += 1
        if start < pos or src[start:start + len(old)] != old:
            return None
        res += src[pos:start] + new
        pos = start + len(old)
    res += src[pos:]
    return ("\n".join(res) + ("\n" if trailing else "")).encode()


def changed_base_lines(hunks):
    """Base line numbers (1-based) touched by each hunk: removed lines, and the line before which '+' lines go."""
    touched = []
    for hk in hunks:
        ln = hk["os"]
        for b in hk["body"]:
            if b[0] == " ":
                ln += 1
            elif b[0] == "-":
                touched.append(ln)
                ln += 1
            else:
                touched.append(ln)
    return touched


# ---- section 6, as token transformations of the healthy items -----------------------------------------------------
def transform(cid, base_items, mut_items, bfile, mfile):
    """[(label, ok)] for one Architect definition. Items are token lists keyed by B's item keys."""
    r = []
    add = lambda label, ok: r.append((label, ok))  # noqa: E731
    ack = (PUB, "acknowledgement")
    if cid in ("C3a", "C3b", "C3c"):
        gone = {"C3a": "if set.frontier != now.frontier { Some(AckRefusal::Stale) } else",
                "C3b": "else if set.top > set.head || set.head > now.head { Some(AckRefusal::Future) }",
                "C3c": "else if now.count != set.count || now.digest != set.digest { Some(AckRefusal::Set) }"}[cid]
        want = replace_once(base_items[ack], T(gone), [])
        add(f"item = healthy item with the whole clause `{gone}` removed and nothing else",
            want is not None and same(mut_items[ack], want))
        keep = {"C3a": ["set.top <= set.frontier || set.count < 1", "set.top > set.head || set.head > now.head",
                        "now.count != set.count || now.digest != set.digest"],
                "C3b": ["set.frontier != now.frontier", "set.top <= set.frontier || set.count < 1",
                        "now.count != set.count || now.digest != set.digest"],
                "C3c": ["set.frontier != now.frontier", "set.top <= set.frontier || set.count < 1",
                        "set.top > set.head || set.head > now.head"]}[cid]
        add("the other three tests remain, each once", all(len(find_all(mut_items[ack], T(k))) == 1 for k in keep))
        add("recheck, rollback, insert and commit verbs remain",
            all(find_all(mut_items[ack], T(v)) for v in ("io.recheck(set.frontier, set.top)", "io.insert(snapshot)",
                                                          "io.commit()", "return AckOutcome::Refused(refusal);")))
    elif cid == "C5":
        sig = "fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {"
        body = ("if set.top > self.core.frontier { self.core.frontier = set.top; } self.core.u = None; "
                "self.core.phase = Phase::Healthy; Resolve::Acknowledged(set.top) }")
        m = mut_items[(PUB, "resolve_ack")]
        add("signature unchanged; body = raise frontier, empty U, Healthy, Acknowledged(set.top); no read",
            same(m, T(sig + body)) and same(m[:len(T(sig))], base_items[(PUB, "resolve_ack")][:len(T(sig))]))
        add("io and nonce appear only in the signature", sum(1 for t in m if t[1] in ("io", "nonce")) == 2)
        add("caller resolve unchanged", same(mut_items[(PUB, "resolve")], base_items[(PUB, "resolve")]))
    elif cid == "C6":
        k = ("impl Core", "classify")
        old = "if let Some(known) = self.known(&member.key, facts.explained) {"
        new = ("let known = match &self.x { Some((_, Occupant::Explanation { original, cause }, stage)) "
               "if *original == member.key => { Some(match stage { Stage::Built => Known::Cause(*cause), "
               "Stage::Committed(seq) => Known::Explained(*seq), }) } _ => facts.explained.map(Known::Explained), "
               "}; if let Some(known) = known {")
        want = replace_once(base_items[k], T(old), T(new))
        add("row b: Known from X's explanation of this original, else facts.explained; refusal table not read",
            want is not None and same(mut_items[k], want) and not find_all(mut_items[k], T("self.refusals.get")))
        add("Core::known and Publish::lookup unchanged",
            same(mut_items[("impl Core", "known")], base_items[("impl Core", "known")]) and
            same(mut_items[(PUB, "lookup")], base_items[(PUB, "lookup")]))
    elif cid == "C7":
        k = (PUB, "explain")
        want = replace_once(base_items[k], T("Read::Got(Some(seq)) => { self.core.refusals.remove(key);"),
                            T("Read::Got(Some(seq)) => { self.core.refusals.clear();"))
        add("explain's Explained arm clears the table instead of removing one key; nothing else",
            want is not None and same(mut_items[k], want))
        add("Publish::settle unchanged", same(mut_items[(PUB, "settle")], base_items[(PUB, "settle")]))
    elif cid == "C8":
        m, b = mut_items[(PUB, "facts")], base_items[(PUB, "facts")]
        ids = [t[1] for t in m]
        add("no pending array, no BATCH_MAX, one for-loop, one insert verb",
            "pending" not in ids and "BATCH_MAX" not in ids and ids.count("for") == 1 and
            len(find_all(m, T("io.insert("))) == 1)
        cut_b = find_all(b, T("let mut pending"))
        cut_m = find_all(m, T("let mut any_inserted = false;"))
        add("everything before Step 3 (Steps 0 to 2) unchanged", len(cut_b) == 1 and len(cut_m) == 1 and
            same(m[:cut_m[0]], b[:cut_b[0]]))
        tail_b, tail_m = find_all(b, T("match io.commit()")), find_all(m, T("match io.commit()"))
        add("Step 6 onward unchanged", len(tail_b) == 1 and len(tail_m) == 1 and same(m[tail_m[0]:], b[tail_b[0]:]))
        ins = find_all(m, T("Verdict::Insert => { self.issued = true; any_inserted = true; let reason = match "
                            "io.insert(at, member) { Insert::Inserted(_) => continue, Insert::Constraint => { "
                            "self.core.latch(LatchCause::Constraint); Reason::Constraint } Insert::Failed => "
                            "Reason::Storage, }; self.failed(); let persistence = self.rolled(io.rollback()); "
                            "return self.not_applied(persistence, reason); }"))
        loop = find_all(m, T("for (index, member) in members().enumerate() {"))
        cls = find_all(m, T(".classify(at, member, &facts, members().take(index), false)"))
        add("in one member-order loop: facts, classify, then the insert at once (healthy Step-5 outcomes)",
            len(ins) == 1 and len(loop) == 1 and len(cls) == 1 and loop[0] < cls[0] < ins[0])
        add("refusal: conflict, one rollback, Not written with the classification reason",
            len(find_all(m, T("Verdict::Refused(reason, conflict) => { self.conflict = conflict; "
                              "self.rolled(io.rollback()); return self.not_applied(Persistence::NotWritten, "
                              "reason); }"))) == 1)
        add("no insert: the empty-transaction rollback and Committed after the loop",
            len(find_all(m, T("if !any_inserted { self.rolled(io.rollback()); return FactOutcome::Committed; }")))
            == 1)
        add("facts read and its failure arm carried unchanged",
            len(find_all(m, T("let facts = match io.facts(at, member) { Read::Got(facts) => facts, failed => { "
                              "self.read_failed(failed == Read::Corrupt); self.rolled(io.rollback()); return "
                              "self.not_applied(Persistence::NotWritten, Reason::Storage); } };"))) == 1)
    elif cid == "C11":
        k = (PUB, "facts")
        old = ("io.insert(member_index(index), member) { Insert::Inserted(_) => continue, Insert::Constraint => { "
               "self.core.latch(LatchCause::Constraint); Reason::Constraint }")
        new = ("io.insert(member_index(index), member) { Insert::Inserted(_) => continue, Insert::Constraint => { "
               "self.core.latch(LatchCause::Constraint); self.failed(); self.rolled(io.rollback()); "
               "return self.not_applied(Persistence::NotWritten, Reason::Constraint); }")
        want = replace_once(base_items[k], T(old), T(new))
        add("fact Step-5 Constraint arm only: latch, failed, one rollback ignored, NotWritten + Constraint",
            want is not None and same(mut_items[k], want))
        add("gating, acknowledgement and cell unchanged",
            all(same(mut_items[(PUB, n)], base_items[(PUB, n)]) for n in ("gating", "acknowledgement", "cell")))
    elif cid == "C15":
        add("names = same allocation && same cell; no occupancy test of any kind",
            same(mut_items[("impl Core", "names")],
                 T("fn names(&self, token: &CellToken, cell: Cell) -> bool { "
                   "Arc::ptr_eq(&token.ledger, &self.shared) && token.cell == cell }")))
    elif cid == "C16":
        k = ("impl Drop for Ticket", None)
        want = replace_once(base_items[k],
                            T("TicketState::Counted(_) => { exchange(&self.shared, 0, |word| word | HANDOVER_FAULT);"),
                            T("TicketState::Counted(_) => { exchange(&self.shared, 0, |word| { "
                              "count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false) });"))
        add("Counted arm: count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false); nothing else",
            want is not None and same(mut_items[k], want))
        add("Ticket::confirm unchanged", same(mut_items[("impl Ticket", "confirm")], base_items[("impl Ticket",
                                                                                                  "confirm")]))
    elif cid == "C17":
        k = ("impl Drop for Publish < '_ >", None)
        want = replace_once(base_items[k], T("let persistence = if self.issued && self.conflict.is_none() { "
                                             "Persistence::Uncertain } else { Persistence::NotWritten };"),
                            T("let persistence = Persistence::NotWritten;"))
        add("unwind persistence always NotWritten; keep, phase, conflict, U, exit, publication unchanged",
            want is not None and same(mut_items[k], want))
    elif cid == "C18a":
        k = (PUB, "gating")
        want = replace_once(base_items[k], T("Verdict::Refused(reason, _) => {"),
                            T("Verdict::Refused(reason, _) => { self.keep = true;"))
        add("keep = true only in the classification-refused arm, before its rollback; nothing else",
            want is not None and same(mut_items[k], want))
        add("the guard unchanged", same(mut_items[("impl Drop for Publish < '_ >", None)],
                                        base_items[("impl Drop for Publish < '_ >", None)]))
    elif cid == "C18b":
        k = ("impl Core", "classify")
        want = replace_once(base_items[k],
                            T("self.explanation(member.key, cause); } return Verdict::Refused(conflict, Some(at));"),
                            T("self.explanation(member.key, cause); } self.refusals.insert(member.key, cause); "
                              "return Verdict::Refused(conflict, Some(at));"))
        add("gating branch: X explanation kept, table entry added, same conflict returned; nothing else",
            want is not None and same(mut_items[k], want))
    elif cid == "C20":
        k = (PUB, "cell")
        want = replace_once(base_items[k], T("self.rolled(io.rollback()); self.settle(cell, seq); "
                                             "return CellAppend::Committed(seq);"),
                            T("self.rolled(io.rollback()); if cell == Cell::C { self.settle(cell, seq); } "
                              "return CellAppend::Committed(seq);"))
        add("present path: settle only for C; Committed(seq) for both; Durable path unchanged",
            want is not None and same(mut_items[k], want))
        add("Publish::settle unchanged", same(mut_items[(PUB, "settle")], base_items[(PUB, "settle")]))
    elif cid == "C23":
        k = ("impl Core", "flags")
        want = replace_once(base_items[k], T("| bit(self.x.is_some(), X_OCCUPIED)"), [])
        add("flags = healthy flags without the X bit term", want is not None and same(mut_items[k], want))
    elif cid == "C26":
        k = (PUB, "barrier")
        want = replace_once(base_items[k], T("return BarrierOutcome::Failed; }"),
                            T("return BarrierOutcome::Failed; } self.core.latch = None;"))
        add("latch cleared only after a run that returned true, before the phase assignment",
            want is not None and same(mut_items[k], want))
    elif cid == "A1":
        k = ("", "step_fw")
        b = base_items[k]
        o0 = find_all(b, T("0 => {"))
        rec_end = find_all(b, T("w.fw[i].pos = FwPos::Recorded;"))
        rec = b[o0[0] + len(T("0 => {")):rec_end[0]] if len(o0) == 1 and len(rec_end) == 1 else None
        note = T('obs.note(|| format!("[T] records {held:?}"));')
        guarded = T("FwPos::Got(held) if w.fw[i].ticket.is_none() => {") + (rec or []) + \
            T("w.fw[i].pos = FwPos::Done;") + note + T("}")
        step1 = replace_once(b, T("FwPos::Got(held) => {"), guarded + T("FwPos::Got(held) => {"))
        confirm = T("obs.ticket_steps += 1; w.fw[i].ticket.take().expect(") + [STR] + \
            T(").confirm(); obs.note(|| \"confirm\".into());")
        want = replace_once(step1, T("0 => {") + (rec or []) + T("w.fw[i].pos = FwPos::Recorded;") + note,
                            T("0 => {") + confirm) if step1 and rec else None
        add("Got with a ticket: option 0 confirms, records nothing, keeps Got(held); options, unwind and DROP "
            "unchanged; Got without a ticket: the healthy record block verbatim, then Done, no choice",
            want is not None and same(mut_items[k], want))
        add("settlement unchanged; no new FwPos variant",
            same(mut_items[("", "step_se")], base_items[("", "step_se")]) and
            same(mut_items[("enum", "FwPos")], base_items[("enum", "FwPos")]))
    elif cid == "A2":
        k = ("", "step_adm")
        b = base_items[k]
        old = T("if w.adm == 0 { return false; } obs.views += 1; let view = w.health.view(); let admitted = "
                "view.admits() && w.table.debt == 0 && w.table.h <= w.table.a; w.adm -= 1;")
        new = T("let Some(kept) = w.adm_kept else { if w.adm == 0 { return false; } obs.views += 1; let first = "
                "w.health.view(); w.adm_kept = Some(first.word & FLAGS); obs.note(|| ") + [STR] + \
            T(".into()); return true; }; obs.views += 1; let second = w.health.view(); let view = HealthView { "
              "word: kept | with_field(0, UNAPPLIED_SHIFT, field(second.word, UNAPPLIED_SHIFT)), }; let admitted = "
              "view.admits() && w.table.debt == 0 && w.table.h <= w.table.a; w.adm_kept = None; w.adm -= 1;")
        want = replace_once(b, old, new)
        add("two steps keyed by adm_kept: step one keeps view & FLAGS, enabled, no decrement/branch/property; "
            "step two keeps only unapplied, decides with the healthy expression, clears, decrements once",
            want is not None and same(mut_items[k], want))
        add("World unchanged (no new field)", same(mut_items[("struct", "World")], base_items[("struct", "World")]))
    elif cid == "A3":
        k = ("", "step_wk")
        s1 = replace_once(base_items[k], T("if let Action::EmptyX(_, seq) = &action { "
                                           "w.table.h = w.table.h.max(*seq); }"), [])
        s2 = replace_once(s1, T("worker_op(config, w, &action, o, obs);"),
                          T("worker_op(config, w, &action, o, obs); let next = match &action { "
                            "Action::EmptyX(_, seq) => WkPos::RaiseH(*seq), _ => WkPos::Idle, };")) if s1 else None
        want = replace_once(s2, T("w.wk.pos = WkPos::Idle; } WkPos::RaiseH"),
                            T("w.wk.pos = next; } WkPos::RaiseH")) if s2 else None
        add("Seen raises no H; Act: worker_op, then RaiseH(seq) for EmptyX else Idle; RaiseH unchanged",
            want is not None and same(mut_items[k], want))
        add("no new WkPos variant", same(mut_items[("enum", "WkPos")], base_items[("enum", "WkPos")]))
    elif cid == "A5":
        k = ("", "claimable")
        want = replace_once(base_items[k],
                            T("Class::Conflict if view.phase() == Phase::Healthy && !view.x_occupied() => { "
                              "Some(Claim::Explain) }"),
                            T("Class::Conflict if !view.x_occupied() => Some(Claim::Explain),"))
        add("Conflict guard loses only the Healthy conjunct; X-empty, Explain and the other arms unchanged",
            want is not None and same(mut_items[k], want))
    elif cid == "A6":
        steps = {("struct", "Table"): [("a: Seq,", "a: Seq, recovery_required: bool,")],
                 ("", "initial"): [("h: 0, a: 0,", "h: 0, a: 0, recovery_required: false,")],
                 ("", "encode"): [("e.u(w.table.a);", "e.u(w.table.a); e.b(w.table.recovery_required);")],
                 ("", "decode"): [("a: d.u(),", "a: d.u(), recovery_required: d.b(),")],
                 ("", "step_wk"): [("w.table.a = w.table.a.max(frontier);",
                                    "if frontier > w.table.a { w.table.a = frontier; "
                                    "w.table.recovery_required = false; }"),
                                   ("w.table.h = w.table.h.max(*seq);",
                                    "if *seq > w.table.h { w.table.h = *seq; w.table.recovery_required = true; }"),
                                   ("w.table.h = w.table.h.max(seq);",
                                    "if seq > w.table.h { w.table.h = seq; w.table.recovery_required = true; }")],
                 ("", "step_ack"): [("w.table.a = w.table.a.max(n);",
                                     "if n > w.table.a { w.table.a = n; w.table.recovery_required = false; }")],
                 ("", "step_adm"): [("w.table.h <= w.table.a", "!w.table.recovery_required")]}
        for key, edits in steps.items():
            cur = base_items[key]
            for old, new in edits:
                cur = replace_once(cur, T(old), T(new)) if cur is not None else None
            add(f"{key[1]}: the section-6.21 rule as a token transformation, nothing else",
                cur is not None and same(mut_items[key], cur))
        add("P12/P15 admission properties unchanged",
            same(mut_items[("", "admission_properties")], base_items[("", "admission_properties")]))
        # R2 (mission R2 section 4, 9.D, 9.E): KEY_FIELDS, receipt metadata.
        kf = lambda label, ok: r.append((label, ok, "keyfields"))  # noqa: E731
        kb, km = base_items[("const", "KEY_FIELDS")], mut_items[("const", "KEY_FIELDS")]
        sb, sm = [t for t in kb if t[0] == "str"], [t for t in km if t[0] == "str"]
        shape = T("const KEY_FIELDS: &str = ;")
        kf("KEY_FIELDS is `const KEY_FIELDS: &str = <one string literal>;` in C and in the mutant",
           len(sb) == len(sm) == 1 and [t for t in kb if t[0] != "str"] == shape == [t for t in km if t[0] != "str"])
        raw_b, raw_m = (sb[0][1] if sb else ""), (sm[0][1] if sm else "")
        kf("KEY_FIELDS string token = C's token with the one phrase replaced, nothing else",
           raw_b.count(WAS) == 1 and raw_m == raw_b.replace(WAS, NOW, 1))
        vb, vm = rust_string(raw_b), rust_string(raw_m)
        kf("KEY_FIELDS evaluated: C's value with only the table's description changed",
           vb is not None and vm is not None and vb.count(WAS) == 1 and vm == vb.replace(WAS, NOW, 1))
        kf("KEY_FIELDS evaluated: recovery_required named exactly once; the R1 description gone",
           vm is not None and vm.count("recovery_required") == 1 and vm.count(NOW) == 1 and WAS not in vm)
        tfields = struct_fields(mut_items[("struct", "Table")])
        encoded = []
        dec = mut_items[("", "encode")]
        for i in range(len(dec) - 4):
            if [t[1] for t in dec[i:i + 4]] == ["w", ".", "table", "."] and dec[i + 4][1] not in encoded:
                encoded.append(dec[i + 4][1])
        d = mut_items[("", "decode")]
        lit = find_all(d, T("let table = Table {"))
        decoded = struct_fields(d[lit[0] + 3:]) if len(lit) == 1 else None
        names = None
        if vm is not None and "the test's table (" in vm:
            rest = vm.split("the test's table (", 1)[1]
            names = rest.split(");", 1)[0].split(", ") if ");" in rest else None
        say = {"class": "classes", "debt": "debt counter", "h": "H", "a": "A", "recovery_required": "recovery_required"}
        kf("the state key's table: Table's fields = the fields encode writes = the fields decode reads = the names "
           "KEY_FIELDS prints, one each, in one order", "recovery_required" in tfields and
           tfields == encoded == decoded and names == [say.get(x, "<" + x + ">") for x in tfields])
        mt_all, bt_all = kv(tokens(mfile.decode())), kv(tokens(bfile.decode()))
        site = T('writeln!(text, "  state key fields: {KEY_FIELDS}");')
        kf("three report sites print KEY_FIELDS, unchanged; each would print the new table description",
           len(find_all(mt_all, site)) == len(find_all(bt_all, site)) == 3 and vm is not None and
           NOW in "  state key fields: " + vm)
    return r


def rust_string(raw):
    """Evaluate one Rust string-literal token, character by character (escapes and line continuations)."""
    if len(raw) < 2 or raw[0] != '"' or raw[-1] != '"':
        return None
    s, res, i = raw[1:-1], [], 0
    one = {"n": "\n", "r": "\r", "t": "\t", "\\": "\\", "0": "\0", "'": "'", '"': '"'}
    while i < len(s):
        c = s[i]
        if c != "\\":
            res.append(c)
            i += 1
            continue
        n = s[i + 1] if i + 1 < len(s) else ""
        if n == "\n":
            i += 2
            while i < len(s) and s[i] in " \t\n\r":
                i += 1
        elif n in one:
            res.append(one[n])
            i += 2
        elif n == "x":
            res.append(chr(int(s[i + 2:i + 4], 16)))
            i += 4
        elif n == "u" and s[i + 2:i + 3] == "{":
            j = s.index("}", i)
            res.append(chr(int(s[i + 3:j], 16)))
            i = j + 1
        else:
            return None
    return "".join(res)


def struct_fields(tk):
    """Field names of a braced struct body or struct literal: an identifier directly after `{` or `,` at brace depth
    1 and bracket depth 0, followed by `:`, `,` or `}` (shorthand)."""
    try:
        start = next(i for i, t in enumerate(tk) if t[1] == "{")
    except StopIteration:
        return None
    names, depth, sq = [], 0, 0
    for i in range(start, len(tk)):
        v = tk[i][1]
        if v == "{":
            depth += 1
        elif v == "}":
            depth -= 1
            if depth == 0:
                return names
        elif v in "([":
            sq += 1
        elif v in ")]":
            sq -= 1
        elif depth == 1 and sq == 0 and tk[i][0] == "id" and tk[i - 1][1] in ("{", ",") and \
                tk[i + 1][1] in (":", ",", "}"):
            names.append(v)
    return None


def r1_compare(cat, catdir, base):
    """Mission R2 sections 6, 7, 9.C and 9.D, read from the R1 evidence commit's own git objects."""
    print("== R1 -> R2 (from the R1 evidence commit's git objects)")
    rcat_raw = g("cat-file", "blob", blob_of(R1_EVIDENCE, R1_PACKET + "catalog/catalog.json"))
    out("catalog", "r1-equality", "R1 catalog read at f8115904 hashes to the mission's R1 sha256",
        h256(rcat_raw) == R1_CAT_SHA)
    rcat = json.loads(rcat_raw)
    listing = {l.split("\t")[1].rsplit("/", 1)[1]: l.split()[2] for l in
               g("ls-tree", R1_EVIDENCE, R1_PACKET + "catalog/patches/").decode().splitlines()}
    canon = lambda o: json.dumps(o, sort_keys=True, ensure_ascii=False)  # noqa: E731
    mine = {e["id"]: e for e in cat["controls"]}
    out("catalog", "r1-equality", "same 36 ids in the same order as R1",
        [e["id"] for e in cat["controls"]] == [e["id"] for e in rcat["controls"]])
    out("catalog", "r1-equality", "R2 names R1 as the candidate it supersedes",
        cat["supersedes_as_candidate"].get("sha256") == R1_CAT_SHA and
        cat["supersedes_as_candidate"].get("evidence_commit") == R1_EVIDENCE)
    identical = []
    for old in rcat["controls"]:
        cid = old["id"]
        new = mine.get(cid)
        if new is None:
            out(cid, "r1-equality", "id present in R2", False)
            continue
        if cid == "A4":
            out(cid, "r1-equality", "A4 = R1's A4 (canonical JSON)", canon(new) == canon(old))
            continue
        pf = os.path.join(catdir, "patches", f"{cid}.patch")
        mine_bytes = open(pf, "rb").read() if os.path.exists(pf) else None
        r1_bytes = g("cat-file", "blob", listing[f"{cid}.patch"])
        if cid != "A6":
            diff = [k for k in SAME_34 if canon(new.get(k)) != canon(old.get(k))]
            out(cid, "r1-equality", "patch file bytes = the R1 blob's bytes", mine_bytes == r1_bytes)
            out(cid, "r1-equality", "patch, sha256, mutant blob/sha256, source file, property, test, configuration, "
                "receipt type, restoration, restored receipt, special rules = R1's", not diff, diff)
            out(cid, "r1-equality", "entry = R1's entry (canonical JSON)", canon(new) == canon(old))
            if mine_bytes == r1_bytes and canon(new) == canon(old):
                identical.append(cid)
            continue
        moved = sorted(k for k in set(new) | set(old) if canon(new.get(k)) != canon(old.get(k)))
        out(cid, "r1-equality", "A6 differs from R1 only in fields derived from the KEY_FIELDS hunk",
            set(moved) <= A6_DERIVED, sorted(set(moved) - A6_DERIVED))
        kept = [k for k in SAME_34 if k not in ("patch", "patch_sha256", "mutant_blob", "mutant_sha256")]
        out(cid, "r1-equality", "A6 source file, property, test, configuration, receipt type, restoration and "
            "special rules = R1's", all(canon(new.get(k)) == canon(old.get(k)) for k in kept))
        ne, oe = new.get("edits", []), old["edits"]
        tail = ne[-1] if ne else {}
        out(cid, "r1-equality", "A6 edits: R1's eight first, unchanged (canonical JSON); one more, the KEY_FIELDS "
            "line, role receipt_metadata", len(oe) == 8 and len(ne) == 9 and canon(ne[:8]) == canon(oe) and
            tail.get("role") == "receipt_metadata" and tail.get("anchor", "").endswith("\\\n") and
            tail.get("anchor", "").count("\n") == 1 and
            tail.get("replacement") == tail.get("anchor", "").replace(WAS, NOW, 1) != tail.get("anchor"))
        try:
            p1, p2 = parse_patch(r1_bytes), parse_patch(mine_bytes or b"")
        except Exception as ex:  # noqa: BLE001
            out(cid, "r2-a6", "both A6 patches parse", False, ex)
            continue
        if len(p1) != 1 or len(p2) != 1:
            out(cid, "r2-a6", "both A6 patches have one file section", False, (len(p1), len(p2)))
            continue
        f = MT
        key = lambda hk: (hk["head"], hk["os"], hk["ol"], hk["ns"], hk["nl"], tuple(hk["body"]))  # noqa: E731
        k1, k2 = [key(h) for h in p1[0]["hunks"]], [key(h) for h in p2[0]["hunks"]]
        add = [h for h in p2[0]["hunks"] if key(h) not in k1]
        out(cid, "r2-a6", "B's parser: R2's hunks = R1's 8 hunks (header, numbers, body) in order + exactly 1",
            len(k1) == 8 and [x for x in k2 if x in k1] == k1 and len(k2) == 9 and len(add) == 1)
        idx = lambda b: [l for l in b.decode().split("\n") if l.startswith("index ")]  # noqa: E731
        out(cid, "r2-a6", "file header: same diff/---/+++ lines; index line differs only in the mutant blob",
            (p1[0]["diff"], p1[0]["old"], p1[0]["new"]) == (p2[0]["diff"], p2[0]["old"], p2[0]["new"]) and
            idx(r1_bytes) == [f"index {EXPECT_BLOB[f]}..{old['mutant_blob']} 100644"] and
            idx(mine_bytes or b"") == [f"index {EXPECT_BLOB[f]}..{new['mutant_blob']} 100644"])
        ok = False
        if len(add) == 1:
            btoks = tokens(base[f].decode())
            span = items(btoks).get(("const", "KEY_FIELDS"))
            lo, hi = (btoks[span[0][0]][2], btoks[span[0][1]][2]) if span and len(span) == 1 else (0, -1)
            hk = add[0]
            rem = [b[1:] for b in hk["body"] if b[0] == "-"]
            ins = [b[1:] for b in hk["body"] if b[0] == "+"]
            lines = changed_base_lines([hk])
            blines = base[f].decode().split("\n")
            # changed_base_lines reports the removed line L and, for the '+' line, the line before which it goes
            # (L + 1 after the removal)
            ok = (len(rem) == len(ins) == 1 and len(lines) == 2 and lines[1] == lines[0] + 1 and lo <= lines[0] <= hi
                  and blines[lines[0] - 1] == rem[0] and ins[0] == rem[0].replace(WAS, NOW, 1) and
                  rem[0].count(WAS) == 1 and lo <= hk["os"] and hk["os"] + hk["ol"] - 1 <= hi)
        out(cid, "r2-a6", "the added hunk: one line of KEY_FIELDS removed and re-added with only the phrase changed",
            ok)
        m1 = apply_strict(base[f], p1[0]["hunks"])
        m2 = apply_strict(base[f], p2[0]["hunks"])
        l1 = m1.decode().split("\n") if m1 else []
        l2 = m2.decode().split("\n") if m2 else []
        moved_lines = [i for i in range(min(len(l1), len(l2))) if l1[i] != l2[i]]
        out(cid, "r2-a6", "strict application: R1 mutant and R2 mutant differ in one line, KEY_FIELDS'",
            m1 is not None and m2 is not None and oid(m1) == old["mutant_blob"] and oid(m2) == new["mutant_blob"]
            and len(l1) == len(l2) and len(moved_lines) == 1 and WAS in l1[moved_lines[0]] and
            NOW in l2[moved_lines[0]])
    out("catalog", "r1-equality", "34 controls identical to R1 (patch bytes and entry)", len(identical) == 34 and
        "A6" not in identical, len(identical))


def main(root, catdir):
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01 — STATIC CHECKER B (strict diff engine + tokenizer)")
    for n, d in HASHES.items():
        out("inputs", "inputs", f"{n} sha256", h256(open(os.path.join(root, "inputs", n), "rb").read()) == d)
    base = {}
    for path, want in EXPECT_BLOB.items():
        b = blob_of(CAND, path)
        data = g("cat-file", "blob", b)
        out("inputs", "inputs", f"ls-tree C {path} -> {want[:8]}, content hashes to it", b == want == oid(data))
        base[path] = data
    cat_raw = open(os.path.join(catdir, "catalog.json"), "rb").read()
    cat = json.loads(cat_raw)
    # contract 7.5 as a markdown table
    contract = open(os.path.join(root, "inputs", "CONTRACT-02-R1.md")).read()
    s = contract.find("### 7.5 Controls")
    sec = contract[s:contract.find("\n### ", s + 5)]
    table = {}
    order_ids = []
    for l in sec.split("\n"):
        if not l.startswith("|"):
            continue
        cols = [c.strip() for c in l.strip().strip("|").split("|")]
        if re.fullmatch(r"C\d+[abc]?|A\d", cols[0]):
            order_ids.append(cols[0])
            table[cols[0]] = cols[1:]
    ents = cat["controls"]
    got = [e["id"] for e in ents]
    print("== identity")
    out("catalog", "ids", "contract ids: 36 distinct (C13 absent)", len(order_ids) == 36 == len(set(order_ids)) and
        "C13" not in order_ids)
    out("catalog", "ids", "catalog ids == contract ids, same order, no duplicate, none missing", got == order_ids,
        sorted(set(order_ids) ^ set(got)) or [i for i in got if got.count(i) > 1])
    kill = [e for e in ents if e["id"] != "A4"]
    out("catalog", "ids", "35 kill-required = 36 minus A4", len(kill) == 35 and len({e["id"] for e in kill}) == 35)
    a4 = [e for e in ents if e["id"] == "A4"]
    out("A4", "a4", "A4: one separate accepted comparison, counted nowhere as a mutation",
        len(a4) == 1 and a4[0]["status"] == "SATISFIED — ARCHITECT ACCEPTED" and a4[0]["not_a_mutation"] is True
        and a4[0].get("counted_as_kill_required") is False and a4[0].get("kind") == "comparison" and
        "A4" not in cat["carried_forward"] + cat["architect_defined"] and
        a4[0].get("definition_status") is None and a4[0].get("patch_file") is None and a4[0].get("edits") is None)
    out("catalog", "counts", "version; counts 35/1/35/0; carried 14 + defined 21 = 35",
        cat["version"] == R2_VERSION and cat["counts"]["kill_required"] == 35 and
        cat["counts"]["comparison"] == 1 and cat["counts"]["candidate_ready"] == 35 and
        cat["counts"]["blocked"] == 0 and set(cat["carried_forward"]) == FOURTEEN and
        set(cat["architect_defined"]) == set(MISSION6) and
        set(cat["carried_forward"]) | set(cat["architect_defined"]) == {e["id"] for e in kill})

    def strings(o, path=""):
        if isinstance(o, dict):
            for k, v in o.items():
                yield from strings(v, f"{path}/{k}")
        elif isinstance(o, list):
            for i, v in enumerate(o):
                yield from strings(v, f"{path}/{i}")
        elif isinstance(o, str):
            yield path, o
    a4i = got.index("A4") if "A4" in got else -1
    bad = [p for p, v in strings(cat) if re.search(r"frozen|ready for execution", v, re.I) or
           ("ARCHITECT ACCEPTED" in v.upper() and p != f"/controls/{a4i}/status")]
    out("catalog", "wording", "no frozen / ready-for-execution claim; 'architect accepted' only as A4's status",
        not bad, bad[:3])
    print("== per control")
    blocked = {e["id"]: e for e in json.load(open(os.path.join(root, "inputs", "blocked", "catalog.json")))["controls"]}
    patches, mblobs, mutated = set(), set(), {}
    for e in kill:
        cid = e["id"]
        row = table.get(cid)
        out(cid, "status", "definition CANDIDATE_READY, review pending, execution NOT_ATTEMPTED",
            (e.get("definition_status"), e.get("architect_review_status"), e.get("execution_status")) ==
            ("CANDIDATE_READY", "pending", "NOT_ATTEMPTED"))
        if len(row) == 3:
            out(cid, "property", "property and configuration equal the contract row",
                e["configuration"] == row[2] and " ".join(e["named_property"]) ==
                " ".join(re.findall(r"[PEL]\d+|D1", row[1])))
        if cid in MISSION6:
            f, props, conf = MISSION6[cid]
            out(cid, "property", "file, property and configuration equal mission section 6",
                (e["mutation_source_file"], " ".join(e["named_property"]), e["configuration"]) == (f, props, conf))
        else:
            out(cid, "property", "a section-5 carried-forward id", cid in FOURTEEN and
                e.get("definition_source") == "CARRIED_FORWARD")
        f = e["mutation_source_file"]
        if f not in base:
            out(cid, "source", "source file is protocol.rs or model_tests.rs at C", False, f)
            continue
        btext = base[f].decode()
        cnts = [(btext.count(x["anchor"]), x["expected_count"]) for x in e["edits"]]
        starts = [btext.find(x["anchor"]) for x in e["edits"]]
        out(cid, "anchor", "every anchor's count at C equals its expected cardinality 1 and starts a line",
            all(a == b == 1 for a, b in cnts) and all(s0 == 0 or btext[s0 - 1] == "\n" for s0 in starts), cnts)
        exp = btext
        for x in e["edits"]:
            exp = exp.replace(x["anchor"], x["replacement"], 1)
        exp = exp.encode()
        pdata = open(os.path.join(catdir, "patches", f"{cid}.patch"), "rb").read()
        out(cid, "patch", "patch file sha256 and text as catalogued", h256(pdata) == e["patch_sha256"] and
            pdata.decode() == e["patch"] and e["patch_file"] == f"patches/{cid}.patch")
        if cid in FOURTEEN:
            out(cid, "carried", "patch, anchor, replacement and mutant blob equal the blocked catalog's",
                h256(pdata) == blocked[cid]["patch_sha256"] and e["edits"][0]["anchor"] == blocked[cid]["anchor"]
                and e["edits"][0]["replacement"] == blocked[cid]["replacement"] and
                e["mutant_blob"] == blocked[cid]["mutant_blob"])
        try:
            pf = parse_patch(pdata)
        except Exception as ex:  # noqa: BLE001
            out(cid, "patch", "patch parses", False, ex)
            continue
        names = [(x["old"], x["new"]) for x in pf]
        out(cid, "single-file", "exactly one file section, a/ and b/ of the named source file",
            names == [("a/" + f, "b/" + f)] and pf[0]["diff"] == f"diff --git a/{f} b/{f}")
        if len(pf) != 1:
            continue
        hunks = pf[0]["hunks"]
        fwd = apply_strict(base[f], hunks)
        out(cid, "apply", "strict forward application (no fuzz, no offset) gives the edits' bytes",
            fwd is not None and fwd == exp)
        rev = apply_strict(exp, hunks, reverse=True)
        out(cid, "restoration", "strict reverse application to the recorded mutant gives C's blob",
            rev is not None and oid(rev) == EXPECT_BLOB[f] == e["restoration"]["restores_blob"])
        out(cid, "identity", "mutant blob and sha256 as catalogued", fwd is not None and oid(fwd) ==
            e["mutant_blob"] and h256(fwd) == e["mutant_sha256"])
        out(cid, "distinct", "patch and mutant distinct from all earlier controls",
            e["patch_sha256"] not in patches and (f, e["mutant_blob"]) not in mblobs)
        patches.add(e["patch_sha256"])
        mblobs.add((f, e["mutant_blob"]))
        if fwd is not None:
            mutated[cid] = (f, fwd, hunks)
    r1_compare(cat, catdir, base)
    print("== section 6 (token level)")
    btok = {f: tokens(base[f].decode()) for f in (P, MT)}
    bits = {f: items(btok[f]) for f in btok}
    for cid in MISSION6:
        if cid not in mutated:
            out(cid, "semantic", "mutant available", False)
            continue
        f, mbytes, hunks = mutated[cid]
        mtok = tokens(mbytes.decode())
        mits = items(mtok)
        keys = {k for k in bits[f] if k[1] is not None or k[0].startswith("impl Drop")}
        bi = {k: get(btok[f], bits[f], k) for k in keys}
        mi = {k: get(mtok, mits, k) for k in keys}
        named = ITEMS[cid]
        miss = [k for k in named if bi.get(k) is None or mi.get(k) is None]
        out(cid, "semantic", "named items found once in C and in the mutant", not miss, miss)
        if miss:
            continue
        ranges = [(btok[f][bits[f][k][0][0]][2], btok[f][bits[f][k][0][1]][2]) for k in named]
        lines = changed_base_lines(hunks)
        out(cid, "confine-item", "every changed base line lies inside a named item",
            bool(lines) and all(any(a <= ln <= z for a, z in ranges) for ln in lines))

        def outside(tk, its):
            spans = sorted(its[k][0] for k in named)
            res, pos = [], 0
            for a, z in spans:
                res += kv(tk[pos:a])
                pos = z + 1
            return res + kv(tk[pos:])
        out(cid, "confine-item", "tokens outside the named items identical", outside(btok[f], bits[f]) ==
            outside(mtok, mits))
        for res in transform(cid, bi, mi, base[f], mbytes):
            out(cid, res[2] if len(res) == 3 else "semantic", res[0], res[1])
    print("== tests, receipts, special rules")
    mtt = tokens(base[MT].decode())
    utt = tokens(base[UT].decode())
    vals = [t[1] for t in mtt]
    confs = {}
    for i in range(len(vals) - 10):
        if vals[i] == "const" and vals[i + 2] == ":" and vals[i + 3] == "Config":
            j = vals.index("name", i)
            confs[vals[i + 1]] = mtt[j + 2][1].strip('"')
    tests = {}
    for i in range(len(vals) - 6):
        if vals[i:i + 4] == ["#", "[", "test", "]"]:
            j, ign = i + 4, False
            if vals[j:j + 3] == ["#", "[", "ignore"]:
                ign = True
                j = vals.index("]", j) + 1
            if vals[j] == "fn":
                bb = body_start(mtt, j)
                body = vals[bb:close_of(mtt, bb) + 1]
                tests[vals[j + 1]] = (ign, body)
    unit_fns = {utt[i + 1][1] for i in range(len(utt) - 1) if utt[i][1] == "fn"}
    lst = g("cat-file", "-p", f"{EVID}:docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/"
            "20-list-all/raw-output.txt").decode()
    r4 = open(os.path.join(root, "inputs", "AD1-R4.md")).read()
    rows = {"a": "| (a) | original-explorer mutant FAIL receipt |", "b": "| (b) | named non-explorer mutant-test FAIL "
            "receipt |", "d": "| (d) | C22's intentional mutant compile-failure receipt |",
            "e1": "| (e1) | unchanged-core exploration PASS |", "e2": "| (e2) | restored named-unit-test PASS |",
            "e4": "| (e4) | healthy-build success |"}
    out("catalog", "receipt", "AD-1-R4 F.1 defines types (a), (b), (d), (e1), (e2), (e4)",
        all(v in r4 for v in rows.values()))
    for e in kill:
        cid, name = e["id"], (e.get("test") or "::").rsplit("::", 1)[1]
        if e["kind"] == "explore":
            ign, body = tests.get(name, (None, []))
            cname = next((body[i + 1] for i in range(len(body) - 1) if body[i] in ("&", "..") and
                          body[i + 1] in confs), None)
            out(cid, "test", f"{name}: #[test] at C, ignore status as catalogued, runs {e['configuration']}'s "
                "constant, listed in the pinned executable", name in tests and ign == e["ignored_at_C"] and
                confs.get(cname) == e["configuration"] and f"{e['test']}: test" in lst)
            pair = ("(a)", "(e1)")
        else:
            out(cid, "test", f"{name}: unit test fn at C, listed", name in unit_fns and f"{e['test']}: test" in lst)
            pair = ("(b)", "(e2)") if e["kind"] == "unit" else ("(d)", "(e4)")
        rt = e["receipt_type"]
        out(cid, "receipt", f"mutant receipt {pair[0]}, restored {pair[1]}", rt.startswith(pair[0]) and pair[1] in rt
            and not re.search(r"quotient|symmetr", (e.get("test") or "") + rt, re.I))
        out(cid, "quotient", "never a quotient: not a gc_symmetry/quotient/experimental test",
            not re.search(r"gc_symmetry|quotient|experimental", e.get("test") or ""))
    byid = {e["id"]: e for e in ents}
    c7 = byid["C7"].get("additional_comparison") or {}
    out("C7", "receipt", "F-3 comparison without A4: original explorer full_f3, not required to kill, same patch",
        c7.get("configuration") == "F-3" and c7.get("required_to_kill") is False and
        c7.get("test", "").endswith("::full_f3") and "same" in c7.get("patch", ""))
    c15 = byid["C15"]
    out("C15", "receipt", "AD-1-R5: mutant on original full_fr at 30,000,000; restored side = discharge record only "
        "after verified restoration", c15["test"].endswith("::full_fr") and c15["ceiling_states"] == 30000000 and
        "only after verified restoration" in c15["restoration"]["restored_side"] and
        c15["restored_receipt"].get("ad1r5_sha256") == HASHES["AD1-R5.md"])
    c22 = byid["C22"]["restored_receipt"]
    rec = g("cat-file", "-p", f"{STAGE_ONE}:docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/"
            "10-build/RECEIPT.txt").decode()
    cmd = re.findall(r"cargo test -p nexus-governed-control[^\n`]*", rec)
    out("C22", "c22", "the Stage-1 build command (as recorded) has no -j 2", bool(cmd) and
        not any(re.search(r"(^|\s)-j\s*2\b", c) for c in cmd), cmd[:1])
    out("C22", "c22", "C22 restored e4: PENDING_EXACT_COMMAND_HEALTHY_BUILD, exact future command, not run, Stage-1 "
        "build not cited as the receipt", c22.get("type") == "e4" and
        c22.get("restored_receipt_status") == "PENDING_EXACT_COMMAND_HEALTHY_BUILD" and
        c22.get("required_future_healthy_command", "").endswith("--lib --no-run -j 2") and
        c22.get("required_future_healthy_command") == c22.get("required_future_mutant_command") and
        c22.get("not_run_in_this_mission") is True and "directory" not in c22 and "status" not in c22)
    for e in kill:
        rr = e["restored_receipt"]
        if "directory" in rr:
            q = subprocess.run(["git", "-C", GIT, "cat-file", "-p",
                                f"{rr.get('commit')}:{rr['directory']}/CLASSIFICATION.txt"], capture_output=True)
            cls = q.stdout.decode() if q.returncode == 0 else ""
            out(e["id"], "restored", f"restored e1 receipt {rr['directory'].rsplit('/', 1)[1]} at "
                f"{str(rr.get('commit'))[:8]} is CLASSIFICATION: PASS", cls.startswith("CLASSIFICATION: PASS\n") and
                rr.get("type") == "e1", "" if q.returncode == 0 else "no CLASSIFICATION.txt there")
        else:
            out(e["id"], "restored", "restored side stated", bool(e["restoration"]["restored_side"]))
    verdict = {}
    for cid, ok in log:
        verdict[cid] = verdict.get(cid, True) and ok
    bad = [x for x in log if not x[1]]
    print()
    print(f"checks {len(log)}; failing {len(bad)}")
    print("CHECKER B: " + ("PASS" if not bad else "FAIL"))
    print("VERDICTS " + json.dumps({k: ("PASS" if v else "FAIL") for k, v in sorted(verdict.items())},
                                   sort_keys=True))
    return 0 if not bad else 1


if __name__ == "__main__":
    r = sys.argv[1]
    try:
        sys.exit(main(r, sys.argv[2] if len(sys.argv) > 2 else os.path.join(r, "out", "catalog")))
    except Exception as ex:  # noqa: BLE001 — a crash is reported as a failing check, never as a pass
        out("catalog", "crash", f"checker B raised {type(ex).__name__}: {ex}", False)
        v = {}
        for row in log:
            v[row[0]] = v.get(row[0], True) and row[-1]
        print("CHECKER: FAIL (crash)")
        print("VERDICTS " + json.dumps({k: ("PASS" if x else "FAIL") for k, x in sorted(v.items())}, sort_keys=True))
        sys.exit(1)
