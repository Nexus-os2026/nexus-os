#!/usr/bin/env python3
"""Mechanical checks of the C9/C10/C24/C4 attribution preflight (P3-C11-DR01-D2A1-AD1R3-READINESS-01).
Each source citation the preflight makes is checked against the source commit's bytes (git show): the cited line
holds the cited text. Contract rows, identities and the preflight's own required statements are checked too.
Reads only; writes nothing; runs nothing from Nexus. Exit 0 only if every check passes.
    check-preflight.py <preflight.md> <contract> <repo> <commit> <evidence-root>"""
import hashlib, re, subprocess, sys

pf_path, contract_path, repo, commit, root = sys.argv[1:6]
fails, notes = [], []


def check(ok, what):
    (notes if ok else fails).append(("PASS  " if ok else "FAIL  ") + what)


def show(path):
    return subprocess.run(["git", "-C", repo, "show", f"{commit}:{path}"], check=True,
                          capture_output=True).stdout.decode("utf-8").split("\n")


PF = open(pf_path, encoding="utf-8").read()
PFF = " ".join(PF.split())
P = show("crates/nexus-governed-control/src/ledger/protocol.rs")
M = show("crates/nexus-governed-control/src/ledger/protocol/model_tests.rs")
craw = open(contract_path, "rb").read()
check(hashlib.sha256(craw).hexdigest() == "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c", "contract sha256")
L = craw.decode("utf-8").split("\n")
print(f"preflight {pf_path}\n  sha256 {hashlib.sha256(open(pf_path, 'rb').read()).hexdigest()}")

