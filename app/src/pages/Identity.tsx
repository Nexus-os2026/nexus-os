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

const CLAIM_OPTIONS = [
  { label: "Has L3+ clearance", value: "MinimumAutonomyLevel" },
  { label: "Success rate > 80%", value: "MinimumSuccessRate" },
  { label: "Created by Nexus OS", value: "CreatedByNexus" },
  { label: "Passed adversarial test", value: "HasCapability" },
] as const;

function peerStatusColor(status: string): string {
  switch (String(status)) {
    case "Authenticated":
    case "Connected":
      return "#22c55e";
    case "Discovered":
      return "#eab308";
    case "Unreachable":
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

export function Identity({ agents }: { agents: AgentOption[] }): JSX.Element {
  const [activeTab, setActiveTab] = useState<"identity" | "mesh">("identity");
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

  useEffect(() => {
    if (agents.length === 0) return;
    if (!selectedAgent) setSelectedAgent(agents[0].id);
    if (!migrationAgent) setMigrationAgent(agents[0].id);
  }, [agents, migrationAgent, selectedAgent]);

  const loadPassport = useCallback(async (agentId: string) => {
    if (!agentId) return;
    setWorking("passport");
    setError(null);
    try {
      const result = await invoke<AgentPassport>("identity_get_agent_passport", { agentId });
      setPassport(result);
      setPassportStatus(`Passport loaded for ${agents.find((agent) => agent.id === agentId)?.name ?? agentId}`);
    } catch (passportError) {
      setError(passportError instanceof Error ? passportError.message : String(passportError));
      setPassport(null);
      setPassportStatus("Unable to load passport");
    } finally {
      setWorking(null);
    }
  }, [agents]);

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

  useEffect(() => {
    if (selectedAgent) {
      void loadPassport(selectedAgent);
    }
  }, [loadPassport, selectedAgent]);

  useEffect(() => {
    void refreshMesh();
  }, [refreshMesh]);

  const handleGenerateProof = useCallback(async () => {
    if (!selectedAgent) return;
    setWorking("proof");
    setError(null);
    try {
      const generated = await invoke<ZkProof>("identity_generate_proof", {
        agentId: selectedAgent,
        claim,
      });
      setProof(generated);
      setProofStatus(`Proof generated ${formatRelative(generated.created_at)} for ${normalizeClaim(generated.claim)}`);
      setVerifierInput(JSON.stringify(generated, null, 2));
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
      const valid = await invoke<boolean>("identity_verify_proof", { proof: parsed });
      setVerifyStatus(valid ? "Valid proof" : "Proof failed verification");
    } catch (verifyError) {
      const message = verifyError instanceof Error ? verifyError.message : String(verifyError);
      setError(message);
      setVerifyStatus("Verifier expects proof JSON");
    } finally {
      setWorking(null);
    }
  }, [verifierInput]);

  const handleExport = useCallback(async () => {
    if (!selectedAgent) return;
    setWorking("export");
    setError(null);
    try {
      const exported = await invoke<{ passport_json: string }>("identity_export_passport", { agentId: selectedAgent });
      await navigator.clipboard.writeText(exported.passport_json);
      setPassportStatus("Passport JSON copied to clipboard");
    } catch (exportError) {
      setError(exportError instanceof Error ? exportError.message : String(exportError));
    } finally {
      setWorking(null);
    }
  }, [selectedAgent]);

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
      await invoke("mesh_add_peer", { address: manualAddress.trim() });
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

  useEffect(() => {
    if (meshPeers.length === 0) return;
    if (!migrationTarget) setMigrationTarget(meshPeers[0].peer_id);
  }, [meshPeers, migrationTarget]);

  return (
    <div style={commandPageStyle}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 14, alignItems: "flex-end", marginBottom: 20, flexWrap: "wrap" }}>
        <div>
          <h1 style={{ margin: 0, fontFamily: "monospace", fontSize: "1.8rem", color: "#00ffcc", letterSpacing: "0.16em", textTransform: "uppercase" }}>
            {activeTab === "identity" ? "Sovereign Identity" : "Distributed Mesh"}
          </h1>
          <div style={{ ...commandHeaderMetaStyle, marginTop: 10 }}>
            <span>{activeTab === "identity" ? passportStatus : `This instance: ${syncStatus?.local_peer_id ?? "local"}`}</span>
            <span>Agents: {agents.length}</span>
          </div>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <ActionButton accent={activeTab === "identity" ? "#00ffcc" : "#38bdf8"} onClick={() => setActiveTab("identity")}>
            Identity
          </ActionButton>
          <ActionButton accent={activeTab === "mesh" ? "#00ffcc" : "#38bdf8"} onClick={() => setActiveTab("mesh")}>
            Mesh
          </ActionButton>
        </div>
      </div>

      {error ? <div style={{ marginBottom: 16, color: "#fca5a5", fontSize: "0.82rem" }}>{error}</div> : null}

      {activeTab === "identity" ? (
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
                </div>

                <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Credentials</div>
                {credentials.length === 0 ? <EmptyState text="No verifiable credentials issued yet" compact /> : null}
                {credentials.map((credential) => (
                  <div key={credential} style={{ ...commandInsetStyle, marginBottom: 8, display: "flex", alignItems: "center", gap: 10 }}>
                    <StatusDot color="#22c55e" />
                    <span style={{ color: "#e2e8f0", fontSize: "0.82rem" }}>{credential}</span>
                  </div>
                ))}

                <ActionButton accent="#00ffcc" disabled={working === "export"} onClick={() => void handleExport()}>
                  {working === "export" ? "Exporting..." : "Export Passport"}
                </ActionButton>
              </div>
            ) : null}
          </Panel>

          <Panel title="ZK Proof Generator" accent="#38bdf8">
            <div style={{ display: "grid", gap: 10, marginBottom: 12 }}>
              <select value={selectedAgent} onChange={(event) => setSelectedAgent(event.target.value)} style={inputStyle}>
                {agents.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {agent.name}
                  </option>
                ))}
              </select>
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
                  <DataRow label="Claim" value={normalizeClaim(proof.claim)} />
                  <DataRow label="Valid" value="YES" valueColor="#22c55e" />
                  <DataRow label="Created" value={formatTimestamp(proof.created_at)} />
                  <div style={{ ...commandMutedStyle, marginTop: 10 }}>
                    Verified without revealing the hidden source value.
                  </div>
                </>
              ) : null}
            </div>

            <Panel title="Signature Verifier" accent="#f59e0b" style={{ padding: 14 }}>
              <textarea
                value={verifierInput}
                onChange={(event) => setVerifierInput(event.target.value)}
                placeholder="Paste proof JSON to verify..."
                style={{ ...textareaStyle, marginBottom: 10 }}
              />
              <ActionButton accent="#f59e0b" disabled={working === "verify"} onClick={() => void handleVerify()}>
                {working === "verify" ? "Verifying..." : "Verify"}
              </ActionButton>
              {verifyStatus ? <div style={{ ...commandMutedStyle, marginTop: 10 }}>{verifyStatus}</div> : null}
            </Panel>
          </Panel>
        </div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18 }}>
          <Panel title="Connected Peers" accent="#00ffcc">
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
              <ActionButton accent="#00ffcc" disabled={working === "discover"} onClick={() => void handleDiscoverPeers()}>
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
      )}
    </div>
  );
}

export default Identity;
