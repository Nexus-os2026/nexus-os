#!/usr/bin/python3 -I
"""SB-3E A5 (design section 5 row A5, sections 4.1, 4.2 and 4.4; U4b item 5): route source.

A parser over the route packages' source (route_seams.json `route_sources`),
stdlib only, with its reviewed data in route_a5.json. Comments and literals
are lexed out; an attribute applies to the item, statement, field or arm it
precedes, and a `mod` declaration's attributes apply to the module's file.

  check --repo R --toolchain T --wrapper W --work DIR
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
    These lexical checks (C3) stay as defense in depth (R72: plus a lone CR, a
    symlink, and `includes_allow` rows `file|macro|target` each matched exactly
    once). The claims rest on rustc's own `-Zunpretty=expanded` text of every
    workspace unit, captured by route_a5_expand on the real recipes (design rev
    3.1, SB3E-R71/R72; ship and lane twice, ship at the dev profile, the owners'
    lib tests), under `--wrapper` (the lane's route_rustc) in a fresh `--work`:
      C1:       no reviewed hook name is an identifier in any non-test unit of
                the shipped recipe (release and dev) or the lane recipe; a
                `file|name` row only in the unit owning that file; each row
                occurs in its scope's unit under the test configuration;
      C2:       per unit both recipes compile, the line difference widened to
                its enclosing items (token-aware, B3; the whole unit when any
                line lies in no item) equals tools/gatebox/route/a5_expanded/
                <crate>.pin, with each text's occurrence count;
      placeholder: no include-family invocation left in any expansion (S5);
      capture:  fresh, nonempty, each workspace unit once and equal to Cargo's
                unit graph; first-compile outputs unchanged (B1); environment
                plus RUSTC_BOOTSTRAP only and both RUSTC_BOOTSTRAP values equal
                (B2); argv equal to the plain recipe's (B2); deterministic.
    Not claimed: shipped-binary completeness (A3/A4, A6), third-party macro and
    build-script behaviour (A1/A7), hygiene-level binding identity (printed text
    loses it), ordinary code that adds neither a hook nor lane divergence.
    `--write-pins` writes the current pins (a review aid), then judges them.
  controls --repo R --toolchain T --wrapper W --work DIR
      The plants (mission U4b item 5), each applied to the source as read (or,
      for the Cargo target, to a scratch copy) and restored by hash; every
      plant must be reported by its own check, in a finding with its `expect`.
      SB3E-R72: then the `expanded_plants`, each on a scratch copy built by the
      recipes it names, and the capture controls (mutated healthy captures).
  list --repo R
      Review aid: the writer sites and seam-declared names this tree has.

Exit 0 on pass, 1 on a finding; one JSON verdict on stdout.
"""
import argparse
import bisect
import difflib
import functools
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import route_a5_expand as E  # noqa: E402  (SB3E-R72: the shadow-expansion captures)

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


def lex(text, keep=False):
    """(tokens as (text, kind, offset), refused forms as (form, offset)); comments (nested blocks too)
    and whitespace dropped, or with `keep` (SB3E-R72 B3) comments kept as kind `comment`. A raw identifier keeps
    its `r#` (SB3E-R60.2)."""
    out, bad, at = [], [], 0
    while at < len(text):
        if text.startswith("/*", at):
            start, depth, at = at, 1, at + 2
            while depth and at < len(text):
                depth += text.startswith("/*", at) - text.startswith("*/", at)
                at += 2 if text.startswith(("/*", "*/"), at) else 1
            bad += [("unclosed comment", start)] if depth else []
            out += [(text[start:at], "comment", start)] if keep else []
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
        if kind not in ("ws", "line") or keep and kind == "line":
            out.append((tok, {"raw": "str", "str": "str", "line": "comment"}.get(kind, kind), at))
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


def read_sources(repo, roots, links=None):
    """The `.rs` files under roots, read as written (D4: no newline translation); each symlink met is appended
    to `links` (SB3E-R72 C3: a `subset` finding)."""
    files = {}
    for root in roots:
        for base, dirs, names in os.walk(os.path.join(repo, root)):
            dirs.sort()
            for name in sorted(names + dirs):
                path = os.path.join(base, name)
                if os.path.islink(path) and links is not None:
                    links.append(os.path.relpath(path, repo))
                elif name.endswith(".rs") and name in names:
                    with open(path, encoding="utf-8", newline="") as handle:
                        files[os.path.relpath(path, repo)] = handle.read()
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


