#!/usr/bin/env bash
# P3-C11-DR01-D2A1-C4-WITNESS-01 — Stage 1 provenance authentication (read-only).
# Every check compares against the mission's stated identities; any mismatch prints MISMATCH and exits 1.
set -u
R=/home/nexus/NEXUS/nexus-os
A=/home/nexus/nexus-audits/claude-architect
EV=$A/p3-c11-dr01-d2a1-c4-witness-01
fail=0
chk() { # label actual expected
  if [ "$2" = "$3" ]; then echo "    OK        $1: $2"; else echo "    MISMATCH  $1: actual=$2 expected=$3"; fail=1; fi
}
echo "P3-C11-DR01-D2A1-C4-WITNESS-01 — STAGE 1 AUTHENTICATION   $(date -Iseconds)"
echo "executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5), effort high (mission: claude-opus-5-5, High)"
echo
echo "[1] remote refs (git ls-remote github)"
refs=$(git -C $R ls-remote github)
echo "    lines $(printf '%s\n' "$refs" | wc -l), sha256 $(printf '%s\n' "$refs" | sha256sum | cut -d' ' -f1)"
printf '%s\n' "$refs" | grep -E 'refs/heads/(main|rebuild/phase0-trust-boundary|review/p3-c11-d2a1-ad1r1-witness-01|review/p3-c11-d2a1-ad1r3-readiness-01)$' | sed 's/^/    /'
n=$(printf '%s\n' "$refs" | grep -c 'refs/heads/review/p3-c11-d2a1-c4-witness-01$')
chk "destination refs/heads/review/p3-c11-d2a1-c4-witness-01 present count" "$n" "0"
chk "remote review/p3-c11-d2a1-ad1r3-readiness-01" "$(printf '%s\n' "$refs" | awk '$2=="refs/heads/review/p3-c11-d2a1-ad1r3-readiness-01"{print $1}')" c2c69e3a8a5cc447cac8a86424a1600a1f23cecf
chk "remote review/p3-c11-d2a1-ad1r1-witness-01" "$(printf '%s\n' "$refs" | awk '$2=="refs/heads/review/p3-c11-d2a1-ad1r1-witness-01"{print $1}')" 42dd1bd1cf21ffdea5968b256d2983f6575fab5b
printf '%s\n' "$refs" > "$EV/receipts/github-refs-before.txt"
echo
echo "[2] engineering source"
S=42dd1bd1cf21ffdea5968b256d2983f6575fab5b
chk "type $S" "$(git -C $R cat-file -t $S)" commit
chk "tree" "$(git -C $R rev-parse $S^{tree})" 7ac6bb21430d15b17e3723b217b20a49dccf8282
chk "parents" "$(git -C $R log -1 --format=%P $S)" 2d83b74b3e9f0bfb6ca4763df56d57f3c044bcda
chk "model_tests.rs blob" "$(git -C $R rev-parse $S:crates/nexus-governed-control/src/ledger/protocol/model_tests.rs)" 5ad26ddc727d8be30f4fb8d7478bc69e1c974933
chk "protocol.rs blob" "$(git -C $R rev-parse $S:crates/nexus-governed-control/src/ledger/protocol.rs)" a2df2841b99cad5d452008a6d4150e9369d07e66
chk "protocol.rs sha256" "$(git -C $R cat-file blob a2df2841b99cad5d452008a6d4150e9369d07e66 | sha256sum | cut -d' ' -f1)" a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296
chk "model_tests.rs sha256" "$(git -C $R cat-file blob 5ad26ddc727d8be30f4fb8d7478bc69e1c974933 | sha256sum | cut -d' ' -f1)" be9e3547f06d66d5541bc33660745dd2167b844fdc5000072118309b0b56b2ba
chk "model_tests.rs lines" "$(git -C $R cat-file blob 5ad26ddc727d8be30f4fb8d7478bc69e1c974933 | wc -l)" 8521
echo
echo "[3] R3 evidence commit and draft"
E=c2c69e3a8a5cc447cac8a86424a1600a1f23cecf
PK=docs/evidence/p3-candidate11/ad1r3-review-01
chk "type $E" "$(git -C $R cat-file -t $E)" commit
chk "tree" "$(git -C $R rev-parse $E^{tree})" f246f9f51b221bd0a6d7091052376f337f9e44b9
chk "R3 draft blob" "$(git -C $R rev-parse $E:$PK/ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md)" 7f2463f5421502cc6ee74c070fc3abc41d467770
chk "R3 draft sha256" "$(git -C $R cat-file blob 7f2463f5421502cc6ee74c070fc3abc41d467770 | sha256sum | cut -d' ' -f1)" 485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e
chk "R3 commit leaves source unchanged (protocol.rs)" "$(git -C $R rev-parse $E:crates/nexus-governed-control/src/ledger/protocol.rs)" a2df2841b99cad5d452008a6d4150e9369d07e66
chk "R3 commit leaves source unchanged (model_tests.rs)" "$(git -C $R rev-parse $E:crates/nexus-governed-control/src/ledger/protocol/model_tests.rs)" 5ad26ddc727d8be30f4fb8d7478bc69e1c974933
tmp=$(mktemp -d)
git -C $R archive $E $PK | tar -x -C "$tmp"
( cd "$tmp/$PK" && sha256sum --quiet -c PACKET-SHA256SUMS ); chk "R3 packet PACKET-SHA256SUMS check exit" "$?" 0
echo "    R3 packet PACKET-SHA256SUMS sha256 $(sha256sum "$tmp/$PK/PACKET-SHA256SUMS" | cut -d' ' -f1) entries $(wc -l < "$tmp/$PK/PACKET-SHA256SUMS")"
chk "R3 preflight sha256 (packet copy == local mission copy)" "$(sha256sum "$tmp/$PK/preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md" | cut -d' ' -f1)" "$(sha256sum $A/p3-c11-dr01-d2a1-ad1r3-readiness-01/preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md | cut -d' ' -f1)"
rm -rf "$tmp"
echo
echo "[4] contract and binding reviews (sealed local originals)"
C=$A/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract
chk "contract sha256" "$(sha256sum $C/P3-C11-D2A1-CONTRACT-02-R1.md | cut -d' ' -f1)" cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c
chk "review-02 sha256" "$(sha256sum $C/P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md | cut -d' ' -f1)" 6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce
chk "review-03-01 sha256" "$(sha256sum $C/P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md | cut -d' ' -f1)" 1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35
echo
echo "[5] sealed local manifests (sha256sum -c, run from each directory)"
for d in p3-c11-dr01-d2a1-ad1r3-readiness-01 p3-c11-dr01-d2a1-ad1r1-witness-audit-01 p3-c11-dr01-d2a1-ad1r1-mut-readiness-01 p3-c11-dr01-d2a1-campaign-01; do
  ( cd $A/$d && sha256sum --quiet -c SHA256SUMS ); rc=$?
  echo "    $d SHA256SUMS $(sha256sum $A/$d/SHA256SUMS | cut -d' ' -f1) entries $(wc -l < $A/$d/SHA256SUMS) exit $rc"
  [ $rc = 0 ] || { echo "    MISMATCH seal $d"; fail=1; }
done
D=$A/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py
chk "controls_defs.py sha256 (read as text only)" "$(sha256sum $D | cut -d' ' -f1)" ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f
echo "    controls_defs.py mentions of C4 / C9 as keys: $(grep -c -E '\"C4\"|\"C9\"' $D)"
echo
echo "[6] local repository state (Owner worktree; not altered)"
echo "    $(git -C $R rev-parse --abbrev-ref HEAD) $(git -C $R rev-parse HEAD) porcelain $(git -C $R status --porcelain | wc -l)"
echo "    hooks path $(git -C $R config --get core.hooksPath); non-sample hooks: $(ls $R/.git/hooks | grep -vc '\.sample$')"
echo
[ $fail = 0 ] && echo "STAGE 1 RESULT: ALL CHECKS OK" || echo "STAGE 1 RESULT: MISMATCH — STOP"
exit $fail
