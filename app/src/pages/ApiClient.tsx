import { useState, useCallback, useMemo } from "react";
import "./api-client.css";

/* ─── Tauri invoke ─── */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function invoke(cmd: string, args?: Record<string, unknown>): Promise<any> {
  if (
    typeof window !== "undefined" &&
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    typeof (window as any).__TAURI__?.invoke === "function"
  ) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return (window as any).__TAURI__.invoke(cmd, args);
  }
  return JSON.stringify({ error: "Tauri backend not available" });
}

/* ─── types ─── */
type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";
type BodyType = "json" | "form" | "text" | "none";
type AuthType = "none" | "bearer" | "basic" | "api-key";
type ReqTab = "params" | "headers" | "body" | "auth";
type ResTab = "body" | "headers" | "cookies";

interface KeyValue {
  key: string;
  value: string;
  enabled: boolean;
}

interface ApiRequest {
  id: string;
  name: string;
  method: HttpMethod;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  bodyType: BodyType;
  bodyRaw: string;
  bodyForm: KeyValue[];
  authType: AuthType;
  authToken: string;
  authUser: string;
  authPass: string;
  authKeyName: string;
  authKeyValue: string;
  authKeyIn: "header" | "query";
}

interface ApiResponse {
  status: number;
  statusText: string;
  headers: Record<string, string>;
  body: string;
  duration: number;
  size: number;
  timestamp: number;
}

interface Collection {
  id: string;
  name: string;
  icon: string;
  requests: ApiRequest[];
  collapsed: boolean;
}

interface AuditEntry {
  id: string;
  method: HttpMethod;
  url: string;
  status: number;
  duration: number;
  timestamp: number;
  fuelCost: number;
}

/* ─── constants ─── */
const METHOD_COLORS: Record<HttpMethod, string> = {
  GET: "#34d399", POST: "#fbbf24", PUT: "#38bdf8", PATCH: "#a78bfa",
  DELETE: "#f87171", HEAD: "#64748b", OPTIONS: "#fb923c",
};

const STATUS_COLORS: Record<string, string> = {
  "2": "#34d399", "3": "#38bdf8", "4": "#fbbf24", "5": "#f87171",
};

function newRequest(method: HttpMethod = "GET", name = "New Request", url = ""): ApiRequest {
  return {
    id: `req-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`, name, method, url,
    params: [], headers: [{ key: "Content-Type", value: "application/json", enabled: true }],
    bodyType: "json", bodyRaw: "", bodyForm: [],
    authType: "none", authToken: "", authUser: "", authPass: "",
    authKeyName: "", authKeyValue: "", authKeyIn: "header",
  };
}

const INITIAL_COLLECTIONS: Collection[] = [
  {
    id: "col-1", name: "Nexus OS API", icon: "⬢", collapsed: false,
    requests: [
      { ...newRequest("GET", "List Agents", "http://localhost:8080/api/v1/agents"), id: "req-1", headers: [{ key: "Authorization", value: "Bearer nxs-token", enabled: true }, { key: "Content-Type", value: "application/json", enabled: true }] },
      { ...newRequest("GET", "Get Agent by ID", "http://localhost:8080/api/v1/agents/:id"), id: "req-2", params: [{ key: "id", value: "coder-001", enabled: true }] },
      { ...newRequest("POST", "Create Agent", "http://localhost:8080/api/v1/agents"), id: "req-3", bodyType: "json", bodyRaw: '{\n  "name": "Test Agent",\n  "agent_type": "coder",\n  "autonomy_level": 2,\n  "fuel_budget": 1000\n}' },
      { ...newRequest("GET", "Fuel Balance", "http://localhost:8080/api/v1/fuel/:agentId"), id: "req-4" },
      { ...newRequest("GET", "Audit Trail", "http://localhost:8080/api/v1/audit?limit=50"), id: "req-5" },
      { ...newRequest("DELETE", "Delete Agent", "http://localhost:8080/api/v1/agents/:id"), id: "req-6" },
    ],
  },
  {
    id: "col-2", name: "External APIs", icon: "◈", collapsed: true,
    requests: [
      { ...newRequest("POST", "Claude Chat", "https://api.anthropic.com/v1/messages"), id: "req-7", authType: "api-key", authKeyName: "x-api-key", authKeyValue: "", authKeyIn: "header", bodyType: "json", bodyRaw: '{\n  "model": "claude-sonnet-4-5-20250514",\n  "max_tokens": 1024,\n  "messages": [\n    {"role": "user", "content": "Hello"}\n  ]\n}' },
      { ...newRequest("GET", "GitHub Repos", "https://api.github.com/user/repos"), id: "req-8", authType: "bearer", authToken: "" },
    ],
  },
];

