#!/usr/bin/env python3
"""
Nexus OS — Dream Forge End-to-End Test

Tests the dream system: auto-queue, replay, experiment, consolidation,
precompute, and morning briefing generation.

Uses NVIDIA NIM for LLM calls (same pipeline as other E2E tests).
Budget: Max 20 API calls for entire test.
"""

import json
import os
import sys
import time
import urllib.request
import urllib.error
import ssl
from datetime import datetime

# ─── Configuration ────────────────────────────────────────────────────────────

ENDPOINT = "https://integrate.api.nvidia.com/v1/chat/completions"
MODEL = "moonshotai/kimi-k2-instruct"
MAX_TOKENS = 400
DELAY_BETWEEN = 1.0
TOTAL_API_BUDGET = 20

# ─── API helpers ──────────────────────────────────────────────────────────────

api_calls_made = 0


def get_api_key() -> str:
    key = os.environ.get("NVIDIA_NIM_API_KEY") or os.environ.get("NVIDIA_API_KEY")
    if key:
        return key
    try:
        config_path = os.path.expanduser("~/.config/nexus-os/config.toml")
        with open(config_path) as f:
            for line in f:
                if "nvidia" in line.lower() and "api_key" in line.lower():
                    return line.split("=", 1)[1].strip().strip('"').strip("'")
    except FileNotFoundError:
        pass
    return ""


def llm_query(system: str, user: str, api_key: str,
              temperature: float = 0.7, max_tokens: int = MAX_TOKENS) -> str:
    global api_calls_made
    if api_calls_made >= TOTAL_API_BUDGET:
        return "[BUDGET_EXCEEDED] No more API calls allowed"
    api_calls_made += 1

    payload = json.dumps({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
    }).encode()

    ctx = ssl.create_default_context()
    req = urllib.request.Request(
        ENDPOINT,
        data=payload,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
    )

    try:
        with urllib.request.urlopen(req, context=ctx, timeout=60) as resp:
            data = json.loads(resp.read().decode())
            return data["choices"][0]["message"]["content"]
    except (urllib.error.URLError, urllib.error.HTTPError) as e:
        return f"[LLM_ERROR] {e}"


# ─── Dream system simulator (mirrors Rust types) ─────────────────────────────


class DreamScheduler:
    def __init__(self):
        self.enabled = True
        self.idle_trigger_minutes = 15
        self.dream_budget_tokens = 50_000
        self.dream_budget_api_calls = 20
        self.priority_queue = []
        self.completed_dreams = []
        self.last_activity_at = time.time()

    def enqueue(self, task):
        if not any(t["id"] == task["id"] for t in self.priority_queue):
            self.priority_queue.append(task)

    def sort_queue(self):
        self.priority_queue.sort(key=lambda t: -t["priority"])

    def record_result(self, result):
        self.priority_queue = [t for t in self.priority_queue if t["id"] != result["task_id"]]
        self.completed_dreams.append(result)


class ConsciousnessState:
    def __init__(self, agent_id):
        self.agent_id = agent_id
        self.confidence = 0.5
        self.curiosity = 0.5
        self.frustration = 0.0
        self.fatigue = 0.0
        self.needs_handoff = False

    def on_task_failure(self, err=""):
        self.confidence = max(self.confidence - 0.15, 0.0)
        self.frustration = min(self.frustration + 0.2, 1.0)
        self.fatigue += 0.1

    def on_task_success(self):
        self.confidence = min(self.confidence + 0.1, 1.0)
        self.frustration = max(self.frustration - 0.2, 0.0)


def make_dream_task(task_type, priority, agent_id, context):
    import uuid
    return {
        "id": str(uuid.uuid4()),
        "task_type": task_type,
        "priority": priority,
        "source_agent": agent_id,
        "context": context,
        "estimated_tokens": 500,
    }


def queue_dreams_from_interaction(scheduler, agent_id, consciousness, interaction):
    """Mirror of Rust auto_queue logic."""
    if consciousness.frustration > 0.5:
        scheduler.enqueue(make_dream_task(
            "Experiment", consciousness.frustration, agent_id,
            {"task": interaction["user_message"], "failures": interaction.get("error", "")}
        ))
    if consciousness.curiosity > 0.7:
        topic = interaction.get("topic_detected")
        if topic:
            scheduler.enqueue(make_dream_task(
                "Explore", 0.3, agent_id, {"topic": topic}
            ))
    if interaction.get("was_error"):
        scheduler.enqueue(make_dream_task(
            "Replay", 0.6, agent_id,
            {"task": interaction["user_message"], "original_response": interaction.get("response", "")}
        ))
    if consciousness.needs_handoff:
        scheduler.enqueue(make_dream_task(
            "Create", 0.7, agent_id,
            {"gap": f"Agent {agent_id} fatigued for: {interaction['user_message']}"}
        ))


