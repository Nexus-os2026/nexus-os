import { useEffect, useState } from "react";
import {
  tokenGetAllWallets,
  tokenGetLedger,
  tokenGetSupply,
  tokenGetPricing,
  tokenCalculateReward,
  tokenCalculateBurn,
} from "../api/backend";
import {
  alpha,
  commandMutedStyle,
  commandPageStyle,
} from "./commandCenterUi";

const ACCENT = "#f59e0b";
const GREEN = "#22c55e";
const RED = "#ef4444";
const YELLOW = "#eab308";
const BLUE = "#0ea5e9";
const PURPLE = "#14b8a6";

interface WalletSummary {
  agent_id: string;
  balance: number;
  available_balance: number;
  lifetime_earned: number;
  lifetime_burned: number;
  lifetime_transferred: number;
  lifetime_received: number;
  escrowed: number;
  burn_rate: number;
  autonomy_level: number;
  version: number;
}

interface LedgerEntry {
  entry_id: string;
  timestamp: number;
  agent_id: string;
  transaction_type: string;
  amount: number;
  is_burn: boolean;
  balance_after: number;
}

interface SupplySummary {
  total_supply: number;
  total_burned: number;
  total_minted: number;
  is_deflationary: boolean;
  active_wallets: number;
  active_delegations: number;
  total_escrowed: number;
  net_flow: number;
}

interface PricingEntry {
  model_id: string;
  size_class: string;
  is_local: boolean;
  input_cost_per_1k: number;
  output_cost_per_1k: number;
}

interface RewardEstimate {
  base: number;
  quality_multiplier: number;
  difficulty_multiplier: number;
  speed_multiplier: number;
  final_reward: number;
}

type Tab = "overview" | "wallets" | "ledger" | "pricing";

const cardStyle: React.CSSProperties = {
  background: alpha("#1e1e2e", 0.7),
  borderRadius: 10,
  padding: 16,
  border: "1px solid " + alpha("#ffffff", 0.08),
};

const bigNumStyle: React.CSSProperties = {
  fontSize: 28,
  fontWeight: 700,
  fontFamily: "monospace",
};

const labelStyle: React.CSSProperties = {
  fontSize: 11,
  color: "#888",
  textTransform: "uppercase" as const,
  letterSpacing: 1,
  marginBottom: 4,
};

function walletColor(w: WalletSummary): string {
  if (w.autonomy_level >= 4 && w.available_balance < 1) return RED;
  if (w.burn_rate > 0.8) return YELLOW;
  return GREEN;
}

function formatNxc(v: number): string {
  return v.toFixed(2);
}

function shortTxType(t: string): string {
  if (t.includes("GovernanceMint")) return "Mint";
  if (t.includes("TaskReward")) return "Reward";
  if (t.includes("ComputeBurn")) return "Compute Burn";
  if (t.includes("SpawnBurn")) return "Spawn Burn";
  if (t.includes("ChildAllocation")) return "Child Alloc";
  if (t.includes("DelegationLock")) return "Escrow Lock";
  if (t.includes("DelegationRelease")) return "Escrow Release";
  if (t.includes("DelegationRefund")) return "Refund";
  return t.slice(0, 20);
}

