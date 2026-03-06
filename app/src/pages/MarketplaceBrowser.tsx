import { useMemo, useState } from "react";
import "./marketplace-browser.css";

interface MarketplaceAgent {
  id: string;
  name: string;
  author: string;
  version: string;
  description: string;
  signatureVerified: boolean;
  installed: boolean;
  tags: string[];
}

const MOCK_LISTINGS: MarketplaceAgent[] = [
  { id: "ml-1", name: "Code Review Bot", author: "NexusOS Core", version: "2.1.0", description: "Automated pull request reviews with security scanning and style enforcement", signatureVerified: true, installed: false, tags: ["coding", "security"] },
  { id: "ml-2", name: "Data Pipeline Agent", author: "Community", version: "1.4.2", description: "ETL workflows with schema validation, retry logic, and audit logging", signatureVerified: true, installed: true, tags: ["data", "automation"] },
  { id: "ml-3", name: "Compliance Monitor", author: "NexusOS Core", version: "3.0.0", description: "Continuous SOC2/HIPAA compliance checking with evidence collection", signatureVerified: true, installed: false, tags: ["compliance", "security"] },
  { id: "ml-4", name: "Social Scheduler", author: "Community", version: "1.0.8", description: "Multi-platform social media scheduling with approval gates and analytics", signatureVerified: false, installed: false, tags: ["social", "content"] },
  { id: "ml-5", name: "Incident Responder", author: "NexusOS Core", version: "1.2.0", description: "Automated incident triage, escalation, and post-mortem generation", signatureVerified: true, installed: false, tags: ["ops", "automation"] },
];

export default function MarketplaceBrowser(): JSX.Element {
  const [query, setQuery] = useState("");
  const [installedSet, setInstalledSet] = useState<Set<string>>(
    new Set(MOCK_LISTINGS.filter((l) => l.installed).map((l) => l.id))
  );

  const filtered = useMemo(() => {
    const q = query.toLowerCase();
    if (q.length === 0) return MOCK_LISTINGS;
    return MOCK_LISTINGS.filter(
      (l) => l.name.toLowerCase().includes(q) || l.tags.some((t) => t.includes(q)) || l.description.toLowerCase().includes(q)
    );
  }, [query]);

  return (
    <section className="mb-hub">
      <header className="mb-header">
        <h2 className="mb-title">MARKETPLACE // AGENT REGISTRY</h2>
        <p className="mb-subtitle">{MOCK_LISTINGS.length} agents available</p>
      </header>

      <div className="mb-search-bar">
        <input
          className="mb-search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search agents by name or tag..."
        />
      </div>

      <div className="mb-grid">
        {filtered.map((agent) => {
          const isInstalled = installedSet.has(agent.id);
          return (
            <article key={agent.id} className="mb-card">
              <div className="mb-card-top">
                <h3 className="mb-card-name">{agent.name}</h3>
                {agent.signatureVerified && <span className="mb-verified-badge" title="Signature verified">&#x2713;</span>}
              </div>
              <p className="mb-card-meta">
                <span className="mb-card-author">{agent.author}</span>
                <span className="mb-card-version">v{agent.version}</span>
              </p>
              <p className="mb-card-desc">{agent.description}</p>
              <div className="mb-card-tags">
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
                    onClick={() => setInstalledSet((prev) => new Set(prev).add(agent.id))}
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
