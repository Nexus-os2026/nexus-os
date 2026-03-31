import { useState, useEffect, useCallback, useRef } from "react";
import {
  flashListLocalModels,
  flashDetectHardware,
  flashCreateSession,
  flashUnloadSession,
  flashClearSessions,
  flashGenerate,
  flashSystemMetrics,
  flashEnableSpeculative,
  flashDisableSpeculative,
  flashSpeculativeStatus,
  flashRunBenchmark,
  flashExportBenchmarkReport,
} from "../api/backend";
import { listen } from "@tauri-apps/api/event";
import "./flash-inference.css";

interface LocalModel {
  name: string;
  file_path: string;
  file_size_bytes: number;
  file_size_display: string;
  quant_type: string;
}

interface ChatMsg {
  role: "user" | "assistant";
  content: string;
  model?: string;
}

type Tier = "fast" | "balanced" | "power";
type RouteMode = "auto" | "fast" | "balanced" | "power";

interface LoadedSlot {
  tier: Tier;
  sessionId: string;
  name: string;
  path: string;
  fileSizeBytes: number;
}

const HISTORY_KEY = "flash-chat-history";
const TIER_COLORS: Record<Tier, string> = { fast: "#34d399", balanced: "#5eead4", power: "#c084fc" };
const TIER_LABELS: Record<Tier, string> = { fast: "Fast", balanced: "Balanced", power: "Max Power" };
const MODE_LABELS: Record<RouteMode, string> = { auto: "Auto", fast: "Fast", balanced: "Balanced", power: "Max Power" };

/** Template tags emitted by models that should not appear in user-visible output. */
const TEMPLATE_TAGS = new Set([
  "<start_of_turn>", "</start_of_turn>", "<end_of_turn>",
  "<|im_start|>", "<|im_end|>", "<|end|>",
  "<|assistant|>", "<|user|>", "<|system|>",
  "<|start_header_id|>", "<|end_header_id|>",
  "<|begin_of_text|>", "<|end_of_text|>",
  "<|eot_id|>",
  "<|begin\u{2581}of\u{2581}sentence|>",
  "<|Assistant|>", "<|User|>",
  "[INST]", "[/INST]",
  "model\n", "user\n", "model", "assistant",
]);

function loadHistory(): ChatMsg[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (raw) return JSON.parse(raw);
  } catch (err) {
    console.error("Failed to load chat history:", err);
  }
  return [];
}

function saveHistory(msgs: ChatMsg[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(msgs.slice(-100)));
  } catch (err) {
    console.error("Failed to save chat history:", err);
  }
}

