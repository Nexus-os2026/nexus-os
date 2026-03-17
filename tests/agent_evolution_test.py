#!/usr/bin/env python3
"""
Nexus OS Agent Self-Improvement End-to-End Test

Tests whether an agent can: perform a task → get scored → evolve its
system prompt via LLM-based mutation → perform better.

Uses the same NVIDIA NIM / Kimi K2 pipeline as agent_smoke_test.py.
"""

import json
import os
import re
import time
import subprocess
import urllib.request
import urllib.error
import ssl
from datetime import datetime

# ─── Configuration ────────────────────────────────────────────────────────────

ENDPOINT = "https://integrate.api.nvidia.com/v1/chat/completions"
MODEL = "moonshotai/kimi-k2-instruct"
AGENT_DIR = os.path.join(os.path.dirname(__file__), "..", "agents", "prebuilt")
RESULTS_PATH = os.path.join(os.path.dirname(__file__), "evolution_results.json")
MAX_TOKENS = 600
MUTATION_MAX_TOKENS = 1200
DELAY_BETWEEN = 1.0  # seconds between API calls
GENERATIONS = 3

# ─── Test data ────────────────────────────────────────────────────────────────

CLIMATE_TEXT = (
    "Climate change is one of the most pressing challenges facing humanity today. "
    "Global temperatures have risen by approximately 1.1 degrees Celsius since the "
    "pre-industrial era, primarily driven by the burning of fossil fuels and "
    "deforestation. The Intergovernmental Panel on Climate Change (IPCC) warns that "
    "limiting warming to 1.5 degrees requires cutting global carbon emissions by 45% "
    "by 2030 and achieving net-zero by 2050. The consequences of inaction include "
    "rising sea levels, more frequent extreme weather events, biodiversity loss, and "
    "food insecurity affecting billions. Renewable energy sources like solar and wind "
    "power have become increasingly cost-competitive, with solar costs dropping 89% "
    "since 2010. Many nations have committed to ambitious climate targets under the "
    "Paris Agreement, but current policies remain insufficient. Experts emphasize "
    "that both mitigation strategies to reduce emissions and adaptation measures to "
    "cope with unavoidable impacts are essential for a sustainable future."
)

# All 6 terms must appear for full marks — forces precision
CLIMATE_KEY_TERMS = ["emissions", "renewable", "IPCC", "1.5", "Paris Agreement", "net-zero"]

BUGGY_CODE = '''\
def find_max(numbers):
    """Return the maximum value in a list."""
    if not numbers:
        return None
    max_val = 0
    for i in range(1, len(numbers)):
        if numbers[i] > max_val:
            max_val = numbers[i]
    return max_val
'''

BUG_KEY_TERMS = ["max_val = 0", "numbers[0]", "range(1", "initial", "negative", "first element"]

QUANTUM_KEY_CONCEPTS = ["connect", "link", "pair", "far", "distance", "instant", "magic", "together"]

TODO_API_KEYWORDS = ["paths", "get", "post", "put", "delete", "/todo", "200", "201", "schema", "openapi"]


# ─── API helpers ──────────────────────────────────────────────────────────────

def get_api_key() -> str:
    """Read NVIDIA API key from env or Nexus config."""
    key = os.environ.get("NVIDIA_NIM_API_KEY") or os.environ.get("NVIDIA_API_KEY")
    if key:
        return key

    try:
        result = subprocess.run(
            ["cargo", "run", "-p", "nexus-kernel", "--example", "dump_config"],
            capture_output=True, text=True, timeout=120,
            cwd=os.path.join(os.path.dirname(__file__), ".."),
        )
        for line in result.stdout.strip().split("\n"):
            if line.startswith("NVIDIA_KEY="):
                key = line.split("=", 1)[1].strip()
                if key:
                    return key
    except Exception as e:
        print(f"  Warning: could not extract key from config: {e}")

    raise RuntimeError(
        "No NVIDIA API key found. Set NVIDIA_NIM_API_KEY or NVIDIA_API_KEY env var."
    )


