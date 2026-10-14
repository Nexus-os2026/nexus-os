#!/usr/bin/env bash
# Final static gate, mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Run once, after the freeze.
# Usage: final-gate.sh <worktree> <packet-dir> <scratch-dir>
# Reads git objects, the sealed evidence root and (read-only) the GitHub remote; writes only
# <packet-dir>/receipts and <scratch-dir>. No build, no test, no witness, no mutation, no hosted CI.
set -u
# The tools run from the packet; no bytecode cache may be written into it.
export PYTHONDONTWRITEBYTECODE=1
WT=$1; PKT=$2; SCR=$3
C=d5964ee5326c2733177307db6f507328023d361a
C_TREE=51bac5fd335999cc995c0ab9f582a30244973ef0
C_PARENT=71264aa81a40b86189783facc8437e0427887c16
D=066a322a9182fc978e3f5dc8454157caacec6241
D_TREE=da31dfa7c9a0c4f358bfde4f15815f6c8d46f806
REL=docs/evidence/p3-candidate11/final-source-proof-readiness-01
DEST=refs/heads/review/p3-c11-d2a1-final-source-proof-readiness-01
T=$PKT/tools; RC=$PKT/receipts
mkdir -p "$RC" "$SCR"
LOG=$RC/COMMANDS.log; : > "$LOG"
FAILED=0

# run <receipt> <expected-exit> <command...>: the exact command, its output and its own exit status.
run() {
  local out=$1 want=$2; shift 2
  local cmd rc; cmd=$(printf '%q ' "$@")
  echo "\$ $cmd" >> "$RC/$out"
  "$@" >> "$RC/$out" 2>&1; rc=$?
  printf '\nexit=%s\n\n' "$rc" >> "$RC/$out"
  local verdict=OK; [ "$rc" = "$want" ] || { verdict=FAIL; FAILED=$((FAILED+1)); }
  echo "$verdict exit=$rc want=$want :: $out :: $cmd" >> "$LOG"
}
check() {  # check <receipt> <description> <condition-command...>
  local out=$1 what=$2; shift 2
  if "$@" >/dev/null 2>&1; then echo "PASS $what" >> "$RC/$out"; echo "OK check :: $out :: $what" >> "$LOG"
  else echo "FAIL $what" >> "$RC/$out"; echo "FAIL check :: $out :: $what" >> "$LOG"; FAILED=$((FAILED+1)); fi
}
g() { git -C "$WT" "$@"; }
echo "gate start $(date -u +%Y-%m-%dT%H:%M:%SZ) python=$(python3 --version 2>&1) git=$(git --version)" >> "$LOG"

# 00 the freeze holds at the start.
run 00-freeze-verify.txt 0 bash -c 'cd "$1" && sha256sum -c --strict receipts/00-FREEZE.sha256' _ "$PKT"

# 01 identity of C, D and the worktree.
run 01-identity.txt 0 git -C "$WT" log -1 --format='commit %H%ntree %T%nparents %P%nsubject %s' "$C"
run 01-identity.txt 0 git -C "$WT" log -1 --format='commit %H%ntree %T%nparents %P%nsubject %s' "$D"
check 01-identity.txt "C is $C with tree $C_TREE and the single parent $C_PARENT" \
  test "$(g rev-list --parents -n1 $C) $(g rev-parse $C^{tree})" = "$C $C_PARENT $C_TREE"
check 01-identity.txt "D is $D with tree $D_TREE and the single parent C" \
  test "$(g rev-list --parents -n1 $D) $(g rev-parse $D^{tree})" = "$D $C $D_TREE"
check 01-identity.txt "the worktree HEAD is D" test "$(g rev-parse HEAD)" = "$D"
run 02-object-integrity.txt 0 git -C "$WT" fsck --connectivity-only --no-dangling --no-progress

# 10-14 the static tools, each on git objects (and the sealed evidence root for 14).
run 10-source-closure.log 0 python3 "$T/source_closure.py" "$WT" --out "$RC/10-source-closure.json"
run 11-witness-check.log 0 python3 "$T/witness_check.py" "$WT" --out "$RC/11-witness-check.json"
run 12-build-inputs.log 0 python3 "$T/build_inputs.py" "$WT" --out "$RC/12-build-inputs.json"
run 13-config-identity.log 0 python3 "$T/config_identity.py" "$WT" --out "$RC/13-config-identity.json"
run 14-proof-docs.log 0 python3 "$T/proof_docs.py" "$WT" --out "$RC/14-proof-docs.json"
check 11-witness-check.log "both witness additions NON_INTERFERING" python3 -c '
import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if d["verdicts"]=={"C1/C25":"NON_INTERFERING","C4":"NON_INTERFERING"} else 1)' "$RC/11-witness-check.json"

