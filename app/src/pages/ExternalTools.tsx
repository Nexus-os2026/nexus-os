import { useCallback, useEffect, useState } from "react";
import {
  toolsExecute,
  toolsGetAudit,
  toolsGetPolicy,
  toolsGetRegistry,
  toolsRefreshAvailability,
  toolsVerifyAudit,
  getRateLimitStatus,
} from "../api/backend";
import { alpha, commandPageStyle } from "./commandCenterUi";

const ACCENT = "#f59e0b";
const GREEN = "#22c55e";
const RED = "#ef4444";
const BLUE = "#3b82f6";

const CATEGORY_COLORS: Record<string, string> = {
  CodeRepository: "#8b5cf6",
  ProjectManagement: "#3b82f6",
  Communication: "#22c55e",
  Search: "#f59e0b",
  Database: "#ef4444",
  Storage: "#06b6d4",
  Webhook: "#ec4899",
  RestApi: "#64748b",
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

export default function ExternalTools() {
  const [tools, setTools] = useState<any[]>([]);
  const [audit, setAudit] = useState<any[]>([]);
  const [policy, setPolicy] = useState<any>(null);
  const [rateLimits, setRateLimits] = useState<any>(null);
  const [selectedTool, setSelectedTool] = useState("");
  const [paramsJson, setParamsJson] = useState("{}");
  const [agentId, setAgentId] = useState("agent-1");
  const [autonomy, setAutonomy] = useState(4);
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    const [reg, a, p] = await Promise.all([
      toolsGetRegistry().catch(() => []),
      toolsGetAudit(20).catch(() => []),
      toolsGetPolicy().catch(() => null),
    ]);
    setTools(Array.isArray(reg) ? reg : []);
    setAudit(Array.isArray(a) ? a : []);
    setPolicy(p);
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const handleRefreshAvail = useCallback(async () => {
    const reg = await toolsRefreshAvailability().catch(() => []);
    setTools(Array.isArray(reg) ? reg : []);
  }, []);

  const handleExecute = useCallback(async () => {
    if (!selectedTool) return;
    setLoading(true);
    setError("");
    setResult(null);
    try {
      const res = await toolsExecute(agentId, autonomy, selectedTool, paramsJson);
      setResult(res);
      const a = await toolsGetAudit(20).catch(() => []);
      setAudit(Array.isArray(a) ? a : []);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [selectedTool, paramsJson, agentId, autonomy]);

  const handleVerify = useCallback(async () => {
    try {
      const ok = await toolsVerifyAudit();
      setError(ok ? "" : "Audit chain BROKEN");
      if (ok) setResult({ success: true, response_body: "Audit chain verified" });
    } catch (e: any) {
      setError(String(e));
    }
  }, []);

  const selectedToolObj = tools.find((t) => t.id === selectedTool);

  return (
    <div style={{ ...commandPageStyle, padding: 24, color: "#e0e0e0" }}>
      <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 4 }}>
        <span style={{ color: ACCENT }}>External Tools</span>
      </h1>
      <p style={{ color: "#888", fontSize: 13, marginBottom: 16 }}>
        Governed API integrations — GitHub, Slack, Jira, search, webhooks, and more.
      </p>

      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        <button onClick={handleRefreshAvail} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Refresh Availability</button>
        <button onClick={handleVerify} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Verify Audit</button>
        <button onClick={() => getRateLimitStatus().then(setRateLimits).catch((e) => { if (import.meta.env.DEV) console.warn("[ExternalTools]", e); })} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Rate Limits</button>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        {/* Left: Tools grid + executor */}
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div style={cardStyle}>
            <div style={labelStyle}>Available Tools ({tools.filter((t) => t.available).length}/{tools.length})</div>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 8, marginTop: 8 }}>
              {tools.map((t) => (
                <div
                  key={t.id}
                  onClick={() => { setSelectedTool(t.id); setParamsJson("{}"); }}
                  style={{
                    padding: 10,
                    borderRadius: 6,
                    cursor: "pointer",
                    background: selectedTool === t.id ? alpha(ACCENT, 0.15) : alpha("#fff", 0.02),
                    border: selectedTool === t.id ? `1px solid ${ACCENT}` : "1px solid transparent",
                  }}
                >
                  <div style={{ fontSize: 13, fontWeight: 600 }}>{t.name}</div>
                  <div style={{ display: "flex", gap: 4, marginTop: 4, alignItems: "center" }}>
                    <span style={{
                      fontSize: 9,
                      padding: "1px 4px",
                      borderRadius: 3,
                      background: alpha(CATEGORY_COLORS[t.category] || "#888", 0.2),
                      color: CATEGORY_COLORS[t.category] || "#888",
                    }}>
                      {t.category}
                    </span>
                    <span style={{ fontSize: 9, color: t.available ? GREEN : RED }}>
                      {t.available ? "ready" : "no auth"}
                    </span>
                  </div>
                  <div style={{ fontSize: 10, color: "#555", marginTop: 2 }}>
                    L{t.min_autonomy_level}+ | {(t.cost_per_call / 1_000_000).toFixed(0)} NXC
                  </div>
                </div>
              ))}
            </div>
          </div>

          {selectedToolObj && (
            <div style={cardStyle}>
              <div style={labelStyle}>Execute: {selectedToolObj.name}</div>
              <p style={{ fontSize: 12, color: "#888", marginBottom: 8 }}>{selectedToolObj.description}</p>
              <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
                <input placeholder="Agent ID" value={agentId} onChange={(e) => setAgentId(e.target.value)} style={{ ...inputStyle, width: 140 }} />
                <div style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12 }}>
                  <span style={{ color: "#888" }}>L</span>
                  <input type="number" min={0} max={5} value={autonomy} onChange={(e) => setAutonomy(Number(e.target.value))} style={{ ...inputStyle, width: 50 }} />
                </div>
              </div>
              <textarea
                placeholder='{"action": "list_repos", "user": "octocat"}'
                value={paramsJson}
                onChange={(e) => setParamsJson(e.target.value)}
                rows={4}
                style={{ ...inputStyle, fontFamily: "monospace", fontSize: 12, resize: "vertical", marginBottom: 8 }}
              />
              <button
                onClick={handleExecute}
                disabled={loading}
                style={{ ...btnStyle, width: "100%", background: ACCENT, color: "#000" }}
              >
                {loading ? "Executing..." : "Execute"}
              </button>
            </div>
          )}

          {policy && (
            <div style={cardStyle}>
              <div style={labelStyle}>Governance Policy</div>
              <div style={{ fontSize: 12, display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6, marginTop: 6 }}>
                <div>Min Autonomy: <span style={{ color: BLUE }}>L{policy.min_autonomy_level}</span></div>
                <div>Side-effects approval: <span style={{ color: policy.side_effects_require_approval ? RED : GREEN }}>{policy.side_effects_require_approval ? "yes" : "no"}</span></div>
                <div>Max body: <span style={{ color: BLUE }}>{(policy.max_body_size_bytes / 1024).toFixed(0)} KB</span></div>
                <div>URL denylist: <span style={{ color: "#888" }}>{policy.url_denylist?.length || 0} patterns</span></div>
              </div>
            </div>
          )}
        </div>

        {/* Right: Result + Audit + Rate Limits */}
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          {rateLimits && (
            <div style={cardStyle}>
              <div style={labelStyle}>Rate Limit Status</div>
              <pre style={{ fontSize: 11, color: BLUE, marginTop: 6, whiteSpace: "pre-wrap" }}>
                {typeof rateLimits === "string" ? rateLimits : JSON.stringify(rateLimits, null, 2)}
              </pre>
            </div>
          )}
          {error && (
            <div style={{ ...cardStyle, borderColor: RED }}>
              <div style={{ ...labelStyle, color: RED }}>Error</div>
              <div style={{ fontSize: 13, color: RED }}>{error}</div>
            </div>
          )}

          {result && (
            <div style={cardStyle}>
              <div style={labelStyle}>Result</div>
              <div style={{ display: "flex", gap: 10, fontSize: 12, marginTop: 6 }}>
                <span style={{ color: result.success ? GREEN : RED }}>{result.success ? "Success" : "Failed"}</span>
                {result.status_code && <span style={{ color: "#888" }}>HTTP {result.status_code}</span>}
                {result.duration_ms !== undefined && <span style={{ color: "#888" }}>{result.duration_ms}ms</span>}
                {result.cost && <span style={{ color: ACCENT }}>{(result.cost / 1_000_000).toFixed(1)} NXC</span>}
              </div>
              {result.response_body && (
                <pre style={{ fontSize: 11, color: GREEN, background: alpha("#000", 0.3), padding: 10, borderRadius: 6, marginTop: 8, whiteSpace: "pre-wrap", maxHeight: 300, overflow: "auto" }}>
                  {result.response_body.substring(0, 3000)}
                </pre>
              )}
            </div>
          )}

          <div style={cardStyle}>
            <div style={labelStyle}>Audit Trail ({audit.length})</div>
            {audit.length === 0 && <div style={{ fontSize: 12, color: "#555", marginTop: 6 }}>No tool calls recorded yet</div>}
            <div style={{ display: "flex", flexDirection: "column", gap: 4, marginTop: 6 }}>
              {audit.slice().reverse().map((e) => (
                <div key={e.entry_id} style={{ display: "flex", gap: 8, fontSize: 11, padding: "4px 6px", background: alpha("#fff", 0.02), borderRadius: 4 }}>
                  <span style={{ color: e.success ? GREEN : RED, minWidth: 20 }}>{e.success ? "OK" : "ERR"}</span>
                  <span style={{ color: ACCENT, minWidth: 70 }}>{e.tool_id}</span>
                  <span style={{ flex: 1, color: "#888" }}>{e.action}</span>
                  <span style={{ color: "#555" }}>{e.agent_id}</span>
                  {e.has_side_effects && <span style={{ color: RED, fontSize: 9 }}>side-effect</span>}
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
