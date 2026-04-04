import React, { useCallback, useEffect, useState } from "react";
import {
  schedulerCreate,
  schedulerDelete,
  schedulerDisable,
  schedulerEnable,
  schedulerList,
  schedulerTriggerNow,
} from "../api/backend";

/* ---------- types --------------------------------------------------------- */

interface ScheduleEntry {
  id: string;
  agent_did: string;
  name: string;
  description: string;
  trigger: Record<string, unknown>;
  task: { task_type: string; parameters: Record<string, unknown>; timeout_seconds: number };
  enabled: boolean;
  created_at: string;
  last_run: string | null;
  next_run: string | null;
  run_count: number;
  max_runs: number | null;
  max_fuel_per_run: number;
  requires_hitl: boolean;
  on_failure: string | Record<string, unknown>;
}

type TriggerKind = "Cron" | "Interval" | "OneShot" | "Webhook" | "Event";

const CRON_PRESETS: { label: string; value: string }[] = [
  { label: "Every minute", value: "* * * * *" },
  { label: "Every 5 minutes", value: "*/5 * * * *" },
  { label: "Every 15 minutes", value: "*/15 * * * *" },
  { label: "Every hour", value: "0 * * * *" },
  { label: "Every day at 3 AM", value: "0 3 * * *" },
  { label: "Daily at midnight", value: "0 0 * * *" },
  { label: "Weekly (Mon 9 AM)", value: "0 9 * * 1" },
];

/* ---------- helpers ------------------------------------------------------- */

function triggerLabel(trigger: Record<string, unknown>): string {
  if ("Cron" in trigger) {
    const c = trigger.Cron as { expression: string };
    return `Cron: ${c.expression}`;
  }
  if ("Interval" in trigger) {
    const i = trigger.Interval as { seconds: number };
    return `Every ${i.seconds}s`;
  }
  if ("OneShot" in trigger) {
    const o = trigger.OneShot as { at: string };
    return `Once at ${new Date(o.at).toLocaleString()}`;
  }
  if ("Webhook" in trigger) {
    const w = trigger.Webhook as { path: string };
    return `Webhook: ${w.path}`;
  }
  if ("Event" in trigger) {
    return "Event";
  }
  return "Unknown";
}

function fmtDate(iso: string | null): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleString();
}

/* ---------- main component ------------------------------------------------ */

