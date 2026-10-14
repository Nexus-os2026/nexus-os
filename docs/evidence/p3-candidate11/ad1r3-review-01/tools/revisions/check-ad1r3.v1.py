#!/usr/bin/env python3
"""Independent mechanical checks of the AD-1-R3 draft (P3-C11-DR01-D2A1-AD1R3-READINESS-01).
Derived from AD-1-R2's check-ad1r2.py (categories C0-C9 kept and adapted); C10-C16 are new for R3.
Reads only: the draft, the contract's and the two binding reviews' bytes, the repository at the source commit (git show
/ rev-parse), and sealed receipts under the evidence root. Writes nothing. Exit status 0 only if every check passes.
    check-ad1r3.py <draft> <contract> <review-02> <review-03> <repo> <source-commit> <evidence-root>"""
import hashlib, re, subprocess, sys

draft_path, contract_path, r02_path, r03_path, repo, commit, root = sys.argv[1:8]
fails, notes = [], []


def check(ok, what):
    (notes if ok else fails).append(("PASS  " if ok else "FAIL  ") + what)


def git(*args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True).stdout


draft_bytes = open(draft_path, "rb").read()
draft = draft_bytes.decode("utf-8")
D = draft.split("\n")
SHA = {"L": "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c",
       "R2": "6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce",
       "R3": "1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35"}
SRC = {}
for key, path in (("L", contract_path), ("R2", r02_path), ("R3", r03_path)):
    raw = open(path, "rb").read()
    check(hashlib.sha256(raw).hexdigest() == SHA[key], f"C0 {key} bytes sha256 = {SHA[key][:12]}…")
    SRC[key] = raw.decode("utf-8").split("\n")
L = SRC["L"]
print(f"draft {draft_path}\n  sha256 {hashlib.sha256(draft_bytes).hexdigest()}  bytes {len(draft_bytes)}  lines {len(D) - 1}")
print(f"contract {contract_path}\n  lines {len(L) - 1}")

# ---- C1 verbatim blocks (contract and both reviews) -----------------------------------------------------------------
MARK = {"": "L", " REVIEW-02": "R2", " REVIEW-03": "R3"}
pairs = re.findall(r"<!-- VERBATIM((?: REVIEW-0[23])?) ([0-9,]+) -->\n```text\n(.*?)\n```", draft, flags=re.S)
fences = len(re.findall(r"^```text$", draft, flags=re.M))
markers = len(re.findall(r"<!-- VERBATIM", draft))
check(len(pairs) == fences == markers, f"C1a every text fence follows a VERBATIM marker ({len(pairs)} pairs, {fences} fences, {markers} markers)")
quoted = []
for which, marker, block in pairs:
    key = MARK[which]
    nums = [int(x) for x in marker.split(",")]
    rows = block.split("\n")
    ok = rows == [SRC[key][n - 1] for n in nums] and all(SRC[key][n - 1].strip() for n in nums)
    check(ok, f"C1b block{which} {marker}: {len(rows)} line(s) byte-equal to {key} lines {nums}")
    if key == "L":
        quoted += nums
print(f"  contract verbatim lines quoted: {len(quoted)} ({len(set(quoted))} distinct)")

# ---- C2 inline quotations -------------------------------------------------------------------------------------------
INLINE = [  # (source, line, fragment)
    ("L", 548, "The loop needs no load"), ("L", 548, "The word is touched in exactly two private functions"),
    ("L", 609, "so that a violation has a shortest trace"), ("L", 813, "a cap was reached"),
    ("L", 855, "Reaching one"), ("L", 855, "a run killed from outside"), ("L", 827, "under its cap"),
    ("L", 831, "Missing evidence for any of them is a failed mission"), ("L", 831, "every one"),
    ("L", 863, "states and transitions"), ("L", 863, "with the receipt that shows it ran"),
    ("L", 699, "the count becomes a flag"), ("L", 721, "a gating append answers Committed on `Commit::Unknown`"),
    ("L", 742, "nothing may fail"), ("L", 731, "E0283"), ("L", 912, "TEXT UPDATE, SAME PROPERTY"),
    ("L", 923, "every control is killed"), ("L", 923, "Codex's review of the evidence passes"),
    ("L", 928, "only after the evidence and Codex's review have actually passed"),
    ("L", 976, "Codex, under that arrangement"), ("L", 4, "Claude — Lead Architect"),
    ("L", 5, "First Architect"), ("L", 8, "First Architect"), ("L", 31, "First Architect"),
    ("L", 972, "First Architect"), ("L", 982, "First Architect"),
    ("L", 748, "C7 without A4 is expected to fail E11 on F-3"), ("L", 813, "INCOMPLETE is never PASS"),
    ("L", 750, "a script kept outside the repository"), ("L", 704, "P4, P8"),
    ("R2", 4, "ChatGPT — First Architect / Independent Auditor"), ("R2", 5, "Claude — Lead Architect"),
    ("R3", 4, "ChatGPT — First Architect / Independent Auditor"), ("R3", 5, "Claude — Lead Architect"),
    ("R2", 238, "A reviewer does not independently accept a repair it authored."),
    ("R3", 83, "Implementation review remains independent."),
]
for key, n, frag in INLINE:
    check(frag in SRC[key][n - 1] and frag in draft, f"C2 inline quotation of {key} line {n} present in source and draft: {frag!r}")
