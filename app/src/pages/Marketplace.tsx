import { useEffect, useMemo, useState, useCallback } from "react";
import {
  marketplaceSearch,
  marketplaceInstall,
  marketplaceInfo,
  hasDesktopRuntime,
} from "../api/backend";
import type { MarketplaceAgent, MarketplaceDetail } from "../types";
import "./marketplace.css";

// Add category to MarketplaceAgent for filtering since backend doesn't have it
interface AgentWithCategory extends MarketplaceAgent {
  category?: string;
}


const CATEGORIES = ["All", "Coding", "Content", "Social", "Analytics", "Productivity", "Research"];

function formatInstalls(count: number): string {
  if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
  return String(count);
}

function renderStars(rating: number): string {
  const full = Math.floor(rating);
  const half = rating - full >= 0.5;
  return "\u2605".repeat(full) + (half ? "\u00BD" : "") + "\u2606".repeat(5 - full - (half ? 1 : 0));
}

function inferCategory(agent: MarketplaceAgent): string {
  const text = `${agent.name} ${agent.description} ${agent.tags.join(" ")}`.toLowerCase();
  if (text.includes("code") || text.includes("bug") || text.includes("git") || text.includes("pr")) return "Coding";
  if (text.includes("content") || text.includes("blog") || text.includes("seo")) return "Content";
  if (text.includes("social") || text.includes("twitter") || text.includes("post")) return "Social";
  if (text.includes("analytics") || text.includes("monitor") || text.includes("track")) return "Analytics";
  if (text.includes("email") || text.includes("meeting") || text.includes("slack") || text.includes("product")) return "Productivity";
  if (text.includes("scrape") || text.includes("research") || text.includes("data")) return "Research";
  return "Productivity";
}

