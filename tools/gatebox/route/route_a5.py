#!/usr/bin/python3 -I
"""SB-3E A5 (design section 5 row A5, sections 4.1, 4.2 and 4.4; U4b item 5): route source.

A parser over the route packages' source (route_seams.json `route_sources`),
stdlib only, with its reviewed data in route_a5.json. Comments and literals
are lexed out; an attribute applies to the item, statement, field or arm it
precedes, and a `mod` declaration's attributes apply to the module's file.

  check --repo R --toolchain T
      cfg:      every cfg naming the route cfg (attribute, cfg_attr, cfg!) is
                exactly `#[cfg(mother_route_seams)]`, or the allowlisted
                `#[cfg(any(test, mother_route_seams))]` at a listed item;
      names:    every occurrence of a name the seams declare (an item, field
                or module declared under the route cfg) lies under exactly
                that cfg;
      writers:  `route_seams::at`/`note` calls are exactly the reviewed 4.4
                sites (file, writer, first argument, enclosing fn), each a
                whole statement under the cfg; no import of a writer;
      signature: each writer's `fn` is exactly its reviewed signature, once;
      markers:  each seam body named in the data begins with its marker
                (`const M` and `std::hint::black_box(M.as_bytes())`);
      hooks:    each reviewed test hook (a name, or `file|name`) occurs only
                under a cfg that holds only in tests (none widened; R47.4:
                `test`, `all` with one such, `any` of all such; never `not`);
      includes: no `#[path]` or `include!`-family path into a `tests`
                directory, outside code under exactly `#[cfg(test)]`;
      targets:  the route packages' Cargo targets equal the reviewed list.
  controls --repo R --toolchain T --work DIR
      The plants (mission U4b item 5), each applied to the source as read (or,
      for the Cargo target, to a scratch copy) and restored by hash; every
      plant must be reported by its own check, in a finding with its `expect`.
  list --repo R
      Review aid: the writer sites and seam-declared names this tree has.

Exit 0 on pass, 1 on a finding; one JSON verdict on stdout.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

DATA = "tools/gatebox/route/route_a5.json"
SEAMS = "tools/gatebox/route/route_seams.json"
CFG = "mother_route_seams"
ROUTE, BROAD, TEST = f"#[cfg({CFG})]", f"#[cfg(any(test,{CFG}))]", "#[cfg(test)]"
ITEMS = {"fn", "impl", "mod", "struct", "enum", "trait", "union", "unsafe", "extern", "async", "macro_rules",
         "const", "static", "use", "type", "let"}
ENDS_AT_SEMICOLON = {"const", "static", "use", "type", "let"}
CONTINUE = {"else", "=", ".", "?", "as", "&&", "||", "+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">="}
LEX = re.compile(r"""(?P<ws>\s+)|(?P<line>//[^\n]*)
    |(?P<raw>b?r(?P<h>\#*)"(?:.|\n)*?"(?P=h))|(?P<str>b?"(?:\\.|[^"\\])*")
    |(?P<chr>b?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^'\\\n])')|(?P<lt>'[A-Za-z_]\w*)
    |(?P<id>(?:r\#)?[A-Za-z_]\w*)|(?P<num>\d[\w.]*)
    |(?P<p>::|->|=>|&&|\|\||==|!=|<=|>=|\.\.=|\.\.|.)""", re.X | re.S)


def lex(text):
    """Tokens as (text, kind, offset); comments (nested blocks too) and whitespace dropped."""
    out, at = [], 0
    while at < len(text):
        if text.startswith("/*", at):
            depth, at = 1, at + 2
            while depth and at < len(text):
                depth += text.startswith("/*", at) - text.startswith("*/", at)
                at += 2 if text.startswith(("/*", "*/"), at) else 1
            continue
        match = LEX.match(text, at)
        kind = match.lastgroup if match.lastgroup != "h" else "raw"
        if kind not in ("ws", "line"):
            out.append((match.group(), "str" if kind in ("raw", "str") else kind, at))
        at = match.end()
    return out


class Source:
    """One file: tokens, attributes and the token span each attribute governs."""

    def __init__(self, rel, text):
        self.rel, self.text, self.toks = rel, text, lex(text)
        self.t = [x[0] for x in self.toks]
        self.match = self.brackets()
        self.attrs = []  # (normalized, first token, attribute end, governed span start, end)
        i = 0
        while i < len(self.t):
            if self.t[i] == "#" and i + 1 < len(self.t) and self.t[i + 1] in ("[", "!"):
                open_at = i + 1 if self.t[i + 1] == "[" else i + 2
                close = self.match.get(open_at, open_at)
                norm = "".join(self.t[i:close + 1])
                span = (0, len(self.t)) if self.t[i + 1] == "!" else (close + 1, self.extent(close + 1))
                self.attrs.append((norm, i, close + 1) + span)
                i = close + 1
                continue
            i += 1

    def brackets(self):
        stack, match = [], {}
        for i, tok in enumerate(self.t):
            if tok in "([{" and self.toks[i][1] == "p":
                stack.append(i)
            elif tok in ")]}" and self.toks[i][1] == "p" and stack:
                match[stack.pop()] = i
        return match

    def extent(self, k):
        """The end (exclusive) of the item, statement, field or arm starting at token k."""
        t, n = self.t, len(self.t)
        while k + 1 < n and t[k] == "#" and t[k + 1] == "[":
            k = self.match.get(k + 1, k + 1) + 1
        j = k
        if j < n and t[j] == "pub":
            j += 1 + (j + 1 < n and t[j + 1] == "(")
            if t[j - 1] == "(":
                j = self.match.get(j - 1, j) + 1
        item = j < n and t[j] in ITEMS and not (t[j] == "unsafe" and j + 1 < n and t[j + 1] == "{")
        i = k
        while i < n:
            if t[i] in "([{" and self.toks[i][1] == "p":
                close = self.match.get(i, n - 1)
                if t[i] == "{" and not (item and t[j] in ENDS_AT_SEMICOLON):
                    nxt = t[close + 1] if close + 1 < n else ""
                    if item or (nxt not in CONTINUE and nxt != ","):
                        return close + 1
                    if nxt == "," and not item:
                        return close + 2
                i = close + 1
                continue
            if t[i] in ")]}":
                return i
            if t[i] == ";" or (t[i] == "," and not item):
                return i + 1
            i += 1
        return n

    def governed(self, i, norm):
        """Whether token i lies in a span governed by an attribute normalizing to norm."""
        return any(a[0] == norm and a[3] <= i < a[4] for a in self.attrs)

    def fns(self):
        """(name, body open, body close, `fn` token) for every fn with a body."""
        out = []
        for i, tok in enumerate(self.t[:-1]):
            if tok == "fn" and self.toks[i + 1][1] == "id":
                j = i
                while j < len(self.t) and self.t[j] not in ("{", ";"):
                    j = self.match.get(j, j) + 1 if self.t[j] in "([" else j + 1
                if j < len(self.t) and self.t[j] == "{":
                    out.append((self.t[i + 1], j, self.match.get(j, len(self.t)), i))
        return out

    def enclosing_fn(self, i):
        inner = [f for f in self.fns() if f[1] < i < f[2]]
        return min(inner, key=lambda f: f[2] - f[1])[0] if inner else "-"


def read_sources(repo, roots):
    files = {}
    for root in roots:
        for base, dirs, names in os.walk(os.path.join(repo, root)):
            dirs.sort()
            for name in sorted(names):
                if name.endswith(".rs"):
                    path = os.path.join(base, name)
                    files[os.path.relpath(path, repo)] = open(path, encoding="utf-8").read()
    return files


def module_file(rel, name, files, path_attr=None):
    folder = os.path.dirname(rel)
    if path_attr:
        return os.path.normpath(os.path.join(folder, path_attr))
    if os.path.basename(rel) not in ("lib.rs", "main.rs", "mod.rs"):
        folder = os.path.join(folder, os.path.basename(rel)[:-3])
    first = os.path.join(folder, name + ".rs")
    return first if first in files else os.path.join(folder, name, "mod.rs")


def file_gates(parsed):
    """Each file's inherited gates: the normalized attributes on its `mod` declaration chain."""
    parent = {}
    for rel, src in parsed.items():
        for i, tok in enumerate(src.t[:-2]):
            if tok == "mod" and src.toks[i + 1][1] == "id" and src.t[i + 2] == ";":
                own = [a for a in src.attrs if a[3] <= i < a[4]]
                path = next((src.t[a[1] + 4][1:-1] for a in own if src.t[a[1] + 2] == "path"), None)
                child = module_file(rel, src.t[i + 1], parsed, path)
                parent[child] = (rel, {a[0] for a in own})
    gates = {}
    for rel in parsed:
        seen, cur, acc = set(), rel, set()
        while cur in parent and cur not in seen:
            seen.add(cur)
            cur, own = parent[cur]
            acc |= own
        gates[rel] = acc
    return gates


def under(src, gates, i, norm):
    return norm in gates[src.rel] or src.governed(i, norm)


def implies_test(norm):
    """Whether a normalized `#[cfg(..)]` holds only under `test` (SB3E-R47.4, recursive): `test`,
    an `all` with at least one such argument, or an `any` whose every argument is such; `not(..)`
    and anything else never (fail closed). `#[cfg_attr(..)]` and inner `#![..]` are not gates."""
    if not (norm.startswith("#[cfg(") and norm.endswith(")]")):
        return False
    def holds(pred):
        if pred == "test":
            return True
        for op, need in (("all(", any), ("any(", all)):
            if pred.startswith(op) and pred.endswith(")"):
                args, depth, cur = [], 0, ""
                for ch in pred[len(op):-1]:
                    depth += (ch == "(") - (ch == ")")
                    if ch == "," and depth == 0:
                        args.append(cur)
                        cur = ""
                    else:
                        cur += ch
                args = [a for a in args + [cur] if a]
                return bool(args) and need(holds(a) for a in args)
        return False
    return holds(norm[len("#[cfg("):-2])


def test_only(src, gates, i):
    return any(implies_test(g) for g in gates[src.rel]) or any(
        implies_test(a[0]) and a[3] <= i < a[4] for a in src.attrs)


def seam_names(parsed, gates):
    """Names the seams declare: items and fields declared under exactly the route cfg (outside
    whole gated files), and the gated modules; minus any also declared ungated."""
    names, ungated = set(), set()
    for rel, src in parsed.items():
        for i, tok in enumerate(src.t[:-1]):
            decl = (tok in ("fn", "struct", "enum", "mod", "const", "static", "type", "trait")
                    and src.toks[i + 1][1] == "id")
            field = (src.toks[i][1] == "id" and src.t[i + 1] == ":" and i > 0 and src.t[i - 1] in (",", "{", "]"))
            if not (decl or field):
                continue
            name = src.t[i + 1] if decl else tok
            if ROUTE in gates[rel]:
                continue
            (names if src.governed(i, ROUTE) else ungated).add(name)
    return sorted(names - ungated)


def writer_calls(src):
    """(token index, writer, first-argument tokens) for every `route_seams::at|note(` call."""
    out = []
    for i in range(len(src.t) - 3):
        if src.t[i] == "route_seams" and src.t[i + 1] == "::" and src.t[i + 2] in ("at", "note") \
                and src.t[i + 3] == "(":
            end = src.match.get(i + 3, i + 3)
            arg, depth = [], 0
            for tok in src.t[i + 4:end]:
                if tok == "," and depth == 0:
                    break
                depth += (tok in "([{") - (tok in ")]}")
                arg.append(tok)
            out.append((i, src.t[i + 2], "".join(arg)))
    return out


def check(repo, toolchain, files=None, cargo_meta=None):
    data = json.load(open(os.path.join(repo, DATA), encoding="utf-8"))
    seams = json.load(open(os.path.join(repo, SEAMS), encoding="utf-8"))
    files = files if files is not None else read_sources(repo, seams["route_sources"])
    parsed = {rel: Source(rel, text) for rel, text in files.items()}
    gates, out = file_gates(parsed), []
    allowed = {tuple(x.split("|")) for x in data["broad_allowlist"]}
    for rel, src in parsed.items():
        for norm, first, end, start, stop in src.attrs:
            if CFG in norm and norm != ROUTE:
                item = "".join(src.t[start:min(stop, start + 6)])
                if not (norm == BROAD and (rel, item) in allowed):
                    out.append({"check": "cfg", "file": rel, "attribute": norm, "item": item})
        for i, tok in enumerate(src.t[:-2]):
            if tok in ("cfg", "cfg_attr") and src.t[i + 1] == "!" and CFG in "".join(src.t[i:src.match.get(i + 2, i)]):
                out.append({"check": "cfg", "file": rel, "macro": "".join(src.t[i:src.match.get(i + 2, i) + 1])})
    names = set(seam_names(parsed, gates)) | set(data["seam_modules"])
    if sorted(names) != sorted(data["seam_names"]):
        out.append({"check": "names", "reason": "seam-declared names differ", "found": sorted(names)})
    for rel, src in parsed.items():
        for i, tok in enumerate(src.t):
            if tok in names and src.toks[i][1] == "id" and not under(src, gates, i, ROUTE) \
                    and not (under(src, gates, i, BROAD) and ROUTE not in gates[rel]):
                out.append({"check": "names", "file": rel, "name": tok, "offset": src.toks[i][2]})
    rows = []
    for rel, src in parsed.items():
        for i, writer, arg in writer_calls(src):
            rows.append(f"{rel}|{writer}|{arg}|{src.enclosing_fn(i)}")
            start = i - 2 if src.t[i - 2:i] == ["crate", "::"] else i
            start = start - 4 if src.t[start - 4:start] == ["crate", "::", "broker", "::"] else start
            end = src.match.get(i + 3, i + 3)
            if src.t[start - 1] not in (";", "{", "}", "]") or end + 1 >= len(src.t) or src.t[end + 1] != ";":
                out.append({"check": "writers", "file": rel, "reason": "not a whole statement", "site": rows[-1]})
        for i, tok in enumerate(src.t):
            if tok == "use":
                stop = src.extent(i)
                body = src.t[i:stop]
                for j, x in enumerate(body[:-2]):
                    if x == "route_seams" and body[j + 1] == "::" and ({"at", "note", "*"} & set(body[j + 2:])):
                        out.append({"check": "writers", "file": rel, "reason": "writer imported",
                                    "use": "".join(body)})
                        break
    if sorted(rows) != sorted(data["writers"]):
        out.append({"check": "writers", "reason": "sites differ", "extra": sorted(set(rows) - set(data["writers"])),
                    "missing": sorted(set(data["writers"]) - set(rows)),
                    "counts_equal": len(rows) == len(data["writers"])})
    for rel, sigs in data["signatures"].items():
        src = parsed.get(rel)
        for sig in sigs:
            name = sig.split("fn", 1)[1].split("(", 1)[0]
            found = [f for f in (src.fns() if src else []) if f[0] == name]
            heads = ["".join(src.t[max(0, f[3] - 6):f[1] + 1]) for f in found]
            if len(found) != 1 or not any(h.endswith(sig) for h in heads):
                out.append({"check": "signature", "file": rel, "expected": sig, "found": heads})
    markers = {s["name"]: s["marker"] for s in seams["seams"]}
    for row in data["marked"]:
        seam, rel, fn, scope = row.split("|")
        src, lit = parsed.get(rel), json.dumps(markers[seam])
        bb = ["std", "::", "hint", "::", "black_box", "(", "M", ".", "as_bytes", "(", ")", ")", ";"]
        prologue = (["const", "M", ":", "&", "str", "=", lit, ";"] if scope == "local" else []) + bb
        bodies = [f for f in (src.fns() if src else []) if f[0] == fn]
        good = [f for f in bodies if src.t[f[1] + 1:f[1] + 1 + len(prologue)] == prologue]
        module_ok = scope == "local" or (src and "".join(src.t).count(f"constM:&str={lit};") == 1)
        if len(good) != 1 or not module_ok:
            out.append({"check": "markers", "seam": seam, "file": rel, "fn": fn})
    for rel, src in parsed.items():
        for i, tok in enumerate(src.t):
            hook = tok in data["test_hooks"] or f"{rel}|{tok}" in data["test_hooks"]
            if hook and src.toks[i][1] == "id" and not test_only(src, gates, i):
                out.append({"check": "hooks", "file": rel, "hook": tok, "offset": src.toks[i][2]})
        for i, tok in enumerate(src.t[:-3]):
            target = None
            if tok == "path" and src.t[i - 2:i] == ["#", "["] and src.t[i + 1] == "=":
                target = src.t[i + 2]
            elif tok in ("include", "include_str", "include_bytes") and src.t[i + 1] == "!":
                target = src.t[i + 3]
            if target and target.startswith('"'):
                path = os.path.normpath(os.path.join(os.path.dirname(rel), target[1:-1]))
                if {"tests", "route_cells"} & set(path.split(os.sep)) and not under(src, gates, i, TEST):
                    out.append({"check": "includes", "file": rel, "path": path})
    targets = cargo_meta if cargo_meta is not None else cargo_targets(repo, toolchain, seams["route_packages"])
    if targets != data["targets"]:
        out.append({"check": "targets", "found": targets,
                    "extra": sorted(set(targets) - set(data["targets"])),
                    "missing": sorted(set(data["targets"]) - set(targets))})
    return out


def cargo_targets(repo, toolchain, packages):
    env = {"HOME": os.environ["HOME"], "PATH": os.path.join(toolchain, "bin") + ":/usr/bin:/bin", "LANG": "C.UTF-8"}
    run = subprocess.run([os.path.join(toolchain, "bin", "cargo"), "metadata", "--no-deps", "--locked", "--offline",
                          "--format-version", "1"], cwd=repo, env=env, capture_output=True, text=True)
    if run.returncode != 0:
        return ["metadata failed: " + run.stderr.strip()[-200:]]
    rows = []
    for p in json.loads(run.stdout)["packages"]:
        if p["name"] in packages:
            for t in p["targets"]:
                rows.append("|".join([p["name"], t["name"], ",".join(t["kind"]), ",".join(t["crate_types"]),
                                      os.path.relpath(t["src_path"], repo), ",".join(t.get("required-features", [])),
                                      str(t["doctest"]), str(t["test"]), t["edition"]]))
    return sorted(rows)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def controls(repo, toolchain, work):
    data = json.load(open(os.path.join(repo, DATA), encoding="utf-8"))
    seams = json.load(open(os.path.join(repo, SEAMS), encoding="utf-8"))
    files = read_sources(repo, seams["route_sources"])
    meta = cargo_targets(repo, toolchain, seams["route_packages"])
    base = check(repo, toolchain, dict(files), meta)
    results, findings = [], []
    if base:
        findings.append({"kind": "unplanted_tree_has_findings", "findings": base[:5]})
    for plant in data["plants"]:
        before = {rel: sha(text) for rel, text in files.items()}
        planted, meta_planted, row = dict(files), meta, {"name": plant["name"], "expect": plant["check"]}
        if plant["file"].endswith("Cargo.toml"):
            os.makedirs(work, exist_ok=True)
            tree = os.path.join(work, plant["name"])
            shutil.copytree(repo, tree, ignore=shutil.ignore_patterns("target", ".git"))
            path = os.path.join(tree, plant["file"])
            text = open(path, encoding="utf-8").read()
            row["unique"] = text.count(plant["old"]) == 1
            open(path, "w", encoding="utf-8").write(text.replace(plant["old"], plant["new"]))
            meta_planted = cargo_targets(tree, toolchain, seams["route_packages"])
            shutil.rmtree(tree)
        else:
            text = planted.get(plant["file"], "")
            row["unique"] = text.count(plant["old"]) == 1
            planted[plant["file"]] = text.replace(plant["old"], plant["new"])
            row["sha_planted"] = sha(planted[plant["file"]])
        got = check(repo, toolchain, planted, meta_planted)
        row["reported_by"] = sorted({f["check"] for f in got})
        expect = plant.get("expect") or {}
        row["matched"] = bool(expect) and any(all(f.get(k) == v for k, v in expect.items()) for f in got)
        if not row["matched"]:
            row["got"] = [f for f in got if f["check"] == plant["check"]][:5]
        row["restored"] = {rel: sha(text) for rel, text in files.items()} == before
        row["killed"] = (row["unique"] and row["restored"] and plant["check"] in row["reported_by"]
                         and row["matched"] and expect.get("check") == plant["check"])
        results.append(row)
        if not row["killed"]:
            findings.append({"kind": "plant_not_reported_by_its_check", "plant": plant["name"], "row": row})
    return findings, {"plants": results}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "controls", "list"))
    parser.add_argument("--repo", required=True)
    parser.add_argument("--toolchain")
    parser.add_argument("--work")
    args = parser.parse_args()
    repo = os.path.abspath(args.repo)
    if args.command == "list":
        seams = json.load(open(os.path.join(repo, SEAMS), encoding="utf-8"))
        parsed = {r: Source(r, t) for r, t in read_sources(repo, seams["route_sources"]).items()}
        rows = sorted(f"{r}|{w}|{a}|{s.enclosing_fn(i)}" for r, s in parsed.items() for i, w, a in writer_calls(s))
        print(json.dumps({"writers": rows, "seam_names": seam_names(parsed, file_gates(parsed))}, indent=1))
        return 0
    if not args.toolchain:
        sys.exit("route_a5: --toolchain is required")
    toolchain = os.path.abspath(args.toolchain)
    if args.command == "check":
        findings, out = check(repo, toolchain), {}
    else:
        work = os.path.abspath(args.work or "")
        if not args.work or os.path.lexists(work):
            sys.exit("route_a5: controls need a fresh --work directory")
        findings, out = controls(repo, toolchain, work)
    print(json.dumps(dict(out, verdict="fail" if findings else "pass", findings=findings), sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
