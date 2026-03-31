# A2A Crate Wiring Report

Generated: 2026-03-28
Commit: 0434455d (v10.2.0)
Crate: `nexus-a2a` (crates/nexus-a2a/)

## Summary: PARTIALLY WIRED

The nexus-a2a crate compiles, passes 32 unit tests, and is fully registered in the Tauri command system. TypeScript bindings are correct. Frontend UI exposes 3 of 6 crate commands. **However**, the SkillRegistry is initialized empty at runtime (no agent manifests loaded), A2aState uses `Default::default()` with no governance wiring (no fuel, no HITL, nil UUID), and 3 crate commands have no frontend UI.

---

## Check 1: Tauri Commands

All 6 commands exist with `#[tauri::command]` and are registered in `generate_handler![]`.

| Command | Defined | Registered | Location (main.rs) |
|---------|---------|------------|-------------------|
| `a2a_crate_get_agent_card` | YES | YES (line 27721) | lines 5832-5836, handler at 23912-23917 |
| `a2a_crate_list_skills` | YES | YES (line 27722) | lines 5838-5842, handler at 23919-23924 |
| `a2a_crate_send_task` | YES | YES (line 27723) | lines 5844-5851, handler at 23926-23933 |
| `a2a_crate_get_task` | YES | YES (line 27724) | lines 5853-5860, handler at 23935-23942 |
| `a2a_crate_discover_agent` | YES | YES (line 27725) | lines 5862-5868, handler at 23944-23950 |
| `a2a_crate_get_status` | YES | YES (line 27726) | lines 5870-5874, handler at 23952-23957 |

**Verdict: PASS** — All 6 commands fully registered.

---

## Check 2: AppState

| Aspect | Status | Details |
|--------|--------|---------|
| `a2a_state` field in AppState | YES | `a2a_state: Arc<A2aState>` at main.rs line 942 |
| Initialized in production path | YES | `Arc::new(A2aState::default())` at line 1267 |
| Initialized in test path | YES | `Arc::new(A2aState::default())` at line 1483 |
| Initialized with real kernel components | **NO** | Uses `Default::default()` which creates empty state |
| Silent failure risk | **MEDIUM** | No failure — but silently starts with zero capabilities |

### A2aState internals (tauri_commands.rs:17-37):
```rust
pub struct A2aState {
    pub client: Mutex<A2aClient>,      // A2aClient::new() — empty known_agents, no HITL, no fuel, nil UUID
    pub registry: Mutex<SkillRegistry>, // SkillRegistry::new() — empty agent_skills HashMap
    pub bridge: Mutex<A2aBridge>,       // A2aBridge::new() — empty tasks HashMap
}
```

### Governance gaps in A2aClient (kernel/src/protocols/a2a_client.rs:72-91):
- `consent: None` — no HITL approval gate
- `fuel: None` — no fuel metering
- `agent_id: Uuid::nil()` — nil UUID, not connected to any real agent
- `known_agents: HashMap::new()` — no peers known

**Verdict: PARTIAL** — Field exists and initializes without panic, but governance is not connected.

---

## Check 3: TypeScript Bindings

All 6 bindings exist in `app/src/api/backend.ts` (lines 530-554).

| Binding | Invoke Name | Match | Parameters |
|---------|------------|-------|------------|
| `a2aCrateGetAgentCard()` | `a2a_crate_get_agent_card` | CORRECT | none |
| `a2aCrateListSkills()` | `a2a_crate_list_skills` | CORRECT | none |
| `a2aCrateSendTask(agentUrl, message)` | `a2a_crate_send_task` | CORRECT | `{ agentUrl, agent_url, message }` |
| `a2aCrateGetTask(taskId, agentUrl?)` | `a2a_crate_get_task` | CORRECT | `{ taskId, task_id, agentUrl, agent_url }` |
| `a2aCrateDiscoverAgent(url)` | `a2a_crate_discover_agent` | CORRECT | `{ url }` |
| `a2aCrateGetStatus()` | `a2a_crate_get_status` | CORRECT | none |

**Note**: `a2aCrateSendTask` and `a2aCrateGetTask` send both camelCase and snake_case parameter names (redundant but functional — Rust deserializes the snake_case keys).

**Verdict: PASS** — All invoke names match exactly.

---

## Check 4: Frontend UI

**Section**: "A2A Protocol (nexus-a2a)" in `app/src/pages/Protocols.tsx` lines 598-637.

