"""SB-3E A7 evidence schema and graph reconciliation (ruling SB3E-R14 item 1).

Closes review-u3-batch findings 2 and 3 (and review-u3a-1 finding 5) at
their shared root: A7 compared the two builds with each other and checked
records by shape, so an omission made in both builds, or a reference no
recorded unit produced, passed. Here each build is checked against an
absolute model before any cross-build comparison:

- schema: every evidence file and field present with its type (`evidence_schema`);
- Cargo's own unit graph for the exact recipe (`cargo ... --unit-graph`,
  recorded by the runner), its packages and edges reconciled against
  `cargo metadata` (`graph_not_in_metadata`) and its roots fixed (`graph_roots`);
- every recorded compile and build-script run matched to exactly one graph
  unit and every graph unit to exactly one record (`unit_not_in_graph`,
  `unit_missing_from_build`), features and `--cfg` values bound to the unit
  and its own build script's output (a stray one leaves the record unmatched);
- every `--extern` bound to the compile that produced it, of the graph's
  dependency, with an inventory hash (`extern_unbound`); every `-L` and `-l`
  declared by a build-script run the unit reaches, path resolved and free of
  traversal (`path_unowned`); output directories fixed (`out_dir_unowned`);
- every unit's artifacts hashed (`artifact_missing`); every dep-info file's
  structure and content validated (`dep_info_malformed`);
- every file in the target owned by a recorded unit or run, or Cargo's own
  bookkeeping (`unowned_file`).
"""
import json
import os
import re
import shutil
import subprocess

HASH = r"[0-9a-f]{16}"
META = {"kind": str, "repo": str, "target": str, "cargo_home": str, "toolchain": str, "wrapper": (str, type(None)),
        "command": list, "status": int, "seconds": (int, float), "changed_since_start": list, "launch_block": list,
        "rustc": str, "cargo": str, "sysroot": str, "repo_hash_before": str, "repo_hash_after": str, "pre_build": list}
INVENTORY = {"files": dict, "symlinks": list, "dep_info": dict, "outputs": dict}
RUN_FILES = ("invoked.timestamp", "output", "root-output", "stderr")
ROOTS = {"ship": ("nexus-mother-desktop", "nexus-mother", "build"), "lane": ("nexus-mother-host", "route_cells", "test")}


def d(kind, **detail):
    return {"kind": kind, **detail}


def read(path):
    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
        return handle.read()


def record(command, repo, block, evidence, target, host):
    """Before the build: Cargo's unit graph for the exact recipe, and `cargo metadata`.

    `--unit-graph` is unstable, so that one query (never a build) runs with
    RUSTC_BOOTSTRAP=1, in a scratch target. A failed query leaves its file
    absent, which fails the schema.
    """
    scratch = target + ".graph"
    for name, argv, env in (
            ("unit_graph.json", command + ["-Z", "unstable-options", "--unit-graph"],
             dict(block, CARGO_TARGET_DIR=scratch, RUSTC_BOOTSTRAP="1")),
            ("metadata.json", [command[0], "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", host], block)):
        run = subprocess.run(argv, cwd=repo, env=env, capture_output=True)
        if run.returncode == 0:
            with open(os.path.join(evidence, name), "wb") as handle:
                handle.write(run.stdout)
    shutil.rmtree(scratch, ignore_errors=True)
    if os.path.exists(scratch + ".route_rustc.log"):
        os.unlink(scratch + ".route_rustc.log")