export default function TokenEconomy() {
  const [tab, setTab] = useState<Tab>("overview");
  const [wallets, setWallets] = useState<WalletSummary[]>([]);
  const [ledger, setLedger] = useState<LedgerEntry[]>([]);
  const [supply, setSupply] = useState<SupplySummary | null>(null);
  const [pricing, setPricing] = useState<PricingEntry[]>([]);
  const [loading, setLoading] = useState(true);

  // Reward calculator state
  const [quality, setQuality] = useState(0.8);
  const [difficulty, setDifficulty] = useState(0.6);
  const [speed, setSpeed] = useState(30);
  const [rewardEst, setRewardEst] = useState<RewardEstimate | null>(null);

  // Burn calculator state
  const [burnModel, setBurnModel] = useState("flash-2b");
  const [burnIn, setBurnIn] = useState(1000);
  const [burnOut, setBurnOut] = useState(500);
  const [burnEst, setBurnEst] = useState<{ cost_nxc: number } | null>(null);

  useEffect(() => {
    Promise.all([
      tokenGetAllWallets().catch(() => []),
      tokenGetLedger(undefined, 100).catch(() => []),
      tokenGetSupply().catch(() => null),
      tokenGetPricing().catch(() => []),
    ])
      .then(([w, l, s, p]) => {
        setWallets(Array.isArray(w) ? w : []);
        setLedger(Array.isArray(l) ? l : []);
        setSupply(s as SupplySummary | null);
        setPricing(Array.isArray(p) ? p : []);
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    tokenCalculateReward(quality, difficulty, speed)
      .then((r) => setRewardEst(r as RewardEstimate))
      .catch((e) => { if (import.meta.env.DEV) console.warn("[TokenEconomy]", e); });
  }, [quality, difficulty, speed]);

  useEffect(() => {
    tokenCalculateBurn(burnModel, burnIn, burnOut)
      .then((b) => setBurnEst(b as { cost_nxc: number }))
      .catch((e) => { if (import.meta.env.DEV) console.warn("[TokenEconomy]", e); });
  }, [burnModel, burnIn, burnOut]);

  if (loading) {
    return (
      <div style={commandPageStyle}>
        <div style={{ textAlign: "center", padding: 48, color: "#888" }}>
          Loading token economy...
        </div>
      </div>
    );
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: "overview", label: "Supply Overview" },
    { id: "wallets", label: "Agent Wallets" },
    { id: "ledger", label: "Transaction Feed" },
    { id: "pricing", label: "Pricing & Rewards" },
  ];

  return (
    <div style={commandPageStyle}>
      <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0, marginBottom: 4 }}>
        Token Economy
      </h1>
      <p style={{ ...commandMutedStyle, marginBottom: 16, fontSize: 13 }}>
        NXC coin economy — agents earn, burn, delegate, and get gated by balance.
      </p>

      {/* Tab bar */}
      <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            style={{
              background: tab === t.id ? alpha(ACCENT, 0.2) : alpha("#ffffff", 0.05),
              border: tab === t.id ? `1px solid ${ACCENT}` : "1px solid " + alpha("#ffffff", 0.1),
              color: tab === t.id ? ACCENT : "#aaa",
              borderRadius: 6,
              padding: "6px 14px",
              fontSize: 13,
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "overview" && (
        <OverviewTab supply={supply} wallets={wallets} />
      )}
      {tab === "wallets" && <WalletsTab wallets={wallets} />}
      {tab === "ledger" && <LedgerTab ledger={ledger} />}
      {tab === "pricing" && (
        <PricingTab
          pricing={pricing}
          quality={quality}
          setQuality={setQuality}
          difficulty={difficulty}
          setDifficulty={setDifficulty}
          speed={speed}
          setSpeed={setSpeed}
          rewardEst={rewardEst}
          burnModel={burnModel}
          setBurnModel={setBurnModel}
          burnIn={burnIn}
          setBurnIn={setBurnIn}
          burnOut={burnOut}
          setBurnOut={setBurnOut}
          burnEst={burnEst}
        />
      )}
    </div>
  );
}