def check(repo, toolchain, files=None, cargo_meta=None, links=None):
    data = json.load(open(os.path.join(repo, DATA), encoding="utf-8"))
    seams = json.load(open(os.path.join(repo, SEAMS), encoding="utf-8"))
    if files is None:
        links = []
        files = read_sources(repo, seams["route_sources"], links)
    parsed = {rel: Source(rel, text) for rel, text in files.items()}
    out = [{"check": "subset", "file": rel, "form": f, "offset": at} for rel, src in parsed.items()
           for f, at in src.refused]
    # SB3E-R72 C3: a lone CR (rustc ends a line comment only at LF) and a symlink among the route sources.
    out += [{"check": "subset", "file": rel, "form": "lone CR", "offset": m.start()} for rel, text in files.items()
            for m in re.finditer(r"\r(?!\n)", text)]
    out += [{"check": "subset", "file": rel, "form": "symlink"} for rel in links or ()]
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
    hits, used = {}, {}
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
                    sites.append((i, os.path.normpath(os.path.join(os.path.dirname(rel), unquote(src.toks[o + 1]))), tok))
                else:
                    out.append({"check": "includes", "file": rel, "reason": "not a plain string literal",
                                "offset": src.toks[i][2]})
        for a in src.attrs:
            for j in [j for j in range(a[1], a[2]) if src.n[j] == "path" and src.toks[j][1] == "id"]:
                k = src.after_vis(a[2])
                if j == a[1] + 2 and a[2] == a[1] + 6 and src.t[j + 1] == "=" and unquote(src.toks[j + 2]) is not None \
                        and src.t[k:k + 1] == ["mod"] and src.toks[k + 1][1] == "id" and src.t[k + 2:k + 3] == [";"]:
                    sites.append((j, mod_child(src, k, parsed)[0], "path"))
                else:
                    lit = unquote(src.toks[j + 2]) if src.t[j + 1:j + 2] == ["="] and j + 2 < a[2] else None
                    out.append(dict({"check": "includes", "file": rel, "reason": "not `#[path]` on a `mod x;`",
                                     "offset": src.toks[j][2]}, **({"path": os.path.normpath(os.path.join(
                                         os.path.dirname(rel), lit))} if lit is not None else {})))
        for i, path, macro in sites:
            row = f"{rel}|{macro}|{path}"
            used[row] = used.get(row, 0) + 1
            if (path not in parsed or {"tests", "route_cells"} & set(path.split(os.sep))) and not under(src, gates, i, TEST) \
                    and row not in data["includes_allow"]:
                out.append({"check": "includes", "file": rel, "path": path})
    out += [{"check": "includes", "row": row, "reason": "allow row matched %d times" % used.get(row, 0)}
            for row in data["includes_allow"] if used.get(row, 0) != 1]
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


# ---- SB3E-R72 (design rev 3.1): C1 and C2 over rustc's own expanded text, captured by route_a5_expand ----------
PINS = "tools/gatebox/route/a5_expanded"
RECIPES = (("ship", "ship"), ("ship2", "ship"), ("lane", "lane"), ("lane2", "lane"), ("ship-dev", "ship-dev"),
           ("test", "test"))
QUALIFIERS = ("const", "async", "unsafe", "safe", "default", "extern")
STARTERS = {"pub", "fn", "const", "static", "struct", "enum", "union", "trait", "impl", "mod", "use", "type", "extern",
            "unsafe", "async", "macro_rules", "default", "safe", "#"}