| Aspect | Status | Details |
|--------|--------|---------|
| Section visible | YES | Always rendered, no feature flags or conditional gates |
| Error handling | YES | Each button has `try/catch` with `setA2aError()` |
| Results displayed | YES | JSON preview via `JSON.stringify(x, null, 2)` |
| State variables | YES | `a2aCrateCard`, `a2aCrateSkills`, `a2aCrateStatus` (lines 101-103) |

### Button coverage:

| Crate Command | Has UI Button | Handler |
|---------------|--------------|---------|
| `a2aCrateGetAgentCard()` | YES | line 601-606 |
| `a2aCrateListSkills()` | YES | line 607-612 |
| `a2aCrateGetStatus()` | YES | line 613-618 |
| `a2aCrateSendTask()` | **NO** | Binding exists but no UI |
| `a2aCrateGetTask()` | **NO** | Binding exists but no UI |
| `a2aCrateDiscoverAgent()` | **NO** | Binding exists but no UI |

**Note**: The OLD set (`a2aDiscoverAgent`, `a2aSendTask`, etc.) is used in the "A2A Client (Outbound)" section for external agent interaction. The missing 3 crate commands overlap with those old commands.

**Verdict: PARTIAL** — 3 of 6 commands wired to UI.

---

## Check 5: Bridge Integration

| Aspect | Status | Details |
|--------|--------|---------|
| Agent routing works | YES (in code) | `bridge.route_task()` uses skill-tag scoring (bridge.rs:67-134) |
| Governance applied | YES | `GovernanceContext` attached to every routed task (bridge.rs:114-122) |
| HITL gate | YES | `hitl_approved: false` on all routed tasks — requires approval |
| Fuel metering | YES | `fuel_budget` parameter passed through, tracked in context |
| Autonomy level | FIXED | Hardcoded at L2 for all A2A tasks |
| Real execution | NO | Bridge prepares `RoutedTask` but does NOT execute agents — returns task for Supervisor to process |
| Audit trail | PARTIAL | `audit_hash: None` — ready for hashing but not yet connected to chain |

### Bridge routing algorithm:
1. Iterates all agents in SkillRegistry
2. Scores by matching skill tags vs. requested tags
3. Selects highest-scoring agent (alphabetical tie-break)
4. Creates `RoutedTask` with `GovernanceContext`
5. Stores in `tasks` HashMap for lifecycle tracking