check(L[928] == "", "C2c contract line 929 is empty (the line-929 statement of R)")
check(L[1428].strip() == "pub(super) struct Core {" and L[1441].strip() == "}"
      and not any("publish" in l for l in L[1429:1441]) and "contract lines 1429–1442" in draft,
      "C2b Appendix A's Core (lines 1429-1442) has no field naming a published value, as L states")

# ---- C3 supersession table -------------------------------------------------------------------------------------------
rows = re.findall(r"^\| (S-\d+) \| ([0-9, ]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", draft, flags=re.M)
ids = [r[0] for r in rows]
check(ids == [f"S-{i}" for i in range(1, 22)], f"C3a supersession ids S-1..S-21, unique, in order ({len(ids)} rows)")
headings = set(re.findall(r"^## ([A-R])\. ", draft, flags=re.M))
check(headings == set("ABCDEFGHIJKLMNOPQR") and len(re.findall(r"^## [A-R]\. ", draft, flags=re.M)) == 18,
      f"C3b sections A..R each present once ({''.join(sorted(headings))})")
s_lines = set()
EFFECTS = ("REPLACED", "SUPPLEMENTED", "CLARIFIED", "PROPOSED CLARIFICATION")
for sid, lines_, name, effect, secs in rows:
    nums = [int(x) for x in lines_.replace(" ", "").split(",")]
    s_lines |= set(nums)
    check(any(e in effect for e in EFFECTS), f"C3c {sid} effect names an allowed kind")
    letters = set(re.findall(r"\b([A-R])\b", secs))
    check(letters and letters <= headings, f"C3d {sid} sections {sorted(letters)} exist")
    m = re.search(rf"\*\*{sid}, line ([0-9, and]+)\*\* \(verbatim[^\n]*\n\n<!-- VERBATIM ([0-9,]+) -->", draft)
    head = {int(x) for x in re.findall(r"\d+", m.group(1))} if m else set()
    check(bool(m) and set(nums) <= head and set(nums) <= {int(x) for x in m.group(2).split(",")},
          f"C3e {sid} (line {lines_.strip()}) has its own verbatim block containing that line")
ns = draft.split("### Not superseded", 1)[1].split("\n---\n", 1)[0]
ns_lines = set()
for marker in re.findall(r"<!-- VERBATIM ([0-9,]+) -->", ns):
    ns_lines |= {int(x) for x in marker.split(",")}
check(not (ns_lines & s_lines), f"C3f no line is both superseded and 'not superseded' (not superseded: {sorted(ns_lines)})")
for sid, n in (("S-16", 742), ("S-17", 748)):
    check(n in s_lines and n not in ns_lines, f"C3g line {n} stays out of 'not superseded' ({sid})")
replaced = [r[0] for r in rows if "REPLACED" in r[3]]
print(f"  rows with an actual replacement: {', '.join(replaced)}")
row = {r[0]: r for r in rows}

# ---- C4 ignored-test inventory --------------------------------------------------------------------------------------
FILES = ["crates/nexus-governed-control/src/ledger/protocol/model_tests.rs",
         "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs"]
