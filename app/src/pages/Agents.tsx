/**
 * Agents page — Phase 2 shell.
 *
 * The interactive panes (DAG viewer, streaming agent cards, event tape,
 * Director console) land in Phase 3. What lives here now is the skeleton
 * those panes hang off: a 4-region grid, a header showing live provider
 * health (driven by the swarm event bus via the store), and a footer that
 * reflects the currently active run. The shell is how we prove end-to-end
 * event flow: Rust broadcast → Tauri emit → swarmBus → store → React.
 */

import { useSwarmStore } from "../lib/swarm/store";
import { refreshProviderHealth } from "../lib/swarm/commands";
import type { ProviderHealth, ProviderHealthStatus } from "../lib/swarm/types";

function statusColor(status: ProviderHealthStatus): string {
  switch (status) {
    case "Ok":
      return "#22c55e";
    case "Degraded":
      return "#eab308";
    case "Unhealthy":
      return "#ef4444";
  }
}

function ProviderDot({ provider }: { provider: ProviderHealth }): JSX.Element {
  const onClick = (): void => {
    void refreshProviderHealth();
  };
  return (
    <button
      type="button"
      onClick={onClick}
      title={
        provider.latency_ms !== null
          ? `${provider.provider_id} — ${provider.status} (${provider.latency_ms}ms)`
          : `${provider.provider_id} — ${provider.status}`
      }
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        padding: "4px 10px",
        background: "rgba(30,41,59,0.4)",
        border: "1px solid rgba(100,116,139,0.25)",
        borderRadius: 999,
        color: "#cbd5e1",
        fontSize: 12,
        cursor: "pointer",
      }}
      aria-label={`refresh ${provider.provider_id} health`}
      data-testid={`provider-dot-${provider.provider_id}`}
    >
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: statusColor(provider.status),
          boxShadow: `0 0 6px ${statusColor(provider.status)}`,
          display: "inline-block",
        }}
      />
      <span>{provider.provider_id}</span>
      {provider.latency_ms !== null && provider.status === "Ok" && (
        <span style={{ color: "#64748b", fontSize: 11 }}>
          {provider.latency_ms}ms
        </span>
      )}
    </button>
  );
}

function ProviderStrip(): JSX.Element {
  const providers = useSwarmStore((s) => s.providerHealth);
  return (
    <header
      style={{
        display: "flex",
        gap: 8,
        alignItems: "center",
        padding: "10px 16px",
        borderBottom: "1px solid rgba(100,116,139,0.2)",
        background: "rgba(10,14,26,0.4)",
      }}
      data-testid="provider-strip"
    >
      <span style={{ color: "#94a3b8", fontSize: 11, textTransform: "uppercase", letterSpacing: "0.08em", marginRight: 4 }}>
        Providers
      </span>
      {providers.length === 0 && (
        <span style={{ color: "#475569", fontSize: 12 }}>no providers registered</span>
      )}
      {providers.map((p) => (
        <ProviderDot key={p.provider_id} provider={p} />
      ))}
    </header>
  );
}

function Placeholder({ label, testId }: { label: string; testId: string }): JSX.Element {
  return (
    <div
      data-testid={testId}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "#475569",
        fontSize: 13,
        fontFamily: "var(--font-mono, monospace)",
        border: "1px dashed rgba(100,116,139,0.25)",
        borderRadius: 10,
        background: "rgba(30,41,59,0.25)",
        minHeight: 0,
      }}
    >
      {label}
    </div>
  );
}

function RunFooter(): JSX.Element {
  const activeRun = useSwarmStore((s) => s.activeRun);
  return (
    <footer
      style={{
        padding: "10px 16px",
        borderTop: "1px solid rgba(100,116,139,0.2)",
        background: "rgba(10,14,26,0.4)",
        color: "#94a3b8",
        fontSize: 12,
        display: "flex",
        gap: 16,
        alignItems: "center",
      }}
      data-testid="run-footer"
    >
      {activeRun === null ? (
        <span style={{ color: "#475569" }}>no active run</span>
      ) : (
        <>
          <span>
            run:{" "}
            <span style={{ color: "#e2e8f0", fontFamily: "var(--font-mono, monospace)" }} data-testid="run-id">
              {activeRun.run_id}
            </span>
          </span>
          <span>nodes: {activeRun.dag.nodes.length}</span>
          <span>
            elapsed: {Math.max(0, Math.floor((Date.now() - activeRun.started_at_ms) / 1000))}s
          </span>
        </>
      )}
    </footer>
  );
}

export function Agents(): JSX.Element {
  return (
    <div
      data-testid="agents-page-shell"
      style={{
        display: "grid",
        gridTemplateRows: "auto 1fr auto",
        height: "100%",
        minHeight: 0,
        background: "#0a0e1a",
        color: "#e2e8f0",
      }}
    >
      <ProviderStrip />
      <main
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gridTemplateRows: "1fr 1fr",
          gap: 12,
          padding: 16,
          minHeight: 0,
        }}
      >
        <Placeholder label="DAG viewer (Phase 3)" testId="region-dag" />
        <Placeholder label="Streaming agents (Phase 3)" testId="region-swarm" />
        <Placeholder label="Event tape (Phase 3)" testId="region-events" />
        <Placeholder label="Director console (Phase 3)" testId="region-director" />
      </main>
      <RunFooter />
    </div>
  );
}

export default Agents;
