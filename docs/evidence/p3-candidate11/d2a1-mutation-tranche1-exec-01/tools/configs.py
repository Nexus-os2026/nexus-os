#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01: source-derived configuration expectations.

  configs.py            prints every configuration's derived fields, test, bounds line, actors line, property
                        applicability and operation applicability; exit 0 only if every source anchor holds

Everything is read from model_tests.rs at C (git object): the Config constants (struct update syntax resolved), the
limit constants, the tests that run each configuration, and the source text of the bounds/actors/property printing,
which is ANCHORED (its exact text must be present) so that the Python re-implementation below cannot silently drift
from the explorer's own formatting. No expected state count is derived or asserted.
"""
import json
import re
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common  # noqa: E402

P = common.TEST_PREFIX

# The source text this module re-implements (whitespace-normalised).
ANCHORS = [
    # explore(): the bounds line, the actors line, E10 and L1.
    '"  bounds: facts {} conflicting {:?} ENV {} batch {} faults {} admissions {} gating callers {} \\',
    'gating conflict {} acknowledgement actors {} attempts each {} malformed snapshots {} cell constraint {} \\',
    'worker verbs unwind {} DROP {} REPLAY {} A4 {}; ticket limit {TICKET_LIMIT}, refusal limit \\',
    '{REFUSAL_LIMIT}; settlement and the worker on; caps {} states, {:?}",',
    'config.facts, config.conflict, config.env, config.batch, config.faults, config.admissions, config.gating, '
    'config.gating_conflict, config.acks, config.ack_attempts, config.malformed, config.cell_constraint, '
    'config.worker_unwinds, config.drop, config.replays, config.blanket_uncertain, config.cap, config.time',
    'let _ = writeln!(text, "  actors: {all:?}");',
    'let mut l1 = "not claimed for this configuration".to_string();',
    '} else if property == "E10" && !config.worker_unwinds { "not applicable here (worker verbs do not unwind)"'
    '.to_string() } else { "HOLDS".to_string() };',
    # actors().
    'fn actors(config: &Config) -> Vec<Actor> { let writers = if config.batch { 1 } else { config.facts }; let mut all: '
    'Vec<Actor> = (0..writers).map(Actor::Fw).collect(); all.push(Actor::Se); all.push(Actor::Wk); '
    'all.extend((0..config.gating).map(Actor::Gc)); all.extend((0..config.acks).map(Actor::Ack)); '
    'all.push(Actor::Adm); if config.env { all.push(Actor::Env); } if config.replays > 0 { all.push(Actor::Replay); } '
    'all }',
    # Operation applicability: the actors and the options that make an operation possible at all.
    'let options: &[bool] = if config.drop && counted_ticket { &[false, true, false] } else { &[false, true] };',
    'let options = if config.malformed { 2 + MALFORMATIONS.len() } else { 2 };',
    'AckPos::Idle => { if w.ack[i].attempts == 0 { return false; }',
    'fn step_adm(config: &Config, w: &mut World, obs: &mut Obs) -> bool { if w.adm == 0 { return false; }',
    'gc: vec![GcPos::Start; config.gating as usize],',
    'ack: (0..config.acks) .map(|_| Ack { attempts: config.ack_attempts,',
    'adm: config.admissions,',
    'faults: config.faults,',
    'fn expires(o: &mut Oracle) -> bool { o.choose(&[false, true]) == 1 }',
    '.filter(|&k| !fault[k] || self.faults > 0)',
]
FIELDS = ["name", "facts", "conflict", "env", "batch", "faults", "admissions", "gating", "gating_conflict", "acks",
          "ack_attempts", "malformed", "cell_constraint", "worker_unwinds", "drop", "replays", "blanket_uncertain", "l1",
          "cap", "time"]
# The required unchanged-core runs: configuration name -> (test, explorer).
REQUIRED = {
    "X-1": ("x1_two_facts_two_writers_one_admission", "original"),
    "X-2": ("x2_one_fact_two_gating_callers", "original"),
    "X-3": ("x3_one_conflicting_fact_with_env", "original"),
    "X-4": ("x4_one_conflicting_fact_one_acknowledgement", "original"),
    "X-5": ("x5_one_clean_batch_of_two", "original"),
    "F-1": ("full_f1", "original"), "F-2": ("full_f2", "original"),
    "F-3": ("gc_symmetry_f3", "composed"), "F-PT": ("gc_symmetry_fpt", "composed"), "F-B": ("gc_symmetry_fb", "composed"),
    "F-G": ("full_fg", "original"), "F-B2": ("full_fb2", "original"), "F-4": ("full_f4", "original"),
    "F-W": ("full_fw", "original"), "F-8": ("full_f8", "original"), "F-5": ("full_f5", "original"),
    "F-6": ("full_f6", "original"), "F-7": ("full_f7", "original"), "F-D": ("full_fd", "original"),
    "F-R": ("full_fr", "original"),
}
REMAINING = ["F-G", "F-B2", "F-4", "F-W", "F-8", "F-5", "F-6", "F-7", "F-D", "F-R"]
PROPERTIES_N = 25


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


def norm_tokens(s):
    """Whitespace removed between punctuation so that rustfmt's line breaks do not matter."""
    return re.sub(r"\s+", "", s)


