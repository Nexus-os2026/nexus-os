#!/usr/bin/env bash
# P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01 — Stage 1 provenance authentication (read-only).
# Every check compares against the mission's stated identities; any mismatch prints MISMATCH and exits 1.
# Reads git objects, sealed local evidence and the two retained executables (hashed, never run). Writes only
# receipts/github-refs-before.txt.
set -u
R=/home/nexus/NEXUS/nexus-os
A=/home/nexus/nexus-audits/claude-architect
M=$A/p3-c11-dr01-d2a1-ad1r4-consolidation-01
fail=0
chk() { # label actual expected
  if [ "$2" = "$3" ]; then echo "    OK        $1: $2"; else echo "    MISMATCH  $1: actual=$2 expected=$3"; fail=1; fi
}
gsha() { git -C $R show "$1" | sha256sum | cut -d' ' -f1; }
echo "P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01 — STAGE 1 AUTHENTICATION   $(date -Iseconds)"
echo "executor: Claude Code, Claude Opus 5.5 (claude-opus-5-5); mission: claude-opus-5-5, effort High"
echo
echo "[1] remote refs (git ls-remote github)"
refs=$(git -C $R ls-remote github)
echo "    lines $(printf '%s\n' "$refs" | wc -l), sha256 $(printf '%s\n' "$refs" | sha256sum | cut -d' ' -f1)"
printf '%s\n' "$refs" | grep -E 'refs/heads/(main|rebuild/phase0-trust-boundary|review/p3-c11-d2a1-(ad1r1-witness-01|ad1r2-audit-packet-01|ad1r3-readiness-01|c4-witness-01))$' | sed 's/^/    /'
chk "destination refs/heads/review/p3-c11-d2a1-ad1r4-consolidation-01 present count" \
    "$(printf '%s\n' "$refs" | grep -c 'refs/heads/review/p3-c11-d2a1-ad1r4-consolidation-01$')" 0
