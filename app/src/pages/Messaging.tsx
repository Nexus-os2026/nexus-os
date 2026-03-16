import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getConfig,
  getMessagingStatus,
  listAgents,
  saveConfig,
  setDefaultAgent,
} from "../api/backend";
import type { AgentSummary, NexusConfig } from "../types";

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

  const load = useCallback(async () => {
    try {
      const [configRow, statusRows, agentRows] = await Promise.all([
        getConfig(),
        getMessagingStatus<PlatformStatus>(),
        listAgents(),
      ]);
      setConfig(configRow);
      setStatuses(statusRows);
      setAgents(agentRows);
      setDefaultAgentId((current) => current || agentRows[0]?.id || "");
      setTokens(
        Object.fromEntries(
          PLATFORMS.map((platform) => [platform.key, platform.tokenPath(configRow)]),
        ),
      );
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  useEffect(() => {
    void load();
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
              <div className="mt-4 flex gap-3">
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
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
