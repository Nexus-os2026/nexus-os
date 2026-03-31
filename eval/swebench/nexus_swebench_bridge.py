#!/usr/bin/env python3
"""
Bridge between SWE-bench evaluation and Nexus OS.

Routes issue descriptions through the Nexus OS OpenAI-compatible API
(POST /v1/chat/completions) which delegates to the configured LLM provider
with full governance (audit trail, capability checks, fuel metering).

Usage:
    python nexus_swebench_bridge.py dataset.jsonl --method api --output ./predictions
    python nexus_swebench_bridge.py dataset.jsonl --method api --limit 5  # quick validation

Environment variables:
    NEXUS_API_URL     Base URL for the Nexus OS API (default: http://localhost:3000/v1)
    NEXUS_API_KEY     Bearer token for authentication (optional in dev mode)
    NEXUS_MODEL       Model to use (default: nexus-governed)
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path

# Optional: requests may not be installed
try:
    import requests
except ImportError:
    requests = None

NEXUS_API_URL = os.getenv("NEXUS_API_URL", "http://localhost:3000/v1")
NEXUS_API_KEY = os.getenv("NEXUS_API_KEY", "")
NEXUS_MODEL = os.getenv("NEXUS_MODEL", "nexus-governed")


def build_patch_prompt(instance: dict) -> tuple[str, str]:
    """Build system and user messages for patch generation."""
    repo = instance.get("repo", "unknown")
    issue = instance.get("problem_statement", "")
    hints = instance.get("hints_text", "")

    system_msg = (
        f"You are an expert software engineer working on the {repo} repository.\n"
        "Your task is to fix the described issue by producing a minimal unified diff patch.\n"
        "Respond with ONLY the patch in unified diff format (starting with --- and +++).\n"
        "Do not include any explanation, just the patch.\n"
        "Make the smallest change that correctly fixes the issue."
    )

    user_parts = [f"Issue:\n{issue}"]
    if hints:
        user_parts.append(f"\nHints:\n{hints}")
    user_parts.append("\nProduce a unified diff patch that fixes this issue.")
    user_msg = "\n".join(user_parts)

    return system_msg, user_msg


def generate_patch_via_api(instance: dict) -> str:
    """Use Nexus OS OpenAI-compatible API to generate a patch."""
    if requests is None:
        print("  ERROR: 'requests' package not installed", file=sys.stderr)
        return ""

    system_msg, user_msg = build_patch_prompt(instance)

    headers = {"Content-Type": "application/json"}
    if NEXUS_API_KEY:
        headers["Authorization"] = f"Bearer {NEXUS_API_KEY}"

    payload = {
        "model": NEXUS_MODEL,
        "messages": [
            {"role": "system", "content": system_msg},
            {"role": "user", "content": user_msg},
        ],
        "temperature": 0.0,
        "max_tokens": 4096,
    }

    try:
        resp = requests.post(
            f"{NEXUS_API_URL}/chat/completions",
            headers=headers,
            json=payload,
            timeout=300,
        )
        resp.raise_for_status()
        data = resp.json()
        return data["choices"][0]["message"]["content"].strip()
    except requests.exceptions.Timeout:
        print("  TIMEOUT: API call exceeded 5 minutes", file=sys.stderr)
        return ""
    except Exception as e:
        print(f"  ERROR: API call failed: {e}", file=sys.stderr)
        return ""


def generate_patch_offline(instance: dict) -> str:
    """Offline mode: generate a prompt file for manual LLM invocation."""
    system_msg, user_msg = build_patch_prompt(instance)
    return f"[SYSTEM]\n{system_msg}\n\n[USER]\n{user_msg}"


def load_dataset(path: str) -> list[dict]:
    """Load a JSONL dataset."""
    instances = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                instances.append(json.loads(line))
    return instances


def run_evaluation(
    dataset_path: str,
    method: str = "api",
    output_dir: str = "./predictions",
    limit: int | None = None,
):
    """Run SWE-bench evaluation on a dataset."""
    os.makedirs(output_dir, exist_ok=True)

    instances = load_dataset(dataset_path)
    if limit is not None:
        instances = instances[:limit]

    total = len(instances)
    print(f"SWE-bench evaluation: {total} instances, method={method}, model={NEXUS_MODEL}")
    print(f"API: {NEXUS_API_URL}")
    print(f"Output: {output_dir}")

    predictions = []
    results_log = []

    for i, instance in enumerate(instances):
        instance_id = instance.get("instance_id", f"unknown-{i}")
        print(f"\n[{i+1}/{total}] {instance_id}")

        start = time.time()

        if method == "api":
            patch = generate_patch_via_api(instance)
        elif method == "offline":
            patch = generate_patch_offline(instance)
        else:
            print(f"  ERROR: Unknown method '{method}'", file=sys.stderr)
            patch = ""

        elapsed = time.time() - start

        predictions.append({
            "instance_id": instance_id,
            "model_patch": patch,
            "model_name_or_path": f"nexus-os/{NEXUS_MODEL}",
        })

        results_log.append({
            "instance_id": instance_id,
            "repo": instance.get("repo", ""),
            "has_patch": bool(patch) and not patch.startswith("[SYSTEM]"),
            "patch_length": len(patch),
            "elapsed_seconds": round(elapsed, 2),
        })

        status = "Patch" if results_log[-1]["has_patch"] else "No patch"
        print(f"  {status} ({elapsed:.1f}s, {len(patch)} chars)")

        # Save incrementally
        pred_path = os.path.join(output_dir, "predictions.jsonl")
        with open(pred_path, "w") as f:
            for pred in predictions:
                f.write(json.dumps(pred) + "\n")

    # Save results log
    log_path = os.path.join(output_dir, "results_log.json")
    with open(log_path, "w") as f:
        json.dump(results_log, f, indent=2)

    # Print summary
    patches = sum(1 for r in results_log if r["has_patch"])
    avg_time = sum(r["elapsed_seconds"] for r in results_log) / total if total else 0

    print(f"\n{'=' * 60}")
    print("SWE-bench Evaluation Summary")
    print(f"{'=' * 60}")
    print(f"Model:              {NEXUS_MODEL}")
    print(f"Total instances:    {total}")
    print(f"Patches generated:  {patches}/{total} ({patches/total*100:.1f}%)" if total else "")
    print(f"Average time:       {avg_time:.1f}s per instance")
    print(f"Predictions:        {os.path.join(output_dir, 'predictions.jsonl')}")
    print(f"Results log:        {log_path}")
    print()
    print("Next steps:")
    print(f"  1. Verify predictions: head -1 {pred_path} | python -m json.tool")
    print(f"  2. Run SWE-bench harness:")
    print(f"     python -m swebench.harness.run_evaluation \\")
    print(f"       --predictions_path {pred_path} \\")
    print(f"       --swe_bench_tasks {dataset_path} \\")
    print(f"       --log_dir {output_dir}/logs")


def main():
    parser = argparse.ArgumentParser(
        description="Nexus OS SWE-bench evaluation bridge",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Quick validation (5 instances)
  python nexus_swebench_bridge.py data.jsonl --limit 5

  # Full run against Nexus OS API
  NEXUS_MODEL=gpt-4o python nexus_swebench_bridge.py data.jsonl

  # Offline: generate prompts for manual invocation
  python nexus_swebench_bridge.py data.jsonl --method offline
        """,
    )
    parser.add_argument("dataset", help="Path to SWE-bench dataset (JSONL)")
    parser.add_argument(
        "--method",
        choices=["api", "offline"],
        default="api",
        help="How to invoke Nexus OS (default: api)",
    )
    parser.add_argument("--output", default="./predictions", help="Output directory")
    parser.add_argument("--limit", type=int, default=None, help="Limit to first N instances")
    args = parser.parse_args()

    run_evaluation(args.dataset, args.method, args.output, args.limit)


if __name__ == "__main__":
    main()