chk "remote review/p3-c11-d2a1-c4-witness-01" "$(printf '%s\n' "$refs" | awk '$2=="refs/heads/review/p3-c11-d2a1-c4-witness-01"{print $1}')" a464a36621523f539d2554a6b176d678c7f885ea
chk "remote review/p3-c11-d2a1-ad1r3-readiness-01" "$(printf '%s\n' "$refs" | awk '$2=="refs/heads/review/p3-c11-d2a1-ad1r3-readiness-01"{print $1}')" c2c69e3a8a5cc447cac8a86424a1600a1f23cecf
chk "remote review/p3-c11-d2a1-ad1r1-witness-01" "$(printf '%s\n' "$refs" | awk '$2=="refs/heads/review/p3-c11-d2a1-ad1r1-witness-01"{print $1}')" 42dd1bd1cf21ffdea5968b256d2983f6575fab5b
printf '%s\n' "$refs" > "$M/receipts/github-refs-before.txt"
echo
echo "[2] engineering source baseline for historical source references"
S=42dd1bd1cf21ffdea5968b256d2983f6575fab5b
chk "type $S" "$(git -C $R cat-file -t $S)" commit
chk "tree" "$(git -C $R rev-parse $S^{tree})" 7ac6bb21430d15b17e3723b217b20a49dccf8282
chk "parents" "$(git -C $R log -1 --format=%P $S)" 2d83b74b3e9f0bfb6ca4763df56d57f3c044bcda
chk "protocol.rs blob" "$(git -C $R rev-parse $S:crates/nexus-governed-control/src/ledger/protocol.rs)" a2df2841b99cad5d452008a6d4150e9369d07e66
chk "model_tests.rs blob" "$(git -C $R rev-parse $S:crates/nexus-governed-control/src/ledger/protocol/model_tests.rs)" 5ad26ddc727d8be30f4fb8d7478bc69e1c974933
echo
echo "[3] C4 prototype A and evidence B"
A4=04e42a7ddf0d0a2af17b6506c51997006e7a08dc
B=a464a36621523f539d2554a6b176d678c7f885ea
chk "type A" "$(git -C $R cat-file -t $A4)" commit
chk "A tree" "$(git -C $R rev-parse $A4^{tree})" fca5c2a28e1c6cfef3cd92fbb81484a56769a9b3
chk "A parent" "$(git -C $R log -1 --format=%P $A4)" $S
chk "A model_tests.rs blob" "$(git -C $R rev-parse $A4:crates/nexus-governed-control/src/ledger/protocol/model_tests.rs)" 28085ce232827301104b5d757af042ec933976f8
chk "A protocol.rs blob (baseline, unchanged)" "$(git -C $R rev-parse $A4:crates/nexus-governed-control/src/ledger/protocol.rs)" a2df2841b99cad5d452008a6d4150e9369d07e66
chk "A delta (numstat vs parent)" "$(git -C $R diff --numstat $S $A4 | tr '\t' ' ')" "678 0 crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
chk "type B" "$(git -C $R cat-file -t $B)" commit
chk "B parent" "$(git -C $R log -1 --format=%P $B)" $A4
chk "B tree" "$(git -C $R rev-parse $B^{tree})" e1f3e48f5b5fc7c792d99fb21e274f5c5e3af2b3
chk "B paths outside docs/evidence/p3-candidate11/c4-witness-01/" "$(git -C $R diff --name-only $A4 $B | grep -vc '^docs/evidence/p3-candidate11/c4-witness-01/')" 0
CP=docs/evidence/p3-candidate11/c4-witness-01
chk "C4 patch sha256" "$(gsha $B:$CP/patches/C4-test1-removed.patch)" 24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d
tmp=$(mktemp -d)
mkdir -p "$tmp/crates/nexus-governed-control/src/ledger"
git -C $R show $S:crates/nexus-governed-control/src/ledger/protocol.rs > "$tmp/crates/nexus-governed-control/src/ledger/protocol.rs"
git -C $R show $B:$CP/patches/C4-test1-removed.patch > "$tmp/c4.patch"
( cd "$tmp" && patch -s -p1 < c4.patch ) >/dev/null 2>&1
chk "mutant protocol.rs blob (baseline + C4 patch, in a scratch directory)" "$(git hash-object "$tmp/crates/nexus-governed-control/src/ledger/protocol.rs")" f3793c2ff61f8adbf43fe17d6b5237b49bc507b0
chk "C4 patch: lines removed / added" "$(grep -c '^-[^-]' "$tmp/c4.patch") / $(grep -c '^+[^+]' "$tmp/c4.patch")" "16 / 0"
rm -rf "$tmp"
chk "B packet manifest check exit" "$(d=$(mktemp -d); git -C $R archive $B $CP | tar -x -C $d; (cd $d/$CP && sha256sum --quiet -c PACKET-SHA256SUMS >/dev/null 2>&1); echo $?; rm -rf $d)" 0
chk "B pins: healthy executable" "$(git -C $R show $B:$CP/receipts/pins-final.json | python3 -c 'import json,sys;print(json.load(sys.stdin)["baseline_executable_sha256"])')" f72a09a338df176121e69bda8cba8bb30ff11b4ef9666a9a796cf77c6a52f62e
chk "B pins: mutant executable" "$(git -C $R show $B:$CP/receipts/pins-final.json | python3 -c 'import json,sys;print(json.load(sys.stdin)["mutant_executable_sha256"])')" 5a4c4f17ea5b3883fb87a1a16fe19d53810608d4c229548a37932c405aa1fe8c
chk "B pins: mutant protocol blob" "$(git -C $R show $B:$CP/receipts/pins-final.json | python3 -c 'import json,sys;print(json.load(sys.stdin)["mutant_protocol_blob"])')" f3793c2ff61f8adbf43fe17d6b5237b49bc507b0
HX=/home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3
MX=/home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01-mutant/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3
chk "retained healthy executable sha256 (hashed, not run)" "$(sha256sum $HX | cut -d' ' -f1)" f72a09a338df176121e69bda8cba8bb30ff11b4ef9666a9a796cf77c6a52f62e
chk "retained mutant executable sha256 (hashed, not run)" "$(sha256sum $MX | cut -d' ' -f1)" 5a4c4f17ea5b3883fb87a1a16fe19d53810608d4c229548a37932c405aa1fe8c
echo
echo "[4] contract, binding reviews, AD-1-R3, disposition sheet (git objects and local copies)"
R3C=c2c69e3a8a5cc447cac8a86424a1600a1f23cecf
CT=docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract
chk "type $R3C" "$(git -C $R cat-file -t $R3C)" commit
chk "R3 commit parent" "$(git -C $R log -1 --format=%P $R3C)" 8693e3a742ee4d3917cbfdb809b377bcd919d9f4
chk "contract sha256 (at $R3C)" "$(gsha $R3C:$CT/P3-C11-D2A1-CONTRACT-02-R1.md)" cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c
chk "review-02 sha256" "$(gsha $R3C:$CT/P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md)" 6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce
chk "review-03-01 sha256" "$(gsha $R3C:$CT/P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md)" 1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35
chk "AD-1-R3 sha256 (at $R3C)" "$(gsha $R3C:docs/evidence/p3-candidate11/ad1r3-review-01/ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md)" 485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e
chk "AD-1-R3 blob" "$(git -C $R rev-parse $R3C:docs/evidence/p3-candidate11/ad1r3-review-01/ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md)" 7f2463f5421502cc6ee74c070fc3abc41d467770
chk "disposition sheet sha256 (at B)" "$(gsha $B:$CP/disposition/AD1R3-DISPOSITION-01-DRAFT.md)" a8a6d29376cf6f393a7eaa9047923719a339377caa554527a8878760cbdcf11c
C=$A/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/contract
chk "sealed local contract" "$(sha256sum $C/P3-C11-D2A1-CONTRACT-02-R1.md | cut -d' ' -f1)" cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c
chk "sealed local review-02" "$(sha256sum $C/P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md | cut -d' ' -f1)" 6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce
chk "sealed local review-03-01" "$(sha256sum $C/P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md | cut -d' ' -f1)" 1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35
for f in AD1-R3.md:485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e CONTRACT-02-R1.md:cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c \
         REVIEW-02.md:6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce REVIEW-03-01.md:1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35 \
         DISPOSITION-01.md:a8a6d29376cf6f393a7eaa9047923719a339377caa554527a8878760cbdcf11c C4-test1-removed.patch:24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d \
         controls_defs.py.txt:ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f; do
  chk "mission input copy inputs/${f%%:*}" "$(sha256sum $M/inputs/${f%%:*} | cut -d' ' -f1)" "${f##*:}"
