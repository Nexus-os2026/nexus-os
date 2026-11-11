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
# SB-3E A2 (design section 5, SB3E-R9.1, R30.4): the build runs in a fresh
# private target with -v, and before any copy the script refuses unless the
# compiler invocations Cargo really ran are the plain recipe's (a2_check).
# The active toolchain's rustc, which every compiler line must name:
rustc_path="$(cd "$repo" && rustc --print sysroot)/bin/rustc"
[[ "$rustc_path" == /* && -f "$rustc_path" ]] || fail "no active toolchain rustc: $rustc_path"

# SB-3E A2: CARGO_TARGET_DIR (or $repo/target) is the parent of a private
# target created here, so nothing a previous build left can be reused.
parent="${CARGO_TARGET_DIR:-$repo/target}"
target="$parent/nexus-mother-install"
log="$target/a2-build.log"
mkdir -p "$parent"
mkdir -m 0700 "$target" 2>/dev/null ||
  fail "the private target $target could not be created fresh (it exists: a planted or earlier build?); remove it and retry"
export CARGO_TARGET_DIR="$target"

# The compiler invocations that build is checked against: Cargo's -v lines.
# Every rustc line starts with the active toolchain's rustc (no wrapper, no
# other rustc, by any environment or configuration file); no argument begins
# with @; no mother_route_seams except the one reviewed --check-cfg argument
# on each of the three route packages' units (R9.1); no debug assertions;
# each route unit compiled once with the recipe's exact -C set; nothing Fresh.
a2_check() {
  A2_RUSTC="$1" A2_TARGET="$target" awk '
    function fail(why) { print why; bad = 1 }
    BEGIN {
      rustc = ENVIRON["A2_RUSTC"] " "; scripts = ENVIRON["A2_TARGET"] "/release/build/"
      h = "[0-9a-f]"; hex = h h h h h h h h h h h h h h h h
      unit["nexus_governed_control crates/nexus-governed-control/src/lib.rs"] = "lint lib"
      unit["nexus_mother mother/src/lib.rs"] = "lint lib"
      unit["nexus_mother_host mother/host/src/lib.rs"] = "lint lib"
      unit["nexus_mother_desktop mother/desktop/src/lib.rs"] = "lib"
      unit["nexus_mother mother/desktop/src/main.rs"] = "bin"
      flags["lib"] = "codegen-units=16 extra-filename=* linker-plugin-lto metadata=* opt-level=3 strip=symbols"
      flags["bin"] = "codegen-units=16 extra-filename=* lto=thin metadata=* opt-level=3 strip=symbols"
      exempt = " --check-cfg '\''cfg(mother_route_seams)'\''"
    }
    /\033/ { fail("a colored line") }
    /^ +Fresh / { fail("a Fresh unit: " $0) }
    index($0, "Running `") && !/^ +Running `[^`]*`$/ { fail("an unparsed Running line: " $0) }
    /^ +Running `/ {
      cmd = substr($0, index($0, "`") + 1); cmd = substr(cmd, 1, length(cmd) - 1)
      if (index(cmd, scripts) == 1 && substr(cmd, length(scripts) + 1) ~ ("^[A-Za-z0-9_-]+-" hex "/build-script-[A-Za-z0-9_-]+$")) next
      if (index(cmd, rustc) != 1) fail("a compiler line not starting with " rustc ": " cmd)
      if (cmd ~ /[ ][\047"]?@/) fail("an @ argument: " cmd)
      key = ""
      if (match(cmd, / --crate-name [^ ]+ --edition=[^ ]+ [^ ]+ /)) {
        split(substr(cmd, RSTART + 1, RLENGTH - 2), part, " "); key = part[2] " " part[4]
      }
      if (key in unit) {
        seen[key]++; kind = unit[key]
        if (kind == "lint lib") {
          at = index(cmd, exempt)
          if (at) cmd = substr(cmd, 1, at - 1) substr(cmd, at + length(exempt))
          kind = "lib"
        }
        if (cmd ~ / -C[^ ]/ || index(cmd, "--codegen")) fail("a -C spelling outside the recipe: " key)
        got = ""; rest = cmd
        while (match(rest, / -C [^ ]+/)) {
          c = substr(rest, RSTART + 4, RLENGTH - 4); rest = substr(rest, RSTART + RLENGTH)
          if (c ~ ("^metadata=" hex "$")) c = "metadata=*"
          if (c ~ ("^extra-filename=-" hex "$")) c = "extra-filename=*"
          list[++n] = c
        }
        for (i = 2; i <= n; i++) for (j = i; j > 1 && list[j - 1] > list[j]; j--) { t = list[j]; list[j] = list[j - 1]; list[j - 1] = t }
        for (i = 1; i <= n; i++) got = got (i > 1 ? " " : "") list[i]
        n = 0
        if (got != flags[kind]) fail("the -C set of " key " is not the recipe'\''s: " got)
      }
      $0 = cmd
    }
    { line = $0; while ((at = index(line, " -C debug-assertions=off"))) line = substr(line, 1, at - 1) substr(line, at + 24) }
    index(line, "mother_route_seams") { fail("mother_route_seams outside the reviewed --check-cfg: " $0) }
    index(line, "debug-assertions") { fail("debug assertions: " $0) }
    END {
      for (key in unit) if (seen[key] != 1) fail("route unit " key " compiled " (seen[key] + 0) " times")
      exit bad
    }' "$log"
}

(cd "$repo" && cargo build --release --locked -v --color never -p nexus-mother-desktop --bin nexus-mother) 2>&1 |
  tee "$log" >&2 || fail "the build failed; its log is $log (remove $target before retrying)"
if ! refusals="$(a2_check "$rustc_path")"; then
  # sed reads all of its input, so no SIGPIPE can end the script before
  # the refusal is reported (pipefail is set).
  printf '%s\n' "$refusals" | sed -n '1,20p' >&2
  fail "SB-3E A2 refused this build before any copy; its log is $log (remove $target before retrying)"
fi

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

mv -f "$log" "$parent/nexus-mother-install.log"
rm -rf -- "$target"

echo "installed: $dest"
echo "a2 log:    $parent/nexus-mother-install.log"
echo "commit:    $(git -C "$repo" rev-parse HEAD)"
echo "sha256:    $installed_sum"
