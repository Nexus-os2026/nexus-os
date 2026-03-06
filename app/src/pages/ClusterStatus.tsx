import "./cluster-status.css";

interface ClusterNode {
  id: string;
  name: string;
  address: string;
  state: "Active" | "Suspect" | "Down";
  lastHeartbeat: number;
  capabilities: string[];
}

const MOCK_NODES: ClusterNode[] = [
  { id: "n1", name: "nexus-primary", address: "10.0.1.10:9090", state: "Active", lastHeartbeat: Date.now() - 2000, capabilities: ["quorum", "replication", "audit-sync"] },
  { id: "n2", name: "nexus-worker-01", address: "10.0.1.11:9090", state: "Active", lastHeartbeat: Date.now() - 5000, capabilities: ["replication", "audit-sync"] },
  { id: "n3", name: "nexus-worker-02", address: "10.0.1.12:9090", state: "Suspect", lastHeartbeat: Date.now() - 45000, capabilities: ["replication"] },
  { id: "n4", name: "nexus-edge-01", address: "10.0.2.20:9090", state: "Down", lastHeartbeat: Date.now() - 300000, capabilities: ["audit-sync"] },
];

const STATE_COLORS: Record<string, string> = {
  Active: "#22c55e",
  Suspect: "#eab308",
  Down: "#ef4444",
};

function formatHeartbeat(ts: number): string {
  const secs = Math.round((Date.now() - ts) / 1000);
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
}

export default function ClusterStatus(): JSX.Element {
  const active = MOCK_NODES.filter((n) => n.state === "Active").length;
  const quorumReached = active >= Math.ceil(MOCK_NODES.length / 2);

  return (
    <section className="cs-hub">
      <header className="cs-header">
        <h2 className="cs-title">CLUSTER STATUS // NODE HEALTH</h2>
        <p className="cs-subtitle">{MOCK_NODES.length} nodes in cluster</p>
      </header>

      <div className="cs-summary">
        <div className="cs-stat">
          <span className="cs-stat-value">{MOCK_NODES.length}</span>
          <span className="cs-stat-label">Total Nodes</span>
        </div>
        <div className="cs-stat">
          <span className="cs-stat-value" style={{ color: "#22c55e" }}>{active}</span>
          <span className="cs-stat-label">Active</span>
        </div>
        <div className="cs-stat">
          <span className="cs-stat-value" style={{ color: quorumReached ? "#22c55e" : "#ef4444" }}>
            {quorumReached ? "REACHED" : "LOST"}
          </span>
          <span className="cs-stat-label">Quorum</span>
        </div>
      </div>

      <div className="cs-grid">
        {MOCK_NODES.map((node) => (
          <article key={node.id} className="cs-card">
            <div className="cs-card-top">
              <div className="cs-card-name-row">
                <span className="cs-state-dot" style={{ background: STATE_COLORS[node.state] }} />
                <h3 className="cs-card-name">{node.name}</h3>
              </div>
              <span className="cs-card-state" style={{ color: STATE_COLORS[node.state] }}>
                {node.state}
              </span>
            </div>
            <div className="cs-card-detail">
              <span className="cs-label">Address</span>
              <span className="cs-value-mono">{node.address}</span>
            </div>
            <div className="cs-card-detail">
              <span className="cs-label">Last Heartbeat</span>
              <span className="cs-value-mono">{formatHeartbeat(node.lastHeartbeat)}</span>
            </div>
            <div className="cs-card-caps">
              {node.capabilities.map((cap) => (
                <span key={cap} className="cs-cap-tag">{cap}</span>
              ))}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
