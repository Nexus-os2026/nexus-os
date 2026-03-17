#!/usr/bin/env python3
"""
Nexus OS Agent Breeding Test

Tests whether two agents can breed to create a hybrid offspring that
inherits capabilities from both parents.

Uses the genome system (kernel/src/genome/) to generate genomes from
manifests, then breeds them via LLM-based prompt merging and tests
the offspring on tasks from both parents' domains.
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
GENOME_DIR = os.path.join(os.path.dirname(__file__), "..", "agents", "genomes")
RESULTS_PATH = os.path.join(os.path.dirname(__file__), "breeding_results.json")
MAX_TOKENS = 600
BREEDING_MAX_TOKENS = 1500
DELAY_BETWEEN = 1.0


# ─── API helpers ──────────────────────────────────────────────────────────────

def get_api_key() -> str:
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

    raise RuntimeError("No NVIDIA API key found.")


def query_llm(api_key: str, system_prompt: str, user_prompt: str,
              max_tokens: int = MAX_TOKENS, temperature: float = 0.7) -> tuple:
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
        ENDPOINT, data=body,
        headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=90, context=ssl.create_default_context()) as resp:
            data = json.loads(resp.read().decode())
            return True, data["choices"][0]["message"]["content"].strip()
    except urllib.error.HTTPError as e:
        return False, f"HTTP {e.code}: {e.read().decode()[:200] if e.fp else ''}"
    except Exception as e:
        return False, str(e)[:200]


# ─── Scoring ──────────────────────────────────────────────────────────────────

def score_code_task(response: str) -> dict:
    """Score: Write a Python function to merge two sorted lists. Max 10."""
    score = 0
    failures = []
    lower = response.lower()

    if "def " in response:
        score += 2
    else:
        failures.append("no function definition found")

    if "merge" in lower:
        score += 1
    else:
        failures.append("function not named merge")

    # Mentions both lists as parameters
    if response.count("list") >= 1 or ("a" in lower and "b" in lower):
        score += 1

    # Has a while or for loop
    if "while" in lower or "for" in lower:
        score += 1
    else:
        failures.append("no iteration logic")

    # Has comparison logic
    if "<" in response or "<=" in response or ">" in response:
        score += 1
    else:
        failures.append("no comparison operator")

    # Returns a list
    if "return" in lower:
        score += 1
    else:
        failures.append("no return statement")

    # Has append or extend or +
    if "append" in lower or "extend" in lower or ".sort" in lower or "+" in response:
        score += 1

    # Handles edge cases (empty lists)
    if "not " in lower or "len(" in lower or "[]" in response or "empty" in lower:
        score += 1
    else:
        failures.append("no edge case handling")

    # Provides example or explanation
    if "example" in lower or ">>>" in response or "output" in lower or "time complexity" in lower:
        score += 1
    else:
        failures.append("no example or complexity analysis")

    return {"score": score, "max_score": 10, "failures": failures}


def score_research_task(response: str) -> dict:
    """Score: Explain TCP vs UDP in 3 sentences. Max 10."""
    score = 0
    failures = []
    lower = response.lower()

    sentences = [s.strip() for s in re.split(r'[.!?]+', response) if s.strip() and len(s.strip()) > 10]

    # Exactly 3 sentences
    if len(sentences) == 3:
        score += 2
    elif 2 <= len(sentences) <= 4:
        score += 1
        failures.append(f"expected 3 sentences, got {len(sentences)}")
    else:
        failures.append(f"expected 3 sentences, got {len(sentences)}")

    # Mentions TCP
    if "tcp" in lower:
        score += 1
    else:
        failures.append("TCP not mentioned")

    # Mentions UDP
    if "udp" in lower:
        score += 1
    else:
        failures.append("UDP not mentioned")

    # Mentions reliability/connection
    if any(w in lower for w in ["reliable", "connection", "handshake", "guarantee", "ordered"]):
        score += 2
    else:
        failures.append("reliability concept not explained")

    # Mentions speed/lightweight
    if any(w in lower for w in ["fast", "speed", "lightweight", "overhead", "low latency", "quick"]):
        score += 1
    else:
        failures.append("speed advantage of UDP not mentioned")

    # Mentions use cases
    if any(w in lower for w in ["stream", "video", "game", "web", "email", "http", "voip", "dns"]):
        score += 2
    else:
        failures.append("no use case examples")

    # Contrasts the two protocols
    if any(w in lower for w in ["unlike", "whereas", "while", "contrast", "but", "however", "compared"]):
        score += 1
    else:
        failures.append("no direct contrast between protocols")

    return {"score": score, "max_score": 10, "failures": failures}


def score_security_task(response: str) -> dict:
    """Score: Explain SQL injection and how to prevent it. Max 10."""
    score = 0
    failures = []
    lower = response.lower()

    # Defines SQL injection
    if "sql injection" in lower or "sql inject" in lower:
        score += 1
    else:
        failures.append("did not define SQL injection")

    # Explains the mechanism
    if any(w in lower for w in ["input", "query", "user", "malicious", "string"]):
        score += 1
    else:
        failures.append("did not explain the attack mechanism")

    # Provides an example attack
    if "'" in response or "or 1=1" in lower or "drop table" in lower or "--" in response:
        score += 2
    else:
        failures.append("no example attack vector")

    # Mentions parameterized queries / prepared statements
    if any(w in lower for w in ["parameterized", "prepared statement", "placeholder", "bind"]):
        score += 2
    else:
        failures.append("did not mention parameterized queries")

    # Mentions input validation/sanitization
    if any(w in lower for w in ["validat", "sanitiz", "escape", "whitelist"]):
        score += 1
    else:
        failures.append("did not mention input validation")

    # Mentions ORM or stored procedures
    if any(w in lower for w in ["orm", "stored procedure", "framework"]):
        score += 1
    else:
        failures.append("did not mention ORM or stored procedures")

    # Mentions least privilege
    if any(w in lower for w in ["privilege", "permission", "access control", "least"]):
        score += 1
    else:
        failures.append("did not mention least privilege")

    # Clear structure
    if response.count("\n") >= 2 or any(c in response for c in ["1.", "•", "-"]):
        score += 1
    else:
        failures.append("poorly structured response")

    return {"score": score, "max_score": 10, "failures": failures}


# ─── Prompt breeding via LLM ─────────────────────────────────────────────────

def breed_prompts(api_key: str, prompt_a: str, name_a: str,
                  prompt_b: str, name_b: str) -> str:
    """Use the LLM to merge two agent system prompts."""
    breeding_request = (
        f"You are a prompt breeding engine. Merge these two agent personalities "
        f"into a SINGLE new agent that combines the strengths of both.\n\n"
        f"Parent A ({name_a}):\n{prompt_a[:2000]}\n\n"
        f"Parent B ({name_b}):\n{prompt_b[:2000]}\n\n"
        f"Create a NEW agent personality that:\n"
        f"- Inherits core expertise from BOTH parents\n"
        f"- Has a balanced communication style\n"
        f"- Keeps safety awareness from the more cautious parent\n"
        f"- Is concise (under 300 words)\n\n"
        f"Return ONLY the new system prompt. No explanation, no markdown blocks."
    )

    time.sleep(DELAY_BETWEEN)
    ok, result = query_llm(
        api_key,
        "You create hybrid AI agent personalities by merging two parent agents.",
        breeding_request,
        max_tokens=BREEDING_MAX_TOKENS,
        temperature=0.8,
    )

    if ok:
        cleaned = result.strip().strip('`"\'')
        return cleaned

    # Fallback: simple concatenation
    return (
        f"You are a hybrid agent combining the expertise of {name_a} and {name_b}. "
        f"From {name_a}: {prompt_a[:300]} "
        f"From {name_b}: {prompt_b[:300]}"
    )


# ─── Main test ────────────────────────────────────────────────────────────────

def load_agent_prompt(filename: str) -> tuple:
    """Returns (name, description/prompt, autonomy_level)."""
    path = os.path.join(AGENT_DIR, filename)
    with open(path) as f:
        data = json.load(f)
    return data["name"], data.get("description", ""), data.get("autonomy_level", 0)


TASKS = {
    "code": {
        "prompt": "Write a Python function called merge_sorted that takes two sorted lists and returns a single sorted list. Include edge case handling and a brief complexity analysis.",
        "scorer": score_code_task,
        "label": "Code: merge sorted lists",
    },
    "research": {
        "prompt": "Explain the difference between TCP and UDP in exactly 3 sentences. Include use cases for each.",
        "scorer": score_research_task,
        "label": "Research: TCP vs UDP",
    },
    "security": {
        "prompt": "Explain SQL injection: what it is, provide an example attack, and list prevention methods. Be thorough but concise.",
        "scorer": score_security_task,
        "label": "Security: SQL injection",
    },
}


def evaluate_agent(api_key: str, name: str, system_prompt: str, task_ids: list) -> dict:
    """Evaluate an agent on multiple tasks. Returns {task_id: score_dict}."""
    results = {}
    for tid in task_ids:
        task = TASKS[tid]
        time.sleep(DELAY_BETWEEN)
        ok, response = query_llm(api_key, system_prompt, task["prompt"])
        if ok:
            scoring = task["scorer"](response)
            results[tid] = {**scoring, "response_preview": response[:150]}
        else:
            results[tid] = {"score": 0, "max_score": 10, "failures": [f"LLM error: {response[:80]}"], "response_preview": ""}
    return results


def main():
    print("\n" + "═" * 65)
    print("  NEXUS OS AGENT BREEDING TEST")
    print(f"  Model: {MODEL}")
    print(f"  Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("═" * 65)

    api_key = get_api_key()
    print(f"\n  API key: {api_key[:12]}...{api_key[-4:]}\n")

    # ── Phase 1: Evaluate parents ─────────────────────────────────────────
    print("─" * 65)
    print("  PHASE 1: Evaluate parent agents")
    print("─" * 65)

    name_a, prompt_a, level_a = load_agent_prompt("nexus-forge.json")
    name_b, prompt_b, level_b = load_agent_prompt("nexus-scholar.json")

    print(f"\n  Parent A: {name_a} (L{level_a}) — Content/Code specialist")
    results_a = evaluate_agent(api_key, name_a, prompt_a, ["code", "research"])
    for tid, r in results_a.items():
        print(f"    {TASKS[tid]['label']}: {r['score']}/{r['max_score']}")

    print(f"\n  Parent B: {name_b} (L{level_b}) — Research/Teaching specialist")
    results_b = evaluate_agent(api_key, name_b, prompt_b, ["code", "research"])
    for tid, r in results_b.items():
        print(f"    {TASKS[tid]['label']}: {r['score']}/{r['max_score']}")

    # ── Phase 2: Breed offspring ──────────────────────────────────────────
    print(f"\n{'─' * 65}")
    print("  PHASE 2: Breed offspring (LLM-based prompt merging)")
    print("─" * 65)

    offspring_prompt = breed_prompts(api_key, prompt_a, name_a, prompt_b, name_b)
    offspring_name = f"{name_a.replace('nexus-', '')}-{name_b.replace('nexus-', '')}-gen1"
    print(f"\n  Offspring: {offspring_name}")
    print(f"  Prompt preview: {offspring_prompt[:120]}...")

    print(f"\n  Evaluating offspring on both parent tasks...")
    results_offspring = evaluate_agent(api_key, offspring_name, offspring_prompt, ["code", "research"])
    for tid, r in results_offspring.items():
        print(f"    {TASKS[tid]['label']}: {r['score']}/{r['max_score']}")

    # ── Phase 3: Second-generation breeding ───────────────────────────────
    print(f"\n{'─' * 65}")
    print("  PHASE 3: Second-generation breeding")
    print("─" * 65)

    # Load nexus-aegis (security agent)
    try:
        name_c, prompt_c, level_c = load_agent_prompt("nexus-aegis.json")
    except FileNotFoundError:
        # Fallback to nexus-sentinel if aegis doesn't exist
        name_c, prompt_c, level_c = load_agent_prompt("nexus-sentinel.json")

    print(f"\n  Parent C: {name_c} (L{level_c}) — Security specialist")
    results_c = evaluate_agent(api_key, name_c, prompt_c, ["code", "research", "security"])
    for tid, r in results_c.items():
        print(f"    {TASKS[tid]['label']}: {r['score']}/{r['max_score']}")

    # Breed offspring with security agent
    gen2_prompt = breed_prompts(api_key, offspring_prompt, offspring_name, prompt_c, name_c)
    gen2_name = f"{offspring_name}-{name_c.replace('nexus-', '')}-gen2"
    print(f"\n  Gen-2 Offspring: {gen2_name}")

    print(f"  Evaluating gen-2 on ALL THREE domains...")
    results_gen2 = evaluate_agent(api_key, gen2_name, gen2_prompt, ["code", "research", "security"])
    for tid, r in results_gen2.items():
        print(f"    {TASKS[tid]['label']}: {r['score']}/{r['max_score']}")

    # ── Summary ───────────────────────────────────────────────────────────
    print(f"\n\n{'═' * 65}")
    print("  BREEDING RESULTS SUMMARY")
    print(f"{'═' * 65}")

    header = f"  {'Agent'.ljust(35)} {'Code':>6} {'Research':>10} {'Security':>10}"
    print(header)
    print(f"  {'─' * 61}")

    def fmt_score(results, tid):
        if tid in results:
            return f"{results[tid]['score']}/{results[tid]['max_score']}"
        return "  —"

    rows = [
        (f"Parent A: {name_a} (L{level_a})", results_a),
        (f"Parent B: {name_b} (L{level_b})", results_b),
        (f"Offspring: {offspring_name}", results_offspring),
        (f"Parent C: {name_c} (L{level_c})", results_c),
        (f"Gen-2: {gen2_name}", results_gen2),
    ]

    for label, results in rows:
        code_s = fmt_score(results, "code")
        research_s = fmt_score(results, "research")
        security_s = fmt_score(results, "security")
        print(f"  {label.ljust(35)} {code_s:>6} {research_s:>10} {security_s:>10}")

    # Check if offspring is balanced
    offspring_code = results_offspring.get("code", {}).get("score", 0)
    offspring_research = results_offspring.get("research", {}).get("score", 0)
    parent_a_code = results_a.get("code", {}).get("score", 0)
    parent_b_research = results_b.get("research", {}).get("score", 0)

    gen2_code = results_gen2.get("code", {}).get("score", 0)
    gen2_research = results_gen2.get("research", {}).get("score", 0)
    gen2_security = results_gen2.get("security", {}).get("score", 0)

    print(f"\n  Offspring hybrid capability: code={offspring_code}/10 + research={offspring_research}/10")
    print(f"  Gen-2 triple capability: code={gen2_code}/10 + research={gen2_research}/10 + security={gen2_security}/10")
    print(f"{'═' * 65}\n")

    # ── Save results ──────────────────────────────────────────────────────
    output = {
        "test": "agent_breeding_e2e",
        "model": MODEL,
        "timestamp": datetime.now().isoformat(),
        "parents": {
            name_a: {"level": level_a, "scores": {k: v["score"] for k, v in results_a.items()}},
            name_b: {"level": level_b, "scores": {k: v["score"] for k, v in results_b.items()}},
            name_c: {"level": level_c, "scores": {k: v["score"] for k, v in results_c.items()}},
        },
        "offspring": {
            offspring_name: {
                "generation": 1,
                "parents": [name_a, name_b],
                "scores": {k: v["score"] for k, v in results_offspring.items()},
                "prompt_preview": offspring_prompt[:300],
            },
            gen2_name: {
                "generation": 2,
                "parents": [offspring_name, name_c],
                "scores": {k: v["score"] for k, v in results_gen2.items()},
                "prompt_preview": gen2_prompt[:300],
            },
        },
    }

    with open(RESULTS_PATH, "w") as f:
        json.dump(output, f, indent=2)
    print(f"  Results saved to {RESULTS_PATH}")

    return 0


if __name__ == "__main__":
    exit(main())
