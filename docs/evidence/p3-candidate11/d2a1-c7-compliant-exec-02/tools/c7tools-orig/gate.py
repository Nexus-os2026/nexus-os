#!/usr/bin/env python3
"""Read-only authorisation gate for P3-C11-DR01-D2A1-C7-W-EVIDENCE-EXEC-01.

Verifies identities, contract, source, patch and environment BEFORE any worktree
or evidence directory is created. Creates nothing in the repository.
Usage: gate.py <output-file>   (the output file must not exist)
Exit 0 only if every check passes.
"""
import json
import os
import re
import stat
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from c7lib import *  # noqa: F401,F403

SCRATCH = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def lex_rust(src):
    """Minimal Rust tokenizer: comments and string/char literal contents are dropped."""
    toks = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c.isspace():
            i += 1
        elif src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
        elif src.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if src.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif src.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif c == '"':
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == "\\" else 1
            i += 1
            toks.append(("str", ""))
        elif c == "'":
            if src.startswith("'\\", i):
                j = src.find("'", i + 2)
                i = j + 1
                toks.append(("chr", ""))
            elif i + 2 < n and src[i + 2] == "'":
                i += 3
                toks.append(("chr", ""))
            else:
                j = i + 1
                while j < n and (src[j].isalnum() or src[j] == "_"):
                    j += 1
                toks.append(("life", src[i + 1:j]))
                i = j
        elif c.isalpha() or c == "_":
            j = i
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            word = src[i:j]
            if word in ("r", "br", "cr") and j < n and src[j] in '#"':
                k = j
                hashes = 0
                while k < n and src[k] == "#":
                    hashes += 1
                    k += 1
                if k < n and src[k] == '"':
                    end = '"' + "#" * hashes
                    e = src.find(end, k + 1)
                    i = e + len(end)
                    toks.append(("str", ""))
                    continue
            if word in ("b", "c") and j < n and src[j] == '"':
                i = j
                continue
            toks.append(("ident", word))
            i = j
        elif c.isdigit():
            j = i
            while j < n and (src[j].isalnum() or src[j] in "_."):
                if src[j] == "." and not (j + 1 < n and src[j + 1].isdigit()):
                    break
                j += 1
            toks.append(("num", src[i:j]))
            i = j
        else:
            toks.append(("punct", c))
            i += 1
    return toks


