"""SB-3E A7 evidence schema and graph reconciliation (rulings SB3E-R14 item 1, SB3E-R15 item 1).

Closes review-u3-batch findings 2 and 3 (and review-u3a-1 finding 5) at
their shared root: A7 compared the two builds with each other and checked
records by shape, so an omission made in both builds, or a reference no
recorded unit produced, passed. Here each build is checked against an
absolute model before any cross-build comparison:

- schema: every evidence file, and every key path of `SCHEMA` (closed key
  sets, typed leaves, 64-hex digests, graph indices in range) (`evidence_schema`);
- Cargo's own unit graph for the exact recipe (`cargo ... --unit-graph`,
  recorded by the runner as the projection `SCHEMA` names), its packages and
  edges equal to `cargo metadata`'s projection (`graph_not_in_metadata`), its
  roots fixed (`graph_roots`), and every edge bound to the record it causes:
  `--extern`, its own build-script run, `CARGO_BIN_EXE_*`, `links` runs (`graph_edge_unbound`);
- every recorded compile and build-script run matched to exactly one graph
  unit and every graph unit to exactly one record (`unit_not_in_graph`,
  `unit_missing_from_build`), features and `--cfg` values bound to the unit
  and its own build script's output (a stray one leaves the record unmatched);
- every `--extern` bound to the compile that produced it, of the graph's
  dependency, with an inventory hash (`extern_unbound`); every `-L native=`
  the out directory of a run the unit reaches that declares it and holds
  inventoried files, every `-l` declared by such a run, every static one's
  archive inventoried in that directory (`path_unowned`); output
  directories fixed (`out_dir_unowned`);
- the inventory exactly the files Cargo's layout requires of the recorded
  units and runs (artifacts, dep-info, fingerprints, run files, uplifted
  binaries, `OUT_PINS` out-directory contents; cached-diagnostic files counted
  against the log's warning summaries), no more (`unowned_file`) and no less
  (`artifact_missing`, `inventory_incomplete`, `out_dir_contents`); every
  dep-info and output text the bytes its inventory digest names (`inventory_mismatch`);
  every dep-info file's structure validated (`dep_info_malformed`).
"""
import collections
import hashlib
import json
import os
import re
import subprocess
import sys

HASH = r"[0-9a-f]{16}"
TEXT, DIGEST, PATH, POSITIVE = "text", "digest", "path", "positive"
UNIT = {"pkg_id": str, "target": {"kind": [str], "name": str, "src_path": PATH}, "profile": {"opt_level": str},
        "mode": str, "features": [str], "dependencies": [{"index": int, "extern_crate_name": str}]}
SCHEMA = {
    "meta.json": {"kind": str, "repo": PATH, "target": PATH, "cargo_home": PATH, "toolchain": PATH, "wrapper": (PATH, None),
                  "command": [str], "status": int, "seconds": POSITIVE, "changed_since_start": [str], "launch_block": [str],
                  "rustc": str, "cargo": str, "sysroot": PATH, "repo_hash_before": DIGEST, "repo_hash_after": DIGEST,
                  "pre_build": [dict]},
    "inventory.json": {"files": {str: DIGEST}, "symlinks": [str], "dep_info": {str: TEXT}, "outputs": {str: TEXT}},
    "unit_graph.json": {"units": [UNIT], "roots": [int]},
    "metadata.json": {"sha256": DIGEST, "packages": [{"id": str, "name": str, "links": (str, None)}],
                      "resolve": {"nodes": [{"id": str, "deps": [{"pkg": str}]}]}},
}
FILES = {"ship": ("build.log",), "lane": ("build.log", "wrapper.log")}
LEAF = {str: lambda v: isinstance(v, str) and v != "", TEXT: lambda v: isinstance(v, str), int: lambda v: type(v) is int,
        DIGEST: lambda v: isinstance(v, str) and re.fullmatch(r"[0-9a-f]{64}", v) is not None, dict: lambda v: isinstance(v, dict),
        PATH: lambda v: isinstance(v, str) and os.path.isabs(v) and os.path.normpath(v) == v,
        POSITIVE: lambda v: type(v) in (int, float) and v > 0}
