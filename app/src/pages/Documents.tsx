import { useCallback, useEffect, useRef, useState } from "react";
import {
  indexDocument,
  chatWithDocuments,
  listIndexedDocuments,
  removeIndexedDocument,
  getDocumentGovernance,
  getSemanticMap,
  getDocumentAccessLog,
} from "../api/backend";

/* ── types ── */

interface RagDocument {
  path: string;
  format: string;
  chunk_count: number;
  indexed_at: string;
  governance: DocumentGovernance;
}

interface DocumentGovernance {
  content_hash: string;
  redacted_hash: string;
  pii_findings_count: number;
  pii_types_found: string[];
  redaction_mode: string;
  integrity_verified: boolean;
}

interface ProjectedPoint {
  chunk_id: string;
  doc_path: string;
  x: number;
  y: number;
  label: string;
}

interface DocumentAccessEntry {
  timestamp: string;
  operation: string;
  agent_or_user: string;
  detail: string;
}

interface ChatSource {
  doc_path: string;
  chunk_index: number;
  score: number;
}

interface ChatEntry {
  question: string;
  answer: string;
  sources: ChatSource[];
}

/* ── helpers ── */

function fileName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
}

const FORMAT_COLORS: Record<string, string> = {
  PlainText: "#fbbf24",
  Markdown: "var(--nexus-accent)",
  Code: "#a78bfa",
};

const SUPPORTED_EXTENSIONS = [
  ".txt", ".md", ".rs", ".ts", ".js", ".py", ".go", ".java",
  ".c", ".cpp", ".h", ".css", ".html", ".json", ".toml",
  ".yaml", ".yml", ".sh", ".sql", ".rb", ".swift", ".kt",
  ".log", ".csv", ".text", ".markdown", ".bash",
];

const DOC_PALETTE = [
  "#00ff9d", "var(--nexus-accent)", "#a78bfa", "#fbbf24", "#f472b6",
  "#fb923c", "#34d399", "#60a5fa", "#c084fc", "#f87171",
];

function docColor(docPath: string, allPaths: string[]): string {
  const idx = allPaths.indexOf(docPath);
  return DOC_PALETTE[idx >= 0 ? idx % DOC_PALETTE.length : 0];
}

function truncateHash(hash: string): string {
  return hash.length > 12 ? hash.slice(0, 12) + "..." : hash;
}

const OP_DOT_COLORS: Record<string, string> = {
  ingest: "#00ff9d",
  query: "#60a5fa",
  remove: "#f87171",
};

/* ── component ── */

