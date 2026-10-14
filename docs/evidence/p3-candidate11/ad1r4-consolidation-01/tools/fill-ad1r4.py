#!/usr/bin/env python3
"""Fills AD-1-R4's verbatim blocks from authenticated bytes (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01).
Derived from AD-1-R3's fill-ad1r3.py; two sources added. Every fenced `text` block of the template must hold only
placeholders of one source and must follow a VERBATIM marker of that source naming exactly those line numbers:
    {{L:n}}  contract                     <!-- VERBATIM n,... -->
    {{R2:n}} REVIEW-02                    <!-- VERBATIM REVIEW-02 n,... -->
    {{R3:n}} REVIEW-03-01                 <!-- VERBATIM REVIEW-03 n,... -->
    {{P:n}}  protocol.rs at 42dd1bd1      <!-- VERBATIM PROTOCOL n,... -->
    {{D:n}}  sealed controls_defs.py      <!-- VERBATIM DEFS n,... -->
No placeholder may appear outside such a block. Refuses to write over an existing file. The independent check of the
result is tools/check-ad1r4.py.
    fill-ad1r4.py <template> <contract> <review-02> <review-03> <protocol.rs> <controls_defs.py> <out>"""
import hashlib, os, re, sys

template, contract, review02, review03, protocol, defs, out = sys.argv[1:8]
EXPECT = {
    "L": (contract, "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c"),
    "R2": (review02, "6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce"),
    "R3": (review03, "1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35"),
    "P": (protocol, "a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296"),
    "D": (defs, "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"),
}
LINES = {}
for key, (path, sha) in EXPECT.items():
    raw = open(path, "rb").read()
    assert hashlib.sha256(raw).hexdigest() == sha, f"{path}: not the authenticated file"
    LINES[key] = raw.decode("utf-8").split("\n")
MARK = {"": "L", " REVIEW-02": "R2", " REVIEW-03": "R3", " PROTOCOL": "P", " DEFS": "D"}
src = open(template, encoding="utf-8").read()
pairs = re.findall(r"<!-- VERBATIM((?: REVIEW-0[23]| PROTOCOL| DEFS)?) ([0-9,]+) -->\n```text\n(.*?)\n```", src, flags=re.S)
fences = re.findall(r"^```text$", src, flags=re.M)
assert len(pairs) == len(fences), f"{len(fences)} text fences but {len(pairs)} follow a VERBATIM marker"
quoted = {k: [] for k in EXPECT}
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
inside = sum(len(b.split("\n")) for _, _, b in pairs)
assert len(re.findall(r"\{\{(?:L|R2|R3|P|D):\d+\}\}", src)) == inside, "a placeholder stands outside a verbatim block"
filled = re.sub(r"\{\{(L|R2|R3|P|D):(\d+)\}\}", lambda m: LINES[m.group(1)][int(m.group(2)) - 1], src)
assert "{{" not in filled, "a placeholder is left"
if os.path.exists(out):
    sys.exit(f"REFUSED: {out} exists")
with open(out, "x", encoding="utf-8") as f:
    f.write(filled)
print(f"verbatim blocks {len(pairs)}; written {out}")
for key in EXPECT:
    print(f"  {key}: quoted lines {len(quoted[key])} ({len(set(quoted[key]))} distinct): {sorted(set(quoted[key]))}")
