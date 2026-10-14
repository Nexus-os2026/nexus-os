#!/usr/bin/env python3
"""Independent mechanical checks of the AD-1-R4 draft (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01).
Derived from AD-1-R3's check-ad1r3.py: categories C0-C16 kept and adapted to the consolidated draft; C17-C27 are new
(role identity, one taxonomy, C22 artifacts, C4, the Oracle rule, C9-b, C10/C24 retention, C1/C25 rules,
self-containment, no authorisation, carried policies).
Reads only: the draft; the contract's, the two binding reviews', protocol.rs's and controls_defs.py's bytes; the
repository's git objects (source commit 42dd1bd1, prototype A, evidence commit B); and sealed receipts under the
evidence root. Writes nothing. Exit status 0 only if every check passes.
    check-ad1r4.py <draft> <contract> <review-02> <review-03> <protocol.rs> <controls_defs.py> <repo> <source-commit>
                   <evidence-root> <evidence-commit-B>"""
import ast, hashlib, re, subprocess, sys

(draft_path, contract_path, r02_path, r03_path, proto_path, defs_path, repo, commit, root, evb) = sys.argv[1:11]
fails, notes = [], []


def check(ok, what):
    (notes if ok else fails).append(("PASS  " if ok else "FAIL  ") + what)


def git(*args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True).stdout


def flat(s):
    return " ".join(s.split())


draft_bytes = open(draft_path, "rb").read()
draft = draft_bytes.decode("utf-8")
D = draft.split("\n")
SHA = {"L": "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c",
       "R2": "6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce",
       "R3": "1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35",
       "P": "a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296",
       "D": "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"}
SRC = {}
for key, path in (("L", contract_path), ("R2", r02_path), ("R3", r03_path), ("P", proto_path), ("D", defs_path)):
    raw = open(path, "rb").read()
    check(hashlib.sha256(raw).hexdigest() == SHA[key], f"C0 {key} bytes sha256 = {SHA[key][:12]}…")
    SRC[key] = raw.decode("utf-8").split("\n")
L, P = SRC["L"], SRC["P"]
PROTO_GIT = git("show", f"{commit}:crates/nexus-governed-control/src/ledger/protocol.rs")
check(hashlib.sha256(PROTO_GIT).hexdigest() == SHA["P"], "C0 protocol.rs bytes equal protocol.rs at the source commit")
CP = "docs/evidence/p3-candidate11/c4-witness-01"
RAW_H = git("show", f"{evb}:{CP}/receipts/09-run-healthy/raw-output.txt").decode("utf-8").split("\n")
RAW_M = git("show", f"{evb}:{CP}/receipts/12-run-mutant/raw-output.txt").decode("utf-8").split("\n")
print(f"draft {draft_path}\n  sha256 {hashlib.sha256(draft_bytes).hexdigest()}  bytes {len(draft_bytes)}  lines {len(D) - 1}")
print(f"contract {contract_path}\n  lines {len(L) - 1}")

operative = draft.split("## Changes from AD-1-R3", 1)[0]
OP = operative.split("\n")


def sec(start, end, text=draft):
    """Text from the first line starting with `start` up to (not including) the next line starting with `end`."""
    if start not in text:
        return ""
    body = text.split(start, 1)[1]
    return body.split(end, 1)[0] if end in body else body


# ---- C1 verbatim blocks (contract, both reviews, protocol.rs, controls_defs.py) --------------------------------------
MARK = {"": "L", " REVIEW-02": "R2", " REVIEW-03": "R3", " PROTOCOL": "P", " DEFS": "D"}
pairs = re.findall(r"<!-- VERBATIM((?: REVIEW-0[23]| PROTOCOL| DEFS)?) ([0-9,]+) -->\n```text\n(.*?)\n```", draft, flags=re.S)
fences = len(re.findall(r"^```text$", draft, flags=re.M))
markers = len(re.findall(r"<!-- VERBATIM", draft))
check(len(pairs) == fences == markers, f"C1a every text fence follows a VERBATIM marker ({len(pairs)} pairs, {fences} fences, {markers} markers)")
quoted = []
for which, marker, block in pairs:
    key = MARK[which]
    nums = [int(x) for x in marker.split(",")]
    rows = block.split("\n")
    ok = rows == [SRC[key][n - 1] for n in nums] and all(SRC[key][n - 1].strip() for n in nums)
    check(ok, f"C1b block{which} {marker[:40]}{'…' if len(marker) > 40 else ''}: {len(rows)} line(s) byte-equal to {key} lines")
    if key == "L":
        quoted += nums
print(f"  contract verbatim lines quoted: {len(quoted)} ({len(set(quoted))} distinct)")

# ---- C1r rust blocks: never quotations; each derived mechanically from the verbatim text before it ------------------
rust = re.findall(r"^```rust\n(.*?)\n```$", draft, flags=re.S | re.M)
check(len(rust) == 2, f"C1r-a exactly two rust blocks (the C4 and C9 resulting texts) ({len(rust)})")
c4_expect = P[1618:1626] + [P[1642]]
check(len(rust) > 0 and rust[0].split("\n") == c4_expect,
      "C1r-b C4's rust block = protocol.rs lines 1619-1626 and 1643 (lines 1627-1642 removed)")
mut = P[:1626] + P[1642:]
blob = subprocess.run(["git", "hash-object", "--stdin"], input="\n".join(mut).encode(), capture_output=True).stdout.decode().strip()
check(blob == "f3793c2ff61f8adbf43fe17d6b5237b49bc507b0" and "`f3793c2ff61f8adbf43fe17d6b5237b49bc507b0`" in draft,
      f"C1r-c removing exactly lines 1627-1642 from protocol.rs gives the mutant blob the draft names ({blob[:12]})")
patch = git("show", f"{evb}:{CP}/patches/C4-test1-removed.patch").decode("utf-8").split("\n")
removed = [l[1:] for l in patch if l.startswith("-") and not l.startswith("---")]
added = [l for l in patch if l.startswith("+") and not l.startswith("+++")]
check(hashlib.sha256("\n".join(patch).encode()).hexdigest() == "24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d"
      and removed == P[1626:1642] and not added and "@@ -1624,22 +1624,6 @@" in patch,
      "C1r-d the authenticated C4 patch removes exactly protocol.rs lines 1627-1642 and adds nothing")
c9_expect = P[1291:1295]
c9_expect = c9_expect[:2] + [c9_expect[2].replace("Persistence::Uncertain", "Persistence::NotWritten")] + c9_expect[3:]
check(len(rust) > 1 and rust[1].split("\n") == c9_expect and P[1293].strip() == "Persistence::Uncertain",
      "C1r-e C9's rust block = protocol.rs lines 1292-1295 with only line 1294's Persistence::Uncertain -> Persistence::NotWritten")

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
    ("L", 704, "acknowledgement test 1 removed"), ("L", 709, "`Rollback::Unknown` is treated as `Undone`"),
    ("L", 710, "`Commit::Unknown` followed by `Rollback::Undone` gives Not written"),
    ("L", 720, "resolve empties U for a present gating record without placing the correction"),
    ("R2", 4, "ChatGPT — First Architect / Independent Auditor"), ("R2", 5, "Claude — Lead Architect"),
    ("R3", 4, "ChatGPT — First Architect / Independent Auditor"), ("R3", 5, "Claude — Lead Architect"),
    ("R2", 238, "A reviewer does not independently accept a repair it authored."),
    ("R3", 83, "Implementation review remains independent."),
    ("P", 1626, "let core = &mut *self.core;"), ("P", 1643, "let set = snapshot.set;"),
    ("P", 1476, "return refused(Persistence::Uncertain, Reason::Storage);"),
    ("P", 1619, "fn acknowledgement(&mut self, snapshot: &Snapshot"),
]
for key, n, frag in INLINE:
    check(frag in SRC[key][n - 1] and frag in draft, f"C2 inline quotation of {key} line {n} present in source and draft: {frag!r}")
check(L[928] == "", "C2c contract line 929 is empty (the line-929 statement of R)")
check(L[1428].strip() == "pub(super) struct Core {" and L[1441].strip() == "}"
      and not any("publish" in l for l in L[1429:1441]) and "contract lines 1429–1442" in draft,
      "C2b Appendix A's Core (lines 1429-1442) has no field naming a published value, as L states")
