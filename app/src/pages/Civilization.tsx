import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import RequiresLlm from "../components/RequiresLlm";
import {
  civGetEconomyStatus,
  civGetGovernanceLog,
  civGetParliamentStatus,
  civGetRoles,
  civProposeRule,
  civResolveDispute,
  civVote,
  civRunElection,
  economyCreateWallet,
  economyGetWallet,
  economyEarn,
  economySpend,
  economyTransfer,
  economyFreezeWallet,
  economyGetHistory,
  economyGetStats,
  economyCreateContract,
  economyCompleteContract,
  economyListContracts,
  economyDisputeContract,
  economyAgentPerformance,
  paymentCreatePlan,
  paymentListPlans,
  paymentCreateInvoice,
  paymentPayInvoice,
  paymentGetRevenueStats,
  paymentCreatePayout,
  hasDesktopRuntime,
} from "../api/backend";
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

/* ─── Types ─── */

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

/* ─── Tab definition ─── */

type CivTab = "overview" | "wallets" | "contracts" | "parliament" | "payments";

const TAB_ITEMS: { key: CivTab; label: string; accent: string }[] = [
  { key: "overview", label: "Overview", accent: "#00ffcc" },
  { key: "parliament", label: "Parliament", accent: "#a78bfa" },
  { key: "wallets", label: "Economy / Wallets", accent: "#f59e0b" },
  { key: "contracts", label: "Contracts", accent: "#38bdf8" },
  { key: "payments", label: "Payments", accent: "#f472b6" },
];

/* ─── Helpers ─── */

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

function safeParse<T>(json: string | T): T {
  if (typeof json === "string") {
    try {
      return JSON.parse(json) as T;
    } catch {
      return json as unknown as T;
    }
  }
  return json;
}

function safeStr(val: unknown): string {
  if (typeof val === "string") return val;
  if (val === null || val === undefined) return "";
  return JSON.stringify(val, null, 2);
}

/* ─── Tab bar component ─── */

