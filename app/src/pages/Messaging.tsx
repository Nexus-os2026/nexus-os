import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getConfig,
  getMessagingStatus,
  listAgents,
  messagingConnectPlatform,
  messagingSend,
  messagingPollMessages,
  saveConfig,
  setDefaultAgent,
} from "../api/backend";
import type { AgentSummary, NexusConfig } from "../types";
import { normalizeConfig } from "../utils/config";

type PlatformCard = {
  key: "telegram" | "discord" | "slack" | "whatsapp";
  label: string;
  tokenPath: (config: NexusConfig) => string;
  assign: (config: NexusConfig, value: string) => void;
};

type PlatformStatus = {
  name?: string;
  connected?: boolean;
  message_count?: number;
};

const PLATFORMS: PlatformCard[] = [
  {
    key: "telegram",
    label: "Telegram",
    tokenPath: (config) => config.messaging.telegram_bot_token,
    assign: (config, value) => {
      config.messaging.telegram_bot_token = value;
    },
  },
  {
    key: "discord",
    label: "Discord",
    tokenPath: (config) => config.messaging.discord_bot_token,
    assign: (config, value) => {
      config.messaging.discord_bot_token = value;
    },
  },
  {
    key: "slack",
    label: "Slack",
    tokenPath: (config) => config.messaging.slack_bot_token,
    assign: (config, value) => {
      config.messaging.slack_bot_token = value;
    },
  },
  {
    key: "whatsapp",
    label: "WhatsApp",
    tokenPath: (config) => config.messaging.whatsapp_api_token,
    assign: (config, value) => {
      config.messaging.whatsapp_api_token = value;
    },
  },
];

function statusColor(connected: boolean, configured: boolean): string {
  if (connected) {
    return "bg-emerald-400";
  }
  if (configured) {
    return "bg-amber-300";
  }
  return "bg-rose-400";
}

