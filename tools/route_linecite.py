#!/usr/bin/python3 -I
"""SB-3E GB4b-B line-cite pre-check (SB3E-R248, R255.8 P4, R257 F5 and F8, R274).

  route_linecite.py [--repo R] [--sha SHA] [--anchors FILE]    check; one JSON verdict; exit 0 only with no finding
  route_linecite.py --write [--repo R]                         regenerate tools/route_linecite.json from the tree
  route_linecite.py --selftest [--repo R]                      the controls over git history, and two plants

A plant expect in tools/gatebox/route/route_plants.json may cite a panic location, `path:line[:col]`. A line shift
leaves such an expect stale, and the plant then reads as alive only after hours of route_exit (R248). For every such
citation this check
  1. applies that plant's own mutation to an in-memory copy of the cited file (route_plants.apply_run's rules; the
     files are read from the working tree, or from `--sha`'s git objects; nothing is ever written to the tree), then
  2. compares the cited line, from the cited column to the end of the line (no column: from its first non-blank
     character), with the stored content anchor (F5: that the line holds some panic site is not enough).
It fails closed. A finding is: a cited path that is not exactly one tracked file (`unresolved`), a mutation that
cannot be applied (`mutation`), a line or column that the planted file does not have (`absent`), a citation without
an anchor (`missing_anchor`), an anchor that differs (`stale`), and an anchor that no citation uses (`extra_anchor`).

tools/route_linecite.json holds the anchors: cited file -> plant name -> citation -> anchor. `--write` takes them from
the current tree, so its diff is reviewed data: a citation that only moved keeps its anchor text.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

PLANTS = "tools/gatebox/route/route_plants.json"
ANCHORS = "tools/route_linecite.json"
CITE = re.compile(r"([\w./-]+\.[A-Za-z]\w*):(\d+)(?::(\d+))?")
ABOUT = ["SB-3E GB4b-B line-cite anchors (tools/route_linecite.py --write; SB3E-R248, R257 F5): cited file -> plant ->",
         "citation -> the text from the cited column to the end of the cited line, with the plant's own mutation applied."]


class Source:
    """The checked files: a commit's git objects (`sha`) or the working tree. Read only."""

    def __init__(self, repo, sha=None):
        self.repo, self.sha, self.cache = repo, sha, {}
        self.tracked = (self.git("ls-tree", "-r", "--name-only", sha) if sha else self.git("ls-files")).splitlines()

    def git(self, *args):
        run = subprocess.run(["git", "--no-replace-objects", "-C", self.repo, *args], capture_output=True)
        if run.returncode != 0:
            raise SystemExit("route_linecite: git " + " ".join(args) + ": " + run.stderr.decode(errors="replace")[-200:])
        return run.stdout.decode("utf-8")

    def text(self, path):
        if path not in self.cache:
            if self.sha:
                self.cache[path] = self.git("show", f"{self.sha}:{path}")
            else:
                with open(os.path.join(self.repo, path), encoding="utf-8", newline="") as handle:
                    self.cache[path] = handle.read()
        return self.cache[path]

    def plants(self):
        return json.loads(self.text(PLANTS))["plants"]


class Memory:
    """A source held in memory (the selftest's own small cases)."""

    def __init__(self, files, plants):
        self.tracked, self.text, self.plants = sorted(files), files.__getitem__, lambda: plants


def planted(src, plant, path):
    """(`path` as this plant's own run sees it, None), or (None, why it cannot be read). route_plants.apply_run's rules."""
    if plant["file"] != path:
        return src.text(path), None
    if "symlink" in plant:
        return None, "the cited file is the plant's own symlink"
    if "create" in plant:
        return plant["create"], None
    text = src.text(path)
    if "append" in plant:
        return text + plant["append"], None
    if text.count(plant["old"]) != 1:
        return None, "old text not found exactly once"
    return text.replace(plant["old"], plant["new"]), None


def observe(src):
    """Every citation once, in the file's order: its cited file, plant, citation and the text there (`at`), or the
    finding `kind` that says why there is none."""
    rows = {}
    for plant in src.plants():
        files = src.tracked + ([plant["file"]] if "create" in plant or "symlink" in plant else [])
        for expect in plant.get("expect", []):
            for cite in CITE.finditer(expect):
                found = sorted({p for p in files if p == cite[1] or p.endswith("/" + cite[1])})
                row = {"path": found[0] if len(found) == 1 else cite[1], "plant": plant["name"], "cite": cite[0], "at": None}
                rows.setdefault((row["plant"], row["cite"]), row)
                if len(found) != 1:
                    row.update(kind="unresolved", detail=f"{len(found)} tracked files end with the cited path")
                    continue
                text, why = planted(src, plant, found[0])
                if text is None:
                    row.update(kind="mutation", detail=why)
                    continue
                lines, number = text.split("\n"), int(cite[2])
                line = lines[number - 1] if 0 < number <= len(lines) else ""
                column = int(cite[3]) if cite[3] else len(line) - len(line.lstrip()) + 1
                if 0 < column <= len(line):
                    row["at"] = line[column - 1:]
                else:
                    row.update(kind="absent", detail="the planted file has no such line and column")
    return list(rows.values())


