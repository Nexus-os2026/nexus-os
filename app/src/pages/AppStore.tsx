import { useCallback, useDeferredValue, useEffect, useMemo, useState } from "react";
import {
  getPreinstalledAgents,
  hasDesktopRuntime,
  marketplaceInstall,
  marketplaceSearch,
  marketplaceSearchGitlab,
  startAgent,
} from "../api/backend";
import type { MarketplaceAgent, PreinstalledAgent } from "../types";
import "./app-store.css";

type LevelFilter = "All" | "L1" | "L2" | "L3" | "L4" | "L5" | "L6";

const LEVEL_FILTERS: LevelFilter[] = ["All", "L1", "L2", "L3", "L4", "L5", "L6"];

function firstSentences(text: string, count: number): string {
  const parts = text
    .replace(/\s+/g, " ")
    .trim()
    .split(/(?<=[.!?])\s+/)
    .filter(Boolean);
  if (parts.length === 0) {
    return text.trim();
  }
  return parts.slice(0, count).join(" ");
}

function normalize(text: string): string {
  return text.trim().toLowerCase();
}

function matchesSearch(
  query: string,
  name: string,
  description: string,
  capabilities: string[],
): boolean {
  if (!query) {
    return true;
  }
  const haystack = `${name} ${description} ${capabilities.join(" ")}`.toLowerCase();
  return haystack.includes(query);
}

function matchesLevel(filter: LevelFilter, level: string | null | undefined): boolean {
  return filter === "All" || level === filter;
}

function levelLabel(level: number): string {
  return `L${level}`;
}

