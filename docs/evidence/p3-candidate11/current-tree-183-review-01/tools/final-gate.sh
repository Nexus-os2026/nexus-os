#!/usr/bin/env bash
# Final static verification gate for candidate C, mission
# P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01. Run once, after the tools are frozen.
# Usage: final-gate.sh <repo> <C> <packet-dir> <scratch-dir>
# Reads git objects; writes only <packet-dir>/receipts, <packet-dir>/review and <scratch-dir>.
# No build, no test, no mutation applied to any file, no hosted CI.
set -u
REPO=$1; C=$2; PKT=$3; SCR=$4
R4=71264aa81a40b86189783facc8437e0427887c16
R4P=a464a36621523f539d2554a6b176d678c7f885ea
HIST=docs/evidence/p3-candidate10/original-183/manifest.json
AD1R4=docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md
CT=docs/evidence/p3-candidate11/current-tree-183/manifest.json
ADOPT=docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md
DEST=refs/heads/review/p3-c11-d2a1-current-tree-183-01
T=$PKT/tools; RC=$PKT/receipts; RV=$PKT/review
mkdir -p "$RC" "$RV" "$SCR"
LOG=$RC/COMMANDS.log; : > "$LOG"
FAILED=0

# run <receipt> <expected-exit> <command...>: log the exact command, its output and exit code.
# The exit status is taken from the command itself, never parsed back from the receipt.
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
g() { git -C "$REPO" "$@"; }

# 00 freeze record of every generator/checker input.
{
  echo "FREEZE: the files below are the exact tools and inputs this gate ran."
  echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "python=$(python3 --version 2>&1) git=$(git --version)"
  (cd "$T" && find . -type f | LC_ALL=C sort | xargs sha256sum)
} > "$RC/00-FREEZE.txt"

# 01 provenance and candidate identity.
run 01-provenance.txt 0 git -C "$REPO" cat-file -t "$R4"
run 01-provenance.txt 0 git -C "$REPO" rev-list --parents -n1 "$R4"
run 01-provenance.txt 0 git -C "$REPO" rev-list --parents -n1 "$C"
run 01-provenance.txt 0 git -C "$REPO" log -1 --format='commit %H%ntree %T%nparents %P%nauthor %an <%ae> %ad%nsubject %s' "$C"
check 01-provenance.txt "R4 has the single parent $R4P" test "$(g rev-list --parents -n1 $R4)" = "$R4 $R4P"
check 01-provenance.txt "C has the single parent R4" test "$(g rev-list --parents -n1 $C)" = "$C $R4"
{
  echo "CANDIDATE BASELINE C — D2A-1 CANDIDATE FINAL BASELINE — REVIEW PENDING (not frozen, not accepted, not integrated)"
  echo "commit  $(g rev-parse $C)"
  echo "tree    $(g rev-parse $C^{tree})"
  echo "parent  $(g rev-parse $C^)"
  echo "manifest $CT"
  echo "  blob   $(g rev-parse $C:$CT)"
  echo "  sha256 $(g cat-file blob $C:$CT | sha256sum | cut -c1-64)"
  echo "  bytes  $(g cat-file -s $C:$CT)"
  echo "  version $(g cat-file blob $C:$CT | python3 -c 'import json,sys; print(json.load(sys.stdin)["version"])')"
  echo "adoption record $ADOPT"
  echo "  blob   $(g rev-parse $C:$ADOPT)"
  echo "  sha256 $(g cat-file blob $C:$ADOPT | sha256sum | cut -c1-64)"
  echo "changed paths (C^..C):"
  g diff-tree -r --no-commit-id --name-status --no-renames "$C^" "$C"
} > "$RC/01-C-IDENTITY.txt"

# 02 object integrity.
run 02-object-integrity.txt 0 git -C "$REPO" fsck --connectivity-only --no-dangling --no-progress
run 02-object-integrity.txt 0 python3 -c '
import subprocess,sys,hashlib
repo,base,head=sys.argv[1:4]
objs=[l.split()[0] for l in subprocess.run(["git","-C",repo,"rev-list","--objects",base+".."+head],capture_output=True,text=True,check=True).stdout.splitlines()]
bad=0
for o in objs:
    kind=subprocess.run(["git","-C",repo,"cat-file","-t",o],capture_output=True,text=True,check=True).stdout.strip()
    data=subprocess.run(["git","-C",repo,"cat-file",kind,o],capture_output=True,check=True).stdout
    h=hashlib.sha1(kind.encode()+b" "+str(len(data)).encode()+b"\0"+data).hexdigest()
    print(kind,o,"OK" if h==o else "MISMATCH "+h); bad+=h!=o