class Items:
    """B3: each item of one expansion, (start, end, container path, header), from tokens (strings, raw strings, byte
    and C strings, char vs lifetime, nested comments) by bracket depth and `;`, never indentation. The attributes and
    comments directly above an item are its own; a span takes whole lines when only blanks lie outside it. `mod`,
    `impl`, `trait` and `extern` blocks are descended. `uncertain` names any form not classified (fail closed)."""

    def __init__(self, text):
        self.text, (self.all, bad) = text, lex(text, keep=True)
        self.code = [i for i, x in enumerate(self.all) if x[1] != "comment"]
        self.t, self.k = [self.all[i][0] for i in self.code], [self.all[i][1] for i in self.code]
        self.uncertain, self.match, self.spans, stack = [f"lexer: {f} at {at}" for f, at in bad], {}, [], []
        for i, (tok, kind) in enumerate(zip(self.t, self.k)):
            if kind == "p" and tok in "([{":
                stack.append(i)
            elif kind == "p" and tok in ")]}":
                if not stack or "([{"[")]}".index(tok)] != self.t[stack[-1]]:
                    stack = [self.uncertain.append(f"unbalanced {tok} at {self.all[self.code[i]][2]}")]
                    break
                self.match[stack.pop()] = i
        self.uncertain += ["unclosed bracket"] * bool(stack)
        if not self.uncertain:
            self.container(0, len(self.t), ())

    def add(self, start, end, path, header):
        j = self.code[start]
        while j > 0 and self.all[j - 1][1] == "comment":
            j -= 1
        a, z, text = self.all[j][2], self.all[self.code[end - 1]][2] + len(self.t[end - 1]), self.text
        b, e = text.rfind("\n", 0, a) + 1, text.find("\n", z)
        e = len(text) if e < 0 else e + 1
        self.spans.append((b if not text[b:a].strip(" \t") else a, e if not text[z:e].strip(" \t\n") else z, path,
                           header.replace("\t", "\\t").replace("\n", "\\n")))

    def container(self, i, close, path):
        t, k = self.t, self.k
        while i < close:
            start = i
            if t[i:i + 3] == ["#", "!", "["]:
                self.add(start, self.match[i + 2] + 1, path, "#!")
                i = self.match[i + 2] + 1
                continue
            while i + 1 < close and t[i:i + 2] == ["#", "["]:
                i = self.match[i + 1] + 1
            q = i + (t[i:i + 1] == ["pub"])
            q = self.match[q] + 1 if t[i:i + 1] == ["pub"] and t[q:q + 1] == ["("] else q
            while q + 1 < close and t[q] in QUALIFIERS and t[q:q + 2] != ["extern", "crate"] and not (
                    t[q] == "const" and k[q + 1] == "id" and t[q + 1] not in QUALIFIERS + ("fn",)):
                q += 2 if t[q] == "extern" and k[q + 1] == "str" else 1
            kw, body, end = t[q] if q < close else "", None, None
            if kw in ("use", "static", "type", "const") or t[q:q + 2] == ["extern", "crate"]:
                end = self.scan(q, close, ";")
            elif kw in ("fn", "struct", "enum", "union", "impl", "trait", "mod") or kw == "{" and "extern" in t[i:q]:
                end = self.scan(q, close, ";{")
                if end is not None and t[end - 1] == "{":
                    body = end - 1
                    end = self.match[body] + 1
                    body = body if kw not in ("fn", "struct", "enum", "union") else None
            elif t[q:q + 2] == ["macro_rules", "!"] and q + 3 < close and t[q + 3] in "([{":
                end = self.match[q + 3] + 1
                end += t[q + 3] != "{" and end < close and t[end] == ";"
            if end is None or end > close or end < close and t[end] not in STARTERS and k[end] != "id":
                return self.uncertain.append(f"unclassified item at {self.all[self.code[start]][2]}: {' '.join(t[start:start + 6])}")
            stop = self.scan(q, end, ";{") or end
            self.add(start, end, path, " ".join(t[i:min(stop - (t[stop - 1] in ";{"), i + 40)]))
            if body is not None:
                self.container(body + 1, self.match[body], path + (self.spans[-1][3],))
            i = end

    def scan(self, i, close, stop):
        """Past the first token in `stop` at depth 0 (brackets skipped); None if none before close."""
        while i < close:
            if self.k[i] == "p" and self.t[i] in stop:
                return i + 1
            i = self.match[i] + 1 if self.k[i] == "p" and self.t[i] in "([{" else i + 1
        return None

    def chosen(self, lines):
        """B3: the innermost item holding each differing line (0-based); None when a line lies in no item (then
        the caller pins the whole unit). Chosen items inside other chosen items are dropped."""
        starts = [0] + [m.end() for m in re.finditer("\n", self.text)]
        owner = {}
        for n, (a, z, _, _) in sorted(enumerate(self.spans), key=lambda s: s[1][0] - s[1][1]):
            if (a == 0 or self.text[a - 1] == "\n") and (z == len(self.text) or self.text[z - 1] == "\n"):
                for line in range(bisect.bisect_right(starts, a) - 1, bisect.bisect_left(starts, z)):
                    owner[line] = n
        if any(line not in owner for line in lines):
            return None
        picked = {self.spans[owner[line]] for line in lines}
        return sorted(s for s in picked if not any(o != s and o[0] <= s[0] and s[1] <= o[1] for o in picked))