export default function Messaging(): JSX.Element {
  const [config, setConfig] = useState<NexusConfig | null>(null);
  const [tokens, setTokens] = useState<Record<string, string>>({});
  const [statuses, setStatuses] = useState<PlatformStatus[]>([]);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [defaultAgentId, setDefaultAgentId] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [connectingPlatform, setConnectingPlatform] = useState<string | null>(null);
  const [connectionResults, setConnectionResults] = useState<Record<string, {connected: boolean; name?: string}>>({});
  const [replyText, setReplyText] = useState("");
  const [replyChannel, setReplyChannel] = useState("");
  const [replyPlatform, setReplyPlatform] = useState("");
  const [messages, setMessages] = useState<{platform: string; channel: string; from: string; text: string; time: string}[]>([]);
  const [sendingReply, setSendingReply] = useState(false);

  /* ─── Real-time message listener via Tauri events ─── */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlisten = await eventMod.listen<{ platform: string; channel: string; from: string; text: string }>("slack-message", (event) => {
          const msg = event.payload;
          setMessages(prev => [...prev, { platform: msg.platform || "slack", channel: msg.channel || "general", from: msg.from || "unknown", text: msg.text || "", time: new Date().toLocaleTimeString() }]);
        });
      } catch { /* not in desktop runtime */ }
    })();
    return () => { unlisten?.(); };
  }, []);

  /* ─── Poll for new messages every 15s for connected platforms ─── */
  const lastPollId = useRef("0");
  useEffect(() => {
    const poll = async () => {
      // Poll each connected platform
      for (const [platform, result] of Object.entries(connectionResults)) {
        if (!result.connected) continue;
        try {
          const raw = await messagingPollMessages(platform, "general", lastPollId.current);
          const parsed = JSON.parse(raw);
          const msgs = parsed.messages ?? parsed.result ?? (Array.isArray(parsed) ? parsed : []);
          if (Array.isArray(msgs) && msgs.length > 0) {
            setMessages(prev => [...prev, ...msgs.map((m: any) => ({
              platform,
              channel: m.channel || "general",
              from: m.from || m.user || m.username || "unknown",
              text: m.text || m.content || "",
              time: new Date().toLocaleTimeString(),
            }))]);
          }
        } catch { /* not connected or no messages */ }
      }
    };
    const iv = setInterval(poll, 15_000);
    return () => clearInterval(iv);
  }, [connectionResults]);

  const load = useCallback(async () => {
    try {
      const [configRow, statusRows, agentRows] = await Promise.all([
        getConfig(),
        getMessagingStatus<PlatformStatus>(),
        listAgents(),
      ]);
      const normalizedConfig = normalizeConfig(configRow);
      setConfig(normalizedConfig);
      setStatuses(Array.isArray(statusRows) ? statusRows : []);
      setAgents(Array.isArray(agentRows) ? agentRows : []);
      setDefaultAgentId((current) => current || agentRows[0]?.id || "");
      setTokens(
        Object.fromEntries(
          PLATFORMS.map((platform) => [platform.key, platform.tokenPath(normalizedConfig)]),
        ),
      );
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  useEffect(() => {
    void load().catch((error) => {
      setMessage(error instanceof Error ? error.message : String(error));
    });
  }, [load]);

  const statusByName = useMemo(
    () =>
      new Map(
        statuses.map((status) => [status.name?.toLowerCase() ?? "", status]),
      ),
    [statuses],
  );

  const connectPlatform = useCallback(async (platform: PlatformCard) => {
    if (!config) {
      return;
    }
    const next = JSON.parse(JSON.stringify(config)) as NexusConfig;
    platform.assign(next, tokens[platform.key] ?? "");
    try {
      await saveConfig(next);
      setConfig(next);
      await load();
      setMessage(`${platform.label} settings saved.`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [config, load, tokens]);

  const handleConnect = useCallback(async (platform: string, token: string) => {
    if (!token.trim()) return;
    setConnectingPlatform(platform);
    try {
      const result = await messagingConnectPlatform(platform, token);
      const data = JSON.parse(result);
      setConnectionResults(prev => ({ ...prev, [platform]: { connected: true, name: data.bot_name || data.team || platform } }));
    } catch (e: any) {
      setConnectionResults(prev => ({ ...prev, [platform]: { connected: false } }));
      alert(`Connection failed: ${e?.message || e}`);
    } finally {
      setConnectingPlatform(null);
    }
  }, []);

  const handleSendReply = useCallback(async () => {
    if (!replyPlatform || !replyChannel || !replyText.trim()) return;
    setSendingReply(true);
    try {
      await messagingSend(replyPlatform, replyChannel, replyText);
      setMessages(prev => [...prev, { platform: replyPlatform, channel: replyChannel, from: "You", text: replyText, time: new Date().toLocaleTimeString() }]);
      setReplyText("");
    } catch (e: any) {
      alert(`Send failed: ${e?.message || e}`);
    } finally {
      setSendingReply(false);
    }
  }, [replyPlatform, replyChannel, replyText]);

  const saveDefaultAgent = useCallback(async (agentId: string) => {
    setDefaultAgentId(agentId);
    try {
      await setDefaultAgent("messaging-default", agentId);
      setMessage("Default messaging agent updated.");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  return (
    <section className="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
      <header className="nexus-panel rounded-3xl p-6">
        <p className="text-xs uppercase tracking-[0.24em] text-cyan-300/70">Messaging</p>
        <h2 className="nexus-display mt-2 text-3xl text-cyan-50">Messaging Configuration</h2>
        <p className="mt-2 text-sm text-cyan-100/65">
          Backed by `get_messaging_status`, persisted token settings, and a real `set_default_agent` route for inbound handling.
        </p>
      </header>

      {message ? (
        <div className="rounded-2xl border border-cyan-500/20 bg-slate-950/60 p-4 text-sm text-cyan-100/70">
          {message}
        </div>
      ) : null}

      <section className="nexus-panel rounded-2xl p-5">
        <label className="grid gap-2 text-sm text-cyan-100/70 md:max-w-sm">
          Default Agent For Incoming Messages
          <select
            value={defaultAgentId}
            onChange={(event) => void saveDefaultAgent(event.target.value)}
            className="rounded-xl border border-cyan-500/20 bg-slate-950/60 px-3 py-2 text-cyan-50"
          >
            <option value="">Select agent</option>
            {agents.map((agent) => (
              <option key={agent.id} value={agent.id}>
                {agent.name}
              </option>
            ))}
          </select>
        </label>
      </section>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {PLATFORMS.map((platform) => {
          const status = statusByName.get(platform.label.toLowerCase());
          const token = tokens[platform.key] ?? "";
          const connected = Boolean(status?.connected);
          const configured = token.trim().length > 0;
          return (
            <article key={platform.key} className="nexus-panel rounded-2xl p-5">
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-lg text-cyan-50">{platform.label}</h3>
                <span className={`inline-flex h-3 w-3 rounded-full ${statusColor(connected, configured)}`} />
              </div>
              <p className="mt-2 text-sm text-cyan-100/60">
                {connected
                  ? "Connected"
                  : configured
                    ? "Configured but not tested"
                    : "Disconnected"}
              </p>
              <p className="mt-1 text-xs text-cyan-100/45">
                Messages observed: {status?.message_count ?? 0}
              </p>
              <input
                value={token}
                onChange={(event) =>
                  setTokens((current) => ({ ...current, [platform.key]: event.target.value }))
                }
                placeholder={`${platform.label} token`}
                className="mt-4 w-full rounded-xl border border-cyan-500/20 bg-slate-950/60 px-3 py-2 text-sm text-cyan-50"
              />
              <div className="mt-4 flex flex-wrap gap-3">
                <button
                  type="button"
                  onClick={() => void connectPlatform(platform)}
                  className="rounded-full border border-cyan-400/30 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100"
                >
                  Connect
                </button>
                <button
                  type="button"
                  onClick={() => void load()}
                  className="rounded-full border border-cyan-400/20 bg-slate-950/60 px-4 py-2 text-sm text-cyan-100/75"
                >
                  Test
                </button>
                <button
                  type="button"
                  className="cursor-pointer rounded-full px-4 py-2 text-xs"
                  onClick={() => void handleConnect(platform.key, tokens[platform.key] ?? "")}
                  disabled={connectingPlatform === platform.key}
                  style={{
                    background: connectionResults[platform.key]?.connected ? "rgba(34,197,94,0.2)" : "rgba(129,140,248,0.2)",
                    border: `1px solid ${connectionResults[platform.key]?.connected ? "rgba(34,197,94,0.3)" : "rgba(129,140,248,0.3)"}`,
                    color: connectionResults[platform.key]?.connected ? "#22c55e" : "#818cf8",
                    fontFamily: "inherit",
                    cursor: "pointer",
                  }}
                >
                  {connectionResults[platform.key]?.connected ? "\u2713 Connected" : connectingPlatform === platform.key ? "Testing..." : "Test & Connect"}
                </button>
              </div>
            </article>
          );
        })}
      </div>

      {/* ── Live Messages ── */}
      {Object.values(connectionResults).some(r => r.connected) && (
        <div style={{ marginTop: 24, background: "var(--bg-secondary, #1e293b)", border: "1px solid var(--border, #334155)", borderRadius: 8, padding: 16 }}>
          <h3 style={{ margin: "0 0 12px", fontSize: "1rem", fontWeight: 600 }}>
            Live Messages
            <span style={{ fontSize: "0.7rem", opacity: 0.5, marginLeft: 8 }}>
              {Object.entries(connectionResults).filter(([,r]) => r.connected).map(([p, r]) => `${p}: ${r.name}`).join(" \u00b7 ")}
            </span>
          </h3>
          <div style={{ maxHeight: 300, overflowY: "auto", marginBottom: 12 }}>
            {messages.length === 0 ? (
              <div style={{ opacity: 0.4, padding: 16, textAlign: "center", fontSize: "0.8rem" }}>No messages yet. Messages will appear here when received.</div>
            ) : messages.map((msg, i) => (
              <div key={i} style={{ padding: "6px 8px", borderBottom: "1px solid rgba(255,255,255,0.05)", fontSize: "0.8rem" }}>
                <span style={{ opacity: 0.4, fontSize: "0.7rem" }}>[{msg.platform}] {msg.time}</span>
                <span style={{ fontWeight: 600, marginLeft: 6 }}>{msg.from}:</span>
                <span style={{ marginLeft: 4 }}>{msg.text}</span>
              </div>
            ))}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <select value={replyPlatform} onChange={e => setReplyPlatform(e.target.value)} style={{ background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 4, color: "inherit", padding: "4px 8px", fontSize: "0.8rem", fontFamily: "inherit" }}>
              <option value="">Platform</option>
              {Object.entries(connectionResults).filter(([,r]) => r.connected).map(([p]) => (
                <option key={p} value={p}>{p}</option>
              ))}
            </select>
            <input value={replyChannel} onChange={e => setReplyChannel(e.target.value)} placeholder="Channel / Chat ID" style={{ flex: "0 0 140px", background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 4, color: "inherit", padding: "4px 8px", fontSize: "0.8rem", fontFamily: "inherit" }} />
            <input value={replyText} onChange={e => setReplyText(e.target.value)} placeholder="Type a message..." style={{ flex: 1, background: "var(--bg-primary, #0f172a)", border: "1px solid var(--border, #334155)", borderRadius: 4, color: "inherit", padding: "4px 8px", fontSize: "0.8rem", fontFamily: "inherit" }} onKeyDown={e => e.key === 'Enter' && handleSendReply()} />
            <button className="cursor-pointer" onClick={handleSendReply} disabled={sendingReply || !replyText.trim() || !replyPlatform || !replyChannel} style={{ padding: "4px 12px", background: "rgba(129,140,248,0.2)", border: "1px solid rgba(129,140,248,0.3)", borderRadius: 4, color: "#818cf8", fontSize: "0.8rem", fontFamily: "inherit", cursor: "pointer" }}>{sendingReply ? "..." : "Send"}</button>
          </div>
        </div>
      )}
    </section>
  );
}
