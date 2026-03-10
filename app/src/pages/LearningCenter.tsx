import { useState, useCallback, useMemo } from "react";
import "./learning-center.css";

/* ─── types ─── */
type View = "courses" | "challenges" | "knowledge" | "videos" | "progress";

type Difficulty = "beginner" | "intermediate" | "advanced";
type CourseStatus = "not-started" | "in-progress" | "completed";
type ChallengeResult = "untried" | "pass" | "fail";

interface Tutorial {
  id: string;
  title: string;
  description: string;
  category: string;
  difficulty: Difficulty;
  steps: TutorialStep[];
  duration: string;
  agent: string;
  xp: number;
  tags: string[];
}

interface TutorialStep {
  title: string;
  content: string;
  code?: string;
  hint?: string;
  completed: boolean;
}

interface Course {
  id: string;
  title: string;
  description: string;
  category: string;
  difficulty: Difficulty;
  lessons: number;
  completedLessons: number;
  status: CourseStatus;
  agent: string;
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
  result: ChallengeResult;
  xp: number;
  category: string;
  solvedBy: number;
}

interface KnowledgeEntry {
  id: string;
  title: string;
  content: string;
  category: string;
  author: string;
  agent?: string;
  upvotes: number;
  createdAt: number;
  tags: string[];
}

interface VideoTutorial {
  id: string;
  title: string;
  description: string;
  duration: string;
  agent: string;
  category: string;
  thumbnail: string;
  views: number;
  difficulty: Difficulty;
}

interface AgentLearning {
  id: string;
  agent: string;
  agentColor: string;
  title: string;
  insight: string;
  category: string;
  timestamp: number;
  confidence: number;
  applied: boolean;
}

/* ─── constants ─── */
const DIFF_COLORS: Record<Difficulty, string> = { beginner: "#22c55e", intermediate: "#f59e0b", advanced: "#ef4444" };
const CATEGORIES = ["Governance", "Agents", "Kernel", "WASM", "Deployment", "Security", "Rust", "React"];

const INITIAL_COURSES: Course[] = [
  {
    id: "c-1", title: "Nexus OS Fundamentals", description: "Master the core concepts — kernel, agents, governance, fuel budgets, and audit trails. The essential foundation.",
    category: "Governance", difficulty: "beginner", lessons: 12, completedLessons: 8, status: "in-progress",
    agent: "Self-Improve Agent", xp: 600, thumbnail: "linear-gradient(135deg, #0f172a 0%, #22d3ee 100%)", tags: ["governance", "kernel", "basics"],
  },
  {
    id: "c-2", title: "Building Governed Agents", description: "Create agents that respect capability boundaries, check fuel before action, and log everything to the audit chain.",
    category: "Agents", difficulty: "intermediate", lessons: 10, completedLessons: 10, status: "completed",
    agent: "Coder Agent", xp: 800, thumbnail: "linear-gradient(135deg, #0f172a 0%, #a78bfa 100%)", tags: ["agents", "capabilities", "fuel"],
  },
  {
    id: "c-3", title: "WASM Sandboxing Deep Dive", description: "Understand how Nexus OS uses Wasmtime for agent isolation — memory limits, capability-based security, and WASI integration.",
    category: "WASM", difficulty: "advanced", lessons: 8, completedLessons: 0, status: "not-started",
    agent: "Research Agent", xp: 1000, thumbnail: "linear-gradient(135deg, #0f172a 0%, #f59e0b 100%)", tags: ["wasm", "sandbox", "security"],
  },
  {
    id: "c-4", title: "Rust for Nexus OS", description: "Learn Rust patterns used throughout the kernel — thiserror, serde, tokio async, and the unsafe-free philosophy.",
    category: "Rust", difficulty: "intermediate", lessons: 15, completedLessons: 3, status: "in-progress",
    agent: "Self-Improve Agent", xp: 900, thumbnail: "linear-gradient(135deg, #0f172a 0%, #ef4444 100%)", tags: ["rust", "patterns", "async"],
  },
  {
    id: "c-5", title: "Deploying Nexus OS Apps", description: "From code to production — CI/CD pipelines, environment management, rollback strategies, and governed deployments.",
    category: "Deployment", difficulty: "intermediate", lessons: 7, completedLessons: 0, status: "not-started",
    agent: "DevOps Agent", xp: 500, thumbnail: "linear-gradient(135deg, #0f172a 0%, #3b82f6 100%)", tags: ["deploy", "ci-cd", "production"],
  },
  {
    id: "c-6", title: "Security Architecture", description: "Ed25519 signatures, hash-chain audit, PII redaction, HITL tiers, and capability permission model — security from the ground up.",
    category: "Security", difficulty: "advanced", lessons: 11, completedLessons: 0, status: "not-started",
    agent: "Research Agent", xp: 1200, thumbnail: "linear-gradient(135deg, #0f172a 0%, #22c55e 100%)", tags: ["security", "crypto", "permissions"],
  },
  {
    id: "c-7", title: "React UI for Governed Apps", description: "Build cyberpunk interfaces with governance badges, fuel bars, audit panels, and HITL approval dialogs.",
    category: "React", difficulty: "beginner", lessons: 9, completedLessons: 9, status: "completed",
    agent: "Designer Agent", xp: 450, thumbnail: "linear-gradient(135deg, #0f172a 0%, #ec4899 100%)", tags: ["react", "ui", "design"],
  },
];

