import { useState, useCallback, useMemo } from "react";
import "./app-store.css";

/* ─── types ─── */
type View = "featured" | "browse" | "installed" | "updates" | "publish";
type Category = "productivity" | "security" | "data" | "social" | "devtools" | "automation" | "ai" | "utilities";
type InstallStatus = "not-installed" | "installing" | "installed" | "update-available";

interface AppScreenshot {
  label: string;
  gradient: string;
}

interface AppReview {
  id: string;
  author: string;
  rating: number;
  text: string;
  date: number;
  helpful: number;
}

interface StoreApp {
  id: string;
  name: string;
  developer: string;
  developerVerified: boolean;
  description: string;
  longDescription: string;
  version: string;
  category: Category;
  rating: number;
  reviewCount: number;
  downloads: number;
  size: number;
  icon: string;
  gradient: string;
  screenshots: AppScreenshot[];
  reviews: AppReview[];
  capabilities: string[];
  dependencies: string[];
  signature: string;
  signatureValid: boolean;
  installStatus: InstallStatus;
  installedVersion?: string;
  featured?: boolean;
  fuelCost: number;
  autonomyLevel: string;
  lastUpdated: number;
  changelog?: string;
}

/* ─── constants ─── */
const CATEGORIES: { id: Category; label: string; icon: string }[] = [
  { id: "productivity", label: "Productivity", icon: "⚡" },
  { id: "security", label: "Security", icon: "🛡" },
  { id: "data", label: "Data", icon: "📊" },
  { id: "social", label: "Social", icon: "📱" },
  { id: "devtools", label: "Dev Tools", icon: "⌨" },
  { id: "automation", label: "Automation", icon: "⎇" },
  { id: "ai", label: "AI / ML", icon: "✦" },
  { id: "utilities", label: "Utilities", icon: "🔧" },
];

