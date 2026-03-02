#!/usr/bin/env bash
# Refresh Rust security audit toolchain and advisory DB, then rerun audits.
# - Detects installed cargo-audit and cargo-deny versions
# - Upgrades both tools to latest crates.io releases (CVSS v4-compatible path)
# - Clears and refreshes advisory database
# - Retries operations for transient network failures
# - Emits machine-readable JSON summary at the end

set -u
set -o pipefail

SCRIPT_NAME="$(basename "$0")"
WORKDIR="${WORKDIR:-$(pwd)}"
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
ADVISORY_DB_DIR="${ADVISORY_DB_DIR:-}"
TMP_DIR="$(mktemp -d -t rust-audit-refresh.XXXXXX)"
MAX_RETRIES="${MAX_RETRIES:-3}"
SLEEP_SECONDS_BASE="${SLEEP_SECONDS_BASE:-2}"

# Track summary fields for the final JSON object.
SYSTEM_UPGRADED=false
AUDIT_STATUS="not_run"
DENY_STATUS="not_run"
AUDIT_EXIT_CODE=0
DENY_EXIT_CODE=0
AUDIT_BEFORE=""
AUDIT_AFTER=""
DENY_BEFORE=""
DENY_AFTER=""
INSTALL_AUDIT_OK=false
INSTALL_DENY_OK=false

AUDIT_LOG="$TMP_DIR/cargo_audit.log"
DENY_LOG="$TMP_DIR/cargo_deny.log"
INSTALL_AUDIT_LOG="$TMP_DIR/install_cargo_audit.log"
INSTALL_DENY_LOG="$TMP_DIR/install_cargo_deny.log"
FETCH_DENY_LOG="$TMP_DIR/fetch_deny.log"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

log() {
  # UTC timestamp keeps logs comparable across Linux/macOS hosts.
  printf '[%s] [%s] %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$1" "$2" >&2
}

log_info() { log "INFO" "$1"; }
log_warn() { log "WARN" "$1"; }
log_error() { log "ERROR" "$1"; }

sanitize_output() {
  # Redact common credential patterns before printing logs.
  sed -E \
    -e 's#(https?://)[^/@[:space:]]+:[^/@[:space:]]+@#\1***:***@#g' \
    -e 's/([Pp]assword|[Tt]oken|[Ss]ecret)=([^[:space:]]+)/\1=***REDACTED***/g'
}

show_log_excerpt() {
  local file="$1"
  if [ -f "$file" ]; then
    log_error "Last log lines from $file:"
    # Trim very long lines to keep CI logs readable.
    tail -n 20 "$file" | cut -c1-400 | sanitize_output >&2
  fi
}

classify_and_log_failure() {
  local desc="$1"
  local file="$2"

  if [ ! -f "$file" ]; then
    log_error "$desc failed (no log available)."
    return
  fi

  if grep -Eqi 'timed out|timeout|could not resolve host|network|tls|ssl|connection|temporary failure|failed to download' "$file"; then
    log_error "$desc failed due to a likely network/TLS issue."
  elif grep -Eqi 'linker|rustc|toolchain|no such command|command not found|failed to compile|could not compile|error: package .* is not installed' "$file"; then
    log_error "$desc failed due to a likely Rust toolchain/setup issue."
  else
    log_error "$desc failed."
  fi

  show_log_excerpt "$file"
}

json_escape() {
  # Escape a string for JSON value usage.
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e ':a;N;$!ba;s/\n/\\n/g'
}