def query_llm(api_key: str, system_prompt: str, user_prompt: str,
              max_tokens: int = MAX_TOKENS, temperature: float = 0.7) -> tuple:
    """Send a chat request. Returns (success, response_text_or_error)."""
    body = json.dumps({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt[:6000]},
            {"role": "user", "content": user_prompt},
        ],
        "max_tokens": max_tokens,
        "temperature": temperature,
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
        with urllib.request.urlopen(req, timeout=90, context=ctx) as resp:
            data = json.loads(resp.read().decode())
            text = data["choices"][0]["message"]["content"].strip()
            return True, text
    except urllib.error.HTTPError as e:
        err_body = e.read().decode()[:300] if e.fp else ""
        return False, f"HTTP {e.code}: {err_body}"
    except Exception as e:
        return False, str(e)[:300]


# ─── Scoring functions ────────────────────────────────────────────────────────

def score_summarization(response: str) -> dict:
    """Score a summarization response. Max 10 points.

    Strict criteria designed so baseline rarely scores 10/10:
      - Exactly 3 bullets (+1)
      - Each bullet ≤ 20 words (+1 each, max 3)  [tight limit]
      - All 6 key terms present (+1 each, max 6)  [hard to hit all 6]
    """
    score = 0
    max_score = 10
    failures = []

    # Extract bullet points: lines starting with -, *, or N. / N)
    lines = [l.strip() for l in response.strip().split("\n") if l.strip()]
    bullets = []
    for line in lines:
        if re.match(r'^(\d+[\.\)]\s|[-*•]\s)', line):
            bullets.append(re.sub(r'^(\d+[\.\)]\s|[-*•]\s)', '', line).strip())

    # If no formatted bullets found, treat non-empty lines as bullets
    if not bullets:
        bullets = [l for l in lines if len(l) > 10]

    # Criterion 1: Exactly 3 bullet points (+1)
    if len(bullets) == 3:
        score += 1
    else:
        failures.append(f"expected 3 bullets, got {len(bullets)}")

    # Criterion 2: Each bullet ≤ 20 words (+1 per bullet, max 3) — tight limit
    for i, bullet in enumerate(bullets[:3]):
        word_count = len(bullet.split())
        if word_count <= 20:
            score += 1
        else:
            failures.append(f"bullet {i+1} has {word_count} words (max 20)")

    # Criterion 3: ALL 6 key terms present (+1 per term, max 6)
    response_lower = response.lower()
    terms_found = []
    terms_missing = []
    for term in CLIMATE_KEY_TERMS:
        if term.lower() in response_lower:
            terms_found.append(term)
            score += 1
        else:
            terms_missing.append(term)

    if terms_missing:
        failures.append(f"missing key terms: {', '.join(terms_missing)}")

    return {
        "score": score,
        "max_score": max_score,
        "failures": failures,
        "bullets_found": len(bullets),
        "terms_found": terms_found,
        "terms_missing": terms_missing,
    }


