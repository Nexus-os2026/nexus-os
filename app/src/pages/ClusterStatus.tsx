import { useEffect, useState, useCallback } from "react";
import {
  hasDesktopRuntime,
  getLiveSystemMetrics,
  listAgents,
  meshDiscoverPeers,
  meshGetPeers,
  meshGetSyncStatus,
  meshDistributeTask,
  meshMigrateAgent,
} from "../api/backend";
import type { AgentSummary } from "../types";
import "./cluster-status.css";

interface MeshPeer {
  peer_id: string;
  address: string;
  port: number;
  name: string;
  status: string;
}

interface SyncStatus {
  synced: boolean;
  last_sync: number;
  pending_items: number;
}

export default function ClusterStatus(): JSX.Element {
  const [runtimeName, setRuntimeName] = useState<string | null>(null);
  const [cpuUsage, setCpuUsage] = useState<number>(0);
  const [memUsage, setMemUsage] = useState<number>(0);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [peers, setPeers] = useState<MeshPeer[]>([]);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [discovering, setDiscovering] = useState(false);
  const [taskInput, setTaskInput] = useState("");
  const [taskAgentIds, setTaskAgentIds] = useState("");
  const [migrateAgentId, setMigrateAgentId] = useState("");
  const [migrateTarget, setMigrateTarget] = useState("");
  const [meshError, setMeshError] = useState<string | null>(null);
  const [meshSuccess, setMeshSuccess] = useState<string | null>(null);

  const loadPeers = useCallback(async () => {
    if (!hasDesktopRuntime()) return;
    try {
      const [peersRaw, syncRaw] = await Promise.all([
        meshGetPeers(),
        meshGetSyncStatus(),
      ]);
      setPeers(JSON.parse(peersRaw));
      setSyncStatus(JSON.parse(syncRaw));
    } catch {
      // mesh not available yet
    }
  }, []);

  useEffect(() => {
    async function loadRuntime() {
      if (!hasDesktopRuntime()) {
        setLoading(false);
        return;
      }
      try {
        const [raw, agentList] = await Promise.all([
          getLiveSystemMetrics(),
          listAgents(),
        ]);
        const data = JSON.parse(raw);
        setRuntimeName(data.cpu_name || "Local runtime");
        setCpuUsage(data.cpu_usage_percent ?? 0);
        setMemUsage(data.memory_usage_percent ?? 0);
        setAgents(agentList);
      } catch {
        setRuntimeName(null);
      }
      setLoading(false);
    }
    void loadRuntime();
    void loadPeers();
    const iv = setInterval(() => { void loadRuntime(); void loadPeers(); }, 10_000);
    return () => clearInterval(iv);
  }, [loadPeers]);

  const handleDiscover = useCallback(async () => {
    setDiscovering(true);
    setMeshError(null);
    setMeshSuccess(null);
    try {
      const raw = await meshDiscoverPeers();
      const result = JSON.parse(raw);
      setMeshSuccess(`Discovery complete: found ${result.discovered ?? 0} peer(s)`);
      await loadPeers();
    } catch (e) {
      setMeshError(e instanceof Error ? e.message : String(e));
    }
    setDiscovering(false);
  }, [loadPeers]);

  const handleDistributeTask = useCallback(async () => {
    if (!taskInput.trim()) return;
    setMeshError(null);
    setMeshSuccess(null);
    try {
      const ids = taskAgentIds.split(",").map(s => s.trim()).filter(Boolean);
      const raw = await meshDistributeTask(taskInput, ids);
      const result = JSON.parse(raw);
      setMeshSuccess(`Task distributed: ${result.status ?? "ok"}`);
      setTaskInput("");
      setTaskAgentIds("");
    } catch (e) {
      setMeshError(e instanceof Error ? e.message : String(e));
    }
  }, [taskInput, taskAgentIds]);

  const handleMigrateAgent = useCallback(async () => {
    if (!migrateAgentId.trim() || !migrateTarget.trim()) return;
    setMeshError(null);
    setMeshSuccess(null);
    try {
      const raw = await meshMigrateAgent(migrateAgentId, migrateTarget);
      const result = JSON.parse(raw);
      setMeshSuccess(`Agent migrated: ${result.status ?? "ok"}`);
      setMigrateAgentId("");
      setMigrateTarget("");
      await loadPeers();
    } catch (e) {
      setMeshError(e instanceof Error ? e.message : String(e));
    }
  }, [migrateAgentId, migrateTarget, loadPeers]);

  const activeAgents = agents.filter(a => a.status === "Running").length;
  const cardStyle: React.CSSProperties = {
    background: "rgba(15,23,42,0.7)",
    border: "1px solid rgba(34,211,238,0.15)",
    borderRadius: 10,
    padding: "1.2rem",
  };

  return (
    <section className="cs-hub">
      <header className="cs-header">
        <h2 className="cs-title">CLUSTER STATUS // NODE HEALTH</h2>
        <p className="cs-subtitle">Single node &mdash; local runtime</p>
      </header>

      {loading && <div style={{ padding: "2rem", textAlign: "center", opacity: 0.5 }}>Loading cluster status...</div>}

      {!loading && (
        <div style={{ padding: "1.5rem", display: "flex", flexDirection: "column", gap: 16 }}>
          {/* Node card */}
          <div style={{ ...cardStyle, borderColor: "rgba(34,211,238,0.3)" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
              <span style={{ width: 10, height: 10, borderRadius: "50%", background: "#22d3ee", display: "inline-block" }} />
              <span style={{ fontFamily: "monospace", fontSize: "1rem", color: "#22d3ee", fontWeight: 600 }}>PRIMARY NODE</span>
              <span style={{ marginLeft: "auto", fontSize: "0.75rem", color: "#4ade80", background: "rgba(74,222,128,0.1)", padding: "2px 10px", borderRadius: 8 }}>Online</span>
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16 }}>
              <div>
                <div style={{ fontSize: "0.7rem", color: "#64748b", textTransform: "uppercase", marginBottom: 4 }}>Runtime</div>
                <div style={{ fontSize: "0.85rem", color: "#e2e8f0" }}>{runtimeName || "Unknown"}</div>
              </div>
              <div>
                <div style={{ fontSize: "0.7rem", color: "#64748b", textTransform: "uppercase", marginBottom: 4 }}>CPU</div>
                <div style={{ fontSize: "0.85rem", color: "#e2e8f0" }}>{cpuUsage.toFixed(1)}%</div>
              </div>
              <div>
                <div style={{ fontSize: "0.7rem", color: "#64748b", textTransform: "uppercase", marginBottom: 4 }}>Memory</div>
                <div style={{ fontSize: "0.85rem", color: "#e2e8f0" }}>{memUsage.toFixed(1)}%</div>
              </div>
              <div>
                <div style={{ fontSize: "0.7rem", color: "#64748b", textTransform: "uppercase", marginBottom: 4 }}>Agents</div>
                <div style={{ fontSize: "0.85rem", color: "#e2e8f0" }}>{activeAgents} / {agents.length}</div>
              </div>
            </div>
          </div>

          {/* Mesh Controls */}
          <div style={cardStyle}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
              <span style={{ fontFamily: "monospace", fontSize: "0.9rem", color: "#22d3ee", fontWeight: 600 }}>MESH NETWORK</span>
              <button type="button"
                onClick={() => void handleDiscover()}
                disabled={discovering}
                style={{
                  padding: "5px 14px", borderRadius: 6, border: "1px solid #22d3ee",
                  background: "rgba(34,211,238,0.1)", color: "#22d3ee", cursor: "pointer",
                  fontFamily: "monospace", fontSize: "0.78rem", fontWeight: 600,
                }}
              >
                {discovering ? "Discovering..." : "Discover Peers"}
              </button>
            </div>

            {meshError && <div style={{ color: "#f87171", fontSize: "0.82rem", marginBottom: 8 }}>{meshError}</div>}
            {meshSuccess && <div style={{ color: "#4ade80", fontSize: "0.82rem", marginBottom: 8 }}>{meshSuccess}</div>}

            {/* Sync status */}
            {syncStatus && (
              <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 12, marginBottom: 16 }}>
                <div>
                  <div style={{ fontSize: "0.7rem", color: "#64748b", textTransform: "uppercase", marginBottom: 4 }}>Synced</div>
                  <div style={{ fontSize: "0.85rem", color: syncStatus.synced ? "#4ade80" : "#f59e0b" }}>{syncStatus.synced ? "Yes" : "No"}</div>
                </div>
                <div>
                  <div style={{ fontSize: "0.7rem", color: "#64748b", textTransform: "uppercase", marginBottom: 4 }}>Last Sync</div>
                  <div style={{ fontSize: "0.85rem", color: "#e2e8f0" }}>{syncStatus.last_sync ? new Date(syncStatus.last_sync * 1000).toLocaleTimeString() : "Never"}</div>
                </div>
                <div>
                  <div style={{ fontSize: "0.7rem", color: "#64748b", textTransform: "uppercase", marginBottom: 4 }}>Pending</div>
                  <div style={{ fontSize: "0.85rem", color: "#e2e8f0" }}>{syncStatus.pending_items}</div>
                </div>
              </div>
            )}

            {/* Peer list */}
            {peers.length > 0 ? (
              <div style={{ marginBottom: 16 }}>
                {peers.map(p => (
                  <div key={p.peer_id} style={{ display: "flex", alignItems: "center", gap: 10, padding: "6px 0", borderBottom: "1px solid rgba(30,41,59,0.5)", fontSize: "0.82rem" }}>
                    <span style={{ width: 8, height: 8, borderRadius: "50%", background: p.status === "Connected" || p.status === "Authenticated" ? "#4ade80" : "#f59e0b", display: "inline-block" }} />
                    <span style={{ color: "#e2e8f0", fontFamily: "monospace", flex: 1 }}>{p.name || p.peer_id.slice(0, 12)}</span>
                    <span style={{ color: "#64748b", fontSize: "0.75rem" }}>{p.address}:{p.port}</span>
                    <span style={{ color: "#94a3b8", fontSize: "0.72rem" }}>{p.status}</span>
                  </div>
                ))}
              </div>
            ) : (
              <div style={{ fontSize: "0.82rem", color: "#64748b", marginBottom: 16, textAlign: "center", padding: "0.5rem 0" }}>
                No peers connected. Click "Discover Peers" to scan the network.
              </div>
            )}

            {/* Distribute Task */}
            <div style={{ marginBottom: 14 }}>
              <div style={{ fontSize: "0.72rem", color: "#64748b", textTransform: "uppercase", marginBottom: 6 }}>Distribute Task</div>
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  type="text" value={taskInput} onChange={e => setTaskInput(e.target.value)}
                  placeholder="Task description"
                  style={{ flex: 2, padding: "6px 10px", background: "#0f172a", border: "1px solid #334155", borderRadius: 6, color: "#e2e8f0", fontFamily: "monospace", fontSize: "0.8rem" }}
                />
                <input
                  type="text" value={taskAgentIds} onChange={e => setTaskAgentIds(e.target.value)}
                  placeholder="Agent IDs (comma sep)"
                  style={{ flex: 1, padding: "6px 10px", background: "#0f172a", border: "1px solid #334155", borderRadius: 6, color: "#e2e8f0", fontFamily: "monospace", fontSize: "0.8rem" }}
                />
                <button type="button"
                  onClick={() => void handleDistributeTask()}
                  disabled={!taskInput.trim()}
                  style={{ padding: "6px 14px", borderRadius: 6, border: "1px solid #22d3ee", background: "rgba(34,211,238,0.1)", color: "#22d3ee", cursor: "pointer", fontFamily: "monospace", fontSize: "0.78rem" }}
                >
                  Send
                </button>
              </div>
            </div>

            {/* Migrate Agent */}
            <div>
              <div style={{ fontSize: "0.72rem", color: "#64748b", textTransform: "uppercase", marginBottom: 6 }}>Migrate Agent</div>
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  type="text" value={migrateAgentId} onChange={e => setMigrateAgentId(e.target.value)}
                  placeholder="Agent ID"
                  style={{ flex: 1, padding: "6px 10px", background: "#0f172a", border: "1px solid #334155", borderRadius: 6, color: "#e2e8f0", fontFamily: "monospace", fontSize: "0.8rem" }}
                />
                <input
                  type="text" value={migrateTarget} onChange={e => setMigrateTarget(e.target.value)}
                  placeholder="Target peer ID"
                  style={{ flex: 1, padding: "6px 10px", background: "#0f172a", border: "1px solid #334155", borderRadius: 6, color: "#e2e8f0", fontFamily: "monospace", fontSize: "0.8rem" }}
                />
                <button type="button"
                  onClick={() => void handleMigrateAgent()}
                  disabled={!migrateAgentId.trim() || !migrateTarget.trim()}
                  style={{ padding: "6px 14px", borderRadius: 6, border: "1px solid #f59e0b", background: "rgba(245,158,11,0.1)", color: "#f59e0b", cursor: "pointer", fontFamily: "monospace", fontSize: "0.78rem" }}
                >
                  Migrate
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