def pin_blocks(a, b):
    """C2: per differing enclosing item, (side, module path, item, occurrences of its exact text in that side's unit,
    text); both whole units when either side is uncertain or a differing line lies in no item (B3)."""
    if a == b:
        return []
    la, lb, sides = a.split("\n"), b.split("\n"), ([], [])
    for op, i1, i2, j1, j2 in difflib.SequenceMatcher(None, la, lb, autojunk=False).get_opcodes():
        if op != "equal":
            sides[0].extend(range(i1, i2))
            sides[1].extend(range(j1, j2))
    out = []
    for side, text, lines in (("ship", a, sides[0]), ("lane", b, sides[1])):
        items = Items(text)
        spans = None if items.uncertain else items.chosen(lines)
        if spans is None:
            return [("whole-ship", "", "", 1, a), ("whole-lane", "", "", 1, b)]
        out += [(side, " > ".join(s[2]), s[3], text.count(text[s[0]:s[1]]), text[s[0]:s[1]]) for s in spans]
    return out


def render_pin(crate, source, blocks):
    head = [f"# SB-3E A5 C2 pin (SB3E-R72): {crate} ({source}), rustc -Zunpretty=expanded, shipped vs lane recipe.\n",
            "# Per differing enclosing item: === side, module path, item, occurrences of the exact text in that side's"
            " unit, bytes (tab-separated); then exactly that many bytes of text.\n"]
    return "".join(head + [f"=== {s}\t{p}\t{h}\tcount {n}\tbytes {len(t.encode())}\n{t}" for s, p, h, n, t in blocks])


def parse_pin(text):
    """The blocks of a pin file (its byte counts frame each text); None if it does not parse."""
    data, out, at = text.encode(), [], 0
    while at < len(data):
        end = data.find(b"\n", at)
        line = data[at:end].decode()
        if line.startswith("#"):
            at = end + 1
            continue
        m = re.fullmatch(r"=== ([^\t]*)\t([^\t]*)\t([^\t]*)\tcount (\d+)\tbytes (\d+)", line)
        if end < 0 or not m or end + 1 + int(m[5]) > len(data):
            return None
        out.append((m[1], m[2], m[3], int(m[4]), data[end + 1:end + 1 + int(m[5])].decode()))
        at = end + 1 + int(m[5])
    return out


@functools.lru_cache(maxsize=16)
def idents(text):
    """(identifier set, include-family invocations as (macro, literal)) of one expansion; comments and literals out."""
    toks = lex(text)[0]
    found = {x[0][2:] if x[0].startswith("r#") else x[0] for x in toks if x[1] == "id"}
    calls = [(x[0], unquote(toks[i + 3]) if i + 3 < len(toks) else None) for i, x in enumerate(toks[:-1])
             if x[1] == "id" and x[0] in INCLUDES and toks[i + 1][0] == "!"]
    return frozenset(found), tuple(calls)


def units(cap, test=None):
    return {(r["crate"], r["source"], r["test"]): cap["text"].get(r.get("file"), "") for r in cap["rows"]
            if "passthrough" not in r and (test is None or r["test"] == test)}


