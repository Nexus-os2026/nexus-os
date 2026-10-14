#!/usr/bin/env python3
"""A small, self-contained Rust lexer and item extractor (read only; no build).

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01.

It splits a Rust source file into tokens (comments, string/char/byte/raw literals,
lifetimes, identifiers, numbers, punctuation) and then into items: the top-level
items of the file and, recursively, the items inside `impl`, `trait` and `mod`
bodies. Each item has a path (for example `fn explore`, `impl Oracle/fn new`), an
ordinal among items with the same path, and two byte ranges:

  span: from the end of the previous item (leading whitespace skipped) to the end
        of this item -- it includes leading doc comments, plain comments and
        attributes;
  decl: from the item's first attribute or keyword (comments skipped) to its end.

Identity is by bytes: sha256 of span and of decl. Line numbers are reported for
reading only. Every byte of the file belongs to exactly one top-level span or to
trailing whitespace/comments, and the extractor checks that coverage.
"""
import hashlib
import json
import sys

KEYWORDS_ITEM = {'fn', 'struct', 'enum', 'union', 'trait', 'impl', 'mod', 'const',
                 'static', 'type', 'use', 'extern', 'macro_rules'}
SEMI_ITEMS = {'const', 'static', 'type', 'use', 'let'}
QUALIFIERS = {'pub', 'unsafe', 'async', 'default', 'auto'}


class Tok:
    __slots__ = ('kind', 'text', 'start', 'end')

    def __init__(self, kind, text, start, end):
        self.kind, self.text, self.start, self.end = kind, text, start, end

    def __repr__(self):
        return '%s(%r@%d)' % (self.kind, self.text, self.start)


def _ident_start(c):
    return c == '_' or c.isalpha()


def _ident_char(c):
    return c == '_' or c.isalnum()


