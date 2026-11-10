#!/usr/bin/python3 -I
"""SB-3E A7 generated evidence mutation sweep (ruling SB3E-R15 item 1).

`--evidence DIR --wrapper W --work DIR [--offline] [--jobs N] [--json OUT]`.
Every key path is enumerated from `route_a7_graph.SCHEMA` itself over a real
evidence pair (DIR/ship, DIR/lane). For each concrete path of each build: the
key deleted (a list element dropped), the value blanked, a digest emptied and
made malformed; every evidence file deleted and blanked. Each mutant is applied
to the ship build, to the lane build, and to both when both builds hold the
same value there. A mutant is killed only when the comparator fails it with
named kinds; a pass or an exception is a survivor. `run` mode gives the
comparator the observation `run` makes (the pristine recorded runner fields);
`--offline` exercises `compare` on stored evidence.
"""
import argparse
import collections
import json
import multiprocessing
import os
import pickle
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import route_a7 as a7  # noqa: E402
import route_a7_graph as graph  # noqa: E402

BUILDS = ("ship", "lane")
OBSERVED = ("repo", "target", "toolchain", "cargo_home", "launch_block", "rustc", "cargo", "sysroot",
            "repo_hash_before", "repo_hash_after", "pre_build")
STATE = {}


def schema_paths(node, prefix, out):
    """Every key path the schema declares (map keys as `*`, list elements as `[]`)."""
    out.append(prefix)
    node = node[0] if isinstance(node, tuple) else node
    if isinstance(node, dict):
        for k, v in node.items():
            schema_paths(v, f"{prefix}.{'*' if k is str else k}", out)
    elif isinstance(node, list):
        schema_paths(node[0], prefix + "[]", out)
    return out


def walk(node, value, path, abstract, out):
    """Every concrete key path of one evidence value under its schema node: (path, abstract path, value, digest)."""
    node = node[0] if isinstance(node, tuple) else node
    if path:
        out.append((path, abstract, value, node == graph.DIGEST))
    if isinstance(node, dict) and isinstance(value, dict):
        for k, v in value.items():
            if list(node) == [str] or k in node:
                walk(node[str] if list(node) == [str] else node[k], v, path + (k,), f"{abstract}.{'*' if list(node) == [str] else k}", out)
    elif isinstance(node, list) and isinstance(value, list):
        for i, v in enumerate(value):
            walk(node[0], v, path + (i,), abstract + "[]", out)
    return out


def blank(value):
    return None if value is None else type(value)()


def mutations(value, digest):
    """(op, new value) pairs at one path; identical results collapse onto the first op."""
    ops = [("delete", None), ("blank", blank(value))]
    if digest and isinstance(value, str):
        ops += [("digest_empty", ""), ("digest_malformed", "z" + value[1:]), ("digest_short", value[:-1])]
    seen, out = [], []
    for op, new in ops:
        if op != "delete" and (new == value or new in seen):
            continue
        seen.append(new)
        out.append((op, new))
    return out


def get(data, path):
    for k in path:
        data = data[k]
    return data


def enumerate_mutants(data):
    mutants, noops = [], 0
    for name in [*graph.SCHEMA, "build.log", "wrapper.log"]:
        holders = [k for k in BUILDS if name in data[k]]
        for variant in holders + (["both"] if len(holders) == 2 else []):
            mutants += [(variant, name, (), op, None, name) for op in ("delete", "blank")]
    for name, node in graph.SCHEMA.items():
        paths = {k: {p: (a, v, dg) for p, a, v, dg in walk(node, data[k][name], (), name, [])} for k in BUILDS}
        for k in BUILDS:
            for path, (abstract, value, digest) in paths[k].items():
                ops = mutations(value, digest)
                noops += 2 + 3 * digest - len(ops)
                both = k == "ship" and path in paths["lane"] and paths["lane"][path][1] == value
                for op, new in ops:
                    mutants.append((k, name, path, op, new, abstract))
                    if both:
                        mutants.append(("both", name, path, op, new, abstract))
    return mutants, noops


def apply(data, name, path, op, new):
    """Mutate in place; returns the undo."""
    parent, key = get(data[name], path[:-1]), path[-1]
    old = parent[key]
    if op == "delete":
        parent.pop(key)
        return lambda: parent.insert(key, old) if isinstance(parent, list) else parent.__setitem__(key, old)
    parent[key] = new
    return lambda: parent.__setitem__(key, old)


def write(path, data):
    with open(path, "wb") as handle:
        handle.write(data)


