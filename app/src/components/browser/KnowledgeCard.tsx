import type { KnowledgeEntry } from "../../types";

interface KnowledgeCardProps {
  entry: KnowledgeEntry;
}

function timeAgo(ts: number): string {
  const diff = Date.now() - ts;
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

function relevanceColor(score: number): string {
  if (score >= 0.8) return "#00ffd5";
  if (score >= 0.5) return "#22d3ee";
  return "#64748b";
}

function relevanceLabel(score: number): string {
  if (score >= 0.8) return "High";
  if (score >= 0.5) return "Medium";
  return "Low";
}

export function KnowledgeCard({ entry }: KnowledgeCardProps): JSX.Element {
  const rColor = relevanceColor(entry.relevance_score);

  return (
    <div className={`knowledge-card${entry.is_new ? " knowledge-card--new" : ""}`}>
      <div className="knowledge-card-header">
        <span className="knowledge-card-title">{entry.title}</span>
        {entry.is_new && <span className="knowledge-card-badge">NEW</span>}
      </div>

      <div className="knowledge-card-meta">
        <span
          className="knowledge-card-relevance"
          style={{ color: rColor, borderColor: rColor }}
        >
          {relevanceLabel(entry.relevance_score)}
        </span>
        <span className="knowledge-card-category">{entry.category}</span>
        <span className="knowledge-card-time">{timeAgo(entry.timestamp)}</span>
      </div>

      <ul className="knowledge-card-points">
        {entry.key_points.map((point, i) => (
          <li key={i} className="knowledge-card-point">
            {point}
          </li>
        ))}
      </ul>

      {entry.change_summary && (
        <div className="knowledge-card-change">
          <span className="knowledge-card-change-label">Changed:</span>{" "}
          {entry.change_summary}
        </div>
      )}

      <a
        className="knowledge-card-source"
        href={entry.source_url}
        target="_blank"
        rel="noopener noreferrer"
        title={entry.source_url}
      >
        {entry.source_url.replace(/^https?:\/\//, "").slice(0, 60)}
      </a>
    </div>
  );
}
