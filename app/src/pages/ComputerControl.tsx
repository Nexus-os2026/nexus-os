import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  captureScreen,
  computerControlGetHistory,
  computerControlStatus,
  computerControlToggle,
  getInputControlStatus,
  omniscienceDisable,
  omniscienceEnable,
  omniscienceExecuteAction,
  omniscienceGetAppContext,
  omniscienceGetPredictions,
  omniscienceGetScreenContext,
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

  // Omniscience state
  const [omniEnabled, setOmniEnabled] = useState(false);
  const [omniLoading, setOmniLoading] = useState(false);
  const [omniScreenCtx, setOmniScreenCtx] = useState<string | null>(null);
  const [omniPredictions, setOmniPredictions] = useState<string | null>(null);
  const [omniAppName, setOmniAppName] = useState("");
  const [omniAppCtx, setOmniAppCtx] = useState<string | null>(null);
  const [omniActionInput, setOmniActionInput] = useState('{"type":"click","x":100,"y":200}');
  const [omniActionResult, setOmniActionResult] = useState<string | null>(null);

  const [mode, setMode] = useState<'demo' | 'live'>('live');
  const [demoStep, setDemoStep] = useState(0);
  const [demoRunning, setDemoRunning] = useState(false);

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

  // --- Omniscience callbacks ---

  const toggleOmniscience = useCallback(async () => {
    setOmniLoading(true);
    try {
      if (omniEnabled) {
        await omniscienceDisable();
        setOmniEnabled(false);
        setOmniScreenCtx(null);
        setOmniPredictions(null);
        setMessage("Omniscience disabled.");
      } else {
        await omniscienceEnable(5000);
        setOmniEnabled(true);
        setMessage("Omniscience enabled.");
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setOmniLoading(false);
    }
  }, [omniEnabled]);

  const fetchOmniScreenCtx = useCallback(async () => {
    try {
      const ctx = await omniscienceGetScreenContext();
      setOmniScreenCtx(typeof ctx === "string" ? ctx : JSON.stringify(ctx, null, 2));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  const fetchOmniPredictions = useCallback(async () => {
    try {
      const preds = await omniscienceGetPredictions();
      setOmniPredictions(typeof preds === "string" ? preds : JSON.stringify(preds, null, 2));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, []);

  const fetchOmniAppCtx = useCallback(async () => {
    if (!omniAppName.trim()) {
      setMessage("Enter an app name first.");
      return;
    }
    try {
      const ctx = await omniscienceGetAppContext(omniAppName.trim());
      setOmniAppCtx(typeof ctx === "string" ? ctx : JSON.stringify(ctx, null, 2));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [omniAppName]);

  const executeOmniAction = useCallback(async () => {
    try {
      const parsed: unknown = JSON.parse(omniActionInput);
      const result = await omniscienceExecuteAction(parsed);
      setOmniActionResult(typeof result === "string" ? result : JSON.stringify(result, null, 2));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [omniActionInput]);

  const DEMO_ACTIONS = [
    { action: "observe", description: "Agent scans the screen for context", detail: "Analyzing visible windows, text content, and UI elements..." },
    { action: "identify", description: "Found: Text editor with untitled document", detail: "Detected application: Code Editor, state: empty file, cursor at line 1" },
    { action: "plan", description: "Planning action sequence", detail: "Goal: Create hello world program. Steps: 1) Click editor area, 2) Type code, 3) Save file" },
    { action: "click", description: "Agent would click: Editor text area at (450, 320)", detail: "Mouse move to (450, 320) → Left click → Focus confirmed" },
    { action: "type", description: "Agent would type: fn main() { println!(\"Hello, world!\"); }", detail: "Keystroke sequence: 45 characters, typing speed: 60 WPM simulated" },
    { action: "shortcut", description: "Agent would press: Ctrl+S to save", detail: "Key combination: Ctrl+S → File save dialog detected → Confirmed save" },
    { action: "verify", description: "Agent verifies: File saved successfully", detail: "Screen capture shows: saved indicator, file name visible, no error dialogs" },
    { action: "complete", description: "Task complete: Hello world program created", detail: "Audit trail recorded. Fuel used: 12 units. All actions governed." },
  ];

  const runDemo = useCallback(() => {
    setDemoRunning(true);
    setDemoStep(0);
    let step = 0;
    const interval = setInterval(() => {
      step++;
      if (step >= DEMO_ACTIONS.length) {
        clearInterval(interval);
        setDemoRunning(false);
      } else {
        setDemoStep(step);
      }
    }, 2000);
    return () => clearInterval(interval);
  }, []);

  // Auto-refresh omniscience data while enabled
  useEffect(() => {
    if (!omniEnabled) return;
    void fetchOmniScreenCtx();
    void fetchOmniPredictions();
    const interval = window.setInterval(() => {
      void fetchOmniScreenCtx();
      void fetchOmniPredictions();
    }, 6000);
    return () => window.clearInterval(interval);
  }, [omniEnabled, fetchOmniScreenCtx, fetchOmniPredictions]);

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

      {/* ── Mode Selector ── */}
      <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16 }}>
        <button className="cursor-pointer" onClick={() => setMode('demo')} style={{ padding: "6px 16px", background: mode === 'demo' ? "rgba(129,140,248,0.2)" : "transparent", border: `1px solid ${mode === 'demo' ? "rgba(129,140,248,0.4)" : "#334155"}`, borderRadius: 6, color: "#e2e8f0", fontSize: "0.85rem", fontFamily: "inherit", cursor: "pointer", fontWeight: mode === 'demo' ? 600 : 400 }}>Preview Mode</button>
        <button className="cursor-pointer" onClick={() => setMode('live')} style={{ padding: "6px 16px", background: mode === 'live' ? "rgba(239,68,68,0.2)" : "transparent", border: `1px solid ${mode === 'live' ? "rgba(239,68,68,0.4)" : "#334155"}`, borderRadius: 6, color: "#e2e8f0", fontSize: "0.85rem", fontFamily: "inherit", cursor: "pointer", fontWeight: mode === 'live' ? 600 : 400 }}>Live Mode</button>
        <span style={{ fontSize: "0.7rem", opacity: 0.5 }}>{mode === 'demo' ? "Safe preview — shows governed action sequence without executing" : "Agent controls your computer"}</span>
      </div>

      {/* ── Demo Mode Panel ── */}
      {mode === 'demo' && (
        <div style={{ background: "#1e293b", border: "1px solid #334155", borderRadius: 8, padding: 16, marginBottom: 16 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
            <h3 style={{ margin: 0, fontSize: "0.95rem" }}>Preview: Watch What the Agent Would Do</h3>
            <button className="cursor-pointer" onClick={runDemo} disabled={demoRunning} style={{ padding: "6px 14px", background: "rgba(129,140,248,0.2)", border: "1px solid rgba(129,140,248,0.3)", borderRadius: 6, color: "#818cf8", fontSize: "0.8rem", fontFamily: "inherit", cursor: "pointer" }}>{demoRunning ? "Running..." : "Run Preview"}</button>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {DEMO_ACTIONS.slice(0, demoStep + 1).map((action, i) => (
              <div key={i} style={{ padding: "8px 12px", background: i === demoStep && demoRunning ? "rgba(129,140,248,0.1)" : "rgba(255,255,255,0.02)", borderRadius: 6, border: i === demoStep && demoRunning ? "1px solid rgba(129,140,248,0.3)" : "1px solid transparent", transition: "all 0.3s" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: "0.8rem" }}>
                  <span style={{ background: "rgba(129,140,248,0.2)", padding: "2px 8px", borderRadius: 4, fontSize: "0.65rem", fontWeight: 600, color: "#818cf8", textTransform: "uppercase" }}>{action.action}</span>
                  <span style={{ fontWeight: 500 }}>{action.description}</span>
                </div>
                <div style={{ fontSize: "0.7rem", opacity: 0.5, marginTop: 4, paddingLeft: 8 }}>{action.detail}</div>
              </div>
            ))}
          </div>
          <div style={{ marginTop: 12, padding: "8px 12px", background: "rgba(34,197,94,0.05)", borderRadius: 6, fontSize: "0.75rem", opacity: 0.7 }}>
            Preview mode shows what agents WOULD do without taking real actions. All actions are governed by fuel limits, HITL approval, and audit trails. No system changes are made during preview.
          </div>
        </div>
      )}

      {mode === 'live' && !enabled && (
        <div style={{ background: "rgba(239,68,68,0.05)", border: "1px solid rgba(239,68,68,0.2)", borderRadius: 8, padding: 20, marginBottom: 16, textAlign: "center" }}>
          <h3 style={{ margin: "0 0 8px", fontSize: "0.95rem", display: "flex", alignItems: "center", justifyContent: "center", gap: 8 }}><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg> Live Mode</h3>
          <p style={{ margin: "0 0 12px", fontSize: "0.8rem", opacity: 0.7, lineHeight: 1.5 }}>
            Live mode lets agents control your mouse and keyboard. Everything is governed — fuel limits, HITL approval on dangerous actions, full audit trail.
            Press ESC 3 times to force-stop.
          </p>
          <button className="cursor-pointer" onClick={() => computerControlToggle(true).then(() => setEnabled(true))} style={{ padding: "8px 20px", background: "rgba(239,68,68,0.2)", border: "1px solid rgba(239,68,68,0.4)", borderRadius: 6, color: "#ef4444", fontSize: "0.85rem", fontFamily: "inherit", cursor: "pointer", fontWeight: 600 }}>I Understand — Enable Live Control</button>
        </div>
      )}

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
                {status ? JSON.stringify(status, null, 2) : "Not connected — desktop runtime required."}
              </pre>
            </div>
            <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
              <p className="text-xs uppercase tracking-[0.18em] text-cyan-300/50">Input Limits</p>
              <pre className="mt-3 whitespace-pre-wrap text-xs text-cyan-100/65">
                {inputStatus ? JSON.stringify(inputStatus, null, 2) : "Not connected — desktop runtime required."}
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

      {/* ── Omniscience ──────────────────────────────────────────── */}
      <section className="nexus-panel rounded-3xl p-6">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.24em] text-violet-300/70">Omniscience</p>
            <h3 className="nexus-display mt-2 text-2xl text-cyan-50">
              Omniscience: {omniEnabled ? "ON" : "OFF"}
            </h3>
            <p className="mt-2 text-sm text-cyan-100/65">
              Screen-aware context engine. Captures screen context, predicts user intent, and executes actions.
            </p>
          </div>
          <button
            type="button"
            disabled={omniLoading}
            onClick={() => void toggleOmniscience()}
            className={`rounded-full border px-4 py-2 text-sm ${
              omniEnabled
                ? "border-rose-400/30 bg-rose-500/10 text-rose-100"
                : "border-violet-400/30 bg-violet-500/10 text-violet-100"
            }`}
          >
            {omniLoading ? "..." : omniEnabled ? "Disable" : "Enable"}
          </button>
        </div>

        <div className="mt-5 grid gap-4 xl:grid-cols-2">
          {/* Screen Context */}
          <div className="rounded-2xl border border-violet-500/15 bg-slate-950/50 p-4">
            <div className="flex items-center justify-between gap-3">
              <p className="text-xs uppercase tracking-[0.18em] text-violet-300/50">Screen Context</p>
              <button
                type="button"
                onClick={() => void fetchOmniScreenCtx()}
                className="rounded-full border border-violet-400/20 bg-violet-500/10 px-3 py-1 text-xs text-violet-200"
              >
                Refresh
              </button>
            </div>
            <pre className="mt-3 max-h-[200px] overflow-auto whitespace-pre-wrap text-xs text-cyan-100/65">
              {omniScreenCtx ?? "No screen context captured yet."}
            </pre>
          </div>

          {/* Intent Predictions */}
          <div className="rounded-2xl border border-violet-500/15 bg-slate-950/50 p-4">
            <div className="flex items-center justify-between gap-3">
              <p className="text-xs uppercase tracking-[0.18em] text-violet-300/50">Intent Predictions</p>
              <button
                type="button"
                onClick={() => void fetchOmniPredictions()}
                className="rounded-full border border-violet-400/20 bg-violet-500/10 px-3 py-1 text-xs text-violet-200"
              >
                Refresh
              </button>
            </div>
            <pre className="mt-3 max-h-[200px] overflow-auto whitespace-pre-wrap text-xs text-cyan-100/65">
              {omniPredictions ?? "No predictions available."}
            </pre>
          </div>

          {/* App Context */}
          <div className="rounded-2xl border border-violet-500/15 bg-slate-950/50 p-4">
            <p className="text-xs uppercase tracking-[0.18em] text-violet-300/50">App Context</p>
            <div className="mt-3 flex gap-2">
              <input
                type="text"
                value={omniAppName}
                onChange={(e) => setOmniAppName(e.target.value)}
                placeholder="App name (e.g. Firefox)"
                className="flex-1 rounded-xl border border-violet-500/20 bg-slate-950/70 px-3 py-2 text-sm text-cyan-50 placeholder:text-cyan-100/30"
              />
              <button
                type="button"
                onClick={() => void fetchOmniAppCtx()}
                className="rounded-full border border-violet-400/20 bg-violet-500/10 px-3 py-2 text-xs text-violet-200"
              >
                Get Context
              </button>
            </div>
            <pre className="mt-3 max-h-[200px] overflow-auto whitespace-pre-wrap text-xs text-cyan-100/65">
              {omniAppCtx ?? "Enter an app name and click Get Context."}
            </pre>
          </div>

          {/* Execute Action */}
          <div className="rounded-2xl border border-violet-500/15 bg-slate-950/50 p-4">
            <p className="text-xs uppercase tracking-[0.18em] text-violet-300/50">Execute Action</p>
            <textarea
              value={omniActionInput}
              onChange={(e) => setOmniActionInput(e.target.value)}
              rows={3}
              className="mt-3 w-full rounded-xl border border-violet-500/20 bg-slate-950/70 px-3 py-2 text-sm text-cyan-50 font-mono"
              placeholder='{"type":"click","x":100,"y":200}'
            />
            <div className="mt-2 flex items-center gap-3">
              <button
                type="button"
                onClick={() => void executeOmniAction()}
                className="rounded-full border border-emerald-400/30 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-100"
              >
                Execute
              </button>
            </div>
            {omniActionResult ? (
              <pre className="mt-3 max-h-[150px] overflow-auto whitespace-pre-wrap text-xs text-cyan-100/65">
                {omniActionResult}
              </pre>
            ) : null}
          </div>
        </div>
      </section>
    </section>
  );
}