src_ignored = {}
for f in FILES:
    text = git("show", f"{commit}:{f}").decode("utf-8").split("\n")
    for i, line in enumerate(text):
        if re.match(r"\s*#\[ignore\b", line):
            for j in range(i + 1, min(i + 8, len(text))):
                m = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?fn ([a-z0-9_]+)\(", text[j])
                if m:
                    src_ignored[m.group(1)] = (f.rsplit("/", 1)[1], j + 1)
                    break
            else:
                fails.append(f"FAIL  C4 an #[ignore] at {f}:{i + 1} has no fn within 8 lines")
inv = [tuple(c.strip() for c in r) for r in
       re.findall(r"^\| (\d+) \| `([a-z0-9_]+)` \((\d+)\) \|([^|]+)\|([^|]+)\|([^|]+)\|([^|]*)\|$", draft, flags=re.M)]
check(len(inv) == len([l for l in D if re.match(r"^\| \d+ \| `[a-z0-9_]+` \(\d+\) \|", l)]),
      "C4 every inventory-looking row parses")
inv_names = {r[1]: r for r in inv}
check(len(inv) == len(inv_names), f"C4a inventory names unique ({len(inv)} rows)")
check(set(inv_names) == set(src_ignored),
      f"C4b inventory = every #[ignore] in both files at {commit[:8]} ({len(src_ignored)} in source: "
      f"model_tests.rs {sum(1 for v in src_ignored.values() if v[0] == 'model_tests.rs')}, "
      f"unit_tests.rs {sum(1 for v in src_ignored.values() if v[0] == 'unit_tests.rs')})")
for name, r in sorted(inv_names.items(), key=lambda kv: int(kv[1][0])):
    if name in src_ignored:
        check(int(r[2]) == src_ignored[name][1], f"C4c {name}: cited line {r[2]} = fn line {src_ignored[name][1]}")
    disp = r[4].strip()
    ok = (disp in ("REQUIRED_FINAL_RUN", "EXPERIMENTAL_NOT_REQUIRED", "REQUIRED_FINAL_RUN, if E.1 holds")
          or re.fullmatch(r"REPLACED_BY `[a-z0-9_]+`", disp) is not None)
    check(ok, f"C4d {name}: disposition '{disp}' is allowed")
    m = re.fullmatch(r"REPLACED_BY `([a-z0-9_]+)`", disp)
    if m:
        tgt = m.group(1)
        check(tgt in inv_names and inv_names[tgt][4].strip() == "REQUIRED_FINAL_RUN, if E.1 holds"
              and "if E.1 does not hold: REQUIRED_FINAL_RUN" in r[6],
              f"C4e {name} REPLACED_BY {tgt}: target listed as conditional REQUIRED_FINAL_RUN; fallback stated")
    if disp == "EXPERIMENTAL_NOT_REQUIRED":
        check(len(r[6].strip()) > 0, f"C4f {name}: EXPERIMENTAL_NOT_REQUIRED has a stated reason")
full_tests = [n for n in inv_names if n.startswith("full_")]
check(len(full_tests) == 15, f"C4g fifteen required configurations (full_*) carry explicit dispositions ({len(full_tests)})")

# ---- C5 diagnostic tests named by D ---------------------------------------------------------------------------------
mt = git("show", f"{commit}:{FILES[0]}").decode("utf-8")
MT = mt.split("\n")
gate_names = []
for g in ("31-adm-quotient-differential-x1-x5", "32-adm-quotient-negatives-observer-refusals",
          "33-gc-x2-sym1-symmetry-differentials", "34-sym-pt-b-bi-differentials", "35-configuration-exercise",
          "36-negatives-and-fail-closed", "37-symmetry-supporting"):
    head = open(f"{root}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/receipts/stage5/{g}.txt", encoding="utf-8").read()
    cmd = [l for l in head.split("\n") if l.startswith("command:")][0]
    gate_names += re.findall(r"model_tests::([a-z0-9_]+)", cmd)
check(len(gate_names) == len(set(gate_names)) == 51, f"C5a gates 31-37 select 51 distinct tests ({len(gate_names)})")
for n in gate_names:
    m = re.search(rf"\n((?:[ \t]*(?:#\[|\)\]|expected = )[^\n]*\n)*)fn {n}\(\)", mt)
    attrs = m.group(1) if m else ""
    check(bool(m) and "#[ignore" not in attrs and "#[test]" in attrs, f"C5b {n} is an ordinary #[test]")
named_in_d = set(re.findall(r"`((?:quotient|gc_symmetry|an_admission)[a-z0-9_]*)`",
                            draft.split("**Diagnostic tests kept.**", 1)[1].split("## E.", 1)[0]))
named_in_d -= {"quotient_x1_matches_the_original", "quotient_x5_matches_the_original"} - set(gate_names)
check(named_in_d <= set(gate_names), f"C5c every test D names is one of the 51 ({len(named_in_d)} named explicitly)")

# ---- C6 identities quoted in the draft ------------------------------------------------------------------------------
def sha_file(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def sha_git(path, rev=commit):
    return hashlib.sha256(git("show", f"{rev}:{path}")).hexdigest()


A = root
known = {
    "contract": SHA["L"], "review-02": SHA["R2"], "review-03": SHA["R3"],
    "AD-1": sha_file(f"{A}/p3-c11-dr01-d2a1-closure-prep-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-DRAFT.md"),
    "AD-1-R1": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R1-DRAFT.md"),
    "AD-1-R2": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r2-contract-evidence-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md"),
    "protocol.rs": sha_git("crates/nexus-governed-control/src/ledger/protocol.rs"),
    "historical manifest": sha_git("docs/evidence/p3-candidate10/original-183/manifest.json"),
    "C1 patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C1-mutation.patch"),
    "C25 narrow patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-witness-audit-01/patches/C25-narrow-mutation.patch"),
    "C25 historical patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C25-mutation.patch"),
}
EXPECT = {"AD-1": "01778ac35cf98dfe97935dfc0ed413cbdfc18cb953b9d426822bef430061176e",
          "AD-1-R1": "589eb6cb24626585862749fed3fa619d2261cdbc8b08c56e7139c6d12bc04cda",
          "AD-1-R2": "53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc"}
for k, v in EXPECT.items():
    check(known[k] == v, f"C6a {k} sha256 = mission's expected {v[:8]}…")
full = set(re.findall(r"\b[0-9a-f]{64}\b", draft))
for h in sorted(full):
    owner = [k for k, v in known.items() if v == h]
    check(bool(owner), f"C6b full sha256 {h[:12]}… in draft is a computed identity ({', '.join(owner) or 'UNKNOWN'})")
for a, b in re.findall(r"`([0-9a-f]{8})…([0-9a-f]{4})`", draft):
    owner = [k for k, v in known.items() if v.startswith(a) and v.endswith(b)]
    check(bool(owner), f"C6c abbreviated sha256 {a}…{b} matches {', '.join(owner) or 'NOTHING'}")
BLOBS = {"a2df2841b99c": "crates/nexus-governed-control/src/ledger/protocol.rs",
         "5ad26ddc": "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs",
         "55dc6640": "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs",
         "d872b908": "crates/nexus-governed-control/src/ledger.rs",
         "b806ed57": "crates/nexus-governed-control/src/lib.rs",
         "726005b2": "rust-toolchain.toml", "1af53a60": "Cargo.lock",
         "2543c745": "crates/nexus-governed-control/Cargo.toml", "2c558a44": "Cargo.toml",
         "05a42b50": ".cargo/config.toml", "65fa69dc": "docs/evidence/p3-candidate10/original-183/manifest.json"}