check(sum(1 for l in P if "return refused(Persistence::Uncertain, Reason::Storage);" in l) == 1,
      "C2d C25's replaced line occurs exactly once in protocol.rs (line 1476)")
check(P[174].startswith("fn count_step(") and "`count_step` (`protocol.rs` line 175 at 42dd1bd1)" in draft,
      "C2e C1's count_step is at protocol.rs line 175")
impl = max(i for i in range(1288) if P[i].startswith("impl<'a> Publish<'a> {"))
check(impl == 1256 and all(not P[i].startswith("impl") and P[i] != "}" for i in range(impl + 1, 1963))
      and P[1963] == "}" and P[1965].startswith("impl Drop for Publish")
      and all(P[n - 1].startswith("    fn " + f) for n, f in ((1289, "rolled("), (1309, "facts("), (1413, "gating("),
                                                             (1619, "acknowledgement("), (1765, "resolve_gating("))),
      "C2f rolled (1289), facts (1309), gating (1413), acknowledgement (1619) and resolve_gating (1765) are methods of Publish (impl 1257-1964)")
MT0 = git("show", f"{commit}:crates/nexus-governed-control/src/ledger/protocol/model_tests.rs").decode("utf-8").split("\n")
check(MT0[551].strip() == "let pick = self.prefix.get(at).map_or(0, |&p| p as usize);" and "line 552 at 42dd1bd1" in draft,
      "C2g Oracle::choose defaults to option zero beyond its prefix (model_tests.rs line 552), as G.6 states")
WIT_H = ["C4 WITNESS F-8 P4: 8 steps, each enabled and consuming its choices as frozen; no violation; step 8 shows the healthy core's 'append_acknowledgement: Refused(NotNow(CellsBusy))', consuming 0 of its 6 offered choices; U stays empty and X holds F1's explanation",
         "C4 WITNESS F-8 P8: 12 steps, each enabled and consuming its choices as frozen; no violation; step 12 shows the healthy core's 'append_acknowledgement: Refused(NotNow(NotHealthy))', consuming 0 of its 4 offered choices; G1's written attempt stays in U",
         "C4 WITNESS F-8: 2 scenarios healthy; no violation"]
WIT_M = ["NAMED PROPERTY P4: OBSERVED on the world after step 8 of scenario P4 (append_acknowledgement: NotApplied(Uncertain); U holds the acknowledgement while X holds F1's explanation)",
         "NAMED PROPERTY P8: OBSERVED on the world after step 12 of scenario P8 (append_acknowledgement: Committed; G1's written attempt, not told Committed, is neither in U nor corrected in C or in the ledger)"]
for w in WIT_H:
    check(w in RAW_H and f"`{w}`" in draft, f"C2w healthy witness line quoted exactly as printed at A: {w[:60]}…")
for w in WIT_M:
    check(w in RAW_M and f"`{w}`" in draft, f"C2w mutant witness line quoted exactly as printed at A: {w[:60]}…")
TEMPL = [("C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; <n> violations",
          r"C4 WITNESS F-8: 2 scenarios, every step enabled and consuming its choices as planned; \d+ violations"),
         ("OTHER PROPERTIES VIOLATED in scenario P4, each reported separately and not counted as P4: [...]",
          r"OTHER PROPERTIES VIOLATED in scenario P4, each reported separately and not counted as P4: \[.*\]"),
         ("OTHER PROPERTIES VIOLATED in scenario P8, each reported separately and not counted as P8: [...]",
          r"OTHER PROPERTIES VIOLATED in scenario P8, each reported separately and not counted as P8: \[.*\]")]
for shown, rx in TEMPL:
    check(f"`{shown}`" in draft and any(re.fullmatch(rx, l) for l in RAW_M),
          f"C2w mutant witness template line matches a line printed at A: {shown[:50]}…")
for pre in ("NAMED PROPERTY P4: OBSERVED on the world after step 8 of scenario P4 (",
            "NAMED PROPERTY P8: OBSERVED on the world after step 12 of scenario P8 ("):
    check(f"`{pre}`" in draft and sum(l.startswith(pre) for l in RAW_M) == 1 and not any(pre in l for l in RAW_H),
          f"C2x frozen locus prefix printed once by the mutant and never by the healthy core: {pre[:40]}…")
for frag, raw in (("offered [0, 0, 0, 1, 0, 0]", RAW_M), ("[(0, 4), (0, 4), (0, 4), (1, 3), (0, 2), (0, 3)]", RAW_M),
                  ("offered [0, 0, 0, 0]: the healthy core must consume []", RAW_M),
                  ("consume [(0, 4), (0, 4), (0, 4), (0, 3)]", RAW_M)):
    shown = frag.replace("offered ", "").split(":")[0].replace("consume ", "")
    check(any(frag in l for l in raw) and f"`{shown}`" in draft, f"C2y G.6 stream {shown} as printed at A")
check(any("test result: ok. 1 passed; 0 failed" in l for l in RAW_H) and any("test result: FAILED. 0 passed; 1 failed" in l for l in RAW_M),
      "C2z prototype harness results: healthy 1 passed, mutant 1 failed")

# ---- C3 supersession table -------------------------------------------------------------------------------------------
rows = re.findall(r"^\| (S-\d+) \| ([0-9, ]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", draft, flags=re.M)
ids = [r[0] for r in rows]
check(ids == [f"S-{i}" for i in range(1, 26)], f"C3a supersession ids S-1..S-25, unique, in order ({len(ids)} rows)")
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
for sid, n in (("S-16", 742), ("S-17", 748), ("S-22", 704), ("S-23", 709), ("S-24", 710), ("S-25", 720)):
    check(n in s_lines and n not in ns_lines, f"C3g line {n} stays out of 'not superseded' ({sid})")
check("every row of 7.5 other than the readings of S-7, S-8, S-16 and S-22 to S-25" in ns,
      "C3h 'not superseded' names every row reading S-7, S-8, S-16, S-22..S-25")
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
check(inv_names.get("full_f8", ("",) * 7)[4] == "REQUIRED_FINAL_RUN" and "C4" in inv_names.get("full_f8", ("",) * 7)[5],
      "C4h full_f8 is REQUIRED_FINAL_RUN and its mutant run is C4's")
check("C9" in inv_names.get("full_fb2", ("",) * 7)[5] and "C10" in inv_names.get("full_f1", ("",) * 7)[5]
      and "C24" in inv_names.get("full_f1", ("",) * 7)[5], "C4i C9 runs on full_fb2, C10 and C24 on full_f1 (E.3)")

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
pins = git("show", f"{evb}:{CP}/receipts/pins-final.json").decode("utf-8")
known = {
    "contract": SHA["L"], "review-02": SHA["R2"], "review-03": SHA["R3"], "controls_defs.py": SHA["D"],
    "AD-1": sha_file(f"{A}/p3-c11-dr01-d2a1-closure-prep-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-DRAFT.md"),
    "AD-1-R1": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R1-DRAFT.md"),
    "AD-1-R2": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r2-contract-evidence-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md"),
    "AD-1-R3": sha_git("docs/evidence/p3-candidate11/ad1r3-review-01/ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md",
                       "c2c69e3a8a5cc447cac8a86424a1600a1f23cecf"),
    "disposition sheet": sha_git(f"{CP}/disposition/AD1R3-DISPOSITION-01-DRAFT.md", evb),
    "protocol.rs": sha_git("crates/nexus-governed-control/src/ledger/protocol.rs"),
    "historical manifest": sha_git("docs/evidence/p3-candidate10/original-183/manifest.json"),
    "C1 patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C1-mutation.patch"),
    "C25 narrow patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-witness-audit-01/patches/C25-narrow-mutation.patch"),
    "C25 historical patch": sha_file(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C25-mutation.patch"),
    "C4 patch": sha_git(f"{CP}/patches/C4-test1-removed.patch", evb),
    "healthy executable (pins at B)": re.search(r'"baseline_executable_sha256": "([0-9a-f]{64})"', pins).group(1),
    "mutant executable (pins at B)": re.search(r'"mutant_executable_sha256": "([0-9a-f]{64})"', pins).group(1),
}
EXPECT = {"AD-1": "01778ac35cf98dfe97935dfc0ed413cbdfc18cb953b9d426822bef430061176e",
          "AD-1-R1": "589eb6cb24626585862749fed3fa619d2261cdbc8b08c56e7139c6d12bc04cda",
          "AD-1-R2": "53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc",
          "AD-1-R3": "485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e",
          "disposition sheet": "a8a6d29376cf6f393a7eaa9047923719a339377caa554527a8878760cbdcf11c",
          "C4 patch": "24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d",
          "healthy executable (pins at B)": "f72a09a338df176121e69bda8cba8bb30ff11b4ef9666a9a796cf77c6a52f62e",
          "mutant executable (pins at B)": "5a4c4f17ea5b3883fb87a1a16fe19d53810608d4c229548a37932c405aa1fe8c"}