def check(src, anchors):
    """(rows, findings): every citation against its anchor, and every anchor against the citations."""
    rows, findings = observe(src), []
    for row in rows:
        want = anchors.get(row["path"], {}).get(row["plant"], {}).get(row["cite"])
        if row["at"] is None:
            findings.append({k: row[k] for k in ("kind", "plant", "cite", "path", "detail")})
        elif want is None:
            findings.append({"kind": "missing_anchor", "plant": row["plant"], "cite": row["cite"], "path": row["path"]})
        elif row["at"] != want:
            findings.append({"kind": "stale", "plant": row["plant"], "cite": row["cite"], "path": row["path"],
                             "anchor": want, "found": row["at"][:200]})
    cited = {(r["path"], r["plant"], r["cite"]) for r in rows}
    for path, plants in anchors.items():
        for plant, cites in plants.items():
            findings += [{"kind": "extra_anchor", "plant": plant, "cite": cite, "path": path}
                         for cite in cites if (path, plant, cite) not in cited]
    return rows, findings


def nested(rows):
    out = {}
    for row in rows:
        out.setdefault(row["path"], {}).setdefault(row["plant"], {})[row["cite"]] = row["at"]
    return out


def verdict(rows, findings):
    return {"plants": len({r["plant"] for r in rows}), "citations": len(rows), "findings": findings,
            "verdict": "fail" if findings else "pass"}


# selftest (mission GB4b-B item 3): real history. BASE's expects were correct; U20c moved s5.rs up by one line in STALE
# (R248, six stale expects); FIXED changed those six citations by -1.
BASE = "652af6e663cbb188d5a482a428573fd7cf9382b8"
BASE_ANCHORS_SHA256 = "1b402891da0fa077060e052489908b0f1b0f00d4b8ff6851cf86203b2a997550"
STALE = ("77e8ae74254873b2fbec066668629daf2b141058", "36d43f3fe31c1bba4dec1e6bcceee20d6b35ec27")
FIXED = ("5bdffa94b96ab441767192b4c77f7c1f26ffac5c", "3ec9759ab81c254d6a9eff79af4ab7428cb47330")
R248 = sorted(["l_s4b_poisons_nothing", "l_s4b_applied_after_the_lock_is_taken", "l_s5_wrong_invocation",
               "l_s5_session_from_no_context", "l_s5_admit_widened_to_charged", "l_s5_recording_lane_records_nothing"])
RS = "fn a() {\n    one();\n    assert!(two);\n}\n"
# Two plants of this script itself, applied in memory. Each must fail these controls with exactly the flagged plants
# listed: token-only misses the two 1085:13 expects (F5); without the mutation first, l_s4_panic_not_caught is flagged
# on a correct tree and the stale l_s5_recording_lane_records_nothing is missed (R249).
TOKEN = r"(?:debug_)?assert(?:_eq|_ne)?!|panic!|unreachable!|unimplemented!|todo!|\.?unwrap\(|\.?expect\("
SELF_PLANTS = (
    ("token_only", 'elif row["at"] ' + "!= want:", f'elif not re.match(r"""{TOKEN}""", row["at"]):',
     {"stale_77e8ae74": R248[2:], "stale_36d43f3f": R248[2:], "memory_other_assert": []}),
    ("no_mutation_first", 'if plant["file"] ' + "!= path:", "if True:",
     {"fixed_5bdffa94": ["l_s4_panic_not_caught", "l_s5_recording_lane_records_nothing"],
      "fixed_3ec9759a": ["l_s4_panic_not_caught", "l_s5_recording_lane_records_nothing"],
      "stale_77e8ae74": sorted(["l_s4_panic_not_caught"] + R248[:3] + R248[4:]),
      "memory_mutation_first": ["m"], "memory_removed": [], "memory_old_absent": []}),
)


def carried(base, base_rows, rows):
    """BASE's anchor texts under another commit's citations, per plant in citation order: what a reviewed
    regeneration keeps when a citation only moved."""
    texts, out = {}, {}
    for row in base_rows:
        texts.setdefault(row["plant"], []).append(base[row["path"]][row["plant"]][row["cite"]])
    for row in rows:
        if texts.get(row["plant"]):
            out.setdefault(row["path"], {}).setdefault(row["plant"], {})[row["cite"]] = texts[row["plant"]].pop(0)
    return out


