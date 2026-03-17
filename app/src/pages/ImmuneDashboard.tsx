import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ActionButton,
  DataRow,
  EmptyState,
  MetricBar,
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
  formatRelative,
  formatTimestamp,
  inputStyle,
  normalizeArray,
  slugify,
  toTitleCase,
} from "./commandCenterUi";

interface ImmuneStatus {
  threat_level: string;
  active_antibodies: number;
  threats_blocked: number;
  last_scan: number;
  privacy_violations_blocked: number;
}

interface ThreatEvent {
  id: string;
  threat_type: string;
  severity: string;
  agent_id: string;
  description: string;
  matched_pattern?: string | null;
  timestamp: number;
}

interface ThreatSignature {
  threat_hash: string;
  pattern: string;
  antibody_response: string;
  first_seen: number;
  times_blocked: number;
}

interface RoundResult {
  round: number;
  attacker_score: number;
  defender_score: number;
  attack_type: string;
  defense_successful: boolean;
}

interface ArenaSession {
  id: string;
  attacker_id: string;
  defender_id: string;
  rounds: number;
  results: RoundResult[];
}

interface AgentOption {
  id: string;
  name: string;
}

function threatPresentation(level: string): { label: string; color: string } {
  switch (String(level)) {
    case "Green":
      return { label: "LOW", color: "#22c55e" };
    case "Yellow":
      return { label: "MEDIUM", color: "#eab308" };
    case "Orange":
      return { label: "HIGH", color: "#fb923c" };
    case "Red":
      return { label: "CRITICAL", color: "#ef4444" };
    default:
      return { label: "UNKNOWN", color: "#94a3b8" };
  }
}

function severityColor(severity: string): string {
  switch (String(severity)) {
    case "Critical":
      return "#ef4444";
    case "High":
      return "#fb923c";
    case "Medium":
      return "#eab308";
    case "Low":
      return "#22c55e";
    default:
      return "#94a3b8";
  }
}

function fallbackThreatAction(threat: ThreatEvent): string {
  if (threat.matched_pattern) return "quarantined";
  if (threat.severity === "Critical" || threat.severity === "High") return "contained";
  return "monitored";
}

async function loadThreatLog(): Promise<ThreatEvent[]> {
  try {
    return normalizeArray<ThreatEvent>(await invoke("get_threat_log", { limit: 20 }));
  } catch {
    return normalizeArray<ThreatEvent>(await invoke("get_threat_log"));
  }
}

