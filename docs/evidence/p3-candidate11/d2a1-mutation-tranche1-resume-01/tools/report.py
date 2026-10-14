"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01: shared, deterministic parsers for the pinned executable's output.

parse_report(text)   one `EXPLORATION ...` block, as `report()` / `explore_symmetric_kept` print it at C
check_pass(rep)      the PASS form AD-1-R4 Q requires, with the branch list of 8.4 (D) complete
model_constants()    PROPERTIES and BRANCHES, read from model_tests.rs at C (git object, not the worktree)

Nothing here runs or builds anything.
"""
import re
import subprocess

C = "d5964ee5326c2733177307db6f507328023d361a"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
REPO = "/home/nexus/NEXUS/nexus-os"
E10_NA = "not applicable here (worker verbs do not unwind)"
COMPOSED = ("(gating-caller symmetry quotient composed with the admission observer quotient: experimental, not the "
            "contract's explorer)")

STATES = re.compile(r"^  states (\d+) \(expanded (\d+)\), transitions (\d+) \(with a fault (\d+)\), states with no "
                    r"enabled step (\d+)$")
ELAPSED = re.compile(r"^  elapsed ([0-9.]+) s; explorer tables (\d+) MiB$")
QUOTIENT = re.compile(r"^  quotient: ADM is not scheduled; its one step is evaluated at every expanded state \((\d+) "
                      r"evaluations, (\d+) admitted\)")
SYMMETRY = re.compile(r"^  symmetry: G1 and G2 exchanged; .* (\d+) expanded keys are their own mirror\.")
L1_HOLDS = re.compile(r"^HOLDS: (\d+) non-fault edges, no cycle; (\d+) end states, each terminal \(D1\)$")
BOUNDS = re.compile(r"caps (\d+) states, (\d+)s$")


def model_text():
    return subprocess.run(["git", "-C", REPO, "cat-file", "blob", f"{C}:{MODEL}"], capture_output=True,
                          check=True).stdout.decode()


def model_constants():
    text = model_text()
    props = re.search(r"const PROPERTIES: \[&str; 25\] = \[(.*?)\];", text, re.S).group(1)
    branches = re.search(r"const BRANCHES: &\[&str\] = &\[(.*?)\n\];", text, re.S).group(1)
    return re.findall(r'"([^"]+)"', props), re.findall(r'"((?:[^"\\]|\\.)*)"', branches)


def parse_report(text):
    """The first EXPLORATION block in text: every printed field, kept as printed."""
    lines = text.splitlines()
    start = next((i for i, l in enumerate(lines) if "EXPLORATION " in l), None)
    if start is None:
        return None
    head = lines[start][lines[start].index("EXPLORATION "):]
    rep = {"header": head, "name": head.split()[1], "lines": [], "properties": [], "branches": [], "extra_branches": [],
           "key_fields": None, "incomplete": None, "fail": None, "trace_steps": None}
    for l in lines[start + 1:]:
        rep["lines"].append(l)
        if l.startswith("  state key fields: "):
            rep["key_fields"] = l[len("  state key fields: "):]
            break
        if l.startswith("  outcome: "):
            rep["outcome"] = l[len("  outcome: "):]
        elif l.startswith("  bounds: "):
            rep["bounds"] = l[len("  bounds: "):]
            m = BOUNDS.search(l)
            rep["cap"], rep["time_cap_s"] = (int(m.group(1)), int(m.group(2))) if m else (None, None)
        elif l.startswith("  actors: "):
            rep["actors"] = l[len("  actors: "):]
        elif QUOTIENT.match(l):
            m = QUOTIENT.match(l)
            rep["quotient_line"] = l.strip()
            rep["evaluations"], rep["admitted"] = int(m.group(1)), int(m.group(2))
        elif SYMMETRY.match(l):
            rep["symmetry_line"] = l.strip()
            rep["self_mirrored"] = int(SYMMETRY.match(l).group(1))
        elif STATES.match(l):
            st, ex, tr, fe, ns = map(int, STATES.match(l).groups())
            rep.update(states=st, expanded=ex, transitions=tr, fault_edges=fe, no_step=ns)
        elif ELAPSED.match(l):
            rep["elapsed_s"], rep["tables_mib"] = float(ELAPSED.match(l).group(1)), int(ELAPSED.match(l).group(2))
        elif l.startswith("  INCOMPLETE: "):
            rep["incomplete"] = l[len("  INCOMPLETE: "):]
        elif l.startswith("  FAIL: "):
            rep["fail"] = l[len("  FAIL: "):]
        elif l.startswith("  shortest trace ("):
            rep["trace_steps"] = l
        elif re.match(r"^  (P\d+|D1|L1|E\d+|KEY): ", l):
            k, _, v = l.strip().partition(": ")
            rep["properties"].append((k, v))
        elif l.startswith("    [taken    ] ") or l.startswith("    [NOT TAKEN] "):
            label = l[16:]
            if label.endswith(" (not in the listed set)"):
                rep["extra_branches"].append(label)
            else:
                rep["branches"].append((l[5:14], label))
    return rep


def check_pass(rep, props, branches):
    """[(label, ok, detail)] for the PASS form of AD-1-R4 Q and D (F.1 (e1) raw output)."""
    out = []
    add = lambda label, ok, detail="": out.append((label, bool(ok), detail))
    add("outcome PASS", rep.get("outcome") == "PASS", rep.get("outcome"))
    add("no INCOMPLETE line, no FAIL line", rep["incomplete"] is None and rep["fail"] is None,
        f"incomplete {rep['incomplete']}, fail {rep['fail']}")
    names = [k for k, _ in rep["properties"]]
    add("one line for each of the 25 PROPERTIES, in order", names == props, f"{names}")
    pv = dict(rep["properties"])
    add("E10: not applicable", pv.get("E10") == E10_NA, pv.get("E10"))
    l1 = L1_HOLDS.match(pv.get("L1", ""))
    add("L1 HOLDS (non-fault edges, no cycle; end states terminal)", l1, pv.get("L1"))
    add("D1 HOLDS", pv.get("D1") == "HOLDS", pv.get("D1"))
    holds = [k for k, v in rep["properties"] if v == "HOLDS" or (k == "L1" and l1)]
    others = [k for k, v in rep["properties"] if k not in holds and k != "E10"]
    add("24 lines HOLD, E10 not applicable, no other line", len(holds) == 24 and not others,
        f"{len(holds)} HOLD; others {others}")
    have = ("states", "expanded", "transitions", "fault_edges", "no_step")
    add("counts printed", all(k in rep for k in have), "")
    if all(k in rep for k in have):
        add("expanded equals states (0 unexpanded)", rep["expanded"] == rep["states"],
            f"states {rep['states']}, expanded {rep['expanded']}")
        add("states within the printed cap (K.1, inclusive)", rep.get("cap") is not None and rep["states"] <= rep["cap"],
            f"{rep['states']} <= {rep.get('cap')}")
        if l1:
            add("L1's non-fault edges = transitions - fault edges", int(l1.group(1)) == rep["transitions"] - rep["fault_edges"],
                f"{l1.group(1)} = {rep['transitions']} - {rep['fault_edges']}")
            add("L1's end states = states with no enabled step", int(l1.group(2)) == rep["no_step"],
                f"{l1.group(2)} = {rep['no_step']}")
    labels = [b for _, b in rep["branches"]]
    add("branch list: every BRANCHES label once, in order, each marked taken or NOT TAKEN",
        labels == branches and all(m in ("taken    ", "NOT TAKEN") for m, _ in rep["branches"]),
        f"{len(labels)} labels listed of {len(branches)}")
    # The explorer prints a taken label that BRANCHES does not list as '(not in the listed set)'. Historical PASS
    # receipts carry such lines (F-1, F-PT); they are recorded as printed, never dropped, and are not a failure.
    add("taken labels outside the listed set: recorded as printed", True, f"{rep['extra_branches']}")
    add("state key fields line present", rep["key_fields"] is not None, "")
    return out