def lex(src):
    """Tokens of src (a str). Kinds: ws, line_comment, block_comment, str, char,
    lifetime, ident, num, punct. Raises ValueError on an unterminated construct."""
    toks = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        start = i
        if c.isspace():
            while i < n and src[i].isspace():
                i += 1
            toks.append(Tok('ws', src[start:i], start, i))
            continue
        if src.startswith('//', i):
            j = src.find('\n', i)
            i = n if j < 0 else j
            toks.append(Tok('line_comment', src[start:i], start, i))
            continue
        if src.startswith('/*', i):
            depth, i = 1, i + 2
            while depth:
                if i >= n:
                    raise ValueError('unterminated block comment at %d' % start)
                if src.startswith('/*', i):
                    depth, i = depth + 1, i + 2
                elif src.startswith('*/', i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
            toks.append(Tok('block_comment', src[start:i], start, i))
            continue
        # raw strings: r"..", r#".."#, br"..", cr".."
        m = None
        for prefix in ('br', 'cr', 'r'):
            if src.startswith(prefix, i):
                j = i + len(prefix)
                hashes = 0
                while j < n and src[j] == '#':
                    hashes, j = hashes + 1, j + 1
                if j < n and src[j] == '"' and not (prefix == 'r' and hashes == 0 and i > 0 and _ident_char(src[i - 1])):
                    m = (j, hashes)
                break
        if m is not None:
            j, hashes = m
            close = '"' + '#' * hashes
            k = src.find(close, j + 1)
            if k < 0:
                raise ValueError('unterminated raw string at %d' % start)
            i = k + len(close)
            toks.append(Tok('str', src[start:i], start, i))
            continue
        # raw identifiers r#name
        if src.startswith('r#', i) and i + 2 < n and _ident_start(src[i + 2]):
            j = i + 2
            while j < n and _ident_char(src[j]):
                j += 1
            i = j
            toks.append(Tok('ident', src[start:i], start, i))
            continue
        # strings: "..", b"..", c".."
        if c == '"' or (c in 'bc' and i + 1 < n and src[i + 1] == '"'):
            j = i + (1 if c != '"' else 0) + 1
            while True:
                if j >= n:
                    raise ValueError('unterminated string at %d' % start)
                if src[j] == '\\':
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            i = j
            toks.append(Tok('str', src[start:i], start, i))
            continue
        # byte char b'x'
        if c == 'b' and i + 1 < n and src[i + 1] == "'":
            j = i + 2
            if src[j] == '\\':
                j += 2
                while src[j] != "'":
                    j += 1
            else:
                j += 1
            if src[j] != "'":
                raise ValueError('bad byte char at %d' % start)
            i = j + 1
            toks.append(Tok('char', src[start:i], start, i))
            continue
        if c == "'":
            # char literal or lifetime/label
            if i + 1 < n and src[i + 1] == '\\':
                j = i + 2
                if src[j] == 'u' and src[j + 1] == '{':
                    j = src.index('}', j) + 1
                else:
                    j += 1
                    while src[j] != "'":
                        j += 1
                if src[j] != "'":
                    raise ValueError('bad char at %d' % start)
                i = j + 1
                toks.append(Tok('char', src[start:i], start, i))
                continue
            if i + 2 < n and src[i + 2] == "'":
                i += 3
                toks.append(Tok('char', src[start:i], start, i))
                continue
            if i + 1 < n and _ident_start(src[i + 1]):
                j = i + 1
                while j < n and _ident_char(src[j]):
                    j += 1
                i = j
                toks.append(Tok('lifetime', src[start:i], start, i))
                continue
            raise ValueError('unrecognised quote at %d' % start)
        if _ident_start(c):
            j = i
            while j < n and _ident_char(src[j]):
                j += 1
            i = j
            toks.append(Tok('ident', src[start:i], start, i))
            continue
        if c.isdigit():
            j = i
            while j < n and (_ident_char(src[j]) or (src[j] == '.' and j + 1 < n and src[j + 1].isdigit())):
                j += 1
            i = j
            toks.append(Tok('num', src[start:i], start, i))
            continue
        i += 1
        toks.append(Tok('punct', c, start, i))
    return toks


def code_tokens(toks):
    return [t for t in toks if t.kind not in ('ws', 'line_comment', 'block_comment')]


OPEN = {'{': '}', '(': ')', '[': ']'}
CLOSE = {'}', ')', ']'}


def _item_header(code, k):
    """Index of the item keyword after attributes/visibility starting at code[k], the
    keyword, and the name (or a header text for impl/use/macro invocations)."""
    j = k
    # outer attributes
    while j < len(code) and code[j].text == '#':
        j += 1
        if j < len(code) and code[j].text == '!':
            j += 1
        if code[j].text != '[':
            raise ValueError('attribute without [ at %d' % code[j].start)
        depth = 0
        while True:
            t = code[j].text
            if t in OPEN:
                depth += 1
            elif t in CLOSE:
                depth -= 1
            j += 1
            if depth == 0:
                break
    # visibility and qualifiers
    while j < len(code):
        t = code[j].text
        if t == 'pub':
            j += 1
            if j < len(code) and code[j].text == '(':
                depth = 0
                while True:
                    tt = code[j].text
                    if tt == '(':
                        depth += 1
                    elif tt == ')':
                        depth -= 1
                    j += 1
                    if depth == 0:
                        break
            continue
        if t in ('unsafe', 'async', 'default', 'auto'):
            j += 1
            continue
        if t == 'const' and j + 1 < len(code) and code[j + 1].text in ('fn', 'unsafe', 'async', 'extern'):
            j += 1
            continue
        if t == 'extern' and j + 1 < len(code) and code[j + 1].kind == 'str':
            # extern "C" fn / extern "C" { }
            if j + 2 < len(code) and code[j + 2].text in ('fn', 'unsafe'):
                j += 2
                continue
        break
    kw = code[j].text if j < len(code) else ''
    return j, kw


def _name_after(code, j, kw):
    if kw in ('fn', 'struct', 'enum', 'union', 'trait', 'mod', 'type', 'static', 'const'):
        k = j + 1
        if kw == 'static' and code[k].text == 'mut':
            k += 1
        return code[k].text
    if kw == 'macro_rules':
        return code[j + 2].text  # macro_rules ! name
    if kw == 'extern':
        if code[j + 1].text == 'crate':
            return 'crate ' + code[j + 2].text
        return 'block'
    if kw in ('impl', 'use'):
        parts = []
        k = j + 1
        while k < len(code) and code[k].text not in ('{', ';'):
            parts.append(code[k].text)
            k += 1
        return _join(parts)
    # item-level macro invocation, e.g. thread_local! { }
    if j + 1 < len(code) and code[j + 1].text == '!':
        return code[j].text + '!'
    return code[j].text


def _join(parts):
    out = ''
    for p in parts:
        if out and (out[-1].isalnum() or out[-1] in '_') and (p[0].isalnum() or p[0] in "_'"):
            out += ' '
        out += p
    return out


def items(src, lo=0, hi=None, prefix='', toks=None):
    """Items of src between byte offsets lo and hi (a body), as dicts."""
    if toks is None:
        toks = lex(src)
    if hi is None:
        hi = len(src)
    allt = [t for t in toks if t.start >= lo and t.end <= hi]
    code = code_tokens(allt)
    out = []
    counts = {}
    k = 0
    prev_end = lo
    while k < len(code):
        if code[k].text == '#' and k + 1 < len(code) and code[k + 1].text == '!':
            # an inner attribute stands alone: record it as its own item, ending at its ']'
            depth, m = 0, k + 2
            while True:
                if code[m].text in OPEN:
                    depth += 1
                elif code[m].text in CLOSE:
                    depth -= 1
                m += 1
                if depth == 0:
                    break
            span_start = prev_end
            for t in allt:
                if t.start >= prev_end and t.kind != 'ws':
                    span_start = t.start
                    break
            path = (prefix + '/' if prefix else '') + 'inner_attribute #!'
            counts[path] = counts.get(path, 0) + 1
            out.append({'path': path, 'kind': 'inner_attribute', 'name': '#!', 'ordinal': counts[path],
                        'span': [span_start, code[m - 1].end], 'decl': [code[k].start, code[m - 1].end]})
            prev_end = code[m - 1].end
            k = m
            continue
        j, kw = _item_header(code, k)
        if kw == 'macro_rules':
            kind = 'macro_rules'
        elif j + 1 < len(code) and code[j + 1].text == '!' and kw not in KEYWORDS_ITEM:
            kind = 'macro'
        else:
            kind = kw
        name = _name_after(code, j, kw)
        semi_only = kind in SEMI_ITEMS or (kind == 'extern' and name.startswith('crate'))
        depth = 0
        m = j
        end_tok = None
        body_open = None
        while m < len(code):
            t = code[m].text
            if t in OPEN:
                if depth == 0 and t == '{' and body_open is None:
                    body_open = m
                depth += 1
            elif t in CLOSE:
                depth -= 1
                if depth == 0 and t == '}' and not semi_only:
                    end_tok = m
                    break
            elif t == ';' and depth == 0:
                end_tok = m
                break
            m += 1
        if end_tok is None:
            raise ValueError('item %s %s at %d has no end' % (kind, name, code[k].start))
        # span starts after whitespace following the previous item
        span_start = prev_end
        for t in allt:
            if t.start >= prev_end and t.kind != 'ws':
                span_start = t.start
                break
        decl_start = code[k].start
        end = code[end_tok].end
        path = (prefix + '/' if prefix else '') + '%s %s' % (kind, name)
        counts[path] = counts.get(path, 0) + 1
        item = {
            'path': path, 'kind': kind, 'name': name, 'ordinal': counts[path],
            'span': [span_start, end], 'decl': [decl_start, end],
        }
        out.append(item)
        if kind in ('impl', 'trait', 'mod') and body_open is not None and code[end_tok].text == '}':
            inner_lo = code[body_open].end
            inner_hi = code[end_tok].start
            item['children'] = items(src, inner_lo, inner_hi, path, toks)
        prev_end = end
        k = end_tok + 1
    return out


def flatten(its):
    for it in its:
        yield it
        yield from flatten(it.get('children', []))


def annotate(data, its):
    """Adds sha256 and line numbers (1-based, for reading) to every item."""
    line_starts = [0]
    for i, b in enumerate(data):
        if b == 0x0A:
            line_starts.append(i + 1)
    import bisect

    def line_of(off):
        return bisect.bisect_right(line_starts, off)

    for it in flatten(its):
        for key in ('span', 'decl'):
            a, b = it[key]
            it[key + '_sha256'] = hashlib.sha256(data[a:b]).hexdigest()
        it['lines'] = [line_of(it['decl'][0]), line_of(it['decl'][1] - 1)]
    return its


def byte_offsets(src_str, data):
    """Map str offsets to byte offsets (UTF-8)."""
    offs = [0] * (len(src_str) + 1)
    b = 0
    for i, ch in enumerate(src_str):
        offs[i] = b
        b += len(ch.encode('utf-8'))
    offs[len(src_str)] = b
    assert b == len(data)
    return offs


def extract(data):
    """Items of a file's bytes, with byte offsets, hashes and lines; checks coverage."""
    src = data.decode('utf-8')
    toks = lex(src)
    # the tokens tile the file exactly
    pos = 0
    for t in toks:
        assert t.start == pos, ('gap in tokens at', pos)
        pos = t.end
    assert pos == len(src)
    its = items(src, toks=toks)
    offs = byte_offsets(src, data)
    for it in flatten(its):
        for key in ('span', 'decl'):
            it[key] = [offs[it[key][0]], offs[it[key][1]]]
    # top-level coverage: spans are ordered, disjoint, and only trivia lies between
    prev = 0
    for it in its:
        a, b = it['span']
        assert a >= prev, ('overlap', it['path'])
        gap = data[prev:a]
        assert gap.strip() == b'', ('non-trivia gap before', it['path'])
        prev = b
    tail = data[prev:].decode('utf-8')
    tail_toks = lex(tail)
    assert all(t.kind in ('ws', 'line_comment', 'block_comment') for t in tail_toks), 'code after last item'
    return annotate(data, its), toks, offs


def identifiers(toks, offs, lo, hi):
    """Identifier tokens (byte range [lo, hi)) as {text: [byte offsets]}."""
    out = {}
    for t in toks:
        if t.kind == 'ident' and offs[t.start] >= lo and offs[t.end] <= hi:
            out.setdefault(t.text, []).append(offs[t.start])
    return out


if __name__ == '__main__':
    data = open(sys.argv[1], 'rb').read()
    its, _, _ = extract(data)
    for it in flatten(its):
        print('%-60s #%d bytes %d-%d lines %d-%d %s' % (
            it['path'][:60], it['ordinal'], it['decl'][0], it['decl'][1], it['lines'][0], it['lines'][1],
            it['decl_sha256'][:12]))
