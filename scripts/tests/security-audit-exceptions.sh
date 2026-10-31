#!/usr/bin/env bash
# Hermetic test of scripts/security-audit.sh's handling of deny.toml's exception
# list (M0-FIX2 F12). It copies the script into a scratch checkout and puts a
# recording `cargo` shim on PATH, so no scanner or network is needed:
# - an EMPTY exception list is accepted (zero ignores is the strictest state),
#   no --ignore is passed, and both scanners still run with the stale-exception
#   check (-D advisory-not-detected);
# - a listed ID is passed to cargo-audit as --ignore;
# - a scanner failure (for example cargo-deny reporting a stale exception or an
#   unignored vulnerability) makes the script fail.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

mkdir -p "$tmp/scripts" "$tmp/bin"
cp "$here/security-audit.sh" "$tmp/scripts/security-audit.sh"
: > "$tmp/Cargo.lock"
cat > "$tmp/bin/cargo" <<'SHIM'
#!/usr/bin/env bash
echo "$*" >> "$SHIM_LOG"
case "$1" in
    audit)
        if [ "${2:-}" = "--version" ]; then echo "cargo-audit-audit 0.22.1"; exit 0; fi
        exit "${SHIM_AUDIT_STATUS:-0}" ;;
    deny)
        if [ "${2:-}" = "--version" ]; then echo "cargo-deny 0.19.6"; exit 0; fi
        exit "${SHIM_DENY_STATUS:-0}" ;;
esac
exit 0
SHIM
chmod +x "$tmp/bin/cargo"

run() { # run <label>; env SHIM_* set by caller; sets status and out
    : > "$tmp/log"
    set +e
    out="$(cd "$tmp" && PATH="$tmp/bin:$PATH" SHIM_LOG="$tmp/log" bash scripts/security-audit.sh --no-fetch 2>&1)"
    status=$?
    set -e
}

# 1. Empty exception list: accepted.
printf '[advisories]\nignore = [\n]\n' > "$tmp/deny.toml"
run
[ "$status" -eq 0 ] || fail "empty exception list rejected (status $status): $out"
grep -q -- '--ignore' "$tmp/log" && fail "empty list still passed --ignore"
grep -q '^audit --file Cargo.lock' "$tmp/log" || fail "cargo-audit did not run on an empty list"
grep -q -- '^deny --locked check .*-D advisory-not-detected' "$tmp/log" \
    || fail "cargo-deny lost its stale-exception check on an empty list"

# 2. A listed exception is passed through to cargo-audit.
printf '[advisories]\nignore = [\n    { id = "RUSTSEC-2099-0001", reason = "test" },\n]\n' > "$tmp/deny.toml"
run
[ "$status" -eq 0 ] || fail "a valid exception list failed (status $status): $out"
grep -q -- '--ignore RUSTSEC-2099-0001' "$tmp/log" || fail "listed ID not passed to cargo-audit"

# 3. A stale exception / unignored vulnerability (scanner failure) fails the gate.
SHIM_DENY_STATUS=1 run
[ "$status" -ne 0 ] || fail "a cargo-deny failure (stale exception) was masked"
SHIM_AUDIT_STATUS=1 run
[ "$status" -ne 0 ] || fail "a cargo-audit failure (vulnerability) was masked"
printf '[advisories]\nignore = [\n]\n' > "$tmp/deny.toml"
SHIM_DENY_STATUS=1 run
[ "$status" -ne 0 ] || fail "a cargo-deny failure was masked on an empty list"

# 4. A malformed entry (an advisory ID not in the canonical `id = "..."` form,
#    which cargo-deny might honour but cargo-audit would not receive) is
#    rejected rather than silently ignored.
printf '[advisories]\nignore = ["RUSTSEC-2099-0001"]\n' > "$tmp/deny.toml"
run
[ "$status" -ne 0 ] || fail "a malformed exception entry was accepted"
grep -q '^audit --file' "$tmp/log" && fail "scanners ran despite a malformed exception entry"
# A mention in a comment is not an entry.
printf '# RUSTSEC-2099-0001 was removed\n[advisories]\nignore = []\n' > "$tmp/deny.toml"
run
[ "$status" -eq 0 ] || fail "a commented advisory ID was treated as an entry (status $status): $out"

if [ "$failures" -ne 0 ]; then echo "security-audit-exceptions: $failures failure(s)" >&2; exit 1; fi
echo "security-audit-exceptions: ok"
