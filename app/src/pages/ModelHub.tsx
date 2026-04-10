import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  searchModels,
  getModelInfo,
  checkModelCompatibility,
  downloadModel,
  listLocalModels,
  listProviderModels,
  deleteLocalModel,
  getSystemSpecs,
  nexusLinkStatus,
  nexusLinkToggleSharing,
  nexusLinkAddPeer,
  nexusLinkRemovePeer,
  nexusLinkListPeers,
  nexusLinkSendModel,
  getActiveLlmProvider,
} from "../api/backend";
import type { ProviderModel } from "../types";

/* ── types ── */

interface HfModelFile {
  filename: string;
  size_bytes: number;
  quantization: string | null;
}

interface HfModelInfo {
  model_id: string;
  author: string;
  name: string;
  description: string;
  downloads: number;
  likes: number;
  tags: string[];
  last_modified: string;
  files: HfModelFile[];
}

interface ModelSearchResult {
  models: HfModelInfo[];
  total_count: number;
  query: string;
}

interface SystemCompatibility {
  total_ram_mb: number;
  available_ram_mb: number;
  can_run: boolean;
  recommended_quantization: string;
  warning: string | null;
}

interface DownloadProgress {
  model_id: string;
  filename: string;
  bytes_downloaded: number;
  total_bytes: number;
  percent: number;
  status: string | { Failed: string };
}

interface LocalModelConfig {
  model_id: string;
  model_path: string;
  quantization: string;
  max_context_length: number;
  recommended_tasks: string[];
  min_ram_mb: number;
}

interface InstalledModelEntry {
  key: string;
  title: string;
  subtitle: string;
  chips: string[];
  removable: boolean;
  deleteId?: string;
  readyLabel: string;
}

interface SystemSpecs {
  total_ram_mb: number;
  available_ram_mb: number;
  cpu_name: string;
  cpu_cores: number;
}

interface NexusLinkPeer {
  device_id: string;
  address: string;
  name: string;
  status?: string;
}

interface NexusLinkStatusInfo {
  sharing_enabled: boolean;
  peer_count: number;
  status: string;
}

interface ActiveProviderInfo {
  provider: string;
  model?: string;
  status?: string;
}

/* ── helpers ── */

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function quantBadge(quant: string | null): { label: string; color: string } {
  if (!quant) return { label: "Unknown", color: "#888" };
  const q = quant.toUpperCase();
  if (q.includes("Q4") || q.includes("Q3") || q.includes("Q2"))
    return { label: "Small / Fast", color: "#00ff9d" };
  if (q.includes("Q5")) return { label: "Balanced", color: "#60a5fa" };
  if (q.includes("Q6") || q.includes("Q8")) return { label: "High Quality", color: "#a78bfa" };
  if (q.includes("F16") || q.includes("F32"))
    return { label: "Maximum Quality", color: "#fb923c" };
  return { label: quant, color: "#888" };
}

function downloadStatusText(status: string | { Failed: string }): string {
  if (typeof status === "string") return status;
  if (status && typeof status === "object" && "Failed" in status) return "Failed";
  return "Unknown";
}

const QUICK_FILTERS = ["LLaMA", "Mistral", "Phi", "Gemma", "CodeLlama", "All"];

/* ── component ── */

