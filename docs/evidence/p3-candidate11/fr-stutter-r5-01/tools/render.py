#!/usr/bin/env python3
"""Renders citation macros into exact citations.

  render.py SRC.md OUT.md

A macro {{FILE|ITEM|L1-L2}} (or {{FILE|ITEM|L}}) becomes
  [FILE · ITEM · L1–L2 · blob XXXXXXXX · item YYYYYYYYYYYYYYYY]
with the item's blob and the first 16 hex digits of the sha256 of its exact
bytes, taken from Checker A's item index of the frozen sources. A macro
whose lines fall outside the item, or whose item does not exist, stops the
render. Checkers A and B resolve every rendered citation again.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a_items  # noqa: E402
import a_source  # noqa: E402

FULL = {"protocol.rs": "ledger/protocol.rs", "model_tests.rs": "ledger/protocol/model_tests.rs",
        "unit_tests.rs": "ledger/protocol/unit_tests.rs"}
MACRO = re.compile(r"\{\{(protocol\.rs|model_tests\.rs|unit_tests\.rs)\|([^|}]+)\|(\d+)(?:-(\d+))?\}\}")


def main(src_md, out_md):
    src = a_source.Source()
    index = {}
    for short, rel in FULL.items():
        data, txt = src.text(rel)
        root, its, toks, pos = a_items.items(txt)
        for it in its:
            b0, b1, body = a_items.item_bytes(data, txt, it)
            index[(short, it.name)] = (it.line1, it.line2, src.blob(rel), a_items.sha256(body))
    text = open(src_md, encoding="utf-8").read()
    bad = []

    def sub(m):
        short, item, l1, l2 = m.group(1), m.group(2).strip(), int(m.group(3)), int(m.group(4) or m.group(3))
        rec = index.get((short, item))
        if rec is None or not (rec[0] <= l1 <= l2 <= rec[1]):
            bad.append(m.group(0) + (f" (item lines {rec[0]}-{rec[1]})" if rec else " (no such item)"))
            return m.group(0)
        rng = f"L{l1}" if l1 == l2 else f"L{l1}–{l2}"
        return f"[{short} · {item} · {rng} · blob {rec[2][:8]} · item {rec[3][:16]}]"
    out = MACRO.sub(sub, text)
    if bad:
        for b in bad:
            print("BAD CITATION:", b)
        return 1
    open(out_md, "w", encoding="utf-8").write(out)
    print(f"RENDER: {len(MACRO.findall(text))} citations rendered into {os.path.basename(out_md)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