export default function Scheduler(): JSX.Element {
  const [entries, setEntries] = useState<ScheduleEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [showCreate, setShowCreate] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [runResult, setRunResult] = useState<string | null>(null);

  // Create form state
  const [name, setName] = useState("");
  const [agentDid, setAgentDid] = useState("");
  const [triggerKind, setTriggerKind] = useState<TriggerKind>("Cron");
  const [cronExpr, setCronExpr] = useState("*/5 * * * *");
  const [intervalSec, setIntervalSec] = useState(300);
  const [webhookPath, setWebhookPath] = useState("/hooks/my-agent");
  const [webhookSecret, setWebhookSecret] = useState("");
  const [taskType, setTaskType] = useState("run_agent");
  const [taskParams, setTaskParams] = useState('{"agent_did": "", "input": {}}');
  const [maxFuel, setMaxFuel] = useState(5000);
  const [requiresHitl, setRequiresHitl] = useState(false);
  const [failurePolicy, setFailurePolicy] = useState("Ignore");

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      const data = await schedulerList();
      const parsed = typeof data === "string" ? JSON.parse(data) : data;
      setEntries(Array.isArray(parsed) ? parsed : []);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleCreate = async () => {
    setError(null);
    let trigger: Record<string, unknown>;
    switch (triggerKind) {
      case "Cron":
        trigger = { Cron: { expression: cronExpr, timezone: "UTC" } };
        break;
      case "Interval":
        trigger = { Interval: { seconds: intervalSec } };
        break;
      case "Webhook":
        trigger = { Webhook: { path: webhookPath, secret: webhookSecret || null, filter: null } };
        break;
      case "Event":
        trigger = { Event: { event_kind: { Custom: { name: "custom" } }, filter: null } };
        break;
      case "OneShot":
        trigger = { OneShot: { at: new Date(Date.now() + 60_000).toISOString() } };
        break;
    }
    let params: Record<string, unknown>;
    try {
      params = JSON.parse(taskParams);
    } catch {
      setError("Invalid JSON in task parameters");
      return;
    }
    const on_failure =
      failurePolicy === "Retry"
        ? { Retry: { max_attempts: 3, backoff_seconds: 30 } }
        : failurePolicy === "Alert"
          ? { Alert: { channel: "default" } }
          : failurePolicy;

    try {
      await schedulerCreate({
        id: crypto.randomUUID(),
        agent_did: agentDid,
        name,
        description: "",
        trigger,
        task: { task_type: taskType, parameters: params, timeout_seconds: 300 },
        enabled: true,
        created_at: new Date().toISOString(),
        last_run: null,
        next_run: null,
        run_count: 0,
        max_runs: null,
        max_fuel_per_run: maxFuel,
        requires_hitl: requiresHitl,
        on_failure,
      });
      setShowCreate(false);
      setName("");
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleToggle = async (entry: ScheduleEntry) => {
    try {
      if (entry.enabled) {
        await schedulerDisable(entry.id);
      } else {
        await schedulerEnable(entry.id);
      }
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await schedulerDelete(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleRunNow = async (id: string) => {
    setRunResult(null);
    try {
      const result = await schedulerTriggerNow(id);
      setRunResult(typeof result === "string" ? result : JSON.stringify(result, null, 2));
      await refresh();
    } catch (e) {
      setRunResult(`Error: ${e}`);
    }
  };

  return (
    <div style={{ padding: 24, maxWidth: 1200, margin: "0 auto", color: "#e2e8f0" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 700 }}>Background Scheduler</h1>
        <div style={{ display: "flex", gap: 8 }}>
          <button onClick={() => void refresh()} style={btnStyle}>Refresh</button>
          <button onClick={() => setShowCreate(!showCreate)} style={{ ...btnStyle, background: "rgba(37,99,235,0.3)", border: "1px solid rgba(37,99,235,0.5)", color: "#60a5fa" }}>
            {showCreate ? "Cancel" : "+ New Schedule"}
          </button>
        </div>
      </div>

      {error && (
        <div style={{ background: "rgba(239,68,68,0.12)", border: "1px solid rgba(239,68,68,0.3)", borderRadius: 6, padding: 12, marginBottom: 12, color: "#f87171" }}>
          {error}
          <button onClick={() => setError(null)} style={{ marginLeft: 8, cursor: "pointer", background: "none", border: "none", color: "#f87171", fontWeight: 700 }}>×</button>
        </div>
      )}

      {runResult && (
        <div style={{ background: "rgba(34,197,94,0.1)", border: "1px solid rgba(34,197,94,0.3)", borderRadius: 6, padding: 12, marginBottom: 12, fontFamily: "monospace", fontSize: 13, whiteSpace: "pre-wrap", maxHeight: 200, overflow: "auto", color: "#86efac" }}>
          {runResult}
          <button onClick={() => setRunResult(null)} style={{ float: "right", cursor: "pointer", background: "none", border: "none", fontWeight: 700, color: "#86efac" }}>×</button>
        </div>
      )}

      {/* ── Create Form ── */}
      {showCreate && (
        <div style={{ background: "rgba(30,41,59,0.6)", border: "1px solid rgba(100,116,139,0.3)", borderRadius: 8, padding: 20, marginBottom: 20 }}>
          <h3 style={{ marginTop: 0, color: "#e2e8f0" }}>Create Schedule</h3>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
            <label>
              Name
              <input value={name} onChange={(e) => setName(e.target.value)} style={inputStyle} placeholder="my-scheduled-task" />
            </label>
            <label>
              Agent DID
              <input value={agentDid} onChange={(e) => setAgentDid(e.target.value)} style={inputStyle} placeholder="agent-uuid" />
            </label>
            <label>
              Trigger Type
              <select value={triggerKind} onChange={(e) => setTriggerKind(e.target.value as TriggerKind)} style={inputStyle}>
                <option value="Cron">Cron</option>
                <option value="Interval">Interval</option>
                <option value="Webhook">Webhook</option>
                <option value="Event">Event</option>
                <option value="OneShot">One-Shot</option>
              </select>
            </label>
            {triggerKind === "Cron" && (
              <label>
                Cron Expression
                <select value={cronExpr} onChange={(e) => setCronExpr(e.target.value)} style={inputStyle}>
                  {CRON_PRESETS.map((p) => (
                    <option key={p.value} value={p.value}>{p.label} ({p.value})</option>
                  ))}
                </select>
              </label>
            )}
            {triggerKind === "Interval" && (
              <label>
                Interval (seconds)
                <input type="number" value={intervalSec} onChange={(e) => setIntervalSec(Number(e.target.value))} style={inputStyle} />
              </label>
            )}
            {triggerKind === "Webhook" && (
              <>
                <label>
                  Webhook Path
                  <input value={webhookPath} onChange={(e) => setWebhookPath(e.target.value)} style={inputStyle} />
                </label>
                <label>
                  HMAC Secret (optional)
                  <input value={webhookSecret} onChange={(e) => setWebhookSecret(e.target.value)} style={inputStyle} type="password" />
                </label>
              </>
            )}
            <label>
              Task Type
              <select value={taskType} onChange={(e) => setTaskType(e.target.value)} style={inputStyle}>
                <option value="run_agent">Run Agent</option>
                <option value="send_notification">Send Notification</option>
                <option value="execute_command">Execute Command</option>
              </select>
            </label>
            <label>
              Max Fuel Per Run
              <input type="number" value={maxFuel} onChange={(e) => setMaxFuel(Number(e.target.value))} style={inputStyle} />
            </label>
            <label style={{ gridColumn: "1 / -1" }}>
              Task Parameters (JSON)
              <textarea value={taskParams} onChange={(e) => setTaskParams(e.target.value)} style={{ ...inputStyle, fontFamily: "monospace", minHeight: 60 }} />
            </label>
            <label>
              Failure Policy
              <select value={failurePolicy} onChange={(e) => setFailurePolicy(e.target.value)} style={inputStyle}>
                <option value="Ignore">Ignore</option>
                <option value="Retry">Retry (3x)</option>
                <option value="Disable">Disable on Failure</option>
                <option value="Alert">Alert</option>
              </select>
            </label>
            <label style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 22 }}>
              <input type="checkbox" checked={requiresHitl} onChange={(e) => setRequiresHitl(e.target.checked)} />
              Require HITL Approval
            </label>
          </div>
          <button onClick={() => void handleCreate()} style={{ ...btnStyle, background: "rgba(22,163,74,0.3)", border: "1px solid rgba(22,163,74,0.5)", color: "#4ade80", marginTop: 12 }}>
            Create Schedule
          </button>
        </div>
      )}

      {/* ── Schedule Table ── */}
      {loading ? (
        <p style={{ color: "#94a3b8" }}>Loading schedules...</p>
      ) : entries.length === 0 ? (
        <div style={{ textAlign: "center", padding: 60, color: "#94a3b8" }}>
          <p style={{ fontSize: 18 }}>No schedules yet</p>
          <p>Create a schedule to automate agent tasks on cron, webhook, or event triggers.</p>
        </div>
      ) : (
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 14 }}>
            <thead>
              <tr style={{ borderBottom: "2px solid rgba(100,116,139,0.3)", textAlign: "left" }}>
                <th style={thStyle}>Name</th>
                <th style={thStyle}>Agent</th>
                <th style={thStyle}>Trigger</th>
                <th style={thStyle}>Task</th>
                <th style={thStyle}>Runs</th>
                <th style={thStyle}>Last Run</th>
                <th style={thStyle}>Next Run</th>
                <th style={thStyle}>Status</th>
                <th style={thStyle}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e) => (
                <tr key={e.id} style={{ borderBottom: "1px solid rgba(100,116,139,0.2)" }}>
                  <td style={tdStyle}>
                    <strong>{e.name}</strong>
                    {e.requires_hitl && <span style={{ marginLeft: 4, color: "#f59e0b", fontSize: 11 }}>[HITL]</span>}
                  </td>
                  <td style={{ ...tdStyle, fontFamily: "monospace", fontSize: 12 }}>{e.agent_did.slice(0, 12)}...</td>
                  <td style={tdStyle}>{triggerLabel(e.trigger)}</td>
                  <td style={tdStyle}>{e.task.task_type}</td>
                  <td style={tdStyle}>{e.run_count}{e.max_runs != null ? `/${e.max_runs}` : ""}</td>
                  <td style={tdStyle}>{fmtDate(e.last_run)}</td>
                  <td style={tdStyle}>{fmtDate(e.next_run)}</td>
                  <td style={tdStyle}>
                    <span style={{ display: "inline-block", width: 8, height: 8, borderRadius: "50%", background: e.enabled ? "#22c55e" : "#94a3b8", marginRight: 6 }} />
                    {e.enabled ? "Active" : "Disabled"}
                  </td>
                  <td style={tdStyle}>
                    <div style={{ display: "flex", gap: 4 }}>
                      <button onClick={() => void handleToggle(e)} style={smallBtnStyle}>
                        {e.enabled ? "Disable" : "Enable"}
                      </button>
                      <button onClick={() => void handleRunNow(e.id)} style={{ ...smallBtnStyle, background: "rgba(59,130,246,0.15)", color: "#60a5fa" }}>
                        Run Now
                      </button>
                      <button onClick={() => void handleDelete(e.id)} style={{ ...smallBtnStyle, background: "rgba(239,68,68,0.15)", color: "#f87171" }}>
                        Delete
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

const btnStyle: React.CSSProperties = {
  padding: "8px 16px",
  borderRadius: 6,
  border: "1px solid rgba(100,116,139,0.3)",
  cursor: "pointer",
  fontWeight: 600,
  fontSize: 14,
  background: "rgba(30,41,59,0.6)",
  color: "#e2e8f0",
};

const smallBtnStyle: React.CSSProperties = {
  padding: "4px 8px",
  borderRadius: 4,
  border: "1px solid rgba(100,116,139,0.3)",
  cursor: "pointer",
  fontSize: 12,
  background: "rgba(30,41,59,0.6)",
  color: "#e2e8f0",
};

const inputStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  padding: "8px 10px",
  borderRadius: 6,
  border: "1px solid rgba(100,116,139,0.4)",
  marginTop: 4,
  fontSize: 14,
  boxSizing: "border-box",
  background: "rgba(15,23,42,0.8)",
  color: "#e2e8f0",
};

const thStyle: React.CSSProperties = { padding: "8px 12px", fontWeight: 600, fontSize: 13, color: "#94a3b8" };
const tdStyle: React.CSSProperties = { padding: "10px 12px" };