for k, v in EXPECT.items():
    check(known[k] == v, f"C6a {k} sha256 = mission's expected {v[:8]}…")
full = set(re.findall(r"\b[0-9a-f]{64}\b", draft))
for h in sorted(full):
    owner = [k for k, v in known.items() if v == h]
    check(bool(owner), f"C6b full sha256 {h[:12]}… in draft is a computed identity ({', '.join(owner) or 'UNKNOWN'})")
for a, b in re.findall(r"`([0-9a-f]{8})…([0-9a-f]{4})`", draft):
    owner = [k for k, v in known.items() if v.startswith(a) and v.endswith(b)]
    check(bool(owner), f"C6c abbreviated sha256 {a}…{b} matches {', '.join(owner) or 'NOTHING'}")
for a in re.findall(r"`([0-9a-f]{8})…`", draft):
    owner = [k for k, v in known.items() if v.startswith(a)]
    check(bool(owner), f"C6c' abbreviated sha256 {a}… matches {', '.join(owner) or 'NOTHING'}")
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
          "b1d535f1", "8693e3a7", "c2c69e3a", "04e42a7d", "a464a366"):
    t = subprocess.run(["git", "-C", repo, "cat-file", "-t", c], capture_output=True, text=True).stdout.strip()
    check(t == "commit" and c in draft, f"C6e {c} names a commit in the repository")
check(git("rev-parse", f"{commit}^{{tree}}").decode().startswith("7ac6bb21"), "C6f source tree is 7ac6bb21")
GITIDS = {  # every full 40-hex id the draft quotes, and what it must be
    "04e42a7ddf0d0a2af17b6506c51997006e7a08dc": ("commit", lambda: git("log", "-1", "--format=%P", "04e42a7ddf0d0a2af17b6506c51997006e7a08dc").decode().strip() == commit),
    "fca5c2a28e1c6cfef3cd92fbb81484a56769a9b3": ("tree", lambda: git("rev-parse", "04e42a7ddf0d0a2af17b6506c51997006e7a08dc^{tree}").decode().strip() == "fca5c2a28e1c6cfef3cd92fbb81484a56769a9b3"),
    "28085ce232827301104b5d757af042ec933976f8": ("blob", lambda: git("rev-parse", "04e42a7ddf0d0a2af17b6506c51997006e7a08dc:" + FILES[0]).decode().strip() == "28085ce232827301104b5d757af042ec933976f8"),
    "a464a36621523f539d2554a6b176d678c7f885ea": ("commit", lambda: git("log", "-1", "--format=%P", "a464a36621523f539d2554a6b176d678c7f885ea").decode().strip() == "04e42a7ddf0d0a2af17b6506c51997006e7a08dc"),
    "f3793c2ff61f8adbf43fe17d6b5237b49bc507b0": ("blob (derived in C1r-c)", lambda: True),
    "a2df2841b99cad5d452008a6d4150e9369d07e66": ("blob", lambda: git("rev-parse", f"{commit}:crates/nexus-governed-control/src/ledger/protocol.rs").decode().strip() == "a2df2841b99cad5d452008a6d4150e9369d07e66"),
    "7f2463f5421502cc6ee74c070fc3abc41d467770": ("blob", lambda: git("rev-parse", "c2c69e3a8a5cc447cac8a86424a1600a1f23cecf:docs/evidence/p3-candidate11/ad1r3-review-01/ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md").decode().strip() == "7f2463f5421502cc6ee74c070fc3abc41d467770"),
}
for h in sorted(set(re.findall(r"(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])", draft))):
    ok = h in GITIDS and GITIDS[h][1]()
    check(ok, f"C6g full git id {h[:12]}… in draft is verified ({GITIDS.get(h, ('UNKNOWN',))[0]})")
check(git("diff", "--numstat", commit, "04e42a7ddf0d0a2af17b6506c51997006e7a08dc").decode().split() == ["678", "0", FILES[0]]
      and "+678/−0" in draft, "C6h A's only change is model_tests.rs +678/-0")

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
                  ("Q: F-3/F-PT/F-B 24", r"^\| F-3, F-PT, F-B \(original or admitted quotient\) \| 24 \| E10 \|"),
                  ("Q: F-8 among the 24/E10 rows", r"^\| F-1, F-2, F-4, F-5, F-6, F-7, F-8, F-B2, F-G, F-R \| 24 \| E10 \|")):
    check(re.search(rx, draft, flags=re.M) is not None, f"C8b result-line counts kept: {label}")

# ---- C9 status and stale or contradictory operative phrases ---------------------------------------------------------
head = "\n".join(D[:4])
check("AD-1-R4" in D[0] and "NOT ACCEPTED" in head and "NOT ADOPTED" in head and "NOT APPLIED" in head and "DRAFT" in head,
      "C9a title AD-1-R4; status DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED")
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
    (r"not settled here", [], "C4's reading ruled (S-22)"),
    (r"a list or a\s*conjunction|a list or a$", [], "C4's reading ruled (S-22)"),
    (r"other than the mutant executable of \(a\)", [], "R3-01: role-specific identity"),
    (r"type \(e\)|types? \(e\)[^0-9]|F\.1 \(e\)[^0-9]|\(e\) Unchanged-core", [], "R3-02: (e) split into (e1)-(e4)"),
    (r"every restored receipt of 7\.5", [], "R3-02: restored receipts typed"),
    (r"C1/C25 witness receipt\.\*\*", [], "type (c) covers C1, C25 and C4"),
    (r"G applies to C1 and C25 only", [], "G.3 for C1/C25, G.5 for C4"),
    (r"in the review packet as inert copies", [], "this packet carries no patch"),
    (r"ad1r3-review-01/`, in one documentation-only commit", [], "this packet's prefix"),
]
for pat, allowed, why in STALE:
    hits = [l for l in OP if re.search(pat, l)]
    bad = [l for l in hits if not any(a in l for a in allowed)]
    check(not bad, f"C9b operative text has no '{pat}' outside its allowed context ({why}); hits {len(hits)}")
claims = re.findall(r"\b(?:C1|C25|C4|C9|C10|C24)(?: and C25)? (?:is|are|was|were|has been|have been) (?:now |finally )?killed\b", operative)
check(not claims, f"C9c no sentence claims a control is killed ({claims})")
check("It may be used" not in operative and "now in force" not in operative, "C9d nothing is declared in force")
check(operative.count("No final mutation kill") + operative.count("No final kill") + operative.count("no final mutation kill")
      + operative.count("No final kill is established") >= 6,
      "C9e 'no final kill' stated for C1, C25 and C4 (rulings and G.1)")


def guard(cid, fn):
    try:
        fn()
    except Exception as e:  # a check that cannot be evaluated is a FAIL, never a crash
        fails.append(f"FAIL  {cid} checker could not evaluate this category: {type(e).__name__}: {e}")


f1 = sec("### F.1 Receipt types", "\n### F.2 ")
f2 = sec("### F.2 Witness invocations", "\n## G. ")
TYPES = ["a", "b", "c", "d", "e1", "e2", "e3", "e4", "f"]
tdict = {}


def _types():
    heads = re.findall(r"^\*\*\(([a-f][0-9]?)\) ([^*]+)\*\*", f1, flags=re.M)
    for i, (letter, _) in enumerate(heads):
        nxt = heads[i + 1][0] if i + 1 < len(heads) else None
        body = f1.split(f"**({letter}) ", 1)[1]
        tdict[letter] = body.split(f"**({nxt}) ", 1)[0] if nxt else body
    return [h[0] for h in heads]


heads = _types()


