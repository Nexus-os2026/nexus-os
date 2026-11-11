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
final_log="$parent/nexus-mother-install.log"
mkdir -p "$parent"
# The log of an accepted build is kept at $final_log; refused up front unless
# that path is absent or a regular file (never a directory or a link).
if [[ -L "$final_log" || ( -e "$final_log" && ! -f "$final_log" ) ]]; then
  fail "the log path $final_log exists and is not a regular file; remove it and retry"
fi
if ! made="$(mkdir -m 0700 "$target" 2>&1)"; then
  fail "the private target $target could not be created fresh ($made): a planted or earlier build? remove it and retry"
fi
export CARGO_TARGET_DIR="$target"

# a2_check RUSTC TARGET LOG: the compiler invocations that build ran, from
# Cargo's -v lines, against the plain recipe. Every line of the log is
# classified: a complete Running line, Compiling, Finished, one of Cargo's
# download or lock status lines, a warning or rendered diagnostic, or blank;
# Fresh, any other line (a split Running line's continuation too), a
# carriage return and any colour refuse. Each Running line is split into its
# arguments as Cargo quotes them; quoting that does not close refuses (an
# argument holding a newline or backtick splits its line), as does any
# argument holding a backtick or carriage return. Every compiler line names
# the active toolchain's rustc (no wrapper, no other rustc, by any
# environment or configuration file); no argument begins with @ (a response
# file is refused, never expanded); -C is spelled only as Cargo spells it.
# Each route unit is compiled exactly once, with exactly the recipe's
# arguments after normalising the private target path to TARGET and Cargo's
# 16-hex unit hashes to *. Anywhere in the log, mother_route_seams refuses
# except the one reviewed --check-cfg argument on each of the three route
# packages' units (R9.1), and debug-assertions or debug_assertions refuses
# except Cargo's own `-C debug-assertions=off`. A compiler warning that
# quotes the route cfg therefore refuses too: fail closed.
a2_check() {
  local recipe
  IFS= read -r -d '' recipe <<'RECIPE' || :
lint-lib nexus_governed_control crates/nexus-governed-control/src/lib.rs
  --crate-name nexus_governed_control
  --edition=2021
  crates/nexus-governed-control/src/lib.rs
  --error-format=json
  --json=diagnostic-rendered-ansi,artifacts,future-incompat
  --crate-type lib
  --emit=dep-info,metadata,link
  -C opt-level=3
  -C linker-plugin-lto
  -C codegen-units=16
  --warn=unexpected_cfgs
  --check-cfg 'cfg(mother_route_seams)'
  --check-cfg 'cfg(docsrs,test)'
  --check-cfg 'cfg(feature, values())'
  -C metadata=*
  -C extra-filename=-*
  --out-dir TARGET/release/deps
  -C strip=symbols
  -L dependency=TARGET/release/deps
  --extern base64=TARGET/release/deps/libbase64-*.rmeta
  --extern getrandom=TARGET/release/deps/libgetrandom-*.rmeta
  --extern hex=TARGET/release/deps/libhex-*.rmeta
  --extern libc=TARGET/release/deps/liblibc-*.rmeta
  --extern nexus_kernel=TARGET/release/deps/libnexus_kernel-*.rmeta
  --extern png=TARGET/release/deps/libpng-*.rmeta
  --extern reqwest=TARGET/release/deps/libreqwest-*.rmeta
  --extern rusqlite=TARGET/release/deps/librusqlite-*.rmeta
  --extern serde=TARGET/release/deps/libserde-*.rmeta
  --extern serde_json=TARGET/release/deps/libserde_json-*.rmeta
  --extern sha2=TARGET/release/deps/libsha2-*.rmeta
  --extern tokio=TARGET/release/deps/libtokio-*.rmeta
  --extern unicode_segmentation=TARGET/release/deps/libunicode_segmentation-*.rmeta
  --extern url=TARGET/release/deps/liburl-*.rmeta
  --extern x11rb=TARGET/release/deps/libx11rb-*.rmeta
  --extern zeroize=TARGET/release/deps/libzeroize-*.rmeta
  -L native=TARGET/release/build/libsqlite3-sys-*/out
  -L native=TARGET/release/build/ring-*/out
lint-lib nexus_mother mother/src/lib.rs
  --crate-name nexus_mother
  --edition=2021
  mother/src/lib.rs
  --error-format=json
  --json=diagnostic-rendered-ansi,artifacts,future-incompat
  --crate-type lib
  --emit=dep-info,metadata,link
  -C opt-level=3
  -C linker-plugin-lto
  -C codegen-units=16
  --warn=unexpected_cfgs
  --check-cfg 'cfg(mother_route_seams)'
  --check-cfg 'cfg(docsrs,test)'
  --check-cfg 'cfg(feature, values())'
  -C metadata=*
  -C extra-filename=-*
  --out-dir TARGET/release/deps
  -C strip=symbols
  -L dependency=TARGET/release/deps
  --extern hex=TARGET/release/deps/libhex-*.rmeta
  --extern nexus_governed_control=TARGET/release/deps/libnexus_governed_control-*.rmeta
  --extern nix=TARGET/release/deps/libnix-*.rmeta
  --extern serde=TARGET/release/deps/libserde-*.rmeta
  --extern serde_json=TARGET/release/deps/libserde_json-*.rmeta
  --extern sha2=TARGET/release/deps/libsha2-*.rmeta
  -L native=TARGET/release/build/libsqlite3-sys-*/out
  -L native=TARGET/release/build/ring-*/out
lint-lib nexus_mother_host mother/host/src/lib.rs
  --crate-name nexus_mother_host
  --edition=2021
  mother/host/src/lib.rs
  --error-format=json
  --json=diagnostic-rendered-ansi,artifacts,future-incompat
  --crate-type lib
  --emit=dep-info,metadata,link
  -C opt-level=3
  -C linker-plugin-lto
  -C codegen-units=16
  --warn=unexpected_cfgs
  --check-cfg 'cfg(mother_route_seams)'
  --check-cfg 'cfg(docsrs,test)'
  --check-cfg 'cfg(feature, values())'
  -C metadata=*
  -C extra-filename=-*
  --out-dir TARGET/release/deps
  -C strip=symbols
  -L dependency=TARGET/release/deps
  --extern nexus_governed_control=TARGET/release/deps/libnexus_governed_control-*.rmeta
  --extern nexus_mother=TARGET/release/deps/libnexus_mother-*.rmeta
  --extern nix=TARGET/release/deps/libnix-*.rmeta
  --extern sha2=TARGET/release/deps/libsha2-*.rmeta
  --extern uuid=TARGET/release/deps/libuuid-*.rmeta
  -L native=TARGET/release/build/libsqlite3-sys-*/out
  -L native=TARGET/release/build/ring-*/out
lib nexus_mother_desktop mother/desktop/src/lib.rs
  --crate-name nexus_mother_desktop
  --edition=2021
  mother/desktop/src/lib.rs
  --error-format=json
  --json=diagnostic-rendered-ansi,artifacts,future-incompat
  --crate-type lib
  --emit=dep-info,metadata,link
  -C opt-level=3
  -C linker-plugin-lto
  -C codegen-units=16
  --forbid=unsafe_code
  --check-cfg 'cfg(docsrs,test)'
  --check-cfg 'cfg(feature, values())'
  -C metadata=*
  -C extra-filename=-*
  --out-dir TARGET/release/deps
  -C strip=symbols
  -L dependency=TARGET/release/deps
  --extern gtk4=TARGET/release/deps/libgtk4-*.rmeta
  --extern nexus_governed_control=TARGET/release/deps/libnexus_governed_control-*.rmeta
  --extern nexus_kernel=TARGET/release/deps/libnexus_kernel-*.rmeta
  --extern nexus_mother_host=TARGET/release/deps/libnexus_mother_host-*.rmeta
  -L native=TARGET/release/build/libsqlite3-sys-*/out
  -L native=TARGET/release/build/ring-*/out
bin nexus_mother mother/desktop/src/main.rs
  --crate-name nexus_mother
  --edition=2021
  mother/desktop/src/main.rs
  --error-format=json
  --json=diagnostic-rendered-ansi,artifacts,future-incompat
  --crate-type bin
  --emit=dep-info,link
  -C opt-level=3
  -C lto=thin
  -C codegen-units=16
  --forbid=unsafe_code
  --check-cfg 'cfg(docsrs,test)'
  --check-cfg 'cfg(feature, values())'
  -C metadata=*
  -C extra-filename=-*
  --out-dir TARGET/release/deps
  -C strip=symbols
  -L dependency=TARGET/release/deps
  --extern gtk4=TARGET/release/deps/libgtk4-*.rlib
  --extern nexus_governed_control=TARGET/release/deps/libnexus_governed_control-*.rlib
  --extern nexus_kernel=TARGET/release/deps/libnexus_kernel-*.rlib
  --extern nexus_mother_desktop=TARGET/release/deps/libnexus_mother_desktop-*.rlib
  --extern nexus_mother_host=TARGET/release/deps/libnexus_mother_host-*.rlib
  -L native=TARGET/release/build/libsqlite3-sys-*/out
  -L native=TARGET/release/build/ring-*/out
RECIPE
  A2_RECIPE="$recipe" A2_RUSTC="$1" A2_TARGET="$2" awk '
    function fail(why) { print why; bad = 1 }
    # Cargo prints each argument shell-escaped: plain, or in single quotes,
    # with a backslash escaping a quote outside them. Splits CMD into raw[]
    # (as printed) and val[] (unquoted); returns the count, or -1 if the
    # quoting does not close or a word is empty (a doubled or edge space).
    function words(cmd,   n, i, len, c, q, w, r) {
      n = 0; i = 1; len = length(cmd)
      while (i <= len) {
        w = ""; r = ""; q = 0
        for (; i <= len; i++) {
          c = substr(cmd, i, 1)
          if (q) { r = r c; if (c == "\047") q = 0; else w = w c; continue }
          if (c == " ") break
          r = r c
          if (c == "\047") q = 1
          else if (c == "\\") { if (i == len) return -1; i++; c = substr(cmd, i, 1); r = r c; w = w c }
          else w = w c
        }
        if (q || r == "") return -1
        n++; raw[n] = r; val[n] = w; i++
      }
      if (substr(cmd, len, 1) == " ") return -1
      return n
    }
    BEGIN {
      rustc = ENVIRON["A2_RUSTC"]; target = ENVIRON["A2_TARGET"]; scripts = target "/release/build/"
      h = "[0-9a-f]"; hex = h h h h h h h h h h h h h h h h
      exempt = " --check-cfg \047cfg(mother_route_seams)\047"
      n = split(ENVIRON["A2_RECIPE"], line, "\n")
      for (i = 1; i <= n; i++) {
        if (line[i] == "") continue
        if (substr(line[i], 1, 2) == "  ") { pin[key] = pin[key] (pin[key] == "" ? "" : " ") substr(line[i], 3); continue }
        split(line[i], part, " "); key = part[2] " " part[3]; kind[key] = part[1]; crate[part[2]] = 1
      }
    }
    { text = $0; lint = 0 }
    index(text, "\r") { fail("a carriage return: " text) }
    /\033/ { fail("a colored line") }
    /^ +Running `/ {
      if (text !~ /^ +Running `[^`]*`$/) { fail("an unclassified line (a split Running line?): " text); next }
      cmd = substr(text, index(text, "`") + 1); cmd = substr(cmd, 1, length(cmd) - 1)
      argc = words(cmd)
      if (argc < 1) { fail("an argument with a newline, backtick or carriage return (its quoting does not close): " cmd); next }
      for (i = 1; i <= argc; i++) if (val[i] ~ /[`\r]/) fail("an argument with a backtick or carriage return: " cmd)
      if (argc == 1 && index(val[1], scripts) == 1 && substr(val[1], length(scripts) + 1) ~ ("^[A-Za-z0-9_-]+-" hex "/build-script-[A-Za-z0-9_-]+$")) next
      if (val[1] != rustc) fail("a compiler line not starting with " rustc ": " cmd)
      for (i = 2; i <= argc; i++) {
        if (val[i] ~ /^@/) fail("an @ argument: " cmd)
        if (val[i] ~ /^-C./ || val[i] ~ /^--codegen/) fail("a -C spelling outside the recipe: " cmd)
      }
      key = (val[2] == "--crate-name" && val[4] ~ /^--edition=/) ? val[3] " " val[5] : ""
      if (val[3] in crate && !(key in pin)) fail("a route crate compiled outside the recipe: " cmd)
      if (key in pin) {
        seen[key]++; lint = kind[key] == "lint-lib"; got = ""
        for (i = 2; i <= argc; i++) {
          w = raw[i]
          while ((at = index(w, target))) w = substr(w, 1, at - 1) "TARGET" substr(w, at + length(target))
          gsub(hex, "*", w); got = got (i > 2 ? " " : "") w
        }
        if (got != pin[key]) fail("the arguments of " key " are not the recipe'\''s: " got)
      }
      text = cmd
    }
    !/^ +Running `/ {
      if (/^ +Fresh /) fail("a Fresh unit: " text)
      else if (!(text == "" || /^ +Compiling [^ ]+ v[0-9][^ ]*( \(.*\))?$/ ||
          /^ +Finished `release` profile \[optimized\] target\(s\) in [0-9][0-9.ms ]*$/ ||
          /^ +(Updating crates\.io index|Downloading crates \.\.\.)$/ || /^ +Downloaded [^ ]+ v[0-9][^ ]*$/ ||
          /^ +Downloaded [0-9]+ crates? \([^)]*\) in [0-9][0-9.ms ]*$/ || /^ +Blocking waiting for file lock on [a-z ]+$/ ||
          /^warning: / || /^ *--> / || /^ *[0-9]* *\|/ || /^ *= (note|help): /))
        fail("an unclassified line: " text)
    }
    {
      if (lint && (at = index(text, exempt))) text = substr(text, 1, at - 1) substr(text, at + length(exempt))
      while ((at = index(text, " -C debug-assertions=off"))) text = substr(text, 1, at - 1) substr(text, at + 24)
      if (index(text, "mother_route_seams")) fail("mother_route_seams outside the reviewed --check-cfg: " $0)
      if (text ~ /debug[-_]assertions/) fail("debug assertions: " $0)
    }
    END {
      for (key in pin) if (seen[key] != 1) fail("route unit " key " compiled " (seen[key] + 0) " times")
      exit bad
    }' "$3"
}

(cd "$repo" && cargo build --release --locked -v --color never -p nexus-mother-desktop --bin nexus-mother) 2>&1 |
  tee "$log" >&2 || fail "the build failed; its log is $log (remove $target before retrying)"
if ! refusals="$(a2_check "$rustc_path" "$target" "$log")"; then
  # sed reads all of its input, so no SIGPIPE can end the script before
  # the refusal is reported (pipefail is set).
  printf '%s\n' "$refusals" | sed -n '1,20p' >&2
  fail "SB-3E A2 refused this build before any copy; its log is $log (remove $target before retrying)"
fi

built="$target/release/nexus-mother"
[[ -f "$built" && ! -L "$built" ]] || fail "the release binary was not built: $built"

# The accepted build's log is kept before anything is copied.
mv -f "$log" "$final_log" || fail "could not keep the A2 log at $final_log"

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
echo "a2 log:    $final_log"
echo "commit:    $(git -C "$repo" rev-parse HEAD)"
echo "sha256:    $installed_sum"
# The binary is installed and reported; a failure to remove the private
# target is still reported, and the script exits nonzero.
rm -rf -- "$target" || fail "installed, but the private target $target could not be removed"