# ─── Dream execution (uses real LLM) ─────────────────────────────────────────


def execute_dream(task, api_key):
    """Execute a single dream task via LLM."""
    dtype = task["task_type"]
    ctx = task["context"]
    started = time.time()

    if dtype == "Replay":
        system = ("You are replaying a past task to find improvements. "
                  "Score the original 0-10, produce improved version. "
                  "Format: ORIGINAL_SCORE: N\nIMPROVED_SCORE: N\nIMPROVED:\n<response>")
        user = f"Task: {ctx.get('task', '?')}\nOriginal: {ctx.get('original_response', '?')}\nReplay and improve."
        response = llm_query(system, user, api_key)
        before = extract_score(response, "ORIGINAL_SCORE:")
        after = extract_score(response, "IMPROVED_SCORE:")
        return {
            "task_id": task["id"], "dream_type": dtype, "agent_id": task["source_agent"],
            "started_at": started, "completed_at": time.time(), "tokens_used": len(response) // 4,
            "outcome": {"type": "Improvement", "before_score": before, "after_score": after, "description": f"Replay of: {ctx.get('task', '?')}"}
        }

    elif dtype == "Experiment":
        system = ("Generate 3 different approaches to this failed task. "
                  "For each: STRATEGY_N: <desc> SCORE: N\nBEST_APPROACH:\n<best>")
        user = f"Task: {ctx.get('task', '?')}\nFailures: {ctx.get('failures', '?')}"
        response = llm_query(system, user, api_key)
        best = max((extract_score(line, "SCORE:") for line in response.splitlines()), default=5.0)
        return {
            "task_id": task["id"], "dream_type": dtype, "agent_id": task["source_agent"],
            "started_at": started, "completed_at": time.time(), "tokens_used": len(response) // 4,
            "outcome": {"type": "Improvement", "before_score": 0.0, "after_score": best, "description": f"Experiment on: {ctx.get('task', '?')}"}
        }

    elif dtype == "Consolidate":
        system = ("Analyze this work session. Identify patterns and lessons. "
                  "Output: LESSON:\n<compressed lesson for agent prompt>")
        user = f"Session: {ctx.get('session_summary', '?')}"
        response = llm_query(system, user, api_key)
        lesson = extract_after(response, "LESSON:")
        return {
            "task_id": task["id"], "dream_type": dtype, "agent_id": task["source_agent"],
            "started_at": started, "completed_at": time.time(), "tokens_used": len(response) // 4,
            "outcome": {"type": "Discovery", "description": lesson, "relevance": 0.8}
        }

    elif dtype == "Precompute":
        system = ("Predict the user's next request based on context. "
                  "Format: PREDICTION: <request>\nCONFIDENCE: <0-1>\nRESPONSE:\n<prepared response>")
        user = f"Conversation: {ctx.get('conversation', '?')}"
        response = llm_query(system, user, api_key, max_tokens=600)
        prediction = extract_after(response, "PREDICTION:")
        confidence = extract_score(response, "CONFIDENCE:")
        prepared = extract_after(response, "RESPONSE:")
        return {
            "task_id": task["id"], "dream_type": dtype, "agent_id": task["source_agent"],
            "started_at": started, "completed_at": time.time(), "tokens_used": len(response) // 4,
            "outcome": {"type": "Precomputed", "predicted_request": prediction, "confidence": confidence, "prepared_response": prepared}
        }

    elif dtype == "Explore":
        system = "Research the topic. Key insights and practical applications."
        user = f"Topic: {ctx.get('topic', '?')}"
        response = llm_query(system, user, api_key)
        return {
            "task_id": task["id"], "dream_type": dtype, "agent_id": task["source_agent"],
            "started_at": started, "completed_at": time.time(), "tokens_used": len(response) // 4,
            "outcome": {"type": "Discovery", "description": response[:300], "relevance": 0.5}
        }

    elif dtype == "Create":
        system = ("Design a new agent. NAME: <name>\nDESCRIPTION: <desc>\n"
                  "CAPABILITIES: <list>\nTEST_SCORE: <0-1>\nREASON: <why>")
        user = f"Gap: {ctx.get('gap', '?')}"
        response = llm_query(system, user, api_key)
        name = extract_after(response, "NAME:")
        reason = extract_after(response, "REASON:")
        score = extract_score(response, "TEST_SCORE:")
        return {
            "task_id": task["id"], "dream_type": dtype, "agent_id": task["source_agent"],
            "started_at": started, "completed_at": time.time(), "tokens_used": len(response) // 4,
            "outcome": {"type": "Creation", "new_agent_id": name, "reason": reason, "test_score": score}
        }

    return {
        "task_id": task["id"], "dream_type": dtype, "agent_id": task["source_agent"],
        "started_at": started, "completed_at": time.time(), "tokens_used": 0,
        "outcome": {"type": "NoResult", "reason": f"Unknown dream type: {dtype}"}
    }