# ---- C10 typed receipts ---------------------------------------------------------------------------------------------
def _c10():
    s6 = row["S-6"][3]
    check("every failing receipt" not in s6 and "REPLACED for C1 and C25 only: the kill criterion is G.3." in s6
          and "REPLACED for C4 only: the kill criterion is G.5." in s6 and "types (c) and (e3), never to any other receipt" in s6
          and "F.1" in s6 and "K.2" in s6 and "C22 exception as contracted" in s6,
          "C10a S-6: G.3 for C1/C25, G.5 for C4; witness rules only on types (c)/(e3); F.1/K.2; C22 exception kept")
    check(heads == TYPES, f"C10b F.1 defines types (a),(b),(c),(d),(e1)-(e4),(f) once each, in order ({heads})")
    trow = re.findall(r"^\| \(([a-f][0-9]?)\) \| ([^|]+) \| (mutant|baseline) \| ([^|]+) \|$", f1, flags=re.M)
    check([t[0] for t in trow] == TYPES, f"C10b' F.1's taxonomy table lists the same nine types ({[t[0] for t in trow]})")
    ta, tb, tc, td, te1, te2, te3, te4, tf = (flat(tdict.get(k, "")) for k in TYPES)
    check("Not required: any witness line, witness locus or witness verdict" in ta and "without a witness marker is complete" in ta
          and "4130–4140" in ta and "4115–4128" in ta and "K.1" in ta and "named property" in ta
          and "primary receipt of C1, C25 and C4" in ta and "C4's attribution of both P4 and P8 comes from G.5(B), never from this receipt alone" in ta,
          "C10c type (a): explorer verdict assertion; named property; cardinality; no witness requirement; C4 via G.5(B)")
    check(all(x in tb for x in ("C14 T-BIND", "C19 T-CELL", "C21 W6", "C27 T-PREC", "C28 T-RES", "unit_tests.rs:<line>"))
          and "Not required: an explorer outcome line, or any witness line" in tb,
          "C10d type (b): the five named unit-test controls; own-check location; no explorer/witness requirement")
    check("only to them" in tc and "do not apply to the primary receipt of C1, C25 or C4 (type (a)), or to any other control" in tc
          and "G.3(b)" in tc and "G.5(B)" in tc and "NOT PRODUCED BY DESIGN" in tc and "exit 101" in tc,
          "C10e type (c): the mutant witness; witness-only rules confined to the witness invocations; explorer fields not produced")
    check(all(x in td for x in ("error[E0283]", "line 1089", "internal compiler error", "type (e4) receipt",
                                 "NOT PRODUCED BY DESIGN", "No executable hash is invented", "no witness is required",
                                 "E0283 at another line", "out-of-memory", "recorded as found",
                                 "never extends to the healthy build's outputs (e4)")),
          "C10f type (d): C22 nonzero compile, E0283 at the T-OWN line, rejections, (e4) healthy evidence, artifacts recorded as found")
    ut = git("show", f"{commit}:{FILES[1]}").decode("utf-8").split("\n")
    check(ut[1088].strip() == "<Ticket as NotClone<_>>::check();" and "`<Ticket as NotClone<_>>::check();`" in td
          and "fn t_own_tickets_snapshots_and_the_worker_cannot_be_cloned" in ut[1087 - 1 + 0] + ut[1087],
          "C10g unit_tests.rs line 1089 at the source commit is T-OWN's Ticket check, as type (d) quotes")
    check(all(x in te1 for x in ("exited 0", "not by a signal", "out-of-memory", "K.1", "N.11", "every property line of Q",
                                  "Only this type requires an explorer outcome, state or key counts, property lines and branch coverage",
                                  "the restored receipt of every control that names a configuration")),
          "C10h type (e1): unchanged-core exploration PASS; the only type needing explorer evidence")
    check(all(x in te2 for x in ("`--exact`", "pinned baseline executable", "test result: ok. 1 passed; 0 failed",
                                  "K.2 points 5 to 7", "NOT PRODUCED BY DESIGN", "C14 T-BIND", "C28 T-RES")),
          "C10h2 type (e2): restored named unit test on the baseline executable; explorer fields not produced")
    check(all(x in te3 for x in ("pinned unchanged-baseline executable", "1 passed", "exit 0", "no violation", "K.2 points 5 to 7",
                                  "NOT PRODUCED BY DESIGN", "G.3(d)", "G.5(C)", "G.6")),
          "C10h3 type (e3): healthy witness on the baseline executable; healthy lines; explorer fields not produced")
    check(all(x in te4 for x in ("exit 0", "finished line", "Every artifact the build actually produces, executables included",
                                  "path, sha256 and size", "None is marked NOT PRODUCED BY DESIGN", "unmutated baseline")),
          "C10h4 type (e4): healthy build success; every produced artifact recorded; none marked not produced")
    check("Two admissible receipts of type (e1)" in tf and "not a kill receipt" in tf, "C10i type (f): two (e1) receipts; not a kill receipt")
    g3 = sec("### G.3 The kill criterion", "### G.4")
    g3a = g3.split("(b) **", 1)[0]
    check("NAMED PROPERTY" not in g3a and "No witness line is required" in g3a and "K.2" in g3a,
          "C10j G.3(a) (the primary receipt) carries no witness-line requirement and defers to K.2")
    check("(c) **Rejections of the witness receipt.** They apply only to the witness invocations of (b) and (d) (F.1 types (c) and (e3); F.2)" in flat(g3),
          "C10k G.3(c) is scoped to the witness invocations (types (c) and (e3))")
    named_lines = [i for i, l in enumerate(OP) if "NAMED PROPERTY P" in l]
    g3_start = next(i for i, l in enumerate(OP) if l.startswith("### G.3 "))
    g4_start = next(i for i, l in enumerate(OP) if l.startswith("### G.4 "))
    g5_start = next(i for i, l in enumerate(OP) if l.startswith("### G.5 "))
    g6_start = next(i for i, l in enumerate(OP) if l.startswith("### G.6 "))
    check(named_lines and all(g3_start < i < g4_start or g5_start < i < g6_start for i in named_lines),
          f"C10l the exact positive witness lines appear only inside G.3 and G.5 ({len(named_lines)} lines)")
    fsec = flat(sec("\n## F. ", "### F.1"))
    check("admissible receipt of its own type" in fsec and "beyond its ceiling (K.1)" in fsec
          and "read under G.3 for C1 and C25 and under G.5 for C4" in fsec,
          "C10m F's kill sentence: an admissible receipt of its own type; G.3 for C1/C25, G.5 for C4; FAIL beyond C kills nothing")
    check("restored receipt that shows the unchanged core passes the configuration is of type (e1)" in fsec
          and "(type (e2))" in fsec and "(type (e4))" in fsec,
          "C10o F's restored receipts typed: (e1) for configurations, (e2) for named unit tests, (e4) for C22")
    isec = flat(sec("\n## I. ", "\n## J. "))
    check("for C22: NOT PRODUCED BY DESIGN, since its build is meant to fail, F.1 (d); any artifact it does leave is recorded as found" in isec
          and "(e1) for a control that names a configuration, (e2) for a control killed by a named unit test, (e4) for C22 (the healthy build evidence of F.1 (d))" in isec,
          "C10n I's mutant identity list: C22 not produced but artifacts recorded; restored receipts typed")


