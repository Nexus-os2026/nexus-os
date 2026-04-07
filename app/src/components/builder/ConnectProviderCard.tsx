/**
 * ConnectProviderCard — Shows CLI provider auth status with in-app login.
 *
 * Renders a card for Claude CLI or Codex CLI showing connection state.
 * If not authenticated, provides a one-click button that spawns the CLI
 * login flow (opens browser for OAuth) and polls for success.
 */
import { useState, useEffect, useCallback } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { builderAuthenticateCli } from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  err: "#f85149",
  ok: "#3fb950",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
  sans: "system-ui,-apple-system,sans-serif",
};

interface Props {
  cli: "claude" | "codex";
  displayName: string;
  authenticated: boolean;
  onAuthChanged: () => void;
}

export default function ConnectProviderCard({ cli, displayName, authenticated, onAuthChanged }: Props) {
  const [status, setStatus] = useState<"idle" | "connecting" | "success" | "failed">(
    authenticated ? "success" : "idle",
  );
  const [progressMsg, setProgressMsg] = useState("");
  const [failReason, setFailReason] = useState("");

  // Sync external auth state
  useEffect(() => {
    if (authenticated && status === "idle") setStatus("success");
  }, [authenticated, status]);

  const handleConnect = useCallback(async () => {
    setStatus("connecting");
    setProgressMsg("Starting login...");
    setFailReason("");

    // Listen for events from the Tauri backend
    const unlisteners: UnlistenFn[] = [];

    const ulProgress = await listen("cli-auth-progress", (ev: any) => {
      const p = ev.payload;
      if (p.cli === cli) setProgressMsg(p.message);
    });
    unlisteners.push(ulProgress);

    const ulSuccess = await listen("cli-auth-success", (ev: any) => {
      const p = ev.payload;
      if (p.cli === cli) {
        setStatus("success");
        setProgressMsg("");
        onAuthChanged();
      }
    });
    unlisteners.push(ulSuccess);

    const ulFailed = await listen("cli-auth-failed", (ev: any) => {
      const p = ev.payload;
      if (p.cli === cli) {
        setStatus("failed");
        setFailReason(p.reason || "Unknown error");
        setProgressMsg("");
      }
    });
    unlisteners.push(ulFailed);

    try {
      await builderAuthenticateCli(cli);
    } catch (e: any) {
      setStatus("failed");
      setFailReason(e?.message ?? String(e));
      setProgressMsg("");
    }

    // Cleanup listeners after a delay (auth polling continues in background)
    setTimeout(() => {
      unlisteners.forEach((ul) => ul());
    }, 130000);
  }, [cli, onAuthChanged]);

  const icon = cli === "claude" ? "\u{1F916}" : "\u{26A1}";

  return (
    <div
      style={{
        background: C.surfaceAlt,
        border: `1px solid ${status === "success" ? "rgba(63,185,80,0.3)" : C.border}`,
        borderRadius: 6,
        padding: "8px 12px",
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      {/* Icon + Name */}
      <span style={{ fontSize: 16, flexShrink: 0 }}>{icon}</span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 11, fontWeight: 600, color: C.text, fontFamily: C.mono }}>
          {displayName}
        </div>
        {status === "success" && (
          <div style={{ fontSize: 10, color: C.ok, fontFamily: C.mono, marginTop: 1 }}>
            {"\u2713"} Connected
          </div>
        )}
        {status === "idle" && (
          <div style={{ fontSize: 10, color: C.muted, fontFamily: C.mono, marginTop: 1 }}>
            Not connected
          </div>
        )}
        {status === "connecting" && (
          <div style={{ fontSize: 10, color: C.accent, fontFamily: C.mono, marginTop: 1 }}>
            {progressMsg || "Connecting..."}
          </div>
        )}
        {status === "failed" && (
          <div style={{ fontSize: 10, color: C.err, fontFamily: C.mono, marginTop: 1 }}>
            Failed: {failReason}.{" "}
            Try <code style={{ fontSize: 9 }}>{cli === "claude" ? "claude auth login" : "codex login"}</code> in terminal.
          </div>
        )}
      </div>

      {/* Action */}
      {status === "success" && (
        <span
          style={{
            fontSize: 10,
            color: C.ok,
            fontWeight: 700,
            fontFamily: C.mono,
            flexShrink: 0,
          }}
        >
          {"\u2713"}
        </span>
      )}
      {status === "idle" && (
        <button
          onClick={handleConnect}
          style={{
            background: C.accent,
            color: C.bg,
            border: "none",
            borderRadius: 4,
            padding: "4px 10px",
            fontSize: 10,
            fontWeight: 700,
            fontFamily: C.mono,
            cursor: "pointer",
            flexShrink: 0,
            letterSpacing: 0.3,
          }}
        >
          Connect
        </button>
      )}
      {status === "connecting" && (
        <div
          style={{
            width: 14,
            height: 14,
            borderRadius: "50%",
            border: `2px solid ${C.border}`,
            borderTopColor: C.accent,
            animation: "nbspin 0.8s linear infinite",
            flexShrink: 0,
          }}
        />
      )}
      {status === "failed" && (
        <button
          onClick={handleConnect}
          style={{
            background: "transparent",
            color: C.muted,
            border: `1px solid ${C.border}`,
            borderRadius: 4,
            padding: "4px 8px",
            fontSize: 10,
            fontFamily: C.mono,
            cursor: "pointer",
            flexShrink: 0,
          }}
        >
          Retry
        </button>
      )}
    </div>
  );
}