def schema(evidence, kind):
    """Every file and field the comparison relies on, present with its type."""
    out = []
    names = ["meta.json", "inventory.json", "build.log", "unit_graph.json", "metadata.json"] + (["wrapper.log"] if kind == "lane" else [])
    for name in names:
        if not os.path.isfile(os.path.join(evidence, name)):
            out.append(d("evidence_schema", build=kind, missing=name))
    if out:
        return out
    try:
        meta, inventory = (json.loads(read(os.path.join(evidence, n))) for n in ("meta.json", "inventory.json"))
        graph, metadata = (json.loads(read(os.path.join(evidence, n))) for n in ("unit_graph.json", "metadata.json"))
    except ValueError as error:
        return [d("evidence_schema", build=kind, detail=repr(error))]
    for record, fields, name in ((meta, META, "meta"), (inventory, INVENTORY, "inventory")):
        if not isinstance(record, dict) or set(record) != set(fields):
            out.append(d("evidence_schema", build=kind, record=name, keys=sorted(record) if isinstance(record, dict) else None))
            continue
        out += [d("evidence_schema", build=kind, field=f"{name}.{k}") for k, t in fields.items() if not isinstance(record[k], t)]
    if not out:
        for field in ("repo_hash_before", "repo_hash_after"):
            if not re.fullmatch(r"[0-9a-f]{64}", meta[field]):
                out.append(d("evidence_schema", build=kind, field=field))
        if meta["kind"] != kind:
            out.append(d("evidence_schema", build=kind, field="kind"))
    if not (isinstance(graph, dict) and isinstance(graph.get("units"), list) and isinstance(graph.get("roots"), list)
            and isinstance(metadata, dict) and isinstance(metadata.get("packages"), list) and isinstance(metadata.get("resolve"), dict)):
        out.append(d("evidence_schema", build=kind, record="unit_graph/metadata"))
    return out


def lines_of(text, directive):
    """Values of `cargo:KEY=` / `cargo::KEY=` lines of a build-script output."""
    return [m.group(1) for m in re.finditer(rf"^cargo::?{directive}=(.*)$", text or "", re.M)]


