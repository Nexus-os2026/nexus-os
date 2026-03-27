import { useCallback, useRef, useState } from "react";
import {
  perceptionAnalyzeChart,
  perceptionDescribe,
  perceptionExtractData,
  perceptionExtractText,
  perceptionFindUiElements,
  perceptionGetPolicy,
  perceptionInit,
  perceptionQuestion,
  perceptionReadError,
} from "../api/backend";
import { alpha, commandPageStyle } from "./commandCenterUi";

const ACCENT = "#a855f7";
const GREEN = "#22c55e";
const BLUE = "#3b82f6";

type TaskType =
  | "describe"
  | "extract_text"
  | "question"
  | "find_ui"
  | "extract_data"
  | "read_error"
  | "analyze_chart";

const TASK_LABELS: Record<TaskType, string> = {
  describe: "Describe Image",
  extract_text: "Extract Text (OCR)",
  question: "Visual Question",
  find_ui: "Find UI Elements",
  extract_data: "Extract Structured Data",
  read_error: "Read Error Message",
  analyze_chart: "Analyze Chart",
};

interface PerceptionResult {
  input_id: string;
  task: string;
  success: boolean;
  description: string;
  extracted_text?: string | null;
  structured_data?: any;
  ui_elements?: any[] | null;
  confidence: number;
  model_used: string;
  tokens_used: number;
}

const cardStyle: React.CSSProperties = {
  background: alpha("#1e1e2e", 0.7),
  borderRadius: 10,
  padding: 16,
  border: "1px solid " + alpha("#ffffff", 0.08),
};

const labelStyle: React.CSSProperties = {
  fontSize: 11,
  color: "#888",
  textTransform: "uppercase" as const,
  letterSpacing: 1,
  marginBottom: 4,
};

const btnStyle: React.CSSProperties = {
  padding: "8px 16px",
  borderRadius: 6,
  border: "none",
  cursor: "pointer",
  fontWeight: 600,
  fontSize: 13,
};

