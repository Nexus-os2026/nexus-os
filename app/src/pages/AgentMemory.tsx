import { useCallback, useEffect, useState } from "react";
import {
  listAgents,
  memoryBuildContext,
  memoryConsolidate,
  memoryDeleteEntry,
  memoryGetPolicy,
  memoryGetStats,
  memoryLoad,
  memoryQueryEntries,
  memorySave,
  memoryStoreEntry,
} from "../api/backend";
import { alpha, commandPageStyle } from "./commandCenterUi";

const ACCENT = "#14b8a6";
const GREEN = "#22c55e";
const BLUE = "#0ea5e9";
const YELLOW = "#eab308";

const MEMORY_TYPES = ["Episodic", "Semantic", "Procedural", "Relational"] as const;
const TYPE_COLORS: Record<string, string> = {
  Episodic: "#f97316",
  Semantic: "#0ea5e9",
  Procedural: "#22c55e",
  Relational: "#14b8a6",
};

const cardStyle: React.CSSProperties = {
  background: alpha("#1e1e2e", 0.7),
  borderRadius: 10,
  padding: 16,
  border: "1px solid " + alpha("#ffffff", 0.08),
};

const labelStyle: React.CSSProperties = {
  fontSize: 11,
  color: "#888",
  textTransform: "uppercase" as const,
  letterSpacing: 1,
  marginBottom: 4,
};

const btnStyle: React.CSSProperties = {
  padding: "6px 14px",
  borderRadius: 6,
  border: "none",
  cursor: "pointer",
  fontWeight: 600,
  fontSize: 12,
};

const inputStyle: React.CSSProperties = {
  padding: 8,
  borderRadius: 6,
  background: "#2a2a3e",
  color: "#e0e0e0",
  border: "1px solid #444",
  width: "100%",
  boxSizing: "border-box" as const,
};