export function Marketplace(): JSX.Element {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("All");
  const [agents, setAgents] = useState<AgentWithCategory[]>([]);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState<Set<string>>(new Set());
  const [selectedAgent, setSelectedAgent] = useState<MarketplaceDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [backendAvailable, setBackendAvailable] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isDesktop = hasDesktopRuntime();

  /* ─── Load agents from backend ─── */
  const loadAgents = useCallback(async (searchQuery: string) => {
    if (!isDesktop) {
      setAgents([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const results = await marketplaceSearch(searchQuery || "");
      setBackendAvailable(true);
      setAgents(results.map(a => ({ ...a, category: inferCategory(a) })));
    } catch (e) {
      // Backend unavailable — use fallback templates only in non-desktop mode
      if (!isDesktop) {
        setAgents([]);
      } else {
        setAgents([]);
        setError(`Marketplace unavailable: ${e}`);
      }
    }
    setLoading(false);
  }, [isDesktop]);

  useEffect(() => {
    loadAgents("");
  }, [loadAgents]);

  /* ─── Search with debounce ─── */
  useEffect(() => {
    if (!isDesktop || !backendAvailable) return;
    const timer = setTimeout(() => {
      if (query.length > 0) loadAgents(query);
    }, 300);
    return () => clearTimeout(timer);
  }, [query, isDesktop, backendAvailable, loadAgents]);

  /* ─── Install agent ─── */
  const handleInstall = useCallback(async (packageId: string) => {
    if (installed.has(packageId)) return;

    if (!isDesktop) {
      // State-only install for non-desktop
      setInstalled(prev => new Set(prev).add(packageId));
      return;
    }

    setInstalling(prev => new Set(prev).add(packageId));
    try {
      await marketplaceInstall(packageId);
      setInstalled(prev => new Set(prev).add(packageId));
    } catch (e) {
      setError(`Install failed: ${e}`);
    }
    setInstalling(prev => {
      const next = new Set(prev);
      next.delete(packageId);
      return next;
    });
  }, [installed, isDesktop, backendAvailable]);

  /* ─── View agent details ─── */
  const viewDetails = useCallback(async (packageId: string) => {
    if (!isDesktop) return;
    try {
      const detail = await marketplaceInfo(packageId);
      setSelectedAgent(detail);
    } catch {
      // silently fail — detail not available
    }
  }, [isDesktop]);

  const filtered = useMemo(() => {
    const q = query.toLowerCase();
    return agents.filter((t) => {
      if (category !== "All" && t.category !== category) return false;
      if (q.length === 0) return true;
      return t.name.toLowerCase().includes(q) || t.description.toLowerCase().includes(q);
    });
  }, [agents, query, category]);

  return (
    <section className="mp-hub">
      <header className="mp-header">
        <div>
          <h2 className="mp-title">
            AGENT MARKETPLACE // {agents.length} AVAILABLE
            {backendAvailable && <span style={{ fontSize: "0.7rem", marginLeft: "1rem", color: "#22c55e" }}>LIVE</span>}
          </h2>
          <p className="mp-subtitle">
            {backendAvailable
              ? "Browsing marketplace SQLite registry \u2014 Ed25519 verified agents"
              : isDesktop
                ? "Marketplace registry connected \u2014 publish agents with `nexus package`"
                : "Connect to desktop runtime for live marketplace"}
          </p>
        </div>
      </header>

      {error && (
        <div style={{ padding: "0.75rem 1rem", margin: "0 1rem", borderRadius: 8, background: "#ef444422", border: "1px solid #ef444444", color: "#ef4444", fontSize: "0.85rem" }}>
          {error}
          <button onClick={() => setError(null)} style={{ float: "right", background: "none", border: "none", color: "#ef4444", cursor: "pointer" }}>dismiss</button>
        </div>
      )}

      <div className="mp-toolbar">
        <div className="mp-search-wrap">
          <span className="mp-search-icon">&#x1F50D;</span>
          <input
            className="mp-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={backendAvailable ? "Search marketplace..." : "Filter templates..."}
          />
        </div>
        <div className="mp-categories">
          {CATEGORIES.map((cat) => (
            <button
              key={cat}
              type="button"
              className={`mp-cat-btn ${category === cat ? "active" : ""}`}
              onClick={() => setCategory(cat)}
            >
              {cat}
            </button>
          ))}
        </div>
      </div>

      {/* ─── Detail overlay ─── */}
      {selectedAgent && (
        <div style={{
          position: "fixed", inset: 0, background: "#000000cc", zIndex: 100,
          display: "flex", alignItems: "center", justifyContent: "center",
        }} onClick={() => setSelectedAgent(null)}>
          <div style={{
            background: "#0f172a", borderRadius: 16, padding: "2rem", maxWidth: 600, width: "90%",
            border: "1px solid #1e293b",
          }} onClick={e => e.stopPropagation()}>
            <h3 style={{ color: "#e2e8f0", marginBottom: "0.5rem" }}>{selectedAgent.agent.name}</h3>
            <p style={{ color: "#94a3b8", fontSize: "0.9rem" }}>{selectedAgent.agent.description}</p>
            <div style={{ display: "flex", gap: "1rem", marginTop: "1rem", fontSize: "0.85rem", color: "#64748b" }}>
              <span>v{selectedAgent.agent.version}</span>
              <span>by {selectedAgent.agent.author}</span>
              <span>{renderStars(selectedAgent.agent.rating)} {selectedAgent.agent.rating.toFixed(1)}</span>
              <span>{formatInstalls(selectedAgent.agent.downloads)} downloads</span>
            </div>
            {selectedAgent.agent.capabilities.length > 0 && (
              <div style={{ marginTop: "1rem" }}>
                <div style={{ color: "#94a3b8", fontSize: "0.8rem", marginBottom: "0.5rem" }}>Capabilities:</div>
                <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
                  {selectedAgent.agent.capabilities.map(c => (
                    <span key={c} style={{ padding: "0.2rem 0.5rem", background: "#1e293b", borderRadius: 4, fontSize: "0.75rem", color: "var(--nexus-accent)" }}>{c}</span>
                  ))}
                </div>
              </div>
            )}
            {selectedAgent.reviews.length > 0 && (
              <div style={{ marginTop: "1rem" }}>
                <div style={{ color: "#94a3b8", fontSize: "0.8rem", marginBottom: "0.5rem" }}>Reviews:</div>
                {selectedAgent.reviews.slice(0, 3).map((r, i) => (
                  <div key={i} style={{ padding: "0.5rem", borderBottom: "1px solid #1e293b", fontSize: "0.85rem" }}>
                    <span style={{ color: "#f59e0b" }}>{"★".repeat(r.stars)}</span>{" "}
                    <span style={{ color: "#94a3b8" }}>{r.reviewer}</span>
                    <div style={{ color: "#e2e8f0", marginTop: "0.25rem" }}>{r.comment}</div>
                  </div>
                ))}
              </div>
            )}
            {selectedAgent.versions.length > 0 && (
              <div style={{ marginTop: "1rem" }}>
                <div style={{ color: "#94a3b8", fontSize: "0.8rem", marginBottom: "0.5rem" }}>Version History:</div>
                {selectedAgent.versions.slice(0, 3).map((v, i) => (
                  <div key={i} style={{ padding: "0.25rem 0", fontSize: "0.8rem", color: "#64748b" }}>
                    v{v.version} — {v.changelog || "No changelog"}
                  </div>
                ))}
              </div>
            )}
            <div style={{ marginTop: "1.5rem", display: "flex", gap: "0.5rem" }}>
              <button
                className={`mp-install-btn ${installed.has(selectedAgent.agent.package_id) ? "done" : ""}`}
                onClick={() => handleInstall(selectedAgent.agent.package_id)}
              >
                {installed.has(selectedAgent.agent.package_id) ? "Installed \u2713" : "Install"}
              </button>
              <button
                style={{ background: "#1e293b", border: "1px solid #334155", color: "#94a3b8", padding: "0.5rem 1rem", borderRadius: 8, cursor: "pointer" }}
                onClick={() => setSelectedAgent(null)}
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="mp-grid">
        {loading ? (
          <p className="mp-empty">Loading marketplace...</p>
        ) : filtered.length === 0 ? (
          <p className="mp-empty">
            {query ? "No agents match your search." : "No agents published yet. Use `nexus package` + `nexus marketplace publish` to add agents."}
          </p>
        ) : (
          filtered.map((t) => {
            const isInstalled = installed.has(t.package_id);
            const isInstalling = installing.has(t.package_id);
            return (
              <article
                key={t.package_id}
                className={`mp-card ${isInstalled ? "installed" : ""}`}
                onClick={() => viewDetails(t.package_id)}
                style={{ cursor: backendAvailable ? "pointer" : "default" }}
              >
                <span className="mp-card-cat-tag">{t.category}</span>
                <h3 className="mp-card-name">{t.name}</h3>
                <p className="mp-card-desc">{t.description}</p>
                <p className="mp-card-author">by {t.author} · v{t.version}</p>
                {t.capabilities && t.capabilities.length > 0 && (
                  <div style={{ display: "flex", gap: "0.25rem", flexWrap: "wrap", marginTop: "0.5rem" }}>
                    {t.capabilities.slice(0, 3).map(c => (
                      <span key={c} style={{ padding: "0.1rem 0.4rem", background: "#0f172a", borderRadius: 4, fontSize: "0.7rem", color: "var(--nexus-accent)", border: "1px solid var(--nexus-accent)33" }}>{c}</span>
                    ))}
                    {t.capabilities.length > 3 && (
                      <span style={{ padding: "0.1rem 0.4rem", fontSize: "0.7rem", color: "#64748b" }}>+{t.capabilities.length - 3}</span>
                    )}
                  </div>
                )}
                <div className="mp-card-footer">
                  <div className="mp-card-stats">
                    <span className="mp-card-stars">{renderStars(t.rating)} {t.rating.toFixed(1)}</span>
                    <span className="mp-card-downloads">{formatInstalls(t.downloads)} installs</span>
                  </div>
                  <button
                    type="button"
                    className={`mp-install-btn ${isInstalled ? "done" : ""}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (!isInstalled && !isInstalling) handleInstall(t.package_id);
                    }}
                  >
                    {isInstalling ? "Installing..." : isInstalled ? "Installed \u2713" : "Install"}
                  </button>
                </div>
              </article>
            );
          })
        )}
      </div>

      {/* Status footer */}
      <div style={{
        padding: "0.5rem 1rem", borderTop: "1px solid #1e293b", display: "flex",
        justifyContent: "space-between", fontSize: "0.75rem", color: "#64748b",
      }}>
        <span>{filtered.length} agents shown · {installed.size} installed</span>
        <span>{isDesktop ? "SQLite registry connected" : "Fallback mode \u2014 desktop runtime needed"}</span>
      </div>
    </section>
  );
}
