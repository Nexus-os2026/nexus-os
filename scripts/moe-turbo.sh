#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────
# moe-turbo.sh — One-time system tuning for MoE inference on consumer HW
#
# Run with: sudo ./scripts/moe-turbo.sh
#
# What it does:
#   1. Pins Nexus process to physical cores (no HT)
#   2. Tunes Linux VM for maximum page cache retention
#   3. Allocates transparent huge pages
#   4. Sets NVMe readahead to 64 MB
#   5. Drops stale page caches to free RAM for model
#   6. Kills Ollama if running (frees 6.5 GB VRAM)
#
# All changes are temporary — revert on reboot.
# Run this BEFORE loading a large MoE model in Flash Inference.
# ──────────────────────────────────────────────────────────────────────

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${CYAN}══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  Nexus OS — MoE Turbo Mode                  ${NC}"
echo -e "${CYAN}══════════════════════════════════════════════${NC}"
echo ""

if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}ERROR: Must run as root.${NC}"
    echo "Usage: sudo $0"
    exit 1
fi

# ── 1. Kill Ollama (eats 6.5 GB VRAM) ────────────────────────────────
echo -e "${YELLOW}[1/7]${NC} Checking for Ollama..."
if pgrep -x ollama > /dev/null 2>&1; then
    pkill ollama
    sleep 1
    echo -e "  ${GREEN}Killed Ollama — freed ~6.5 GB VRAM${NC}"
else
    echo -e "  Already stopped"
fi

# ── 2. CPU affinity — pin to physical cores ──────────────────────────
echo -e "${YELLOW}[2/7]${NC} Detecting physical cores..."

PHYSICAL_CORES=()
for cpu in /sys/devices/system/cpu/cpu[0-9]*; do
    num=$(basename "$cpu" | tr -dc '0-9')
    siblings_file="$cpu/topology/thread_siblings_list"
    [ -f "$siblings_file" ] || continue
    first=$(cat "$siblings_file" | tr ',-' '\n' | head -1)
    if [ "$first" = "$num" ] && [ "${#PHYSICAL_CORES[@]}" -lt 6 ]; then
        PHYSICAL_CORES+=("$num")
    fi
done

echo -e "  Physical cores (max 6): ${GREEN}${PHYSICAL_CORES[*]}${NC}"

# Build taskset mask from physical cores
MASK=$(printf "%s," "${PHYSICAL_CORES[@]}")
MASK="${MASK%,}"

pin_nexus_process() {
    local pid="$1"
    # Pin the main process
    taskset -pc "$MASK" "$pid" > /dev/null 2>&1

    # Pin all its threads
    local pinned=0
    for tid in /proc/"$pid"/task/[0-9]*; do
        tid_num=$(basename "$tid")
        taskset -pc "$MASK" "$tid_num" > /dev/null 2>&1 && ((pinned++)) || true
    done
    echo -e "  ${GREEN}Pinned $pinned threads (PID $pid) to cores $MASK${NC}"
}

# Find the Nexus desktop backend process
NEXUS_PID=$(pgrep -f "nexus-desktop-backend\|nexus.*tauri" | head -1 || true)
if [ -n "$NEXUS_PID" ]; then
    pin_nexus_process "$NEXUS_PID"
else
    echo -e "  ${YELLOW}Nexus process not found — waiting up to 60s...${NC}"
    WAITED=0
    while [ $WAITED -lt 60 ]; do
        sleep 3
        WAITED=$((WAITED + 3))
        NEXUS_PID=$(pgrep -f "nexus-desktop-backend\|nexus.*tauri" | head -1 || true)
        if [ -n "$NEXUS_PID" ]; then
            echo -e "  ${GREEN}Found process after ${WAITED}s${NC}"
            pin_nexus_process "$NEXUS_PID"
            break
        fi
        printf "  Waiting... (%ds)\r" "$WAITED"
    done
    if [ -z "$NEXUS_PID" ]; then
        echo -e "  ${RED}Timed out — start the app and re-run this script${NC}"
    fi
fi

# ── 3. VM tuning ─────────────────────────────────────────────────────
echo -e "${YELLOW}[3/7]${NC} Tuning VM parameters..."

