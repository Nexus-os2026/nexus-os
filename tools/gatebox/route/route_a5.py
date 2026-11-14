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
      includes: (R60.5, an accept-list) each include-family name is `name!`,
                one delimiter, one plain string literal; each attribute `path`
                is exactly `#[path = ".."]` on a `mod x;`; each target is a
                parsed file outside `tests`/`route_cells`, or the site is under
                exactly `#[cfg(test)]`, or it is an `includes_allow` row;
      subset:   a form outside the closed subset (Source.refused), a file two
                `mod` declarations name or no `mod` chain reaches from a Cargo
                target root, `#[path]` on an inline module (R55.1, R60.3), a
                macro invocation holding `mod`, `fn`, `#[`, `include*`, `path`,
                a hook or a seam name unless it is a `subset_allow` row (R60.2);
      gated:    each file's `#[cfg(mother_route_seams)]` spans, by the sha256
                of each with its context (R60.6), exactly the reviewed list;
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
# SB3E-R60.1: the lexer is an accept-list. Whitespace, punctuation, ASCII identifiers, numbers and the
# literals of the Rust reference (escapes exactly; `_` in `\u{..}`); any other character outside a
# literal or comment, a `'` or `"` that starts no literal, and a suffixed literal are refused.
UNI = r"u\{(?:[0-9a-fA-F]_*){1,6}\}"
ESC = r"\\(?:[nrt\\0'\"]|x[0-7][0-9a-fA-F]|" + UNI + ")"
BESC = r"\\(?:[nrt\\0'\"]|x[0-9a-fA-F]{2})"
LEX = re.compile(r"""(?P<ws>[ \t\n\r]+)|(?P<line>//[^\n]*)
    |(?P<raw>[bc]?r(?P<h>\#*)"(?:[^\r]|\r\n)*?"(?P=h))
    |(?P<str>c?"(?:[^"\\\r]|\r\n|ESC|\\\r?\n)*"|b"(?:[\x00-\x0c\x0e-\x21\x23-\x5b\x5d-\x7f]|\r\n|BESC|\\\r?\n)*")
    |(?P<chr>'(?:[^'\\\n\r\t]|ESC)'|b'(?:[\x00-\x08\x0b\x0c\x0e-\x26\x28-\x5b\x5d-\x7f]|BESC)')
    |(?P<lt>'(?:r\#)?[A-Za-z_][A-Za-z0-9_]*+(?!'))
    |(?P<id>(?:r\#)?[A-Za-z_][A-Za-z0-9_]*)|(?P<num>[0-9][0-9A-Za-z_]*(?:\.[0-9][0-9A-Za-z_]*)?)
    |(?P<p>::|->|=>|&&|\|\||==|!=|<=|>=|\.\.=|\.\.|[-+*/%^!&|=<>@.,;:\#$?~()\[\]{}])""".replace(
    "BESC", BESC).replace("ESC", ESC), re.X)


def lex(text):
    """(tokens as (text, kind, offset), refused forms as (form, offset)); comments (nested blocks too)
    and whitespace dropped. A raw identifier keeps its `r#` (SB3E-R60.2)."""
    out, bad, at = [], [], 0
    while at < len(text):
        if text.startswith("/*", at):
            depth, at = 1, at + 2
            while depth and at < len(text):
                depth += text.startswith("/*", at) - text.startswith("*/", at)
                at += 2 if text.startswith(("/*", "*/"), at) else 1
            continue
        match = LEX.match(text, at)
        if match is None:
            bad.append((f"character U+{ord(text[at]):04X}", at))
            at += 1
            continue
        kind, tok = match.lastgroup if match.lastgroup != "h" else "raw", match.group()
        if kind in ("raw", "str", "chr") and (re.match(r"\w", text[match.end():match.end() + 1])
                                             or tok.startswith("br") and not tok.isascii()):
            bad.append(("literal suffix or non-ASCII byte literal", at))
        if kind not in ("ws", "line"):
            out.append((tok, "str" if kind in ("raw", "str") else kind, at))
        at = match.end()
    return out, bad


