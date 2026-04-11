#!/usr/bin/env bash
# run_batch.sh — Nexus OS Phase 1 page audit driver
#
# Drives Claude Code (in print mode + chrome) over every route in _ROUTES.txt,
# then captures screenshots via Puppeteer. Resumable, vite-aware, safe to ctrl-c.
#
# Usage:
#   bash run_batch.sh             # normal run (resumes if _PROGRESS.tsv exists)
#   FORCE_RESET=1 bash run_batch.sh   # wipe progress and start over
#   SKIP_SMOKE=1 bash run_batch.sh    # skip the 3-page pause (for resumes)
#
# Files written:
#   docs/page-audits/phase1-vite/_PROGRESS.tsv     resumable state
#   docs/page-audits/phase1-vite/_RUN.log          run log
#   docs/page-audits/phase1-vite/_MASTER_FINDINGS.md   summary table
#   docs/page-audits/phase1-vite/findings/<slug>.md
#   docs/page-audits/phase1-vite/findings/<slug>-{1920,1280,1024}.png
#
# Requirements:
#   - Vite running at http://localhost:1420
#   - Claude Code 2.1+ on PATH (`claude --version`)
#   - Claude in Chrome extension installed and authenticated
#   - node + npm on PATH
#   - Puppeteer (auto-installed in script dir on first run)

set -uo pipefail

# ============================================================================
# Config
# ============================================================================

REPO_ROOT="${REPO_ROOT:-/home/nexus/NEXUS/nexus-os}"
PHASE_DIR="$REPO_ROOT/docs/page-audits/phase1-vite"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

ROUTES="$PHASE_DIR/_ROUTES.txt"
PROGRESS="$PHASE_DIR/_PROGRESS.tsv"
MASTER="$PHASE_DIR/_MASTER_FINDINGS.md"
FINDINGS="$PHASE_DIR/findings"
LOG="$PHASE_DIR/_RUN.log"

PROMPT_TEMPLATE="$SCRIPT_DIR/audit_prompt_template.txt"
SCREENSHOT="$SCRIPT_DIR/capture_screenshots.mjs"

VITE_URL="http://localhost:1420"
SMOKE_TEST_PAGES=3
PER_PAGE_TIMEOUT=600          # 10 min hard cap per claude audit
MCP_RESET_EVERY=10            # pkill claude MCP every N pages

# ============================================================================
# Colors & logging
# ============================================================================

if [[ -t 1 ]]; then
  RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
  BLUE='\033[0;34m'; NC='\033[0m'
else
  RED=''; GREEN=''; YELLOW=''; BLUE=''; NC=''
fi

log()  { echo -e "$(date '+%H:%M:%S') $*" | tee -a "$LOG"; }
warn() { echo -e "${YELLOW}$(date '+%H:%M:%S') WARN: $*${NC}" | tee -a "$LOG"; }
err()  { echo -e "${RED}$(date '+%H:%M:%S') ERROR: $*${NC}" | tee -a "$LOG" >&2; }

# ============================================================================
# Pre-flight
# ============================================================================

preflight() {
  log "${BLUE}=== PRE-FLIGHT CHECKS ===${NC}"

  [[ -f "$ROUTES" ]] || { err "Routes file not found: $ROUTES"; exit 1; }
  [[ -f "$PROMPT_TEMPLATE" ]] || { err "Prompt template not found: $PROMPT_TEMPLATE"; exit 1; }
  [[ -f "$SCREENSHOT" ]] || { err "Screenshot script not found: $SCREENSHOT"; exit 1; }

  command -v claude >/dev/null || { err "claude not on PATH"; exit 1; }
  command -v node >/dev/null || { err "node not on PATH"; exit 1; }
  command -v npm >/dev/null || { err "npm not on PATH"; exit 1; }
  command -v curl >/dev/null || { err "curl not on PATH"; exit 1; }

  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" "$VITE_URL/" || echo "000")
  if [[ "$code" != "200" ]]; then
    err "Vite not responding at $VITE_URL (got HTTP $code)"
    err "Start it with: cd $REPO_ROOT/app && npm run dev"
    exit 1
  fi

  if [[ ! -d "$SCRIPT_DIR/node_modules/puppeteer" ]]; then
    log "Installing puppeteer (one-time, ~1 min)..."
    (cd "$SCRIPT_DIR" && {
      [[ -f package.json ]] || npm init -y >/dev/null 2>&1
      npm install puppeteer >/dev/null 2>&1
    }) || { err "puppeteer install failed"; exit 1; }
    log "${GREEN}✓ puppeteer installed${NC}"
  fi

  mkdir -p "$FINDINGS"

  if [[ "${FORCE_RESET:-}" == "1" ]] || [[ ! -f "$PROGRESS" ]]; then
    log "Initializing progress file..."
    {
      printf 'url\tslug\tstatus\tstart_time\tend_time\tp0\tp1\tp2\terror\n'
      while IFS= read -r url; do
        [[ -z "$url" ]] && continue
        local slug
        slug=$(echo "$url" | sed -E 's|.*/||' | sed -E 's|[^a-zA-Z0-9_-]||g')
        printf '%s\t%s\tpending\t\t\t\t\t\t\n' "$url" "$slug"
      done < "$ROUTES"
    } > "$PROGRESS"
  fi

  log "${GREEN}✓ Pre-flight checks passed${NC}"
}

