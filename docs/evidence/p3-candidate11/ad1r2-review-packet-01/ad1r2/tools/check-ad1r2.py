#!/usr/bin/env python3
"""Independent mechanical checks of the AD-1-R2 draft (P3-C11-DR01-D2A1-AD1R2-CONTRACT-EVIDENCE-01).
Reads only: the draft, the accepted contract's bytes, the repository at the source commit (git show / rev-parse),
and sealed receipts under the evidence root. Writes nothing. Exit status 0 only if every check passes.
    check-ad1r2.py <draft> <contract> <repo> <source-commit> <evidence-root>"""
import hashlib, re, subprocess, sys

draft_path, contract_path, repo, commit, root = sys.argv[1:6]
fails, notes = [], []


def check(ok, what):
    (notes if ok else fails).append(("PASS  " if ok else "FAIL  ") + what)


def git(*args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True).stdout


draft_bytes = open(draft_path, "rb").read()
draft = draft_bytes.decode("utf-8")
D = draft.split("\n")
craw = open(contract_path, "rb").read()
CONTRACT_SHA256 = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
check(hashlib.sha256(craw).hexdigest() == CONTRACT_SHA256, f"C0 contract bytes sha256 = {CONTRACT_SHA256}")
L = craw.decode("utf-8").split("\n")
print(f"draft {draft_path}\n  sha256 {hashlib.sha256(draft_bytes).hexdigest()}  bytes {len(draft_bytes)}  lines {len(D) - 1}")
print(f"contract {contract_path}\n  lines {len(L) - 1}")

# ---- C1 verbatim blocks --------------------------------------------------------------------------------------------
pairs = re.findall(r"<!-- VERBATIM ([0-9,]+) -->\n```text\n(.*?)\n```", draft, flags=re.S)
fences = len(re.findall(r"^```text$", draft, flags=re.M))
markers = len(re.findall(r"<!-- VERBATIM", draft))
check(len(pairs) == fences == markers, f"C1a every text fence follows a VERBATIM marker ({len(pairs)} pairs, {fences} fences, {markers} markers)")
verb_lines = {}
quoted = []
for marker, block in pairs:
    nums = [int(x) for x in marker.split(",")]
    rows = block.split("\n")
    ok = rows == [L[n - 1] for n in nums] and all(L[n - 1].strip() for n in nums)
    check(ok, f"C1b block {marker}: {len(rows)} line(s) byte-equal to contract lines {nums}")
    verb_lines[marker] = nums
    quoted += nums
print(f"  verbatim lines quoted: {len(quoted)} ({len(set(quoted))} distinct)")

# ---- C2 inline quotations of the contract ----------------------------------------------------------------------------
INLINE = [
    (548, "The loop needs no load"),
    (548, "The word is touched in exactly two private functions"),
    (609, "so that a violation has a shortest trace"),
    (813, "a cap was reached"),
    (855, "Reaching one"),
    (827, "under its cap"),
    (831, "Missing evidence for any of them is a failed mission"),
    (831, "every one"),
    (863, "states and transitions"),
    (863, "with the receipt that shows it ran"),
    (699, "the count becomes a flag"),
    (721, "a gating append answers Committed on `Commit::Unknown`"),
    (742, "nothing may fail"),
    (731, "E0283"),
    (912, "TEXT UPDATE, SAME PROPERTY"),
    (923, "every control is killed"),
    (748, "C7 without A4 is expected to fail E11 on F-3"),
    (813, "INCOMPLETE is never PASS"),
    (750, "a script kept outside the repository"),
]
for n, frag in INLINE:
    check(frag in L[n - 1] and frag in draft, f"C2 inline quotation of line {n} present in contract and draft: {frag!r}")

check(L[1428].strip() == "pub(super) struct Core {" and L[1441].strip() == "}"
      and not any("publish" in l for l in L[1429:1441]) and "contract lines 1429–1442" in draft,
      "C2b Appendix A's Core (lines 1429-1442) has no field naming a published value, as L states")

