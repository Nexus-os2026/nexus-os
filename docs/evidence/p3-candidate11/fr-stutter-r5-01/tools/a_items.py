"""Checker A, part 1: a token-level Rust lexer and item extractor.

Independent of Checker B (which masks text with a character state machine
and finds items with regular expressions). Nothing here is shared with B.

Item naming convention (stated once in tools/NAMING.md, implemented twice):
  free fn                      NAME
  fn in `impl Type`            Type::NAME
  fn in `impl Trait for Type`  <Type as Trait>::NAME
  fn nested in fn OUTER        OUTER/NAME
  struct/enum/const/static/type/trait/macro   'struct NAME' etc.
  items inside an inline module M are prefixed 'M::'
  tokens in no item            <module>
An item's span runs from its first leading outer attribute or outer doc
comment (contiguous, nothing but whitespace between) to its closing '}' or
';'. Its hash is sha256 of exactly those bytes.
"""
import hashlib
import re

IDENT_START = re.compile(r"[A-Za-z_]")
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
NUMBER = re.compile(r"0[xob][0-9A-Fa-f_]+[a-z0-9]*|[0-9][0-9_]*(\.[0-9][0-9_]*)?([eE][+-]?[0-9_]+)?[a-z0-9]*")


class Tok:
    __slots__ = ("kind", "text", "start", "end", "line", "col")

    def __init__(self, kind, text, start, end, line, col):
        self.kind, self.text, self.start, self.end, self.line, self.col = kind, text, start, end, line, col

    def __repr__(self):
        return f"Tok({self.kind},{self.text!r},L{self.line}:{self.col})"


class LexError(Exception):
    pass


def lex(src):
    """Tokens of `src` (a str). Kinds: ident, punct, str, char, lifetime,
    num, comment. Comments are kept as tokens."""
    toks = []
    i, n = 0, len(src)
    line_starts = [0]
    for k, ch in enumerate(src):
        if ch == "\n":
            line_starts.append(k + 1)

    def pos(k):
        # binary search for line
        lo, hi = 0, len(line_starts) - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if line_starts[mid] <= k:
                lo = mid
            else:
                hi = mid - 1
        return lo + 1, k - line_starts[lo] + 1

    def add(kind, a, b):
        ln, col = pos(a)
        toks.append(Tok(kind, src[a:b], a, b, ln, col))

    while i < n:
        c = src[i]
        if c in " \t\r\n":
            i += 1
            continue
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            add("comment", i, j)
            i = j
            continue
        if src.startswith("/*", i):
            depth, j = 1, i + 2
            while depth:
                if j >= n:
                    raise LexError("unterminated block comment")
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            add("comment", i, j)
            i = j
            continue
        # raw strings r"..", r#".."#, br"..", br#".."#
        m = re.compile(r"b?r(#*)\"").match(src, i)
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            hashes = m.group(1)
            close = '"' + hashes
            j = src.find(close, m.end())
            if j < 0:
                raise LexError("unterminated raw string")
            add("str", i, j + len(close))
            i = j + len(close)
            continue
        if c == '"' or (c == "b" and src.startswith('b"', i)):
            j = i + (2 if c == "b" else 1)
            while True:
                if j >= n:
                    raise LexError("unterminated string")
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            add("str", i, j)
            i = j
            continue
        if c == "b" and src.startswith("b'", i):
            j = i + 2
            if src[j] == "\\":
                j = src.index("'", j + 2) + 1
            else:
                j = j + 2
            add("char", i, j)
            i = j
            continue
        if c == "'":
            # escape char literal, plain char literal, or lifetime
            if i + 1 < n and src[i + 1] == "\\":
                j = i + 2
                if src[j] == "u":
                    j = src.index("}", j) + 1
                else:
                    j += 1
                if src[j] != "'":
                    raise LexError(f"bad escape char literal at {i}")
                add("char", i, j + 1)
                i = j + 1
                continue
            if i + 2 < n and src[i + 2] == "'":
                add("char", i, i + 3)
                i += 3
                continue
            m = IDENT.match(src, i + 1)
            if not m:
                raise LexError(f"lone quote at {i}")
            add("lifetime", i, m.end())
            i = m.end()
            continue
        if src.startswith("r#", i) and IDENT_START.match(src, i + 2 if i + 2 < n else i):
            m = IDENT.match(src, i + 2)
            add("ident", i, m.end())
            i = m.end()
            continue
        if IDENT_START.match(c):
            m = IDENT.match(src, i)
            add("ident", i, m.end())
            i = m.end()
            continue
        if c.isdigit():
            m = NUMBER.match(src, i)
            add("num", i, m.end())
            i = m.end()
            continue
        add("punct", i, i + 1)
        i += 1
    return toks, pos