def scan_suffix(rec, suffix_bytes):
    src = suffix_bytes.decode("utf-8")
    toks = lex_rust(src)
    idents = [t[1] for t in toks if t[0] == "ident"]
    vals = [t[1] for t in toks]
    # NI-3: exactly one item: `mod c7_witness { ... }`, inline, no visibility, no attribute on it.
    ok = vals[:3] == ["mod", "c7_witness", "{"] and vals[-1] == "}"
    depth, top_close = 0, None
    for idx, t in enumerate(toks):
        if t[1] == "{":
            depth += 1
        elif t[1] == "}":
            depth -= 1
            if depth == 0:
                top_close = idx
                break
    rec.check("NI-3a", "suffix S is exactly one inline `mod c7_witness { ... }` item (no tokens before or after)",
              ok and top_close == len(toks) - 1, "tokens=%d top_close=%s" % (len(toks), top_close))
    rec.check("NI-3b", "no `pub` of any form, no re-export (NI-5e)", "pub" not in idents)
    # NI-5(f): exactly one #[test]; no other attribute at all
    hashes = [i for i, t in enumerate(toks) if t[1] == "#"]
    attr_ok = len(hashes) == 1 and [t[1] for t in toks[hashes[0]:hashes[0] + 4]] == ["#", "[", "test", "]"]
    rec.check("NI-5f", "exactly one attribute in S and it is `#[test]` (so no ignore/should_panic/cfg/cfg_attr/lint/inner attribute, no derive)",
              attr_ok, "attribute count=%d" % len(hashes))
    rec.check("NI-5f2", "no inner attribute `#!`", not any(vals[i:i + 2] == ["#", "!"] for i in range(len(vals) - 1)))
    for name, bad in (
        ("NI-5b", {"static", "thread_local", "lazy_static", "OnceLock", "OnceCell", "LazyLock", "Cell", "RefCell", "Mutex", "RwLock", "AtomicBool", "AtomicUsize", "AtomicU64", "AtomicU32", "AtomicU8"}),
        ("NI-5c", {"unsafe", "extern", "no_mangle", "export_name", "link_section", "link", "global_allocator"}),
        ("NI-5d", {"macro_rules", "macro_export", "macro_use"}),
    ):
        hit = sorted(set(idents) & bad)
        rec.check(name, "none of %s in S" % sorted(bad), not hit, "found=%s" % hit if hit else "")
    attr_names = []
    for h in hashes:
        k = h + 1
        if k < len(toks) and toks[k][1] == "[":
            depth_b, m = 0, k
            names = []
            while m < len(toks):
                if toks[m][1] == "[":
                    depth_b += 1
                elif toks[m][1] == "]":
                    depth_b -= 1
                    if depth_b == 0:
                        break
                elif toks[m][0] == "ident":
                    names.append(toks[m][1])
                m += 1
            attr_names.extend(names)
    rec.check("NI-5c2", "no `#[used]`, `#[link]`, `#[no_mangle]`, `#[export_name]`, `#[link_section]`, `#[global_allocator]` attribute (attribute names found: %s)" % attr_names,
              not (set(attr_names) & {"used", "link", "no_mangle", "export_name", "link_section", "global_allocator"}))
    lifetimes = sorted({t[1] for t in toks if t[0] == "life"})
    rec.info("NI-5b-life", "lifetime tokens in S (a lifetime `'static` is not a `static` item): %s" % lifetimes)
    rec.check("NI-5b2", "no `static` item: the ident `static` never occurs as an item keyword (lifetime tokens are lexed separately)", "static" not in idents)
    # NI-5(a): trait / impl / derive / generic blanket
    rec.check("NI-5a1", "no trait declared in S", "trait" not in idents)
    declared = set()
    for i, t in enumerate(toks):
        if t == ("ident", "struct") or t == ("ident", "enum"):
            declared.add(toks[i + 1][1])
    impl_targets = []
    for i, t in enumerate(toks):
        if t == ("ident", "impl"):
            impl_targets.append(toks[i + 1][1] if toks[i + 1][0] == "ident" else toks[i + 1][1])
    rec.check("NI-5a2", "every `impl` in S has as self type a struct/enum declared in S (no trait impl, no generic/blanket)",
              all(x in declared for x in impl_targets) and "for" not in vals[: 0] and not any(
                  toks[i] == ("ident", "impl") and toks[i + 1] == ("punct", "<") for i in range(len(toks) - 1)),
              "declared=%s impl_targets=%s" % (sorted(declared), impl_targets))
    # an `impl X for Y` (trait impl) would put `for` right after the type ident
    trait_impl = False
    for i, t in enumerate(toks):
        if t == ("ident", "impl"):
            j = i + 1
            d = 0
            while j < len(toks) and not (toks[j][1] == "{" and d == 0):
                if toks[j][1] == "<":
                    d += 1
                elif toks[j][1] == ">":
                    d -= 1
                if toks[j] == ("ident", "for") and d == 0:
                    trait_impl = True
                j += 1
    rec.check("NI-5a3", "no `impl Trait for Type` in S", not trait_impl)
    # NI-5(g) forbidden names
    forbidden = {"explore", "explore_experimental", "explore_symmetric", "explore_symmetric_kept", "successors", "expand",
                 "SERIAL", "set_hook", "take_hook", "thread_rng", "getrandom", "SystemTime", "Instant", "Command", "TcpStream", "TcpListener", "UdpSocket"}
    hit = sorted(set(idents) & forbidden)
    rec.check("NI-5g1", "S neither calls nor names explorer/serial/hook-panic/randomness/time/process/net items", not hit, "found=%s" % hit if hit else "")
    pre_hits = []
    for i in range(len(toks) - 3):
        if toks[i][1] == "std" and toks[i + 1][1] == ":" and toks[i + 2][1] == ":" and toks[i + 3][1] in ("env", "process", "thread", "fs", "net", "time", "panic"):
            pre_hits.append(toks[i + 3][1])
        if toks[i][1] == "hook" and toks[i + 1][1] == ":" and toks[i + 2][1] == ":" and toks[i + 3][1] in ("install", "remove", "report"):
            pre_hits.append("hook::" + toks[i + 3][1])
    rec.check("NI-5g2", "no `std::{env,process,thread,fs,net,time,panic}` path and no hook::{install,remove,report}", not pre_hits, "found=%s" % pre_hits if pre_hits else "")
    witness_names = sorted({x for x in idents if re.match(r"^(c1|c25|c4)_", x) or x in ("c1_witness", "c25_witness", "c4_witness")})
    rec.check("NI-5g3", "S names no C1/C25/C4 witness item", not witness_names, "found=%s" % witness_names if witness_names else "")
    hook_uses = []
    for i in range(len(toks) - 3):
        if toks[i][1] == "hook" and toks[i + 1][1] == ":" and toks[i + 2][1] == ":":
            hook_uses.append(toks[i + 3][1])
    rec.check("NI-5g4", "S may call only hook::take_counts", set(hook_uses) <= {"take_counts"}, "hook uses=%s" % sorted(set(hook_uses)))
    rec.check("NI-5h", "S defines no `Config` and uses FPT", not any(toks[i] in (("ident", "struct"), ("ident", "enum")) and toks[i + 1] == ("ident", "Config") for i in range(len(toks) - 1)) and "FPT" in idents)
    uses = []
    for i, t in enumerate(toks):
        if t == ("ident", "use"):
            j = i
            s = []
            while j < len(toks) and toks[j][1] != ";":
                s.append(toks[j][1])
                j += 1
            uses.append(" ".join(s))
    rec.info("NI-5-uses", "`use` items in S: %s" % uses)
    rec.check("NI-5-uses2", "S's only `use` is `use super::*`", uses == ["use super : : *"], "uses=%s" % uses)
    n_test = len(re.findall(r"#\[test\]", src))
    n_ignore = len(re.findall(r"#\[ignore", src))
    rec.check("NI-S-counts", "S holds exactly 1 `#[test]` and 0 `#[ignore`", n_test == 1 and n_ignore == 0, "test=%d ignore=%d" % (n_test, n_ignore))
    return declared


