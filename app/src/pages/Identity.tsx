import { useEffect, useState } from "react";
import { getAgentIdentity, listIdentities, hasDesktopRuntime } from "../api/backend";
import type { IdentityInfo } from "../types";

export function Identity({ agents }: { agents: { id: string; name: string }[] }) {
  const [identities, setIdentities] = useState<IdentityInfo[]>([]);
  const [selected, setSelected] = useState<IdentityInfo | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setLoading(false);
      setIdentities(mockIdentities());
      return;
    }
    listIdentities()
      .then(setIdentities)
      .catch(() => setIdentities(mockIdentities()))
      .finally(() => setLoading(false));
  }, []);

  function handleSelect(agentId: string) {
    if (!hasDesktopRuntime()) {
      const mock = mockIdentities().find((i) => i.agent_id === agentId) ?? mockIdentities()[0];
      setSelected(mock);
      return;
    }
    getAgentIdentity(agentId)
      .then(setSelected)
      .catch(() => {});
  }

  return (
    <div style={{ padding: "1.5rem", maxWidth: 960, margin: "0 auto" }}>
      <h2 style={{ fontFamily: "var(--font-display, monospace)", color: "var(--text-primary, #e2e8f0)", marginBottom: "0.25rem" }}>
        Agent Identity
      </h2>
      <p style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.85rem", marginBottom: "1.5rem" }}>
        Ed25519 cryptographic identities with DID derivation
      </p>

      {loading ? (
        <p style={{ color: "var(--text-secondary, #94a3b8)" }}>Loading...</p>
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.5rem", marginBottom: "1.5rem" }}>
            {(identities.length > 0 ? identities : agents.map((a) => ({ agent_id: a.id, did: "", created_at: 0, public_key_hex: "" }))).map((row) => {
              const agent = agents.find((a) => a.id === row.agent_id);
              return (
                <button
                  key={row.agent_id}
                  onClick={() => handleSelect(row.agent_id)}
                  style={{
                    background: selected?.agent_id === row.agent_id ? "var(--accent-bg, #1e3a5f)" : "var(--bg-secondary, #1e293b)",
                    border: `1px solid ${selected?.agent_id === row.agent_id ? "var(--accent, #3b82f6)" : "var(--border, #334155)"}`,
                    borderRadius: 8,
                    padding: "0.75rem 1rem",
                    cursor: "pointer",
                    textAlign: "left",
                    color: "var(--text-primary, #e2e8f0)",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                  }}
                >
                  <span style={{ fontFamily: "monospace", fontSize: "0.85rem" }}>
                    {agent?.name ?? row.agent_id.slice(0, 8)}
                  </span>
                  {row.did && (
                    <span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem", fontFamily: "monospace" }}>
                      {row.did.slice(0, 24)}...
                    </span>
                  )}
                </button>
              );
            })}
          </div>

          {selected && (
            <div style={{ background: "var(--bg-secondary, #1e293b)", border: "1px solid var(--border, #334155)", borderRadius: 10, padding: "1.25rem" }}>
              <h3 style={{ color: "var(--text-primary, #e2e8f0)", fontFamily: "monospace", marginBottom: "1rem", fontSize: "0.95rem" }}>
                Identity Details
              </h3>
              <div style={{ display: "grid", gap: "0.6rem" }}>
                <Field label="Agent ID" value={selected.agent_id} />
                <Field label="DID" value={selected.did} />
                <Field label="Public Key" value={selected.public_key_hex} />
                <Field label="Created" value={new Date(selected.created_at * 1000).toISOString()} />
                <Field label="Algorithm" value="Ed25519" />
                <Field label="Key Format" value="did:key:z6Mk... (Multicodec + Base58btc)" />
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", gap: "1rem", alignItems: "baseline" }}>
      <span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.8rem", minWidth: 100 }}>{label}</span>
      <span style={{ color: "var(--text-primary, #e2e8f0)", fontFamily: "monospace", fontSize: "0.8rem", wordBreak: "break-all" }}>{value}</span>
    </div>
  );
}

function mockIdentities(): IdentityInfo[] {
  return [
    {
      agent_id: "00000000-0000-0000-0000-000000000001",
      did: "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
      created_at: Math.floor(Date.now() / 1000) - 3600,
      public_key_hex: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
    },
  ];
}

export default Identity;