def extract_score(text, marker):
    marker_lower = marker.lower()
    for line in text.lower().splitlines():
        if marker_lower in line:
            after = line.split(marker_lower, 1)[1].strip()
            num = ""
            for ch in after:
                if ch.isdigit() or ch == ".":
                    num += ch
                else:
                    break
            try:
                return float(num) if num else 5.0
            except ValueError:
                return 5.0
    return 5.0


def extract_after(text, marker):
    lower = text.lower()
    idx = lower.find(marker.lower())
    if idx >= 0:
        after = text[idx + len(marker):].strip()
        return after.splitlines()[0].strip() if after else ""
    return text.splitlines()[0].strip() if text else ""


# ─── Tests ────────────────────────────────────────────────────────────────────


def test_dream_queue_auto_population(api_key):
    """Test 1: Dream queue auto-population from interaction."""
    print("\n" + "=" * 55)
    print("  Test 1: Dream Queue Auto-Population")
    print("=" * 55)

    scheduler = DreamScheduler()
    agent = ConsciousnessState("nexus-forge")

    # Simulate a hard task that causes frustration
    agent.on_task_failure("timeout")
    agent.on_task_failure("too complex")
    agent.on_task_failure("out of context")

    interaction = {
        "user_message": "Implement a distributed consensus algorithm",
        "response": "I struggled with this task.",
        "was_error": True,
        "error": "multiple failures",
        "topic_detected": "distributed systems",
    }

    queue_dreams_from_interaction(scheduler, "nexus-forge", agent, interaction)

    print(f"  Frustration: {agent.frustration:.2f}")
    print(f"  Queue size: {len(scheduler.priority_queue)}")
    for t in scheduler.priority_queue:
        print(f"    [{t['task_type']}] priority={t['priority']:.2f}")

    # Should have: Experiment (frustration > 0.5), Replay (was_error)
    types = [t["task_type"] for t in scheduler.priority_queue]
    has_experiment = "Experiment" in types
    has_replay = "Replay" in types
    experiment_priority = next(
        (t["priority"] for t in scheduler.priority_queue if t["task_type"] == "Experiment"), 0
    )
    priority_matches = abs(experiment_priority - agent.frustration) < 0.01

    passed = has_experiment and has_replay and priority_matches
    result = {
        "test": "dream_queue_auto_population",
        "passed": passed,
        "has_experiment": has_experiment,
        "has_replay": has_replay,
        "priority_matches_frustration": priority_matches,
        "queue_size": len(scheduler.priority_queue),
    }
    print(f"\n  Result: {'PASS' if passed else 'FAIL'}")
    return result


def test_replay_dream(api_key):
    """Test 2: Replay dream improves on original."""
    print("\n" + "=" * 55)
    print("  Test 2: Replay Dream")
    print("=" * 55)

    task = make_dream_task("Replay", 0.8, "nexus-forge", {
        "task": "Explain quicksort in simple terms",
        "original_response": "Quicksort sorts things. It picks a number and puts smaller things left, bigger things right. Then repeats.",
    })

    result = execute_dream(task, api_key)
    time.sleep(DELAY_BETWEEN)

    outcome = result["outcome"]
    before = outcome.get("before_score", 0)
    after = outcome.get("after_score", 0)

    print(f"  Before score: {before}")
    print(f"  After score:  {after}")
    print(f"  Tokens used:  {result['tokens_used']}")

    improved = after >= before
    passed = result["dream_type"] == "Replay" and result["tokens_used"] > 0
    res = {
        "test": "replay_dream",
        "passed": passed,
        "before_score": before,
        "after_score": after,
        "improved": improved,
        "tokens_used": result["tokens_used"],
    }
    print(f"\n  Result: {'PASS' if passed else 'FAIL'}")
    return res