print("objects new in C:",len(objs),"hash mismatches:",bad); sys.exit(1 if bad or not objs else 0)
' "$REPO" "$R4" "$C"

# 03 historical manifest integrity; 04 AD-1-R4 integrity.
for rev in "$R4" "$C"; do
  run 03-historical-manifest-integrity.txt 0 git -C "$REPO" rev-parse "$rev:$HIST"
  run 04-ad1r4-integrity.txt 0 git -C "$REPO" rev-parse "$rev:$AD1R4"
done
check 03-historical-manifest-integrity.txt "historical manifest sha256 at C is 2c4adc84...2e14" test "$(g cat-file blob $C:$HIST | sha256sum | cut -c1-64)" = 2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14
check 03-historical-manifest-integrity.txt "historical manifest blob at C equals R4's (65fa69dc...)" test "$(g rev-parse $C:$HIST)" = "$(g rev-parse $R4:$HIST)"
check 04-ad1r4-integrity.txt "AD-1-R4 sha256 at C is 00ced6d5...b693" test "$(g cat-file blob $C:$AD1R4 | sha256sum | cut -c1-64)" = 00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693
check 04-ad1r4-integrity.txt "AD-1-R4 blob at C is 35d5074c...b6c1" test "$(g rev-parse $C:$AD1R4)" = 35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1

# 05 changed-path confinement.
run 05-changed-path-confinement.txt 0 git -C "$REPO" diff-tree -r --no-commit-id --name-status --no-renames "$C^" "$C"
check 05-changed-path-confinement.txt "every changed path is an addition under the two allowed directories" \
  python3 -c '
import subprocess,sys
rows=subprocess.run(["git","-C",sys.argv[1],"diff-tree","-r","--no-commit-id","--name-status","--no-renames",sys.argv[2]+"^",sys.argv[2]],capture_output=True,text=True,check=True).stdout.splitlines()
allowed=("docs/evidence/p3-candidate11/ad1r4-adoption/","docs/evidence/p3-candidate11/current-tree-183/")
sys.exit(0 if rows and all(r.split("\t")[0]=="A" and r.split("\t")[1].startswith(allowed) for r in rows) else 1)' "$REPO" "$C"
run 05-changed-path-confinement.txt 0 python3 -c '
import subprocess,sys
repo,c=sys.argv[1:3]
allowed=("docs/evidence/p3-candidate11/ad1r4-adoption/","docs/evidence/p3-candidate11/current-tree-183/")
def tree(rev):
    out=subprocess.run(["git","-C",repo,"ls-tree","-r","-z",rev],capture_output=True,check=True).stdout.split(b"\0")
    return {e.split(b"\t",1)[1].decode():e.split(b"\t",1)[0].decode() for e in out if e}
a,b=tree(c+"^"),tree(c)
outside_a={k:v for k,v in a.items() if not k.startswith(allowed)}
outside_b={k:v for k,v in b.items() if not k.startswith(allowed)}
print("paths outside the allowed directories: parent",len(outside_a),"C",len(outside_b))
print("identical (path, mode, type, blob) outside the allowed directories:",outside_a==outside_b)
print("allowed-directory paths at parent:",sorted(k for k in a if k.startswith(allowed)))
print("allowed-directory paths at C:",sorted(k for k in b if k.startswith(allowed)))
sys.exit(0 if outside_a==outside_b else 1)' "$REPO" "$C"

# 06 all-183 alignment, independent checker, against C's committed manifest.
rm -rf "$SCR/extract-C"
run 06-all-183-check.txt 0 python3 "$T/check-ct1.py" --repo "$REPO" --commit "$C" --extract-dir "$SCR/extract-C"
rm -rf "$SCR/extract-C"

# 07 deterministic regeneration: from C's tree and from R4's tree, twice each; byte equality with C's blob.
g cat-file blob "$C:$CT" > "$SCR/committed.json"
for i in 1 2; do
  run 07-regeneration.txt 0 python3 "$T/gen-ct1.py" --repo "$REPO" --commit "$C" --out "$SCR/regen-C-$i.json"
  run 07-regeneration.txt 0 python3 "$T/gen-ct1.py" --repo "$REPO" --commit "$R4" --out "$SCR/regen-R4-$i.json"
done
for f in regen-C-1 regen-C-2 regen-R4-1 regen-R4-2; do
  run 07-regeneration.txt 0 cmp "$SCR/committed.json" "$SCR/$f.json"
done
run 07-regeneration.txt 0 sha256sum "$SCR/committed.json" "$SCR/regen-C-1.json" "$SCR/regen-C-2.json" "$SCR/regen-R4-1.json" "$SCR/regen-R4-2.json"