export default function Documents() {
  const [documents, setDocuments] = useState<RagDocument[]>([]);
  const [chatHistory, setChatHistory] = useState<ChatEntry[]>([]);
  const [isIndexing, setIsIndexing] = useState(false);
  const [isQuerying, setIsQuerying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [question, setQuestion] = useState("");
  const [isDragOver, setIsDragOver] = useState(false);
  const [indexingFile, setIndexingFile] = useState<string | null>(null);

  // New state
  const [selectedDoc, setSelectedDoc] = useState<string | null>(null);
  const [governance, setGovernance] = useState<DocumentGovernance | null>(null);
  const [accessLog, setAccessLog] = useState<DocumentAccessEntry[]>([]);
  const [semanticMap, setSemanticMap] = useState<ProjectedPoint[]>([]);
  const [viewMode, setViewMode] = useState<"list" | "cluster">("list");
  const [isLoadingGovernance, setIsLoadingGovernance] = useState(false);
  const [hoveredPoint, setHoveredPoint] = useState<string | null>(null);

  const chatEndRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const errorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (errorTimerRef.current) clearTimeout(errorTimerRef.current);
    };
  }, []);

  const showError = useCallback((msg: string) => {
    setError(msg);
    if (errorTimerRef.current) clearTimeout(errorTimerRef.current);
    errorTimerRef.current = setTimeout(() => setError(null), 5000);
  }, []);

  const loadDocuments = useCallback(async () => {
    try {
      const raw = await listIndexedDocuments();
      const docs: RagDocument[] = JSON.parse(raw);
      setDocuments(docs);
    } catch {
      setDocuments([]);
    }
  }, []);

  const loadSemanticMap = useCallback(async () => {
    try {
      const raw = await getSemanticMap();
      const points: ProjectedPoint[] = JSON.parse(raw);
      setSemanticMap(points);
    } catch {
      setSemanticMap([]);
    }
  }, []);

  const loadGovernance = useCallback(async (docPath: string) => {
    setIsLoadingGovernance(true);
    try {
      const [govRaw, logRaw] = await Promise.all([
        getDocumentGovernance(docPath),
        getDocumentAccessLog(docPath),
      ]);
      setGovernance(JSON.parse(govRaw));
      setAccessLog(JSON.parse(logRaw));
    } catch {
      setGovernance(null);
      setAccessLog([]);
    } finally {
      setIsLoadingGovernance(false);
    }
  }, []);

  useEffect(() => {
    loadDocuments();
  }, [loadDocuments]);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chatHistory]);

  useEffect(() => {
    if (viewMode === "cluster") {
      loadSemanticMap();
    }
  }, [viewMode, loadSemanticMap]);

  useEffect(() => {
    if (selectedDoc) {
      loadGovernance(selectedDoc);
    } else {
      setGovernance(null);
      setAccessLog([]);
    }
  }, [selectedDoc, loadGovernance]);

  const handleSelectDoc = useCallback((docPath: string) => {
    setSelectedDoc((prev) => (prev === docPath ? null : docPath));
  }, []);

  const handleIndex = useCallback(
    async (filePath: string) => {
      setIsIndexing(true);
      setIndexingFile(fileName(filePath));
      try {
        await indexDocument(filePath);
        await loadDocuments();
        if (viewMode === "cluster") await loadSemanticMap();
      } catch (e) {
        showError(String(e));
      } finally {
        setIsIndexing(false);
        setIndexingFile(null);
      }
    },
    [loadDocuments, loadSemanticMap, viewMode, showError]
  );

  const handleRemove = useCallback(
    async (docPath: string) => {
      try {
        await removeIndexedDocument(docPath);
        if (selectedDoc === docPath) setSelectedDoc(null);
        await loadDocuments();
        if (viewMode === "cluster") await loadSemanticMap();
      } catch (e) {
        showError(String(e));
      }
    },
    [loadDocuments, loadSemanticMap, viewMode, selectedDoc, showError]
  );

  const handleChat = useCallback(async () => {
    const q = question.trim();
    if (!q) return;
    setIsQuerying(true);
    setQuestion("");
    try {
      const raw = await chatWithDocuments(q);
      const resp = JSON.parse(raw) as {
        prompt: string;
        sources: ChatSource[];
        chunk_count: number;
      };
      setChatHistory((prev) => [
        ...prev,
        { question: q, answer: resp.prompt, sources: resp.sources },
      ]);
    } catch (e) {
      showError(String(e));
    } finally {
      setIsQuerying(false);
    }
  }, [question, showError]);

  const handleFileDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      setIsDragOver(false);
      const files = e.dataTransfer?.files;
      if (files && files.length > 0) {
        for (let i = 0; i < files.length; i++) {
          const f = files[i];
          const path = (f as unknown as { path?: string }).path || f.name;
          handleIndex(path);
        }
      }
    },
    [handleIndex]
  );

  const handleFileInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (files && files.length > 0) {
        for (let i = 0; i < files.length; i++) {
          const f = files[i];
          const path = (f as unknown as { path?: string }).path || f.name;
          handleIndex(path);
        }
      }
      e.target.value = "";
    },
    [handleIndex]
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

  /* ── derived ── */

  const uniqueDocPaths = [...new Set(semanticMap.map((p) => p.doc_path))];

  /* ── sub-renders ── */

  const renderDocList = () => (
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
          whiteSpace: "normal",
          overflowWrap: "anywhere",
          lineHeight: 1.4,
        }}
      >
        Indexed Documents
      </div>
      {documents.length === 0 ? (
        <div style={{ padding: 20, textAlign: "center", color: textSecondary, fontSize: 13 }}>
          No documents indexed yet. Drop files above to get started.
        </div>
      ) : (
        documents.map((doc) => (
          <div
            key={doc.path}
            onClick={() => handleSelectDoc(doc.path)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              padding: "10px 14px",
              borderBottom: `1px solid ${borderColor}`,
              cursor: "pointer",
              background: selectedDoc === doc.path ? `${accent}0d` : "transparent",
              borderLeft: selectedDoc === doc.path ? `3px solid ${accent}` : "3px solid transparent",
              transition: "all 0.15s",
            }}
          >
            <div style={{ flex: 1, minWidth: 0 }}>
              <div
                style={{
                  fontWeight: 500,
                  fontSize: 13,
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
                title={doc.path}
              >
                {fileName(doc.path)}
              </div>
              <div style={{ display: "flex", gap: 8, marginTop: 3, alignItems: "center" }}>
                <span
                  style={{
                    fontSize: 10,
                    padding: "1px 6px",
                    borderRadius: 3,
                    background: `${FORMAT_COLORS[doc.format] ?? "#888"}22`,
                    color: FORMAT_COLORS[doc.format] ?? "#888",
                    fontWeight: 600,
                  }}
                >
                  {doc.format}
                </span>
                <span style={{ fontSize: 11, color: textSecondary }}>
                  {doc.chunk_count} chunks
                </span>
              </div>
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleRemove(doc.path);
              }}
              style={{
                background: "none",
                border: "none",
                color: "#ff4444",
                cursor: "pointer",
                fontSize: 16,
                padding: "2px 6px",
                borderRadius: 4,
                lineHeight: 1,
              }}
              title="Remove document"
            >
              {"\u00D7"}
            </button>
          </div>
        ))
      )}
    </div>
  );

  const SVG_W = 320;
  const SVG_H = 280;
  const SVG_PAD = 20;

  const renderClusterView = () => {
    if (documents.length === 0) {
      return (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: textSecondary,
            fontSize: 13,
            background: bgPanel,
            borderRadius: 8,
            border: `1px solid ${borderColor}`,
          }}
        >
          Index documents to see the semantic map
        </div>
      );
    }

    const toX = (x: number) => SVG_PAD + ((x + 1) / 2) * (SVG_W - 2 * SVG_PAD);
    const toY = (y: number) => SVG_PAD + ((1 - y) / 2) * (SVG_H - 2 * SVG_PAD);

    return (
      <div
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          background: bgPanel,
          borderRadius: 8,
          border: `1px solid ${borderColor}`,
          overflow: "hidden",
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
          Semantic Cluster Map
        </div>
        <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", padding: 8 }}>
          <svg
            width={SVG_W}
            height={SVG_H}
            style={{ background: bgPage, borderRadius: 6 }}
          >
            {/* axis lines */}
            <line
              x1={toX(-1)} y1={toY(0)} x2={toX(1)} y2={toY(0)}
              stroke={borderColor} strokeWidth={1}
            />
            <line
              x1={toX(0)} y1={toY(-1)} x2={toX(0)} y2={toY(1)}
              stroke={borderColor} strokeWidth={1}
            />
            {/* points */}
            {semanticMap.map((pt) => {
              const isHovered = hoveredPoint === pt.chunk_id;
              const isSelected = selectedDoc === pt.doc_path;
              const color = docColor(pt.doc_path, uniqueDocPaths);
              return (
                <circle
                  key={pt.chunk_id}
                  cx={toX(pt.x)}
                  cy={toY(pt.y)}
                  r={isHovered ? 10 : 7}
                  fill={color}
                  opacity={isHovered || isSelected ? 1 : 0.7}
                  stroke={isSelected ? "#fff" : "none"}
                  strokeWidth={isSelected ? 2 : 0}
                  style={{ cursor: "pointer", transition: "r 0.15s, opacity 0.15s" }}
                  onMouseEnter={() => setHoveredPoint(pt.chunk_id)}
                  onMouseLeave={() => setHoveredPoint(null)}
                  onClick={() => handleSelectDoc(pt.doc_path)}
                >
                  <title>{`${fileName(pt.doc_path)}\n${pt.label}`}</title>
                </circle>
              );
            })}
          </svg>
        </div>
        {/* legend */}
        <div
          style={{
            padding: "8px 14px",
            borderTop: `1px solid ${borderColor}`,
            display: "flex",
            flexWrap: "wrap",
            gap: 10,
          }}
        >
          {uniqueDocPaths.map((dp) => (
            <div key={dp} style={{ display: "flex", alignItems: "center", gap: 5, fontSize: 11 }}>
              <div
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: docColor(dp, uniqueDocPaths),
                  flexShrink: 0,
                }}
              />
              <span style={{ color: textSecondary }}>{fileName(dp)}</span>
            </div>
          ))}
        </div>
      </div>
    );
  };

  const renderGovernanceSidebar = () => {
    if (!selectedDoc) {
      return (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: textSecondary,
            fontSize: 13,
            textAlign: "center",
            padding: 20,
          }}
        >
          Select a document to view governance details
        </div>
      );
    }

    if (isLoadingGovernance) {
      return (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: textSecondary,
            fontSize: 13,
          }}
        >
          Loading governance data...
        </div>
      );
    }

    if (!governance) {
      return (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: textSecondary,
            fontSize: 13,
          }}
        >
          Governance data unavailable
        </div>
      );
    }

    return (
      <div style={{ flex: 1, overflow: "auto", padding: 14, display: "flex", flexDirection: "column", gap: 16 }}>
        {/* selected doc header */}
        <div style={{ fontSize: 13, fontWeight: 600, color: accent, wordBreak: "break-all" }}>
          {fileName(selectedDoc)}
        </div>

        {/* INTEGRITY section */}
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
            Integrity
          </div>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              marginBottom: 8,
            }}
          >
            <span style={{ fontSize: 16 }}>{governance.integrity_verified ? "\uD83D\uDD12" : "\u26A0\uFE0F"}</span>
            <span
              style={{
                fontSize: 12,
                fontWeight: 600,
                color: governance.integrity_verified ? accent : "#f87171",
              }}
            >
              {governance.integrity_verified ? "Integrity Verified" : "Unverified"}
            </span>
          </div>
          <div style={{ fontSize: 11, color: textSecondary, marginBottom: 4 }}>
            <span style={{ color: textPrimary }}>Content: </span>
            <span title={governance.content_hash} style={{ fontFamily: "monospace" }}>
              {truncateHash(governance.content_hash)}
            </span>
          </div>
          <div style={{ fontSize: 11, color: textSecondary, marginBottom: 6 }}>
            <span style={{ color: textPrimary }}>Redacted: </span>
            <span title={governance.redacted_hash} style={{ fontFamily: "monospace" }}>
              {truncateHash(governance.redacted_hash)}
            </span>
          </div>
          <div style={{ fontSize: 10, color: textSecondary, fontStyle: "italic" }}>
            Hashes verify document hasn't been tampered with
          </div>
        </div>

        {/* PII PROTECTION section */}
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
            PII Redaction
          </div>
          <div style={{ marginBottom: 8 }}>
            {governance.pii_findings_count > 0 ? (
              <span
                style={{
                  fontSize: 11,
                  padding: "2px 8px",
                  borderRadius: 10,
                  background: "#fbbf2422",
                  color: "#fbbf24",
                  fontWeight: 600,
                }}
              >
                {governance.pii_findings_count} item{governance.pii_findings_count !== 1 ? "s" : ""} redacted
              </span>
            ) : (
              <span
                style={{
                  fontSize: 11,
                  padding: "2px 8px",
                  borderRadius: 10,
                  background: `${accent}22`,
                  color: accent,
                  fontWeight: 600,
                }}
              >
                Clean — no PII detected
              </span>
            )}
          </div>
          {governance.pii_types_found.length > 0 && (
            <div style={{ display: "flex", flexWrap: "wrap", gap: 4, marginBottom: 8 }}>
              {governance.pii_types_found.map((t) => (
                <span
                  key={t}
                  style={{
                    fontSize: 10,
                    padding: "1px 7px",
                    borderRadius: 3,
                    background: "#a78bfa22",
                    color: "#a78bfa",
                    fontWeight: 600,
                  }}
                >
                  {t}
                </span>
              ))}
            </div>
          )}
          <div style={{ fontSize: 11, color: textSecondary }}>
            Mode: <span style={{ color: textPrimary, fontWeight: 500 }}>{governance.redaction_mode}</span>
          </div>
        </div>

        {/* ACCESS LOG section */}
        <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0 }}>
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
            Access History
          </div>
          {accessLog.length === 0 ? (
            <div style={{ fontSize: 12, color: textSecondary }}>No access history</div>
          ) : (
            <div style={{ flex: 1, overflow: "auto" }}>
              {[...accessLog].reverse().map((entry, i) => (
                <div
                  key={i}
                  style={{
                    display: "flex",
                    gap: 8,
                    alignItems: "flex-start",
                    padding: "6px 0",
                    borderBottom: `1px solid ${borderColor}`,
                    fontSize: 11,
                  }}
                >
                  <div
                    style={{
                      width: 7,
                      height: 7,
                      borderRadius: "50%",
                      background: OP_DOT_COLORS[entry.operation] ?? textSecondary,
                      marginTop: 4,
                      flexShrink: 0,
                    }}
                  />
                  <div style={{ minWidth: 0 }}>
                    <div style={{ fontWeight: 600, color: textPrimary }}>{entry.operation}</div>
                    <div
                      style={{
                        color: textSecondary,
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        maxWidth: 180,
                      }}
                      title={entry.detail}
                    >
                      {entry.detail.length > 80 ? entry.detail.slice(0, 80) + "..." : entry.detail}
                    </div>
                    <div style={{ color: textSecondary, fontSize: 10 }}>{entry.timestamp}</div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  };

  /* ── render ── */

  return (
    <div style={{ padding: 24, color: textPrimary, height: "100%", display: "flex", flexDirection: "column", background: bgPage }}>
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
        <h1 style={{ color: accent, margin: 0, fontSize: 22 }}>Documents</h1>
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
          {documents.length} indexed
        </span>
      </div>

      {/* Main 3-panel layout */}
      <div style={{ display: "flex", gap: 16, flex: 1, minHeight: 0 }}>
        {/* Left panel — Document Management */}
        <div
          style={{
            width: "30%",
            display: "flex",
            flexDirection: "column",
            gap: 12,
            minHeight: 0,
          }}
        >
          {/* Drop zone */}
          <div
            onDragOver={(e) => {
              e.preventDefault();
              setIsDragOver(true);
            }}
            onDragLeave={() => setIsDragOver(false)}
            onDrop={handleFileDrop}
            onClick={() => fileInputRef.current?.click()}
            style={{
              border: `2px dashed ${isDragOver ? accent : borderColor}`,
              borderRadius: 8,
              padding: 20,
              textAlign: "center",
              cursor: "pointer",
              background: isDragOver ? `${accent}08` : bgPanel,
              transition: "all 0.2s",
            }}
          >
            <input
              ref={fileInputRef}
              type="file"
              multiple
              accept={SUPPORTED_EXTENSIONS.join(",")}
              onChange={handleFileInput}
              style={{ display: "none" }}
            />
            <div style={{ fontSize: 24, marginBottom: 6 }}>
              {isIndexing ? "\u23F3" : "\uD83D\uDCC4"}
            </div>
            {isIndexing ? (
              <div>
                <div style={{ color: accent, fontWeight: 600, fontSize: 13, marginBottom: 2 }}>
                  Indexing...
                </div>
                <div style={{ color: textSecondary, fontSize: 12 }}>{indexingFile}</div>
              </div>
            ) : (
              <div>
                <div style={{ color: textPrimary, fontWeight: 500, fontSize: 12, marginBottom: 2 }}>
                  Drag & drop files or click to browse
                </div>
                <div style={{ color: textSecondary, fontSize: 11 }}>
                  .txt .md .rs .py .js .ts .json .toml ...
                </div>
              </div>
            )}
          </div>

          {/* View toggle */}
          <div style={{ display: "flex", gap: 4 }}>
            <button
              onClick={() => setViewMode("list")}
              style={{
                flex: 1,
                padding: "6px 0",
                fontSize: 12,
                fontWeight: 600,
                border: `1px solid ${viewMode === "list" ? accent + "44" : borderColor}`,
                borderRadius: 5,
                background: viewMode === "list" ? `${accent}18` : bgCard,
                color: viewMode === "list" ? accent : textSecondary,
                cursor: "pointer",
              }}
            >
              List View
            </button>
            <button
              onClick={() => setViewMode("cluster")}
              style={{
                flex: 1,
                padding: "6px 0",
                fontSize: 12,
                fontWeight: 600,
                border: `1px solid ${viewMode === "cluster" ? accent + "44" : borderColor}`,
                borderRadius: 5,
                background: viewMode === "cluster" ? `${accent}18` : bgCard,
                color: viewMode === "cluster" ? accent : textSecondary,
                cursor: "pointer",
              }}
            >
              Cluster View
            </button>
          </div>

          {/* List or Cluster */}
          {viewMode === "list" ? renderDocList() : renderClusterView()}
        </div>

        {/* Center panel — Chat with Documents */}
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
          {/* Chat header */}
          <div
            style={{
              padding: "12px 16px",
              borderBottom: `1px solid ${borderColor}`,
              fontWeight: 600,
              fontSize: 14,
            }}
          >
            Chat with Your Documents
          </div>

          {/* Chat messages */}
          <div
            style={{
              flex: 1,
              overflow: "auto",
              padding: 16,
              display: "flex",
              flexDirection: "column",
              gap: 16,
            }}
          >
            {documents.length === 0 && chatHistory.length === 0 ? (
              <div
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  flexDirection: "column",
                  gap: 8,
                  color: textSecondary,
                }}
              >
                <div style={{ fontSize: 32 }}>{"\uD83D\uDCDA"}</div>
                <div>Index some documents to start chatting</div>
              </div>
            ) : (
              chatHistory.map((entry, i) => (
                <div key={i}>
                  {/* Question */}
                  <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: 8 }}>
                    <div
                      style={{
                        background: `${accent}18`,
                        border: `1px solid ${accent}33`,
                        borderRadius: 8,
                        padding: "8px 14px",
                        maxWidth: "80%",
                        fontSize: 13,
                      }}
                    >
                      {entry.question}
                    </div>
                  </div>
                  {/* Answer */}
                  <div style={{ display: "flex", justifyContent: "flex-start" }}>
                    <div
                      style={{
                        background: bgCard,
                        border: `1px solid ${borderColor}`,
                        borderRadius: 8,
                        padding: "10px 14px",
                        maxWidth: "90%",
                        fontSize: 13,
                      }}
                    >
                      <pre
                        style={{
                          whiteSpace: "pre-wrap",
                          wordBreak: "break-word",
                          margin: 0,
                          fontFamily: "inherit",
                          fontSize: 13,
                          lineHeight: 1.5,
                          maxHeight: 300,
                          overflow: "auto",
                        }}
                      >
                        {entry.answer}
                      </pre>
                      {entry.sources.length > 0 && (
                        <div
                          style={{
                            marginTop: 8,
                            paddingTop: 8,
                            borderTop: `1px solid ${borderColor}`,
                          }}
                        >
                          <div style={{ fontSize: 11, color: textSecondary, marginBottom: 4 }}>
                            Sources ({entry.sources.length}):
                          </div>
                          {entry.sources.map((src, j) => (
                            <div
                              key={j}
                              style={{
                                fontSize: 11,
                                color: textSecondary,
                                display: "flex",
                                gap: 8,
                                alignItems: "center",
                              }}
                            >
                              <span style={{ color: accent }}>{fileName(src.doc_path)}</span>
                              <span>chunk {src.chunk_index}</span>
                              <span style={{ color: "#fbbf24" }}>
                                {(src.score * 100).toFixed(1)}%
                              </span>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))
            )}
            {isQuerying && (
              <div style={{ display: "flex", justifyContent: "flex-start" }}>
                <div
                  style={{
                    background: bgCard,
                    border: `1px solid ${borderColor}`,
                    borderRadius: 8,
                    padding: "10px 14px",
                    fontSize: 13,
                    color: textSecondary,
                  }}
                >
                  Searching documents...
                </div>
              </div>
            )}
            <div ref={chatEndRef} />
          </div>

          {/* Input area */}
          <div
            style={{
              padding: "12px 16px",
              borderTop: `1px solid ${borderColor}`,
              display: "flex",
              gap: 8,
            }}
          >
            <input
              type="text"
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleChat();
                }
              }}
              placeholder={
                documents.length === 0
                  ? "Index documents first..."
                  : "Ask a question about your documents..."
              }
              disabled={isQuerying || documents.length === 0}
              style={{
                flex: 1,
                padding: "8px 12px",
                background: bgInput,
                color: textPrimary,
                border: `1px solid ${borderColor}`,
                borderRadius: 6,
                fontSize: 13,
                outline: "none",
              }}
            />
            <button
              onClick={() => {
                handleChat();
              }}
              disabled={isQuerying || !question.trim() || documents.length === 0}
              style={{
                padding: "8px 20px",
                background: question.trim() && documents.length > 0 ? `${accent}22` : bgCard,
                color: question.trim() && documents.length > 0 ? accent : textSecondary,
                border: `1px solid ${question.trim() && documents.length > 0 ? `${accent}44` : borderColor}`,
                borderRadius: 6,
                cursor:
                  isQuerying || !question.trim() || documents.length === 0
                    ? "not-allowed"
                    : "pointer",
                fontWeight: 600,
                fontSize: 13,
              }}
            >
              {isQuerying ? "..." : "Send"}
            </button>
          </div>
        </div>

        {/* Right panel — Governance Sidebar */}
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
            }}
          >
            Governance
          </div>
          {renderGovernanceSidebar()}
        </div>
      </div>
    </div>
  );
}
