import { useState, useCallback, useMemo, useEffect, useRef } from "react";
import { Check, X, Circle, BookOpen, Code, Braces, Library, BarChart } from "lucide-react";
import RequiresLlm from "../components/RequiresLlm";
import "./learning-center.css";
import {
  getUserProfile,
  getLearningPaths,
  startTeachMode,
  teachModeRespond,
  completeLearningStep,
  learningSaveProgress,
  learningGetProgress,
  learningExecuteChallenge,
} from "../api/backend";

/* ─── types ─── */
type View = "courses" | "challenges" | "knowledge" | "progress" | "build";

interface SkillLevel {
  name: string;
  level: number;
  maxLevel: number;
}

interface TeachModeMessage {
  role: "system" | "user";
  content: string;
}

interface BuildProject {
  id: string;
  title: string;
  description: string;
  difficulty: Difficulty;
  category: string;
  prompts: string[];
  xp: number;
}

type Difficulty = "beginner" | "intermediate" | "advanced";

interface CourseStep {
  title: string;
  instruction: string;
  code?: string;
  tip?: string;
}

interface Course {
  id: string;
  title: string;
  description: string;
  difficulty: Difficulty;
  category: string;
  steps: CourseStep[];
  xp: number;
  thumbnail: string;
  tags: string[];
}

interface Challenge {
  id: string;
  title: string;
  description: string;
  difficulty: Difficulty;
  starterCode: string;
  expectedOutput: string;
  hints: string[];
  xp: number;
  category: string;
}

interface KnowledgeEntry {
  id: string;
  title: string;
  content: string;
  category: string;
  tags: string[];
}

/* ─── constants ─── */
const DIFF_COLORS: Record<Difficulty, string> = { beginner: "#22c55e", intermediate: "#f59e0b", advanced: "#ef4444" };
const CATEGORIES = ["All", "Getting Started", "RAG", "Models", "Security", "Time Machine"];

const STORAGE_KEY = "nexus-learning-progress";

function loadProgress(): Record<string, number> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveProgress(progress: Record<string, number>) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(progress));
  // Also persist to backend for cross-session durability
  learningSaveProgress(JSON.stringify(progress)).catch((e) => { if (import.meta.env.DEV) console.warn("[LearningCenter]", e); });
}

async function loadBackendProgress(): Promise<Record<string, number> | null> {
  try {
    const raw = await learningGetProgress();
    const data = typeof raw === "string" ? JSON.parse(raw) : raw;
    return data && typeof data === "object" ? data as Record<string, number> : null;
  } catch {
    return null;
  }
}

/* ─── backend-first helpers ─── */
async function fetchSkillLevels(): Promise<SkillLevel[]> {
  try {
    const raw = await getUserProfile();
    const profile = typeof raw === "string" ? JSON.parse(raw) : raw;
    const skills = profile?.skills ?? profile?.skill_levels ?? {};
    return Object.entries(skills).map(([name, val]) => ({
      name,
      level: typeof val === "number" ? val : (val as { level?: number })?.level ?? 0,
      maxLevel: 5,
    }));
  } catch {
    return [];
  }
}

async function fetchBackendPaths(): Promise<Record<string, number> | null> {
  try {
    const raw = await getLearningPaths();
    const data = typeof raw === "string" ? JSON.parse(raw) : raw;
    const paths: Record<string, number> = {};
    if (data && typeof data === "object") {
      const items = Array.isArray(data) ? data : (data as { paths?: unknown[] }).paths ?? [];
      for (const item of items as Array<{ id?: string; path_id?: string; step?: number; completed_steps?: number }>) {
        const id = item.id ?? item.path_id ?? "";
        if (id) paths[id] = item.step ?? item.completed_steps ?? 0;
      }
    }
    return Object.keys(paths).length > 0 ? paths : null;
  } catch {
    return null;
  }
}

async function syncStepToBackend(pathId: string, stepId: string): Promise<void> {
  try {
    await completeLearningStep(pathId, stepId);
  } catch {
    // offline — localStorage fallback already persisted
  }
}

/* ─── LEARN BY BUILDING PROJECTS ─── */
const BUILD_PROJECTS: BuildProject[] = [
  {
    id: "build-agent",
    title: "Build a File-Reader Agent",
    description: "Create an agent from scratch that reads files with proper governance. Follow the teach-mode prompts to build it step by step.",
    difficulty: "beginner",
    category: "Getting Started",
    prompts: [
      "Create a TOML manifest for a file-reader agent with fs.read capability, L1 autonomy, and 1000 fuel budget.",
      "Register the agent with the kernel and verify it appears in the agent list.",
      "Start the agent and ask it to read a file. Observe the audit trail entry.",
      "Try asking it to write a file and confirm the capability denial is logged.",
    ],
    xp: 300,
  },
  {
    id: "build-rag",
    title: "Build a RAG Pipeline",
    description: "Set up a complete retrieval-augmented generation pipeline: index documents, search, and chat with them.",
    difficulty: "intermediate",
    category: "RAG",
    prompts: [
      "Index a markdown document into the RAG pipeline using the Chat Hub.",
      "Search the indexed documents with a semantic query and review the ranked results.",
      "Enable document context mode and ask a question grounded in your indexed data.",
      "Verify the sources section shows which document chunks were used.",
    ],
    xp: 400,
  },
  {
    id: "build-security",
    title: "Lock Down an Agent",
    description: "Take an over-permissioned agent and reduce it to minimum viable capabilities using the Permission Dashboard.",
    difficulty: "intermediate",
    category: "Security",
    prompts: [
      "Open the Permission Dashboard and identify an agent with more capabilities than it needs.",
      "Remove unnecessary capabilities (e.g., shell.exec from a read-only agent).",
      "Set the HITL tier to Tier2 for any write operations.",
      "Run the agent and confirm denied actions appear in the audit trail.",
    ],
    xp: 400,
  },
  {
    id: "build-pipeline",
    title: "Create a Multi-Agent Pipeline",
    description: "Wire together multiple agents into a governed pipeline where output from one agent feeds into the next.",
    difficulty: "advanced",
    category: "Security",
    prompts: [
      "Create two agents: a researcher (llm.query + web.search) and a writer (llm.query + fs.write).",
      "Configure the researcher at L2 autonomy and the writer at L1.",
      "Set up delegation so the researcher can pass context to the writer.",
      "Run the pipeline end-to-end and verify the audit trail shows the complete chain of actions.",
    ],
    xp: 600,
  },
];

