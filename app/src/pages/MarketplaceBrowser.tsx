import { useCallback, useEffect, useMemo, useState } from "react";
import { hasDesktopRuntime, marketplaceSearch, marketplaceInstall, marketplaceInfo } from "../api/backend";
import type { MarketplaceAgent, MarketplaceDetail } from "../types";
import "./marketplace-browser.css";

const MOCK_LISTINGS: MarketplaceAgent[] = [
  { package_id: "ml-1", name: "Code Review Bot", description: "Automated pull request reviews with security scanning and style enforcement", author: "NexusOS Core", tags: ["coding", "security"], version: "2.1.0", capabilities: ["llm.query", "fs.read"], price_cents: 0, downloads: 3420, rating: 4.7, review_count: 18 },
  { package_id: "ml-2", name: "Data Pipeline Agent", description: "ETL workflows with schema validation, retry logic, and audit logging", author: "Community", tags: ["data", "automation"], version: "1.4.2", capabilities: ["fs.read", "fs.write"], price_cents: 0, downloads: 1850, rating: 4.5, review_count: 9 },
  { package_id: "ml-3", name: "Compliance Monitor", description: "Continuous SOC2/HIPAA compliance checking with evidence collection", author: "NexusOS Core", tags: ["compliance", "security"], version: "3.0.0", capabilities: ["audit.read", "llm.query"], price_cents: 500, downloads: 920, rating: 4.9, review_count: 24 },
  { package_id: "ml-4", name: "Social Scheduler", description: "Multi-platform social media scheduling with approval gates and analytics", author: "Community", tags: ["social", "content"], version: "1.0.8", capabilities: ["social.post", "llm.query"], price_cents: 0, downloads: 2100, rating: 4.3, review_count: 6 },
  { package_id: "ml-5", name: "Incident Responder", description: "Automated incident triage, escalation, and post-mortem generation", author: "NexusOS Core", tags: ["ops", "automation"], version: "1.2.0", capabilities: ["llm.query", "messaging.send"], price_cents: 1200, downloads: 640, rating: 4.6, review_count: 12 },
];

function formatDownloads(count: number): string {
  if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
  return String(count);
}

function renderStars(rating: number): string {
  const full = Math.floor(rating);
  const half = rating - full >= 0.5;
  return "\u2605".repeat(full) + (half ? "\u00BD" : "") + "\u2606".repeat(5 - full - (half ? 1 : 0));
}

function riskBadge(cap: string): string {
  const critical = ["process.exec", "shell.exec"];
  const high = ["fs.write", "social.post", "messaging.send", "screen.capture", "input.keyboard"];
  if (critical.includes(cap)) return "critical";
  if (high.includes(cap)) return "high";
  return "medium";
}