export default function ModelHub() {
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<HfModelInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState<HfModelInfo | null>(null);
  const [selectedModelDetails, setSelectedModelDetails] = useState<HfModelInfo | null>(null);
  const [localModels, setLocalModels] = useState<LocalModelConfig[]>([]);
  const [ollamaModels, setOllamaModels] = useState<ProviderModel[]>([]);
  const [systemSpecs, setSystemSpecs] = useState<SystemSpecs | null>(null);
  const [compatibilityMap, setCompatibilityMap] = useState<Record<string, SystemCompatibility>>({});
  const [activeDownloads, setActiveDownloads] = useState<Record<string, DownloadProgress>>({});
  const [completedDownloads, setCompletedDownloads] = useState<Set<string>>(new Set());
  const [isSearching, setIsSearching] = useState(false);
  const [isLoadingDetails, setIsLoadingDetails] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
  const [localDiscoveryAttempted, setLocalDiscoveryAttempted] = useState(false);

  /* Nexus Link state */
  const [linkStatus, setLinkStatus] = useState<NexusLinkStatusInfo | null>(null);
  const [linkPeers, setLinkPeers] = useState<NexusLinkPeer[]>([]);
  const [linkLoading, setLinkLoading] = useState(false);
  const [linkError, setLinkError] = useState<string | null>(null);
  const [addPeerAddress, setAddPeerAddress] = useState("");
  const [addPeerName, setAddPeerName] = useState("");
  const [sendModelPeerAddress, setSendModelPeerAddress] = useState("");
  const [sendModelId, setSendModelId] = useState("");
  const [sendModelFilename, setSendModelFilename] = useState("");
  const [sendingModel, setSendingModel] = useState(false);

  /* Active LLM Provider state */
  const [activeProvider, setActiveProvider] = useState<ActiveProviderInfo | null>(null);
  const [providerLoading, setProviderLoading] = useState(false);

  const searchTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (searchTimeout.current) clearTimeout(searchTimeout.current);
      if (errorTimerRef.current) clearTimeout(errorTimerRef.current);
    };
  }, []);

  const showError = useCallback((msg: string) => {
    setError(msg);
    if (errorTimerRef.current) clearTimeout(errorTimerRef.current);
    errorTimerRef.current = setTimeout(() => setError(null), 5000);
  }, []);

  /* ── data loading ── */

  const loadLocalModels = useCallback(async () => {
    setLocalDiscoveryAttempted(true);
    try {
      const [raw, providerModels] = await Promise.all([
        listLocalModels(),
        listProviderModels(),
      ]);
      const models: LocalModelConfig[] = JSON.parse(raw);
      setLocalModels(models);
      setOllamaModels(
        providerModels.filter(
          (model) => model.local && model.provider === "ollama" && model.installed,
        ),
      );
    } catch (err) {
      if (import.meta.env.DEV) console.error("[ModelHub] failed to load installed models", err);
      setLocalModels([]);
      setOllamaModels([]);
    }
  }, []);

  const loadSystemSpecs = useCallback(async () => {
    try {
      const raw = await getSystemSpecs();
      setSystemSpecs(JSON.parse(raw));
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    loadLocalModels();
    loadSystemSpecs();
  }, [loadLocalModels, loadSystemSpecs]);

  /* ── Nexus Link data loading ── */

  const loadLinkStatus = useCallback(async () => {
    try {
      const raw = await nexusLinkStatus();
      const parsed: NexusLinkStatusInfo = JSON.parse(raw);
      setLinkStatus(parsed);
    } catch {
      setLinkStatus(null);
    }
  }, []);

  const loadLinkPeers = useCallback(async () => {
    try {
      const raw = await nexusLinkListPeers();
      const parsed: NexusLinkPeer[] = JSON.parse(raw);
      setLinkPeers(parsed);
    } catch {
      setLinkPeers([]);
    }
  }, []);

  const loadActiveProvider = useCallback(async () => {
    setProviderLoading(true);
    try {
      const raw = await getActiveLlmProvider();
      const parsed: ActiveProviderInfo = JSON.parse(raw);
      setActiveProvider(parsed);
    } catch {
      setActiveProvider(null);
    } finally {
      setProviderLoading(false);
    }
  }, []);

  useEffect(() => {
    loadLinkStatus();
    loadLinkPeers();
    loadActiveProvider();
  }, [loadLinkStatus, loadLinkPeers, loadActiveProvider]);

  /* ── Nexus Link actions ── */

  const handleToggleSharing = useCallback(async (enabled: boolean) => {
    setLinkLoading(true);
    setLinkError(null);
    try {
      await nexusLinkToggleSharing(enabled);
      await loadLinkStatus();
    } catch (e) {
      setLinkError(String(e));
    } finally {
      setLinkLoading(false);
    }
  }, [loadLinkStatus]);

  const handleAddPeer = useCallback(async () => {
    if (!addPeerAddress.trim() || !addPeerName.trim()) return;
    setLinkLoading(true);
    setLinkError(null);
    try {
      await nexusLinkAddPeer(addPeerAddress.trim(), addPeerName.trim());
      setAddPeerAddress("");
      setAddPeerName("");
      await loadLinkPeers();
      await loadLinkStatus();
    } catch (e) {
      setLinkError(String(e));
    } finally {
      setLinkLoading(false);
    }
  }, [addPeerAddress, addPeerName, loadLinkPeers, loadLinkStatus]);

  const handleRemovePeer = useCallback(async (deviceId: string) => {
    setLinkLoading(true);
    setLinkError(null);
    try {
      await nexusLinkRemovePeer(deviceId);
      await loadLinkPeers();
      await loadLinkStatus();
    } catch (e) {
      setLinkError(String(e));
    } finally {
      setLinkLoading(false);
    }
  }, [loadLinkPeers, loadLinkStatus]);

  const handleSendModel = useCallback(async () => {
    if (!sendModelPeerAddress.trim() || !sendModelId.trim() || !sendModelFilename.trim()) return;
    setSendingModel(true);
    setLinkError(null);
    try {
      await nexusLinkSendModel(
        sendModelPeerAddress.trim(),
        sendModelId.trim(),
        sendModelFilename.trim(),
      );
      setSendModelPeerAddress("");
      setSendModelId("");
      setSendModelFilename("");
    } catch (e) {
      setLinkError(String(e));
    } finally {
      setSendingModel(false);
    }
  }, [sendModelPeerAddress, sendModelId, sendModelFilename]);

  /* ── event listeners ── */

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenComplete: (() => void) | undefined;

    import("@tauri-apps/api/event").then((mod) => {
      mod
        .listen<DownloadProgress>("model-download-progress", (event) => {
          setActiveDownloads((prev) => ({
            ...prev,
            [event.payload.filename]: event.payload,
          }));
          const st = downloadStatusText(event.payload.status);
          if (st === "Completed") {
            setCompletedDownloads((prev) => new Set(prev).add(event.payload.filename));
            setActiveDownloads((prev) => {
              const next = { ...prev };
              delete next[event.payload.filename];
              return next;
            });
          }
          if (st === "Failed") {
            setActiveDownloads((prev) => {
              const next = { ...prev };
              delete next[event.payload.filename];
              return next;
            });
          }
        })
        .then((fn) => {
          unlistenProgress = fn;
        });

      mod
        .listen("model-download-complete", () => {
          loadLocalModels();
        })
        .then((fn) => {
          unlistenComplete = fn;
        });
    });

    return () => {
      unlistenProgress?.();
      unlistenComplete?.();
    };
  }, [loadLocalModels]);

  /* ── search ── */

  const doSearch = useCallback(
    async (query: string) => {
      if (!query.trim()) {
        setSearchResults([]);
        return;
      }
      setIsSearching(true);
      try {
        const raw = await searchModels(query, 20);
        const result: ModelSearchResult = JSON.parse(raw);
        setSearchResults(result.models);
      } catch (e) {
        showError(String(e));
        setSearchResults([]);
      } finally {
        setIsSearching(false);
      }
    },
    [showError],
  );

  const handleSearchInput = useCallback(
    (value: string) => {
      setSearchQuery(value);
      if (searchTimeout.current) clearTimeout(searchTimeout.current);
      searchTimeout.current = setTimeout(() => doSearch(value), 300);
    },
    [doSearch],
  );

  const handleFilterClick = useCallback(
    (filter: string) => {
      const q = filter === "All" ? "" : filter;
      setSearchQuery(q);
      if (q) doSearch(q);
      else setSearchResults([]);
    },
    [doSearch],
  );

  /* ── select model ── */

  const handleSelectModel = useCallback(
    async (model: HfModelInfo) => {
      setSelectedModel(model);
      setSelectedModelDetails(null);
      setCompatibilityMap({});
      setIsLoadingDetails(true);
      try {
        const raw = await getModelInfo(model.model_id);
        const details: HfModelInfo = JSON.parse(raw);
        setSelectedModelDetails(details);

        // Check compatibility for each file in parallel
        for (const file of details.files) {
          checkModelCompatibility(file.size_bytes)
            .then((cRaw) => {
              const compat: SystemCompatibility = JSON.parse(cRaw);
              setCompatibilityMap((prev) => ({ ...prev, [file.filename]: compat }));
            })
            .catch((e) => { if (import.meta.env.DEV) console.warn("[ModelHub]", e); });
        }
      } catch (e) {
        showError(String(e));
      } finally {
        setIsLoadingDetails(false);
      }
    },
    [showError],
  );

  /* ── download ── */

  const handleDownload = useCallback(
    async (modelId: string, filename: string) => {
      setActiveDownloads((prev) => ({
        ...prev,
        [filename]: {
          model_id: modelId,
          filename,
          bytes_downloaded: 0,
          total_bytes: 0,
          percent: 0,
          status: "Starting",
        },
      }));
      try {
        await downloadModel(modelId, filename);
      } catch (e) {
        showError(String(e));
        setActiveDownloads((prev) => {
          const next = { ...prev };
          delete next[filename];
          return next;
        });
      }
    },
    [showError],
  );

  /* ── delete ── */

  const handleDelete = useCallback(
    async (modelId: string) => {
      setDeleteConfirm(null);
      try {
        const raw = await deleteLocalModel(modelId);
        const result = JSON.parse(raw);
        if (result.deleted) {
          await loadLocalModels();
        } else {
          showError(result.error || "Failed to delete model");
        }
      } catch (e) {
        showError(String(e));
      }
    },
    [loadLocalModels, showError],
  );

  /* ── styles ── */

  const accent = "#00ff9d";
  const bgPage = "#0d0d1a";
  const bgPanel = "#141428";
  const bgCard = "#1a1a2e";
  const bgInput = "#0f0f1e";
  const borderColor = "#2a2a3e";
  const textPrimary = "#e0e0e0";
  const textSecondary = "#888";

  /* ── find recommended file ── */

  const recommendedFile = selectedModelDetails?.files.find((f) => {
    const compat = compatibilityMap[f.filename];
    return compat?.can_run && !compat.warning;
  });

  const installedModels = useMemo<InstalledModelEntry[]>(() => {
    const entries: InstalledModelEntry[] = [];
    const seen = new Set<string>();

    for (const model of ollamaModels) {
      const key = `ollama:${model.id}`;
      if (seen.has(key)) continue;
      seen.add(key);
      entries.push({
        key,
        title: model.name,
        subtitle: "Installed in Ollama",
        chips: [
          "Ollama",
          model.size_gb ? `${model.size_gb.toFixed(1)} GB` : "Local model",
        ],
        removable: false,
        readyLabel: "Ready",
      });
    }

    for (const model of localModels) {
      const key = `registry:${model.model_id}:${model.model_path}`;
      if (seen.has(key)) continue;
      seen.add(key);
      entries.push({
        key,
        title: model.model_id.split("/").pop() || model.model_id,
        subtitle: model.model_path,
        chips: [
          model.quantization,
          model.min_ram_mb >= 1024
            ? `${(model.min_ram_mb / 1024).toFixed(1)} GB RAM`
            : `${model.min_ram_mb} MB RAM`,
        ],
        removable: true,
        deleteId: model.model_id,
        readyLabel: "Downloaded",
      });
    }

    return entries;
  }, [localModels, ollamaModels]);

  /* ── sub-renders ── */

  const renderSearchPanel = () => (
    <div
      style={{
        width: "30%",
        display: "flex",
        flexDirection: "column",
        gap: 12,
        minHeight: 0,
      }}
    >
      {/* Search input */}
      <div style={{ position: "relative" }}>
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => handleSearchInput(e.target.value)}
          placeholder="Search GGUF models on HuggingFace..."
          style={{
            width: "100%",
            padding: "10px 12px 10px 34px",
            background: bgInput,
            color: textPrimary,
            border: `1px solid ${borderColor}`,
            borderRadius: 6,
            fontSize: 13,
            outline: "none",
            boxSizing: "border-box",
          }}
        />
        <span
          style={{
            position: "absolute",
            left: 10,
            top: "50%",
            transform: "translateY(-50%)",
            color: textSecondary,
            fontSize: 15,
          }}
          >
            {isSearching ? "\u23F3" : "\uD83D\uDD0D"}
          </span>
      </div>

      {/* Quick filters */}
      <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
        {QUICK_FILTERS.map((f) => (
          <button type="button"
            key={f}
            onClick={() => handleFilterClick(f)}
            style={{
              padding: "4px 10px",
              fontSize: 11,
              fontWeight: 600,
              border: `1px solid ${searchQuery === f || (f === "All" && !searchQuery) ? accent + "44" : borderColor}`,
              borderRadius: 4,
              background:
                searchQuery === f || (f === "All" && !searchQuery) ? `${accent}18` : bgCard,
              color:
                searchQuery === f || (f === "All" && !searchQuery) ? accent : textSecondary,
              cursor: "pointer",
            }}
          >
            {f}
          </button>
        ))}
      </div>

      {/* System specs bar */}
      {systemSpecs && (
        <div
          style={{
            padding: "6px 12px",
            background: bgCard,
            border: `1px solid ${borderColor}`,
            borderRadius: 6,
            fontSize: 11,
            color: textSecondary,
            display: "flex",
            alignItems: "center",
            gap: 6,
          }}
        >
          <span>{"\uD83D\uDCBB"}</span>
          <span style={{ color: textPrimary, fontWeight: 500 }}>{systemSpecs.cpu_name}</span>
          <span>|</span>
          <span style={{ color: accent, fontWeight: 600 }}>
            {Math.round(systemSpecs.total_ram_mb / 1024)} GB RAM
          </span>
        </div>
      )}

      {/* Results */}
      <div
        style={{
          flex: 1,
          overflow: "auto",
          background: bgPanel,
          borderRadius: 8,
          border: `1px solid ${borderColor}`,
        }}
      >
        <div
          style={{
            padding: "10px 14px",
            borderBottom: `1px solid ${borderColor}`,
            fontWeight: 600,
            fontSize: 13,
            color: textSecondary,
          }}
        >
          Search Results{searchResults.length > 0 && ` (${searchResults.length})`}
        </div>
        {searchResults.length === 0 ? (
          <div
            style={{
              padding: 20,
              textAlign: "center",
              color: textSecondary,
              fontSize: 13,
            }}
          >
            {isSearching
              ? "Searching HuggingFace..."
              : searchQuery
                ? "No models found"
                : "Search HuggingFace on the left to discover more models, or use the Installed Models panel to confirm what Ollama already has available."}
          </div>
        ) : (
          searchResults.map((model) => (
            <div
              key={model.model_id}
              onClick={() => handleSelectModel(model)}
              style={{
                padding: "10px 14px",
                borderBottom: `1px solid ${borderColor}`,
                cursor: "pointer",
                background:
                  selectedModel?.model_id === model.model_id ? `${accent}0d` : "transparent",
                borderLeft:
                  selectedModel?.model_id === model.model_id
                    ? `3px solid ${accent}`
                    : "3px solid transparent",
                transition: "all 0.15s",
              }}
            >
              <div
                style={{
                  fontWeight: 600,
                  fontSize: 13,
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
                title={model.model_id}
              >
                {model.name}
              </div>
              <div style={{ fontSize: 11, color: textSecondary, marginTop: 2 }}>
                {model.author}
              </div>
              <div
                style={{
                  display: "flex",
                  gap: 10,
                  marginTop: 4,
                  fontSize: 11,
                  color: textSecondary,
                }}
              >
                <span>{"\u2B07"} {formatNumber(model.downloads)}</span>
                <span>{"\u2764"} {formatNumber(model.likes)}</span>
              </div>
              {model.tags.length > 0 && (
                <div style={{ display: "flex", flexWrap: "wrap", gap: 3, marginTop: 5 }}>
                  {model.tags.slice(0, 5).map((tag) => (
                    <span
                      key={tag}
                      style={{
                        fontSize: 9,
                        padding: "1px 6px",
                        borderRadius: 3,
                        background: "#a78bfa22",
                        color: "#a78bfa",
                        fontWeight: 500,
                      }}
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );

  const renderDetailsPanel = () => (
    <div
      style={{
        width: "45%",
        display: "flex",
        flexDirection: "column",
        background: bgPanel,
        borderRadius: 8,
        border: `1px solid ${borderColor}`,
        minHeight: 0,
      }}
    >
      <div
        style={{
          padding: "12px 16px",
          borderBottom: `1px solid ${borderColor}`,
          fontWeight: 600,
          fontSize: 14,
        }}
      >
        Model Details
      </div>
      <div style={{ flex: 1, overflow: "auto", padding: 16 }}>
        {!selectedModel ? (
          <div
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexDirection: "column",
              gap: 8,
              color: textSecondary,
              height: "100%",
            }}
          >
            <div style={{ fontSize: 32 }}>{"\uD83E\uDDE0"}</div>
            <div>Search and select a model to view details</div>
          </div>
        ) : isLoadingDetails ? (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: textSecondary,
              height: "100%",
            }}
          >
            Loading model details...
          </div>
        ) : selectedModelDetails ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {/* Header */}
            <div>
              <div style={{ fontSize: 18, fontWeight: 700, color: textPrimary }}>
                {selectedModelDetails.name}
              </div>
              <div style={{ fontSize: 12, color: textSecondary, marginTop: 2 }}>
                by {selectedModelDetails.author}
              </div>
              {selectedModelDetails.description && (
                <div
                  style={{
                    fontSize: 12,
                    color: textSecondary,
                    marginTop: 8,
                    lineHeight: 1.5,
                  }}
                >
                  {selectedModelDetails.description}
                </div>
              )}
              <div
                style={{
                  display: "flex",
                  gap: 12,
                  marginTop: 8,
                  fontSize: 12,
                }}
              >
                <span
                  style={{
                    padding: "2px 10px",
                    borderRadius: 10,
                    background: `${accent}22`,
                    color: accent,
                    fontWeight: 600,
                  }}
                >
                  {"\u2B07"} {formatNumber(selectedModelDetails.downloads)}
                </span>
                <span
                  style={{
                    padding: "2px 10px",
                    borderRadius: 10,
                    background: "#f472b622",
                    color: "#f472b6",
                    fontWeight: 600,
                  }}
                >
                  {"\u2764"} {formatNumber(selectedModelDetails.likes)}
                </span>
              </div>
            </div>

            {/* Files section */}
            <div>
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  color: textSecondary,
                  textTransform: "uppercase",
                  letterSpacing: 1,
                  marginBottom: 10,
                }}
              >
                Available Files ({selectedModelDetails.files.length})
              </div>
              {selectedModelDetails.files.length === 0 ? (
                <div style={{ fontSize: 12, color: textSecondary }}>
                  No GGUF files found for this model
                </div>
              ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {selectedModelDetails.files.map((file) => {
                    const compat = compatibilityMap[file.filename];
                    const badge = quantBadge(file.quantization);
                    const isRecommended = recommendedFile?.filename === file.filename;
                    const download = activeDownloads[file.filename];
                    const isCompleted = completedDownloads.has(file.filename);
                    const isLocallyAvailable = localModels.some(
                      (m) =>
                        m.model_id === selectedModelDetails.model_id &&
                        m.model_path.includes(file.filename.replace(".gguf", "")),
                    );

                    return (
                      <div
                        key={file.filename}
                        style={{
                          padding: "10px 14px",
                          background: bgCard,
                          border: `1px solid ${isRecommended ? accent + "44" : borderColor}`,
                          borderRadius: 8,
                          position: "relative",
                        }}
                      >
                        {isRecommended && (
                          <div
                            style={{
                              position: "absolute",
                              top: -8,
                              right: 10,
                              fontSize: 9,
                              padding: "1px 8px",
                              background: accent,
                              color: "#0d0d1a",
                              borderRadius: 3,
                              fontWeight: 700,
                            }}
                          >
                            RECOMMENDED
                          </div>
                        )}
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "space-between",
                            gap: 8,
                          }}
                        >
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <div
                              style={{
                                fontWeight: 500,
                                fontSize: 12,
                                whiteSpace: "nowrap",
                                overflow: "hidden",
                                textOverflow: "ellipsis",
                              }}
                              title={file.filename}
                            >
                              {file.filename}
                            </div>
                            <div
                              style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                                marginTop: 4,
                              }}
                            >
                              <span style={{ fontSize: 11, color: textSecondary }}>
                                {formatBytes(file.size_bytes)}
                              </span>
                              {file.quantization && (
                                <span
                                  style={{
                                    fontSize: 10,
                                    padding: "1px 6px",
                                    borderRadius: 3,
                                    background: badge.color + "22",
                                    color: badge.color,
                                    fontWeight: 600,
                                  }}
                                >
                                  {file.quantization} — {badge.label}
                                </span>
                              )}
                            </div>
                          </div>

                          {/* Compatibility */}
                          <div
                            style={{
                              textAlign: "center",
                              fontSize: 11,
                              minWidth: 80,
                            }}
                          >
                            {!compat ? (
                              <span style={{ color: textSecondary }}>{"\u23F3"}</span>
                            ) : compat.can_run && !compat.warning ? (
                              <span style={{ color: accent }}>{"\u2705"} Compatible</span>
                            ) : compat.can_run && compat.warning ? (
                              <span style={{ color: "#fbbf24" }}>
                                {"\u26A0\uFE0F"} Tight fit
                              </span>
                            ) : (
                              <div>
                                <div style={{ color: "#f87171" }}>{"\u274C"} Too large</div>
                                <div style={{ fontSize: 9, color: textSecondary, marginTop: 2 }}>
                                  Try {compat.recommended_quantization}
                                </div>
                              </div>
                            )}
                          </div>

                          {/* Download button / progress */}
                          <div style={{ minWidth: 100, textAlign: "right" }}>
                            {isCompleted || isLocallyAvailable ? (
                              <span
                                style={{
                                  fontSize: 11,
                                  color: accent,
                                  fontWeight: 600,
                                }}
                              >
                                Downloaded {"\u2713"}
                              </span>
                            ) : download ? (
                              <div style={{ width: 100 }}>
                                <div
                                  style={{
                                    height: 6,
                                    background: borderColor,
                                    borderRadius: 3,
                                    overflow: "hidden",
                                  }}
                                >
                                  <div
                                    style={{
                                      height: "100%",
                                      width: `${Math.min(download.percent, 100)}%`,
                                      background: accent,
                                      borderRadius: 3,
                                      transition: "width 0.3s",
                                    }}
                                  />
                                </div>
                                <div
                                  style={{
                                    fontSize: 10,
                                    color: textSecondary,
                                    marginTop: 3,
                                    textAlign: "center",
                                  }}
                                >
                                  {download.percent.toFixed(1)}%{" "}
                                  {download.total_bytes > 0 &&
                                    `${formatBytes(download.bytes_downloaded)} / ${formatBytes(download.total_bytes)}`}
                                </div>
                              </div>
                            ) : (
                              <button type="button"
                                onClick={() =>
                                  handleDownload(
                                    selectedModelDetails.model_id,
                                    file.filename,
                                  )
                                }
                                style={{
                                  padding: "5px 14px",
                                  fontSize: 11,
                                  fontWeight: 600,
                                  border: `1px solid ${accent}44`,
                                  borderRadius: 5,
                                  background: `${accent}18`,
                                  color: accent,
                                  cursor: "pointer",
                                }}
                              >
                                Download
                              </button>
                            )}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Tags */}
            {selectedModelDetails.tags.length > 0 && (
              <div>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 700,
                    color: textSecondary,
                    textTransform: "uppercase",
                    letterSpacing: 1,
                    marginBottom: 8,
                  }}
                >
                  Tags
                </div>
                <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
                  {selectedModelDetails.tags.map((tag) => (
                    <span
                      key={tag}
                      style={{
                        fontSize: 10,
                        padding: "2px 8px",
                        borderRadius: 3,
                        background: "#a78bfa22",
                        color: "#a78bfa",
                        fontWeight: 500,
                      }}
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );

  const renderLocalPanel = () => (
    <div
      style={{
        width: "25%",
        display: "flex",
        flexDirection: "column",
        background: bgPanel,
        borderRadius: 8,
        border: `1px solid ${borderColor}`,
        minHeight: 0,
      }}
    >
      <div
        style={{
          padding: "12px 14px",
          borderBottom: `1px solid ${borderColor}`,
          fontWeight: 600,
          fontSize: 13,
          color: textSecondary,
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        Installed Models
        {installedModels.length > 0 && (
          <span
            style={{
              fontSize: 10,
              padding: "1px 8px",
              borderRadius: 10,
              background: `${accent}22`,
              color: accent,
              fontWeight: 600,
            }}
          >
            {installedModels.length}
          </span>
        )}
      </div>
      <div style={{ flex: 1, overflow: "auto" }}>
        {installedModels.length === 0 ? (
          <div
            style={{
              padding: 20,
              textAlign: "center",
              color: textSecondary,
              fontSize: 12,
              lineHeight: 1.6,
            }}
          >
            {localDiscoveryAttempted
              ? "No local models detected yet. This page discovers installed Ollama models automatically and also shows downloaded GGUF models. Make sure Ollama is running if you expect local models here."
              : "Discovering local models..."}
          </div>
        ) : (
          installedModels.map((model) => (
            <div
              key={model.key}
              style={{
                padding: "10px 14px",
                borderBottom: `1px solid ${borderColor}`,
              }}
            >
              <div
                style={{
                  fontWeight: 600,
                  fontSize: 12,
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
                title={model.title}
              >
                {model.title}
              </div>
              <div
                style={{
                  fontSize: 10,
                  color: textSecondary,
                  marginTop: 2,
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
                title={model.subtitle}
              >
                {model.subtitle}
              </div>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  marginTop: 4,
                  flexWrap: "wrap",
                }}
              >
                {model.chips.map((chip) => (
                  <span
                    key={chip}
                    style={{
                      fontSize: 10,
                      padding: "1px 6px",
                      borderRadius: 3,
                      background: `${accent}18`,
                      color: chip === "Ollama" ? accent : textSecondary,
                      fontWeight: chip === "Ollama" ? 600 : 500,
                    }}
                  >
                    {chip}
                  </span>
                ))}
              </div>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  marginTop: 6,
                }}
              >
                <span
                  style={{
                    fontSize: 10,
                    padding: "1px 8px",
                    borderRadius: 10,
                    background: `${accent}22`,
                    color: accent,
                    fontWeight: 600,
                  }}
                >
                  {model.readyLabel}
                </span>
                {model.removable && deleteConfirm === model.deleteId ? (
                  <div style={{ display: "flex", gap: 4 }}>
                    <button type="button"
                      onClick={() => handleDelete(model.deleteId!)}
                      style={{
                        padding: "2px 8px",
                        fontSize: 10,
                        fontWeight: 600,
                        border: "1px solid #f8717144",
                        borderRadius: 3,
                        background: "#f8717122",
                        color: "#f87171",
                        cursor: "pointer",
                      }}
                    >
                      Confirm
                    </button>
                    <button type="button"
                      onClick={() => setDeleteConfirm(null)}
                      style={{
                        padding: "2px 8px",
                        fontSize: 10,
                        fontWeight: 600,
                        border: `1px solid ${borderColor}`,
                        borderRadius: 3,
                        background: bgCard,
                        color: textSecondary,
                        cursor: "pointer",
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                ) : model.removable ? (
                  <button type="button"
                    onClick={() => setDeleteConfirm(model.deleteId!)}
                    style={{
                      background: "none",
                      border: "none",
                      color: "#f87171",
                      cursor: "pointer",
                      fontSize: 14,
                      padding: "2px 6px",
                      borderRadius: 4,
                      lineHeight: 1,
                    }}
                    title="Delete model"
                  >
                    {"\uD83D\uDDD1"}
                  </button>
                ) : (
                  <span style={{ fontSize: 10, color: textSecondary }}>
                    Managed by Ollama
                  </span>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );

  /* ── Nexus Link + Provider panels ── */

  const renderNexusLinkPanel = () => (
    <div
      style={{
        background: bgPanel,
        borderRadius: 8,
        border: `1px solid ${borderColor}`,
        padding: 16,
        display: "flex",
        flexDirection: "column",
        gap: 14,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ fontWeight: 700, fontSize: 14, color: textPrimary }}>
          Nexus Link
        </div>
        {linkStatus && (
          <span
            style={{
              fontSize: 10,
              padding: "2px 10px",
              borderRadius: 10,
              background: linkStatus.sharing_enabled ? `${accent}22` : "#f8717122",
              color: linkStatus.sharing_enabled ? accent : "#f87171",
              fontWeight: 600,
            }}
          >
            {linkStatus.sharing_enabled ? "Sharing On" : "Sharing Off"}
          </span>
        )}
      </div>

      {linkError && (
        <div
          style={{
            padding: "6px 10px",
            background: "#ff444422",
            border: "1px solid #ff444466",
            borderRadius: 4,
            color: "#ff6666",
            fontSize: 11,
          }}
        >
          {linkError}
        </div>
      )}

      {/* Status display */}
      {linkStatus && (
        <div
          style={{
            padding: "8px 12px",
            background: bgCard,
            border: `1px solid ${borderColor}`,
            borderRadius: 6,
            fontSize: 12,
            display: "flex",
            flexDirection: "column",
            gap: 4,
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: textSecondary }}>Status</span>
            <span style={{ color: textPrimary, fontWeight: 500 }}>{linkStatus.status}</span>
          </div>
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: textSecondary }}>Connected Peers</span>
            <span style={{ color: accent, fontWeight: 600 }}>{linkStatus.peer_count}</span>
          </div>
        </div>
      )}

      {/* Toggle sharing */}
      <div style={{ display: "flex", gap: 8 }}>
        <button type="button"
          onClick={() => handleToggleSharing(true)}
          disabled={linkLoading || linkStatus?.sharing_enabled === true}
          style={{
            flex: 1,
            padding: "6px 0",
            fontSize: 11,
            fontWeight: 600,
            border: `1px solid ${accent}44`,
            borderRadius: 5,
            background: linkStatus?.sharing_enabled ? `${accent}22` : bgCard,
            color: linkStatus?.sharing_enabled ? accent : textSecondary,
            cursor: linkLoading ? "wait" : "pointer",
            opacity: linkLoading ? 0.6 : 1,
          }}
        >
          Enable Sharing
        </button>
        <button type="button"
          onClick={() => handleToggleSharing(false)}
          disabled={linkLoading || linkStatus?.sharing_enabled === false}
          style={{
            flex: 1,
            padding: "6px 0",
            fontSize: 11,
            fontWeight: 600,
            border: `1px solid #f8717144`,
            borderRadius: 5,
            background: !linkStatus?.sharing_enabled ? "#f8717122" : bgCard,
            color: !linkStatus?.sharing_enabled ? "#f87171" : textSecondary,
            cursor: linkLoading ? "wait" : "pointer",
            opacity: linkLoading ? 0.6 : 1,
          }}
        >
          Disable Sharing
        </button>
      </div>

      {/* Peer list */}
      <div>
        <div
          style={{
            fontSize: 11,
            fontWeight: 700,
            color: textSecondary,
            textTransform: "uppercase",
            letterSpacing: 1,
            marginBottom: 6,
          }}
        >
          Peers ({linkPeers.length})
        </div>
        {linkPeers.length === 0 ? (
          <div style={{ fontSize: 11, color: textSecondary, padding: "4px 0" }}>
            No peers connected. Add one below.
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {linkPeers.map((peer) => (
              <div
                key={peer.device_id}
                style={{
                  padding: "8px 10px",
                  background: bgCard,
                  border: `1px solid ${borderColor}`,
                  borderRadius: 6,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                }}
              >
                <div>
                  <div style={{ fontSize: 12, fontWeight: 600, color: textPrimary }}>
                    {peer.name}
                  </div>
                  <div style={{ fontSize: 10, color: textSecondary, marginTop: 1 }}>
                    {peer.address}
                  </div>
                  {peer.status && (
                    <div style={{ fontSize: 10, color: accent, marginTop: 1 }}>{peer.status}</div>
                  )}
                </div>
                <button type="button"
                  onClick={() => handleRemovePeer(peer.device_id)}
                  disabled={linkLoading}
                  style={{
                    background: "none",
                    border: "none",
                    color: "#f87171",
                    cursor: linkLoading ? "wait" : "pointer",
                    fontSize: 13,
                    padding: "2px 6px",
                    borderRadius: 4,
                    lineHeight: 1,
                  }}
                  title="Remove peer"
                >
                  {"\u2715"}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Add peer form */}
      <div>
        <div
          style={{
            fontSize: 11,
            fontWeight: 700,
            color: textSecondary,
            textTransform: "uppercase",
            letterSpacing: 1,
            marginBottom: 6,
          }}
        >
          Add Peer
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <input
            type="text"
            value={addPeerAddress}
            onChange={(e) => setAddPeerAddress(e.target.value)}
            placeholder="Peer address (e.g. 192.168.1.100:9090)"
            style={{
              width: "100%",
              padding: "7px 10px",
              background: bgInput,
              color: textPrimary,
              border: `1px solid ${borderColor}`,
              borderRadius: 5,
              fontSize: 12,
              outline: "none",
              boxSizing: "border-box",
            }}
          />
          <input
            type="text"
            value={addPeerName}
            onChange={(e) => setAddPeerName(e.target.value)}
            placeholder="Peer name (e.g. Office Desktop)"
            style={{
              width: "100%",
              padding: "7px 10px",
              background: bgInput,
              color: textPrimary,
              border: `1px solid ${borderColor}`,
              borderRadius: 5,
              fontSize: 12,
              outline: "none",
              boxSizing: "border-box",
            }}
          />
          <button type="button"
            onClick={handleAddPeer}
            disabled={linkLoading || !addPeerAddress.trim() || !addPeerName.trim()}
            style={{
              padding: "7px 0",
              fontSize: 12,
              fontWeight: 600,
              border: `1px solid ${accent}44`,
              borderRadius: 5,
              background: `${accent}18`,
              color: accent,
              cursor: linkLoading ? "wait" : "pointer",
              opacity: !addPeerAddress.trim() || !addPeerName.trim() ? 0.5 : 1,
            }}
          >
            {linkLoading ? "Adding..." : "Add Peer"}
          </button>
        </div>
      </div>

      {/* Send model form */}
      <div>
        <div
          style={{
            fontSize: 11,
            fontWeight: 700,
            color: textSecondary,
            textTransform: "uppercase",
            letterSpacing: 1,
            marginBottom: 6,
          }}
        >
          Send Model to Peer
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <input
            type="text"
            value={sendModelPeerAddress}
            onChange={(e) => setSendModelPeerAddress(e.target.value)}
            placeholder="Peer address"
            style={{
              width: "100%",
              padding: "7px 10px",
              background: bgInput,
              color: textPrimary,
              border: `1px solid ${borderColor}`,
              borderRadius: 5,
              fontSize: 12,
              outline: "none",
              boxSizing: "border-box",
            }}
          />
          <input
            type="text"
            value={sendModelId}
            onChange={(e) => setSendModelId(e.target.value)}
            placeholder="Model ID (e.g. TheBloke/Llama-2-7B-GGUF)"
            style={{
              width: "100%",
              padding: "7px 10px",
              background: bgInput,
              color: textPrimary,
              border: `1px solid ${borderColor}`,
              borderRadius: 5,
              fontSize: 12,
              outline: "none",
              boxSizing: "border-box",
            }}
          />
          <input
            type="text"
            value={sendModelFilename}
            onChange={(e) => setSendModelFilename(e.target.value)}
            placeholder="Filename (e.g. llama-2-7b.Q4_K_M.gguf)"
            style={{
              width: "100%",
              padding: "7px 10px",
              background: bgInput,
              color: textPrimary,
              border: `1px solid ${borderColor}`,
              borderRadius: 5,
              fontSize: 12,
              outline: "none",
              boxSizing: "border-box",
            }}
          />
          <button type="button"
            onClick={handleSendModel}
            disabled={
              sendingModel ||
              !sendModelPeerAddress.trim() ||
              !sendModelId.trim() ||
              !sendModelFilename.trim()
            }
            style={{
              padding: "7px 0",
              fontSize: 12,
              fontWeight: 600,
              border: `1px solid #60a5fa44`,
              borderRadius: 5,
              background: "#60a5fa18",
              color: "#60a5fa",
              cursor: sendingModel ? "wait" : "pointer",
              opacity:
                !sendModelPeerAddress.trim() || !sendModelId.trim() || !sendModelFilename.trim()
                  ? 0.5
                  : 1,
            }}
          >
            {sendingModel ? "Sending..." : "Send Model"}
          </button>
        </div>
      </div>
    </div>
  );

  const renderActiveProviderPanel = () => (
    <div
      style={{
        background: bgPanel,
        borderRadius: 8,
        border: `1px solid ${borderColor}`,
        padding: 16,
        display: "flex",
        flexDirection: "column",
        gap: 10,
      }}
    >
      <div style={{ fontWeight: 700, fontSize: 14, color: textPrimary }}>
        Active Provider
      </div>
      {providerLoading ? (
        <div style={{ fontSize: 12, color: textSecondary }}>Loading provider info...</div>
      ) : activeProvider ? (
        <div
          style={{
            padding: "10px 12px",
            background: bgCard,
            border: `1px solid ${borderColor}`,
            borderRadius: 6,
            display: "flex",
            flexDirection: "column",
            gap: 6,
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
            <span style={{ color: textSecondary }}>Provider</span>
            <span style={{ color: accent, fontWeight: 600 }}>{activeProvider.provider}</span>
          </div>
          {activeProvider.model && (
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
              <span style={{ color: textSecondary }}>Model</span>
              <span style={{ color: textPrimary, fontWeight: 500 }}>{activeProvider.model}</span>
            </div>
          )}
          {activeProvider.status && (
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
              <span style={{ color: textSecondary }}>Status</span>
              <span
                style={{
                  color: activeProvider.status === "active" || activeProvider.status === "connected"
                    ? accent
                    : "#fbbf24",
                  fontWeight: 500,
                }}
              >
                {activeProvider.status}
              </span>
            </div>
          )}
        </div>
      ) : (
        <div style={{ fontSize: 12, color: textSecondary }}>
          No active LLM provider detected. Configure one in Settings.
        </div>
      )}
    </div>
  );

  /* ── render ── */

  return (
    <div
      style={{
        padding: 24,
        color: textPrimary,
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: bgPage,
      }}
    >
      {/* Error banner */}
      {error && (
        <div
          style={{
            padding: "10px 16px",
            background: "#ff444422",
            border: "1px solid #ff444466",
            borderRadius: 6,
            color: "#ff6666",
            marginBottom: 12,
            fontSize: 13,
          }}
        >
          {error}
        </div>
      )}

      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 20 }}>
        <h1 style={{ color: accent, margin: 0, fontSize: 22 }}>
          {"\uD83E\uDDE0"} Model Hub
        </h1>
        <span
          style={{
            background: `${accent}22`,
            color: accent,
            padding: "2px 10px",
            borderRadius: 10,
            fontSize: 12,
            fontWeight: 600,
          }}
        >
          {installedModels.length} installed
        </span>
      </div>

      {/* 3-panel layout */}
      <div style={{ display: "flex", gap: 16, flex: 1, minHeight: 0 }}>
        {renderSearchPanel()}
        {renderDetailsPanel()}
        {renderLocalPanel()}
      </div>

      {/* Bottom row: Nexus Link + Active Provider */}
      <div style={{ display: "flex", gap: 16, marginTop: 16 }}>
        <div style={{ flex: 2 }}>{renderNexusLinkPanel()}</div>
        <div style={{ flex: 1 }}>{renderActiveProviderPanel()}</div>
      </div>
    </div>
  );
}