/* ─── REAL COURSES ─── */
const COURSES: Course[] = [
  {
    id: "getting-started",
    title: "Getting Started with Nexus OS",
    description: "Create your first governed agent, assign capabilities, configure fuel budgets, and run it. The foundational course for all Nexus OS users.",
    difficulty: "beginner",
    category: "Getting Started",
    xp: 500,
    thumbnail: "linear-gradient(135deg, #0f172a 0%, var(--nexus-accent) 100%)",
    tags: ["agents", "basics", "governance"],
    steps: [
      {
        title: "Understanding the Architecture",
        instruction: "Nexus OS is built on a kernel-agent model. The kernel enforces governance (capabilities, fuel, audit), while agents perform actions. Every agent declares its capabilities in a TOML manifest — the kernel checks these before allowing any action.\n\nKey invariants:\n• Every action goes through kernel capability checks\n• Fuel is checked BEFORE execution, never after\n• Audit trail is append-only with hash-chain integrity\n• PII is redacted at the LLM gateway boundary",
        tip: "Think of the kernel as a strict firewall — agents can only do what their manifest permits.",
      },
      {
        title: "Create Your First Agent Manifest",
        instruction: "Agents are defined by TOML manifests. Create a file called `my-agent.toml` in the `config/agents/` directory with the following structure. The `capabilities` list controls what the agent can do — start with minimal permissions.",
        code: `[agent]
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
memory_limit_mb = 64`,
        tip: "L1 means the agent suggests actions but YOU decide. Start here before granting higher autonomy.",
      },
      {
        title: "Register the Agent",
        instruction: "Navigate to the Agent Browser in the sidebar. Click '+ Create Agent' and paste your manifest TOML. The kernel will validate your manifest — checking that all requested capabilities are recognized and governance fields are valid.\n\nIf validation passes, your agent appears in the agent list with status 'Created'. You'll see its fuel budget and declared capabilities on the agent card.",
        tip: "Watch the audit log — you'll see a 'StateChange' event recording agent creation.",
      },
      {
        title: "Start and Interact",
        instruction: "Click 'Start' on your agent card. The kernel allocates fuel from the budget and moves the agent to 'Running' state. Now try asking it to read a file — it will succeed because `fs.read` is in its capabilities.\n\nTry asking it to write a file — it will be DENIED because `fs.write` is not in the manifest. This is governance in action.",
        tip: "Every denied action also gets logged to the audit trail — nothing is silent.",
      },
      {
        title: "Review the Audit Trail",
        instruction: "Open System Monitor → Audit Logs. You'll see a complete record of every action your agent attempted, including:\n\n• Successful reads with fuel cost\n• Denied write attempts with reason\n• State changes (created → running)\n• Fuel debits for each operation\n\nThe audit trail uses a hash-chain: each entry includes the hash of the previous entry, making tampering detectable.",
      },
    ],
  },
  {
    id: "rag-basics",
    title: "RAG Basics — Chat With Your Documents",
    description: "Index documents into the RAG pipeline, search them semantically, and ask questions with retrieval-augmented generation.",
    difficulty: "beginner",
    category: "RAG",
    xp: 600,
    thumbnail: "linear-gradient(135deg, #0f172a 0%, #a78bfa 100%)",
    tags: ["rag", "documents", "search"],
    steps: [
      {
        title: "Understanding the RAG Pipeline",
        instruction: "RAG (Retrieval-Augmented Generation) lets you chat with your documents. The pipeline has three stages:\n\n1. **Index**: Documents are chunked, embedded, and stored in a vector index\n2. **Retrieve**: Your query is embedded and matched against document chunks\n3. **Generate**: The LLM answers using the retrieved context\n\nNexus OS supports PDF, Markdown, plain text, and code files. All operations are governed — the indexing agent needs `fs.read` capability.",
      },
      {
        title: "Index Your First Document",
        instruction: "Open AI Chat Hub and click the 'RAG' tab. Use the 'Index Document' button to select a file. The backend calls `index_document` which:\n\n1. Reads the file with PII redaction\n2. Splits it into chunks (default ~500 tokens)\n3. Generates embeddings via the configured LLM provider\n4. Stores chunks in the vector index\n\nYou can also index from the terminal:\n",
        code: `// From the AI Chat Hub RAG tab:
// 1. Click "Index Document"
// 2. Select a file (PDF, MD, TXT, or code)
// 3. Watch the progress bar as chunks are indexed

// Or use the API directly:
import { indexDocument } from "../api/backend";
const result = await indexDocument("/path/to/document.md");`,
        tip: "Start with a README or documentation file — they produce the best RAG results.",
      },
      {
        title: "Search Your Documents",
        instruction: "Once indexed, use the search bar to find relevant chunks. Nexus OS uses semantic search — you don't need exact keywords. Try natural language queries like 'how does authentication work?' instead of 'auth login function'.\n\nThe search returns ranked results with similarity scores. Each result shows the source file, chunk position, and a relevance percentage.",
        code: `// Search returns ranked chunks:
import { searchDocuments } from "../api/backend";
const results = await searchDocuments("how does fuel work?", 5);
// Returns top 5 matching chunks with similarity scores`,
      },
      {
        title: "Chat With Documents",
        instruction: "The real power is conversational RAG. In the Chat Hub, enable 'Document Context' mode. Now when you ask a question, the system:\n\n1. Searches your indexed documents for relevant context\n2. Injects the top chunks into the LLM prompt\n3. The LLM answers grounded in YOUR data\n\nThis prevents hallucination — the model cites your actual documents.",
        tip: "Check the 'Sources' section below each answer to verify which documents were used.",
      },
      {
        title: "Manage Your Index",
        instruction: "Use the RAG management panel to:\n\n• **List indexed documents** — see all files in your index\n• **Remove documents** — delete a file from the index\n• **View governance** — see who accessed which documents and when\n• **Semantic map** — visualize document relationships\n\nAll document operations are audit-logged, so you always know who accessed what.",
      },
    ],
  },
  {
    id: "model-hub",
    title: "Model Hub — Download and Manage Models",
    description: "Search for compatible models, check hardware requirements, download them locally, and configure agents to use specific models.",
    difficulty: "intermediate",
    category: "Models",
    xp: 700,
    thumbnail: "linear-gradient(135deg, #0f172a 0%, #f59e0b 100%)",
    tags: ["models", "ollama", "hardware"],
    steps: [
      {
        title: "Check Your Hardware",
        instruction: "Before downloading models, check your system capabilities. Open Settings → Hardware or run the hardware profile check. Nexus OS detects:\n\n• **CPU**: Cores, architecture, speed\n• **RAM**: Total and available memory\n• **GPU**: VRAM, CUDA/Metal support\n• **Disk**: Available storage\n\nThis determines which models can run locally. A 7B parameter model needs ~4GB VRAM; a 70B model needs ~40GB.",
        tip: "If you have <8GB VRAM, stick with 7B models or use quantized (Q4/Q5) variants.",
      },
      {
        title: "Search for Models",
        instruction: "Open Settings → Model Hub. The search queries available model registries and returns models with metadata:\n\n• Name, size, and parameter count\n• Quantization level (Q4, Q5, Q8, F16)\n• Required VRAM and disk space\n• Compatibility status with your hardware\n\nFilter by size to find models that fit your system. Green indicators mean compatible, red means too large.",
        code: `// Backend API for model search:
import { searchModels, checkModelCompatibility } from "../api/backend";

// Search by name or capability
const models = await searchModels("codellama", 10);

// Check if a specific model fits your hardware
const compat = await checkModelCompatibility(4_000_000_000);`,
      },
      {
        title: "Download a Model",
        instruction: "Click 'Download' on a compatible model. The system:\n\n1. Verifies disk space and VRAM requirements\n2. Downloads the model file (progress shown in real-time)\n3. Registers it with the local model registry\n4. Makes it available for agent configuration\n\nDownloads are resumable — if interrupted, they continue from where they left off.",
        tip: "Start with a small model like 'phi-3-mini' (3.8B) for testing before downloading larger ones.",
      },
      {
        title: "Configure Agents to Use Models",
        instruction: "Each agent can be configured to use a specific LLM provider and model. Open Settings → LLM Providers and:\n\n1. Select an agent from the list\n2. Choose a provider (Ollama for local, or cloud providers)\n3. Select the model from your downloaded models\n4. Set token and dollar budgets\n\nThe `local_only` flag ensures the agent never sends data to cloud providers — important for sensitive workloads.",
      },
      {
        title: "Monitor Model Usage",
        instruction: "The Settings → Usage Stats panel shows:\n\n• Tokens consumed per agent per model\n• Cost tracking (local = free, cloud = metered)\n• Latency metrics per provider\n• Error rates and retry counts\n\nUse this to optimize which models each agent uses. If an agent's task is simple, a smaller model saves resources.",
      },
    ],
  },
  {
    id: "security",
    title: "Security — Capabilities, HITL, and Fuel",
    description: "Deep dive into the Nexus OS security model: capability-based permissions, human-in-the-loop approval tiers, fuel budgets, and audit integrity.",
    difficulty: "intermediate",
    category: "Security",
    xp: 800,
    thumbnail: "linear-gradient(135deg, #0f172a 0%, #22c55e 100%)",
    tags: ["security", "capabilities", "hitl", "fuel"],
    steps: [
      {
        title: "Capability-Based Security",
        instruction: "Every action an agent takes requires a capability. Capabilities are declared in the agent manifest and checked by the kernel before execution.\n\nValid capabilities: `llm.query`, `web.search`, `social.post`, `messaging.send`, `fs.read`, `fs.write`, `screen.capture`, `input.keyboard`, `shell.exec`, `audit.read`\n\nThe principle: agents get MINIMUM privileges needed. A research agent needs `llm.query` and `web.search` but NOT `shell.exec`.",
        code: `# Agent manifest — minimal permissions:
[capabilities]
allowed = ["llm.query", "web.search"]

# Kernel check (Rust):
# supervisor.check_capability(agent_id, "fs.write")?;
# → Returns Err(CapabilityDenied) if not in manifest`,
        tip: "Use the Permission Dashboard (Settings → Permissions) to visualize which agents have which capabilities.",
      },
      {
        title: "Human-in-the-Loop (HITL) Tiers",
        instruction: "Not all actions are equal. Nexus OS classifies actions into HITL tiers:\n\n• **Tier 0**: Auto-approved (reading audit logs, status checks)\n• **Tier 1**: Notification required (file reads, LLM queries)\n• **Tier 2**: Human approval required (file writes, deploys, social posts)\n• **Tier 3**: Board approval required (agent creation, capability changes)\n\nTier 2+ actions trigger an approval dialog. The speculative engine shows you what WOULD happen before you approve.",
      },
      {
        title: "Fuel Budgets",
        instruction: "Fuel prevents runaway agents. Every action costs fuel, checked BEFORE execution:\n\n1. Agent requests action (cost: N fuel)\n2. Kernel checks: `remaining_fuel >= N`?\n3. If yes: debit N, execute, log\n4. If no: deny action, log denial\n\nFuel budgets are set per-agent in the manifest. When an agent runs out, it gracefully stops — no crashes, no unbounded resource consumption.",
        code: `# Set fuel budget in manifest:
[governance]
fuel_budget = 5000  # total fuel units

# Typical costs:
# LLM query: 10-50 fuel (depends on token count)
# File read: 5 fuel
# File write: 15 fuel
# Web search: 20 fuel
# Shell exec: 50 fuel`,
      },
      {
        title: "Audit Trail Integrity",
        instruction: "Every action — approved or denied — is logged to the audit trail. The trail uses a hash-chain:\n\n• Each event has a hash that includes the previous event's hash\n• This creates a tamper-evident chain\n• If any event is modified, all subsequent hashes break\n• Ed25519 signatures provide non-repudiation\n\nTo verify: open System Monitor → Audit and check the chain integrity indicator.",
      },
      {
        title: "Autonomy Levels",
        instruction: "Agents operate at autonomy levels L0-L5:\n\n• **L0 Inert**: Agent does nothing\n• **L1 Suggest**: Agent suggests, human decides\n• **L2 Act-with-approval**: Human approves each action\n• **L3 Act-then-report**: Agent acts freely, reports after\n• **L4 Autonomous-bounded**: Full autonomy within limits\n• **L5 Full autonomy**: Only kernel override stops it\n\nMost agents start at L1-L2. Promotion requires demonstrated track record. The kernel can ALWAYS override any agent, regardless of autonomy level.",
        tip: "Never set an untested agent to L3+. Start at L1, observe behavior, then promote gradually.",
      },
    ],
  },
  {
    id: "time-machine",
    title: "Time Machine — Checkpoints and Undo",
    description: "Create system checkpoints, undo actions, redo them, and view diffs between states. Your safety net for agent operations.",
    difficulty: "beginner",
    category: "Time Machine",
    xp: 400,
    thumbnail: "linear-gradient(135deg, #0f172a 0%, #ec4899 100%)",
    tags: ["time-machine", "undo", "checkpoints"],
    steps: [
      {
        title: "What is the Time Machine?",
        instruction: "The Time Machine captures snapshots of system state — agent configurations, fuel levels, audit entries — so you can undo any change.\n\nThink of it as version control for your entire Nexus OS instance. Before any risky operation, create a checkpoint. If something goes wrong, restore to that point instantly.",
      },
      {
        title: "Create a Checkpoint",
        instruction: "Navigate to System Monitor → Time Machine. Click 'Create Checkpoint' and give it a label (e.g., 'before-deploy' or 'pre-upgrade').\n\nThe checkpoint captures:\n• All agent states and fuel levels\n• Configuration values\n• Permission settings\n• A snapshot of the audit trail position",
        code: `// API usage:
import { timeMachineCreateCheckpoint } from "../api/backend";
const checkpoint = await timeMachineCreateCheckpoint("before-deploy");
// Returns checkpoint ID for later reference`,
        tip: "Always create a checkpoint before running a full pipeline deploy or changing agent permissions.",
      },
      {
        title: "Undo and Redo",
        instruction: "The Time Machine supports linear undo/redo:\n\n• **Undo**: Reverts the last state change\n• **Redo**: Re-applies the last undone change\n• **Undo to checkpoint**: Reverts ALL changes back to a specific checkpoint\n\nUndo/redo works on state changes only — audit trail entries are never deleted (append-only invariant).",
        code: `// Quick undo/redo:
import { timeMachineUndo, timeMachineRedo } from "../api/backend";
await timeMachineUndo();   // revert last change
await timeMachineRedo();   // re-apply last undo`,
      },
      {
        title: "View Diffs",
        instruction: "Before restoring a checkpoint, view the diff to see exactly what will change. The diff shows:\n\n• Added/removed agents\n• Changed fuel levels\n• Modified permissions\n• Configuration changes\n\nThis lets you make an informed decision before reverting.",
        code: `import { timeMachineGetDiff } from "../api/backend";
const diff = await timeMachineGetDiff(checkpointId);
// Shows what changed between checkpoint and current state`,
      },
      {
        title: "Best Practices",
        instruction: "Tips for effective Time Machine usage:\n\n1. **Name checkpoints descriptively** — 'before-deploy-v5.0.2' not 'checkpoint-1'\n2. **Checkpoint before risky ops** — deploys, permission changes, agent promotions\n3. **Review diffs before restoring** — don't blindly undo\n4. **Checkpoints are lightweight** — create them freely, they're cheap\n5. **Audit trail is immutable** — even undo/redo creates new audit entries tracking the revert",
      },
    ],
  },
];

