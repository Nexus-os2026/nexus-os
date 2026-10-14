#!/usr/bin/env bash
# P3-C11-DR01-D2A1-AD1R3-READINESS-01 — Stage 1 provenance authentication (read-only).
# Every check compares against the mission's stated identities; any mismatch prints MISMATCH and exits 1.
set -u
R=/home/nexus/NEXUS/nexus-os
A=/home/nexus/nexus-audits/claude-architect
fail=0
chk() { # label actual expected
  if [ "$2" = "$3" ]; then echo "    OK        $1: $2"; else echo "    MISMATCH  $1: actual=$2 expected=$3"; fail=1; fi
}
echo "P3-C11-DR01-D2A1-AD1R3-READINESS-01 — STAGE 1 AUTHENTICATION   $(date -Iseconds)"
echo "executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5), effort high"
echo
echo "[1] remote refs (git ls-remote github)"
refs=$(git -C $R ls-remote github)
echo "    lines $(printf '%s\n' "$refs" | wc -l), sha256 $(printf '%s\n' "$refs" | sha256sum | cut -d' ' -f1)"
printf '%s\n' "$refs" | grep -E 'refs/heads/(main|rebuild/phase0-trust-boundary|review/p3-c11-d2a1-ad1r1-witness-01|review/p3-c11-d2a1-ad1r2-audit-packet-01)$' | sed 's/^/    /'
n=$(printf '%s\n' "$refs" | grep -c 'refs/heads/review/p3-c11-d2a1-ad1r3-readiness-01$')
chk "destination refs/heads/review/p3-c11-d2a1-ad1r3-readiness-01 present count" "$n" "0"
printf '%s\n' "$refs" > "$(dirname "$0")/../receipts/github-refs-before.txt"
echo
echo "[2] engineering source"
S=42dd1bd1cf21ffdea5968b256d2983f6575fab5b
chk "type $S" "$(git -C $R cat-file -t $S)" commit
chk "tree" "$(git -C $R rev-parse $S^{tree})" 7ac6bb21430d15b17e3723b217b20a49dccf8282
chk "parents" "$(git -C $R log -1 --format=%P $S)" 2d83b74b3e9f0bfb6ca4763df56d57f3c044bcda
chk "model_tests.rs blob" "$(git -C $R rev-parse $S:crates/nexus-governed-control/src/ledger/protocol/model_tests.rs)" 5ad26ddc727d8be30f4fb8d7478bc69e1c974933
chk "protocol.rs blob" "$(git -C $R rev-parse $S:crates/nexus-governed-control/src/ledger/protocol.rs)" a2df2841b99cad5d452008a6d4150e9369d07e66
echo
echo "[3] evidence-transport commit"
P=8693e3a742ee4d3917cbfdb809b377bcd919d9f4
chk "type $P" "$(git -C $R cat-file -t $P)" commit
chk "tree" "$(git -C $R rev-parse $P^{tree})" 1e9568ddb5a40336820d266652643d6067cd33f6
chk "parents" "$(git -C $R log -1 --format=%P $P)" $S
chk "remote review/p3-c11-d2a1-ad1r2-audit-packet-01" "$(printf '%s\n' "$refs" | awk '$2=="refs/heads/review/p3-c11-d2a1-ad1r2-audit-packet-01"{print $1}')" $P
chk "packet added-file count" "$(git -C $R diff --name-status $S $P | grep -c '^A')" 169
chk "packet non-added count" "$(git -C $R diff --name-status $S $P | grep -vc '^A')" 0
chk "source tree unchanged under packet (diff outside docs/evidence/p3-candidate11/ad1r2-review-packet-01)" "$(git -C $R diff --name-only $S $P | grep -vc '^docs/evidence/p3-candidate11/ad1r2-review-packet-01/')" 0
PK=docs/evidence/p3-candidate11/ad1r2-review-packet-01
R2P=$PK/ad1r2/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md
chk "AD-1-R2 blob in packet" "$(git -C $R rev-parse $P:$R2P)" 8505fb80867f787e29a20d8f4f3b121cee77213b
chk "AD-1-R2 sha256" "$(git -C $R cat-file -p 8505fb80867f787e29a20d8f4f3b121cee77213b | sha256sum | cut -d' ' -f1)" 53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc
chk "AD-1-R2 bytes" "$(git -C $R cat-file -s 8505fb80867f787e29a20d8f4f3b121cee77213b)" 81534
chk "AD-1-R2 local mission copy sha256" "$(sha256sum $A/p3-c11-dr01-d2a1-ad1r2-contract-evidence-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md | cut -d' ' -f1)" 53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc
echo
echo "[4] packet transport manifest (verified from git objects, no checkout)"
tmp=$(mktemp -d)
git -C $R archive $P $PK | tar -x -C "$tmp"
( cd "$tmp/$PK" && sha256sum -c TRANSPORT-SHA256SUMS >/dev/null 2>"$tmp/err"; echo "    TRANSPORT-SHA256SUMS sha256 $(sha256sum TRANSPORT-SHA256SUMS | cut -d' ' -f1) entries $(wc -l < TRANSPORT-SHA256SUMS) exit $?" )
( cd "$tmp/$PK" && sha256sum --quiet -c TRANSPORT-SHA256SUMS ); chk "packet TRANSPORT-SHA256SUMS check exit" "$?" 0
echo
echo "[5] contract and binding reviews (packet copies vs sealed local originals)"
C=$A/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract
chk "contract sha256" "$(sha256sum $C/P3-C11-D2A1-CONTRACT-02-R1.md | cut -d' ' -f1)" cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c
chk "review-02 sha256" "$(sha256sum $C/P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md | cut -d' ' -f1)" 6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce
chk "review-03-01 sha256" "$(sha256sum $C/P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md | cut -d' ' -f1)" 1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35
for f in P3-C11-D2A1-CONTRACT-02-R1.md P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md; do
  chk "packet copy == sealed original: $f" "$(sha256sum "$tmp/$PK/evidence/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract/$f" | cut -d' ' -f1)" "$(sha256sum $C/$f | cut -d' ' -f1)"