for prefix, path in BLOBS.items():
    blob = git("rev-parse", f"{commit}:{path}").decode().strip()
    check(blob.startswith(prefix) and f"`{prefix}" in draft, f"C6d blob {prefix} = {path} at {commit[:8]} ({blob[:12]})")
for c in ("244c5b23", "83816e66", "77a146ca", "2d83b74b", "42dd1bd1", "243501f0", "9e945ee7", "22ebf9a5", "862a11ce",
          "b1d535f1", "8693e3a7"):
    t = subprocess.run(["git", "-C", repo, "cat-file", "-t", c], capture_output=True, text=True).stdout.strip()
    check(t == "commit" and c in draft, f"C6e {c} names a commit in the repository")
check(git("rev-parse", f"{commit}^{{tree}}").decode().startswith("7ac6bb21"), "C6f source tree is 7ac6bb21")

# ---- C7 numbers recomputed from sealed receipts ---------------------------------------------------------------------
def receipt(rel):
    return open(f"{A}/{rel}", encoding="utf-8").read()


def nums(s):
    return int(s.replace(",", ""))


for name, rel in (("F-3", "p3-c11-dr01-d2a1-gc-sym-f3-01/receipts/stage3/gc-symmetry-full-F-3.txt"),
                  ("F-PT", "p3-c11-dr01-d2a1-pt-fb-experiment-01/receipts/stage3/gc-symmetry-full-F-PT.txt"),
                  ("F-B", "p3-c11-dr01-d2a1-pt-fb-experiment-01/receipts/stage4/gc-symmetry-full-F-B.txt")):
    t = receipt(rel)
    K = int(re.search(r"^  states (\d+) \(expanded (\d+)\)", t, flags=re.M).group(1))
    Kf = int(re.search(r"(\d+) expanded keys are their own mirror", t).group(1))
    adm = int(re.search(r"admissions (\d+) gating", t).group(1))
    brow = re.search(rf"^\| {re.escape(name)} at [0-9a-f]{{8}} \| ([0-9,]+) \| ([0-9,]+) \| ([0-9,]+) \| ([0-9,]+) \|$", draft, flags=re.M)
    ok = brow and [nums(x) for x in brow.groups()] == [K, Kf, 2 * K - Kf, (adm + 1) * (2 * K - Kf)]
    check(bool(ok), f"C7a B table {name}: K {K}, Kf {Kf}, 2K-Kf {2 * K - Kf}, (N+1)(2K-Kf) {(adm + 1) * (2 * K - Kf)} (N={adm})")
HIST = [("CAMPAIGN-01", "244c5b23", "p3-c11-dr01-d2a1-campaign-01/receipts/explorations/full-F-3.txt"),
        ("F3-CAP60-01", "9e945ee7", "p3-c11-dr01-d2a1-f3-cap60-01/receipts/stage4/full-F-3-cap60.txt"),
        ("F3-QUOTIENT-01", "83816e66", "p3-c11-dr01-d2a1-f3-quotient-01/receipts/stage3/quotient-full-F-3.txt"),
        ("GC-SYMMETRY-01", "77a146ca", "p3-c11-dr01-d2a1-gc-symmetry-01/receipts/stage4/40-f3-prefix-diagnostic.txt"),
        ("GC-SYM-F3-01", "77a146ca", "p3-c11-dr01-d2a1-gc-sym-f3-01/receipts/stage3/gc-symmetry-full-F-3.txt"),
        ("PT-FB-EXPERIMENT-01", "2d83b74b", "p3-c11-dr01-d2a1-pt-fb-experiment-01/receipts/stage3/gc-symmetry-full-F-PT.txt"),
        ("PT-FB-EXPERIMENT-01", "2d83b74b", "p3-c11-dr01-d2a1-pt-fb-experiment-01/receipts/stage4/gc-symmetry-full-F-B.txt"),
        ("CAMPAIGN-01", "244c5b23", "p3-c11-dr01-d2a1-campaign-01/receipts/explorations/full-F-1.txt"),
        ("F1-CAP60-01", "243501f0", "p3-c11-dr01-d2a1-f1-cap60-01/receipts/explorations/full-F-1-cap60.txt"),
        ("CAMPAIGN-01", "244c5b23", "p3-c11-dr01-d2a1-campaign-01/receipts/explorations/full-F-2.txt")]
mrows = re.findall(r"^\| ([^|]+) \| ([A-Z0-9-]+) \| ([0-9a-f]{8}) \| ([^|]+) \| ([0-9,]+) \(([0-9,]+|all)\) \| ([^|]+) \|$", draft, flags=re.M)
check(len(mrows) == len(HIST), f"C7b M history table has {len(mrows)} rows (expected {len(HIST)})")
for (mission, c, rel), mrow in zip(HIST, mrows):
    t = receipt(rel)
    st, ex = re.search(r"^  states (\d+) \(expanded (\d+)\)", t, flags=re.M).groups()
    outcome = re.search(r"^  outcome: (\w+)", t, flags=re.M).group(1)
    cap = re.search(r"caps (\d+) states", t).group(1)
    commit_line = re.search(r"commit: ([0-9a-f]{40})", t).group(1)
    exp_ok = (mrow[5] == "all" and st == ex) or (mrow[5] != "all" and nums(mrow[5]) == int(ex))
    ok = (mrow[1] == mission and mrow[2] == c and commit_line.startswith(c) and nums(mrow[4]) == int(st) and exp_ok
          and mrow[6].split()[0] == outcome and nums(mrow[3].split()[0]) == int(cap))
    check(ok, f"C7c M row {mrow[0].strip()} / {mission}: {outcome} {st} ({ex} expanded) cap {cap} at {commit_line[:8]}")