# Rust 2021 strict and reserved keywords: never a macro's name unless raw (SB3E-R60.2).
KEYWORDS = set("as break const continue crate else enum extern false fn for if impl in let loop match mod move mut pub ref "
               "return self Self static struct super trait true type unsafe use where while async await dyn abstract become "
               "box do final macro override priv typeof unsized virtual yield try".split())
STARTS = {"(", "[", "&", "&&", "*", "-", "!", "|", "||", "::", "<", "..", "..="}  # an expression's first token
WORDS = {"return", "break", "continue", "move", "async", "self", "Self", "super", "crate", "true", "false"}


class Source:
    """One file: tokens, attributes and the token span each attribute governs. SB3E-R55.1/R60: a closed
    subset; `refused` lists each form outside it (a byte order mark, a shebang, the lexer's refusals, a `c`/`cr`
    literal, a cfg-bearing attribute inside a macro invocation, an attribute on a form `extent` does not
    classify), each a finding. An attribute inside a macro invocation gates nothing. `t` is each token as
    written (`r#if` is never the keyword); `n` is the name compared (raw identifiers normalized)."""

    def __init__(self, rel, text):
        self.rel, self.text, self.refused = rel, text, [("byte order mark", 0)] if text[:1] == "\ufeff" else []
        if text.startswith("#!") and not re.match(r"#!\s*\[", text):
            self.refused.append(("shebang", 0))
            text = re.sub(r"[^\n]", " ", text.split("\n", 1)[0]) + text[len(text.split("\n", 1)[0]):]
        self.toks, bad = lex(text)
        self.refused += bad
        self.t = [x[0] for x in self.toks]
        self.n = [x[0][2:] if x[1] == "id" and x[0].startswith("r#") else x[0] for x in self.toks]
        self.refused += [("c literal", x[2]) for x in self.toks if x[1] == "str" and x[0][0] == "c"]
        self.match, n, self._fns = self.brackets(), len(self.t), None
        self.back = {z: a for a, z in self.match.items()}
        self.macros = [(i, i + 2, self.match.get(i + 2, n)) for i in range(n - 2)  # (name, open, close)
                       if self.toks[i][1] == "id" and self.t[i] not in KEYWORDS and self.t[i + 1] == "!"
                       and self.t[i + 2] in "([{" and self.toks[i + 2][1] == "p"]
        self.macros += [(i, i + 3, self.match.get(i + 3, n)) for i in range(n - 3)
                        if self.t[i:i + 2] == ["macro_rules", "!"] and self.toks[i + 2][1] == "id" and self.t[i + 3] in "([{"]
        depth = [0] * (n + 1)
        for _, o, z in self.macros:
            depth[o + 1] += 1
            depth[min(z, n)] -= 1
        for k in range(1, n):
            depth[k] += depth[k - 1]
        self.inmac = [d > 0 for d in depth[:n]]
        self.attrs = []  # (normalized, first token, attribute end, governed span start, end)
        i = 0
        while i < n:
            if self.t[i] == "#" and i + 1 < n and self.t[i + 1] in ("[", "!"):
                open_at = i + 1 if self.t[i + 1] == "[" else i + 2
                close = self.match.get(open_at, open_at)
                norm = "".join(self.t[i:close + 1])
                if self.in_macro(i):
                    if re.match(r"#!?\[cfg(_attr)?\(", norm):
                        self.refused.append(("cfg in a macro", self.toks[i][2]))
                else:
                    span = self.inner(i) if self.t[i + 1] == "!" else (close + 1, self.extent(close + 1))
                    if self.t[open_at:open_at + 1] != ["["] or None in span:
                        self.refused.append(("unclassified attributed form", self.toks[i][2]))
                        span = (close + 1, close + 1)
                    self.attrs.append((norm, i, close + 1) + span)
                i = close + 1
                continue
            i += 1

    def in_macro(self, i):
        return self.inmac[i]

    def outer_macros(self):
        return [m for m in self.macros if not self.inmac[m[0]]]

    def inner(self, i):
        """SB3E-R60.4: an inner attribute governs the file (before any other token) or the block it opens."""
        p = i - 1
        while p >= 0 and self.t[p] == "]" and self.back.get(p, 0) >= 2 and self.t[self.back[p] - 2:self.back[p]] == ["#", "!"]:
            p = self.back[p] - 3
        if p < 0:
            return 0, len(self.t)
        return (p + 1, self.match.get(p)) if self.t[p] == "{" and self.toks[p][1] == "p" else (None, None)

    def after_vis(self, k):
        """The token after k's outer attributes and `pub`/`pub(..)`."""
        t, n = self.t, len(self.t)
        while k + 1 < n and t[k] == "#" and t[k + 1] == "[":
            k = self.match.get(k + 1, n - 1) + 1
        if k < n and t[k] == "pub":
            k += 1
            if k < n and t[k] == "(":
                k = self.match.get(k, n - 1) + 1
        return k

    def brackets(self):
        stack, match = [], {}
        for i, tok in enumerate(self.t):
            if tok in "([{" and self.toks[i][1] == "p":
                stack.append(i)
            elif tok in ")]}" and self.toks[i][1] == "p" and stack:
                match[stack.pop()] = i
        return match

    def extent(self, k):
        """SB3E-R60.4, an accept-list: the end (exclusive) of the classified form at token k, or None.
        Items (qualifiers `const`/`async`/`unsafe`/`extern ".."`) end at their first block or `;`;
        `use`/`type`/`static`/`let`/`const X` at `;`; a block-like statement at its block; an expression
        statement, field, variant or parameter at `;`/`,` or the enclosing close; an arm after its body."""
        t, n = self.t, len(self.t)
        j = h = self.after_vis(k)
        ext = False
        while h + 1 < n and t[h] in ("const", "async", "unsafe", "safe", "default", "extern") and (
                t[h] == "extern" or t[h + 1] not in ("{", "move", "|", "||")):
            ext = ext or t[h] == "extern"
            h += 2 if t[h] == "extern" and self.toks[h + 1][1] == "str" else 1
        if h < n and (t[h] in ("fn", "impl", "mod", "struct", "enum", "trait") or ext and t[h] in ("{", "crate")
                      or t[h] == "union" and h + 1 < n and self.toks[h + 1][1] == "id" or t[h:h + 2] == ["macro_rules", "!"]):
            return self.scan(h, item=True)
        if self.blocklike(j):
            return self.block(j)
        if j < n and t[j] in ("use", "type", "static", "let", "const"):
            return self.scan(j, semi=True)
        if j < n and (self.toks[j][1] in ("str", "chr", "num") or self.toks[j][1] == "id" and t[j] not in KEYWORDS
                      or t[j] in WORDS or t[j] in STARTS and self.toks[j][1] == "p"):
            return self.scan(j)
        return None

    def scan(self, i, item=False, semi=False, arm=False):
        """From token i at this depth, brackets skipped: an item ends after its first block or at `;`; a
        `semi` form at `;`; anything else at `;` or `,` (included) or the enclosing close, or after the
        arm `=>` opens. None for any other end, or when `<` and `>` do not pair."""
        t, n, angle = self.t, len(self.t), 0
        plain = not (item or semi)
        while i < n:
            tok, punct = t[i], self.toks[i][1] == "p"
            if punct and tok in ("(", "[", "{"):
                if self.match.get(i) is None:
                    return None
                if tok == "{" and item:
                    return self.match[i] + 1
                i = self.match[i] + 1
                continue
            if punct and tok in (")", "]", "}"):
                return i if plain and not angle else None
            if tok == ";":
                return i + 1 if not angle else None
            if plain and tok == "," and not angle:
                return i + 1
            if plain and tok == "=>":
                return None if arm or angle else self.arm(i + 1)
            if plain:
                angle += (tok == "<") - (tok == ">")
                if angle < 0:
                    return None
            i += 1
        return None

    def arm(self, b):
        """After an arm's `=>`: a block or block-like body ends at its `}` (and its `,`) unless `.` or
        `?` continues it; any other body at `,` or the match's close."""
        if b < len(self.t) and self.blocklike(b):
            end = self.block(b, arm=True)
            return end + 1 if end is not None and end < len(self.t) and self.t[end] == "," else end
        return self.scan(b, arm=True)

    def blocklike(self, j):
        """`{`, `if`, `match`, `loop`, `while`, `for`, `unsafe {`, `const {`, a label, or a braced macro."""
        t, n = self.t, len(self.t)
        if j >= n:
            return False
        if t[j] in ("{", "if", "match", "loop", "while", "for") or t[j] in ("unsafe", "const") and t[j + 1:j + 2] == ["{"]:
            return True
        if self.toks[j][1] == "lt":
            return t[j + 1:j + 2] == [":"]
        p = j + (t[j] == "::")
        while p + 2 < n and self.toks[p][1] == "id" and t[p + 1] == "::":
            p += 2
        return p + 2 < n and self.toks[p][1] == "id" and t[p + 1] == "!" and t[p + 2] == "{"

    def block(self, j, arm=False):
        """A block-like form's end: after its block's `}`, past `else` blocks, or continued by `.`/`?`.
        A `let` or `for` pattern (which may hold braces) runs to its `=` or `in`."""
        t, n, i, pattern = self.t, len(self.t), j, None
        while i < n:
            if t[i] in ("let", "for") or t[i] == pattern:
                pattern = {"let": "=", "for": "in"}.get(t[i])
            elif t[i] == "{" and self.toks[i][1] == "p" and pattern:
                i = self.match.get(i, n) + 1
                continue
            if t[i] == "{" and self.toks[i][1] == "p":
                close = self.match.get(i)
                if close is None:
                    return None
                nxt = t[close + 1] if close + 1 < n else ""
                if nxt == "else":
                    i = close + 2
                    continue
                return self.scan(close + 1, arm=arm) if nxt in (".", "?") else close + 1
            if t[i] in ("(", "[") and self.toks[i][1] == "p":
                i = self.match.get(i, n) + 1
                continue
            if t[i] in (";", ",", ")", "]", "}", "=>"):
                return None
            i += 1
        return None

    def governed(self, i, norm):
        """Whether token i lies in a span governed by an attribute normalizing to norm."""
        return any(a[0] == norm and a[3] <= i < a[4] for a in self.attrs)

    def fns(self):
        """(name, body open, body close, `fn` token) for every fn with a body, outside macros."""
        if self._fns is not None:
            return self._fns
        out = []
        for i, tok in enumerate(self.t[:-1]):
            if tok == "fn" and self.toks[i + 1][1] == "id" and not self.in_macro(i):
                j = i
                while j < len(self.t) and self.t[j] not in ("{", ";"):
                    j = self.match.get(j, j) + 1 if self.t[j] in "([" else j + 1
                if j < len(self.t) and self.t[j] == "{":
                    out.append((self.n[i + 1], j, self.match.get(j, len(self.t)), i))
        self._fns = out
        return out

    def inline(self, i):
        """The inline modules (`mod y { }`, outermost first) around token i."""
        return [self.n[b + 1] for b in range(len(self.t) - 2) if self.t[b] == "mod" and self.toks[b + 1][1] == "id"
                and self.t[b + 2] == "{" and b < i < self.match.get(b + 2, b) and not self.in_macro(b)]

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


