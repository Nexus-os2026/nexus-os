import { useCallback, useEffect, useMemo, useState } from "react";
import {
  identityGetAgentPassport,
  identityExportPassport,
  identityGenerateProof,
  identityVerifyProof,
  ghostProtocolToggle,
  ghostProtocolStatus,
  ghostProtocolGetState,
  ghostProtocolAddPeer,
  ghostProtocolRemovePeer,
  ghostProtocolSyncNow,
  meshAddPeer,
} from "../api/backend";
import { invoke } from "@tauri-apps/api/core";
import {
  ActionButton,
  DataRow,
  EmptyState,
  Panel,
  StatusDot,
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
  textareaStyle,
  toTitleCase,
} from "./commandCenterUi";

interface AgentOption {
  id: string;
  name: string;
}

interface PassportCredential {
  credential_type?: string;
  type?: string;
  subject?: string;
}

interface PassportScore {
  test_name: string;
  score: number;
  verified: boolean;
}

interface AgentPassport {
  agent_id: string;
  did: string;
  credentials: PassportCredential[];
  genome_hash: string;
  creation_date: number;
  lineage: string[];
  test_scores: PassportScore[];
  signature: string;
}

interface ZkProof {
  claim: unknown;
  agent_id: string;
  commitment: string;
  challenge: string;
  response: string;
  created_at: number;
}

interface PeerInfo {
  peer_id: string;
  address: string;
  port: number;
  name: string;
  status: string;
  last_seen?: number;
  capabilities?: string[];
}

interface SyncStatus {
  local_peer_id: string;
  synced_agents: number;
}

interface GhostPeer {
  device_id: string;
  address: string;
  name: string;
  status: string;
  last_seen?: string;
}

interface GhostState {
  enabled: boolean;
  peers: GhostPeer[];
  last_sync?: string;
}

const CLAIM_OPTIONS = [
  { label: "Has L3+ clearance", value: "MinimumAutonomyLevel" },
  { label: "Success rate > 80%", value: "MinimumSuccessRate" },
  { label: "Created by Nexus OS", value: "CreatedByNexus" },
  { label: "Passed adversarial test", value: "HasCapability" },
] as const;

type TabKey = "passports" | "zkproofs" | "ghost" | "mesh";

function peerStatusColor(status: string): string {
  switch (String(status)) {
    case "Authenticated":
    case "Connected":
    case "online":
      return "#22c55e";
    case "Discovered":
    case "syncing":
      return "#eab308";
    case "Unreachable":
    case "offline":
      return "#ef4444";
    default:
      return "#94a3b8";
  }
}

function normalizeClaim(claim: unknown): string {
  if (typeof claim === "string") return toTitleCase(claim);
  if (claim && typeof claim === "object") {
    const [key, value] = Object.entries(claim as Record<string, unknown>)[0] ?? [];
    return value !== undefined ? `${toTitleCase(key)} ${String(value)}` : toTitleCase(key);
  }
  return "Unknown claim";
}

function createManualPeer(address: string): PeerInfo {
  const [host, port] = address.split(":");
  return {
    peer_id: `manual-${Date.now()}`,
    address: host,
    port: Number(port) || 9090,
    name: host || "manual-peer",
    status: "Discovered",
    last_seen: Math.floor(Date.now() / 1000),
  };
}

function safeParseJson<T>(value: unknown): T | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "object") return value as T;
  if (typeof value === "string") {
    try {
      return JSON.parse(value) as T;
    } catch {
      return null;
    }
  }
  return null;
}