# ---- C3 supersession table -------------------------------------------------------------------------------------------
rows = re.findall(r"^\| (S-\d+) \| ([0-9, ]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", draft, flags=re.M)
ids = [r[0] for r in rows]
check(ids == [f"S-{i}" for i in range(1, 19)], f"C3a supersession ids S-1..S-18, unique, in order ({len(ids)} rows)")
headings = set(re.findall(r"^## ([A-Q])\. ", draft, flags=re.M))
check(headings == set("ABCDEFGHIJKLMNOPQ"), f"C3b sections A..Q each present once ({''.join(sorted(headings))})")
s_lines = set()
EFFECTS = ("REPLACED", "SUPPLEMENTED", "CLARIFIED", "PROPOSED CLARIFICATION")
for sid, lines_, name, effect, secs in rows:
    nums = [int(x) for x in lines_.replace(" ", "").split(",")]
    s_lines |= set(nums)
    check(any(e in effect for e in EFFECTS), f"C3c {sid} effect names an allowed kind")
    letters = set(re.findall(r"\b([A-Q])\b", secs))
    check(letters and letters <= headings, f"C3d {sid} sections {sorted(letters)} exist")
    m = re.search(rf"\*\*{sid}, line (\d+)\*\* \(verbatim[^\n]*\n\n<!-- VERBATIM ([0-9,]+) -->", draft)
    check(bool(m) and int(m.group(1)) in nums and set(nums) <= {int(x) for x in m.group(2).split(",")},
          f"C3e {sid} (line {lines_.strip()}) has its own verbatim block containing that line")
ns = draft.split("### Not superseded", 1)[1].split("\n---\n", 1)[0]
ns_lines = set()
for marker in re.findall(r"<!-- VERBATIM ([0-9,]+) -->", ns):
    ns_lines |= {int(x) for x in marker.split(",")}
check(not (ns_lines & s_lines), f"C3f no line is both superseded and 'not superseded' (not superseded: {sorted(ns_lines)})")
for sid, n in (("S-16", 742), ("S-17", 748)):
    check(n in s_lines and n not in ns_lines, f"C3g line {n} moved from AD-1-R1's 'not superseded' list to {sid}")
replaced = [r[0] for r in rows if "REPLACED" in r[3]]
print(f"  rows with an actual replacement: {', '.join(replaced)}")

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
counts = {}
for r in inv:
    counts[r[4].strip().split(" `")[0]] = counts.get(r[4].strip().split(" `")[0], 0) + 1
print(f"  inventory dispositions: {counts}")

# ---- C5 diagnostic tests named by D ---------------------------------------------------------------------------------
mt = git("show", f"{commit}:{FILES[0]}").decode("utf-8")
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
    "contract": CONTRACT_SHA256,
    "review-02": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract/P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md"),
    "review-03": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract/P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md"),
    "AD-1": sha_file(f"{A}/p3-c11-dr01-d2a1-closure-prep-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-DRAFT.md"),
    "AD-1-R1": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R1-DRAFT.md"),
    "protocol.rs": sha_git("crates/nexus-governed-control/src/ledger/protocol.rs"),
    "historical manifest": sha_git("docs/evidence/p3-candidate10/original-183/manifest.json"),
    "C1 patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C1-mutation.patch"),
    "C25 narrow patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-witness-audit-01/patches/C25-narrow-mutation.patch"),
    "C25 historical patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C25-mutation.patch"),
}
EXPECT = {"review-02": "6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce",
          "review-03": "1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35",
          "AD-1": "01778ac35cf98dfe97935dfc0ed413cbdfc18cb953b9d426822bef430061176e",
          "AD-1-R1": "589eb6cb24626585862749fed3fa619d2261cdbc8b08c56e7139c6d12bc04cda"}
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
          "b1d535f1"):
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
    row = re.search(rf"^\| {re.escape(name)} at [0-9a-f]{{8}} \| ([0-9,]+) \| ([0-9,]+) \| ([0-9,]+) \| ([0-9,]+) \|$", draft, flags=re.M)
    ok = row and [nums(x) for x in row.groups()] == [K, Kf, 2 * K - Kf, (adm + 1) * (2 * K - Kf)]
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
for (mission, c, rel), row in zip(HIST, mrows):
    t = receipt(rel)
    st, ex = re.search(r"^  states (\d+) \(expanded (\d+)\)", t, flags=re.M).groups()
    outcome = re.search(r"^  outcome: (\w+)", t, flags=re.M).group(1)
    cap = re.search(r"caps (\d+) states", t).group(1)
    commit_line = re.search(r"commit: ([0-9a-f]{40})", t).group(1)
    exp_ok = (row[5] == "all" and st == ex) or (row[5] != "all" and nums(row[5]) == int(ex))
    ok = (row[1] == mission and row[2] == c and commit_line.startswith(c) and nums(row[4]) == int(st) and exp_ok
          and row[6].split()[0] == outcome and nums(row[3].split()[0]) == int(cap))
    check(ok, f"C7c M row {row[0].strip()} / {mission}: {outcome} {st} ({ex} expanded) cap {cap} at {commit_line[:8]}")

