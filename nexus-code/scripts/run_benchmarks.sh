#!/bin/bash
set -euo pipefail

TASKS_FILE="${1:-swe-bench-verified.jsonl}"
LIMIT="${2:-50}"
FUEL="${3:-20000}"
MAX_TURNS="${4:-15}"
WORKSPACE="/tmp/nx-bench"
OUTPUT_DIR="bench-results"

mkdir -p "$OUTPUT_DIR"

echo "=== Nexus Code Benchmark Suite ==="
echo "Tasks: $TASKS_FILE (limit: $LIMIT)"

if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
  echo "Running with Anthropic..."
  nx bench run --tasks-file "$TASKS_FILE" --limit "$LIMIT" --fuel "$FUEL" --max-turns "$MAX_TURNS" --workspace "$WORKSPACE"
  cp nx-bench-report.json "$OUTPUT_DIR/anthropic.json"
fi

if [ -n "${OPENAI_API_KEY:-}" ]; then
  echo "Running with OpenAI..."
  NX_PROVIDER=openai NX_MODEL=gpt-4o nx bench run --tasks-file "$TASKS_FILE" --limit "$LIMIT" --fuel "$FUEL" --max-turns "$MAX_TURNS" --workspace "$WORKSPACE"
  cp nx-bench-report.json "$OUTPUT_DIR/openai.json"
fi

REPORTS=$(ls "$OUTPUT_DIR"/*.json 2>/dev/null | tr '\n' ',' | sed 's/,$//')
if [ -n "$REPORTS" ]; then
  echo "Generating paper data..."
  nx bench paper --reports "$REPORTS" --output "$OUTPUT_DIR/paper-data.json" --latex
fi

echo "Done. Results in $OUTPUT_DIR/"