export default function Perception() {
  const [provider, setProvider] = useState("groq");
  const [apiKey, setApiKey] = useState("");
  const [modelId, setModelId] = useState("llama-4-scout-17b-16e-instruct");
  const [initialized, setInitialized] = useState(false);
  const [initError, setInitError] = useState("");

  const [imageBase64, setImageBase64] = useState("");
  const [imagePreview, setImagePreview] = useState("");
  const [imageFormat, setImageFormat] = useState("png");

  const [task, setTask] = useState<TaskType>("describe");
  const [question, setQuestion] = useState("");
  const [schema, setSchema] = useState("");

  const [result, setResult] = useState<PerceptionResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const [policy, setPolicy] = useState<any>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const handleInit = useCallback(async () => {
    setInitError("");
    try {
      await perceptionInit(provider, apiKey, modelId);
      setInitialized(true);
      const p = await perceptionGetPolicy().catch(() => null);
      setPolicy(p);
    } catch (e: any) {
      setInitError(String(e));
    }
  }, [provider, apiKey, modelId]);

  const handleFileUpload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const ext = file.name.split(".").pop()?.toLowerCase() || "png";
    setImageFormat(ext === "jpg" ? "jpeg" : ext);
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      setImagePreview(dataUrl);
      const b64 = dataUrl.split(",")[1] || "";
      setImageBase64(b64);
    };
    reader.readAsDataURL(file);
  }, []);

  const handlePaste = useCallback((e: React.ClipboardEvent) => {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const item of Array.from(items)) {
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (!file) continue;
        setImageFormat(item.type.split("/")[1] || "png");
        const reader = new FileReader();
        reader.onload = () => {
          const dataUrl = reader.result as string;
          setImagePreview(dataUrl);
          setImageBase64(dataUrl.split(",")[1] || "");
        };
        reader.readAsDataURL(file);
        break;
      }
    }
  }, []);

  const handlePerceive = useCallback(async () => {
    if (!imageBase64) return;
    setLoading(true);
    setError("");
    setResult(null);
    try {
      let res: any;
      switch (task) {
        case "describe":
          res = await perceptionDescribe(imageBase64, imageFormat);
          break;
        case "extract_text":
          res = await perceptionExtractText(imageBase64, imageFormat);
          break;
        case "question":
          res = await perceptionQuestion(imageBase64, imageFormat, question);
          break;
        case "find_ui":
          res = { ui_elements: await perceptionFindUiElements(imageBase64), success: true, description: "UI elements found" };
          break;
        case "extract_data":
          res = await perceptionExtractData(imageBase64, imageFormat, schema || undefined);
          break;
        case "read_error":
          res = await perceptionReadError(imageBase64);
          break;
        case "analyze_chart":
          res = await perceptionAnalyzeChart(imageBase64, imageFormat);
          break;
      }
      setResult(res);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [imageBase64, imageFormat, task, question, schema]);

  return (
    <div style={{ ...commandPageStyle, padding: 24, color: "#e0e0e0" }} onPaste={handlePaste}>
      <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 4 }}>
        <span style={{ color: ACCENT }}>Multi-Modal Perception</span>
      </h1>
      <p style={{ color: "#888", fontSize: 13, marginBottom: 20 }}>
        Give agents eyes — process screenshots, documents, and images through vision models.
      </p>

      {!initialized ? (
        <div style={{ ...cardStyle, maxWidth: 480, marginBottom: 20 }}>
          <div style={labelStyle}>Initialize Vision Provider</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 10, marginTop: 8 }}>
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value)}
              style={{ padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444" }}
            >
              <option value="groq">Groq (llama-4-scout)</option>
              <option value="nim">NVIDIA NIM (llama-3.2-vision)</option>
            </select>
            <input
              type="password"
              placeholder="API Key"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              style={{ padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444" }}
            />
            <input
              placeholder="Model ID"
              value={modelId}
              onChange={(e) => setModelId(e.target.value)}
              style={{ padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444" }}
            />
            <button onClick={handleInit} style={{ ...btnStyle, background: ACCENT, color: "#fff" }}>
              Initialize
            </button>
            {initError && <div style={{ color: "#ef4444", fontSize: 12 }}>{initError}</div>}
          </div>
        </div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
          {/* Left column — Input */}
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={cardStyle}>
              <div style={labelStyle}>Image Input</div>
              <div
                onClick={() => fileRef.current?.click()}
                style={{
                  border: "2px dashed " + alpha("#ffffff", 0.15),
                  borderRadius: 8,
                  padding: 24,
                  textAlign: "center",
                  cursor: "pointer",
                  marginTop: 8,
                  minHeight: 120,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                {imagePreview ? (
                  <img src={imagePreview} alt="preview" style={{ maxWidth: "100%", maxHeight: 240, borderRadius: 6 }} />
                ) : (
                  <span style={{ color: "#666" }}>Click to upload or paste (Ctrl+V) an image</span>
                )}
              </div>
              <input ref={fileRef} type="file" accept="image/*" onChange={handleFileUpload} style={{ display: "none" }} />
            </div>

            <div style={cardStyle}>
              <div style={labelStyle}>Perception Task</div>
              <select
                value={task}
                onChange={(e) => setTask(e.target.value as TaskType)}
                style={{ width: "100%", padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444", marginTop: 8 }}
              >
                {Object.entries(TASK_LABELS).map(([k, v]) => (
                  <option key={k} value={k}>{v}</option>
                ))}
              </select>

              {task === "question" && (
                <input
                  placeholder="Ask a question about the image..."
                  value={question}
                  onChange={(e) => setQuestion(e.target.value)}
                  style={{ width: "100%", padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444", marginTop: 8, boxSizing: "border-box" }}
                />
              )}

              {task === "extract_data" && (
                <textarea
                  placeholder='Optional JSON schema, e.g. {"type": "object"}'
                  value={schema}
                  onChange={(e) => setSchema(e.target.value)}
                  rows={3}
                  style={{ width: "100%", padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444", marginTop: 8, fontFamily: "monospace", fontSize: 12, resize: "vertical", boxSizing: "border-box" }}
                />
              )}

              <button
                onClick={handlePerceive}
                disabled={!imageBase64 || loading}
                style={{
                  ...btnStyle,
                  width: "100%",
                  marginTop: 12,
                  background: imageBase64 && !loading ? ACCENT : "#444",
                  color: "#fff",
                }}
              >
                {loading ? "Perceiving..." : "Perceive"}
              </button>
            </div>

            {policy && (
              <div style={cardStyle}>
                <div style={labelStyle}>Governance Policy</div>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginTop: 8, fontSize: 12 }}>
                  <div>Min Autonomy: <span style={{ color: BLUE }}>L{policy.min_autonomy_level}</span></div>
                  <div>Max Image: <span style={{ color: BLUE }}>{(policy.max_image_size_bytes / 1024 / 1024).toFixed(0)} MB</span></div>
                  <div>Rate Limit: <span style={{ color: BLUE }}>{policy.max_perception_calls_per_minute}/min</span></div>
                  <div>Cost: <span style={{ color: ACCENT }}>{(policy.cost_per_perception / 1_000_000).toFixed(0)} NXC</span></div>
                </div>
              </div>
            )}
          </div>

          {/* Right column — Results */}
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {error && (
              <div style={{ ...cardStyle, borderColor: "#ef4444" }}>
                <div style={{ ...labelStyle, color: "#ef4444" }}>Error</div>
                <div style={{ fontSize: 13, color: "#ef4444", marginTop: 4 }}>{error}</div>
              </div>
            )}

            {result && (
              <>
                <div style={cardStyle}>
                  <div style={labelStyle}>Result</div>
                  <div style={{ display: "flex", gap: 12, marginTop: 8, fontSize: 12 }}>
                    <span style={{ color: result.success ? GREEN : "#ef4444" }}>
                      {result.success ? "Success" : "Failed"}
                    </span>
                    {result.confidence !== undefined && (
                      <span style={{ color: "#888" }}>Confidence: {(result.confidence * 100).toFixed(0)}%</span>
                    )}
                    {result.model_used && (
                      <span style={{ color: "#888" }}>Model: {result.model_used}</span>
                    )}
                    {result.tokens_used !== undefined && (
                      <span style={{ color: ACCENT }}>~{result.tokens_used} tokens</span>
                    )}
                  </div>
                </div>

                {result.description && (
                  <div style={cardStyle}>
                    <div style={labelStyle}>Description</div>
                    <div style={{ fontSize: 13, lineHeight: 1.6, marginTop: 6, whiteSpace: "pre-wrap" }}>
                      {result.description}
                    </div>
                  </div>
                )}

                {result.extracted_text && (
                  <div style={cardStyle}>
                    <div style={labelStyle}>Extracted Text</div>
                    <pre style={{ fontSize: 12, lineHeight: 1.5, marginTop: 6, whiteSpace: "pre-wrap", color: GREEN, background: alpha("#000", 0.3), padding: 10, borderRadius: 6 }}>
                      {result.extracted_text}
                    </pre>
                  </div>
                )}

                {result.structured_data && (
                  <div style={cardStyle}>
                    <div style={labelStyle}>Structured Data</div>
                    <pre style={{ fontSize: 11, lineHeight: 1.4, marginTop: 6, whiteSpace: "pre-wrap", color: BLUE, background: alpha("#000", 0.3), padding: 10, borderRadius: 6, overflow: "auto", maxHeight: 300 }}>
                      {JSON.stringify(result.structured_data, null, 2)}
                    </pre>
                  </div>
                )}

                {result.ui_elements && result.ui_elements.length > 0 && (
                  <div style={cardStyle}>
                    <div style={labelStyle}>UI Elements ({result.ui_elements.length})</div>
                    <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 6 }}>
                      {result.ui_elements.map((el: any, i: number) => (
                        <div key={i} style={{ display: "flex", gap: 10, fontSize: 12, padding: "6px 8px", background: alpha("#fff", 0.03), borderRadius: 4 }}>
                          <span style={{ color: ACCENT, minWidth: 70 }}>{typeof el.element_type === "string" ? el.element_type : el.element_type?.Other || Object.keys(el.element_type)[0]}</span>
                          <span style={{ flex: 1 }}>{el.label}</span>
                          <span style={{ color: el.interactive ? GREEN : "#666" }}>
                            {el.interactive ? "interactive" : "static"}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </>
            )}

            {!result && !error && !loading && (
              <div style={{ ...cardStyle, textAlign: "center", padding: 40 }}>
                <div style={{ color: "#555", fontSize: 14 }}>Upload an image and run a perception task</div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