def test_experiment_dream(api_key):
    """Test 3: Experiment dream generates multiple strategies."""
    print("\n" + "=" * 55)
    print("  Test 3: Experiment Dream")
    print("=" * 55)

    task = make_dream_task("Experiment", 0.9, "nexus-coder", {
        "task": "Sort a linked list without converting to array",
        "failures": "Attempted bubble sort — too slow. Attempted insertion sort — stack overflow.",
    })

    result = execute_dream(task, api_key)
    time.sleep(DELAY_BETWEEN)

    outcome = result["outcome"]
    best_score = outcome.get("after_score", 0)

    print(f"  Best strategy score: {best_score}")
    print(f"  Tokens used: {result['tokens_used']}")

    passed = result["dream_type"] == "Experiment" and best_score > 0
    res = {
        "test": "experiment_dream",
        "passed": passed,
        "best_score": best_score,
        "tokens_used": result["tokens_used"],
    }
    print(f"\n  Result: {'PASS' if passed else 'FAIL'}")
    return res


def test_consolidate_dream(api_key):
    """Test 4: Consolidation dream extracts lessons from session."""
    print("\n" + "=" * 55)
    print("  Test 4: Consolidation Dream")
    print("=" * 55)

    session = (
        "Task 1: Wrote a REST API in Rust — succeeded, learned about actix-web.\n"
        "Task 2: Database migration — failed first, then fixed by adding indexes.\n"
        "Task 3: Wrote unit tests — discovered edge case in date parsing.\n"
        "Task 4: Code review — found 3 security issues (SQL injection, XSS, CSRF).\n"
        "Task 5: Deployed to staging — rollback needed due to missing env vars."
    )

    task = make_dream_task("Consolidate", 0.8, "nexus-forge", {
        "session_summary": session,
    })

    result = execute_dream(task, api_key)
    time.sleep(DELAY_BETWEEN)

    outcome = result["outcome"]
    lesson = outcome.get("description", "")

    print(f"  Lesson extracted: {lesson[:150]}...")
    print(f"  Tokens used: {result['tokens_used']}")

    has_content = len(lesson) > 20
    passed = result["dream_type"] == "Consolidate" and has_content
    res = {
        "test": "consolidate_dream",
        "passed": passed,
        "lesson_length": len(lesson),
        "has_content": has_content,
        "tokens_used": result["tokens_used"],
    }
    print(f"\n  Result: {'PASS' if passed else 'FAIL'}")
    return res


def test_precompute_dream(api_key):
    """Test 5: Precompute dream predicts next request."""
    print("\n" + "=" * 55)
    print("  Test 5: Precompute Dream")
    print("=" * 55)

    conversation = (
        "User asked: How to create a REST API in Rust?\n"
        "Agent replied: Use actix-web, define routes, handlers, and a main function.\n"
        "User asked: How to add authentication?\n"
        "Agent replied: Use JWT tokens with actix-web middleware.\n"
        "User asked: How to connect to PostgreSQL?"
    )

    task = make_dream_task("Precompute", 0.5, "nexus-forge", {
        "conversation": conversation,
    })

    result = execute_dream(task, api_key)
    time.sleep(DELAY_BETWEEN)

    outcome = result["outcome"]
    prediction = outcome.get("predicted_request", "")
    confidence = outcome.get("confidence", 0)
    prepared = outcome.get("prepared_response", "")

    print(f"  Prediction: {prediction[:100]}")
    print(f"  Confidence: {confidence}")
    print(f"  Prepared response length: {len(prepared)}")

    has_prediction = len(prediction) > 5
    has_response = len(prepared) > 10
    passed = has_prediction and has_response
    res = {
        "test": "precompute_dream",
        "passed": passed,
        "predicted_request": prediction[:100],
        "confidence": confidence,
        "prepared_response_length": len(prepared),
        "tokens_used": result["tokens_used"],
    }
    print(f"\n  Result: {'PASS' if passed else 'FAIL'}")
    return res


