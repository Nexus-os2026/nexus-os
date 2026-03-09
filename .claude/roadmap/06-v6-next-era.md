# Phase 6: v6.0 - The Next Era

> Priority: HIGH | Theme: From secure runtime to global standard
> Author: Suresh Karicheti — Lead Architect, Nexus OS

## 6.1 Real Wasm Agent Sandboxing (kernel/src/wasm_runtime.rs)

Replace InProcessSandbox with real wasmtime-based isolation. Agents compiled to .wasm run inside a memory-isolated sandbox. Host functions are the ONLY bridge to the kernel — no direct system calls possible.

Create:
- kernel/src/wasm_runtime.rs — WasmAgentRuntime using wasmtime engine
- WasmHostFunctions struct exposing governed capabilities as WASI-compatible imports: fd_read, fd_write, llm_query, request_approval, emit_audit
- WasmMemoryLimits: configurable per-agent memory ceiling (default 256MB)
- WasmFuelMeter: map wasmtime fuel to Nexus OS fuel system
- WasmCapabilityGate: each host function checks agent manifest capabilities before executing
- Agent .wasm loading from marketplace with signature verification before instantiation

Tests:
- Agent in wasm sandbox cannot access host filesystem directly
- Host function checks capability before granting access
- Memory limit kills agent that exceeds allocation
- Fuel exhaustion halts wasm execution
- Unsigned wasm refused to load

Done When:
- [ ] wasmtime-based runtime executes .wasm agents
- [ ] Host functions enforce full governance (capabilities, fuel, audit)
- [ ] Memory and fuel limits enforced at wasm level
- [ ] Marketplace agents load only after signature verification
- [ ] Integration test: untrusted agent blocked from unauthorized access

## 6.2 Speculative Execution — Shadow Simulation (kernel/src/speculative.rs)

Before a high-risk action executes, the kernel forks a shadow state, runs the action in simulation, and presents the predicted outcome to the user for approval.

Create:
- kernel/src/speculative.rs — SpeculativeEngine
- StateSnapshot: serializable snapshot of current agent state, filesystem refs, and fuel
- SimulationResult: predicted_changes (Vec of FileChange, NetworkCall, DataModification), resource_impact (disk_bytes, fuel_cost, time_estimate), risk_assessment (low/medium/high/critical)
- FileChange struct: path, change_type (Create/Modify/Delete), size_before, size_after, preview (first 100 chars of diff)
- SpeculativeEngine methods: fork_state(agent_id) -> StateSnapshot, simulate(snapshot, action) -> SimulationResult, present_preview(result) -> ApprovalRequest with rich context, commit_or_rollback(approved)
- Wire into HITL approval: when autonomy level requires approval AND action is high-risk, auto-trigger simulation before presenting to user
- Desktop UI: SpeculativePreview component showing "If this proceeds:" with file changes, resource impact, and risk badge

Tests:
- Simulate file deletion shows correct file count and size impact
- Simulate LLM call shows estimated token cost and fuel impact
- Simulation does not modify real state (fork is isolated)
- High-risk action auto-triggers simulation before approval
- User rejection rolls back cleanly

Done When:
- [ ] SpeculativeEngine forks state and simulates actions
- [ ] SimulationResult shows human-readable impact preview
- [ ] Desktop UI shows rich preview before high-risk approvals
- [ ] Simulation is fully isolated from real state
- [ ] Integration test: simulate-then-approve vs simulate-then-reject

## 6.3 Local SLM Integration via Candle (connectors/llm/src/local_slm.rs)

Native support for running small language models locally using candle (pure Rust ML framework). Used for governance tasks: PII redaction, capability checking, prompt classification — without calling cloud APIs.

Create:
- connectors/llm/src/local_slm.rs — LocalSlmProvider implementing LlmProvider trait
- ModelRegistry: discover and manage local .safetensors models in ~/.nexus/models/
- SupportedModels: Phi-4, Llama-3-8B, Qwen-2.5 with quantized variants (Q4, Q8)
- GovernanceSlm: specialized wrapper for governance-specific tasks with pre-built prompts for PII detection, capability risk assessment, prompt injection detection
- Auto-routing in LLM Gateway: governance tasks route to local SLM, creative/complex tasks route to cloud providers
- Download manager: fetch models from HuggingFace with progress tracking and checksum verification
- Wire into existing ProviderRouter as highest-priority provider for governance tasks

Tests:
- LocalSlmProvider loads quantized model and generates response
- GovernanceSlm correctly classifies PII in text
- Auto-routing sends governance tasks local, complex tasks to cloud
- Model download with checksum verification
- Fallback to cloud when local model unavailable

Done When:
- [ ] Candle-based local inference runs Phi-4 or similar
- [ ] Governance tasks (PII, capability check) run 100% on-device
- [ ] Auto-routing in gateway prefers local for governance
- [ ] Model download and management CLI
- [ ] Integration test: PII redaction without any cloud API call

## 6.4 Distributed Immutable Audit via Content-Addressable Storage (distributed/src/immutable_audit.rs)

Audit logs gossiped across user devices using content-addressable storage. Attacker must compromise ALL devices to tamper with evidence.

Create:
- distributed/src/immutable_audit.rs — ImmutableAuditStore
- ContentAddress: SHA-256 hash used as key for every audit block
- AuditBlock: events (Vec of AuditEvent), previous_hash, node_id, timestamp, signature
- GossipProtocol: periodic sync between paired devices (phone, laptop, server)
- DevicePairing: secure pairing with QR code or one-time token, Ed25519 key exchange
- ConflictResolution: append-only merge — all blocks from all devices form the complete log
- VerificationQuery: given any audit event, prove it exists across N devices
- Optional IPFS backend: pin audit blocks to IPFS for public verifiability

Tests:
- Audit block created with content address matching SHA-256 of contents
- Gossip syncs blocks between two paired devices
- Tampered block detected on receiving device (hash mismatch)
- Verification proves event exists on 3 of 3 devices
- Unpaired device rejected from gossip

Done When:
- [ ] Content-addressable audit blocks with hash-chain linking
- [ ] Gossip protocol syncs between paired devices
- [ ] Tamper detection on receiving end
- [ ] Multi-device verification query
- [ ] Integration test: 3-device gossip with tamper detection

## 6.5 Visual Permission Dashboard (app/src/pages/PermissionDashboard.tsx)

Android/iOS-style permission toggles for agent capabilities. Non-technical users can see and control exactly what each agent can access.

Create:
- app/src/pages/PermissionDashboard.tsx — Visual permission manager
- Permission categories with icons: Filesystem (folder icon), Network (globe), LLM/AI (brain), Social Media (share), Screen/Camera (eye), Financial (dollar), Messaging (chat)
- Per-agent view: toggle switches for each capability with risk level indicator (green/yellow/red)
- Permission history: timeline of when permissions were granted/revoked
- Comparison view: side-by-side "Current vs Requested" when an agent requests new capabilities
- "What does this mean?" tooltip for each permission explaining in plain English what the agent can do with it
- Bulk controls: "Revoke all network access", "Read-only mode" for any agent
- Wire permission changes to kernel capability engine via Tauri commands

Tests are UI-focused — ensure npm run build passes and all toggles update state correctly.

Done When:
- [ ] Permission dashboard renders all agent capabilities as toggles
- [ ] Risk level indicators per permission
- [ ] Permission history timeline
- [ ] Comparison view for new capability requests
- [ ] Plain English tooltips
- [ ] Toggle changes propagate to backend