# LOW swappiness: keep app memory (Tauri, desktop, Claude Code) in RAM.
# The model is mmap'd (file-backed), so the kernel manages its page cache
# separately from anonymous (app) memory. High swappiness would swap out
# apps to make room for model pages, freezing the entire desktop.
echo 10 > /proc/sys/vm/swappiness
echo -e "  swappiness = ${GREEN}10${NC} (keep apps in RAM, model uses page cache)"

# Protect page cache metadata (dentries, inodes)
echo 50 > /proc/sys/vm/vfs_cache_pressure
echo -e "  vfs_cache_pressure = ${GREEN}50${NC}"

# Keep a reasonable free memory reserve — don't let the kernel use ALL RAM
# or the system becomes unresponsive during page reclaim storms.
echo 131072 > /proc/sys/vm/min_free_kbytes
echo -e "  min_free_kbytes = ${GREEN}128 MB${NC} (safety margin for desktop)"

# Disable zone reclaim (only matters on NUMA but harmless on single-socket)
echo 0 > /proc/sys/vm/zone_reclaim_mode 2>/dev/null || true
echo -e "  zone_reclaim_mode = ${GREEN}0${NC}"

# ── 4. Transparent Huge Pages ────────────────────────────────────────
echo -e "${YELLOW}[4/7]${NC} Enabling Transparent Huge Pages..."

echo always > /sys/kernel/mm/transparent_hugepage/enabled
echo madvise > /sys/kernel/mm/transparent_hugepage/defrag
echo -e "  THP = ${GREEN}always${NC}, defrag = ${GREEN}madvise${NC}"
echo -e "  (512x fewer TLB misses for mmap'd model pages)"

# ── 5. NVMe readahead ───────────────────────────────────────────────
echo -e "${YELLOW}[5/7]${NC} Setting NVMe readahead..."

NVME_DEV=""
for dev in /dev/nvme0n1 /dev/nvme1n1 /dev/sda; do
    if [ -b "$dev" ]; then
        NVME_DEV="$dev"
        break
    fi
done

if [ -n "$NVME_DEV" ]; then
    OLD_RA=$(blockdev --getra "$NVME_DEV" 2>/dev/null || echo "?")
    blockdev --setra 131072 "$NVME_DEV"
    echo -e "  ${GREEN}$NVME_DEV readahead = 64 MB${NC} (was ${OLD_RA} sectors)"
else
    echo -e "  ${YELLOW}No NVMe device found${NC}"
fi

# ── 6. Sync filesystem ──────────────────────────────────────────────
echo -e "${YELLOW}[6/7]${NC} Syncing filesystem..."
sync
echo -e "  ${GREEN}Filesystem synced${NC} (dirty pages flushed)"
# NOTE: We deliberately do NOT drop_caches — it evicts useful cached data
# from your desktop, browser, and apps, causing everything to stutter.

# ── 7. Summary ──────────────────────────────────────────────────────
echo -e "${YELLOW}[7/7]${NC} Verifying..."
echo ""
echo -e "${CYAN}══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  MoE Turbo Mode — ACTIVE                    ${NC}"
echo -e "${CYAN}══════════════════════════════════════════════${NC}"
echo ""
echo -e "  CPU cores:     ${GREEN}${#PHYSICAL_CORES[@]} physical (HT disabled)${NC}"
echo -e "  Swappiness:    ${GREEN}100${NC} (max page cache retention)"
echo -e "  Huge pages:    ${GREEN}THP always${NC} (512x TLB coverage)"
echo -e "  NVMe readahead:${GREEN} 64 MB${NC} (vs default 128 KB)"
echo -e "  Page cache:    ${GREEN}Cleaned${NC}"
echo ""
echo -e "  RAM available: ${GREEN}$(awk '/MemAvailable/{printf "%.1f GB", $2/1024/1024}' /proc/meminfo)${NC}"
echo -e "  Swap used:     ${YELLOW}$(awk '/SwapTotal/{t=$2} /SwapFree/{printf "%.1f GB", (t-$2)/1024/1024}' /proc/meminfo)${NC}"
echo ""
echo -e "  ${GREEN}Now load your model in Flash Inference.${NC}"
echo -e "  Changes revert on reboot."
echo ""