def owners(data, meta):
    """Each scoped hook row's owning unit key (crate, root, False): the lib, else the one bin, of the route targets whose
    root's directory is the longest holding the file (design rev 3 C1); None when unresolvable."""
    found = {}
    for row in (h for h in data["test_hooks"] if "|" in h):
        rel = row.split("|")[0]
        cands = [x.split("|") for x in meta if x.split("|")[2] in ("lib", "bin")
                 and rel.startswith(os.path.dirname(x.split("|")[4]) + "/")]
        if not cands:
            found[row] = None
            continue
        best = [c for c in cands if len(c[4]) - len(os.path.basename(c[4])) == max(len(x[4]) - len(os.path.basename(x[4])) for x in cands)]
        pick = [c for c in best if c[2] == "lib"] or best
        found[row] = (pick[0][1].replace("-", "_"), pick[0][4], False) if len(pick) == 1 else None
    return found


def capture_rules(name, cap, graph, plain):
    """Fresh, nonempty, each workspace unit once and equal to Cargo's unit graph; each first compile exit 0 with its
    outputs unchanged (B1); env difference exactly RUSTC_BOOTSTRAP and the differential equal (B2); every expansion
    nonempty; the argv equal to the plain recipe's modulo R72 B2's list (B2)."""
    out, rows = [], [r for r in cap["rows"] if "passthrough" not in r]
    f = lambda reason, **kw: out.append(dict({"check": "capture", "recipe": name, "reason": reason}, **kw))
    if cap["rc"] != 0 or not rows:
        f("recipe failed or captured nothing", exit=cap["rc"], units=len(rows))
    for r in cap["rows"]:
        if "passthrough" in r and r["passthrough"] != ["-vV"] and "___" not in values_of(r["passthrough"], "--crate-name"):
            f("unexpected passthrough", argv=r["passthrough"][:8])
    keys = [(r["crate"], r["source"], r["test"]) for r in rows]
    for r in rows:
        unit, sh = [r["crate"], r["source"]], r.get("shadow", [])
        reasons = [why for bad, why in (
            (r["rc"] != 0, "compile failed"), (not r.get("unchanged"), "first compile's outputs changed"),
            (not r.get("env_ok"), "environment differs beyond RUSTC_BOOTSTRAP"),
            (len(sh) != 2 or any(s["rc"] or not s["bytes"] for s in sh), "expansion failed"),
            (len({s["sha256"] for s in sh}) > 1, "RUSTC_BOOTSTRAP differential differs"),
            (any(s["wrote"] for s in sh), "shadow wrote outputs"),
            (not cap["text"].get(r.get("file")) or sh and sha(cap["text"].get(r.get("file"), "")) != sh[0]["sha256"],
             "capture missing or empty")) if bad]
        for why in reasons:
            f(why, unit=unit)
    if len(set(keys)) != len(keys):
        f("a unit captured more than once")
    if graph is not None and sorted(keys) != graph[1]:
        f("units differ from Cargo's unit graph", only_capture=sorted(set(keys) - set(graph[1] or [])),
          only_graph=sorted(set(graph[1] or []) - set(keys)))
    if plain is not None:
        found = {k: v for k, v in plain[0].items() if k[1] and not k[1].startswith("/")}
        if plain[1] != 0 or sorted(found) != sorted(keys):
            f("plain recipe units differ", exit=plain[1])
        for r in rows:
            mine = E.normalized(r["argv"], cap["target"], r["extra"])
            if found.get((r["crate"], r["source"], r["test"])) != [mine]:
                f("argv differs from the plain recipe", unit=[r["crate"], r["source"]])
    return out


def values_of(args, opt):
    return [b for a, b in zip(args, args[1:]) if a == opt]