# (file, line, substring that the line must contain, the preflight citation it supports)
CITES = [
    ("P", 1289, "fn rolled(", "P 1289-1297 rolled"), ("P", 1292, "Rollback::Unknown => {", "P 1292-1295"),
    ("P", 1293, "self.failed();", "P 1292-1295"), ("P", 1294, "Persistence::Uncertain", "P 1292-1295"),
    ("P", 1398, "let persistence = self.rolled(io.rollback());", "A7 P 1398"),
    ("P", 1486, "let persistence = self.rolled(io.rollback());", "A7 P 1486"),
    ("P", 1687, "let persistence = self.rolled(io.rollback());", "A7 P 1687"),
    ("P", 1913, "return match self.rolled(io.rollback()) {", "A7 P 1913"),
    ("P", 1384, "for (index, (member, slot)) in members().zip(pending.iter()).enumerate() {", "P 1384-1400 step 5"),
    ("P", 1397, "self.failed();", "P 1397"), ("P", 1405, "self.failed();", "P 1405"),
    ("P", 1404, "Commit::Unknown => {", "C10 anchor P 1404-1408"),
    ("P", 1407, "self.not_applied(Persistence::Uncertain, Reason::Storage)", "C10 anchor P 1404-1408"),
    ("P", 1774, "Read::Got(Some(row)) if row.digest == key.digest => {", "C24 anchor P 1774-1787"),
    ("P", 1786, "Resolve::Corrected(self.core.token(Cell::C, n))", "C24 anchor P 1774-1787"),
    ("P", 1813, "self.core.u = None;", "P 1813-1815"), ("P", 1814, "self.core.phase = Phase::Healthy;", "P 1813-1815"),
    ("P", 1472, "Commit::Unknown => {", "P 1470-1477"), ("P", 1475, "self.keep = true;", "P 1470-1477"),
    ("P", 1984, "if self.hold != Hold::Nothing {", "P 1984-1994 guard"), ("P", 1986, "self.core.u = None;", "P 1984-1987"),
    ("P", 1713, "fn barrier(", "P 1713-1729"), ("P", 1723, "self.core.phase = if self.core.u.is_some() {", "P 1713-1729"),
    ("P", 1732, "fn resolve(", "P 1732-1763"), ("P", 1995, "let flags = self.core.flags();", "P 1995-2004"),
    ("P", 1625, "// Test 1.", "P 1625-1642"), ("P", 1641, "return not_now(Entry::Latched);", "P 1625-1642"),
    ("P", 1643, "let set = snapshot.set;", "after test 1"),
    ("P", 1139, "fn place(&mut self, cell: Cell, occupant: Occupant) -> bool {", "P 1139-1149 place"),
    ("P", 1144, "Cell::U => self.u = Some((occupancy, occupant)),", "place overwrites U"),
    ("P", 1688, "self.keep = persistence == Persistence::Uncertain;", "P 1680-1704"),
    ("P", 1702, "self.keep = true;", "P 1680-1704"), ("P", 1982, "self.keep = self.issued;", "P 1966-1983"),
    ("P", 1601, "if set.top > set.frontier && set.count > 0 {", "P 1599-1610"),
    ("P", 1859, "fn cell(", "cell append"), ("P", 858, "if let Some(holder) = facts.holder {", "P 858-864"),
    ("P", 859, "return Some(CauseKind::DuplicateRole {", "P 858-864"),
    ("P", 1221, "self.refusals.insert(member.key, cause);", "P 1221-1224"),
    ("P", 1223, "self.explanation(member.key, cause);", "P 1221-1224"),
    ("P", 1391, "Insert::Constraint => {", "constraint latch"), ("P", 1392, "self.core.latch(LatchCause::Constraint);", "constraint latch"),
    ("M", 3882, "pub(super) fn explore(config: &Config) -> Report {", "M 3882-4110"),
    ("M", 3906, "while (id as usize) < visited.len() {", "M 3906"), ("M", 3915, "let key = visited.key(id).to_vec();", "M 3915"),
    ("M", 3896, "let path_to = |parent: &[(u32, u64)], mut id: u32| {", "M 3896-3904"),
    ("M", 3812, "state_properties(config, &base, &mut state);", "M 3812-3815"),
    ("M", 3917, "if let Some((property, detail)) = state_violations.into_iter().next() {", "M 3917-3920"),
    ("M", 3921, "if next.is_empty() {", "M 3921-3931"), ("M", 3817, "for (index, &actor) in all.iter().enumerate() {", "M 3817"),
    ("M", 3838, "match oracle.next() {", "M 3819-3842"),
    ("M", 3935, "if let Some((property, detail)) = successor.violations.into_iter().next() {", "M 3932-3940"),
    ("M", 4070, "FAIL: {property}: {detail}", "M 4067-4072"), ("M", 4071, "shortest trace ({} steps):", "M 4067-4072"),
    ("M", 3403, "fn state_properties(", "M 3403-3511"), ("M", 3407, "\"E1\"", "E1 first"), ("M", 3431, "\"E3\"", "E3"),
    ("M", 3440, "\"P2\"", "P2"), ("M", 3443, "\"P4\"", "P4 at M 3442-3444"), ("M", 3448, "\"P5\"", "P5"),
    ("M", 3475, "\"P7\"", "P7"), ("M", 3486, "if record_in && !in_u && !in_c && !corrected {", "P8 at M 3486"),
    ("M", 3488, "\"P8\"", "P8"), ("M", 3495, "\"P9\"", "P9"), ("M", 3501, "\"P13\"", "P13"), ("M", 3509, "\"E11\"", "E11"),
    ("M", 2427, "obs.fail(\"E5\"", "E5 before E4 in fact_append"), ("M", 2436, "if not_written && w.rows != before {", "E4 at M 2436"),
    ("M", 2438, "\"E4\"", "M 2434-2444"), ("M", 2419, "let mut io = Io::new(&mut w.rows, o, true, true);", "M 2419"),
    ("M", 3039, "obs.fail(\"E5\"", "M 3038-3101"), ("M", 3205, "\"E5\"", "M 3203-3236"), ("M", 3225, "\"E4\"", "M 3203-3236"),
    ("M", 2903, "obs.fail(\"E5\"", "M 2901-2959"),
    ("M", 2942, "label.ends_with(\"NotWritten\") || label.ends_with(\"Stale\") || label.contains(\"Refused\");", "M 2941-2945"),
    ("M", 2954, "\"E8\"", "M 2946-2959"), ("M", 3514, "fn transition_properties(", "M 3514-3545"),
    ("M", 546, "fn choose(&mut self, fault: &[bool]) -> usize {", "M 546-580"), ("M", 572, "fn next(&self) -> Option<Vec<u8>> {", "M 546-580"),
    ("M", 652, "fn pick(&mut self, verb: Verb, faults: &[Answer]) -> Answer {", "M 652-666"),
    ("M", 691, "outcomes.insert(0, Vec::new());", "M 684-694 absent first"),
    ("M", 750, "self.pending.clear();", "M 746-752 A-1"), ("M", 726, "_ => Insert::Failed,", "M 704-728"),
    ("M", 2332, "fn actors(config: &Config) -> Vec<Actor> {", "M 2332-2347"),
    ("M", 2963, "fn step_wk(", "M 2963-3016"), ("M", 2540, "if expires(o) {", "expiry choice first"),
    ("M", 449, "fn survey(rows: &[SimRow]) -> SetFacts {", "M 449-464"),
    ("M", 1085, "fn initial_rows(config: &Config) -> Vec<SimRow> {", "M 1085-1100"),
]
for f, n, sub, why in CITES:
    src = P if f == "P" else M
    check(sub in src[n - 1], f"{f} {n} holds {sub[:60]!r} ({why})")