# 08 negative controls against C's manifest.
rm -rf "$SCR/negative"
run 08-negative-controls.txt 0 python3 "$T/negative-controls.py" --repo "$REPO" --commit "$C" --manifest-file "$SCR/committed.json" --checker "$T/check-ct1.py" --work "$SCR/negative"
rm -rf "$SCR/negative"

# 09 the 22 stale pairs: raw facts; 10 the rendered reconciliation report.
python3 "$T/reconcile-22.py" --repo "$REPO" --commit "$C" > "$RC/09-reconcile-22-facts.txt" 2>&1
rc=$?; echo "$([ $rc = 0 ] && echo OK || echo FAIL) exit=$rc want=0 :: 09-reconcile-22-facts.txt :: python3 $T/reconcile-22.py --repo $REPO --commit $C" >> "$LOG"; [ $rc = 0 ] || FAILED=$((FAILED+1))
run 10-reconciliation-render.txt 0 python3 "$T/render-reconciliation.py" --facts "$RC/09-reconcile-22-facts.txt" --analysis "$T/analysis-22.json" --out "$RV/RECONCILIATION-22.md"

# 11 mission section 9: C9/C4/C25 adopted-definition conflict check.
run 11-c-series-definition-check.txt 0 python3 "$T/c-series-check.py" --repo "$REPO" --commit "$C"

# 12 raw diffs H -> C of the six stale files, and the filter check for the appended test.
: > "$RC/12-stale-file-diffs-H-to-C.diff"
for pair in "22ebf9a5c5b16bb6f5169daf3731a3417c5289d3 app/src-tauri/src/phase3_tests.rs" \
            "22ebf9a5c5b16bb6f5169daf3731a3417c5289d3 crates/nexus-governed-control/src/lib.rs" \
            "862a11cef284c5e3c34234dfd2677a5462c86d5d crates/nexus-governed-control/src/broker.rs" \
            "6be32bf43cebce623681f2d1e82965ba784a7f04 crates/nexus-governed-control/src/authority/approval.rs" \
            "6be32bf43cebce623681f2d1e82965ba784a7f04 crates/nexus-governed-control/src/authority/mod.rs" \
            "6be32bf43cebce623681f2d1e82965ba784a7f04 crates/nexus-governed-control/src/authority/run.rs"; do
  set -- $pair
  { echo "### transition $1; H = $1^; file $2"; g diff "$1^" "$C" -- "$2"; } >> "$RC/12-stale-file-diffs-H-to-C.diff"
done
run 12-p3-append-filter-check.txt 0 python3 -c '
import json,re,subprocess,sys
repo,c=sys.argv[1:3]
H="22ebf9a5c5b16bb6f5169daf3731a3417c5289d3^"; F="app/src-tauri/src/phase3_tests.rs"
diff=subprocess.run(["git","-C",repo,"diff","-U0",H,c,"--",F],capture_output=True,text=True,check=True).stdout
hunks=[l for l in diff.splitlines() if l.startswith("@@")]
removed=[l for l in diff.splitlines() if l.startswith("-") and not l.startswith("---")]
added_fns=re.findall(r"^\+\s*fn\s+(\w+)",diff,re.M)
old_len=len(subprocess.run(["git","-C",repo,"cat-file","blob",H+":"+F],capture_output=True,check=True).stdout.decode().split("\n"))-1
print("hunks:",hunks); print("removed lines:",len(removed)); print("file lines at H:",old_len); print("functions added:",added_fns)
m=json.loads(subprocess.run(["git","-C",repo,"cat-file","blob",c+":docs/evidence/p3-candidate11/current-tree-183/manifest.json"],capture_output=True,check=True).stdout)
bad=0
for ctl in m["controls"]:
    if not any(t.startswith("phase3_tests::") for t in ctl["expected_failure_tests"]): continue
    cmd=ctl["command"]; filters=cmd[cmd.index("--")+1:]
    hit=[(f,n) for n in added_fns for f in filters if f in "phase3_tests::"+n]
    print(ctl["id"],"filters",filters,"select an added function:",hit or "no"); bad+=bool(hit)
pure_append=len(hunks)==1 and not removed and hunks[0].startswith("@@ -%d,0 "%old_len)
print("pure append after the last line at H:",pure_append)
sys.exit(1 if bad or not pure_append else 0)' "$REPO" "$C"