# 15-16 the matrix regenerated from the gate's own receipts must equal the frozen files byte for byte.
run 15-matrix.log 0 python3 "$T/matrix.py" "$WT" --source-closure "$RC/10-source-closure.json" \
  --witness-check "$RC/11-witness-check.json" --proof-docs "$RC/14-proof-docs.json" \
  --tsv "$SCR/PROOF-PRECONDITION-MATRIX.tsv" --out "$RC/15-matrix.json"
run 15-matrix.log 0 cmp "$SCR/PROOF-PRECONDITION-MATRIX.tsv" "$PKT/PROOF-PRECONDITION-MATRIX.tsv"
run 16-render.log 0 python3 "$T/render_matrix.py" "$SCR/PROOF-PRECONDITION-MATRIX.tsv" "$RC/15-matrix.json" \
  "$RC/14-proof-docs.json" "$SCR/PROOF-PRECONDITION-MATRIX.md"
run 16-render.log 0 cmp "$SCR/PROOF-PRECONDITION-MATRIX.md" "$PKT/PROOF-PRECONDITION-MATRIX.md"
check 15-matrix.log "every matrix row has a status from the mission's vocabulary and none claims acceptance" python3 -c '
import csv,sys; csv.field_size_limit(10**9)
ok={"IDENTICAL","NON_INTERFERING_CHANGE","CHANGED_REVIEW_REQUIRED","EXECUTION_PENDING","OWNER_ALREADY_SATISFIED","NOT_ESTABLISHED"}
rows=list(csv.DictReader(open(sys.argv[1]),delimiter="\t"))
sys.exit(0 if rows and all(r["static_status"] in ok for r in rows) else 1)' "$PKT/PROOF-PRECONDITION-MATRIX.tsv"

# 17 the authored documents carry the gate's own computed facts and no acceptance phrase.
run 17-doc-claims.txt 0 python3 "$T/check_docs.py" "$PKT" "$RC"
# 18 the checkers' sensitivity.
run 18-negative-controls.txt 0 python3 "$T/negative_controls.py" "$WT"

# 19 packet confinement and no source change.
run 19-confinement.txt 0 git -C "$WT" status --porcelain=v1 --untracked-files=all
check 19-confinement.txt "no tracked file in the worktree is modified or staged" bash -c 'git -C "$1" diff --quiet HEAD && git -C "$1" diff --cached --quiet' _ "$WT"
check 19-confinement.txt "every untracked path is inside the packet directory" bash -c '
out=$(git -C "$1" status --porcelain=v1 --untracked-files=all); [ -n "$out" ] && ! echo "$out" | grep -v "^?? $2/"' _ "$WT" "$REL"
run 19-confinement.txt 0 python3 -c '
import os,sys
root=sys.argv[1]; total=0; bad=[]
for dp,dn,fn in os.walk(root):
    for f in fn:
        p=os.path.join(dp,f); total+=os.path.getsize(p)
        if os.path.islink(p): bad.append(p+" is a symlink")
        data=open(p,"rb").read()
        if b"\0" in data: bad.append(p+" has a NUL byte")
        try: data.decode("utf-8")
        except UnicodeDecodeError: bad.append(p+" is not UTF-8")
        if "__pycache__" in p or p.endswith(".pyc"): bad.append(p+" is a cache file")
print("packet bytes:",total,"limit:",2*1024*1024); print("problems:",bad or "none")
sys.exit(0 if total<=2*1024*1024 and not bad else 1)' "$PKT"
check 19-confinement.txt "C's tree object is unchanged ($C_TREE)" test "$(g rev-parse $C^{tree})" = "$C_TREE"

# 20 credentials.
run 20-secret-scan.txt 1 grep -r -l -E 'ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|gho_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|BEGIN [A-Z ]*PRIVATE KEY|xox[abpr]-[A-Za-z0-9]|glpat-[A-Za-z0-9]|AIza[0-9A-Za-z_-]{30,}|sk-[A-Za-z0-9]{20,}' "$PKT"

