import { useEffect, useState, type ReactNode } from "react";
import { hasDesktopRuntime, checkLlmStatus } from "../api/backend";
import { Cpu, Key, ExternalLink } from "lucide-react";

interface Props {
  feature: string;
  children: ReactNode;
}

export default function RequiresLlm({ feature, children }: Props) {
  const [ready, setReady] = useState<boolean | null>(null);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setReady(false);
      return;
    }
    checkLlmStatus()
      .then((s) => {
        const available =
          s?.providers?.some((p: { available: boolean }) => p.available) ?? false;
        setReady(available);
      })
      .catch(() => setReady(false));
  }, []);

  if (ready === null) return null; // loading
  if (ready) return <>{children}</>;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "1.5rem",
        padding: "3rem",
        height: "100%",
        fontFamily: "var(--font-mono, monospace)",
        color: "var(--text-primary, #e2e8f0)",
      }}
    >
      <div
        style={{
          background: "var(--bg-secondary, #1e293b)",
          border: "1px solid var(--border, #334155)",
          borderRadius: 16,
          padding: "2.5rem",
          maxWidth: 520,
          width: "100%",
          textAlign: "center",
        }}
      >
        <Cpu size={48} style={{ opacity: 0.6, marginBottom: 16 }} />
        <h2 style={{ margin: "0 0 0.5rem", fontSize: "1.4rem" }}>
          {feature} needs an AI engine
        </h2>
        <p style={{ opacity: 0.7, margin: "0 0 1.5rem", lineHeight: 1.6 }}>
          This feature uses AI to work. Set up a provider to get started:
        </p>

        <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
          <button
            onClick={() => {
              if (hasDesktopRuntime()) {
                import("../api/backend").then((b) =>
                  b.terminalExecute?.("curl -fsSL https://ollama.com/install.sh | sh", "~")
                );
              }
            }}
            className="cursor-pointer"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 8,
              padding: "0.75rem 1.5rem",
              background: "var(--nexus-accent, #818cf8)",
              color: "#fff",
              border: "none",
              borderRadius: 8,
              fontSize: "0.9rem",
              fontWeight: 600,
              fontFamily: "inherit",
              cursor: "pointer",
            }}
          >
            <ExternalLink size={16} />
            Install Ollama (Free, Local, Private)
          </button>

          <a
            href="#/settings"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: 8,
              padding: "0.75rem 1.5rem",
              background: "transparent",
              color: "var(--text-primary, #e2e8f0)",
              border: "1px solid var(--border, #334155)",
              borderRadius: 8,
              fontSize: "0.9rem",
              fontWeight: 500,
              fontFamily: "inherit",
              textDecoration: "none",
            }}
          >
            <Key size={16} />
            I have an API key (OpenAI, Anthropic, etc.)
          </a>
        </div>

        <p
          style={{
            opacity: 0.5,
            margin: "1.5rem 0 0",
            fontSize: "0.8rem",
            lineHeight: 1.5,
          }}
        >
          Once set up, come back here and {feature} will work instantly.
        </p>
      </div>
    </div>
  );
}
