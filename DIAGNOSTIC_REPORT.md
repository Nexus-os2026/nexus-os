# Diagnostic Report: Agent Execution Bugs

## Date: 2026-03-27

---

## Bug 1: Planner→Executor Gap

### Planner Output
- Function: `CognitivePlanner::plan_goal()` at [planner.rs:27](kernel/src/cognitive/planner.rs#L27)
- Return type: `Result<Vec<AgentStep>, AgentError>`
- Each `AgentStep` contains a `PlannedAction` (the action to execute), `StepStatus`, `fuel_cost`, `attempts`, `max_retries`, and `result`

### Executor Input
- Function: `RegistryExecutor::execute()` at [loop_runtime.rs:210](kernel/src/cognitive/loop_runtime.rs#L210)
- Expected input type: `(&str, &PlannedAction, &mut AuditTrail)` → `Result<String, String>`
- The executor takes a `&PlannedAction` extracted from `AgentStep.action` — **types match correctly**

### Handoff Analysis
- Where handoff occurs: [loop_runtime.rs:977-1181](kernel/src/cognitive/loop_runtime.rs#L977-L1181)
- Type match: **YES** — planner returns `Vec<AgentStep>`, executor receives `&PlannedAction` from `step.action.clone()`
- Is planner output actually passed to executor: **YES, BUT with a gate in between**

### Root Cause: HITL Approval Allowance Starts at 0 and Blocks Every Planned Action

The pipeline works as follows:
1. Planner produces `Vec<AgentStep>` → stored in `state.steps` (line 977)
2. `state.hitl_approval_allowance` is **reset to 0** after every plan (line 980)
3. ACT phase starts, picks the current step (line 996)
4. HITL check at line 1054-1074:

```rust
let requires_hitl = action_requires_hitl(&step.action, autonomy_level);
if requires_hitl && state.hitl_approval_allowance == 0 {
    state.phase = CognitivePhase::Blocked;
    // ... returns CycleResult with phase=Blocked, should_continue=true
    return Ok(CycleResult { phase: Blocked, ... });
}
```

5. `action_requires_hitl()` returns `true` for nearly all useful actions at autonomy levels < 3:
   - L<3: FileWrite, ShellCommand, DockerCommand, ApiCall, CodeExecute, all mouse/keyboard actions
   - L<2: WebFetch, BrowserAutomate, AgentMessage, CaptureScreen
   - Always: ComputerAction, HitlRequest, SelfModify*, CreateSubAgent, SelfEvolve

6. The cognitive loop in main.rs (line 16472) correctly detects Blocked phase and creates a consent request
7. `notify.notified().await` (line 16592) suspends the loop until approval
8. `approve_consent_request()` (line 17251) calls `approve_blocked_step()` which grants allowance +1
9. Loop resumes with `continue` (line 16639)

**The mechanism appears correct in isolation.** The actual root cause is more subtle:

### TRUE ROOT CAUSE: Race condition between plan and act phases within the same cycle

At line 980, `hitl_approval_allowance` is reset to 0 **in the same cycle** that planning occurs. The ACT phase runs in the **same cycle** immediately after planning. So every first action after planning is guaranteed to hit the HITL gate.

**This is actually by design** — HITL approval IS supposed to block. The real issue is:

### ROOT CAUSE HYPOTHESIS A: Agent autonomy level is too low

If agents are created at L0 or L1 (the default), nearly every action requires HITL approval. The consent request IS created, but:
- The frontend consent UI at [Agents.tsx:1154-1237](app/src/pages/Agents.tsx#L1154-L1237) only shows consents fetched via the `agent-blocked` event listener (line 247-254)
- The `agent-blocked` listener calls `listPendingConsents()` which may fail silently (`.catch(() => {})`)
- If the consent list never renders, the user never sees the approval button, and the agent stays blocked forever

### ROOT CAUSE HYPOTHESIS B: `should_continue: true` causes infinite blocking loop

When blocked, the cycle returns `should_continue: true` (line 1071). The main.rs loop correctly handles this by waiting for consent (line 16592). However, after approval and `continue` (line 16639), the next cycle runs the **same step** again. If the step still requires HITL (which it does), and `approve_blocked_step` only grants +1 allowance, this works correctly for one step — but **after each step executes, the next step blocks again**.

For a plan with N steps that all require HITL, the user must approve N separate times. This creates the **illusion** that the agent "plans but doesn't execute" because:
1. Plan is created (visible in UI)
2. First step blocks for HITL → user may not notice the consent request
3. Even if approved, the next step blocks again immediately
4. The agent appears stuck

### ROOT CAUSE HYPOTHESIS C: Silent consent DB failure

Line 16577: `let _ = state.db.enqueue_consent(&consent_row);` — if the DB write fails, the consent is never persisted, `listPendingConsents()` returns no results, and the agent blocks forever waiting for an approval that can never be granted because the consent request was never saved.

### Evidence

**Plan phase resets allowance (line 980):**
```rust
state.steps = new_steps;
state.current_step_index = 0;
state.consecutive_failures = 0;
state.hitl_approval_allowance = 0;  // ← ALWAYS 0 after plan
```

**HITL gate (lines 1054-1074):**
```rust
let requires_hitl = action_requires_hitl(&step.action, autonomy_level);
if requires_hitl && state.hitl_approval_allowance == 0 {
    state.phase = CognitivePhase::Blocked;
    return Ok(CycleResult {
        phase: CognitivePhase::Blocked,
        steps_executed: 0,
        should_continue: true,
        blocked_reason: Some(reason),
    });
}
```

**Approval grants only +1 (line 1580):**
```rust
pub fn approve_blocked_steps(&self, agent_id: &str, count: u32) -> Result<(), AgentError> {
    state.hitl_approval_allowance = state.hitl_approval_allowance.saturating_add(count.max(1));
}
```

**Single-step approve call from Tauri (line 17251):**
```rust
let _ = state.cognitive_runtime.approve_blocked_step(&agent_id_str);
// approve_blocked_step calls approve_blocked_steps(agent_id, 1)
```

**Consent DB write is silent (line 16577):**
```rust
let _ = state.db.enqueue_consent(&consent_row);  // ← failure silently ignored
```

---

## Bug 2: White Screen Crash

### Agent Execution UI Component
- Component: `Agents` at [Agents.tsx](app/src/pages/Agents.tsx)
- Has ErrorBoundary: **YES** — wrapped in `PageErrorBoundary` at [App.tsx:1959-1970](app/src/App.tsx#L1959-L1970)

### Why PageErrorBoundary Doesn't Prevent White Screen

`PageErrorBoundary` (React ErrorBoundary) only catches **synchronous render-time errors**. It does NOT catch:
- Errors in async event handlers (onClick, etc.)
- Errors in `useEffect` callbacks
- Errors in Promise chains / `.then()` handlers
- Errors in Tauri event listeners

Since the Agents page uses Tauri event listeners (lines 193-261) and async onClick handlers (lines 1174, 1192, 1213), crashes in those paths bypass the ErrorBoundary entirely.

### Likely Crash Point 1: Unhandled async errors in consent approval buttons

**Location:** [Agents.tsx:1174-1176](app/src/pages/Agents.tsx#L1174-L1176)
```tsx
onClick={async () => {
    await approveConsentRequest(consent.consent_id, "user");  // NO try-catch
    setPendingConsents(prev => prev.filter(c => c.consent_id !== consent.consent_id));
}}
```

**Also at line 1194-1196:**
```tsx
for (const c of pendingConsents) {
    await approveConsentRequest(c.consent_id, "user");  // NO try-catch
}
```

**Also at line 1214:**
```tsx
await denyConsentRequest(consent.consent_id, "user", "User denied");  // NO try-catch
```

While async errors don't crash React directly, they produce unhandled promise rejections. If the Tauri runtime has a global error handler that disrupts the webview, this could cause a white screen.

### Likely Crash Point 2: State update on unmounted component during rapid agent execution

**Location:** [Agents.tsx:193-261](app/src/pages/Agents.tsx#L193-L261)

The event listeners capture `selectedAgentId` in their closure (line 202, 232, 248). If the user navigates away from the Agents page while an agent is running, the cleanup function (lines 256-260) unsubscribes from listeners. However, there's a race:

```tsx
return () => {
    unlistenCycle.then(f => f());    // Async cleanup
    unlistenComplete.then(f => f()); // Async cleanup
    unlistenBlocked.then(f => f());  // Async cleanup
};
```

The `unlisten` calls are **async** — `listen()` returns a `Promise<UnlistenFn>`. Between `listen()` being called and the promise resolving, events can arrive and trigger state updates on an unmounting/unmounted component. React 18 doesn't crash on this, but rapid state updates (`setGoalSteps`, `setGoalFuel`, `setGoalStepDetails`) can cause re-render storms.

### Likely Crash Point 3: Unbounded goalStepDetails array growth

**Location:** [Agents.tsx:221](app/src/pages/Agents.tsx#L221)
```tsx
setGoalStepDetails(prev => [...prev, ...safeSteps]);
```

Each cognitive cycle appends steps. For a long-running agent with 500 cycles (max_cycles = 500 at main.rs:16357), each emitting multiple step details, this array grows unbounded. Combined with the `.map()` render at line 1384+, this could cause:
- Memory exhaustion
- Extremely slow renders
- Browser tab crash → white screen

### Likely Crash Point 4: selectedPreinstalled access without null guard

**Location:** Multiple lines in [Agents.tsx](app/src/pages/Agents.tsx)

`selectedPreinstalled` is derived from `preinstalledAgents` and `selectedAgentId`. If the agents list refreshes during execution and the selected agent is no longer in the list, `selectedPreinstalled` becomes `undefined`. Then lines like:

```tsx
selectedPreinstalled.name                    // line 745
selectedPreinstalled.autonomy_level          // line 749
selectedPreinstalled.fuel_budget.toLocaleString()  // line 768
selectedPreinstalled.capabilities.slice(0, 4)      // line 773
```

...will throw a TypeError during render, which IS caught by PageErrorBoundary. But if PageErrorBoundary's fallback UI itself has issues, or if the error occurs in a child component that renders before the boundary catches it, the user sees a white flash.

### Most Probable White Screen Scenario

**The most likely crash sequence:**
1. User starts agent execution → `startGoalExecution()` called
2. Agent plans successfully → planner returns steps
3. First step requires HITL → `agent-blocked` event emitted
4. `agent-blocked` listener fires → `listPendingConsents().then(...)`
5. Consent list renders with approval buttons
6. User clicks "Approve" → `approveConsentRequest()` called without try-catch
7. Backend processes approval → emits `consent-resolved` and `agent-resumed` events
8. Next cognitive cycle runs immediately → emits `agent-cognitive-cycle` with step details
9. **Multiple rapid state updates hit React simultaneously:** `setGoalPhase`, `setGoalSteps`, `setGoalFuel`, `setGoalStepDetails`, `setPendingConsents`
10. If `selectedPreinstalled` becomes stale during this flurry (e.g., agent list refresh triggered by `getPreinstalledAgents` interval), a render-time crash occurs
11. PageErrorBoundary catches it → but the error state may look like a white screen if the fallback doesn't render properly

---

## Recommended Fixes

### Bug 1: Planner→Executor Gap

1. **[loop_runtime.rs:980]** After planning, grant initial HITL allowance based on autonomy level:
   ```rust
   state.hitl_approval_allowance = if autonomy_level >= 3 { u32::MAX } else { 0 };
   ```
   This way L3+ agents execute without blocking, while L0-L2 still require approval.

2. **[main.rs:16577]** Make consent DB write fail-hard instead of silent:
   ```rust
   state.db.enqueue_consent(&consent_row)
       .map_err(|e| format!("consent DB failure: {e}"))?;
   // or at minimum: .expect("consent enqueue must not fail")
   ```

3. **[main.rs:17251]** When batch-approving (via "Approve All" button), use `approve_blocked_steps(agent_id, count)` with the full batch count instead of approving one at a time, so all pending steps get their allowance in one call.

4. **[Agents.tsx:250-253]** Add error handling to `listPendingConsents()`:
   ```tsx
   listPendingConsents().then(consents => {
       const agentConsents = consents.filter(c => c.agent_id === selectedAgentId);
       setPendingConsents(agentConsents);
   }).catch(err => {
       console.error("Failed to load consents:", err);
       // Show inline error so user knows approval is needed
   });
   ```

5. **Default agent autonomy**: Verify that agents are created at the correct autonomy level. If demo agents default to L0/L1, they will block on every action. Consider L3 for demo agents.

### Bug 2: White Screen Crash

6. **[Agents.tsx:1174, 1194, 1214]** Wrap all async onClick handlers in try-catch:
   ```tsx
   onClick={async () => {
       try {
           await approveConsentRequest(consent.consent_id, "user");
           setPendingConsents(prev => prev.filter(c => c.consent_id !== consent.consent_id));
       } catch (err) {
           console.error("Approval failed:", err);
           // Show error in UI
       }
   }}
   ```

7. **[Agents.tsx:221]** Cap `goalStepDetails` array to prevent unbounded growth:
   ```tsx
   setGoalStepDetails(prev => {
       const updated = [...prev, ...safeSteps];
       return updated.length > 200 ? updated.slice(-200) : updated;
   });
   ```

8. **[Agents.tsx]** Add null guard before rendering `selectedPreinstalled`:
   ```tsx
   {selectedPreinstalled && (
       // ... all selectedPreinstalled.* accesses
   )}
   ```
   Verify this guard exists everywhere `selectedPreinstalled` is accessed.

9. **[Agents.tsx:256-260]** Use synchronous cleanup pattern for event listeners:
   ```tsx
   // Store unlisten promises and resolved functions
   const unlistenFns = useRef<Array<() => void>>([]);
   // In the effect, push to ref as they resolve
   // In cleanup, call all resolved ones synchronously
   ```

10. **[App.tsx]** Add a global unhandled promise rejection handler:
    ```tsx
    useEffect(() => {
        const handler = (event: PromiseRejectionEvent) => {
            console.error("Unhandled rejection:", event.reason);
            event.preventDefault(); // Prevent white screen
        };
        window.addEventListener("unhandledrejection", handler);
        return () => window.removeEventListener("unhandledrejection", handler);
    }, []);
    ```

### Priority Order
1. Fix #2 (silent consent DB write) — most likely the "invisible" root cause of Bug 1
2. Fix #5 (check default agent autonomy levels) — if agents are L0/L1, every action blocks
3. Fix #6 (try-catch on consent buttons) — prevents the white screen trigger
4. Fix #7 (cap goalStepDetails) — prevents memory exhaustion on long runs
5. Fix #1 (HITL allowance for L3+ agents) — removes unnecessary blocking
6. Fix #8 (null guard on selectedPreinstalled) — defensive rendering
7. Fixes #3, #4, #9, #10 — quality improvements