const INITIAL_APPS: StoreApp[] = [
  {
    id: "app-1", name: "Code Review Agent", developer: "Nexus Labs", developerVerified: true,
    description: "Automated code review with security analysis and style enforcement.",
    longDescription: "Code Review Agent scans pull requests for security vulnerabilities, code style violations, performance issues, and best practice adherence. Supports Rust, TypeScript, Python, Go. Integrates with GitHub, GitLab, and Bitbucket.",
    version: "2.4.1", category: "devtools", rating: 4.8, reviewCount: 342, downloads: 12400, size: 2800000,
    icon: "⌨", gradient: "linear-gradient(135deg, #0f172a 0%, #22d3ee 100%)",
    screenshots: [
      { label: "PR Analysis View", gradient: "linear-gradient(135deg, #0b1120 0%, #1e293b 50%, #22d3ee 100%)" },
      { label: "Security Report", gradient: "linear-gradient(135deg, #0f172a 0%, #ef4444 50%, #fbbf24 100%)" },
      { label: "Style Dashboard", gradient: "linear-gradient(135deg, #1e293b 0%, #818cf8 50%, #22d3ee 100%)" },
    ],
    reviews: [
      { id: "r-1", author: "devops_sarah", rating: 5, text: "Catches issues our team misses. The security scanning alone is worth it.", date: Date.now() - 86400000 * 3, helpful: 24 },
      { id: "r-2", author: "rust_engineer", rating: 5, text: "Excellent Rust support. Found 3 unsafe blocks we missed.", date: Date.now() - 86400000 * 7, helpful: 18 },
      { id: "r-3", author: "team_lead_mike", rating: 4, text: "Great tool. Would love to see more customizable rules.", date: Date.now() - 86400000 * 14, helpful: 12 },
    ],
    capabilities: ["fs.read", "llm.query", "net.http"], dependencies: [],
    signature: "ed25519:a8f3b2c1d4e5f6a7b8c9d0e1f2a3b4c5", signatureValid: true,
    installStatus: "installed", installedVersion: "2.4.0", featured: true,
    fuelCost: 50, autonomyLevel: "L2", lastUpdated: Date.now() - 86400000 * 2,
    changelog: "v2.4.1: Added Go support, fixed false positives in async patterns",
  },
  {
    id: "app-2", name: "Data Pipeline Agent", developer: "DataFlow Inc", developerVerified: true,
    description: "ETL pipeline builder with governed data access and transformation.",
    longDescription: "Build and run ETL pipelines with visual DAG editor. Supports PostgreSQL, MySQL, SQLite, S3, and REST APIs. All data access is governed with capability checks and audit logging. Includes 40+ built-in transformations.",
    version: "1.8.0", category: "data", rating: 4.6, reviewCount: 189, downloads: 8200, size: 4100000,
    icon: "📊", gradient: "linear-gradient(135deg, #0f172a 0%, #a78bfa 100%)",
    screenshots: [
      { label: "Pipeline Builder", gradient: "linear-gradient(135deg, #0f172a 0%, #6d28d9 50%, #a78bfa 100%)" },
      { label: "Data Preview", gradient: "linear-gradient(135deg, #1e293b 0%, #334155 50%, #22c55e 100%)" },
    ],
    reviews: [
      { id: "r-4", author: "data_analyst_j", rating: 5, text: "Replaced our entire ETL stack. The governance layer gives us confidence.", date: Date.now() - 86400000 * 5, helpful: 31 },
      { id: "r-5", author: "startup_cto", rating: 4, text: "Solid agent. S3 connector could be faster.", date: Date.now() - 86400000 * 12, helpful: 8 },
    ],
    capabilities: ["fs.read", "fs.write", "llm.query", "net.http", "db.query"], dependencies: [],
    signature: "ed25519:b9c4d3e2f1a0b7c8d9e0f1a2b3c4d5e6", signatureValid: true,
    installStatus: "not-installed", featured: true,
    fuelCost: 80, autonomyLevel: "L3", lastUpdated: Date.now() - 86400000 * 8,
  },
  {
    id: "app-3", name: "Threat Monitor", developer: "SecureOps", developerVerified: true,
    description: "Real-time security threat detection and response automation.",
    longDescription: "Monitors network traffic, system logs, and agent behavior for security threats. Uses ML-based anomaly detection. Automatically quarantines suspicious agents and alerts administrators. Integrates with SIEM tools.",
    version: "3.1.2", category: "security", rating: 4.9, reviewCount: 567, downloads: 21300, size: 5600000,
    icon: "🛡", gradient: "linear-gradient(135deg, #0f172a 0%, #ef4444 100%)",
    screenshots: [
      { label: "Threat Dashboard", gradient: "linear-gradient(135deg, #0b1120 0%, #ef4444 50%, #fbbf24 100%)" },
      { label: "Alert Timeline", gradient: "linear-gradient(135deg, #1e293b 0%, #dc2626 50%, #f97316 100%)" },
      { label: "Quarantine Panel", gradient: "linear-gradient(135deg, #0f172a 0%, #7f1d1d 50%, #991b1b 100%)" },
    ],
    reviews: [
      { id: "r-6", author: "infosec_lead", rating: 5, text: "Best security agent on the store. The ML anomaly detection is top-tier.", date: Date.now() - 86400000 * 1, helpful: 45 },
      { id: "r-7", author: "soc_analyst", rating: 5, text: "Caught a supply chain attack attempt before it reached production.", date: Date.now() - 86400000 * 10, helpful: 67 },
    ],
    capabilities: ["net.http", "net.listen", "llm.query", "process.inspect"], dependencies: [],
    signature: "ed25519:c0d5e4f3a2b1c8d7e6f5a4b3c2d1e0f9", signatureValid: true,
    installStatus: "installed", installedVersion: "3.1.2", featured: true,
    fuelCost: 120, autonomyLevel: "L4", lastUpdated: Date.now() - 86400000 * 4,
  },
  {
    id: "app-4", name: "Social Scheduler", developer: "PostFlow", developerVerified: false,
    description: "Multi-platform social media scheduling with AI content optimization.",
    longDescription: "Schedule posts across X, Instagram, LinkedIn, Reddit, and Facebook. AI-powered content optimization suggests best posting times, hashtags, and content variations. All posts require HITL approval before publishing.",
    version: "1.3.0", category: "social", rating: 4.2, reviewCount: 95, downloads: 3400, size: 1800000,
    icon: "📱", gradient: "linear-gradient(135deg, #0f172a 0%, #ec4899 100%)",
    screenshots: [
      { label: "Schedule View", gradient: "linear-gradient(135deg, #0f172a 0%, #ec4899 50%, #f472b6 100%)" },
      { label: "Analytics", gradient: "linear-gradient(135deg, #1e293b 0%, #a855f7 50%, #ec4899 100%)" },
    ],
    reviews: [
      { id: "r-8", author: "marketing_lead", rating: 4, text: "Good scheduling. Wish it had better analytics.", date: Date.now() - 86400000 * 6, helpful: 9 },
    ],
    capabilities: ["llm.query", "net.http", "request_approval"], dependencies: [],
    signature: "ed25519:d1e6f5a4b3c2d9e8f7a6b5c4d3e2f1a0", signatureValid: true,
    installStatus: "not-installed",
    fuelCost: 30, autonomyLevel: "L2", lastUpdated: Date.now() - 86400000 * 20,
  },
  {
    id: "app-5", name: "Test Runner Pro", developer: "Nexus Labs", developerVerified: true,
    description: "Parallel test execution with AI-generated test cases and coverage reports.",
    longDescription: "Run test suites in parallel across sandboxed environments. AI generates edge case tests from code analysis. Coverage reports with diff tracking. Supports Rust (cargo test), Node (jest/vitest), Python (pytest).",
    version: "2.0.3", category: "devtools", rating: 4.7, reviewCount: 223, downloads: 9800, size: 3200000,
    icon: "✓", gradient: "linear-gradient(135deg, #0f172a 0%, #22c55e 100%)",
    screenshots: [
      { label: "Test Results", gradient: "linear-gradient(135deg, #0b1120 0%, #22c55e 50%, #16a34a 100%)" },
      { label: "Coverage Map", gradient: "linear-gradient(135deg, #1e293b 0%, #15803d 50%, #22c55e 100%)" },
    ],
    reviews: [
      { id: "r-9", author: "qa_engineer", rating: 5, text: "The AI-generated edge cases found bugs our manual tests missed.", date: Date.now() - 86400000 * 9, helpful: 33 },
      { id: "r-10", author: "fullstack_dev", rating: 4, text: "Fast and reliable. Love the parallel execution.", date: Date.now() - 86400000 * 15, helpful: 14 },
    ],
    capabilities: ["fs.read", "fs.write", "llm.query", "process.exec"], dependencies: ["app-1"],
    signature: "ed25519:e2f7a6b5c4d3e0f9a8b7c6d5e4f3a2b1", signatureValid: true,
    installStatus: "update-available", installedVersion: "1.9.8",
    fuelCost: 60, autonomyLevel: "L3", lastUpdated: Date.now() - 86400000 * 1,
    changelog: "v2.0.3: Major perf improvements, added vitest support, AI test gen v2",
  },
  {
    id: "app-6", name: "Doc Generator", developer: "DocWorks", developerVerified: true,
    description: "Auto-generate documentation from code with AI-enhanced explanations.",
    longDescription: "Scans your codebase and generates comprehensive documentation including API docs, architecture diagrams, usage examples, and migration guides. Supports JSDoc, Rustdoc, and custom formats.",
    version: "1.5.1", category: "productivity", rating: 4.4, reviewCount: 156, downloads: 6100, size: 2100000,
    icon: "📄", gradient: "linear-gradient(135deg, #0f172a 0%, #fbbf24 100%)",
    screenshots: [
      { label: "Doc Preview", gradient: "linear-gradient(135deg, #0f172a 0%, #fbbf24 50%, #f59e0b 100%)" },
    ],
    reviews: [
      { id: "r-11", author: "tech_writer", rating: 5, text: "Saves hours of documentation work. The AI explanations are surprisingly good.", date: Date.now() - 86400000 * 4, helpful: 22 },
    ],
    capabilities: ["fs.read", "llm.query"], dependencies: [],
    signature: "ed25519:f3a8b7c6d5e4f1a0b9c8d7e6f5a4b3c2", signatureValid: true,
    installStatus: "installed", installedVersion: "1.5.1",
    fuelCost: 40, autonomyLevel: "L2", lastUpdated: Date.now() - 86400000 * 11,
  },
  {
    id: "app-7", name: "Workflow Automator", developer: "AutomateHQ", developerVerified: true,
    description: "Visual workflow builder with 50+ integrations and conditional logic.",
    longDescription: "Create complex automation workflows with a visual drag-and-drop builder. Supports conditional branching, loops, error handling, and parallel execution. 50+ built-in integrations including Slack, GitHub, Jira, and custom webhooks.",
    version: "3.2.0", category: "automation", rating: 4.5, reviewCount: 278, downloads: 11500, size: 3800000,
    icon: "⎇", gradient: "linear-gradient(135deg, #0f172a 0%, #06b6d4 100%)",
    screenshots: [
      { label: "Workflow Builder", gradient: "linear-gradient(135deg, #0b1120 0%, #06b6d4 50%, #22d3ee 100%)" },
      { label: "Integration Hub", gradient: "linear-gradient(135deg, #1e293b 0%, #0891b2 50%, #06b6d4 100%)" },
    ],
    reviews: [
      { id: "r-12", author: "ops_manager", rating: 5, text: "Replaced 4 different automation tools with this one agent.", date: Date.now() - 86400000 * 2, helpful: 41 },
    ],
    capabilities: ["net.http", "llm.query", "fs.read", "fs.write"], dependencies: [],
    signature: "ed25519:a4b9c8d7e6f5a0b1c2d3e4f5a6b7c8d9", signatureValid: true,
    installStatus: "not-installed",
    fuelCost: 70, autonomyLevel: "L3", lastUpdated: Date.now() - 86400000 * 6,
  },
  {
    id: "app-8", name: "Model Fine-Tuner", developer: "AI Works", developerVerified: true,
    description: "Fine-tune local LLMs on your data with governed training pipelines.",
    longDescription: "Fine-tune Llama, Mistral, Qwen, and other open models on your domain-specific data. Governed data access ensures PII protection during training. Supports LoRA, QLoRA, and full fine-tuning. Automatic evaluation and deployment.",
    version: "1.1.0", category: "ai", rating: 4.3, reviewCount: 67, downloads: 2100, size: 8400000,
    icon: "✦", gradient: "linear-gradient(135deg, #0f172a 0%, #4f46e5 100%)",
    screenshots: [
      { label: "Training Dashboard", gradient: "linear-gradient(135deg, #0f172a 0%, #4f46e5 50%, #818cf8 100%)" },
      { label: "Eval Results", gradient: "linear-gradient(135deg, #1e293b 0%, #6366f1 50%, #a78bfa 100%)" },
    ],
    reviews: [
      { id: "r-13", author: "ml_engineer", rating: 4, text: "Great for quick fine-tuning. PII protection is a killer feature.", date: Date.now() - 86400000 * 8, helpful: 19 },
    ],
    capabilities: ["fs.read", "fs.write", "llm.query", "llm.finetune", "gpu.access"], dependencies: [],
    signature: "ed25519:b5c0d9e8f7a6b1c2d3e4f5a6b7c8d9e0", signatureValid: true,
    installStatus: "not-installed",
    fuelCost: 200, autonomyLevel: "L3", lastUpdated: Date.now() - 86400000 * 15,
  },
  {
    id: "app-9", name: "Backup Agent", developer: "VaultOps", developerVerified: false,
    description: "Automated encrypted backups with governed restore and retention policies.",
    longDescription: "Schedule encrypted backups of files, databases, and configuration. Supports local, S3, and GCS destinations. Governed restore requires HITL approval. Configurable retention policies with automatic cleanup.",
    version: "1.0.2", category: "utilities", rating: 4.1, reviewCount: 43, downloads: 1800, size: 1400000,
    icon: "💾", gradient: "linear-gradient(135deg, #0f172a 0%, #64748b 100%)",
    screenshots: [
      { label: "Backup Schedule", gradient: "linear-gradient(135deg, #0f172a 0%, #475569 50%, #64748b 100%)" },
    ],
    reviews: [
      { id: "r-14", author: "sysadmin_k", rating: 4, text: "Simple and reliable. Wish it had incremental backups.", date: Date.now() - 86400000 * 18, helpful: 6 },
    ],
    capabilities: ["fs.read", "fs.write", "net.http", "request_approval"], dependencies: [],
    signature: "ed25519:INVALID_SIGNATURE_TAMPERED", signatureValid: false,
    installStatus: "not-installed",
    fuelCost: 25, autonomyLevel: "L2", lastUpdated: Date.now() - 86400000 * 30,
  },
];

