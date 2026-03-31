import { useCallback, useEffect, useState } from "react";
import {
  listAgents,
  mkGetStats,
  mkQuery,
  mkSearch,
  mkGetProcedures,
  mkGetCandidates,
  mkClearWorking,
  mkRunGc,
  mkCreateCheckpoint,
  mkListCheckpoints,
} from "../api/backend";
import type { AgentSummary } from "../types";

type MemoryTab = "working" | "episodic" | "semantic" | "procedural" | "all";

const TAB_LABELS: { key: MemoryTab; label: string }[] = [
  { key: "all", label: "All" },
  { key: "working", label: "Working" },
  { key: "episodic", label: "Episodic" },
  { key: "semantic", label: "Semantic" },
  { key: "procedural", label: "Procedural" },
];

const TYPE_COLORS: Record<string, string> = {
  Working: "#60a5fa",
  Episodic: "#34d399",
  Semantic: "#a78bfa",
  Procedural: "#f59e0b",
};

const TRUST_COLOR = (v: number) =>
  v >= 0.8 ? "#22c55e" : v >= 0.5 ? "#eab308" : "#ef4444";

export default function Memory(): JSX.Element {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [selectedAgent, setSelectedAgent] = useState("");
  const [stats, setStats] = useState<any>(null);
  const [tab, setTab] = useState<MemoryTab>("all");
  const [entries, setEntries] = useState<any[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchPolicy, setSearchPolicy] = useState("planning");
  const [searchResults, setSearchResults] = useState<any[]>([]);
  const [procedures, setProcedures] = useState<any[]>([]);
  const [candidates, setCandidates] = useState<any[]>([]);
  const [checkpoints, setCheckpoints] = useState<any[]>([]);
  const [checkpointLabel, setCheckpointLabel] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const agentRows = await listAgents();
      setAgents(agentRows);
      if (agentRows.length > 0 && !selectedAgent) {
        setSelectedAgent(agentRows[0].name);
      }
    } catch {
      /* no desktop backend */
    }
    setLoading(false);
  }, [selectedAgent]);

  useEffect(() => { load(); }, [load]);

  const refresh = useCallback(async () => {
    if (!selectedAgent) return;
    setError(null);
    try {
      const [s, e, p, c, cp] = await Promise.all([
        mkGetStats(selectedAgent).catch(() => null),
        mkQuery(selectedAgent, tab === "all" ? undefined : tab, 50).catch(() => []),
        mkGetProcedures(selectedAgent).catch(() => []),
        mkGetCandidates(selectedAgent).catch(() => []),
        mkListCheckpoints(selectedAgent).catch(() => []),
      ]);
      setStats(s);
      setEntries(e);
      setProcedures(p);
      setCandidates(c);
      setCheckpoints(cp);
    } catch (err: any) {
      setError(err?.toString() ?? "Failed to load memory data");
    }
  }, [selectedAgent, tab]);

  useEffect(() => { refresh(); }, [refresh]);

  const doSearch = async () => {
    if (!selectedAgent || !searchQuery.trim()) return;
    try {
      const results = await mkSearch(selectedAgent, searchQuery, searchPolicy, 20);
      setSearchResults(results);
    } catch (err: any) {
      setError(err?.toString() ?? "Search failed");
    }
  };

  const doClearWorking = async () => {
    if (!selectedAgent) return;
    try {
      await mkClearWorking(selectedAgent);
      setMessage("Working memory cleared");
      refresh();
    } catch (err: any) {
      setError(err?.toString() ?? "Clear failed");
    }
  };

  const doGc = async () => {
    try {
      const report = await mkRunGc();
      setMessage(`GC: ${report.entries_scanned ?? 0} scanned, ${report.working_cleared ?? 0} working cleared, ${report.semantic_soft_deleted ?? 0} semantic deleted, ${report.procedural_demoted ?? 0} procedural demoted`);
      refresh();
    } catch (err: any) {
      setError(err?.toString() ?? "GC failed");
    }
  };

  const doCreateCheckpoint = async () => {
    if (!selectedAgent || !checkpointLabel.trim()) return;
    try {
      const id = await mkCreateCheckpoint(selectedAgent, checkpointLabel);
      setMessage(`Checkpoint created: ${id.slice(0, 8)}...`);
      setCheckpointLabel("");
      refresh();
    } catch (err: any) {
      setError(err?.toString() ?? "Checkpoint failed");
    }
  };

  const typeLabel = (mt: string) => mt || "Unknown";
  const trustBadge = (v: number) => (
    <span style={{ color: TRUST_COLOR(v), fontWeight: 600 }}>
      {(v * 100).toFixed(0)}%
    </span>
  );

  if (loading) {
    return <div style={{ padding: 32, color: "#9ca3af" }}>Loading memory subsystem...</div>;
  }

  return (
    <div style={{ padding: 24, maxWidth: 1200, margin: "0 auto", color: "#e5e7eb" }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 16, color: "#f9fafb" }}>
        Agent Memory
      </h1>

      {error && (
        <div style={{ background: "#7f1d1d", padding: "8px 16px", borderRadius: 6, marginBottom: 12, color: "#fca5a5" }}>
          {error}
          <button onClick={() => setError(null)} style={{ marginLeft: 12, color: "#fca5a5", background: "none", border: "none", cursor: "pointer" }}>✕</button>
        </div>
      )}
      {message && (
        <div style={{ background: "#14532d", padding: "8px 16px", borderRadius: 6, marginBottom: 12, color: "#86efac" }}>
          {message}
          <button onClick={() => setMessage(null)} style={{ marginLeft: 12, color: "#86efac", background: "none", border: "none", cursor: "pointer" }}>✕</button>
        </div>
      )}

      {/* Agent selector + stats */}
      <div style={{ display: "flex", gap: 16, marginBottom: 20, alignItems: "center" }}>
        <select
          value={selectedAgent}
          onChange={(e) => setSelectedAgent(e.target.value)}
          style={{ background: "#1f2937", color: "#e5e7eb", border: "1px solid #374151", borderRadius: 6, padding: "6px 12px", fontSize: 14 }}
        >
          <option value="">Select Agent</option>
          {agents.map((a) => (
            <option key={a.name} value={a.name}>{a.name}</option>
          ))}
        </select>

        {stats && (
          <div style={{ display: "flex", gap: 12 }}>
            {[
              { label: "Working", count: stats.working_count ?? 0, color: TYPE_COLORS.Working },
              { label: "Episodic", count: stats.episodic_count ?? 0, color: TYPE_COLORS.Episodic },
              { label: "Semantic", count: stats.semantic_count ?? 0, color: TYPE_COLORS.Semantic },
              { label: "Procedural", count: stats.procedural_count ?? 0, color: TYPE_COLORS.Procedural },
            ].map((s) => (
              <div key={s.label} style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 13 }}>
                <div style={{ width: 8, height: 8, borderRadius: "50%", background: s.color }} />
                <span style={{ color: "#9ca3af" }}>{s.label}:</span>
                <span style={{ fontWeight: 600 }}>{s.count}</span>
              </div>
            ))}
          </div>
        )}

        <button onClick={refresh} style={{ marginLeft: "auto", background: "#374151", color: "#e5e7eb", border: "none", borderRadius: 6, padding: "6px 14px", cursor: "pointer", fontSize: 13 }}>
          Refresh
        </button>
      </div>

      {/* Tab bar */}
      <div style={{ display: "flex", gap: 2, marginBottom: 16, borderBottom: "1px solid #374151" }}>
        {TAB_LABELS.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            style={{
              background: tab === t.key ? "#1f2937" : "transparent",
              color: tab === t.key ? "#f9fafb" : "#9ca3af",
              border: "none",
              borderBottom: tab === t.key ? "2px solid #60a5fa" : "2px solid transparent",
              padding: "8px 16px",
              cursor: "pointer",
              fontSize: 13,
              fontWeight: tab === t.key ? 600 : 400,
            }}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Memory entries */}
      <div style={{ marginBottom: 24 }}>
        {entries.length === 0 ? (
          <div style={{ color: "#6b7280", padding: 16, textAlign: "center" }}>No memories found</div>
        ) : (
          entries.map((entry: any) => {
            const id = entry.id ?? "";
            const isExpanded = expanded === id;
            return (
              <div
                key={id}
                onClick={() => setExpanded(isExpanded ? null : id)}
                style={{ background: "#111827", borderRadius: 6, padding: "10px 14px", marginBottom: 6, cursor: "pointer", border: "1px solid #1f2937" }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                  <span style={{ background: TYPE_COLORS[entry.memory_type] ?? "#6b7280", color: "#000", padding: "1px 8px", borderRadius: 4, fontSize: 11, fontWeight: 600 }}>
                    {typeLabel(entry.memory_type)}
                  </span>
                  <span style={{ color: "#9ca3af", flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {JSON.stringify(entry.content)?.slice(0, 100)}
                  </span>
                  <span style={{ fontSize: 11, color: "#6b7280" }}>
                    Trust: {trustBadge(entry.trust_score ?? 0)}
                  </span>
                </div>
                {isExpanded && (
                  <pre style={{ marginTop: 8, fontSize: 11, color: "#d1d5db", whiteSpace: "pre-wrap", maxHeight: 300, overflow: "auto" }}>
                    {JSON.stringify(entry, null, 2)}
                  </pre>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Search */}
      <div style={{ background: "#111827", borderRadius: 8, padding: 16, marginBottom: 24, border: "1px solid #1f2937" }}>
        <h2 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, color: "#f9fafb" }}>Memory Search</h2>
        <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && doSearch()}
            placeholder="Search memories..."
            style={{ flex: 1, background: "#1f2937", color: "#e5e7eb", border: "1px solid #374151", borderRadius: 6, padding: "6px 12px", fontSize: 13 }}
          />
          <select
            value={searchPolicy}
            onChange={(e) => setSearchPolicy(e.target.value)}
            style={{ background: "#1f2937", color: "#e5e7eb", border: "1px solid #374151", borderRadius: 6, padding: "6px 8px", fontSize: 13 }}
          >
            <option value="planning">Planning</option>
            <option value="execution">Execution</option>
            <option value="safety">Safety</option>
          </select>
          <button onClick={doSearch} style={{ background: "#2563eb", color: "#fff", border: "none", borderRadius: 6, padding: "6px 16px", cursor: "pointer", fontSize: 13 }}>Search</button>
        </div>
        {searchResults.length > 0 && (
          <div>
            {searchResults.map((r: any, i: number) => (
              <div key={i} style={{ padding: "6px 10px", background: "#1f2937", borderRadius: 4, marginBottom: 4, fontSize: 12 }}>
                <span style={{ color: "#60a5fa", fontWeight: 600 }}>{((r.relevance_score ?? 0) * 100).toFixed(0)}%</span>
                <span style={{ color: "#6b7280", marginLeft: 8 }}>{r.match_type}</span>
                <span style={{ color: "#d1d5db", marginLeft: 8 }}>{JSON.stringify(r.entry?.content)?.slice(0, 80)}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Procedures */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginBottom: 24 }}>
        <div style={{ background: "#111827", borderRadius: 8, padding: 16, border: "1px solid #1f2937" }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, color: "#f9fafb" }}>Procedures ({procedures.length})</h2>
          {procedures.length === 0 ? (
            <div style={{ color: "#6b7280", fontSize: 13 }}>No procedures learned yet</div>
          ) : (
            procedures.map((p: any, i: number) => (
              <div key={i} style={{ padding: "6px 10px", background: "#1f2937", borderRadius: 4, marginBottom: 4, fontSize: 12 }}>
                <span style={{ fontWeight: 600, color: "#f9fafb" }}>{p.content?.Procedure?.name ?? "unnamed"}</span>
                <span style={{ marginLeft: 8, color: TRUST_COLOR(p.trust_score ?? 0) }}>
                  {((p.trust_score ?? 0) * 100).toFixed(0)}% success
                </span>
              </div>
            ))
          )}
        </div>

        <div style={{ background: "#111827", borderRadius: 8, padding: 16, border: "1px solid #1f2937" }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, color: "#f9fafb" }}>Candidates ({candidates.length})</h2>
          {candidates.length === 0 ? (
            <div style={{ color: "#6b7280", fontSize: 13 }}>No candidates being tracked</div>
          ) : (
            candidates.map((c: any, i: number) => (
              <div key={i} style={{ padding: "6px 10px", background: "#1f2937", borderRadius: 4, marginBottom: 4, fontSize: 12 }}>
                <span style={{ fontWeight: 600, color: "#f9fafb" }}>{c.name}</span>
                <span style={{ marginLeft: 8, color: "#9ca3af" }}>{c.executions?.length ?? 0} runs</span>
              </div>
            ))
          )}
        </div>
      </div>

      {/* Governance: Checkpoints + GC */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        <div style={{ background: "#111827", borderRadius: 8, padding: 16, border: "1px solid #1f2937" }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, color: "#f9fafb" }}>Checkpoints ({checkpoints.length})</h2>
          <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
            <input
              value={checkpointLabel}
              onChange={(e) => setCheckpointLabel(e.target.value)}
              placeholder="Checkpoint label..."
              style={{ flex: 1, background: "#1f2937", color: "#e5e7eb", border: "1px solid #374151", borderRadius: 6, padding: "6px 12px", fontSize: 13 }}
            />
            <button onClick={doCreateCheckpoint} style={{ background: "#374151", color: "#e5e7eb", border: "none", borderRadius: 6, padding: "6px 14px", cursor: "pointer", fontSize: 13 }}>Create</button>
          </div>
          {checkpoints.map((cp: any, i: number) => (
            <div key={i} style={{ padding: "4px 10px", background: "#1f2937", borderRadius: 4, marginBottom: 4, fontSize: 12 }}>
              <span style={{ fontWeight: 600, color: "#f9fafb" }}>{cp.label}</span>
              <span style={{ marginLeft: 8, color: "#6b7280" }}>{cp.id?.slice(0, 8)}...</span>
            </div>
          ))}
        </div>

        <div style={{ background: "#111827", borderRadius: 8, padding: 16, border: "1px solid #1f2937" }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12, color: "#f9fafb" }}>Maintenance</h2>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button onClick={doGc} style={{ background: "#374151", color: "#e5e7eb", border: "none", borderRadius: 6, padding: "6px 14px", cursor: "pointer", fontSize: 13 }}>
              Run GC
            </button>
            <button onClick={doClearWorking} style={{ background: "#7f1d1d", color: "#fca5a5", border: "none", borderRadius: 6, padding: "6px 14px", cursor: "pointer", fontSize: 13 }}>
              Clear Working Memory
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
