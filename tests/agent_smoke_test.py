#!/usr/bin/env python3
"""
Nexus OS Agent Smoke Test — Kimi K2 Instruct via NVIDIA NIM

Sends a prompt through every prebuilt agent's system prompt to verify
the LLM pipeline works end-to-end.
"""

import json
import os
import glob
import time
import subprocess
import urllib.request
import urllib.error
import ssl

ENDPOINT = "https://integrate.api.nvidia.com/v1/chat/completions"
MODEL = "moonshotai/kimi-k2-instruct"
AGENT_DIR = os.path.join(os.path.dirname(__file__), "..", "agents", "prebuilt")
USER_PROMPT = "Hello, what can you help me with? Reply in 2 sentences max."
MAX_TOKENS = 150
DELAY_BETWEEN = 0.5  # seconds


def get_api_key() -> str:
    """Read NVIDIA API key from env or from Nexus config via kernel example."""
    key = os.environ.get("NVIDIA_NIM_API_KEY") or os.environ.get("NVIDIA_API_KEY")
    if key:
        return key

    # Try extracting from encrypted config via kernel example
    try:
        result = subprocess.run(
            ["cargo", "run", "-p", "nexus-kernel", "--example", "dump_config"],
            capture_output=True, text=True, timeout=120,
            cwd=os.path.join(os.path.dirname(__file__), "..")
        )
        for line in result.stdout.strip().split("\n"):
            if line.startswith("NVIDIA_KEY="):
                key = line.split("=", 1)[1].strip()
                if key:
                    return key
    except Exception as e:
        print(f"  Warning: could not extract key from config: {e}")

    raise RuntimeError(
        "No NVIDIA API key found. Set NVIDIA_NIM_API_KEY or NVIDIA_API_KEY env var, "
        "or save it in the Nexus config."
    )


def load_agents():
    """Load all prebuilt agent JSON files, sorted by autonomy level then name."""
    agents = []
    for path in sorted(glob.glob(os.path.join(AGENT_DIR, "*.json"))):
        try:
            with open(path) as f:
                data = json.load(f)
            agents.append({
                "name": data.get("name", os.path.basename(path)),
                "level": data.get("autonomy_level", 0),
                "description": data.get("description", ""),
                "file": os.path.basename(path),
            })
        except Exception as e:
            print(f"  Warning: failed to parse {path}: {e}")
    agents.sort(key=lambda a: (a["level"], a["name"]))
    return agents


def query_llm(api_key: str, system_prompt: str) -> tuple[bool, str]:
    """Send a chat request to NVIDIA NIM. Returns (success, response_or_error)."""
    body = json.dumps({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt[:4000]},  # truncate huge prompts
            {"role": "user", "content": USER_PROMPT},
        ],
        "max_tokens": MAX_TOKENS,
        "temperature": 0.7,
    }).encode()

    req = urllib.request.Request(
        ENDPOINT,
        data=body,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )

    ctx = ssl.create_default_context()

    try:
        with urllib.request.urlopen(req, timeout=60, context=ctx) as resp:
            data = json.loads(resp.read().decode())
            text = data["choices"][0]["message"]["content"].strip()
            return True, text
    except urllib.error.HTTPError as e:
        body = e.read().decode()[:200] if e.fp else ""
        return False, f"HTTP {e.code}: {body}"
    except Exception as e:
        return False, str(e)[:200]


LEVEL_NAMES = {
    0: "L0 (Inert)",
    1: "L1 (Reactive)",
    2: "L2 (Guided)",
    3: "L3 (Autonomous)",
    4: "L4 (Bounded-Auto)",
    5: "L5 (Full Autonomy)",
    6: "L6 (Transcendent)",
}


def main():
    print("\n" + "=" * 60)
    print("  NEXUS OS AGENT SMOKE TEST")
    print(f"  Model: {MODEL}")
    print(f"  Endpoint: NVIDIA NIM")
    print("=" * 60 + "\n")

    api_key = get_api_key()
    print(f"  API key: {api_key[:12]}...{api_key[-4:]}")

    agents = load_agents()
    print(f"  Agents found: {len(agents)}\n")

    results = []
    current_level = -1

    for i, agent in enumerate(agents):
        if agent["level"] != current_level:
            current_level = agent["level"]
            level_label = LEVEL_NAMES.get(current_level, f"L{current_level}")
            print(f"\n  {level_label} AGENTS:")
            print(f"  {'─' * 54}")

        success, response = query_llm(api_key, agent["description"])
        snippet = response.replace("\n", " ")[:80] if success else response[:80]
        icon = "✅" if success else "❌"
        status = "PASS" if success else "FAIL"
        results.append((agent["name"], agent["level"], success, snippet))

        name_padded = agent["name"].ljust(26)
        print(f"    {icon} {name_padded} {status}  \"{snippet}\"")

        if i < len(agents) - 1:
            time.sleep(DELAY_BETWEEN)

    # Summary
    passed = sum(1 for _, _, s, _ in results if s)
    failed = len(results) - passed

    print(f"\n\n{'=' * 60}")
    print(f"  SUMMARY: {passed}/{len(results)} passed, {failed} failed")

    if failed > 0:
        print(f"\n  FAILURES:")
        for name, level, success, snippet in results:
            if not success:
                print(f"    ❌ {name} (L{level}): {snippet}")

    print("=" * 60 + "\n")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    exit(main())