# ---- C8 configuration facts behind Q --------------------------------------------------------------------------------
consts = dict(re.findall(r"^const ([A-Z0-9_]+): Config = (?:default_run\()?Config \{(.*?)\n\}", mt, flags=re.M | re.S))
wu = sorted(k for k, v in consts.items() if "worker_unwinds: true" in v)
l1f = sorted(k for k, v in consts.items() if "l1: false" in v)
check(wu == ["FW"] and l1f == ["FD"], f"C8 worker_unwinds true only in {wu}; l1 false only in {l1f}")
for label, rx in (("Q: X-1 to X-5 24/E10", r"^\| X-1 to X-5 \| 24 \| E10: "), ("Q: F-W 25", r"^\| F-W \| 25, "),
                  ("Q: F-D 23, L1 not claimed", r"^\| F-D \| 23 \| E10 \| L1: \"not claimed"),
                  ("Q: F-3/F-PT/F-B 24", r"^\| F-3, F-PT, F-B \(original or admitted quotient\) \| 24 \| E10 \|")):
    check(re.search(rx, draft, flags=re.M) is not None, f"C8b result-line counts kept: {label}")

# ---- C9 status and stale or contradictory operative phrases ---------------------------------------------------------
head = "\n".join(D[:4])
check("AD-1-R3" in D[0] and "NOT ACCEPTED" in head and "NOT ADOPTED" in head and "NOT APPLIED" in head and "DRAFT" in head,
      "C9a title AD-1-R3; status DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED")
operative = draft.split("## Changes from AD-1-R2", 1)[0]
OP = operative.split("\n")
STALE = [  # (pattern, the only operative contexts where it may appear, why)
    (r"local, not pushed", [], "witness commit is published"),
    (r"ARGUED, not probed", [], "the narrow form was probed once"),
    (r"wrong guess costs one failed exchange", [], "no one-failure bound"),
    (r"replaces only the third sentence", [], "R1 sentence targeted by its opening words"),
    (r"third sentence", ["AD-1-R1 called it the third"], "only as history"),
    (r"final_tree", ["AD-1-R1's field `final_tree` is withdrawn"], "manifest self-reference removed"),
    (r"needs the Owner's one-run exception, as before", [], "no one-run authorisation inferred"),
    (r"must print them", [], "no new instrumentation"),
    (r"C25'?s? (?:exact )?mutant text is (?:still )?open", [], "narrow C25 selected"),
    (r"never by exit status", ["is withdrawn", "in place of"], "R2-02: text-only classification withdrawn"),
    (r"the report text,\s*not the exit status", [], "R2-02: G.3(a) aligned with K.2"),
    (r"rejections apply to every failing receipt", [], "R2-01: blanket G.3(c) removed"),
    (r"is a FAIL of the explored graph", [], "R2-02: raw FAIL beyond C is not a final verdict"),
    (r"If the Architect reads it as claiming", [], "O.4 has one operative reading"),
    (r"execution-evidence authentication pending", [], "superseded by the AD1R3 ruling"),
    (r"EXECUTOR-REPORTED until the Architect authenticates", [], "superseded by the AD1R3 ruling"),
    (r"remain unanalysed", [], "C9/C10/C24 analysed by the preflight"),
]
for pat, allowed, why in STALE:
    hits = [l for l in OP if re.search(pat, l)]
    bad = [l for l in hits if not any(a in l for a in allowed)]
    check(not bad, f"C9b operative text has no '{pat}' outside its allowed context ({why}); hits {len(hits)}")
claims = re.findall(r"\b(?:C1|C25|C4|C9|C10|C24)(?: and C25)? (?:is|are|was|were|has been|have been) (?:now |finally )?killed\b", operative)
check(not claims, f"C9c no sentence claims a control is killed ({claims})")
check("It may be used" not in operative and "now in force" not in operative, "C9d nothing is declared in force")
check(operative.count("No final mutation kill") + operative.count("No final kill") >= 4,
      "C9e 'no final kill' stated for C1 and C25 (ruling and G.1)")

# ---- C10 R2-01: typed receipts ---------------------------------------------------------------------------------------
s6 = row["S-6"][3]
check("every failing receipt" not in s6 and "G.3(c) applies only to the C1/C25 witness invocation" in s6
      and "F.1" in s6 and "K.2" in s6 and "C22 exception as contracted" in s6,
      "C10a S-6 no longer applies G.3(c) to every failing receipt; points to F.1/K.2; C22 exception kept")
f1 = draft.split("### F.1 Receipt types", 1)[1].split("\n## G.", 1)[0] if "### F.1 Receipt types" in draft else ""
types = re.findall(r"^\*\*\(([a-f])\) ([^*]+)\*\*", f1, flags=re.M)
check([t[0] for t in types] == list("abcdef"), f"C10b F.1 defines types (a)-(f) once each, in order ({[t[0] for t in types]})")
tdict = {}
for i, (letter, _) in enumerate(types):
    nxt = types[i + 1][0] if i + 1 < len(types) else None
    body = f1.split(f"**({letter}) ", 1)[1]
    tdict[letter] = body.split(f"**({nxt}) ", 1)[0] if nxt else body