export default function AppStore(): JSX.Element {
  const [preinstalledAgents, setPreinstalledAgents] = useState<PreinstalledAgent[]>([]);
  const [marketplaceAgents, setMarketplaceAgents] = useState<MarketplaceAgent[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [levelFilter, setLevelFilter] = useState<LevelFilter>("All");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [startingIds, setStartingIds] = useState<string[]>([]);
  const [installingIds, setInstallingIds] = useState<string[]>([]);
  const [gitlabAgents, setGitlabAgents] = useState<any[]>([]);
  const [gitlabSearching, setGitlabSearching] = useState(false);
  const [activeTab, setActiveTab] = useState<'installed' | 'community'>('installed');

  const deferredQuery = useDeferredValue(normalize(searchQuery));
  const isDesktop = hasDesktopRuntime();

  async function loadStore(): Promise<void> {
    if (!isDesktop) {
      setLoading(false);
      setError("Agent Store requires the desktop runtime.");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const [preinstalled, marketplace] = await Promise.all([
        getPreinstalledAgents(),
        marketplaceSearch(""),
      ]);
      setPreinstalledAgents(preinstalled);
      setMarketplaceAgents(
        marketplace.filter(
          (agent) => !agent.tags.includes("prebuilt") && agent.author !== "nexus-os",
        ),
      );
    } catch (loadError) {
      if (import.meta.env.DEV) console.error("agent store load failed", loadError);
      setError(String(loadError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadStore();
  }, []);

  const filteredPreinstalled = useMemo(
    () =>
      preinstalledAgents.filter((agent) => {
        const level = levelLabel(agent.autonomy_level);
        return (
          matchesLevel(levelFilter, level) &&
          matchesSearch(deferredQuery, agent.name, agent.description, agent.capabilities)
        );
      }),
    [deferredQuery, levelFilter, preinstalledAgents],
  );

  const filteredMarketplace = useMemo(
    () =>
      marketplaceAgents.filter((agent) => {
        return (
          matchesLevel(levelFilter, agent.autonomy_level) &&
          matchesSearch(deferredQuery, agent.name, agent.description, agent.capabilities)
        );
      }),
    [deferredQuery, levelFilter, marketplaceAgents],
  );

  const groupedPreinstalled = useMemo(
    () =>
      LEVEL_FILTERS.slice(1)
        .filter((level) => levelFilter === "All" || levelFilter === level)
        .map((level) => ({
          level,
          agents: filteredPreinstalled.filter(
            (agent) => levelLabel(agent.autonomy_level) === level,
          ),
        }))
        .filter((group) => group.agents.length > 0),
    [filteredPreinstalled, levelFilter],
  );

  const handleGitlabSearch = useCallback(async (query: string) => {
    if (!query.trim()) return;
    setGitlabSearching(true);
    try {
      const raw = await marketplaceSearchGitlab(query);
      const agents = JSON.parse(raw);
      setGitlabAgents(agents);
    } catch {
      setGitlabAgents([]);
    } finally {
      setGitlabSearching(false);
    }
  }, []);

  async function handleStart(agentId: string): Promise<void> {
    if (!agentId) {
      return;
    }
    setStartingIds((current) => [...current, agentId]);
    setError(null);
    try {
      await startAgent(agentId);
      await loadStore();
    } catch (startError) {
      if (import.meta.env.DEV) console.error("agent start failed", startError);
      setError(String(startError));
    } finally {
      setStartingIds((current) => current.filter((id) => id !== agentId));
    }
  }

  async function handleInstall(packageId: string): Promise<void> {
    setInstallingIds((current) => [...current, packageId]);
    setError(null);
    try {
      await marketplaceInstall(packageId);
      await loadStore();
    } catch (installError) {
      if (import.meta.env.DEV) console.error("marketplace install failed", installError);
      setError(String(installError));
    } finally {
      setInstallingIds((current) => current.filter((id) => id !== packageId));
    }
  }

  return (
    <section className="as-page">
      <header className="as-hero">
        <div>
          <p className="as-kicker">Agent Store</p>
          <h1 className="as-title">Unified runtime + community marketplace</h1>
          <p className="as-subtitle">
            Pre-installed agents come from `agents/prebuilt/`. Community agents come from the
            marketplace SQLite registry.
          </p>
        </div>
        <div className="as-hero-stats">
          <div className="as-stat">
            <span className="as-stat-value">{preinstalledAgents.length}</span>
            <span className="as-stat-label">Pre-installed</span>
          </div>
          <div className="as-stat">
            <span className="as-stat-value">{marketplaceAgents.length}</span>
            <span className="as-stat-label">Community</span>
          </div>
        </div>
      </header>

      <div className="as-toolbar">
        <input
          className="as-search"
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
          placeholder="Search by name, description, or capability..."
        />
        <div className="as-filter-row">
          {LEVEL_FILTERS.map((level) => (
            <button type="button"
              key={level}
              className={`as-filter-btn ${levelFilter === level ? "active" : ""}`}
              onClick={() => setLevelFilter(level)}
            >
              {level}
            </button>
          ))}
        </div>
      </div>

      {error ? <div className="as-banner as-banner-error">{error}</div> : null}
      {!isDesktop ? (
        <div className="as-banner">Desktop runtime unavailable, so live agent data cannot be loaded.</div>
      ) : null}

      <div className="as-sections">
        <section className="as-section">
          <div className="as-section-head">
            <div>
              <p className="as-section-kicker">Section A</p>
              <h2 className="as-section-title">PRE-INSTALLED AGENTS</h2>
            </div>
            <p className="as-section-note">
              {filteredPreinstalled.length} matching agent
              {filteredPreinstalled.length === 1 ? "" : "s"}
            </p>
          </div>

          {loading ? <p className="as-empty">Loading pre-installed agents...</p> : null}
          {!loading && groupedPreinstalled.length === 0 ? (
            <p className="as-empty">No pre-installed agents match the current search or autonomy filter.</p>
          ) : null}

          {groupedPreinstalled.map((group) => (
            <div key={group.level} className="as-group">
              <div className="as-group-head">
                <span className="as-level-pill">{group.level}</span>
                <span className="as-group-count">
                  {group.agents.length} agent{group.agents.length === 1 ? "" : "s"}
                </span>
              </div>
              <div className="as-grid">
                {group.agents.map((agent) => {
                  const isRunning = agent.status === "Running" || agent.status === "Starting";
                  const isStarting = startingIds.includes(agent.agent_id);
                  return (
                    <article key={`${agent.name}-${agent.agent_id}`} className="as-card">
                      <div className="as-card-head">
                        <div>
                          <h3 className="as-card-title">{agent.name}</h3>
                          <p className="as-card-status">{agent.status}</p>
                        </div>
                        <span className="as-level-pill">{levelLabel(agent.autonomy_level)}</span>
                      </div>
                      <p className="as-card-description">
                        {firstSentences(agent.description, 2)}
                      </p>
                      <div className="as-cap-list">
                        {agent.capabilities.map((capability) => (
                          <span key={capability} className="as-chip">
                            {capability}
                          </span>
                        ))}
                      </div>
                      <div className="as-meta">
                        <span>Fuel budget: {agent.fuel_budget.toLocaleString()}</span>
                        <span>Schedule: {agent.schedule ?? "On demand"}</span>
                      </div>
                      <button type="button"
                        className="as-action-btn"
                        onClick={() => void handleStart(agent.agent_id)}
                        disabled={!agent.agent_id || isRunning || isStarting}
                      >
                        {isStarting ? "Starting..." : "Start"}
                      </button>
                    </article>
                  );
                })}
              </div>
            </div>
          ))}
        </section>

        <section className="as-section">
          <div className="as-section-head">
            <div>
              <p className="as-section-kicker">Section B</p>
              <h2 className="as-section-title">MARKETPLACE</h2>
            </div>
            <p className="as-section-note">
              {filteredMarketplace.length} matching agent
              {filteredMarketplace.length === 1 ? "" : "s"}
            </p>
          </div>

          {loading ? <p className="as-empty">Loading marketplace agents...</p> : null}
          {!loading && marketplaceAgents.length === 0 ? (
            <p className="as-empty">
              No community agents published yet. Use `nexus publish` to share your agents.
            </p>
          ) : null}
          {!loading && marketplaceAgents.length > 0 && filteredMarketplace.length === 0 ? (
            <p className="as-empty">No community agents match the current search or autonomy filter.</p>
          ) : null}

          <div className="as-grid">
            {filteredMarketplace.map((agent) => {
              const isInstalling = installingIds.includes(agent.package_id);
              return (
                <article key={agent.package_id} className="as-card">
                  <div className="as-card-head">
                    <div>
                      <h3 className="as-card-title">{agent.name}</h3>
                      <p className="as-card-status">by {agent.author}</p>
                    </div>
                    {agent.autonomy_level ? (
                      <span className="as-level-pill">{agent.autonomy_level}</span>
                    ) : null}
                  </div>
                  <p className="as-card-description">{firstSentences(agent.description, 2)}</p>
                  <div className="as-cap-list">
                    {agent.capabilities.map((capability) => (
                      <span key={capability} className="as-chip">
                        {capability}
                      </span>
                    ))}
                  </div>
                  <div className="as-meta">
                    <span>Version: {agent.version}</span>
                    <span>Downloads: {agent.downloads.toLocaleString()}</span>
                    {agent.fuel_budget ? <span>Fuel budget: {agent.fuel_budget.toLocaleString()}</span> : null}
                    <span>Schedule: {agent.schedule ?? "On demand"}</span>
                  </div>
                  <button type="button"
                    className="as-action-btn as-action-btn-secondary"
                    onClick={() => void handleInstall(agent.package_id)}
                    disabled={isInstalling}
                  >
                    {isInstalling ? "Installing..." : "Install"}
                  </button>
                </article>
              );
            })}
          </div>
        </section>

        {/* ── Community Agents from GitLab ── */}
        <div style={{ marginTop: 24 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 12 }}>
            <button type="button" className="cursor-pointer" onClick={() => setActiveTab('installed')} style={{ padding: "6px 16px", background: activeTab === 'installed' ? "rgba(129,140,248,0.2)" : "transparent", border: `1px solid ${activeTab === 'installed' ? "rgba(129,140,248,0.4)" : "var(--border, #334155)"}`, borderRadius: 6, color: "var(--text-primary, #e2e8f0)", fontSize: "0.8rem", fontFamily: "inherit", cursor: "pointer" }}>Pre-installed</button>
            <button type="button" className="cursor-pointer" onClick={() => setActiveTab('community')} style={{ padding: "6px 16px", background: activeTab === 'community' ? "rgba(129,140,248,0.2)" : "transparent", border: `1px solid ${activeTab === 'community' ? "rgba(129,140,248,0.4)" : "var(--border, #334155)"}`, borderRadius: 6, color: "var(--text-primary, #e2e8f0)", fontSize: "0.8rem", fontFamily: "inherit", cursor: "pointer" }}>Community (GitLab)</button>
          </div>

          {activeTab === 'community' && (
            <div>
              <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
                <input
                  placeholder="Search nexus-agent repos on GitLab..."
                  style={{ flex: 1, background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 6, color: "var(--text-primary)", padding: "8px 12px", fontSize: "0.85rem", fontFamily: "inherit" }}
                  onKeyDown={e => e.key === 'Enter' && handleGitlabSearch((e.target as HTMLInputElement).value)}
                />
                <button type="button" className="cursor-pointer" onClick={() => handleGitlabSearch("nexus agent")} disabled={gitlabSearching} style={{ padding: "8px 16px", background: "rgba(129,140,248,0.2)", border: "1px solid rgba(129,140,248,0.3)", borderRadius: 6, color: "#818cf8", fontSize: "0.85rem", fontFamily: "inherit", cursor: "pointer" }}>{gitlabSearching ? "Searching..." : "Search GitLab"}</button>
              </div>
              {gitlabAgents.length === 0 && !gitlabSearching && (
                <div style={{ padding: 24, textAlign: "center", opacity: 0.5, fontSize: "0.85rem" }}>Search GitLab for community agents tagged "nexus-agent". Or publish your own with `nexus publish`.</div>
              )}
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))", gap: 12 }}>
                {gitlabAgents.map((agent: any) => (
                  <div key={agent.id} style={{ background: "var(--bg-secondary, #1e293b)", border: "1px solid var(--border, #334155)", borderRadius: 8, padding: 16 }}>
                    <div style={{ fontWeight: 600, marginBottom: 4 }}>{agent.name}</div>
                    <div style={{ fontSize: "0.75rem", opacity: 0.6, marginBottom: 8 }}>{agent.description?.slice(0, 120) || "Community agent"}</div>
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "0.7rem" }}>
                      <span style={{ opacity: 0.5 }}>by {agent.author} · {agent.stars} stars</span>
                      <div style={{ display: "flex", gap: 8 }}>
                        <button type="button" className="cursor-pointer" onClick={() => void handleInstall(agent.name)} style={{ padding: "4px 12px", background: "rgba(34,211,238,0.15)", border: "1px solid rgba(34,211,238,0.3)", borderRadius: 4, color: "#22d3ee", fontSize: "0.7rem", cursor: "pointer", fontFamily: "inherit" }}>Install</button>
                        <a href={agent.url} target="_blank" rel="noopener noreferrer" style={{ color: "#818cf8", textDecoration: "none", padding: "4px 0" }}>View</a>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