def module_file(rel, name, files, path_attr=None, inline=()):
    """The file of `mod name;` in rel, inside the inline modules `inline` (outermost first)."""
    folder = os.path.dirname(rel)
    if os.path.basename(rel) not in ("lib.rs", "main.rs", "mod.rs") and (inline or not path_attr):
        folder = os.path.join(folder, os.path.basename(rel)[:-3])
    folder = os.path.join(folder, *inline)
    if path_attr:
        return os.path.normpath(os.path.join(folder, path_attr))
    first = os.path.join(folder, name + ".rs")
    return first if first in files else os.path.join(folder, name, "mod.rs")


def mod_child(src, i, files):
    """The file of the `mod x;` at token i (its `#[path]`, its inline modules) and the attributes over it."""
    own = [a for a in src.attrs if a[3] <= i < a[4]]
    path = next((unquote(src.toks[a[1] + 4]) for a in own if src.n[a[1] + 2:a[1] + 4] == ["path", "="]), None)
    return module_file(src.rel, src.n[i + 1], files, path, src.inline(i)), {a[0] for a in own}


def file_gates(parsed, out=None, roots=None):
    """Each file's inherited gates: the normalized attributes on its `mod` declaration chain. A
    `mod x;` inside inline `mod y { }` is y/x.rs; a file declared twice is a finding (R55.1); so are
    (R60.3) `#[path]` on an inline module and a file no `mod` chain reaches from a Cargo target root."""
    parent = {}
    for rel, src in parsed.items():
        for i, tok in enumerate(src.t[:-2]):
            if tok == "mod" and src.toks[i + 1][1] == "id" and src.t[i + 2] == ";" and not src.in_macro(i):
                child, own = mod_child(src, i, parsed)
                if child in parent and out is not None:
                    out.append({"check": "subset", "file": child, "form": "two parents"})
                parent[child] = (rel, own)
        for a in src.attrs if out is not None else ():
            k = src.after_vis(a[3])
            if "path" in src.n[a[1]:a[2]] and src.t[k:k + 1] == ["mod"] and src.t[k + 2:k + 3] == ["{"]:
                out.append({"check": "subset", "file": rel, "form": "path on an inline module"})
    if out is not None and roots is not None:
        reached, grew = set(roots) & set(parsed), True
        while grew:
            new = {c for c, (p, _) in parent.items() if p in reached and c in parsed} - reached
            grew, reached = bool(new), reached | new
        out += [{"check": "subset", "file": rel, "form": "no parent"} for rel in sorted(set(parsed) - reached)]
    gates = {}
    for rel in parsed:
        seen, cur, acc = set(), rel, set()
        while cur in parent and cur not in seen:
            seen.add(cur)
            cur, own = parent[cur]
            acc |= own
        gates[rel] = acc
    return gates


