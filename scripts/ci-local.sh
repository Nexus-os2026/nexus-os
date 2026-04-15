#!/usr/bin/env bash
# =============================================================================
# scripts/ci-local.sh — Local mirror of GitLab CI pipeline for Nexus OS
# =============================================================================
#
# WHAT IT DOES
#   Runs the exact same commands that .gitlab-ci.yml runs on GitLab, locally,
#   so we can see what CI sees BEFORE pushing. Mirrors the 6 jobs in the
#   failing pipeline: cargo-audit, cargo-deny, rust-lint, rust-tests-core,
#   rust-tests-full, frontend-tests.
#
#   This is the mandatory pre-push gate. Run it before every push to main.
#
# HOW TO RUN
#   bash scripts/ci-local.sh                 # run all 6 jobs (matches CI)
#   bash scripts/ci-local.sh --clean-check   # stash working tree, run against committed main
#   bash scripts/ci-local.sh --skip-frontend # skip frontend-tests
#   bash scripts/ci-local.sh --skip-security # skip cargo-audit + cargo-deny
#   bash scripts/ci-local.sh --skip-security --skip-frontend  # Rust jobs only
#
# --clean-check    Stash uncommitted changes (including untracked), run the
#                  mirror against committed main, restore the stash on exit via
#                  a trap handler (survives Ctrl+C and job failures). USE THIS
#                  BEFORE EVERY PUSH. This is the mode that catches working-
#                  tree-vs-committed-tree drift — e.g. a struct literal that
#                  compiles locally only because the working tree happens to
#                  carry a field the committed tree is missing.
#
# LOGS
#   Per-job logs: /tmp/ci-local-output/<job_name>.log
#   Summary capture (recommended): pipe stdout through `tee`:
#     bash scripts/ci-local.sh 2>&1 | tee /tmp/ci-local-summary.log
#
# EXIT CODE
#   0 if all executed jobs pass, 1 if any fail.
#
# REPRODUCIBILITY CAVEATS
#   - GitLab CI does not pin an image in .gitlab-ci.yml, so runner toolchain
#     is unknown. Local rustc may differ — lint/test divergence is possible.
#   - cargo-audit and cargo-deny are installed on-the-fly by this script
#     (mirroring CI's `cargo install --locked ... || true` pattern).
#   - CARGO_BUILD_JOBS=2 mirrors CI's global throttle; machine may have more
#     cores but we preserve CI behavior for result parity.
# =============================================================================

set -euo pipefail

# ----- Global env (mirrors .gitlab-ci.yml top-level `variables:`) ------------
export CARGO_BUILD_JOBS="2"
export CARGO_PROFILE_DEV_DEBUG="1"

# ----- Config ----------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="/tmp/ci-local-output"
mkdir -p "$LOG_DIR"

# Argument flags
SKIP_FRONTEND=0
SKIP_SECURITY=0
CLEAN_CHECK=0
for arg in "$@"; do
    case "$arg" in
        --skip-frontend) SKIP_FRONTEND=1 ;;
        --skip-security) SKIP_SECURITY=1 ;;
        --clean-check)   CLEAN_CHECK=1 ;;
        -h|--help)
            grep -E '^# ' "$0" | sed 's/^# //; s/^#//'
            exit 0
            ;;
        *)
            echo "[ci-local] unknown argument: $arg" >&2
            echo "[ci-local] use --help for usage" >&2
            exit 2
            ;;
    esac
done

# ----- --clean-check: stash working tree so we compile committed main --------
# Stash BEFORE main() so the trap is armed before any job runs. Trap restores
# the stash on normal exit, signal, or error.
STASHED=0
STASH_MSG="ci-local-clean-check-$$"

cleanup_clean_check() {
    if [ "${STASHED}" -eq 1 ]; then
        echo ""
        echo "[ci-local] ===== Restoring stashed working tree ====="
        if ! git -C "$REPO_ROOT" stash pop; then
            echo "[ci-local] WARNING: git stash pop failed." >&2
            echo "[ci-local] Stash entry may still exist:" >&2
            git -C "$REPO_ROOT" stash list | grep "${STASH_MSG}" >&2 || true
            echo "[ci-local] Restore manually with: git stash pop" >&2
        fi
    fi
}

