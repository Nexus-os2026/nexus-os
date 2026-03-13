import { useState, useCallback, useMemo, useEffect } from "react";
import {
  marketplaceSearch,
  marketplaceInstall,
  marketplaceInfo,
  marketplacePublish,
  marketplaceMyAgents,
  hasDesktopRuntime,
} from "../api/backend";
import type { MarketplaceAgent, MarketplaceDetail, MarketplacePublishResult } from "../types";
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
  { id: "productivity", label: "Productivity", icon: "\u26A1" },
  { id: "security", label: "Security", icon: "\uD83D\uDEE1" },
  { id: "data", label: "Data", icon: "\uD83D\uDCCA" },
  { id: "social", label: "Social", icon: "\uD83D\uDCF1" },
  { id: "devtools", label: "Dev Tools", icon: "\u2328" },
  { id: "automation", label: "Automation", icon: "\u238B" },
  { id: "ai", label: "AI / ML", icon: "\u2726" },
  { id: "utilities", label: "Utilities", icon: "\uD83D\uDD27" },
];

const CATEGORY_ICONS: Record<string, string> = {
  productivity: "\u26A1",
  security: "\uD83D\uDEE1",
  data: "\uD83D\uDCCA",
  social: "\uD83D\uDCF1",
  devtools: "\u2328",
  automation: "\u238B",
  ai: "\u2726",
  utilities: "\uD83D\uDD27",
};

const CATEGORY_GRADIENTS: Record<string, string> = {
  productivity: "linear-gradient(135deg, #0f172a 0%, #fbbf24 100%)",
  security: "linear-gradient(135deg, #0f172a 0%, #ef4444 100%)",
  data: "linear-gradient(135deg, #0f172a 0%, #a78bfa 100%)",
  social: "linear-gradient(135deg, #0f172a 0%, #ec4899 100%)",
  devtools: "linear-gradient(135deg, #0f172a 0%, var(--nexus-accent) 100%)",
  automation: "linear-gradient(135deg, #0f172a 0%, #06b6d4 100%)",
  ai: "linear-gradient(135deg, #0f172a 0%, #4f46e5 100%)",
  utilities: "linear-gradient(135deg, #0f172a 0%, #64748b 100%)",
};

/* ─── helpers to map backend data to UI model ─── */
function inferCategory(agent: MarketplaceAgent): Category {
  const text = `${agent.name} ${agent.description} ${agent.tags.join(" ")}`.toLowerCase();
  if (text.includes("security") || text.includes("threat") || text.includes("monitor")) return "security";
  if (text.includes("code") || text.includes("test") || text.includes("bug") || text.includes("dev")) return "devtools";
  if (text.includes("data") || text.includes("pipeline") || text.includes("etl")) return "data";
  if (text.includes("social") || text.includes("post") || text.includes("schedule")) return "social";
  if (text.includes("automat") || text.includes("workflow")) return "automation";
  if (text.includes("ai") || text.includes("model") || text.includes("ml") || text.includes("llm")) return "ai";
  if (text.includes("backup") || text.includes("util")) return "utilities";
  return "productivity";
}

function marketplaceAgentToStoreApp(agent: MarketplaceAgent, index: number): StoreApp {
  const cat = inferCategory(agent);
  return {
    id: agent.package_id,
    name: agent.name,
    developer: agent.author,
    developerVerified: agent.downloads > 1000,
    description: agent.description,
    longDescription: agent.description,
    version: agent.version,
    category: cat,
    rating: agent.rating,
    reviewCount: agent.review_count,
    downloads: agent.downloads,
    size: 0,
    icon: CATEGORY_ICONS[cat] ?? "\u2B21",
    gradient: CATEGORY_GRADIENTS[cat] ?? "linear-gradient(135deg, #0f172a 0%, var(--nexus-accent) 100%)",
    screenshots: [],
    reviews: [],
    capabilities: agent.capabilities,
    dependencies: [],
    signature: "",
    signatureValid: true,
    installStatus: "not-installed",
    featured: index < 3,
    fuelCost: agent.price_cents > 0 ? agent.price_cents : 50,
    autonomyLevel: "L2",
    lastUpdated: Date.now() - 86400000 * (index + 1),
  };
}

function applyDetailToApp(app: StoreApp, detail: MarketplaceDetail): StoreApp {
  return {
    ...app,
    longDescription: detail.agent.description,
    rating: detail.agent.rating,
    reviewCount: detail.agent.review_count,
    downloads: detail.agent.downloads,
    version: detail.agent.version,
    capabilities: detail.agent.capabilities,
    reviews: detail.reviews.map((r, i) => ({
      id: `r-${i}`,
      author: r.reviewer,
      rating: r.stars,
      text: r.comment,
      date: new Date(r.created_at).getTime(),
      helpful: 0,
    })),
    changelog: detail.versions.length > 0
      ? `v${detail.versions[0].version}: ${detail.versions[0].changelog}`
      : undefined,
  };
}