**Verdict: PASS** — Design is sound, governance is attached, but depends on SkillRegistry being populated (which it isn't at startup).

---

## Check 6: Skill Registry

| Aspect | Status | Details |
|--------|--------|---------|
| Skills populated at startup | **NO** | `SkillRegistry::new()` creates empty `agent_skills: HashMap::new()` |
| Auto-population from manifests | **NO** | No code calls `register_manifest()` during app init |
| AgentCard endpoint | PARTIAL | `GET /a2a/agent-card` exists in CLI router; `GET /.well-known/agent.json` documented but not implemented |
| Capabilities from 54 agents | **NO** | Registry never receives agent manifests |

### AgentCard output when empty:
```json
{
  "name": "nexus-os",
  "description": "Nexus OS governed agent instance with 0 agents and 0 skills",
  "url": "http://localhost:9090/a2a",
  "skills": [],
  "version": "0.2.1"
}
```

**Verdict: INCOMPLETE** — Registry infrastructure exists but is never populated.

---

## Check 7: Cross-Compatibility

### Old commands (kernel A2A client — `state.a2a_client`):
| Command | Registered | Line |
|---------|-----------|------|
| `a2a_discover_agent` | YES | 27716 |
| `a2a_send_task` | YES | 27717 |
| `a2a_get_task_status` | YES | 27718 |
| `a2a_cancel_task` | YES | 27719 |
| `a2a_known_agents` | YES | 27720 |

### New commands (nexus-a2a crate — `state.a2a_state`):
| Command | Registered | Line |
|---------|-----------|------|
| `a2a_crate_get_agent_card` | YES | 27721 |
| `a2a_crate_list_skills` | YES | 27722 |
| `a2a_crate_send_task` | YES | 27723 |
| `a2a_crate_get_task` | YES | 27724 |
| `a2a_crate_discover_agent` | YES | 27725 |
| `a2a_crate_get_status` | YES | 27726 |

### Conflicts: **NONE**
- Old: `a2a_*` prefix
- New: `a2a_crate_*` prefix
- No naming collisions

### Frontend usage:
- **Protocols.tsx** uses BOTH sets:
  - Old set → "A2A Client (Outbound)" section for external agent operations
  - New set → "A2A Protocol (nexus-a2a)" section for local instance inspection

### Duplicate state problem:
AppState holds both `a2a_client: Arc<Mutex<A2aClient>>` (line 914) and `a2a_state: Arc<A2aState>` (line 942). Both contain independent `A2aClient` instances with separate `known_agents` registries. Agents discovered via old commands won't appear in the new crate's state.

### Recommendation:
**Long-term**: migrate outbound operations to the crate set and deprecate the kernel-direct commands (crate versions apply governance). **Short-term**: both coexist safely with `a2a_crate_*` prefix preventing collisions.

---

## Check 8: Functional Test

All tests run via `cargo test -p nexus-a2a` — **32 passed, 0 failed**.

| Test | Result | Assertion |
|------|--------|-----------|
| `get_agent_card_returns_card` | PASS | Card name = "nexus-os", 1 skill, correct version |
| `list_skills_returns_summaries` | PASS | Returns 1 skill with id "test-skill" |
| `get_status_returns_summary` | PASS | `running: true`, `skills_count: 1`, `known_peers: 0` |
| `get_task_not_found` | PASS | Returns error containing "not found" |
| `discover_agent_fails_on_bad_url` | PASS | Returns error on unreachable URL |
| `send_and_wait_fails_on_bad_url` | PASS | Returns error on bad URL |

**Note**: Tests use `make_state()` helper which pre-registers a test skill. In production, the registry starts empty.

**Verdict: PASS** — All functional tests pass with pre-populated state.

---

## Issues Found

1. **SkillRegistry never populated at startup** — `A2aState::default()` creates an empty registry. No code in main.rs calls `register_manifest()` or `register_skills()` for any of the 54 prebuilt agents. The `Get Agent Card` and `List Skills` buttons will return empty results in production. (tauri_commands.rs:30-36, main.rs:1267)

2. **A2aClient lacks governance at initialization** — Created via `A2aClient::new()` with `consent: None`, `fuel: None`, `agent_id: Uuid::nil()`. A2A outbound operations bypass HITL gates and fuel metering. The `with_consent()` and `set_fuel_context()` methods exist but are never called. (kernel/src/protocols/a2a_client.rs:80-89, main.rs:1267)

3. **3 crate commands have no frontend UI** — `a2aCrateSendTask`, `a2aCrateGetTask`, `a2aCrateDiscoverAgent` exist as TypeScript bindings but have no buttons in Protocols.tsx. (backend.ts:541-549, Protocols.tsx:598-637)

4. **`/.well-known/agent.json` not implemented** — Documented in server.rs comments but the CLI router only exposes `GET /a2a/agent-card`. The A2A spec standard path is missing. (server.rs:4-5, cli/src/router.rs:1141)

5. **Legacy A2A commands use unsafe mutex recovery** — `state.a2a_client.lock().unwrap_or_else(|p| p.into_inner())` silently recovers from poisoned mutexes without logging. New crate commands properly propagate errors. (main.rs:5786-5825)

6. **Duplicate A2A client state** — AppState holds independent `a2a_client` and `a2a_state.client` with separate `known_agents` registries. Discovery via one set is invisible to the other. (main.rs:914, 942)

---

## Fixes Needed

1. **Populate SkillRegistry on startup** — After creating `A2aState`, iterate loaded agent manifests and call `registry.register_manifest()` for each. This gives the AgentCard real skills and enables bridge routing.

2. **Wire governance into A2aClient** — Initialize with `A2aClient::with_consent(consent_runtime, agent_id)` and call `set_fuel_context()` using the Supervisor's fuel ledger.

3. **Add UI for remaining 3 crate commands** — Wire `a2aCrateSendTask`, `a2aCrateGetTask`, `a2aCrateDiscoverAgent` into Protocols.tsx, or merge them with the existing outbound UI.

4. **Add `/.well-known/agent.json` route** — In `cli/src/router.rs`, add a `GET /.well-known/agent.json` handler returning `SkillRegistry::build_instance_card()`.

5. **Unify A2A client state** — Route old `a2a_*` commands through `A2aState.client` (single source of truth) and deprecate the standalone `a2a_client` field.

6. **Fix legacy mutex handling** — Replace `unwrap_or_else(|p| p.into_inner())` with proper `map_err` in old A2A commands.