json_string_or_null() {
  local val="$1"
  if [ -z "$val" ]; then
    printf 'null'
  else
    printf '"%s"' "$(json_escape "$val")"
  fi
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

extract_semver() {
  # Extract first x.y.z-like version from any version string.
  printf '%s' "$1" | sed -nE 's/.*([0-9]+\.[0-9]+\.[0-9]+).*/\1/p' | head -n 1
}

get_subcommand_version() {
  # Return semantic version for cargo subcommands (audit/deny), or empty string.
  local subcmd="$1"
  local raw
  if ! raw="$(cargo "$subcmd" --version 2>/dev/null)"; then
    printf ''
    return 1
  fi

  extract_semver "$raw"
}

run_with_retries() {
  # run_with_retries <description> <logfile> <cmd...>
  local desc="$1"
  local logfile="$2"
  shift 2

  : > "$logfile"

  local attempt=1
  local rc=0
  while [ "$attempt" -le "$MAX_RETRIES" ]; do
    "$@" >>"$logfile" 2>&1
    rc=$?
    if [ "$rc" -eq 0 ]; then
      return 0
    fi

    if [ "$attempt" -lt "$MAX_RETRIES" ]; then
      local backoff=$((SLEEP_SECONDS_BASE * attempt))
      log_warn "$desc failed (attempt $attempt/$MAX_RETRIES, exit $rc). Retrying in ${backoff}s..."
      sleep "$backoff"
    fi

    attempt=$((attempt + 1))
  done

  classify_and_log_failure "$desc" "$logfile"
  return "$rc"
}

safe_clear_dir() {
  local dir="$1"

  if [ -z "$dir" ] || [ "$dir" = "/" ]; then
    log_error "Refusing to clear unsafe directory path: '$dir'"
    return 1
  fi

  rm -rf "$dir"
  mkdir -p "$dir"
}

collect_rustsec_ids() {
  # Pull unique advisory IDs from a log or json file.
  local file="$1"
  if [ ! -f "$file" ]; then
    return 0
  fi

  grep -Eo 'RUSTSEC-[0-9]{4}-[0-9]{4}' "$file" | sort -u
  return 0
}

emit_summary_json() {
  local remaining_ids_file="$TMP_DIR/remaining_ids.txt"
  {
    collect_rustsec_ids "$AUDIT_LOG"
    collect_rustsec_ids "$DENY_LOG"
  } | sort -u > "$remaining_ids_file"

  local remaining_count
  remaining_count="$(wc -l < "$remaining_ids_file" | tr -d '[:space:]')"

  printf '{\n'
  printf '  "system_upgraded": %s,\n' "$SYSTEM_UPGRADED"
  printf '  "upgrades": {\n'
  printf '    "cargo_audit": {"before": %s, "after": %s, "upgraded": %s, "install_ok": %s},\n' \
    "$(json_string_or_null "$AUDIT_BEFORE")" \
    "$(json_string_or_null "$AUDIT_AFTER")" \
    "$( [ "$AUDIT_BEFORE" != "$AUDIT_AFTER" ] && printf 'true' || printf 'false' )" \
    "$INSTALL_AUDIT_OK"
  printf '    "cargo_deny": {"before": %s, "after": %s, "upgraded": %s, "install_ok": %s}\n' \
    "$(json_string_or_null "$DENY_BEFORE")" \
    "$(json_string_or_null "$DENY_AFTER")" \
    "$( [ "$DENY_BEFORE" != "$DENY_AFTER" ] && printf 'true' || printf 'false' )" \
    "$INSTALL_DENY_OK"
  printf '  },\n'
  printf '  "audit_results": {\n'
  printf '    "cargo_audit": {"status": %s, "exit_code": %s},\n' "$(json_string_or_null "$AUDIT_STATUS")" "$AUDIT_EXIT_CODE"
  printf '    "cargo_deny": {"status": %s, "exit_code": %s}\n' "$(json_string_or_null "$DENY_STATUS")" "$DENY_EXIT_CODE"
  printf '  },\n'
  printf '  "remaining_advisories": {\n'
  printf '    "count": %s,\n' "$remaining_count"
  printf '    "ids": ['

  local first=true
  while IFS= read -r id; do
    [ -z "$id" ] && continue
    if [ "$first" = true ]; then
      first=false
    else
      printf ', '
    fi
    printf '"%s"' "$(json_escape "$id")"
  done < "$remaining_ids_file"

  printf ']\n'
  printf '  }\n'
  printf '}\n'
}

main() {
  log_info "Starting Rust security toolchain refresh in $WORKDIR"

  # Ensure a writable cargo home for install/cache/database operations.
  if ! mkdir -p "$CARGO_HOME_DIR" 2>/dev/null || \
     ! (mkdir -p "$CARGO_HOME_DIR/.audit-write-test.$$" 2>/dev/null && rmdir "$CARGO_HOME_DIR/.audit-write-test.$$" 2>/dev/null); then
    CARGO_HOME_DIR="$TMP_DIR/cargo-home"
    mkdir -p "$CARGO_HOME_DIR"
    log_warn "Configured cargo home is not writable. Falling back to temporary cargo home: $CARGO_HOME_DIR"
  fi
  export CARGO_HOME="$CARGO_HOME_DIR"

  if [ -z "$ADVISORY_DB_DIR" ]; then
    ADVISORY_DB_DIR="$CARGO_HOME/advisory-db"
  fi

  if ! command_exists cargo; then
    log_error "cargo is not installed or not in PATH. Install Rust toolchain first: https://rustup.rs"
    AUDIT_STATUS="error"
    DENY_STATUS="error"
    AUDIT_EXIT_CODE=127
    DENY_EXIT_CODE=127
    emit_summary_json
    return 1
  fi

  # Detect current versions before any changes.
  AUDIT_BEFORE="$(get_subcommand_version audit || true)"
  DENY_BEFORE="$(get_subcommand_version deny || true)"

  log_info "Detected cargo-audit version: ${AUDIT_BEFORE:-not installed}"
  log_info "Detected cargo-deny version: ${DENY_BEFORE:-not installed}"

  # Force-install latest releases. This is idempotent (safe to rerun) and ensures
  # parser compatibility with newer advisory metadata (including CVSS v4 format).
  if run_with_retries "Installing/upgrading cargo-audit" "$INSTALL_AUDIT_LOG" \
    cargo install --locked --force cargo-audit; then
    INSTALL_AUDIT_OK=true
  else
    INSTALL_AUDIT_OK=false
  fi

  if run_with_retries "Installing/upgrading cargo-deny" "$INSTALL_DENY_LOG" \
    cargo install --locked --force cargo-deny; then
    INSTALL_DENY_OK=true
  else
    INSTALL_DENY_OK=false
  fi

  # Re-check versions after installation attempt.
  AUDIT_AFTER="$(get_subcommand_version audit || true)"
  DENY_AFTER="$(get_subcommand_version deny || true)"

  if [ "$AUDIT_BEFORE" != "$AUDIT_AFTER" ] || [ "$DENY_BEFORE" != "$DENY_AFTER" ]; then
    SYSTEM_UPGRADED=true
  fi

  log_info "Post-upgrade cargo-audit version: ${AUDIT_AFTER:-unavailable}"
  log_info "Post-upgrade cargo-deny version: ${DENY_AFTER:-unavailable}"

  # If install failed, continue to attempt audit with whatever is available, but log clearly.
  if [ "$INSTALL_AUDIT_OK" != true ]; then
    log_warn "cargo-audit upgrade did not succeed; attempting audit with current binary state."
  fi
  if [ "$INSTALL_DENY_OK" != true ]; then
    log_warn "cargo-deny upgrade did not succeed; attempting deny check with current binary state."
  fi

  # Clear advisory DB safely and recreate directory to force clean refresh.
  log_info "Clearing advisory database directory: $ADVISORY_DB_DIR"
  if ! safe_clear_dir "$ADVISORY_DB_DIR"; then
    log_error "Could not reset advisory database directory."
  fi

  # Refresh advisory sources explicitly for cargo-deny.
  if command_exists cargo-deny || cargo deny --version >/dev/null 2>&1; then
    if ! run_with_retries "Refreshing advisories with cargo-deny" "$FETCH_DENY_LOG" \
      cargo deny fetch db; then
      log_warn "cargo deny advisory refresh failed; continuing to run checks for visibility."
    fi
  fi

  # Re-run cargo audit and capture machine-readable output.
  if run_with_retries "Running cargo audit" "$AUDIT_LOG" \
    cargo audit --db "$ADVISORY_DB_DIR" --json; then
    AUDIT_STATUS="pass"
    AUDIT_EXIT_CODE=0
  else
    AUDIT_EXIT_CODE=$?
    if grep -Eqi 'unsupported CVSS version|unsupported cvss version' "$AUDIT_LOG"; then
      AUDIT_STATUS="error_unsupported_cvss"
    else
      AUDIT_STATUS="fail"
    fi
  fi

  # Re-run cargo deny advisories check and capture output.
  if run_with_retries "Running cargo deny advisories check" "$DENY_LOG" \
    cargo deny --format json check advisories; then
    DENY_STATUS="pass"
    DENY_EXIT_CODE=0
  else
    DENY_EXIT_CODE=$?
    if grep -Eqi 'unsupported CVSS version|unsupported cvss version' "$DENY_LOG"; then
      DENY_STATUS="error_unsupported_cvss"
    else
      DENY_STATUS="fail"
    fi
  fi

  # Print JSON summary required by CI/automation.
  emit_summary_json

  # Return non-zero if either audit stage failed.
  if [ "$AUDIT_STATUS" != "pass" ] || [ "$DENY_STATUS" != "pass" ]; then
    return 1
  fi

  return 0
}

main "$@"
