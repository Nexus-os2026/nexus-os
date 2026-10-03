#!/usr/bin/env python3
"""P2-V1-R3B-I4-R1 citation listing (I4's, for this evidence's matrices): every `file:line` or `file:a-b` citation
in the evidence's matrices, resolved against the checkout's sources, with
the cited lines printed, so a reader can verify each one. A citation of a
line that does not exist fails the check. Short forms (`:123` after a
named file in the same table cell) are resolved against the last named
file.

Usage: citations.py <checkout> <output file>
"""
import pathlib
import re
import sys

SRC = "crates/nexus-verifier-sandbox/src/"
MATRICES = "docs/evidence/p2-v1-r3b-i4-r1-native-scope/matrices"
FILE = re.compile(r"`((?:scope/|execution/)?[a-z_]+\.rs|scope\.rs):(\d+)(?:-(\d+))?")
SHORT = re.compile(r"`:(\d+)(?:-(\d+))?`")


def main():
    root = pathlib.Path(sys.argv[1]).resolve()
    out = pathlib.Path(sys.argv[2])
    lines, ok, count = [], True, 0
    for md in sorted((root / MATRICES).glob("*.md")):
        for number, text in enumerate(md.read_text().splitlines(), 1):
            for cell in text.split("|"):
                last = None
                tokens = []
                for match in re.finditer(r"`((?:scope/|execution/)?[a-z_]+\.rs):(\d+)(?:-(\d+))?|`:(\d+)(?:-(\d+))?`", cell):
                    if match.group(1):
                        last = match.group(1)
                        tokens.append((last, int(match.group(2)), int(match.group(3) or match.group(2))))
                    elif last:
                        tokens.append((last, int(match.group(4)), int(match.group(5) or match.group(4))))
                for name, first, final in tokens:
                    count += 1
                    path = root / SRC / name
                    source = path.read_text().splitlines() if path.exists() else []
                    good = 1 <= first <= final <= len(source)
                    ok &= good
                    lines.append(f"{md.name}:{number}: {name}:{first}-{final} {'ok' if good else 'MISSING'}")
                    for at in range(first, min(final, first + 2) + 1):
                        if 1 <= at <= len(source):
                            lines.append(f"    {at}| {source[at - 1]}")
    lines.append(f"citations: {count}")
    lines.append("RESULT: " + ("PASS" if ok else "FAIL"))
    out.write_text("\n".join(lines) + "\n")
    print(lines[-2], lines[-1])
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
