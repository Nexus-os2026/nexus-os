#!/usr/bin/env bash
# SWE-bench evaluation setup for Nexus OS
# Usage: bash setup.sh
set -euo pipefail

EVAL_DIR="$(cd "$(dirname "$0")" && pwd)"
VENV_DIR="$EVAL_DIR/venv"

echo "=== Nexus OS SWE-bench Evaluation Setup ==="
echo "Directory: $EVAL_DIR"

# 1. Create Python virtual environment
if [ ! -d "$VENV_DIR" ]; then
    echo "Creating Python venv..."
    python3 -m venv "$VENV_DIR"
fi
# shellcheck source=/dev/null
source "$VENV_DIR/bin/activate"

# 2. Install dependencies
echo "Installing Python dependencies..."
pip install --quiet --upgrade pip
pip install --quiet requests

# 3. Try installing SWE-bench (may fail if datasets/docker not available)
echo "Attempting SWE-bench install..."
if pip install --quiet swebench 2>/dev/null; then
    echo "SWE-bench installed successfully"
    SWEBENCH_AVAILABLE=true
else
    echo "WARNING: SWE-bench install failed (may need Docker). Bridge still works."
    SWEBENCH_AVAILABLE=false
fi

# 4. Try downloading SWE-bench Verified dataset
DATASET="$EVAL_DIR/swebench_verified.jsonl"
if [ ! -f "$DATASET" ]; then
    echo "Attempting to download SWE-bench Verified dataset..."
    python3 -c "
from datasets import load_dataset
ds = load_dataset('princeton-nlp/SWE-bench_Verified', split='test')
print(f'Downloaded {len(ds)} instances')
ds.to_json('$DATASET')
" 2>/dev/null || echo "WARNING: Could not download dataset (need 'datasets' package). Use --offline mode or provide your own dataset."
fi

# 5. Create a small sample dataset for quick testing
if [ -f "$DATASET" ]; then
    echo "Creating 5-instance sample..."
    head -5 "$DATASET" > "$EVAL_DIR/swebench_sample_5.jsonl"
    echo "Sample: $EVAL_DIR/swebench_sample_5.jsonl"
fi

echo ""
echo "=== Setup Complete ==="
echo "Activate venv:  source $VENV_DIR/bin/activate"
echo ""
echo "Run evaluation:"
echo "  # Quick validation (5 instances)"
echo "  python $EVAL_DIR/nexus_swebench_bridge.py $EVAL_DIR/swebench_sample_5.jsonl --limit 5"
echo ""
echo "  # Full Verified set"
echo "  python $EVAL_DIR/nexus_swebench_bridge.py $DATASET"
echo ""
echo "  # Offline mode (generate prompts without API)"
echo "  python $EVAL_DIR/nexus_swebench_bridge.py $DATASET --method offline"
