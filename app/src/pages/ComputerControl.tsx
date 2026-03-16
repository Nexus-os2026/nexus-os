import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  captureScreen,
  computerControlGetHistory,
  computerControlStatus,
  computerControlToggle,
  getInputControlStatus,
  startComputerAction,
  stopComputerAction,
} from "../api/backend";

type ActionRecord = {
  timestamp?: number;
  success?: boolean;
  error?: string | null;
  action?: Record<string, unknown>;
};

export default function ComputerControl(): JSX.Element {
  const [enabled, setEnabled] = useState(false);
  const [status, setStatus] = useState<Record<string, unknown> | null>(null);
  const [inputStatus, setInputStatus] = useState<Record<string, unknown> | null>(null);
  const [history, setHistory] = useState<ActionRecord[]>([]);
  const [screenPath, setScreenPath] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("Open the target app and summarize the current state.");
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [statusRow, historyRows, inputRow] = await Promise.all([
        computerControlStatus(),
        computerControlGetHistory(),
        getInputControlStatus(),
      ]);
      setStatus(statusRow);
      setEnabled(Boolean(statusRow.enabled));
      setHistory(historyRows);
      setInputStatus(inputRow as unknown as Record<string, unknown>);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  const capturePreview = useCallback(async () => {
    try {
      const path = await captureScreen();
      setScreenPath(path);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!enabled) {
      return;
    }
    void capturePreview();
    const interval = window.setInterval(() => {
      void capturePreview();
      void refresh();
    }, 5000);
    return () => window.clearInterval(interval);
  }, [capturePreview, enabled, refresh]);

  const previewUrl = useMemo(
    () => (screenPath ? convertFileSrc(screenPath) : ""),
    [screenPath],
  );

  const toggleControl = useCallback(async () => {
    try {
      const next = !enabled;
      await computerControlToggle(next);
      setEnabled(next);
      if (next) {
        await capturePreview();
      } else {
        setScreenPath(null);
      }
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [capturePreview, enabled, refresh]);

  const killSwitch = useCallback(async () => {
    try {
      if (sessionId) {
        await stopComputerAction(sessionId);
      }
      await computerControlToggle(false);
      setEnabled(false);
      setSessionId(null);
      await refresh();
      setMessage("Computer control halted.");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [refresh, sessionId]);

  const startAction = useCallback(async () => {
    try {
      const nextSessionId = await startComputerAction(prompt, 12);
      setSessionId(nextSessionId);
      setMessage(`Started computer action ${nextSessionId}.`);
      await refresh();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [prompt, refresh]);

  return (
    <section className="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
      <header className="nexus-panel rounded-3xl p-6">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.24em] text-cyan-300/70">Computer Control</p>
            <h2 className="nexus-display mt-2 text-3xl text-cyan-50">
              Computer Control: {enabled ? "ON" : "OFF"}
            </h2>
            <p className="mt-2 text-sm text-cyan-100/65">
              Real desktop control state from `computer_control_status`, live captures via `capture_screen`, and action history from `computer_control_get_history`.
            </p>
          </div>
          <div className="flex gap-3">
            <button
              type="button"
              onClick={() => void toggleControl()}
              className="rounded-full border border-cyan-400/30 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100"
            >
              {enabled ? "Disable" : "Enable"}
            </button>
            <button
              type="button"
              onClick={() => void killSwitch()}
              className="rounded-full border border-rose-400/30 bg-rose-500/10 px-4 py-2 text-sm text-rose-100"
            >
              Kill Switch
            </button>
          </div>
        </div>
      </header>

      {message ? (
        <div className="rounded-2xl border border-cyan-500/20 bg-slate-950/60 p-4 text-sm text-cyan-100/70">
          {message}
        </div>
      ) : null}

      <div className="grid gap-4 xl:grid-cols-[1.05fr_0.95fr]">
        <section className="nexus-panel rounded-2xl p-5">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h3 className="text-lg text-cyan-50">Live Screen Preview</h3>
              <p className="text-sm text-cyan-100/60">Updates every 5 seconds while enabled.</p>
            </div>
          </div>
          <div className="mt-4 overflow-hidden rounded-2xl border border-cyan-500/15 bg-slate-950/60 p-3">
            {previewUrl ? (
              <img src={previewUrl} alt="Current desktop capture" className="w-full rounded-xl object-contain" />
            ) : (
              <div className="flex min-h-[320px] items-center justify-center text-sm text-cyan-100/55">
                Enable computer control to start capturing the screen.
              </div>
            )}
          </div>
          <div className="mt-4 grid gap-3 md:grid-cols-2">
            <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
              <p className="text-xs uppercase tracking-[0.18em] text-cyan-300/50">Status</p>
              <pre className="mt-3 whitespace-pre-wrap text-xs text-cyan-100/65">
                {JSON.stringify(status, null, 2)}
              </pre>
            </div>
            <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
              <p className="text-xs uppercase tracking-[0.18em] text-cyan-300/50">Input Limits</p>
              <pre className="mt-3 whitespace-pre-wrap text-xs text-cyan-100/65">
                {JSON.stringify(inputStatus, null, 2)}
              </pre>
            </div>
          </div>
        </section>

        <section className="nexus-panel rounded-2xl p-5">
          <h3 className="text-lg text-cyan-50">Action Log</h3>
          <div className="mt-4 max-h-[360px] space-y-3 overflow-auto pr-1">
            {history.length === 0 ? (
              <p className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4 text-sm text-cyan-100/60">
                No computer actions recorded yet.
              </p>
            ) : (
              history
                .slice()
                .reverse()
                .map((record, index) => (
                  <article key={`${record.timestamp ?? 0}-${index}`} className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
                    <div className="flex items-center justify-between gap-3">
                      <strong className="text-sm text-cyan-50">
                        {record.success ? "Action succeeded" : "Action failed"}
                      </strong>
                      <span className="text-xs text-cyan-100/50">
                        {record.timestamp ? new Date(record.timestamp * 1000).toLocaleString() : "Unknown time"}
                      </span>
                    </div>
                    <pre className="mt-3 whitespace-pre-wrap text-xs text-cyan-100/65">
                      {JSON.stringify(record.action, null, 2)}
                    </pre>
                    {record.error ? <p className="mt-2 text-xs text-rose-200">{record.error}</p> : null}
                  </article>
                ))
            )}
          </div>

          <div className="mt-4 rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
            <h3 className="text-lg text-cyan-50">Manual Action Input</h3>
            <textarea
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
              rows={4}
              className="mt-3 w-full rounded-2xl border border-cyan-500/20 bg-slate-950/70 px-3 py-3 text-sm text-cyan-50"
            />
            <div className="mt-3 flex flex-wrap gap-3">
              <button
                type="button"
                onClick={() => void startAction()}
                className="rounded-full border border-emerald-400/30 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-100"
              >
                Start Action
              </button>
              {sessionId ? (
                <button
                  type="button"
                  onClick={() => void stopComputerAction(sessionId)}
                  className="rounded-full border border-amber-400/30 bg-amber-500/10 px-4 py-2 text-sm text-amber-100"
                >
                  Stop Session
                </button>
              ) : null}
            </div>
          </div>
        </section>
      </div>
    </section>
  );
}