export default function ImmuneDashboard(): JSX.Element {
  const [status, setStatus] = useState<ImmuneStatus | null>(null);
  const [threats, setThreats] = useState<ThreatEvent[]>([]);
  const [memory, setMemory] = useState<ThreatSignature[]>([]);
  const [agents, setAgents] = useState<AgentOption[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [scanStatus, setScanStatus] = useState("Scanner standing by");
  const [arenaAttacker, setArenaAttacker] = useState("");
  const [arenaDefender, setArenaDefender] = useState("");
  const [arenaRounds, setArenaRounds] = useState(10);
  const [arenaRunning, setArenaRunning] = useState(false);
  const [arenaSession, setArenaSession] = useState<ArenaSession | null>(null);
  const [revealedRounds, setRevealedRounds] = useState(0);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const [statusResult, threatResult, memoryResult, agentResult] = await Promise.allSettled([
        invoke<ImmuneStatus>("get_immune_status"),
        loadThreatLog(),
        invoke<ThreatSignature[]>("get_immune_memory"),
        invoke<AgentOption[]>("list_agents"),
      ]);

      if (statusResult.status === "fulfilled") {
        setStatus(statusResult.value);
        setScanStatus(statusResult.value.last_scan ? `Last scan ${formatRelative(statusResult.value.last_scan)}` : "Scanner standing by");
      }
      if (threatResult.status === "fulfilled") {
        const sorted = normalizeArray<ThreatEvent>(threatResult.value).sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0));
        setThreats(sorted.slice(0, 20));
      }
      if (memoryResult.status === "fulfilled") {
        const sorted = normalizeArray<ThreatSignature>(memoryResult.value).sort((a, b) => (b.times_blocked ?? 0) - (a.times_blocked ?? 0));
        setMemory(sorted);
      }
      if (agentResult.status === "fulfilled") {
        setAgents(normalizeArray<AgentOption>(agentResult.value));
      }
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const interval = window.setInterval(() => void refresh(), 10_000);
    return () => window.clearInterval(interval);
  }, [refresh]);

  useEffect(() => {
    if (agents.length === 0) return;
    if (!arenaAttacker) setArenaAttacker(agents[0].id);
    if (!arenaDefender) setArenaDefender((agents[1] ?? agents[0]).id);
  }, [agents, arenaAttacker, arenaDefender]);

  useEffect(() => {
    if (!arenaSession) return;
    setRevealedRounds(0);
    const timer = window.setInterval(() => {
      setRevealedRounds((current) => {
        if (current >= arenaSession.results.length) {
          window.clearInterval(timer);
          return current;
        }
        return current + 1;
      });
    }, 220);
    return () => window.clearInterval(timer);
  }, [arenaSession]);

  const handleScan = useCallback(async () => {
    setScanning(true);
    setError(null);
    setScanStatus("Running full privacy scan...");
    try {
      await invoke("trigger_immune_scan");
      setScanStatus("Full scan completed");
      await refresh();
    } catch (scanError) {
      setError(scanError instanceof Error ? scanError.message : String(scanError));
      setScanStatus("Scan failed");
    } finally {
      setScanning(false);
    }
  }, [refresh]);

  const handleArenaStart = useCallback(async () => {
    if (!arenaAttacker || !arenaDefender) {
      setError("Select both an attacker and defender agent.");
      return;
    }
    setArenaRunning(true);
    setError(null);
    try {
      const session = await invoke<ArenaSession>("run_adversarial_session", {
        attackerId: arenaAttacker,
        defenderId: arenaDefender,
        rounds: arenaRounds,
      });
      setArenaSession({
        ...session,
        results: normalizeArray<RoundResult>(session?.results),
      });
    } catch (arenaError) {
      setError(arenaError instanceof Error ? arenaError.message : String(arenaError));
    } finally {
      setArenaRunning(false);
    }
  }, [arenaAttacker, arenaDefender, arenaRounds]);

  const threatTone = threatPresentation(status?.threat_level ?? "");
  const visibleArenaResults = arenaSession?.results.slice(0, revealedRounds) ?? [];
  const defenseWins = visibleArenaResults.filter((round) => round.defense_successful).length;
  const attackWins = visibleArenaResults.length - defenseWins;

  const registryEntries = useMemo(() => {
    return memory.slice(0, Math.max(status?.active_antibodies ?? 0, 4)).map((entry, index) => ({
      id: `antibody-${slugify(entry.pattern || entry.threat_hash)}-${String(index + 1).padStart(3, "0")}`,
      created: entry.first_seen,
      blocked: entry.times_blocked,
      response: entry.antibody_response,
    }));
  }, [memory, status?.active_antibodies]);

  const immuneMemoryHighlights = useMemo(() => memory.slice(0, 3), [memory]);

  return (
    <div style={commandPageStyle}>
      <div style={{ marginBottom: 20 }}>
        <h1 style={{ margin: 0, fontFamily: "monospace", fontSize: "1.8rem", color: "#00ffcc", letterSpacing: "0.16em", textTransform: "uppercase" }}>
          Immune System
        </h1>
        <div style={{ ...commandHeaderMetaStyle, marginTop: 10 }}>
          <span>{status ? `${status.threats_blocked} threats blocked` : "Loading immune telemetry"}</span>
          <span>{status ? `${status.active_antibodies} antibodies active` : "Antibody registry pending"}</span>
          <span>{scanStatus}</span>
        </div>
      </div>

      <div
        style={{
          marginBottom: 18,
          borderRadius: 16,
          border: `1px solid ${alpha(threatTone.color, 0.5)}`,
          background: `linear-gradient(90deg, ${alpha(threatTone.color, 0.22)}, rgba(4, 10, 18, 0.94))`,
          padding: "16px 18px",
          display: "flex",
          justifyContent: "space-between",
          gap: 14,
          alignItems: "center",
          flexWrap: "wrap",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <StatusDot color={threatTone.color} />
          <div>
            <div style={commandLabelStyle}>Threat Level</div>
            <div style={{ ...commandMonoValueStyle, color: threatTone.color, fontSize: "1.3rem", fontWeight: 700 }}>
              {threatTone.label}
            </div>
          </div>
        </div>
        <div style={{ display: "flex", gap: 22, flexWrap: "wrap" }}>
          <div>
            <div style={commandLabelStyle}>Last Scan</div>
            <div style={commandMonoValueStyle}>{status ? formatTimestamp(status.last_scan) : "Loading..."}</div>
          </div>
          <div>
            <div style={commandLabelStyle}>Privacy Flags</div>
            <div style={commandMonoValueStyle}>{status?.privacy_violations_blocked ?? "Loading..."}</div>
          </div>
        </div>
      </div>

      {error ? <div style={{ marginBottom: 16, color: "#fca5a5", fontSize: "0.82rem" }}>{error}</div> : null}

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(340px, 1fr))", gap: 18 }}>
        <Panel title="Live Threat Feed" accent="#00ffcc" style={{ minHeight: 330 }}>
          <div style={{ ...commandScrollStyle, maxHeight: 270, paddingRight: 6 }}>
            {loading ? <EmptyState text="Loading..." /> : null}
            {!loading && threats.length === 0 ? <EmptyState text="No threat events in the last 20 entries" /> : null}
            {threats.map((threat) => {
              const accent = severityColor(threat.severity);
              const action = fallbackThreatAction(threat);
              const stateLabel = threat.matched_pattern ? "BLOCKED" : "DETECTED";
              return (
                <article key={threat.id} style={{ ...commandInsetStyle, marginBottom: 10 }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 10 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                      <span style={{ ...commandMonoValueStyle, color: "#f8fafc" }}>{formatTimestamp(threat.timestamp, "short")}</span>
                      <span style={{ ...commandLabelStyle, color: accent }}>{stateLabel}</span>
                    </div>
                    <span style={{ ...commandMonoValueStyle, color: accent }}>{toTitleCase(threat.severity)}</span>
                  </div>
                  <div style={{ color: "#e2e8f0", fontSize: "0.88rem", marginBottom: 8 }}>{threat.description}</div>
                  <DataRow label="Threat" value={toTitleCase(threat.threat_type)} valueColor={accent} />
                  <DataRow label="Agent" value={threat.agent_id} />
                  <DataRow label="Action" value={action} />
                </article>
              );
            })}
          </div>
        </Panel>

        <Panel title="Antibody Registry" accent="#4ade80" style={{ minHeight: 330 }}>
          {loading ? <EmptyState text="Loading..." /> : null}
          {!loading && registryEntries.length === 0 ? <EmptyState text="No antibodies registered yet" /> : null}
          {registryEntries.map((entry) => (
            <article key={entry.id} style={{ ...commandInsetStyle, marginBottom: 10 }}>
              <div style={{ ...commandMonoValueStyle, color: "#4ade80", marginBottom: 8 }}>{entry.id}</div>
              <DataRow label="Created" value={formatRelative(entry.created)} />
              <DataRow label="Threats blocked" value={entry.blocked} valueColor="#00ffcc" />
              <div style={{ ...commandMutedStyle, marginTop: 8 }}>{entry.response || "Adaptive response waiting for first successful block"}</div>
            </article>
          ))}
          <div style={{ ...commandLabelStyle, marginTop: 12, color: "#4ade80" }}>
            Total: {status?.active_antibodies ?? registryEntries.length} antibodies active
          </div>
        </Panel>

        <Panel title="Immune Memory" accent="#38bdf8" style={{ minHeight: 310 }}>
          {loading ? <EmptyState text="Loading..." /> : null}
          {!loading && immuneMemoryHighlights.length === 0 ? <EmptyState text="No threat patterns stored yet" /> : null}
          {immuneMemoryHighlights.map((entry, index) => (
            <article key={entry.threat_hash} style={{ ...commandInsetStyle, marginBottom: 10 }}>
              <div style={{ ...commandMonoValueStyle, color: "#38bdf8", marginBottom: 8 }}>
                Pattern #{index + 1}: {entry.pattern || entry.threat_hash.slice(0, 18)}
              </div>
              <DataRow label="Blocked" value={`${entry.times_blocked} times`} valueColor="#4ade80" />
              <DataRow label="First seen" value={formatTimestamp(entry.first_seen)} />
            </article>
          ))}
          <div style={{ ...commandLabelStyle, marginTop: 12 }}>[{memory.length} patterns stored]</div>
        </Panel>

        <Panel title="Adversarial Arena" accent="#f59e0b" style={{ minHeight: 310 }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 10, marginBottom: 14 }}>
            <select value={arenaAttacker} onChange={(event) => setArenaAttacker(event.target.value)} style={inputStyle}>
              <option value="">Select attacker</option>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>
            <select value={arenaDefender} onChange={(event) => setArenaDefender(event.target.value)} style={inputStyle}>
              <option value="">Select defender</option>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>
            <input
              value={arenaRounds}
              min={1}
              max={50}
              type="number"
              onChange={(event) => setArenaRounds(Math.max(1, Math.min(50, Number(event.target.value) || 1)))}
              style={inputStyle}
            />
          </div>
          <ActionButton accent="#00ffcc" disabled={arenaRunning || !arenaAttacker || !arenaDefender} onClick={() => void handleArenaStart()}>
            {arenaRunning ? "Starting Session..." : "Start New Session"}
          </ActionButton>

          {arenaSession ? (
            <div style={{ marginTop: 16 }}>
              <div style={{ ...commandInsetStyle, marginBottom: 12 }}>
                <DataRow label="Attacker" value={arenaSession.attacker_id} />
                <DataRow label="Defender" value={arenaSession.defender_id} />
                <DataRow label="Rounds" value={arenaSession.rounds} />
                <DataRow label="Attack wins" value={attackWins} valueColor="#f87171" />
                <DataRow label="Defense wins" value={defenseWins} valueColor="#4ade80" />
              </div>
              <div style={{ ...commandScrollStyle, maxHeight: 160, paddingRight: 6 }}>
                {visibleArenaResults.map((round) => (
                  <div key={round.round} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 10 }}>
                      <span style={commandLabelStyle}>Round {round.round}</span>
                      <span style={{ ...commandMonoValueStyle, color: round.defense_successful ? "#4ade80" : "#f87171" }}>
                        {round.defense_successful ? "Defense Hold" : "Attack Win"}
                      </span>
                    </div>
                    <div style={{ marginBottom: 8 }}>
                      <div style={{ ...commandLabelStyle, marginBottom: 6 }}>Attack Vector</div>
                      <div style={{ color: "#e2e8f0", fontSize: "0.82rem" }}>{toTitleCase(round.attack_type)}</div>
                    </div>
                    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
                      <div>
                        <div style={{ ...commandLabelStyle, marginBottom: 6 }}>Attacker Score</div>
                        <MetricBar value={round.attacker_score * 100} color="#f87171" />
                      </div>
                      <div>
                        <div style={{ ...commandLabelStyle, marginBottom: 6 }}>Defender Score</div>
                        <MetricBar value={round.defender_score * 100} color="#4ade80" />
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <EmptyState text="Select two agents and run a session to stream arena results." />
          )}
        </Panel>

        <Panel title="Privacy Scanner" accent="#00ffcc" style={{ gridColumn: "1 / -1" }}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 18, flexWrap: "wrap", alignItems: "center" }}>
            <div>
              <div style={commandHeaderMetaStyle}>
                <span>Last scan: {status ? formatRelative(status.last_scan) : "Loading..."}</span>
                <span>Items flagged: {status?.privacy_violations_blocked ?? 0}</span>
              </div>
              <p style={{ ...commandMutedStyle, marginTop: 10, marginBottom: 0 }}>
                Monitoring: API keys, passwords, PII, IPs, sensitive file paths, outbound exfiltration patterns.
              </p>
            </div>
            <ActionButton accent="#00ffcc" disabled={scanning} onClick={() => void handleScan()}>
              {scanning ? "Running Scan..." : "Run Full Scan"}
            </ActionButton>
          </div>
        </Panel>
      </div>
    </div>
  );
}