/* ─── JSON syntax highlight ─── */
function highlightJson(json: string): string {
  return json
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"([^"]+)"(?=\s*:)/g, '<span class="ac-json-key">"$1"</span>')
    .replace(/:\s*"([^"]*)"(?=[,\n\r}\]])/g, ': <span class="ac-json-string">"$1"</span>')
    .replace(/:\s*(\d+\.?\d*)(?=[,\n\r}\]])/g, ': <span class="ac-json-number">$1</span>')
    .replace(/:\s*(true|false)(?=[,\n\r}\]])/g, ': <span class="ac-json-bool">$1</span>')
    .replace(/:\s*(null)(?=[,\n\r}\]])/g, ': <span class="ac-json-null">$1</span>');
}

/* ─── component ─── */
export default function ApiClient() {
  const [collections, setCollections] = useState<Collection[]>(INITIAL_COLLECTIONS);
  const [activeReqId, setActiveReqId] = useState("req-1");
  const [response, setResponse] = useState<ApiResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [reqTab, setReqTab] = useState<ReqTab>("params");
  const [resTab, setResTab] = useState<ResTab>("body");
  const [showAudit, setShowAudit] = useState(false);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [fuelUsed, setFuelUsed] = useState(0);
  const [requestError, setRequestError] = useState<string | null>(null);

  const activeReq = useMemo(() => {
    for (const col of collections) {
      const found = col.requests.find(r => r.id === activeReqId);
      if (found) return found;
    }
    return collections[0].requests[0];
  }, [collections, activeReqId]);

  /* ─── update request ─── */
  const updateReq = useCallback((updates: Partial<ApiRequest>) => {
    setCollections(prev => prev.map(col => ({
      ...col,
      requests: col.requests.map(r => r.id === activeReqId ? { ...r, ...updates } : r),
    })));
  }, [activeReqId]);

  /* ─── send request (real HTTP via Tauri/curl) ─── */
  const sendRequest = useCallback(async () => {
    setLoading(true);
    setResTab("body");
    setRequestError(null);

    // Build headers array from enabled headers + auth
    const headers: [string, string][] = activeReq.headers
      .filter(h => h.enabled && h.key)
      .map(h => [h.key, h.value]);

    // Add auth headers
    if (activeReq.authType === "bearer" && activeReq.authToken) {
      headers.push(["Authorization", `Bearer ${activeReq.authToken}`]);
    } else if (activeReq.authType === "basic" && activeReq.authUser) {
      const encoded = btoa(`${activeReq.authUser}:${activeReq.authPass}`);
      headers.push(["Authorization", `Basic ${encoded}`]);
    } else if (activeReq.authType === "api-key" && activeReq.authKeyName && activeReq.authKeyIn === "header") {
      headers.push([activeReq.authKeyName, activeReq.authKeyValue]);
    }

    // Build URL with query params
    let finalUrl = activeReq.url;
    const enabledParams = activeReq.params.filter(p => p.enabled && p.key);
    if (enabledParams.length > 0) {
      const paramStr = enabledParams.map(p => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`).join("&");
      finalUrl += (finalUrl.includes("?") ? "&" : "?") + paramStr;
    }

    // Add API key as query param if configured
    if (activeReq.authType === "api-key" && activeReq.authKeyName && activeReq.authKeyIn === "query") {
      finalUrl += (finalUrl.includes("?") ? "&" : "?") + `${encodeURIComponent(activeReq.authKeyName)}=${encodeURIComponent(activeReq.authKeyValue)}`;
    }

    // Build body
    let body = "";
    if (activeReq.bodyType === "json" || activeReq.bodyType === "text") {
      body = activeReq.bodyRaw;
    } else if (activeReq.bodyType === "form") {
      body = activeReq.bodyForm.filter(f => f.enabled && f.key).map(f => `${encodeURIComponent(f.key)}=${encodeURIComponent(f.value)}`).join("&");
    }

    try {
      const raw: string = await invoke("api_client_request", {
        method: activeReq.method,
        url: finalUrl,
        headersJson: JSON.stringify(headers),
        body,
      });

      const data = JSON.parse(raw);

      // Flatten header_json from curl (it's an object with array values)
      let flatHeaders: Record<string, string> = {};
      if (data.headers && typeof data.headers === "object") {
        for (const [k, v] of Object.entries(data.headers)) {
          if (Array.isArray(v)) {
            flatHeaders[k] = (v as string[]).join(", ");
          } else if (typeof v === "string") {
            flatHeaders[k] = v;
          }
        }
      }

      const res: ApiResponse = {
        status: data.status,
        statusText: data.statusText,
        headers: flatHeaders,
        body: data.body,
        duration: data.duration,
        size: data.size,
        timestamp: Date.now(),
      };

      setResponse(res);
      const cost = data.fuel_cost ?? (Math.ceil((data.body?.length ?? 0) / 1000) + 2);
      setFuelUsed(f => f + cost);
      setAudit(prev => [{ id: `ae-${Date.now()}`, method: activeReq.method, url: finalUrl, status: res.status, duration: res.duration, timestamp: Date.now(), fuelCost: cost }, ...prev]);
    } catch (err) {
      setRequestError(String(err));
      setResponse(null);
    } finally {
      setLoading(false);
    }
  }, [activeReq]);

  /* ─── collection management ─── */
  const toggleCollection = (id: string) => {
    setCollections(prev => prev.map(c => c.id === id ? { ...c, collapsed: !c.collapsed } : c));
  };

  const addRequest = (colId: string) => {
    const req = newRequest();
    setCollections(prev => prev.map(c => c.id === colId ? { ...c, requests: [...c.requests, req] } : c));
    setActiveReqId(req.id);
  };

  const deleteRequest = (colId: string, reqId: string) => {
    setCollections(prev => prev.map(c => c.id === colId ? { ...c, requests: c.requests.filter(r => r.id !== reqId) } : c));
    if (activeReqId === reqId) {
      const first = collections.flatMap(c => c.requests).find(r => r.id !== reqId);
      if (first) setActiveReqId(first.id);
    }
  };

  const addCollection = () => {
    const col: Collection = { id: `col-${Date.now()}`, name: "New Collection", icon: "◇", requests: [], collapsed: false };
    setCollections(prev => [...prev, col]);
  };

  /* ─── KV helpers ─── */
  const updateKV = (field: "params" | "headers" | "bodyForm", idx: number, updates: Partial<KeyValue>) => {
    updateReq({ [field]: activeReq[field].map((kv, i) => i === idx ? { ...kv, ...updates } : kv) });
  };

  const addKV = (field: "params" | "headers" | "bodyForm") => {
    updateReq({ [field]: [...activeReq[field], { key: "", value: "", enabled: true }] });
  };

  const removeKV = (field: "params" | "headers" | "bodyForm", idx: number) => {
    updateReq({ [field]: activeReq[field].filter((_, i) => i !== idx) });
  };

  const formatTimestamp = (ts: number) => {
    const diff = Date.now() - ts;
    if (diff < 60000) return "just now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    return new Date(ts).toLocaleTimeString();
  };

  const statusColor = (status: number) => STATUS_COLORS[String(status).charAt(0)] ?? "#64748b";

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    return `${(bytes / 1024).toFixed(1)} KB`;
  };

  /* ─── render ─── */
  return (
    <div className="ac-container">
      {/* ─── Sidebar ─── */}
      <aside className="ac-sidebar">
        <div className="ac-sidebar-header">
          <h2 className="ac-sidebar-title">API Client</h2>
          <div className="ac-sidebar-actions">
            <button className="ac-btn-icon" onClick={addCollection} title="New collection">+</button>
            <button className={`ac-btn-icon ${showAudit ? "active" : ""}`} onClick={() => setShowAudit(!showAudit)} title="Audit">⧉</button>
          </div>
        </div>

        {/* audit panel */}
        {showAudit && (
          <div className="ac-audit-panel">
            <div className="ac-audit-header">Audit Trail</div>
            {audit.length === 0 && <div style={{ padding: 12, color: "#64748b", fontSize: 12 }}>No requests yet</div>}
            {audit.slice(0, 8).map(entry => (
              <div key={entry.id} className="ac-audit-item">
                <div className="ac-audit-item-top">
                  <span className="ac-method-badge-sm" style={{ color: METHOD_COLORS[entry.method] }}>{entry.method}</span>
                  <span className="ac-audit-status" style={{ color: statusColor(entry.status) }}>{entry.status}</span>
                  <span className="ac-audit-time">{formatTimestamp(entry.timestamp)}</span>
                </div>
                <div className="ac-audit-url">{entry.url.replace(/https?:\/\//, "").slice(0, 35)}...</div>
                <div className="ac-audit-meta">{entry.duration}ms · ⚡{entry.fuelCost}</div>
              </div>
            ))}
          </div>
        )}

        {/* collections */}
        {!showAudit && (
          <div className="ac-collections">
            {collections.map(col => (
              <div key={col.id} className="ac-collection">
                <div className="ac-collection-header" onClick={() => toggleCollection(col.id)}>
                  <span className="ac-collection-arrow">{col.collapsed ? "▸" : "▾"}</span>
                  <span className="ac-collection-icon">{col.icon}</span>
                  <span className="ac-collection-name">{col.name}</span>
                  <span className="ac-collection-count">{col.requests.length}</span>
                  <button className="ac-btn-tiny" onClick={e => { e.stopPropagation(); addRequest(col.id); }} title="Add request">+</button>
                </div>
                {!col.collapsed && (
                  <div className="ac-collection-requests">
                    {col.requests.map(req => (
                      <div key={req.id} className={`ac-req-item ${activeReqId === req.id ? "active" : ""}`} onClick={() => { setActiveReqId(req.id); setResponse(null); setRequestError(null); }}>
                        <span className="ac-req-method" style={{ color: METHOD_COLORS[req.method] }}>{req.method.slice(0, 3)}</span>
                        <span className="ac-req-name">{req.name}</span>
                        <button className="ac-btn-del" onClick={e => { e.stopPropagation(); deleteRequest(col.id, req.id); }}>×</button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </aside>

      {/* ─── Main ─── */}
      <div className="ac-main">
        {/* URL bar */}
        <div className="ac-url-bar">
          <select className="ac-method-select" value={activeReq.method} onChange={e => updateReq({ method: e.target.value as HttpMethod })} style={{ color: METHOD_COLORS[activeReq.method] }}>
            {(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] as HttpMethod[]).map(m => (
              <option key={m} value={m} style={{ color: METHOD_COLORS[m] }}>{m}</option>
            ))}
          </select>
          <input className="ac-url-input" value={activeReq.url} onChange={e => updateReq({ url: e.target.value })} placeholder="Enter request URL..." onKeyDown={e => e.key === "Enter" && sendRequest()} />
          <button className="ac-btn-send" onClick={sendRequest} disabled={loading}>
            {loading ? "Sending..." : "Send"}
          </button>
        </div>

        {/* request name */}
        <div className="ac-req-name-bar">
          <input className="ac-req-name-input" value={activeReq.name} onChange={e => updateReq({ name: e.target.value })} />
        </div>

        {/* request tabs */}
        <div className="ac-req-tabs">
          {(["params", "headers", "body", "auth"] as ReqTab[]).map(tab => (
            <button key={tab} className={`ac-req-tab ${reqTab === tab ? "active" : ""}`} onClick={() => setReqTab(tab)}>
              {tab.charAt(0).toUpperCase() + tab.slice(1)}
              {tab === "params" && activeReq.params.length > 0 && <span className="ac-tab-count">{activeReq.params.length}</span>}
              {tab === "headers" && <span className="ac-tab-count">{activeReq.headers.length}</span>}
            </button>
          ))}
        </div>

        <div className="ac-req-body-area">
          {/* params */}
          {reqTab === "params" && (
            <div className="ac-kv-section">
              <div className="ac-kv-header">
                <span>Query Parameters</span>
                <button className="ac-btn-add" onClick={() => addKV("params")}>+ Add</button>
              </div>
              {activeReq.params.length === 0 && <div className="ac-kv-empty">No parameters. Click + Add to create one.</div>}
              {activeReq.params.map((kv, i) => (
                <div key={i} className="ac-kv-row">
                  <input type="checkbox" checked={kv.enabled} onChange={e => updateKV("params", i, { enabled: e.target.checked })} className="ac-kv-check" />
                  <input className="ac-kv-key" value={kv.key} onChange={e => updateKV("params", i, { key: e.target.value })} placeholder="Key" />
                  <input className="ac-kv-value" value={kv.value} onChange={e => updateKV("params", i, { value: e.target.value })} placeholder="Value" />
                  <button className="ac-btn-remove" onClick={() => removeKV("params", i)}>×</button>
                </div>
              ))}
            </div>
          )}

          {/* headers */}
          {reqTab === "headers" && (
            <div className="ac-kv-section">
              <div className="ac-kv-header">
                <span>Headers</span>
                <button className="ac-btn-add" onClick={() => addKV("headers")}>+ Add</button>
              </div>
              {activeReq.headers.map((kv, i) => (
                <div key={i} className="ac-kv-row">
                  <input type="checkbox" checked={kv.enabled} onChange={e => updateKV("headers", i, { enabled: e.target.checked })} className="ac-kv-check" />
                  <input className="ac-kv-key" value={kv.key} onChange={e => updateKV("headers", i, { key: e.target.value })} placeholder="Header name" />
                  <input className="ac-kv-value" value={kv.value} onChange={e => updateKV("headers", i, { value: e.target.value })} placeholder="Value" />
                  <button className="ac-btn-remove" onClick={() => removeKV("headers", i)}>×</button>
                </div>
              ))}
            </div>
          )}

          {/* body */}
          {reqTab === "body" && (
            <div className="ac-body-section">
              <div className="ac-body-type-bar">
                {(["none", "json", "form", "text"] as BodyType[]).map(bt => (
                  <button key={bt} className={`ac-body-type-btn ${activeReq.bodyType === bt ? "active" : ""}`} onClick={() => updateReq({ bodyType: bt })}>
                    {bt.charAt(0).toUpperCase() + bt.slice(1)}
                  </button>
                ))}
              </div>
              {activeReq.bodyType === "none" && <div className="ac-kv-empty">This request does not have a body.</div>}
              {activeReq.bodyType === "json" && (
                <textarea className="ac-body-editor" value={activeReq.bodyRaw} onChange={e => updateReq({ bodyRaw: e.target.value })} placeholder='{"key": "value"}' spellCheck={false} />
              )}
              {activeReq.bodyType === "text" && (
                <textarea className="ac-body-editor" value={activeReq.bodyRaw} onChange={e => updateReq({ bodyRaw: e.target.value })} placeholder="Raw text body..." spellCheck={false} />
              )}
              {activeReq.bodyType === "form" && (
                <div className="ac-kv-section">
                  <div className="ac-kv-header">
                    <span>Form Data</span>
                    <button className="ac-btn-add" onClick={() => addKV("bodyForm")}>+ Add</button>
                  </div>
                  {activeReq.bodyForm.map((kv, i) => (
                    <div key={i} className="ac-kv-row">
                      <input type="checkbox" checked={kv.enabled} onChange={e => updateKV("bodyForm", i, { enabled: e.target.checked })} className="ac-kv-check" />
                      <input className="ac-kv-key" value={kv.key} onChange={e => updateKV("bodyForm", i, { key: e.target.value })} placeholder="Key" />
                      <input className="ac-kv-value" value={kv.value} onChange={e => updateKV("bodyForm", i, { value: e.target.value })} placeholder="Value" />
                      <button className="ac-btn-remove" onClick={() => removeKV("bodyForm", i)}>×</button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* auth */}
          {reqTab === "auth" && (
            <div className="ac-auth-section">
              <div className="ac-auth-type-bar">
                {(["none", "bearer", "basic", "api-key"] as AuthType[]).map(at => (
                  <button key={at} className={`ac-auth-type-btn ${activeReq.authType === at ? "active" : ""}`} onClick={() => updateReq({ authType: at })}>
                    {at === "api-key" ? "API Key" : at.charAt(0).toUpperCase() + at.slice(1)}
                  </button>
                ))}
              </div>
              {activeReq.authType === "none" && <div className="ac-kv-empty">No authentication configured.</div>}
              {activeReq.authType === "bearer" && (
                <div className="ac-auth-fields">
                  <label>Token</label>
                  <input className="ac-auth-input" value={activeReq.authToken} onChange={e => updateReq({ authToken: e.target.value })} placeholder="Bearer token..." type="password" />
                  <div className="ac-auth-hint">Token will be sent as: Authorization: Bearer &lt;token&gt;</div>
                </div>
              )}
              {activeReq.authType === "basic" && (
                <div className="ac-auth-fields">
                  <label>Username</label>
                  <input className="ac-auth-input" value={activeReq.authUser} onChange={e => updateReq({ authUser: e.target.value })} placeholder="Username" />
                  <label>Password</label>
                  <input className="ac-auth-input" value={activeReq.authPass} onChange={e => updateReq({ authPass: e.target.value })} placeholder="Password" type="password" />
                </div>
              )}
              {activeReq.authType === "api-key" && (
                <div className="ac-auth-fields">
                  <label>Key Name</label>
                  <input className="ac-auth-input" value={activeReq.authKeyName} onChange={e => updateReq({ authKeyName: e.target.value })} placeholder="e.g. x-api-key" />
                  <label>Value</label>
                  <input className="ac-auth-input" value={activeReq.authKeyValue} onChange={e => updateReq({ authKeyValue: e.target.value })} placeholder="API key value" type="password" />
                  <label>Add to</label>
                  <div className="ac-auth-location">
                    <button className={`ac-auth-loc-btn ${activeReq.authKeyIn === "header" ? "active" : ""}`} onClick={() => updateReq({ authKeyIn: "header" })}>Header</button>
                    <button className={`ac-auth-loc-btn ${activeReq.authKeyIn === "query" ? "active" : ""}`} onClick={() => updateReq({ authKeyIn: "query" })}>Query Param</button>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {/* ─── Response ─── */}
        <div className="ac-response-area">
          <div className="ac-res-header">
            <span className="ac-res-label">Response</span>
            {response && (
              <div className="ac-res-meta">
                <span className="ac-res-status" style={{ color: statusColor(response.status) }}>
                  {response.status} {response.statusText}
                </span>
                <span className="ac-res-duration">{response.duration}ms</span>
                <span className="ac-res-size">{formatSize(response.size)}</span>
              </div>
            )}
            {response && (
              <div className="ac-res-tabs">
                {(["body", "headers", "cookies"] as ResTab[]).map(tab => (
                  <button key={tab} className={`ac-res-tab ${resTab === tab ? "active" : ""}`} onClick={() => setResTab(tab)}>
                    {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  </button>
                ))}
              </div>
            )}
          </div>

          <div className="ac-res-body">
            {loading && (
              <div className="ac-loading">
                <div className="ac-loading-spinner" />
                <span>Sending request...</span>
              </div>
            )}
            {!loading && requestError && (
              <div style={{ padding: 16, color: "#f87171", fontSize: 13 }}>
                <strong>Error:</strong> {requestError}
              </div>
            )}
            {!loading && !response && !requestError && (
              <div className="ac-no-response">
                <div className="ac-no-response-icon">⤴</div>
                <div>Click Send to make a request</div>
                <div className="ac-no-response-hint">Or press Enter in the URL bar</div>
              </div>
            )}
            {!loading && response && resTab === "body" && (
              <pre className="ac-json-viewer" dangerouslySetInnerHTML={{ __html: highlightJson(response.body) }} />
            )}
            {!loading && response && resTab === "headers" && (
              <div className="ac-res-headers-list">
                {Object.entries(response.headers).map(([k, v]) => (
                  <div key={k} className="ac-res-header-row">
                    <span className="ac-res-header-key">{k}</span>
                    <span className="ac-res-header-val">{v}</span>
                  </div>
                ))}
              </div>
            )}
            {!loading && response && resTab === "cookies" && (
              <div className="ac-kv-empty">No cookies in response.</div>
            )}
          </div>
        </div>
      </div>

      {/* ─── Status Bar ─── */}
      <div className="ac-status-bar">
        <span className="ac-status-item">{activeReq.method} {activeReq.url.replace(/https?:\/\//, "").slice(0, 40)}</span>
        {response && <span className="ac-status-item" style={{ color: statusColor(response.status) }}>{response.status} {response.statusText}</span>}
        {response && <span className="ac-status-item">{response.duration}ms</span>}
        <span className="ac-status-item">⚡ {fuelUsed} fuel</span>
        <span className="ac-status-item ac-status-right">{audit.length} requests logged</span>
        <span className="ac-status-item">{collections.reduce((s, c) => s + c.requests.length, 0)} saved requests</span>
      </div>
    </div>
  );
}