consts = dict(re.findall(r"^const ([A-Z0-9]+): Config = Config \{(.*?)\n\};", "\n".join(M), flags=re.M | re.S))
start = {k: next(i + 1 for i, l in enumerate(M) if l.startswith(f"const {k}: Config")) for k in ("F1", "F8", "FB2")}
check(start == {"F1": 155, "F8": 207, "FB2": 227}, f"configuration constants at M 155, 207, 227 ({start})")
f1, f8, fb2 = consts["F1"], consts["F8"], consts["FB2"]
check("faults: 3" in f1 and "admissions: 2" in f1 and "gating: 2" in f1 and "conflict" not in f1, "F-1 as stated")
check("conflict: [true, false]" in f8 and "faults: 2" in f8 and "gating: 1" in f8 and "acks: 1" in f8 and "ack_attempts: 1" in f8,
      "F-8 as stated")
check("batch: true" in fb2 and "faults: 3" in fb2 and "admissions: 1" in fb2 and "gating" not in fb2 and "acks" not in fb2,
      "F-B2 as stated")

ROWS = {704: "| C4 | acknowledgement test 1 removed | P4, P8 | F-8 |",
        709: "| C9 | `Rollback::Unknown` is treated as `Undone` | E4 | F-B2 |",
        710: "| C10 | `Commit::Unknown` followed by `Rollback::Undone` gives Not written | E4 | F-1 |",
        720: "| C24 | resolve empties U for a present gating record without placing the correction | P8 | F-1 |",
        701: "| C3a | acknowledgement test 2 removed | P13 or P15 | F-7 |",
        702: "| C3b | acknowledgement test 4 removed | P13 or P12 | F-6 |",
        744: "| A6 | \"recovery required\" is a flag, not a comparison of H with A | P12 or P15 | F-5 |"}
for n, row in ROWS.items():
    check(L[n - 1] == row, f"contract line {n} is {row[:50]!r}")
check("open; not latched; Healthy; C and X empty. (U occupied while Healthy: latched, as 4.5.)" in L[436],
      "contract line 437 defines test 1 as quoted")
check(sum(1 for l in L if re.search(r"\| (P\d+|E\d+|L1), (P\d+|E\d+|L1) \|", l)) == 1, "C4's row is the only 7.5 row with a comma between two properties")

A = root
defs = f"{A}/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py"
check(hashlib.sha256(open(defs, "rb").read()).hexdigest() == "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f",
      "controls_defs.py sha256 ef0420a1…")
dtext = open(defs, encoding="utf-8").read()
check('"C9"' not in dtext and '"C4"' not in dtext and '"C10"' in dtext and '"C24"' in dtext,
      "controls_defs.py defines C10 and C24, and neither C9 nor C4")
r4 = f"{A}/p3-c11-d2a-model-01/REPORT-R4.txt"
check(hashlib.sha256(open(r4, "rb").read()).hexdigest() == "10312fb4b679f32f87d020a3fbe49888228fed3bd4d9fc5cdb884a0d78a35a5f"
      and "NC18  CFG-8: k_append test 1 removed entirely. Expected P9, P4 or P8. RESULT: P4, P8 and P9 VIOLATED." in open(r4, encoding="utf-8").read(),
      "scratch REPORT-R4 hash and NC18 line as quoted")
mx = open(f"{A}/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/receipts/stage6/03-configuration-and-control-matrix.txt", encoding="utf-8").read()
check(re.search(r"^F-8 .*never run", mx, flags=re.M) is not None and re.search(r"^F-B2 .*never run", mx, flags=re.M) is not None,
      "readiness matrix: F-8 and F-B2 never run unchanged")
cap = open(f"{A}/p3-c11-dr01-d2a1-f3-cap60-01/receipts/stage5/02-c1-c25-attribution.txt", encoding="utf-8").read()
check("Examples to check: C10 and C9 against E4, C24 against P8." in " ".join(cap.split()), "F3-CAP60-01 flag quoted as stated")

for phrase in ("READ-ONLY. NOTHING WAS EXECUTED.", "NONE AUTHENTICATED.", "not definitions", "not a definition",
               "So the original explorer is predicted to report P4 and never P8.",
               "It is not designed or implemented here.", "Neither P4 nor P8 is waived.",
               "The C1/C25 witness exception is not extended to C4.", "G is not broadened"):
    check(phrase in PFF, f"preflight states: {phrase!r}")
check(PF.count("NONE AUTHENTICATED.") == 2, "exactly C9 and C4 marked NONE AUTHENTICATED")
check(not re.search(r"\b(?:C4|C9|C10|C24) (?:is|was|has been) killed\b", PF), "no kill claimed")
check(not re.search(r"\bmeasured\b(?! from)", PF.replace("None was measured", "").replace("Not measured", "")),
      "no ordering is called measured")

print()
for n in notes:
    print(n)
for f in fails:
    print(f)
print(f"\nSUMMARY: {len(notes)} passed, {len(fails)} failed")
sys.exit(1 if fails else 0)