# ---- C11 process-aware classification ------------------------------------------------------------------------------
def _c11():
    k = sec("\n## K. ", "\n## L. ")
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
    k2flat = flat(k2)
    check(all(x in k2flat for x in need), "C11f K.2's seven points name identity, selection, raw output, harness result, exit/signal, supervisor records, completion/cardinality")
    check("PASS text followed by a kill, a timeout, an out-of-memory kill, an incomplete termination, an identity mismatch or missing completion evidence is not PASS" in k2flat,
          "C11g K.2: PASS text plus kill/timeout/OOM/incomplete/identity/missing completion is not PASS")
    check("a nonzero status alone shows nothing" in k2flat and "raw outcome, unchanged, with any counterexample" in k2flat,
          "C11h K.2: nonzero status insufficient; raw outcome and admissibility kept apart")
    crow = re.findall(r"^\| (\d+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", k2, flags=re.M)
    check([c[0] for c in crow] == [str(i) for i in range(1, 15)], f"C11i classification table has cases 1-14 ({[c[0] for c in crow]})")
    EXP = {"1": ("(F.1 (e1))", "PASS"), "2": ("kills the process", "never PASS"),
           "3": ("positive line", "G.3(b) holds"), "4": ("not the named test's own verdict", "NOT ADMISSIBLE"),
           "5": ("no witness line", "no witness marker is required"), "6": ("no mutant executable exists", "NOT PRODUCED BY DESIGN"),
           "7": ("printed count above C", "INCOMPLETE — BUDGET EXCEEDED"), "8": ("both exact positive lines", "G.5(B) holds"),
           "9": ("only one of P4 and P8", "both properties are required"), "10": ("(F.1 (e3))", "PASS (type (e3))"),
           "11": ("other role's executable", "NOT ADMISSIBLE (role identity, F.2)"), "12": ("(F.1 (e4))", "no artifact marked NOT PRODUCED BY DESIGN"),
           "13": ("(F.1 (e2))", "PASS (type (e2))"), "14": ("beyond its offered stream", "NOT ADMISSIBLE (G.6)")}
    for c in crow:
        case, outer = c[1], c[3]
        a, b = EXP.get(c[0], ("\x00", "\x00"))
        check(a in case and b in outer, f"C11j case {c[0]}: '{a}' -> '{b}'")
    check("These seven points apply to every receipt type of F.1" in k2flat
          and "(F.1 (b), (c), (e2) and (e3)), point 2 needs no configuration bounds and point 7 no discovered count" in k2flat
          and "For C22's build-failure receipt (F.1 (d)), points 1, 2, 4 and 7 read as" in k2flat
          and "No executable, test selection or test run is required of it, since none is produced by design" in k2flat
          and "For a healthy build's receipt (F.1 (e4)), points 1, 2, 4 and 7 read as for (d), but with the intended zero exit" in k2flat
          and "every artifact the build actually produced recorded by path, sha256 and size" in k2flat
          and "Points 3, 5 and 6 read alike for every type" in k2flat,
          "C11m K.2 applies to every type, with readings for (b), (c), (e2), (e3), (d) and (e4) consistent with F.1")
    kill = {c[0]: c[4].strip() for c in crow}
    check(all(kill.get(n) == "no" for n in ("2", "4", "7", "9", "11", "14")), "C11k cases 2, 4, 7, 9, 11 and 14 are never kill evidence")
    check(all(kill.get(n, "").startswith("a part only") for n in ("3", "8", "10")), "C11n witness cases 3, 8 and 10 are a part only")
    s1 = row["S-1"][3]; s14 = row["S-14"][3]
    check("BUDGET EXCEEDED" in s1 and "K.2" in s1 and "K.2" in s14 and "BUDGET EXCEEDED" in s14 and "VERIFICATION INVALID" in s14,
          "C11l S-1 and S-14 carry K.1/K.2 explicitly")


# ---- C12 trace wording ----------------------------------------------------------------------------------------------
def _c12():
    s2, s19 = row["S-2"][3], row["S-19"]
    check("shortest trace" not in s2 and "S-19" in s2, "C12a S-2 no longer clarifies the trace phrase; points to S-19")
    check(s19[1].strip() == "609" and s19[3].strip().startswith("REPLACED") and "shortest breadth-first parent path to the checked state" in s19[3]
          and "offending transition appended" in s19[3] and "No global counterexample minimality and no shortest cycle witness" in s19[3],
          "C12b S-19 replaces line 609's phrase with the scoped statement; no global minimality; no cycle witness")
    o = sec("\n## O. ", "\n## P. ")
    o4 = o.split("\n4. ", 1)[1].split("\n5. ", 1)[0]
    o4flat = flat(o4)
    check("is REPLACED (S-19)" in o4flat and "shortest breadth-first parent path" in o4flat and "checked state" in o4flat
          and "offending transition appended" in o4flat and "If the Architect" not in o4flat and "proposes" not in o4flat
          and "kept unedited" in o4flat and "replay requirement of D" in o4flat,
          "C12c O.4 has one operative reading; raw trace wording kept; replay unchanged")
    oflat = flat(o)
    check("Global counterexample minimality" in oflat and "not a shortest witness of one" in oflat
          and "The minimality of the C1, C25 and C4 witness schedules" in oflat,
          "C12d O.3 keeps global minimality, cycle witness and witness-schedule minimality NOT ESTABLISHED")
    check(any("path_to" in l for l in MT[5056:5058]) and any("path_to" in l for l in MT[6597:6599])
          and any("path_to" in l for l in MT[3895:3897]),
          "C12e all three explorers build the trace with the breadth-first parent table (lines 3896, 5057, 6598)")
    check(any("shortest trace ({} steps):" in l for l in MT[4070:4072]), "C12f explore prints 'shortest trace (<n> steps):' (line 4071)")
    check("C4's first-reported property is recorded exactly as printed" in oflat and "never by relabelling the explorer's report" in oflat
          and "E3 is never counted as P1, and E4 never as P7" in oflat,
          "C12g O.6: C4's first property as printed; no relabelling; E3/E4 never P1/P7")


# ---- C13 authority mapping -------------------------------------------------------------------------------------------
def _c13():
    rsec = sec("\n## R. ", "\n---\n")
    rflat = flat(rsec)
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
    r_start = OP.index(next(l for l in OP if l.startswith("## R. ")))
    r_end = r_start + next(i for i, l in enumerate(OP[r_start:]) if l == "---")
    ok_ctx, n = [], 0
    for i, l in enumerate(OP):
        for word in ("Codex", "Lead Architect", "First Architect", "Claude Chat"):
            if word not in l:
                continue
            n += 1
            in_r = r_start <= i <= r_end
            in_s = (l.startswith("| S-20 ") or l.startswith("| S-21 ") or l.startswith("| AU ")
                    or l in (L[3], L[4], L[922], L[927], L[975], SRC["R2"][237], SRC["R3"][80], SRC["R3"][82]))
            ok_ctx.append(in_r or in_s)
            if not (in_r or in_s):
                print(f"  role word outside the mapping: line {i + 1}: {l[:100]}")
    check(all(ok_ctx), f"C13f every operative mention of Codex, Lead/First Architect or Claude Chat is a quotation or the mapping itself ({n})")


# ---- C14 dispositions carried (mission section 9) --------------------------------------------------------------------
def _c14():
    f = flat(operative)
    CARRIED = [
        ("F-1 60M standing ceiling only after Owner approval", "in force only once the Owner approves it"),
        ("no one-run authorisation inferred", "No further one is inferred"),
        ("composed quotient only F-3, F-PT, F-B", "Only F-3, F-PT and F-B may use the composed quotient for 8.3"),
        ("quotient keys counted only when E admits", "For F-3, F-PT and F-B, and only when E admits them, the 30,000,000-state ceiling of 8.3 counts the keys of the composed quotient"),
        ("every other configuration and every mutant run counts original states", "For every other configuration, and for every mutant run, the ceiling counts the original explorer's states."),
        ("symmetry never for F-8 among others", "F-5, F-6, F-7, F-8 or F-B2 (acknowledgements or callers, ASYM-1/2)"),
        ("no mutation control under a quotient", "No mutation control is ever run under a quotient (F)."),
        ("a control is never run under a quotient", "A control is never run under a quotient."),
        ("proof, configuration and final-source prerequisites (N)", "If any one fails, the claim is not made."),
        ("no inherited approval", "A change to a proof-relevant input inherits no approval."),
        ("fifteen configurations: twelve original + three", "For the other twelve it is the original explorer's"),
        ("ignored-test policy: proposed only", "It approves skipping no test the contract now requires."),
        ("ignored-test policy: recomputed at the final commit", "It is recomputed mechanically from both test files at the exact final commit."),
        ("diagnostics kept", "Diagnostic tests kept."), ("D1/L1 kept", "Kahn's algorithm over the quotient's non-fault edges"),
        ("replay kept", "replayed in the original semantics before it is cited"),
        ("no additional full-run instrumentation", "No new instrumentation."),
        ("original explorer for mutation controls", "runs the ORIGINAL explorer (`explore`, through that configuration's `full_*` or default test), on the mutant build"),
        ("historical INCOMPLETE stays INCOMPLETE", "Every INCOMPLETE run below stays INCOMPLETE, establishes no property"),
        ("no report says the original F-3 graph passed", "No report may say that the original F-3 graph was explored or passed."),
        ("cap-overshoot is detection, not budget", "The overshoot is how the stop is detected. It is not extra budget"),
        ("inclusive cardinality", "a run is within its ceiling if and only if it completes with at most C keys discovered"),
        ("exact-final-baseline evidence, no historical substitution", "No historical PASS substitutes for a final-baseline run."),
        ("mutant tree is not the baseline tree", "A mutant tree is not the baseline tree"),
        ("detached attestation", "It is never placed in the commit it attests."),
        ("historical manifest preserved", "is preserved byte for byte. It is not edited, regenerated or overwritten."),
        ("historical review fields not current approval", "They are not current-tree approval."),
        ("current-tree manifest neither created nor approved", "This draft neither creates nor approves it."),
        ("corrected CAS retry wording", "No bound on the number of failed exchanges, and no wait-freedom, is claimed (6.3)."),
        ("A-1/A-2 undischarged", "stay stated and undischarged"),
        ("results conditional on storage assumptions", "Every result above is conditional on them"),
        ("no SQLite durability claim", "No model result establishes actual SQLite durability"),
        ("property lines applicability", "A line that is not applicable, or not claimed, is printed as such. It is never counted as HOLDS, and never as a failure."),
        ("result lines authoritative", "The individual lines are authoritative: a summary count is a check on them, never a substitute."),
        ("receipt linking: own restoration", "its own verified baseline restoration"),
        ("receipt linking: own artifact-identity check", "its own artifact-identity check"),
        ("receipt linking still proposed", "Until it is approved, the present reading of line 853 governs."),
        ("probe helpers historical, not controllers", "They are not approved final campaign controllers."),
        ("C25 narrow keeps self.keep = true", "preserve `self.failed();`, `self.rolled(io.rollback());` and `self.keep = true;`"),
        ("INCOMPLETE never PASS", "INCOMPLETE is never PASS"),
        ("trace wording replaced once", "The one operative text, for every run, original or quotient, healthy or mutant"),
    ]
    for label, phrase in CARRIED:
        check(phrase in f, f"C14 carried: {label}")


