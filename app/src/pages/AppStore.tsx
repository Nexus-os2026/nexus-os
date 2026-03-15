import { useDeferredValue, useEffect, useMemo, useState } from "react";
import {
  getPreinstalledAgents,
  hasDesktopRuntime,
  marketplaceInstall,
  marketplaceSearch,
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
      console.error("agent store load failed", loadError);
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
      console.error("agent start failed", startError);
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
      console.error("marketplace install failed", installError);
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
            <button
              key={level}
              type="button"
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
                      <button
                        type="button"
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
                  <button
                    type="button"
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
      </div>
    </section>
  );
}