/* ─── CHALLENGES ─── */
const CHALLENGES: Challenge[] = [
  {
    id: "ch-1", title: "Implement Fuel Check", difficulty: "beginner", category: "Security",
    description: "Write a function that checks if an agent has enough fuel before executing an action. Return an error if fuel is insufficient.",
    starterCode: `fn check_fuel(available: u64, required: u64) -> Result<(), String> {\n    // Your code here\n    todo!()\n}`,
    expectedOutput: "Ok(()) when available >= required\nErr(\"Insufficient fuel\") when available < required",
    hints: ["Compare available against required", "Use if/else to return the right Result variant"],
    xp: 50,
  },
  {
    id: "ch-2", title: "Capability Gate", difficulty: "intermediate", category: "Security",
    description: "Implement a capability check that verifies an agent has the required capability before allowing an action.",
    starterCode: `fn has_capability(\n    agent_caps: &[&str],\n    required: &str\n) -> bool {\n    // Support exact match and \"*\" wildcard\n    todo!()\n}`,
    expectedOutput: "true when agent has exact cap or \"*\"\nfalse otherwise",
    hints: ["Check for \"*\" in agent_caps first", "Then check for exact match with .contains()"],
    xp: 100,
  },
  {
    id: "ch-3", title: "Hash Chain Audit", difficulty: "advanced", category: "Security",
    description: "Build a simple hash-chain audit trail. Each event's hash must include the previous event's hash.",
    starterCode: `use std::collections::hash_map::DefaultHasher;\nuse std::hash::{Hash, Hasher};\n\nstruct AuditEvent {\n    data: String,\n    prev_hash: u64,\n    hash: u64,\n}\n\nfn append_event(chain: &mut Vec<AuditEvent>, data: String) {\n    todo!()\n}`,
    expectedOutput: "Each event.prev_hash == previous event.hash\nFirst event.prev_hash == 0",
    hints: ["Get last event hash (or 0)", "Hash both prev_hash and data together", "Push the new event"],
    xp: 200,
  },
  {
    id: "ch-4", title: "PII Redactor", difficulty: "intermediate", category: "Security",
    description: "Write a PII redaction function that replaces email addresses and phone numbers with [REDACTED].",
    starterCode: `fn redact_pii(input: &str) -> String {\n    // Redact emails: user@domain.com -> [REDACTED]\n    // Redact phones: +1-234-567-8901 -> [REDACTED]\n    todo!()\n}`,
    expectedOutput: "\"Contact [REDACTED] or call [REDACTED]\"",
    hints: ["Use regex or manual pattern matching", "Emails contain @ with domain"],
    xp: 100,
  },
  {
    id: "ch-5", title: "HITL Approval Flow", difficulty: "beginner", category: "Security",
    description: "Implement a HITL check. Tier0 auto-approves, Tier1+ requires human approval.",
    starterCode: `enum HitlTier { Tier0, Tier1, Tier2, Tier3 }\n\nfn needs_approval(tier: HitlTier) -> bool {\n    todo!()\n}`,
    expectedOutput: "false for Tier0\ntrue for Tier1, Tier2, Tier3",
    hints: ["Match on the tier enum", "Only Tier0 is auto-approved"],
    xp: 50,
  },
];