def init(work):
    root = os.path.join(work, f"w{os.getpid()}")
    shutil.rmtree(root, ignore_errors=True)
    for k in BUILDS:
        os.makedirs(os.path.join(root, k))
        for name, raw in STATE["raw"][k].items():
            write(os.path.join(root, k, name), raw)
    STATE["root"] = root
    real = a7.Units.parse
    cache = {}

    def parse(self, path):  # memoised: a pure function of the log and the meta fields it reads
        st, m = os.stat(path), self.meta
        key = (path, st.st_size, st.st_mtime_ns, m["wrapper"], m["toolchain"], m["target"], m["repo"], m["cargo_home"])
        if key not in cache:
            real(self, path)
            cache[key] = pickle.dumps((self.hashes, self.compiles, self.runs, self.problems))
        self.hashes, self.compiles, self.runs, self.problems = pickle.loads(cache[key])
    a7.Units.parse = parse


def run_one(index):
    variant, name, path, op, new, abstract = STATE["mutants"][index]
    root, data = STATE["root"], STATE["data"]
    targets = BUILDS if variant == "both" else (variant,)
    undo = []
    for k in targets:
        file = os.path.join(root, k, name)
        if not path:
            os.unlink(file) if op == "delete" else write(file, b"")
        else:
            undo.append(apply(data[k], name, path, op, new))
            write(file, json.dumps(data[k][name], sort_keys=True).encode("utf-8", "surrogateescape"))
    try:
        result = a7.compare(os.path.join(root, "ship"), os.path.join(root, "lane"), STATE["wrapper"], STATE["wrapper_sha"], STATE["observed"])
        kinds = sorted({x["kind"] for x in result["differences"]}) if result["verdict"] == "fail" else []
        error = None
    except Exception as exc:  # a crash is a survivor, not a kill
        kinds, error = [], repr(exc)[:300]
    for k in targets:
        for u in undo:
            u()
        undo = []
        write(os.path.join(root, k, name), STATE["raw"][k][name])
    return index, kinds, error


def main():
    parser = argparse.ArgumentParser()
    for name in ("--evidence", "--wrapper", "--work"):
        parser.add_argument(name, required=True)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--json")
    args = parser.parse_args()
    began = time.monotonic()
    raw = {k: {n: open(os.path.join(args.evidence, k, n), "rb").read() for n in [*graph.SCHEMA, *graph.FILES[k]]} for k in BUILDS}
    data = {k: {n: json.loads(raw[k][n]) for n in graph.SCHEMA} for k in BUILDS}
    for k in BUILDS:
        data[k].update({n: None for n in graph.FILES[k]})
    mutants, noops = enumerate_mutants(data)
    abstract = schema_paths(graph.SCHEMA, "", [])[1:]
    observed = None if args.offline else {k: {f: json.loads(raw[k]["meta.json"])[f] for f in OBSERVED} for k in BUILDS}  # never aliased
    STATE.update(raw=raw, data=data, mutants=mutants, wrapper=args.wrapper, wrapper_sha=a7.sha256_file(args.wrapper), observed=observed)
    os.makedirs(args.work, exist_ok=True)
    init(args.work)
    baseline = a7.compare(os.path.join(STATE["root"], "ship"), os.path.join(STATE["root"], "lane"), args.wrapper, STATE["wrapper_sha"], observed)
    shutil.rmtree(STATE["root"])
    if baseline["verdict"] != "pass":
        sys.exit(f"route_a7_sweep: the unmutated evidence does not pass: {sorted({x['kind'] for x in baseline['differences']})}")
    survivors, kinds_by_path, by = [], collections.defaultdict(collections.Counter), collections.Counter()
    with multiprocessing.get_context("fork").Pool(args.jobs, initializer=init, initargs=(args.work,)) as pool:
        for index, kinds, error in pool.imap_unordered(run_one, range(len(mutants)), chunksize=64):
            variant, name, path, op, new, abstract_path = mutants[index]
            by[(variant, op)] += 1
            for kind in kinds:
                kinds_by_path[abstract_path][kind] += 1
            if not kinds:
                survivors.append({"variant": variant, "file": name, "path": list(path), "op": op, "error": error})
    shutil.rmtree(args.work, ignore_errors=True)
    summary = {
        "mode": "offline" if args.offline else "run", "schema_key_paths": len(abstract), "schema_paths": abstract,
        "concrete_key_paths": {k: sum(len(walk(n, data[k][f], (), f, [])) for f, n in graph.SCHEMA.items()) for k in BUILDS},
        "mutants": len(mutants), "identical_mutations_skipped": noops,
        "by_variant_op": {f"{v}/{o}": n for (v, o), n in sorted(by.items())},
        "survivors": len(survivors), "survivor_list": survivors[:200],
        "kinds_by_schema_path": {p: dict(c) for p, c in sorted(kinds_by_path.items())},
        "seconds": round(time.monotonic() - began, 1),
    }
    if args.json:
        a7.write_json(args.json, summary)
    print(json.dumps({k: v for k, v in summary.items() if k not in ("schema_paths", "kinds_by_schema_path", "survivor_list")}, sort_keys=True))
    for s in survivors[:40]:
        print("SURVIVOR", json.dumps(s))
    return 0 if not survivors else 1


if __name__ == "__main__":
    sys.exit(main())
