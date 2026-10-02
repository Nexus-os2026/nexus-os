#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R1 evidence normalization.

Copies a raw output file into the evidence directory with one documented
transformation, so that `git diff --check` accepts it without any
.gitattributes exemption (none is authorized):

- each line loses its trailing spaces, tabs and carriage returns;
- blank lines at the end of the file are removed;
- the file ends with exactly one newline (an empty file stays empty).

Nothing else changes: no path, time, number or word is rewritten. Each copy
is recorded in a TSV manifest with its raw and normalized SHA-256 and byte
counts, so the raw output (kept outside the repository) can be matched.

Usage: normalize.py <manifest.tsv> <raw file> <evidence file> [<raw file> <evidence file>]...
"""
import hashlib
import pathlib
import sys


def normalize(data: bytes) -> bytes:
    lines = data.split(b"\n")
    lines = [line.rstrip(b" \t\r") for line in lines]
    while lines and lines[-1] == b"":
        lines.pop()
    return b"\n".join(lines) + b"\n" if lines else b""


def main():
    args = sys.argv[1:]
    if len(args) < 3 or len(args) % 2 != 1:
        raise SystemExit(__doc__)
    manifest = pathlib.Path(args[0])
    rows = []
    for raw_name, out_name in zip(args[1::2], args[2::2]):
        raw = pathlib.Path(raw_name).read_bytes()
        normalized = normalize(raw)
        out = pathlib.Path(out_name)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(normalized)
        rows.append("\t".join([
            out_name, hashlib.sha256(raw).hexdigest(), str(len(raw)),
            hashlib.sha256(normalized).hexdigest(), str(len(normalized)),
            "unchanged" if raw == normalized else "whitespace-normalized",
        ]))
    with manifest.open("a") as handle:
        for row in rows:
            handle.write(row + "\n")


if __name__ == "__main__":
    main()