def unquote(tok):
    """A plain, raw or byte string literal's text; None for anything else or an escape."""
    m = re.fullmatch(r'b?(?:r(#*)"(.*)"\1|"([^"\\]*)")', tok[0], re.S) if tok[1] == "str" else None
    return None if m is None else m.group(2) if m.group(2) is not None else m.group(3)


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
            name = src.n[i + 1] if decl else src.n[i]
            if ROUTE in gates[rel]:
                continue
            (names if src.governed(i, ROUTE) else ungated).add(name)
    return sorted(names - ungated)


def gated_rows(src, files):
    """SB3E-R55.4/R60.6: per `#[cfg(mother_route_seams)]`, the sha256 of its enclosing fn, the two tokens
    before it, the tokens it governs and the two after; a gated `mod x;` adds its file's tokens' sha256."""
    rows = []
    for a in src.attrs:
        if a[0] == ROUTE:
            words = [src.enclosing_fn(a[1])] + src.t[max(0, a[1] - 2):a[1]] + ["|"] + src.t[a[3]:a[4]] + ["|"] + src.t[a[4]:a[4] + 2]
            k = src.after_vis(a[3])
            if src.t[k:k + 1] == ["mod"] and src.t[k + 2:k + 3] == [";"]:
                child = mod_child(src, k, files)[0]
                words.append(sha(" ".join(files[child].t)) if child in files else "no file")
            rows.append(sha(" ".join(words)))
    return rows


