import { useMemo, useState } from "react";
import "./marketplace.css";

interface MarketplaceTemplate {
  id: string;
  name: string;
  author: string;
  description: string;
  rating: number;
  installs: number;
  category: string;
}

const TEMPLATES: MarketplaceTemplate[] = [
  { id: "t1", name: "SEO Blog Writer", author: "NexusOS Core", description: "AI-powered blog posts optimized for search rankings", rating: 4.8, installs: 2400, category: "Content" },
  { id: "t2", name: "GitHub Issue Triager", author: "Community", description: "Auto-label, prioritize, and assign GitHub issues", rating: 4.6, installs: 1800, category: "Coding" },
  { id: "t3", name: "Email Outreach Agent", author: "Community", description: "Personalized cold email campaigns with follow-up sequences", rating: 4.3, installs: 3100, category: "Productivity" },
  { id: "t4", name: "Data Scraper Pro", author: "NexusOS Core", description: "Extract structured data from any website with anti-detection", rating: 4.7, installs: 5200, category: "Research" },
  { id: "t5", name: "Meeting Summarizer", author: "NexusOS Core", description: "Record, transcribe, and extract action items from meetings", rating: 4.9, installs: 4700, category: "Productivity" },
  { id: "t6", name: "Competitor Monitor", author: "Community", description: "Track competitor pricing, features, and announcements daily", rating: 4.5, installs: 1200, category: "Analytics" },
  { id: "t7", name: "Bug Fixer", author: "NexusOS Core", description: "Scan codebases for bugs, generate fixes, open PRs automatically", rating: 4.4, installs: 2900, category: "Coding" },
  { id: "t8", name: "Social Analytics", author: "Community", description: "Cross-platform engagement tracking with weekly insight reports", rating: 4.6, installs: 1500, category: "Social" },
  { id: "t9", name: "PR Draft Writer", author: "Community", description: "Generate release notes and PR descriptions from git diffs", rating: 4.2, installs: 890, category: "Coding" },
  { id: "t10", name: "Slack Digest Bot", author: "NexusOS Core", description: "Summarize busy Slack channels into daily/weekly digests", rating: 4.7, installs: 2100, category: "Productivity" }
];

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

export function Marketplace(): JSX.Element {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("All");
  const [installed, setInstalled] = useState<Set<string>>(new Set());

  const filtered = useMemo(() => {
    const q = query.toLowerCase();
    return TEMPLATES.filter((t) => {
      if (category !== "All" && t.category !== category) return false;
      if (q.length === 0) return true;
      return t.name.toLowerCase().includes(q) || t.description.toLowerCase().includes(q);
    });
  }, [query, category]);

  return (
    <section className="mp-hub">
      <header className="mp-header">
        <div>
          <h2 className="mp-title">AGENT MARKETPLACE // {TEMPLATES.length} AVAILABLE</h2>
          <p className="mp-subtitle">Curated agent templates, workflow packages, and trust-verified extensions</p>
        </div>
      </header>

      <div className="mp-toolbar">
        <div className="mp-search-wrap">
          <span className="mp-search-icon">&#x1F50D;</span>
          <input
            className="mp-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search templates..."
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

      <div className="mp-grid">
        {filtered.length === 0 ? (
          <p className="mp-empty">No templates match your search.</p>
        ) : (
          filtered.map((t) => {
            const isInstalled = installed.has(t.id);
            return (
              <article key={t.id} className={`mp-card ${isInstalled ? "installed" : ""}`}>
                <span className="mp-card-cat-tag">{t.category}</span>
                <h3 className="mp-card-name">{t.name}</h3>
                <p className="mp-card-desc">{t.description}</p>
                <p className="mp-card-author">by {t.author}</p>
                <div className="mp-card-footer">
                  <div className="mp-card-stats">
                    <span className="mp-card-stars">{renderStars(t.rating)} {t.rating.toFixed(1)}</span>
                    <span className="mp-card-downloads">{formatInstalls(t.installs)} installs</span>
                  </div>
                  <button
                    type="button"
                    className={`mp-install-btn ${isInstalled ? "done" : ""}`}
                    onClick={() => {
                      if (!isInstalled) {
                        setInstalled((prev) => new Set(prev).add(t.id));
                      }
                    }}
                  >
                    {isInstalled ? "Installed \u2713" : "Install"}
                  </button>
                </div>
              </article>
            );
          })
        )}
      </div>
    </section>
  );
}
