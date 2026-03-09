import "./distributed-audit.css";

interface AuditBlockInfo {
  sequence: number;
  contentHash: string;
  previousHash: string;
  eventCount: number;
  timestamp: number;
}

interface PairedDevice {
  nodeId: string;
  name: string;
  status: "Synced" | "Behind" | "Offline";
  lastSync: number;
  blocksMatching: number;
  blocksTotal: number;
}

const MOCK_BLOCKS: AuditBlockInfo[] = [
  { sequence: 0, contentHash: "a3f1c9e2d4b5...", previousHash: "0000000000...", eventCount: 5, timestamp: Date.now() - 600_000 },
  { sequence: 1, contentHash: "7b2e8f1a09c3...", previousHash: "a3f1c9e2d4b5...", eventCount: 8, timestamp: Date.now() - 300_000 },
  { sequence: 2, contentHash: "e5d4c3b2a190...", previousHash: "7b2e8f1a09c3...", eventCount: 3, timestamp: Date.now() - 60_000 },
];

const MOCK_DEVICES: PairedDevice[] = [
  { nodeId: "n1", name: "nexus-primary", status: "Synced", lastSync: Date.now() - 5_000, blocksMatching: 3, blocksTotal: 3 },
  { nodeId: "n2", name: "nexus-laptop", status: "Synced", lastSync: Date.now() - 15_000, blocksMatching: 3, blocksTotal: 3 },
  { nodeId: "n3", name: "nexus-edge", status: "Behind", lastSync: Date.now() - 120_000, blocksMatching: 2, blocksTotal: 3 },
];

const STATUS_COLORS: Record<string, string> = {
  Synced: "#22c55e",
  Behind: "#eab308",
  Offline: "#ef4444",
};

function formatTime(ts: number): string {
  const secs = Math.round((Date.now() - ts) / 1000);
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
}

export default function DistributedAudit(): JSX.Element {
  const tamperIncidents = 0;
  const totalEvents = MOCK_BLOCKS.reduce((sum, b) => sum + b.eventCount, 0);
  const syncedDevices = MOCK_DEVICES.filter((d) => d.status === "Synced").length;

  return (
    <section className="da-hub">
      <header className="da-header">
        <h2 className="da-title">DISTRIBUTED AUDIT // IMMUTABLE CHAIN</h2>
        <p className="da-subtitle">Cross-device verification and tamper detection</p>
      </header>

      {/* Tamper alert banner */}
      {tamperIncidents > 0 ? (
        <div className="da-tamper-banner">
          <span className="da-tamper-banner-icon">!</span>
          <span>{tamperIncidents} tamper incident{tamperIncidents > 1 ? "s" : ""} detected — review chain integrity</span>
        </div>
      ) : (
        <div className="da-tamper-banner da-tamper-banner--clean">
          <span className="da-tamper-banner-icon">OK</span>
          <span>Chain integrity verified — no tamper incidents detected</span>
        </div>
      )}

      {/* Summary stats */}
      <div className="da-summary">
        <div className="da-stat">
          <span className="da-stat-value">{MOCK_BLOCKS.length}</span>
          <span className="da-stat-label">Blocks</span>
        </div>
        <div className="da-stat">
          <span className="da-stat-value">{totalEvents}</span>
          <span className="da-stat-label">Events</span>
        </div>
        <div className="da-stat">
          <span className="da-stat-value">{MOCK_DEVICES.length}</span>
          <span className="da-stat-label">Devices</span>
        </div>
        <div className="da-stat">
          <span className="da-stat-value" style={{ color: syncedDevices === MOCK_DEVICES.length ? "#22c55e" : "#eab308" }}>
            {syncedDevices}/{MOCK_DEVICES.length}
          </span>
          <span className="da-stat-label">Synced</span>
        </div>
        <div className="da-stat">
          <span className="da-stat-value" style={{ color: tamperIncidents === 0 ? "#22c55e" : "#ef4444" }}>
            {tamperIncidents === 0 ? "CLEAN" : tamperIncidents}
          </span>
          <span className="da-stat-label">Tamper</span>
        </div>
      </div>

      {/* Chain block visualization */}
      <h3 className="da-section-title">Audit Chain</h3>
      <div className="da-chain-grid">
        {MOCK_BLOCKS.map((block) => (
          <div key={block.sequence} className="da-block-card">
            <span className="da-block-seq">Block #{block.sequence}</span>
            <span className="da-block-hash">{block.contentHash}</span>
            <span className="da-block-events">{block.eventCount} events</span>
            <span className="da-block-link">prev: {block.previousHash}</span>
          </div>
        ))}
      </div>

      {/* Device sync status */}
      <h3 className="da-section-title">Paired Devices</h3>
      <div className="da-devices-grid">
        {MOCK_DEVICES.map((device) => (
          <article key={device.nodeId} className="da-device-card">
            <div className="da-device-top">
              <div className="da-device-name-row">
                <span className="da-sync-dot" style={{ background: STATUS_COLORS[device.status] }} />
                <h3 className="da-device-name">{device.name}</h3>
              </div>
              <span className="da-device-status" style={{ color: STATUS_COLORS[device.status] }}>
                {device.status}
              </span>
            </div>
            <div className="da-device-detail">
              <span className="da-label">Last Sync</span>
              <span className="da-value-mono">{formatTime(device.lastSync)}</span>
            </div>
            <div className="da-device-detail">
              <span className="da-label">Blocks</span>
              <span className="da-value-mono">{device.blocksMatching}/{device.blocksTotal}</span>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