ta, tb, tc, td, te, tf = (tdict.get(k, "") for k in "abcdef")
check("Not required: any witness line, witness locus or witness verdict" in ta and "without a witness marker is complete" in ta
      and "4130–4140" in ta and "4115–4128" in ta and "K.1" in ta and "named property" in ta,
      "C10c type (a): explorer verdict assertion lines; named property; cardinality; no witness requirement")
check(all(x in tb for x in ("C14 T-BIND", "C19 T-CELL", "C21 W6", "C27 T-PREC", "C28 T-RES", "unit_tests.rs:<line>"))
      and "Not required: an explorer outcome line, or any witness line" in tb,
      "C10d type (b): the five named unit-test controls; own-check location; no explorer/witness requirement")
check("only to it" in tc and "do not apply to the primary receipt of C1 or C25" in tc,
      "C10e type (c): witness-only rules confined to the witness invocation")
check(all(x in td for x in ("error[E0283]", "line 1089", "internal compiler error", "healthy build evidence",
                             "NOT PRODUCED BY DESIGN", "No\n  executable hash is invented", "no witness is required",
                             "E0283 at another line", "out-of-memory")),
      "C10f type (d): C22 nonzero compile, E0283 at the T-OWN line, rejections, NOT PRODUCED BY DESIGN, no invented hash")
ut = git("show", f"{commit}:{FILES[1]}").decode("utf-8").split("\n")
check(ut[1088].strip() == "<Ticket as NotClone<_>>::check();" and "`<Ticket as NotClone<_>>::check();`" in td
      and "fn t_own_tickets_snapshots_and_the_worker_cannot_be_cloned" in ut[1087 - 1 + 0] + ut[1087],
      "C10g unit_tests.rs line 1089 at the source commit is T-OWN's Ticket check, as type (d) quotes")
check(all(x in te for x in ("exited 0", "not by a signal", "out-of-memory", "K.1", "N.11", "every property line of Q")),
      "C10h type (e): unchanged-core PASS needs exit 0, no signal, supervisor records, Q lines, cardinality")
check("not a\nkill receipt" in tf or "not a kill receipt" in tf.replace("\n", " "), "C10i type (f): A4 comparison is not a kill receipt")
g3 = draft.split("### G.3 The kill criterion", 1)[1].split("### G.4", 1)[0]
g3a = g3.split("(b) **", 1)[0]
check("NAMED PROPERTY" not in g3a and "No witness line is required" in g3a and "K.2" in g3a,
      "C10j G.3(a) (the primary receipt) carries no witness-line requirement and defers to K.2")
check("(c) **Rejections of the witness receipt.** They apply only to the witness invocations" in g3,
      "C10k G.3(c) is scoped to the witness invocations")
named_lines = [i for i, l in enumerate(OP) if "NAMED PROPERTY P" in l]
g3_start = next(i for i, l in enumerate(OP) if l.startswith("### G.3 "))
g4_start = next(i for i, l in enumerate(OP) if l.startswith("### G.4 "))
check(named_lines and all(g3_start < i < g4_start for i in named_lines),
      f"C10l the exact positive witness lines appear only inside G.3 ({len(named_lines)} lines)")
fsec = draft.split("\n## F. ", 1)[1].split("### F.1", 1)[0]
check("admissible receipt of its own type" in fsec and "beyond its ceiling (K.1)" in fsec,
      "C10m F's kill sentence requires an admissible receipt of its own type; FAIL beyond C kills nothing")
isec = draft.split("\n## I. ", 1)[1].split("\n## J. ", 1)[0]
check("for C22: NOT PRODUCED\n   BY DESIGN" in isec and "healthy build\n   evidence of F.1 (d)" in isec,
      "C10n I's mutant identity list aligned with C22 (no executable produced; healthy build evidence)")

# ---- C11 R2-02: process-aware classification ------------------------------------------------------------------------
k = draft.split("\n## K. ", 1)[1].split("\n## L. ", 1)[0]
k1 = k.split("### K.1", 1)[1].split("### K.2", 1)[0] if "### K.1" in k and "### K.2" in k else ""
k2 = k.split("### K.2", 1)[1] if "### K.2" in k else ""
check("INCOMPLETE — BUDGET EXCEEDED" in k1 and "kept exactly as printed and never discarded" in k1
      and "separate, explicit authorisation" in k1 and "not a final mutation-kill receipt" in k1,
      "C11a K.1: raw FAIL kept; beyond C the outer gate is INCOMPLETE — BUDGET EXCEEDED absent separate authorisation")
check("INCOMPLETE —\nVERIFICATION INVALID" in k1 or "INCOMPLETE — VERIFICATION INVALID" in k1,
      "C11b K.1: quotient precondition refusal stays INCOMPLETE — VERIFICATION INVALID")
check("if and only if it completes with at most C keys" in k1, "C11c K.1: inclusive cardinality <= C kept")
check("Neither the printed output nor the exit status alone establishes any result" in k2,
      "C11d K.2: neither output nor exit status alone")
