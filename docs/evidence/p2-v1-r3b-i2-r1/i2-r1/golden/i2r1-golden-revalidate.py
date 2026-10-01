#!/usr/bin/env python3
"""Revalidate every golden vector published in tests/phase2_custody_codec.rs.

Stdlib only. Imports the independent generator (golden_vectors.py, beside
this file), renders its version-1 vectors from the format table, and
compares them, entry by entry and in order, with the three literal tables
in the test file: RECORD_VECTORS, REQUEST_VECTORS and RECEIPT_VECTORS.
Entries are compared after removing whitespace and trailing commas, which
only rustfmt changes. Each published frame is also re-hashed here with
hashlib. Exit status 0 only if everything matches.

Usage: golden_revalidate.py <path to phase2_custody_codec.rs>
"""
import hashlib
import importlib.util
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent


def load_generator():
    for name in ("golden_vectors.py", "i2r1-golden-vectors.py"):
        path = HERE / name
        if path.exists():
            spec = importlib.util.spec_from_file_location("golden_vectors", path)
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            return module, path
    raise SystemExit("the generator is not beside this script")


def normalize(text):
    text = re.sub(r"\s+", "", text)
    return text.replace(",}", "}").replace(",)", ")").replace(",]", "]")


def entries(source, table):
    start = source.index(f"const {table}: &[")
    body_start = source.index("= &[", start) + 4
    body_end = source.index("\n];\n", body_start)
    body = source[body_start:body_end]
    kind = "RecordVector" if table == "RECORD_VECTORS" else "RequestVector"
    parts = [part for part in body.split(f"{kind} {{")[1:]]
    return [normalize(f"{kind} {{" + part) for part in parts]


def rendered(generator, table):
    if table == "RECORD_VECTORS":
        items = [generator.record(*spec)["rust"] for spec in generator.RECORDS]
    elif table == "REQUEST_VECTORS":
        items = [generator.request(*spec)["rust"] for spec in generator.REQUESTS]
    else:
        items = [generator.request(*spec)["rust"] for spec in generator.RECEIPTS]
    return [normalize(item) for item in items]


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    source = pathlib.Path(sys.argv[1]).read_text()
    generator, path = load_generator()
    print(f"generator: {path.name} sha256 {hashlib.sha256(path.read_bytes()).hexdigest()}")
    ok = True
    total = 0
    for table in ("RECORD_VECTORS", "REQUEST_VECTORS", "RECEIPT_VECTORS"):
        published = entries(source, table)
        expected = rendered(generator, table)
        same = published == expected
        ok &= same
        print(f"{table}: {len(published)} published, {len(expected)} generated, identical: {same}")
        for index, (got, want) in enumerate(zip(published, expected)):
            if got != want:
                print(f"  entry {index} differs:\n    published {got}\n    generated {want}")
        total += len(published)
    pairs = re.findall(r'frame:\s*"([0-9a-f ]+)",\s*digest:\s*"([0-9a-f]{64})"', source)
    rehashed = 0
    for frame, digest in pairs:
        covered = bytes.fromhex(frame.replace(" ", ""))
        if hashlib.sha256(covered).hexdigest() != digest:
            ok = False
            print(f"  digest mismatch for {frame}")
        else:
            rehashed += 1
    print(f"frames re-hashed with hashlib: {rehashed} of {len(pairs)} match")
    ok &= rehashed == len(pairs) == total
    print("RESULT:", "all published vectors match the independent generator" if ok else "MISMATCH")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