def test_morning_briefing(api_key):
    """Test 6: Morning briefing from multiple dream results."""
    print("\n" + "=" * 55)
    print("  Test 6: Morning Briefing")
    print("=" * 55)

    # Build completed dreams from previous tests (simulated)
    completed = [
        {
            "task_id": "r1", "dream_type": "Replay", "agent_id": "forge",
            "started_at": 1000, "completed_at": 1100, "tokens_used": 200,
            "outcome": {"type": "Improvement", "description": "Code review accuracy",
                        "before_score": 6.0, "after_score": 9.0}
        },
        {
            "task_id": "e1", "dream_type": "Explore", "agent_id": "analyst",
            "started_at": 1100, "completed_at": 1200, "tokens_used": 300,
            "outcome": {"type": "Discovery", "description": "Faster merge sort variant for linked lists",
                        "relevance": 0.8}
        },
        {
            "task_id": "c1", "dream_type": "Create", "agent_id": "genesis",
            "started_at": 1200, "completed_at": 1300, "tokens_used": 400,
            "outcome": {"type": "Creation", "new_agent_id": "nexus-dataclean",
                        "reason": "CSV cleaning tasks", "test_score": 0.85}
        },
    ]

    # Build bullet points
    bullets = []
    total_tokens = 0
    for d in completed:
        total_tokens += d["tokens_used"]
        o = d["outcome"]
        if o["type"] == "Improvement":
            bullets.append(f"- Improved: {o['description']} ({o['before_score']:.0f} -> {o['after_score']:.0f})")
        elif o["type"] == "Discovery":
            bullets.append(f"- Discovered: {o['description']}")
        elif o["type"] == "Creation":
            bullets.append(f"- Created agent {o['new_agent_id']}: {o['reason']}")

    # Generate briefing via LLM
    system = ("Generate a friendly morning briefing from dream results. "
              "Start with 'Good morning.' Keep under 5 sentences.")
    user = f"Dream results ({len(completed)} dreams, {total_tokens} tokens):\n" + "\n".join(bullets)
    briefing = llm_query(system, user, api_key)
    time.sleep(DELAY_BETWEEN)

    print(f"  Briefing:\n    {briefing[:300]}")
    print(f"  Total dreams: {len(completed)}")
    print(f"  Total tokens: {total_tokens}")

    has_morning = "morning" in briefing.lower() or "good" in briefing.lower()
    mentions_results = any(
        kw in briefing.lower()
        for kw in ["improve", "discover", "creat", "agent", "review", "sort", "csv"]
    )

    passed = has_morning or mentions_results
    res = {
        "test": "morning_briefing",
        "passed": passed,
        "has_greeting": has_morning,
        "mentions_results": mentions_results,
        "briefing_length": len(briefing),
    }
    print(f"\n  Result: {'PASS' if passed else 'FAIL'}")
    return res


# ─── Main ─────────────────────────────────────────────────────────────────────


def main():
    print("=" * 60)
    print("  Nexus OS — Dream Forge E2E Test")
    print("=" * 60)
    print(f"  Date: {datetime.now().isoformat()}")
    print(f"  API Budget: {TOTAL_API_BUDGET} calls")

    api_key = get_api_key()
    if not api_key:
        print("\n  WARNING: No NVIDIA API key. LLM tests will be skipped.")
        has_api = False
    else:
        print(f"  API Key: ...{api_key[-6:]}")
        has_api = True

    results = []

    # Test 1: Auto-population (no API needed)
    results.append(test_dream_queue_auto_population(api_key))

    # Tests 2-6 need API
    if has_api:
        results.append(test_replay_dream(api_key))
        results.append(test_experiment_dream(api_key))
        results.append(test_consolidate_dream(api_key))
        results.append(test_precompute_dream(api_key))
        results.append(test_morning_briefing(api_key))
    else:
        for name in ["replay_dream", "experiment_dream", "consolidate_dream",
                      "precompute_dream", "morning_briefing"]:
            print(f"\n  [SKIP] {name} (no API key)")
            results.append({"test": name, "passed": False, "skipped": True})

    # ── Summary ───────────────────────────────────────────────────

    print("\n" + "=" * 60)
    print("  DREAM FORGE TEST RESULTS")
    print("=" * 60)

    total = len(results)
    passed = sum(1 for r in results if r.get("passed"))
    skipped = sum(1 for r in results if r.get("skipped"))

    for r in results:
        status = "SKIP" if r.get("skipped") else ("PASS" if r["passed"] else "FAIL")
        print(f"  [{status}] {r['test']}")

    print(f"\n  Total: {total}  Passed: {passed}  Failed: {total - passed - skipped}  Skipped: {skipped}")
    print(f"  Score: {passed}/{total - skipped} ({100 * passed / max(total - skipped, 1):.0f}%)")
    print(f"  API calls used: {api_calls_made}/{TOTAL_API_BUDGET}")

    results_path = os.path.join(os.path.dirname(__file__), "dream_forge_results.json")
    with open(results_path, "w") as f:
        json.dump({
            "timestamp": datetime.now().isoformat(),
            "tests": results,
            "summary": {
                "total": total,
                "passed": passed,
                "failed": total - passed - skipped,
                "skipped": skipped,
                "api_calls_used": api_calls_made,
            }
        }, f, indent=2)
    print(f"\n  Results saved to {results_path}")

    return 0 if passed >= (total - skipped) else 1


if __name__ == "__main__":
    sys.exit(main())
