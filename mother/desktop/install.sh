#!/usr/bin/env bash
# Mother's desktop, the installed build (M2b decision O9): build the release
# binary from this checkout with the locked dependencies, copy it to
# ~/.local/bin/nexus-mother, and print its SHA-256 as the build's receipt.
# It installs nothing else: no system package, no service, no desktop entry.
set -euo pipefail
export LC_ALL=C

fail() {
  echo "install: $1" >&2
  exit 1
}

case "${HOME:-}" in
  /*) ;;
  *) fail "HOME must be an absolute path" ;;
esac

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
[[ -f "$repo/Cargo.toml" && -f "$repo/Cargo.lock" ]] || fail "not a checkout: $repo"

# One absolute target directory, used for the build and for the copy: a
# relative CARGO_TARGET_DIR would mean one place to cargo (inside the
# checkout) and another to this script (the caller's directory).
if [[ -n "${CARGO_TARGET_DIR:-}" && "$CARGO_TARGET_DIR" != /* ]]; then
  fail "CARGO_TARGET_DIR must be an absolute path (it is: $CARGO_TARGET_DIR)"
fi
target="${CARGO_TARGET_DIR:-$repo/target}"
export CARGO_TARGET_DIR="$target"

(cd "$repo" && cargo build --release --locked -p nexus-mother-desktop --bin nexus-mother)

built="$target/release/nexus-mother"
[[ -f "$built" && ! -L "$built" ]] || fail "the release binary was not built: $built"

bin="$HOME/.local/bin"
dest="$bin/nexus-mother"
mkdir -p "$bin"
[[ -d "$bin" && ! -L "$bin" ]] || fail "$bin is not a directory"
[[ ! -L "$dest" ]] || fail "$dest is a symbolic link; refusing to write through it"

tmp="$(mktemp "$bin/.nexus-mother.XXXXXX")"
trap 'rm -f "$tmp"' EXIT
install -m 0755 "$built" "$tmp"
mv -f "$tmp" "$dest"
trap - EXIT

built_sum="$(sha256sum "$built" | cut -d' ' -f1)"
installed_sum="$(sha256sum "$dest" | cut -d' ' -f1)"
[[ "$built_sum" == "$installed_sum" ]] || fail "the installed copy differs from the build"

echo "installed: $dest"
echo "commit:    $(git -C "$repo" rev-parse HEAD)"
echo "sha256:    $installed_sum"
