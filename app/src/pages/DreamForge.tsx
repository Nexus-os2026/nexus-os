import React, { useCallback, useEffect, useState } from "react";
import { Check, Play, Circle, X } from "lucide-react";
import {
  getDreamStatus as fetchDreamStatus,
  getDreamQueue as fetchDreamQueue,
  getDreamHistory as fetchDreamHistory,
  getMorningBriefing as fetchMorningBriefing,
  triggerDreamNow,
  setDreamConfig,
} from "../api/backend";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface DreamStatus {
  active_dreams: number;
  completed_today: number;
  next_scheduled: string;
  budget_remaining: number;
  budget_total: number;
  enabled: boolean;
}

interface DreamBriefing {
  summary: string;
  improvements: string[];
  agents_created: string[];
  presolved_count: number;
}

interface DreamQueueItem {
  id: string;
  dream_type: string;
  agent_id: string;
  priority: number;
  status: string;
}

interface DreamHistoryEntry {
  id: string;
  dream_type: string;
  agent_id: string;
  status: string;
  summary: string;
  score_before: number;
  score_after: number;
  timestamp: number;
}

interface DreamConfig {
  enabled: boolean;
  idle_trigger_minutes: number;
  token_budget: number;
  api_call_budget: number;
}

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function dreamTypeColor(t: string): string {
  switch (t.toLowerCase()) {
    case "experiment": return "#a78bfa";
    case "consolidate": return "#22d3ee";
    case "explore": return "#22c55e";
    case "precompute": return "#eab308";
    default: return "#94a3b8";
  }
}

function statusIcon(s: string): React.ReactNode {
  switch (s.toLowerCase()) {
    case "completed": return <Check size={14} aria-hidden="true" />;
    case "running": return <Play size={14} aria-hidden="true" />;
    case "pending": return <Circle size={14} aria-hidden="true" />;
    case "failed": return <X size={14} aria-hidden="true" />;
    default: return <Circle size={10} aria-hidden="true" />;
  }
}

