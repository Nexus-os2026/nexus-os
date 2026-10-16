"""P3-C11-DR01-D2A1-FINAL-CLOSURE-01 shared read-only helpers.

Every function here reads Git objects or files. Nothing writes outside the mission evidence root, nothing executes a
Nexus binary, and nothing changes a ref, index, worktree or historical evidence.
"""
import hashlib
import json
import os
import re
import subprocess

REPO = "/home/nexus/NEXUS/nexus-os"
ROOT = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-closure-01"
AUDITS = "/home/nexus/nexus-audits/claude-architect"

C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
C_PARENT = "71264aa81a40b86189783facc8437e0427887c16"
HEAD_EV = "cca0d773d9a405be566e576886321acf6a28f29b"  # C7 final evidence commit (evidence-only descendant of C)
HEAD_EV_TREE = "68256471d53479a99538462cbd4cb1f53f4c2003"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"  # final-baseline-known-runs-01
STAGE2 = "89326b859792a442c1853c3d35d30a48301c009e"  # final-baseline-remaining-configs-01
E_C = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
E_C_SIZE = 8984464
EVB = "docs/evidence/p3-candidate11"

SRC_BLOBS = {
    "crates/nexus-governed-control/src/ledger/protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
    "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs": "28085ce232827301104b5d757af042ec933976f8",
    "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs": "55dc66409eaccdaa93e8f0a56f2ccb14be81e114",
}
CONTRACT_SHA = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
AD1R4 = ("35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1", "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693")
AD1R5 = ("ae3cabdb45627f9b3179a098c50d27e50c55101f", "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473")
ADC7W = ("cb3a208e1d79a7e94e2ae0001e208a2b6436bc46", "989490ec9272a528fe0185ad0dc9f41e3aed1e414d2eb4a88346197dc60b3f3b")
CT1 = ("docs/evidence/p3-candidate11/current-tree-183/manifest.json", "5d31cb45123403626f06c521dd2d53103676250e",
       "20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279", "ct-1")
HIST183 = ("docs/evidence/p3-candidate10/original-183/manifest.json", "65fa69dcc0742e04658a3360c42e051e38ebbf5b",
           "2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14")


def git(*args, binary=False, check=True):
    r = subprocess.run(["git", "-C", REPO, *args], capture_output=True)
    if check and r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed rc={r.returncode}: {r.stderr.decode(errors='replace')}")
    return r.stdout if binary else r.stdout.decode()


def show(rev, path):
    return git("show", f"{rev}:{path}", binary=True)


def blob_id(rev, path):
    return git("rev-parse", f"{rev}:{path}").strip()


def exists(rev, path):
    return subprocess.run(["git", "-C", REPO, "cat-file", "-e", f"{rev}:{path}"], capture_output=True).returncode == 0


def sha(b):
    return hashlib.sha256(b).hexdigest()


def git_blob_of_bytes(b):
    return hashlib.sha1(b"blob %d\0" % len(b) + b).hexdigest()


def file_sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def jshow(rev, path):
    return json.loads(show(rev, path))


def write_new(path, text):
    """Write a new file inside the mission root only; refuse to overwrite."""
    ap = os.path.abspath(path)
    assert ap.startswith(ROOT + "/"), ap
    with open(ap, "x") as f:
        f.write(text)


# ------------------------------------------------------------------------------------------------ explorer reports
PROP_RE = re.compile(r"^\s{2}(P1|P2|P4|P5|P6|P7|P8|P9|P12|P13|P15|D1|L1|E1|E2|E3|E4|E5|E6|E7|E8|E9|E10|E11|KEY): (.*)$")
BR_RE = re.compile(r"^\s{4}\[(taken    |NOT TAKEN)\] (.*)$")
UNLISTED_RE = re.compile(r"^\s{4}(?:\[taken    \] )?(.*) \(not in the listed set\)$")


def parse_reports(text):
    """Parse every EXPLORATION block in a raw libtest output. Returns a list of dicts in order of appearance."""
    lines = text.splitlines()
    out = []
    i = 0
    while i < len(lines):
        m = re.search(r"EXPLORATION (\S+)(.*)$", lines[i])
        if not m:
            i += 1
            continue
        rep = {"header": "EXPLORATION " + m.group(1) + m.group(2), "config": m.group(1), "line_index": i,
               "props": {}, "taken": [], "not_taken": [], "unlisted": [], "quotient": None, "symmetry": None}
        j = i + 1
        while j < len(lines):
            l = lines[j]
            if re.search(r"EXPLORATION \S+", l) or l.startswith("test ") or l.startswith("thread ") \
                    or l.startswith("failures:") or l.startswith("test result:"):
                break
            s = l.strip()
            if s.startswith("outcome: "):
                rep["outcome"] = s[len("outcome: "):]
            elif s.startswith("bounds: "):
                rep["bounds"] = s[len("bounds: "):]
            elif s.startswith("actors: "):
                rep["actors"] = s[len("actors: "):]
            elif s.startswith("quotient: "):
                rep["quotient"] = s[len("quotient: "):]
            elif s.startswith("symmetry: "):
                rep["symmetry"] = s[len("symmetry: "):]
            elif s.startswith("states "):
                mm = re.match(r"states (\d+) \(expanded (\d+)\), transitions (\d+) \(with a fault (\d+)\), "
                              r"states with no enabled step (\d+)", s)
                if mm:
                    rep.update(states=int(mm.group(1)), expanded=int(mm.group(2)), transitions=int(mm.group(3)),
                               fault_edges=int(mm.group(4)), no_step=int(mm.group(5)))
                else:
                    mm = re.match(r"states (\d+) \(expanded (\d+)\)", s)
                    if mm:
                        rep.update(states=int(mm.group(1)), expanded=int(mm.group(2)))
                    rep["states_line"] = s
            elif s.startswith("elapsed "):
                rep["elapsed"] = s
            elif s.startswith("FAIL: "):
                rep["fail"] = s[len("FAIL: "):]
            elif s.startswith("INCOMPLETE") or s.startswith("incomplete"):
                rep.setdefault("incomplete", s)
            elif PROP_RE.match(l):
                pm = PROP_RE.match(l)
                rep["props"][pm.group(1)] = pm.group(2)
            elif UNLISTED_RE.match(l):
                rep["unlisted"].append(UNLISTED_RE.match(l).group(1))
            elif BR_RE.match(l):
                bm = BR_RE.match(l)
                (rep["taken"] if bm.group(1).startswith("taken") else rep["not_taken"]).append(bm.group(2))
            elif s.startswith("shortest trace"):
                rep["trace"] = s
            j += 1
        rep["end_index"] = j
        out.append(rep)
        i = j
    return out


def props_summary(rep):
    p = rep["props"]
    holds = sum(1 for v in p.values() if v.startswith("HOLDS"))
    na = [k for k, v in p.items() if v.startswith("not applicable")]
    nc = [k for k, v in p.items() if v.startswith("not claimed")]
    ne = [k for k, v in p.items() if v.startswith("NOT ESTABLISHED")]
    other = {k: v for k, v in p.items() if not (v.startswith("HOLDS") or v.startswith("not applicable")
                                                  or v.startswith("not claimed") or v.startswith("NOT ESTABLISHED"))}
    return {"lines": len(p), "holds": holds, "not_applicable": na, "not_claimed": nc, "not_established": ne,
            "other": other}