if [ "${CLEAN_CHECK}" -eq 1 ]; then
    echo "[ci-local] ===== --clean-check mode ====="
    echo "[ci-local] Stashing working tree to test against committed main"
    cd "$REPO_ROOT"
    if git diff --quiet \
        && git diff --cached --quiet \
        && [ -z "$(git ls-files --others --exclude-standard)" ]; then
        echo "[ci-local] Working tree is clean, no stash needed"
        STASHED=0
    else
        if ! git stash push --include-untracked --message "${STASH_MSG}"; then
            echo "[ci-local] ERROR: git stash push failed, aborting" >&2
            exit 1
        fi
        STASHED=1
    fi
    # Arm the trap AFTER the stash succeeded, so a pre-stash failure does
    # not try to pop a nonexistent stash.
    trap cleanup_clean_check EXIT INT TERM
fi

# Result accumulators
JOB_RESULTS=()   # "name|status|exit|elapsed|log"
FAILED_JOBS=()   # "name|log"

# ----- run_job helper --------------------------------------------------------
# Wraps a job function: captures exit code via PIPESTATUS (tee would otherwise
# mask it), measures elapsed time, appends to accumulators, prints status line.
run_job() {
    local name="$1"
    local fn="$2"
    local log="$LOG_DIR/${name}.log"
    local start end elapsed exit_code

    echo ""
    echo "[ci-local] ===== RUN $name ====="
    start=$(date +%s)

    # Run the job function. Use PIPESTATUS[0] to get the function's exit code
    # before tee mangles it. `set +e` locally so a failure does not abort the
    # parent script under `set -e`.
    set +e
    ( set -e; "$fn" ) 2>&1 | tee "$log"
    exit_code=${PIPESTATUS[0]}
    set -e

    end=$(date +%s)
    elapsed=$((end - start))

    if [ "$exit_code" -eq 0 ]; then
        echo "[ci-local] $name: PASS (${elapsed}s)"
        JOB_RESULTS+=("${name}|PASS|0|${elapsed}|${log}")
    else
        echo "[ci-local] $name: FAIL (exit=${exit_code}, elapsed=${elapsed}s, log=${log})"
        JOB_RESULTS+=("${name}|FAIL|${exit_code}|${elapsed}|${log}")
        FAILED_JOBS+=("${name}|${log}")
    fi
}

# =============================================================================
# JOBS
# =============================================================================

# ----- preflight: toolchain sanity (not a CI job — runs first) ---------------
job_preflight() {
    cd "$REPO_ROOT"
    echo "--- tool versions ---"
    rustc --version || { echo "rustc not found"; return 1; }
    cargo --version || { echo "cargo not found"; return 1; }
    rustup --version || { echo "rustup not found"; return 1; }
    echo ""
    echo "--- ensuring components (rustfmt, clippy) ---"
    rustup component add rustfmt clippy
    echo ""
    echo "--- ensuring target wasm32-wasip1 ---"
    rustup target add wasm32-wasip1
    echo ""
    echo "--- installed targets ---"
    rustup target list --installed
    echo ""
    echo "--- active toolchain ---"
    rustup show active-toolchain
}

# ----- cargo-audit (.gitlab-ci.yml:22-32) ------------------------------------
# before_script: cargo install cargo-audit --locked 2>/dev/null || true
# script:        cargo audit --ignore RUSTSEC-2026-0044 ... (6 ignores)
job_cargo_audit() {
    cd "$REPO_ROOT"
    cargo install cargo-audit --locked 2>/dev/null || true
    cargo audit \
        --ignore RUSTSEC-2026-0044 \
        --ignore RUSTSEC-2026-0048 \
        --ignore RUSTSEC-2023-0071 \
        --ignore RUSTSEC-2026-0049 \
        --ignore RUSTSEC-2026-0067 \
        --ignore RUSTSEC-2026-0068
}

# ----- cargo-deny (.gitlab-ci.yml:34-44) -------------------------------------
# before_script: cargo install cargo-deny --locked 2>/dev/null || true
# script:        cargo deny check
job_cargo_deny() {
    cd "$REPO_ROOT"
    cargo install cargo-deny --locked 2>/dev/null || true
    cargo deny check
}