def value(tok, consts):
    tok = tok.strip()
    if tok in ("true", "false"):
        return tok == "true"
    if re.fullmatch(r"\[\s*(true|false)\s*,\s*(true|false)\s*\]", tok):
        return [x == "true" for x in re.findall(r"true|false", tok)]
    if re.fullmatch(r"\d[\d_]*", tok):
        return int(tok.replace("_", ""))
    if re.fullmatch(r'"[^"]*"', tok):
        return tok[1:-1]
    if tok in consts:
        return consts[tok]
    raise SystemExit(f"configs: cannot read value {tok!r}")


def derive():
    src = common.source(common.MODEL)
    problems = []
    flat = norm_tokens(src)
    for a in ANCHORS:
        if norm_tokens(a) not in flat:
            problems.append(f"anchor not found: {a[:90]}")
    consts = {}
    for name in ("TICKET_LIMIT", "REFUSAL_LIMIT", "DEFAULT_CAP", "FULL_CAP"):
        m = re.search(r"const " + name + r": \w+ = ([\d_]+);", src)
        consts[name] = int(m.group(1).replace("_", ""))
    m = re.search(r"const DEFAULT_TIME: Duration = Duration::from_secs\((\d+)\);", src)
    consts["DEFAULT_TIME"] = int(m.group(1))
    m = re.search(r"const FULL_TIME: Duration = Duration::from_secs\((\d+) \* (\d+)\);", src)
    consts["FULL_TIME"] = int(m.group(1)) * int(m.group(2))
    raw = {}
    for m in re.finditer(r"const (\w+): Config = (default_run\()?Config \{(.*?)\}\)?;", src, re.S):
        name, dr, body = m.group(1), bool(m.group(2)), m.group(3)
        fields, base = {}, None
        for line in body.split("\n"):
            line = line.strip().rstrip(",")
            if not line or line.startswith("//"):
                continue
            if line.startswith(".."):
                base = line[2:].strip()
                continue
            k, _, v = line.partition(":")
            fields[k.strip()] = v.strip()
        raw[name] = (dr, fields, base)
    cfgs = {}

    def resolve(name):
        if name in cfgs:
            return cfgs[name]
        dr, fields, base = raw[name]
        out = dict(resolve(base)) if base else {}
        for k, v in fields.items():
            out[k] = value(v, consts)
        if dr:
            out["cap"], out["time"] = consts["DEFAULT_CAP"], consts["DEFAULT_TIME"]
        cfgs[name] = out
        return out
    for name in raw:
        resolve(name)
    for name, c in cfgs.items():
        missing = [f for f in FIELDS if f not in c]
        if missing:
            problems.append(f"Config {name} lacks {missing}")
    # Tests: which configuration each required test runs, and how.
    by_cfg = {}
    for cfgname, (test, explorer) in REQUIRED.items():
        m = re.search(r"fn " + test + r"\(\) \{(.*?)\n\}", src, re.S)
        if not m:
            problems.append(f"test {test} not found")
            continue
        body = norm(m.group(1))
        ignored = bool(re.search(r"#\[ignore[^\]]*\]\s*fn " + test + r"\(", src))
        mm = re.fullmatch(r"(default_exploration|full_exploration|symmetric_full_exploration)\(&(\w+)\);", body) or \
            re.fullmatch(r"let \(report, _, _\) = explore_symmetric\(&(\w+), step_adm, Mode::Quotient\);.*", body)
        if mm and mm.lastindex == 2:
            runner, const = mm.group(1), mm.group(2)
            c = dict(cfgs[const])
        elif mm:
            runner, const = "explore_symmetric", mm.group(1)
            c = dict(cfgs[const])
        else:
            mm = re.fullmatch(r"full_exploration\(&Config \{ cap: ([\d_]+), \.\.(\w+) \}\);", body)
            if not mm:
                problems.append(f"test {test}: body not recognised: {body[:120]}")
                continue
            runner, const = "full_exploration", mm.group(2)
            c = dict(cfgs[const])
            c["cap"] = int(mm.group(1).replace("_", ""))
        if c["name"] != cfgname:
            problems.append(f"test {test} runs {c['name']}, expected {cfgname}")
        want_runner = {"original": ("default_exploration", "full_exploration"),
                       "composed": ("symmetric_full_exploration", "explore_symmetric")}[explorer]
        if runner not in want_runner:
            problems.append(f"test {test}: runner {runner} is not the {explorer} explorer")
        by_cfg[cfgname] = dict(config=c, const=const, test=P + test, runner=runner, explorer=explorer,
                               ignored=ignored, bounds=bounds_line(c, consts), actors=actors_line(c),
                               properties=property_lines(c), na=na_operations(c))
    return dict(consts=consts, configs=cfgs, required=by_cfg, problems=problems)


