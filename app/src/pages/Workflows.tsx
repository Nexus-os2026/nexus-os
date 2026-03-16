import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getAgentTaskHistory,
  getScheduledAgents,
  hasDesktopRuntime,
  listAgents,
  startHivemind,
} from "../api/backend";
import type { AgentSummary, ScheduledAgent } from "../types";
import "./workflows.css";

function formatNextRun(epoch: number): string {
  if (!epoch) {
    return "Not scheduled";
  }
  return new Date(epoch * 1000).toLocaleString();
}

export function Workflows(): JSX.Element {
  const [scheduledAgents, setScheduledAgents] = useState<ScheduledAgent[]>([]);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [history, setHistory] = useState<Record<string, unknown>[]>([]);
  const [goal, setGoal] = useState("Coordinate a short status sweep across active agents.");
  const [selectedAgentIds, setSelectedAgentIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const isDesktop = hasDesktopRuntime();

  const loadData = useCallback(async () => {
    if (!isDesktop) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setMessage(null);
    try {
      const [scheduled, registered] = await Promise.all([getScheduledAgents(), listAgents()]);
      setScheduledAgents(scheduled);
      setAgents(registered);
      setSelectedAgentIds((current) => current.length > 0 ? current : registered.slice(0, 3).map((agent) => agent.id));
      const histories = await Promise.all(
        registered.slice(0, 8).map((agent) => getAgentTaskHistory(agent.id, 8)),
      );
      const flattened = histories
        .flat()
        .sort((left, right) => {
          const leftCreated = String(left.created_at ?? "");
          const rightCreated = String(right.created_at ?? "");
          return rightCreated.localeCompare(leftCreated);
        })
        .slice(0, 20);
      setHistory(flattened);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }, [isDesktop]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const agentNameById = useMemo(
    () => new Map(agents.map((agent) => [agent.id, agent.name])),
    [agents],
  );

  const triggerHivemind = useCallback(async () => {
    if (selectedAgentIds.length === 0) {
      setMessage("Select at least one agent for the hivemind task.");
      return;
    }
    try {
      const session = await startHivemind(goal, selectedAgentIds);
      const sessionId =
        String(session.session_id ?? session.id ?? session.sessionId ?? "unknown");
      setMessage(`Hivemind session started: ${sessionId}`);
      await loadData();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [goal, loadData, selectedAgentIds]);

  return (
    <section className="wf-engine">
      <header className="wf-header">
        <div>
          <h2 className="wf-title">WORKFLOW ENGINE // {scheduledAgents.length} SCHEDULED WORKFLOWS</h2>
          <p className="wf-subtitle">Real scheduler state plus task history from `get_agent_task_history`.</p>
        </div>
        <button type="button" className="wf-create-btn" onClick={() => void loadData()}>
          Refresh
        </button>
      </header>

      {message ? (
        <div className="wf-history" style={{ marginBottom: "1rem" }}>
          <h3 className="wf-history-title">STATUS</h3>
          <div className="wf-history-table-wrap" style={{ padding: "1rem" }}>{message}</div>
        </div>
      ) : null}

      <section className="wf-history" style={{ marginBottom: "1rem" }}>
        <h3 className="wf-history-title">HIVEMIND</h3>
        <div className="wf-history-table-wrap" style={{ padding: "1rem", display: "grid", gap: "0.75rem" }}>
          <textarea
            value={goal}
            onChange={(event) => setGoal(event.target.value)}
            rows={3}
            style={{ width: "100%", background: "rgba(2, 6, 23, 0.7)", color: "inherit", border: "1px solid rgba(34, 211, 238, 0.18)", borderRadius: 12, padding: "0.85rem" }}
          />
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
            {agents.map((agent) => {
              const selected = selectedAgentIds.includes(agent.id);
              return (
                <button
                  key={agent.id}
                  type="button"
                  onClick={() =>
                    setSelectedAgentIds((current) =>
                      selected
                        ? current.filter((candidate) => candidate !== agent.id)
                        : [...current, agent.id],
                    )
                  }
                  style={{
                    borderRadius: 999,
                    padding: "0.45rem 0.9rem",
                    border: selected ? "1px solid rgba(34, 211, 238, 0.55)" : "1px solid rgba(148, 163, 184, 0.2)",
                    background: selected ? "rgba(34, 211, 238, 0.14)" : "rgba(15, 23, 42, 0.8)",
                    color: "inherit",
                  }}
                >
                  {agent.name}
                </button>
              );
            })}
          </div>
          <div>
            <button type="button" className="wf-create-btn" onClick={() => void triggerHivemind()}>
              Start Hivemind Task
            </button>
          </div>
        </div>
      </section>

      {loading ? (
        <div className="wf-history-table-wrap" style={{ padding: "2rem", textAlign: "center" }}>
          Loading workflows...
        </div>
      ) : null}

      {!loading && !isDesktop ? (
        <div className="wf-history-table-wrap" style={{ padding: "2rem", textAlign: "center" }}>
          Desktop runtime unavailable.
        </div>
      ) : null}

      <div className="wf-grid">
        {scheduledAgents.map((workflow) => (
          <article key={workflow.agent_id} className="wf-card success">
            <div className="wf-card-status-bar success" />
            <div className="wf-card-body">
              <h3 className="wf-card-name">
                {agentNameById.get(workflow.agent_id) ?? workflow.agent_id}
              </h3>
              <p className="wf-card-desc">{workflow.default_goal || "No workflow description"}</p>
              <div className="wf-card-meta">
                <span className="wf-card-nodes">1 scheduled task</span>
                <span className="wf-card-separator">|</span>
                <span className="wf-card-run-status success">Schedule active</span>
                <span className="wf-card-separator">|</span>
                <span className="wf-card-when">{workflow.cron_expression}</span>
              </div>
              <p className="wf-card-detail">Next run: {formatNextRun(workflow.next_run_epoch)}</p>
            </div>
          </article>
        ))}
      </div>

      <section className="wf-history">
        <h3 className="wf-history-title">EXECUTION HISTORY</h3>
        <div className="wf-history-table-wrap" style={{ padding: "1rem" }}>
          {history.length === 0 ? (
            "No workflow run history is available."
          ) : (
            <div style={{ display: "grid", gap: "0.75rem" }}>
              {history.map((item, index) => (
                <article key={String(item.id ?? index)} style={{ border: "1px solid rgba(34, 211, 238, 0.14)", borderRadius: 12, padding: "0.9rem", background: "rgba(2, 6, 23, 0.45)" }}>
                  <strong>{String(item.goal ?? item.goal_description ?? item.task_description ?? "Task")}</strong>
                  <p style={{ marginTop: 8, fontSize: "0.85rem", opacity: 0.78 }}>
                    Agent: {agentNameById.get(String(item.agent_id ?? "")) ?? String(item.agent_id ?? "unknown")}
                  </p>
                  <p style={{ marginTop: 4, fontSize: "0.85rem", opacity: 0.78 }}>
                    Status: {String(item.status ?? "unknown")}
                  </p>
                </article>
              ))}
            </div>
          )}
        </div>
      </section>
    </section>
  );
}
