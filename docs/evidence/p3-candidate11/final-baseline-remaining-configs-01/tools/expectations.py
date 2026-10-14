#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01, mission section 13: the expectations frozen before the first witness.

  expectations.py OUT.json      writes the record; refuses to overwrite

Every value is read from the frozen tools' own constants or derived from the source at C (configs.py); nothing is
typed here. No expected state count exists.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_witness  # noqa: E402
import common  # noqa: E402
import configs  # noqa: E402
import supervise  # noqa: E402


def main(out):
    if os.path.exists(out):
        raise SystemExit("REFUSED: exists")
    d = configs.derive()
    if d["problems"]:
        raise SystemExit(f"REFUSED: {d['problems']}")
    rec = dict(
        mission=common.MISSION, commit=common.C, tree=common.C_TREE, blobs=common.BLOBS,
        executable=dict(path=common.EXE, sha256=common.EXE_SHA256, size=common.EXE_SIZE),
        configurations={k: {f: v[f] for f in ("test", "bounds", "actors", "properties", "na")}
                        for k, v in d["required"].items()},
        remaining_order=[supervise.CFG[k] for k in supervise.ORDER],
        expected_states="none asserted (mission section 8)",
        witnesses={k: dict(test=w["test"], control=w["control"], clause=w["clause"], healthy_lines=w["healthy"],
                           receipt_dir=w["dir"]) for k, w in check_witness.W.items()},
        c4_schedule=dict(commit=common.C4_SCHEDULE[0], path=common.C4_SCHEDULE[1], sha256=common.C4_SCHEDULE[2]),
        supervision=dict(memory_max="16G", memory_swap_max=0, wall_limits_seconds=dict(witness=180, full=2700),
                         backstop="RuntimeMaxSec = limit + 10", min_mem_available_kib=supervise.NEED_KIB,
                         env=dict(supervise.RUN_ENV), cwd=supervise.CRATE_DIR, nice=10, unit_prefix=supervise.UNIT_PREFIX,
                         witness_argv=["<exe>", "<test>", "--exact", "--nocapture", "--test-threads=1"],
                         full_argv=["<exe>", "<test>", "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                         order=["C1", "C25", "C4"] + [supervise.CFG[k] for k in supervise.ORDER],
                         rules=["one call, one receipt; nothing retried", "a run starts only after the previous one is "
                                "classified PASS", "freeze, source and executable identity checked before and after",
                                "no cap increase, no alternate explorer, no quotient"]),
        authorised_state_ceiling=30_000_000)
    json.dump(rec, open(out, "w"), indent=1, ensure_ascii=False)
    print(f"wrote {out}: {len(rec['configurations'])} configurations, {len(rec['witnesses'])} witnesses")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