def b(x):
    return "true" if x else "false"


def bounds_line(c, k):
    return (f"facts {c['facts']} conflicting [{b(c['conflict'][0])}, {b(c['conflict'][1])}] ENV {b(c['env'])} batch "
            f"{b(c['batch'])} faults {c['faults']} admissions {c['admissions']} gating callers {c['gating']} gating "
            f"conflict {b(c['gating_conflict'])} acknowledgement actors {c['acks']} attempts each {c['ack_attempts']} "
            f"malformed snapshots {b(c['malformed'])} cell constraint {b(c['cell_constraint'])} worker verbs unwind "
            f"{b(c['worker_unwinds'])} DROP {b(c['drop'])} REPLAY {c['replays']} A4 {b(c['blanket_uncertain'])}; ticket "
            f"limit {k['TICKET_LIMIT']}, refusal limit {k['REFUSAL_LIMIT']}; settlement and the worker on; caps "
            f"{c['cap']} states, {c['time']}s")


def actors_line(c):
    writers = 1 if c["batch"] else c["facts"]
    a = [f"Fw({i})" for i in range(writers)] + ["Se", "Wk"] + [f"Gc({i})" for i in range(c["gating"])] + \
        [f"Ack({i})" for i in range(c["acks"])] + ["Adm"] + (["Env"] if c["env"] else []) + \
        (["Replay"] if c["replays"] > 0 else [])
    return "[" + ", ".join(a) + "]"


def property_lines(c):
    """{property: expected form} for a PASS: 'HOLDS', 'L1-HOLDS' (the HOLDS: … form), or an exact text."""
    out = {}
    for p in PROPERTIES:
        if p == "L1":
            out[p] = "L1-HOLDS" if c["l1"] else "not claimed for this configuration"
        elif p == "E10":
            out[p] = "HOLDS" if c["worker_unwinds"] else "not applicable here (worker verbs do not unwind)"
        else:
            out[p] = "HOLDS"
    return out


PROPERTIES = ["P1", "P2", "P4", "P5", "P6", "P7", "P8", "P9", "P12", "P13", "P15", "D1", "L1", "E1", "E2", "E3", "E4",
              "E5", "E6", "E7", "E8", "E9", "E10", "E11", "KEY"]


def na_operations(c):
    """{operation: reason} for every operation absent by configuration (source-anchored rules); others applicable."""
    na = {}
    if c["facts"] == 0:
        na["append_facts"] = "facts = 0: no FW actor (actors())"
    if c["faults"] == 0:
        na["expire"] = ("faults = 0: expiry is the fault option of expires(); Oracle::choose offers a fault option only "
                        "while faults > 0")
    if not c["drop"]:
        na["DROP"] = ("DROP false: step_fw offers the DROP option only when config.drop and the held ticket is "
                      "Counted")
    if c["gating"] == 0:
        na["append_gating"] = "gating callers = 0: no Gc actor (actors(); initial gc vector empty)"
    if c["acks"] == 0 or c["ack_attempts"] == 0:
        why = ("acknowledgement actors = 0: no Ack actor (actors())" if c["acks"] == 0 else
               "attempts each = 0: AckPos::Idle returns false with no attempt left")
        na["snapshot"] = why
        na["append_acknowledgement"] = why
        na["malformed snapshot"] = why
    elif not c["malformed"]:
        na["malformed snapshot"] = ("malformed snapshots false: the Owner's choice offers only confirm and decline "
                                    "(options = 2)")
    if c["admissions"] == 0:
        na["admission"] = "admissions = 0: step_adm returns false while w.adm == 0 (initial adm = config.admissions)"
    return na


def main():
    d = derive()
    if PROPERTIES != re.findall(r'"(\w+)"', re.search(r"const PROPERTIES: \[&str; 25\] = \[(.*?)\];",
                                                          common.source(common.MODEL), re.S).group(1)):
        d["problems"].append("PROPERTIES differ from the source")
    for k, r in d["required"].items():
        print(f"{k:5} {r['test']}  runner {r['runner']} ({r['explorer']}) ignored {r['ignored']}")
        print(f"      bounds: {r['bounds']}")
        print(f"      actors: {r['actors']}")
        pl = r["properties"]
        print(f"      properties: {sum(v in ('HOLDS', 'L1-HOLDS') for v in pl.values())} HOLD; "
              f"L1 {pl['L1']}; E10 {pl['E10']}")
        print(f"      NOT APPLICABLE operations: {r['na'] if r['na'] else 'none'}")
    for p in d["problems"]:
        print("PROBLEM:", p)
    print(f"CONFIGS: {'OK' if not d['problems'] else 'FAILED'} ({len(d['required'])} required configurations)")
    return 0 if not d["problems"] else 1


if __name__ == "__main__":
    sys.exit(main())
