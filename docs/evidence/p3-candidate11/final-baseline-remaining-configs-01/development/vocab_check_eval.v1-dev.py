#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 5: static CHECK B — label derivation by evaluation.

  vocab_check_eval.py VOCABULARY.json [--source-dir DIR] [--dump]

Independent of the generator and of check A: no inventory, no anchors. It tokenises the crate's Rust files at C with
its own lexer, finds every `.branch(` call whose receiver's declared type is `Obs`, parses the enclosing function into
an expression tree, and evaluates the call's argument over every value the program can give it:

  - a function parameter or an operation's result ranges over its declared type: an enum over its declared variants
    (payloads recursively; Option as None/Some), bool over both values; a struct is opaque until a field is read, when
    the field ranges over the field's declared type; anything else is UNKNOWN;
  - `run_op(|| core.op(..))` is Option of the op's declared return type (the signature of run_op is asserted);
  - match arms are tried in order with Rust's semantics (patterns, or-patterns, bindings, guards; an arm consumes a
    value only when its pattern surely matches and it has no guard or a surely-true guard); an UNKNOWN scrutinee or
    guard may take every arm;
  - `let` bindings keep correlation (one world per value), `if` takes both ways when its condition is UNKNOWN;
  - format! renders `{x}` (strings) and `{x:?}` (derived Debug); an UNKNOWN or unbounded value reaching a label is a
    failure, never dropped.

It then compares, site by site, the derived (operation, label) set with the vocabulary's; any difference fails.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402

CHECKER_VERSION = "vocab_check_eval v1"


# ---- lexer ------------------------------------------------------------------------------------------------------

class Tok:
    __slots__ = ("kind", "val", "pos")

    def __init__(self, kind, val, pos):
        self.kind, self.val, self.pos = kind, val, pos

    def __repr__(self):
        return f"{self.kind}:{self.val!r}"


PUNCT = ["..=", "...", "<<=", ">>=", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=",
         "%=", "^=", "&=", "|=", "<<", ">>", "..",
         "+", "-", "*", "/", "%", "^", "!", "&", "|", "=", "<", ">", "@", ".", ",", ";", ":", "#", "$", "?", "~",
         "{", "}", "[", "]", "(", ")"]
ESC = {"n": "\n", "t": "\t", "r": "\r", "0": "\0", "\\": "\\", "'": "'", '"': '"'}