/* ─── component ─── */
export default function AppStore() {
  const [view, setView] = useState<View>("featured");
  const [apps, setApps] = useState<StoreApp[]>(INITIAL_APPS);
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<Category | "all">("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [sortBy, setSortBy] = useState<"popular" | "rating" | "recent" | "name">("popular");
  const [fuelUsed, setFuelUsed] = useState(18);
  const [auditLog, setAuditLog] = useState<string[]>([
    "Store opened",
    "Installed Code Review Agent v2.4.0",
    "Signature verified: Threat Monitor",
  ]);

  // review state
  const [showReviewForm, setShowReviewForm] = useState(false);
  const [reviewRating, setReviewRating] = useState(5);
  const [reviewText, setReviewText] = useState("");

  // publish state
  const [pubName, setPubName] = useState("");
  const [pubDesc, setPubDesc] = useState("");
  const [pubCategory, setPubCategory] = useState<Category>("devtools");
  const [pubVersion, setPubVersion] = useState("1.0.0");
  const [pubFuel, setPubFuel] = useState("50");

  const selectedApp = useMemo(() => apps.find(a => a.id === selectedAppId) ?? null, [apps, selectedAppId]);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 30)), []);

  /* ─── filtered apps ─── */
  const filteredApps = useMemo(() => {
    let list = [...apps];
    if (view === "installed") list = list.filter(a => a.installStatus === "installed" || a.installStatus === "update-available");
    if (view === "updates") list = list.filter(a => a.installStatus === "update-available");
    if (selectedCategory !== "all") list = list.filter(a => a.category === selectedCategory);
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(a => a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q) || a.developer.toLowerCase().includes(q));
    }
    list.sort((a, b) => {
      if (sortBy === "rating") return b.rating - a.rating;
      if (sortBy === "recent") return b.lastUpdated - a.lastUpdated;
      if (sortBy === "name") return a.name.localeCompare(b.name);
      return b.downloads - a.downloads;
    });
    return list;
  }, [apps, view, selectedCategory, searchQuery, sortBy]);

  const featuredApps = useMemo(() => apps.filter(a => a.featured), [apps]);

  /* ─── helpers ─── */
  const formatSize = (bytes: number) => {
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  };

  const formatNumber = (n: number) => n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`;

  const formatDate = (ts: number) => new Date(ts).toLocaleDateString();

  const renderStars = (rating: number) => {
    const full = Math.floor(rating);
    const half = rating - full >= 0.5;
    return Array.from({ length: 5 }, (_, i) => {
      if (i < full) return "★";
      if (i === full && half) return "⯨";
      return "☆";
    }).join("");
  };

  /* ─── actions ─── */
  const installApp = useCallback((id: string) => {
    const app = apps.find(a => a.id === id);
    if (!app) return;
    if (!app.signatureValid) {
      logAudit(`BLOCKED: ${app.name} — invalid Ed25519 signature`);
      return;
    }
    setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installing" as InstallStatus } : a));
    setFuelUsed(f => f + 5);
    logAudit(`Installing ${app.name} v${app.version}...`);
    setTimeout(() => {
      setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installed", installedVersion: a.version, downloads: a.downloads + 1 } : a));
      logAudit(`Installed ${app.name} v${app.version}`);
    }, 2000);
  }, [apps, logAudit]);

  const updateApp = useCallback((id: string) => {
    const app = apps.find(a => a.id === id);
    if (!app) return;
    setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installing" as InstallStatus } : a));
    setFuelUsed(f => f + 3);
    logAudit(`Updating ${app.name} to v${app.version}...`);
    setTimeout(() => {
      setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installed", installedVersion: a.version } : a));
      logAudit(`Updated ${app.name} to v${app.version}`);
    }, 1500);
  }, [apps, logAudit]);

  const uninstallApp = useCallback((id: string) => {
    const app = apps.find(a => a.id === id);
    if (!app) return;
    setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "not-installed", installedVersion: undefined } : a));
    logAudit(`Uninstalled ${app.name}`);
  }, [apps, logAudit]);

  const submitReview = useCallback(() => {
    if (!selectedApp || !reviewText.trim()) return;
    const review: AppReview = {
      id: `r-${Date.now()}`, author: "suresh_k", rating: reviewRating,
      text: reviewText, date: Date.now(), helpful: 0,
    };
    setApps(prev => prev.map(a => a.id === selectedApp.id ? {
      ...a,
      reviews: [review, ...a.reviews],
      reviewCount: a.reviewCount + 1,
      rating: Math.round(((a.rating * a.reviewCount + reviewRating) / (a.reviewCount + 1)) * 10) / 10,
    } : a));
    setReviewText("");
    setReviewRating(5);
    setShowReviewForm(false);
    setFuelUsed(f => f + 2);
    logAudit(`Review submitted for ${selectedApp.name}`);
  }, [selectedApp, reviewRating, reviewText, logAudit]);

  const handlePublish = useCallback(() => {
    if (!pubName.trim() || !pubDesc.trim()) return;
    const newApp: StoreApp = {
      id: `app-${Date.now()}`, name: pubName, developer: "Suresh Karicheti", developerVerified: true,
      description: pubDesc, longDescription: pubDesc, version: pubVersion, category: pubCategory,
      rating: 0, reviewCount: 0, downloads: 0, size: 1500000,
      icon: "⬢", gradient: "linear-gradient(135deg, #0f172a 0%, #22d3ee 100%)",
      screenshots: [], reviews: [], capabilities: ["llm.query"], dependencies: [],
      signature: `ed25519:${Date.now().toString(16)}`, signatureValid: true,
      installStatus: "installed", installedVersion: pubVersion,
      fuelCost: parseInt(pubFuel) || 50, autonomyLevel: "L2",
      lastUpdated: Date.now(),
    };
    setApps(prev => [newApp, ...prev]);
    setPubName(""); setPubDesc(""); setPubVersion("1.0.0");
    setFuelUsed(f => f + 10);
    logAudit(`Published: ${pubName} v${pubVersion}`);
  }, [pubName, pubDesc, pubVersion, pubCategory, pubFuel, logAudit]);

  /* ─── render ─── */
  return (
    <div className="as-container">
      {/* ─── Sidebar ─── */}
      <aside className="as-sidebar">
        <div className="as-sidebar-header">
          <h2 className="as-sidebar-title">App Store</h2>
        </div>

        <div className="as-views">
          {([["featured", "◈", "Featured"], ["browse", "⬡", "Browse All"], ["installed", "✓", "Installed"], ["updates", "↑", "Updates"], ["publish", "⬆", "Publish"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`as-view-btn ${view === id ? "active" : ""}`} onClick={() => { setView(id); setSelectedAppId(null); }}>
              <span className="as-view-icon">{icon}</span>{label}
              {id === "updates" && apps.filter(a => a.installStatus === "update-available").length > 0 && (
                <span className="as-update-badge">{apps.filter(a => a.installStatus === "update-available").length}</span>
              )}
            </button>
          ))}
        </div>

        {(view === "browse" || view === "featured") && (
          <div className="as-categories">
            <div className="as-section-header">Categories</div>
            <button className={`as-cat-btn ${selectedCategory === "all" ? "active" : ""}`} onClick={() => setSelectedCategory("all")}>All</button>
            {CATEGORIES.map(c => (
              <button key={c.id} className={`as-cat-btn ${selectedCategory === c.id ? "active" : ""}`} onClick={() => setSelectedCategory(c.id)}>
                <span>{c.icon}</span> {c.label}
              </button>
            ))}
          </div>
        )}

        <div className="as-stats">
          <div className="as-section-header">Store Stats</div>
          <div className="as-stat-item">{apps.length} agents available</div>
          <div className="as-stat-item">{apps.filter(a => a.installStatus === "installed" || a.installStatus === "update-available").length} installed</div>
          <div className="as-stat-item">{apps.filter(a => a.installStatus === "update-available").length} updates</div>
        </div>

        <div className="as-audit">
          <div className="as-section-header">Activity</div>
          {auditLog.slice(0, 5).map((msg, i) => (
            <div key={i} className="as-audit-entry">{msg}</div>
          ))}
        </div>
      </aside>

      {/* ─── Main ─── */}
      <div className="as-main">
        {/* app detail view */}
        {selectedApp ? (
          <div className="as-app-detail">
            <button className="as-back-btn" onClick={() => setSelectedAppId(null)}>← Back</button>

            <div className="as-detail-hero">
              <div className="as-detail-icon" style={{ background: selectedApp.gradient }}>{selectedApp.icon}</div>
              <div className="as-detail-info">
                <h2 className="as-detail-name">{selectedApp.name}</h2>
                <div className="as-detail-developer">
                  {selectedApp.developer}
                  {selectedApp.developerVerified && <span className="as-verified">✓ Verified</span>}
                </div>
                <div className="as-detail-meta">
                  <span className="as-detail-stars">{renderStars(selectedApp.rating)} {selectedApp.rating}</span>
                  <span>{selectedApp.reviewCount} reviews</span>
                  <span>{formatNumber(selectedApp.downloads)} downloads</span>
                  <span>{formatSize(selectedApp.size)}</span>
                </div>
                <div className="as-detail-actions">
                  {selectedApp.installStatus === "not-installed" && (
                    <button className={`as-install-btn ${!selectedApp.signatureValid ? "as-btn-blocked" : ""}`} onClick={() => installApp(selectedApp.id)} disabled={!selectedApp.signatureValid}>
                      {selectedApp.signatureValid ? "Install" : "⚠ Invalid Signature"}
                    </button>
                  )}
                  {selectedApp.installStatus === "installing" && (
                    <button className="as-install-btn as-btn-installing" disabled>Installing...</button>
                  )}
                  {selectedApp.installStatus === "installed" && (
                    <button className="as-uninstall-btn" onClick={() => uninstallApp(selectedApp.id)}>Uninstall</button>
                  )}
                  {selectedApp.installStatus === "update-available" && (
                    <button className="as-update-btn" onClick={() => updateApp(selectedApp.id)}>Update to v{selectedApp.version}</button>
                  )}
                </div>
              </div>
            </div>

            {/* signature */}
            <div className={`as-sig-panel ${selectedApp.signatureValid ? "as-sig-valid" : "as-sig-invalid"}`}>
              <span className="as-sig-icon">{selectedApp.signatureValid ? "🔒" : "⚠"}</span>
              <div className="as-sig-info">
                <div className="as-sig-status">{selectedApp.signatureValid ? "Ed25519 Signature Verified" : "SIGNATURE VERIFICATION FAILED"}</div>
                <div className="as-sig-hash">{selectedApp.signature}</div>
              </div>
            </div>

            {/* details grid */}
            <div className="as-detail-grid">
              <div className="as-detail-section">
                <div className="as-section-header">About</div>
                <p className="as-detail-desc">{selectedApp.longDescription}</p>
                {selectedApp.changelog && (
                  <div className="as-changelog">
                    <span className="as-changelog-label">Changelog:</span> {selectedApp.changelog}
                  </div>
                )}
              </div>

              <div className="as-detail-section">
                <div className="as-section-header">Details</div>
                <div className="as-detail-props">
                  <div className="as-prop"><span>Version</span><span>{selectedApp.version}</span></div>
                  {selectedApp.installedVersion && <div className="as-prop"><span>Installed</span><span>v{selectedApp.installedVersion}</span></div>}
                  <div className="as-prop"><span>Category</span><span>{selectedApp.category}</span></div>
                  <div className="as-prop"><span>Fuel Cost</span><span>⚡ {selectedApp.fuelCost}/op</span></div>
                  <div className="as-prop"><span>Autonomy</span><span>{selectedApp.autonomyLevel}</span></div>
                  <div className="as-prop"><span>Updated</span><span>{formatDate(selectedApp.lastUpdated)}</span></div>
                </div>

                <div className="as-section-header" style={{ marginTop: 14 }}>Capabilities Required</div>
                <div className="as-caps">
                  {selectedApp.capabilities.map(c => <span key={c} className="as-cap">{c}</span>)}
                </div>

                {selectedApp.dependencies.length > 0 && (
                  <>
                    <div className="as-section-header" style={{ marginTop: 14 }}>Dependencies</div>
                    <div className="as-deps">
                      {selectedApp.dependencies.map(d => {
                        const dep = apps.find(a => a.id === d);
                        return <span key={d} className={`as-dep ${dep?.installStatus === "installed" ? "as-dep-met" : "as-dep-missing"}`}>
                          {dep?.name ?? d} {dep?.installStatus === "installed" ? "✓" : "✗ missing"}
                        </span>;
                      })}
                    </div>
                  </>
                )}
              </div>
            </div>

            {/* screenshots */}
            {selectedApp.screenshots.length > 0 && (
              <div className="as-screenshots-section">
                <div className="as-section-header">Screenshots</div>
                <div className="as-screenshots">
                  {selectedApp.screenshots.map((s, i) => (
                    <div key={i} className="as-screenshot">
                      <div className="as-screenshot-img" style={{ background: s.gradient }} />
                      <div className="as-screenshot-label">{s.label}</div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* reviews */}
            <div className="as-reviews-section">
              <div className="as-reviews-header">
                <span className="as-section-header">Reviews ({selectedApp.reviewCount})</span>
                <button className="as-sm-btn" onClick={() => setShowReviewForm(!showReviewForm)}>Write Review</button>
              </div>

              {showReviewForm && (
                <div className="as-review-form">
                  <div className="as-review-stars">
                    {[1, 2, 3, 4, 5].map(n => (
                      <button key={n} className={`as-star-btn ${n <= reviewRating ? "active" : ""}`} onClick={() => setReviewRating(n)}>★</button>
                    ))}
                  </div>
                  <textarea className="as-review-input" value={reviewText} onChange={e => setReviewText(e.target.value)} placeholder="Write your review..." rows={3} />
                  <div className="as-review-form-actions">
                    <button className="as-sm-btn as-btn-primary" onClick={submitReview} disabled={!reviewText.trim()}>Submit</button>
                    <button className="as-sm-btn" onClick={() => setShowReviewForm(false)}>Cancel</button>
                  </div>
                </div>
              )}

              {selectedApp.reviews.map(r => (
                <div key={r.id} className="as-review">
                  <div className="as-review-header">
                    <span className="as-review-author">{r.author}</span>
                    <span className="as-review-rating">{renderStars(r.rating)}</span>
                    <span className="as-review-date">{formatDate(r.date)}</span>
                  </div>
                  <div className="as-review-text">{r.text}</div>
                  <div className="as-review-helpful">👍 {r.helpful} found helpful</div>
                </div>
              ))}
            </div>
          </div>
        ) : (
          /* ─── list / grid views ─── */
          <div className="as-list-view">
            {/* toolbar */}
            <div className="as-toolbar">
              <input className="as-search" placeholder="Search agents..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
              <select className="as-select" value={sortBy} onChange={e => setSortBy(e.target.value as typeof sortBy)}>
                <option value="popular">Most Popular</option>
                <option value="rating">Top Rated</option>
                <option value="recent">Recently Updated</option>
                <option value="name">Name</option>
              </select>
            </div>

            {/* featured banner */}
            {view === "featured" && selectedCategory === "all" && !searchQuery && (
              <div className="as-featured-section">
                <div className="as-featured-header">Featured Agents</div>
                <div className="as-featured-grid">
                  {featuredApps.map(app => (
                    <div key={app.id} className="as-featured-card" onClick={() => setSelectedAppId(app.id)} style={{ background: app.gradient }}>
                      <div className="as-featured-icon">{app.icon}</div>
                      <div className="as-featured-info">
                        <div className="as-featured-name">{app.name}</div>
                        <div className="as-featured-desc">{app.description}</div>
                        <div className="as-featured-meta">
                          <span>{renderStars(app.rating)} {app.rating}</span>
                          <span>{formatNumber(app.downloads)} ↓</span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* updates banner */}
            {view === "updates" && filteredApps.length === 0 && (
              <div className="as-empty">
                <div className="as-empty-icon">✓</div>
                <div>All agents are up to date</div>
              </div>
            )}

            {/* app grid */}
            <div className="as-app-grid">
              {filteredApps.map(app => (
                <div key={app.id} className="as-app-card" onClick={() => setSelectedAppId(app.id)}>
                  <div className="as-app-card-icon" style={{ background: app.gradient }}>{app.icon}</div>
                  <div className="as-app-card-body">
                    <div className="as-app-card-name">
                      {app.name}
                      {!app.signatureValid && <span className="as-sig-warn">⚠</span>}
                    </div>
                    <div className="as-app-card-dev">{app.developer}{app.developerVerified && " ✓"}</div>
                    <div className="as-app-card-desc">{app.description}</div>
                    <div className="as-app-card-footer">
                      <span className="as-app-card-rating">{renderStars(app.rating)} {app.rating}</span>
                      <span className="as-app-card-dl">{formatNumber(app.downloads)} ↓</span>
                      <span className="as-app-card-fuel">⚡{app.fuelCost}</span>
                      {app.installStatus === "installed" && <span className="as-installed-badge">Installed</span>}
                      {app.installStatus === "update-available" && <span className="as-update-avail-badge">Update</span>}
                    </div>
                  </div>
                </div>
              ))}
            </div>

            {/* publish view */}
            {view === "publish" && (
              <div className="as-publish">
                <h3 className="as-pub-title">⬆ Publish Agent to Store</h3>
                <div className="as-pub-form">
                  <div className="as-pub-field">
                    <label>Agent Name</label>
                    <input value={pubName} onChange={e => setPubName(e.target.value)} placeholder="My Agent" />
                  </div>
                  <div className="as-pub-field">
                    <label>Description</label>
                    <textarea value={pubDesc} onChange={e => setPubDesc(e.target.value)} placeholder="What does your agent do?" rows={3} />
                  </div>
                  <div className="as-pub-row">
                    <div className="as-pub-field">
                      <label>Category</label>
                      <select value={pubCategory} onChange={e => setPubCategory(e.target.value as Category)}>
                        {CATEGORIES.map(c => <option key={c.id} value={c.id}>{c.label}</option>)}
                      </select>
                    </div>
                    <div className="as-pub-field">
                      <label>Version</label>
                      <input value={pubVersion} onChange={e => setPubVersion(e.target.value)} placeholder="1.0.0" />
                    </div>
                    <div className="as-pub-field">
                      <label>Fuel Cost</label>
                      <input value={pubFuel} onChange={e => setPubFuel(e.target.value)} placeholder="50" type="number" />
                    </div>
                  </div>
                  <div className="as-pub-info">
                    <span>🔒</span> Your agent will be signed with your Ed25519 developer key. Governance checks will be run before listing.
                  </div>
                  <button className="as-pub-btn" onClick={handlePublish} disabled={!pubName.trim() || !pubDesc.trim()}>Publish Agent</button>
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* ─── Status Bar ─── */}
      <div className="as-status-bar">
        <span className="as-status-item">{view.charAt(0).toUpperCase() + view.slice(1)}</span>
        <span className="as-status-item">{filteredApps.length} agents</span>
        <span className="as-status-item">{apps.filter(a => a.installStatus === "installed" || a.installStatus === "update-available").length} installed</span>
        <span className="as-status-item">{apps.filter(a => a.installStatus === "update-available").length} updates</span>
        <span className="as-status-item as-status-right">⚡ {fuelUsed} fuel</span>
        <span className="as-status-item">{apps.filter(a => a.signatureValid).length}/{apps.length} verified</span>
      </div>
    </div>
  );
}
