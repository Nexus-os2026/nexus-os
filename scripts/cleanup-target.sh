#!/usr/bin/env bash
# =============================================================================
# scripts/cleanup-target.sh — workspace target/ disk hygiene (Bug X)
# =============================================================================
#
# WHY THIS EXISTS
#   Phase 4b-herald hit ENOSPC mid-gate-run because target/debug/ reached
#   251GB. target/debug/incremental/ alone was 96GB. Manual recovery via
#   `rm -rf target/debug/incremental` freed enough to finish. This script
#   makes that cleanup discoverable + scriptable so the same wall doesn't
#   block another commit.
#
# USAGE
#   scripts/cleanup-target.sh --incremental    # default: rm incremental dirs (~80–96GB)
#   scripts/cleanup-target.sh --aggressive     # full cargo clean (10–15 min cold rebuild)
#   scripts/cleanup-target.sh --report-only    # print breakdown, no deletion
#
# POLICY
#   - --incremental — run when target/ approaches 200GB. Fast (~2 min next compile).
#   - --aggressive  — after major milestones (Track close, version bump). Cold rebuild.
#   - --report-only — for monitoring scripts / health checks.
# =============================================================================

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

MODE="${1:---incremental}"

bytes_used() {
  # Returns size in bytes for the given path; 0 if path is absent.
  if [[ -e "$1" ]]; then
    du -sb "$1" 2>/dev/null | awk '{print $1}'
  else
    echo 0
  fi
}

human() {
  # Convert bytes → human-readable. Numfmt is GNU; fall back to bytes if absent.
  if command -v numfmt >/dev/null 2>&1; then
    numfmt --to=iec --suffix=B "$1"
  else
    echo "${1}B"
  fi
}

print_breakdown() {
  echo "[cleanup-target] breakdown:"
  printf "  %-40s %s\n" "target/"                "$(human "$(bytes_used target)")"
  printf "  %-40s %s\n" "  target/debug/"          "$(human "$(bytes_used target/debug)")"
  printf "  %-40s %s\n" "    target/debug/incremental/" "$(human "$(bytes_used target/debug/incremental)")"
  printf "  %-40s %s\n" "    target/debug/deps/"   "$(human "$(bytes_used target/debug/deps)")"
  printf "  %-40s %s\n" "  target/release/"        "$(human "$(bytes_used target/release)")"
  printf "  %-40s %s\n" "    target/release/incremental/" "$(human "$(bytes_used target/release/incremental)")"
  printf "  %-40s %s\n" "    target/release/deps/" "$(human "$(bytes_used target/release/deps)")"
}

case "$MODE" in
  --report-only)
    print_breakdown
    ;;

  --incremental)
    BEFORE="$(bytes_used target)"
    print_breakdown
    echo ""
    echo "[cleanup-target] mode: --incremental — removing target/{debug,release}/incremental/"
    rm -rf target/debug/incremental
    rm -rf target/release/incremental
    AFTER="$(bytes_used target)"
    FREED=$(( BEFORE - AFTER ))
    echo ""
    echo "[cleanup-target] freed $(human "$FREED") (target/: $(human "$BEFORE") → $(human "$AFTER"))"
    echo "[cleanup-target] next compile rebuilds incremental state only (~2 min); deps preserved."
    ;;

  --aggressive)
    BEFORE="$(bytes_used target)"
    print_breakdown
    echo ""
    echo "[cleanup-target] mode: --aggressive — running 'cargo clean'"
    cargo clean
    AFTER="$(bytes_used target)"
    FREED=$(( BEFORE - AFTER ))
    echo ""
    echo "[cleanup-target] freed $(human "$FREED") (target/: $(human "$BEFORE") → $(human "$AFTER"))"
    echo "[cleanup-target] full cold rebuild required on next compile (10–15 min workspace-wide)."
    ;;

  -h|--help)
    sed -n '1,30p' "$0" | sed 's/^# \?//'
    ;;

  *)
    echo "[cleanup-target] unknown mode: $MODE" >&2
    echo "[cleanup-target] usage: $0 [--incremental | --aggressive | --report-only | --help]" >&2
    exit 2
    ;;
esac
