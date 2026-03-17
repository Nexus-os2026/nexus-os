import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ActionButton,
  DataRow,
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandHeaderMetaStyle,
  commandInsetStyle,
  commandLabelStyle,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
  formatTimestamp,
  inputStyle,
  normalizeArray,
  textareaStyle,
  toTitleCase,
} from "./commandCenterUi";

interface ParliamentStatus {
  active_proposals: number;
  passed_rules: number;
  total_votes: number;
  proposals?: ProposalRecord[];
}

interface ProposalRecord {
  id: string;
  proposer_id: string;
  rule_text: string;
  votes_for: number;
  votes_against: number;
  status: string;
  created_at: number;
  expires_at?: number;
}

interface RoleAssignment {
  role: string;
  agent_id: string;
  elected_at: number;
  term_expires_at?: number;
  election_score?: number;
}

interface EconomyStatus {
  total_agents: number;
  total_tokens_circulating: number;
  transactions_today: number;
  balances?: TokenBalance[];
  transactions?: TransactionRecord[];
}

interface TokenBalance {
  agent_id: string;
  balance: number;
}

interface TransactionRecord {
  from: string;
  to: string;
  amount: number;
  reason: string;
  timestamp: number;
}

interface DisputeRecord {
  id: string;
  agent_a: string;
  agent_b: string;
  issue: string;
  status: string;
  resolution?: string | null;
  arbiter_id?: string | null;
  created_at: number;
}

interface GovernanceEvent {
  id?: string;
  event_type: string;
  details: string;
  timestamp: number;
}

interface AgentOption {
  id: string;
  name: string;
}

const ROLE_OPTIONS = ["Coordinator", "Auditor", "Researcher", "Guardian"] as const;

function proposalTone(proposal: ProposalRecord): { label: string; color: string } {
  const status = String(proposal.status).toLowerCase();
  if (status === "passed") return { label: "PASSED", color: "#22c55e" };
  if (status === "rejected") return { label: "FAILED", color: "#ef4444" };
  if (status === "expired") return { label: "CLOSED", color: "#94a3b8" };
  if (proposal.votes_for > proposal.votes_against) return { label: "PASSING", color: "#22c55e" };
  if (proposal.votes_against > proposal.votes_for) return { label: "FAILING", color: "#ef4444" };
  return { label: "ACTIVE", color: "#94a3b8" };
}

function roleColor(role: string): string {
  switch (String(role)) {
    case "Coordinator":
      return "#00ffcc";
    case "Auditor":
      return "#38bdf8";
    case "Researcher":
      return "#a78bfa";
    case "Guardian":
      return "#f59e0b";
    default:
      return "#94a3b8";
  }
}

function mergeProposals(remote: ProposalRecord[], local: ProposalRecord[]): ProposalRecord[] {
  const entries = new Map<string, ProposalRecord>();
  for (const proposal of [...remote, ...local]) {
    entries.set(proposal.id, proposal);
  }
  return Array.from(entries.values()).sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
}

function agentName(agents: AgentOption[], id: string): string {
  return agents.find((agent) => agent.id === id)?.name ?? id;
}

