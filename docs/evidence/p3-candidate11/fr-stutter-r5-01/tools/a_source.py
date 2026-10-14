"""Checker A, part 2: exact-source access and the target-occurrence scan.

Sources are read from git objects of the frozen commit, never from a
working tree. An override map (path -> file) substitutes content for the
negative controls only; it never writes into the repository.
"""
import hashlib
import json
import re
import subprocess

import a_items

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
CRATE = "crates/nexus-governed-control/src/"
BLOBS = {
    "ledger/protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
    "ledger/protocol/model_tests.rs": "28085ce232827301104b5d757af042ec933976f8",
    "ledger/protocol/unit_tests.rs": "55dc66409eaccdaa93e8f0a56f2ccb14be81e114",
}
VARIANTS = ("Nothing", "Barrier", "Resolve", "AppendC", "AppendX", "EmptyX")
EXACT = ("replays", "Replay", "step_replay", "last")
REPLAY_RUN = re.compile(r"[A-Za-z0-9_]*[Rr][Ee][Pp][Ll][Aa][Yy][A-Za-z0-9_]*")


def git(*args):
    return subprocess.run(["git", "-C", REPO, *args], check=True, capture_output=True).stdout


def crate_files():
    out = git("ls-tree", "-r", C, CRATE).decode()
    files = []
    for line in out.splitlines():
        meta, path = line.split("\t", 1)
        mode, typ, blob = meta.split()
        if path.endswith(".rs"):
            files.append((path[len(CRATE):], blob))
    return files


class Source:
    def __init__(self, overrides=None):
        self.overrides = overrides or {}
        self.files = crate_files()
        self.cache = {}

    def text(self, rel):
        if rel in self.cache:
            return self.cache[rel]
        blob = dict(self.files)[rel]
        if rel in self.overrides:
            data = open(self.overrides[rel], "rb").read()
        else:
            data = git("cat-file", "blob", blob)
            if rel in BLOBS and blob != BLOBS[rel]:
                raise SystemExit(f"STOP: {rel} blob {blob} != authenticated {BLOBS[rel]}")
        txt = data.decode("utf-8")
        self.cache[rel] = (data, txt)
        return self.cache[rel]

    def blob(self, rel):
        return dict(self.files)[rel]


def scan_file(rel, data, txt):
    """Target occurrences of one file, each with region and enclosing item."""
    root, its, toks, pos = a_items.items(txt)
    occ = []
    sig = [t for t in toks if t.kind != "comment"]
    sig_index = {id(t): k for k, t in enumerate(sig)}

    def encl(offset):
        it = a_items.innermost(its, offset)
        return it.name if it else "<module>"

    def rec(group, token, region, t_off, line, col):
        occ.append({
            "file": rel, "line": line, "col": col,
            "byte": len(txt[:t_off].encode("utf-8")),
            "token": token, "region": region, "group": group,
            "item": encl(t_off),
        })

    for t in toks:
        if t.kind == "ident":
            k = sig_index[id(t)]
            prev = sig[k - 1] if k > 0 else None
            if t.text in EXACT:
                rec("exact", t.text, "code", t.start, t.line, t.col)
            elif t.text == "name" and prev is not None and prev.kind == "punct" and prev.text == ".":
                rec("name", ".name", "code", t.start, t.line, t.col)
            elif t.text == "Action" and k + 3 < len(sig) and sig[k + 1].text == ":" and sig[k + 2].text == ":" \
                    and sig[k + 3].kind == "ident" and sig[k + 3].text in VARIANTS:
                rec("action", "Action::" + sig[k + 3].text, "code", t.start, t.line, t.col)
            elif REPLAY_RUN.fullmatch(t.text):
                rec("lexical", t.text, "code", t.start, t.line, t.col)
        elif t.kind in ("comment", "str"):
            region = "comment" if t.kind == "comment" else "string"
            for m in REPLAY_RUN.finditer(t.text):
                off = t.start + m.start()
                line, col = pos(off)
                rec("lexical", m.group(0), region, off, line, col)
    return occ, its


def scan(src):
    allocc = []
    item_index = {}
    for rel, blob in src.files:
        data, txt = src.text(rel)
        occ, its = scan_file(rel, data, txt)
        allocc.extend(occ)
        item_index[rel] = its
    allocc.sort(key=lambda o: (o["file"], o["byte"]))
    return allocc, item_index


def ordinals(occ):
    """Ordinal of each occurrence within (file, item, region, token)."""
    seen = {}
    for o in occ:
        key = (o["file"], o["item"], o["region"], o["token"])
        seen[key] = seen.get(key, 0) + 1
        o["ordinal"] = seen[key]
    return occ


def sha(b):
    return hashlib.sha256(b).hexdigest()


def line_text(txt, line):
    return txt.split("\n")[line - 1]
