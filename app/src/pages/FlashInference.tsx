import { useState, useEffect, useCallback, useRef } from "react";
import {
  Zap, FolderOpen, Send, Square, Trash2, AlertTriangle, X, Cpu,
  HardDrive, MemoryStick, Gauge, Download, Package, Search, Check,
  Loader, ChevronDown, ChevronUp,
} from "lucide-react";
import {
  flashDetectHardware,
  flashProfileModel,
  flashEstimatePerformance,
  flashCreateSession,
  flashUnloadSession,
  flashGetMetrics,
  flashGenerate,
  flashCatalogRecommend,
  flashCatalogSearch,
  flashListLocalModels,
  flashDownloadModel,
  flashDeleteLocalModel,
  flashAvailableDiskSpace,
  hasDesktopRuntime,
} from "../api/backend";
import "./flash-inference.css";

/* ── types ── */

interface HwInfo {
  total_ram_mb: number;
  available_ram_mb: number;
  cpu_cores: number;
  cpu_model: string;
  ssd_type: string;
  ssd_read_mb_per_sec: number;
}

interface ModelProfile {
  name: string;
  parameters_b: number;
  quantization: string;
  file_size_mb: number;
  is_moe: boolean;
  num_experts: number;
  active_experts: number;
  active_parameters_b: number;
  max_context_length: number;
  architecture: string;
}

interface PerfEstimate {
  tokens_per_second: number;
  estimated_ram_mb: number;
  cache_hit_rate: number;
  needs_disk_streaming: boolean;
  max_context_at_budget: number;
}

interface InferenceMetrics {
  session_id: string;
  tokens_per_second: number;
  prompt_tokens_per_second: number;
  memory_used_mb: number;
  memory_budget_mb: number;
  memory_utilization: number;
  expert_cache_hit_rate: number;
  io_read_mb_per_sec: number;
  cpu_utilization: number;
  context_used: number;
  context_max: number;
  total_tokens_generated: number;
  uptime_seconds: number;
}

interface ChatMsg {
  role: "user" | "assistant";
  content: string;
}

interface LocalModel {
  name: string;
  file_path: string;
  file_size_bytes: number;
  file_size_display: string;
  quant_type: string;
  downloaded_at: string;
  sha256: string | null;
  verified: boolean;
}

interface CatalogRec {
  entry: {
    name: string;
    provider: string;
    huggingface_id: string;
    license: string;
    total_params: number;
    is_moe: boolean;
    active_params: number | null;
    num_experts: number | null;
    specialization: string;
    available_quants: Array<{
      quant_type: string;
      file_size_gb: number;
      min_ram_gb: number;
      quality_rating: number;
    }>;
  };
  best_quant: {
    quant_type: string;
    file_size_gb: number;
    min_ram_gb: number;
    quality_rating: number;
  };
  fitness_score: number;
  estimated_tok_per_sec: number;
  reason: string;
}

interface DlProgress {
  model_name: string;
  file_index: number;
  file_count: number;
  bytes_downloaded: number;
  total_bytes: number;
  percent: number;
  speed_mb_per_sec: number;
  eta_seconds: number;
  status: string | { Failed: string };
}

type Priority = "speed" | "balanced" | "context";
type ModelTab = "local" | "browse";

/* ── helpers ── */

function fmtMb(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}

function fmtNum(n: number, decimals = 1): string {
  return n.toFixed(decimals);
}

function fmtBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(0)} KB`;
}

function fmtEta(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

function dlStatusLabel(status: DlProgress["status"]): string {
  if (typeof status === "string") return status;
  if (typeof status === "object" && "Failed" in status) return `Failed: ${status.Failed}`;
  return "Unknown";
}

/* ── component ── */

export default function FlashInference(): JSX.Element {
  /* ─ hardware ─ */
  const [hw, setHw] = useState<HwInfo | null>(null);

  /* ─ model ─ */
  const [modelPath, setModelPath] = useState("");
  const [profile, setProfile] = useState<ModelProfile | null>(null);
  const [estimate, setEstimate] = useState<PerfEstimate | null>(null);
  const [priority, setPriority] = useState<Priority>("balanced");

  /* ─ session ─ */
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  /* ─ chat ─ */
  const [messages, setMessages] = useState<ChatMsg[]>([]);
  const [draft, setDraft] = useState("");
  const [isGenerating, setIsGenerating] = useState(false);
  const [streamingText, setStreamingText] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  /* ─ metrics ─ */
  const [metrics, setMetrics] = useState<InferenceMetrics | null>(null);
  const metricsIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  /* ─ error ─ */
  const [error, setError] = useState<string | null>(null);

  /* ─ model library ─ */
  const [modelTab, setModelTab] = useState<ModelTab>("browse");
  const [localModels, setLocalModels] = useState<LocalModel[]>([]);
  const [catalogRecs, setCatalogRecs] = useState<CatalogRec[]>([]);
  const [catalogQuery, setCatalogQuery] = useState("");
  const [diskSpace, setDiskSpace] = useState<number | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<Record<string, DlProgress>>({});
  const [expandedRec, setExpandedRec] = useState<string | null>(null);
  const [deletingModel, setDeletingModel] = useState<string | null>(null);

  /* ── detect hardware on mount ── */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const info = await flashDetectHardware();
        if (!cancelled) setHw(info);
      } catch {
        if (!cancelled) {
          setHw({
            total_ram_mb: 0,
            available_ram_mb: 0,
            cpu_cores: navigator.hardwareConcurrency || 0,
            cpu_model: "Unknown",
            ssd_type: "Unknown",
            ssd_read_mb_per_sec: 0,
          });
        }
      }
    })();
    return () => { cancelled = true; };
  }, []);

  /* ── load catalog recommendations ── */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const recs = await flashCatalogRecommend();
        if (!cancelled && Array.isArray(recs)) setCatalogRecs(recs);
      } catch { /* non-critical */ }
    })();
    return () => { cancelled = true; };
  }, []);

  /* ── load local models + disk space ── */
  const refreshLocalModels = useCallback(async () => {
    try {
      const [models, space] = await Promise.all([
        flashListLocalModels(),
        flashAvailableDiskSpace(),
      ]);
      if (Array.isArray(models)) setLocalModels(models);
      if (typeof space === "number") setDiskSpace(space);
    } catch { /* non-critical */ }
  }, []);

  useEffect(() => {
    refreshLocalModels();
  }, [refreshLocalModels]);

  /* ── listen for download progress events ── */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlisten = await eventMod.listen<DlProgress>("flash-download-progress", (event) => {
          const p = event.payload;
          setDownloadProgress((prev) => ({ ...prev, [p.model_name]: p }));
          // When complete, refresh local models list
          const status = typeof p.status === "string" ? p.status : "";
          if (status === "Complete") {
            setTimeout(() => refreshLocalModels(), 500);
          }
        });
      } catch { /* not in Tauri */ }
    })();
    return () => { unlisten?.(); };
  }, [refreshLocalModels]);

  /* ── scroll chat to bottom ── */
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingText]);

  /* ── metrics polling ── */
  useEffect(() => {
    if (metricsIntervalRef.current) {
      clearInterval(metricsIntervalRef.current);
      metricsIntervalRef.current = null;
    }
    if (!sessionId) { setMetrics(null); return; }
    const poll = async () => {
      try { setMetrics(await flashGetMetrics(sessionId)); } catch { /* */ }
    };
    poll();
    metricsIntervalRef.current = setInterval(poll, 2000);
    return () => {
      if (metricsIntervalRef.current) {
        clearInterval(metricsIntervalRef.current);
        metricsIntervalRef.current = null;
      }
    };
  }, [sessionId]);

  /* ── streaming events ── */
  useEffect(() => {
    if (!sessionId) return;
    let unlistenToken: (() => void) | undefined;
    let unlistenDone: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    (async () => {
      try {
        const eventMod = await import("@tauri-apps/api/event");
        unlistenToken = await eventMod.listen<{ text: string }>("flash-token", (event) => {
          setStreamingText((prev) => prev + event.payload.text);
        });
        unlistenDone = await eventMod.listen<{ stats?: Record<string, unknown> }>("flash-done", () => {
          setIsGenerating(false);
          setStreamingText((prev) => {
            if (prev) setMessages((msgs) => [...msgs, { role: "assistant", content: prev }]);
            return "";
          });
        });
        unlistenError = await eventMod.listen<{ message: string }>("flash-error", (event) => {
          setError(event.payload.message);
          setIsGenerating(false);
          setStreamingText((prev) => {
            if (prev) setMessages((msgs) => [...msgs, { role: "assistant", content: prev }]);
            return "";
          });
        });
      } catch { /* not in Tauri */ }
    })();
    return () => { unlistenToken?.(); unlistenDone?.(); unlistenError?.(); };
  }, [sessionId]);

  /* ── profile model when path changes ── */
  const profileModel = useCallback(async (path: string) => {
    if (!path.trim()) { setProfile(null); setEstimate(null); return; }
    try {
      setError(null);
      const [prof, est] = await Promise.all([
        flashProfileModel(path),
        flashEstimatePerformance(path),
      ]);
      setProfile(prof);
      setEstimate(est);
    } catch (err) {
      setError(`Failed to profile model: ${err}`);
      setProfile(null);
      setEstimate(null);
    }
  }, []);

  /* ── browse for GGUF file ── */
  const handleBrowse = useCallback(async () => {
    try {
      const importer = new Function("specifier", "return import(specifier)") as (
        specifier: string,
      ) => Promise<{
        open: (options: {
          multiple?: boolean;
          filters?: Array<{ name: string; extensions: string[] }>;
        }) => Promise<string | string[] | null>;
      }>;
      const dialogMod = await importer("@tauri-apps/plugin-dialog");
      const selected = await dialogMod.open({
        multiple: false,
        filters: [{ name: "GGUF Models", extensions: ["gguf"] }],
      });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (path) { setModelPath(path); profileModel(path); }
    } catch {
      setError("File dialog unavailable — please type the model path manually");
    }
  }, [profileModel]);

  /* ── select a local model ── */
  const selectLocalModel = useCallback((model: LocalModel) => {
    setModelPath(model.file_path);
    profileModel(model.file_path);
  }, [profileModel]);

  /* ── download a model from catalog ── */
  const handleDownload = useCallback(async (rec: CatalogRec) => {
    const hfId = rec.entry.huggingface_id;
    const quant = rec.best_quant.quant_type;
    // Construct a plausible filename from the catalog data
    const safeName = rec.entry.name.replace(/[^a-zA-Z0-9._-]/g, "-");
    const filename = `${safeName}-${quant}.gguf`;

    setDownloadProgress((prev) => ({
      ...prev,
      [filename]: {
        model_name: filename,
        file_index: 1,
        file_count: 1,
        bytes_downloaded: 0,
        total_bytes: Math.round(rec.best_quant.file_size_gb * 1_073_741_824),
        percent: 0,
        speed_mb_per_sec: 0,
        eta_seconds: 0,
        status: "Starting",
      },
    }));

    try {
      await flashDownloadModel(hfId, filename);
      refreshLocalModels();
    } catch (err) {
      setError(`Download failed: ${err}`);
      setDownloadProgress((prev) => {
        const next = { ...prev };
        if (next[filename]) {
          next[filename] = { ...next[filename], status: { Failed: String(err) } };
        }
        return next;
      });
    }
  }, [refreshLocalModels]);

  /* ── delete a local model ── */
  const handleDelete = useCallback(async (filename: string) => {
    setDeletingModel(filename);
    try {
      await flashDeleteLocalModel(filename);
      await refreshLocalModels();
    } catch (err) {
      setError(`Delete failed: ${err}`);
    } finally {
      setDeletingModel(null);
    }
  }, [refreshLocalModels]);

  /* ── catalog search ── */
  const handleCatalogSearch = useCallback(async () => {
    if (!catalogQuery.trim()) {
      try {
        const recs = await flashCatalogRecommend();
        if (Array.isArray(recs)) setCatalogRecs(recs);
      } catch { /* */ }
      return;
    }
    try {
      const results = await flashCatalogSearch(catalogQuery);
      if (Array.isArray(results)) {
        // Wrap search results into recommendation-like shape
        const asRecs: CatalogRec[] = results.map((entry) => ({
          entry,
          best_quant: entry.available_quants?.[0] ?? {
            quant_type: "Q4_K_M",
            file_size_gb: 0,
            min_ram_gb: 0,
            quality_rating: 0,
          },
          fitness_score: 0,
          estimated_tok_per_sec: 0,
          reason: "",
        }));
        setCatalogRecs(asRecs);
      }
    } catch (err) {
      setError(`Search failed: ${err}`);
    }
  }, [catalogQuery]);

  /* ── load model ── */
  const handleLoad = useCallback(async () => {
    if (!modelPath.trim()) return;
    setIsLoading(true);
    setError(null);
    try {
      const contextLen = profile?.max_context_length ?? 4096;
      const id = await flashCreateSession(modelPath, contextLen, priority);
      setSessionId(id);
    } catch (err) {
      setError(`Failed to load model: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, [modelPath, priority, profile]);

  /* ── unload model ── */
  const handleUnload = useCallback(async () => {
    if (!sessionId) return;
    try {
      await flashUnloadSession(sessionId);
      setSessionId(null);
      setMetrics(null);
      setMessages([]);
      setStreamingText("");
    } catch (err) {
      setError(`Failed to unload: ${err}`);
    }
  }, [sessionId]);

  /* ── send message ── */
  const handleSend = useCallback(async () => {
    const text = draft.trim();
    if (!text || !sessionId || isGenerating) return;
    setDraft("");
    setMessages((prev) => [...prev, { role: "user", content: text }]);
    setIsGenerating(true);
    setStreamingText("");
    try {
      const result = await flashGenerate(sessionId, text);
      if (result && typeof result === "object" && result.text) {
        setMessages((prev) => [...prev, { role: "assistant", content: result.text }]);
        setIsGenerating(false);
      } else if (typeof result === "string" && result) {
        setMessages((prev) => [...prev, { role: "assistant", content: result }]);
        setIsGenerating(false);
      }
    } catch (err) {
      if (streamingText) {
        setMessages((prev) => [...prev, { role: "assistant", content: streamingText }]);
        setStreamingText("");
      }
      setError(`Generation failed: ${err}`);
      setIsGenerating(false);
    }
  }, [draft, sessionId, isGenerating, streamingText]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); }
    },
    [handleSend],
  );

  const isDesktop = hasDesktopRuntime();

  // Active downloads
  const activeDownloads = Object.values(downloadProgress).filter(
    (p) => typeof p.status === "string" && (p.status === "Starting" || p.status === "Downloading" || p.status === "Verifying"),
  );

  /* ── render ── */
  return (
    <section className="flash-inference">
      {/* Header */}
      <div className="flash-inference__header">
        <Zap size={28} />
        <h1>Flash Inference</h1>
      </div>
      <p className="flash-inference__subtitle">
        Run AI models locally with automatic hardware-aware configuration
      </p>

      {/* Error banner */}
      {error && (
        <div className="flash-error">
          <AlertTriangle size={16} />
          <span>{error}</span>
          <button className="flash-error__dismiss" onClick={() => setError(null)}>
            <X size={14} />
          </button>
        </div>
      )}

      {/* Hardware panel */}
      <div className="flash-hw">
        <div className="flash-hw__item">
          <span className="flash-hw__label">RAM</span>
          <span className="flash-hw__value">
            <MemoryStick size={14} style={{ marginRight: 4, verticalAlign: "middle" }} />
            {hw ? fmtMb(hw.total_ram_mb) : "Detecting..."}
          </span>
        </div>
        <div className="flash-hw__divider" />
        <div className="flash-hw__item">
          <span className="flash-hw__label">CPU</span>
          <span className="flash-hw__value">
            <Cpu size={14} style={{ marginRight: 4, verticalAlign: "middle" }} />
            {hw ? `${hw.cpu_cores} cores` : "..."}
          </span>
        </div>
        <div className="flash-hw__divider" />
        <div className="flash-hw__item">
          <span className="flash-hw__label">Storage</span>
          <span className="flash-hw__value">
            <HardDrive size={14} style={{ marginRight: 4, verticalAlign: "middle" }} />
            {hw ? hw.ssd_type : "..."}
          </span>
        </div>
        <div className="flash-hw__divider" />
        <div className="flash-hw__item">
          <span className="flash-hw__label">Available for inference</span>
          <span className="flash-hw__value">
            {hw ? fmtMb(hw.available_ram_mb) : "..."}
          </span>
        </div>
        {diskSpace !== null && (
          <>
            <div className="flash-hw__divider" />
            <div className="flash-hw__item">
              <span className="flash-hw__label">Disk free</span>
              <span className="flash-hw__value">{fmtBytes(diskSpace)}</span>
            </div>
          </>
        )}
      </div>

      {/* Active Downloads */}
      {activeDownloads.length > 0 && (
        <div className="flash-dl-active">
          {activeDownloads.map((dl) => (
            <div key={dl.model_name} className="flash-dl-item">
              <div className="flash-dl-item__header">
                <Loader size={14} className="flash-dl-item__spinner" />
                <span className="flash-dl-item__name">{dl.model_name}</span>
                <span className="flash-dl-item__status">{dlStatusLabel(dl.status)}</span>
                {dl.file_count > 1 && (
                  <span className="flash-dl-item__shard">
                    Part {dl.file_index}/{dl.file_count}
                  </span>
                )}
              </div>
              <div className="flash-dl-item__bar-bg">
                <div
                  className="flash-dl-item__bar-fill"
                  style={{ width: `${Math.min(dl.percent, 100)}%` }}
                />
              </div>
              <div className="flash-dl-item__stats">
                <span>{dl.percent.toFixed(1)}%</span>
                <span>{fmtBytes(dl.bytes_downloaded)} / {fmtBytes(dl.total_bytes)}</span>
                <span>{dl.speed_mb_per_sec.toFixed(1)} MB/s</span>
                {dl.eta_seconds > 0 && <span>ETA: {fmtEta(dl.eta_seconds)}</span>}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Model Library / Browser */}
      <div className="flash-model">
        <div className="flash-model__tabs">
          <button
            className={`flash-model__tab${modelTab === "browse" ? " flash-model__tab--active" : ""}`}
            onClick={() => setModelTab("browse")}
          >
            <Search size={14} /> Browse Models
          </button>
          <button
            className={`flash-model__tab${modelTab === "local" ? " flash-model__tab--active" : ""}`}
            onClick={() => { setModelTab("local"); refreshLocalModels(); }}
          >
            <Package size={14} /> My Models
            {localModels.length > 0 && (
              <span className="flash-model__tab-badge">{localModels.length}</span>
            )}
          </button>
        </div>

        {/* Browse tab: catalog recommendations + search */}
        {modelTab === "browse" && (
          <div className="flash-model__browse">
            <div className="flash-model__search-row">
              <input
                className="flash-model__path-input"
                type="text"
                placeholder="Search models (e.g. Llama, Qwen, code, vision)..."
                value={catalogQuery}
                onChange={(e) => setCatalogQuery(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") handleCatalogSearch(); }}
              />
              <button className="flash-model__browse-btn" onClick={handleCatalogSearch}>
                <Search size={16} /> Search
              </button>
            </div>

            {catalogRecs.length === 0 && (
              <div className="flash-model__empty">No models found. Try a different search.</div>
            )}

            <div className="flash-catalog-list">
              {catalogRecs.slice(0, 20).map((rec) => {
                const key = `${rec.entry.huggingface_id}-${rec.best_quant.quant_type}`;
                const isExpanded = expandedRec === key;
                const dlState = Object.values(downloadProgress).find(
                  (p) => p.model_name.includes(rec.entry.name.replace(/[^a-zA-Z0-9._-]/g, "-")),
                );
                const isDownloading = dlState && typeof dlState.status === "string" &&
                  (dlState.status === "Starting" || dlState.status === "Downloading");
                const isDownloaded = localModels.some((m) =>
                  m.name.includes(rec.entry.name.replace(/[^a-zA-Z0-9._-]/g, "-")),
                );

                return (
                  <div key={key} className="flash-catalog-card">
                    <div
                      className="flash-catalog-card__header"
                      onClick={() => setExpandedRec(isExpanded ? null : key)}
                    >
                      <div className="flash-catalog-card__info">
                        <span className="flash-catalog-card__name">{rec.entry.name}</span>
                        <span className="flash-catalog-card__meta">
                          {rec.entry.provider} &middot; {rec.best_quant.quant_type} &middot;{" "}
                          {rec.best_quant.file_size_gb.toFixed(1)} GB &middot;{" "}
                          {rec.entry.license}
                        </span>
                      </div>
                      <div className="flash-catalog-card__actions">
                        {rec.estimated_tok_per_sec > 0 && (
                          <span className="flash-catalog-card__speed">
                            ~{fmtNum(rec.estimated_tok_per_sec)} tok/s
                          </span>
                        )}
                        {isDownloaded ? (
                          <span className="flash-catalog-card__done">
                            <Check size={14} /> Downloaded
                          </span>
                        ) : isDownloading ? (
                          <span className="flash-catalog-card__downloading">
                            <Loader size={14} className="flash-dl-item__spinner" /> Downloading...
                          </span>
                        ) : (
                          <button
                            className="flash-catalog-card__dl-btn"
                            onClick={(e) => { e.stopPropagation(); handleDownload(rec); }}
                          >
                            <Download size={14} /> Download
                          </button>
                        )}
                        {isExpanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
                      </div>
                    </div>

                    {isExpanded && (
                      <div className="flash-catalog-card__details">
                        <div className="flash-model__info">
                          <div className="flash-model__info-item">
                            <span className="flash-model__info-label">Parameters</span>
                            <span className="flash-model__info-value">
                              {(rec.entry.total_params / 1e9).toFixed(1)}B
                            </span>
                          </div>
                          {rec.entry.is_moe && (
                            <div className="flash-model__info-item">
                              <span className="flash-model__info-label">MoE Experts</span>
                              <span className="flash-model__info-value">
                                {rec.entry.num_experts ?? "?"}
                              </span>
                            </div>
                          )}
                          {rec.entry.active_params && (
                            <div className="flash-model__info-item">
                              <span className="flash-model__info-label">Active Params</span>
                              <span className="flash-model__info-value">
                                {(rec.entry.active_params / 1e9).toFixed(1)}B
                              </span>
                            </div>
                          )}
                          <div className="flash-model__info-item">
                            <span className="flash-model__info-label">Min RAM</span>
                            <span className="flash-model__info-value">
                              {rec.best_quant.min_ram_gb.toFixed(1)} GB
                            </span>
                          </div>
                          <div className="flash-model__info-item">
                            <span className="flash-model__info-label">Quality</span>
                            <span className="flash-model__info-value">
                              {(rec.best_quant.quality_rating * 100).toFixed(0)}%
                            </span>
                          </div>
                          <div className="flash-model__info-item">
                            <span className="flash-model__info-label">Specialization</span>
                            <span className="flash-model__info-value">
                              {rec.entry.specialization}
                            </span>
                          </div>
                          <div className="flash-model__info-item">
                            <span className="flash-model__info-label">HuggingFace</span>
                            <span className="flash-model__info-value" style={{ fontSize: "0.75rem" }}>
                              {rec.entry.huggingface_id}
                            </span>
                          </div>
                        </div>
                        {rec.reason && (
                          <div className="flash-catalog-card__reason">{rec.reason}</div>
                        )}
                        {/* Quant variants */}
                        {rec.entry.available_quants.length > 1 && (
                          <div className="flash-catalog-card__quants">
                            <span className="flash-model__info-label" style={{ marginBottom: 4 }}>
                              Available quantizations:
                            </span>
                            <div className="flash-catalog-card__quant-list">
                              {rec.entry.available_quants.map((q) => (
                                <span
                                  key={q.quant_type}
                                  className={`flash-catalog-card__quant-chip${
                                    q.quant_type === rec.best_quant.quant_type
                                      ? " flash-catalog-card__quant-chip--best"
                                      : ""
                                  }`}
                                >
                                  {q.quant_type} ({q.file_size_gb.toFixed(1)}GB)
                                </span>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* Local tab: downloaded models */}
        {modelTab === "local" && (
          <div className="flash-model__local">
            {localModels.length === 0 ? (
              <div className="flash-model__empty">
                No downloaded models. Browse the catalog to download one.
              </div>
            ) : (
              <div className="flash-local-list">
                {localModels.map((model) => (
                  <div
                    key={model.name}
                    className={`flash-local-card${
                      modelPath === model.file_path ? " flash-local-card--selected" : ""
                    }`}
                  >
                    <div
                      className="flash-local-card__info"
                      onClick={() => selectLocalModel(model)}
                    >
                      <span className="flash-local-card__name">{model.name}</span>
                      <span className="flash-local-card__meta">
                        {model.file_size_display} &middot; {model.quant_type}
                        {model.downloaded_at && (
                          <> &middot; {new Date(model.downloaded_at).toLocaleDateString()}</>
                        )}
                      </span>
                    </div>
                    <div className="flash-local-card__actions">
                      <button
                        className="flash-local-card__select-btn"
                        onClick={() => selectLocalModel(model)}
                        disabled={!!sessionId}
                      >
                        <Zap size={14} /> Use
                      </button>
                      <button
                        className="flash-local-card__delete-btn"
                        onClick={() => handleDelete(model.name)}
                        disabled={deletingModel === model.name || modelPath === model.file_path}
                      >
                        {deletingModel === model.name ? (
                          <Loader size={14} className="flash-dl-item__spinner" />
                        ) : (
                          <Trash2 size={14} />
                        )}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Model Selection & Config (when a model is selected) */}
      {modelPath && (
        <div className="flash-model">
          <h3 className="flash-model__section-label">Selected Model</h3>

          <div className="flash-model__input-row">
            <input
              className="flash-model__path-input"
              type="text"
              placeholder="Path to GGUF model file..."
              value={modelPath}
              onChange={(e) => setModelPath(e.target.value)}
              onBlur={() => profileModel(modelPath)}
              disabled={!!sessionId}
            />
            {isDesktop && !sessionId && (
              <button className="flash-model__browse-btn" onClick={handleBrowse}>
                <FolderOpen size={16} /> Browse
              </button>
            )}
            {!sessionId && modelPath && !profile && (
              <button className="flash-model__browse-btn" onClick={() => profileModel(modelPath)}>
                <Gauge size={16} /> Profile
              </button>
            )}
          </div>

          {/* Model info */}
          {profile && (
            <div className="flash-model__info">
              <div className="flash-model__info-item">
                <span className="flash-model__info-label">Model</span>
                <span className="flash-model__info-value">{profile.name}</span>
              </div>
              <div className="flash-model__info-item">
                <span className="flash-model__info-label">Size</span>
                <span className="flash-model__info-value">{fmtMb(profile.file_size_mb)}</span>
              </div>
              <div className="flash-model__info-item">
                <span className="flash-model__info-label">Parameters</span>
                <span className="flash-model__info-value">{fmtNum(profile.parameters_b)}B</span>
              </div>
              <div className="flash-model__info-item">
                <span className="flash-model__info-label">Quantization</span>
                <span className="flash-model__info-value">{profile.quantization}</span>
              </div>
              {profile.is_moe && (
                <>
                  <div className="flash-model__info-item">
                    <span className="flash-model__info-label">Experts</span>
                    <span className="flash-model__info-value">{profile.num_experts}</span>
                  </div>
                  <div className="flash-model__info-item">
                    <span className="flash-model__info-label">Active</span>
                    <span className="flash-model__info-value">{fmtNum(profile.active_parameters_b)}B</span>
                  </div>
                </>
              )}
              <div className="flash-model__info-item">
                <span className="flash-model__info-label">Max Context</span>
                <span className="flash-model__info-value">
                  {(profile.max_context_length / 1024).toFixed(0)}K
                </span>
              </div>
            </div>
          )}

          {/* Performance estimate */}
          {estimate && (
            <div className="flash-estimate">
              <div className="flash-estimate__title">
                <Zap size={14} /> Performance Estimate
              </div>
              <div className="flash-estimate__row">
                <div className="flash-estimate__stat">
                  <span className="flash-estimate__stat-val">~{fmtNum(estimate.tokens_per_second)}</span>
                  <span className="flash-estimate__stat-unit">tok/s</span>
                </div>
                <div className="flash-estimate__stat">
                  <span className="flash-estimate__stat-val">{fmtMb(estimate.estimated_ram_mb)}</span>
                  <span className="flash-estimate__stat-unit">RAM</span>
                </div>
                <div className="flash-estimate__stat">
                  <span className="flash-estimate__stat-val">{(estimate.cache_hit_rate * 100).toFixed(0)}%</span>
                  <span className="flash-estimate__stat-unit">cache</span>
                </div>
                <div className="flash-estimate__stat">
                  <span className="flash-estimate__stat-val">{(estimate.max_context_at_budget / 1024).toFixed(0)}K</span>
                  <span className="flash-estimate__stat-unit">ctx</span>
                </div>
              </div>
              {estimate.needs_disk_streaming && (
                <div className="flash-estimate__warning">
                  <AlertTriangle size={14} /> Will use disk streaming (NVMe recommended)
                </div>
              )}
            </div>
          )}

          {/* Priority selector */}
          {profile && !sessionId && (
            <div className="flash-priority">
              <span className="flash-priority__label">Priority:</span>
              {(["speed", "balanced", "context"] as Priority[]).map((p) => (
                <button
                  key={p}
                  className={`flash-priority__btn${priority === p ? " flash-priority__btn--active" : ""}`}
                  onClick={() => setPriority(p)}
                >
                  {p.charAt(0).toUpperCase() + p.slice(1)}
                </button>
              ))}
            </div>
          )}

          {/* Load / Unload button */}
          {!sessionId ? (
            <button
              className="flash-load-btn flash-load-btn--load"
              disabled={!modelPath.trim() || isLoading}
              onClick={handleLoad}
            >
              {isLoading && <span className="flash-load-btn__spinner" />}
              {isLoading ? "Loading Model..." : "Load Model"}
            </button>
          ) : (
            <button className="flash-load-btn flash-load-btn--unload" onClick={handleUnload}>
              Unload Model
            </button>
          )}
        </div>
      )}

      {/* Chat */}
      <div className="flash-chat">
        <div className="flash-chat__header">
          <span className="flash-chat__header-label">Chat</span>
          {messages.length > 0 && (
            <button
              className="flash-chat__clear-btn"
              onClick={() => { setMessages([]); setStreamingText(""); }}
            >
              <Trash2 size={12} /> Clear
            </button>
          )}
        </div>

        <div className="flash-chat__messages">
          {messages.length === 0 && !streamingText && (
            <div className="flash-chat__empty">
              <Zap size={40} />
              <span className="flash-chat__empty-text">
                {sessionId
                  ? "Model loaded. Type a message to begin."
                  : "Load a model to start chatting."}
              </span>
            </div>
          )}
          {messages.map((msg, i) => (
            <div key={i} className={`flash-msg flash-msg--${msg.role}`}>
              {msg.content}
            </div>
          ))}
          {streamingText && (
            <div className="flash-msg flash-msg--assistant">
              {streamingText}
              <span className="flash-msg__cursor" />
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>

        <div className="flash-chat__input-row">
          <input
            className="flash-chat__input"
            type="text"
            placeholder={sessionId ? "Type a message..." : "Load a model first..."}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={!sessionId || isGenerating}
          />
          {isGenerating ? (
            <button className="flash-chat__stop-btn" onClick={() => setIsGenerating(false)}>
              <Square size={14} /> Stop
            </button>
          ) : (
            <button
              className="flash-chat__send-btn"
              disabled={!sessionId || !draft.trim()}
              onClick={handleSend}
            >
              <Send size={14} /> Send
            </button>
          )}
        </div>
      </div>

      {/* Live Metrics */}
      {sessionId && metrics && (
        <div className="flash-metrics">
          <div className="flash-metrics__item">
            <span className="flash-metrics__label">Tokens/sec</span>
            <span className="flash-metrics__value">{fmtNum(metrics.tokens_per_second)}</span>
          </div>
          <div className="flash-metrics__divider" />
          <div className="flash-metrics__item">
            <span className="flash-metrics__label">Memory</span>
            <span className={`flash-metrics__value${metrics.memory_utilization > 0.9 ? " flash-metrics__value--danger" : ""}`}>
              {fmtMb(metrics.memory_used_mb)} / {fmtMb(metrics.memory_budget_mb)}
            </span>
          </div>
          <div className="flash-metrics__divider" />
          <div className="flash-metrics__item">
            <span className="flash-metrics__label">Cache Hit</span>
            <span className="flash-metrics__value">{(metrics.expert_cache_hit_rate * 100).toFixed(1)}%</span>
          </div>
          <div className="flash-metrics__divider" />
          <div className="flash-metrics__item">
            <span className="flash-metrics__label">I/O</span>
            <span className="flash-metrics__value">{fmtNum(metrics.io_read_mb_per_sec)} GB/s</span>
          </div>
          <div className="flash-metrics__divider" />
          <div className="flash-metrics__item">
            <span className="flash-metrics__label">Tokens</span>
            <span className="flash-metrics__value">{metrics.total_tokens_generated}</span>
          </div>
          <div className="flash-metrics__divider" />
          <div className="flash-metrics__item">
            <span className="flash-metrics__label">Context</span>
            <span className={`flash-metrics__value${
              metrics.context_max > 0 && metrics.context_used / metrics.context_max > 0.85 ? " flash-metrics__value--warn" : ""
            }`}>
              {metrics.context_used.toLocaleString()}
              {metrics.context_max > 0 ? ` / ${metrics.context_max.toLocaleString()}` : ""}
            </span>
          </div>
        </div>
      )}
    </section>
  );
}