QUALIFIERS = {"pub", "const", "unsafe", "async", "extern", "default"}
ITEM_KW = {"fn", "struct", "enum", "union", "const", "static", "type", "trait", "impl", "mod", "macro_rules"}


def _type_name(tokens):
    """Last path segment's identifier before any generic list, skipping
    references and `dyn`/`mut`."""
    name = None
    depth = 0
    for t in tokens:
        if t.text == "<":
            depth += 1
        elif t.text == ">":
            depth -= 1
        elif depth == 0 and t.kind == "ident" and t.text not in ("mut", "dyn", "for"):
            name = t.text
    return name


class Item:
    __slots__ = ("name", "kind", "start", "end", "line1", "line2", "parent", "children", "body_open")

    def __init__(self, name, kind, start, end, parent):
        self.name, self.kind, self.start, self.end, self.parent = name, kind, start, end, parent
        self.children = []
        self.line1 = self.line2 = None
        self.body_open = None


def items(src):
    """Every item of `src`, nested, with spans and names."""
    toks, pos = lex(src)
    sig = [t for t in toks if t.kind != "comment"]
    # index of each significant token in the full list
    full_index = {id(t): k for k, t in enumerate(toks)}

    def match_close(k):
        """Index in sig of the bracket closing sig[k]."""
        pairs = {"{": "}", "(": ")", "[": "]"}
        stack = []
        j = k
        while j < len(sig):
            t = sig[j]
            if t.kind == "punct" and t.text in pairs:
                stack.append(pairs[t.text])
            elif t.kind == "punct" and t.text in ")}]":
                if not stack or stack[-1] != t.text:
                    raise LexError(f"bracket mismatch at line {t.line}")
                stack.pop()
                if not stack:
                    return j
            j += 1
        raise LexError("unclosed bracket")

    def leading_start(k):
        """Start offset of item whose keyword-or-qualifier run begins at sig[k]:
        extend back over visibility, qualifiers, outer attributes and outer doc
        comments that are contiguous."""
        start_full = full_index[id(sig[k])]
        j = start_full - 1
        best = toks[start_full].start
        while j >= 0:
            t = toks[j]
            if t.kind == "comment":
                if (t.text.startswith("///") and not t.text.startswith("////")) or t.text.startswith("/**"):
                    best = t.start
                    j -= 1
                    continue
                break
            if t.kind == "punct" and t.text == "]":
                # find matching '[' backwards, then require '#'
                depth, m = 0, j
                while m >= 0:
                    if toks[m].kind == "punct" and toks[m].text == "]":
                        depth += 1
                    elif toks[m].kind == "punct" and toks[m].text == "[":
                        depth -= 1
                        if depth == 0:
                            break
                    m -= 1
                p = m - 1
                if p >= 0 and toks[p].kind == "punct" and toks[p].text == "!":
                    break  # inner attribute: belongs to the enclosing scope
                if p >= 0 and toks[p].kind == "punct" and toks[p].text == "#":
                    best = toks[p].start
                    j = p - 1
                    continue
                break
            break
        return best

    def qualifier_start(k):
        """Walk back from keyword sig[k] over visibility and qualifiers."""
        j = k - 1
        first = k
        while j >= 0:
            t = sig[j]
            if t.kind == "ident" and t.text in QUALIFIERS:
                first = j
                j -= 1
                continue
            if t.kind == "str" and j - 1 >= 0 and sig[j - 1].text == "extern":
                first = j - 1
                j -= 2
                continue
            if t.kind == "punct" and t.text == ")":
                # pub(super) / pub(crate) / pub(in path)
                m = j
                while m >= 0 and sig[m].text != "(":
                    m -= 1
                if m - 1 >= 0 and sig[m - 1].text == "pub":
                    first = m - 1
                    j = m - 2
                    continue
            break
        return first

    root = Item("<module>", "module", 0, len(src), None)
    stack = [(root, len(sig))]  # (item, index in sig where it ends)

    def current(k):
        while stack and k > stack[-1][1]:
            stack.pop()
        return stack[-1][0]

    def ctx_name(parent, name, kind):
        # compute the canonical name
        if parent.kind == "impl":
            return f"{parent.name}::{name}" if kind == "fn" else f"{parent.name}::{name}"
        if parent.kind == "fn":
            return f"{parent.name}/{name}" if kind == "fn" else f"{parent.name}/{kind} {name}"
        if parent.kind == "mod":
            return f"{parent.name}::{name}" if kind == "fn" else f"{parent.name}::{kind} {name}"
        if parent.kind in ("trait",):
            return f"{parent.name}::{name}"
        return name if kind == "fn" else f"{kind} {name}"

    all_items = []
    k = 0
    prev_sig = None
    while k < len(sig):
        t = sig[k]
        parent = current(k)
        is_item_pos = prev_sig is None or prev_sig.text in ("}", ";", "{", "]") or (
            prev_sig.kind == "ident" and prev_sig.text in QUALIFIERS) or (
            prev_sig.text == ")" and k >= 2)
        kw = t.text if t.kind == "ident" else None
        if kw == "macro_rules" and k + 2 < len(sig) and sig[k + 1].text == "!":
            name = sig[k + 2].text
            # body: next bracket
            b = k + 3
            close = match_close(b)
            start = leading_start(qualifier_start(k))
            it = Item(ctx_name(parent, name, "macro_rules!"), "macro", start, sig[close].end, parent)
            parent.children.append(it)
            all_items.append(it)
            prev_sig = sig[close]
            k = close + 1
            continue
        if kw in ITEM_KW and kw != "macro_rules":
            nxt = sig[k + 1] if k + 1 < len(sig) else None
            # `const fn`: the item is the fn
            if kw == "const" and nxt is not None and nxt.text in ("fn", "unsafe"):
                prev_sig = t
                k += 1
                continue
            if kw == "fn" and (nxt is None or nxt.kind != "ident"):
                prev_sig = t
                k += 1
                continue  # fn pointer type
            if kw == "impl" and not is_item_pos:
                prev_sig = t
                k += 1
                continue  # impl Trait in type position
            if kw in ("struct", "enum", "union", "trait", "mod", "type", "static", "const") and (
                    nxt is None or nxt.kind != "ident"):
                prev_sig = t
                k += 1
                continue
            if kw == "static" and nxt.text == "mut":
                nxt = sig[k + 2]
            if kw in ("const", "static", "type", "struct", "enum", "trait", "mod", "union") and not is_item_pos:
                prev_sig = t
                k += 1
                continue
            # find body end
            j = k + 1
            header = []
            depth_angle = 0
            body = None
            end_idx = None
            while j < len(sig):
                s = sig[j]
                if s.kind == "punct" and s.text in "([":
                    j = match_close(j) + 1
                    continue
                if s.kind == "punct" and s.text == "{":
                    if kw in ("const", "static", "type"):
                        j = match_close(j) + 1
                        continue
                    body = j
                    end_idx = match_close(j)
                    break
                if s.kind == "punct" and s.text == ";":
                    end_idx = j
                    break
                if s.kind == "punct" and s.text == "=" and kw in ("const", "static", "type"):
                    pass
                header.append(s)
                j += 1
            if end_idx is None:
                raise LexError(f"item without end at line {t.line}")
            start = leading_start(qualifier_start(k))
            if kw == "impl":
                # header tokens up to '{'
                htoks = header
                # skip generics directly after impl
                hi = 0
                if htoks and htoks[0].text == "<":
                    d = 0
                    while hi < len(htoks):
                        if htoks[hi].text == "<":
                            d += 1
                        elif htoks[hi].text == ">":
                            d -= 1
                            if d == 0:
                                hi += 1
                                break
                        hi += 1
                rest = htoks[hi:]
                fors = [q for q, x in enumerate(rest) if x.kind == "ident" and x.text == "for"]
                if fors:
                    trait = _type_name(rest[:fors[0]])
                    typ = _type_name(rest[fors[0] + 1:])
                    name = f"<{typ} as {trait}>"
                else:
                    name = _type_name(rest)
                if parent.kind == "mod":
                    name = f"{parent.name}::{name}"
                it = Item(name, "impl", start, sig[end_idx].end, parent)
            elif kw == "mod":
                nm = nxt.text
                it = Item(nm if parent.kind != "mod" else f"{parent.name}::{nm}", "mod", start, sig[end_idx].end, parent)
            else:
                nm = nxt.text
                kind = kw if kw != "union" else "union"
                it = Item(ctx_name(parent, nm, kw), kind, start, sig[end_idx].end, parent)
            it.body_open = sig[body].start if body is not None else None
            parent.children.append(it)
            all_items.append(it)
            if body is not None and kw in ("fn", "impl", "mod", "trait"):
                stack.append((it, end_idx))
                prev_sig = sig[body]
                k = body + 1
                continue
            prev_sig = sig[end_idx]
            k = end_idx + 1
            continue
        prev_sig = t
        k += 1
    for it in all_items:
        it.line1 = pos(it.start)[0]
        it.line2 = pos(it.end - 1)[0]
    return root, all_items, toks, pos


def sha256(b):
    return hashlib.sha256(b).hexdigest()


def item_bytes(src_bytes, src, it):
    """Exact bytes of an item (spans are str offsets; convert)."""
    pre = len(src[: it.start].encode("utf-8"))
    body = src[it.start: it.end].encode("utf-8")
    return pre, pre + len(body), body


def innermost(all_items, offset):
    best = None
    for it in all_items:
        if it.start <= offset < it.end:
            if best is None or (it.end - it.start) < (best.end - best.start):
                best = it
    return best