items = re.findall(r"^(\d)\. ", k2.split("**Text.**", 1)[1].split("PASS text followed", 1)[0], flags=re.M) if "**Text.**" in k2 else []
check(items == list("1234567"), f"C11e K.2 lists the seven agreement points ({items})")
need = ["source and executable identity", "test selection and configuration", "complete raw output",
        "harness result expected", "exit status, or the signal", "termination and resource-limit records",
        "completion and cardinality"]
k2flat = " ".join(k2.split())
check(all(x in k2flat for x in need), "C11f K.2's seven points name identity, selection, raw output, harness result, exit/signal, supervisor records, completion/cardinality")
check("PASS text followed by a kill, a timeout, an out-of-memory kill, an incomplete termination, an identity mismatch or missing completion evidence is not PASS" in k2flat,
      "C11g K.2: PASS text plus kill/timeout/OOM/incomplete/identity/missing completion is not PASS")
check("a nonzero status alone shows nothing" in k2flat and "raw outcome, unchanged, with any counterexample" in k2flat,
      "C11h K.2: nonzero status insufficient; raw outcome and admissibility kept apart")
crow = re.findall(r"^\| ([1-7]) \| ([^|]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", k2, flags=re.M)
check([c[0] for c in crow] == list("1234567"), f"C11i classification table has cases 1-7 ({[c[0] for c in crow]})")
EXP = {"1": ("outcome: PASS", "PASS"), "2": ("kills the process", "never PASS"),
       "3": ("positive line", "G.3(b) holds"), "4": ("not the named test's own verdict", "NOT ADMISSIBLE"),
       "5": ("no witness line", "no witness marker is required"), "6": ("no executable exists", "NOT PRODUCED BY DESIGN"),
       "7": ("printed count above C", "INCOMPLETE — BUDGET EXCEEDED")}
for c in crow:
    case, outer = c[1], c[3]
    a, b = EXP[c[0]]
    check(a in case and b in outer, f"C11j case {c[0]}: '{a}' -> '{b}'")
check([c[4].strip() for c in crow][1] == "no" and [c[4].strip() for c in crow][6] == "no" and [c[4].strip() for c in crow][3] == "no",
      "C11k cases 2, 4 and 7 are never kill evidence")
s1 = row["S-1"][3]; s14 = row["S-14"][3]
check("BUDGET EXCEEDED" in s1 and "K.2" in s1 and "K.2" in s14 and "BUDGET EXCEEDED" in s14 and "VERIFICATION INVALID" in s14,
      "C11l S-1 and S-14 carry K.1/K.2 explicitly")

# ---- C12 trace wording ----------------------------------------------------------------------------------------------
s2, s19 = row["S-2"][3], row["S-19"]
check("shortest trace" not in s2 and "S-19" in s2, "C12a S-2 no longer clarifies the trace phrase; points to S-19")
check(s19[1].strip() == "609" and s19[3].strip().startswith("REPLACED") and "shortest breadth-first parent path to the checked state" in s19[3]
      and "offending transition appended" in s19[3] and "No global counterexample minimality and no shortest cycle witness" in s19[3],
      "C12b S-19 replaces line 609's phrase with the scoped statement; no global minimality; no cycle witness")
o = draft.split("\n## O. ", 1)[1].split("\n## P. ", 1)[0]
o4 = o.split("\n4. ", 1)[1].split("\n5. ", 1)[0]
o4flat = " ".join(o4.split())
check("is REPLACED (S-19)" in o4flat and "shortest breadth-first parent path" in o4flat and "checked state" in o4flat
      and "offending transition appended" in o4flat and "If the Architect" not in o4flat and "proposes" not in o4flat
      and "kept unedited" in o4flat and "replay requirement of D" in o4flat,
      "C12c O.4 has one operative reading; raw trace wording kept; replay unchanged")
oflat = " ".join(o.split())
check("Global counterexample minimality" in oflat and "not a shortest witness of one" in oflat,
      "C12d O.3 keeps global minimality and cycle witness NOT ESTABLISHED")
check(any("path_to" in l for l in MT[5056:5058]) and any("path_to" in l for l in MT[6597:6599])
      and any("path_to" in l for l in MT[3895:3897]),
      "C12e all three explorers build the trace with the breadth-first parent table (lines 3896, 5057, 6598)")
check(any("shortest trace ({} steps):" in l for l in MT[4070:4072]), "C12f explore prints 'shortest trace (<n> steps):' (line 4071)")

# ---- C13 authority mapping -------------------------------------------------------------------------------------------
rsec = draft.split("\n## R. ", 1)[1].split("\n---\n", 1)[0]
rflat = " ".join(rsec.split())
check(all(x in rflat for x in ("ChatGPT is the Sole Architect and the independent reviewer of Claude Code's work",
                               "Claude Code is the implementation executor", "Neither Claude Chat nor Codex",
                               "requires neither")),
      "C13a R states the Owner's current arrangement and requires neither Claude Chat nor Codex")
for ref in ("contract line 923", "contract line 928", "contract line 976", "REVIEW-02 line 238", "REVIEW-03 line 81",
            "REVIEW-03 line 83", "contract line 4", "contract lines 5, 8, 31, 972 and 982"):
    check(ref in rflat, f"C13b R maps {ref}")