const INITIAL_CHALLENGES: Challenge[] = [
  {
    id: "ch-1", title: "Implement Fuel Check", difficulty: "beginner", category: "Kernel",
    description: "Write a function that checks if an agent has enough fuel before executing an action. Return an error if fuel is insufficient.",
    starterCode: `fn check_fuel(available: u64, required: u64) -> Result<(), String> {\n    // Your code here\n    todo!()\n}`,
    expectedOutput: "Ok(()) when available >= required\nErr(\"Insufficient fuel\") when available < required",
    hints: ["Compare available against required", "Use if/else to return the right Result variant"],
    result: "pass", xp: 50, solvedBy: 847,
  },
  {
    id: "ch-2", title: "Capability Gate", difficulty: "intermediate", category: "Governance",
    description: "Implement a capability check that verifies an agent has the required capability before allowing an action. Support wildcard capabilities.",
    starterCode: `fn has_capability(\n    agent_caps: &[&str],\n    required: &str\n) -> bool {\n    // Your code here\n    // Support exact match and "*" wildcard\n    todo!()\n}`,
    expectedOutput: "true when agent has exact cap or \"*\"\nfalse otherwise",
    hints: ["Check for \"*\" in agent_caps first", "Then check for exact match with .contains()"],
    result: "untried", xp: 100, solvedBy: 412,
  },
  {
    id: "ch-3", title: "Hash Chain Audit", difficulty: "advanced", category: "Security",
    description: "Build a simple hash-chain audit trail. Each event's hash must include the previous event's hash, creating a tamper-evident chain.",
    starterCode: `use std::collections::hash_map::DefaultHasher;\nuse std::hash::{Hash, Hasher};\n\nstruct AuditEvent {\n    data: String,\n    prev_hash: u64,\n    hash: u64,\n}\n\nfn append_event(\n    chain: &mut Vec<AuditEvent>,\n    data: String\n) {\n    // Your code here\n    todo!()\n}`,
    expectedOutput: "Each event.prev_hash == previous event.hash\nFirst event.prev_hash == 0",
    hints: ["Get the last event's hash (or 0 if empty)", "Hash both prev_hash and data together", "Push the new event"],
    result: "untried", xp: 200, solvedBy: 156,
  },
  {
    id: "ch-4", title: "PII Redactor", difficulty: "intermediate", category: "Security",
    description: "Write a PII redaction function that replaces email addresses and phone numbers with [REDACTED] before content reaches the LLM gateway.",
    starterCode: `fn redact_pii(input: &str) -> String {\n    // Redact emails: user@domain.com -> [REDACTED]\n    // Redact phones: +1-234-567-8901 -> [REDACTED]\n    // Your code here\n    todo!()\n}`,
    expectedOutput: "\"Contact [REDACTED] or call [REDACTED]\"",
    hints: ["Use regex or manual pattern matching", "Emails contain @ with domain", "Phones start with + or are digit groups with dashes"],
    result: "fail", xp: 100, solvedBy: 389,
  },
  {
    id: "ch-5", title: "HITL Approval Flow", difficulty: "beginner", category: "Governance",
    description: "Implement a basic HITL approval check. Tier0 auto-approves, Tier1+ requires human approval.",
    starterCode: `enum HitlTier { Tier0, Tier1, Tier2, Tier3 }\n\nfn needs_approval(tier: HitlTier) -> bool {\n    // Your code here\n    todo!()\n}`,
    expectedOutput: "false for Tier0\ntrue for Tier1, Tier2, Tier3",
    hints: ["Match on the tier enum", "Only Tier0 is auto-approved"],
    result: "pass", xp: 50, solvedBy: 923,
  },
  {
    id: "ch-6", title: "Agent Manifest Parser", difficulty: "advanced", category: "Agents",
    description: "Parse a TOML agent manifest and extract name, version, and capabilities list. Validate that required fields exist.",
    starterCode: `struct Manifest {\n    name: String,\n    version: String,\n    capabilities: Vec<String>,\n}\n\nfn parse_manifest(toml: &str)\n  -> Result<Manifest, String>\n{\n    // Your code here\n    todo!()\n}`,
    expectedOutput: "Ok(Manifest { name, version, caps })\nErr(\"Missing field: ...\") on invalid input",
    hints: ["Split by lines and look for key = \"value\" patterns", "Check for [capabilities] section", "Return error if name or version missing"],
    result: "untried", xp: 200, solvedBy: 98,
  },
];

