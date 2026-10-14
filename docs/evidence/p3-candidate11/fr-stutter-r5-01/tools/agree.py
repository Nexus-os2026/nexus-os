#!/usr/bin/env python3
"""Agreement of the two independent checkers.

  agree.py A_DIR B_DIR

Checks that Checker A and Checker B found exactly the same target
occurrences (file, line, column, token, region, group, enclosing item and
ordinal), that both classified each with the same status, and that both
call closures of `explore` hold the same functions. Exit 0 only if all
agree.
"""
import json
import os
import sys


def key(o):
    return (o["file"], o["line"], o["col"], o["token"], o["region"], o["group"], o["item"], o["ordinal"])


def main(a_dir, b_dir):
    a = json.load(open(os.path.join(a_dir, "A-occurrences.json")))
    b = json.load(open(os.path.join(b_dir, "B-occurrences.json")))
    ka = {key(o): o for o in a}
    kb = {key(o): o for o in b}
    only_a = sorted(set(ka) - set(kb))
    only_b = sorted(set(kb) - set(ka))
    status = [k for k in set(ka) & set(kb) if ka[k].get("status") != kb[k].get("status")]
    ca = [l.strip() for l in open(os.path.join(a_dir, "A-explore-closure.txt")) if not l.startswith("#")]
    cb = [l.strip() for l in open(os.path.join(b_dir, "B-explore-closure.txt")) if not l.startswith("#")]
    sa, sb = set(filter(None, ca)), set(filter(None, cb))
    ok = not only_a and not only_b and not status and sa == sb
    print(f"AGREEMENT: A {len(ka)} occurrences, B {len(kb)}; only A {len(only_a)}, only B {len(only_b)}; "
          f"status differences {len(status)}")
    print(f"AGREEMENT: explore closure A {len(sa)} fns, B {len(sb)} fns; only A {sorted(sa - sb)}; only B {sorted(sb - sa)}")
    for k in only_a[:20]:
        print("  only A:", k)
    for k in only_b[:20]:
        print("  only B:", k)
    for k in status[:20]:
        print("  status:", k, ka[k].get("status"), kb[k].get("status"))
    print("AGREEMENT:", "BOTH ANALYSES AGREE" if ok else "DISAGREEMENT")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
