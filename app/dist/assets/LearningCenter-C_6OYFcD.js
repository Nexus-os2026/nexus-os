import{r as i,c9 as de,c7 as he,cn as ue,j as e,co as pe,cp as me,cq as ge,cr as fe,cs as ve}from"./admin-gl7rYrJS.js";import{j as xe,bp as ye,bq as be,br as je,bs as we,af as R,ai as Q,ah as Ne}from"./enterprise-D7FEnw0A.js";const b={beginner:"#22c55e",intermediate:"#f59e0b",advanced:"#ef4444"},ke=["All","Getting Started","RAG","Models","Security","Time Machine"],te="nexus-learning-progress";function Se(){try{const n=localStorage.getItem(te);return n?JSON.parse(n):{}}catch{return{}}}function Z(n){localStorage.setItem(te,JSON.stringify(n)),fe(JSON.stringify(n)).catch(()=>{})}async function Ce(){try{const n=await ge(),l=typeof n=="string"?JSON.parse(n):n;return l&&typeof l=="object"?l:null}catch{return null}}async function Te(){try{const n=await pe(),l=typeof n=="string"?JSON.parse(n):n,g=(l==null?void 0:l.skills)??(l==null?void 0:l.skill_levels)??{};return Object.entries(g).map(([v,d])=>({name:v,level:typeof d=="number"?d:(d==null?void 0:d.level)??0,maxLevel:5}))}catch{return[]}}async function Ae(){try{const n=await me(),l=typeof n=="string"?JSON.parse(n):n,g={};if(l&&typeof l=="object"){const v=Array.isArray(l)?l:l.paths??[];for(const d of v){const j=d.id??d.path_id??"";j&&(g[j]=d.step??d.completed_steps??0)}}return Object.keys(g).length>0?g:null}catch{return null}}async function Le(n,l){try{await ve(n,l)}catch{}}const G=[{id:"build-agent",title:"Build a File-Reader Agent",description:"Create an agent from scratch that reads files with proper governance. Follow the teach-mode prompts to build it step by step.",difficulty:"beginner",category:"Getting Started",prompts:["Create a TOML manifest for a file-reader agent with fs.read capability, L1 autonomy, and 1000 fuel budget.","Register the agent with the kernel and verify it appears in the agent list.","Start the agent and ask it to read a file. Observe the audit trail entry.","Try asking it to write a file and confirm the capability denial is logged."],xp:300},{id:"build-rag",title:"Build a RAG Pipeline",description:"Set up a complete retrieval-augmented generation pipeline: index documents, search, and chat with them.",difficulty:"intermediate",category:"RAG",prompts:["Index a markdown document into the RAG pipeline using the Chat Hub.","Search the indexed documents with a semantic query and review the ranked results.","Enable document context mode and ask a question grounded in your indexed data.","Verify the sources section shows which document chunks were used."],xp:400},{id:"build-security",title:"Lock Down an Agent",description:"Take an over-permissioned agent and reduce it to minimum viable capabilities using the Permission Dashboard.",difficulty:"intermediate",category:"Security",prompts:["Open the Permission Dashboard and identify an agent with more capabilities than it needs.","Remove unnecessary capabilities (e.g., shell.exec from a read-only agent).","Set the HITL tier to Tier2 for any write operations.","Run the agent and confirm denied actions appear in the audit trail."],xp:400},{id:"build-pipeline",title:"Create a Multi-Agent Pipeline",description:"Wire together multiple agents into a governed pipeline where output from one agent feeds into the next.",difficulty:"advanced",category:"Security",prompts:["Create two agents: a researcher (llm.query + web.search) and a writer (llm.query + fs.write).","Configure the researcher at L2 autonomy and the writer at L1.","Set up delegation so the researcher can pass context to the writer.","Run the pipeline end-to-end and verify the audit trail shows the complete chain of actions."],xp:600}],u=[{id:"getting-started",title:"Getting Started with Nexus OS",description:"Create your first governed agent, assign capabilities, configure fuel budgets, and run it. The foundational course for all Nexus OS users.",difficulty:"beginner",category:"Getting Started",xp:500,thumbnail:"linear-gradient(135deg, #0f172a 0%, var(--nexus-accent) 100%)",tags:["agents","basics","governance"],steps:[{title:"Understanding the Architecture",instruction:`Nexus OS is built on a kernel-agent model. The kernel enforces governance (capabilities, fuel, audit), while agents perform actions. Every agent declares its capabilities in a TOML manifest — the kernel checks these before allowing any action.

Key invariants:
• Every action goes through kernel capability checks
• Fuel is checked BEFORE execution, never after
• Audit trail is append-only with hash-chain integrity
• PII is redacted at the LLM gateway boundary`,tip:"Think of the kernel as a strict firewall — agents can only do what their manifest permits."},{title:"Create Your First Agent Manifest",instruction:"Agents are defined by TOML manifests. Create a file called `my-agent.toml` in the `config/agents/` directory with the following structure. The `capabilities` list controls what the agent can do — start with minimal permissions.",code:`[agent]
name = "my-first-agent"
version = "1.0.0"
description = "A simple agent that can read files"

[governance]
autonomy_level = "L1"  # Suggest only
fuel_budget = 1000

[capabilities]
allowed = ["fs.read", "audit.read"]

[runtime]
sandbox = "wasm"
memory_limit_mb = 64`,tip:"L1 means the agent suggests actions but YOU decide. Start here before granting higher autonomy."},{title:"Register the Agent",instruction:`Navigate to the Agent Browser in the sidebar. Click '+ Create Agent' and paste your manifest TOML. The kernel will validate your manifest — checking that all requested capabilities are recognized and governance fields are valid.

If validation passes, your agent appears in the agent list with status 'Created'. You'll see its fuel budget and declared capabilities on the agent card.`,tip:"Watch the audit log — you'll see a 'StateChange' event recording agent creation."},{title:"Start and Interact",instruction:"Click 'Start' on your agent card. The kernel allocates fuel from the budget and moves the agent to 'Running' state. Now try asking it to read a file — it will succeed because `fs.read` is in its capabilities.\n\nTry asking it to write a file — it will be DENIED because `fs.write` is not in the manifest. This is governance in action.",tip:"Every denied action also gets logged to the audit trail — nothing is silent."},{title:"Review the Audit Trail",instruction:`Open System Monitor → Audit Logs. You'll see a complete record of every action your agent attempted, including:

• Successful reads with fuel cost
• Denied write attempts with reason
• State changes (created → running)
• Fuel debits for each operation

The audit trail uses a hash-chain: each entry includes the hash of the previous entry, making tampering detectable.`}]},{id:"rag-basics",title:"RAG Basics — Chat With Your Documents",description:"Index documents into the RAG pipeline, search them semantically, and ask questions with retrieval-augmented generation.",difficulty:"beginner",category:"RAG",xp:600,thumbnail:"linear-gradient(135deg, #0f172a 0%, #a78bfa 100%)",tags:["rag","documents","search"],steps:[{title:"Understanding the RAG Pipeline",instruction:`RAG (Retrieval-Augmented Generation) lets you chat with your documents. The pipeline has three stages:

1. **Index**: Documents are chunked, embedded, and stored in a vector index
2. **Retrieve**: Your query is embedded and matched against document chunks
3. **Generate**: The LLM answers using the retrieved context

Nexus OS supports PDF, Markdown, plain text, and code files. All operations are governed — the indexing agent needs \`fs.read\` capability.`},{title:"Index Your First Document",instruction:`Open AI Chat Hub and click the 'RAG' tab. Use the 'Index Document' button to select a file. The backend calls \`index_document\` which:

1. Reads the file with PII redaction
2. Splits it into chunks (default ~500 tokens)
3. Generates embeddings via the configured LLM provider
4. Stores chunks in the vector index

You can also index from the terminal:
`,code:`// From the AI Chat Hub RAG tab:
// 1. Click "Index Document"
// 2. Select a file (PDF, MD, TXT, or code)
// 3. Watch the progress bar as chunks are indexed

// Or use the API directly:
import { indexDocument } from "../api/backend";
const result = await indexDocument("/path/to/document.md");`,tip:"Start with a README or documentation file — they produce the best RAG results."},{title:"Search Your Documents",instruction:`Once indexed, use the search bar to find relevant chunks. Nexus OS uses semantic search — you don't need exact keywords. Try natural language queries like 'how does authentication work?' instead of 'auth login function'.

The search returns ranked results with similarity scores. Each result shows the source file, chunk position, and a relevance percentage.`,code:`// Search returns ranked chunks:
import { searchDocuments } from "../api/backend";
const results = await searchDocuments("how does fuel work?", 5);
// Returns top 5 matching chunks with similarity scores`},{title:"Chat With Documents",instruction:`The real power is conversational RAG. In the Chat Hub, enable 'Document Context' mode. Now when you ask a question, the system:

1. Searches your indexed documents for relevant context
2. Injects the top chunks into the LLM prompt
3. The LLM answers grounded in YOUR data

This prevents hallucination — the model cites your actual documents.`,tip:"Check the 'Sources' section below each answer to verify which documents were used."},{title:"Manage Your Index",instruction:`Use the RAG management panel to:

• **List indexed documents** — see all files in your index
• **Remove documents** — delete a file from the index
• **View governance** — see who accessed which documents and when
• **Semantic map** — visualize document relationships

All document operations are audit-logged, so you always know who accessed what.`}]},{id:"model-hub",title:"Model Hub — Download and Manage Models",description:"Search for compatible models, check hardware requirements, download them locally, and configure agents to use specific models.",difficulty:"intermediate",category:"Models",xp:700,thumbnail:"linear-gradient(135deg, #0f172a 0%, #f59e0b 100%)",tags:["models","ollama","hardware"],steps:[{title:"Check Your Hardware",instruction:`Before downloading models, check your system capabilities. Open Settings → Hardware or run the hardware profile check. Nexus OS detects:

• **CPU**: Cores, architecture, speed
• **RAM**: Total and available memory
• **GPU**: VRAM, CUDA/Metal support
• **Disk**: Available storage

This determines which models can run locally. A 7B parameter model needs ~4GB VRAM; a 70B model needs ~40GB.`,tip:"If you have <8GB VRAM, stick with 7B models or use quantized (Q4/Q5) variants."},{title:"Search for Models",instruction:`Open Settings → Model Hub. The search queries available model registries and returns models with metadata:

• Name, size, and parameter count
• Quantization level (Q4, Q5, Q8, F16)
• Required VRAM and disk space
• Compatibility status with your hardware

Filter by size to find models that fit your system. Green indicators mean compatible, red means too large.`,code:`// Backend API for model search:
import { searchModels, checkModelCompatibility } from "../api/backend";

// Search by name or capability
const models = await searchModels("codellama", 10);

// Check if a specific model fits your hardware
const compat = await checkModelCompatibility(4_000_000_000);`},{title:"Download a Model",instruction:`Click 'Download' on a compatible model. The system:

1. Verifies disk space and VRAM requirements
2. Downloads the model file (progress shown in real-time)
3. Registers it with the local model registry
4. Makes it available for agent configuration

Downloads are resumable — if interrupted, they continue from where they left off.`,tip:"Start with a small model like 'phi-3-mini' (3.8B) for testing before downloading larger ones."},{title:"Configure Agents to Use Models",instruction:`Each agent can be configured to use a specific LLM provider and model. Open Settings → LLM Providers and:

1. Select an agent from the list
2. Choose a provider (Ollama for local, or cloud providers)
3. Select the model from your downloaded models
4. Set token and dollar budgets

The \`local_only\` flag ensures the agent never sends data to cloud providers — important for sensitive workloads.`},{title:"Monitor Model Usage",instruction:`The Settings → Usage Stats panel shows:

• Tokens consumed per agent per model
• Cost tracking (local = free, cloud = metered)
• Latency metrics per provider
• Error rates and retry counts

Use this to optimize which models each agent uses. If an agent's task is simple, a smaller model saves resources.`}]},{id:"security",title:"Security — Capabilities, HITL, and Fuel",description:"Deep dive into the Nexus OS security model: capability-based permissions, human-in-the-loop approval tiers, fuel budgets, and audit integrity.",difficulty:"intermediate",category:"Security",xp:800,thumbnail:"linear-gradient(135deg, #0f172a 0%, #22c55e 100%)",tags:["security","capabilities","hitl","fuel"],steps:[{title:"Capability-Based Security",instruction:"Every action an agent takes requires a capability. Capabilities are declared in the agent manifest and checked by the kernel before execution.\n\nValid capabilities: `llm.query`, `web.search`, `social.post`, `messaging.send`, `fs.read`, `fs.write`, `screen.capture`, `input.keyboard`, `shell.exec`, `audit.read`\n\nThe principle: agents get MINIMUM privileges needed. A research agent needs `llm.query` and `web.search` but NOT `shell.exec`.",code:`# Agent manifest — minimal permissions:
[capabilities]
allowed = ["llm.query", "web.search"]

# Kernel check (Rust):
# supervisor.check_capability(agent_id, "fs.write")?;
# → Returns Err(CapabilityDenied) if not in manifest`,tip:"Use the Permission Dashboard (Settings → Permissions) to visualize which agents have which capabilities."},{title:"Human-in-the-Loop (HITL) Tiers",instruction:`Not all actions are equal. Nexus OS classifies actions into HITL tiers:

• **Tier 0**: Auto-approved (reading audit logs, status checks)
• **Tier 1**: Notification required (file reads, LLM queries)
• **Tier 2**: Human approval required (file writes, deploys, social posts)
• **Tier 3**: Board approval required (agent creation, capability changes)

Tier 2+ actions trigger an approval dialog. The speculative engine shows you what WOULD happen before you approve.`},{title:"Fuel Budgets",instruction:`Fuel prevents runaway agents. Every action costs fuel, checked BEFORE execution:

1. Agent requests action (cost: N fuel)
2. Kernel checks: \`remaining_fuel >= N\`?
3. If yes: debit N, execute, log
4. If no: deny action, log denial

Fuel budgets are set per-agent in the manifest. When an agent runs out, it gracefully stops — no crashes, no unbounded resource consumption.`,code:`# Set fuel budget in manifest:
[governance]
fuel_budget = 5000  # total fuel units

# Typical costs:
# LLM query: 10-50 fuel (depends on token count)
# File read: 5 fuel
# File write: 15 fuel
# Web search: 20 fuel
# Shell exec: 50 fuel`},{title:"Audit Trail Integrity",instruction:`Every action — approved or denied — is logged to the audit trail. The trail uses a hash-chain:

• Each event has a hash that includes the previous event's hash
• This creates a tamper-evident chain
• If any event is modified, all subsequent hashes break
• Ed25519 signatures provide non-repudiation

To verify: open System Monitor → Audit and check the chain integrity indicator.`},{title:"Autonomy Levels",instruction:`Agents operate at autonomy levels L0-L5:

• **L0 Inert**: Agent does nothing
• **L1 Suggest**: Agent suggests, human decides
• **L2 Act-with-approval**: Human approves each action
• **L3 Act-then-report**: Agent acts freely, reports after
• **L4 Autonomous-bounded**: Full autonomy within limits
• **L5 Full autonomy**: Only kernel override stops it

Most agents start at L1-L2. Promotion requires demonstrated track record. The kernel can ALWAYS override any agent, regardless of autonomy level.`,tip:"Never set an untested agent to L3+. Start at L1, observe behavior, then promote gradually."}]},{id:"time-machine",title:"Time Machine — Checkpoints and Undo",description:"Create system checkpoints, undo actions, redo them, and view diffs between states. Your safety net for agent operations.",difficulty:"beginner",category:"Time Machine",xp:400,thumbnail:"linear-gradient(135deg, #0f172a 0%, #ec4899 100%)",tags:["time-machine","undo","checkpoints"],steps:[{title:"What is the Time Machine?",instruction:`The Time Machine captures snapshots of system state — agent configurations, fuel levels, audit entries — so you can undo any change.

Think of it as version control for your entire Nexus OS instance. Before any risky operation, create a checkpoint. If something goes wrong, restore to that point instantly.`},{title:"Create a Checkpoint",instruction:`Navigate to System Monitor → Time Machine. Click 'Create Checkpoint' and give it a label (e.g., 'before-deploy' or 'pre-upgrade').

The checkpoint captures:
• All agent states and fuel levels
• Configuration values
• Permission settings
• A snapshot of the audit trail position`,code:`// API usage:
import { timeMachineCreateCheckpoint } from "../api/backend";
const checkpoint = await timeMachineCreateCheckpoint("before-deploy");
// Returns checkpoint ID for later reference`,tip:"Always create a checkpoint before running a full pipeline deploy or changing agent permissions."},{title:"Undo and Redo",instruction:`The Time Machine supports linear undo/redo:

• **Undo**: Reverts the last state change
• **Redo**: Re-applies the last undone change
• **Undo to checkpoint**: Reverts ALL changes back to a specific checkpoint

Undo/redo works on state changes only — audit trail entries are never deleted (append-only invariant).`,code:`// Quick undo/redo:
import { timeMachineUndo, timeMachineRedo } from "../api/backend";
await timeMachineUndo();   // revert last change
await timeMachineRedo();   // re-apply last undo`},{title:"View Diffs",instruction:`Before restoring a checkpoint, view the diff to see exactly what will change. The diff shows:

• Added/removed agents
• Changed fuel levels
• Modified permissions
• Configuration changes

This lets you make an informed decision before reverting.`,code:`import { timeMachineGetDiff } from "../api/backend";
const diff = await timeMachineGetDiff(checkpointId);
// Shows what changed between checkpoint and current state`},{title:"Best Practices",instruction:`Tips for effective Time Machine usage:

1. **Name checkpoints descriptively** — 'before-deploy-v5.0.2' not 'checkpoint-1'
2. **Checkpoint before risky ops** — deploys, permission changes, agent promotions
3. **Review diffs before restoring** — don't blindly undo
4. **Checkpoints are lightweight** — create them freely, they're cheap
5. **Audit trail is immutable** — even undo/redo creates new audit entries tracking the revert`}]}],f=[{id:"ch-1",title:"Implement Fuel Check",difficulty:"beginner",category:"Security",description:"Write a function that checks if an agent has enough fuel before executing an action. Return an error if fuel is insufficient.",starterCode:`fn check_fuel(available: u64, required: u64) -> Result<(), String> {
    // Your code here
    todo!()
}`,expectedOutput:`Ok(()) when available >= required
Err("Insufficient fuel") when available < required`,hints:["Compare available against required","Use if/else to return the right Result variant"],xp:50},{id:"ch-2",title:"Capability Gate",difficulty:"intermediate",category:"Security",description:"Implement a capability check that verifies an agent has the required capability before allowing an action.",starterCode:`fn has_capability(
    agent_caps: &[&str],
    required: &str
) -> bool {
    // Support exact match and "*" wildcard
    todo!()
}`,expectedOutput:`true when agent has exact cap or "*"
false otherwise`,hints:['Check for "*" in agent_caps first',"Then check for exact match with .contains()"],xp:100},{id:"ch-3",title:"Hash Chain Audit",difficulty:"advanced",category:"Security",description:"Build a simple hash-chain audit trail. Each event's hash must include the previous event's hash.",starterCode:`use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

struct AuditEvent {
    data: String,
    prev_hash: u64,
    hash: u64,
}

fn append_event(chain: &mut Vec<AuditEvent>, data: String) {
    todo!()
}`,expectedOutput:`Each event.prev_hash == previous event.hash
First event.prev_hash == 0`,hints:["Get last event hash (or 0)","Hash both prev_hash and data together","Push the new event"],xp:200},{id:"ch-4",title:"PII Redactor",difficulty:"intermediate",category:"Security",description:"Write a PII redaction function that replaces email addresses and phone numbers with [REDACTED].",starterCode:`fn redact_pii(input: &str) -> String {
    // Redact emails: user@domain.com -> [REDACTED]
    // Redact phones: +1-234-567-8901 -> [REDACTED]
    todo!()
}`,expectedOutput:'"Contact [REDACTED] or call [REDACTED]"',hints:["Use regex or manual pattern matching","Emails contain @ with domain"],xp:100},{id:"ch-5",title:"HITL Approval Flow",difficulty:"beginner",category:"Security",description:"Implement a HITL check. Tier0 auto-approves, Tier1+ requires human approval.",starterCode:`enum HitlTier { Tier0, Tier1, Tier2, Tier3 }

fn needs_approval(tier: HitlTier) -> bool {
    todo!()
}`,expectedOutput:`false for Tier0
true for Tier1, Tier2, Tier3`,hints:["Match on the tier enum","Only Tier0 is auto-approved"],xp:50}],ee=[{id:"k-1",title:"Why `unsafe` is Forbidden",content:"Nexus OS enforces `#![forbid(unsafe_code)]` across all crates. If an agent could trigger undefined behavior, it could bypass capability checks, corrupt the audit trail, or escalate privileges. Safety is not just a language feature — it's a governance invariant.",category:"Security",tags:["rust","safety"]},{id:"k-2",title:"Fuel Pre-Check Pattern",content:`Every agent action costs fuel. The kernel checks fuel BEFORE execution:

1. check_fuel(cost) — verify budget
2. debit_fuel(cost) — subtract
3. perform_action() — execute
4. audit_log(action) — record

This ensures agents cannot consume resources beyond their budget.`,category:"Security",tags:["fuel","governance"]},{id:"k-3",title:"Autonomy Levels L0-L5",content:"L0 Inert → L1 Suggest → L2 Act-with-approval → L3 Act-then-report → L4 Autonomous-bounded → L5 Full autonomy. Most agents start at L1-L2. Promotion requires demonstrated track record. The kernel can always override.",category:"Getting Started",tags:["autonomy","governance"]},{id:"k-4",title:"Speculative Execution Engine",content:"Before a Tier2+ action is approved, the kernel runs it speculatively in a shadow context. This 'what-if' simulation shows exactly what would happen without committing. If approved: commit. If denied: discard. Eliminates surprises.",category:"Security",tags:["speculative","hitl"]},{id:"k-5",title:"RAG Pipeline Architecture",content:"Documents → Chunker → Embedder → Vector Store → Retriever → LLM. Supports PDF, MD, TXT, code. All operations are governed — indexing agents need fs.read capability. PII is redacted before embedding.",category:"RAG",tags:["rag","documents"]}];function Oe(){const[n,l]=i.useState("courses"),[g,v]=i.useState(null),[d,j]=i.useState(null),[O,q]=i.useState(""),[p,F]=i.useState({}),[S,z]=i.useState(-1),[C,se]=i.useState("All"),[h,H]=i.useState(Se),[ae,E]=i.useState(0),[T,ie]=i.useState([]),[U,Y]=i.useState(!1),[x,P]=i.useState(null),[D,y]=i.useState([]),[A,I]=i.useState(""),[B,X]=i.useState(!1),[w,W]=i.useState(0),V=i.useRef(null);i.useEffect(()=>{let t=!1;return(async()=>{const s=await Te();!t&&s.length>0&&(ie(s),Y(!0));const a=await Ae(),o=await Ce(),L={...a,...o};!t&&Object.keys(L).length>0&&(Y(!0),H(oe=>{const M={...oe};for(const[J,K]of Object.entries(L))K!=null&&(M[J]=Math.max(M[J]??0,K));return Z(M),M}))})(),()=>{t=!0}},[]),i.useEffect(()=>{Z(h)},[h]);const r=i.useMemo(()=>u.find(t=>t.id===g)??null,[g]),c=i.useMemo(()=>f.find(t=>t.id===d)??null,[d]),N=i.useMemo(()=>u.filter(t=>(h[t.id]??0)>=t.steps.length).length,[h]),m=i.useMemo(()=>{let t=0;for(const s of u)(h[s.id]??0)>=s.steps.length&&(t+=s.xp);for(const s of f)p[s.id]==="pass"&&(t+=s.xp);return t},[h,p]),k=i.useMemo(()=>{const t=u.reduce((a,o)=>a+o.steps.length,0),s=u.reduce((a,o)=>a+Math.min(h[o.id]??0,o.steps.length),0);return t>0?Math.round(s/t*100):0},[h]),$=i.useMemo(()=>C==="All"?u:u.filter(t=>t.category===C),[C]),ne=i.useCallback((t,s)=>{H(a=>{const o=a[t]??0;return s>=o?{...a,[t]:s+1}:a}),E(s+1),Le(t,String(s))},[]),le=i.useCallback(async t=>{P(t),W(0),y([]),I("");try{const s=await de(t),a=typeof s=="string"?s:JSON.stringify(s);y([{role:"system",content:a||"Teach mode started. Follow the prompts below to build step by step."}])}catch{y([{role:"system",content:"Teach mode started (offline). Follow the prompts below to build step by step."}])}},[]),_=i.useCallback(async t=>{if(!x||!t.trim())return;const s={role:"user",content:t.trim()};y(a=>[...a,s]),I(""),X(!0);try{const a=await he(x,t.trim()),o=typeof a=="string"?a:JSON.stringify(a);y(L=>[...L,{role:"system",content:o||"Step completed. Continue to the next prompt."}])}catch{y(a=>[...a,{role:"system",content:"Response recorded (offline). Continue to the next prompt."}])}X(!1)},[x]);i.useEffect(()=>{var t;(t=V.current)==null||t.scrollIntoView({behavior:"smooth"})},[D]);const re=i.useCallback(async()=>{if(c)try{const t=await ue(c.id,O,"rust"),s=JSON.parse(t);F(a=>({...a,[c.id]:s.passed?"pass":"fail"}))}catch{F(t=>({...t,[c.id]:"fail"}))}},[c,O]),ce=i.useCallback(t=>{j(t);const s=f.find(a=>a.id===t);s&&q(s.starterCode),z(-1)},[]);return e.jsxs("div",{className:"lc-container",children:[e.jsxs("aside",{className:"lc-sidebar",children:[e.jsxs("div",{className:"lc-sidebar-header",children:[e.jsx("h2",{className:"lc-sidebar-title",children:"Learning Center"}),e.jsxs("div",{className:"lc-xp-badge",children:["XP ",m]})]}),e.jsx("div",{className:"lc-views",children:[["courses","courses","Courses"],["challenges","challenges","Challenges"],["build","build","Build"],["knowledge","knowledge","Knowledge"],["progress","progress","Progress"]].map(([t,s,a])=>e.jsxs("button",{className:`lc-view-btn cursor-pointer ${n===t?"active":""}`,onClick:()=>{l(t),v(null),j(null),P(null)},children:[e.jsx("span",{children:s==="courses"?e.jsx(xe,{size:14,"aria-hidden":"true"}):s==="challenges"?e.jsx(ye,{size:14,"aria-hidden":"true"}):s==="build"?e.jsx(be,{size:14,"aria-hidden":"true"}):s==="knowledge"?e.jsx(je,{size:14,"aria-hidden":"true"}):e.jsx(we,{size:14,"aria-hidden":"true"})})," ",a]},t))}),e.jsxs("div",{className:"lc-progress-card",children:[e.jsx("div",{className:"lc-section-header",children:"Your Progress"}),e.jsx("div",{className:"lc-progress-bar-container",children:e.jsx("div",{className:"lc-progress-bar",style:{width:`${k}%`}})}),e.jsxs("div",{className:"lc-progress-label",children:[k,"% complete"]}),e.jsxs("div",{className:"lc-progress-stats",children:[e.jsxs("div",{className:"lc-stat",children:[e.jsx("span",{children:N}),e.jsx("span",{children:"Completed"})]}),e.jsxs("div",{className:"lc-stat",children:[e.jsx("span",{children:u.length-N}),e.jsx("span",{children:"Remaining"})]}),e.jsxs("div",{className:"lc-stat",children:[e.jsxs("span",{children:[Object.values(p).filter(t=>t==="pass").length,"/",f.length]}),e.jsx("span",{children:"Challenges"})]})]})]}),T.length>0&&e.jsxs("div",{className:"lc-progress-card",children:[e.jsx("div",{className:"lc-section-header",children:"Skill Levels"}),T.map(t=>e.jsxs("div",{style:{marginBottom:8},children:[e.jsxs("div",{style:{display:"flex",justifyContent:"space-between",fontSize:"0.8rem",color:"#94a3b8",marginBottom:2},children:[e.jsx("span",{style:{textTransform:"capitalize"},children:t.name.replace(/_/g," ")}),e.jsxs("span",{children:[t.level,"/",t.maxLevel]})]}),e.jsx("div",{className:"lc-progress-bar-container",style:{height:4},children:e.jsx("div",{className:"lc-progress-bar",style:{width:`${t.level/t.maxLevel*100}%`,background:t.level>=4?"#22c55e":t.level>=2?"#f59e0b":"#64748b"}})})]},t.name))]}),e.jsxs("div",{className:"lc-filters",children:[e.jsx("div",{className:"lc-section-header",children:"Filter"}),e.jsx("select",{className:"lc-filter-select",value:C,onChange:t=>se(t.target.value),children:ke.map(t=>e.jsx("option",{value:t,children:t},t))})]}),e.jsxs("div",{className:"lc-audit",children:[e.jsx("div",{className:"lc-section-header",children:"Completion"}),u.map(t=>{const s=(h[t.id]??0)>=t.steps.length;return e.jsxs("div",{className:"lc-audit-entry",style:{color:s?"#22c55e":"#94a3b8"},children:[s?e.jsx(R,{size:12,"aria-hidden":"true",style:{display:"inline",verticalAlign:"middle",marginRight:4}}):e.jsx(Q,{size:12,"aria-hidden":"true",style:{display:"inline",verticalAlign:"middle",marginRight:4}})," ",t.title.slice(0,28)]},t.id)})]})]}),e.jsxs("div",{className:"lc-main",children:[n==="courses"&&!g&&e.jsxs("div",{className:"lc-courses",children:[e.jsxs("div",{className:"lc-view-header",children:[e.jsx("h3",{className:"lc-view-title",children:"Courses"}),e.jsxs("span",{className:"lc-view-count",children:[$.length," courses"]})]}),e.jsx("div",{className:"lc-course-grid",children:$.map(t=>{const s=h[t.id]??0,a=s>=t.steps.length;return e.jsxs("div",{className:"lc-course-card",onClick:()=>{v(t.id),E(Math.min(s,t.steps.length-1))},children:[e.jsxs("div",{className:"lc-course-thumb",style:{background:t.thumbnail},children:[e.jsx("span",{className:"lc-course-diff",style:{color:b[t.difficulty]},children:t.difficulty}),a&&e.jsx("span",{style:{position:"absolute",top:8,right:8,color:"#22c55e",fontWeight:700,fontSize:"1.1rem"},children:"COMPLETE"})]}),e.jsxs("div",{className:"lc-course-body",children:[e.jsx("div",{className:"lc-course-title",children:t.title}),e.jsxs("div",{className:"lc-course-desc",children:[t.description.slice(0,100),"..."]}),e.jsxs("div",{className:"lc-course-meta",children:[e.jsx("span",{children:t.category}),e.jsxs("span",{children:["XP ",t.xp]})]}),e.jsxs("div",{className:"lc-course-progress-row",children:[e.jsx("div",{className:"lc-course-progress-bar",children:e.jsx("div",{className:"lc-course-progress-fill",style:{width:`${Math.min(s,t.steps.length)/t.steps.length*100}%`}})}),e.jsxs("span",{className:"lc-course-progress-text",children:[Math.min(s,t.steps.length),"/",t.steps.length]})]}),e.jsx("div",{className:"lc-course-tags",children:t.tags.map(o=>e.jsx("span",{className:"lc-tag",children:o},o))})]})]},t.id)})})]}),n==="courses"&&r&&e.jsxs("div",{className:"lc-course-detail",children:[e.jsx("button",{className:"lc-back-btn",onClick:()=>v(null),children:"← Back to Courses"}),e.jsxs("div",{className:"lc-cd-header",children:[e.jsx("div",{className:"lc-cd-thumb",style:{background:r.thumbnail}}),e.jsxs("div",{className:"lc-cd-info",children:[e.jsx("h3",{className:"lc-cd-title",children:r.title}),e.jsx("div",{className:"lc-cd-desc",children:r.description}),e.jsxs("div",{className:"lc-cd-meta",children:[e.jsx("span",{className:"lc-cd-diff",style:{color:b[r.difficulty]},children:r.difficulty}),e.jsx("span",{children:r.category}),e.jsxs("span",{children:["XP ",r.xp]}),e.jsxs("span",{children:[r.steps.length," steps"]})]}),e.jsxs("div",{className:"lc-cd-progress",children:[e.jsx("div",{className:"lc-cd-progress-bar",children:e.jsx("div",{className:"lc-cd-progress-fill",style:{width:`${Math.min(h[r.id]??0,r.steps.length)/r.steps.length*100}%`}})}),e.jsxs("span",{children:[Math.min(h[r.id]??0,r.steps.length),"/",r.steps.length," completed"]})]})]})]}),e.jsxs("div",{className:"lc-cd-lessons",children:[e.jsx("h4",{children:"Steps"}),r.steps.map((t,s)=>{const a=s<(h[r.id]??0),o=ae===s;return e.jsxs("div",{children:[e.jsxs("div",{className:`lc-cd-lesson ${a?"completed":o?"current":""}`,onClick:()=>E(s),style:{cursor:"pointer"},children:[e.jsx("span",{className:"lc-cd-lesson-num",children:a?e.jsx(R,{size:14,"aria-hidden":"true"}):s+1}),e.jsx("span",{className:"lc-cd-lesson-title",children:t.title})]}),o&&e.jsxs("div",{style:{padding:"1rem 1rem 1rem 3rem",borderLeft:"2px solid var(--nexus-accent)33",marginLeft:"0.5rem",marginBottom:"0.5rem"},children:[e.jsx("div",{style:{color:"#e2e8f0",lineHeight:1.7,whiteSpace:"pre-wrap"},children:t.instruction}),t.code&&e.jsx("pre",{style:{background:"#0f172a",padding:"1rem",borderRadius:8,marginTop:"1rem",border:"1px solid #1e293b",overflow:"auto",fontSize:"0.85rem",color:"var(--nexus-accent)"},children:t.code}),t.tip&&e.jsxs("div",{style:{marginTop:"1rem",padding:"0.75rem 1rem",borderRadius:8,background:"#f59e0b11",border:"1px solid #f59e0b33",color:"#f59e0b",fontSize:"0.85rem"},children:["Tip: ",t.tip]}),!a&&e.jsx("button",{className:"lc-start-course-btn",style:{marginTop:"1rem"},onClick:()=>ne(r.id,s),children:s===r.steps.length-1?"Complete Course":"Mark Complete & Next"})]})]},s)})]})]}),n==="challenges"&&e.jsxs("div",{className:"lc-challenges",children:[e.jsxs("div",{className:"lc-view-header",children:[e.jsx("h3",{className:"lc-view-title",children:"Code Challenges"}),e.jsxs("span",{className:"lc-view-count",children:[Object.values(p).filter(t=>t==="pass").length,"/",f.length," solved"]})]}),e.jsxs("div",{className:"lc-challenges-grid",children:[e.jsx("div",{className:"lc-challenge-list",children:f.map(t=>e.jsxs("div",{className:`lc-challenge-item ${d===t.id?"active":""}`,onClick:()=>ce(t.id),children:[e.jsx("div",{className:"lc-ch-status",children:p[t.id]==="pass"?e.jsx("span",{className:"lc-ch-pass",children:e.jsx(R,{size:14,"aria-hidden":"true"})}):p[t.id]==="fail"?e.jsx("span",{className:"lc-ch-fail",children:e.jsx(Ne,{size:14,"aria-hidden":"true"})}):e.jsx("span",{className:"lc-ch-untried",children:e.jsx(Q,{size:14,"aria-hidden":"true"})})}),e.jsxs("div",{className:"lc-ch-info",children:[e.jsx("div",{className:"lc-ch-title",children:t.title}),e.jsxs("div",{className:"lc-ch-meta",children:[e.jsx("span",{style:{color:b[t.difficulty]},children:t.difficulty}),e.jsxs("span",{children:["XP ",t.xp]})]})]})]},t.id))}),c?e.jsxs("div",{className:"lc-challenge-editor",children:[e.jsxs("div",{className:"lc-ce-header",children:[e.jsx("h4",{children:c.title}),e.jsxs("span",{className:"lc-ce-diff",style:{color:b[c.difficulty]},children:[c.difficulty," · XP ",c.xp]})]}),e.jsx("div",{className:"lc-ce-desc",children:c.description}),e.jsxs("div",{className:"lc-ce-expected",children:[e.jsx("div",{className:"lc-ce-expected-label",children:"Expected Output:"}),e.jsx("pre",{children:c.expectedOutput})]}),e.jsxs("div",{className:"lc-ce-code-area",children:[e.jsxs("div",{className:"lc-ce-code-header",children:[e.jsx("span",{children:"Solution"}),e.jsx("span",{className:"lc-ce-lang",children:"rust"})]}),e.jsx("textarea",{className:"lc-ce-textarea",value:O,onChange:t=>q(t.target.value),spellCheck:!1})]}),e.jsxs("div",{className:"lc-ce-actions",children:[e.jsx("button",{className:"lc-ce-run",onClick:re,children:"Run & Check"}),c.hints.map((t,s)=>e.jsxs("button",{className:"lc-ce-hint",onClick:()=>z(S===s?-1:s),children:["Hint ",s+1]},s))]}),S>=0&&S<c.hints.length&&e.jsx("div",{className:"lc-ce-hint-box",children:c.hints[S]}),p[c.id]==="pass"&&e.jsxs("div",{className:"lc-ce-result lc-ce-result-pass",children:["All tests passed! +",c.xp," XP"]}),p[c.id]==="fail"&&e.jsx("div",{className:"lc-ce-result lc-ce-result-fail",children:"Tests failed — check your logic and try again."})]}):e.jsxs("div",{className:"lc-ce-empty",children:[e.jsx("div",{className:"lc-ce-empty-icon",children:">_"}),e.jsx("div",{children:"Select a challenge to begin"})]})]})]}),n==="knowledge"&&e.jsxs("div",{className:"lc-knowledge",children:[e.jsxs("div",{className:"lc-view-header",children:[e.jsx("h3",{className:"lc-view-title",children:"Knowledge Base"}),e.jsxs("span",{className:"lc-view-count",children:[ee.length," articles"]})]}),e.jsx("div",{className:"lc-kb-list",children:ee.map(t=>e.jsxs("div",{className:"lc-kb-card",children:[e.jsxs("div",{className:"lc-kb-header",children:[e.jsx("div",{className:"lc-kb-title",children:t.title}),e.jsx("div",{className:"lc-kb-meta",children:e.jsx("span",{children:t.category})})]}),e.jsx("div",{className:"lc-kb-content",style:{whiteSpace:"pre-wrap"},children:t.content}),e.jsx("div",{className:"lc-kb-tags",children:t.tags.map(s=>e.jsx("span",{className:"lc-tag",children:s},s))})]},t.id))})]}),n==="build"&&!x&&e.jsxs("div",{className:"lc-courses",children:[e.jsxs("div",{className:"lc-view-header",children:[e.jsx("h3",{className:"lc-view-title",children:"Learn by Building"}),e.jsxs("span",{className:"lc-view-count",children:[G.length," projects"]})]}),e.jsx("p",{style:{color:"#94a3b8",marginBottom:"1rem",fontSize:"0.9rem"},children:"Hands-on projects using teach mode. The backend guides you step by step as you build real Nexus OS features."}),e.jsx("div",{className:"lc-course-grid",children:G.map(t=>e.jsxs("div",{className:"lc-course-card",onClick:()=>le(t.id),children:[e.jsx("div",{className:"lc-course-thumb",style:{background:"linear-gradient(135deg, #0f172a 0%, #6366f1 100%)"},children:e.jsx("span",{className:"lc-course-diff",style:{color:b[t.difficulty]},children:t.difficulty})}),e.jsxs("div",{className:"lc-course-body",children:[e.jsx("div",{className:"lc-course-title",children:t.title}),e.jsx("div",{className:"lc-course-desc",children:t.description}),e.jsxs("div",{className:"lc-course-meta",children:[e.jsx("span",{children:t.category}),e.jsxs("span",{children:["XP ",t.xp]}),e.jsxs("span",{children:[t.prompts.length," steps"]})]})]})]},t.id))})]}),n==="build"&&x&&(()=>{const t=G.find(s=>s.id===x);return t?e.jsxs("div",{className:"lc-course-detail",children:[e.jsx("button",{className:"lc-back-btn",onClick:()=>P(null),children:"← Back to Projects"}),e.jsxs("div",{className:"lc-cd-header",children:[e.jsx("div",{className:"lc-cd-thumb",style:{background:"linear-gradient(135deg, #0f172a 0%, #6366f1 100%)"}}),e.jsxs("div",{className:"lc-cd-info",children:[e.jsx("h3",{className:"lc-cd-title",children:t.title}),e.jsx("div",{className:"lc-cd-desc",children:t.description}),e.jsxs("div",{className:"lc-cd-meta",children:[e.jsx("span",{className:"lc-cd-diff",style:{color:b[t.difficulty]},children:t.difficulty}),e.jsx("span",{children:t.category}),e.jsxs("span",{children:["XP ",t.xp]})]})]})]}),e.jsxs("div",{style:{margin:"1rem 0"},children:[e.jsx("h4",{style:{color:"#e2e8f0",marginBottom:"0.5rem"},children:"Steps"}),t.prompts.map((s,a)=>e.jsxs("div",{className:`lc-cd-lesson ${a<w?"completed":a===w?"current":""}`,style:{cursor:a===w?"pointer":"default"},onClick:()=>{a===w&&(W(a+1),_(s))},children:[e.jsx("span",{className:"lc-cd-lesson-num",children:a<w?e.jsx(R,{size:14,"aria-hidden":"true"}):a+1}),e.jsx("span",{className:"lc-cd-lesson-title",children:s})]},a))]}),e.jsxs("div",{style:{background:"#0f172a",border:"1px solid #1e293b",borderRadius:8,padding:"1rem",maxHeight:320,overflowY:"auto",marginBottom:"1rem"},children:[D.length===0&&e.jsx("div",{style:{color:"#64748b",textAlign:"center",padding:"2rem 0"},children:"Click a step above or type below to start the teach-mode conversation."}),D.map((s,a)=>e.jsxs("div",{style:{marginBottom:8,padding:"0.5rem 0.75rem",borderRadius:6,background:s.role==="user"?"#1e293b":"transparent",borderLeft:s.role==="system"?"2px solid var(--nexus-accent)":"none"},children:[e.jsx("div",{style:{fontSize:"0.7rem",color:"#64748b",marginBottom:2},children:s.role==="user"?"You":"Nexus Teach Mode"}),e.jsx("div",{style:{color:"#e2e8f0",fontSize:"0.85rem",whiteSpace:"pre-wrap"},children:s.content})]},a)),B&&e.jsx("div",{style:{color:"#64748b",fontSize:"0.85rem",padding:"0.25rem 0.75rem"},children:"Thinking..."}),e.jsx("div",{ref:V})]}),e.jsxs("div",{style:{display:"flex",gap:8},children:[e.jsx("input",{style:{flex:1,background:"#1e293b",border:"1px solid #334155",borderRadius:6,padding:"0.5rem 0.75rem",color:"#e2e8f0",fontSize:"0.85rem",outline:"none"},placeholder:"Type your response or describe what you built...",value:A,onChange:s=>I(s.target.value),onKeyDown:s=>{s.key==="Enter"&&!s.shiftKey&&(s.preventDefault(),_(A))},disabled:B}),e.jsx("button",{className:"lc-start-course-btn",onClick:()=>_(A),disabled:B||!A.trim(),children:"Send"})]})]}):null})(),n==="progress"&&e.jsxs("div",{className:"lc-progress-view",children:[e.jsx("div",{className:"lc-view-header",children:e.jsx("h3",{className:"lc-view-title",children:"Your Learning Progress"})}),e.jsxs("div",{className:"lc-pv-hero",children:[e.jsxs("div",{className:"lc-pv-xp",children:[e.jsxs("div",{className:"lc-pv-xp-number",children:["XP ",m]}),e.jsx("div",{className:"lc-pv-xp-label",children:"Total XP"})]}),e.jsxs("div",{className:"lc-pv-level",children:[e.jsxs("div",{className:"lc-pv-level-badge",children:["Level ",Math.floor(m/500)+1]}),e.jsx("div",{className:"lc-pv-level-title",children:m<500?"Apprentice":m<1e3?"Practitioner":m<2e3?"Engineer":m<4e3?"Architect":"Master"}),e.jsx("div",{className:"lc-pv-level-bar",children:e.jsx("div",{className:"lc-pv-level-fill",style:{width:`${m%500/5}%`}})}),e.jsxs("div",{className:"lc-pv-level-next",children:[500-m%500," XP to next level"]})]})]}),e.jsxs("div",{className:"lc-pv-stats",children:[e.jsxs("div",{className:"lc-pv-stat-card",children:[e.jsxs("div",{className:"lc-pv-stat-value",children:[k,"%"]}),e.jsx("div",{className:"lc-pv-stat-label",children:"Course Completion"}),e.jsx("div",{className:"lc-pv-stat-bar",children:e.jsx("div",{style:{width:`${k}%`,background:"var(--nexus-accent)",height:"100%",borderRadius:2}})})]}),e.jsxs("div",{className:"lc-pv-stat-card",children:[e.jsxs("div",{className:"lc-pv-stat-value",children:[N,"/",u.length]}),e.jsx("div",{className:"lc-pv-stat-label",children:"Courses Completed"}),e.jsx("div",{className:"lc-pv-stat-bar",children:e.jsx("div",{style:{width:`${N/u.length*100}%`,background:"#22c55e",height:"100%",borderRadius:2}})})]}),e.jsxs("div",{className:"lc-pv-stat-card",children:[e.jsxs("div",{className:"lc-pv-stat-value",children:[Object.values(p).filter(t=>t==="pass").length,"/",f.length]}),e.jsx("div",{className:"lc-pv-stat-label",children:"Challenges Solved"}),e.jsx("div",{className:"lc-pv-stat-bar",children:e.jsx("div",{style:{width:`${Object.values(p).filter(t=>t==="pass").length/f.length*100}%`,background:"#f59e0b",height:"100%",borderRadius:2}})})]})]}),T.length>0&&e.jsxs(e.Fragment,{children:[e.jsx("h4",{className:"lc-sub-title",children:"Skill Levels"}),e.jsx("div",{className:"lc-pv-stats",style:{marginBottom:"1.5rem"},children:T.map(t=>e.jsxs("div",{className:"lc-pv-stat-card",children:[e.jsxs("div",{className:"lc-pv-stat-value",children:[t.level,"/",t.maxLevel]}),e.jsx("div",{className:"lc-pv-stat-label",style:{textTransform:"capitalize"},children:t.name.replace(/_/g," ")}),e.jsx("div",{className:"lc-pv-stat-bar",children:e.jsx("div",{style:{width:`${t.level/t.maxLevel*100}%`,background:t.level>=4?"#22c55e":t.level>=2?"#f59e0b":"#6366f1",height:"100%",borderRadius:2}})})]},t.name))})]}),e.jsx("h4",{className:"lc-sub-title",children:"Course Progress"}),e.jsx("div",{className:"lc-pv-courses",children:u.map(t=>{const s=Math.min(h[t.id]??0,t.steps.length),a=s>=t.steps.length;return e.jsxs("div",{className:"lc-pv-course-row",children:[e.jsxs("div",{className:"lc-pv-course-info",children:[e.jsx("span",{className:"lc-pv-course-name",children:t.title}),e.jsx("span",{className:"lc-pv-course-status",style:{color:a?"#22c55e":s>0?"#f59e0b":"#64748b"},children:a?"Complete":s>0?"In Progress":"Not Started"})]}),e.jsxs("div",{className:"lc-pv-course-bar-row",children:[e.jsx("div",{className:"lc-pv-course-bar",children:e.jsx("div",{className:"lc-pv-course-fill",style:{width:`${s/t.steps.length*100}%`}})}),e.jsxs("span",{className:"lc-pv-course-pct",children:[Math.round(s/t.steps.length*100),"%"]})]})]},t.id)})})]})]}),e.jsxs("div",{className:"lc-status-bar",children:[e.jsxs("span",{className:"lc-status-item",children:["XP ",m]}),e.jsxs("span",{className:"lc-status-item",children:["Level ",Math.floor(m/500)+1]}),e.jsxs("span",{className:"lc-status-item",children:[N,"/",u.length," courses"]}),e.jsxs("span",{className:"lc-status-item",children:[Object.values(p).filter(t=>t==="pass").length,"/",f.length," challenges"]}),e.jsxs("span",{className:"lc-status-item",children:[k,"% progress"]}),e.jsx("span",{className:"lc-status-item lc-status-right",style:{color:U?"#22c55e":"#f59e0b"},children:U?"backend synced":"localStorage (offline)"})]})]})}export{Oe as default};