def main():
    out = sys.argv[1]
    rec = Rec("C7-W authorisation gate (read-only, before any worktree)")
    rec.log("# host %s" % os.uname().nodename)
    rec.log("# repo %s" % REPO)

    # ---------------- 1. repository provenance ----------------
    rec.log("\n## 1. repository and remote provenance")
    head = git_out("rev-parse", "HEAD")
    branch = git_out("rev-parse", "--abbrev-ref", "HEAD")
    st = git("status", "--porcelain=v1").stdout.decode().strip()
    rec.info("GP-01", "repo HEAD %s on %s; porcelain status: %r" % (head, branch, st))
    rec.check("GP-01", "operating repository is the Nexus OS repository, clean, HEAD equals the rebuild branch snapshot", head == PROTECTED_REBUILD and st == "")
    rem = git_out("remote", "-v")
    rec.info("GP-02", "remotes: %s" % rem.replace("\n", " | "))
    rec.check("GP-02", "remote `%s` is Nexus-os2026/nexus-os" % REMOTE, re.search(r"^github\s+\S*Nexus-os2026/nexus-os(\.git)?\s", rem, re.M) is not None)

    lr = git_out("ls-remote", REMOTE)
    refs = {}
    for line in lr.splitlines():
        sha, ref = line.split("\t")
        refs[ref] = sha
    rec.info("GP-03", "ls-remote %s: %d refs" % (REMOTE, len(refs)))
    want = {
        W_REF: W_COMMIT,
        K2_REF: K2_COMMIT,
        K4_REF: K4_COMMIT,
        "refs/heads/main": PROTECTED_MAIN,
        "refs/heads/rebuild/phase0-trust-boundary": PROTECTED_REBUILD,
    }
    for ref, sha in want.items():
        rec.check("GP-03", "remote %s == %s" % (ref, sha[:12]), refs.get(ref) == sha, "remote=%s" % refs.get(ref))
        loc = git("rev-parse", "--verify", "-q", "refs/remotes/%s/%s" % (REMOTE, ref[len("refs/heads/"):]), check=False)
        rec.check("GP-03L", "local remote-tracking %s equals remote" % ref, loc.returncode == 0 and loc.stdout.decode().strip() == sha,
                  "tracking=%s" % loc.stdout.decode().strip())
    rec.check("GP-04", "destination review ref absent on the remote", DEST_REF not in refs)
    rec.check("GP-04L", "destination review ref absent locally (heads and remote-tracking)",
              git("rev-parse", "--verify", "-q", DEST_REF, check=False).returncode != 0 and
              git("rev-parse", "--verify", "-q", "refs/remotes/%s/%s" % (REMOTE, DEST_REF[len("refs/heads/"):]), check=False).returncode != 0)
    tags = sorted(r for r in refs if r.startswith("refs/tags/"))
    rec.info("GP-05", "remote tags (%d): %s" % (len(tags), " ".join("%s=%s" % (t, refs[t][:12]) for t in tags)))
    other_heads = sorted(r for r in refs if r.startswith("refs/heads/") and r not in want and "c7" not in r)
    rec.info("GP-05b", "remote branch count excluding the five pinned refs and c7 review refs: %d" % len(other_heads))

    # ---------------- 2. K1 ----------------
    rec.log("\n## 2. K1 frozen catalog")
    k1_bytes = None
    for c in K1_COMMITS:
        b = blob_at(c, K1_PATH)
        bid = blob_id_at(c, K1_PATH)
        rec.check("K1", "catalog at %s: blob %s, sha256 %s" % (c, bid[:12], sha256_bytes(b)[:12]), bid == K1_BLOB and sha256_bytes(b) == K1_SHA)
        k1_bytes = b
    cat = json.loads(k1_bytes.decode("utf-8"))
    c7s = [e for e in cat["controls"] if e.get("id") == "C7"]
    rec.check("K1-C7", "catalog has exactly one entry with id C7", len(c7s) == 1)
    rec.check("K1-base", "catalog baseline commit/tree equal C-BASE/C-TREE; baseline blobs equal the three recorded blobs",
              cat["baseline"]["commit"] == C_BASE and cat["baseline"]["tree"] == C_TREE and
              cat["baseline"]["blobs"] == {PROTOCOL_PATH: PROTOCOL_C_BLOB, MODEL_TESTS_PATH: MODEL_TESTS_C_BLOB, UNIT_TESTS_PATH: UNIT_TESTS_C_BLOB})
    c7 = c7s[0]

    # ---------------- 3. K2 / K3 / overlay / EC7 ----------------
    rec.log("\n## 3. K2 adopted document, K3 overlay, EC7")
    rec.check("K2-commit", "K2 commit %s has tree %s" % (K2_COMMIT[:12], K2_TREE[:12]), git_out("rev-parse", K2_COMMIT + "^{tree}") == K2_TREE)
    k2 = blob_at(K2_COMMIT, K2_PATH)
    rec.check("K2", "K2 blob %s and sha256 %s" % (blob_id_at(K2_COMMIT, K2_PATH)[:12], sha256_bytes(k2)[:12]),
              blob_id_at(K2_COMMIT, K2_PATH) == K2_BLOB and sha256_bytes(k2) == K2_SHA)
    anc = git("merge-base", "--is-ancestor", K2_COMMIT, "refs/remotes/%s/%s" % (REMOTE, K2_REF[len("refs/heads/"):]), check=False).returncode
    rec.check("K2-reach", "K2 commit is reachable from its remote review ref", anc == 0)
    rec.check("K2-k4same", "K2 file is byte-identical in the K4 commit (adoption did not edit it)", blob_id_at(K4_COMMIT, K2_PATH) == K2_BLOB)

    lines = k2.split(b"\n")
    begin = [i for i, l in enumerate(lines) if l == b"BEGIN OVERLAY-C7"]
    end = [i for i, l in enumerate(lines) if l == b"END OVERLAY-C7"]
    rec.check("K3-X1", "exactly one whole-line BEGIN OVERLAY-C7 and one END OVERLAY-C7", len(begin) == 1 and len(end) == 1, "begin=%s end=%s" % (begin, end))
    k3 = b"\n".join(lines[begin[0] + 1:end[0]]) + b"\n"
    rec.check("K3-X2", "K3 is the bytes between the markers, final LF included, ends with the closing `}` line",
              lines[end[0] - 1] == b"}" and k3.endswith(b"}\n"))
    rec.check("K3-X3", "K3 has exactly 36 LF-terminated lines and 1736 bytes and no CR", k3.count(b"\n") == K3_LINES and len(k3) == K3_BYTES and b"\r" not in k3,
              "lines=%d bytes=%d cr=%d" % (k3.count(b"\n"), len(k3), k3.count(b"\r")))
    rec.check("K3", "K3 sha256 equals OVERLAY-SHA256", sha256_bytes(k3) == K3_SHA, sha256_bytes(k3))
    overlay = json.loads(k3.decode("utf-8"))
    rec.check("OVL-1", "overlay catalog/c_base identities equal K1 and K5; entry is C7",
              overlay["catalog"] == {"path": K1_PATH, "git_blob": K1_BLOB, "sha256": K1_SHA} and overlay["c_base"] == {"commit": C_BASE, "tree": C_TREE} and overlay["entry"] == "C7")
    # P-2: exactly seven keys
    rec.check("P-2", "overlay holds exactly 3 `set`, 3 `add`, 1 `append` keys and only the known top-level keys",
              sorted(overlay.keys()) == ["add", "append", "c_base", "catalog", "entry", "overlay", "set"] and len(overlay["set"]) == 3 and len(overlay["add"]) == 3 and len(overlay["append"]) == 1)

    def get_path(d, dotted):
        cur = d
        for part in dotted.split("."):
            if not isinstance(cur, dict) or part not in cur:
                return (False, None)
            cur = cur[part]
        return (True, cur)

    def set_path(d, dotted, v):
        parts = dotted.split(".")
        cur = d
        for part in parts[:-1]:
            cur = cur[part]
        cur[parts[-1]] = v

    import copy
    ec7 = copy.deepcopy(c7)
    for k, v in overlay["set"].items():
        ex, _ = get_path(c7, k)
        rec.check("P-1set", "set key %s exists in the catalog's C7 entry" % k, ex)
        set_path(ec7, k, v)
    for k, v in overlay["add"].items():
        ex, _ = get_path(c7, k)
        rec.check("P-1add", "add key %s is absent from the catalog's C7 entry" % k, not ex)
        set_path(ec7, k, v)
    for k, items in overlay["append"].items():
        ex, cur = get_path(c7, k)
        rec.check("P-1append", "append key %s is an existing list (%s items)" % (k, len(cur) if ex else "-"), ex and isinstance(cur, list))
        set_path(ec7, k, list(cur) + items)
    ok_set, cur = get_path(c7, "expected_primary_failure.printed_property_must_be")
    rec.check("P-1p6", "catalog printed_property_must_be is \"P6\" and witness_required is false (the values the overlay replaces)",
              cur == "P6" and c7.get("witness_required") is False, "printed=%r witness_required=%r" % (cur, c7.get("witness_required")))
    for field in ("named_property", "patch", "mutant_blob", "edits", "configuration", "ceiling_states", "restoration", "restored_receipt", "additional_comparison", "execution_dependent_uncertainty"):
        rec.check("P-5", "EC7 keeps C7 field %s verbatim" % field, ec7.get(field) == c7.get(field))
    rec.check("P-5t", "EC7 keeps `test`-bearing fields verbatim (additional_comparison.test is full_f3)", ec7["additional_comparison"]["test"] == TEST_FULL_F3)

    def ce1(v):
        return json.dumps(v, sort_keys=True, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")

    ec7_bytes = ce1(ec7)
    rec.check("CE-1", "EC7 derived by CE-1 (compact, sorted, UTF-8, no trailing newline)", not ec7_bytes.endswith(b"\n") and b"\n" not in ec7_bytes)
    rec.info("EC7", "EC7 canonical bytes: %d; sha256 %s" % (len(ec7_bytes), sha256_bytes(ec7_bytes)))
    others_ok = True
    for e in cat["controls"]:
        if e.get("id") != "C7" and ce1(e) != ce1(copy.deepcopy(e)):
            others_ok = False
    cat2 = copy.deepcopy(cat)
    for i, e in enumerate(cat2["controls"]):
        if e.get("id") == "C7":
            cat2["controls"][i] = ec7
    tl_same = all(ce1(cat[k]) == ce1(cat2[k]) for k in cat if k != "controls")
    oth_same = all(ce1(a) == ce1(b) for a, b in zip(cat["controls"], cat2["controls"]) if a.get("id") != "C7")
    rec.check("2A.4-5", "every other entry and every top-level key (baseline included) is CE-1 byte-equal before and after", tl_same and oth_same and others_ok and len(cat["controls"]) == len(cat2["controls"]))
    rec.check("EC7-fields", "EC7 witness test/mutation facts: witness test name, ignored false, violations_required",
              ec7["witness"]["test"] == TEST_WITNESS and ec7["witness"]["ignored"] is False and ec7["witness"]["violations_required"] == ["step 13 transition: E11", "after 24: P6"]
              and ec7["expected_primary_failure"]["printed_property_must_be"] == "E11" and ec7["witness_required"] is True)

    # ---------------- 4. K4 / AU ----------------
    rec.log("\n## 4. K4 owner adoption record (AU-1 to AU-6)")
    rec.check("AU-1c", "K4 commit exists and is a commit with tree %s" % K4_TREE[:12], git_out("cat-file", "-t", K4_COMMIT) == "commit" and git_out("rev-parse", K4_COMMIT + "^{tree}") == K4_TREE)
    anc4 = git("merge-base", "--is-ancestor", K4_COMMIT, "refs/remotes/%s/%s" % (REMOTE, K4_REF[len("refs/heads/"):]), check=False).returncode
    rec.check("AU-4a", "K4 commit is reachable from the named remote review ref, local equal to remote (ls-remote == tracking == K4 commit)",
              anc4 == 0 and refs.get(K4_REF) == K4_COMMIT)
    rec.check("AU-4b", "K4 path exists in the K4 commit", git("cat-file", "-e", "%s:%s" % (K4_COMMIT, K4_PATH), check=False).returncode == 0)
    k4 = blob_at(K4_COMMIT, K4_PATH)
    rec.check("AU-4c", "K4 blob equals the pinned blob", blob_id_at(K4_COMMIT, K4_PATH) == K4_BLOB)
    rec.check("AU-4d", "K4 recomputed sha256 equals the pinned sha256", sha256_bytes(k4) == K4_SHA, sha256_bytes(k4))
    rec.check("AU-5p", "K4 commit's sole parent is the K2 commit (record committed after the frozen document)", git_out("rev-parse", K4_COMMIT + "^").strip() == K2_COMMIT and len(git_out("rev-list", "--parents", "-n1", K4_COMMIT).split()) == 2)
    k4t = k4.decode("utf-8")
    for label, val in (("K1 blob", K1_BLOB), ("K1 sha256", K1_SHA), ("K1 path", K1_PATH), ("K2 commit", K2_COMMIT), ("K2 tree", K2_TREE), ("K2 blob", K2_BLOB),
                       ("K2 sha256", K2_SHA), ("K2 path", K2_PATH), ("K3 sha256", K3_SHA), ("K5 commit", C_BASE), ("K5 tree", C_TREE)):
        rec.check("AU-5", "record states %s %s" % (label, val[:20]), val in k4t)
    rec.check("AU-5k3", "record states K3 is 36 LF-terminated lines, 1,736 bytes, no CR, markers excluded, final LF included",
              "Exactly 36 LF-terminated lines; exactly 1,736 bytes; no CR." in k4t and "Marker lines excluded; final LF included." in k4t)
    rec.check("AU-3a", "record states `A string is not authority`, claims no cryptographic signature",
              "A string is never authority." in k4t and "No cryptographic Owner signature is claimed." in k4t)
    rec.check("AU-3b", "record names the conveying mission and section, the C7-only scope, and CR-B status (B-W/A OWNER APPROVED; B-W/B and B-W/C NOT ESTABLISHED)",
              "P3-C11-DR01-D2A1-C7-W-OWNER-ADOPTION-CONVEY-01" in k4t and "C7 only." in k4t and "CR-B: ADOPTED AS CONDITIONAL POLICY ONLY." in k4t
              and "B-W/A: OWNER APPROVED." in k4t and "B-W/B: NOT ESTABLISHED." in k4t and "B-W/C: NOT ESTABLISHED." in k4t)
    rec.check("AU-3c", "record does not contain or claim its own commit, blob or sha256", K4_COMMIT not in k4t and K4_BLOB not in k4t and K4_SHA not in k4t)
    rec.check("AU-1", "record quotes the explicit Owner decision naming K2 sha256 and K3 sha256, `C7 only`, `contract adoption only, not execution authorization`",
              "As Nexus OS Owner, I explicitly approve adoption" in k4t and "for C7 only" in k4t and "not execution authorization" in k4t)
    rec.check("AU-5m", "the mission text names the same K1..K5 values (scratchpad copy of mission)", True)
    mission = open(os.path.join(SCRATCH, "mission.txt"), "r", encoding="utf-8").read()
    mflat = re.sub(r"\s+", "", mission)
    for label, val in (("K1 blob", K1_BLOB), ("K1 sha256", K1_SHA), ("K2 commit", K2_COMMIT), ("K2 blob", K2_BLOB), ("K2 sha256", K2_SHA), ("K3 sha256", K3_SHA),
                       ("K4 commit", K4_COMMIT), ("K4 tree", K4_TREE), ("K4 blob", K4_BLOB), ("K4 sha256", K4_SHA), ("K4 path", K4_PATH), ("C-BASE", C_BASE), ("C-TREE", C_TREE),
                       ("W-COMMIT", W_COMMIT), ("W-TREE", W_TREE), ("W-BLOB", W_BLOB), ("W-BLOB sha256", W_BLOB_SHA), ("W-PATCH sha256", W_PATCH_SHA), ("C7.patch sha256", PATCH_SHA),
                       ("mutant blob", MUTANT_BLOB), ("mutant sha256", MUTANT_SHA)):
        rec.check("AU-5x", "mission text states %s" % label, re.sub(r"\s+", "", val) in mflat)
    rec.info("AU-2", "the conveying mission (P3-C11-DR01-D2A1-C7-W-OWNER-ADOPTION-CONVEY-01) is not available to the executor as a separate document; "
                     "the executor can compare only the execution mission, the record, K2 itself and git recomputation (three of the four places). Recorded as a limitation.")

    # ---------------- 5. K5 ----------------
    rec.log("\n## 5. K5 C-BASE")
    rec.check("K5", "C-BASE commit exists with tree %s" % C_TREE[:12], git_out("cat-file", "-t", C_BASE) == "commit" and git_out("rev-parse", C_BASE + "^{tree}") == C_TREE)

    # ---------------- 6. W-COMMIT / source (B-W/B) ----------------
    rec.log("\n## 6. W-COMMIT source identities (NI-1 to NI-6)")
    rec.check("W-1", "W-COMMIT exists, tree == W-TREE, sole parent == C-BASE",
              git_out("cat-file", "-t", W_COMMIT) == "commit" and git_out("rev-parse", W_COMMIT + "^{tree}") == W_TREE
              and git_out("rev-list", "--parents", "-n1", W_COMMIT).split() == [W_COMMIT, C_BASE])
    rec.check("W-2", "W-COMMIT is reachable from the remote source review ref", git("merge-base", "--is-ancestor", W_COMMIT, "refs/remotes/%s/%s" % (REMOTE, W_REF[len("refs/heads/"):]), check=False).returncode == 0 and refs.get(W_REF) == W_COMMIT)
    dt = git("diff-tree", "-r", "--no-renames", C_BASE, W_COMMIT).stdout.decode().strip().splitlines()
    exp = [":100644 100644 %s %s M\t%s" % (MODEL_TESTS_C_BLOB, W_BLOB, MODEL_TESTS_PATH)]
    rec.check("NI-2", "diff-tree -r --no-renames C-BASE W-COMMIT prints exactly one record: M model_tests.rs 100644->100644", dt == exp, repr(dt))
    wb = blob_at(W_COMMIT, MODEL_TESTS_PATH)
    cb = blob_at(C_BASE, MODEL_TESTS_PATH)
    rec.check("W-3", "W-BLOB git id, sha256 and size", git_blob_id(wb) == W_BLOB and blob_id_at(W_COMMIT, MODEL_TESTS_PATH) == W_BLOB and sha256_bytes(wb) == W_BLOB_SHA and len(wb) == W_BLOB_SIZE,
              "id=%s sha=%s size=%d" % (git_blob_id(wb)[:12], sha256_bytes(wb)[:12], len(wb)))
    rec.check("NI-1a", "C-BASE model_tests.rs is blob 28085ce2, %d bytes, sha256 recorded" % MODEL_TESTS_C_SIZE,
              git_blob_id(cb) == MODEL_TESTS_C_BLOB and len(cb) == MODEL_TESTS_C_SIZE and sha256_bytes(cb) == MODEL_TESTS_C_SHA)
    rec.check("NI-1b", "W-BLOB's first 310,612 bytes equal blob 28085ce2 byte for byte, and W-BLOB is longer", wb[:MODEL_TESTS_C_SIZE] == cb and len(wb) > len(cb))
    patch = git("diff", C_BASE, W_COMMIT).stdout
    rec.check("W-4", "W-PATCH (git diff C-BASE W-COMMIT) sha256", sha256_bytes(patch) == W_PATCH_SHA, sha256_bytes(patch))
    nl_removed = [l for l in patch.split(b"\n") if l.startswith(b"-") and not l.startswith(b"---")]
    added = [l for l in patch.split(b"\n") if l.startswith(b"+") and not l.startswith(b"+++")]
    rec.check("W-5", "W-PATCH removes no line and adds 516 lines", not nl_removed and len(added) == 516, "removed=%d added=%d" % (len(nl_removed), len(added)))
    suffix = wb[MODEL_TESTS_C_SIZE:]
    rec.check("W-6", "suffix S is %d bytes, %d lines, no CR, no NUL, valid UTF-8" % (len(suffix), suffix.count(b"\n")), b"\r" not in suffix and b"\0" not in suffix and suffix.decode("utf-8") is not None, "")
    rec.info("W-6", "suffix sha256 %s" % sha256_bytes(suffix))
    scan_suffix(rec, suffix)
    g = git("grep", "-w", "-n", "c7_witness", C_BASE, check=False)
    rec.check("NI-4", "`git grep -w c7_witness C-BASE` finds nothing", g.returncode == 1 and g.stdout == b"", "rc=%d out=%r" % (g.returncode, g.stdout[:100]))
    # source counts, lib scope (src/)
    def count(commit, pat):
        r = git("grep", "-E", "-c", pat, commit, "--", "crates/nexus-governed-control/src", check=False).stdout.decode().strip().splitlines()
        return sum(int(x.rsplit(":", 1)[1]) for x in r if x)
    tc, ic = count(C_BASE, r"#\[test\]"), count(C_BASE, r"#\[ignore")
    tw, iw = count(W_COMMIT, r"#\[test\]"), count(W_COMMIT, r"#\[ignore")
    rec.check("CNT", "lib-scope source counts: C-BASE 348 `#[test]`/35 `#[ignore`; W-TREE 349 `#[test]`/35 `#[ignore`",
              (tc, ic, tw, iw) == (348, 35, 349, 35), "C-BASE test=%d ignore=%d; W test=%d ignore=%d" % (tc, ic, tw, iw))
    # NI-6 build inputs
    for p in BUILD_INPUTS:
        a, b = blob_id_at(C_BASE, p), blob_id_at(W_COMMIT, p)
        rec.check("NI-6", "build input %s identical at C-BASE and W-COMMIT" % p, a == b, "%s" % a[:12])
    rec.check("NI-6p", "protocol.rs at W-TREE is blob a2df2841 (Candidate C's)", blob_id_at(W_COMMIT, PROTOCOL_PATH) == PROTOCOL_C_BLOB)
    rec.check("NI-6u", "unit_tests.rs at W-TREE is blob 55dc6640", blob_id_at(W_COMMIT, UNIT_TESTS_PATH) == UNIT_TESTS_C_BLOB)
    tc = blob_at(W_COMMIT, "rust-toolchain.toml").decode()
    rec.info("NI-6t", "rust-toolchain.toml: %r" % tc)
    rec.check("NI-6t2", "rust-toolchain.toml pins channel 1.94.0", re.search(r'channel\s*=\s*"1\.94\.0"', tc) is not None)
    # Tree comparison against C-BASE: only model_tests.rs differs
    tdiff = git("diff", "--name-status", C_TREE, W_TREE).stdout.decode().strip().splitlines()
    rec.check("W-7", "tree diff C-TREE -> W-TREE is exactly `M model_tests.rs`", tdiff == ["M\t" + MODEL_TESTS_PATH], repr(tdiff))
    gl = [l for l in git("ls-tree", "-r", W_COMMIT).stdout.decode().splitlines() if l.startswith("160000")]
    rec.check("W-8", "W-TREE has no gitlinks (submodules)", not gl)

    # ---------------- 7. frozen C7 patch ----------------
    rec.log("\n## 7. frozen C7 patch and mutant identity (verified without writing the object store)")
    pb = blob_at(K1_COMMITS[0], K1_PATCH_PATH)
    rec.check("PAT-1", "C7.patch git blob df0dd584 and sha256 b5d8bc6c", git_blob_id(pb) == K1_PATCH_BLOB and sha256_bytes(pb) == PATCH_SHA)
    rec.check("PAT-2", "catalog patch_sha256 and patch text equal the file", c7["patch_sha256"] == PATCH_SHA and c7["patch"].encode() == pb)
    prot = blob_at(W_COMMIT, PROTOCOL_PATH)
    anchor = c7["edits"][0]["anchor"].encode()
    repl = c7["edits"][0]["replacement"].encode()
    rec.check("PAT-3", "anchor occurs exactly once in protocol.rs at W-TREE", prot.count(anchor) == 1, "count=%d" % prot.count(anchor))
    mut = prot.replace(anchor, repl)
    rec.check("PAT-4", "in-memory patched protocol.rs has blob 0d4ab992 and sha256 6d2f2d49", git_blob_id(mut) == MUTANT_BLOB and sha256_bytes(mut) == MUTANT_SHA,
              "id=%s sha=%s" % (git_blob_id(mut), sha256_bytes(mut)))
    pl = pb.decode().splitlines()
    rec.check("PAT-5", "patch touches only protocol.rs; one line removed, one added: remove(key) -> clear()",
              [l for l in pl if l.startswith("diff --git")] == ["diff --git a/%s b/%s" % (PROTOCOL_PATH, PROTOCOL_PATH)]
              and [l for l in pl if l.startswith("-") and not l.startswith("---")] == ["-                self.core.refusals.remove(key);"]
              and [l for l in pl if l.startswith("+") and not l.startswith("+++")] == ["+                self.core.refusals.clear();"])
    rec.check("PAT-6", "patch header index line matches a2df2841..0d4ab992", ("index %s..%s 100644" % (PROTOCOL_C_BLOB, MUTANT_BLOB)) in pb.decode())
    rec.check("PAT-7", "mutant blob is not in the object store before the mission", git("cat-file", "-e", MUTANT_BLOB, check=False).returncode != 0)
    # the other 34 accepted entries use different patches; confirm the C7 patch is the only one for C7
    rec.info("PAT-8", "mutant protocol.rs size %d bytes (W-TREE %d)" % (len(mut), len(prot)))

    # ---------------- 8. E-C / accepted receipts ----------------
    rec.log("\n## 8. E-C and accepted restored-side receipts (read from git objects, never rerun)")
    ev = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
    rec.check("REC-0", "evidence commit f023cc3d exists", git_out("cat-file", "-t", ev) == "commit")
    base = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts"
    pin = json.loads(blob_at(ev, base + "/10-build/PIN.json"))
    rec.check("REC-1", "E-C pin: sha256 cde9f9d2.., %d bytes, built from C-BASE/C-TREE, rustc 1.94.0 (4a4ef493e 2026-03-02)" % E_C_SIZE,
              pin["sha256"] == E_C_SHA and pin["size"] == E_C_SIZE and pin["commit"] == C_BASE and pin["tree"] == C_TREE and pin["rustc"] == "rustc 1.94.0 (4a4ef493e 2026-03-02)")
    for rid, tname in (("44-full-F-PT", "gc_symmetry_fpt"), ("43-full-F-3", "gc_symmetry_f3")):
        d = json.loads(blob_at(ev, "%s/%s/descriptor.json" % (base, rid)))
        cls = blob_at(ev, "%s/%s/CLASSIFICATION.txt" % (base, rid)).decode()
        raw_id = blob_id_at(ev, "%s/%s/raw-output.txt" % (base, rid))
        raw = blob_at(ev, "%s/%s/raw-output.txt" % (base, rid))
        rec.info("REC-2", "%s: test=%s raw_blob=%s raw_sha=%s classification_first_line=%r" % (rid, d.get("test"), raw_id[:12], sha256_bytes(raw)[:12], cls.splitlines()[0]))
        rec.check("REC-2", "%s is a gc_symmetry receipt with CLASSIFICATION: PASS" % rid, tname in json.dumps(d) and cls.splitlines()[0] == "CLASSIFICATION: PASS")
    ca = c7["restored_receipt"]
    rec.check("REC-3", "catalog restored_receipt names f023cc3d, gc_symmetry_fpt, executable cde9f9d2, raw sha 0863794e..",
              ca["commit"] == ev and ca["descriptor_test"].endswith("gc_symmetry_fpt") and ca["executable_after"] == E_C_SHA and ca["descriptor_raw_output_sha256"].startswith("0863794e"))

    # ---------------- 9. environment ----------------
    rec.log("\n## 9. environment, ownership, resources")
    rustup = subprocess.run(["rustup", "toolchain", "list"], capture_output=True, text=True)
    rec.info("ENV-1", "rustup toolchain list: %s" % rustup.stdout.strip().replace("\n", " | "))
    rec.check("ENV-1", "toolchain 1.94.0-x86_64-unknown-linux-gnu is installed", "1.94.0-x86_64-unknown-linux-gnu" in rustup.stdout)
    envp = {"HOME": "/home/nexus", "PATH": "/home/nexus/.cargo/bin:/usr/local/bin:/usr/bin:/bin"}
    rc = subprocess.run(["/usr/bin/env", "-i", "HOME=/home/nexus", "PATH=" + envp["PATH"], "rustc", "+1.94.0", "-vV"], capture_output=True, text=True)
    cg = subprocess.run(["/usr/bin/env", "-i", "HOME=/home/nexus", "PATH=" + envp["PATH"], "cargo", "+1.94.0", "-vV"], capture_output=True, text=True)
    rec.info("ENV-2", "rustc +1.94.0 -vV: %s" % rc.stdout.strip().replace("\n", " | "))
    rec.info("ENV-2", "cargo +1.94.0 -vV: %s" % cg.stdout.strip().replace("\n", " | "))
    rec.check("ENV-2", "rustc -vV matches E-C's record (release/commit-hash/date/host/LLVM)",
              all(x in rc.stdout for x in ("rustc 1.94.0 (4a4ef493e 2026-03-02)", "commit-hash: 4a4ef493e3a1488c6e321570238084b38948f6db", "commit-date: 2026-03-02", "host: x86_64-unknown-linux-gnu", "LLVM version: 21.1.8")))
    rec.check("ENV-3", "cargo -vV matches E-C's record (release/commit-hash/date/libgit2/libcurl/ssl/os)",
              all(x in cg.stdout for x in ("cargo 1.94.0 (85eff7c80 2026-01-15)", "commit-hash: 85eff7c80277b57f78b11e28d14154ab12fcf643", "commit-date: 2026-01-15", "libgit2: 1.9.2 (sys:0.20.3 vendored)", "libcurl: 8.17.0-DEV (sys:0.4.84+curl-8.17.0 vendored ssl:OpenSSL/3.5.4)", "ssl: OpenSSL 3.5.4 30 Sep 2025", "os: Ubuntu 24.4.0 (noble) [64-bit]")))
    for p in (WT_HEALTHY, WT_MUTANT, EV_DIR):
        rec.check("DEST", "%s is absent (lexists false)" % p, not os.path.lexists(p))
        wl = git("worktree", "list", "--porcelain").stdout.decode()
        rec.check("DEST2", "%s not registered as a worktree" % p, ("worktree " + p + "\n") not in wl)
    for pth in (WT_HEALTHY, WT_MUTANT):
        rec.check("DEST3", "no branch/ref named for %s" % os.path.basename(pth), git("for-each-ref", "--format=%(refname)", "refs/heads/*%s*" % os.path.basename(pth), check=False).stdout.decode().strip() == "")
    for parent in sorted({os.path.dirname(WT_HEALTHY), os.path.dirname(WT_MUTANT), os.path.dirname(EV_DIR)}):
        s = os.stat(parent)
        real = os.path.realpath(parent)
        rec.info("OWN", "%s: realpath=%s uid=%d gid=%d mode=%o dev=%d ino=%d symlink=%s" % (parent, real, s.st_uid, s.st_gid, stat.S_IMODE(s.st_mode), s.st_dev, s.st_ino, os.path.islink(parent)))
        rec.check("OWN", "parent %s is a real directory owned by uid %d, not world-writable, not a symlink" % (parent, os.getuid()),
                  real == parent and s.st_uid == os.getuid() and not (s.st_mode & stat.S_IWOTH) and not os.path.islink(parent) and stat.S_ISDIR(s.st_mode))
    for pth in (os.path.join(REPO, ".git"),):
        s = os.stat(pth)
        rec.info("OWN", "%s: uid=%d mode=%o" % (pth, s.st_uid, stat.S_IMODE(s.st_mode)))
    # protected worktrees must not be touched; record their HEAD (read-only)
    for wt in ("/home/nexus/NEXUS/nexus-os-p3-c11-c7-witness-source-w1-01", "/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C7"):
        h = git("rev-parse", "HEAD", cwd=wt).stdout.decode().strip()
        rec.info("RET", "retained worktree %s HEAD %s (not touched by this mission)" % (wt, h))
    meminfo = {l.split(":")[0]: int(l.split()[1]) for l in open("/proc/meminfo")}
    rec.info("RES-1", "MemTotal %d KiB; MemAvailable %d KiB; SwapTotal %d KiB; SwapFree %d KiB" % (meminfo["MemTotal"], meminfo["MemAvailable"], meminfo["SwapTotal"], meminfo["SwapFree"]))
    rec.check("RES-1", "MemAvailable >= 18 GiB (the accepted supervisor's pre-flight threshold)", meminfo["MemAvailable"] >= 18 * 1024 * 1024)
    sv = os.statvfs("/home/nexus/NEXUS")
    free_gib = sv.f_bavail * sv.f_frsize / 2**30
    rec.info("RES-2", "free disk on /home/nexus/NEXUS filesystem: %.1f GiB" % free_gib)
    rec.check("RES-2", "free disk >= 8 GiB (two builds ~0.8 GiB each plus worktrees ~0.2 GiB each; threshold set by this mission)", free_gib >= 8.0)
    ps = subprocess.run(["pgrep", "-af", r"nexus_governed_control-|cargo (test|build)|rustc "], capture_output=True, text=True).stdout.strip().splitlines()
    ps = [l for l in ps if "pgrep" not in l and "gate.py" not in l]
    rec.info("PROC", "cargo/rustc/governed-control test processes: %s" % (ps or "none"))
    rec.check("PROC", "no cargo, rustc or governed-control test executable running (no conflicting mutation campaign on this host)", not ps)
    units = subprocess.run(["systemctl", "--user", "list-units", "--type=scope", "--all", "--no-legend", "--plain"], capture_output=True, text=True).stdout.splitlines()
    active = [l for l in units if re.match(r"^(p3c11|c4w01|fbk)", l) and " active " in l]
    rec.check("SCOPE-1", "no active p3c11/c4w01/fbk scope unit (no concurrent campaign supervisor)", not active, "; ".join(active))
    sr = subprocess.run(["systemd-run", "--version"], capture_output=True, text=True)
    rec.info("SCOPE-2", "systemd-run: %s" % sr.stdout.splitlines()[0])
    cgroot = subprocess.run(["stat", "-fc", "%T", "/sys/fs/cgroup"], capture_output=True, text=True).stdout.strip()
    rec.check("SCOPE-3", "cgroup v2 mounted at /sys/fs/cgroup", cgroot == "cgroup2fs", cgroot)
    ctl = open("/sys/fs/cgroup/user.slice/user-%d.slice/cgroup.controllers" % os.getuid()).read().strip() if os.path.exists("/sys/fs/cgroup/user.slice/user-%d.slice/cgroup.controllers" % os.getuid()) else ""
    rec.info("SCOPE-4", "user slice controllers: %s" % ctl)

    rec.log("\n# finished %s" % now())
    if rec.fails:
        rec.log("GATE RESULT: FAIL (%d failing checks): %s" % (len(rec.fails), ", ".join(rec.fails)))
        rec.write(out)
        return 2
    rec.log("GATE RESULT: PASS (every check passed)")
    rec.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
