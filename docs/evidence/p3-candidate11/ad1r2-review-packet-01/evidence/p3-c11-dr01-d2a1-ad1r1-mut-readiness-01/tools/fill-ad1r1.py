#!/usr/bin/env python3
"""Fills AD-1-R1's verbatim blocks from the contract, then checks every fenced block that holds a placeholder
against the contract's lines, character for character. Refuses to write if any check fails.
    fill-ad1r1.py <template> <contract> <out>"""
import hashlib, re, sys
template, contract, out = sys.argv[1:4]
lines = open(contract, encoding="utf-8").read().split("\n")
assert hashlib.sha256(open(contract, "rb").read()).hexdigest() == \
    "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c", "not the accepted contract"
src = open(template, encoding="utf-8").read()
blocks = re.findall(r"```text\n(.*?)\n```", src, flags=re.S)
checked = 0
for block in blocks:
    for row in block.split("\n"):
        m = re.fullmatch(r"\{\{L:(\d+)\}\}", row)
        assert m, f"a verbatim block holds something other than a placeholder: {row!r}"
        checked += 1
filled = re.sub(r"\{\{L:(\d+)\}\}", lambda m: lines[int(m.group(1)) - 1], src)
assert not re.search(r"\{\{L:\d+\}\}", filled), "a placeholder is left"
# Re-check from the filled text: every fenced text block equals contract lines exactly, in the template's order.
fb = re.findall(r"```text\n(.*?)\n```", filled, flags=re.S)
assert len(fb) == len(blocks)
for tb, filled_block in zip(blocks, fb):
    nums = [int(x) for x in re.findall(r"\{\{L:(\d+)\}\}", tb)]
    assert filled_block.split("\n") == [lines[n - 1] for n in nums], f"block {nums} differs"
    for n in nums:
        assert lines[n - 1].strip(), f"line {n} is empty"
open(out, "w", encoding="utf-8").write(filled)
print(f"verbatim blocks {len(fb)}, quoted lines {checked}, all equal to the contract's lines; written {out}")
print("lines quoted:", sorted({int(x) for x in re.findall(r'\{\{L:(\d+)\}\}', src)}))
