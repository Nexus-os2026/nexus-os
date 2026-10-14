#!/usr/bin/env python3
"""Fills AD-1-R3's verbatim blocks from the accepted contract's bytes and from the two binding reviews' bytes.
Derived from AD-1-R2's fill-ad1r2.py. Every fenced `text` block of the template must hold only placeholders of one
source ({{L:n}} contract, {{R2:n}} REVIEW-02, {{R3:n}} REVIEW-03-01) and must follow a VERBATIM marker of that source
naming exactly those line numbers. Refuses to write over an existing file. The independent check of the result is
tools/check-ad1r3.py.
    fill-ad1r3.py <template> <contract> <review-02> <review-03> <out>"""
import hashlib, os, re, sys

template, contract, review02, review03, out = sys.argv[1:6]
EXPECT = {
    "L": (contract, "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"),
    "R2": (review02, "6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce"),
    "R3": (review03, "1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35"),
}
LINES = {}
for key, (path, sha) in EXPECT.items():
    raw = open(path, "rb").read()
    assert hashlib.sha256(raw).hexdigest() == sha, f"{path}: not the authenticated file"
    LINES[key] = raw.decode("utf-8").split("\n")
MARK = {"": "L", " REVIEW-02": "R2", " REVIEW-03": "R3"}
src = open(template, encoding="utf-8").read()
pairs = re.findall(r"<!-- VERBATIM((?: REVIEW-0[23])?) ([0-9,]+) -->\n```text\n(.*?)\n```", src, flags=re.S)
fences = re.findall(r"```text\n", src)
assert len(pairs) == len(fences), f"{len(fences)} text fences but {len(pairs)} follow a VERBATIM marker"
quoted = {"L": [], "R2": [], "R3": []}
for which, marker, block in pairs:
    key = MARK[which]
    nums = [int(x) for x in marker.split(",")]
    rows = block.split("\n")
    pat = r"\{\{" + key + r":(\d+)\}\}"
    assert all(re.fullmatch(pat, r) for r in rows), f"block{which} {marker} holds something other than {key} placeholders"
    assert [int(re.fullmatch(pat, r).group(1)) for r in rows] == nums, f"marker{which} {marker} != block"
    for n in nums:
        assert 1 <= n <= len(LINES[key]) and LINES[key][n - 1].strip(), f"{key} line {n} is empty or out of range"
        assert LINES[key][n - 1] == LINES[key][n - 1].rstrip(), f"{key} line {n} has trailing whitespace"
    quoted[key] += nums
filled = re.sub(r"\{\{(L|R2|R3):(\d+)\}\}", lambda m: LINES[m.group(1)][int(m.group(2)) - 1], src)
assert "{{" not in filled, "a placeholder is left"
if os.path.exists(out):
    sys.exit(f"REFUSED: {out} exists")
with open(out, "x", encoding="utf-8") as f:
    f.write(filled)
print(f"verbatim blocks {len(pairs)}; written {out}")
for key in ("L", "R2", "R3"):
    print(f"  {key}: quoted lines {len(quoted[key])} ({len(set(quoted[key]))} distinct): {sorted(set(quoted[key]))}")