const INITIAL_KNOWLEDGE: KnowledgeEntry[] = [
  {
    id: "k-1", title: "Why `unsafe` is Forbidden in Nexus OS", category: "Rust",
    content: "Nexus OS enforces `#![forbid(unsafe_code)]` across all crates. This ensures that memory safety bugs cannot compromise the governance model. If an agent could trigger undefined behavior, it could bypass capability checks, corrupt the audit trail, or escalate privileges. The trade-off is slightly reduced performance in some hot paths, but the security guarantee is non-negotiable.\n\n**Key insight**: Safety is not just a language feature — it's a governance invariant.",
    author: "Suresh K.", upvotes: 47, createdAt: Date.now() - 2592000000, tags: ["rust", "safety", "governance"],
  },
  {
    id: "k-2", title: "How Fuel Budgets Prevent Runaway Agents", category: "Governance",
    content: "Every agent action costs fuel. The kernel checks fuel BEFORE execution, never after. This pre-check pattern ensures an agent cannot consume resources it hasn't been budgeted for.\n\n```rust\nfn execute(ctx: &Context, cost: u64) -> Result<()> {\n    ctx.check_fuel(cost)?;  // Check FIRST\n    ctx.debit_fuel(cost);   // Then debit\n    perform_action()?;      // Then act\n    ctx.audit_log(action);  // Always log\n    Ok(())\n}\n```\n\nIf an agent runs out of fuel mid-workflow, it gracefully degrades rather than crashing.",
    author: "Self-Improve Agent", agent: "Self-Improve Agent", upvotes: 62, createdAt: Date.now() - 1728000000, tags: ["fuel", "governance", "agents"],
  },
  {
    id: "k-3", title: "Ed25519 Signatures for Agent Manifests", category: "Security",
    content: "Every agent manifest is signed with Ed25519. The App Store verifies signatures before installation — if the signature is invalid, the install is blocked.\n\nThe signing flow:\n1. Developer generates Ed25519 keypair\n2. Manifest TOML is hashed (SHA-256)\n3. Hash is signed with developer's private key\n4. Signature is embedded in the manifest\n5. App Store verifies with developer's public key\n\nThis prevents supply chain attacks — a tampered manifest will fail verification.",
    author: "Research Agent", agent: "Research Agent", upvotes: 38, createdAt: Date.now() - 864000000, tags: ["security", "crypto", "app-store"],
  },
  {
    id: "k-4", title: "Autonomy Levels Explained (L0-L5)", category: "Governance",
    content: "Nexus OS defines 6 autonomy levels:\n\n- **L0 Inert**: Agent does nothing\n- **L1 Suggest**: Agent suggests, human decides\n- **L2 Act-with-approval**: Agent acts after human approves (HITL)\n- **L3 Act-then-report**: Agent acts, then reports to human\n- **L4 Autonomous-bounded**: Full autonomy within bounds, anomaly-triggered review\n- **L5 Full autonomy**: Only kernel can override\n\nMost agents start at L1-L2. Promotion to L3+ requires demonstrated track record and governance board approval.",
    author: "Suresh K.", upvotes: 91, createdAt: Date.now() - 5184000000, tags: ["autonomy", "governance", "levels"],
  },
  {
    id: "k-5", title: "The Speculative Execution Engine", category: "Kernel",
    content: "Before a Tier2+ action is approved, the kernel runs it speculatively in a shadow context. This \"what-if\" simulation shows the user exactly what would happen without actually committing the change.\n\nThe speculative engine:\n1. Clones the current state into a sandbox\n2. Executes the proposed action\n3. Diffs the resulting state against current\n4. Presents the diff to the human for HITL review\n5. If approved: commits. If denied: discards.\n\nThis eliminates \"I didn't know it would do that\" surprises.",
    author: "Self-Improve Agent", agent: "Self-Improve Agent", upvotes: 55, createdAt: Date.now() - 432000000, tags: ["speculative", "kernel", "hitl"],
  },
];

const INITIAL_VIDEOS: VideoTutorial[] = [
  { id: "v-1", title: "Getting Started with Nexus OS", description: "Your first 10 minutes — setup, configuration, and your first governed agent.", duration: "12:30", agent: "Self-Improve Agent", category: "Governance", thumbnail: "linear-gradient(135deg, #0f172a 0%, #22d3ee 50%, #0f172a 100%)", views: 3240, difficulty: "beginner" },
  { id: "v-2", title: "Building Your First Agent", description: "From manifest to deployment — create a governed agent step by step.", duration: "18:45", agent: "Coder Agent", category: "Agents", thumbnail: "linear-gradient(135deg, #0f172a 0%, #a78bfa 50%, #0f172a 100%)", views: 2180, difficulty: "beginner" },
  { id: "v-3", title: "Advanced Governance Patterns", description: "Multi-tier HITL, speculative execution, and autonomous escalation workflows.", duration: "24:10", agent: "Research Agent", category: "Governance", thumbnail: "linear-gradient(135deg, #0f172a 0%, #f59e0b 50%, #0f172a 100%)", views: 1560, difficulty: "advanced" },
  { id: "v-4", title: "WASM Agent Sandboxing", description: "Deep dive into Wasmtime integration, memory limits, and capability-based isolation.", duration: "22:00", agent: "Research Agent", category: "WASM", thumbnail: "linear-gradient(135deg, #0f172a 0%, #ef4444 50%, #0f172a 100%)", views: 890, difficulty: "advanced" },
  { id: "v-5", title: "Designing Cyberpunk UIs", description: "Create the signature Nexus OS dark theme — navy backgrounds, cyan accents, glowing elements.", duration: "15:20", agent: "Designer Agent", category: "React", thumbnail: "linear-gradient(135deg, #0f172a 0%, #ec4899 50%, #0f172a 100%)", views: 1920, difficulty: "intermediate" },
  { id: "v-6", title: "CI/CD with Governed Deploys", description: "Set up deployment pipelines with environment promotion, rollbacks, and audit trails.", duration: "20:15", agent: "DevOps Agent", category: "Deployment", thumbnail: "linear-gradient(135deg, #0f172a 0%, #3b82f6 50%, #0f172a 100%)", views: 1100, difficulty: "intermediate" },
];