/* ─── KNOWLEDGE BASE ─── */
const KNOWLEDGE: KnowledgeEntry[] = [
  {
    id: "k-1", title: "Why `unsafe` is Forbidden",
    content: "Nexus OS enforces `#![forbid(unsafe_code)]` across all crates. If an agent could trigger undefined behavior, it could bypass capability checks, corrupt the audit trail, or escalate privileges. Safety is not just a language feature — it's a governance invariant.",
    category: "Security", tags: ["rust", "safety"],
  },
  {
    id: "k-2", title: "Fuel Pre-Check Pattern",
    content: "Every agent action costs fuel. The kernel checks fuel BEFORE execution:\n\n1. check_fuel(cost) — verify budget\n2. debit_fuel(cost) — subtract\n3. perform_action() — execute\n4. audit_log(action) — record\n\nThis ensures agents cannot consume resources beyond their budget.",
    category: "Security", tags: ["fuel", "governance"],
  },
  {
    id: "k-3", title: "Autonomy Levels L0-L5",
    content: "L0 Inert → L1 Suggest → L2 Act-with-approval → L3 Act-then-report → L4 Autonomous-bounded → L5 Full autonomy. Most agents start at L1-L2. Promotion requires demonstrated track record. The kernel can always override.",
    category: "Getting Started", tags: ["autonomy", "governance"],
  },
  {
    id: "k-4", title: "Speculative Execution Engine",
    content: "Before a Tier2+ action is approved, the kernel runs it speculatively in a shadow context. This 'what-if' simulation shows exactly what would happen without committing. If approved: commit. If denied: discard. Eliminates surprises.",
    category: "Security", tags: ["speculative", "hitl"],
  },
  {
    id: "k-5", title: "RAG Pipeline Architecture",
    content: "Documents → Chunker → Embedder → Vector Store → Retriever → LLM. Supports PDF, MD, TXT, code. All operations are governed — indexing agents need fs.read capability. PII is redacted before embedding.",
    category: "RAG", tags: ["rag", "documents"],
  },
];