def lex(text):
    toks, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith("//", i):
            i = text.find("\n", i)
            i = n if i < 0 else i
        elif text.startswith("/*", i):
            d, i = 1, i + 2
            while d:
                if text.startswith("/*", i):
                    d, i = d + 1, i + 2
                elif text.startswith("*/", i):
                    d, i = d - 1, i + 2
                else:
                    i += 1
        elif c == '"' or (c in "br" and re.match(r'(b?r#*"|b")', text[i:i + 6])):
            m = re.match(r'(b?)(r?)(#*)"', text[i:])
            raw, hashes = m.group(2), m.group(3)
            j = i + len(m.group(0))
            if raw:
                end = text.index('"' + hashes, j)
                val, k = text[j:end], end + 1 + len(hashes)
            else:
                out = []
                while text[j] != '"':
                    if text[j] == "\\":
                        nx = text[j + 1]
                        if nx == "\n":
                            j += 2
                            while text[j] in " \t\n":
                                j += 1
                            continue
                        if nx == "u":
                            e = text.index("}", j)
                            out.append(chr(int(text[j + 3:e], 16)))
                            j = e + 1
                            continue
                        if nx == "x":
                            out.append(chr(int(text[j + 2:j + 4], 16)))
                            j += 4
                            continue
                        out.append(ESC[nx])
                        j += 2
                    else:
                        out.append(text[j])
                        j += 1
                val, k = "".join(out), j + 1
            toks.append(Tok("str", val, i))
            i = k
        elif c == "'":
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\x[0-9a-fA-F]{2}|\\.|[^\\'])'", text[i:])
            if m:
                toks.append(Tok("char", m.group(1), i))
                i += len(m.group(0))
            else:
                m = re.match(r"'[A-Za-z_]\w*", text[i:])
                toks.append(Tok("life", m.group(0), i))
                i += len(m.group(0))
        elif c.isdigit():
            m = re.match(r"0x[0-9a-fA-F_]+(?:[iu]\d+|[iu]size)?|0b[01_]+(?:[iu]\d+)?|\d[\d_]*(?:\.\d[\d_]*)?(?:e[+-]?\d+)?"
                         r"(?:[iu]\d+|[iu]size|f32|f64)?", text[i:])
            toks.append(Tok("num", m.group(0), i))
            i += len(m.group(0))
        elif c.isalpha() or c == "_":
            m = re.match(r"r#\w+|\w+", text[i:])
            toks.append(Tok("id", m.group(0), i))
            i += len(m.group(0))
        else:
            for p in PUNCT:
                if text.startswith(p, i):
                    toks.append(Tok("p", p, i))
                    i += len(p)
                    break
            else:
                raise SystemExit(f"lex: unexpected {c!r} at {i}")
    return toks


# ---- parser -------------------------------------------------------------------------------------------------------

class N:
    """An AST node: kind and fields."""

    def __init__(self, kind, **kw):
        self.kind = kind
        self.__dict__.update(kw)

    def __repr__(self):
        return f"N({self.kind})"


class ParseError(Exception):
    pass


BIN = [("||",), ("&&",), ("==", "!=", "<", ">", "<=", ">="), ("|",), ("^",), ("&",), ("<<", ">>"), ("+", "-"),
       ("*", "/", "%")]
BLOCKLIKE = {"if", "match", "loop", "while", "for", "unsafe"}


class Parser:
    def __init__(self, toks, i=0):
        self.t, self.i = toks, i

    def peek(self, k=0):
        j = self.i + k
        return self.t[j] if j < len(self.t) else Tok("eof", None, -1)

    def at(self, val, k=0):
        tk = self.peek(k)
        return tk.kind in ("p", "id") and tk.val == val

    def eat(self, val):
        if not self.at(val):
            raise ParseError(f"expected {val!r}, found {self.peek()!r} at token {self.i}")
        self.i += 1

    def skip_balanced(self):
        """Skip one balanced group starting at ( [ {."""
        pairs = {"(": ")", "[": "]", "{": "}"}
        stack = [pairs[self.peek().val]]
        self.i += 1
        while stack:
            tk = self.peek()
            if tk.kind == "eof":
                raise ParseError("unbalanced")
            if tk.kind == "p" and tk.val in pairs:
                stack.append(pairs[tk.val])
            elif tk.kind == "p" and tk.val in ")]}":
                if stack.pop() != tk.val:
                    raise ParseError("mismatched")
            self.i += 1

    def skip_generics(self):
        if not self.at("<"):
            return
        depth = 0
        while True:
            tk = self.peek()
            if tk.kind == "p" and tk.val == "<":
                depth += 1
            elif tk.kind == "p" and tk.val == ">":
                depth -= 1
            elif tk.kind == "p" and tk.val == ">>":
                depth -= 2
            elif tk.kind == "p" and tk.val == "->":
                pass
            self.i += 1
            if depth <= 0:
                return

    def type_text(self, stops):
        """Collect a type's tokens until a stop token at depth 0; returns the type text."""
        depth, out = 0, []
        while True:
            tk = self.peek()
            if tk.kind == "eof":
                break
            if depth == 0 and tk.kind == "p" and tk.val in stops:
                break
            if tk.kind == "p" and tk.val in ("<", "(", "["):
                depth += 1
            elif tk.kind == "p" and tk.val in (">", ")", "]"):
                depth -= 1
            elif tk.kind == "p" and tk.val == ">>":
                depth -= 2
            elif tk.kind == "p" and tk.val == "->":
                pass
            out.append(tk.val)
            self.i += 1
        return type_str(out)

    # Blocks and statements.
    def block(self):
        start = self.peek().pos
        self.eat("{")
        stmts, tail = [], None
        while not self.at("}"):
            if self.at(";"):
                self.i += 1
                continue
            if self.at("#"):
                self.i += 1
                if self.at("!"):
                    self.i += 1
                self.skip_balanced()
                continue
            if self.at("let"):
                stmts.append(self.let())
                continue
            if self.peek().kind == "id" and self.peek().val in ("fn", "const", "static", "struct", "enum", "use",
                                                                   "impl", "type", "mod"):
                self.skip_item()
                continue
            e = self.expr()
            if self.at(";"):
                self.i += 1
                stmts.append(N("expr_stmt", e=e))
            elif self.at("}"):
                tail = e
            elif e.kind in BLOCKLIKE or e.kind in ("block", "if", "match", "loop", "while", "for"):
                stmts.append(N("expr_stmt", e=e))
            else:
                raise ParseError(f"statement end expected at {self.peek()!r}")
        self.eat("}")
        return N("block", stmts=stmts, tail=tail, pos=start)

    def skip_item(self):
        while not (self.at(";") or self.at("{")):
            if self.at("(") or self.at("["):
                self.skip_balanced()
            else:
                self.i += 1
        if self.at(";"):
            self.i += 1
        else:
            self.skip_balanced()

    def let(self):
        self.eat("let")
        pat = self.pattern()
        ty = None
        if self.at(":"):
            self.i += 1
            ty = self.type_text({"=", ";"})
        init = els = None
        if self.at("="):
            self.i += 1
            init = self.expr()
            if self.at("else"):
                self.i += 1
                els = self.block()
        self.eat(";")
        return N("let", pat=pat, ty=ty, init=init, els=els)

    # Patterns.
    def pattern(self):
        first = self.pattern1()
        alts = [first]
        while self.at("|"):
            self.i += 1
            alts.append(self.pattern1())
        return alts[0] if len(alts) == 1 else N("p_or", alts=alts)

    def pattern1(self):
        tk = self.peek()
        if self.at("|"):
            self.i += 1
            return self.pattern1()
        if self.at("&"):
            self.i += 1
            if self.at("mut"):
                self.i += 1
            return N("p_ref", sub=self.pattern1())
        if self.at("("):
            self.i += 1
            subs = []
            while not self.at(")"):
                subs.append(self.pattern())
                if self.at(","):
                    self.i += 1
            self.eat(")")
            return subs[0] if len(subs) == 1 and not self.t[self.i - 2].val == "," else N("p_tuple", subs=subs)
        if self.at("["):
            self.skip_balanced()
            return N("p_unknown")
        if self.at(".."):
            self.i += 1
            return N("p_rest")
        if tk.kind in ("num", "str", "char") or self.at("-"):
            if self.at("-"):
                self.i += 1
            v = self.peek()
            self.i += 1
            if self.at("..=") or self.at(".."):
                self.i += 1
                self.i += 1
                return N("p_unknown")
            return N("p_lit", val=lit_value(v))
        if tk.kind == "id" and tk.val in ("true", "false"):
            self.i += 1
            return N("p_lit", val=("bool", tk.val == "true"))
        if tk.kind == "id" and tk.val == "_":
            self.i += 1
            return N("p_wild")
        if tk.kind == "id" and tk.val in ("ref", "mut"):
            self.i += 1
            if self.at("mut"):
                self.i += 1
            return self.pattern1()
        if tk.kind == "id":
            path = self.path_segments()
            if self.at("("):
                self.i += 1
                subs = []
                while not self.at(")"):
                    subs.append(self.pattern())
                    if self.at(","):
                        self.i += 1
                self.eat(")")
                return N("p_tstruct", path=path, subs=subs)
            if self.at("{"):
                self.i += 1
                fields, rest = [], False
                while not self.at("}"):
                    if self.at(".."):
                        self.i += 1
                        rest = True
                        continue
                    if self.at("ref") or self.at("mut"):
                        self.i += 1
                    fname = self.peek().val
                    self.i += 1
                    if self.at(":"):
                        self.i += 1
                        fields.append((fname, self.pattern()))
                    else:
                        fields.append((fname, N("p_ident", name=fname)))
                    if self.at(","):
                        self.i += 1
                self.eat("}")
                return N("p_struct", path=path, fields=fields, rest=rest)
            if self.at("@"):
                self.i += 1
                sub = self.pattern1()
                return N("p_bind", name=path[0], sub=sub)
            if len(path) == 1 and (path[0][0].islower() or path[0][0] == "_"):
                return N("p_ident", name=path[0])
            return N("p_path", path=path)
        raise ParseError(f"pattern at {tk!r}")

    def path_segments(self):
        segs = []
        if self.at("::"):
            self.i += 1
        while True:
            tk = self.peek()
            if tk.kind != "id":
                raise ParseError(f"path at {tk!r}")
            segs.append(tk.val)
            self.i += 1
            if self.at("::"):
                self.i += 1
                if self.at("<"):
                    self.skip_generics()
                    if self.at("::"):
                        self.i += 1
                        continue
                    break
                continue
            break
        return segs

    # Expressions.
    def expr(self, no_struct=False):
        if self.at("return"):
            self.i += 1
            if self.at(";") or self.at("}") or self.at(",") or self.at(")"):
                return N("return", e=None)
            return N("return", e=self.expr(no_struct))
        if self.at("break") or self.at("continue"):
            self.i += 1
            if self.peek().kind == "life":
                self.i += 1
            if not (self.at(";") or self.at("}") or self.at(",")):
                self.expr(no_struct)
            return N("return", e=None)
        if self.at("|") or self.at("||") or self.at("move"):
            return self.closure()
        lhs = self.binary(0, no_struct)
        if self.at("=") or self.peek().val in ("+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "<<=", ">>="):
            op = self.peek().val
            self.i += 1
            rhs = self.expr(no_struct)
            return N("assign", target=lhs, op=op, e=rhs)
        if self.at("..") or self.at("..="):
            self.i += 1
            if not (self.at(")") or self.at("]") or self.at(";") or self.at(",") or self.at("{")):
                self.binary(0, no_struct)
            return N("unknown", why="range")
        return lhs

    def closure(self):
        if self.at("move"):
            self.i += 1
        params = []
        if self.at("||"):
            self.i += 1
        else:
            self.eat("|")
            while not self.at("|"):
                p = self.pattern1()
                if self.at(":"):
                    self.i += 1
                    self.type_text({",", "|"})
                params.append(p)
                if self.at(","):
                    self.i += 1
            self.eat("|")
        if self.at("->"):
            self.i += 1
            self.type_text({"{"})
        body = self.expr()
        return N("closure", params=params, body=body)

    def binary(self, level, no_struct):
        if level == len(BIN):
            return self.cast(no_struct)
        lhs = self.binary(level + 1, no_struct)
        while self.peek().kind == "p" and self.peek().val in BIN[level]:
            op = self.peek().val
            self.i += 1
            rhs = self.binary(level + 1, no_struct)
            lhs = N("bin", op=op, l=lhs, r=rhs)
        return lhs

    def cast(self, no_struct):
        e = self.unary(no_struct)
        while self.at("as"):
            self.i += 1
            self.type_text({")", ",", ";", "}", "]", "==", "!=", "<", ">", "+", "-", "*", "/", "&&", "||", "{", "=>"})
            e = N("unknown", why="cast")
        return e

    def unary(self, no_struct):
        if self.at("!"):
            self.i += 1
            return N("not", e=self.unary(no_struct))
        if self.at("-"):
            self.i += 1
            self.unary(no_struct)
            return N("unknown", why="negation")
        if self.at("*"):
            self.i += 1
            return self.unary(no_struct)
        if self.at("&") or self.at("&&"):
            self.i += 1
            if self.at("mut"):
                self.i += 1
            return self.unary(no_struct)
        return self.postfix(self.primary(no_struct), no_struct)

    def postfix(self, e, no_struct):
        while True:
            if self.at("?"):
                self.i += 1
                e = N("unknown", why="try")
            elif self.at("."):
                self.i += 1
                tk = self.peek()
                self.i += 1
                if tk.kind == "num":
                    e = N("field", e=e, name=tk.val)
                    continue
                if tk.val == "await":
                    continue
                if self.at("::"):
                    self.i += 1
                    self.skip_generics()
                if self.at("("):
                    args = self.args(")")
                    e = N("mcall", recv=e, name=tk.val, args=args, pos=tk.pos)
                else:
                    e = N("field", e=e, name=tk.val)
            elif self.at("("):
                args = self.args(")")
                e = N("call", f=e, args=args)
            elif self.at("["):
                self.i += 1
                idx = self.expr()
                self.eat("]")
                e = N("index", e=e, idx=idx)
            else:
                return e

    def args(self, close):
        self.i += 1
        out = []
        while not self.at(close):
            out.append(self.expr())
            if self.at(","):
                self.i += 1
        self.eat(close)
        return out

    def primary(self, no_struct):
        tk = self.peek()
        if tk.kind == "str":
            self.i += 1
            return N("lit", val=("str", tk.val))
        if tk.kind in ("num", "char"):
            self.i += 1
            return N("lit", val=lit_value(tk))
        if tk.kind == "life":
            self.i += 1
            self.eat(":")
            return self.primary(no_struct)
        if self.at("("):
            self.i += 1
            items, comma = [], False
            while not self.at(")"):
                items.append(self.expr())
                if self.at(","):
                    comma = True
                    self.i += 1
            self.eat(")")
            return items[0] if len(items) == 1 and not comma else N("tuple", items=items)
        if self.at("["):
            self.i += 1
            items = []
            while not self.at("]"):
                items.append(self.expr())
                if self.at(";"):
                    self.i += 1
                    self.expr()
                    items = None
                    break
                if self.at(","):
                    self.i += 1
            self.eat("]")
            return N("array", items=items)
        if self.at("{"):
            return self.block()
        if self.at("unsafe"):
            self.i += 1
            return self.block()
        if self.at("if"):
            return self.if_expr()
        if self.at("match"):
            self.i += 1
            scrut = self.expr(no_struct=True)
            self.eat("{")
            arms = []
            while not self.at("}"):
                if self.at("#"):
                    self.i += 1
                    self.skip_balanced()
                    continue
                pat = self.pattern()
                guard = None
                if self.at("if"):
                    self.i += 1
                    guard = self.expr(no_struct=True)
                self.eat("=>")
                body = self.expr()
                arms.append((pat, guard, body))
                if self.at(","):
                    self.i += 1
            self.eat("}")
            return N("match", scrut=scrut, arms=arms)
        if self.at("loop"):
            self.i += 1
            return N("loop", body=self.block())
        if self.at("while"):
            self.i += 1
            if self.at("let"):
                self.i += 1
                pat = self.pattern()
                self.eat("=")
                self.expr(no_struct=True)
            else:
                self.expr(no_struct=True)
            return N("loop", body=self.block())
        if self.at("for"):
            self.i += 1
            pat = self.pattern()
            self.eat("in")
            self.expr(no_struct=True)
            return N("for", pat=pat, body=self.block())
        if tk.kind == "id" and tk.val in ("true", "false"):
            self.i += 1
            return N("lit", val=("bool", tk.val == "true"))
        if tk.kind == "id" or self.at("::") or self.at("<"):
            if self.at("<"):
                self.skip_generics()
                if self.at("::"):
                    self.i += 1
            path = self.path_segments()
            if self.at("!"):
                self.i += 1
                return self.macro(path[-1])
            if self.at("{") and not no_struct and (path[-1][0].isupper()):
                return self.struct_lit(path)
            return N("path", path=path)
        raise ParseError(f"expression at {tk!r}")

    def if_expr(self):
        self.eat("if")
        if self.at("let"):
            self.i += 1
            pat = self.pattern()
            self.eat("=")
            scrut = self.expr(no_struct=True)
            then = self.block()
            els = None
            if self.at("else"):
                self.i += 1
                els = self.if_expr() if self.at("if") else self.block()
            return N("iflet", pat=pat, scrut=scrut, then=then, els=els)
        cond = self.expr(no_struct=True)
        then = self.block()
        els = None
        if self.at("else"):
            self.i += 1
            els = self.if_expr() if self.at("if") else self.block()
        return N("if", cond=cond, then=then, els=els)

    def struct_lit(self, path):
        self.i += 1
        fields, base = [], None
        while not self.at("}"):
            if self.at(".."):
                self.i += 1
                base = self.expr()
                continue
            name = self.peek().val
            self.i += 1
            if self.at(":"):
                self.i += 1
                fields.append((name, self.expr()))
            else:
                fields.append((name, N("path", path=[name])))
            if self.at(","):
                self.i += 1
        self.eat("}")
        return N("struct_lit", path=path, fields=fields, base=base)

    def macro(self, name):
        open_ = self.peek().val
        close = {"(": ")", "[": "]", "{": "}"}[open_]
        if name in ("format", "matches", "vec") and open_ in "([":
            start = self.i
            try:
                if name == "matches":
                    self.i += 1
                    e = self.expr()
                    self.eat(",")
                    pat = self.pattern()
                    guard = None
                    if self.at("if"):
                        self.i += 1
                        guard = self.expr()
                    if self.at(","):
                        self.i += 1
                    self.eat(close)
                    return N("matches", e=e, pat=pat, guard=guard)
                if name == "vec":
                    self.skip_balanced()
                    return N("unknown", why="vec!")
                self.i += 1
                fmt = self.peek()
                if fmt.kind != "str":
                    raise ParseError("format! without a literal")
                self.i += 1
                args, named = [], {}
                while self.at(","):
                    self.i += 1
                    if self.at(close):
                        break
                    if self.peek().kind == "id" and self.at("=", 1) and not self.at("==", 1):
                        nm = self.peek().val
                        self.i += 2
                        named[nm] = self.expr()
                    else:
                        args.append(self.expr())
                self.eat(close)
                return N("format", fmt=fmt.val, args=args, named=named)
            except ParseError:
                self.i = start
        self.skip_balanced()
        return N("unknown", why=f"{name}!")


def type_str(tokens):
    s = ""
    for t in tokens:
        if s and (t[0].isalnum() or t[0] == "_") and (s[-1].isalnum() or s[-1] == "_"):
            s += " "
        s += t
    return s


def lit_value(tk):
    if tk.kind == "num":
        m = re.match(r"(\d+)", tk.val.replace("_", ""))
        return ("int", int(m.group(1))) if m and not tk.val.startswith("0x") else ("int", None)
    if tk.kind == "char":
        return ("char", tk.val)
    if tk.kind == "str":
        return ("str", tk.val)
    raise ParseError("literal")


# ---- declarations ---------------------------------------------------------------------------------------------------

class Decls:
    def __init__(self, files):
        self.enums, self.structs, self.fns, self.consts = {}, {}, {}, {}
        self.toks = {}
        for path, text in files.items():
            toks = lex(text)
            self.toks[path] = (toks, text)
            self.scan(path, toks)

    def scan(self, path, toks):
        i = 0
        while i < len(toks):
            tk = toks[i]
            if tk.kind == "id" and tk.val == "enum" and toks[i + 1].kind == "id":
                name = toks[i + 1].val
                p = Parser(toks, i + 2)
                p.skip_generics()
                if p.at("{"):
                    self.enums.setdefault(name, []).append((path, self.enum_body(p)))
                i += 2
                continue
            if tk.kind == "id" and tk.val == "struct" and toks[i + 1].kind == "id":
                name = toks[i + 1].val
                p = Parser(toks, i + 2)
                p.skip_generics()
                if p.at("{"):
                    p.i += 1
                    fields = {}
                    while not p.at("}"):
                        if p.at("#"):
                            p.i += 1
                            p.skip_balanced()
                            continue
                        if p.at("pub"):
                            p.i += 1
                            if p.at("("):
                                p.skip_balanced()
                        fname = p.peek().val
                        p.i += 1
                        p.eat(":")
                        fields[fname] = p.type_text({",", "}"})
                        if p.at(","):
                            p.i += 1
                    self.structs.setdefault(name, []).append((path, fields))
                elif p.at("("):
                    self.structs.setdefault(name, []).append((path, None))
                i += 2
                continue
            if tk.kind == "id" and tk.val == "fn" and toks[i + 1].kind == "id":
                name = toks[i + 1].val
                p = Parser(toks, i + 2)
                generics = None
                if p.at("<"):
                    g0 = p.i
                    p.skip_generics()
                    generics = type_str([t.val for t in toks[g0:p.i]])
                p.eat("(")
                params = []
                while not p.at(")"):
                    if p.at("&") or p.at("mut") or (p.at("self")):
                        while not (p.at(",") or p.at(")")):
                            p.i += 1
                        params.append(("self", "Self"))
                    else:
                        pat = p.pattern1()
                        p.eat(":")
                        ty = p.type_text({",", ")"})
                        params.append((pat.name if pat.kind == "p_ident" else None, ty))
                    if p.at(","):
                        p.i += 1
                p.eat(")")
                ret = None
                if p.at("->"):
                    p.i += 1
                    ret = p.type_text({"{", ";", "where"})
                if p.at("where"):
                    while not p.at("{"):
                        p.i += 1
                body_at = p.i if p.at("{") else None
                self.fns.setdefault(name, []).append(dict(file=path, params=params, ret=ret, body_at=body_at,
                                                           generics=generics, tok=i))
                i += 2
                continue
            if tk.kind == "id" and tk.val == "const" and toks[i + 1].kind == "id" and toks[i + 2].val == ":":
                name = toks[i + 1].val
                p = Parser(toks, i + 3)
                ty = p.type_text({"="})
                p.eat("=")
                try:
                    e = p.expr()
                    self.consts[name] = (ty, e)
                except ParseError:
                    pass
                i += 2
                continue
            i += 1

    def enum_body(self, p):
        p.eat("{")
        variants = []
        while not p.at("}"):
            if p.at("#"):
                p.i += 1
                p.skip_balanced()
                continue
            name = p.peek().val
            p.i += 1
            if p.at("("):
                p.i += 1
                tys = []
                while not p.at(")"):
                    tys.append(p.type_text({",", ")"}))
                    if p.at(","):
                        p.i += 1
                p.eat(")")
                variants.append((name, "tuple", tys))
            elif p.at("{"):
                p.i += 1
                fields = []
                while not p.at("}"):
                    if p.at("#"):
                        p.i += 1
                        p.skip_balanced()
                        continue
                    fname = p.peek().val
                    p.i += 1
                    p.eat(":")
                    fields.append((fname, p.type_text({",", "}"})))
                    if p.at(","):
                        p.i += 1
                p.eat("}")
                variants.append((name, "struct", fields))
            else:
                variants.append((name, "unit", []))
            if p.at("="):
                while not (p.at(",") or p.at("}")):
                    p.i += 1
            if p.at(","):
                p.i += 1
        p.eat("}")
        return variants

    def enum(self, name):
        d = self.enums.get(name)
        if not d:
            return None
        if len(d) > 1:
            return "AMBIGUOUS"
        return d[0][1]

    def struct(self, name):
        d = self.structs.get(name)
        if not d or len(d) > 1:
            return None
        return d[0][1]


# ---- abstract values --------------------------------------------------------------------------------------------------

UNKNOWN = ("unknown",)


class Unresolved(Exception):
    pass


def base_type(t):
    t = t.strip()
    t = re.sub(r"^&\s*('\w+\s+)?(mut\s+)?", "", t).strip()
    return t


def dom(decls, ty, depth=0):
    """Every value of a declared type, as far as it is finite; otherwise [UNKNOWN] or an opaque struct."""
    if depth > 8:
        return [UNKNOWN]
    t = base_type(ty)
    if t in ("Self",):
        return [UNKNOWN]
    if t == "bool":
        return [("bool", True), ("bool", False)]
    m = re.match(r"^Option\s*<(.*)>$", t)
    if m:
        return [("enum", "Option", "None", ())] + [("enum", "Option", "Some", (v,)) for v in dom(decls, m.group(1),
                                                                                                   depth + 1)]
    if re.match(r"^\w+$", t):
        e = decls.enum(t)
        if e == "AMBIGUOUS":
            raise Unresolved(f"enum {t} declared more than once")
        if e is not None:
            out = []
            for name, kind, payload in e:
                if kind == "unit":
                    out.append(("enum", t, name, ()))
                elif kind == "tuple":
                    for combo in product([dom(decls, p, depth + 1) for p in payload]):
                        out.append(("enum", t, name, tuple(combo)))
                else:
                    for combo in product([dom(decls, p, depth + 1) for _, p in payload]):
                        out.append(("enum", t, name, tuple((f, v) for (f, _), v in zip(payload, combo))))
            return out
        if decls.struct(t) is not None:
            return [("struct", t)]
    return [UNKNOWN]


def product(lists):
    out = [[]]
    for l in lists:
        out = [o + [x] for o in out for x in l]
    return out


def debug(v):
    k = v[0]
    if k == "str":
        return json.dumps(v[1])
    if k == "bool":
        return "true" if v[1] else "false"
    if k == "enum":
        _, t, name, payload = v
        if not payload:
            return name
        if isinstance(payload[0], tuple) and len(payload[0]) == 2 and isinstance(payload[0][0], str) \
                and not payload[0][0] in ("str", "bool", "enum", "struct", "unknown", "int", "tuple", "closure", "ctor"):
            return name + " { " + ", ".join(f"{f}: {debug(x)}" for f, x in payload) + " }"
        return name + "(" + ", ".join(debug(x) for x in payload) + ")"
    raise Unresolved(f"Debug of an unbounded value {v[0]}{(' ' + v[1]) if len(v) > 1 and isinstance(v[1], str) else ''}")


def display(v):
    if v[0] == "str":
        return v[1]
    if v[0] == "bool":
        return "true" if v[1] else "false"
    raise Unresolved(f"Display of {v[0]}")


def render(fmt, args, named, env, ev):
    """[strings] of a format! over every combination of its arguments' values."""
    pieces, i, auto = [], 0, 0
    holes = []
    while i < len(fmt):
        if fmt.startswith("{{", i):
            pieces.append(("lit", "{"))
            i += 2
        elif fmt.startswith("}}", i):
            pieces.append(("lit", "}"))
            i += 2
        elif fmt[i] == "{":
            j = fmt.index("}", i)
            spec = fmt[i + 1:j]
            name, _, f = spec.partition(":")
            if f not in ("", "?"):
                raise Unresolved(f"format spec {spec!r}")
            if name == "":
                src = ("pos", auto)
                auto += 1
            elif name.isdigit():
                src = ("pos", int(name))
            else:
                src = ("name", name)
            pieces.append(("hole", src, f == "?"))
            holes.append(src)
            i = j + 1
        else:
            j = i
            while j < len(fmt) and fmt[j] not in "{}":
                j += 1
            pieces.append(("lit", fmt[i:j]))
            i = j
    # Each distinct argument is evaluated once per world (correlation within one format! is kept by the environment).
    srcs = sorted(set(holes), key=str)
    choices = []
    for s in srcs:
        if s[0] == "pos":
            choices.append(ev(args[s[1]], env))
        elif s[1] in named:
            choices.append(ev(named[s[1]], env))
        else:
            choices.append(ev(N("path", path=[s[1]]), env))
    out = []
    for combo in product(choices):
        val = dict(zip(srcs, combo))
        s = ""
        for p in pieces:
            if p[0] == "lit":
                s += p[1]
            else:
                v = val[p[1]]
                if v == UNKNOWN:
                    raise Unresolved(f"format hole {p[1]} is UNKNOWN")
                s += debug(v) if p[2] else display(v)
        out.append(("str", s))
    return out


# ---- the evaluator -----------------------------------------------------------------------------------------------------

class Evaluator:
    def __init__(self, decls):
        self.d = decls
        self.notes = []

    # Pattern matching: returns "yes" (with bindings), "maybe" (with bindings) or "no".
    def match(self, pat, v, env):
        k = pat.kind
        if k == "p_wild" or k == "p_rest":
            return "yes", {}
        if k == "p_ident":
            if v != UNKNOWN and v[0] == "enum" and not v[3] and pat.name == v[2]:
                return "yes", {}
            return "yes", {pat.name: v}
        if k == "p_bind":
            r, b = self.match(pat.sub, v, env)
            b = dict(b)
            b[pat.name] = v
            return r, b
        if k == "p_ref":
            return self.match(pat.sub, v, env)
        if k == "p_or":
            res, binds = "no", {}
            for a in pat.alts:
                r, b = self.match(a, v, env)
                if r == "yes":
                    return ("yes", b) if res == "no" else ("maybe", b)
                if r == "maybe":
                    res, binds = "maybe", b
            return res, binds
        if v == UNKNOWN or v[0] == "struct":
            # Bind every identifier of the pattern to UNKNOWN.
            return "maybe", {n: UNKNOWN for n in pat_names(pat)}
        if k == "p_lit":
            if v[0] == pat.val[0]:
                if v[1] is None or pat.val[1] is None:
                    return "maybe", {}
                return ("yes" if v[1] == pat.val[1] else "no"), {}
            return "maybe", {}
        if k == "p_path":
            name = pat.path[-1]
            if v[0] == "enum":
                return ("yes" if v[2] == name and not v[3] else "no"), {}
            return "maybe", {}
        if k == "p_tstruct":
            name = pat.path[-1]
            if v[0] != "enum":
                return "maybe", {n: UNKNOWN for n in pat_names(pat)}
            if v[2] != name:
                return "no", {}
            subs = [s for s in pat.subs]
            payload = v[3]
            if any(s.kind == "p_rest" for s in subs):
                subs = [s for s in subs if s.kind != "p_rest"]
                payload = payload[:len(subs)]
            if len(subs) != len(payload):
                raise Unresolved(f"pattern {name} arity {len(subs)} vs payload {len(payload)}")
            res, binds = "yes", {}
            for s, x in zip(subs, payload):
                r, b = self.match(s, x, env)
                if r == "no":
                    return "no", {}
                if r == "maybe":
                    res = "maybe"
                binds.update(b)
            return res, binds
        if k == "p_struct":
            name = pat.path[-1]
            if v[0] != "enum":
                return "maybe", {n: UNKNOWN for n in pat_names(pat)}
            if v[2] != name:
                return "no", {}
            fields = dict(v[3])
            res, binds = "yes", {}
            for fname, s in pat.fields:
                if fname not in fields:
                    raise Unresolved(f"pattern field {name}.{fname} not in the variant")
                r, b = self.match(s, fields[fname], env)
                if r == "no":
                    return "no", {}
                if r == "maybe":
                    res = "maybe"
                binds.update(b)
            if not pat.rest and len(pat.fields) != len(fields):
                raise Unresolved(f"pattern {name} names {len(pat.fields)} of {len(fields)} fields without ..")
            return res, binds
        if k == "p_tuple":
            if v[0] != "tuple" or len(v[1]) != len(pat.subs):
                return "maybe", {n: UNKNOWN for n in pat_names(pat)}
            res, binds = "yes", {}
            for s, x in zip(pat.subs, v[1]):
                r, b = self.match(s, x, env)
                if r == "no":
                    return "no", {}
                if r == "maybe":
                    res = "maybe"
                binds.update(b)
            return res, binds
        return "maybe", {n: UNKNOWN for n in pat_names(pat)}

    def truth(self, vals):
        s = set()
        for v in vals:
            if v[0] == "bool":
                s.add(v[1])
            else:
                s.update((True, False))
        return s

    # Expressions → list of values (one per world).
    def ev(self, e, env):
        k = e.kind
        if k == "lit":
            return [e.val]
        if k == "path":
            if len(e.path) == 1:
                n = e.path[0]
                if n in env:
                    return [env[n]]
                if n in ("None",):
                    return [("enum", "Option", "None", ())]
                if n in self.d.consts:
                    return [("const", n)]
                if n in self.d.fns:
                    return [("fn", n)]
                return [UNKNOWN]
            t, name = e.path[-2], e.path[-1]
            en = self.d.enum(t)
            if en and en != "AMBIGUOUS":
                for vname, kind, payload in en:
                    if vname == name:
                        return [("enum", t, name, ())] if kind == "unit" else [("ctor", t, name)]
            return [UNKNOWN]
        if k == "tuple":
            return [("tuple", tuple(c)) for c in product([self.ev(x, env) for x in e.items])]
        if k == "format":
            return render(e.fmt, e.args, e.named, env, self.ev)
        if k == "block":
            return self.block(e, env)
        if k == "if":
            out = []
            for t in self.truth(self.ev(e.cond, env)):
                if t:
                    out += self.ev(e.then, env)
                elif e.els is not None:
                    out += self.ev(e.els, env)
                else:
                    out.append(("tuple", ()))
            return out
        if k == "match":
            out = []
            for v in self.ev(e.scrut, env):
                out += self.arms(e.arms, v, env)
            return out
        if k == "not":
            return [("bool", not t) for t in self.truth(self.ev(e.e, env))]
        if k == "bin":
            if e.op in ("&&", "||"):
                l = self.truth(self.ev(e.l, env))
                r = self.truth(self.ev(e.r, env))
                f = (lambda a, b: a and b) if e.op == "&&" else (lambda a, b: a or b)
                return [("bool", x) for x in {f(a, b) for a in l for b in r}]
            if e.op in ("==", "!="):
                out = set()
                for a in self.ev(e.l, env):
                    for b in self.ev(e.r, env):
                        if a == UNKNOWN or b == UNKNOWN or a[0] in ("struct", "int") or b[0] in ("struct", "int"):
                            out.update([("bool", True), ("bool", False)])
                        else:
                            out.add(("bool", (a == b) == (e.op == "==")))
                return list(out)
            if e.op in ("<", ">", "<=", ">="):
                return [("bool", True), ("bool", False)]
            return [UNKNOWN]
        if k == "field":
            out = []
            for b in self.ev(e.e, env):
                if b == UNKNOWN:
                    out.append(UNKNOWN)
                elif b[0] == "struct":
                    fields = self.d.struct(b[1])
                    if fields is None or e.name not in fields:
                        out.append(UNKNOWN)
                    else:
                        out += dom(self.d, fields[e.name])
                elif b[0] == "tuple" and e.name.isdigit():
                    out.append(b[1][int(e.name)])
                elif b[0] == "enum" and b[3] and isinstance(b[3][0], tuple) and len(b[3][0]) == 2 \
                        and isinstance(b[3][0][0], str) and e.name in dict(b[3]):
                    out.append(dict(b[3])[e.name])
                else:
                    out.append(UNKNOWN)
            return out
        if k == "index":
            out = []
            for b in self.ev(e.e, env):
                if b[0] == "const":
                    ty, init = self.d.consts[b[1]]
                    if init.kind != "array" or init.items is None:
                        out.append(UNKNOWN)
                        continue
                    idxs = self.ev(e.idx, env)
                    items = [self.ev(x, {})[0] for x in init.items]
                    for ix in idxs:
                        if ix[0] == "int" and ix[1] is not None:
                            out.append(items[ix[1]])
                        else:
                            out += items
                else:
                    out.append(UNKNOWN)
            return out
        if k == "closure":
            return [("closure", e, env)]
        if k == "call":
            return self.call(e, env)
        if k == "mcall":
            return self.mcall(e, env)
        if k == "matches":
            out = set()
            for v in self.ev(e.e, env):
                r, b = self.match(e.pat, v, env)
                if r == "no":
                    out.add(("bool", False))
                elif r == "yes" and e.guard is None:
                    out.add(("bool", True))
                else:
                    out.update([("bool", True), ("bool", False)])
            return list(out)
        if k == "return":
            return []
        if k in ("loop", "for", "while", "assign", "array", "struct_lit", "unknown", "iflet"):
            return [UNKNOWN]
        raise Unresolved(f"expression kind {k}")

    def arms(self, arms, v, env):
        out = []
        for pat, guard, body in arms:
            r, b = self.match(pat, v, env)
            if r == "no":
                continue
            env2 = dict(env)
            env2.update(b)
            truths = {True} if guard is None else self.truth(self.ev(guard, env2))
            if True in truths:
                out += self.ev(body, env2)
            if r == "yes" and truths == {True}:
                return out
        return out

    def block(self, blk, env, upto=None):
        """Values of a block's tail, threading `let` worlds; statements that bind nothing are not evaluated."""
        def run(i, env):
            if i == len(blk.stmts):
                return self.ev(blk.tail, env) if blk.tail is not None else [("tuple", ())]
            s = blk.stmts[i]
            if s.kind == "let":
                if s.init is None:
                    return run(i + 1, {**env, **{n: UNKNOWN for n in pat_names(s.pat)}})
                out = []
                vals = self.ev(s.init, env) if s.ty is None else self.typed_let(s, env)
                for v in vals:
                    r, b = self.match(s.pat, v, env)
                    if r == "no":
                        continue
                    out += run(i + 1, {**env, **b})
                return out
            if s.kind == "expr_stmt" and s.e.kind == "return":
                return []
            if s.kind == "expr_stmt" and s.e.kind == "assign" and s.e.target.kind == "path":
                return run(i + 1, {**env, s.e.target.path[0]: UNKNOWN})
            return run(i + 1, env)
        return run(0, env)

    def typed_let(self, s, env):
        vals = self.ev(s.init, env)
        if all(v == UNKNOWN for v in vals):
            return dom(self.d, s.ty)
        return vals

    def call(self, e, env):
        out = []
        for f in self.ev(e.f, env):
            if f[0] == "ctor":
                for combo in product([self.ev(a, env) for a in e.args]):
                    out.append(("enum", f[1], f[2], tuple(combo)))
            elif f == UNKNOWN and e.f.kind == "path" and e.f.path[-1] == "Some":
                for v in self.ev(e.args[0], env):
                    out.append(("enum", "Option", "Some", (v,)))
            elif f[0] == "fn":
                out += self.user_fn(f[1], e.args, env)
            elif f[0] == "closure":
                out += self.apply(f, [self.ev(a, env) for a in e.args])
            else:
                out.append(UNKNOWN)
        return out

    def apply(self, clo, argvals):
        _, node, cenv = clo
        out = []
        for combo in product(argvals):
            env2 = dict(cenv)
            for p, v in zip(node.params, combo):
                r, b = self.match(p, v, env2)
                env2.update(b)
            out += self.ev(node.body, env2)
        return out

    def user_fn(self, name, args, env):
        defs = self.d.fns[name]
        if name == "run_op":
            d = defs[0]
            if len(defs) != 1 or d["generics"] != "<T>" or d["params"] != [("op", "impl FnOnce() -> T")] \
                    or d["ret"] != "Option<T>":
                raise Unresolved(f"run_op's signature is not the asserted one: {defs}")
            out = [("enum", "Option", "None", ())]
            for clo in self.ev(args[0], env):
                if clo[0] != "closure":
                    raise Unresolved("run_op without a closure")
                out += [("enum", "Option", "Some", (v,)) for v in self.apply(clo, [])]
            return out
        if len(defs) != 1:
            rets = {d["ret"] for d in defs}
            if len(rets) == 1 and None not in rets:
                return dom(self.d, rets.pop())
            raise Unresolved(f"fn {name} is ambiguous")
        d = defs[0]
        if d["body_at"] is None:
            return dom(self.d, d["ret"]) if d["ret"] else [UNKNOWN]
        body = self.parse_body(d)
        out = []
        for combo in product([self.ev(a, env) for a in args]):
            env2 = {}
            for (pname, pty), v in zip(d["params"], combo):
                if pname:
                    env2[pname] = v if v != UNKNOWN else UNKNOWN
            out += self.block(body, env2)
        return out

    def parse_body(self, d):
        toks, _ = self.d.toks[d["file"]]
        return Parser(toks, d["body_at"]).block()

    def mcall(self, e, env):
        name = e.name
        out = []
        for r in self.ev(e.recv, env):
            if name in ("to_string", "into", "clone", "to_owned", "as_str", "as_ref", "as_mut", "borrow"):
                out.append(r)
            elif name == "map_or" and r[0] == "enum" and r[1] == "Option":
                if r[2] == "None":
                    out += self.ev(e.args[0], env)
                else:
                    for f in self.ev(e.args[1], env):
                        out += self.apply(f, [[r[3][0]]]) if f[0] == "closure" else [UNKNOWN]
            elif name == "map" and r[0] == "enum" and r[1] == "Option":
                if r[2] == "None":
                    out.append(r)
                else:
                    for f in self.ev(e.args[0], env):
                        if f[0] == "ctor":
                            out.append(("enum", "Option", "Some", (("enum", f[1], f[2], (r[3][0],)),)))
                        elif f[0] == "closure":
                            out += [("enum", "Option", "Some", (x,)) for x in self.apply(f, [[r[3][0]]])]
                        else:
                            out.append(UNKNOWN)
            elif name in ("is_some", "is_none") and r[0] == "enum" and r[1] == "Option":
                out.append(("bool", (r[2] == "Some") == (name == "is_some")))
            else:
                defs = [d for d in self.d.fns.get(name, []) if d["params"][:1] == [("self", "Self")]]
                rets = {d["ret"] for d in defs}
                if defs and len(rets) == 1 and None not in rets:
                    out += dom(self.d, rets.pop())
                elif defs and len(rets) > 1:
                    raise Unresolved(f"method {name} has several return types {rets}")
                else:
                    out.append(UNKNOWN)
        return out


def pat_names(p):
    k = p.kind
    if k == "p_ident":
        return [p.name]
    if k == "p_bind":
        return [p.name] + pat_names(p.sub)
    if k in ("p_tstruct", "p_tuple"):
        return [n for s in p.subs for n in pat_names(s)]
    if k == "p_struct":
        return [n for _, s in p.fields for n in pat_names(s)]
    if k == "p_or":
        return pat_names(p.alts[0])
    if k == "p_ref":
        return pat_names(p.sub)
    return []


# ---- path-directed evaluation of one call site ----------------------------------------------------------------------

def find_path(node, target, path=()):
    """The chain of (node, step) from node down to the target call node."""
    if node is target:
        return list(path)
    if not isinstance(node, N):
        return None
    for key, val in list(node.__dict__.items()):
        if key in ("kind", "pos"):
            continue
        items = val if isinstance(val, list) else [val]
        for idx, it in enumerate(items):
            if isinstance(it, tuple):  # match arms (pat, guard, body) and struct fields (name, expr)
                for j, sub in enumerate(it):
                    r = find_path(sub, target, path + ((node, key, idx, j),))
                    if r is not None:
                        return r
            elif isinstance(it, N):
                r = find_path(it, target, path + ((node, key, idx, None),))
                if r is not None:
                    return r
    return None


def find_call(node, pos):
    if isinstance(node, N):
        if node.kind == "mcall" and node.name == "branch" and getattr(node, "pos", None) == pos:
            return node
        for key, val in node.__dict__.items():
            items = val if isinstance(val, list) else [val]
            for it in items:
                for sub in (it if isinstance(it, tuple) else (it,)):
                    r = find_call(sub, pos)
                    if r is not None:
                        return r
    return None


def site_labels(ev, fn, call):
    """Every string the call's argument can be, from the function entry, along the path to the call."""
    body = ev.parse_body(fn)
    path = find_path(body, call)
    if path is None:
        raise Unresolved("call not found in the parsed body")
    env0 = {}
    for pname, pty in fn["params"]:
        if pname:
            vals = dom(ev.d, pty)
            env0[pname] = vals
    names = [n for n in env0]
    results = []
    for combo in product([env0[n] for n in names]):
        results += descend(ev, body, path, 0, dict(zip(names, combo)), call)
    return results


def descend(ev, node, path, k, env, call):
    if node is call:
        out = []
        for combo in product([ev.ev(a, env) for a in call.args]):
            out.append(combo[0])
        return out
    parent, key, idx, j = path[k]
    assert parent is node
    kind = node.kind
    if kind == "block":
        if key == "tail":
            return descend_block_prefix(ev, node, len(node.stmts), env,
                                        lambda env2: descend(ev, node.tail, path, k + 1, env2, call))
        stmt = node.stmts[idx]
        return descend_block_prefix(ev, node, idx, env, lambda env2: descend(ev, stmt, path, k + 1, env2, call))
    if kind == "expr_stmt":
        return descend(ev, node.e, path, k + 1, env, call)
    if kind == "let":
        if key == "init":
            return descend(ev, node.init, path, k + 1, env, call)
        return descend(ev, node.els, path, k + 1, env, call)
    if kind == "match":
        if key == "scrut":
            return descend(ev, node.scrut, path, k + 1, env, call)
        out = []
        target_arm = idx
        for v in ev.ev(node.scrut, env):
            for ai, (pat, guard, body) in enumerate(node.arms):
                r, b = ev.match(pat, v, env)
                if r == "no":
                    continue
                env2 = {**env, **b}
                truths = {True} if guard is None else ev.truth(ev.ev(guard, env2))
                if ai == target_arm:
                    if True in truths:
                        sub = {0: pat, 1: guard, 2: body}[j]
                        out += descend(ev, sub, path, k + 1, env2, call)
                    break
                if r == "yes" and truths == {True}:
                    break
        return out
    if kind == "if":
        if key == "cond":
            return descend(ev, node.cond, path, k + 1, env, call)
        truths = ev.truth(ev.ev(node.cond, env))
        want = key == "then"
        if want in truths:
            return descend(ev, getattr(node, key), path, k + 1, env, call)
        return []
    if kind == "iflet":
        if key == "scrut":
            return descend(ev, node.scrut, path, k + 1, env, call)
        out = []
        for v in ev.ev(node.scrut, env):
            r, b = ev.match(node.pat, v, env)
            if key == "then" and r != "no":
                out += descend(ev, node.then, path, k + 1, {**env, **b}, call)
            if key == "els" and r != "yes":
                out += descend(ev, node.els, path, k + 1, env, call)
        return out
    if kind in ("loop", "for"):
        env2 = dict(env)
        if kind == "for":
            env2.update({n: UNKNOWN for n in pat_names(node.pat)})
        return descend(ev, node.body, path, k + 1, env2, call)
    if kind == "closure":
        env2 = {**env, **{n: UNKNOWN for p in node.params for n in pat_names(p)}}
        return descend(ev, node.body, path, k + 1, env2, call)
    # Any other enclosing expression (a call's argument, a method receiver ...): the call is evaluated there.
    sub = getattr(node, key)
    sub = sub[idx] if isinstance(sub, list) else sub
    if isinstance(sub, tuple):
        sub = sub[j]
    return descend(ev, sub, path, k + 1, env, call)


def descend_block_prefix(ev, blk, upto, env, cont):
    def run(i, env):
        if i == upto:
            return cont(env)
        s = blk.stmts[i]
        if s.kind == "let":
            if s.init is None:
                return run(i + 1, {**env, **{n: UNKNOWN for n in pat_names(s.pat)}})
            names = pat_names(s.pat)
            if not any(uses(blk, j, n) for j in range(i + 1, len(blk.stmts) + 1) for n in names):
                return run(i + 1, {**env, **{n: UNKNOWN for n in names}})
            out = []
            vals = ev.typed_let(s, env) if s.ty else ev.ev(s.init, env)
            for v in vals:
                r, b = ev.match(s.pat, v, env)
                if r == "no":
                    continue
                out += run(i + 1, {**env, **b})
            return out
        if s.kind == "expr_stmt" and s.e.kind == "assign" and s.e.target.kind == "path":
            return run(i + 1, {**env, s.e.target.path[0]: UNKNOWN})
        if s.kind == "expr_stmt" and s.e.kind == "return":
            return []
        return run(i + 1, env)
    return run(0, env)


def uses(blk, j, name):
    node = blk.stmts[j] if j < len(blk.stmts) else blk.tail
    return name in idents(node)


def idents(node):
    out = set()
    if isinstance(node, N):
        if node.kind == "path" and len(node.path) == 1:
            out.add(node.path[0])
        if node.kind == "format":
            out.update(re.findall(r"\{([A-Za-z_]\w*)", node.fmt))
        for key, val in node.__dict__.items():
            items = val if isinstance(val, list) else [val]
            for it in items:
                for sub in (it if isinstance(it, tuple) else (it,)):
                    out |= idents(sub)
    return out


# ---- main ------------------------------------------------------------------------------------------------------------

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


def main(argv):
    vocab = json.load(open(argv[0]))
    source_dir = argv[argv.index("--source-dir") + 1] if "--source-dir" in argv else None
    texts = load(source_dir)
    lines, fails = [], []

    def check(label, ok, detail=""):
        lines.append(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
        if not ok:
            fails.append(label)

    # Declarations from the two protocol files (labels only interpolate their types); sites from every file.
    decls = Decls({p: texts[p] for p in (common.PROTOCOL, common.MODEL)})
    ev = Evaluator(decls)
    sites = []
    for p, text in texts.items():
        toks = decls.toks[p][0] if p in decls.toks else lex(text)
        fns = Decls.__new__(Decls)
        fns.enums, fns.structs, fns.fns, fns.consts, fns.toks = {}, {}, {}, {}, {p: (toks, text)}
        fns.scan(p, toks)
        spans = []
        for name, defs in fns.fns.items():
            for d in defs:
                if d["body_at"] is not None:
                    end = Parser(toks, d["body_at"])
                    end.skip_balanced()
                    spans.append((d["body_at"], end.i, name, d))
        for i, tk in enumerate(toks):
            if tk.kind == "id" and tk.val == "branch" and i + 1 < len(toks) and toks[i + 1].val == "(" \
                    and toks[i - 1].val == "." and toks[i - 2].kind == "id":
                recv = toks[i - 2].val
                encl = [s for s in spans if s[0] <= i < s[1]]
                encl = max(encl, key=lambda s: s[0]) if encl else None
                ty = None
                if encl:
                    for pname, pty in encl[3]["params"]:
                        if pname == recv:
                            ty = base_type(pty)
                if recv == "self" or (toks[i - 3].val == "." if i >= 3 else False):
                    ty = ty or "self-or-field"
                if ty is None and encl:
                    # a local binding: `let [mut] recv = Type::…` or `let [mut] recv = Type {`
                    seg = toks[encl[0]:i]
                    for j in range(len(seg) - 3):
                        if seg[j].val == "let" and (seg[j + 1].val == recv or (seg[j + 1].val == "mut" and
                                                                               seg[j + 2].val == recv)):
                            k2 = j + (3 if seg[j + 1].val == "mut" else 2)
                            if seg[k2].val == "=" and seg[k2 + 1].kind == "id":
                                ty = seg[k2 + 1].val
                sites.append(dict(file=p, line=text.count("\n", 0, tk.pos) + 1, recv=recv, type=ty,
                                  function=encl[2] if encl else None, fn=encl[3] if encl else None, tok=i, pos=tk.pos))
    obs_sites = [s for s in sites if s["type"] == "Obs"]
    others = [s for s in sites if s["type"] != "Obs"]
    check("B1 every .branch( call found by the lexer has a receiver of a known type",
          all(s["type"] in ("Obs", "Oracle", "self-or-field") for s in sites),
          f"{len(obs_sites)} Obs; others {[(s['file'].split('/')[-1], s['line'], s['recv'], s['type']) for s in others]}")
    if source_dir is None:
        check("B1 the self/field receivers are Io's oracle (a choice, not a label)",
              all(s["recv"] in ("oracle", "self", "o") for s in others if s["type"] == "self-or-field"),
              [(s["line"], s["recv"]) for s in others if s["type"] == "self-or-field"])

    derived = {}
    for s in obs_sites:
        if s["file"] not in decls.toks:
            check(f"B2 site {s['file']}:{s['line']} lies in a file whose declarations are parsed", False)
            continue
        toks = decls.toks[s["file"]][0]
        body = Parser(toks, s["fn"]["body_at"])
        try:
            call = find_call(ev.parse_body(s["fn"]), s["pos"])
            if call is None:
                raise Unresolved("the call node was not found")
            vals = site_labels(ev, s["fn"], call)
            labels = set()
            for v in vals:
                if v == UNKNOWN or v[0] != "str":
                    raise Unresolved(f"a label value is {v}")
                labels.add(v[1])
            derived[(s["function"], s["line"])] = labels
            lines.append(f"     site {s['file'].split('/')[-1]}:{s['line']} fn {s['function']}: {len(labels)} labels")
        except (Unresolved, ParseError, RecursionError) as ex:
            check(f"B2 site {s['file'].split('/')[-1]}:{s['line']} fn {s['function']} evaluated", False, str(ex))

    # Compare with the vocabulary, site by site.
    vsite = {(x["function"], x["line"]): x["site"] for x in vocab["sites"]}
    per_site = {}
    for r in vocab["rows"]:
        for s in r["sites"]:
            per_site.setdefault((s["function"], s["line"]), set()).add(r["label"])
    check("B3 the Obs sites the lexer finds equal the vocabulary's sites", set(derived) == set(vsite)
          and len(obs_sites) == len(vsite),
          f"derived {sorted(derived)}; vocabulary {sorted(vsite)}")
    for key in sorted(set(derived) | set(per_site)):
        got, want = derived.get(key, set()), per_site.get(key, set())
        check(f"B4 site {vsite.get(key, 'UNLISTED')} ({key[0]}:{key[1]}): derived labels equal the vocabulary's",
              got == want, f"{len(got)} derived, {len(want)} in the vocabulary; derived only "
              f"{sorted(got - want)}; vocabulary only {sorted(want - got)}")
    # Operation identity, and collisions.
    by_label = {}
    for key, labels in derived.items():
        for l in labels:
            by_label.setdefault(l, set()).add(vsite.get(key, f"UNLISTED {key}"))
    vrows = {r["label"]: r for r in vocab["rows"]}
    op_bad = [l for l in by_label if l in vrows and vrows[l]["operation"] != l.split(": ", 1)[0]]
    check("B5 every derived label's operation (its text before ': ') is the vocabulary row's operation", not op_bad,
          op_bad)
    resolved = {frozenset(c["sites"]) for c in vocab["collisions"] if c["resolved"]}
    coll = {l: sorted(s) for l, s in by_label.items() if len(s) > 1}
    unres = {l: s for l, s in coll.items() if frozenset(s) not in resolved}
    check("B6 every label derived at two sites is a collision the vocabulary records as resolved", not unres,
          f"collisions {coll}; unresolved {unres}")
    all_derived = set(by_label)
    check("B7 every vocabulary row is derived from source (no row without a source derivation)",
          set(vrows) <= all_derived, sorted(set(vrows) - all_derived))
    check("B7 every derived label is a vocabulary row", all_derived <= set(vrows), sorted(all_derived - set(vrows)))
    if "--dump" in argv:
        for key, labels in sorted(derived.items(), key=lambda kv: kv[0][1]):
            for l in sorted(labels):
                lines.append(f"     {key[0]}:{key[1]}  {l}")
    lines.append(f"CHECK B ({CHECKER_VERSION}): {'PASS' if not fails else 'FAIL: ' + '; '.join(fails)}")
    print("\n".join(lines))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.setrecursionlimit(20000)
    sys.exit(main(sys.argv[1:]))