done
rm -rf "$tmp"
echo
echo "[6] sealed local manifests (sha256sum -c, run from each directory)"
for d in p3-c11-dr01-d2a1-ad1r2-contract-evidence-01 p3-c11-dr01-d2a1-ad1r1-witness-audit-01 p3-c11-dr01-d2a1-ad1r1-mut-readiness-01 p3-c11-dr01-d2a1-campaign-01 p3-c11-dr01-d2a1-f3-cap60-01 p3-c11-dr01-d2a1-adm-quotient-01; do
  ( cd $A/$d && sha256sum --quiet -c SHA256SUMS ); rc=$?
  echo "    $d SHA256SUMS $(sha256sum $A/$d/SHA256SUMS | cut -d' ' -f1) entries $(wc -l < $A/$d/SHA256SUMS) exit $rc"
  [ $rc = 0 ] || { echo "    MISMATCH seal $d"; fail=1; }
done
echo
echo "[7] controls definition module (read as text; never imported or executed)"
D=$A/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py
chk "controls_defs.py sha256" "$(sha256sum $D | cut -d' ' -f1)" ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f
chk "controls_defs.py listed in campaign-01 seal" "$(grep -c 'ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f  tools/stage5-prepared-NOT-EXECUTED/controls_defs.py' $A/p3-c11-dr01-d2a1-campaign-01/SHA256SUMS)" 1
chk "readiness probe.py pins DEFS_SHA" "$(grep -c 'ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f' $A/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/tools/probe.py)" 1
echo
echo "[8] local repository state (Owner worktree; not altered)"
echo "    $(git -C $R rev-parse --abbrev-ref HEAD) $(git -C $R rev-parse HEAD) porcelain $(git -C $R status --porcelain | wc -l)"
echo
[ $fail = 0 ] && echo "STAGE 1 RESULT: ALL CHECKS OK" || echo "STAGE 1 RESULT: MISMATCH — STOP"
exit $fail