function formatTime(ts: number): string {
  if (ts === 0) return "—";
  return new Date(ts * 1000).toLocaleString();
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function DreamForge(): JSX.Element {
  const [status, setStatus] = useState<DreamStatus | null>(null);
  const [briefing, setBriefing] = useState<DreamBriefing | null>(null);
  const [queue, setQueue] = useState<DreamQueueItem[]>([]);
  const [history, setHistory] = useState<DreamHistoryEntry[]>([]);
  const [config, setConfig] = useState<DreamConfig>({ enabled: true, idle_trigger_minutes: 15, token_budget: 50000, api_call_budget: 20 });
  const [saving, setSaving] = useState(false);
  const [triggering, setTriggering] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const results = await Promise.allSettled([
        fetchDreamStatus().then((raw) => JSON.parse(raw) as DreamStatus),
        fetchMorningBriefing().then((raw) => JSON.parse(raw) as DreamBriefing),
        fetchDreamQueue().then((raw) => JSON.parse(raw) as DreamQueueItem[]),
        fetchDreamHistory(20).then((raw) => JSON.parse(raw) as DreamHistoryEntry[]),
      ]);
      if (results[0].status === "fulfilled") {
        const s = results[0].value;
        setStatus(s);
        setConfig((prev) => ({ ...prev, enabled: s.enabled }));
      }
      if (results[1].status === "fulfilled") setBriefing(results[1].value);
      if (results[2].status === "fulfilled") setQueue(Array.isArray(results[2].value) ? results[2].value : []);
      if (results[3].status === "fulfilled") setHistory(Array.isArray(results[3].value) ? results[3].value : []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const iv = setInterval(() => void refresh(), 10_000);
    return () => clearInterval(iv);
  }, [refresh]);

  const handleTrigger = useCallback(async () => {
    setTriggering(true);
    try {
      await triggerDreamNow();
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setTriggering(false);
    }
  }, [refresh]);

  const handleSaveConfig = useCallback(async () => {
    setSaving(true);
    try {
      await setDreamConfig(
        config.enabled,
        config.idle_trigger_minutes,
        config.token_budget,
        config.api_call_budget,
      );
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }, [config, refresh]);

  const budgetPct = status ? Math.round((status.budget_remaining / Math.max(status.budget_total, 1)) * 100) : 0;

  return (
    <div style={{ padding: 24, color: "#e2e8f0", maxWidth: 1400, margin: "0 auto" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 20 }}>
        <div>
          <h1 style={{ fontFamily: "monospace", fontSize: "1.8rem", color: "#a78bfa", marginBottom: 8 }}>
            DREAM FORGE
          </h1>
          <p style={{ color: "#94a3b8", fontSize: "0.85rem" }}>
            Status: {status?.active_dreams ? "Active" : "Idle"} | Budget: {status?.budget_remaining?.toLocaleString() ?? "—"}/{status?.budget_total?.toLocaleString() ?? "—"} tokens
          </p>
        </div>
        {/* Budget bar */}
        <div style={{ width: 200 }}>
          <div style={{ fontSize: "0.7rem", color: "#64748b", marginBottom: 4, textAlign: "right" }}>{budgetPct}% remaining</div>
          <div style={{ height: 6, background: "#1e293b", borderRadius: 3, overflow: "hidden" }}>
            <div style={{ width: `${budgetPct}%`, height: "100%", background: budgetPct > 30 ? "#a78bfa" : "#ef4444", borderRadius: 3 }} />
          </div>
        </div>
      </div>

      {error && <div style={{ color: "#f87171", marginBottom: 12, fontSize: "0.85rem" }}>{error}</div>}

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
        {/* Morning Briefing */}
        <div style={panelStyle}>
          <h3 style={headStyle}>Morning Briefing</h3>
          {briefing ? (
            <div>
              <div style={{ fontSize: "0.85rem", color: "#e2e8f0", marginBottom: 12, fontStyle: "italic", lineHeight: 1.5 }}>
                "{briefing.summary}"
              </div>
              {briefing.improvements.length > 0 && (
                <div style={{ marginBottom: 8 }}>
                  <div style={{ fontSize: "0.72rem", color: "#64748b", marginBottom: 4 }}>IMPROVEMENTS</div>
                  {briefing.improvements.map((imp, i) => (
                    <div key={i} style={{ fontSize: "0.78rem", color: "#22c55e", padding: "2px 0" }}>• {imp}</div>
                  ))}
                </div>
              )}
              {briefing.agents_created.length > 0 && (
                <div style={{ marginBottom: 8 }}>
                  <div style={{ fontSize: "0.72rem", color: "#64748b", marginBottom: 4 }}>AGENTS CREATED</div>
                  {briefing.agents_created.map((a) => (
                    <div key={a} style={{ fontSize: "0.78rem", color: "#a78bfa", padding: "2px 0" }}>+ {a}</div>
                  ))}
                </div>
              )}
              {briefing.presolved_count > 0 && (
                <div style={{ fontSize: "0.78rem", color: "#eab308" }}>
                  Pre-solved {briefing.presolved_count} likely request{briefing.presolved_count > 1 ? "s" : ""}
                </div>
              )}
            </div>
          ) : (
            <div style={{ color: "#64748b", fontSize: "0.82rem" }}>No briefing available</div>
          )}
        </div>

        {/* Dream Queue */}
        <div style={panelStyle}>
          <h3 style={headStyle}>Dream Queue ({queue.length} pending)</h3>
          {queue.length > 0 ? queue.map((item, i) => (
            <div key={item.id} style={{
              display: "flex", alignItems: "center", gap: 12, padding: "8px 0",
              borderBottom: i < queue.length - 1 ? "1px solid #1e293b" : "none",
            }}>
              <span style={{ color: "#64748b", fontSize: "0.72rem", width: 20 }}>{i + 1}.</span>
              <span style={{
                fontSize: "0.72rem", padding: "2px 6px", borderRadius: 4,
                background: `${dreamTypeColor(item.dream_type)}20`,
                color: dreamTypeColor(item.dream_type),
              }}>
                {item.dream_type}
              </span>
              <span style={{ flex: 1, fontSize: "0.78rem", fontFamily: "monospace", color: "#e2e8f0" }}>
                {item.agent_id.length > 16 ? item.agent_id.slice(0, 16) + "..." : item.agent_id}
              </span>
              <span style={{ fontSize: "0.72rem", color: "#94a3b8" }}>
                Priority: {item.priority.toFixed(1)}
              </span>
            </div>
          )) : (
            <div style={{ color: "#64748b", fontSize: "0.82rem" }}>No dreams queued</div>
          )}
        </div>

        {/* Dream History */}
        <div style={panelStyle}>
          <h3 style={headStyle}>Dream History</h3>
          <div style={{ maxHeight: 350, overflowY: "auto" }}>
            {history.length > 0 ? history.map((entry) => (
              <div key={entry.id} style={{ padding: "8px 0", borderBottom: "1px solid #1e293b" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                  <span style={{ color: entry.status === "completed" ? "#22c55e" : "#ef4444", fontSize: "0.85rem" }}>
                    {statusIcon(entry.status)}
                  </span>
                  <span style={{
                    fontSize: "0.72rem", padding: "2px 6px", borderRadius: 4,
                    background: `${dreamTypeColor(entry.dream_type)}20`,
                    color: dreamTypeColor(entry.dream_type),
                  }}>
                    {entry.dream_type}
                  </span>
                  <span style={{ fontSize: "0.72rem", color: "#64748b", marginLeft: "auto" }}>
                    {formatTime(entry.timestamp)}
                  </span>
                </div>
                <div style={{ fontSize: "0.78rem", color: "#94a3b8" }}>{entry.summary}</div>
                {entry.score_before !== entry.score_after && (
                  <div style={{ fontSize: "0.72rem", color: "#22d3ee", marginTop: 2 }}>
                    Score: {entry.score_before}→{entry.score_after}/10
                  </div>
                )}
              </div>
            )) : (
              <div style={{ color: "#64748b", fontSize: "0.82rem" }}>No dream history</div>
            )}
          </div>
        </div>

        {/* Configuration */}
        <div style={panelStyle}>
          <h3 style={headStyle}>Configuration</h3>
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span style={{ fontSize: "0.82rem", color: "#94a3b8" }}>Enabled</span>
              <button type="button" onClick={() => setConfig((c) => ({ ...c, enabled: !c.enabled }))} style={{
                padding: "4px 16px", borderRadius: 4, cursor: "pointer", fontFamily: "monospace", fontSize: "0.78rem", fontWeight: 600,
                background: config.enabled ? "rgba(34,211,238,0.15)" : "rgba(100,116,139,0.15)",
                border: `1px solid ${config.enabled ? "#22d3ee" : "#475569"}`,
                color: config.enabled ? "#22d3ee" : "#64748b",
              }}>
                {config.enabled ? "ON" : "OFF"}
              </button>
            </div>

            <ConfigInput label="Idle trigger (minutes)" value={config.idle_trigger_minutes}
              onChange={(v) => setConfig((c) => ({ ...c, idle_trigger_minutes: v }))} />
            <ConfigInput label="Token budget" value={config.token_budget}
              onChange={(v) => setConfig((c) => ({ ...c, token_budget: v }))} />
            <ConfigInput label="API call budget" value={config.api_call_budget}
              onChange={(v) => setConfig((c) => ({ ...c, api_call_budget: v }))} />

            <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
              <button type="button" onClick={() => void handleSaveConfig()} disabled={saving} style={btnStyle}>
                {saving ? "Saving..." : "Save Settings"}
              </button>
              <button type="button" onClick={() => void handleTrigger()} disabled={triggering} style={{
                ...btnStyle, background: "rgba(167,139,250,0.15)", borderColor: "#a78bfa", color: "#a78bfa",
              }}>
                {triggering ? "Triggering..." : "Trigger Dream Now"}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ================================================================== */
/*  Sub-components                                                     */
/* ================================================================== */

function ConfigInput({ label, value, onChange }: { label: string; value: number; onChange: (v: number) => void }): JSX.Element {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
      <span style={{ fontSize: "0.82rem", color: "#94a3b8" }}>{label}</span>
      <input type="number" value={value} onChange={(e) => onChange(Number(e.target.value))}
        style={{
          width: 100, padding: "4px 8px", background: "#0f172a", border: "1px solid #334155",
          borderRadius: 4, color: "#e2e8f0", fontFamily: "monospace", fontSize: "0.82rem", textAlign: "right",
        }}
      />
    </div>
  );
}

/* ================================================================== */
/*  Styles                                                             */
/* ================================================================== */

const panelStyle: React.CSSProperties = {
  background: "rgba(15,23,42,0.7)",
  border: "1px solid #1e293b",
  borderRadius: 10,
  padding: 20,
  backdropFilter: "blur(8px)",
};

const headStyle: React.CSSProperties = {
  fontFamily: "monospace",
  fontSize: "0.95rem",
  color: "#a78bfa",
  marginBottom: 14,
  paddingBottom: 8,
  borderBottom: "1px solid #1e293b",
};

const btnStyle: React.CSSProperties = {
  padding: "8px 20px",
  background: "rgba(34,211,238,0.15)",
  border: "1px solid #22d3ee",
  borderRadius: 6,
  color: "#22d3ee",
  cursor: "pointer",
  fontFamily: "monospace",
  fontSize: "0.82rem",
  fontWeight: 600,
};
