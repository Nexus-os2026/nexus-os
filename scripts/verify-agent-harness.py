"""Read-only, offline validation. Does not execute receipt or mission commands."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

from agent_harness import InvalidEvidence, validate_campaign, verify_repository


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--mission", type=Path, required=True,
                        help="Explicit mission directory; never discovered by recency")
    parser.add_argument("--check-isolation", action="store_true",
                        help="Also compare mission host refs/Candidate-10 worktrees/evidence")
    parser.add_argument("--campaign", type=Path, help="Optional campaign JSON; validates receipts/mapping only")
    args = parser.parse_args()
    root = args.root.resolve()
    mission = args.mission if args.mission.is_absolute() else root / args.mission
    try:
        result = verify_repository(root, mission, args.check_isolation)
        if args.campaign:
            result["campaign"] = validate_campaign(json.loads(args.campaign.read_text()), mission / "evidence")
        print(json.dumps(result, indent=2))
    except (InvalidEvidence, OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as exc:
        print("HARNESS_ERROR: " + str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