def controls(repo, base):
    """The controls, each {control, flagged plants, finding kinds, ok}; `base` is BASE's anchors."""
    out = []

    def control(name, findings, flagged, kinds):
        got = {"control": name, "flagged": sorted({f["plant"] for f in findings}), "kinds": sorted({f["kind"] for f in findings})}
        out.append(dict(got, ok=got["flagged"] == sorted(flagged) and got["kinds"] == sorted(kinds)))

    base_rows = observe(Source(repo, BASE))
    for sha in STALE:
        control("stale_" + sha[:8], check(Source(repo, sha), base)[1], R248, ["absent", "stale"])
    for sha in FIXED:
        src = Source(repo, sha)
        control("fixed_" + sha[:8], check(src, carried(base, base_rows, observe(src)))[1], [], [])
        control("unregenerated_" + sha[:8], check(src, base)[1], R248, ["extra_anchor", "missing_anchor"])

    def memory(name, plant, cite, anchors, flagged, kinds, files=None):
        plant = dict({"name": "m", "file": "a/x.rs", "expect": [f"panicked at {cite}"]}, **plant)
        control("memory_" + name, check(Memory(files or {"a/x.rs": RS}, [plant]), anchors)[1], flagged, kinds)

    anchor = {"a/x.rs": {"m": {"x.rs:3:5": "assert!(two);"}}}
    same = {"old": "one", "new": "one"}
    memory("positive", same, "x.rs:3:5", anchor, [], [])
    memory("no_column", same, "x.rs:3", {"a/x.rs": {"m": {"x.rs:3": "assert!(two);"}}}, [], [])
    memory("other_assert", {"old": "two", "new": "six"}, "x.rs:3:5", anchor, ["m"], ["stale"])
    memory("mutation_first", {"old": "    one();\n", "new": ""}, "x.rs:2:5", {"a/x.rs": {"m": {"x.rs:2:5": "assert!(two);"}}}, [], [])
    memory("removed", {"old": "    assert!(two);\n}\n", "new": ""}, "x.rs:3:5", anchor, ["m"], ["absent"])
    memory("old_absent", {"old": "absent", "new": ""}, "x.rs:3:5", anchor, ["m"], ["mutation"])
    memory("unresolved", same, "x.rs:3:5", {"x.rs": {"m": {"x.rs:3:5": "assert!(two);"}}}, ["m"], ["unresolved"],
           {"a/x.rs": RS, "b/x.rs": RS})
    memory("missing_anchor", same, "x.rs:3:5", {}, ["m"], ["missing_anchor"])
    return out


def selftest(repo):
    base_src = Source(repo, BASE)
    rows, findings = check(base_src, nested(observe(base_src)))
    base = nested(rows)
    digest = hashlib.sha256(json.dumps(base, sort_keys=True).encode()).hexdigest()
    checks = [{"control": "base_anchors", "ok": not findings and digest == BASE_ANCHORS_SHA256, "sha256": digest,
               "plants": len({r["plant"] for r in rows}), "citations": len(rows)}] + controls(repo, base)
    with open(os.path.abspath(__file__), encoding="utf-8") as handle:
        source = handle.read()
    plants = []
    for name, old, new, red in SELF_PLANTS:
        scope = {"__name__": "route_linecite_plant", "__file__": os.path.abspath(__file__)}
        if source.count(old) == 1:
            exec(compile(source.replace(old, new), f"route_linecite.py[{name}]", "exec"), scope)
        failed = {c["control"]: c["flagged"] for c in scope["controls"](repo, base) if not c["ok"]} if scope.get("controls") else None
        plants.append({"plant": name, "red": failed, "ok": bool(failed) and all(failed.get(k) == v for k, v in red.items())})
    failed = [c.get("control") or c["plant"] for c in checks + plants if not c["ok"]]
    print(json.dumps({"controls": len(checks), "plants": plants, "failed": failed, "checks": checks,
                      "verdict": "fail" if failed else "pass"}, sort_keys=True))
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    parser.add_argument("--sha")
    parser.add_argument("--anchors")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--selftest", action="store_true")
    args = parser.parse_args()
    repo = os.path.abspath(args.repo)
    if args.selftest:
        return selftest(repo)
    if args.write and (args.sha or args.anchors):
        sys.exit("route_linecite: --write takes the anchors from the working tree only")
    src = Source(repo, args.sha)
    if args.write:
        rows = observe(src)
        unread = [{k: r[k] for k in ("kind", "plant", "cite", "path", "detail")} for r in rows if r["at"] is None]
        if not unread:
            with open(os.path.join(repo, ANCHORS), "w", encoding="utf-8") as handle:
                json.dump({"about": ABOUT, "anchors": nested(rows)}, handle, indent=1, sort_keys=True)
                handle.write("\n")
        print(json.dumps(verdict(rows, unread), sort_keys=True))
        return 1 if unread else 0
    if args.anchors:
        with open(args.anchors, encoding="utf-8") as handle:
            anchors = json.load(handle)["anchors"]
    else:
        anchors = json.loads(src.text(ANCHORS))["anchors"]
    rows, findings = check(src, anchors)
    print(json.dumps(verdict(rows, findings), sort_keys=True))
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