def expanded(repo, data, caps, meta, graph=None, plain=None):
    """C1, C2, placeholders and the capture rules over whichever recipes `caps` holds."""
    graph, plain, out = graph or {}, plain or {}, []
    for name, cap in caps.items():
        kind = dict(RECIPES)[name]
        out += capture_rules(name, cap, (kind, graph[kind]) if kind in graph else None,
                             plain.get(kind) if name in ("ship", "lane") else None)
        for (crate, source, test), text in units(cap).items():  # S5: every expansion scanned
            for macro, lit in idents(text)[1]:
                path = os.path.normpath(os.path.join(os.path.dirname(source), lit or ""))
                ok = lit and os.path.isfile(os.path.join(repo, path)) and \
                    f"{crate}|{path}|{hashlib.sha256(open(os.path.join(repo, path), 'rb').read()).hexdigest()}" in data["placeholders"]
                out += [] if ok else [{"check": "placeholder", "recipe": name, "unit": crate, "macro": macro, "literal": lit}]
    for a in ("ship", "lane"):  # determinism: each real recipe captured twice, byte-equal
        if a in caps and a + "2" in caps and units(caps[a]) != units(caps[a + "2"]):
            out.append({"check": "capture", "recipe": a, "reason": "nondeterministic expansion",
                        "units": sorted(k[0] for k in set(units(caps[a])) | set(units(caps[a + "2"]))
                                        if units(caps[a]).get(k) != units(caps[a + "2"]).get(k))})
    own = owners(data, meta)
    out += [{"check": "C1", "reason": "owner unresolved", "row": row} for row, key in own.items() if key is None]
    for name in ("ship", "ship-dev", "lane"):  # C1 (B4: every non-test unit of both recipes and the dev profile)
        for key, text in units(caps.get(name, {"rows": [], "text": {}}), False).items():
            ids = idents(text)[0]
            out += [{"check": "C1", "recipe": name, "unit": key[0], "source": key[1], "hook": row.split("|")[-1]}
                    for row in data["test_hooks"] if row.split("|")[-1] in ids and ("|" not in row or own[row] == key)]
    if "test" in caps:  # positive control: each row occurs in its scope's unit under the test configuration
        tests = units(caps["test"], True)
        for row in data["test_hooks"]:
            scope = [t for k, t in tests.items() if "|" not in row or own[row] and k[:2] == own[row][:2]]
            if not any(row.split("|")[-1] in idents(t)[0] for t in scope):
                out.append({"check": "C1", "reason": "stale row", "row": row})
    if "ship" in caps and "lane" in caps:
        out += c2(repo, units(caps["ship"], False), units(caps["lane"], False))
    return out


def c2(repo, ship, lane):
    """C2: per unit both recipes compile (non-test), the item-level difference equals its committed pin."""
    out, common = [], sorted(k for k in set(ship) & set(lane))
    folder = os.path.join(repo, PINS)
    have = sorted(n[:-4] for n in os.listdir(folder) if n.endswith(".pin")) if os.path.isdir(folder) else []
    if have != sorted(k[0] for k in common) or len({k[0] for k in common}) != len(common):
        out.append({"check": "C2", "reason": "pin files differ from the common units", "have": have,
                    "common": sorted(k[0] for k in common)})
    for crate, source, _ in common:
        blocks = pin_blocks(ship[(crate, source, False)], lane[(crate, source, False)])
        path = os.path.join(folder, crate + ".pin")
        want_text = open(path, encoding="utf-8", newline="").read() if os.path.isfile(path) else ""
        if render_pin(crate, source, blocks) != want_text:
            want = parse_pin(want_text) or []
            out += [{"check": "C2", "unit": crate, "change": change, "side": b[0], "path": b[1], "item": b[2], "count": b[3]}
                    for change, have_, other in (("unpinned", blocks, want), ("stale", want, blocks)) for b in have_ if b not in other]
            out += [{"check": "C2", "unit": crate, "reason": "pin text differs"}] if blocks == want else []
    return out


def collect(repo, toolchain, wrapper, work):
    """Every A5 capture (ship and lane twice, ship-dev, test), Cargo's unit graphs and the plain ship and lane runs."""
    os.makedirs(work)
    caps = {name: load(*E.capture(repo, toolchain, kind, work, name, wrapper), os.path.join(work, "t-" + name))
            for name, kind in RECIPES}
    graph = {kind: E.graph(repo, toolchain, kind, work) for kind in ("ship", "lane", "ship-dev", "test")}
    plain = {kind: E.plain(repo, toolchain, kind, work, wrapper) for kind in ("ship", "lane")}
    return caps, graph, plain


def load(rows, capdir, rc, target, keep=False):
    """One capture's rows and texts in memory; its target is removed unless kept warm (controls)."""
    if not keep:
        shutil.rmtree(target, ignore_errors=True)
    text = {r["file"]: open(os.path.join(capdir, r["file"]), encoding="utf-8", newline="").read() for r in rows
            if r.get("file") and os.path.isfile(os.path.join(capdir, r["file"]))}
    return {"rows": rows, "text": text, "rc": rc, "target": target}


