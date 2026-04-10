import { useMemo, useState } from "react";

interface HistoryEntry {
  id: string;
  timestamp: number;
  preview: string;
}

interface HistoryProps {
  open: boolean;
  entries: HistoryEntry[];
  onClose: () => void;
  onSelect: (entry: HistoryEntry) => void;
}

function formatTimestamp(timestamp: number): string {
  return new Date(timestamp).toLocaleString("en-GB", {
    hour12: false,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

export function History({ open, entries, onClose, onSelect }: HistoryProps): JSX.Element {
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const lowered = query.trim().toLowerCase();
    if (lowered.length === 0) {
      return entries;
    }
    return entries.filter((entry) => entry.preview.toLowerCase().includes(lowered));
  }, [entries, query]);

  return (
    <aside className={`jarvis-history ${open ? "open" : ""}`}>
      <header className="jarvis-history-header">
        <h3>CHAT HISTORY</h3>
        <button type="button" onClick={onClose} aria-label="Close history panel">
          ✕
        </button>
      </header>

      <div className="jarvis-history-search">
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search transmissions..."
          aria-label="Search history"
        />
      </div>

      <div className="jarvis-history-list">
        {filtered.length === 0 ? (
          <p className="jarvis-history-empty">No matching transmissions.</p>
        ) : (
          filtered.map((entry) => (
            <button type="button"
              key={entry.id}
              className="jarvis-history-entry"
              onClick={() => onSelect(entry)}
            >
              <span className="jarvis-history-time">{formatTimestamp(entry.timestamp)}</span>
              <span className="jarvis-history-preview">{entry.preview}</span>
            </button>
          ))
        )}
      </div>
    </aside>
  );
}
