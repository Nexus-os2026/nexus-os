#!/usr/bin/env bash
# P2-V1-R3B-I3-I1-R3 post-commit validation on the exact committed bytes:
# a scratch Git checkout made from `git archive` of the commit (its tree is
# checked equal to the commit's), an empty target directory used by nothing
# else, then the candidate runs (section 19 commands, store controls, the
# I2-R1 rerun, API probes) and the repository checks (scope now base..HEAD)
# and the adaptation check.
# Usage: post_commit.sh <repository> <commit> <work directory> <scratch directory> <kernel directory>
set -eu
repo="$1"; commit="$2"; work="$3"; scratch="$4"; kernel="$5"
W="$work/post-commit"
CHK="$scratch/post-checkout"
TGT="$scratch/post-target"
rm -rf "$W" "$CHK" "$TGT"
mkdir -p "$W" "$CHK" "$TGT"
git -C "$repo" archive "$commit" | tar -x -C "$CHK"
cd "$CHK"
git init -q
git add -f -A
git -c user.name=scratch -c user.email=scratch@localhost commit -q -m "post-commit checkout of $commit"
tree_here=$(git rev-parse 'HEAD^{tree}')
tree_commit=$(git -C "$repo" rev-parse "$commit^{tree}")
{
  echo "commit: $commit"
  echo "commit tree: $tree_commit"
  echo "scratch checkout tree: $tree_here"
  [ "$tree_here" = "$tree_commit" ] && echo "trees equal: yes" || echo "trees equal: NO"
  echo "target directory (empty before the runs): $TGT"
} > "$W/identity.txt"
cat "$W/identity.txt"
[ "$tree_here" = "$tree_commit" ] || exit 3
export CARGO_TARGET_DIR="$TGT" PYTHONDONTWRITEBYTECODE=1
bash "$CHK/docs/evidence/p2-v1-r3b-i3-i1-r3/scripts/candidate_runs.sh" "$CHK" "$W/runs" > "$W/runs.console" 2>&1 || true
echo "candidate runs:"; grep -E "exit=" "$W/runs/runs.txt"
cd "$repo"
bash docs/evidence/p2-v1-r3b-i3-i1-r3/scripts/checks.sh "$repo" "$W/checks" "$kernel" > /dev/null 2>&1 || true
python3 -B docs/evidence/p2-v1-r3b-i3-i1-r3/scripts/adaptation_check.py "$repo" "$W/checks/adaptation-diff-apply.txt" > /dev/null 2>&1 || true
echo "checks:"; cat "$W/checks/checks.txt"; tail -1 "$W/checks/adaptation-diff-apply.txt"