/* ─── component ─── */
export default function AppStore() {
  const [view, setView] = useState<View>("featured");
  const [apps, setApps] = useState<StoreApp[]>([]);
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<Category | "all">("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [sortBy, setSortBy] = useState<"popular" | "rating" | "recent" | "name">("popular");
  const [fuelUsed, setFuelUsed] = useState(0);
  const [auditLog, setAuditLog] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [demoMode, setDemoMode] = useState(false);

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
  const [publishing, setPublishing] = useState(false);

  const selectedApp = useMemo(() => apps.find(a => a.id === selectedAppId) ?? null, [apps, selectedAppId]);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 30)), []);

  const isDesktop = hasDesktopRuntime();

  /* ─── load apps from backend on mount ─── */
  const loadApps = useCallback(async (query: string) => {
    if (!isDesktop) {
      setDemoMode(true);
      logAudit("Browser mode -- no backend");
      return;
    }
    setLoading(true);
    try {
      const results = await marketplaceSearch(query);
      if (results.length > 0) {
        setApps(results.map((a, i) => marketplaceAgentToStoreApp(a, i)));
        setDemoMode(false);
        logAudit(`Loaded ${results.length} agents from marketplace`);
      } else {
        setApps([]);
        setDemoMode(false);
        logAudit("Marketplace returned 0 agents");
      }
    } catch (err) {
      console.error("marketplace_search failed:", err);
      setDemoMode(true);
      logAudit("Backend unavailable -- demo mode");
    } finally {
      setLoading(false);
    }
  }, [isDesktop, logAudit]);

  useEffect(() => {
    loadApps("");
  }, [loadApps]);

  /* ─── load detail from backend when an app is selected ─── */
  const loadAppDetail = useCallback(async (appId: string) => {
    if (!isDesktop || demoMode) return;
    try {
      const detail = await marketplaceInfo(appId);
      setApps(prev => prev.map(a => a.id === appId ? applyDetailToApp(a, detail) : a));
    } catch (err) {
      console.error("marketplace_info failed:", err);
    }
  }, [isDesktop, demoMode]);

  const handleSelectApp = useCallback((appId: string) => {
    setSelectedAppId(appId);
    loadAppDetail(appId);
  }, [loadAppDetail]);

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
    if (bytes <= 0) return "--";
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  };

  const formatNumber = (n: number) => n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`;

  const formatDate = (ts: number) => new Date(ts).toLocaleDateString();

  const renderStars = (rating: number) => {
    const full = Math.floor(rating);
    const half = rating - full >= 0.5;
    return Array.from({ length: 5 }, (_, i) => {
      if (i < full) return "\u2605";
      if (i === full && half) return "\u2BE8";
      return "\u2606";
    }).join("");
  };

  /* ─── actions ─── */
  const installApp = useCallback(async (id: string) => {
    const app = apps.find(a => a.id === id);
    if (!app) return;
    if (!app.signatureValid) {
      logAudit(`BLOCKED: ${app.name} -- invalid signature (backend rejected)`);
      return;
    }
    setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installing" as InstallStatus } : a));
    logAudit(`Installing ${app.name} v${app.version}...`);

    if (!isDesktop || demoMode) {
      // fallback: simulate
      setTimeout(() => {
        setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installed", installedVersion: a.version, downloads: a.downloads + 1 } : a));
        logAudit(`Installed ${app.name} v${app.version} (demo)`);
      }, 1500);
      return;
    }

    try {
      const result = await marketplaceInstall(id);
      setApps(prev => prev.map(a => a.id === id ? {
        ...a,
        installStatus: "installed",
        installedVersion: result.version,
        downloads: result.downloads,
      } : a));
      setFuelUsed(f => f + 5);
      logAudit(`Installed ${app.name} v${result.version}`);
    } catch (err) {
      console.error("marketplace_install failed:", err);
      setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "not-installed" } : a));
      logAudit(`FAILED to install ${app.name}: ${err}`);
    }
  }, [apps, logAudit, isDesktop, demoMode]);

  const updateApp = useCallback(async (id: string) => {
    const app = apps.find(a => a.id === id);
    if (!app) return;
    setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installing" as InstallStatus } : a));
    logAudit(`Updating ${app.name} to v${app.version}...`);

    if (!isDesktop || demoMode) {
      setTimeout(() => {
        setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "installed", installedVersion: a.version } : a));
        logAudit(`Updated ${app.name} to v${app.version} (demo)`);
      }, 1500);
      return;
    }

    try {
      const result = await marketplaceInstall(id);
      setApps(prev => prev.map(a => a.id === id ? {
        ...a,
        installStatus: "installed",
        installedVersion: result.version,
      } : a));
      setFuelUsed(f => f + 3);
      logAudit(`Updated ${app.name} to v${result.version}`);
    } catch (err) {
      console.error("marketplace_install (update) failed:", err);
      setApps(prev => prev.map(a => a.id === id ? { ...a, installStatus: "update-available" } : a));
      logAudit(`FAILED to update ${app.name}: ${err}`);
    }
  }, [apps, logAudit, isDesktop, demoMode]);

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

  const handlePublish = useCallback(async () => {
    if (!pubName.trim() || !pubDesc.trim()) return;

    if (!isDesktop || demoMode) {
      logAudit(`Cannot publish in demo mode`);
      return;
    }

    setPublishing(true);
    const manifest = JSON.stringify({
      name: pubName,
      description: pubDesc,
      version: pubVersion,
      category: pubCategory,
      fuel_cost: parseInt(pubFuel) || 50,
      capabilities: ["llm.query"],
    });

    try {
      const result: MarketplacePublishResult = await marketplacePublish(manifest);
      logAudit(`Published: ${result.name} v${result.version} (${result.verdict})`);
      // Reload to show the new agent
      await loadApps("");
      setPubName(""); setPubDesc(""); setPubVersion("1.0.0");
      setFuelUsed(f => f + 10);
    } catch (err) {
      console.error("marketplace_publish failed:", err);
      logAudit(`FAILED to publish ${pubName}: ${err}`);
    } finally {
      setPublishing(false);
    }
  }, [pubName, pubDesc, pubVersion, pubCategory, pubFuel, logAudit, isDesktop, demoMode, loadApps]);

  const handleLoadMyAgents = useCallback(async () => {
    if (!isDesktop || demoMode) return;
    setLoading(true);
    try {
      const results = await marketplaceMyAgents("Suresh Karicheti");
      setApps(results.map((a, i) => ({
        ...marketplaceAgentToStoreApp(a, i),
        installStatus: "installed" as InstallStatus,
        installedVersion: a.version,
      })));
      logAudit(`Loaded ${results.length} of your published agents`);
    } catch (err) {
      console.error("marketplace_my_agents failed:", err);
      logAudit(`Failed to load your agents: ${err}`);
    } finally {
      setLoading(false);
    }
  }, [isDesktop, demoMode, logAudit]);

  /* ─── search handler ─── */
  useEffect(() => {
    if (!searchQuery && view !== "publish") return;
    const timer = setTimeout(() => {
      if (isDesktop && !demoMode && searchQuery.trim()) {
        loadApps(searchQuery);
      }
    }, 400);
    return () => clearTimeout(timer);
  }, [searchQuery, isDesktop, demoMode, loadApps, view]);

  /* ─── render ─── */
  return (
    <div className="as-container">
      {/* ─── Sidebar ─── */}
      <aside className="as-sidebar">
        <div className="as-sidebar-header">
          <h2 className="as-sidebar-title">App Store</h2>
          {demoMode && <span style={{ fontSize: 10, color: "var(--nexus-accent)", opacity: 0.7 }}>(Demo Data)</span>}
        </div>

        <div className="as-views">
          {([["featured", "\u25C8", "Featured"], ["browse", "\u2B21", "Browse All"], ["installed", "\u2713", "Installed"], ["updates", "\u2191", "Updates"], ["publish", "\u2B06", "Publish"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`as-view-btn ${view === id ? "active" : ""}`} onClick={() => {
              setView(id);
              setSelectedAppId(null);
              if (id === "installed") handleLoadMyAgents();
              if (id === "featured" || id === "browse") loadApps("");
            }}>
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
        {/* loading state */}
        {loading && (
          <div className="as-empty">
            <div className="as-empty-icon">\u25CC</div>
            <div>Loading marketplace...</div>
          </div>
        )}

        {/* demo mode banner */}
        {demoMode && !loading && (
          <div style={{
            padding: "8px 16px",
            background: "rgba(251, 191, 36, 0.1)",
            border: "1px solid rgba(251, 191, 36, 0.3)",
            borderRadius: 6,
            margin: "0 0 12px",
            fontSize: 12,
            color: "#fbbf24",
          }}>
            Desktop runtime not detected. Marketplace features require the Tauri backend. Showing empty state.
          </div>
        )}

        {/* app detail view */}
        {selectedApp ? (
          <div className="as-app-detail">
            <button className="as-back-btn" onClick={() => setSelectedAppId(null)}>\u2190 Back</button>

            <div className="as-detail-hero">
              <div className="as-detail-icon" style={{ background: selectedApp.gradient }}>{selectedApp.icon}</div>
              <div className="as-detail-info">
                <h2 className="as-detail-name">{selectedApp.name}</h2>
                <div className="as-detail-developer">
                  {selectedApp.developer}
                  {selectedApp.developerVerified && <span className="as-verified">\u2713 Verified</span>}
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
                      {selectedApp.signatureValid ? "Install" : "\u26A0 Blocked by Backend"}
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

            {/* signature -- status comes from backend */}
            <div className={`as-sig-panel ${selectedApp.signatureValid ? "as-sig-valid" : "as-sig-invalid"}`}>
              <span className="as-sig-icon">{selectedApp.signatureValid ? "\uD83D\uDD12" : "\u26A0"}</span>
              <div className="as-sig-info">
                <div className="as-sig-status">{selectedApp.signatureValid ? "Signature Verified by Backend" : "SIGNATURE VERIFICATION FAILED"}</div>
                {selectedApp.signature && <div className="as-sig-hash">{selectedApp.signature}</div>}
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
                  <div className="as-prop"><span>Fuel Cost</span><span>\u26A1 {selectedApp.fuelCost}/op</span></div>
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
                          {dep?.name ?? d} {dep?.installStatus === "installed" ? "\u2713" : "\u2717 missing"}
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
                      <button key={n} className={`as-star-btn ${n <= reviewRating ? "active" : ""}`} onClick={() => setReviewRating(n)}>\u2605</button>
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
                  <div className="as-review-helpful">\uD83D\uDC4D {r.helpful} found helpful</div>
                </div>
              ))}
            </div>
          </div>
        ) : !loading && (
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
            {view === "featured" && selectedCategory === "all" && !searchQuery && featuredApps.length > 0 && (
              <div className="as-featured-section">
                <div className="as-featured-header">Featured Agents</div>
                <div className="as-featured-grid">
                  {featuredApps.map(app => (
                    <div key={app.id} className="as-featured-card" onClick={() => handleSelectApp(app.id)} style={{ background: app.gradient }}>
                      <div className="as-featured-icon">{app.icon}</div>
                      <div className="as-featured-info">
                        <div className="as-featured-name">{app.name}</div>
                        <div className="as-featured-desc">{app.description}</div>
                        <div className="as-featured-meta">
                          <span>{renderStars(app.rating)} {app.rating}</span>
                          <span>{formatNumber(app.downloads)} \u2193</span>
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
                <div className="as-empty-icon">\u2713</div>
                <div>All agents are up to date</div>
              </div>
            )}

            {/* empty state */}
            {!demoMode && filteredApps.length === 0 && view !== "updates" && view !== "publish" && (
              <div className="as-empty">
                <div className="as-empty-icon">\u2B21</div>
                <div>No agents found</div>
              </div>
            )}

            {/* app grid */}
            <div className="as-app-grid">
              {filteredApps.map(app => (
                <div key={app.id} className="as-app-card" onClick={() => handleSelectApp(app.id)}>
                  <div className="as-app-card-icon" style={{ background: app.gradient }}>{app.icon}</div>
                  <div className="as-app-card-body">
                    <div className="as-app-card-name">
                      {app.name}
                      {!app.signatureValid && <span className="as-sig-warn">\u26A0</span>}
                    </div>
                    <div className="as-app-card-dev">{app.developer}{app.developerVerified && " \u2713"}</div>
                    <div className="as-app-card-desc">{app.description}</div>
                    <div className="as-app-card-footer">
                      <span className="as-app-card-rating">{renderStars(app.rating)} {app.rating}</span>
                      <span className="as-app-card-dl">{formatNumber(app.downloads)} \u2193</span>
                      <span className="as-app-card-fuel">\u26A1{app.fuelCost}</span>
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
                <h3 className="as-pub-title">\u2B06 Publish Agent to Store</h3>
                {demoMode && (
                  <div style={{ padding: "8px 12px", background: "rgba(251, 191, 36, 0.1)", border: "1px solid rgba(251, 191, 36, 0.3)", borderRadius: 6, marginBottom: 12, fontSize: 12, color: "#fbbf24" }}>
                    Publishing requires the Tauri desktop runtime.
                  </div>
                )}
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
                    <span>\uD83D\uDD12</span> Your agent will be signed and verified by the backend governance pipeline before listing.
                  </div>
                  <button className="as-pub-btn" onClick={handlePublish} disabled={!pubName.trim() || !pubDesc.trim() || demoMode || publishing}>
                    {publishing ? "Publishing..." : "Publish Agent"}
                  </button>
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
        <span className="as-status-item as-status-right">\u26A1 {fuelUsed} fuel</span>
        <span className="as-status-item">{apps.filter(a => a.signatureValid).length}/{apps.length} verified</span>
        {demoMode && <span className="as-status-item" style={{ color: "#fbbf24" }}>(Demo)</span>}
      </div>
    </div>
  );
}