def controls(repo, toolchain, work, wrapper):
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
    more, rows = expanded_controls(repo, toolchain, wrapper, os.path.join(work, "expanded"), data, seams, meta)
    return findings + more, {"plants": results, "expanded": rows}


def expanded_controls(repo, toolchain, wrapper, work, data, seams, meta):
    """SB3E-R72 plants on a scratch copy, each built by the recipes it names with the shadow wrapper (warm targets,
    every workspace unit rebuilt) and judged by the new checks on the planted tree; then the capture controls,
    each a mutation of the healthy captures or of the data. Each must give a finding carrying its `expect`."""
    import route_a4  # noqa: E402  (scratch_copy)
    os.makedirs(work)
    tree = route_a4.scratch_copy(repo, work)[0]
    rows, findings, targets = [], [], {}

    def run(name, kinds, keep_emit=False):
        caps = {}
        for kind in kinds:
            target = targets.setdefault(kind, os.path.join(work, "t-" + kind))
            caps[kind] = load(*E.capture(tree, toolchain, kind, work, f"{name}-{kind}", wrapper, target, keep_emit), target, True)
        return caps

    def judge(name, cls, expect, got, **extra):
        row = dict(extra, name=name, **{"class": cls, "expect": expect})
        row["killed"] = any(all(f.get(k) == v for k, v in expect.items()) for f in got)
        row["got"] = [] if row["killed"] else got[:5]
        rows.append(row)
        findings.extend([] if row["killed"] else [{"kind": "expanded_plant_not_killed", "plant": name, "got": got[:5]}])

    healthy = run("healthy", ("ship", "lane", "test"))
    graph = {"ship": E.graph(tree, toolchain, "ship", work)}
    base = expanded(repo, data, healthy, meta, graph)
    findings += [{"kind": "unplanted_captures_have_findings", "findings": base[:5]}] if base else []
    for plant in data["expanded_plants"]:
        made, before, unique = [], {}, True
        for rel, text in list(plant.get("create", {}).items()) + list(plant.get("symlink", {}).items()):
            path = os.path.join(tree, rel)
            up = os.path.dirname(path)
            while not os.path.isdir(up):
                made.insert(0, up)
                up = os.path.dirname(up)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            unique &= not os.path.lexists(path)
            os.symlink(text, path) if rel in plant.get("symlink", {}) else open(path, "w", encoding="utf-8", newline="").write(text)
        for e in plant.get("edits", []):
            path = os.path.join(tree, e["file"])
            text = before.setdefault(path, open(path, encoding="utf-8", newline="").read())
            now = open(path, encoding="utf-8", newline="").read()
            unique &= now.count(e["old"]) == 1
            open(path, "w", encoding="utf-8", newline="").write(now.replace(e["old"], e["new"]))
        try:
            caps = run(plant["name"], plant["recipes"], plant.get("keep_emit", False))
            links = []
            got = expanded(repo, data, caps, meta) + check(repo, toolchain, read_sources(tree, seams["route_sources"], links), meta, links)
        finally:
            for path, text in before.items():
                open(path, "w", encoding="utf-8", newline="").write(text)
            for rel in list(plant.get("create", {})) + list(plant.get("symlink", {})):
                os.unlink(os.path.join(tree, rel))
            for up in reversed(made):
                os.rmdir(up)
        restored = all(open(p, encoding="utf-8", newline="").read() == t for p, t in before.items())
        judge(plant["name"], plant["class"], plant["expect"], got if unique and restored else [], unique=unique, restored=restored)
    # Capture controls: mutations of the healthy captures and data (design rev 3: preservation; R72 B1, B2, S5).
    ship, lane = healthy["ship"], healthy["lane"]
    first = lambda cap, crate: next(r for r in cap["rows"] if r.get("crate") == crate)
    variant = lambda cap, **kw: dict(cap, **kw)
    hands = first(lane, "mother_hands")["file"]
    cases = [
        ("missing_unit", {"ship": variant(ship, rows=[r for r in ship["rows"] if r.get("crate") != "nexus_crypto"])}, data,
         {"check": "capture", "recipe": "ship", "reason": "units differ from Cargo's unit graph"}),
        ("empty_capture", {"ship": variant(ship, text=dict(ship["text"], **{first(ship, "nexus_crypto")["file"]: ""}))}, data,
         {"check": "capture", "recipe": "ship", "reason": "capture missing or empty"}),
        ("nondeterministic", {"lane": lane, "lane2": variant(lane, text=dict(lane["text"], **{hands: lane["text"][hands] + "\n"}))},
         data, {"check": "capture", "recipe": "lane", "reason": "nondeterministic expansion"}),
        ("environment", {"ship": variant(ship, rows=[dict(r, env_ok=False) for r in ship["rows"]])}, data,
         {"check": "capture", "recipe": "ship", "reason": "environment differs beyond RUSTC_BOOTSTRAP"}),
        ("placeholder_lane_only", {"lane": variant(lane, text=dict(lane["text"], **{hands: lane["text"][hands] + 'const P: &str = include_str!("x");\n'}))},
         data, {"check": "placeholder", "recipe": "lane", "unit": "mother_hands", "macro": "include_str"}),
        ("stale_row", healthy, dict(data, test_hooks=data["test_hooks"] + ["never_written_for_test"]),
         {"check": "C1", "reason": "stale row", "row": "never_written_for_test"}),
        ("owner_unresolved", healthy, dict(data, test_hooks=data["test_hooks"] + ["elsewhere/x.rs|kept_for_test"]),
         {"check": "C1", "reason": "owner unresolved", "row": "elsewhere/x.rs|kept_for_test"}),
        ("scope_dropped_model_record", {"ship": ship}, dict(data, test_hooks=data["test_hooks"] + ["model_record"]),
         {"check": "C1", "recipe": "ship", "unit": "nexus_governed_control", "hook": "model_record"})]
    for name, caps, rows_data, expect in cases:
        judge(name, "capture", expect, expanded(repo, rows_data, caps, meta, graph if "ship" in caps else None))
    wrong = {k: [v[0] + ["--cfg", "planted"]] for k, v in ((r_key(r), [E.normalized(r["argv"], ship["target"], r["extra"])])
                                                         for r in ship["rows"] if "passthrough" not in r)}
    judge("argv_reconcile", "capture", {"check": "capture", "recipe": "ship", "reason": "argv differs from the plain recipe"},
          expanded(repo, data, {"ship": ship}, meta, None, {"ship": (wrong, 0)}))
    return findings, rows