/** Simple heuristic to pick the best tier for a prompt. */
function autoRoute(prompt: string, available: Tier[]): Tier {
  if (available.length === 1) return available[0];
  const lower = prompt.toLowerCase();
  const words = lower.split(/\s+/).length;

  const powerKeywords = /\b(prove|derive|analyze deeply|step by step|implement|debug|refactor|algorithm)\b/;
  const hasCodeBlock = /```/.test(prompt);
  const hasMath = /[=+\-*/^∑∫∂∇].*[=+\-*/^∑∫∂∇]/.test(prompt) || /\$.*\$/.test(prompt);

  if ((powerKeywords.test(lower) || hasCodeBlock || hasMath) && available.includes("power")) {
    return "power";
  }

  const balancedKeywords = /\b(explain|compare|write|summarize|describe|list|outline|review|translate)\b/;
  if (balancedKeywords.test(lower) && available.includes("balanced")) {
    return "balanced";
  }

  if (words < 20 && available.includes("fast")) {
    return "fast";
  }

  // Default: best available in order balanced > fast > power
  if (available.includes("balanced")) return "balanced";
  if (available.includes("fast")) return "fast";
  return available[0];
}

export default function FlashInference() {
  const [models, setModels] = useState<LocalModel[]>([]);
  const [slots, setSlots] = useState<LoadedSlot[]>([]);
  const [loadingTier, setLoadingTier] = useState<Tier | null>(null);
  const [selectedPaths, setSelectedPaths] = useState<Record<Tier, string>>({ fast: "", balanced: "", power: "" });
  const [mode, setMode] = useState<RouteMode>("auto");
  const [messages, setMessages] = useState<ChatMsg[]>(loadHistory);
  const [input, setInput] = useState("");
  const [generating, setGenerating] = useState(false);
  const [streamText, setStreamText] = useState("");
  const [activeModel, setActiveModel] = useState("");
  const [activeTier, setActiveTier] = useState<Tier | null>(null);
  const [error, setError] = useState("");
  const [hwRam, setHwRam] = useState(0);
  const [hwCores, setHwCores] = useState(0);
  const [sysMetrics, setSysMetrics] = useState<{ ram_used_mb: number; ram_total_mb: number; cpu_percent: number; vram_used_mb: number; vram_total_mb: number; ssd_read_mb_s: number; cache_hit_percent: number } | null>(null);
  const [tokenCount, setTokenCount] = useState(0);
  const [genStartTime, setGenStartTime] = useState(0);
  const [tokPerSec, setTokPerSec] = useState(0);
  const [showLoader, setShowLoader] = useState(false);
  const [benchmarkResults, setBenchmarkResults] = useState<any>(null);
  const [benchmarkReport, setBenchmarkReport] = useState<string>("");
  const [specEnabled, setSpecEnabled] = useState(false);
  const [specRate, setSpecRate] = useState(0);
  const [specDraftLen, setSpecDraftLen] = useState(0);
  const [specLoading, setSpecLoading] = useState(false);
  const chatRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const inThinkRef = useRef(false);
  const thinkBufRef = useRef("");
  // Refs to avoid triple-render from nested setState callbacks in flash-done handler
  const activeModelRef = useRef(activeModel);
  activeModelRef.current = activeModel;

  // Init: load model list + hardware info
  useEffect(() => {
    flashListLocalModels().then((list: any[]) => {
      const typed = list as LocalModel[];
      setModels(typed);
    }).catch(() => setModels([]));
    flashDetectHardware().then((hw: any) => {
      setHwRam(hw.total_ram_mb || 0);
      setHwCores(hw.cpu_cores || 0);
    }).catch((e) => { if (import.meta.env.DEV) console.warn("[FlashInference]", e); });
    // Clear stale backend sessions from previous navigation and reset frontend state.
    // This prevents "session not found" errors when the backend was cleared but
    // the frontend still held stale session IDs.
    flashClearSessions().then(() => {
      setSlots([]);
    }).catch(() => {
      setSlots([]);
    });
  }, []);

  // Poll live system metrics + speculative status every 2s when models are loaded
  useEffect(() => {
    if (slots.length === 0) return;
    let active = true;
    const poll = () => {
      flashSystemMetrics().then((m: any) => { if (active) setSysMetrics(m); }).catch((e) => { if (import.meta.env.DEV) console.warn("[FlashInference]", e); });
      flashSpeculativeStatus().then((s: any) => {
        if (active && s) {
          setSpecEnabled(!!s.enabled);
          setSpecRate(s.acceptance_rate || 0);
          setSpecDraftLen(s.draft_length || 0);
        }
      }).catch((e) => { if (import.meta.env.DEV) console.warn("[FlashInference]", e); });
    };
    poll();
    const id = setInterval(poll, 2000);
    return () => { active = false; clearInterval(id); };
  }, [slots.length]);

  // Toggle speculative decoding — use the "fast" tier model as draft for "power" tier
  const handleSpecToggle = useCallback(async () => {
    if (specEnabled) {
      setSpecLoading(true);
      try { await flashDisableSpeculative(); setSpecEnabled(false); } catch (err) { console.error("Failed to disable speculative:", err); }
      setSpecLoading(false);
      return;
    }
    // Find a small fast model to use as draft
    const fastSlot = slots.find(s => s.tier === "fast") || slots.find(s => s.tier === "balanced");
    if (!fastSlot) { setError("Load a Fast or Balanced model first to enable speculative decoding"); return; }
    setSpecLoading(true);
    try {
      await flashEnableSpeculative(fastSlot.path, 5);
      setSpecEnabled(true);
    } catch (e: any) {
      setError("Speculative decoding failed: " + (e?.toString() || "unknown"));
    }
    setSpecLoading(false);
  }, [specEnabled, slots]);

  useEffect(() => { saveHistory(messages); }, [messages]);

  useEffect(() => {
    const el = chatRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [streamText, messages]);

  // Token streaming listeners
  useEffect(() => {
    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    async function setup() {
      const u1 = await listen("flash-token", (e: any) => {
        if (cancelled) return;
        const text = e.payload?.text || e.payload?.token || "";
        if (!text || TEMPLATE_TAGS.has(text.trim())) return;

        // Filter <think>...</think> reasoning blocks (Qwen3.5 etc.)
        // Tokens arrive piecemeal so we buffer partial tags.
        let remaining = text;
        let visibleText = "";
        while (remaining.length > 0) {
          if (inThinkRef.current) {
            const closeIdx = remaining.indexOf("</think>");
            if (closeIdx >= 0) {
              inThinkRef.current = false;
              thinkBufRef.current = "";
              remaining = remaining.slice(closeIdx + "</think>".length);
            } else {
              // Still inside think block — consume all
              remaining = "";
            }
          } else {
            const openIdx = remaining.indexOf("<think>");
            if (openIdx >= 0) {
              visibleText += remaining.slice(0, openIdx);
              inThinkRef.current = true;
              thinkBufRef.current = "";
              remaining = remaining.slice(openIdx + "<think>".length);
            } else {
              // Check for partial "<think" at the end of token
              const partial = "<think>";
              let partialMatch = 0;
              for (let i = 1; i < partial.length && i <= remaining.length; i++) {
                if (remaining.endsWith(partial.slice(0, i))) {
                  partialMatch = i;
                }
              }
              if (partialMatch > 0) {
                visibleText += remaining.slice(0, remaining.length - partialMatch);
                thinkBufRef.current = remaining.slice(remaining.length - partialMatch);
              } else {
                // Flush any buffered partial that didn't become a tag
                if (thinkBufRef.current) {
                  visibleText += thinkBufRef.current;
                  thinkBufRef.current = "";
                }
                visibleText += remaining;
              }
              remaining = "";
            }
          }
        }

        if (!visibleText) return;

        setStreamText(prev => prev + visibleText);
        setTokenCount(c => {
          const nc = c + 1;
          setGenStartTime(start => {
            if (start > 0) {
              const el = (Date.now() - start) / 1000;
              if (el > 0) setTokPerSec(Number((nc / el).toFixed(1)));
            }
            return start;
          });
          return nc;
        });
      });
      if (cancelled) { u1(); return; }
      unlisteners.push(u1);

      const u2 = await listen("flash-done", () => {
        if (cancelled) return;
        inThinkRef.current = false;
        thinkBufRef.current = "";
        setStreamText(text => {
          const trimmed = text.replace(/<think>[\s\S]*?<\/think>/g, "").replace(/^\s+/, "");
          if (trimmed) {
            // Use ref to read model name — avoids nested setState that causes triple render
            const modelName = activeModelRef.current || undefined;
            setMessages(prev => [...prev, { role: "assistant", content: trimmed, model: modelName }]);
          }
          return "";
        });
        setGenerating(false);
      });
      if (cancelled) { u2(); return; }
      unlisteners.push(u2);

      const u3 = await listen("flash-error", (e: any) => {
        if (cancelled) return;
        setError(e.payload?.message || "Generation failed");
        setGenerating(false);
        setStreamText("");
      });
      if (cancelled) { u3(); return; }
      unlisteners.push(u3);
    }
    setup();

    return () => {
      cancelled = true;
      unlisteners.forEach(fn => fn());
    };
  }, []);

  const handleLoadSlot = useCallback(async (tier: Tier) => {
    const path = selectedPaths[tier];
    if (!path || loadingTier) return;
    setLoadingTier(tier);
    setError("");
    try {
      const model = models.find(m => m.file_path === path);
      const fileSizeBytes = model?.file_size_bytes || 0;
      const fileSizeGB = fileSizeBytes / (1024 * 1024 * 1024);

      // Auto-unload ALL other models when loading a huge model (>50 GB).
      // Multiple loaded models split RAM and starve mmap page cache,
      // killing MoE expert streaming performance.
      if (fileSizeGB > 50) {
        for (const slot of slots) {
          try { await flashUnloadSession(slot.sessionId); } catch (err) { console.error("Unload failed:", err); }
        }
        setSlots([]);
      } else {
        // Otherwise just unload the same tier if already occupied
        const existing = slots.find(s => s.tier === tier);
        if (existing) {
          try { await flashUnloadSession(existing.sessionId); } catch (err) { console.error("Unload failed:", err); }
          setSlots(prev => prev.filter(s => s.tier !== tier));
        }
      }

      const sid = await flashCreateSession(path, 2048, tier === "fast" ? "speed" : "balanced");
      const name = model?.name || path.split("/").pop() || "Model";
      setSlots(prev => [...prev.filter(s => s.tier !== tier), { tier, sessionId: sid, name, path, fileSizeBytes }]);
      inputRef.current?.focus();
    } catch (e: any) { setError(typeof e === "string" ? e : e?.message || "Failed to load"); }
    setLoadingTier(null);
  }, [selectedPaths, loadingTier, models, slots]);

  const handleUnloadSlot = useCallback(async (tier: Tier) => {
    const slot = slots.find(s => s.tier === tier);
    if (!slot) return;
    try { await flashUnloadSession(slot.sessionId); } catch (err) { console.error("Unload failed:", err); }
    setSlots(prev => prev.filter(s => s.tier !== tier));
  }, [slots]);

  const handleSend = useCallback(async () => {
    if (!input.trim() || slots.length === 0 || generating) return;
    const userMsg = input.trim();

    // Determine which model to use
    const availableTiers = slots.map(s => s.tier);
    let targetTier: Tier;
    if (mode === "auto") {
      targetTier = autoRoute(userMsg, availableTiers);
    } else {
      targetTier = availableTiers.includes(mode as Tier) ? (mode as Tier) : availableTiers[0];
    }

    const slot = slots.find(s => s.tier === targetTier)!;
    setActiveModel(slot.name);
    setActiveTier(targetTier);
    setInput("");
    setMessages(prev => [...prev, { role: "user", content: userMsg }]);
    setGenerating(true); setStreamText(""); setTokenCount(0); setGenStartTime(Date.now()); setTokPerSec(0); setError("");
    inThinkRef.current = false; thinkBufRef.current = "";
    try { await flashGenerate(slot.sessionId, userMsg, 19999); }
    catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message || "Generation failed";
      // If the backend session was lost (e.g. cleared on remount), remove the stale slot
      if (msg.includes("not found")) {
        setSlots(prev => prev.filter(s => s.sessionId !== slot.sessionId));
        setError("Session expired — model was unloaded. Please reload the model.");
      } else {
        setError(msg);
      }
      setGenerating(false);
    }
  }, [input, slots, generating, mode]);

  const handleKeyDown = (e: React.KeyboardEvent) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } };
  const clearHistory = () => { setMessages([]); localStorage.removeItem(HISTORY_KEY); };

  /** Detect if the last assistant message looks cut off mid-sentence. */
  const lastMsgIncomplete = (() => {
    if (generating || messages.length === 0) return false;
    const last = messages[messages.length - 1];
    if (last.role !== "assistant") return false;
    const t = last.content.trimEnd();
    if (!t) return false;
    const lastChar = t[t.length - 1];
    // Ends mid-sentence: no terminal punctuation and not a code fence
    return !/[.!?:;\n`")\]]$/.test(lastChar);
  })();

  const handleContinue = useCallback(async () => {
    if (slots.length === 0 || generating) return;
    const availableTiers = slots.map(s => s.tier);
    // Use the same tier as the last response, or fall back to auto
    const targetTier = activeTier && availableTiers.includes(activeTier) ? activeTier : availableTiers[0];
    const slot = slots.find(s => s.tier === targetTier)!;
    setActiveModel(slot.name);
    setActiveTier(targetTier);
    setMessages(prev => [...prev, { role: "user", content: "Continue" }]);
    setGenerating(true); setStreamText(""); setTokenCount(0); setGenStartTime(Date.now()); setTokPerSec(0); setError("");
    inThinkRef.current = false; thinkBufRef.current = "";
    try { await flashGenerate(slot.sessionId, "Continue", 19999); }
    catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message || "Generation failed";
      if (msg.includes("not found")) {
        setSlots(prev => prev.filter(s => s.sessionId !== slot.sessionId));
        setError("Session expired — model was unloaded. Please reload the model.");
      } else {
        setError(msg);
      }
      setGenerating(false);
    }
  }, [slots, generating, activeTier]);
  const ramGB = (hwRam / 1024).toFixed(1);
  const hasAnyModel = slots.length > 0;

  // Dropdown style shared across all selectors
  const selectStyle = { background:"#0d1117", color:"#e5e7eb", border:"1px solid #1e3a5f", borderRadius:"6px", padding:"4px 8px", fontSize:"12px", outline:"none" };

  return (
    <div style={{ display:"flex", flexDirection:"column", flex:"1 1 auto", minHeight:0, width:"100%", overflow:"hidden", background:"#080e1a" }}>
      {/* Header bar */}
      <div style={{ flexShrink:0, minHeight:"48px", display:"flex", alignItems:"center", gap:"10px", padding:"0 16px", background:"#0a1628", borderBottom:"1px solid #1e3a5f" }}>
        <span style={{ color:"#5eead4", fontSize:"14px", fontWeight:600, whiteSpace:"nowrap" }}>⚡ Flash Inference</span>

        {/* Loaded model pills */}
        {slots.map(slot => (
          <span key={slot.tier} style={{ display:"inline-flex", alignItems:"center", gap:"6px", background:"#0d1117", border:`1px solid ${TIER_COLORS[slot.tier]}`, borderRadius:"12px", padding:"3px 10px", fontSize:"11px", color: TIER_COLORS[slot.tier] }}>
            <span style={{ fontWeight:600, textTransform:"uppercase", fontSize:"9px", opacity:0.7 }}>{TIER_LABELS[slot.tier]}</span>
            <span style={{ color:"#e5e7eb", maxWidth:"140px", overflow:"hidden", textOverflow:"ellipsis", whiteSpace:"nowrap" }}>{slot.name}</span>
            <button onClick={() => handleUnloadSlot(slot.tier)} style={{ background:"none", border:"none", color:"#f87171", cursor:"pointer", fontSize:"12px", padding:0, lineHeight:1 }}>✕</button>
          </span>
        ))}

        {/* Load more button */}
        {slots.length < 3 && (
          <button onClick={() => setShowLoader(!showLoader)} style={{ background:"transparent", border:"1px solid #5eead4", color:"#5eead4", padding:"4px 12px", borderRadius:"6px", fontSize:"11px", cursor:"pointer", whiteSpace:"nowrap" }}>
            {showLoader ? "Cancel" : "+ Load Model"}
          </button>
        )}

        {/* Mode selector */}
        {hasAnyModel && (
          <div style={{ marginLeft:"auto", display:"flex", alignItems:"center", gap:"2px", background:"#0d1117", borderRadius:"6px", border:"1px solid #1e3a5f", padding:"2px" }}>
            {(["auto", "fast", "balanced", "power"] as RouteMode[]).map(m => {
              const isAvailable = m === "auto" || slots.some(s => s.tier === m);
              const isActive = mode === m;
              return (
                <button key={m} onClick={() => isAvailable && setMode(m)} disabled={!isAvailable} style={{
                  background: isActive ? "#1e3a5f" : "transparent",
                  border: "none",
                  color: !isAvailable ? "#374151" : isActive ? "#5eead4" : "#9ca3af",
                  padding: "3px 10px",
                  borderRadius: "4px",
                  fontSize: "11px",
                  fontWeight: isActive ? 600 : 400,
                  cursor: isAvailable ? "pointer" : "default",
                  whiteSpace: "nowrap",
                }}>{MODE_LABELS[m]}</button>
              );
            })}
          </div>
        )}

        {!hasAnyModel && <span style={{ marginLeft:"auto" }}/>}
        <span style={{ fontSize:"11px", color:"#6b7280", whiteSpace:"nowrap" }}>{ramGB} GB | {hwCores} cores</span>
      </div>

      {/* Model loader panel */}
      {showLoader && (
        <div style={{ flexShrink:0, padding:"10px 16px", background:"#060d18", borderBottom:"1px solid #1e3a5f", display:"flex", gap:"12px", alignItems:"center", flexWrap:"wrap" }}>
          {(["fast", "balanced", "power"] as Tier[]).filter(t => !slots.some(s => s.tier === t)).map(tier => (
            <div key={tier} style={{ display:"flex", alignItems:"center", gap:"6px" }}>
              <span style={{ color: TIER_COLORS[tier], fontSize:"11px", fontWeight:600, width:"60px" }}>{TIER_LABELS[tier]}</span>
              <select value={selectedPaths[tier]} onChange={e => setSelectedPaths(prev => ({ ...prev, [tier]: e.target.value }))} style={{ ...selectStyle, width:"280px" }}>
                <option value="">Select model...</option>
                {models.map(m => <option key={m.file_path} value={m.file_path}>{m.name} ({m.file_size_display})</option>)}
              </select>
              <button onClick={() => handleLoadSlot(tier)} disabled={!selectedPaths[tier] || loadingTier !== null} style={{
                background: "transparent",
                border: `1px solid ${TIER_COLORS[tier]}`,
                color: TIER_COLORS[tier],
                padding: "4px 12px",
                borderRadius: "6px",
                fontSize: "11px",
                cursor: (!selectedPaths[tier] || loadingTier) ? "not-allowed" : "pointer",
                opacity: (!selectedPaths[tier] || loadingTier) ? 0.4 : 1,
                whiteSpace: "nowrap",
              }}>{loadingTier === tier ? "Loading..." : "Load"}</button>
            </div>
          ))}
          {loadingTier && (() => {
            const loadingModel = models.find(m => m.file_path === selectedPaths[loadingTier]);
            const sizeGB = loadingModel ? loadingModel.file_size_bytes / (1024 * 1024 * 1024) : 0;
            if (sizeGB > 50) return (
              <div style={{ width:"100%", padding:"6px 0 0", fontSize:"11px", color:"#c084fc" }}>
                Loading {Math.round(sizeGB * 10) / 10} GB model via SSD streaming. Estimated time: ~5 minutes for first load.
              </div>
            );
            return null;
          })()}
        </div>
      )}

      {/* Metrics bar */}
      {hasAnyModel && (
        <div style={{ flexShrink:0, height:"28px", display:"flex", alignItems:"center", gap:"16px", padding:"0 16px", background:"#060d18", borderBottom:"1px solid #1e3a5f", fontSize:"11px" }}>
          {sysMetrics && (
            <>
              <span style={{ color:"#6b7280" }}>RAM <span style={{ color:"#5eead4", fontWeight:500 }}>{(sysMetrics.ram_used_mb / 1024).toFixed(1)}</span><span style={{ color:"#4b5563" }}> / {(sysMetrics.ram_total_mb / 1024).toFixed(1)} GB</span></span>
              <span style={{ color:"#6b7280" }}>CPU <span style={{ color: sysMetrics.cpu_percent > 80 ? "#f87171" : "#5eead4", fontWeight:500 }}>{Math.round(sysMetrics.cpu_percent)}%</span></span>
              {sysMetrics.vram_total_mb > 0 && (
                <span style={{ color:"#6b7280" }}>VRAM <span style={{ color:"#c084fc", fontWeight:500 }}>{(sysMetrics.vram_used_mb / 1024).toFixed(1)}</span><span style={{ color:"#4b5563" }}> / {(sysMetrics.vram_total_mb / 1024).toFixed(1)} GB</span></span>
              )}
              {sysMetrics.cache_hit_percent > 0 && (
                <span style={{ color:"#6b7280" }}>Cache <span style={{ color: sysMetrics.cache_hit_percent > 70 ? "#34d399" : "#fbbf24", fontWeight:500 }}>{Math.round(sysMetrics.cache_hit_percent)}%</span></span>
              )}
              {sysMetrics.ssd_read_mb_s > 0 && (
                <span style={{ color:"#6b7280" }}>I/O <span style={{ color:"#5eead4", fontWeight:500 }}>{sysMetrics.ssd_read_mb_s >= 1024 ? (sysMetrics.ssd_read_mb_s / 1024).toFixed(1) + " GB/s" : Math.round(sysMetrics.ssd_read_mb_s) + " MB/s"}</span></span>
              )}
            </>
          )}
          <span style={{ color:"#6b7280" }}><span style={{ color:"#5eead4", fontWeight:500 }}>{Number(tokPerSec).toFixed(1)}</span> tok/s</span>
          <span style={{ color:"#6b7280" }}><span style={{ color:"#5eead4", fontWeight:500 }}>{Math.round(tokenCount)}</span> tokens</span>
          {slots.some(s => s.tier === "power") && (
            <button
              onClick={handleSpecToggle}
              disabled={specLoading || generating}
              title={specEnabled ? `Speculative decoding ON — ${Math.round(specRate * 100)}% accept rate, draft len ${specDraftLen}` : "Enable speculative decoding (uses Fast model as draft)"}
              style={{ background: specEnabled ? "#065f46" : "#1e293b", border: "1px solid " + (specEnabled ? "#34d399" : "#374151"), color: specEnabled ? "#34d399" : "#9ca3af", padding: "2px 8px", borderRadius: "4px", cursor: specLoading ? "wait" : "pointer", fontSize: "10px", fontWeight: 600, whiteSpace: "nowrap" }}
            >
              {specLoading ? "..." : specEnabled ? `Spec ${Math.round(specRate * 100)}%` : "Spec Off"}
            </button>
          )}
          {activeTier && activeModel && (
            <span style={{ color:"#6b7280" }}>
              <span style={{ color: TIER_COLORS[activeTier], fontWeight:500 }}>{TIER_LABELS[activeTier]}</span>
              {" "}<span style={{ color:"#9ca3af" }}>{activeModel}</span>
            </span>
          )}
          {messages.length > 0 && !generating && (
            <button onClick={clearHistory} title="Clear chat history" style={{ marginLeft:"auto", background:"#dc2626", border:"none", color:"#ffffff", padding:"3px 12px", borderRadius:"4px", cursor:"pointer", fontSize:"11px", fontWeight:600, display:"flex", alignItems:"center", gap:"5px", whiteSpace:"nowrap" }}>
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
              Clear Chat
            </button>
          )}
        </div>
      )}

      {/* Chat area */}
      <div ref={chatRef} style={{ flex:1, minHeight:0, overflowY:"auto", padding:"16px", display:"flex", flexDirection:"column", gap:"12px" }}>
        {!hasAnyModel && messages.length === 0 && (
          <div style={{ flex:1, display:"flex", flexDirection:"column", alignItems:"center", justifyContent:"center", color:"#6b7280", fontSize:"14px", gap:"12px" }}>
            <span style={{ fontSize:"40px", opacity:0.3 }}>⚡</span>
            <span>Load models to start chatting.</span>
            <span style={{ fontSize:"12px", color:"#4b5563" }}>Load a Fast model for quick answers, Balanced for medium tasks, Max Power for deep reasoning.</span>
          </div>
        )}
        {hasAnyModel && messages.length === 0 && !generating && (
          <div style={{ flex:1, display:"flex", flexDirection:"column", alignItems:"center", justifyContent:"center", color:"#6b7280", fontSize:"14px", gap:"12px" }}>
            <span style={{ fontSize:"40px", opacity:0.3 }}>⚡</span>
            <span>{slots.length === 1 ? "Model loaded." : `${slots.length} models loaded.`} Type a message to begin.</span>
            {slots.length > 1 && mode === "auto" && <span style={{ fontSize:"12px", color:"#4b5563" }}>Auto mode routes to the best model for each question.</span>}
          </div>
        )}
        {messages.map((msg, i) => (
          <div key={i} style={{ alignSelf:msg.role==="user"?"flex-end":"flex-start", maxWidth:"80%", padding:"10px 14px", borderRadius:msg.role==="user"?"12px 12px 2px 12px":"12px 12px 12px 2px", background:msg.role==="user"?"#1e3a5f":"#1a1f2e", color:"#e5e7eb", fontSize:"13px", lineHeight:1.6, whiteSpace:"pre-wrap", wordBreak:"break-word" }}>
            {msg.role === "assistant" && msg.model && <div style={{ fontSize:"10px", color:"#5eead4", marginBottom:"4px" }}>{msg.model}</div>}
            {msg.content}
          </div>
        ))}
        {generating && streamText.replace(/^\s+/, "") && (
          <div style={{ alignSelf:"flex-start", maxWidth:"80%", padding:"10px 14px", borderRadius:"12px 12px 12px 2px", background:"#1a1f2e", color:"#e5e7eb", fontSize:"13px", lineHeight:1.6, whiteSpace:"pre-wrap", wordBreak:"break-word" }}>
            {activeTier && <div style={{ fontSize:"10px", color: TIER_COLORS[activeTier], marginBottom:"4px" }}>{TIER_LABELS[activeTier]} {activeModel}</div>}
            {streamText.replace(/^\s+/, "")}<span style={{ display:"inline-block", width:"6px", height:"14px", background:"#5eead4", marginLeft:"2px", animation:"blink 1s step-end infinite" }}/>
            {activeTier === "power" && tokPerSec > 0 && tokPerSec < 1 && slots.length > 0 && slots.find(s => s.tier === "power")?.fileSizeBytes && (slots.find(s => s.tier === "power")?.fileSizeBytes || 0) > 50 * 1024 * 1024 * 1024 && (
              <div style={{ fontSize:"10px", color:"#9ca3af", marginTop:"6px", fontStyle:"italic" }}>Generating at {Number(tokPerSec).toFixed(1)} tok/s — large models are slower but smarter.</div>
            )}
          </div>
        )}
        {lastMsgIncomplete && (
          <div style={{ alignSelf:"flex-start" }}>
            <button onClick={handleContinue} style={{ background:"transparent", border:"1px solid #5eead4", color:"#5eead4", padding:"6px 16px", borderRadius:"8px", fontSize:"12px", cursor:"pointer" }}>Continue ▶</button>
          </div>
        )}
        {generating && !streamText && (
          <div style={{ alignSelf:"flex-start", padding:"10px 14px", borderRadius:"12px 12px 12px 2px", background:"#1a1f2e", color:"#6b7280", fontSize:"13px" }}>
            {activeTier && <span style={{ color: TIER_COLORS[activeTier], marginRight:"6px" }}>{TIER_LABELS[activeTier]}</span>}
            Thinking...
            {activeTier === "power" && (() => {
              const slot = slots.find(s => s.tier === "power");
              if (slot && slot.fileSizeBytes > 50 * 1024 * 1024 * 1024) return (
                <div style={{ fontSize:"10px", color:"#9ca3af", marginTop:"4px" }}>Large models are slower but smarter. For faster responses, try the Balanced model.</div>
              );
              return null;
            })()}
          </div>
        )}
      </div>

      {/* Error bar */}
      {error && (
        <div style={{ flexShrink:0, padding:"8px 16px", background:"#2d1b1b", borderTop:"1px solid #5f2020", color:"#f87171", fontSize:"12px", display:"flex", alignItems:"center", justifyContent:"space-between" }}>
          <span>{error}</span>
          <button onClick={() => setError("")} style={{ background:"none", border:"none", color:"#f87171", cursor:"pointer", fontSize:"14px" }}>✕</button>
        </div>
      )}

      {/* Benchmark bar */}
      <div style={{ flexShrink:0, display:"flex", alignItems:"center", gap:"8px", padding:"6px 16px", background:"#0d1117", borderTop:"1px solid #1e3a5f", fontSize:12 }}>
        <span style={{ color:"#64748b" }}>Benchmark:</span>
        <button onClick={async () => { try { const r = await flashRunBenchmark(models[0]?.file_path || "", "balanced"); setBenchmarkResults(r); } catch(e) { console.error(e); } }} disabled={models.length === 0} style={{ background:"#1e3a5f", color: models.length === 0 ? "#475569" : "#5eead4", border:"none", padding:"4px 12px", borderRadius:4, cursor: models.length === 0 ? "not-allowed" : "pointer", fontSize:12 }}>Run Benchmark</button>
        <button onClick={async () => { try { if(benchmarkResults) { const p = await flashExportBenchmarkReport(Array.isArray(benchmarkResults) ? benchmarkResults : [benchmarkResults]); setBenchmarkReport(p); } } catch(e) { console.error(e); } }} disabled={!benchmarkResults} style={{ background:"#1e3a5f", color:!benchmarkResults ? "#475569" : "#5eead4", border:"none", padding:"4px 12px", borderRadius:4, cursor:!benchmarkResults ? "not-allowed" : "pointer", fontSize:12 }}>Export Report</button>
        {benchmarkReport && <span style={{ color:"#22c55e" }}>Saved: {benchmarkReport}</span>}
      </div>

      {/* Input bar */}
      <div style={{ flexShrink:0, height:"56px", display:"flex", alignItems:"center", gap:"8px", padding:"0 16px", background:"#0a1628", borderTop:"1px solid #1e3a5f" }}>
        <input ref={inputRef} value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} disabled={!hasAnyModel || generating} placeholder={!hasAnyModel ? "Load a model first..." : generating ? "Generating..." : mode === "auto" ? "Type a message (auto-routed)..." : `Type a message (${MODE_LABELS[mode]})...`} style={{ flex:1, background:"#0d1117", border:"1px solid #1e3a5f", borderRadius:"8px", color:"#e5e7eb", padding:"10px 14px", fontSize:"13px", outline:"none", opacity:!hasAnyModel?0.5:1 }}/>
        <button onClick={handleSend} disabled={!hasAnyModel || generating || !input.trim()} style={{ background:(!hasAnyModel||generating||!input.trim())?"#1e3a5f":"#5eead4", color:(!hasAnyModel||generating||!input.trim())?"#6b7280":"#0d1117", border:"none", padding:"10px 20px", borderRadius:"8px", fontSize:"13px", fontWeight:500, cursor:(!hasAnyModel||generating||!input.trim())?"not-allowed":"pointer", whiteSpace:"nowrap" }}>{generating ? "Stop" : "Send"}</button>
      </div>
      <style>{`@keyframes blink { 50% { opacity: 0; } }`}</style>
    </div>
  );
}