RUN_FILES = ("invoked.timestamp", "output", "root-output", "stderr")
TOP_FILES = (".rustc_info.json", "CACHEDIR.TAG", "release/.cargo-lock")
ROOTS = {"ship": ("nexus-mother-desktop", "nexus-mother", "build"), "lane": ("nexus-mother-host", "route_cells", "test")}
# Every build-script out directory's contents for the pinned Cargo.lock (cc's per-directory hash prefix as `*-`).
OUT_PINS = {k: sorted(v.split()) for k, v in {
    "html5ever": "rules.rs", "libsqlite3-sys": "*-sqlite3.o bindgen.rs libsqlite3.a", "markup5ever": "generated.rs named_entities.rs",
    "rustix": "rustix_test_can_compile", "rustversion": "version.expr", "serde": "private.rs", "serde_core": "private.rs",
    "target-lexicon": "host.rs", "thiserror": "private.rs", "typenum": "tests.rs", "web_atoms": "generated.rs named_entities.rs",
    "ring": " ".join(["libring_core_0_17_14_.a", "libring_core_0_17_14__test.a"] + [f"*-{n}.o" for n in (
        "aes-gcm-avx2-x86_64-elf aes_nohw aesni-gcm-x86_64-elf aesni-x86_64-elf chacha-x86_64-elf chacha20_poly1305_x86_64-elf "
        "constant_time_test cpu_intel crypto curve25519 curve25519_64_adx ecp_nistz fiat_curve25519_adx_mul fiat_curve25519_adx_square "
        "gfp_p256 gfp_p384 ghash-x86_64-elf limbs mem montgomery montgomery_inv p256-nistz p256-x86_64-asm-elf p256 poly1305 "
        "sha256-x86_64-elf sha512-x86_64-elf vpaes-x86_64-elf x86_64-mont-elf x86_64-mont5-elf").split()])}.items()}


def d(kind, **detail):
    return {"kind": kind, **detail}


def read(path):
    with open(path, encoding="utf-8", errors="surrogateescape") as handle:
        return handle.read()


def sha256_text(text):
    return hashlib.sha256(text.encode("utf-8", "surrogateescape")).hexdigest()


def record(command, repo, block, evidence, target, host):
    """Before the build: Cargo's unit graph for the exact recipe, and `cargo metadata`, as `SCHEMA` projects them.

    `--unit-graph` is unstable, so that one query (never a build) runs with
    RUSTC_BOOTSTRAP=1, in a scratch target. A failed query is a hard error.
    """
    scratch, out = target + ".graph", []
    for argv, env in ((command + ["-Z", "unstable-options", "--unit-graph"], dict(block, CARGO_TARGET_DIR=scratch, RUSTC_BOOTSTRAP="1")),
                      ([command[0], "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", host], block)):
        run = subprocess.run(argv, cwd=repo, env=env, capture_output=True)
        try:
            out.append((json.loads(run.stdout), run.stdout) if run.returncode == 0 else None)
        except ValueError:
            out.append(None)
        if out[-1] is None:
            sys.exit(f"route_a7: the graph query failed ({run.returncode}): {' '.join(argv[1:])}")
    for path in (scratch, scratch + ".route_rustc.log"):
        subprocess.run(["rm", "-rf", "--", path], check=True)
    (graph, _), (metadata, raw) = out
    units = [{"pkg_id": u["pkg_id"], "target": {k: u["target"][k] for k in ("kind", "name", "src_path")},
              "profile": {"opt_level": u["profile"]["opt_level"]}, "mode": u["mode"], "features": u["features"],
              "dependencies": [{k: x[k] for k in ("index", "extern_crate_name")} for x in u["dependencies"]]} for u in graph["units"]]
    edges = edges_of(units)
    with open(os.path.join(evidence, "unit_graph.json"), "w", encoding="utf-8") as handle:
        json.dump({"units": units, "roots": graph["roots"]}, handle, sort_keys=True)
    with open(os.path.join(evidence, "metadata.json"), "w", encoding="utf-8") as handle:
        json.dump({"sha256": hashlib.sha256(raw).hexdigest(),
                   "packages": [{"id": p["id"], "name": p["name"], "links": p.get("links")} for p in metadata["packages"] if p["id"] in edges],
                   "resolve": {"nodes": [{"id": n["id"], "deps": [{"pkg": x["pkg"]} for x in n["deps"] if x["pkg"] in edges[n["id"]]]}
                                         for n in metadata["resolve"]["nodes"] if n["id"] in edges]}}, handle, sort_keys=True)


