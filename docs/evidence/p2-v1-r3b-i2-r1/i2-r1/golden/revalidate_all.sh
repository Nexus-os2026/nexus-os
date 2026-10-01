#!/usr/bin/env bash
# Revalidate every golden vector published in the committed candidate's
# tests/phase2_custody_codec.rs with the tools published beside this log,
# and reproduce the retained generator listings. Run from i2-r1/golden.
set -u
evidence_worktree=$1
scratch=$2
commit=$(git -C "$evidence_worktree" rev-parse HEAD)
path=crates/nexus-verifier-sandbox/tests/phase2_custody_codec.rs
base=0eb2a947e839b5639d9688da4e6b51b5e72a4d13
failed=0

step() {
  echo
  echo "=== \$ $*"
  "$@"
  local status=$?
  echo "=== exit $status"
  [ "$status" -eq 0 ] || failed=$((failed + 1))
}

same() {
  # same <label> <file a> <file b>: byte-identical, by sha256 and cmp.
  local a b
  a=$(sha256sum "$2" | cut -d' ' -f1)
  b=$(sha256sum "$3" | cut -d' ' -f1)
  if cmp -s "$2" "$3"; then
    echo "IDENTICAL  $1  ($a)"
  else
    echo "DIFFERENT  $1  ($a vs $b)"
    failed=$((failed + 1))
  fi
}

mkdir -p "$scratch"
git -C "$evidence_worktree" show "$commit:$path" > "$scratch/phase2_custody_codec.rs"
echo "# golden vector revalidation  $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "commit: $commit"
echo "file: $path"
echo "blob: $(git -C "$evidence_worktree" rev-parse "$commit:$path")"
echo "file sha256: $(sha256sum "$scratch/phase2_custody_codec.rs" | cut -d' ' -f1)"
echo "tools: $(python3 -V 2>&1); $(sha256sum --version | head -1)"
echo "tool sha256:"
sha256sum i2r1-golden-vectors.py i2r1-golden-revalidate.py golden_coreutils.sh ../../original-i2a/i2a-golden-vectors.py | sed 's/^/  /'

echo
echo "## 1. every published vector against the independent generator, re-hashed with hashlib"
step python3 -B i2r1-golden-revalidate.py "$scratch/phase2_custody_codec.rs"

echo
echo "## 2. every published frame re-hashed with GNU coreutils alone"
step bash golden_coreutils.sh "$scratch/phase2_custody_codec.rs"

echo
echo "## 3. the generator reproduces the retained listings byte for byte"
python3 -B i2r1-golden-vectors.py --with-receipts | grep -E '^[RQT] ' > "$scratch/all.txt"
same "i2r1-golden-vectors.py --with-receipts (R/Q/T lines) = i2r1-all-vectors.txt" "$scratch/all.txt" i2r1-all-vectors.txt
python3 -B i2r1-golden-vectors.py --with-receipts | grep -E '^T ' > "$scratch/receipts.txt"
same "i2r1-golden-vectors.py --with-receipts (T lines) = i2r1-receipts.txt" "$scratch/receipts.txt" i2r1-receipts.txt
python3 -B i2r1-golden-vectors.py --rust-receipts > "$scratch/receipts.rs.txt"
same "i2r1-golden-vectors.py --rust-receipts = i2r1-receipts.rs.txt" "$scratch/receipts.rs.txt" i2r1-receipts.rs.txt

echo
echo "## 4. the I2A vectors are unchanged by the I2-R1 extension"
python3 -B i2r1-golden-vectors.py --rust > "$scratch/i2r1-rust.txt"
python3 -B ../../original-i2a/i2a-golden-vectors.py --rust > "$scratch/i2a-rust.txt"
same "i2r1-golden-vectors.py --rust = i2a-golden-vectors.py --rust" "$scratch/i2r1-rust.txt" "$scratch/i2a-rust.txt"
same "i2a-golden-vectors.py --rust = original-i2a/i2a-vectors.rs.txt" "$scratch/i2a-rust.txt" ../../original-i2a/i2a-vectors.rs.txt
python3 -B ../../original-i2a/i2a-golden-vectors.py | grep -E '^[RQ] ' > "$scratch/i2a-all.txt"
same "i2a-golden-vectors.py (R/Q lines) = original-i2a/i2a-all.txt" "$scratch/i2a-all.txt" ../../original-i2a/i2a-all.txt
for table in RECORD_VECTORS REQUEST_VECTORS; do
  git -C "$evidence_worktree" show "$base:$path" | awk -v t="const $table:" 'index($0, t) == 1 {p = 1} p {print} p && /^];$/ {exit}' > "$scratch/$table.base"
  awk -v t="const $table:" 'index($0, t) == 1 {p = 1} p {print} p && /^];$/ {exit}' "$scratch/phase2_custody_codec.rs" > "$scratch/$table.candidate"
  echo "$table: $(grep -c 'digest: "' "$scratch/$table.candidate") entries in the candidate"
  same "$table at the candidate = $table at the base $base" "$scratch/$table.candidate" "$scratch/$table.base"
done

echo
echo "RESULT: $failed failure(s)"
[ "$failed" -eq 0 ]