# ---- C15 Not superseded list and 0A ---------------------------------------------------------------------------------
def _c15():
    check("read as S-18 and S-20 state" in draft and "read under S-21 (role names only)" in draft,
          "C15a 'Not superseded' list aligned with S-20/S-21")
    rul = sec("### 0A.1", "### 0A.2")
    c4r = rul.split("**The ruling of P3-C11-DR01-D2A1-AD1R3-READINESS-01**", 1)[0]
    c4flat = flat(c4r)
    need = ["**The ruling of P3-C11-DR01-D2A1-C4-WITNESS-01** (governs where it and the earlier rulings below differ)",
            "SOURCE AND EVIDENCE REVIEW PASSED. Prototype acceptance only. It is not a final mutation kill.",
            "ARCHITECT ACCEPTED AS THE REQUIRED REPAIR BASIS.", "| R3-01 | ACCEPT.", "| R3-02 | ACCEPT.",
            "Both P4 AND P8 remain required. The comma is never read as \"or\".", "The deterministic witness does not replace `full_f8`.",
            "exactly as the authenticated experimental patch defines it", "ACCEPT as proposed in AD1R3-DISPOSITION-01. Oracle is not changed.",
            "C9-b selected: inside `Publish::rolled`, `Rollback::Unknown` keeps `self.failed()` and changes only `Persistence::Uncertain` to `Persistence::NotWritten`.",
            "Retain the authenticated sealed definitions.", "No C9, C10 or C24 execution is authorised.",
            "The prior rulings are unchanged. No final kill is established."]
    for x in need:
        check(x in c4flat, f"C15b 0A.1 carries the C4-WITNESS-01 ruling: {x[:70]}…")
    rflat2 = flat(rul)
    check(all(x in rflat2 for x in ("AD-1-R2 | REPAIR.", "historical receipts support its intended P1 attribution",
                                    "self.keep = true` preserved", "No additional probe is justified",
                                    "The old compound C25 definition is historical only",
                                    "Final AQ/SQ proof acceptance", "final baseline selection", "integration")),
          "C15c 0A.1 still carries the AD1R3-READINESS-01 ruling (REPAIR; C1; C25; pending items)")
    rep = re.findall(r"^\| (R2-01|R2-02|AU|TR|R3-01|R3-02|C4|C9-C10-C24) \|", sec("### 0A.2", "### 0A.3"), flags=re.M)
    check(rep == ["R2-01", "R2-02", "AU", "TR", "R3-01", "R3-02", "C4", "C9-C10-C24"], f"C15d 0A.2 lists the eight repairs ({rep})")


# ---- C16 open items -------------------------------------------------------------------------------------------------
def _c16():
    oi = sec("## Open items", "## Changes from AD-1-R3")
    nums_oi = re.findall(r"^(\d+)\. ", oi, flags=re.M)
    check(nums_oi == [str(i) for i in range(1, 13)], f"C16a open items 1-12 ({nums_oi})")
    oiflat = flat(oi)
    check("Closed as to C4's reading by the Architect's ruling (S-22, G.5): both P4 and P8 are required" in oiflat
          and "remain predictions, not measurements" in oiflat and "G.3 and G.5 extend to no other control" in oiflat
          and "C9, C10 and C24 under their G.2 definitions, each by its own authorised mission" in oiflat,
          "C16b item 10: C4's reading closed by ruling; C9/C10/C24 need missions; predictions; no extension")
    check("`full_f8` primary run on the C4 mutant (G.5(A))" in oiflat and "none is made or authorised here" in oiflat,
          "C16c item 5: full_f8 and carrying the witnesses need their own missions")
    check("The Owner's approval of A, and of this addendum as a whole." in oiflat, "C16d item 1: Owner approval")


# ---- C17 R3-01: witness identity by role ----------------------------------------------------------------------------
def _c17():
    ff = flat(f2)
    check(all(x in ff for x in (
        "artifact identity is role-specific",
        "**Mutant witness (type (c)).** It runs on the same pinned mutant executable as its control's primary mutation invocation (that control's type (a) receipt), where that primary invocation exists, and it binds the named mutation source identity of I",
        "**Healthy witness (type (e3)).** It runs on the pinned unchanged-baseline executable of I",
        "a wrong source or executable identity, including an executable other than the one its role requires",
        "a wrong test selection", "a fixture failure (`WITNESS FIXTURE FAILURE`)", "an unrelated panic or failure",
        "a compilation failure", "incomplete raw output", "invalid process termination or resource evidence",
        "Expected outcomes remain role-specific",
        "A positive marker in a healthy-witness receipt, or a healthy line in a mutant-witness receipt, is NOT ADMISSIBLE.")),
        "C17a F.2: mutant witness on its control's mutant executable; healthy on the baseline executable; common rejections")
    rr = re.findall(r"^\| (mutant|healthy) \| \((c|e3)\) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$", f2, flags=re.M)
    check(len(rr) == 2 and rr[0][0:2] == ("mutant", "c") and "mutant executable" in rr[0][2] and "exit 101" in rr[0][3]
          and rr[1][0:2] == ("healthy", "e3") and "unchanged-baseline executable" in rr[1][2] and "mutant" not in rr[1][2]
          and "exit 0" in rr[1][3] and "no positive marker" in rr[1][4], "C17b F.2's role table: (c) mutant/exit 101; (e3) baseline/exit 0")
    g3 = flat(sec("### G.3 The kill criterion", "### G.4"))
    check("an executable other than the one its role requires (F.2): for (b), the pinned mutant executable of (a); for (d), the pinned unchanged-baseline executable" in g3
          and "on the pinned unchanged-baseline executable of the exact final baseline (F.2), PASSes (type (e3))" in g3,
          "C17c G.3(c) and (d) are role-specific; the healthy replay runs on the baseline executable")
    isec = flat(sec("\n## I. ", "\n## J. "))
    check("for C1, C25 and C4, its mutant witness run. Every healthy witness runs on the pinned unchanged-baseline executable (F.2)." in isec,
          "C17d I: the mutant executable serves the mutant witness; every healthy witness on the baseline executable")