done
echo
echo "[5] sealed local manifests (sha256sum -c, run from each directory)"
for d in p3-c11-dr01-d2a1-c4-witness-01 p3-c11-dr01-d2a1-ad1r3-readiness-01 p3-c11-dr01-d2a1-ad1r2-contract-evidence-01 p3-c11-dr01-d2a1-ad1r1-witness-audit-01 p3-c11-dr01-d2a1-ad1r1-mut-readiness-01 p3-c11-dr01-d2a1-campaign-01; do
  ( cd $A/$d && sha256sum --quiet -c SHA256SUMS ); rc=$?
  echo "    $d SHA256SUMS $(sha256sum $A/$d/SHA256SUMS | cut -d' ' -f1) entries $(wc -l < $A/$d/SHA256SUMS) exit $rc"
  [ $rc = 0 ] || { echo "    MISMATCH seal $d"; fail=1; }
done
chk "c4-witness-01 seal sha256" "$(sha256sum $A/p3-c11-dr01-d2a1-c4-witness-01/SHA256SUMS | cut -d' ' -f1)" 7b7849f50bb06174a39a2e3203d64fb63ddf3cbf44b9897fc113bc81f99fb579
D=$A/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py
chk "controls_defs.py sha256 (C10/C24 sealed definitions)" "$(sha256sum $D | cut -d' ' -f1)" ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f
chk "controls_defs.py listed in campaign-01 seal" "$(grep -c 'ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f  tools/stage5-prepared-NOT-EXECUTED/controls_defs.py' $A/p3-c11-dr01-d2a1-campaign-01/SHA256SUMS)" 1
echo
echo "[6] local repository state (Owner worktree; not altered)"
echo "    $(git -C $R rev-parse --abbrev-ref HEAD) $(git -C $R rev-parse HEAD) porcelain $(git -C $R status --porcelain | wc -l)"
echo "    C4 witness worktrees: $(git -C /home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01 rev-parse HEAD) porcelain $(git -C /home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01 status --porcelain | wc -l); $(git -C /home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01-mutant rev-parse HEAD) porcelain $(git -C /home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01-mutant status --porcelain | wc -l)"
echo
[ $fail = 0 ] && echo "STAGE 1 RESULT: ALL CHECKS OK" || echo "STAGE 1 RESULT: MISMATCH — STOP"
exit $fail
