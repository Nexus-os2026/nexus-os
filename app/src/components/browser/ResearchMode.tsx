import { useCallback, useState } from "react";
import { ActivityStream } from "./ActivityStream";
import {
  hasDesktopRuntime,
  startResearch,
  researchAgentAction,
  completeResearch,
} from "../../api/backend";
import type {
  ActivityMessage,
  ResearchSessionState,
  SubAgentState,
} from "../../types";

function makeId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.floor(Math.random() * 100_000)}`;
}

const AGENT_COLORS = [
  "var(--nexus-accent)",
  "#3b82f6",
  "#f59e0b",
  "#8b5cf6",
  "#10b981",
];

const STATUS_LABELS: Record<string, string> = {
  searching: "Searching",
  reading: "Reading",
  extracting: "Extracting",
  merging: "Merging",
  idle: "Idle",
  done: "Done",
  error: "Error",
};

interface ResearchModeProps {
  activities: ActivityMessage[];
  onActivity: (
    type: ActivityMessage["message_type"],
    content: string,
    agentName?: string,
  ) => void;
  iframeSrc: string | null;
  onIframeSrc: (src: string | null) => void;
}

/** Generate search URLs for a research query. */
function searchUrls(query: string): string[] {
  const encoded = encodeURIComponent(query.slice(0, 40));
  return [
    `https://en.wikipedia.org/wiki/${encoded}`,
    `https://arxiv.org/search/?query=${encoded}`,
    `https://scholar.google.com/scholar?q=${encoded}`,
  ];
}