function OverviewTab({ supply, wallets }: { supply: SupplySummary | null; wallets: WalletSummary[] }) {
  const s = supply || {
    total_supply: 0, total_minted: 0, total_burned: 0,
    is_deflationary: false, active_wallets: 0, active_delegations: 0,
    total_escrowed: 0, net_flow: 0,
  };

  return (
    <div>
      {/* Big numbers */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 16, marginBottom: 24 }}>
        <div style={cardStyle}>
          <div style={labelStyle}>Total Supply</div>
          <div style={{ ...bigNumStyle, color: ACCENT }}>{formatNxc(s.total_supply)} NXC</div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Total Minted</div>
          <div style={{ ...bigNumStyle, color: GREEN }}>{formatNxc(s.total_minted)} NXC</div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Total Burned</div>
          <div style={{ ...bigNumStyle, color: RED }}>{formatNxc(s.total_burned)} NXC</div>
        </div>
      </div>

      {/* Indicators */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 12, marginBottom: 24 }}>
        <div style={cardStyle}>
          <div style={labelStyle}>Economy State</div>
          <div style={{ fontSize: 16, fontWeight: 600, color: s.is_deflationary ? RED : GREEN }}>
            {s.is_deflationary ? "Deflationary" : "Inflationary"}
          </div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Net Flow</div>
          <div style={{ fontSize: 16, fontWeight: 600, color: s.net_flow >= 0 ? GREEN : RED }}>
            {s.net_flow >= 0 ? "+" : ""}{s.net_flow} micro
          </div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Active Wallets</div>
          <div style={{ fontSize: 16, fontWeight: 600, color: BLUE }}>{s.active_wallets}</div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Escrowed</div>
          <div style={{ fontSize: 16, fontWeight: 600, color: PURPLE }}>{formatNxc(s.total_escrowed)} NXC</div>
        </div>
      </div>

      {/* Wallet summary cards */}
      {wallets.length > 0 && (
        <>
          <h2 style={{ color: "#ccc", fontSize: 16, fontWeight: 600, marginBottom: 12 }}>Agent Balances</h2>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))", gap: 12 }}>
            {wallets.map((w) => (
              <div
                key={w.agent_id}
                style={{
                  ...cardStyle,
                  borderLeft: `3px solid ${walletColor(w)}`,
                }}
              >
                <div style={{ fontSize: 13, fontWeight: 600, color: "#ddd", marginBottom: 6 }}>
                  {w.agent_id.slice(0, 16)}...
                </div>
                <div style={{ fontSize: 20, fontWeight: 700, color: walletColor(w), fontFamily: "monospace" }}>
                  {formatNxc(w.balance)} NXC
                </div>
                <div style={{ fontSize: 11, color: "#888", marginTop: 4 }}>
                  L{w.autonomy_level} | Burn rate: {(w.burn_rate * 100).toFixed(0)}%
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {wallets.length === 0 && (
        <div style={{ textAlign: "center", padding: 32, color: "#666" }}>
          No wallets created yet. Agents will receive wallets when they start tasks.
        </div>
      )}
    </div>
  );
}

function WalletsTab({ wallets }: { wallets: WalletSummary[] }) {
  const [sortBy, setSortBy] = useState<"balance" | "burn_rate" | "earned">("balance");

  const sorted = [...wallets].sort((a, b) => {
    if (sortBy === "balance") return b.balance - a.balance;
    if (sortBy === "burn_rate") return b.burn_rate - a.burn_rate;
    return b.lifetime_earned - a.lifetime_earned;
  });

  return (
    <div>
      <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
        {(["balance", "burn_rate", "earned"] as const).map((s) => (
          <button
            key={s}
            onClick={() => setSortBy(s)}
            style={{
              background: sortBy === s ? alpha(ACCENT, 0.15) : "transparent",
              border: "1px solid " + alpha("#ffffff", 0.1),
              color: sortBy === s ? ACCENT : "#888",
              borderRadius: 4,
              padding: "4px 10px",
              fontSize: 12,
              cursor: "pointer",
            }}
          >
            Sort: {s === "burn_rate" ? "Burn Rate" : s === "earned" ? "Earned" : "Balance"}
          </button>
        ))}
      </div>

      {sorted.length === 0 && (
        <div style={{ textAlign: "center", padding: 32, color: "#666" }}>No wallets yet.</div>
      )}

      <div style={{ display: "grid", gap: 10 }}>
        {sorted.map((w) => (
          <div
            key={w.agent_id}
            style={{
              ...cardStyle,
              display: "grid",
              gridTemplateColumns: "1fr 1fr 1fr 1fr 1fr",
              alignItems: "center",
              gap: 12,
            }}
          >
            <div>
              <div style={{ fontSize: 12, color: "#888" }}>Agent</div>
              <div style={{ fontSize: 14, fontWeight: 600, color: "#ddd" }}>{w.agent_id.slice(0, 20)}</div>
              <div style={{ fontSize: 11, color: "#666" }}>L{w.autonomy_level} | v{w.version}</div>
            </div>
            <div>
              <div style={{ fontSize: 12, color: "#888" }}>Balance</div>
              <div style={{ fontSize: 16, fontWeight: 700, color: walletColor(w), fontFamily: "monospace" }}>
                {formatNxc(w.balance)}
              </div>
              <div style={{ fontSize: 11, color: "#666" }}>Avail: {formatNxc(w.available_balance)}</div>
            </div>
            <div>
              <div style={{ fontSize: 12, color: "#888" }}>Earned / Burned</div>
              <div style={{ fontSize: 13, fontFamily: "monospace" }}>
                <span style={{ color: GREEN }}>+{formatNxc(w.lifetime_earned)}</span>
                {" / "}
                <span style={{ color: RED }}>-{formatNxc(w.lifetime_burned)}</span>
              </div>
            </div>
            <div>
              <div style={{ fontSize: 12, color: "#888" }}>Burn Rate</div>
              <div
                style={{
                  fontSize: 16,
                  fontWeight: 600,
                  color: w.burn_rate > 0.8 ? RED : w.burn_rate > 0.5 ? YELLOW : GREEN,
                  fontFamily: "monospace",
                }}
              >
                {(w.burn_rate * 100).toFixed(1)}%
              </div>
            </div>
            <div>
              <div style={{ fontSize: 12, color: "#888" }}>Escrowed</div>
              <div style={{ fontSize: 13, color: PURPLE, fontFamily: "monospace" }}>
                {formatNxc(w.escrowed)}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function LedgerTab({ ledger }: { ledger: LedgerEntry[] }) {
  const [filterAgent, setFilterAgent] = useState("");
  const [filterType, setFilterType] = useState<"all" | "burn" | "credit">("all");

  const filtered = ledger.filter((e) => {
    if (filterAgent && !e.agent_id.includes(filterAgent)) return false;
    if (filterType === "burn" && !e.is_burn) return false;
    if (filterType === "credit" && e.is_burn) return false;
    return true;
  });

  return (
    <div>
      <div style={{ display: "flex", gap: 8, marginBottom: 16, alignItems: "center" }}>
        <input
          placeholder="Filter by agent ID..."
          value={filterAgent}
          onChange={(e) => setFilterAgent(e.target.value)}
          style={{
            background: alpha("#ffffff", 0.05),
            border: "1px solid " + alpha("#ffffff", 0.1),
            color: "#ddd",
            borderRadius: 4,
            padding: "6px 10px",
            fontSize: 12,
            flex: 1,
            maxWidth: 240,
          }}
        />
        {(["all", "burn", "credit"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setFilterType(t)}
            style={{
              background: filterType === t ? alpha(ACCENT, 0.15) : "transparent",
              border: "1px solid " + alpha("#ffffff", 0.1),
              color: filterType === t ? ACCENT : "#888",
              borderRadius: 4,
              padding: "4px 10px",
              fontSize: 12,
              cursor: "pointer",
            }}
          >
            {t === "all" ? "All" : t === "burn" ? "Burns" : "Credits"}
          </button>
        ))}
      </div>

      {filtered.length === 0 && (
        <div style={{ textAlign: "center", padding: 32, color: "#666" }}>No transactions recorded yet.</div>
      )}

      <div style={{ display: "grid", gap: 6 }}>
        {filtered.map((e) => (
          <div
            key={e.entry_id}
            style={{
              ...cardStyle,
              display: "grid",
              gridTemplateColumns: "120px 1fr 100px 100px 80px",
              alignItems: "center",
              gap: 8,
              padding: "10px 16px",
            }}
          >
            <div style={{ fontSize: 11, color: "#666", fontFamily: "monospace" }}>
              {new Date(e.timestamp * 1000).toLocaleTimeString()}
            </div>
            <div>
              <span style={{ fontSize: 12, color: "#aaa" }}>{e.agent_id.slice(0, 16)}</span>
              <span style={{ fontSize: 12, color: "#666", marginLeft: 8 }}>{shortTxType(e.transaction_type)}</span>
            </div>
            <div
              style={{
                fontSize: 14,
                fontWeight: 600,
                fontFamily: "monospace",
                color: e.is_burn ? RED : GREEN,
                textAlign: "right",
              }}
            >
              {e.is_burn ? "-" : "+"}{formatNxc(e.amount)}
            </div>
            <div style={{ fontSize: 12, color: "#888", fontFamily: "monospace", textAlign: "right" }}>
              bal: {formatNxc(e.balance_after)}
            </div>
            <div
              style={{
                fontSize: 10,
                color: e.is_burn ? alpha(RED, 0.7) : alpha(GREEN, 0.7),
                background: e.is_burn ? alpha(RED, 0.1) : alpha(GREEN, 0.1),
                borderRadius: 3,
                padding: "2px 6px",
                textAlign: "center",
                fontWeight: 600,
              }}
            >
              {e.is_burn ? "BURN" : "CREDIT"}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function PricingTab({
  pricing, quality, setQuality, difficulty, setDifficulty,
  speed, setSpeed, rewardEst,
  burnModel, setBurnModel, burnIn, setBurnIn, burnOut, setBurnOut, burnEst,
}: {
  pricing: PricingEntry[];
  quality: number; setQuality: (v: number) => void;
  difficulty: number; setDifficulty: (v: number) => void;
  speed: number; setSpeed: (v: number) => void;
  rewardEst: RewardEstimate | null;
  burnModel: string; setBurnModel: (v: string) => void;
  burnIn: number; setBurnIn: (v: number) => void;
  burnOut: number; setBurnOut: (v: number) => void;
  burnEst: { cost_nxc: number } | null;
}) {
  const sliderStyle: React.CSSProperties = {
    width: "100%",
    accentColor: ACCENT,
  };

  return (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
      {/* Pricing Table */}
      <div>
        <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, marginBottom: 12 }}>Model Pricing Table</h3>
        <div style={{ ...cardStyle, padding: 0, overflow: "hidden" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
            <thead>
              <tr style={{ background: alpha("#ffffff", 0.05) }}>
                <th style={{ padding: "8px 12px", textAlign: "left", color: "#888" }}>Model</th>
                <th style={{ padding: "8px 12px", textAlign: "left", color: "#888" }}>Class</th>
                <th style={{ padding: "8px 12px", textAlign: "right", color: "#888" }}>In/1K</th>
                <th style={{ padding: "8px 12px", textAlign: "right", color: "#888" }}>Out/1K</th>
                <th style={{ padding: "8px 12px", textAlign: "center", color: "#888" }}>Local</th>
              </tr>
            </thead>
            <tbody>
              {pricing.map((p) => (
                <tr key={p.model_id} style={{ borderTop: "1px solid " + alpha("#ffffff", 0.05) }}>
                  <td style={{ padding: "8px 12px", color: "#ddd", fontFamily: "monospace" }}>{p.model_id}</td>
                  <td style={{ padding: "8px 12px", color: "#aaa" }}>{p.size_class}</td>
                  <td style={{ padding: "8px 12px", textAlign: "right", color: ACCENT, fontFamily: "monospace" }}>
                    {p.input_cost_per_1k.toFixed(6)}
                  </td>
                  <td style={{ padding: "8px 12px", textAlign: "right", color: ACCENT, fontFamily: "monospace" }}>
                    {p.output_cost_per_1k.toFixed(6)}
                  </td>
                  <td style={{ padding: "8px 12px", textAlign: "center" }}>
                    <span style={{ color: p.is_local ? GREEN : BLUE }}>{p.is_local ? "Local" : "Cloud"}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Burn calculator */}
        <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, margin: "20px 0 12px" }}>Burn Calculator</h3>
        <div style={cardStyle}>
          <div style={{ marginBottom: 10 }}>
            <label style={labelStyle}>Model</label>
            <select
              value={burnModel}
              onChange={(e) => setBurnModel(e.target.value)}
              style={{
                background: alpha("#ffffff", 0.05),
                border: "1px solid " + alpha("#ffffff", 0.1),
                color: "#ddd",
                borderRadius: 4,
                padding: "4px 8px",
                fontSize: 12,
                width: "100%",
              }}
            >
              {pricing.map((p) => (
                <option key={p.model_id} value={p.model_id}>{p.model_id}</option>
              ))}
            </select>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginBottom: 10 }}>
            <div>
              <label style={labelStyle}>Input Tokens</label>
              <input
                type="number"
                value={burnIn}
                onChange={(e) => setBurnIn(Number(e.target.value))}
                style={{
                  background: alpha("#ffffff", 0.05),
                  border: "1px solid " + alpha("#ffffff", 0.1),
                  color: "#ddd",
                  borderRadius: 4,
                  padding: "4px 8px",
                  fontSize: 12,
                  width: "100%",
                  boxSizing: "border-box",
                }}
              />
            </div>
            <div>
              <label style={labelStyle}>Output Tokens</label>
              <input
                type="number"
                value={burnOut}
                onChange={(e) => setBurnOut(Number(e.target.value))}
                style={{
                  background: alpha("#ffffff", 0.05),
                  border: "1px solid " + alpha("#ffffff", 0.1),
                  color: "#ddd",
                  borderRadius: 4,
                  padding: "4px 8px",
                  fontSize: 12,
                  width: "100%",
                  boxSizing: "border-box",
                }}
              />
            </div>
          </div>
          {burnEst && (
            <div style={{ fontSize: 18, fontWeight: 700, color: RED, fontFamily: "monospace" }}>
              Burn: {burnEst.cost_nxc.toFixed(6)} NXC
            </div>
          )}
        </div>
      </div>

      {/* Reward Calculator */}
      <div>
        <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, marginBottom: 12 }}>Reward Calculator</h3>
        <div style={cardStyle}>
          <div style={{ marginBottom: 16 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <label style={labelStyle}>Quality Score</label>
              <span style={{ fontSize: 12, color: ACCENT, fontFamily: "monospace" }}>{quality.toFixed(2)}</span>
            </div>
            <input type="range" min={0} max={1} step={0.01} value={quality} onChange={(e) => setQuality(Number(e.target.value))} style={sliderStyle} />
          </div>
          <div style={{ marginBottom: 16 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <label style={labelStyle}>Difficulty</label>
              <span style={{ fontSize: 12, color: ACCENT, fontFamily: "monospace" }}>{difficulty.toFixed(2)}</span>
            </div>
            <input type="range" min={0} max={1} step={0.01} value={difficulty} onChange={(e) => setDifficulty(Number(e.target.value))} style={sliderStyle} />
          </div>
          <div style={{ marginBottom: 16 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <label style={labelStyle}>Completion Time (seconds)</label>
              <span style={{ fontSize: 12, color: ACCENT, fontFamily: "monospace" }}>{speed}s</span>
            </div>
            <input type="range" min={1} max={300} step={1} value={speed} onChange={(e) => setSpeed(Number(e.target.value))} style={sliderStyle} />
          </div>

          {rewardEst && (
            <div style={{ borderTop: "1px solid " + alpha("#ffffff", 0.1), paddingTop: 12 }}>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginBottom: 12 }}>
                <div>
                  <div style={labelStyle}>Quality Mult</div>
                  <div style={{ fontSize: 14, color: "#ddd", fontFamily: "monospace" }}>{rewardEst.quality_multiplier.toFixed(2)}x</div>
                </div>
                <div>
                  <div style={labelStyle}>Difficulty Mult</div>
                  <div style={{ fontSize: 14, color: "#ddd", fontFamily: "monospace" }}>{rewardEst.difficulty_multiplier.toFixed(2)}x</div>
                </div>
                <div>
                  <div style={labelStyle}>Speed Mult</div>
                  <div style={{ fontSize: 14, color: "#ddd", fontFamily: "monospace" }}>{rewardEst.speed_multiplier.toFixed(2)}x</div>
                </div>
                <div>
                  <div style={labelStyle}>Base</div>
                  <div style={{ fontSize: 14, color: "#ddd", fontFamily: "monospace" }}>{rewardEst.base.toFixed(2)} NXC</div>
                </div>
              </div>
              <div style={{ fontSize: 22, fontWeight: 700, color: GREEN, fontFamily: "monospace" }}>
                Reward: {rewardEst.final_reward.toFixed(4)} NXC
              </div>
            </div>
          )}
        </div>

        {/* Gating Rules */}
        <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, margin: "20px 0 12px" }}>Gating Rules</h3>
        <div style={cardStyle}>
          <div style={{ fontSize: 12, color: "#aaa", lineHeight: 1.8 }}>
            <div><span style={{ color: GREEN, fontWeight: 600 }}>L0-L3:</span> Coins tracked, never gated</div>
            <div><span style={{ color: YELLOW, fontWeight: 600 }}>L4:</span> Compute requires sufficient balance</div>
            <div><span style={{ color: RED, fontWeight: 600 }}>L5-L6:</span> All operations require sufficient balance</div>
            <div style={{ marginTop: 8 }}><span style={{ color: PURPLE, fontWeight: 600 }}>Spawning:</span> Always requires balance (any level)</div>
            <div><span style={{ color: BLUE, fontWeight: 600 }}>Delegation:</span> Locks coins in escrow until verified</div>
          </div>
        </div>
      </div>
    </div>
  );
}