# ============================================================================
# Progress file helpers
# ============================================================================

update_progress() {
  local slug="$1" status="$2" start="$3" end="$4"
  local p0="$5" p1="$6" p2="$7" errmsg="$8"
  local tmp="$PROGRESS.tmp"
  awk -F'\t' -v OFS='\t' \
      -v slug="$slug" -v status="$status" -v start="$start" -v end="$end" \
      -v p0="$p0" -v p1="$p1" -v p2="$p2" -v errmsg="$errmsg" '
    NR==1 { print; next }
    $2 == slug { $3=status; $4=start; $5=end; $6=p0; $7=p1; $8=p2; $9=errmsg; print; next }
    { print }
  ' "$PROGRESS" > "$tmp" && mv "$tmp" "$PROGRESS"
}

count_status() {
  awk -F'\t' -v s="$1" 'NR>1 && $3==s {n++} END{print n+0}' "$PROGRESS"
}

derive_name() {
  echo "$1" | sed -E 's/-/ /g' | awk '{
    for (i=1; i<=NF; i++) $i = toupper(substr($i,1,1)) substr($i,2)
    print
  }'
}

# ============================================================================
# Single-page audit
# ============================================================================

audit_page() {
  local url="$1" slug="$2" idx="$3" total="$4"
  local name; name=$(derive_name "$slug")
  local start; start=$(date -Iseconds)

  log "${BLUE}[$idx/$total] $name — $url${NC}"

  # Vite health check
  local code; code=$(curl -s -o /dev/null -w "%{http_code}" "$VITE_URL/" || echo "000")
  if [[ "$code" != "200" ]]; then
    err "Vite went down (HTTP $code). Pausing batch."
    err "Restart with: cd $REPO_ROOT/app && npm run dev"
    err "Then re-run this script — it will resume from page $idx."
    exit 2
  fi

  # Build prompt with placeholders substituted (use | as sed delim, not /)
  local prompt
  prompt=$(sed -e "s|{{URL}}|$url|g" \
              -e "s|{{NAME}}|$name|g" \
              -e "s|{{SLUG}}|$slug|g" \
              "$PROMPT_TEMPLATE")

  local audit_log="$FINDINGS/.${slug}.audit.log"

  # Run claude in print mode with chrome integration
  log "  → claude code audit (timeout ${PER_PAGE_TIMEOUT}s)..."
  if echo "$prompt" | timeout "$PER_PAGE_TIMEOUT" claude -p --chrome >"$audit_log" 2>&1; then
    log "  ${GREEN}✓ audit returned${NC}"
  else
    local rc=$?
    err "  audit failed (exit $rc) — see $audit_log"
    update_progress "$slug" "failed" "$start" "$(date -Iseconds)" "" "" "" "claude_exit_$rc"
    return 1
  fi

  if [[ ! -s "$FINDINGS/$slug.md" ]]; then
    err "  markdown missing or empty: $FINDINGS/$slug.md"
    update_progress "$slug" "failed" "$start" "$(date -Iseconds)" "" "" "" "no_markdown"
    return 1
  fi

  # Capture screenshots via puppeteer
  log "  → capturing screenshots..."
  if node "$SCREENSHOT" "$url" "$slug" "$FINDINGS" >>"$audit_log" 2>&1; then
    log "  ${GREEN}✓ screenshots saved${NC}"
  else
    local rc=$?
    err "  screenshot capture failed (exit $rc) — see $audit_log"
    update_progress "$slug" "failed" "$start" "$(date -Iseconds)" "" "" "" "screenshot_exit_$rc"
    return 1
  fi

  # Verify all 4 files exist and are non-empty
  local missing=0
  for f in "$slug.md" "$slug-1920.png" "$slug-1280.png" "$slug-1024.png"; do
    if [[ ! -s "$FINDINGS/$f" ]]; then
      err "  missing or empty: $f"
      missing=1
    fi
  done
  if (( missing )); then
    update_progress "$slug" "failed" "$start" "$(date -Iseconds)" "" "" "" "files_missing"
    return 1
  fi

  # Extract severity counts
  local p0 p1 p2
  p0=$(grep -oP '^- P0:\s*\K\d+' "$FINDINGS/$slug.md" | head -1)
  p1=$(grep -oP '^- P1:\s*\K\d+' "$FINDINGS/$slug.md" | head -1)
  p2=$(grep -oP '^- P2:\s*\K\d+' "$FINDINGS/$slug.md" | head -1)
  p0=${p0:-0}; p1=${p1:-0}; p2=${p2:-0}

  log "  ${GREEN}✓ done — P0=$p0 P1=$p1 P2=$p2${NC}"
  update_progress "$slug" "done" "$start" "$(date -Iseconds)" "$p0" "$p1" "$p2" ""
  return 0
}