/** Simulate a single sub-agent's research cycle (search → read → extract → done). */
async function simulateAgent(
  session: ResearchSessionState,
  agent: SubAgentState,
  agentIdx: number,
  onActivity: ResearchModeProps["onActivity"],
  onAgentUpdate: (agentId: string, patch: Partial<SubAgentState>) => void,
  onIframeSrc: (src: string | null) => void,
): Promise<void> {
  const urls = searchUrls(agent.query);

  // Searching phase
  onActivity("searching", `Searching: "${agent.query}"`, agent.agent_name);
  onAgentUpdate(agent.agent_id, { status: "searching" });
  await delay(800 + agentIdx * 200);

  // Read each URL
  for (const url of urls) {
    onActivity("reading", `Reading: ${url}`, agent.agent_name);
    onAgentUpdate(agent.agent_id, {
      status: "reading",
      current_url: url,
      pages_visited: (agent.pages_visited || 0) + 1,
    });
    onIframeSrc(url);

    if (hasDesktopRuntime()) {
      try {
        await researchAgentAction(
          session.session_id,
          agent.agent_id,
          "reading",
          url,
        );
      } catch {
        // continue in mock mode
      }
    }

    await delay(1200 + Math.random() * 600);

    // Extract
    const finding = `Key finding from ${new URL(url).hostname}: ${agent.query.slice(0, 60)}`;
    onActivity(
      "extracting",
      `Extracted finding from ${new URL(url).hostname}`,
      agent.agent_name,
    );
    onAgentUpdate(agent.agent_id, {
      status: "extracting",
      findings: [...(agent.findings || []), finding],
    });

    if (hasDesktopRuntime()) {
      try {
        await researchAgentAction(
          session.session_id,
          agent.agent_id,
          "extracting",
          url,
          finding,
        );
      } catch {
        // continue
      }
    }

    await delay(600 + Math.random() * 400);
  }

  // Done
  onActivity(
    "info",
    `Completed with ${urls.length} findings`,
    agent.agent_name,
  );
  onAgentUpdate(agent.agent_id, { status: "done", current_url: null });

  if (hasDesktopRuntime()) {
    try {
      await researchAgentAction(
        session.session_id,
        agent.agent_id,
        "done",
      );
    } catch {
      // continue
    }
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

export function ResearchMode({
  activities,
  onActivity,
  iframeSrc,
  onIframeSrc,
}: ResearchModeProps): JSX.Element {
  const [topic, setTopic] = useState("");
  const [numAgents, setNumAgents] = useState(2);
  const [session, setSession] = useState<ResearchSessionState | null>(null);
  const [running, setRunning] = useState(false);

  // Track sub-agent state locally for real-time updates
  const [agentStates, setAgentStates] = useState<SubAgentState[]>([]);

  const updateAgent = useCallback(
    (agentId: string, patch: Partial<SubAgentState>) => {
      setAgentStates((prev) =>
        prev.map((a) => (a.agent_id === agentId ? { ...a, ...patch } : a)),
      );
    },
    [],
  );

  const handleStart = useCallback(async () => {
    if (!topic.trim() || running) return;
    setRunning(true);

    onActivity("info", `Research started: "${topic}" with ${numAgents} agents`, "Supervisor");

    let sess: ResearchSessionState;

    if (hasDesktopRuntime()) {
      try {
        sess = await startResearch(topic, numAgents);
      } catch {
        // Fallback to local session
        sess = localSession(topic, numAgents);
      }
    } else {
      sess = localSession(topic, numAgents);
    }

    setSession(sess);
    setAgentStates(sess.sub_agents);

    onActivity(
      "info",
      sess.supervisor_message,
      "Supervisor",
    );

    // Run all sub-agents concurrently
    const promises = sess.sub_agents.map((agent, idx) =>
      simulateAgent(sess, agent, idx, onActivity, updateAgent, onIframeSrc),
    );
    await Promise.all(promises);

    // Merge phase
    onActivity("merging", "Merging findings from all agents...", "Supervisor");
    setAgentStates((prev) => prev.map((a) => ({ ...a, status: "done" as const })));
    await delay(1500);

    if (hasDesktopRuntime()) {
      try {
        const completed = await completeResearch(sess.session_id);
        setSession(completed);
      } catch {
        // Mock completion
        setSession((prev) =>
          prev
            ? {
                ...prev,
                status: "complete",
                summary: `Research complete: ${topic}. Found key insights across ${numAgents} agents.`,
              }
            : prev,
        );
      }
    } else {
      setSession((prev) =>
        prev
          ? {
              ...prev,
              status: "complete",
              summary: `Research complete: ${topic}. Found insights across ${numAgents} agents. Connect desktop runtime for deeper content extraction.`,
            }
          : prev,
      );
    }

    onActivity("info", "Research complete — summary generated", "Supervisor");
    onIframeSrc(null);
    setRunning(false);
  }, [topic, numAgents, running, onActivity, onIframeSrc, updateAgent]);

  // Track which agent is "active" for the iframe view
  const activeAgent = agentStates.find(
    (a) => a.status === "reading" || a.status === "extracting",
  );

  return (
    <div className="research-mode">
      <div className="research-split">
        <div className="research-browser-panel">
          {activeAgent?.current_url || iframeSrc ? (
            <>
              <div className="research-browser-bar">
                {activeAgent && (
                  <span
                    className="research-browser-agent-tag"
                    style={{
                      color:
                        AGENT_COLORS[
                          agentStates.findIndex(
                            (a) => a.agent_id === activeAgent.agent_id,
                          ) % AGENT_COLORS.length
                        ],
                    }}
                  >
                    {activeAgent.agent_name}
                  </span>
                )}
                <span className="research-browser-url">
                  {activeAgent?.current_url ?? iframeSrc}
                </span>
              </div>
              <div className="browser-iframe-shell">
                <iframe
                  className="browser-iframe"
                  src={activeAgent?.current_url ?? iframeSrc ?? undefined}
                  title="Research Browser"
                  sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
                />
              </div>
            </>
          ) : (
            <div className="browser-iframe-shell browser-iframe-shell--placeholder">
              <div className="browser-placeholder">
                <span className="browser-placeholder-icon">⌁</span>
                <span className="browser-placeholder-text">Research Mode</span>
                <span className="browser-placeholder-hint">
                  Enter a topic in the right panel to start multi-agent research
                </span>
              </div>
            </div>
          )}
        </div>

        <aside className="research-sidebar">
          <div className="research-supervisor-panel">
            <div className="research-supervisor-header">
              <span className="research-supervisor-icon">◈</span>
              <span className="research-supervisor-title">Supervisor</span>
              {session && (
                <span className={`research-status-badge research-status-${session.status}`}>
                  {session.status}
                </span>
              )}
            </div>

            {!session ? (
              <div className="research-start-form">
                <input
                  type="text"
                  className="research-topic-input"
                  value={topic}
                  onChange={(e) => setTopic(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") void handleStart();
                  }}
                  placeholder="Enter research topic..."
                  disabled={running}
                />
                <div className="research-agent-count">
                  <label htmlFor="num-agents">Agents:</label>
                  <select
                    id="num-agents"
                    value={numAgents}
                    onChange={(e) => setNumAgents(Number(e.target.value))}
                    disabled={running}
                  >
                    {[1, 2, 3, 4, 5].map((n) => (
                      <option key={n} value={n}>
                        {n}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    className="research-start-btn"
                    onClick={() => void handleStart()}
                    disabled={running || !topic.trim()}
                  >
                    {running ? "Researching..." : "Start Research"}
                  </button>
                </div>
              </div>
            ) : (
              <div className="research-supervisor-status">
                <span className="research-supervisor-msg">
                  {session.supervisor_message}
                </span>
                {session.status === "complete" && (
                  <button
                    type="button"
                    className="research-new-btn"
                    onClick={() => {
                      setSession(null);
                      setAgentStates([]);
                      setTopic("");
                    }}
                  >
                    New Research
                  </button>
                )}
              </div>
            )}
          </div>

          {agentStates.length > 0 && (
            <div className="research-info-panel">
              <div className="research-section-title">Sub-Agents</div>
              <div className="research-agents-row">
                {agentStates.map((agent, i) => (
                  <div
                    key={agent.agent_id}
                    className={`research-agent-card ${
                      activeAgent?.agent_id === agent.agent_id ? "active" : ""
                    }`}
                    style={{
                      borderColor: AGENT_COLORS[i % AGENT_COLORS.length],
                    }}
                  >
                    <div className="research-agent-card-header">
                      <span
                        className="research-agent-name"
                        style={{ color: AGENT_COLORS[i % AGENT_COLORS.length] }}
                      >
                        {agent.agent_name}
                      </span>
                      <span
                        className={`research-agent-status research-agent-status-${agent.status}`}
                      >
                        {STATUS_LABELS[agent.status] ?? agent.status}
                      </span>
                    </div>
                    <div className="research-agent-query">
                      {agent.query}
                    </div>
                    {agent.current_url && (
                      <div className="research-agent-url">
                        {agent.current_url}
                      </div>
                    )}
                    <div className="research-agent-stats">
                      <span>Pages: {agent.pages_visited}</span>
                      <span>Findings: {agent.findings.length}</span>
                      <span>Fuel: {agent.fuel_used}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="research-results-panel">
            <div className="research-section-title">Research Activity</div>
            <div className="research-activity-panel">
              <ActivityStream messages={activities} />
            </div>
          </div>

          {session?.summary && (
            <div className="research-summary-panel">
              <div className="research-summary">
                <div className="research-summary-header">Research Summary</div>
                <pre className="research-summary-content">
                  {session.summary}
                </pre>
              </div>
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}

/** Create a local research session when backend is unavailable. */
function localSession(topic: string, numAgents: number): ResearchSessionState {
  const aspects = [
    "overview and key concepts",
    "recent developments and trends",
    "practical applications and examples",
    "challenges and limitations",
    "future directions and outlook",
  ];
  const agents: SubAgentState[] = Array.from(
    { length: numAgents },
    (_, i) => ({
      agent_id: makeId(),
      agent_name: `Sub-Agent-${i + 1}`,
      status: "searching" as const,
      current_url: null,
      query: `${topic} — ${aspects[i] ?? "additional details"}`,
      findings: [],
      pages_visited: 0,
      fuel_used: 0,
    }),
  );

  return {
    session_id: makeId(),
    topic,
    status: "running",
    supervisor_message: `Assigning research task to ${agents.map((a) => a.agent_name).join(" and ")}`,
    sub_agents: agents,
    summary: null,
    total_fuel_used: 0,
    pages_visited: 0,
  };
}