check("**Line 929.**" in rsec and "line 929 is empty" in rflat and "line 928" in rflat, "C13c R reports the line-929 discrepancy")
check("obligation of independent review" in rflat and "Owner's authority" in rflat and "does not accept a repair it wrote" in rflat
      and "never approves its own work" in rflat and "No review is waived" in rflat,
      "C13d R preserves independent review, Owner authority and the self-review rule")
s20 = row["S-20"]
check([int(x) for x in s20[1].replace(" ", "").split(",")] == [923, 928, 976] and "REPLACED" in s20[3] and "R" in s20[4],
      "C13e S-20 covers lines 923, 928, 976")
codex_lines = [l for l in OP if "Codex" in l]
r_start = OP.index(next(l for l in OP if l.startswith("## R. ")))
r_end = r_start + next(i for i, l in enumerate(OP[r_start:]) if l == "---")
ok_ctx = []
for i, l in enumerate(OP):
    if "Codex" not in l:
        continue
    in_r = r_start <= i <= r_end
    in_s = l.startswith("| S-20 ") or l.startswith("| AU ") or l in (L[922], L[927], L[975], SRC["R2"][237])
    in_open = l.lstrip().startswith(("11.",)) or "Codex" not in l
    ok_ctx.append(in_r or in_s)
check(all(ok_ctx), f"C13f every operative mention of Codex is a quotation or the mapping itself ({len(codex_lines)} lines)")

# ---- C14 dispositions carried ---------------------------------------------------------------------------------------
flat = " ".join(operative.split())
CARRIED = [
    ("F-1 60M standing ceiling only after Owner approval", "in force only once the Owner approves it"),
    ("no one-run authorisation inferred", "No further one is inferred"),
    ("composed quotient only F-3, F-PT, F-B", "Only F-3, F-PT and F-B may use the composed quotient for 8.3"),
    ("proof, configuration and final-source prerequisites (N)", "If any one fails, the claim is not made."),
    ("fifteen configurations: twelve original + three", "For the other twelve it is the original explorer's"),
    ("diagnostics kept", "Diagnostic tests kept."), ("D1/L1 kept", "Kahn's algorithm over the quotient's non-fault edges"),
    ("replay kept", "replayed in the original semantics before it is cited"),
    ("no additional full-run instrumentation", "No new instrumentation."),
    ("exact-final-baseline evidence, no historical substitution", "No historical PASS substitutes for a final-baseline run."),
    ("detached attestation", "It is never placed in the commit it attests."),
    ("historical review fields not current approval", "They are not current-tree approval."),
    ("current-tree manifest neither created nor approved", "This draft neither creates nor approves it."),
    ("corrected CAS retry wording", "No bound on the number of failed exchanges, and no wait-freedom, is claimed (6.3)."),
    ("A-1/A-2 undischarged", "stay stated and undischarged"),
    ("receipt linking: own restoration", "its own verified baseline restoration"),
    ("receipt linking: own artifact-identity check", "its own artifact-identity check"),
    ("receipt linking still proposed", "Until it is approved, the present reading of line 853 governs."),
    ("probe helpers historical, not controllers", "They are not approved final campaign controllers."),
    ("C25 narrow keeps self.keep = true", "preserve `self.failed();`, `self.rolled(io.rollback());` and `self.keep = true;`"),
    ("INCOMPLETE never PASS", "INCOMPLETE is never PASS"),
]
for label, phrase in CARRIED:
    check(phrase in flat, f"C14 carried: {label}")

# ---- C15 Not superseded list and 0A ---------------------------------------------------------------------------------
check("read as S-18 and S-20 state" in draft and "read under S-21 (role names only)" in draft,
      "C15a 'Not superseded' list aligned with S-20/S-21")
rul = draft.split("### 0A.1", 1)[1].split("### 0A.2", 1)[0]
rflat2 = " ".join(rul.split())
check(all(x in rflat2 for x in ("AD-1-R2 | REPAIR.", "historical receipts support its intended P1 attribution",
                                "self.keep = true` preserved", "No additional probe is justified",
                                "The old compound C25 definition is historical only",
                                "Final AQ/SQ proof acceptance", "final baseline selection", "integration")),
      "C15b 0A.1 carries this mission's ruling (REPAIR; C1; C25; pending items)")
rep = re.findall(r"^\| (R2-01|R2-02|AU|TR) \|", draft, flags=re.M)
check(rep == ["R2-01", "R2-02", "AU", "TR"], f"C15c 0A.2 lists the four repairs ({rep})")

# ---- C16 open items -------------------------------------------------------------------------------------------------
oi = draft.split("## Open items", 1)[1].split("## Changes from AD-1-R2", 1)[0]
nums_oi = re.findall(r"^(\d+)\. ", oi, flags=re.M)
check(nums_oi == [str(i) for i in range(1, 13)], f"C16a open items 1-12 ({nums_oi})")
oiflat = " ".join(oi.split())
check("P4, P8" in oiflat and "needs the Architect's decision" in oiflat and "G applies to C1 and C25 only" in oiflat
      and "predictions, not measurements" in oiflat, "C16b item 10: preflight is prediction; C4 reading needs Architect decision; G not extended")

print()
for n in notes:
    print(n)
for f in fails:
    print(f)
print(f"\nSUMMARY: {len(notes)} passed, {len(fails)} failed")
sys.exit(1 if fails else 0)