const INITIAL_LEARNINGS: AgentLearning[] = [
  { id: "l-1", agent: "Self-Improve Agent", agentColor: "#f59e0b", title: "Fuel pre-check pattern is 3x more reliable", insight: "After analyzing 1,247 agent executions, pre-checking fuel before action (not after) reduced failed operations by 73%. The pattern `check → debit → act → log` should be enforced in all new agents via the SDK.", category: "Governance", timestamp: Date.now() - 86400000, confidence: 0.94, applied: true },
  { id: "l-2", agent: "Self-Improve Agent", agentColor: "#f59e0b", title: "Batch audit writes improve throughput 4x", insight: "Writing audit events individually causes I/O bottlenecks under load. Batching 10-50 events with a 100ms flush interval maintains hash-chain integrity while improving throughput from 2,400 to 9,800 events/sec.", category: "Kernel", timestamp: Date.now() - 172800000, confidence: 0.88, applied: true },
  { id: "l-3", agent: "Self-Improve Agent", agentColor: "#f59e0b", title: "HITL approval timeout should be configurable", insight: "Default 5-minute HITL timeout causes approval expiry in 18% of Tier2 requests during off-hours. Suggestion: make timeout configurable per-agent with a 30-min max and auto-escalation to the next reviewer.", category: "Governance", timestamp: Date.now() - 259200000, confidence: 0.79, applied: false },
  { id: "l-4", agent: "Coder Agent", agentColor: "#22d3ee", title: "Monaco editor loads 40% faster with lazy imports", insight: "Dynamically importing Monaco editor modules instead of bundling everything at startup reduced initial page load from 2.1s to 1.3s. Applied to Code Editor — should extend to Design Studio code view.", category: "React", timestamp: Date.now() - 432000000, confidence: 0.91, applied: true },
  { id: "l-5", agent: "Research Agent", agentColor: "#22c55e", title: "Wasmtime 15 reduces agent cold-start by 60%", insight: "Benchmarking Wasmtime 15 vs 13 shows cold-start drops from 5.2ms to 2.1ms. Module pre-compilation and caching can reduce this further to sub-millisecond. Recommend upgrade path in next sprint.", category: "WASM", timestamp: Date.now() - 604800000, confidence: 0.85, applied: false },
];

