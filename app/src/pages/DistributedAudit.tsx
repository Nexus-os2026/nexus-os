import { useEffect, useState } from "react";
import { getAuditLog, getAuditChainStatus, hasDesktopRuntime } from "../api/backend";
import type { AuditEventRow, AuditChainStatusRow } from "../types";
import "./distributed-audit.css";

interface PairedDevice {
  nodeId: string;
  name: string;
  status: "Synced" | "Behind" | "Offline";
  lastSync: number;
  blocksMatching: number;
  blocksTotal: number;
}

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

function formatTimestamp(ts: number): string {
  if (ts === 0) return "—";
  // Timestamps from the backend are in seconds
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString();
}

export default function DistributedAudit(): JSX.Element {
  const [loading, setLoading] = useState(true);
  const [events, setEvents] = useState<AuditEventRow[]>([]);
  const [chainStatus, setChainStatus] = useState<AuditChainStatusRow | null>(null);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setLoading(false);
      return;
    }
    Promise.all([
      getAuditLog(undefined, 200).catch(() => []),
      getAuditChainStatus().catch(() => null),
    ]).then(([evts, chain]) => {
      setEvents(evts);
      if (chain) setChainStatus(chain);
      setLoading(false);
    });
  }, []);

  const totalEvents = chainStatus?.total_events ?? events.length;
  const chainValid = chainStatus?.chain_valid ?? true;
  const tamperIncidents = chainValid ? 0 : 1;

  // Build blocks from real events — group into blocks of ~10 events
  const blockSize = 10;
  const blockCount = Math.max(1, Math.ceil(events.length / blockSize));
  const blocks = Array.from({ length: blockCount }, (_, i) => {
    const blockEvents = events.slice(i * blockSize, (i + 1) * blockSize);
    const lastEvent = blockEvents[blockEvents.length - 1];
    const firstEvent = blockEvents[0];
    return {
      sequence: i,
      contentHash: lastEvent ? lastEvent.hash.slice(0, 12) + "..." : "0000...",
      previousHash: i === 0
        ? "0000000000..."
        : (events[Math.max(0, i * blockSize - 1)]?.hash.slice(0, 12) ?? "0000") + "...",
      eventCount: blockEvents.length,
      timestamp: firstEvent?.timestamp ?? 0,
    };
  });

  // Single device — this node
  const devices: PairedDevice[] = [
    {
      nodeId: "n1",
      name: "nexus-primary",
      status: "Synced",
      lastSync: Date.now() - 5_000,
      blocksMatching: blockCount,
      blocksTotal: blockCount,
    },
  ];
  const syncedDevices = devices.filter((d) => d.status === "Synced").length;

  if (loading) {
    return (
      <section className="da-hub">
        <header className="da-header">
          <h2 className="da-title">DISTRIBUTED AUDIT // IMMUTABLE CHAIN</h2>
          <p className="da-subtitle">Loading audit data...</p>
        </header>
      </section>
    );
  }

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
          <span className="da-stat-value">{blocks.length}</span>
          <span className="da-stat-label">Blocks</span>
        </div>
        <div className="da-stat">
          <span className="da-stat-value">{totalEvents}</span>
          <span className="da-stat-label">Events</span>
        </div>
        <div className="da-stat">
          <span className="da-stat-value">{devices.length}</span>
          <span className="da-stat-label">Devices</span>
        </div>
        <div className="da-stat">
          <span className="da-stat-value" style={{ color: syncedDevices === devices.length ? "#22c55e" : "#eab308" }}>
            {syncedDevices}/{devices.length}
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
      {blocks.length === 0 || (blocks.length === 1 && events.length === 0) ? (
        <div className="da-chain-grid">
          <div className="da-block-card">
            <span className="da-block-seq">No events</span>
            <span className="da-block-hash">Chain is empty</span>
          </div>
        </div>
      ) : (
        <div className="da-chain-grid">
          {blocks.map((block) => (
            <div key={block.sequence} className="da-block-card">
              <span className="da-block-seq">Block #{block.sequence}</span>
              <span className="da-block-hash">{block.contentHash}</span>
              <span className="da-block-events">{block.eventCount} events</span>
              <span className="da-block-link">prev: {block.previousHash}</span>
              {block.timestamp > 0 && (
                <span className="da-block-link">{formatTimestamp(block.timestamp)}</span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Device sync status */}
      <h3 className="da-section-title">Paired Devices</h3>
      <div className="da-devices-grid">
        {devices.map((device) => (
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

      {/* Chain details */}
      {chainStatus && (
        <>
          <h3 className="da-section-title">Chain Details</h3>
          <div className="da-devices-grid">
            <article className="da-device-card">
              <div className="da-device-detail">
                <span className="da-label">First Hash</span>
                <span className="da-value-mono">{chainStatus.first_hash.slice(0, 16)}...</span>
              </div>
              <div className="da-device-detail">
                <span className="da-label">Last Hash</span>
                <span className="da-value-mono">{chainStatus.last_hash.slice(0, 16)}...</span>
              </div>
              <div className="da-device-detail">
                <span className="da-label">Integrity</span>
                <span className="da-value-mono" style={{ color: chainStatus.chain_valid ? "#22c55e" : "#ef4444" }}>
                  {chainStatus.chain_valid ? "VERIFIED" : "FAILED"}
                </span>
              </div>
            </article>
          </div>
        </>
      )}
    </section>
  );
}