# ---- C18 R3-02: one taxonomy; every type reference names a defined type ----------------------------------------------
def _c18():
    refs = []
    for m in re.finditer(r"(?:F\.1 types?|F\.1|types?) \(([a-z][0-9]?)\)(?:(?:,? and |, | to | or )\(([a-z][0-9]?)\))*", operative):
        refs += re.findall(r"\(([a-z][0-9]?)\)", m.group(0))
    bad = sorted({r for r in refs if r not in TYPES})
    check(refs and not bad, f"C18a every type reference in operative text names a defined type ({len(refs)} references; undefined: {bad})")
    check(not re.search(r"(?:type|types|F\.1) \(e\)", operative), "C18b no reference to an undivided type (e)")
    outside = operative.replace(f1, "")
    used = set()
    for m in re.finditer(r"(?:F\.1 types?|F\.1|types?) \(([a-z][0-9]?)\)(?:(?:,? and |, | to | or )\(([a-z][0-9]?)\))*", outside):
        used |= set(re.findall(r"\(([a-z][0-9]?)\)", m.group(0)))
    want = {"c", "e1", "e2", "e3", "e4"}
    check(want <= used, f"C18c types (c), (e1)-(e4) are each referred to outside F.1 (missing {sorted(want - used)})")
    s5 = row["S-5"][3]
    check("(e1) for a control that names a configuration, (e2) for a control killed by a named unit test, (e4) for C22" in s5,
          "C18d S-5 states the restored-receipt types")
    c0a = sec("### 0A.2", "### 0A.3")
    check("(a), (b), (c), (d), (e1) to (e4) and (f)" in c0a, "C18e 0A.2 names the one taxonomy")


# ---- C19 C22: produced and not-produced artifacts --------------------------------------------------------------------
def _c19():
    paras = re.split(r"\n(?=\s*[-|*0-9(])|\n\n", operative)
    hits = [p for p in paras if "NOT PRODUCED BY DESIGN" in p]
    ALLOW = ("C22", "type (d)", "(F.1 (d))", "explorer report", "explorer fields", "does not produce", "never erases",
             "None is marked", "no artifact marked", "never extends")
    bad = [p for p in hits if not any(a in flat(p) for a in ALLOW)]
    check(hits and not bad, f"C19a every NOT PRODUCED BY DESIGN is in a C22, explorer-field or never-erases context ({len(hits)} hits; bad {len(bad)})")
    for p in bad:
        print("  NOT PRODUCED context not allowed:", flat(p)[:140])
    te4 = tdict.get("e4", "")
    check("NOT PRODUCED BY DESIGN" in te4 and "None is\n  marked NOT PRODUCED BY DESIGN" in te4
          and te4.count("NOT PRODUCED BY DESIGN") == 1, "C19b type (e4) uses NOT PRODUCED BY DESIGN only to forbid it")
    f1flat = flat(f1)
    check("An artifact or field that does exist is recorded as it is: no NOT PRODUCED BY DESIGN marking ever erases it." in f1flat,
          "C19c F.1's general rule: no NOT PRODUCED BY DESIGN marking erases an existing artifact")
    check("This marking covers only that mutant build. Any artifact it does leave is recorded as found (path, sha256 and size), and it never extends to the healthy build's outputs (e4)." in flat(tdict.get("d", "")),
          "C19d type (d): the C22 exemption is the mutant build's only")


# ---- C20 C4 ----------------------------------------------------------------------------------------------------------
def _c20():
    s22 = row["S-22"]
    check(s22[1].strip() == "704" and "CLARIFIED (ARCHITECT-RULED): the cell \"P4, P8\" is a conjunction: both P4 and P8 must be shown, and the comma is never read as \"or\"." in s22[3]
          and "\"P4, P8 (first failure <exact property printed>; witness)\"" in s22[3] and "exact definition of G.2" in s22[3],
          "C20a S-22: conjunction; never 'or'; recording form; definition in G.2")
    g2 = flat(sec("### G.2 ", "### G.3 "))
    check(all(x in g2 for x in ("the complete removal of Test 1's five checks, exactly as the authenticated experimental patch defines it",
                                 "Test 0 is preserved", "`let core = &mut *self.core;` is preserved",
                                 "the five checks of Test 1 are removed: the session is Open; no latch; the phase is Healthy; C and X are empty; U is empty, with the cell-invariant latch when it is not",
                                 "all code from `let set = snapshot.set;` is preserved", "Nothing else changes.",
                                 "lines 1627–1642 removed, none added")),
          "C20b G.2: C4's definition (Test 0 kept; five checks removed; let core kept; from let set kept)")
    g5 = sec("### G.5 ", "### G.6 ")
    g5f = flat(g5)
    order = [g5.find(f"\n({x}) **") for x in "ABCDEF"]
    check(all(o > 0 for o in order) and order == sorted(order), f"C20c G.5 has clauses (A) to (F) in order ({order})")
    need = ["For C4 only", "Both P4 AND P8 are required (S-22).", "The deterministic witness does not replace `full_f8`.",
            "run as `full_f8` selected alone with `--exact` and `--ignored` on the pinned C4 mutant executable, under F-8's ceiling of 30,000,000 states, FAILs",
            "The receipt is of type (a) (F.1), admissible under K.2, and within its cap",
            "The actual first property is recorded unchanged, exactly as printed. It is never relabelled.",
            "If it names neither P4 nor P8, that is reported as it is and not reinterpreted",
            "on the same pinned mutant executable as (A)", "two separately initialised scenarios",
            "unchanged `initial(F8)` with at most 32 actor steps", "collects both scenarios before its verdict",
            "**Only P4, or only P8, is insufficient.**",
            "(C) **Violation-free healthy replays.** The same witness, selected alone with `--exact` on the pinned unchanged-baseline executable (F.2), on the same two schedules, PASSes (type (e3))",
            "consuming 0 of its 6 offered choices; U stays empty and X holds F1's explanation",
            "consuming 0 of its 4 offered choices; G1's written attempt stays in U",
            "(D) **The unchanged core's F-8 PASS.**", "type (e1)", "(E) **Identity, restoration and review.**",
            "Sole Architect's independent review",
            "(F) **Recording.** Only then is the row recorded as 'C4: P4, P8 (first failure <exact property printed>; witness)'.",
            "historical supporting evidence only", "discharge none of (A) to (E)", "any breach of G.6",
            "They are kept in the raw output, and they are not further conditions of (B)"]
    for x in need:
        check(x in g5f, f"C20d G.5: {x[:80]}…")
    for x in ("G.3 applies to C1 and C25 only. G.5 and G.6 apply to C4 only. Neither is an exception for any other control",):
        check(x in flat(sec("## G. ", "### G.1")), "C20e G's preamble scopes G.3 to C1/C25 and G.5/G.6 to C4")
    rec = [l for l in OP if "C4: P4, P8 (first failure" in l]
    check(len(rec) == 1 and "Only then is the row recorded" in rec[0], f"C20f the C4 recording form appears once, in G.5(F) ({len(rec)})")
    g1 = flat(sec("### G.1 ", "### G.2 "))
    check("Historical supporting evidence only, not final-baseline evidence; it discharges none of G.5(A) to (E). No final kill." in g1
          and "no `full_f8` run on the C4 mutant, has been made" in g1, "C20g G.1: the C4 prototype is historical; no full_f8 run made")


# ---- C21 the offered-versus-consumed Oracle rule --------------------------------------------------------------------
def _c21():
    g6 = sec("### G.6 ", "\n## H. ")
    g6f = flat(g6)
    items = re.findall(r"^(\d)\. ", g6, flags=re.M)
    check(items == list("123456"), f"C21a G.6 has rules 1-6 ({items})")
    need = ["Oracle defaults to option zero once its explicit prefix ends", "Silent fallback is never accepted as a specified schedule",
            "Oracle and the original replay helpers are not changed",
            "Healthy and mutant executions receive the same actor sequence, and the same deterministic stream of choices is offered at every corresponding step",
            "consumption, option counts, branch labels and remaining fault budget are identical on both cores, and frozen",
            "healthy: it refuses before storage and consumes zero storage choices; the full offered suffix remains unused",
            "mutant: it consumes exactly the frozen storage-choice stream; no suffix remains unused",
            "Every consumed choice equals the offered prefix",
            "Rejected: consumption beyond the offered stream; consumption differing from the offered prefix; an unexplained unused suffix (one not justified and frozen before execution); a fixture deviation; a property at the wrong locus",
            "verified twice: by the witness, and by the classifier from the raw output",
            "`c4_witness_p4_and_p8_on_f8`", "Carrying it to the final baseline needs its own authorised mission"]
    for x in need:
        check(x in g6f, f"C21b G.6: {x[:80]}…")
    A_MT = git("show", "04e42a7ddf0d0a2af17b6506c51997006e7a08dc:" + FILES[0]).decode("utf-8")
    check("fn c4_witness_p4_and_p8_on_f8()" in A_MT and "beyond the {} offered" in A_MT and "not a prefix of the offered" in A_MT,
          "C21c the witness at A names that test and rejects consumption beyond, or other than, the offered stream")