function TabBar({ active, onChange }: { active: CivTab; onChange: (t: CivTab) => void }): JSX.Element {
  return (
    <div style={{ display: "flex", gap: 4, marginBottom: 18, flexWrap: "wrap" }}>
      {TAB_ITEMS.map((tab) => {
        const isActive = active === tab.key;
        return (
          <button type="button"
            key={tab.key}
            onClick={() => onChange(tab.key)}
            style={{
              padding: "6px 16px",
              fontSize: "0.82rem",
              fontFamily: "monospace",
              letterSpacing: "0.08em",
              textTransform: "uppercase",
              border: `1px solid ${isActive ? tab.accent : alpha("#94a3b8", 0.3)}`,
              borderRadius: 6,
              background: isActive ? alpha(tab.accent, 0.15) : "transparent",
              color: isActive ? tab.accent : "#94a3b8",
              cursor: "pointer",
              transition: "all 0.15s ease",
            }}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}

/* ─── Main component ─── */

export default function CivilizationPage(): JSX.Element {
  const [activeTab, setActiveTab] = useState<CivTab>("overview");

  /* Original state */
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
  const [showWelcome, setShowWelcome] = useState(false);

  /* New state: wallets / economy */
  const [walletAgent, setWalletAgent] = useState("");
  const [walletData, setWalletData] = useState<string | null>(null);
  const [walletHistory, setWalletHistory] = useState<string | null>(null);
  const [economyStats, setEconomyStats] = useState<string | null>(null);
  const [agentPerf, setAgentPerf] = useState<string | null>(null);
  const [earnAmount, setEarnAmount] = useState("");
  const [earnDesc, setEarnDesc] = useState("");
  const [spendAmount, setSpendAmount] = useState("");
  const [spendType, setSpendType] = useState("compute");
  const [spendDesc, setSpendDesc] = useState("");
  const [transferTo, setTransferTo] = useState("");
  const [transferAmount, setTransferAmount] = useState("");
  const [transferDesc, setTransferDesc] = useState("");

  /* New state: contracts */
  const [contractAgent, setContractAgent] = useState("");
  const [contractList, setContractList] = useState<string | null>(null);
  const [contractClient, setContractClient] = useState("");
  const [contractDesc, setContractDesc] = useState("");
  const [contractCriteria, setContractCriteria] = useState("");
  const [contractReward, setContractReward] = useState("");
  const [contractPenalty, setContractPenalty] = useState("");
  const [contractDeadline, setContractDeadline] = useState("");
  const [completeContractId, setCompleteContractId] = useState("");
  const [completeSuccess, setCompleteSuccess] = useState(true);
  const [completeEvidence, setCompleteEvidence] = useState("");
  const [disputeContractId, setDisputeContractId] = useState("");
  const [disputeContractReason, setDisputeContractReason] = useState("");

  /* New state: payments */
  const [planName, setPlanName] = useState("");
  const [planPrice, setPlanPrice] = useState("");
  const [planInterval, setPlanInterval] = useState("monthly");
  const [planFeatures, setPlanFeatures] = useState("");
  const [plansList, setPlansList] = useState<string | null>(null);
  const [invoicePlanId, setInvoicePlanId] = useState("");
  const [invoiceBuyerId, setInvoiceBuyerId] = useState("");
  const [payInvoiceId, setPayInvoiceId] = useState("");
  const [revenueStats, setRevenueStats] = useState<string | null>(null);
  const [payoutDevId, setPayoutDevId] = useState("");
  const [payoutAgentId, setPayoutAgentId] = useState("");
  const [payoutAmount, setPayoutAmount] = useState("");
  const [payoutPeriod, setPayoutPeriod] = useState("");

  /* New state: parliament extras */
  const [proposeAgent, setProposeAgent] = useState("");
  const [proposeText, setProposeText] = useState("");

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

  // Auto-bootstrap civilization on first visit
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    let cancelled = false;

    const bootstrap = async () => {
      try {
        // Check if already running
        const statusRaw = await civGetParliamentStatus();
        const status = safeParse<ParliamentStatus>(statusRaw);
        if (!cancelled && status && (status.active_proposals > 0 || status.passed_rules > 0 || status.total_votes > 0)) {
          // Already initialized, just load all data
          setParliament(status);
          await refresh();
          return;
        }
      } catch {
        /* not yet initialized */
      }

      if (cancelled) return;

      // Show welcome screen for first-time users
      setShowWelcome(true);
      setLoading(false);
    };

    bootstrap();
    return () => { cancelled = true; };
  }, [refresh]);

  const foundCiv = useCallback(async (govType: string) => {
    setShowWelcome(false);
    setWorking(`Founding ${govType} civilization...`);
    try {
      await civProposeRule("system", `Governance model: ${govType} — all agents must operate within declared fuel budgets`);
    } catch { /* ignore */ }
    try {
      await civProposeRule("system", "Agent actions require audit trail entries");
    } catch { /* ignore */ }
    try {
      await economyCreateWallet("treasury");
    } catch { /* ignore */ }
    try {
      await economyEarn("treasury", 10000, "Initial treasury funding");
    } catch { /* ignore */ }
    try {
      const ps = await civGetParliamentStatus();
      setParliament(safeParse<ParliamentStatus>(ps));
    } catch { /* ignore */ }
    try {
      const rs = await civGetRoles();
      setRoles(normalizeArray<RoleAssignment>(safeParse(rs)));
    } catch { /* ignore */ }
    await refresh();
    setWorking(null);
  }, [refresh]);

  useEffect(() => {
    if (agents.length === 0 || voteAgent) return;
    setVoteAgent(agents[0].id);
    setDisputeAgentA(agents[0].id);
    setDisputeAgentB((agents[1] ?? agents[0]).id);
  }, [agents, voteAgent]);

  /* Load economy stats when wallets tab is active */
  useEffect(() => {
    if (activeTab !== "wallets") return;
    void (async () => {
      try {
        const stats = await economyGetStats();
        setEconomyStats(safeStr(stats));
      } catch {
        /* ignore */
      }
    })();
  }, [activeTab]);

  /* Load plans and revenue when payments tab is active */
  useEffect(() => {
    if (activeTab !== "payments") return;
    void (async () => {
      try {
        const [plans, rev] = await Promise.allSettled([paymentListPlans(), paymentGetRevenueStats()]);
        if (plans.status === "fulfilled") setPlansList(safeStr(plans.value));
        if (rev.status === "fulfilled") setRevenueStats(safeStr(rev.value));
      } catch {
        /* ignore */
      }
    })();
  }, [activeTab]);

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

  /* ─── Original handlers ─── */

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
      await civVote(proposalId, voteAgent, String(inFavor));
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
      await civRunElection(electionRole);
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

  /* ─── New handlers: wallet actions ─── */

  const handleAction = useCallback(async (key: string, fn: () => Promise<unknown>, successMsg: string) => {
    setWorking(key);
    setError(null);
    try {
      const result = await fn();
      appendLog(key, `${successMsg}: ${safeStr(result).slice(0, 200)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setWorking(null);
    }
  }, [appendLog]);

  const handleFetchWallet = useCallback(async () => {
    if (!walletAgent) return;
    setWorking("fetch-wallet");
    setError(null);
    try {
      const [wallet, history, perf] = await Promise.allSettled([
        economyGetWallet(walletAgent),
        economyGetHistory(walletAgent),
        economyAgentPerformance(walletAgent),
      ]);
      if (wallet.status === "fulfilled") setWalletData(safeStr(wallet.value));
      if (history.status === "fulfilled") setWalletHistory(safeStr(history.value));
      if (perf.status === "fulfilled") setAgentPerf(safeStr(perf.value));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setWorking(null);
    }
  }, [walletAgent]);

  const handleCreateWallet = useCallback(async () => {
    if (!walletAgent) return;
    await handleAction("create-wallet", () => economyCreateWallet(walletAgent), "Wallet created");
    void handleFetchWallet();
  }, [walletAgent, handleAction, handleFetchWallet]);

  const handleEarn = useCallback(async () => {
    if (!walletAgent || !earnAmount) return;
    await handleAction("earn", () => economyEarn(walletAgent, Number(earnAmount), earnDesc || "Manual earn"), "Earned");
    setEarnAmount("");
    setEarnDesc("");
    void handleFetchWallet();
  }, [walletAgent, earnAmount, earnDesc, handleAction, handleFetchWallet]);

  const handleSpend = useCallback(async () => {
    if (!walletAgent || !spendAmount) return;
    await handleAction("spend", () => economySpend(walletAgent, Number(spendAmount), spendType, spendDesc || "Manual spend"), "Spent");
    setSpendAmount("");
    setSpendDesc("");
    void handleFetchWallet();
  }, [walletAgent, spendAmount, spendType, spendDesc, handleAction, handleFetchWallet]);

  const handleTransfer = useCallback(async () => {
    if (!walletAgent || !transferTo || !transferAmount) return;
    await handleAction("transfer", () => economyTransfer(walletAgent, transferTo, Number(transferAmount), transferDesc || "Transfer"), "Transferred");
    setTransferAmount("");
    setTransferDesc("");
    void handleFetchWallet();
  }, [walletAgent, transferTo, transferAmount, transferDesc, handleAction, handleFetchWallet]);

  const handleFreezeWallet = useCallback(async () => {
    if (!walletAgent) return;
    await handleAction("freeze-wallet", () => economyFreezeWallet(walletAgent), "Wallet frozen");
  }, [walletAgent, handleAction]);

  /* ─── New handlers: contracts ─── */

  const handleFetchContracts = useCallback(async () => {
    if (!contractAgent) return;
    setWorking("fetch-contracts");
    setError(null);
    try {
      const result = await economyListContracts(contractAgent);
      setContractList(safeStr(result));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setWorking(null);
    }
  }, [contractAgent]);

  const handleCreateContract = useCallback(async () => {
    if (!contractAgent || !contractClient || !contractDesc) return;
    const deadline = contractDeadline ? Number(contractDeadline) : undefined;
    await handleAction("create-contract",
      () => economyCreateContract(contractAgent, contractClient, contractDesc, contractCriteria || "{}", Number(contractReward) || 0, Number(contractPenalty) || 0, deadline),
      "Contract created",
    );
    setContractDesc("");
    setContractCriteria("");
    setContractReward("");
    setContractPenalty("");
    setContractDeadline("");
    void handleFetchContracts();
  }, [contractAgent, contractClient, contractDesc, contractCriteria, contractReward, contractPenalty, contractDeadline, handleAction, handleFetchContracts]);

  const handleCompleteContract = useCallback(async () => {
    if (!completeContractId) return;
    await handleAction("complete-contract",
      () => economyCompleteContract(completeContractId, completeSuccess, completeEvidence || undefined),
      "Contract completed",
    );
    setCompleteContractId("");
    setCompleteEvidence("");
  }, [completeContractId, completeSuccess, completeEvidence, handleAction]);

  const handleDisputeContract = useCallback(async () => {
    if (!disputeContractId || !disputeContractReason) return;
    await handleAction("dispute-contract",
      () => economyDisputeContract(disputeContractId, disputeContractReason),
      "Contract disputed",
    );
    setDisputeContractId("");
    setDisputeContractReason("");
  }, [disputeContractId, disputeContractReason, handleAction]);

  /* ─── New handlers: parliament extras (using backend.ts wrappers) ─── */

  const handleProposeRuleViaBackend = useCallback(async () => {
    if (!proposeAgent || !proposeText.trim()) return;
    setWorking("propose-rule-backend");
    setError(null);
    try {
      const result = await civProposeRule(proposeAgent, proposeText.trim());
      appendLog("ProposalCreated", `Rule proposed via backend: ${safeStr(result).slice(0, 200)}`);
      setProposeText("");
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setWorking(null);
    }
  }, [proposeAgent, proposeText, appendLog, refresh]);

  const handleResolveDisputeViaBackend = useCallback(async () => {
    if (!disputeAgentA || !disputeAgentB || !disputeIssue.trim()) return;
    setWorking("resolve-dispute-backend");
    setError(null);
    try {
      const result = await civResolveDispute(disputeAgentA, disputeAgentB, disputeIssue.trim());
      appendLog("DisputeResolved", safeStr(result).slice(0, 200));
      setDisputeIssue("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setWorking(null);
    }
  }, [disputeAgentA, disputeAgentB, disputeIssue, appendLog]);

  /* ─── New handlers: payments ─── */

  const handleCreatePlan = useCallback(async () => {
    if (!planName || !planPrice) return;
    const features = planFeatures.split(",").map((s) => s.trim()).filter(Boolean);
    await handleAction("create-plan",
      () => paymentCreatePlan(planName, Number(planPrice), planInterval, features),
      "Plan created",
    );
    setPlanName("");
    setPlanPrice("");
    setPlanFeatures("");
    // Refresh plans
    try {
      const plans = await paymentListPlans();
      setPlansList(safeStr(plans));
    } catch { /* ignore */ }
  }, [planName, planPrice, planInterval, planFeatures, handleAction]);

  const handleCreateInvoice = useCallback(async () => {
    if (!invoicePlanId || !invoiceBuyerId) return;
    await handleAction("create-invoice",
      () => paymentCreateInvoice(invoicePlanId, invoiceBuyerId),
      "Invoice created",
    );
    setInvoicePlanId("");
    setInvoiceBuyerId("");
  }, [invoicePlanId, invoiceBuyerId, handleAction]);

  const handlePayInvoice = useCallback(async () => {
    if (!payInvoiceId) return;
    await handleAction("pay-invoice",
      () => paymentPayInvoice(payInvoiceId),
      "Invoice paid",
    );
    setPayInvoiceId("");
    // Refresh revenue
    try {
      const rev = await paymentGetRevenueStats();
      setRevenueStats(safeStr(rev));
    } catch { /* ignore */ }
  }, [payInvoiceId, handleAction]);

  const handleCreatePayout = useCallback(async () => {
    if (!payoutDevId || !payoutAgentId || !payoutAmount || !payoutPeriod) return;
    await handleAction("create-payout",
      () => paymentCreatePayout(payoutDevId, payoutAgentId, Number(payoutAmount), payoutPeriod),
      "Payout created",
    );
    setPayoutDevId("");
    setPayoutAgentId("");
    setPayoutAmount("");
    setPayoutPeriod("");
  }, [payoutDevId, payoutAgentId, payoutAmount, payoutPeriod, handleAction]);

  /* ─── Memos ─── */

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

  /* ─── Agent selector helper ─── */

  function AgentSelect({ value, onChange, label }: { value: string; onChange: (v: string) => void; label?: string }): JSX.Element {
    return (
      <select value={value} onChange={(e) => onChange(e.target.value)} style={inputStyle} aria-label={label}>
        <option value="">{label ?? "Select agent"}</option>
        {agents.map((a) => (
          <option key={a.id} value={a.id}>{a.name}</option>
        ))}
      </select>
    );
  }

  /* ─── JSON display helper ─── */

  function JsonBlock({ data, label }: { data: string | null; label: string }): JSX.Element {
    if (!data) return <EmptyState text={`No ${label} data loaded`} compact />;
    return (
      <pre style={{ ...commandInsetStyle, fontSize: "0.78rem", color: "#cbd5e1", whiteSpace: "pre-wrap", wordBreak: "break-all", maxHeight: 200, overflow: "auto", margin: "8px 0" }}>
        {data}
      </pre>
    );
  }

  /* ─── Render: Overview tab (original content) ─── */

  function renderOverview(): JSX.Element {
    return (
      <>
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
      </>
    );
  }

  /* ─── Render: Parliament tab ─── */

  function renderParliament(): JSX.Element {
    return (
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
        <Panel title="Parliament Status" accent="#a78bfa">
          <div style={{ ...commandInsetStyle, marginBottom: 12 }}>
            <DataRow label="Active proposals" value={parliament?.active_proposals ?? "-"} />
            <DataRow label="Passed rules" value={parliament?.passed_rules ?? "-"} />
            <DataRow label="Total votes" value={parliament?.total_votes ?? "-"} />
          </div>
          <ActionButton accent="#a78bfa" disabled={working === "refresh-parliament"} onClick={() => void (async () => {
            setWorking("refresh-parliament");
            try {
              const result = await civGetParliamentStatus();
              const parsed = safeParse<ParliamentStatus>(result);
              setParliament(parsed);
            } catch (err) {
              setError(err instanceof Error ? err.message : String(err));
            } finally {
              setWorking(null);
            }
          })()}>
            {working === "refresh-parliament" ? "Loading..." : "Refresh Parliament"}
          </ActionButton>
        </Panel>

        <Panel title="Propose Rule (Backend)" accent="#a78bfa">
          <div style={{ display: "grid", gap: 10 }}>
            <AgentSelect value={proposeAgent} onChange={setProposeAgent} label="Proposer" />
            <textarea
              value={proposeText}
              onChange={(e) => setProposeText(e.target.value)}
              placeholder="Rule text..."
              style={textareaStyle}
            />
            <ActionButton accent="#a78bfa" disabled={working === "propose-rule-backend"} onClick={() => void handleProposeRuleViaBackend()}>
              {working === "propose-rule-backend" ? "Proposing..." : "Propose Rule"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Roles" accent="#38bdf8">
          <ActionButton accent="#38bdf8" disabled={working === "refresh-roles"} onClick={() => void (async () => {
            setWorking("refresh-roles");
            try {
              const result = await civGetRoles();
              const parsed = safeParse<RoleAssignment[]>(result);
              if (Array.isArray(parsed)) setRoles(parsed);
            } catch (err) {
              setError(err instanceof Error ? err.message : String(err));
            } finally {
              setWorking(null);
            }
          })()}>
            {working === "refresh-roles" ? "Loading..." : "Refresh Roles"}
          </ActionButton>
          <div style={{ ...commandScrollStyle, maxHeight: 250, marginTop: 10 }}>
            {roles.length === 0 ? <EmptyState text="No roles loaded" compact /> : null}
            {roles.map((r, i) => (
              <div key={`${r.role}-${r.agent_id}-${i}`} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                <DataRow label="Role" value={r.role} valueColor={roleColor(r.role)} />
                <DataRow label="Agent" value={agentName(agents, r.agent_id)} />
                <DataRow label="Elected" value={formatTimestamp(r.elected_at)} />
              </div>
            ))}
          </div>
        </Panel>

        <Panel title="Governance Log" accent="#00ffcc">
          <ActionButton accent="#00ffcc" disabled={working === "refresh-gov-log"} onClick={() => void (async () => {
            setWorking("refresh-gov-log");
            try {
              const result = await civGetGovernanceLog(50);
              const parsed = safeParse<GovernanceEvent[]>(result);
              if (Array.isArray(parsed)) setRemoteLog(parsed);
            } catch (err) {
              setError(err instanceof Error ? err.message : String(err));
            } finally {
              setWorking(null);
            }
          })()}>
            {working === "refresh-gov-log" ? "Loading..." : "Refresh Log (50)"}
          </ActionButton>
          <div style={{ ...commandScrollStyle, maxHeight: 300, marginTop: 10 }}>
            {governanceFeed.length === 0 ? <EmptyState text="No events" compact /> : null}
            {governanceFeed.map((event, index) => (
              <div key={`${event.id ?? event.details}-${index}`} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 4 }}>
                  <span style={{ ...commandLabelStyle, color: "#00ffcc" }}>{toTitleCase(event.event_type)}</span>
                  <span style={{ ...commandMonoValueStyle, color: "#94a3b8" }}>{formatTimestamp(event.timestamp)}</span>
                </div>
                <div style={commandMutedStyle}>{event.details}</div>
              </div>
            ))}
          </div>
        </Panel>

        <Panel title="Resolve Dispute (Backend)" accent="#f87171">
          <div style={{ display: "grid", gap: 10 }}>
            <AgentSelect value={disputeAgentA} onChange={setDisputeAgentA} label="Agent A" />
            <AgentSelect value={disputeAgentB} onChange={setDisputeAgentB} label="Agent B" />
            <textarea
              value={disputeIssue}
              onChange={(e) => setDisputeIssue(e.target.value)}
              placeholder="Issue description..."
              style={textareaStyle}
            />
            <ActionButton destructive disabled={working === "resolve-dispute-backend"} onClick={() => void handleResolveDisputeViaBackend()}>
              {working === "resolve-dispute-backend" ? "Resolving..." : "Resolve Dispute"}
            </ActionButton>
          </div>
        </Panel>
      </div>
    );
  }

  /* ─── Render: Wallets tab ─── */

  function renderWallets(): JSX.Element {
    return (
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
        <Panel title="Economy Stats" accent="#f59e0b">
          <ActionButton accent="#f59e0b" disabled={working === "refresh-stats"} onClick={() => void (async () => {
            setWorking("refresh-stats");
            try {
              const [stats, econ] = await Promise.allSettled([economyGetStats(), civGetEconomyStatus()]);
              if (stats.status === "fulfilled") setEconomyStats(safeStr(stats.value));
              if (econ.status === "fulfilled") {
                const parsed = safeParse<EconomyStatus>(econ.value);
                setEconomy(parsed);
              }
            } catch (err) {
              setError(err instanceof Error ? err.message : String(err));
            } finally {
              setWorking(null);
            }
          })()}>
            {working === "refresh-stats" ? "Loading..." : "Refresh Stats"}
          </ActionButton>
          <JsonBlock data={economyStats} label="economy stats" />
          {economy ? (
            <div style={{ ...commandInsetStyle, marginTop: 8 }}>
              <DataRow label="Total tokens" value={Math.round(economy.total_tokens_circulating).toLocaleString()} valueColor="#fbbf24" />
              <DataRow label="Agents" value={economy.total_agents} />
              <DataRow label="Txns today" value={economy.transactions_today} />
            </div>
          ) : null}
        </Panel>

        <Panel title="Wallet Operations" accent="#f59e0b" style={{ minHeight: 400 }}>
          <div style={{ display: "grid", gap: 10, marginBottom: 14 }}>
            <AgentSelect value={walletAgent} onChange={setWalletAgent} label="Select agent" />
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <ActionButton accent="#f59e0b" disabled={working === "create-wallet" || !walletAgent} onClick={() => void handleCreateWallet()}>
                {working === "create-wallet" ? "Creating..." : "Create Wallet"}
              </ActionButton>
              <ActionButton accent="#f59e0b" disabled={working === "fetch-wallet" || !walletAgent} onClick={() => void handleFetchWallet()}>
                {working === "fetch-wallet" ? "Loading..." : "Get Wallet"}
              </ActionButton>
              <ActionButton destructive disabled={working === "freeze-wallet" || !walletAgent} onClick={() => void handleFreezeWallet()}>
                {working === "freeze-wallet" ? "Freezing..." : "Freeze Wallet"}
              </ActionButton>
            </div>
          </div>

          <div style={{ ...commandLabelStyle, marginBottom: 6 }}>Wallet Data</div>
          <JsonBlock data={walletData} label="wallet" />

          <div style={{ ...commandLabelStyle, marginBottom: 6, marginTop: 10 }}>Transaction History</div>
          <JsonBlock data={walletHistory} label="history" />

          <div style={{ ...commandLabelStyle, marginBottom: 6, marginTop: 10 }}>Agent Performance</div>
          <JsonBlock data={agentPerf} label="performance" />
        </Panel>

        <Panel title="Earn Tokens" accent="#22c55e">
          <div style={{ display: "grid", gap: 10 }}>
            <input type="number" value={earnAmount} onChange={(e) => setEarnAmount(e.target.value)} placeholder="Amount" style={inputStyle} />
            <input value={earnDesc} onChange={(e) => setEarnDesc(e.target.value)} placeholder="Description" style={inputStyle} />
            <ActionButton accent="#22c55e" disabled={working === "earn" || !walletAgent || !earnAmount} onClick={() => void handleEarn()}>
              {working === "earn" ? "Earning..." : "Earn"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Spend Tokens" accent="#ef4444">
          <div style={{ display: "grid", gap: 10 }}>
            <input type="number" value={spendAmount} onChange={(e) => setSpendAmount(e.target.value)} placeholder="Amount" style={inputStyle} />
            <select value={spendType} onChange={(e) => setSpendType(e.target.value)} style={inputStyle}>
              <option value="compute">Compute</option>
              <option value="storage">Storage</option>
              <option value="api_call">API Call</option>
              <option value="other">Other</option>
            </select>
            <input value={spendDesc} onChange={(e) => setSpendDesc(e.target.value)} placeholder="Description" style={inputStyle} />
            <ActionButton destructive disabled={working === "spend" || !walletAgent || !spendAmount} onClick={() => void handleSpend()}>
              {working === "spend" ? "Spending..." : "Spend"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Transfer Tokens" accent="#38bdf8">
          <div style={{ display: "grid", gap: 10 }}>
            <AgentSelect value={transferTo} onChange={setTransferTo} label="Transfer to" />
            <input type="number" value={transferAmount} onChange={(e) => setTransferAmount(e.target.value)} placeholder="Amount" style={inputStyle} />
            <input value={transferDesc} onChange={(e) => setTransferDesc(e.target.value)} placeholder="Description" style={inputStyle} />
            <ActionButton accent="#38bdf8" disabled={working === "transfer" || !walletAgent || !transferTo || !transferAmount} onClick={() => void handleTransfer()}>
              {working === "transfer" ? "Transferring..." : "Transfer"}
            </ActionButton>
          </div>
        </Panel>
      </div>
    );
  }

  /* ─── Render: Contracts tab ─── */

  function renderContracts(): JSX.Element {
    return (
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
        <Panel title="List Contracts" accent="#38bdf8">
          <div style={{ display: "grid", gap: 10, marginBottom: 14 }}>
            <AgentSelect value={contractAgent} onChange={setContractAgent} label="Agent" />
            <ActionButton accent="#38bdf8" disabled={working === "fetch-contracts" || !contractAgent} onClick={() => void handleFetchContracts()}>
              {working === "fetch-contracts" ? "Loading..." : "Load Contracts"}
            </ActionButton>
          </div>
          <JsonBlock data={contractList} label="contracts" />
        </Panel>

        <Panel title="Create Contract" accent="#22c55e" style={{ minHeight: 400 }}>
          <div style={{ display: "grid", gap: 10 }}>
            <AgentSelect value={contractAgent} onChange={setContractAgent} label="Agent (provider)" />
            <AgentSelect value={contractClient} onChange={setContractClient} label="Client" />
            <input value={contractDesc} onChange={(e) => setContractDesc(e.target.value)} placeholder="Description" style={inputStyle} />
            <input value={contractCriteria} onChange={(e) => setContractCriteria(e.target.value)} placeholder='Criteria JSON (e.g. {"quality":"high"})' style={inputStyle} />
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
              <input type="number" value={contractReward} onChange={(e) => setContractReward(e.target.value)} placeholder="Reward" style={inputStyle} />
              <input type="number" value={contractPenalty} onChange={(e) => setContractPenalty(e.target.value)} placeholder="Penalty" style={inputStyle} />
            </div>
            <input type="number" value={contractDeadline} onChange={(e) => setContractDeadline(e.target.value)} placeholder="Deadline (epoch, optional)" style={inputStyle} />
            <ActionButton accent="#22c55e" disabled={working === "create-contract" || !contractAgent || !contractClient || !contractDesc} onClick={() => void handleCreateContract()}>
              {working === "create-contract" ? "Creating..." : "Create Contract"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Complete Contract" accent="#f59e0b">
          <div style={{ display: "grid", gap: 10 }}>
            <input value={completeContractId} onChange={(e) => setCompleteContractId(e.target.value)} placeholder="Contract ID" style={inputStyle} />
            <select value={completeSuccess ? "true" : "false"} onChange={(e) => setCompleteSuccess(e.target.value === "true")} style={inputStyle}>
              <option value="true">Success</option>
              <option value="false">Failure</option>
            </select>
            <input value={completeEvidence} onChange={(e) => setCompleteEvidence(e.target.value)} placeholder="Evidence (optional)" style={inputStyle} />
            <ActionButton accent="#f59e0b" disabled={working === "complete-contract" || !completeContractId} onClick={() => void handleCompleteContract()}>
              {working === "complete-contract" ? "Completing..." : "Complete Contract"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Dispute Contract" accent="#f87171">
          <div style={{ display: "grid", gap: 10 }}>
            <input value={disputeContractId} onChange={(e) => setDisputeContractId(e.target.value)} placeholder="Contract ID" style={inputStyle} />
            <textarea value={disputeContractReason} onChange={(e) => setDisputeContractReason(e.target.value)} placeholder="Reason for dispute..." style={textareaStyle} />
            <ActionButton destructive disabled={working === "dispute-contract" || !disputeContractId || !disputeContractReason} onClick={() => void handleDisputeContract()}>
              {working === "dispute-contract" ? "Disputing..." : "Dispute Contract"}
            </ActionButton>
          </div>
        </Panel>
      </div>
    );
  }

  /* ─── Render: Payments tab ─── */

  function renderPayments(): JSX.Element {
    return (
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
        <Panel title="Revenue Stats" accent="#f472b6">
          <ActionButton accent="#f472b6" disabled={working === "refresh-revenue"} onClick={() => void (async () => {
            setWorking("refresh-revenue");
            try {
              const rev = await paymentGetRevenueStats();
              setRevenueStats(safeStr(rev));
            } catch (err) {
              setError(err instanceof Error ? err.message : String(err));
            } finally {
              setWorking(null);
            }
          })()}>
            {working === "refresh-revenue" ? "Loading..." : "Refresh Revenue"}
          </ActionButton>
          <JsonBlock data={revenueStats} label="revenue" />
        </Panel>

        <Panel title="Plans" accent="#f472b6">
          <ActionButton accent="#f472b6" disabled={working === "refresh-plans"} onClick={() => void (async () => {
            setWorking("refresh-plans");
            try {
              const plans = await paymentListPlans();
              setPlansList(safeStr(plans));
            } catch (err) {
              setError(err instanceof Error ? err.message : String(err));
            } finally {
              setWorking(null);
            }
          })()}>
            {working === "refresh-plans" ? "Loading..." : "Refresh Plans"}
          </ActionButton>
          <JsonBlock data={plansList} label="plans" />
        </Panel>

        <Panel title="Create Plan" accent="#22c55e">
          <div style={{ display: "grid", gap: 10 }}>
            <input value={planName} onChange={(e) => setPlanName(e.target.value)} placeholder="Plan name" style={inputStyle} />
            <input type="number" value={planPrice} onChange={(e) => setPlanPrice(e.target.value)} placeholder="Price (cents)" style={inputStyle} />
            <select value={planInterval} onChange={(e) => setPlanInterval(e.target.value)} style={inputStyle}>
              <option value="monthly">Monthly</option>
              <option value="yearly">Yearly</option>
              <option value="one_time">One-time</option>
            </select>
            <input value={planFeatures} onChange={(e) => setPlanFeatures(e.target.value)} placeholder="Features (comma-separated)" style={inputStyle} />
            <ActionButton accent="#22c55e" disabled={working === "create-plan" || !planName || !planPrice} onClick={() => void handleCreatePlan()}>
              {working === "create-plan" ? "Creating..." : "Create Plan"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Create Invoice" accent="#38bdf8">
          <div style={{ display: "grid", gap: 10 }}>
            <input value={invoicePlanId} onChange={(e) => setInvoicePlanId(e.target.value)} placeholder="Plan ID" style={inputStyle} />
            <input value={invoiceBuyerId} onChange={(e) => setInvoiceBuyerId(e.target.value)} placeholder="Buyer ID" style={inputStyle} />
            <ActionButton accent="#38bdf8" disabled={working === "create-invoice" || !invoicePlanId || !invoiceBuyerId} onClick={() => void handleCreateInvoice()}>
              {working === "create-invoice" ? "Creating..." : "Create Invoice"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Pay Invoice" accent="#f59e0b">
          <div style={{ display: "grid", gap: 10 }}>
            <input value={payInvoiceId} onChange={(e) => setPayInvoiceId(e.target.value)} placeholder="Invoice ID" style={inputStyle} />
            <ActionButton accent="#f59e0b" disabled={working === "pay-invoice" || !payInvoiceId} onClick={() => void handlePayInvoice()}>
              {working === "pay-invoice" ? "Paying..." : "Pay Invoice"}
            </ActionButton>
          </div>
        </Panel>

        <Panel title="Create Payout" accent="#a78bfa">
          <div style={{ display: "grid", gap: 10 }}>
            <input value={payoutDevId} onChange={(e) => setPayoutDevId(e.target.value)} placeholder="Developer ID" style={inputStyle} />
            <AgentSelect value={payoutAgentId} onChange={setPayoutAgentId} label="Agent" />
            <input type="number" value={payoutAmount} onChange={(e) => setPayoutAmount(e.target.value)} placeholder="Amount (cents)" style={inputStyle} />
            <input value={payoutPeriod} onChange={(e) => setPayoutPeriod(e.target.value)} placeholder="Period (e.g. 2026-03)" style={inputStyle} />
            <ActionButton accent="#a78bfa" disabled={working === "create-payout" || !payoutDevId || !payoutAgentId || !payoutAmount || !payoutPeriod} onClick={() => void handleCreatePayout()}>
              {working === "create-payout" ? "Creating..." : "Create Payout"}
            </ActionButton>
          </div>
        </Panel>
      </div>
    );
  }

  /* ─── Main render ─── */

  if (showWelcome) {
    return (
      <RequiresLlm feature="Civilization">
      <div style={commandPageStyle}>
        <div style={{ maxWidth: 640, margin: "60px auto", textAlign: "center" as const }}>
          <h1 style={{ margin: 0, fontFamily: "monospace", fontSize: "2rem", color: "#00ffcc", letterSpacing: "0.16em", textTransform: "uppercase" as const, marginBottom: 16 }}>
            Welcome to Agent Civilization
          </h1>
          <p style={{ color: "#94a3b8", fontSize: "1rem", lineHeight: 1.6, marginBottom: 32 }}>
            Watch AI agents build a micro-society with economy, governance, and trade.
            Choose a governance model to found your civilization.
          </p>

          <div style={{ display: "grid", gap: 14 }}>
            {([
              { type: "democracy", label: "Democracy", desc: "Agents vote on every proposal. Majority rules.", accent: "#3b82f6" },
              { type: "meritocracy", label: "Meritocracy", desc: "Highest-performing agents lead and set policy.", accent: "#f59e0b" },
              { type: "council", label: "Council", desc: "A governing council of elected agents makes decisions.", accent: "#a78bfa" },
            ] as const).map((opt) => (
              <button type="button"
                key={opt.type}
                onClick={() => foundCiv(opt.type)}
                style={{
                  padding: "18px 24px",
                  background: `rgba(${opt.accent === "#3b82f6" ? "59,130,246" : opt.accent === "#f59e0b" ? "245,158,11" : "167,139,250"},0.08)`,
                  border: `1px solid ${opt.accent}44`,
                  borderRadius: 12,
                  cursor: "pointer",
                  textAlign: "left" as const,
                  fontFamily: "var(--font-mono, monospace)",
                  color: "var(--text-primary, #e2e8f0)",
                  transition: "all 0.2s",
                }}
              >
                <div style={{ fontWeight: 700, fontSize: "1.05rem", color: opt.accent, marginBottom: 4 }}>
                  {opt.label}
                </div>
                <div style={{ fontSize: "0.82rem", color: "#94a3b8" }}>{opt.desc}</div>
              </button>
            ))}
          </div>

          <p style={{ color: "#64748b", fontSize: "0.78rem", marginTop: 24 }}>
            Requires an AI engine. Agents will use your LLM to think, debate, and decide.
          </p>
        </div>
      </div>
      </RequiresLlm>
    );
  }

  return (
    <RequiresLlm feature="Civilization">
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

      <TabBar active={activeTab} onChange={setActiveTab} />

      {activeTab === "overview" && renderOverview()}
      {activeTab === "parliament" && renderParliament()}
      {activeTab === "wallets" && renderWallets()}
      {activeTab === "contracts" && renderContracts()}
      {activeTab === "payments" && renderPayments()}
    </div>
    </RequiresLlm>
  );
}