# 13 credential scan of every file C adds and of this packet.
run 13-secret-scan.txt 1 bash -c 'git -C "$1" diff-tree -r --no-commit-id --name-only "$2^" "$2" | while read p; do git -C "$1" cat-file blob "$2:$p"; done | grep -c -E "ghp_|github_pat_|gho_|AKIA[0-9A-Z]{16}|BEGIN [A-Z ]*PRIVATE KEY|xox[abpr]-|sk-[A-Za-z0-9]{20,}|glpat-|AIza[0-9A-Za-z_-]{30,}"' _ "$REPO" "$C"
run 13-secret-scan.txt 1 grep -r -c -l -E 'ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|gho_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|BEGIN [A-Z ]*PRIVATE KEY|xox[abpr]-[A-Za-z0-9]|glpat-[A-Za-z0-9]|AIza[0-9A-Za-z_-]{30,}' "$RV" "$RC/01-C-IDENTITY.txt" "$RC/09-reconcile-22-facts.txt" "$RC/12-stale-file-diffs-H-to-C.diff"

# 14 hosted workflow triggers: in C's tree, and registered on the server.
run 14-workflow-triggers.txt 0 python3 -c '
import subprocess,sys,yaml,fnmatch
repo,c=sys.argv[1:3]
names=subprocess.run(["git","-C",repo,"ls-tree","-r","--name-only",c,"--",".github/workflows"],capture_output=True,text=True,check=True).stdout.split()
dest="review/p3-c11-d2a1-current-tree-183-01"; hits=[]
for n in names:
    d=yaml.safe_load(subprocess.run(["git","-C",repo,"cat-file","blob",c+":"+n],capture_output=True,check=True).stdout)
    on=d.get(True,d.get("on")); on=on if isinstance(on,dict) else {e:None for e in ([on] if isinstance(on,str) else on or [])}
    push=on.get("push","absent") if "push" in on else "absent"
    print(n,"events:",sorted(on),"push:",push)
    if "push" in on:
        p=on["push"] or {}
        br=p.get("branches"); ign=p.get("branches-ignore"); tags=p.get("tags")
        match = (br is None and tags is None and ign is None) or (br is not None and any(fnmatch.fnmatch(dest,b) for b in br)) or (ign is not None and not any(fnmatch.fnmatch(dest,b) for b in ign))
        if match: hits.append(n)
print("in-tree workflows whose push trigger matches",dest,":",hits or "none")
sys.exit(1 if hits else 0)' "$REPO" "$C"
run 14-workflow-triggers.txt 0 gh api repos/Nexus-os2026/nexus-os/actions/workflows --paginate -q '.workflows[] | "\(.state)\t\(.path)"'
run 14-workflow-triggers.txt 0 bash -c 'for p in .github/workflows/ci-deep-diagnostic.yml .github/workflows/rs01-mutation-audit.yml; do git -C "$1" cat-file -e "$2:$p" 2>/dev/null && echo "$p PRESENT in C" || echo "$p absent from C (cannot trigger on a push of C or its children that do not add it)"; done' _ "$REPO" "$C"

# 15 remote state before publication.
run 15-remote-before.txt 0 bash -c 'out=$(git -C "$1" ls-remote github "$2"); echo "destination [$2]: ${out:-ABSENT}"; [ -z "$out" ]' _ "$REPO" "$DEST"
run 15-remote-before.txt 0 git -C "$REPO" ls-remote github refs/heads/main refs/heads/rebuild/phase0-trust-boundary refs/heads/phase3/governed-real-world-control refs/heads/review/p3-c11-d2a1-ad1r4-consolidation-01
run 15-remote-before.txt 0 gh api 'repos/Nexus-os2026/nexus-os/rules/branches/review%2Fp3-c11-d2a1-current-tree-183-01'
run 15-remote-before.txt 0 gh api repos/Nexus-os2026/nexus-os/rulesets -q '.[] | "\(.id)\t\(.name)\t\(.target)\t\(.enforcement)"'
run 15-remote-before.txt 0 bash -c 'echo "core.hooksPath=$(git -C "$1" config --get core.hooksPath)"; ls -A "$(git -C "$1" config --get core.hooksPath)" | grep -v "\.sample$" | sed "s/^/ACTIVE HOOK: /"; echo "non-sample hooks listed above (none = no active hook)"' _ "$REPO"

{
  echo "GATE SUMMARY for C $C"
  echo "failed steps: $FAILED"
  grep -c '^OK' "$LOG" | sed 's/^/OK entries: /'
  grep '^FAIL' "$LOG" || echo "no FAIL entries"
  echo "RESULT $([ $FAILED = 0 ] && echo PASS || echo FAIL)"
} > "$RC/GATE-SUMMARY.txt"
cat "$RC/GATE-SUMMARY.txt"
[ $FAILED = 0 ]