def r_key(r):
    return r["crate"], r["source"], r["test"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "controls", "list"))
    parser.add_argument("--repo", required=True)
    parser.add_argument("--toolchain")
    parser.add_argument("--work")
    parser.add_argument("--wrapper", help="route_rustc, the lane's own wrapper (SB3E-R72)")
    parser.add_argument("--write-pins", action="store_true")
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
    work = os.path.abspath(args.work or "")
    if not args.work or os.path.lexists(work) or not args.wrapper:
        sys.exit("route_a5: check and controls need --wrapper and a fresh --work directory")
    wrapper = os.path.abspath(args.wrapper)
    if args.command == "check":
        seams = json.load(open(os.path.join(repo, SEAMS), encoding="utf-8"))
        meta = cargo_targets(repo, toolchain, seams["route_packages"])
        findings, (caps, graph, plain) = check(repo, toolchain, None, meta), collect(repo, toolchain, wrapper, work)
        if args.write_pins:  # review aid: the current pins, then judged like any other
            os.makedirs(os.path.join(repo, PINS), exist_ok=True)
            ship, lane = units(caps["ship"], False), units(caps["lane"], False)
            for k in sorted(set(ship) & set(lane)):
                with open(os.path.join(repo, PINS, k[0] + ".pin"), "w", encoding="utf-8", newline="") as handle:
                    handle.write(render_pin(k[0], k[1], pin_blocks(ship[k], lane[k])))
        findings += expanded(repo, json.load(open(os.path.join(repo, DATA), encoding="utf-8")), caps, meta, graph, plain)
        out = {"units": {name: sorted(k[0] + ("/test" if k[2] else "") for k in units(cap)) for name, cap in caps.items()}}
    else:
        findings, out = controls(repo, toolchain, work, wrapper)
    print(json.dumps(dict(out, verdict="fail" if findings else "pass", findings=findings), sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
