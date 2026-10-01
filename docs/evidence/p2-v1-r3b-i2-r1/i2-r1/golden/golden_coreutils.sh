#!/usr/bin/env bash
# Re-hash every published golden frame with GNU coreutils alone, independent
# of Python and of the Rust code under test. Each literal
#     frame: "<hex>",
#     digest: "<64 hex>",
# pair in the given test file (the digest on the line after its frame) is
# decoded with basenc and hashed with sha256sum; the result must equal the
# published digest. Exit status 0 only if every pair matches.
#
# Usage: golden_coreutils.sh <path to phase2_custody_codec.rs>
set -u
file=$1
total=0
matched=0
while IFS=' ' read -r frame digest; do
  total=$((total + 1))
  got=$(printf '%s' "$frame" | tr 'a-f' 'A-F' | basenc --base16 -d | sha256sum | cut -d' ' -f1)
  if [ "$got" = "$digest" ]; then
    matched=$((matched + 1))
  else
    echo "mismatch: frame $frame hashes to $got, published $digest"
  fi
done < <(awk '
  match($0, /^ *frame: "[0-9a-f ]+",$/) {
    frame = $0
    sub(/^ *frame: "/, "", frame)
    sub(/",$/, "", frame)
    gsub(/ /, "", frame)
    next
  }
  frame != "" && match($0, /^ *digest: "[0-9a-f]+",$/) {
    digest = $0
    sub(/^ *digest: "/, "", digest)
    sub(/",$/, "", digest)
    print frame, digest
  }
  { frame = "" }
' "$file")
echo "$(sha256sum --version | head -1), $(basenc --version | head -1)"
echo "frames re-hashed with coreutils: $matched of $total match"
[ "$total" -gt 0 ] && [ "$matched" -eq "$total" ]
