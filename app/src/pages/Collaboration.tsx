import { useCallback, useEffect, useState } from "react";
import {
  collabAddParticipant,
  collabCastVote,
  collabCreateSession,
  collabDeclareConsensus,
  collabDetectConsensus,
  collabGetPatterns,
  collabGetPolicy,
  collabGetSession,
  collabListActive,
  collabSendMessage,
  collabStart,
  collabCallVote,
} from "../api/backend";
import { alpha, commandPageStyle } from "./commandCenterUi";

const ACCENT = "#06b6d4";
const GREEN = "#22c55e";
const RED = "#ef4444";
const YELLOW = "#eab308";
const BLUE = "#3b82f6";

const MSG_COLORS: Record<string, string> = {
  Propose: "#f59e0b",
  Agree: "#22c55e",
  Disagree: "#ef4444",
  ShareReasoning: "#3b82f6",
  Question: "#8b5cf6",
  Answer: "#06b6d4",
  RaiseRisk: "#ef4444",
  AddContext: "#64748b",
  CallVote: "#f59e0b",
  Vote: "#eab308",
  DeclareConsensus: "#22c55e",
  EscalateToHuman: "#ef4444",
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

const MSG_TYPES = [
  "ShareReasoning", "Propose", "Agree", "Disagree", "Question",
  "Answer", "RaiseRisk", "AddContext", "CallVote",
];

export default function Collaboration() {
  const [sessions, setSessions] = useState<any[]>([]);
  const [patterns, setPatterns] = useState<any[]>([]);
  const [policy, setPolicy] = useState<any>(null);
  const [selectedSession, setSelectedSession] = useState<any>(null);
  const [consensus, setConsensus] = useState<any>(null);

  // Create form
  const [title, setTitle] = useState("");
  const [goal, setGoal] = useState("");
  const [pattern, setPattern] = useState("PeerReview");
  const [leadAgent, setLeadAgent] = useState("lead-agent");

  // Add participant form
  const [newAgentId, setNewAgentId] = useState("");
  const [newRole, setNewRole] = useState("Contributor");

  // Message form
  const [msgFrom, setMsgFrom] = useState("");
  const [msgType, setMsgType] = useState("Propose");
  const [msgText, setMsgText] = useState("");
  const [msgConfidence, setMsgConfidence] = useState(0.8);

  const [status, setStatus] = useState("");
  const [voteError, setVoteError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([
      collabListActive().catch(() => []),
      collabGetPatterns().catch(() => []),
      collabGetPolicy().catch(() => null),
    ]).then(([s, p, pol]) => {
      setSessions(Array.isArray(s) ? s : []);
      setPatterns(Array.isArray(p) ? p : []);
      setPolicy(pol);
    }).finally(() => setLoading(false));
  }, []);

  const refresh = useCallback(async () => {
    const s = await collabListActive().catch(() => []);
    setSessions(Array.isArray(s) ? s : []);
    if (selectedSession) {
      const updated = await collabGetSession(selectedSession.id).catch(() => null);
      if (updated) {
        setSelectedSession(updated);
        const c = await collabDetectConsensus(updated.id).catch(() => null);
        setConsensus(c);
      }
    }
  }, [selectedSession]);

  const handleCreate = useCallback(async () => {
    if (!title.trim()) return;
    const id = await collabCreateSession(title, goal, pattern, leadAgent, 4);
    setStatus(`Created session ${id.slice(0, 8)}`);
    setTitle("");
    setGoal("");
    refresh();
  }, [title, goal, pattern, leadAgent, refresh]);

  const handleAddParticipant = useCallback(async () => {
    if (!selectedSession || !newAgentId.trim()) return;
    await collabAddParticipant(selectedSession.id, newAgentId, 3, newRole);
    setNewAgentId("");
    refresh();
  }, [selectedSession, newAgentId, newRole, refresh]);

  const handleStart = useCallback(async () => {
    if (!selectedSession) return;
    await collabStart(selectedSession.id);
    refresh();
  }, [selectedSession, refresh]);

  const handleSendMessage = useCallback(async () => {
    if (!selectedSession || !msgFrom.trim() || !msgText.trim()) return;
    await collabSendMessage(selectedSession.id, msgFrom, null, msgType, msgText, msgConfidence);
    setMsgText("");
    refresh();
  }, [selectedSession, msgFrom, msgType, msgText, msgConfidence, refresh]);

  const handleVote = useCallback(async (vote: string) => {
    if (!selectedSession || !msgFrom.trim()) return;
    await collabCastVote(selectedSession.id, msgFrom, vote, undefined);
    refresh();
  }, [selectedSession, msgFrom, refresh]);

  const handleDeclareConsensus = useCallback(async () => {
    if (!selectedSession) return;
    await collabDeclareConsensus(selectedSession.id, leadAgent, msgText || "Consensus reached", []);
    refresh();
  }, [selectedSession, leadAgent, msgText, refresh]);

  const selectSession = useCallback(async (s: any) => {
    const full = await collabGetSession(s.id).catch(() => s);
    setSelectedSession(full);
    setMsgFrom(full.participants?.[0]?.agent_id || "");
    const c = await collabDetectConsensus(s.id).catch(() => null);
    setConsensus(c);
  }, []);

  if (loading) return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "#64748b", fontSize: 14 }}>
      Loading...
    </div>
  );

  return (
    <div style={{ ...commandPageStyle, padding: 24, color: "#e0e0e0" }}>
      <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 4 }}>
        <span style={{ color: ACCENT }}>Agent Collaboration</span>
      </h1>
      <p style={{ color: "#888", fontSize: 13, marginBottom: 16 }}>
        Multi-agent collaboration sessions — debate, review, brainstorm, vote, and converge.
      </p>
      {status && <div style={{ fontSize: 12, color: GREEN, marginBottom: 8 }}>{status}</div>}

      <div style={{ display: "grid", gridTemplateColumns: "300px 1fr", gap: 16 }}>
        {/* Left sidebar: Sessions + Create */}
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <div style={cardStyle}>
            <div style={labelStyle}>Create Session</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6, marginTop: 6 }}>
              <input placeholder="Title" value={title} onChange={(e) => setTitle(e.target.value)} style={inputStyle} />
              <input placeholder="Goal" value={goal} onChange={(e) => setGoal(e.target.value)} style={inputStyle} />
              <select value={pattern} onChange={(e) => setPattern(e.target.value)} style={inputStyle}>
                {patterns.map((p) => <option key={p.id} value={p.id}>{p.id} — {p.description}</option>)}
              </select>
              <input placeholder="Lead Agent ID" value={leadAgent} onChange={(e) => setLeadAgent(e.target.value)} style={inputStyle} />
              <button type="button" onClick={handleCreate} style={{ ...btnStyle, background: ACCENT, color: "#000" }}>Create</button>
            </div>
          </div>

          <div style={cardStyle}>
            <div style={labelStyle}>Active Sessions ({sessions.length})</div>
            {sessions.map((s) => (
              <div
                key={s.id}
                onClick={() => selectSession(s)}
                style={{
                  padding: 8, borderRadius: 6, marginTop: 6, cursor: "pointer",
                  background: selectedSession?.id === s.id ? alpha(ACCENT, 0.15) : alpha("#fff", 0.02),
                  border: selectedSession?.id === s.id ? `1px solid ${ACCENT}` : "1px solid transparent",
                }}
              >
                <div style={{ fontSize: 13, fontWeight: 600 }}>{s.title}</div>
                <div style={{ fontSize: 10, color: "#888" }}>{s.status} | {s.participants?.length || 0} agents</div>
              </div>
            ))}
          </div>

          {policy && (
            <div style={cardStyle}>
              <div style={labelStyle}>Policy</div>
              <div style={{ fontSize: 12, display: "grid", gridTemplateColumns: "1fr 1fr", gap: 4, marginTop: 4 }}>
                <div>Min L: <span style={{ color: BLUE }}>{policy.min_autonomy_level}</span></div>
                <div>Max agents: <span style={{ color: BLUE }}>{policy.max_participants}</span></div>
                <div>Session: <span style={{ color: ACCENT }}>{(policy.session_creation_cost / 1e6).toFixed(0)} NXC</span></div>
                <div>Msg: <span style={{ color: ACCENT }}>{(policy.message_cost / 1e6).toFixed(1)} NXC</span></div>
              </div>
            </div>
          )}
        </div>

        {/* Right: Session detail */}
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {!selectedSession ? (
            <div style={{ ...cardStyle, textAlign: "center", padding: 40, color: "#555" }}>
              Select or create a session
            </div>
          ) : (
            <>
              {/* Participants + controls */}
              <div style={cardStyle}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <div>
                    <div style={labelStyle}>Participants ({selectedSession.participants?.length || 0})</div>
                    <div style={{ display: "flex", gap: 8, marginTop: 4, flexWrap: "wrap" }}>
                      {(selectedSession.participants || []).map((p: any) => (
                        <span key={p.agent_id} style={{ fontSize: 11, padding: "2px 8px", borderRadius: 4, background: alpha(ACCENT, 0.15), color: ACCENT }}>
                          {p.agent_id} ({typeof p.role === "string" ? p.role : Object.keys(p.role)[0]})
                        </span>
                      ))}
                    </div>
                  </div>
                  <div style={{ display: "flex", gap: 6 }}>
                    {selectedSession.status === "Forming" && (
                      <button type="button" onClick={handleStart} style={{ ...btnStyle, background: GREEN, color: "#000" }}>Start</button>
                    )}
                    <button type="button" onClick={refresh} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0" }}>Refresh</button>
                  </div>
                </div>
                {(selectedSession.status === "Forming" || selectedSession.status === "Active") && (
                  <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
                    <input placeholder="Agent ID" value={newAgentId} onChange={(e) => setNewAgentId(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
                    <select value={newRole} onChange={(e) => setNewRole(e.target.value)} style={{ ...inputStyle, width: 120 }}>
                      <option value="Contributor">Contributor</option>
                      <option value="Reviewer">Reviewer</option>
                      <option value="Observer">Observer</option>
                      <option value="lead">Lead</option>
                    </select>
                    <button type="button" onClick={handleAddParticipant} style={{ ...btnStyle, background: ACCENT, color: "#000" }}>Add</button>
                  </div>
                )}
              </div>

              {/* Consensus indicator */}
              {consensus && (
                <div style={{ ...cardStyle, padding: 10 }}>
                  <div style={{ fontSize: 11, color: "#888" }}>
                    Consensus: <span style={{ color: consensus.NaturalConsensus ? GREEN : consensus.Deadlocked ? RED : YELLOW }}>
                      {consensus.NaturalConsensus ? "Natural Consensus" : consensus.Deadlocked ? "Deadlocked" : consensus.InProgress ? "In Progress" : consensus.NoProposalYet ? "No Proposal Yet" : "No Messages"}
                    </span>
                  </div>
                </div>
              )}

              {/* Messages */}
              <div style={cardStyle}>
                <div style={labelStyle}>Messages ({selectedSession.messages?.length || 0})</div>
                <div style={{ maxHeight: 300, overflow: "auto", display: "flex", flexDirection: "column", gap: 6, marginTop: 8 }}>
                  {(selectedSession.messages || []).map((m: any) => (
                    <div key={m.id} style={{ padding: 8, borderRadius: 6, background: alpha("#fff", 0.02), borderLeft: `3px solid ${MSG_COLORS[m.message_type] || "#666"}` }}>
                      <div style={{ display: "flex", gap: 8, fontSize: 11, marginBottom: 4 }}>
                        <span style={{ color: ACCENT, fontWeight: 600 }}>{m.from_agent}</span>
                        <span style={{ color: MSG_COLORS[m.message_type] || "#666" }}>{m.message_type}</span>
                        <span style={{ color: "#555" }}>{(m.content?.confidence * 100).toFixed(0)}% conf</span>
                      </div>
                      <div style={{ fontSize: 13 }}>{m.content?.text}</div>
                    </div>
                  ))}
                </div>
              </div>

              {/* Send message / Vote */}
              {(selectedSession.status === "Active" || selectedSession.status === "Voting") && (
                <div style={cardStyle}>
                  <div style={labelStyle}>{selectedSession.status === "Voting" ? "Cast Vote" : "Send Message"}</div>
                  <div style={{ display: "flex", gap: 6, marginTop: 6 }}>
                    <input placeholder="As agent..." value={msgFrom} onChange={(e) => setMsgFrom(e.target.value)} style={{ ...inputStyle, width: 140 }} />
                    {selectedSession.status !== "Voting" && (
                      <select value={msgType} onChange={(e) => setMsgType(e.target.value)} style={{ ...inputStyle, width: 140 }}>
                        {MSG_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
                      </select>
                    )}
                  </div>
                  {selectedSession.status === "Voting" ? (
                    <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
                      <button type="button" onClick={() => handleVote("approve")} style={{ ...btnStyle, background: GREEN, color: "#000", flex: 1 }}>Approve</button>
                      <button type="button" onClick={() => handleVote("reject")} style={{ ...btnStyle, background: RED, color: "#fff", flex: 1 }}>Reject</button>
                      <button type="button" onClick={() => handleVote("abstain")} style={{ ...btnStyle, background: "#374151", color: "#e0e0e0", flex: 1 }}>Abstain</button>
                    </div>
                  ) : (
                    <>
                      <textarea placeholder="Message..." value={msgText} onChange={(e) => setMsgText(e.target.value)} rows={2} style={{ ...inputStyle, marginTop: 6, resize: "vertical" }} />
                      <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
                        <button type="button" onClick={handleSendMessage} style={{ ...btnStyle, background: ACCENT, color: "#000", flex: 1 }}>Send</button>
                        <button type="button" onClick={handleDeclareConsensus} style={{ ...btnStyle, background: GREEN, color: "#000" }}>Declare Consensus</button>
                        {(selectedSession.messages || []).some((m: any) => m.message_type === "Propose") && (
                          <button type="button" onClick={async () => {
                            setVoteError(null);
                            try {
                              const proposalMsg = (selectedSession.messages || []).find((m: any) => m.message_type === "Propose");
                              if (proposalMsg) {
                                await collabCallVote(selectedSession.id, proposalMsg.id, 0.5, 300);
                                refresh();
                              }
                            } catch (err) {
                              setVoteError(String(err));
                            }
                          }} style={{ ...btnStyle, background: YELLOW, color: "#000" }}>Call Vote</button>
                        )}
                      </div>
                    </>
                  )}
                </div>
              )}

              {voteError && (
                <div style={{ fontSize: 12, color: RED, padding: "6px 10px", background: "rgba(239,68,68,0.08)", borderRadius: 6 }}>
                  {voteError}
                </div>
              )}

              {/* Outcome */}
              {selectedSession.outcome && (
                <div style={{ ...cardStyle, borderColor: GREEN }}>
                  <div style={labelStyle}>Outcome</div>
                  <div style={{ fontSize: 14, fontWeight: 600, marginTop: 6 }}>{selectedSession.outcome.decision}</div>
                  <div style={{ fontSize: 12, color: "#888", marginTop: 4 }}>
                    Method: {typeof selectedSession.outcome.method === "string" ? selectedSession.outcome.method : Object.keys(selectedSession.outcome.method)[0]}
                    {" | "}Confidence: {(selectedSession.outcome.confidence * 100).toFixed(0)}%
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
