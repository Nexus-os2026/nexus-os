#!/usr/bin/env bash
# An isolated snapshot of the candidate: the base commit's tree (git
# archive), overlaid with every changed and new file of the worktree, in a
# fresh Git repository with one scratch commit (so its status is clean).
# Unchanged from docs/evidence/p2-v1-r3b-i4-r1-native-scope/scripts/
# make_snapshot.sh (P2-V1-R3B-I4-R2) except for this comment.
# Usage: make_snapshot.sh <repository> <base> <destination (must not exist)>
set -eu
repo="$1"; base="$2"; dest="$3"
[ ! -e "$dest" ] || { echo "exists: $dest" >&2; exit 2; }
mkdir -p "$dest"
git -C "$repo" archive "$base" | tar -x -C "$dest"
git -C "$repo" status --porcelain=v1 --untracked-files=all | while IFS= read -r line; do
  path="${line:3}"
  case "${line:0:2}" in
    " D"|"D ") rm -f "$dest/$path" ;;
    *) mkdir -p "$dest/$(dirname "$path")"; cp -p "$repo/$path" "$dest/$path" ;;
  esac
done
cd "$dest"
git init -q
git add -f -A
git -c user.name=scratch -c user.email=scratch@localhost commit -q -m "candidate snapshot"
# Every file gets a fresh mtime (no stale build artifact can match it).
find . -path ./.git -prune -o -type f -exec touch {} +
echo "snapshot tree: $(git rev-parse 'HEAD^{tree}')"
