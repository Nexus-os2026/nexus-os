import { useState, useEffect, useCallback, useRef } from "react";
import {
  flashListLocalModels,
  flashDetectHardware,
  flashCreateSession,
  flashUnloadSession,
  flashClearSessions,
  flashGenerate,
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
}

const HISTORY_KEY = "flash-chat-history";
const TIER_COLORS: Record<Tier, string> = { fast: "#34d399", balanced: "#5eead4", power: "#c084fc" };
const TIER_LABELS: Record<Tier, string> = { fast: "Fast", balanced: "Balanced", power: "Max Power" };
const MODE_LABELS: Record<RouteMode, string> = { auto: "Auto", fast: "Fast", balanced: "Balanced", power: "Max Power" };

function loadHistory(): ChatMsg[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (raw) return JSON.parse(raw);
  } catch {}
  return [];
}

function saveHistory(msgs: ChatMsg[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(msgs.slice(-100)));
  } catch {}
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
  const [tokenCount, setTokenCount] = useState(0);
  const [genStartTime, setGenStartTime] = useState(0);
  const [tokPerSec, setTokPerSec] = useState(0);
  const [showLoader, setShowLoader] = useState(false);
  const chatRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Init: load model list + hardware info
  useEffect(() => {
    flashListLocalModels().then((list: any[]) => {
      const typed = list as LocalModel[];
      setModels(typed);
    }).catch(() => setModels([]));
    flashDetectHardware().then((hw: any) => {
      setHwRam(hw.total_ram_mb || 0);
      setHwCores(hw.cpu_cores || 0);
    }).catch(() => {});
    // Clear stale sessions from previous navigation
    flashClearSessions().catch(() => {});
  }, []);

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
        if (text) {
          setStreamText(prev => prev + text);
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
        }
      });
      if (cancelled) { u1(); return; }
      unlisteners.push(u1);

      const u2 = await listen("flash-done", () => {
        if (cancelled) return;
        setStreamText(text => {
          if (text) {
            setActiveModel(name => {
              setActiveTier(tier => {
                setMessages(prev => [...prev, { role: "assistant", content: text, model: name || undefined }]);
                return tier;
              });
              return name;
            });
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
      // If this tier already has a model, unload it first
      const existing = slots.find(s => s.tier === tier);
      if (existing) {
        try { await flashUnloadSession(existing.sessionId); } catch {}
        setSlots(prev => prev.filter(s => s.tier !== tier));
      }
      const sid = await flashCreateSession(path, 2048, tier === "fast" ? "speed" : "balanced");
      const model = models.find(m => m.file_path === path);
      const name = model?.name || path.split("/").pop() || "Model";
      setSlots(prev => [...prev.filter(s => s.tier !== tier), { tier, sessionId: sid, name, path }]);
      inputRef.current?.focus();
    } catch (e: any) { setError(typeof e === "string" ? e : e?.message || "Failed to load"); }
    setLoadingTier(null);
  }, [selectedPaths, loadingTier, models, slots]);

  const handleUnloadSlot = useCallback(async (tier: Tier) => {
    const slot = slots.find(s => s.tier === tier);
    if (!slot) return;
    try { await flashUnloadSession(slot.sessionId); } catch {}
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
    try { await flashGenerate(slot.sessionId, userMsg, 19999); }
    catch (e: any) { setError(typeof e === "string" ? e : e?.message || "Generation failed"); setGenerating(false); }
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
    try { await flashGenerate(slot.sessionId, "Continue", 19999); }
    catch (e: any) { setError(typeof e === "string" ? e : e?.message || "Generation failed"); setGenerating(false); }
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
        </div>
      )}

      {/* Metrics bar */}
      {hasAnyModel && (
        <div style={{ flexShrink:0, height:"28px", display:"flex", alignItems:"center", gap:"20px", padding:"0 16px", background:"#060d18", borderBottom:"1px solid #1e3a5f", fontSize:"11px" }}>
          <span style={{ color:"#6b7280" }}>tok/s <span style={{ color:"#5eead4", fontWeight:500 }}>{tokPerSec}</span></span>
          <span style={{ color:"#6b7280" }}>tokens <span style={{ color:"#5eead4", fontWeight:500 }}>{tokenCount}</span></span>
          {activeTier && activeModel && (
            <span style={{ color:"#6b7280" }}>
              <span style={{ color: TIER_COLORS[activeTier], fontWeight:500 }}>{TIER_LABELS[activeTier]}</span>
              {" "}<span style={{ color:"#9ca3af" }}>{activeModel}</span>
            </span>
          )}
          <span style={{ color:"#6b7280" }}>mode <span style={{ color:"#5eead4", fontWeight:500 }}>{MODE_LABELS[mode]}</span></span>
          {messages.length > 0 && <button onClick={clearHistory} style={{ marginLeft:"auto", background:"none", border:"none", color:"#4b5563", fontSize:"10px", cursor:"pointer" }}>Clear history</button>}
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
        {generating && streamText && (
          <div style={{ alignSelf:"flex-start", maxWidth:"80%", padding:"10px 14px", borderRadius:"12px 12px 12px 2px", background:"#1a1f2e", color:"#e5e7eb", fontSize:"13px", lineHeight:1.6, whiteSpace:"pre-wrap", wordBreak:"break-word" }}>
            {activeTier && <div style={{ fontSize:"10px", color: TIER_COLORS[activeTier], marginBottom:"4px" }}>{TIER_LABELS[activeTier]} {activeModel}</div>}
            {streamText}<span style={{ display:"inline-block", width:"6px", height:"14px", background:"#5eead4", marginLeft:"2px", animation:"blink 1s step-end infinite" }}/>
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

      {/* Input bar */}
      <div style={{ flexShrink:0, height:"56px", display:"flex", alignItems:"center", gap:"8px", padding:"0 16px", background:"#0a1628", borderTop:"1px solid #1e3a5f" }}>
        <input ref={inputRef} value={input} onChange={e => setInput(e.target.value)} onKeyDown={handleKeyDown} disabled={!hasAnyModel || generating} placeholder={!hasAnyModel ? "Load a model first..." : generating ? "Generating..." : mode === "auto" ? "Type a message (auto-routed)..." : `Type a message (${MODE_LABELS[mode]})...`} style={{ flex:1, background:"#0d1117", border:"1px solid #1e3a5f", borderRadius:"8px", color:"#e5e7eb", padding:"10px 14px", fontSize:"13px", outline:"none", opacity:!hasAnyModel?0.5:1 }}/>
        <button onClick={handleSend} disabled={!hasAnyModel || generating || !input.trim()} style={{ background:(!hasAnyModel||generating||!input.trim())?"#1e3a5f":"#5eead4", color:(!hasAnyModel||generating||!input.trim())?"#6b7280":"#0d1117", border:"none", padding:"10px 20px", borderRadius:"8px", fontSize:"13px", fontWeight:500, cursor:(!hasAnyModel||generating||!input.trim())?"not-allowed":"pointer", whiteSpace:"nowrap" }}>{generating ? "Stop" : "Send"}</button>
      </div>
      <style>{`@keyframes blink { 50% { opacity: 0; } }`}</style>
    </div>
  );
}