# ----- rust-lint (.gitlab-ci.yml:61-66, extends .rust-base) ------------------
# .rust-base env vars are scoped to this function only — CI applies them
# to rust-lint, rust-tests-core, and rust-tests-full via `extends:`, but
# NOT to security or frontend jobs.
job_rust_lint() {
    cd "$REPO_ROOT"
    export CARGO_INCREMENTAL="1"
    export RUSTC_WRAPPER=""
    export RUSTFLAGS="-C debuginfo=1"
    export PATH="$HOME/.cargo/bin:$PATH"

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings -A unexpected_cfgs
}

# ----- rust-tests-core (.gitlab-ci.yml:68-72, extends .rust-base) ------------
job_rust_tests_core() {
    cd "$REPO_ROOT"
    export CARGO_INCREMENTAL="1"
    export RUSTC_WRAPPER=""
    export RUSTFLAGS="-C debuginfo=1"
    export PATH="$HOME/.cargo/bin:$PATH"

    cargo test -p nexus-kernel -p nexus-sdk -p nexus-cli
}

# ----- rust-tests-full (.gitlab-ci.yml:74-78, extends .rust-base) ------------
job_rust_tests_full() {
    cd "$REPO_ROOT"
    export CARGO_INCREMENTAL="1"
    export RUSTC_WRAPPER=""
    export RUSTFLAGS="-C debuginfo=1"
    export PATH="$HOME/.cargo/bin:$PATH"

    cargo test --workspace \
        --exclude nexus-kernel \
        --exclude nexus-sdk \
        --exclude nexus-cli \
        --exclude nexus-integration
}

# ----- frontend-tests (.gitlab-ci.yml:80-91) ---------------------------------
# No .rust-base inheritance — CI runs this without CARGO_INCREMENTAL etc.
job_frontend_tests() {
    cd "$REPO_ROOT/app"
    npm ci
    npm test
    npm run build
}

# =============================================================================
# MAIN
# =============================================================================

main() {
    echo "[ci-local] Nexus OS CI local mirror"
    echo "[ci-local] repo: $REPO_ROOT"
    echo "[ci-local] logs: $LOG_DIR"
    echo "[ci-local] flags: SKIP_FRONTEND=$SKIP_FRONTEND SKIP_SECURITY=$SKIP_SECURITY CLEAN_CHECK=$CLEAN_CHECK"

    # Preflight always runs — we need toolchain present regardless of flags.
    run_job "preflight" job_preflight

    # Security stage
    if [ "$SKIP_SECURITY" -eq 0 ]; then
        run_job "cargo-audit" job_cargo_audit
        run_job "cargo-deny" job_cargo_deny
    else
        echo "[ci-local] skipping security stage (--skip-security)"
    fi

    # Test stage — Rust
    run_job "rust-lint" job_rust_lint
    run_job "rust-tests-core" job_rust_tests_core
    run_job "rust-tests-full" job_rust_tests_full

    # Test stage — frontend
    if [ "$SKIP_FRONTEND" -eq 0 ]; then
        run_job "frontend-tests" job_frontend_tests
    else
        echo "[ci-local] skipping frontend-tests (--skip-frontend)"
    fi

    # ----- Summary ----------------------------------------------------------
    local total=${#JOB_RESULTS[@]}
    local failed=${#FAILED_JOBS[@]}
    local passed=$((total - failed))

    echo ""
    echo "============================================================"
    echo "[ci-local] SUMMARY"
    echo "============================================================"
    echo "total:  $total"
    echo "pass:   $passed"
    echo "fail:   $failed"
    echo ""
    echo "per-job results:"
    for row in "${JOB_RESULTS[@]}"; do
        IFS='|' read -r n s e t l <<< "$row"
        printf "  %-20s %-4s  exit=%-3s  %4ss  %s\n" "$n" "$s" "$e" "$t" "$l"
    done

    if [ "$failed" -gt 0 ]; then
        echo ""
        echo "failed jobs:"
        for row in "${FAILED_JOBS[@]}"; do
            IFS='|' read -r n l <<< "$row"
            echo "  - $n  →  $l"
        done
        echo ""
        echo "[ci-local] RESULT: FAIL ($failed job(s) failed)"
        return 1
    fi

    echo ""
    echo "[ci-local] RESULT: PASS (all $total jobs green)"
    return 0
}

main "$@"