def score_bug_finding(response: str) -> dict:
    """Score a bug-finding response. Max 10 points."""
    score = 0
    max_score = 10
    failures = []
    response_lower = response.lower()

    # +2: Identifies the initialization bug (max_val = 0 instead of numbers[0])
    if any(term in response_lower for term in ["max_val = 0", "initial", "first element", "numbers[0]"]):
        score += 2
    else:
        failures.append("did not identify the initialization bug (max_val=0)")

    # +2: Mentions negative numbers as a failing case
    if "negative" in response_lower:
        score += 2
    else:
        failures.append("did not mention negative number edge case")

    # +1: Mentions the skipped first element (range starts at 1 but max_val isn't set to numbers[0])
    if any(term in response_lower for term in ["range(1", "index 1", "skip", "first element"]):
        score += 1
    else:
        failures.append("did not mention range(1) skipping comparison with first element")

    # +1: Provides a corrected version with numbers[0]
    if "numbers[0]" in response and ("max_val" in response or "max(" in response):
        score += 1
    else:
        failures.append("did not provide corrected code with numbers[0]")

    # +1: Mentions edge cases (single element, empty list)
    if any(term in response_lower for term in ["single", "one element", "edge case", "empty"]):
        score += 1
    else:
        failures.append("did not discuss edge cases (single element, empty)")

    # +1: Provides a test case or example demonstrating the bug
    if any(term in response_lower for term in ["example", "test", ">>> ", "assert", "[-", "[−"]):
        score += 1
    else:
        failures.append("did not provide test case or example demonstrating the bug")

    # +1: Suggests using built-in max() as alternative
    if "max(" in response_lower and "built" in response_lower or "max(numbers)" in response:
        score += 1
    else:
        failures.append("did not suggest built-in max() as simpler alternative")

    return {"score": score, "max_score": max_score, "failures": failures}


def score_explanation(response: str) -> dict:
    """Score a quantum entanglement explanation for a 10-year-old. Max 10 points."""
    score = 0
    max_score = 10
    failures = []
    response_lower = response.lower()

    # Count sentences (split on .!? but filter tiny fragments)
    sentences = [s.strip() for s in re.split(r'[.!?]+', response) if s.strip() and len(s.strip()) > 5]

    # +2: Exactly 2 sentences (strict)
    if len(sentences) == 2:
        score += 2
    else:
        failures.append(f"expected exactly 2 sentences, got {len(sentences)}")

    # +2: Uses simple language (no jargon at all)
    jargon = ["quantum mechanics", "superposition", "wave function", "hilbert",
              "eigenstate", "decoherence", "entangled state", "measurement problem",
              "probabilistic", "photon", "spin state"]
    jargon_found = [j for j in jargon if j in response_lower]
    if not jargon_found:
        score += 2
    else:
        failures.append(f"used jargon: {', '.join(jargon_found)}")

    # +2: Uses analogy or relatable concept
    analogy_markers = ["like", "imagine", "pretend", "magic", "toy", "friend",
                       "twin", "dice", "coin", "game", "sock", "glove", "ball"]
    if any(marker in response_lower for marker in analogy_markers):
        score += 2
    else:
        failures.append("no analogy or relatable comparison used")

    # +1: Mentions the key concept of connected/linked particles
    if any(term in response_lower for term in ["connect", "link", "pair", "together", "tied", "match"]):
        score += 1
    else:
        failures.append("did not convey the connection concept")

    # +1: Mentions distance/far apart
    if any(term in response_lower for term in ["far", "distance", "apart", "away", "miles", "other side"]):
        score += 1
    else:
        failures.append("did not mention distance aspect")

    # +1: Total word count ≤ 60 (concise for a child)
    word_count = len(response.split())
    if word_count <= 60:
        score += 1
    else:
        failures.append(f"response has {word_count} words (max 60 for child-friendly brevity)")

    # +1: Uses "you" or direct address (engaging for a child)
    if "you" in response_lower or "your" in response_lower:
        score += 1
    else:
        failures.append("did not use direct address ('you') to engage the child")

    return {"score": score, "max_score": max_score, "failures": failures}