export default function AgentMemory() {
  const [agents, setAgents] = useState<{ id: string; name: string }[]>([]);
  const [selectedAgent, setSelectedAgent] = useState("");
  const [memories, setMemories] = useState<any[]>([]);
  const [stats, setStats] = useState<any>(null);
  const [policy, setPolicy] = useState<any>(null);
  const [contextPreview, setContextPreview] = useState<any>(null);

  // Store form
  const [newType, setNewType] = useState("Semantic");
  const [newSummary, setNewSummary] = useState("");
  const [newTags, setNewTags] = useState("");
  const [newImportance, setNewImportance] = useState(0.7);
  const [newDomain, setNewDomain] = useState("");

  // Query
  const [queryText, setQueryText] = useState("");
  const [queryType, setQueryType] = useState("");
  const [contextTask, setContextTask] = useState("");

  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState("");

  useEffect(() => {
    listAgents()
      .then((a: any) => {
        const list = Array.isArray(a) ? a : [];
        setAgents(list.map((x: any) => ({ id: x.id || x, name: x.name || x.id || x })));
      })
      .catch((e) => { if (import.meta.env.DEV) console.warn("[AgentMemory]", e); });
    memoryGetPolicy().then(setPolicy).catch((e) => { if (import.meta.env.DEV) console.warn("[AgentMemory]", e); });
  }, []);

  const refresh = useCallback(async () => {
    if (!selectedAgent) return;
    setLoading(true);
    try {
      const [mems, st] = await Promise.all([
        memoryQueryEntries(selectedAgent, queryText, queryType || undefined, undefined, 50),
        memoryGetStats(selectedAgent).catch(() => null),
      ]);
      setMemories(Array.isArray(mems) ? mems : []);
      setStats(st);
    } catch {
      setMemories([]);
    } finally {
      setLoading(false);
    }
  }, [selectedAgent, queryText, queryType]);

  useEffect(() => { refresh(); }, [selectedAgent, refresh]);

  const handleStore = useCallback(async () => {
    if (!selectedAgent || !newSummary.trim()) return;
    const tags = newTags.split(",").map((t) => t.trim()).filter(Boolean);
    await memoryStoreEntry(selectedAgent, newType, newSummary, tags, newImportance, newDomain || undefined);
    setNewSummary("");
    setNewTags("");
    setStatus("Memory stored");
    refresh();
  }, [selectedAgent, newType, newSummary, newTags, newImportance, newDomain, refresh]);

  const handleDelete = useCallback(async (memoryId: string) => {
    if (!selectedAgent) return;
    await memoryDeleteEntry(selectedAgent, memoryId);
    refresh();
  }, [selectedAgent, refresh]);

  const handleBuildContext = useCallback(async () => {
    if (!selectedAgent || !contextTask.trim()) return;
    const ctx = await memoryBuildContext(selectedAgent, contextTask, 10);
    setContextPreview(ctx);
  }, [selectedAgent, contextTask]);

  const handleSave = useCallback(async () => {
    if (!selectedAgent) return;
    const msg = await memorySave(selectedAgent);
    setStatus(msg);
  }, [selectedAgent]);

  const handleLoad = useCallback(async () => {
    if (!selectedAgent) return;
    const msg = await memoryLoad(selectedAgent);
    setStatus(msg);
    refresh();
  }, [selectedAgent, refresh]);

  const handleConsolidate = useCallback(async () => {
    if (!selectedAgent) return;
    const result = await memoryConsolidate(selectedAgent);
    setStatus(`Merge candidates: ${result.merge_candidates}, forgettable: ${result.forgettable}`);
  }, [selectedAgent]);

  return (
    <div style={{ ...commandPageStyle, padding: 24, color: "#e0e0e0" }}>
      <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 4 }}>
        <span style={{ color: ACCENT }}>Persistent Agent Memory</span>
      </h1>
      <p style={{ color: "#888", fontSize: 13, marginBottom: 16 }}>
        Episodic, semantic, procedural, and relational memory across sessions.
      </p>

      {status && (
        <div style={{ fontSize: 12, color: GREEN, marginBottom: 10 }}>{status}</div>
      )}

      {/* Agent selector + controls */}
      <div style={{ display: "flex", gap: 10, marginBottom: 16, alignItems: "center" }}>
        <select
          value={selectedAgent}
          onChange={(e) => setSelectedAgent(e.target.value)}
          style={{ ...inputStyle, width: 240 }}
        >
          <option value="">Select Agent</option>
          {agents
            .filter((a, i, arr) => arr.findIndex((x) => x.id === a.id || x.name === a.name) === i)
            .map((a) => (
            <option key={a.id} value={a.id}>{a.name}</option>
          ))}
        </select>
        <button onClick={handleSave} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Save</button>
        <button onClick={handleLoad} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Load</button>
        <button onClick={handleConsolidate} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Consolidate</button>
        <button onClick={refresh} style={{ ...btnStyle, background: ACCENT, color: "#fff" }}>Refresh</button>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        {/* Left: Store + Query + Stats */}
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          {/* Store form */}
          <div style={cardStyle}>
            <div style={labelStyle}>Store New Memory</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 8, marginTop: 8 }}>
              <select value={newType} onChange={(e) => setNewType(e.target.value)} style={inputStyle}>
                {MEMORY_TYPES.map((t) => (
                  <option key={t} value={t}>{t}</option>
                ))}
              </select>
              <textarea
                placeholder="Memory summary..."
                value={newSummary}
                onChange={(e) => setNewSummary(e.target.value)}
                rows={3}
                style={{ ...inputStyle, resize: "vertical", fontFamily: "inherit" }}
              />
              <input placeholder="Tags (comma-separated)" value={newTags} onChange={(e) => setNewTags(e.target.value)} style={inputStyle} />
              <div style={{ display: "flex", gap: 8 }}>
                <input placeholder="Domain" value={newDomain} onChange={(e) => setNewDomain(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
                <div style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12 }}>
                  <span style={{ color: "#888" }}>Importance:</span>
                  <input
                    type="range" min="0" max="1" step="0.1"
                    value={newImportance}
                    onChange={(e) => setNewImportance(Number(e.target.value))}
                    style={{ width: 80 }}
                  />
                  <span style={{ color: YELLOW }}>{newImportance.toFixed(1)}</span>
                </div>
              </div>
              <button onClick={handleStore} disabled={!selectedAgent || !newSummary.trim()} style={{ ...btnStyle, background: ACCENT, color: "#fff" }}>
                Store Memory
              </button>
            </div>
          </div>

          {/* Query */}
          <div style={cardStyle}>
            <div style={labelStyle}>Search Memories</div>
            <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
              <input placeholder="Search query..." value={queryText} onChange={(e) => setQueryText(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
              <select value={queryType} onChange={(e) => setQueryType(e.target.value)} style={{ ...inputStyle, width: 120 }}>
                <option value="">All Types</option>
                {MEMORY_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
              </select>
              <button onClick={refresh} style={{ ...btnStyle, background: BLUE, color: "#fff" }}>Search</button>
            </div>
          </div>

          {/* Context preview */}
          <div style={cardStyle}>
            <div style={labelStyle}>Context Preview</div>
            <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
              <input placeholder="Task description..." value={contextTask} onChange={(e) => setContextTask(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
              <button onClick={handleBuildContext} disabled={!selectedAgent} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Preview</button>
            </div>
            {contextPreview && (
              <pre style={{ fontSize: 11, color: GREEN, background: alpha("#000", 0.3), padding: 10, borderRadius: 6, marginTop: 8, whiteSpace: "pre-wrap", maxHeight: 200, overflow: "auto" }}>
                {contextPreview.context_text || "(empty)"}
              </pre>
            )}
          </div>

          {/* Stats + Policy */}
          {(stats || policy) && (
            <div style={cardStyle}>
              {stats && (
                <>
                  <div style={labelStyle}>Statistics</div>
                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6, fontSize: 12, marginTop: 6 }}>
                    <div>Total: <span style={{ color: ACCENT }}>{stats.total}</span></div>
                    {stats.by_type && Object.entries(stats.by_type).map(([k, v]) => (
                      <div key={k}>{k}: <span style={{ color: TYPE_COLORS[k] || "#888" }}>{String(v)}</span></div>
                    ))}
                  </div>
                </>
              )}
              {policy && (
                <div style={{ marginTop: stats ? 12 : 0 }}>
                  <div style={labelStyle}>Policy</div>
                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6, fontSize: 12, marginTop: 6 }}>
                    <div>Min Autonomy: <span style={{ color: BLUE }}>L{policy.min_autonomy_level}</span></div>
                    <div>Max Per Agent: <span style={{ color: BLUE }}>{policy.max_memories_per_agent}</span></div>
                    <div>Store: <span style={{ color: ACCENT }}>{(policy.store_cost / 1_000_000).toFixed(2)} NXC</span></div>
                    <div>Query: <span style={{ color: ACCENT }}>{(policy.query_cost / 1_000_000).toFixed(2)} NXC</span></div>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Right: Memory browser */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <div style={labelStyle}>Memories ({memories.length})</div>
          {loading && <div style={{ color: "#666", fontSize: 13 }}>Loading...</div>}
          {!loading && memories.length === 0 && (
            <div style={{ ...cardStyle, textAlign: "center", padding: 32, color: "#555" }}>
              {selectedAgent ? "No memories found" : "Select an agent to view memories"}
            </div>
          )}
          {memories.map((m) => (
            <div key={m.id} style={{ ...cardStyle, padding: 12 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span style={{
                    fontSize: 10,
                    padding: "2px 6px",
                    borderRadius: 4,
                    background: alpha(TYPE_COLORS[m.memory_type] || "#888", 0.2),
                    color: TYPE_COLORS[m.memory_type] || "#888",
                    fontWeight: 600,
                  }}>
                    {m.memory_type}
                  </span>
                  <span style={{ fontSize: 11, color: YELLOW }}>
                    {(m.importance * 100).toFixed(0)}%
                  </span>
                  <span style={{ fontSize: 10, color: "#555" }}>
                    {m.access_count}x accessed
                  </span>
                </div>
                <button
                  onClick={() => handleDelete(m.id)}
                  style={{ ...btnStyle, padding: "2px 8px", background: "transparent", color: "#666", fontSize: 10 }}
                >
                  Delete
                </button>
              </div>
              <div style={{ fontSize: 13, lineHeight: 1.5 }}>{m.content?.summary}</div>
              {m.tags && m.tags.length > 0 && (
                <div style={{ display: "flex", gap: 4, marginTop: 6, flexWrap: "wrap" }}>
                  {m.tags.map((t: string) => (
                    <span key={t} style={{ fontSize: 10, padding: "1px 6px", borderRadius: 3, background: alpha("#fff", 0.05), color: "#888" }}>
                      {t}
                    </span>
                  ))}
                </div>
              )}
              {m.metadata?.domain && (
                <div style={{ fontSize: 10, color: "#555", marginTop: 4 }}>Domain: {m.metadata.domain}</div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