/* ─── component ─── */
export default function LearningCenter() {
  const [view, setView] = useState<View>("courses");
  const [selectedCourse, setSelectedCourse] = useState<string | null>(null);
  const [selectedChallenge, setSelectedChallenge] = useState<string | null>(null);
  const [challengeCode, setChallengeCode] = useState("");
  const [challengeResults, setChallengeResults] = useState<Record<string, { status: "pass" | "fail"; issues?: string[]; feedback?: string }>>({});
  const [showHint, setShowHint] = useState(-1);
  const [filterCategory, setFilterCategory] = useState("All");
  const [progress, setProgress] = useState<Record<string, number>>(loadProgress);
  const [currentStep, setCurrentStep] = useState(0);

  /* backend state */
  const [skillLevels, setSkillLevels] = useState<SkillLevel[]>([]);
  const [backendOnline, setBackendOnline] = useState(false);
  const [selectedBuild, setSelectedBuild] = useState<string | null>(null);
  const [teachMessages, setTeachMessages] = useState<TeachModeMessage[]>([]);
  const [teachInput, setTeachInput] = useState("");
  const [teachLoading, setTeachLoading] = useState(false);
  const [buildStep, setBuildStep] = useState(0);
  const teachEndRef = useRef<HTMLDivElement>(null);

  /* ─── on-mount: fetch backend data ─── */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      // 1. Skill levels from backend
      const skills = await fetchSkillLevels();
      if (!cancelled && skills.length > 0) {
        setSkillLevels(skills);
        setBackendOnline(true);
      }

      // 2. Learning paths from backend (merge with localStorage)
      const backendProgress = await fetchBackendPaths();
      const savedProgress = await loadBackendProgress();
      const combined = { ...backendProgress, ...savedProgress };
      if (!cancelled && Object.keys(combined).length > 0) {
        setBackendOnline(true);
        setProgress(prev => {
          const merged = { ...prev };
          for (const [id, step] of Object.entries(combined)) {
            if (step != null) merged[id] = Math.max(merged[id] ?? 0, step);
          }
          saveProgress(merged);
          return merged;
        });
      }
    })();
    return () => { cancelled = true; };
  }, []);

  // Persist progress to localStorage (always)
  useEffect(() => { saveProgress(progress); }, [progress]);

  const activeCourse = useMemo(() => COURSES.find(c => c.id === selectedCourse) ?? null, [selectedCourse]);
  const activeChallenge = useMemo(() => CHALLENGES.find(c => c.id === selectedChallenge) ?? null, [selectedChallenge]);

  const completedCourses = useMemo(
    () => COURSES.filter(c => (progress[c.id] ?? 0) >= c.steps.length).length,
    [progress],
  );

  const totalXp = useMemo(() => {
    let xp = 0;
    for (const c of COURSES) {
      if ((progress[c.id] ?? 0) >= c.steps.length) xp += c.xp;
    }
    for (const ch of CHALLENGES) {
      if (challengeResults[ch.id]?.status === "pass") xp += ch.xp;
    }
    return xp;
  }, [progress, challengeResults]);

  const overallProgress = useMemo(() => {
    const totalSteps = COURSES.reduce((sum, c) => sum + c.steps.length, 0);
    const completed = COURSES.reduce((sum, c) => sum + Math.min(progress[c.id] ?? 0, c.steps.length), 0);
    return totalSteps > 0 ? Math.round((completed / totalSteps) * 100) : 0;
  }, [progress]);

  const filteredCourses = useMemo(() => {
    if (filterCategory === "All") return COURSES;
    return COURSES.filter(c => c.category === filterCategory);
  }, [filterCategory]);

  /* ─── actions ─── */
  const completeStep = useCallback((courseId: string, stepIndex: number) => {
    setProgress(prev => {
      const current = prev[courseId] ?? 0;
      if (stepIndex >= current) {
        return { ...prev, [courseId]: stepIndex + 1 };
      }
      return prev;
    });
    setCurrentStep(stepIndex + 1);
    // sync to backend (fire-and-forget, localStorage is already saved)
    syncStepToBackend(courseId, String(stepIndex));
  }, []);

  /* ─── teach mode (Learn by Building) ─── */
  const startBuildProject = useCallback(async (projectId: string) => {
    setSelectedBuild(projectId);
    setBuildStep(0);
    setTeachMessages([]);
    setTeachInput("");
    try {
      const response = await startTeachMode(projectId);
      const parsed = typeof response === "string" ? response : JSON.stringify(response);
      setTeachMessages([{ role: "system", content: parsed || "Teach mode started. Follow the prompts below to build step by step." }]);
    } catch {
      setTeachMessages([{ role: "system", content: "Teach mode started (offline). Follow the prompts below to build step by step." }]);
    }
  }, []);

  const sendTeachResponse = useCallback(async (input: string) => {
    if (!selectedBuild || !input.trim()) return;
    const userMsg: TeachModeMessage = { role: "user", content: input.trim() };
    setTeachMessages(prev => [...prev, userMsg]);
    setTeachInput("");
    setTeachLoading(true);
    try {
      const response = await teachModeRespond(selectedBuild, input.trim());
      const parsed = typeof response === "string" ? response : JSON.stringify(response);
      setTeachMessages(prev => [...prev, { role: "system", content: parsed || "Step completed. Continue to the next prompt." }]);
    } catch {
      setTeachMessages(prev => [...prev, { role: "system", content: "Response recorded (offline). Continue to the next prompt." }]);
    }
    setTeachLoading(false);
  }, [selectedBuild]);

  // auto-scroll teach mode chat
  useEffect(() => {
    teachEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [teachMessages]);

  const runChallenge = useCallback(async () => {
    if (!activeChallenge) return;
    try {
      const raw = await learningExecuteChallenge(activeChallenge.id, challengeCode, "rust");
      const result = JSON.parse(raw);
      setChallengeResults(prev => ({
        ...prev,
        [activeChallenge.id]: {
          status: result.passed ? "pass" : "fail",
          issues: result.issues ?? result.errors ?? undefined,
          feedback: result.feedback ?? (result.passed ? `Great work! You earned ${activeChallenge.xp} XP.` : undefined),
        },
      }));
    } catch (err) {
      // Backend required — do not grant pass on client side
      if (import.meta.env.DEV) console.error("Challenge execution requires the Nexus OS backend", err);
      setChallengeResults(prev => ({
        ...prev,
        [activeChallenge.id]: {
          status: "fail",
          issues: ["Could not reach the Nexus OS backend. Make sure the backend is running."],
        },
      }));
    }
  }, [activeChallenge, challengeCode]);

  const selectChallenge = useCallback((id: string) => {
    setSelectedChallenge(id);
    const ch = CHALLENGES.find(c => c.id === id);
    if (ch) setChallengeCode(ch.starterCode);
    setShowHint(-1);
  }, []);

  /* ─── render ─── */
  return (
    <div className="lc-container">
      {/* ─── Sidebar ─── */}
      <aside className="lc-sidebar">
        <div className="lc-sidebar-header">
          <h2 className="lc-sidebar-title">Learning Center</h2>
          <div className="lc-xp-badge">XP {totalXp}</div>
        </div>

        <div className="lc-views">
          {([["courses", "courses", "Courses"], ["challenges", "challenges", "Challenges"], ["build", "build", "Build"], ["knowledge", "knowledge", "Knowledge"], ["progress", "progress", "Progress"]] as [View, string, string][]).map(([id, icon, label]) => (
            <button type="button" key={id} className={`lc-view-btn cursor-pointer ${view === id ? "active" : ""}`} onClick={() => { setView(id); setSelectedCourse(null); setSelectedChallenge(null); setSelectedBuild(null); }}>
              <span>{icon === "courses" ? <BookOpen size={14} aria-hidden="true" /> : icon === "challenges" ? <Code size={14} aria-hidden="true" /> : icon === "build" ? <Braces size={14} aria-hidden="true" /> : icon === "knowledge" ? <Library size={14} aria-hidden="true" /> : <BarChart size={14} aria-hidden="true" />}</span> {label}
            </button>
          ))}
        </div>

        {/* progress card */}
        <div className="lc-progress-card">
          <div className="lc-section-header">Your Progress</div>
          <div className="lc-progress-bar-container">
            <div className="lc-progress-bar" style={{ width: `${overallProgress}%` }} />
          </div>
          <div className="lc-progress-label">{overallProgress}% complete</div>
          <div className="lc-progress-stats">
            <div className="lc-stat"><span>{completedCourses}</span><span>Completed</span></div>
            <div className="lc-stat"><span>{COURSES.length - completedCourses}</span><span>Remaining</span></div>
            <div className="lc-stat"><span>{Object.values(challengeResults).filter(r => r.status === "pass").length}/{CHALLENGES.length}</span><span>Challenges</span></div>
          </div>
        </div>

        {/* skill levels from backend */}
        {skillLevels.length > 0 && (
          <div className="lc-progress-card">
            <div className="lc-section-header">Skill Levels</div>
            {skillLevels.map(sk => (
              <div key={sk.name} style={{ marginBottom: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", fontSize: "0.8rem", color: "#94a3b8", marginBottom: 2 }}>
                  <span style={{ textTransform: "capitalize" }}>{sk.name.replace(/_/g, " ")}</span>
                  <span>{sk.level}/{sk.maxLevel}</span>
                </div>
                <div className="lc-progress-bar-container" style={{ height: 4 }}>
                  <div className="lc-progress-bar" style={{ width: `${(sk.level / sk.maxLevel) * 100}%`, background: sk.level >= 4 ? "#22c55e" : sk.level >= 2 ? "#f59e0b" : "#64748b" }} />
                </div>
              </div>
            ))}
          </div>
        )}

        {/* filters */}
        <div className="lc-filters">
          <div className="lc-section-header">Filter</div>
          <select className="lc-filter-select" value={filterCategory} onChange={e => setFilterCategory(e.target.value)}>
            {CATEGORIES.map(c => <option key={c} value={c}>{c}</option>)}
          </select>
        </div>

        {/* audit */}
        <div className="lc-audit">
          <div className="lc-section-header">Completion</div>
          {COURSES.map(c => {
            const done = (progress[c.id] ?? 0) >= c.steps.length;
            return (
              <div key={c.id} className="lc-audit-entry" style={{ color: done ? "#22c55e" : "#94a3b8" }}>
                {done ? <Check size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> : <Circle size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} />} {c.title.slice(0, 28)}
              </div>
            );
          })}
        </div>
      </aside>

      {/* ─── Main ─── */}
      <div className="lc-main">

        {/* ═══ COURSES LIST ═══ */}
        {view === "courses" && !selectedCourse && (
          <div className="lc-courses">
            <div className="lc-view-header">
              <h3 className="lc-view-title">Courses</h3>
              <span className="lc-view-count">{filteredCourses.length} courses</span>
            </div>
            <div className="lc-course-grid">
              {filteredCourses.map(course => {
                const stepsDone = progress[course.id] ?? 0;
                const isComplete = stepsDone >= course.steps.length;
                return (
                  <div key={course.id} className="lc-course-card" onClick={() => { setSelectedCourse(course.id); setCurrentStep(Math.min(stepsDone, course.steps.length - 1)); }}>
                    <div className="lc-course-thumb" style={{ background: course.thumbnail }}>
                      <span className="lc-course-diff" style={{ color: DIFF_COLORS[course.difficulty] }}>{course.difficulty}</span>
                      {isComplete && <span style={{ position: "absolute", top: 8, right: 8, color: "#22c55e", fontWeight: 700, fontSize: "1.1rem" }}>COMPLETE</span>}
                    </div>
                    <div className="lc-course-body">
                      <div className="lc-course-title">{course.title}</div>
                      <div className="lc-course-desc">{course.description.slice(0, 100)}...</div>
                      <div className="lc-course-meta">
                        <span>{course.category}</span>
                        <span>XP {course.xp}</span>
                      </div>
                      <div className="lc-course-progress-row">
                        <div className="lc-course-progress-bar">
                          <div className="lc-course-progress-fill" style={{ width: `${(Math.min(stepsDone, course.steps.length) / course.steps.length) * 100}%` }} />
                        </div>
                        <span className="lc-course-progress-text">{Math.min(stepsDone, course.steps.length)}/{course.steps.length}</span>
                      </div>
                      <div className="lc-course-tags">
                        {course.tags.map(t => <span key={t} className="lc-tag">{t}</span>)}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* ═══ COURSE DETAIL ═══ */}
        {view === "courses" && activeCourse && (
          <div className="lc-course-detail">
            <button type="button" className="lc-back-btn" onClick={() => setSelectedCourse(null)}>← Back to Courses</button>
            <div className="lc-cd-header">
              <div className="lc-cd-thumb" style={{ background: activeCourse.thumbnail }} />
              <div className="lc-cd-info">
                <h3 className="lc-cd-title">{activeCourse.title}</h3>
                <div className="lc-cd-desc">{activeCourse.description}</div>
                <div className="lc-cd-meta">
                  <span className="lc-cd-diff" style={{ color: DIFF_COLORS[activeCourse.difficulty] }}>{activeCourse.difficulty}</span>
                  <span>{activeCourse.category}</span>
                  <span>XP {activeCourse.xp}</span>
                  <span>{activeCourse.steps.length} steps</span>
                </div>
                <div className="lc-cd-progress">
                  <div className="lc-cd-progress-bar">
                    <div className="lc-cd-progress-fill" style={{ width: `${(Math.min(progress[activeCourse.id] ?? 0, activeCourse.steps.length) / activeCourse.steps.length) * 100}%` }} />
                  </div>
                  <span>{Math.min(progress[activeCourse.id] ?? 0, activeCourse.steps.length)}/{activeCourse.steps.length} completed</span>
                </div>
              </div>
            </div>

            {/* Step navigation */}
            <div className="lc-cd-lessons">
              <h4>Steps</h4>
              {activeCourse.steps.map((step, i) => {
                const isDone = i < (progress[activeCourse.id] ?? 0);
                const isCurrent = currentStep === i;
                return (
                  <div key={i}>
                    <div
                      className={`lc-cd-lesson ${isDone ? "completed" : isCurrent ? "current" : ""}`}
                      onClick={() => setCurrentStep(i)}
                      style={{ cursor: "pointer" }}
                    >
                      <span className="lc-cd-lesson-num">{isDone ? <Check size={14} aria-hidden="true" /> : i + 1}</span>
                      <span className="lc-cd-lesson-title">{step.title}</span>
                    </div>

                    {/* Expanded step content */}
                    {isCurrent && (
                      <div style={{ padding: "1rem 1rem 1rem 3rem", borderLeft: "2px solid var(--nexus-accent)33", marginLeft: "0.5rem", marginBottom: "0.5rem" }}>
                        <div style={{ color: "#e2e8f0", lineHeight: 1.7, whiteSpace: "pre-wrap" }}>
                          {step.instruction}
                        </div>
                        {step.code && (
                          <pre style={{
                            background: "#0f172a", padding: "1rem", borderRadius: 8, marginTop: "1rem",
                            border: "1px solid #1e293b", overflow: "auto", fontSize: "0.85rem", color: "var(--nexus-accent)",
                          }}>
                            {step.code}
                          </pre>
                        )}
                        {step.tip && (
                          <div style={{
                            marginTop: "1rem", padding: "0.75rem 1rem", borderRadius: 8,
                            background: "#f59e0b11", border: "1px solid #f59e0b33", color: "#f59e0b",
                            fontSize: "0.85rem",
                          }}>
                            Tip: {step.tip}
                          </div>
                        )}
                        {!isDone && (
                          <button type="button"
                            className="lc-start-course-btn"
                            style={{ marginTop: "1rem" }}
                            onClick={() => completeStep(activeCourse.id, i)}
                          >
                            {i === activeCourse.steps.length - 1 ? "Complete Course" : "Mark Complete & Next"}
                          </button>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* ═══ CHALLENGES VIEW ═══ */}
        {view === "challenges" && (
          <div className="lc-challenges">
            <div className="lc-view-header">
              <h3 className="lc-view-title">Code Challenges</h3>
              <span className="lc-view-count">
                {Object.values(challengeResults).filter(r => r.status === "pass").length}/{CHALLENGES.length} solved
              </span>
            </div>
            <div className="lc-challenges-grid">
              <div className="lc-challenge-list">
                {CHALLENGES.map(ch => (
                  <div key={ch.id} className={`lc-challenge-item ${selectedChallenge === ch.id ? "active" : ""}`} onClick={() => selectChallenge(ch.id)}>
                    <div className="lc-ch-status">
                      {challengeResults[ch.id]?.status === "pass" ? <span className="lc-ch-pass"><Check size={14} aria-hidden="true" /></span> :
                       challengeResults[ch.id]?.status === "fail" ? <span className="lc-ch-fail"><X size={14} aria-hidden="true" /></span> :
                       <span className="lc-ch-untried"><Circle size={14} aria-hidden="true" /></span>}
                    </div>
                    <div className="lc-ch-info">
                      <div className="lc-ch-title">{ch.title}</div>
                      <div className="lc-ch-meta">
                        <span style={{ color: DIFF_COLORS[ch.difficulty] }}>{ch.difficulty}</span>
                        <span>XP {ch.xp}</span>
                      </div>
                    </div>
                  </div>
                ))}
              </div>

              {activeChallenge ? (
                <div className="lc-challenge-editor">
                  <div className="lc-ce-header">
                    <h4>{activeChallenge.title}</h4>
                    <span className="lc-ce-diff" style={{ color: DIFF_COLORS[activeChallenge.difficulty] }}>
                      {activeChallenge.difficulty} · XP {activeChallenge.xp}
                    </span>
                  </div>
                  <div className="lc-ce-desc">{activeChallenge.description}</div>
                  <div className="lc-ce-expected">
                    <div className="lc-ce-expected-label">Expected Output:</div>
                    <pre>{activeChallenge.expectedOutput}</pre>
                  </div>
                  <div className="lc-ce-code-area">
                    <div className="lc-ce-code-header">
                      <span>Solution</span>
                      <span className="lc-ce-lang">rust</span>
                      <span style={{ marginLeft: "auto", fontSize: "0.7rem", color: "#64748b" }}>
                        {challengeCode.split("\n").length} lines
                      </span>
                    </div>
                    <div style={{ display: "flex", position: "relative" }}>
                      <div style={{
                        padding: "0.75rem 0.5rem 0.75rem 0.25rem", fontFamily: "monospace", fontSize: "0.8rem",
                        lineHeight: "1.5", color: "#475569", textAlign: "right", userSelect: "none",
                        borderRight: "1px solid #1e293b", background: "rgba(15,23,42,0.5)", minWidth: 32,
                        whiteSpace: "pre-wrap",
                      }}>
                        {challengeCode.split("\n").map((_, i) => `${i + 1}\n`).join("")}
                      </div>
                      <textarea
                        className="lc-ce-textarea"
                        style={{ flex: 1, borderRadius: "0 0 6px 0" }}
                        value={challengeCode}
                        onChange={e => setChallengeCode(e.target.value)}
                        spellCheck={false}
                      />
                    </div>
                  </div>
                  <div className="lc-ce-actions">
                    <button type="button" className="lc-ce-run" onClick={runChallenge}>Run & Check</button>
                    {activeChallenge.hints.map((_, i) => (
                      <button type="button" key={i} className="lc-ce-hint" onClick={() => setShowHint(showHint === i ? -1 : i)}>
                        Hint {i + 1}
                      </button>
                    ))}
                  </div>
                  {showHint >= 0 && showHint < activeChallenge.hints.length && (
                    <div className="lc-ce-hint-box">{activeChallenge.hints[showHint]}</div>
                  )}
                  {challengeResults[activeChallenge.id]?.status === "pass" && (
                    <div className="lc-ce-result lc-ce-result-pass">All tests passed! +{activeChallenge.xp} XP</div>
                  )}
                  {challengeResults[activeChallenge.id]?.status === "fail" && (
                    <div className="lc-ce-result lc-ce-result-fail">Tests failed — check your logic and try again.</div>
                  )}
                  {/* Challenge hints on failure */}
                  {challengeResults[activeChallenge.id]?.status === "fail" && (
                    <div style={{ marginTop: 8, padding: "8px 12px", background: "rgba(251,191,36,0.05)", border: "1px solid rgba(251,191,36,0.2)", borderRadius: 6, fontSize: "0.75rem" }}>
                      <div style={{ fontWeight: 600, marginBottom: 4, color: "#fbbf24" }}>Hints:</div>
                      {challengeResults[activeChallenge.id]?.issues?.map((issue: string, i: number) => (
                        <div key={i} style={{ opacity: 0.7, paddingLeft: 8, marginBottom: 2 }}>&#8226; {issue}</div>
                      )) || <div style={{ opacity: 0.7 }}>Review the requirements and try again.</div>}
                    </div>
                  )}
                  {/* Challenge success feedback */}
                  {challengeResults[activeChallenge.id]?.status === "pass" && (
                    <div style={{ marginTop: 8, padding: "8px 12px", background: "rgba(34,197,94,0.05)", border: "1px solid rgba(34,197,94,0.2)", borderRadius: 6, fontSize: "0.75rem" }}>
                      <div style={{ fontWeight: 600, color: "#22c55e" }}>Challenge Passed!</div>
                      <div style={{ opacity: 0.7, marginTop: 2 }}>{challengeResults[activeChallenge.id]?.feedback ?? "Well done!"}</div>
                    </div>
                  )}
                </div>
              ) : (
                <div className="lc-ce-empty">
                  <div className="lc-ce-empty-icon">&gt;_</div>
                  <div>Select a challenge to begin</div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ═══ KNOWLEDGE VIEW ═══ */}
        {view === "knowledge" && (
          <div className="lc-knowledge">
            <div className="lc-view-header">
              <h3 className="lc-view-title">Knowledge Base</h3>
              <span className="lc-view-count">{KNOWLEDGE.length} articles</span>
            </div>
            <div className="lc-kb-list">
              {KNOWLEDGE.map(entry => (
                <div key={entry.id} className="lc-kb-card">
                  <div className="lc-kb-header">
                    <div className="lc-kb-title">{entry.title}</div>
                    <div className="lc-kb-meta">
                      <span>{entry.category}</span>
                    </div>
                  </div>
                  <div className="lc-kb-content" style={{ whiteSpace: "pre-wrap" }}>{entry.content}</div>
                  <div className="lc-kb-tags">
                    {entry.tags.map(t => <span key={t} className="lc-tag">{t}</span>)}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ═══ LEARN BY BUILDING VIEW ═══ */}
        {view === "build" && !selectedBuild && (
          <div className="lc-courses">
            <div className="lc-view-header">
              <h3 className="lc-view-title">Learn by Building</h3>
              <span className="lc-view-count">{BUILD_PROJECTS.length} projects</span>
            </div>
            <p style={{ color: "#94a3b8", marginBottom: "1rem", fontSize: "0.9rem" }}>
              Hands-on projects using teach mode. The backend guides you step by step as you build real Nexus OS features.
            </p>
            <div className="lc-course-grid">
              {BUILD_PROJECTS.map(proj => (
                <div key={proj.id} className="lc-course-card" onClick={() => startBuildProject(proj.id)}>
                  <div className="lc-course-thumb" style={{ background: "linear-gradient(135deg, #0f172a 0%, #6366f1 100%)" }}>
                    <span className="lc-course-diff" style={{ color: DIFF_COLORS[proj.difficulty] }}>{proj.difficulty}</span>
                  </div>
                  <div className="lc-course-body">
                    <div className="lc-course-title">{proj.title}</div>
                    <div className="lc-course-desc">{proj.description}</div>
                    <div className="lc-course-meta">
                      <span>{proj.category}</span>
                      <span>XP {proj.xp}</span>
                      <span>{proj.prompts.length} steps</span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ═══ BUILD PROJECT DETAIL ═══ */}
        {view === "build" && selectedBuild && (() => {
          const proj = BUILD_PROJECTS.find(p => p.id === selectedBuild);
          if (!proj) return null;
          return (
            <div className="lc-course-detail">
              <button type="button" className="lc-back-btn" onClick={() => setSelectedBuild(null)}>← Back to Projects</button>
              <div className="lc-cd-header">
                <div className="lc-cd-thumb" style={{ background: "linear-gradient(135deg, #0f172a 0%, #6366f1 100%)" }} />
                <div className="lc-cd-info">
                  <h3 className="lc-cd-title">{proj.title}</h3>
                  <div className="lc-cd-desc">{proj.description}</div>
                  <div className="lc-cd-meta">
                    <span className="lc-cd-diff" style={{ color: DIFF_COLORS[proj.difficulty] }}>{proj.difficulty}</span>
                    <span>{proj.category}</span>
                    <span>XP {proj.xp}</span>
                  </div>
                </div>
              </div>

              {/* Step prompts */}
              <div style={{ margin: "1rem 0" }}>
                <h4 style={{ color: "#e2e8f0", marginBottom: "0.5rem" }}>Steps</h4>
                {proj.prompts.map((prompt, i) => (
                  <div key={i} className={`lc-cd-lesson ${i < buildStep ? "completed" : i === buildStep ? "current" : ""}`}
                    style={{ cursor: i === buildStep ? "pointer" : "default" }}
                    onClick={() => { if (i === buildStep) { setBuildStep(i + 1); sendTeachResponse(prompt); } }}
                  >
                    <span className="lc-cd-lesson-num">{i < buildStep ? <Check size={14} aria-hidden="true" /> : i + 1}</span>
                    <span className="lc-cd-lesson-title">{prompt}</span>
                  </div>
                ))}
              </div>

              {/* Teach mode chat — requires LLM */}
              <RequiresLlm feature="AI Teach Mode">
              <div style={{
                background: "#0f172a", border: "1px solid #1e293b", borderRadius: 8,
                padding: "1rem", maxHeight: 320, overflowY: "auto", marginBottom: "1rem",
              }}>
                {teachMessages.length === 0 && (
                  <div style={{ color: "#64748b", textAlign: "center", padding: "2rem 0" }}>
                    Click a step above or type below to start the teach-mode conversation.
                  </div>
                )}
                {teachMessages.map((msg, i) => (
                  <div key={i} style={{
                    marginBottom: 8, padding: "0.5rem 0.75rem", borderRadius: 6,
                    background: msg.role === "user" ? "#1e293b" : "transparent",
                    borderLeft: msg.role === "system" ? "2px solid var(--nexus-accent)" : "none",
                  }}>
                    <div style={{ fontSize: "0.7rem", color: "#64748b", marginBottom: 2 }}>
                      {msg.role === "user" ? "You" : "Nexus Teach Mode"}
                    </div>
                    <div style={{ color: "#e2e8f0", fontSize: "0.85rem", whiteSpace: "pre-wrap" }}>{msg.content}</div>
                  </div>
                ))}
                {teachLoading && (
                  <div style={{ color: "#64748b", fontSize: "0.85rem", padding: "0.25rem 0.75rem" }}>Thinking...</div>
                )}
                <div ref={teachEndRef} />
              </div>

              {/* Input */}
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  style={{
                    flex: 1, background: "#1e293b", border: "1px solid #334155", borderRadius: 6,
                    padding: "0.5rem 0.75rem", color: "#e2e8f0", fontSize: "0.85rem", outline: "none",
                  }}
                  placeholder="Type your response or describe what you built..."
                  value={teachInput}
                  onChange={e => setTeachInput(e.target.value)}
                  onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); sendTeachResponse(teachInput); } }}
                  disabled={teachLoading}
                />
                <button type="button" className="lc-start-course-btn" onClick={() => sendTeachResponse(teachInput)} disabled={teachLoading || !teachInput.trim()}>
                  Send
                </button>
              </div>
              </RequiresLlm>
            </div>
          );
        })()}

        {/* ═══ PROGRESS VIEW ═══ */}
        {view === "progress" && (
          <div className="lc-progress-view">
            <div className="lc-view-header">
              <h3 className="lc-view-title">Your Learning Progress</h3>
            </div>

            <div className="lc-pv-hero">
              <div className="lc-pv-xp">
                <div className="lc-pv-xp-number">XP {totalXp}</div>
                <div className="lc-pv-xp-label">Total XP</div>
              </div>
              <div className="lc-pv-level">
                <div className="lc-pv-level-badge">Level {Math.floor(totalXp / 500) + 1}</div>
                <div className="lc-pv-level-title">
                  {totalXp < 500 ? "Apprentice" : totalXp < 1000 ? "Practitioner" : totalXp < 2000 ? "Engineer" : totalXp < 4000 ? "Architect" : "Master"}
                </div>
                <div className="lc-pv-level-bar">
                  <div className="lc-pv-level-fill" style={{ width: `${(totalXp % 500) / 5}%` }} />
                </div>
                <div className="lc-pv-level-next">{500 - (totalXp % 500)} XP to next level</div>
              </div>
            </div>

            <div className="lc-pv-stats">
              <div className="lc-pv-stat-card">
                <div className="lc-pv-stat-value">{overallProgress}%</div>
                <div className="lc-pv-stat-label">Course Completion</div>
                <div className="lc-pv-stat-bar"><div style={{ width: `${overallProgress}%`, background: "var(--nexus-accent)", height: "100%", borderRadius: 2 }} /></div>
              </div>
              <div className="lc-pv-stat-card">
                <div className="lc-pv-stat-value">{completedCourses}/{COURSES.length}</div>
                <div className="lc-pv-stat-label">Courses Completed</div>
                <div className="lc-pv-stat-bar"><div style={{ width: `${(completedCourses / COURSES.length) * 100}%`, background: "#22c55e", height: "100%", borderRadius: 2 }} /></div>
              </div>
              <div className="lc-pv-stat-card">
                <div className="lc-pv-stat-value">{Object.values(challengeResults).filter(r => r.status === "pass").length}/{CHALLENGES.length}</div>
                <div className="lc-pv-stat-label">Challenges Solved</div>
                <div className="lc-pv-stat-bar"><div style={{ width: `${(Object.values(challengeResults).filter(r => r.status === "pass").length / CHALLENGES.length) * 100}%`, background: "#f59e0b", height: "100%", borderRadius: 2 }} /></div>
              </div>
            </div>

            {skillLevels.length > 0 && (
              <>
                <h4 className="lc-sub-title">Skill Levels</h4>
                <div className="lc-pv-stats" style={{ marginBottom: "1.5rem" }}>
                  {skillLevels.map(sk => (
                    <div key={sk.name} className="lc-pv-stat-card">
                      <div className="lc-pv-stat-value">{sk.level}/{sk.maxLevel}</div>
                      <div className="lc-pv-stat-label" style={{ textTransform: "capitalize" }}>{sk.name.replace(/_/g, " ")}</div>
                      <div className="lc-pv-stat-bar">
                        <div style={{
                          width: `${(sk.level / sk.maxLevel) * 100}%`,
                          background: sk.level >= 4 ? "#22c55e" : sk.level >= 2 ? "#f59e0b" : "#6366f1",
                          height: "100%", borderRadius: 2,
                        }} />
                      </div>
                    </div>
                  ))}
                </div>
              </>
            )}

            <h4 className="lc-sub-title">Course Progress</h4>
            <div className="lc-pv-courses">
              {COURSES.map(c => {
                const stepsDone = Math.min(progress[c.id] ?? 0, c.steps.length);
                const isComplete = stepsDone >= c.steps.length;
                return (
                  <div key={c.id} className="lc-pv-course-row">
                    <div className="lc-pv-course-info">
                      <span className="lc-pv-course-name">{c.title}</span>
                      <span className="lc-pv-course-status" style={{ color: isComplete ? "#22c55e" : stepsDone > 0 ? "#f59e0b" : "#64748b" }}>
                        {isComplete ? "Complete" : stepsDone > 0 ? "In Progress" : "Not Started"}
                      </span>
                    </div>
                    <div className="lc-pv-course-bar-row">
                      <div className="lc-pv-course-bar"><div className="lc-pv-course-fill" style={{ width: `${(stepsDone / c.steps.length) * 100}%` }} /></div>
                      <span className="lc-pv-course-pct">{Math.round((stepsDone / c.steps.length) * 100)}%</span>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>

      {/* ─── Status Bar ─── */}
      <div className="lc-status-bar">
        <span className="lc-status-item">XP {totalXp}</span>
        <span className="lc-status-item">Level {Math.floor(totalXp / 500) + 1}</span>
        <span className="lc-status-item">{completedCourses}/{COURSES.length} courses</span>
        <span className="lc-status-item">{Object.values(challengeResults).filter(r => r.status === "pass").length}/{CHALLENGES.length} challenges</span>
        <span className="lc-status-item">{overallProgress}% progress</span>
        <span className="lc-status-item lc-status-right" style={{ color: backendOnline ? "#22c55e" : "#f59e0b" }}>
          {backendOnline ? "backend synced" : "localStorage (offline)"}
        </span>
      </div>
    </div>
  );
}