def edges_of(units):
    """Each package of the graph -> the other packages its units depend on."""
    edges = {u["pkg_id"]: set() for u in units}
    for u in units:
        edges[u["pkg_id"]] |= {units[x["index"]]["pkg_id"] for x in u["dependencies"]} - {u["pkg_id"]}
    return edges


def invalid(node, value, path, out):
    """Appends the key path of every value its `SCHEMA` node does not admit."""
    if isinstance(node, tuple):
        if value is None and None in node:
            return
        node = node[0]
    if isinstance(node, dict) and list(node) == [str]:
        if not isinstance(value, dict):
            return out.append(path)
        for k, v in value.items():
            invalid(node[str], v, f"{path}.{k}", out)
    elif isinstance(node, dict):
        if not isinstance(value, dict) or set(value) != set(node):
            return out.append(path)
        for k in node:
            invalid(node[k], value[k], f"{path}.{k}", out)
    elif isinstance(node, list):
        if not isinstance(value, list):
            return out.append(path)
        for i, v in enumerate(value):
            invalid(node[0], v, f"{path}[{i}]", out)
    elif not LEAF[node](value):
        out.append(path)


def schema(evidence, kind):
    """Every evidence file, and every key path of `SCHEMA`, present with its type."""
    missing = [n for n in [*SCHEMA, *FILES[kind]] if not os.path.isfile(os.path.join(evidence, n))]
    if missing:
        return [d("evidence_schema", build=kind, missing=missing)]
    bad, data = [], {}
    for name, node in SCHEMA.items():
        try:
            data[name] = json.loads(read(os.path.join(evidence, name)))
        except ValueError:
            bad.append(name)
            continue
        invalid(node, data[name], name, bad)
    if not bad:
        units = data["unit_graph.json"]["units"]
        bad += [f"unit_graph.json.units[{i}]" for i, u in enumerate(units) if any(not 0 <= x["index"] < len(units) for x in u["dependencies"])]
        bad += ["unit_graph.json.roots"] * any(not 0 <= r < len(units) for r in data["unit_graph.json"]["roots"])
        bad += ["meta.json.kind"] * (data["meta.json"]["kind"] != kind)
    return [d("evidence_schema", build=kind, paths=bad[:20], count=len(bad))] if bad else []


def lines_of(text, directive):
    """Values of `cargo:KEY=` / `cargo::KEY=` lines of a build-script output."""
    return [m.group(1) for m in re.finditer(rf"^cargo::?{directive}=(.*)$", text or "", re.M)]


def fingerprint(unit):
    """The name Cargo gives a unit's fingerprint files."""
    kinds = unit["target"]["kind"]
    desc = next((v for k, v in (("custom-build", "build-script"), ("bin", "bin"), ("test", "integration-test"), ("example", "example"))
                 if k in kinds), "lib")
    flavor = {"test": "test-", "run-custom-build": "run-"}.get(unit["mode"], "")
    return f"{flavor}{desc}-{unit['target']['name']}"


