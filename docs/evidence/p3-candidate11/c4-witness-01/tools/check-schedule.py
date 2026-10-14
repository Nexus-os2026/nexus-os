#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01: static checks of the appended C4 witness (no Nexus code is built or run).

  check-schedule.py <model_tests.rs> [<schedule.json>]

1. Append-only confinement: the file begins with the exact bytes of the baseline blob 5ad26ddc (read from git) and
   adds only after its end; the addition starts with the C4 witness header.
2. The appended block's frozen Rust constants equal the frozen schedule JSON (parsed independently).
3. The appended block uses the original machinery and none of the forbidden constructs.
Exit 0 only if every check holds.
"""
import hashlib, json, re, subprocess, sys

REPO = "/home/nexus/NEXUS/nexus-os"
BASE_BLOB = "5ad26ddc727d8be30f4fb8d7478bc69e1c974933"
HEADER = "\n// ---- named-property witness for control C4 ---"
fails = []


def chk(label, ok, detail=""):
    print(f"  {'OK  ' if ok else 'FAIL'} {label}{(': ' + detail) if detail else ''}")
    if not ok:
        fails.append(label)


def ints(text):
    return [int(v) for v in re.findall(r"\d+", text)]


def pairs(text):
    return [[int(a), int(b)] for a, b in re.findall(r"\((\d+),\s*(\d+)\)", text)]


def strings(text):
    return re.findall(r'"((?:[^"\\]|\\.)*)"', text)


def field(body, name):
    m = re.search(rf"\b{name}:\s*(.*?),\n", body, re.S)
    if not m:
        raise SystemExit(f"field {name} not found")
    return m.group(1)


def list_field(body, name):
    m = re.search(rf"\b{name}:\s*&\[(.*?)\],?\n", body, re.S)
    if m is None:
        m = re.search(rf"\b{name}:\s*&\[(.*?)\]\s*,", body, re.S)
    if m is None:
        raise SystemExit(f"list field {name} not found")
    return m.group(1)


def main():
    path = sys.argv[1]
    sched = json.load(open(sys.argv[2] if len(sys.argv) > 2 else
                           "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c4-witness-01/frozen/c4-schedule.json"))
    data = open(path, "rb").read()
    base = subprocess.run(["git", "-C", REPO, "cat-file", "blob", BASE_BLOB], capture_output=True, check=True).stdout
    print(f"file {path}: {len(data)} bytes, sha256 {hashlib.sha256(data).hexdigest()}")
    print(f"baseline blob {BASE_BLOB}: {len(base)} bytes, sha256 {hashlib.sha256(base).hexdigest()}")
    print("[1] append-only confinement")
    chk("the file begins with every byte of the baseline", data[:len(base)] == base)
    chk("the baseline ends with a newline", base.endswith(b"\n"))
    added = data[len(base):].decode()
    chk("the addition starts with the C4 witness header", added.startswith(HEADER))
    chk("the addition is the rest of the file", len(base) + len(added.encode()) == len(data))
    print(f"  added: {len(added.encode())} bytes, {added.count(chr(10))} lines (lines {base.count(b'\n') + 1}-{data.count(b'\n')})")

    print("[2] frozen constants equal the frozen schedule")
    steps = {}
    for name, body in re.findall(r"const (C4_[A-Z_]+): C4Step = C4Step \{\n(.*?)\n\};", added, re.S):
        body += "\n"
        actor = re.search(r"actor:\s*Actor::(\w+(?:\(\d+\))?)", body).group(1)
        steps[name] = {"actor": actor, "offered": ints(list_field(body, "offered")),
                       "consumed": pairs(list_field(body, "consumed")),
                       "branches": strings(list_field(body, "branches")), "faults": int(field(body, "faults"))}
    chk("step constants", sorted(steps) == sorted(sched["steps"]), f"{sorted(steps)}")
    for name, want in sched["steps"].items():
        chk(f"{name} = schedule", steps.get(name) == want, f"{steps.get(name)}")
    scen = {}
    for name, body in re.findall(r"const C4_(P\d): C4Scenario = C4Scenario \{\n(.*?)\n\};", added, re.S):
        body += "\n"
        def ending(which):
            m = re.search(rf"\b{which}: C4Ending \{{(.*?)\}},\n", body, re.S)
            b = m.group(1)
            return {"consumed": pairs(list_field(b, "consumed")), "branch": strings(field(b, "branch"))[0],
                    "faults": int(re.search(r"faults:\s*(\d+)", b).group(1))}
        named = strings(field(body, "named"))[0]
        top = re.sub(r"\b(healthy|changed): C4Ending \{.*?\},\n", "", body, flags=re.S)
        scen[name] = {"named": named, "steps": re.findall(r"C4_[A-Z_]+", list_field(top, "steps")),
                      "ack": {"actor": "Ack(0)", "offered": ints(list_field(top, "offered")),
                              "healthy": ending("healthy"), "changed": ending("changed")},
                      "keeps": strings(field(top, "keeps"))[0], "locus": strings(field(top, "locus"))[0],
                      "kept": field(top, "kept").strip(), "lost": field(top, "lost").strip()}
    chk("scenario constants", sorted(scen) == sorted(sched["scenarios"]), f"{sorted(scen)}")
    for name, want in sched["scenarios"].items():
        got = scen.get(name, {})
        chk(f"C4_{name}.named", got.get("named") == name)
        for k in ("steps", "ack", "keeps", "locus"):
            chk(f"C4_{name}.{k} = schedule", got.get(k) == want[k], f"{got.get(k)}")
        chk(f"C4_{name} predicates", (got.get("kept"), got.get("lost")) == (f"c4_{name.lower()}_kept", f"c4_{name.lower()}_lost"))
        n = len(want["steps"]) + 1
        chk(f"C4_{name}: {n} actor steps, at most 32", n <= 32)
        chk(f"C4_{name}: healthy end consumes nothing", want["ack"]["healthy"]["consumed"] == [])
        chk(f"C4_{name}: changed end consumes every offered choice",
            [c for c, _ in want["ack"]["changed"]["consumed"]] == want["ack"]["offered"])
        for s in want["steps"]:
            st = sched["steps"][s]
            chk(f"C4_{name}: {s} consumes exactly its offered choices", [c for c, _ in st["consumed"]] == st["offered"])
    chk("C4_SCENARIOS = [C4_P4, C4_P8]", "const C4_SCENARIOS: [C4Scenario; 2] = [C4_P4, C4_P8];" in added)
    chk("the test runs F8 with C4_SCENARIOS",
        re.search(r"#\[test\]\nfn c4_witness_p4_and_p8_on_f8\(\) \{\n    run_c4_witness\(&F8, &C4_SCENARIOS\);\n\}\n$", added) is not None)
    chk("one test named c4_witness_p4_and_p8_on_f8 in the whole file", data.decode().count("fn c4_witness_p4_and_p8_on_f8(") == 1)
    chk("the addition defines exactly one #[test] and no #[ignore]", added.count("#[test]") == 1 and "#[ignore" not in added)

    print("[3] original machinery used; nothing forbidden")
    code = "\n".join(l for l in added.splitlines() if not l.lstrip().startswith("//"))
    for needle in ["let enabled = step(config, &mut w, actor, &mut oracle, &mut obs);",
                   "let mut oracle = Oracle::new(offered.to_vec(), w.faults);",
                   "transition_properties(&base, &w, &mut obs, counts);", "violations_at(config, &next)",
                   "hook::take_counts();", "let base = decode(config, key);", "let mut w = decode(config, key);",
                   "let mut key = encode(config, &initial(config));", "initial: violations_at(config, &key),",
                   "w.faults = oracle.faults;"]:
        chk(f"uses `{needle}`", needle in code)
    forbidden = {
        "explore": r"\bexplore\w*\s*\(", "successors": r"\bsuccessors\s*\(", "Oracle::next": r"\.next\s*\(",
        "graph search / visited set": r"\bVisited\b|\bparent\b", "trace replay": r"\btrace\s*\(",
        "a new Config": r"Config \{", "forced state write": r"\b(core|w)\.(u|c|x|phase|latch|session|frontier|refusals)\s*=[^=]",
        "injected ledger row": r"rows\.(push|insert|append|extend)", "ghost write": r"ghost\.\w+\s*[|&]?=[^=]",
        "fault budget lowered": r"faults\s*[-+]=", "table write": r"table\.\w+\s*=[^=]",
        "new actor variant": r"enum Actor", "force_sample / malform": r"force_sample|malform\(",
        "environment switch": r"std::env|env::var|cfg!\(|#\[cfg", "Oracle field write": r"oracle\.(prefix|taken|faults|fault)\s*=[^=]",
        "substituted predicate": r"fn state_properties|fn transition_properties|fn step\b|fn admission_properties",
    }
    for label, pattern in forbidden.items():
        hits = [l.strip() for l in code.splitlines() if re.search(pattern, l)]
        chk(f"no {label}", not hits, f"{hits}" if hits else "")
    writes = [l.strip() for l in code.splitlines() if re.search(r"\bw\.\w+(\.\w+)*\s*=[^=]", l)]
    chk("the only world write is the oracle's fault budget, as replay_schedule makes it", writes == ["w.faults = oracle.faults;"], f"{writes}")
    print()
    print("CHECK-SCHEDULE RESULT:", "ALL CHECKS OK" if not fails else f"{len(fails)} FAILED: {fails}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