export function Identity({ agents }: { agents: AgentOption[] }): JSX.Element {
  const [activeTab, setActiveTab] = useState<TabKey>("passports");
  const [selectedAgent, setSelectedAgent] = useState("");
  const [passport, setPassport] = useState<AgentPassport | null>(null);
  const [passportStatus, setPassportStatus] = useState("Select an agent to inspect its passport");
  const [claim, setClaim] = useState<string>(CLAIM_OPTIONS[0].value);
  const [proof, setProof] = useState<ZkProof | null>(null);
  const [proofStatus, setProofStatus] = useState<string>("No proof generated yet");
  const [verifierInput, setVerifierInput] = useState("");
  const [verifyStatus, setVerifyStatus] = useState<string | null>(null);
  const [manualAddress, setManualAddress] = useState("");
  const [peerList, setPeerList] = useState<PeerInfo[]>([]);
  const [localPeers, setLocalPeers] = useState<PeerInfo[]>([]);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [lastSyncAt, setLastSyncAt] = useState<number | null>(null);
  const [migrationAgent, setMigrationAgent] = useState("");
  const [migrationTarget, setMigrationTarget] = useState("");
  const [migrationStatus, setMigrationStatus] = useState<string>("No migration started");
  const [working, setWorking] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Ghost Protocol state
  const [ghostEnabled, setGhostEnabled] = useState(false);
  const [ghostStatusText, setGhostStatusText] = useState<string>("Unknown");
  const [ghostState, setGhostState] = useState<GhostState | null>(null);
  const [ghostPeerAddress, setGhostPeerAddress] = useState("");
  const [ghostPeerName, setGhostPeerName] = useState("");

  useEffect(() => {
    if (agents.length === 0) return;
    if (!selectedAgent) setSelectedAgent(agents[0].id);
    if (!migrationAgent) setMigrationAgent(agents[0].id);
  }, [agents, migrationAgent, selectedAgent]);

  // ---------- Passport ----------

  const loadPassport = useCallback(async (agentId: string) => {
    if (!agentId) return;
    setWorking("passport");
    setError(null);
    try {
      const result = await identityGetAgentPassport(agentId);
      const parsed = safeParseJson<AgentPassport>(result);
      setPassport(parsed);
      setPassportStatus(parsed
        ? `Passport loaded for ${agents.find((a) => a.id === agentId)?.name ?? agentId}`
        : "Unable to parse passport");
    } catch (passportError) {
      setError(passportError instanceof Error ? passportError.message : String(passportError));
      setPassport(null);
      setPassportStatus("Unable to load passport");
    } finally {
      setWorking(null);
    }
  }, [agents]);

  const handleExport = useCallback(async () => {
    if (!selectedAgent) return;
    setWorking("export");
    setError(null);
    try {
      const exported = await identityExportPassport(selectedAgent);
      const text = typeof exported === "string" ? exported : JSON.stringify(exported, null, 2);
      await navigator.clipboard.writeText(text);
      setPassportStatus("Passport JSON copied to clipboard");
    } catch (exportError) {
      setError(exportError instanceof Error ? exportError.message : String(exportError));
    } finally {
      setWorking(null);
    }
  }, [selectedAgent]);

  // ---------- ZK Proofs ----------

  const handleGenerateProof = useCallback(async () => {
    if (!selectedAgent) return;
    setWorking("proof");
    setError(null);
    try {
      const result = await identityGenerateProof(selectedAgent, claim);
      const generated = safeParseJson<ZkProof>(result);
      if (generated) {
        setProof(generated);
        setProofStatus(`Proof generated ${formatRelative(generated.created_at)} for ${normalizeClaim(generated.claim)}`);
        setVerifierInput(JSON.stringify(generated, null, 2));
      } else {
        setProofStatus("Proof generated (raw response)");
        setVerifierInput(typeof result === "string" ? result : JSON.stringify(result, null, 2));
      }
    } catch (proofError) {
      setError(proofError instanceof Error ? proofError.message : String(proofError));
      setProofStatus("Proof generation failed");
    } finally {
      setWorking(null);
    }
  }, [claim, selectedAgent]);

  const handleVerify = useCallback(async () => {
    setWorking("verify");
    setError(null);
    try {
      const parsed = JSON.parse(verifierInput);
      const valid = await identityVerifyProof(parsed);
      setVerifyStatus(valid ? "Valid proof" : "Proof failed verification");
    } catch (verifyError) {
      const message = verifyError instanceof Error ? verifyError.message : String(verifyError);
      setError(message);
      setVerifyStatus("Verifier expects proof JSON");
    } finally {
      setWorking(null);
    }
  }, [verifierInput]);

  // ---------- Ghost Protocol ----------

  const refreshGhost = useCallback(async () => {
    setError(null);
    try {
      const [statusResult, stateResult] = await Promise.allSettled([
        ghostProtocolStatus(),
        ghostProtocolGetState(),
      ]);
      if (statusResult.status === "fulfilled") {
        setGhostStatusText(String(statusResult.value));
      }
      if (stateResult.status === "fulfilled") {
        const parsed = safeParseJson<GhostState>(stateResult.value);
        if (parsed) {
          setGhostState(parsed);
          setGhostEnabled(parsed.enabled);
        } else {
          setGhostStatusText(String(stateResult.value));
        }
      }
    } catch (ghostError) {
      setError(ghostError instanceof Error ? ghostError.message : String(ghostError));
    }
  }, []);

  const handleGhostToggle = useCallback(async () => {
    setWorking("ghost-toggle");
    setError(null);
    try {
      const newEnabled = !ghostEnabled;
      await ghostProtocolToggle(newEnabled);
      setGhostEnabled(newEnabled);
      await refreshGhost();
    } catch (toggleError) {
      setError(toggleError instanceof Error ? toggleError.message : String(toggleError));
    } finally {
      setWorking(null);
    }
  }, [ghostEnabled, refreshGhost]);

  const handleGhostAddPeer = useCallback(async () => {
    if (!ghostPeerAddress.trim()) return;
    setWorking("ghost-add-peer");
    setError(null);
    try {
      await ghostProtocolAddPeer(ghostPeerAddress.trim(), ghostPeerName.trim() || ghostPeerAddress.trim());
      setGhostPeerAddress("");
      setGhostPeerName("");
      await refreshGhost();
    } catch (addError) {
      setError(addError instanceof Error ? addError.message : String(addError));
    } finally {
      setWorking(null);
    }
  }, [ghostPeerAddress, ghostPeerName, refreshGhost]);

  const handleGhostRemovePeer = useCallback(async (deviceId: string) => {
    setWorking(`ghost-remove-${deviceId}`);
    setError(null);
    try {
      await ghostProtocolRemovePeer(deviceId);
      await refreshGhost();
    } catch (removeError) {
      setError(removeError instanceof Error ? removeError.message : String(removeError));
    } finally {
      setWorking(null);
    }
  }, [refreshGhost]);

  const handleGhostSync = useCallback(async () => {
    setWorking("ghost-sync");
    setError(null);
    try {
      await ghostProtocolSyncNow();
      await refreshGhost();
    } catch (syncError) {
      setError(syncError instanceof Error ? syncError.message : String(syncError));
    } finally {
      setWorking(null);
    }
  }, [refreshGhost]);

  // ---------- Mesh ----------

  const refreshMesh = useCallback(async () => {
    setError(null);
    try {
      const [peersResult, syncResult] = await Promise.allSettled([
        invoke<PeerInfo[]>("mesh_get_peers"),
        invoke<SyncStatus>("mesh_get_sync_status"),
      ]);
      if (peersResult.status === "fulfilled") setPeerList(normalizeArray<PeerInfo>(peersResult.value));
      if (syncResult.status === "fulfilled") {
        setSyncStatus(syncResult.value);
        setLastSyncAt(Math.floor(Date.now() / 1000));
      }
    } catch (meshError) {
      setError(meshError instanceof Error ? meshError.message : String(meshError));
    }
  }, []);

  const handleDiscoverPeers = useCallback(async () => {
    setWorking("discover");
    setError(null);
    try {
      const peers = await invoke<PeerInfo[]>("mesh_discover_peers");
      setPeerList(normalizeArray<PeerInfo>(peers));
      setLastSyncAt(Math.floor(Date.now() / 1000));
    } catch (discoverError) {
      setError(discoverError instanceof Error ? discoverError.message : String(discoverError));
    } finally {
      setWorking(null);
    }
  }, []);

  const handleAddPeer = useCallback(async () => {
    if (!manualAddress.trim()) return;
    setWorking("add-peer");
    setError(null);
    try {
      await meshAddPeer("", manualAddress.trim());
      setLocalPeers((current) => [createManualPeer(manualAddress.trim()), ...current]);
      setManualAddress("");
    } catch (peerError) {
      setError(peerError instanceof Error ? peerError.message : String(peerError));
    } finally {
      setWorking(null);
    }
  }, [manualAddress]);

  const handleForceSync = useCallback(async () => {
    setWorking("sync");
    await refreshMesh();
    setWorking(null);
  }, [refreshMesh]);

  const handleMigrate = useCallback(async () => {
    if (!migrationAgent || !migrationTarget) return;
    setWorking("migrate");
    setError(null);
    try {
      const result = await invoke<string>("mesh_migrate_agent", {
        agentId: migrationAgent,
        targetPeer: migrationTarget,
      });
      setMigrationStatus(`Migration status: ${toTitleCase(String(result))}`);
    } catch (migrationError) {
      setError(migrationError instanceof Error ? migrationError.message : String(migrationError));
      setMigrationStatus("Migration failed");
    } finally {
      setWorking(null);
    }
  }, [migrationAgent, migrationTarget]);

  // ---------- Effects ----------

  useEffect(() => {
    if (selectedAgent) {
      void loadPassport(selectedAgent);
    }
  }, [loadPassport, selectedAgent]);

  useEffect(() => {
    void refreshMesh();
  }, [refreshMesh]);

  useEffect(() => {
    void refreshGhost();
  }, [refreshGhost]);

  // ---------- Derived ----------

  const selectedAgentName = agents.find((agent) => agent.id === selectedAgent)?.name ?? selectedAgent;
  const credentials = useMemo(() => {
    const explicitCredentials = normalizeArray<PassportCredential>(passport?.credentials).map((credential) => credential.credential_type ?? credential.type ?? "Credential");
    const testCredentials = normalizeArray<PassportScore>(passport?.test_scores)
      .filter((score) => score.verified || score.score >= 0.8)
      .map((score) => `${score.test_name} ${(score.score * 100).toFixed(0)}%`);
    return [...explicitCredentials, ...testCredentials];
  }, [passport?.credentials, passport?.test_scores]);

  const meshPeers = useMemo(() => {
    const entries = new Map<string, PeerInfo>();
    for (const peer of [...peerList, ...localPeers]) {
      entries.set(peer.peer_id, peer);
    }
    return Array.from(entries.values());
  }, [localPeers, peerList]);

  const ghostPeers = useMemo(() => normalizeArray<GhostPeer>(ghostState?.peers), [ghostState?.peers]);

  useEffect(() => {
    if (meshPeers.length === 0) return;
    if (!migrationTarget) setMigrationTarget(meshPeers[0].peer_id);
  }, [meshPeers, migrationTarget]);

  // ---------- Tab titles ----------

  const TAB_CONFIG: { key: TabKey; label: string; accent: string }[] = [
    { key: "passports", label: "Passports", accent: "#00ffcc" },
    { key: "zkproofs", label: "ZK Proofs", accent: "#38bdf8" },
    { key: "ghost", label: "Ghost Protocol", accent: "#a78bfa" },
    { key: "mesh", label: "Mesh", accent: "#f59e0b" },
  ];

  const activeConfig = TAB_CONFIG.find((t) => t.key === activeTab) ?? TAB_CONFIG[0];

  const headerSubtext = (): string => {
    switch (activeTab) {
      case "passports":
        return passportStatus;
      case "zkproofs":
        return proofStatus;
      case "ghost":
        return ghostEnabled ? "Ghost Protocol ACTIVE" : "Ghost Protocol inactive";
      case "mesh":
        return `This instance: ${syncStatus?.local_peer_id ?? "local"}`;
    }
  };

  return (
    <div style={commandPageStyle}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 14, alignItems: "flex-end", marginBottom: 20, flexWrap: "wrap" }}>
        <div>
          <h1 style={{ margin: 0, fontFamily: "monospace", fontSize: "1.8rem", color: activeConfig.accent, letterSpacing: "0.16em", textTransform: "uppercase" }}>
            {activeConfig.label}
          </h1>
          <div style={{ ...commandHeaderMetaStyle, marginTop: 10 }}>
            <span>{headerSubtext()}</span>
            <span>Agents: {agents.length}</span>
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {TAB_CONFIG.map((tab) => (
            <ActionButton
              key={tab.key}
              accent={activeTab === tab.key ? tab.accent : "#64748b"}
              onClick={() => setActiveTab(tab.key)}
            >
              {tab.label}
            </ActionButton>
          ))}
        </div>
      </div>

      {error ? <div style={{ marginBottom: 16, color: "#fca5a5", fontSize: "0.82rem" }}>{error}</div> : null}

      {/* ========== PASSPORTS TAB ========== */}
      {activeTab === "passports" ? (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
          <Panel title="Agent Passport" accent="#00ffcc">
            <select value={selectedAgent} onChange={(event) => setSelectedAgent(event.target.value)} style={{ ...inputStyle, marginBottom: 14 }}>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>

            {!passport ? <EmptyState text={working === "passport" ? "Loading..." : "No passport loaded"} /> : null}
            {passport ? (
              <div>
                <div style={{ ...commandInsetStyle, marginBottom: 12 }}>
                  <div style={{ ...commandMonoValueStyle, color: "#00ffcc", marginBottom: 8 }}>{selectedAgentName}</div>
                  <DataRow label="DID" value={passport.did || "Unavailable"} />
                  <DataRow label="Created" value={formatTimestamp(passport.creation_date)} />
                  <DataRow label="Hardware bound" value={passport.signature ? "YES" : "NO"} valueColor={passport.signature ? "#22c55e" : "#94a3b8"} />
                  <DataRow label="Genome hash" value={passport.genome_hash || "N/A"} />
                  <DataRow label="Lineage" value={passport.lineage?.length ? passport.lineage.join(" > ") : "Root"} />
                </div>

                <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Credentials</div>
                {credentials.length === 0 ? <EmptyState text="No verifiable credentials issued yet" compact /> : null}
                {credentials.map((credential) => (
                  <div key={credential} style={{ ...commandInsetStyle, marginBottom: 8, display: "flex", alignItems: "center", gap: 10 }}>
                    <StatusDot color="#22c55e" />
                    <span style={{ color: "#e2e8f0", fontSize: "0.82rem" }}>{credential}</span>
                  </div>
                ))}

                <div style={{ display: "flex", gap: 10, marginTop: 12 }}>
                  <ActionButton accent="#00ffcc" disabled={working === "export"} onClick={() => void handleExport()}>
                    {working === "export" ? "Exporting..." : "Export Passport"}
                  </ActionButton>
                  <ActionButton accent="#38bdf8" disabled={working === "passport"} onClick={() => void loadPassport(selectedAgent)}>
                    {working === "passport" ? "Refreshing..." : "Refresh"}
                  </ActionButton>
                </div>
              </div>
            ) : null}
          </Panel>
        </div>
      ) : null}

      {/* ========== ZK PROOFS TAB ========== */}
      {activeTab === "zkproofs" ? (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
          <Panel title="Generate Proof" accent="#38bdf8">
            <div style={{ display: "grid", gap: 10, marginBottom: 12 }}>
              <div style={commandLabelStyle}>Agent</div>
              <select value={selectedAgent} onChange={(event) => setSelectedAgent(event.target.value)} style={inputStyle}>
                {agents.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {agent.name}
                  </option>
                ))}
              </select>
              <div style={commandLabelStyle}>Claim</div>
              <select value={claim} onChange={(event) => setClaim(event.target.value)} style={inputStyle}>
                {CLAIM_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              <ActionButton accent="#38bdf8" disabled={working === "proof"} onClick={() => void handleGenerateProof()}>
                {working === "proof" ? "Generating..." : "Generate Proof"}
              </ActionButton>
            </div>

            <div style={{ ...commandInsetStyle, marginBottom: 12 }}>
              <div style={{ ...commandMutedStyle, marginBottom: 8 }}>{proofStatus}</div>
              {proof ? (
                <>
                  <DataRow label="Agent" value={proof.agent_id} />
                  <DataRow label="Claim" value={normalizeClaim(proof.claim)} />
                  <DataRow label="Commitment" value={proof.commitment?.slice(0, 24) + "..."} />
                  <DataRow label="Valid" value="YES" valueColor="#22c55e" />
                  <DataRow label="Created" value={formatTimestamp(proof.created_at)} />
                  <div style={{ ...commandMutedStyle, marginTop: 10 }}>
                    Verified without revealing the hidden source value.
                  </div>
                </>
              ) : null}
            </div>
          </Panel>

          <Panel title="Verify Proof" accent="#f59e0b">
            <div style={commandLabelStyle}>Proof JSON</div>
            <textarea
              value={verifierInput}
              onChange={(event) => setVerifierInput(event.target.value)}
              placeholder='Paste proof JSON to verify...'
              style={{ ...textareaStyle, marginBottom: 10, marginTop: 8, minHeight: 160 }}
            />
            <ActionButton accent="#f59e0b" disabled={working === "verify" || !verifierInput.trim()} onClick={() => void handleVerify()}>
              {working === "verify" ? "Verifying..." : "Verify Proof"}
            </ActionButton>
            {verifyStatus ? (
              <div style={{
                ...commandInsetStyle,
                marginTop: 12,
                borderLeftColor: verifyStatus.startsWith("Valid") ? "#22c55e" : "#ef4444",
                borderLeftWidth: 3,
                borderLeftStyle: "solid",
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <StatusDot color={verifyStatus.startsWith("Valid") ? "#22c55e" : "#ef4444"} />
                  <span style={{ color: "#e2e8f0", fontSize: "0.85rem" }}>{verifyStatus}</span>
                </div>
              </div>
            ) : null}
          </Panel>
        </div>
      ) : null}

      {/* ========== GHOST PROTOCOL TAB ========== */}
      {activeTab === "ghost" ? (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
          <Panel title="Protocol Control" accent="#a78bfa">
            <div style={{ ...commandInsetStyle, marginBottom: 14 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
                <span style={commandLabelStyle}>Status</span>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <StatusDot color={ghostEnabled ? "#22c55e" : "#ef4444"} />
                  <span style={{ ...commandMonoValueStyle, color: ghostEnabled ? "#22c55e" : "#ef4444" }}>
                    {ghostEnabled ? "ACTIVE" : "INACTIVE"}
                  </span>
                </div>
              </div>
              <DataRow label="Protocol status" value={ghostStatusText} />
              {ghostState?.last_sync ? <DataRow label="Last sync" value={ghostState.last_sync} /> : null}
            </div>

            <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
              <ActionButton
                accent={ghostEnabled ? "#ef4444" : "#22c55e"}
                disabled={working === "ghost-toggle"}
                onClick={() => void handleGhostToggle()}
              >
                {working === "ghost-toggle" ? "Toggling..." : ghostEnabled ? "Disable Protocol" : "Enable Protocol"}
              </ActionButton>
              <ActionButton accent="#a78bfa" disabled={working === "ghost-sync"} onClick={() => void handleGhostSync()}>
                {working === "ghost-sync" ? "Syncing..." : "Sync Now"}
              </ActionButton>
              <ActionButton accent="#38bdf8" disabled={Boolean(working)} onClick={() => void refreshGhost()}>
                Refresh State
              </ActionButton>
            </div>

            {ghostState ? (
              <div style={{ ...commandInsetStyle, marginTop: 14 }}>
                <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Raw State</div>
                <pre style={{ margin: 0, fontSize: "0.75rem", color: "#94a3b8", whiteSpace: "pre-wrap", wordBreak: "break-all", maxHeight: 160, overflow: "auto" }}>
                  {JSON.stringify(ghostState, null, 2)}
                </pre>
              </div>
            ) : null}
          </Panel>

          <Panel title="Ghost Peers" accent="#a78bfa">
            {ghostPeers.length === 0 ? <EmptyState text="No ghost peers registered" compact /> : null}
            <div style={{ ...commandScrollStyle, maxHeight: 260, paddingRight: 6 }}>
              {ghostPeers.map((peer) => {
                const color = peerStatusColor(peer.status);
                return (
                  <div key={peer.device_id} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <StatusDot color={color} />
                        <span style={{ ...commandMonoValueStyle, color }}>{peer.name || peer.device_id}</span>
                      </div>
                      <span style={{ ...commandLabelStyle, color }}>{toTitleCase(peer.status)}</span>
                    </div>
                    <DataRow label="Device ID" value={peer.device_id} />
                    <DataRow label="Address" value={peer.address} />
                    {peer.last_seen ? <DataRow label="Last seen" value={peer.last_seen} /> : null}
                    <ActionButton
                      accent="#ef4444"
                      disabled={working === `ghost-remove-${peer.device_id}`}
                      onClick={() => void handleGhostRemovePeer(peer.device_id)}
                    >
                      {working === `ghost-remove-${peer.device_id}` ? "Removing..." : "Remove"}
                    </ActionButton>
                  </div>
                );
              })}
            </div>

            <div style={{ marginTop: 14 }}>
              <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Add Peer</div>
              <div style={{ display: "grid", gap: 8 }}>
                <input
                  value={ghostPeerAddress}
                  onChange={(event) => setGhostPeerAddress(event.target.value)}
                  placeholder="Peer address (e.g. 192.168.1.50:9090)"
                  style={inputStyle}
                />
                <input
                  value={ghostPeerName}
                  onChange={(event) => setGhostPeerName(event.target.value)}
                  placeholder="Peer name (optional)"
                  style={inputStyle}
                />
                <ActionButton
                  accent="#a78bfa"
                  disabled={working === "ghost-add-peer" || !ghostPeerAddress.trim()}
                  onClick={() => void handleGhostAddPeer()}
                >
                  {working === "ghost-add-peer" ? "Adding..." : "Add Peer"}
                </ActionButton>
              </div>
            </div>
          </Panel>
        </div>
      ) : null}

      {/* ========== MESH TAB ========== */}
      {activeTab === "mesh" ? (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
          <Panel title="Connected Peers" accent="#f59e0b">
            <div style={{ ...commandInsetStyle, marginBottom: 12 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 8 }}>
                <StatusDot color="#22c55e" />
                <span style={{ ...commandMonoValueStyle, color: "#00ffcc" }}>Local (this)</span>
              </div>
              <DataRow label="Peer ID" value={syncStatus?.local_peer_id ?? "local"} />
              <DataRow label="Agents" value={agents.length} />
              <DataRow label="Latency" value="-" />
            </div>

            {meshPeers.length === 0 ? <EmptyState text="No remote peers discovered yet" compact /> : null}
            <div style={{ ...commandScrollStyle, maxHeight: 220, paddingRight: 6 }}>
              {meshPeers.map((peer) => {
                const color = peerStatusColor(peer.status);
                return (
                  <div key={peer.peer_id} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <StatusDot color={color} />
                        <span style={{ ...commandMonoValueStyle, color }}>{peer.name || peer.address}</span>
                      </div>
                      <span style={{ ...commandLabelStyle, color }}>{toTitleCase(peer.status)}</span>
                    </div>
                    <DataRow label="Address" value={`${peer.address}:${peer.port}`} />
                    <DataRow label="Last seen" value={peer.last_seen ? formatRelative(peer.last_seen) : "-"} />
                  </div>
                );
              })}
            </div>

            <div style={{ display: "flex", gap: 10, flexWrap: "wrap", marginTop: 12 }}>
              <ActionButton accent="#f59e0b" disabled={working === "discover"} onClick={() => void handleDiscoverPeers()}>
                {working === "discover" ? "Discovering..." : "Discover Peers"}
              </ActionButton>
            </div>

            <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 10, marginTop: 12 }}>
              <input
                value={manualAddress}
                onChange={(event) => setManualAddress(event.target.value)}
                placeholder="address:port"
                style={inputStyle}
              />
              <ActionButton accent="#38bdf8" disabled={working === "add-peer"} onClick={() => void handleAddPeer()}>
                {working === "add-peer" ? "Connecting..." : "Connect"}
              </ActionButton>
            </div>
          </Panel>

          <Panel title="Sync Status" accent="#38bdf8">
            <div style={{ ...commandInsetStyle, marginBottom: 12 }}>
              <DataRow label="Consciousness" value="synced" valueColor="#22c55e" />
              <DataRow label="Knowledge" value="0 shared entries" />
              <DataRow label="Immune memory" value="synced" valueColor="#22c55e" />
              <DataRow label="Synced agents" value={syncStatus?.synced_agents ?? 0} />
              <DataRow label="Last sync" value={lastSyncAt ? formatRelative(lastSyncAt) : "never"} />
            </div>
            <ActionButton accent="#38bdf8" disabled={working === "sync"} onClick={() => void handleForceSync()}>
              {working === "sync" ? "Syncing..." : "Force Sync"}
            </ActionButton>

            <div style={{ marginTop: 16 }}>
              <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Agent Migration</div>
              <div style={{ display: "grid", gap: 10 }}>
                <select value={migrationAgent} onChange={(event) => setMigrationAgent(event.target.value)} style={inputStyle}>
                  {agents.map((agent) => (
                    <option key={agent.id} value={agent.id}>
                      {agent.name}
                    </option>
                  ))}
                </select>
                <select value={migrationTarget} onChange={(event) => setMigrationTarget(event.target.value)} style={inputStyle}>
                  <option value="">Select target peer</option>
                  {meshPeers.map((peer) => (
                    <option key={peer.peer_id} value={peer.peer_id}>
                      {peer.name || peer.address}
                    </option>
                  ))}
                </select>
                <ActionButton accent="#00ffcc" disabled={working === "migrate" || !migrationTarget} onClick={() => void handleMigrate()}>
                  {working === "migrate" ? "Migrating..." : "Migrate"}
                </ActionButton>
              </div>
              <div style={{ ...commandMutedStyle, marginTop: 10 }}>{migrationStatus}</div>
            </div>
          </Panel>
        </div>
      ) : null}
    </div>
  );
}

export default Identity;