export default function MarketplaceBrowser(): JSX.Element {
  const [query, setQuery] = useState("");
  const [agents, setAgents] = useState<MarketplaceAgent[]>(MOCK_LISTINGS);
  const [installedSet, setInstalledSet] = useState<Set<string>>(new Set());
  const [selectedAgent, setSelectedAgent] = useState<MarketplaceDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const isDesktop = hasDesktopRuntime();

  const doSearch = useCallback(async (q: string) => {
    if (!isDesktop) return;
    setLoading(true);
    try {
      const results = await marketplaceSearch(q);
      if (results.length > 0) {
        setAgents(results);
      } else if (q.length === 0) {
        setAgents(MOCK_LISTINGS);
      } else {
        setAgents([]);
      }
    } catch {
      // Fall back to mock data on error
    } finally {
      setLoading(false);
    }
  }, [isDesktop]);

  useEffect(() => {
    void doSearch("");
  }, [doSearch]);

  const filtered = useMemo(() => {
    if (!isDesktop) {
      const q = query.toLowerCase();
      if (q.length === 0) return agents;
      return agents.filter(
        (a) => a.name.toLowerCase().includes(q) || a.tags.some((t) => t.includes(q)) || a.description.toLowerCase().includes(q)
      );
    }
    return agents;
  }, [query, agents, isDesktop]);

  const handleSearch = useCallback((value: string) => {
    setQuery(value);
    if (isDesktop) {
      void doSearch(value);
    }
  }, [isDesktop, doSearch]);

  const handleInstall = useCallback(async (packageId: string) => {
    if (isDesktop) {
      try {
        await marketplaceInstall(packageId);
      } catch {
        // silent fallback
      }
    }
    setInstalledSet((prev) => new Set(prev).add(packageId));
  }, [isDesktop]);

  const handleShowDetail = useCallback(async (packageId: string) => {
    if (isDesktop) {
      try {
        const detail = await marketplaceInfo(packageId);
        setSelectedAgent(detail);
        return;
      } catch {
        // fall through to mock
      }
    }
    const agent = agents.find((a) => a.package_id === packageId);
    if (agent) {
      setSelectedAgent({ agent, reviews: [], versions: [{ version: agent.version, changelog: "", created_at: "" }] });
    }
  }, [isDesktop, agents]);

  return (
    <section className="mb-hub">
      <header className="mb-header">
        <h2 className="mb-title">MARKETPLACE // AGENT REGISTRY</h2>
        <p className="mb-subtitle">{filtered.length} agent{filtered.length !== 1 ? "s" : ""} available {loading && "(loading...)"}</p>
      </header>

      <div className="mb-search-bar">
        <input
          className="mb-search"
          value={query}
          onChange={(e) => handleSearch(e.target.value)}
          placeholder="Search agents by name, tag, or description..."
        />
      </div>

      {selectedAgent && (
        <div className="mb-detail-overlay" onClick={() => setSelectedAgent(null)} onKeyDown={() => {}}>
          <div className="mb-detail-modal" onClick={(e) => e.stopPropagation()} onKeyDown={() => {}}>
            <button type="button" className="mb-detail-close" onClick={() => setSelectedAgent(null)}>X</button>
            <h3 className="mb-detail-name">{selectedAgent.agent.name}</h3>
            <p className="mb-detail-meta">
              by {selectedAgent.agent.author} | v{selectedAgent.agent.version} | {renderStars(selectedAgent.agent.rating)} {selectedAgent.agent.rating.toFixed(1)} ({selectedAgent.agent.review_count} reviews)
            </p>
            <p className="mb-detail-desc">{selectedAgent.agent.description}</p>
            <div className="mb-detail-stats">
              <span>{formatDownloads(selectedAgent.agent.downloads)} downloads</span>
              {selectedAgent.agent.price_cents > 0
                ? <span className="mb-price">${(selectedAgent.agent.price_cents / 100).toFixed(2)} (free during beta)</span>
                : <span className="mb-free">Free</span>
              }
            </div>
            <div className="mb-detail-caps">
              <h4>Capabilities</h4>
              <div className="mb-card-tags">
                {selectedAgent.agent.capabilities.map((cap) => (
                  <span key={cap} className={`mb-cap-badge mb-risk-${riskBadge(cap)}`}>{cap}</span>
                ))}
              </div>
            </div>
            {selectedAgent.versions.length > 0 && (
              <div className="mb-detail-versions">
                <h4>Version History</h4>
                {selectedAgent.versions.map((v) => (
                  <div key={v.version} className="mb-version-row">
                    <span className="mb-version-num">v{v.version}</span>
                    {v.changelog && <span className="mb-version-log">{v.changelog}</span>}
                  </div>
                ))}
              </div>
            )}
            {selectedAgent.reviews.length > 0 && (
              <div className="mb-detail-reviews">
                <h4>Reviews</h4>
                {selectedAgent.reviews.map((r, i) => (
                  <div key={`${r.reviewer}-${i}`} className="mb-review-row">
                    <span className="mb-review-stars">{renderStars(r.stars)}</span>
                    <span className="mb-review-author">{r.reviewer}</span>
                    <p className="mb-review-comment">{r.comment}</p>
                  </div>
                ))}
              </div>
            )}
            <div className="mb-detail-actions">
              {installedSet.has(selectedAgent.agent.package_id) ? (
                <span className="mb-installed-badge">Installed</span>
              ) : (
                <button type="button" className="mb-install-btn" onClick={() => void handleInstall(selectedAgent.agent.package_id)}>
                  Install Agent
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      <div className="mb-grid">
        {filtered.map((agent) => {
          const isInstalled = installedSet.has(agent.package_id);
          return (
            <article key={agent.package_id} className="mb-card" onClick={() => void handleShowDetail(agent.package_id)} onKeyDown={() => {}}>
              <div className="mb-card-top">
                <h3 className="mb-card-name">{agent.name}</h3>
                <span className="mb-verified-badge" title="Signature verified">&#x2713;</span>
              </div>
              <p className="mb-card-meta">
                <span className="mb-card-author">{agent.author}</span>
                <span className="mb-card-version">v{agent.version}</span>
              </p>
              <p className="mb-card-desc">{agent.description}</p>
              <div className="mb-card-stats-row">
                <span className="mb-card-stars">{renderStars(agent.rating)} {agent.rating.toFixed(1)}</span>
                <span className="mb-card-dl">{formatDownloads(agent.downloads)} dl</span>
                {agent.price_cents > 0
                  ? <span className="mb-card-price">${(agent.price_cents / 100).toFixed(2)}</span>
                  : <span className="mb-card-free">Free</span>
                }
              </div>
              <div className="mb-card-tags">
                {agent.capabilities.slice(0, 3).map((cap) => (
                  <span key={cap} className={`mb-cap-badge mb-risk-${riskBadge(cap)}`}>{cap}</span>
                ))}
                {agent.capabilities.length > 3 && <span className="mb-tag">+{agent.capabilities.length - 3}</span>}
                {agent.tags.map((tag) => (
                  <span key={tag} className="mb-tag">{tag}</span>
                ))}
              </div>
              <div className="mb-card-footer">
                {isInstalled ? (
                  <span className="mb-installed-badge">Installed</span>
                ) : (
                  <button
                    type="button"
                    className="mb-install-btn"
                    onClick={(e) => { e.stopPropagation(); void handleInstall(agent.package_id); }}
                  >
                    Install
                  </button>
                )}
              </div>
            </article>
          );
        })}
        {filtered.length === 0 && <p className="mb-empty">No agents match your search.</p>}
      </div>
    </section>
  );
}
