#!/usr/bin/env bash
# Mock Codex that writes malformed JSON to the output file.
set -euo pipefail
OUTPUT_PATH=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --output-last-message)
            OUTPUT_PATH="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done
echo "this is not json {{" > "$OUTPUT_PATH"
