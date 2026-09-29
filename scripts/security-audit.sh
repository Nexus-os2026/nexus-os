#!/usr/bin/env bash
# Linux security gate for Rust dependencies (P0-LINUX-FINAL-R1, repair B).
#
# Runs the pinned cargo-audit and cargo-deny on this checkout's Cargo.lock
# with the one exception set in deny.toml. Exits non-zero when:
# - either scanner is missing or is not the pinned version;
# - either scanner fails for any reason (no failure is masked);
# - a vulnerability not in deny.toml's exception set is found;
# - an exception in deny.toml no longer matches anything (stale);
# - cargo-deny's bans, licenses or sources checks report an error.
# Unmaintained, unsound and yanked findings are printed as warnings by both
# scanners.
#
# Usage:
#   scripts/security-audit.sh            # fetch the advisory databases, scan
#   scripts/security-audit.sh --install  # first install the pinned scanners
#                                        # (cargo install --locked)
# Offline use (for example with a reviewed database snapshot):
#   SECURITY_AUDIT_DB_ROOT=DIR scripts/security-audit.sh --no-fetch
#   where DIR holds cargo-audit's `advisory-db` clone and cargo-deny's
#   `advisory-dbs` directory. Without --no-fetch both are fetched into DIR.
set -euo pipefail

readonly CARGO_AUDIT_VERSION="0.22.1"
readonly CARGO_DENY_VERSION="0.19.6"

install=0
fetch=1
for arg in "$@"; do
    case "$arg" in
        --install) install=1 ;;
        --no-fetch) fetch=0 ;;
        *) echo "security-audit: unknown argument: $arg" >&2; exit 2 ;;
    esac
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [ "$install" -eq 1 ]; then
    cargo install cargo-audit --version "$CARGO_AUDIT_VERSION" --locked
    cargo install cargo-deny --version "$CARGO_DENY_VERSION" --locked
fi

audit_version="$(cargo audit --version)"
deny_version="$(cargo deny --version)"
case "$audit_version" in
    *" $CARGO_AUDIT_VERSION") ;;
    *) echo "security-audit: cargo-audit $CARGO_AUDIT_VERSION required, found: $audit_version" >&2; exit 1 ;;
esac
case "$deny_version" in
    *" $CARGO_DENY_VERSION") ;;
    *) echo "security-audit: cargo-deny $CARGO_DENY_VERSION required, found: $deny_version" >&2; exit 1 ;;
esac

# The exception set: the `id = "RUSTSEC-..."` entries of deny.toml.
mapfile -t accepted < <(grep -oE 'id = "RUSTSEC-[0-9]{4}-[0-9]{4}"' deny.toml | grep -oE 'RUSTSEC-[0-9]{4}-[0-9]{4}')
if [ "${#accepted[@]}" -eq 0 ]; then
    echo "security-audit: no exception entries found in deny.toml" >&2
    exit 1
fi
echo "security-audit: cargo-audit $CARGO_AUDIT_VERSION, cargo-deny $CARGO_DENY_VERSION"
echo "security-audit: accepted advisories (deny.toml): ${accepted[*]}"

audit_args=(--file Cargo.lock)
for id in "${accepted[@]}"; do
    audit_args+=(--ignore "$id")
done
deny_config="deny.toml"
deny_fetch_args=()
if [ -n "${SECURITY_AUDIT_DB_ROOT:-}" ]; then
    audit_args+=(--db "$SECURITY_AUDIT_DB_ROOT/advisory-db")
    # cargo-deny reads its database location only from its configuration: a
    # temporary copy of deny.toml adds it and changes nothing else.
    deny_config="$(mktemp)"
    trap 'rm -f "$deny_config"' EXIT
    sed "s|^\[advisories\]$|[advisories]\ndb-path = \"$SECURITY_AUDIT_DB_ROOT/advisory-dbs\"|" deny.toml > "$deny_config"
fi
if [ "$fetch" -eq 0 ]; then
    audit_args+=(--no-fetch)
    deny_fetch_args+=(--disable-fetch)
fi

echo "security-audit: cargo audit ${audit_args[*]}"
cargo audit "${audit_args[@]}"

echo "security-audit: cargo deny --locked check --config $deny_config"
cargo deny --locked check --config "$deny_config" "${deny_fetch_args[@]}" \
    -W unmaintained -W unsound -D advisory-not-detected

echo "security-audit: passed"