class Reconcile:
    def __init__(self, units, kind):
        self.u, self.kind, self.out = units, kind, []
        graph = json.loads(read(os.path.join(units.evidence, "unit_graph.json")))
        self.graph, self.roots = graph["units"], graph["roots"]
        self.metadata = json.loads(read(os.path.join(units.evidence, "metadata.json")))
        self.target = units.target
        self.of_compile, self.of_run, self.produced, self.required, self.bound = {}, {}, {}, set(TOP_FILES), []

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
        self.runs = {self.of_run[id(r)]: r for r in self.u.runs if id(r) in self.of_run}
        for c in self.u.compiles:
            if id(c) in self.of_compile:
                self.bind(c, self.of_compile[id(c)])
        self.externs()
        self.edges()
        self.inventory()
        return self.out

    def against_metadata(self):
        packages = {p["id"]: p for p in self.metadata["packages"]}
        nodes = {n["id"]: [x["pkg"] for x in n["deps"]] for n in self.metadata["resolve"]["nodes"]}
        edges = edges_of(self.graph)
        if (len(packages) != len(self.metadata["packages"]) or set(packages) != set(edges)
                or len(nodes) != len(self.metadata["resolve"]["nodes"]) or set(nodes) != set(edges)):
            self.fail("graph_not_in_metadata", packages=sorted(set(packages) ^ set(edges) | set(nodes) ^ set(edges)))
        for pkg, deps in nodes.items():
            if len(deps) != len(set(deps)) or set(deps) != edges.get(pkg):
                self.fail("graph_not_in_metadata", pkg=pkg)
        got = [(packages.get(self.graph[r]["pkg_id"], {}).get("name"), self.graph[r]["target"]["name"], self.graph[r]["mode"]) for r in self.roots]
        if got != [ROOTS[self.kind]]:
            self.fail("graph_roots", roots=got)

    def deps(self, index, mode=None):
        return [x["index"] for x in self.graph[index]["dependencies"] if mode is None or self.graph[x["index"]]["mode"] == mode]

    def own_run(self, index):
        runs = [i for i in self.deps(index, "run-custom-build") if self.graph[i]["pkg_id"] == self.graph[index]["pkg_id"]]
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
                if i in taken or unit["mode"] != "run-custom-build" or script is None or script not in self.deps(i):
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
        for dep in self.deps(index):
            if dep not in seen:
                seen.add(dep)
                self.reachable_runs(dep, seen)
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

    def rel(self, path):
        return os.path.relpath(path, self.target)

    def bind(self, c, index):
        """Artifacts, dep-info, externs, link paths and out-dir of one matched compile."""
        files, where = self.u.inventory["files"], list(c["key"])
        out_dir, arts = self.artifacts(c, index)
        for path in arts:
            self.produced[path] = index
            if self.rel(path) not in files:
                self.fail("artifact_missing", unit=where, file=self.rel(path))
        if [v for k, v in c["pairs"] if k == "out-dir"] != [out_dir]:
            self.fail("out_dir_unowned", unit=where)
        self.dep_info(c, where, out_dir, arts)
        reach = self.reachable_runs(index)
        natives = set()
        for j in (j for j in reach if j in self.runs):
            out = f"{self.target}/{self.runs[j]['build_dir']}/out"
            if out in {s[len("native="):] if s.startswith("native=") else s for s in lines_of(self.output_of_run(j), "rustc-link-search")}:
                natives.add(out)
        libs = set(lines_of("\n".join(self.output_of_run(r) or "" for r in reach), "rustc-link-lib"))
        searched = []
        for k, v in c["pairs"]:
            if k == "L" and v != f"dependency={self.target}/release/deps":
                path = v[len("native="):] if v.startswith("native=") else None
                if path not in natives or not any(f.startswith(self.rel(path) + "/") for f in files):
                    self.fail("path_unowned", unit=where, option="L", value=v)
                searched.append(path)
            elif k == "l":
                kind_, eq, name = v.partition("=")
                if v not in libs or (eq and kind_.split(":")[0] == "static"
                                     and not any(p in natives and f"{self.rel(p)}/lib{name}.a" in files for p in searched)):
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
                ok = path in self.produced and self.rel(path) in files
                got[name] = self.produced[path] if ok else None
            if got != want:
                self.fail("extern_unbound", unit=where, names=sorted(n for n in set(got) | set(want) if got.get(n) != want.get(n)))

    def edges(self):
        """Each graph edge no `--extern` binds, bound to the record it causes."""
        links = {p["id"]: p["links"] for p in self.metadata["packages"]}
        env = {i: c["env"] for c, i in self.bound}
        for i, unit in enumerate(self.graph):
            runs = self.deps(i, "run-custom-build")
            for x in unit["dependencies"]:
                dep = self.graph[x["index"]]
                plain = dep["mode"] == "run-custom-build" or not set(dep["target"]["kind"]) & {"lib", "rlib", "proc-macro"}
                if plain and x["extern_crate_name"] != dep["target"]["name"].replace("-", "_"):
                    self.fail("graph_edge_unbound", unit=i, dep=x["index"])
            if unit["mode"] == "run-custom-build":
                owners = [k for k, u in enumerate(self.graph) if u["mode"] != "run-custom-build" and self.own_run(k) == i]
                want = {self.own_run(y) for k in owners for y in self.deps(k) if links.get(self.graph[y]["pkg_id"])} - {None}
                others = [r for r in runs if self.graph[r]["pkg_id"] != unit["pkg_id"]]
                if set(others) != want or len(others) != len(want) or len(others) != len(runs):
                    self.fail("graph_edge_unbound", unit=i, runs=sorted(runs))
            elif "custom-build" not in unit["target"]["kind"]:
                has_run = any(u["mode"] == "run-custom-build" and u["pkg_id"] == unit["pkg_id"] for u in self.graph)
                if len(runs) != int(has_run) or (runs and self.own_run(i) is None):
                    self.fail("graph_edge_unbound", unit=i, runs=sorted(runs))
                bins = {self.graph[y]["target"]["name"] for y in self.deps(i) if "bin" in self.graph[y]["target"]["kind"]}
                if i in env and bins != {k[len("CARGO_BIN_EXE_"):] for k in env[i] if k.startswith("CARGO_BIN_EXE_")}:
                    self.fail("graph_edge_unbound", unit=i, bins=sorted(bins))

    def dep_info(self, c, where, out_dir, arts):
        stem = "build_script_build" if c["key"][0] == "build_script_build" else c["key"][0]
        path = f"{out_dir}/{stem}-{c['extra']}.d"
        text = self.u.inventory["dep_info"].get(self.rel(path))
        self.required.add(self.rel(path))
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

    def inventory(self):
        """The inventory is exactly the files Cargo's layout requires of the recorded units and runs."""
        inv, optional, out_files = self.u.inventory, {}, set()
        files = inv["files"]
        dep_infos = {p for p in self.required if p.endswith(".d")}
        for path, i in self.produced.items():
            unit, rel = self.graph[i], self.rel(path)
            self.required.add(rel)
            name, extra = fingerprint(unit), re.search(rf"-({HASH})(\.\w+)?$", rel).group(1)
            fp = f"release/.fingerprint/{self.package_name(unit['pkg_id'])}-{extra}/"
            self.required |= {fp + "invoked.timestamp", fp + name, fp + name + ".json", fp + "dep-" + name}
            optional[fp + "output-" + name] = unit["pkg_id"]
            if "custom-build" in unit["target"]["kind"]:
                self.required.add(os.path.dirname(rel) + "/build-script-build")
            if "bin" in unit["target"]["kind"]:
                uplifted = "release/" + unit["target"]["name"]
                self.required.add(uplifted)
                if files.get(uplifted) != files.get(rel):
                    self.fail("artifact_missing", file=uplifted)
                if i in self.roots:
                    self.required.add(uplifted + ".d")
                    dep_infos.add(uplifted + ".d")
        for j, r in self.runs.items():
            name, build = fingerprint(self.graph[j]), r["build_dir"]
            fp = f"release/.fingerprint/{self.package_name(self.graph[j]['pkg_id'])}-{build.rsplit('-', 1)[1]}/"
            self.required |= {fp + name, fp + name + ".json", *(f"{build}/{f}" for f in RUN_FILES)}
            mine = {f for f in files if f.startswith(build + "/out/")}
            out_files |= mine
            got = sorted(re.sub(r"^[0-9a-f]{16}-", "*-", f[len(build) + 5:]) for f in mine)
            if got != OUT_PINS.get(self.package_name(self.graph[j]["pkg_id"]), []):
                self.fail("out_dir_contents", run=r["exe"], files=got)
        log = read(os.path.join(self.u.evidence, "build.log"))
        warned = collections.Counter(re.findall(r"^warning: `([^`]+)` \([^)]*\) generated \d+ warnings?", log, re.M))
        cached = collections.Counter(self.package_name(p) for f, p in optional.items() if f in files)
        if warned != cached:
            self.fail("inventory_incomplete", cached_diagnostics=sorted(set((warned - cached) | (cached - warned))))
        for rel in sorted(self.required - set(files)):
            self.fail("inventory_incomplete", file=rel)
        for rel in sorted(set(files) - self.required - set(optional) - out_files):
            self.fail("unowned_file", file=rel)
        outputs = {r["build_dir"] + "/output" for r in self.runs.values()}
        for field, want in (("dep_info", dep_infos), ("outputs", outputs)):
            for rel in sorted(set(inv[field]) ^ want):
                self.fail("inventory_mismatch", field=field, file=rel)
            for rel, text in sorted(inv[field].items()):
                if files.get(rel) != sha256_text(text):
                    self.fail("inventory_mismatch", field=field, file=rel, reason="digest")


def reconcile(units, kind):
    return Reconcile(units, kind).run()
