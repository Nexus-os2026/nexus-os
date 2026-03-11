import { useCallback, useEffect, useRef, useState } from "react";
import {
  indexDocument,
  chatWithDocuments,
  listIndexedDocuments,
  removeIndexedDocument,
} from "../api/backend";

/* ─── types ─── */

interface RagDocument {
  path: string;
  format: string;
  chunk_count: number;
  indexed_at: string;
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

/* ─── helpers ─── */

function fileName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
}

const FORMAT_COLORS: Record<string, string> = {
  PlainText: "#fbbf24",
  Markdown: "#22d3ee",
  Code: "#a78bfa",
};

const SUPPORTED_EXTENSIONS = [
  ".txt", ".md", ".rs", ".ts", ".js", ".py", ".go", ".java",
  ".c", ".cpp", ".h", ".css", ".html", ".json", ".toml",
  ".yaml", ".yml", ".sh", ".sql", ".rb", ".swift", ".kt",
  ".log", ".csv", ".text", ".markdown", ".bash",
];

/* ─── component ─── */

export default function Documents() {
  const [documents, setDocuments] = useState<RagDocument[]>([]);
  const [chatHistory, setChatHistory] = useState<ChatEntry[]>([]);
  const [isIndexing, setIsIndexing] = useState(false);
  const [isQuerying, setIsQuerying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [question, setQuestion] = useState("");
  const [isDragOver, setIsDragOver] = useState(false);
  const [indexingFile, setIndexingFile] = useState<string | null>(null);

  const chatEndRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const showError = useCallback((msg: string) => {
    setError(msg);
    setTimeout(() => setError(null), 5000);
  }, []);

  const loadDocuments = useCallback(async () => {
    try {
      const raw = await listIndexedDocuments();
      const docs: RagDocument[] = JSON.parse(raw);
      setDocuments(docs);
    } catch {
      // Non-desktop mode or no docs yet — use empty list
      setDocuments([]);
    }
  }, []);

  useEffect(() => {
    loadDocuments();
  }, [loadDocuments]);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chatHistory]);

  const handleIndex = useCallback(
    async (filePath: string) => {
      setIsIndexing(true);
      setIndexingFile(fileName(filePath));
      try {
        await indexDocument(filePath);
        await loadDocuments();
      } catch (e) {
        showError(String(e));
      } finally {
        setIsIndexing(false);
        setIndexingFile(null);
      }
    },
    [loadDocuments, showError]
  );

  const handleRemove = useCallback(
    async (docPath: string) => {
      try {
        await removeIndexedDocument(docPath);
        await loadDocuments();
      } catch (e) {
        showError(String(e));
      }
    },
    [loadDocuments, showError]
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
        {
          question: q,
          answer: resp.prompt,
          sources: resp.sources,
        },
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
        // In web mode, file drop provides File objects — extract name as path.
        // In Tauri desktop mode, the path is available via webkitRelativePath or name.
        for (let i = 0; i < files.length; i++) {
          const f = files[i];
          // Use the file path if available (Tauri provides full paths)
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
      // Reset so the same file can be selected again
      e.target.value = "";
    },
    [handleIndex]
  );

  /* ─── styles ─── */

  const accent = "#00ff9d";
  const bgPage = "#0d0d1a";
  const bgPanel = "#141428";
  const bgCard = "#1a1a2e";
  const bgInput = "#0f0f1e";
  const borderColor = "#2a2a3e";
  const textPrimary = "#e0e0e0";
  const textSecondary = "#888";

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

      {/* Main split layout */}
      <div style={{ display: "flex", gap: 16, flex: 1, minHeight: 0 }}>
        {/* Left panel — Document Management */}
        <div
          style={{
            width: "40%",
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
              padding: 24,
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
            <div style={{ fontSize: 28, marginBottom: 8 }}>
              {isIndexing ? "\u23F3" : "\u{1F4C4}"}
            </div>
            {isIndexing ? (
              <div>
                <div style={{ color: accent, fontWeight: 600, marginBottom: 4 }}>
                  Indexing...
                </div>
                <div style={{ color: textSecondary, fontSize: 13 }}>
                  {indexingFile}
                </div>
              </div>
            ) : (
              <div>
                <div style={{ color: textPrimary, fontWeight: 500, marginBottom: 4 }}>
                  Drag & drop files here or click to browse
                </div>
                <div style={{ color: textSecondary, fontSize: 12 }}>
                  Supports: .txt, .md, .rs, .py, .js, .ts, .go, .java, .json, .toml, .yaml, and more
                </div>
              </div>
            )}
          </div>

          {/* Browse button */}
          <button
            onClick={() => fileInputRef.current?.click()}
            disabled={isIndexing}
            style={{
              padding: "8px 16px",
              background: bgCard,
              color: accent,
              border: `1px solid ${accent}44`,
              borderRadius: 6,
              cursor: isIndexing ? "not-allowed" : "pointer",
              fontWeight: 600,
              fontSize: 13,
              opacity: isIndexing ? 0.5 : 1,
            }}
          >
            Browse Files
          </button>

          {/* Indexed documents list */}
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
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 10,
                    padding: "10px 14px",
                    borderBottom: `1px solid ${borderColor}`,
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
                    \u00D7
                  </button>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Right panel — Chat with Documents */}
        <div
          style={{
            width: "60%",
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
                <div style={{ fontSize: 32 }}>{"\u{1F4DA}"}</div>
                <div>Index some documents to start chatting</div>
              </div>
            ) : (
              chatHistory.map((entry, i) => (
                <div key={i}>
                  {/* Question */}
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "flex-end",
                      marginBottom: 8,
                    }}
                  >
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
                      {/* Sources */}
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
                              <span style={{ color: accent }}>
                                {fileName(src.doc_path)}
                              </span>
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
              onClick={() => { handleChat(); }}
              disabled={isQuerying || !question.trim() || documents.length === 0}
              style={{
                padding: "8px 20px",
                background: question.trim() && documents.length > 0 ? `${accent}22` : bgCard,
                color: question.trim() && documents.length > 0 ? accent : textSecondary,
                border: `1px solid ${question.trim() && documents.length > 0 ? `${accent}44` : borderColor}`,
                borderRadius: 6,
                cursor: isQuerying || !question.trim() || documents.length === 0 ? "not-allowed" : "pointer",
                fontWeight: 600,
                fontSize: 13,
              }}
            >
              {isQuerying ? "..." : "Send"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
