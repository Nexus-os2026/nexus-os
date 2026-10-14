#!/usr/bin/env python3
"""Fills AD-1-R2's verbatim blocks from the accepted contract's bytes. Every fenced `text` block of the template must
hold only {{L:n}} placeholders and must follow a VERBATIM marker naming exactly those line numbers. Refuses to write
over an existing file. The independent check of the result is tools/check-ad1r2.py.
    fill-ad1r2.py <template> <contract> <out>"""
import hashlib, os, re, sys

template, contract, out = sys.argv[1:4]
CONTRACT_SHA256 = "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"
raw = open(contract, "rb").read()
assert hashlib.sha256(raw).hexdigest() == CONTRACT_SHA256, "not the accepted contract"
lines = raw.decode("utf-8").split("\n")
src = open(template, encoding="utf-8").read()
pairs = re.findall(r"<!-- VERBATIM ([0-9,]+) -->\n```text\n(.*?)\n```", src, flags=re.S)
fences = re.findall(r"```text\n", src)
assert len(pairs) == len(fences), f"{len(fences)} text fences but {len(pairs)} follow a VERBATIM marker"
quoted = []
for marker, block in pairs:
    nums = [int(x) for x in marker.split(",")]
    rows = block.split("\n")
    assert [re.fullmatch(r"\{\{L:(\d+)\}\}", r) is not None for r in rows] == [True] * len(rows), \
        f"block {marker} holds something other than placeholders"
    assert [int(re.fullmatch(r"\{\{L:(\d+)\}\}", r).group(1)) for r in rows] == nums, f"marker {marker} != block"
    for n in nums:
        assert 1 <= n <= len(lines) and lines[n - 1].strip(), f"line {n} is empty or out of range"
    quoted += nums
filled = re.sub(r"\{\{L:(\d+)\}\}", lambda m: lines[int(m.group(1)) - 1], src)
assert "{{L:" not in filled, "a placeholder is left"
if os.path.exists(out):
    sys.exit(f"REFUSED: {out} exists")
with open(out, "x", encoding="utf-8") as f:
    f.write(filled)
print(f"verbatim blocks {len(pairs)}, quoted lines {len(quoted)} ({len(set(quoted))} distinct); written {out}")
print("lines quoted:", sorted(set(quoted)))