def writer_calls(src):
    """(token index, writer, first-argument tokens) for every `route_seams::at|note(` call."""
    out = []
    for i in range(len(src.t) - 3):
        if src.n[i] == "route_seams" and src.t[i + 1] == "::" and src.n[i + 2] in ("at", "note") \
                and src.t[i + 3] == "(":
            end = src.match.get(i + 3, i + 3)
            arg, depth = [], 0
            for tok in src.t[i + 4:end]:
                if tok == "," and depth == 0:
                    break
                depth += (tok in "([{") - (tok in ")]}")
                arg.append(tok)
            out.append((i, src.n[i + 2], "".join(arg)))
    return out


def check(repo, toolchain, files=None, cargo_meta=None):
    data = json.load(open(os.path.join(repo, DATA), encoding="utf-8"))
    seams = json.load(open(os.path.join(repo, SEAMS), encoding="utf-8"))
    files = files if files is not None else read_sources(repo, seams["route_sources"])
    parsed = {rel: Source(rel, text) for rel, text in files.items()}
    out = [{"check": "subset", "file": rel, "form": f, "offset": at} for rel, src in parsed.items()
           for f, at in src.refused]
    targets = cargo_meta if cargo_meta is not None else cargo_targets(repo, toolchain, seams["route_packages"])
    gates = file_gates(parsed, out, {row.split("|")[4] for row in targets if row.count("|") >= 4})
    gated = {rel: gated_rows(src, parsed) for rel, src in parsed.items()}
    for rel in sorted(set(gated) | set(data["gated"])):  # SB3E-R55.4: every gated span, exactly
        have, want = gated.get(rel, []), data["gated"].get(rel, [])
        if have != want:
            out.append({"check": "gated", "file": rel, "extra": [x for x in have if x not in want],
                        "missing": [x for x in want if x not in have]})
    allowed = {tuple(x.split("|")) for x in data["broad_allowlist"]}
    for rel, src in parsed.items():
        for norm, first, end, start, stop in src.attrs:
            if CFG in norm and norm != ROUTE:
                item = "".join(src.t[start:min(stop, start + 6)])
                if not (norm == BROAD and (rel, item) in allowed):
                    out.append({"check": "cfg", "file": rel, "attribute": norm, "item": item})
        for i, tok in enumerate(src.n[:-2]):
            if tok in ("cfg", "cfg_attr") and src.t[i + 1] == "!" and CFG in "".join(src.t[i:src.match.get(i + 2, i)]):
                out.append({"check": "cfg", "file": rel, "macro": "".join(src.t[i:src.match.get(i + 2, i) + 1])})
    names = set(seam_names(parsed, gates)) | set(data["seam_modules"])
    if sorted(names) != sorted(data["seam_names"]):
        out.append({"check": "names", "reason": "seam-declared names differ", "found": sorted(names)})
    for rel, src in parsed.items():
        for i, tok in enumerate(src.n):
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
                stop = src.extent(i) or len(src.t)
                body = src.n[i:stop]
                for j, x in enumerate(body[:-2]):
                    if x == "route_seams" and body[j + 1] == "::" and ({"at", "note", "*"} & set(body[j + 2:])):
                        out.append({"check": "writers", "file": rel, "reason": "writer imported",
                                    "use": "".join(src.t[i:stop])})
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
    hits = {}
    for rel, src in parsed.items():
        hooked = {h.split("|")[-1] for h in data["test_hooks"] if "|" not in h or h.startswith(rel + "|")}
        for i, tok in enumerate(src.n):
            if tok in hooked and src.toks[i][1] == "id" and not test_only(src, gates, i):
                out.append({"check": "hooks", "file": rel, "hook": tok, "offset": src.toks[i][2]})
        # SB3E-R60.2: in a macro's delimiters, these tokens are refused unless the invocation is allowed.
        for start, open_at, close in src.outer_macros():
            forms = sorted({src.n[i] if src.n[i] != "#" else "#[" for i in range(open_at + 1, close)
                            if src.n[i] in MACRO_WORDS | hooked | names and src.toks[i][1] == "id"
                            or src.t[i] == "#" and src.t[i + 1:i + 2] in (["["], ["!"])})
            if forms:
                hits.setdefault(rel, []).append((sha(" ".join(src.t[start:close + 1])), forms, src.toks[start][2]))
        # SB3E-R60.5, an accept-list: each include-family name is `name!` + one delimiter + one plain
        # string literal; each `path` in an attribute is exactly `#[path = ".."]` on a `mod x;`. A target
        # must be a parsed file outside `tests`/`route_cells`, or the site lies under exactly `#[cfg(test)]`.
        sites = []
        for i, tok in enumerate(src.n):
            if tok in INCLUDES and src.toks[i][1] == "id":
                o = i + 2
                if src.t[i + 1:o] == ["!"] and src.t[o:o + 1] in (["("], ["["], ["{"]) and src.match.get(o) == o + 2 \
                        and unquote(src.toks[o + 1]) is not None:
                    sites.append((i, os.path.normpath(os.path.join(os.path.dirname(rel), unquote(src.toks[o + 1])))))
                else:
                    out.append({"check": "includes", "file": rel, "reason": "not a plain string literal",
                                "offset": src.toks[i][2]})
        for a in src.attrs:
            for j in [j for j in range(a[1], a[2]) if src.n[j] == "path" and src.toks[j][1] == "id"]:
                k = src.after_vis(a[2])
                if j == a[1] + 2 and a[2] == a[1] + 6 and src.t[j + 1] == "=" and unquote(src.toks[j + 2]) is not None \
                        and src.t[k:k + 1] == ["mod"] and src.toks[k + 1][1] == "id" and src.t[k + 2:k + 3] == [";"]:
                    sites.append((j, mod_child(src, k, parsed)[0]))
                else:
                    lit = unquote(src.toks[j + 2]) if src.t[j + 1:j + 2] == ["="] and j + 2 < a[2] else None
                    out.append(dict({"check": "includes", "file": rel, "reason": "not `#[path]` on a `mod x;`",
                                     "offset": src.toks[j][2]}, **({"path": os.path.normpath(os.path.join(
                                         os.path.dirname(rel), lit))} if lit is not None else {})))
        for i, path in sites:
            if (path not in parsed or {"tests", "route_cells"} & set(path.split(os.sep))) and not under(src, gates, i, TEST) \
                    and f"{rel}|{path}" not in data["includes_allow"]:
                out.append({"check": "includes", "file": rel, "path": path})
    allow = data["subset_allow"]
    for rel in sorted(set(hits) | set(allow)):
        have = [h[0] for h in hits.get(rel, [])]
        out += [{"check": "subset", "file": rel, "form": "in a macro: " + ",".join(forms), "invocation": inv,
                 "offset": at} for inv, forms, at in hits.get(rel, []) if inv not in allow.get(rel, [])]
        out += [{"check": "subset", "file": rel, "form": "allow row matches no invocation", "invocation": inv}
                for inv in allow.get(rel, []) if inv not in have]
    if targets != data["targets"]:
        out.append({"check": "targets", "found": targets,
                    "extra": sorted(set(targets) - set(data["targets"])),
                    "missing": sorted(set(data["targets"]) - set(targets))})
    return out


MACRO_WORDS = {"mod", "fn", "include", "include_str", "include_bytes", "path"}
INCLUDES = ("include", "include_str", "include_bytes")


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
        else:  # the plant's edit, then each of its `also` edits (SB3E-R55.3)
            row["unique"] = True
            for e in [plant] + plant.get("also", []):
                text = planted.get(e["file"], "")
                row["unique"] &= text.count(e["old"]) == 1
                planted[e["file"]] = text.replace(e["old"], e["new"])
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