class Reconcile:
    def __init__(self, units, kind):
        self.u, self.kind, self.out = units, kind, []
        self.graph = json.loads(read(os.path.join(units.evidence, "unit_graph.json")))["units"]
        self.metadata = json.loads(read(os.path.join(units.evidence, "metadata.json")))
        self.target = units.target
        self.of_compile, self.of_run, self.produced, self.dep_infos, self.bound = {}, {}, {}, set(), []

    def fail(self, kind, **detail):
        self.out.append(d(kind, build=self.kind, **detail))

    def run(self):
        self.against_metadata()
        scripts = [c for c in self.u.compiles if c["key"][0] == "build_script_build"]
        self.match_compiles(scripts)
        self.match_runs()
        self.match_compiles([c for c in self.u.compiles if c["key"][0] != "build_script_build"])
        for i, unit in enumerate(self.graph):
            if i not in self.of_compile.values() and i not in self.of_run.values():
                self.fail("unit_missing_from_build", unit=[unit["pkg_id"], unit["target"]["name"], unit["mode"]])
        for c in self.u.compiles:
            if id(c) in self.of_compile:
                self.bind(c, self.of_compile[id(c)])
        self.externs()
        self.owned()
        return self.out

    def against_metadata(self):
        packages = {p["id"]: p for p in self.metadata["packages"]}
        deps = {n["id"]: {x["pkg"] for x in n["deps"]} for n in self.metadata["resolve"]["nodes"]}
        for unit in self.graph:
            if unit["pkg_id"] not in packages:
                self.fail("graph_not_in_metadata", pkg=unit["pkg_id"])
                continue
            for dep in unit["dependencies"]:
                other = self.graph[dep["index"]]["pkg_id"]
                if other != unit["pkg_id"] and other not in deps.get(unit["pkg_id"], ()):
                    self.fail("graph_not_in_metadata", pkg=unit["pkg_id"], dep=other)
        roots = json.loads(read(os.path.join(self.u.evidence, "unit_graph.json")))["roots"]
        got = [(packages.get(self.graph[r]["pkg_id"], {}).get("name"), self.graph[r]["target"]["name"], self.graph[r]["mode"]) for r in roots]
        if got != [ROOTS[self.kind]]:
            self.fail("graph_roots", roots=got)

    def own_run(self, index):
        runs = [x["index"] for x in self.graph[index]["dependencies"] if self.graph[x["index"]]["mode"] == "run-custom-build"
                and self.graph[x["index"]]["pkg_id"] == self.graph[index]["pkg_id"]]
        return runs[0] if len(runs) == 1 else None

    def output_of_run(self, index):
        record = next((r for r in self.u.runs if self.of_run.get(id(r)) == index), None)
        return None if record is None else self.u.inventory["outputs"].get(record["build_dir"] + "/output")

    def match_compiles(self, compiles):
        taken = set(self.of_compile.values())
        for c in compiles:
            pairs = c["pairs"]
            source = os.path.normpath(os.path.join(self.u.meta["repo"], next(v for k, v in pairs if k == "source")))
            mode = "test" if ("flag", "--test") in pairs else "build"
            opt = next((v.split("=", 1)[1] for k, v in pairs if k == "C" and v.startswith("opt-level=")), "0")
            cfgs = {v for k, v in pairs if k == "cfg"}
            found = None
            for i, unit in enumerate(self.graph):
                t = unit["target"]
                if (i in taken or unit["mode"] != mode or t["src_path"] != source or t["name"].replace("-", "_") != c["key"][0]
                        or unit["profile"]["opt_level"] != opt):
                    continue
                own = {f'feature="{f}"' for f in unit["features"]}
                run = self.own_run(i)
                emitted = set(lines_of(self.output_of_run(run), "rustc-cfg")) if run is not None else set()
                if own <= cfgs and cfgs - own <= emitted:
                    found = i
                    break
            if found is None:
                self.fail("unit_not_in_graph", unit=list(c["key"]), mode=mode, opt=opt)
            else:
                taken.add(found)
                self.of_compile[id(c)] = found

    def match_runs(self):
        taken = set()
        script_of = {c["extra"]: self.of_compile.get(id(c)) for c in self.u.compiles if c["key"][0] == "build_script_build"}
        for r in self.u.runs:
            script = script_of.get(r["exe_hash"])
            feats = {k[len("CARGO_FEATURE_"):] for k in r["env"] if k.startswith("CARGO_FEATURE_")}
            found = None
            for i, unit in enumerate(self.graph):
                if (i in taken or unit["mode"] != "run-custom-build" or script is None
                        or script not in [x["index"] for x in unit["dependencies"]]):
                    continue
                if feats == {f.upper().replace("-", "_") for f in unit["features"]} and r["env"].get("OPT_LEVEL") == unit["profile"]["opt_level"]:
                    found = i
                    break
            if found is None:
                self.fail("unit_not_in_graph", run=r["exe"])
            else:
                taken.add(found)
                self.of_run[id(r)] = found

    def reachable_runs(self, index, seen=None):
        seen = set() if seen is None else seen
        for dep in self.graph[index]["dependencies"]:
            if dep["index"] not in seen:
                seen.add(dep["index"])
                self.reachable_runs(dep["index"], seen)
        return [i for i in seen if self.graph[i]["mode"] == "run-custom-build"]

    def artifacts(self, c, index):
        unit, extra, name = self.graph[index], c["extra"], c["key"][0]
        kinds = unit["target"]["kind"]
        if "custom-build" in kinds:
            build = os.path.join(self.target, "release", "build", f"{self.package_name(unit['pkg_id'])}-{extra}")
            return build, [f"{build}/build_script_build-{extra}"]
        deps = os.path.join(self.target, "release", "deps")
        if "proc-macro" in kinds:
            return deps, [f"{deps}/lib{name}-{extra}.so"]
        if unit["mode"] == "test" or set(kinds) & {"bin", "test", "example"}:
            return deps, [f"{deps}/{name}-{extra}"]
        return deps, [f"{deps}/lib{name}-{extra}.rlib", f"{deps}/lib{name}-{extra}.rmeta"]

    def package_name(self, pkg_id):
        return next((p["name"] for p in self.metadata["packages"] if p["id"] == pkg_id), None)

    def bind(self, c, index):
        """Artifacts, dep-info, externs, link paths and out-dir of one matched compile."""
        files, unit, where = self.u.inventory["files"], self.graph[index], list(c["key"])
        out_dir, arts = self.artifacts(c, index)
        for path in arts:
            self.produced[path] = index
            if os.path.relpath(path, self.target) not in files:
                self.fail("artifact_missing", unit=where, file=os.path.relpath(path, self.target))
        if [v for k, v in c["pairs"] if k == "out-dir"] != [out_dir]:
            self.fail("out_dir_unowned", unit=where)
        self.dep_info(c, where, out_dir, arts)
        reach = self.reachable_runs(index)
        outputs = "\n".join(self.output_of_run(r) or "" for r in reach)
        searches, libs = set(lines_of(outputs, "rustc-link-search")), set(lines_of(outputs, "rustc-link-lib"))
        for k, v in c["pairs"]:
            if k == "L":
                kind_, eq, path = v.partition("=")
                path = path if eq else v
                clean = os.path.isabs(path) and os.path.normpath(path) == path and ".." not in path.split("/")
                if not clean or (v != f"dependency={self.target}/release/deps" and v not in searches and path not in searches):
                    self.fail("path_unowned", unit=where, option="L", value=v)
            elif k == "l" and v not in libs:
                self.fail("path_unowned", unit=where, option="l", value=v)
        self.bound.append((c, index))

    def externs(self):
        files = self.u.inventory["files"]
        for c, index in self.bound:
            unit, where = self.graph[index], list(c["key"])
            want = {x["extern_crate_name"]: x["index"] for x in unit["dependencies"]
                    if self.graph[x["index"]]["mode"] != "run-custom-build"
                    and set(self.graph[x["index"]]["target"]["kind"]) & {"lib", "rlib", "proc-macro"}}
            got = {}
            for k, v in c["pairs"]:
                if k != "extern":
                    continue
                name, eq, path = v.partition("=")
                if not eq and name == "proc_macro" and "proc-macro" in unit["target"]["kind"]:
                    continue
                ok = path in self.produced and os.path.relpath(path, self.target) in files
                got[name] = self.produced[path] if ok else None
            if got != want:
                self.fail("extern_unbound", unit=where, names=sorted(n for n in set(got) | set(want) if got.get(n) != want.get(n)))

    def dep_info(self, c, where, out_dir, arts):
        stem = "build_script_build" if c["key"][0] == "build_script_build" else c["key"][0]
        path = f"{out_dir}/{stem}-{c['extra']}.d"
        text = self.u.inventory["dep_info"].get(os.path.relpath(path, self.target))
        self.dep_infos.add(os.path.relpath(path, self.target))
        source = next(v for k, v in c["pairs"] if k == "source")
        if text is None:
            return self.fail("missing_dep_info", unit=where)
        rules, phony, problems = {}, [], []
        for line in text.splitlines():
            if not line or line.startswith("# env-dep:"):
                continue
            if line.endswith(":") and ": " not in line:
                phony.append(line[:-1].replace("\\ ", " "))
            elif ": " in line:
                head, _, tail = line.partition(": ")
                rules[head] = [e.replace("\\ ", " ") for e in re.findall(r"(?:\\ |\S)+", tail)]
            else:
                problems.append(line[:80])
        prereqs = list(rules.values())
        if (problems or set(rules) != {path, *arts} or any(p != prereqs[0] for p in prereqs)
                or not prereqs[0] or prereqs[0][0] != source or sorted(phony) != sorted(set(prereqs[0])) or len(phony) != len(set(phony))):
            self.fail("dep_info_malformed", unit=where)

    def owned(self):
        """Every file in the target belongs to a recorded unit or run, or to Cargo's bookkeeping."""
        files = self.u.inventory["files"]
        mine = {os.path.relpath(p, self.target) for p in self.produced} | self.dep_infos
        script_dirs = {os.path.dirname(os.path.relpath(p, self.target)) for p, i in self.produced.items()
                       if "custom-build" in self.graph[i]["target"]["kind"]}
        run_dirs = {r["build_dir"] for r in self.u.runs if id(r) in self.of_run}
        bins = {os.path.basename(p).rsplit("-", 1)[0]: files.get(os.path.relpath(p, self.target)) for p, i in self.produced.items()
                if set(self.graph[i]["target"]["kind"]) & {"bin"}}
        for rel, digest in files.items():
            parts = rel.split("/")
            ok = (rel in mine or rel in (".rustc_info.json", "CACHEDIR.TAG", "release/.cargo-lock")
                  or rel.startswith("release/.fingerprint/")
                  or (len(parts) == 4 and parts[1] == "build" and "/".join(parts[:3]) in script_dirs and parts[3] == "build-script-build")
                  or (len(parts) == 4 and "/".join(parts[:3]) in run_dirs and parts[3] in RUN_FILES)
                  or (len(parts) > 4 and "/".join(parts[:3]) in run_dirs and parts[3] == "out")
                  or (len(parts) == 2 and parts[1].replace("-", "_") in bins and bins[parts[1].replace("-", "_")] == digest)
                  or (len(parts) == 2 and parts[1].endswith(".d") and parts[1][:-2].replace("-", "_") in bins))
            if not ok:
                self.fail("unowned_file", file=rel)


def reconcile(units, kind):
    return Reconcile(units, kind).run()
