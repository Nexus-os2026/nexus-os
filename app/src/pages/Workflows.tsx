import { useCallback, useEffect, useMemo, useState } from "react";
import { getScheduledAgents, hasDesktopRuntime, listAgents } from "../api/backend";
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
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const isDesktop = hasDesktopRuntime();

  const loadData = useCallback(async () => {
    if (!isDesktop) {
      setLoading(false);
      return;
    }
    setError(null);
    try {
      const [scheduled, registered] = await Promise.all([getScheduledAgents(), listAgents()]);
      setScheduledAgents(scheduled);
      setAgents(registered);
    } catch (loadError) {
      setError(String(loadError));
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

  return (
    <section className="wf-engine">
      <header className="wf-header">
        <div>
          <h2 className="wf-title">WORKFLOW ENGINE // {scheduledAgents.length} SCHEDULED WORKFLOWS</h2>
          <p className="wf-subtitle">Scheduled agents registered in the runtime</p>
        </div>
        <button type="button" className="wf-create-btn" disabled title="Workflow creation is not configured in this build">
          Workflow creation not configured
        </button>
      </header>

      {error ? (
        <div className="wf-history" style={{ marginBottom: "1rem" }}>
          <h3 className="wf-history-title">STATUS</h3>
          <div className="wf-history-table-wrap" style={{ padding: "1rem" }}>{error}</div>
        </div>
      ) : null}

      {loading ? (
        <div className="wf-history-table-wrap" style={{ padding: "2rem", textAlign: "center" }}>
          Loading workflows...
        </div>
      ) : null}

      {!loading && !isDesktop ? (
        <div className="wf-history-table-wrap" style={{ padding: "2rem", textAlign: "center" }}>
          No workflows
        </div>
      ) : null}

      {!loading && isDesktop && scheduledAgents.length === 0 ? (
        <div className="wf-history-table-wrap" style={{ padding: "2rem", textAlign: "center" }}>
          No workflows
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
          No workflow run history is available from the backend yet.
        </div>
      </section>
    </section>
  );
}