# ============================================================================
# Master findings aggregator
# ============================================================================

build_master() {
  log "Building master findings file..."
  {
    echo "# Nexus OS Phase 1 Audit — Master Findings"
    echo ""
    echo "Generated: $(date -Iseconds)"
    echo ""
    echo "## Per-page summary"
    echo ""
    echo "| # | Page | Status | P0 | P1 | P2 | File |"
    echo "|---|------|--------|----|----|----|------|"
    awk -F'\t' '
      NR>1 {
        n++
        printf "| %d | %s | %s | %s | %s | %s | [%s.md](findings/%s.md) |\n",
          n, $2, $3, ($6==""?"-":$6), ($7==""?"-":$7), ($8==""?"-":$8), $2, $2
      }
    ' "$PROGRESS"
    echo ""
    echo "## Totals"
    echo ""
    awk -F'\t' '
      NR>1 {
        total++
        if ($3=="done") done++
        if ($3=="failed") failed++
        if ($3=="pending") pending++
        p0 += ($6==""?0:$6)
        p1 += ($7==""?0:$7)
        p2 += ($8==""?0:$8)
      }
      END {
        printf "- Total pages: %d\n", total
        printf "- Done: %d\n", done+0
        printf "- Failed: %d\n", failed+0
        printf "- Pending: %d\n", pending+0
        printf "- Total P0 findings: %d\n", p0+0
        printf "- Total P1 findings: %d\n", p1+0
        printf "- Total P2 findings: %d\n", p2+0
      }
    ' "$PROGRESS"
  } > "$MASTER"
  log "${GREEN}✓ Master findings: $MASTER${NC}"
}

# ============================================================================
# Main loop
# ============================================================================

main() {
  preflight

  local total done_count failed_count pending_count
  total=$(awk 'NF>0' "$ROUTES" | wc -l)
  done_count=$(count_status "done")
  failed_count=$(count_status "failed")
  pending_count=$((total - done_count - failed_count))

  log "${BLUE}=== BATCH RUN ===${NC}"
  log "Routes: $total | Done: $done_count | Failed: $failed_count | Pending: $pending_count"

  if (( pending_count == 0 )); then
    log "${GREEN}Nothing pending. Building master findings...${NC}"
    build_master
    return 0
  fi

  # Iterate pending pages
  local idx=0 processed=0
  local in_smoke_test=1
  if [[ "${SKIP_SMOKE:-}" == "1" ]] || (( done_count > 0 )); then
    in_smoke_test=0
    log "Resume mode (skipping smoke test pause)."
  fi

  while IFS=$'\t' read -r status slug url; do
    [[ "$status" != "pending" ]] && continue
    idx=$((idx + 1))

    audit_page "$url" "$slug" "$idx" "$pending_count" || {
      warn "Page $slug failed — continuing with next page."
    }
    processed=$((processed + 1))

    # Smoke test pause
    if (( in_smoke_test == 1 )) && (( processed == SMOKE_TEST_PAGES )); then
      echo
      log "${YELLOW}=== SMOKE TEST PAUSE ===${NC}"
      log "First $SMOKE_TEST_PAGES pages audited. Verify in:"
      log "  $FINDINGS/"
      log ""
      log "Inspect the markdown + PNGs. If they look right, type CONTINUE."
      log "If something is wrong, type ABORT and we fix the prompt."
      while true; do
        read -rp "> " answer
        case "$answer" in
          CONTINUE) log "${GREEN}Continuing with remaining pages...${NC}"; break ;;
          ABORT)    log "${RED}Aborted by user. Re-run after fixing the issue.${NC}"; exit 0 ;;
          *)        echo "Please type CONTINUE or ABORT (uppercase)" ;;
        esac
      done
      in_smoke_test=0
    fi

    # MCP state reset every N pages
    if (( processed > 0 && processed % MCP_RESET_EVERY == 0 )); then
      log "  → MCP state reset (every $MCP_RESET_EVERY pages)"
      pkill -f "claude.*mcp" 2>/dev/null || true
      sleep 2
    fi
  done < <(awk -F'\t' 'NR>1 {print $3"\t"$2"\t"$1}' "$PROGRESS")

  echo
  log "${GREEN}=== BATCH COMPLETE ===${NC}"
  log "Done: $(count_status done) | Failed: $(count_status failed)"
  build_master
}

main "$@"
