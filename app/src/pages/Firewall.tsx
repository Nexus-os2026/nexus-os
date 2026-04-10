import { useEffect, useState } from "react";
import { getFirewallStatus, getFirewallPatterns, hasDesktopRuntime } from "../api/backend";
import type { FirewallStatus, FirewallPatterns } from "../types";

export function Firewall() {
  const [status, setStatus] = useState<FirewallStatus | null>(null);
  const [patterns, setPatterns] = useState<FirewallPatterns | null>(null);
  const [tab, setTab] = useState<"overview" | "patterns">("overview");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!hasDesktopRuntime()) { setLoading(false); return; }
    Promise.all([
      getFirewallStatus().then(setStatus).catch((e) => { if (import.meta.env.DEV) console.warn("[Firewall]", e); }),
      getFirewallPatterns().then(setPatterns).catch((e) => { if (import.meta.env.DEV) console.warn("[Firewall]", e); }),
    ]).finally(() => setLoading(false));
  }, []);

  if (loading) return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "#64748b", fontSize: 14 }}>
      Loading...
    </div>
  );

  return (
    <div style={{ padding: "1.5rem", maxWidth: 960, margin: "0 auto" }}>
      <h2 style={{ fontFamily: "var(--font-display, monospace)", color: "var(--text-primary, #e2e8f0)", marginBottom: "0.25rem" }}>
        Prompt Firewall
      </h2>
      <p style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.85rem", marginBottom: "1rem" }}>
        Fail-closed input/output filtering with egress governance
      </p>

      {/* Tab bar */}
      <div style={{ display: "flex", gap: "0.5rem", marginBottom: "1.5rem" }}>
        {(["overview", "patterns"] as const).map((t) => (
          <button type="button"
            key={t}
            onClick={() => setTab(t)}
            style={{
              background: tab === t ? "var(--accent, #14b8a6)" : "var(--bg-secondary, #1e293b)",
              border: `1px solid ${tab === t ? "var(--accent, #14b8a6)" : "var(--border, #334155)"}`,
              borderRadius: 6,
              padding: "0.4rem 1rem",
              color: tab === t ? "#fff" : "var(--text-secondary, #94a3b8)",
              cursor: "pointer",
              fontSize: "0.85rem",
              fontFamily: "monospace",
            }}
          >
            {t === "overview" ? "Overview" : "Pattern Library"}
          </button>
        ))}
      </div>

      {!status && !patterns && (
        <p style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.85rem", padding: "1rem 0" }}>
          Connect to desktop runtime to view firewall status.
        </p>
      )}
      {tab === "overview" && status && <OverviewPanel status={status} />}
      {tab === "patterns" && patterns && <PatternsPanel patterns={patterns} />}
    </div>
  );
}

function OverviewPanel({ status }: { status: FirewallStatus }) {
  const statStyle: React.CSSProperties = {
    background: "var(--bg-secondary, #1e293b)",
    border: "1px solid var(--border, #334155)",
    borderRadius: 8,
    padding: "1rem",
    textAlign: "center",
  };
  const labelStyle: React.CSSProperties = { color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem", marginBottom: "0.3rem" };
  const valueStyle: React.CSSProperties = { color: "var(--text-primary, #e2e8f0)", fontSize: "1.5rem", fontFamily: "monospace", fontWeight: 700 };
  const badgeGreen: React.CSSProperties = {
    display: "inline-block",
    background: "#065f4620",
    color: "#10b981",
    border: "1px solid #10b98140",
    borderRadius: 4,
    padding: "0.15rem 0.5rem",
    fontSize: "0.75rem",
    fontFamily: "monospace",
  };

  return (
    <div>
      {/* Status badge */}
      <div style={{ marginBottom: "1.25rem" }}>
        <span style={badgeGreen}>{status.status.toUpperCase()}</span>
        <span style={{ ...badgeGreen, marginLeft: "0.5rem" }}>{status.mode}</span>
      </div>

      {/* Stat grid */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))", gap: "0.75rem", marginBottom: "1.5rem" }}>
        <div style={statStyle}>
          <div style={labelStyle}>Injection Patterns</div>
          <div style={valueStyle}>{status.injection_pattern_count}</div>
        </div>
        <div style={statStyle}>
          <div style={labelStyle}>PII Patterns</div>
          <div style={valueStyle}>{status.pii_pattern_count}</div>
        </div>
        <div style={statStyle}>
          <div style={labelStyle}>Exfil Patterns</div>
          <div style={valueStyle}>{status.exfil_pattern_count}</div>
        </div>
        <div style={statStyle}>
          <div style={labelStyle}>Sensitive Paths</div>
          <div style={valueStyle}>{status.sensitive_path_count}</div>
        </div>
        <div style={statStyle}>
          <div style={labelStyle}>Overflow Threshold</div>
          <div style={{ ...valueStyle, fontSize: "1.1rem" }}>{(status.context_overflow_threshold_bytes / 1024).toFixed(0)} KB</div>
        </div>
        <div style={statStyle}>
          <div style={labelStyle}>Egress Rate Limit</div>
          <div style={valueStyle}>{status.egress_rate_limit_per_min}/min</div>
        </div>
      </div>

      {/* Detection features */}
      <div style={{ background: "var(--bg-secondary, #1e293b)", border: "1px solid var(--border, #334155)", borderRadius: 10, padding: "1rem" }}>
        <h3 style={{ color: "var(--text-primary, #e2e8f0)", fontSize: "0.9rem", marginBottom: "0.75rem", fontFamily: "monospace" }}>
          Detection Features
        </h3>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0.5rem" }}>
          <Check label="SSN Detection" enabled={status.ssn_detection} />
          <Check label="Passport Detection" enabled={status.passport_detection} />
          <Check label="Internal IP Detection" enabled={status.internal_ip_detection} />
          <Check label="Egress Default Deny" enabled={status.egress_default_deny} />
          <Check label="JSON Schema Validation" enabled={true} />
          <Check label="Homoglyph Detection" enabled={true} />
        </div>
      </div>
    </div>
  );
}