# ---- C22 C9-b --------------------------------------------------------------------------------------------------------
def _c22():
    s23 = row["S-23"][3]
    check("C9-b of G.2: only the persistence answer changes, and `self.failed()` is kept" in s23 and "E4 and F-B2: unchanged" in s23,
          "C22a S-23: C9-b; self.failed() kept; E4 and F-B2 unchanged")
    g2 = flat(sec("### G.2 ", "### G.3 "))
    check("(ARCHITECT-RULED: C9-b selected). In `Publish::rolled` (`protocol.rs` lines 1289–1297 at 42dd1bd1), inside the arm `Rollback::Unknown`, `self.failed();` is kept and only `Persistence::Uncertain` (line 1294) is replaced with `Persistence::NotWritten`. No other behaviour changes. Named property E4; configuration F-B2." in g2
          and "which also removed `self.failed();`, is not selected" in g2,
          "C22b G.2: C9-b written out; the other candidate not selected")
    check(not re.search(r"C9-a", operative), "C22c no operative mention of C9-a")


# ---- C23 C10 and C24: retained sealed definitions -------------------------------------------------------------------
def _c23():
    tree = ast.parse("\n".join(SRC["D"]))
    names, controls = {}, None
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            if isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
                names[node.targets[0].id] = node.value.value
            if node.targets[0].id == "CONTROLS" and isinstance(node.value, ast.Dict):
                controls = node.value

    def lit(n):
        if isinstance(n, ast.Constant):
            return n.value
        if isinstance(n, ast.Name):
            return names[n.id]
        if isinstance(n, ast.BinOp) and isinstance(n.op, ast.Add):
            return lit(n.left) + lit(n.right)
        if isinstance(n, (ast.Tuple, ast.List)):
            return [lit(e) for e in n.elts]
        raise ValueError(f"not a literal: {ast.dump(n)[:60]}")
    defs = {lit(k): lit(v) for k, v in zip(controls.keys, controls.values)}
    text = "\n".join(P)
    for cid, prop, lines_ in (("C10", "E4", "1404–1408"), ("C24", "P8", "1774–1787")):
        meaning, path, edits, kind, target, props = defs[cid]
        anchor, repl = edits[0]
        start = text[: text.find(anchor)].count("\n") + 1
        end = start + anchor.count("\n")
        check(len(edits) == 1 and text.count(anchor) == 1 and text.count(repl) == 0 and target == "full_f1" and props == [prop]
              and f"lines {lines_} at 42dd1bd1, occurring exactly once" in flat(draft) and f"{start}–{end}" == lines_,
              f"C23a {cid}: sealed definition, target full_f1, property {prop}; anchor once at protocol.rs {start}-{end}; replacement absent")
    check("C9" not in defs and "C4" not in defs, "C23b the sealed file defines neither C9 nor C4 (their definitions are G.2's)")
    for sid, n in (("S-24", "E4 and F-1: unchanged"), ("S-25", "P8 and F-1: unchanged")):
        check("retained sealed definition quoted in G.2" in row[sid][3] and n in row[sid][3], f"C23c {sid}: retained sealed definition; property and configuration unchanged")
    g2 = flat(sec("### G.2 ", "### G.3 "))
    check(g2.count("ARCHITECT-RULED: the authenticated sealed definition is retained unchanged") == 2
          and "No execution of any of them is authorised by this draft or by the ruling it carries." in g2,
          "C23d G.2: C10 and C24 retained unchanged; no execution authorised")


# ---- C24 C1 and C25 rules (prior rulings unchanged) ------------------------------------------------------------------
def _c24():
    g = flat(sec("## G. ", "\n## H. "))
    need = ["`if shift == UNAPPLIED_SHIFT { return with_field(word, shift, u64::from(up)); }`",
            "preserve `self.failed();`, `self.rolled(io.rollback());` and `self.keep = true;`, and replace ONLY `return refused(Persistence::Uncertain, Reason::Storage);` (line 1476, its only occurrence in the file) with `return GatingOutcome::Committed(seq);`",
            "The compound mutation that also drops `self.keep = true` is historical evidence only.",
            "Predicted: E3 in 4 steps for C1; E4 in 1 step for C25.", "E3 is never counted as P1, and E4 never as P7.",
            "If it reports otherwise, that is reported as it is and not reinterpreted.",
            "`NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)`",
            "`NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))`",
            "'C1: P1 (first failure E3; witness)' and 'C25: P7 (first failure E4; witness)'",
            "Historical evidence, not a final-baseline receipt. No final kill."]
    for x in need:
        check(x in g, f"C24a G: C1/C25 rule kept: {x[:80]}…")
    check(row["S-7"][3].startswith("REPLACED: the cell \"P1\" is read as \"P1 (first failure E3; witness)\"")
          and row["S-8"][3].startswith("REPLACED: the cell \"P7\" is read as \"P7 (first failure E4; witness)\""),
          "C24b S-7 and S-8 unchanged")


# ---- C25 self-containment --------------------------------------------------------------------------------------------
def _c25():
    body = operative.split("## Provenance labels", 1)[1]
    bad = re.findall(r"\bR3 \d{2,4}\b|\(R3 \d|AD-1-R3 line|disposition sheet,? (?:§|section|line) ?\d|§\s?\d|read (?:it )?(?:with|beside) (?:AD-1-R3|the disposition)", body)
    check(not bad, f"C25a no operative text points into AD-1-R3 or the disposition sheet by line or section ({bad[:3]})")
    hd = flat("\n".join(D[:40]))
    check("A reader does not need AD-1, AD-1-R1, AD-1-R2, AD-1-R3 or AD1R3-DISPOSITION-01 to determine it." in hd
          and "none is to be read beside this file as a supplement" in hd, "C25b the header states self-containment")
    t = sec("### 0A.5", "\n---\n")
    for name, h in (("AD-1", EXPECT["AD-1"]), ("AD-1-R1", EXPECT["AD-1-R1"]), ("AD-1-R2", EXPECT["AD-1-R2"]),
                    ("AD-1-R3", EXPECT["AD-1-R3"]), ("AD1R3-DISPOSITION-01", EXPECT["disposition sheet"])):
        check(re.search(rf"^\| {re.escape(name)} \| sha256 `{h}`[^|]*\| history;", t, flags=re.M) is not None,
              f"C25c 0A.5 lists {name} as history with its sha256")
    check("None of them is changed, regenerated or overwritten." in t, "C25d 0A.5: none is changed")


# ---- C26 no authorisation, no adoption --------------------------------------------------------------------------------
def _c26():
    f = flat(operative)
    for x in ("authorises any run, mutation, source change, manifest, adoption or integration",
              "reads C4's \"P4, P8\" as a choice between them",
              "None of these rulings, and nothing in this draft, accepts D2A-1, integrates it, or completes Phase Three.",
              "The Owner keeps authority over every contract amendment and over its adoption.",
              "applies unchanged, with its binding reviews", "No C9, C10 or C24 execution is authorised."):
        check(x in f, f"C26a {x[:80]}")
    check(not re.search(r"\bthis (?:draft|addendum|amendment) (?:is|has been) (?:adopted|accepted|approved|in force)\b", f, flags=re.I),
          "C26b nothing states that this draft is adopted, accepted, approved or in force")


# ---- C27 final-baseline versus historical evidence -------------------------------------------------------------------
def _c27():
    isec = flat(sec("\n## I. ", "\n## J. "))
    check("the C4 witness prototype's healthy and mutant receipts at A (04e42a7d), carried by evidence commit B" in isec
          and "The following are informational and are not 8.3 or 7.5 evidence" in isec,
          "C27a I lists the C4 prototype receipts as informational, not 8.3 or 7.5 evidence")
    check("every C1, C25 and C4 witness receipt (mutant and healthy)" in isec, "C27b I binds every witness receipt to the exact final baseline")


for cid, fn in (("C10", _c10), ("C11", _c11), ("C12", _c12), ("C13", _c13), ("C14", _c14), ("C15", _c15), ("C16", _c16),
                ("C17", _c17), ("C18", _c18), ("C19", _c19), ("C20", _c20), ("C21", _c21), ("C22", _c22), ("C23", _c23),
                ("C24", _c24), ("C25", _c25), ("C26", _c26), ("C27", _c27)):
    guard(cid, fn)

print()
for n in notes:
    print(n)
for f in fails:
    print(f)
print(f"\nSUMMARY: {len(notes)} passed, {len(fails)} failed")
sys.exit(1 if fails else 0)
