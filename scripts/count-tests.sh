#!/bin/bash
# Authoritative test count for Nexus OS
# Run: bash scripts/count-tests.sh
set -euo pipefail

echo "=== Nexus OS Test Inventory ==="
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Commit: $(git rev-parse HEAD)"
echo ""

total=0
while IFS= read -r crate; do
    output=$(cargo test -p "$crate" 2>&1 || true)
    count=$(echo "$output" | grep "test result" | grep -oP '\d+ passed' | grep -oP '\d+' || echo "0")
    if [ "$count" != "0" ] && [ "$count" != "" ]; then
        printf "  %-40s %s passed\n" "$crate" "$count"
        total=$((total + count))
    fi
done < <(cargo metadata --no-deps --format-version 1 2>/dev/null \
    | python3 -c "import sys,json; [print(p['name']) for p in json.load(sys.stdin)['packages']]" 2>/dev/null)

echo ""
echo "Total: $total tests"