# 21 hosted workflow triggers: D's tree (the evidence commit adds only packet files), and those registered.
run 21-workflow-triggers.txt 0 python3 -c '
import subprocess,sys,yaml,fnmatch
repo,c,dest=sys.argv[1:4]
names=subprocess.run(["git","-C",repo,"ls-tree","-r","--name-only",c,"--",".github/workflows"],capture_output=True,text=True,check=True).stdout.split()
hits=[]
for n in names:
    d=yaml.safe_load(subprocess.run(["git","-C",repo,"cat-file","blob",c+":"+n],capture_output=True,check=True).stdout)
    on=d.get(True,d.get("on")); on=on if isinstance(on,dict) else {e:None for e in ([on] if isinstance(on,str) else on or [])}
    print(n,"events:",sorted(on),"push:",on.get("push","absent") if "push" in on else "absent")
    if "push" in on:
        p=on["push"] or {}
        br=p.get("branches"); ign=p.get("branches-ignore"); tags=p.get("tags")
        match=(br is None and tags is None and ign is None) or (br is not None and any(fnmatch.fnmatch(dest,b) for b in br)) or (ign is not None and not any(fnmatch.fnmatch(dest,b) for b in ign))
        if match: hits.append(n)
print("in-tree workflows whose push trigger matches",dest,"(fnmatch, which over-approximates GitHub globs):",hits or "none")
sys.exit(1 if hits else 0)' "$WT" "$D" "review/p3-c11-d2a1-final-source-proof-readiness-01"
run 21-workflow-triggers.txt 0 gh api repos/Nexus-os2026/nexus-os/actions/workflows --paginate -q '.workflows[] | "\(.state)\t\(.path)"'
run 21-workflow-triggers.txt 0 bash -c 'for p in .github/workflows/ci-deep-diagnostic.yml .github/workflows/rs01-mutation-audit.yml; do git -C "$1" cat-file -e "$2:$p" 2>/dev/null && echo "$p PRESENT in D" || echo "$p absent from D (cannot trigger on a push of a commit that does not add it)"; done' _ "$WT" "$D"

# 22 remote state before publication; hooks.
run 22-remote-before.txt 0 bash -c 'out=$(git -C "$1" ls-remote github "$2"); echo "destination [$2]: ${out:-ABSENT}"; [ -z "$out" ]' _ "$WT" "$DEST"
run 22-remote-before.txt 0 git -C "$WT" ls-remote github refs/heads/main refs/heads/rebuild/phase0-trust-boundary refs/heads/review/p3-c11-d2a1-current-tree-183-01 refs/heads/review/p3-c11-d2a1-ad1r2-audit-packet-01
check 22-remote-before.txt "the published D ref is D" test "$(g ls-remote github refs/heads/review/p3-c11-d2a1-current-tree-183-01 | cut -f1)" = "$D"
check 22-remote-before.txt "the published proof-transport ref is 8693e3a7" test "$(g ls-remote github refs/heads/review/p3-c11-d2a1-ad1r2-audit-packet-01 | cut -f1)" = "8693e3a742ee4d3917cbfdb809b377bcd919d9f4"
run 22-remote-before.txt 0 gh api 'repos/Nexus-os2026/nexus-os/rules/branches/review%2Fp3-c11-d2a1-final-source-proof-readiness-01'
run 22-remote-before.txt 0 bash -c 'h=$(git -C "$1" config --get core.hooksPath || echo "$(git -C "$1" rev-parse --git-common-dir)/hooks"); echo "hooks dir: $h"; hooks=$(ls -A "$h" | grep -v "\.sample$"); echo "non-sample hooks: ${hooks:-none}"; [ -z "$hooks" ]' _ "$WT"

# 23 the freeze still holds at the end (no tool or frozen document changed during the gate).
run 23-freeze-verify-end.txt 0 bash -c 'cd "$1" && sha256sum -c --strict receipts/00-FREEZE.sha256' _ "$PKT"

{
  echo "GATE SUMMARY — P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01, Candidate C $C"
  echo "failed steps: $FAILED"
  grep -c '^OK' "$LOG" | sed 's/^/OK entries: /'
  grep '^FAIL' "$LOG" || echo "no FAIL entries"
  echo "gate end $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "RESULT $([ $FAILED = 0 ] && echo PASS || echo FAIL)"
} > "$RC/GATE-SUMMARY.txt"
cat "$RC/GATE-SUMMARY.txt"
[ $FAILED = 0 ]