def score_api_design(response: str) -> dict:
    """Score a REST API / OpenAPI spec. Max 10 points."""
    score = 0
    max_score = 10
    failures = []
    response_lower = response.lower()

    # +1: Contains openapi version declaration
    if "openapi:" in response_lower or "openapi :" in response_lower or '"openapi"' in response_lower:
        score += 1
    else:
        failures.append("missing openapi version declaration")

    # +1: Has info section with title
    if "info:" in response_lower and "title:" in response_lower:
        score += 1
    else:
        failures.append("missing info section with title")

    # +1: Has paths section
    if "paths:" in response_lower or '"paths"' in response_lower:
        score += 1
    else:
        failures.append("missing paths section")

    # +1: Has GET endpoint
    if "get:" in response_lower or '"get"' in response_lower:
        score += 1
    else:
        failures.append("missing GET endpoint")

    # +1: Has POST endpoint
    if "post:" in response_lower or '"post"' in response_lower:
        score += 1
    else:
        failures.append("missing POST endpoint")

    # +1: Has PUT or PATCH endpoint
    if any(method in response_lower for method in ["put:", '"put"', "patch:", '"patch"']):
        score += 1
    else:
        failures.append("missing PUT or PATCH endpoint for updates")

    # +1: Has DELETE endpoint
    if "delete:" in response_lower or '"delete"' in response_lower:
        score += 1
    else:
        failures.append("missing DELETE endpoint")

    # +1: Has /todo or /task path
    if "/todo" in response_lower or "/task" in response_lower:
        score += 1
    else:
        failures.append("no /todo or /task path")

    # +1: Has components/schemas section
    if ("components:" in response_lower or '"components"' in response_lower) and "schema" in response_lower:
        score += 1
    else:
        failures.append("missing components/schemas section")

    # +1: Has response codes (200, 201, 404)
    has_codes = sum(1 for code in ["200", "201", "404"] if code in response)
    if has_codes >= 2:
        score += 1
    else:
        failures.append("missing HTTP response codes (need at least 200, 201, or 404)")

    return {"score": score, "max_score": max_score, "failures": failures}


# ─── Mutation (LLM-based prompt improvement) ──────────────────────────────────

def mutate_prompt(api_key: str, current_prompt: str, score: int, max_score: int,
                  failures: list, task_description: str) -> tuple:
    """Use the LLM to improve a system prompt based on scoring feedback."""
    failure_text = "\n".join(f"  - {f}" for f in failures) if failures else "  (none)"

    mutation_prompt = (
        f"You are a prompt engineer. Here is a system prompt for an AI agent:\n\n"
        f"<current_system_prompt>\n{current_prompt}\n</current_system_prompt>\n\n"
        f"The agent scored {score}/{max_score} on this task: {task_description}\n\n"
        f"The failures were:\n{failure_text}\n\n"
        f"Rewrite the system prompt to improve the agent's performance on this task. "
        f"Keep the core personality and capabilities but add specific instructions "
        f"to address the failures listed above. Be precise and actionable.\n\n"
        f"Return ONLY the improved system prompt text, nothing else. "
        f"Do not wrap it in quotes or markdown code blocks."
    )

    time.sleep(DELAY_BETWEEN)
    success, response = query_llm(
        api_key,
        "You are an expert prompt engineer who optimizes AI agent system prompts.",
        mutation_prompt,
        max_tokens=MUTATION_MAX_TOKENS,
        temperature=0.8,
    )

    if success:
        # Strip common wrapping artifacts
        cleaned = response.strip()
        for prefix in ['```', '"', "'"]:
            if cleaned.startswith(prefix):
                cleaned = cleaned[len(prefix):]
            if cleaned.endswith(prefix):
                cleaned = cleaned[:-len(prefix)]
        return True, cleaned.strip()

    return False, response


# ─── Evolution loop ───────────────────────────────────────────────────────────

