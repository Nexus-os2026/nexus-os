#!/usr/bin/env bash
# Phase 1.4 mock Codex CLI for vision_judge tests.
# Parses --output-last-message FILE from argv and writes a fixed
# JSON-encoded VisionVerdict to that path, then exits 0.

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

if [[ -z "$OUTPUT_PATH" ]]; then
    echo "mock_codex: --output-last-message not provided" >&2
    exit 2
fi

cat > "$OUTPUT_PATH" <<'JSON'
{"verdict":"Changed","confidence":0.92,"reasoning":"button highlighted","detected_changes":["highlight"]}
JSON