# ---- C8 configuration facts behind Q --------------------------------------------------------------------------------
consts = dict(re.findall(r"^const ([A-Z0-9_]+): Config = (?:default_run\()?Config \{(.*?)\n\}", mt, flags=re.M | re.S))
wu = sorted(k for k, v in consts.items() if "worker_unwinds: true" in v)
l1f = sorted(k for k, v in consts.items() if "l1: false" in v)
check(wu == ["FW"] and l1f == ["FD"], f"C8 worker_unwinds true only in {wu}; l1 false only in {l1f}")

# ---- C9 status and stale or contradictory operative phrases ---------------------------------------------------------
head = "\n".join(D[:4])
check("NOT ACCEPTED" in head and "NOT ADOPTED" in head and "NOT APPLIED" in head and "DRAFT" in head,
      "C9a status line: DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED")
operative = draft.split("## Changes from AD-1-R1", 1)[0]
STALE = [  # (pattern, the only operative lines where it may appear, why)
    (r"local, not pushed", [], "witness commit is published"),
    (r"ARGUED, not probed", [], "the narrow form was probed once"),
    (r"wrong guess costs one failed exchange", [], "no one-failure bound"),
    (r"replaces only the third sentence", [], "R1 sentence targeted by its opening words"),
    (r"third sentence", ["AD-1-R1 called it the third"], "only as history"),
    (r"final_tree", ["AD-1-R1's field `final_tree` is withdrawn"], "manifest self-reference removed"),
    (r"Open \(not decided here\)", [], "AD-1-R1's open items are resolved or listed in Open items"),
    (r"needs the Owner's one-run exception, as before", [], "no one-run authorisation inferred"),
    (r"do not affect this, provided no earlier line changes", [], "no inherited approval"),
    (r"must print them", [], "no new instrumentation"),
    (r"C25'?s? (?:exact )?mutant text is (?:still )?open", [], "narrow C25 selected"),
]
for pat, allowed, why in STALE:
    hits = [l for l in operative.split("\n") if re.search(pat, l)]
    bad = [l for l in hits if not any(a in l for a in allowed)]
    check(not bad, f"C9b operative text has no '{pat}' outside its allowed context ({why}); hits {len(hits)}")
for phrase in ("killed finally", "finally killed", "is accepted by this draft", "ADOPTED.", "IN FORCE."):
    pass
claims = re.findall(r"\b(?:C1|C25)(?: and C25)? (?:is|are|was|were|has been|have been) (?:now |finally )?killed\b", operative)
check(not claims, f"C9c no sentence claims C1 or C25 is killed ({claims})")
check(draft.count("No final kill") >= 2 and "No final mutation kill is established" in draft,
      "C9e 'no final kill' stated for C1 and C25")
check("It may be used" not in operative and "now in force" not in operative, "C9d nothing is declared in force")

print()
for n in notes:
    print(n)
for f in fails:
    print(f)
print(f"\nSUMMARY: {len(notes)} passed, {len(fails)} failed")
sys.exit(1 if fails else 0)