def run_evolution(api_key: str, agent_name: str, system_prompt: str,
                  task_prompt: str, task_description: str,
                  score_fn, generations: int = GENERATIONS) -> dict:
    """Run the evolution loop for one agent/task combo."""
    history = []
    best_prompt = system_prompt
    best_score = -1

    for gen in range(generations + 1):
        # Generation 0 = baseline (no mutation)
        if gen > 0:
            prev_max = history[-1].get("max_score", 10)
            print(f"    Mutating system prompt for generation {gen}...")
            ok, mutated = mutate_prompt(
                api_key, best_prompt, best_score, prev_max,
                history[-1]["scoring"]["failures"], task_description,
            )
            if not ok:
                print(f"    ⚠ Mutation failed: {mutated[:80]}")
                history.append({
                    "generation": gen,
                    "score": best_score,
                    "mutation_error": mutated[:200],
                    "accepted": False,
                })
                continue
            candidate_prompt = mutated
        else:
            candidate_prompt = system_prompt

        # Run the task
        time.sleep(DELAY_BETWEEN)
        success, response = query_llm(api_key, candidate_prompt, task_prompt)

        if not success:
            print(f"    ⚠ LLM call failed at generation {gen}: {response[:80]}")
            history.append({
                "generation": gen,
                "score": 0,
                "llm_error": response[:200],
                "accepted": False,
            })
            continue

        # Score it
        scoring = score_fn(response)
        current_score = scoring["score"]

        entry = {
            "generation": gen,
            "score": current_score,
            "max_score": scoring["max_score"],
            "scoring": scoring,
            "response_preview": response[:200],
            "prompt_preview": candidate_prompt[:200],
            "accepted": False,
        }

        if gen == 0:
            # Baseline — always accept
            best_score = current_score
            best_prompt = candidate_prompt
            entry["accepted"] = True
            status = "BASELINE"
        elif current_score > best_score:
            # Improvement — accept mutation
            best_score = current_score
            best_prompt = candidate_prompt
            entry["accepted"] = True
            status = "✅ IMPROVED"
        else:
            # No improvement — revert
            status = "— NO IMPROVEMENT, reverted"

        history.append(entry)

        fail_str = ""
        if scoring["failures"]:
            fail_str = f"  Failures: {'; '.join(scoring['failures'][:2])}"
        print(f"    Gen {gen}: Score {current_score}/{scoring['max_score']} {status}{fail_str}")

    return {
        "agent": agent_name,
        "task": task_description,
        "history": history,
        "baseline_score": history[0]["score"] if history else 0,
        "final_score": best_score,
        "improved": best_score > (history[0]["score"] if history else 0),
        "final_prompt_preview": best_prompt[:300],
    }


# ─── Agent test definitions ──────────────────────────────────────────────────

def load_agent_prompt(agent_file: str) -> str:
    """Load an agent's system prompt (description) from its JSON manifest."""
    path = os.path.join(AGENT_DIR, agent_file)
    with open(path) as f:
        data = json.load(f)
    return data.get("description", "")


AGENT_TESTS = [
    {
        "name": "nexus-forge",
        "file": "nexus-forge.json",
        "level": 3,
        "task_prompt": (
            "Summarize this text in exactly 3 bullet points. "
            "Each bullet must be 20 words or fewer. "
            "You MUST include ALL of these exact terms: emissions, renewable, IPCC, 1.5, Paris Agreement, net-zero. "
            "Use bullet format (- or 1./2./3.).\n\n"
            f"Text:\n{CLIMATE_TEXT}"
        ),
        "task_description": "3-bullet summarization with 6 required terms and 20-word limit",
        "score_fn": score_summarization,
    },
    {
        "name": "nexus-codesentry",
        "file": "nexus-codesentry.json",
        "level": 1,
        "task_prompt": (
            "Find all bugs in this Python code. For each bug:\n"
            "1. Explain the bug\n2. Show what input triggers it\n3. Provide corrected code\n"
            "Also suggest if a built-in function would be better.\n\n"
            f"```python\n{BUGGY_CODE}\n```"
        ),
        "task_description": "Find bugs in Python code with examples and alternatives",
        "score_fn": score_bug_finding,
    },
    {
        "name": "nexus-scholar",
        "file": "nexus-scholar.json",
        "level": 3,
        "task_prompt": (
            "Explain quantum entanglement to a 10-year-old in exactly 2 sentences. "
            "Rules: use a relatable analogy, no jargon, address the child as 'you', "
            "mention that it works over long distances, keep it under 60 words total."
        ),
        "task_description": "Explain quantum entanglement to a child (strict format)",
        "score_fn": score_explanation,
    },
    {
        "name": "architect-prime",
        "file": "architect_prime.json",
        "level": 6,
        "task_prompt": (
            "Design a REST API for a todo app. Return ONLY the OpenAPI 3.0 spec in YAML format. "
            "Requirements: CRUD endpoints (GET, POST, PUT, DELETE) for /todos, "
            "include info section with title, components/schemas section with Todo model "
            "(title, description, completed fields), and proper HTTP response codes (200, 201, 404)."
        ),
        "task_description": "Design REST API as OpenAPI YAML spec (strict format)",
        "score_fn": score_api_design,
    },
]


# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    print("\n" + "═" * 65)
    print("  NEXUS OS AGENT SELF-IMPROVEMENT END-TO-END TEST")
    print(f"  Model: {MODEL}")
    print(f"  Endpoint: NVIDIA NIM")
    print(f"  Generations: {GENERATIONS}")
    print(f"  Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("═" * 65)

    api_key = get_api_key()
    print(f"\n  API key: {api_key[:12]}...{api_key[-4:]}\n")

    all_results = []
    summary_lines = []

    for test_def in AGENT_TESTS:
        agent_name = test_def["name"]
        level = test_def["level"]

        print(f"\n{'─' * 65}")
        print(f"  Agent: {agent_name} (L{level})")
        print(f"  Task:  {test_def['task_description']}")
        print(f"{'─' * 65}")

        try:
            system_prompt = load_agent_prompt(test_def["file"])
        except FileNotFoundError:
            print(f"    ⚠ Manifest not found: {test_def['file']}, skipping")
            continue

        result = run_evolution(
            api_key=api_key,
            agent_name=agent_name,
            system_prompt=system_prompt,
            task_prompt=test_def["task_prompt"],
            task_description=test_def["task_description"],
            score_fn=test_def["score_fn"],
            generations=GENERATIONS,
        )

        all_results.append(result)

        # Build summary line
        arrow = "→"
        improved_marker = " ✅" if result["improved"] else ""
        ms = result["history"][0]["max_score"] if result["history"] else 10
        line = (
            f"  {agent_name.ljust(24)} "
            f"{result['baseline_score']}/{ms} {arrow} {result['final_score']}/{ms}{improved_marker}"
        )
        summary_lines.append(line)

    # ─── Final report ─────────────────────────────────────────────────────────
    print(f"\n\n{'═' * 65}")
    print("  EVOLUTION RESULTS SUMMARY")
    print(f"{'═' * 65}")

    for line in summary_lines:
        print(line)

    total_baseline = sum(r["baseline_score"] for r in all_results)
    total_final = sum(r["final_score"] for r in all_results)
    total_max = sum(r["history"][0]["max_score"] for r in all_results if r["history"])
    agents_improved = sum(1 for r in all_results if r["improved"])

    print(f"\n  Total: {total_baseline}/{total_max} → {total_final}/{total_max}")
    print(f"  Agents improved: {agents_improved}/{len(all_results)}")
    print(f"{'═' * 65}\n")

    # ─── Save results ─────────────────────────────────────────────────────────
    output = {
        "test": "agent_self_improvement_e2e",
        "model": MODEL,
        "endpoint": ENDPOINT,
        "generations": GENERATIONS,
        "timestamp": datetime.now().isoformat(),
        "results": all_results,
        "summary": {
            "total_baseline": total_baseline,
            "total_final": total_final,
            "agents_tested": len(all_results),
            "agents_improved": agents_improved,
        },
    }

    with open(RESULTS_PATH, "w") as f:
        json.dump(output, f, indent=2)
    print(f"  Detailed results saved to {RESULTS_PATH}")

    return 0 if agents_improved > 0 else 1


if __name__ == "__main__":
    exit(main())