/* ─── component ─── */
export default function LearningCenter() {
  const [view, setView] = useState<View>("courses");
  const [courses, setCourses] = useState<Course[]>(INITIAL_COURSES);
  const [challenges, setChallenges] = useState<Challenge[]>(INITIAL_CHALLENGES);
  const [knowledge] = useState<KnowledgeEntry[]>(INITIAL_KNOWLEDGE);
  const [videos] = useState<VideoTutorial[]>(INITIAL_VIDEOS);
  const [learnings] = useState<AgentLearning[]>(INITIAL_LEARNINGS);
  const [selectedCourse, setSelectedCourse] = useState<string | null>(null);
  const [selectedChallenge, setSelectedChallenge] = useState<string | null>(null);
  const [challengeCode, setChallengeCode] = useState("");
  const [showHint, setShowHint] = useState(-1);
  const [filterCategory, setFilterCategory] = useState("all");
  const [filterDifficulty, setFilterDifficulty] = useState<Difficulty | "all">("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [fuelUsed, setFuelUsed] = useState(45);
  const [auditLog, setAuditLog] = useState<string[]>(["Learning Center opened", "7 courses loaded"]);
  const [xpTotal, setXpTotal] = useState(1850);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 50)), []);

  const activeCourse = useMemo(() => courses.find(c => c.id === selectedCourse), [courses, selectedCourse]);
  const activeChallenge = useMemo(() => challenges.find(c => c.id === selectedChallenge), [challenges, selectedChallenge]);

  const filteredCourses = useMemo(() => {
    return courses.filter(c => {
      if (filterCategory !== "all" && c.category !== filterCategory) return false;
      if (filterDifficulty !== "all" && c.difficulty !== filterDifficulty) return false;
      if (searchQuery && !c.title.toLowerCase().includes(searchQuery.toLowerCase())) return false;
      return true;
    });
  }, [courses, filterCategory, filterDifficulty, searchQuery]);

  const filteredChallenges = useMemo(() => {
    return challenges.filter(c => {
      if (filterCategory !== "all" && c.category !== filterCategory) return false;
      if (filterDifficulty !== "all" && c.difficulty !== filterDifficulty) return false;
      return true;
    });
  }, [challenges, filterCategory, filterDifficulty]);

  const totalXp = useMemo(() => {
    const courseXp = courses.filter(c => c.status === "completed").reduce((sum, c) => sum + c.xp, 0);
    const challengeXp = challenges.filter(c => c.result === "pass").reduce((sum, c) => sum + c.xp, 0);
    return xpTotal + courseXp + challengeXp - 1850; // base offset
  }, [courses, challenges, xpTotal]);

  const overallProgress = useMemo(() => {
    const totalLessons = courses.reduce((sum, c) => sum + c.lessons, 0);
    const completed = courses.reduce((sum, c) => sum + c.completedLessons, 0);
    return totalLessons > 0 ? Math.round((completed / totalLessons) * 100) : 0;
  }, [courses]);

  /* ─── actions ─── */
  const startCourse = useCallback((id: string) => {
    setCourses(prev => prev.map(c => c.id === id && c.status === "not-started" ? { ...c, status: "in-progress" as CourseStatus, completedLessons: 1 } : c));
    setFuelUsed(f => f + 5);
    logAudit(`Started course: ${courses.find(c => c.id === id)?.title}`);
  }, [courses, logAudit]);

  const completeLesson = useCallback((courseId: string) => {
    setCourses(prev => prev.map(c => {
      if (c.id !== courseId) return c;
      const newCompleted = Math.min(c.completedLessons + 1, c.lessons);
      const newStatus = newCompleted >= c.lessons ? "completed" as CourseStatus : c.status;
      if (newStatus === "completed") {
        setXpTotal(x => x + c.xp);
        logAudit(`Completed course: ${c.title} (+${c.xp} XP)`);
      }
      return { ...c, completedLessons: newCompleted, status: newStatus };
    }));
    setFuelUsed(f => f + 2);
  }, [logAudit]);

  const runChallenge = useCallback(() => {
    if (!activeChallenge) return;
    // Simulate check — pass if code has non-trivial content
    const passed = challengeCode.trim().length > 20 && !challengeCode.includes("todo!()");
    setChallenges(prev => prev.map(c => c.id === activeChallenge.id ? { ...c, result: passed ? "pass" as ChallengeResult : "fail" as ChallengeResult } : c));
    if (passed) {
      setXpTotal(x => x + activeChallenge.xp);
      logAudit(`Challenge passed: ${activeChallenge.title} (+${activeChallenge.xp} XP)`);
    } else {
      logAudit(`Challenge failed: ${activeChallenge.title}`);
    }
    setFuelUsed(f => f + 5);
  }, [activeChallenge, challengeCode, logAudit]);

  const selectChallenge = useCallback((id: string) => {
    setSelectedChallenge(id);
    const ch = challenges.find(c => c.id === id);
    if (ch) setChallengeCode(ch.starterCode);
    setShowHint(-1);
  }, [challenges]);

  const upvoteKnowledge = useCallback((id: string) => {
    logAudit(`Upvoted knowledge: ${id}`);
  }, [logAudit]);

  /* ─── render ─── */
  return (
    <div className="lc-container">
      {/* ─── Sidebar ─── */}
      <aside className="lc-sidebar">
        <div className="lc-sidebar-header">
          <h2 className="lc-sidebar-title">Learning Center</h2>
          <div className="lc-xp-badge">⭐ {totalXp} XP</div>
        </div>

        {/* views */}
        <div className="lc-views">
          {([["courses", "📚", "Courses"], ["challenges", "⚔", "Challenges"], ["knowledge", "🧠", "Knowledge"], ["videos", "▶", "Videos"], ["progress", "📊", "Progress"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`lc-view-btn ${view === id ? "active" : ""}`} onClick={() => { setView(id); setSelectedCourse(null); setSelectedChallenge(null); }}>
              <span>{icon}</span> {label}
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
            <div className="lc-stat"><span>{courses.filter(c => c.status === "completed").length}</span><span>Completed</span></div>
            <div className="lc-stat"><span>{courses.filter(c => c.status === "in-progress").length}</span><span>In Progress</span></div>
            <div className="lc-stat"><span>{challenges.filter(c => c.result === "pass").length}/{challenges.length}</span><span>Challenges</span></div>
          </div>
        </div>

        {/* filters */}
        <div className="lc-filters">
          <div className="lc-section-header">Filters</div>
          <select className="lc-filter-select" value={filterCategory} onChange={e => setFilterCategory(e.target.value)}>
            <option value="all">All Categories</option>
            {CATEGORIES.map(c => <option key={c} value={c}>{c}</option>)}
          </select>
          <select className="lc-filter-select" value={filterDifficulty} onChange={e => setFilterDifficulty(e.target.value as Difficulty | "all")}>
            <option value="all">All Levels</option>
            <option value="beginner">Beginner</option>
            <option value="intermediate">Intermediate</option>
            <option value="advanced">Advanced</option>
          </select>
          <input className="lc-search" placeholder="Search..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
        </div>

        {/* self-improve learnings */}
        <div className="lc-learnings">
          <div className="lc-section-header">🧠 Agent Insights</div>
          {learnings.slice(0, 3).map(l => (
            <div key={l.id} className="lc-learning-item">
              <div className="lc-learning-header">
                <span className="lc-learning-agent" style={{ color: l.agentColor }}>⬢</span>
                <span className="lc-learning-title">{l.title.slice(0, 30)}...</span>
              </div>
              <div className="lc-learning-conf">
                <div className="lc-conf-bar"><div className="lc-conf-fill" style={{ width: `${l.confidence * 100}%` }} /></div>
                <span>{Math.round(l.confidence * 100)}%</span>
              </div>
            </div>
          ))}
        </div>

        {/* audit */}
        <div className="lc-audit">
          <div className="lc-section-header">Activity</div>
          {auditLog.slice(0, 4).map((msg, i) => (
            <div key={i} className="lc-audit-entry">{msg}</div>
          ))}
        </div>
      </aside>

      {/* ─── Main ─── */}
      <div className="lc-main">

        {/* ═══ COURSES VIEW ═══ */}
        {view === "courses" && !selectedCourse && (
          <div className="lc-courses">
            <div className="lc-view-header">
              <h3 className="lc-view-title">📚 Courses & Tutorials</h3>
              <span className="lc-view-count">{filteredCourses.length} courses</span>
            </div>
            <div className="lc-course-grid">
              {filteredCourses.map(course => (
                <div key={course.id} className="lc-course-card" onClick={() => setSelectedCourse(course.id)}>
                  <div className="lc-course-thumb" style={{ background: course.thumbnail }}>
                    <span className="lc-course-diff" style={{ color: DIFF_COLORS[course.difficulty] }}>{course.difficulty}</span>
                  </div>
                  <div className="lc-course-body">
                    <div className="lc-course-title">{course.title}</div>
                    <div className="lc-course-desc">{course.description.slice(0, 80)}...</div>
                    <div className="lc-course-meta">
                      <span className="lc-course-agent">⬢ {course.agent}</span>
                      <span>⭐ {course.xp} XP</span>
                    </div>
                    <div className="lc-course-progress-row">
                      <div className="lc-course-progress-bar">
                        <div className="lc-course-progress-fill" style={{ width: `${(course.completedLessons / course.lessons) * 100}%` }} />
                      </div>
                      <span className="lc-course-progress-text">{course.completedLessons}/{course.lessons}</span>
                    </div>
                    <div className="lc-course-tags">
                      {course.tags.map(t => <span key={t} className="lc-tag">{t}</span>)}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ═══ COURSE DETAIL ═══ */}
        {view === "courses" && activeCourse && (
          <div className="lc-course-detail">
            <button className="lc-back-btn" onClick={() => setSelectedCourse(null)}>← Back to Courses</button>
            <div className="lc-cd-header">
              <div className="lc-cd-thumb" style={{ background: activeCourse.thumbnail }} />
              <div className="lc-cd-info">
                <h3 className="lc-cd-title">{activeCourse.title}</h3>
                <div className="lc-cd-desc">{activeCourse.description}</div>
                <div className="lc-cd-meta">
                  <span className="lc-cd-diff" style={{ color: DIFF_COLORS[activeCourse.difficulty] }}>{activeCourse.difficulty}</span>
                  <span>⬢ {activeCourse.agent}</span>
                  <span>⭐ {activeCourse.xp} XP</span>
                  <span>{activeCourse.lessons} lessons</span>
                </div>
                <div className="lc-cd-progress">
                  <div className="lc-cd-progress-bar">
                    <div className="lc-cd-progress-fill" style={{ width: `${(activeCourse.completedLessons / activeCourse.lessons) * 100}%` }} />
                  </div>
                  <span>{activeCourse.completedLessons}/{activeCourse.lessons} completed</span>
                </div>
              </div>
            </div>
            <div className="lc-cd-lessons">
              <h4>Lessons</h4>
              {Array.from({ length: activeCourse.lessons }, (_, i) => (
                <div key={i} className={`lc-cd-lesson ${i < activeCourse.completedLessons ? "completed" : i === activeCourse.completedLessons ? "current" : ""}`}>
                  <span className="lc-cd-lesson-num">{i < activeCourse.completedLessons ? "✓" : i + 1}</span>
                  <span className="lc-cd-lesson-title">
                    {i === 0 ? "Introduction & Setup" :
                     i === activeCourse.lessons - 1 ? "Final Project & Review" :
                     `Lesson ${i + 1}: ${activeCourse.category} ${["Concepts", "Patterns", "Practice", "Deep Dive", "Integration", "Testing", "Optimization", "Advanced", "Real-World", "Architecture", "Debugging", "Deployment", "Review"][i % 13]}`}
                  </span>
                  {i === activeCourse.completedLessons && activeCourse.status !== "completed" && (
                    <button className="lc-cd-lesson-start" onClick={() => completeLesson(activeCourse.id)}>
                      Start →
                    </button>
                  )}
                </div>
              ))}
            </div>
            {activeCourse.status === "not-started" && (
              <button className="lc-start-course-btn" onClick={() => startCourse(activeCourse.id)}>🚀 Start Course</button>
            )}
          </div>
        )}

        {/* ═══ CHALLENGES VIEW ═══ */}
        {view === "challenges" && (
          <div className="lc-challenges">
            <div className="lc-view-header">
              <h3 className="lc-view-title">⚔ Code Challenges</h3>
              <span className="lc-view-count">{challenges.filter(c => c.result === "pass").length}/{challenges.length} solved</span>
            </div>
            <div className="lc-challenges-grid">
              {/* list */}
              <div className="lc-challenge-list">
                {filteredChallenges.map(ch => (
                  <div key={ch.id} className={`lc-challenge-item ${selectedChallenge === ch.id ? "active" : ""}`} onClick={() => selectChallenge(ch.id)}>
                    <div className="lc-ch-status">
                      {ch.result === "pass" ? <span className="lc-ch-pass">✓</span> :
                       ch.result === "fail" ? <span className="lc-ch-fail">✗</span> :
                       <span className="lc-ch-untried">○</span>}
                    </div>
                    <div className="lc-ch-info">
                      <div className="lc-ch-title">{ch.title}</div>
                      <div className="lc-ch-meta">
                        <span style={{ color: DIFF_COLORS[ch.difficulty] }}>{ch.difficulty}</span>
                        <span>⭐ {ch.xp} XP</span>
                        <span>{ch.solvedBy} solved</span>
                      </div>
                    </div>
                  </div>
                ))}
              </div>

              {/* editor */}
              {activeChallenge ? (
                <div className="lc-challenge-editor">
                  <div className="lc-ce-header">
                    <h4>{activeChallenge.title}</h4>
                    <span className="lc-ce-diff" style={{ color: DIFF_COLORS[activeChallenge.difficulty] }}>{activeChallenge.difficulty} · ⭐ {activeChallenge.xp} XP</span>
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
                    </div>
                    <textarea className="lc-ce-textarea" value={challengeCode} onChange={e => setChallengeCode(e.target.value)} spellCheck={false} />
                  </div>
                  <div className="lc-ce-actions">
                    <button className="lc-ce-run" onClick={runChallenge}>▶ Run & Check (⚡ 5)</button>
                    {activeChallenge.hints.map((_, i) => (
                      <button key={i} className="lc-ce-hint" onClick={() => setShowHint(showHint === i ? -1 : i)}>
                        💡 Hint {i + 1}
                      </button>
                    ))}
                  </div>
                  {showHint >= 0 && showHint < activeChallenge.hints.length && (
                    <div className="lc-ce-hint-box">{activeChallenge.hints[showHint]}</div>
                  )}
                  {activeChallenge.result === "pass" && (
                    <div className="lc-ce-result lc-ce-result-pass">✓ All tests passed! +{activeChallenge.xp} XP</div>
                  )}
                  {activeChallenge.result === "fail" && (
                    <div className="lc-ce-result lc-ce-result-fail">✗ Tests failed — check your logic and try again.</div>
                  )}
                </div>
              ) : (
                <div className="lc-ce-empty">
                  <div className="lc-ce-empty-icon">⚔</div>
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
              <h3 className="lc-view-title">🧠 Knowledge Base</h3>
              <span className="lc-view-count">{knowledge.length} articles</span>
            </div>

            {/* Agent Learnings Section */}
            <div className="lc-learnings-section">
              <h4 className="lc-sub-title">⬢ Self-Improve Agent Learnings</h4>
              <div className="lc-learnings-grid">
                {learnings.map(l => (
                  <div key={l.id} className={`lc-learning-card ${l.applied ? "applied" : ""}`}>
                    <div className="lc-lc-header">
                      <span className="lc-lc-agent" style={{ color: l.agentColor }}>⬢ {l.agent}</span>
                      <span className="lc-lc-category">{l.category}</span>
                      {l.applied && <span className="lc-lc-applied">Applied ✓</span>}
                    </div>
                    <div className="lc-lc-title">{l.title}</div>
                    <div className="lc-lc-insight">{l.insight}</div>
                    <div className="lc-lc-footer">
                      <div className="lc-lc-conf">
                        <span>Confidence:</span>
                        <div className="lc-conf-bar"><div className="lc-conf-fill" style={{ width: `${l.confidence * 100}%` }} /></div>
                        <span>{Math.round(l.confidence * 100)}%</span>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>

            {/* Community Knowledge */}
            <h4 className="lc-sub-title">📖 Community Knowledge</h4>
            <div className="lc-kb-list">
              {knowledge.map(entry => (
                <div key={entry.id} className="lc-kb-card">
                  <div className="lc-kb-header">
                    <div className="lc-kb-title">{entry.title}</div>
                    <div className="lc-kb-meta">
                      <span>{entry.agent ? `⬢ ${entry.agent}` : entry.author}</span>
                      <span>{entry.category}</span>
                      <button className="lc-kb-upvote" onClick={() => upvoteKnowledge(entry.id)}>▲ {entry.upvotes}</button>
                    </div>
                  </div>
                  <div className="lc-kb-content">{entry.content.slice(0, 200)}...</div>
                  <div className="lc-kb-tags">
                    {entry.tags.map(t => <span key={t} className="lc-tag">{t}</span>)}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ═══ VIDEOS VIEW ═══ */}
        {view === "videos" && (
          <div className="lc-videos">
            <div className="lc-view-header">
              <h3 className="lc-view-title">▶ Video Tutorials</h3>
              <span className="lc-view-count">{videos.length} videos</span>
            </div>
            <div className="lc-video-grid">
              {videos.map(vid => (
                <div key={vid.id} className="lc-video-card">
                  <div className="lc-video-thumb" style={{ background: vid.thumbnail }}>
                    <div className="lc-video-play">▶</div>
                    <span className="lc-video-duration">{vid.duration}</span>
                  </div>
                  <div className="lc-video-body">
                    <div className="lc-video-title">{vid.title}</div>
                    <div className="lc-video-desc">{vid.description}</div>
                    <div className="lc-video-meta">
                      <span>⬢ {vid.agent}</span>
                      <span style={{ color: DIFF_COLORS[vid.difficulty] }}>{vid.difficulty}</span>
                      <span>{vid.views.toLocaleString()} views</span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
            <div className="lc-video-note">
              <span>⬢</span> All video content is generated by Nexus OS agents. Videos are auto-updated as the platform evolves.
            </div>
          </div>
        )}

        {/* ═══ PROGRESS VIEW ═══ */}
        {view === "progress" && (
          <div className="lc-progress-view">
            <div className="lc-view-header">
              <h3 className="lc-view-title">📊 Your Learning Progress</h3>
            </div>

            {/* XP & level */}
            <div className="lc-pv-hero">
              <div className="lc-pv-xp">
                <div className="lc-pv-xp-number">⭐ {totalXp}</div>
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

            {/* stats grid */}
            <div className="lc-pv-stats">
              <div className="lc-pv-stat-card">
                <div className="lc-pv-stat-value">{overallProgress}%</div>
                <div className="lc-pv-stat-label">Course Completion</div>
                <div className="lc-pv-stat-bar"><div style={{ width: `${overallProgress}%`, background: "#22d3ee", height: "100%", borderRadius: 2 }} /></div>
              </div>
              <div className="lc-pv-stat-card">
                <div className="lc-pv-stat-value">{courses.filter(c => c.status === "completed").length}/{courses.length}</div>
                <div className="lc-pv-stat-label">Courses Completed</div>
                <div className="lc-pv-stat-bar"><div style={{ width: `${(courses.filter(c => c.status === "completed").length / courses.length) * 100}%`, background: "#22c55e", height: "100%", borderRadius: 2 }} /></div>
              </div>
              <div className="lc-pv-stat-card">
                <div className="lc-pv-stat-value">{challenges.filter(c => c.result === "pass").length}/{challenges.length}</div>
                <div className="lc-pv-stat-label">Challenges Solved</div>
                <div className="lc-pv-stat-bar"><div style={{ width: `${(challenges.filter(c => c.result === "pass").length / challenges.length) * 100}%`, background: "#f59e0b", height: "100%", borderRadius: 2 }} /></div>
              </div>
              <div className="lc-pv-stat-card">
                <div className="lc-pv-stat-value">{knowledge.length + learnings.length}</div>
                <div className="lc-pv-stat-label">Knowledge Articles</div>
                <div className="lc-pv-stat-bar"><div style={{ width: "75%", background: "#a78bfa", height: "100%", borderRadius: 2 }} /></div>
              </div>
            </div>

            {/* per-course progress */}
            <h4 className="lc-sub-title">Course Progress</h4>
            <div className="lc-pv-courses">
              {courses.map(c => (
                <div key={c.id} className="lc-pv-course-row">
                  <div className="lc-pv-course-info">
                    <span className="lc-pv-course-name">{c.title}</span>
                    <span className="lc-pv-course-status" style={{ color: c.status === "completed" ? "#22c55e" : c.status === "in-progress" ? "#f59e0b" : "#64748b" }}>
                      {c.status === "completed" ? "✓ Complete" : c.status === "in-progress" ? "In Progress" : "Not Started"}
                    </span>
                  </div>
                  <div className="lc-pv-course-bar-row">
                    <div className="lc-pv-course-bar"><div className="lc-pv-course-fill" style={{ width: `${(c.completedLessons / c.lessons) * 100}%` }} /></div>
                    <span className="lc-pv-course-pct">{Math.round((c.completedLessons / c.lessons) * 100)}%</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* ─── Status Bar ─── */}
      <div className="lc-status-bar">
        <span className="lc-status-item">⭐ {totalXp} XP</span>
        <span className="lc-status-item">Level {Math.floor(totalXp / 500) + 1}</span>
        <span className="lc-status-item">{courses.filter(c => c.status === "completed").length}/{courses.length} courses</span>
        <span className="lc-status-item">{challenges.filter(c => c.result === "pass").length}/{challenges.length} challenges</span>
        <span className="lc-status-item">{overallProgress}% progress</span>
        <span className="lc-status-item lc-status-right">⚡ {fuelUsed} fuel</span>
      </div>
    </div>
  );
}
