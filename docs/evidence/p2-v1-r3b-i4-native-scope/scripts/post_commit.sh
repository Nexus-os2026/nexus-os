#!/usr/bin/env bash
# P2-V1-R3B-I4 post-commit validation on the exact committed bytes: a
# scratch Git checkout made from `git archive` of the commit (its tree is
# checked equal to the commit's), then the candidate runs (section 23
# commands, the behavioural controls, the API/type and source guards, the
# accepted suites' reruns, stability and the desktop callers, each mutating
# step with its own empty target directory), the repository checks (scope
# now base..HEAD), the citations and review searches on the committed
# bytes, and the comparison with the pre-commit candidate runs.
# Usage: post_commit.sh <repository> <commit> <work directory> <scratch directory>
set -eu
repo="$1"; commit="$2"; work="$3"; scratch="$4"
W="$work/post-commit"
CHK="$scratch/post-checkout"
TGT="$scratch/post-targets"
for path in "$W" "$CHK" "$TGT"; do
  [ ! -e "$path" ] || { echo "exists: $path" >&2; exit 2; }
done
mkdir -p "$W" "$CHK"
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
  echo "targets (each step's own, empty before it): $TGT"
} > "$W/identity.txt"
cat "$W/identity.txt"
[ "$tree_here" = "$tree_commit" ] || exit 3
export PYTHONDONTWRITEBYTECODE=1
E="$CHK/docs/evidence/p2-v1-r3b-i4-native-scope/scripts"
bash "$E/candidate_runs.sh" "$CHK" "$W/runs" "$TGT" > "$W/runs.console" 2>&1 || true
echo "candidate runs:"; grep -E "exit=" "$W/runs/runs.txt"
python3 -B "$E/citations.py" "$CHK" "$W/citations.txt" || true
bash "$E/review_searches.sh" "$CHK" > "$W/review-searches.txt" 2>&1 || true
cd "$repo"
bash docs/evidence/p2-v1-r3b-i4-native-scope/scripts/checks.sh "$repo" "$W/runs" "$W/checks" > /dev/null 2>&1 || true
echo "checks:"; cat "$W/checks/checks.txt"
python3 -B docs/evidence/p2-v1-r3b-i4-native-scope/scripts/compare_runs.py \
  "$work/candidate-runs" "$W/runs" > "$W/comparison.txt" 2>&1 || true
echo "comparison with the pre-commit runs:"; tail -1 "$W/comparison.txt"