function PatternsPanel({ patterns }: { patterns: FirewallPatterns }) {
  const sectionStyle: React.CSSProperties = {
    background: "var(--bg-secondary, #1e293b)",
    border: "1px solid var(--border, #334155)",
    borderRadius: 10,
    padding: "1rem",
    marginBottom: "1rem",
  };
  const headerStyle: React.CSSProperties = {
    color: "var(--text-primary, #e2e8f0)",
    fontSize: "0.85rem",
    fontFamily: "monospace",
    marginBottom: "0.6rem",
    display: "flex",
    justifyContent: "space-between",
  };
  const chipStyle: React.CSSProperties = {
    display: "inline-block",
    background: "var(--bg-tertiary, #0f172a)",
    border: "1px solid var(--border, #334155)",
    borderRadius: 4,
    padding: "0.2rem 0.5rem",
    fontSize: "0.75rem",
    fontFamily: "monospace",
    color: "var(--text-primary, #e2e8f0)",
    margin: "0.15rem",
  };
  const regexStyle: React.CSSProperties = {
    ...chipStyle,
    color: "#f59e0b",
    borderColor: "#f59e0b40",
  };

  return (
    <div>
      <div style={sectionStyle}>
        <div style={headerStyle}>
          <span>Injection Patterns</span>
          <span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem" }}>{patterns.injection_patterns.length}</span>
        </div>
        <div style={{ display: "flex", flexWrap: "wrap" }}>
          {patterns.injection_patterns.map((p) => <span key={p} style={chipStyle}>{p}</span>)}
        </div>
      </div>

      <div style={sectionStyle}>
        <div style={headerStyle}>
          <span>PII Patterns</span>
          <span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem" }}>{patterns.pii_patterns.length}</span>
        </div>
        <div style={{ display: "flex", flexWrap: "wrap" }}>
          {patterns.pii_patterns.map((p) => <span key={p} style={chipStyle}>{p}</span>)}
        </div>
      </div>

      <div style={sectionStyle}>
        <div style={headerStyle}>
          <span>Exfiltration Patterns</span>
          <span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem" }}>{patterns.exfil_patterns.length}</span>
        </div>
        <div style={{ display: "flex", flexWrap: "wrap" }}>
          {patterns.exfil_patterns.map((p) => <span key={p} style={chipStyle}>{p}</span>)}
        </div>
      </div>

      <div style={sectionStyle}>
        <div style={headerStyle}>
          <span>Sensitive Paths</span>
          <span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem" }}>{patterns.sensitive_paths.length}</span>
        </div>
        <div style={{ display: "flex", flexWrap: "wrap" }}>
          {patterns.sensitive_paths.map((p) => <span key={p} style={chipStyle}>{p}</span>)}
        </div>
      </div>

      <div style={sectionStyle}>
        <div style={headerStyle}><span>Regex Patterns</span></div>
        <div style={{ display: "grid", gap: "0.4rem" }}>
          <div><span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem", marginRight: "0.5rem" }}>SSN:</span><span style={regexStyle}>{patterns.ssn_regex}</span></div>
          <div><span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem", marginRight: "0.5rem" }}>Passport:</span><span style={regexStyle}>{patterns.passport_regex}</span></div>
          <div><span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem", marginRight: "0.5rem" }}>Internal IP:</span><span style={regexStyle}>{patterns.internal_ip_regex}</span></div>
        </div>
      </div>
    </div>
  );
}

function Check({ label, enabled }: { label: string; enabled: boolean }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
      <span style={{ color: enabled ? "#10b981" : "#ef4444", fontSize: "0.9rem" }}>{enabled ? "\u2713" : "\u2717"}</span>
      <span style={{ color: "var(--text-primary, #e2e8f0)", fontSize: "0.8rem" }}>{label}</span>
    </div>
  );
}

export default Firewall;