export default function CivilizationPage(): JSX.Element {
  const [parliament, setParliament] = useState<ParliamentStatus | null>(null);
  const [roles, setRoles] = useState<RoleAssignment[]>([]);
  const [economy, setEconomy] = useState<EconomyStatus | null>(null);
  const [agents, setAgents] = useState<AgentOption[]>([]);
  const [remoteLog, setRemoteLog] = useState<GovernanceEvent[]>([]);
  const [sessionLog, setSessionLog] = useState<GovernanceEvent[]>([]);
  const [localProposals, setLocalProposals] = useState<ProposalRecord[]>([]);
  const [disputes, setDisputes] = useState<DisputeRecord[]>([]);
  const [voteAgent, setVoteAgent] = useState("");
  const [proposalText, setProposalText] = useState("");
  const [electionRole, setElectionRole] = useState<string>("Coordinator");
  const [disputeAgentA, setDisputeAgentA] = useState("");
  const [disputeAgentB, setDisputeAgentB] = useState("");
  const [disputeIssue, setDisputeIssue] = useState("");
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const [parliamentResult, rolesResult, economyResult, logResult, agentResult] = await Promise.allSettled([
        invoke<ParliamentStatus>("civ_get_parliament_status"),
        invoke<RoleAssignment[]>("civ_get_roles"),
        invoke<EconomyStatus>("civ_get_economy_status"),
        invoke<GovernanceEvent[]>("civ_get_governance_log", { limit: 10 }),
        invoke<AgentOption[]>("list_agents"),
      ]);

      if (parliamentResult.status === "fulfilled") setParliament(parliamentResult.value);
      if (rolesResult.status === "fulfilled") setRoles(normalizeArray<RoleAssignment>(rolesResult.value));
      if (economyResult.status === "fulfilled") setEconomy(economyResult.value);
      if (logResult.status === "fulfilled") setRemoteLog(normalizeArray<GovernanceEvent>(logResult.value));
      if (agentResult.status === "fulfilled") setAgents(normalizeArray<AgentOption>(agentResult.value));
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (agents.length === 0 || voteAgent) return;
    setVoteAgent(agents[0].id);
    setDisputeAgentA(agents[0].id);
    setDisputeAgentB((agents[1] ?? agents[0]).id);
  }, [agents, voteAgent]);

  const appendLog = useCallback((eventType: string, details: string) => {
    setSessionLog((current) => [
      {
        id: `${Date.now()}-${current.length}`,
        event_type: eventType,
        details,
        timestamp: Math.floor(Date.now() / 1000),
      },
      ...current,
    ]);
  }, []);

  const handleProposeRule = useCallback(async () => {
    if (!proposalText.trim()) return;
    setWorking("proposal");
    setError(null);
    try {
      const proposal = await invoke<ProposalRecord>("civ_propose_rule", {
        proposerId: voteAgent || agents[0]?.id,
        ruleText: proposalText.trim(),
      });
      setLocalProposals((current) => mergeProposals([], [proposal, ...current]));
      appendLog("ProposalCreated", `Proposal submitted by ${agentName(agents, (voteAgent || agents[0]?.id) ?? "")}`);
      setProposalText("");
    } catch (proposalError) {
      setError(proposalError instanceof Error ? proposalError.message : String(proposalError));
    } finally {
      setWorking(null);
    }
  }, [agents, appendLog, proposalText, voteAgent]);

  const handleVote = useCallback(async (proposalId: string, inFavor: boolean) => {
    if (!voteAgent) {
      setError("Select an agent before voting.");
      return;
    }
    setWorking(`vote-${proposalId}`);
    setError(null);
    try {
      await invoke("civ_vote", {
        agentId: voteAgent,
        proposalId,
        vote: inFavor,
      });
      setLocalProposals((current) =>
        current.map((proposal) => {
          if (proposal.id !== proposalId) return proposal;
          return {
            ...proposal,
            votes_for: proposal.votes_for + (inFavor ? 1 : 0),
            votes_against: proposal.votes_against + (inFavor ? 0 : 1),
          };
        })
      );
      appendLog("VoteCast", `${agentName(agents, voteAgent)} voted ${inFavor ? "YES" : "NO"} on ${proposalId}`);
    } catch (voteError) {
      setError(voteError instanceof Error ? voteError.message : String(voteError));
    } finally {
      setWorking(null);
    }
  }, [agents, appendLog, voteAgent]);

  const handleElection = useCallback(async () => {
    setWorking("election");
    setError(null);
    try {
      await invoke("civ_run_election", { role: electionRole });
      appendLog("ElectionHeld", `Election triggered for ${electionRole}`);
      await refresh();
    } catch (electionError) {
      setError(electionError instanceof Error ? electionError.message : String(electionError));
    } finally {
      setWorking(null);
    }
  }, [appendLog, electionRole, refresh]);

  const handleFileDispute = useCallback(async () => {
    if (!disputeAgentA || !disputeAgentB || !disputeIssue.trim()) return;
    setWorking("dispute");
    setError(null);
    try {
      const dispute = await invoke<DisputeRecord>("civ_resolve_dispute", {
        agentA: disputeAgentA,
        agentB: disputeAgentB,
        issue: disputeIssue.trim(),
      });
      setDisputes((current) => [dispute, ...current]);
      appendLog("DisputeFiled", `${agentName(agents, disputeAgentA)} vs ${agentName(agents, disputeAgentB)}: ${disputeIssue.trim()}`);
      setDisputeIssue("");
    } catch (disputeError) {
      setError(disputeError instanceof Error ? disputeError.message : String(disputeError));
    } finally {
      setWorking(null);
    }
  }, [agents, appendLog, disputeAgentA, disputeAgentB, disputeIssue]);

  const proposals = useMemo(() => {
    const remote = normalizeArray<ProposalRecord>(parliament?.proposals);
    return mergeProposals(remote, localProposals);
  }, [localProposals, parliament?.proposals]);

  const topBalances = useMemo(() => {
    return normalizeArray<TokenBalance>(economy?.balances)
      .slice()
      .sort((a, b) => (b.balance ?? 0) - (a.balance ?? 0));
  }, [economy?.balances]);

  const transactions = useMemo(() => {
    return normalizeArray<TransactionRecord>(economy?.transactions)
      .slice()
      .sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0));
  }, [economy?.transactions]);

  const governanceFeed = useMemo(() => {
    return [...sessionLog, ...remoteLog].sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0)).slice(0, 10);
  }, [remoteLog, sessionLog]);

  return (
    <div style={commandPageStyle}>
      <div style={{ marginBottom: 20 }}>
        <h1 style={{ margin: 0, fontFamily: "monospace", fontSize: "1.8rem", color: "#00ffcc", letterSpacing: "0.16em", textTransform: "uppercase" }}>
          Agent Civilization
        </h1>
        <div style={{ ...commandHeaderMetaStyle, marginTop: 10 }}>
          <span>{parliament ? `${parliament.active_proposals} active proposals` : "Loading parliament"}</span>
          <span>{economy ? `${Math.round(economy.total_tokens_circulating).toLocaleString()} tokens circulating` : "Economy pending"}</span>
          <span>{roles.length} elected roles</span>
        </div>
      </div>

      {error ? <div style={{ marginBottom: 16, color: "#fca5a5", fontSize: "0.82rem" }}>{error}</div> : null}

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18, marginBottom: 18 }}>
        <Panel title="Parliament" accent="#00ffcc" style={{ minHeight: 420 }}>
          <div style={{ display: "grid", gap: 10, marginBottom: 14 }}>
            <select value={voteAgent} onChange={(event) => setVoteAgent(event.target.value)} style={inputStyle}>
              <option value="">Select voting agent</option>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>
            <textarea
              value={proposalText}
              onChange={(event) => setProposalText(event.target.value)}
              placeholder='Propose a rule, for example: "Max 10% token budget per agent"'
              style={textareaStyle}
            />
            <ActionButton accent="#00ffcc" disabled={working === "proposal"} onClick={() => void handleProposeRule()}>
              {working === "proposal" ? "Submitting..." : "Submit Proposal"}
            </ActionButton>
          </div>

          <div style={{ ...commandScrollStyle, maxHeight: 250, paddingRight: 6 }}>
            {loading ? <EmptyState text="Loading..." /> : null}
            {!loading && proposals.length === 0 ? <EmptyState text="No proposals yet. Use the form above to open the next vote." /> : null}
            {proposals.map((proposal) => {
              const tone = proposalTone(proposal);
              return (
                <article key={proposal.id} style={{ ...commandInsetStyle, marginBottom: 10 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
                    <span style={{ ...commandLabelStyle, color: tone.color }}>{tone.label}</span>
                    <span style={{ ...commandMonoValueStyle, color: tone.color }}>{formatTimestamp(proposal.created_at)}</span>
                  </div>
                  <div style={{ color: "#f8fafc", fontSize: "0.92rem", marginBottom: 10 }}>{proposal.rule_text}</div>
                  <DataRow label="Proposed by" value={agentName(agents, proposal.proposer_id)} />
                  <DataRow label="Votes" value={`${proposal.votes_for} for / ${proposal.votes_against} against`} valueColor={tone.color} />
                  <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
                    <ActionButton accent="#22c55e" disabled={working === `vote-${proposal.id}`} onClick={() => void handleVote(proposal.id, true)}>
                      Vote Yes
                    </ActionButton>
                    <ActionButton destructive disabled={working === `vote-${proposal.id}`} onClick={() => void handleVote(proposal.id, false)}>
                      Vote No
                    </ActionButton>
                  </div>
                </article>
              );
            })}
          </div>
        </Panel>

        <Panel title="Elected Roles" accent="#38bdf8" style={{ minHeight: 420 }}>
          <div style={{ ...commandScrollStyle, maxHeight: 255, paddingRight: 6 }}>
            {ROLE_OPTIONS.map((role) => {
              const assignment = roles.find((entry) => entry.role === role);
              const color = roleColor(role);
              return (
                <article key={role} style={{ ...commandInsetStyle, marginBottom: 10 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "center", marginBottom: 8 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <StatusDot color={color} />
                      <span style={{ ...commandMonoValueStyle, color }}>{role}</span>
                    </div>
                    <span style={{ ...commandLabelStyle, color }}>{assignment ? "Elected" : "Vacant"}</span>
                  </div>
                  <DataRow label="Agent" value={assignment ? agentName(agents, assignment.agent_id) : "No holder"} />
                  <DataRow label="Elected" value={assignment ? formatTimestamp(assignment.elected_at) : "-"} />
                </article>
              );
            })}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 10, marginTop: 12 }}>
            <select value={electionRole} onChange={(event) => setElectionRole(event.target.value)} style={inputStyle}>
              {ROLE_OPTIONS.map((role) => (
                <option key={role} value={role}>
                  {role}
                </option>
              ))}
            </select>
            <ActionButton accent="#38bdf8" disabled={working === "election"} onClick={() => void handleElection()}>
              {working === "election" ? "Running..." : "Trigger Election"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Economy" accent="#f59e0b" style={{ minHeight: 360 }}>
          <div style={{ ...commandInsetStyle, marginBottom: 12 }}>
            <DataRow label="Total tokens" value={economy ? Math.round(economy.total_tokens_circulating).toLocaleString() : "Loading..."} valueColor="#fbbf24" />
            <DataRow label="Agents" value={economy?.total_agents ?? "Loading..."} />
            <DataRow label="Transactions today" value={economy?.transactions_today ?? "Loading..."} />
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
            <div>
              <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Top Balances</div>
              {topBalances.length === 0 ? <EmptyState text="No balances reported by the economy engine" compact /> : null}
              {topBalances.slice(0, 6).map((balance) => (
                <div key={balance.agent_id} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                  <DataRow label={agentName(agents, balance.agent_id)} value={Math.round(balance.balance).toLocaleString()} valueColor="#fbbf24" />
                </div>
              ))}
            </div>
            <div>
              <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Recent Transactions</div>
              {transactions.length === 0 ? <EmptyState text="No transactions reported yet" compact /> : null}
              {transactions.slice(0, 6).map((transaction, index) => (
                <div key={`${transaction.from}-${transaction.to}-${index}`} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                  <div style={{ ...commandMonoValueStyle, color: "#f8fafc", marginBottom: 6 }}>
                    {agentName(agents, transaction.from)} {"->"} {agentName(agents, transaction.to)}
                  </div>
                  <DataRow label="Amount" value={`${Math.round(transaction.amount)} tk`} valueColor="#fbbf24" />
                  <div style={commandMutedStyle}>{transaction.reason}</div>
                </div>
              ))}
            </div>
          </div>
        </Panel>

        <Panel title="Disputes" accent="#f87171" style={{ minHeight: 360 }}>
          <div style={{ display: "grid", gap: 10, marginBottom: 14 }}>
            <select value={disputeAgentA} onChange={(event) => setDisputeAgentA(event.target.value)} style={inputStyle}>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>
            <select value={disputeAgentB} onChange={(event) => setDisputeAgentB(event.target.value)} style={inputStyle}>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>
            <textarea
              value={disputeIssue}
              onChange={(event) => setDisputeIssue(event.target.value)}
              placeholder="Describe the dispute..."
              style={textareaStyle}
            />
            <ActionButton destructive disabled={working === "dispute"} onClick={() => void handleFileDispute()}>
              {working === "dispute" ? "Filing..." : "File Dispute"}
            </ActionButton>
          </div>

          <div style={{ ...commandScrollStyle, maxHeight: 180, paddingRight: 6 }}>
            {disputes.length === 0 ? <EmptyState text="No active disputes" compact /> : null}
            {disputes.map((dispute) => (
              <article key={dispute.id} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 10, marginBottom: 8 }}>
                  <span style={{ ...commandMonoValueStyle, color: "#fca5a5" }}>
                    {agentName(agents, dispute.agent_a)} vs {agentName(agents, dispute.agent_b)}
                  </span>
                  <span style={{ ...commandLabelStyle, color: "#f87171" }}>{toTitleCase(dispute.status)}</span>
                </div>
                <div style={{ ...commandMutedStyle, marginBottom: 6 }}>{dispute.issue}</div>
                <DataRow label="Filed" value={formatTimestamp(dispute.created_at)} />
              </article>
            ))}
          </div>
        </Panel>
      </div>

      <Panel title="Governance Log" accent="#00ffcc">
        <div style={{ ...commandScrollStyle, maxHeight: 220, paddingRight: 6 }}>
          {governanceFeed.length === 0 ? <EmptyState text="No governance actions yet" /> : null}
          {governanceFeed.map((event, index) => (
            <div key={`${event.id ?? event.details}-${index}`} style={{ ...commandInsetStyle, marginBottom: 8 }}>
              <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
                <span style={{ ...commandLabelStyle, color: "#00ffcc" }}>{toTitleCase(event.event_type)}</span>
                <span style={{ ...commandMonoValueStyle, color: "#94a3b8" }}>{formatTimestamp(event.timestamp)}</span>
              </div>
              <div style={{ ...commandMutedStyle, marginBottom: 0 }}>{event.details}</div>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}
