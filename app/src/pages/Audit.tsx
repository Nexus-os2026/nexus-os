import { useCallback, useEffect, useMemo, useState } from "react";
import "./audit.css";
import type { AuditEventRow, AuditChainStatusRow } from "../types";
import {
  getAuditLog,
  getAuditChainStatus,
  hasDesktopRuntime,
  tracingStartTrace,
  tracingEndTrace,
  tracingStartSpan,
  tracingEndSpan,
  tracingGetTrace,
  tracingListTraces,
  verifyGovernanceInvariants,
  verifySpecificInvariant,
  exportComplianceReport,
  auditStatistics,
  auditExportReport,
  auditSearch,
} from "../api/backend";
import type { AuditStatistics, AuditSearchQuery } from "../api/backend";

interface AuditProps {
  events: AuditEventRow[];
  onRefresh?: () => void;
}

const EVENT_TYPE_COLORS: Record<string, string> = {
  StateChange: "#3b82f6",
  ToolCall: "#22c55e",
  LlmCall: "#a855f7",
  Error: "#ef4444",
  UserAction: "#f59e0b",
};

function agentColor(agentId: string): string {
  let hash = 0;
  for (let i = 0; i < agentId.length; i++) {
    hash = agentId.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue}, 70%, 55%)`;
}

function shortAgent(agentId: string): string {
  return agentId.length > 12 ? agentId.slice(0, 8) + "..." : agentId;
}

type StatusType = "Success" | "Failed" | "Pending";

function eventStatus(eventType: string): StatusType {
  if (eventType.toLowerCase().includes("error")) return "Failed";
  if (eventType.toLowerCase().includes("approval") && eventType.toLowerCase().includes("required")) return "Pending";
  return "Success";
}

function fuelCost(payload: Record<string, unknown>): number | null {
  if (typeof payload.consumed === "number") return payload.consumed;
  if (typeof payload.tokens === "number") return Math.round((payload.tokens as number) * 0.3);
  if (typeof payload.cost === "number") return Math.round((payload.cost as number) * 10000);
  if (typeof payload.fuel === "number") return payload.fuel as number;
  return null;
}

function formatDateTime(timestamp: number): string {
  const d = new Date(timestamp * 1000);
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

type SortField = "index" | "timestamp" | "agent" | "action" | "status" | "fuel";
type SortDir = "asc" | "desc";
type AuditTab = "log" | "statistics" | "tracing" | "governance";

// --- Distributed Tracing Tab ---

interface TraceSummary {
  trace_id: string;
  operation_name: string;
  agent_id?: string;
  start_time?: string;
  status?: string;
  span_count?: number;
}

interface SpanDetail {
  span_id: string;
  parent_span_id?: string;
  operation_name: string;
  agent_id?: string;
  status?: string;
  start_time?: string;
  end_time?: string;
  error_message?: string;
}

interface TraceDetail {
  trace_id: string;
  operation_name: string;
  agent_id?: string;
  spans: SpanDetail[];
  status?: string;
}

function parseJsonSafe<T>(raw: string, fallback: T): T {
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function DistributedTracingTab(): JSX.Element {
  const [traces, setTraces] = useState<TraceSummary[]>([]);
  const [tracesLoading, setTracesLoading] = useState(false);
  const [tracesError, setTracesError] = useState<string | null>(null);

  // New trace form
  const [newTraceOp, setNewTraceOp] = useState("");
  const [newTraceAgent, setNewTraceAgent] = useState("");
  const [startingTrace, setStartingTrace] = useState(false);
  const [startTraceResult, setStartTraceResult] = useState<string | null>(null);

  // Trace detail view
  const [selectedTraceId, setSelectedTraceId] = useState<string | null>(null);
  const [traceDetail, setTraceDetail] = useState<TraceDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  // End trace
  const [endingTraceId, setEndingTraceId] = useState<string | null>(null);

  // Start span form
  const [spanTraceId, setSpanTraceId] = useState("");
  const [spanParentId, setSpanParentId] = useState("");
  const [spanOpName, setSpanOpName] = useState("");
  const [spanAgentId, setSpanAgentId] = useState("");
  const [startingSpan, setStartingSpan] = useState(false);
  const [startSpanResult, setStartSpanResult] = useState<string | null>(null);

  // End span form
  const [endSpanId, setEndSpanId] = useState("");
  const [endSpanStatus, setEndSpanStatus] = useState("ok");
  const [endSpanError, setEndSpanError] = useState("");
  const [endingSpan, setEndingSpan] = useState(false);
  const [endSpanResult, setEndSpanResult] = useState<string | null>(null);

  async function fetchTraces(): Promise<void> {
    setTracesLoading(true);
    setTracesError(null);
    try {
      const raw = await tracingListTraces(50);
      const parsed = parseJsonSafe<TraceSummary[]>(raw, []);
      setTraces(Array.isArray(parsed) ? parsed : []);
    } catch (err) {
      setTracesError(err instanceof Error ? err.message : "Failed to fetch traces");
    }
    setTracesLoading(false);
  }

  useEffect(() => {
    void fetchTraces();
  }, []);

  async function handleStartTrace(): Promise<void> {
    if (!newTraceOp.trim()) return;
    setStartingTrace(true);
    setStartTraceResult(null);
    try {
      const result = await tracingStartTrace(newTraceOp.trim(), newTraceAgent.trim() || undefined);
      setStartTraceResult(result);
      setNewTraceOp("");
      setNewTraceAgent("");
      void fetchTraces();
    } catch (err) {
      setStartTraceResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    }
    setStartingTrace(false);
  }

  async function handleEndTrace(traceId: string): Promise<void> {
    setEndingTraceId(traceId);
    try {
      await tracingEndTrace(traceId);
      void fetchTraces();
      if (selectedTraceId === traceId) {
        void handleViewTrace(traceId);
      }
    } catch {
      // ignore
    }
    setEndingTraceId(null);
  }

  async function handleViewTrace(traceId: string): Promise<void> {
    setSelectedTraceId(traceId);
    setDetailLoading(true);
    setDetailError(null);
    try {
      const raw = await tracingGetTrace(traceId);
      const parsed = parseJsonSafe<TraceDetail>(raw, { trace_id: traceId, operation_name: "unknown", spans: [] });
      setTraceDetail(parsed);
    } catch (err) {
      setDetailError(err instanceof Error ? err.message : "Failed to load trace");
    }
    setDetailLoading(false);
  }

  async function handleStartSpan(): Promise<void> {
    if (!spanTraceId.trim() || !spanOpName.trim()) return;
    setStartingSpan(true);
    setStartSpanResult(null);
    try {
      const result = await tracingStartSpan(
        spanTraceId.trim(),
        spanParentId.trim(),
        spanOpName.trim(),
        spanAgentId.trim() || undefined,
      );
      setStartSpanResult(result);
      setSpanOpName("");
      setSpanParentId("");
      setSpanAgentId("");
      if (selectedTraceId === spanTraceId.trim()) {
        void handleViewTrace(spanTraceId.trim());
      }
    } catch (err) {
      setStartSpanResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    }
    setStartingSpan(false);
  }

  async function handleEndSpan(): Promise<void> {
    if (!endSpanId.trim()) return;
    setEndingSpan(true);
    setEndSpanResult(null);
    try {
      const result = await tracingEndSpan(
        endSpanId.trim(),
        endSpanStatus,
        endSpanError.trim() || undefined,
      );
      setEndSpanResult(result);
      setEndSpanId("");
      setEndSpanError("");
      if (selectedTraceId) {
        void handleViewTrace(selectedTraceId);
      }
    } catch (err) {
      setEndSpanResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    }
    setEndingSpan(false);
  }

  return (
    <div className="audit-tracing-tab">
      <div className="audit-tracing-top">
        {/* Start New Trace */}
        <div className="audit-panel">
          <h3 className="audit-panel-title">Start New Trace</h3>
          <div className="audit-form-row">
            <input
              className="audit-search"
              value={newTraceOp}
              onChange={(e) => setNewTraceOp(e.target.value)}
              placeholder="Operation name (required)"
            />
            <input
              className="audit-search"
              value={newTraceAgent}
              onChange={(e) => setNewTraceAgent(e.target.value)}
              placeholder="Agent ID (optional)"
            />
            <button
              type="button"
              className="audit-verify-btn"
              disabled={startingTrace || !newTraceOp.trim()}
              onClick={() => void handleStartTrace()}
            >
              {startingTrace ? "Starting..." : "START TRACE"}
            </button>
          </div>
          {startTraceResult && (
            <div className="audit-result-msg mono">{startTraceResult}</div>
          )}
        </div>

        {/* Start Span */}
        <div className="audit-panel">
          <h3 className="audit-panel-title">Start Span</h3>
          <div className="audit-form-row">
            <input
              className="audit-search"
              value={spanTraceId}
              onChange={(e) => setSpanTraceId(e.target.value)}
              placeholder="Trace ID (required)"
            />
            <input
              className="audit-search"
              value={spanParentId}
              onChange={(e) => setSpanParentId(e.target.value)}
              placeholder="Parent Span ID"
            />
            <input
              className="audit-search"
              value={spanOpName}
              onChange={(e) => setSpanOpName(e.target.value)}
              placeholder="Operation name (required)"
            />
            <input
              className="audit-search"
              value={spanAgentId}
              onChange={(e) => setSpanAgentId(e.target.value)}
              placeholder="Agent ID (optional)"
            />
            <button
              type="button"
              className="audit-verify-btn"
              disabled={startingSpan || !spanTraceId.trim() || !spanOpName.trim()}
              onClick={() => void handleStartSpan()}
            >
              {startingSpan ? "Starting..." : "START SPAN"}
            </button>
          </div>
          {startSpanResult && (
            <div className="audit-result-msg mono">{startSpanResult}</div>
          )}
        </div>

        {/* End Span */}
        <div className="audit-panel">
          <h3 className="audit-panel-title">End Span</h3>
          <div className="audit-form-row">
            <input
              className="audit-search"
              value={endSpanId}
              onChange={(e) => setEndSpanId(e.target.value)}
              placeholder="Span ID (required)"
            />
            <select
              className="audit-select"
              value={endSpanStatus}
              onChange={(e) => setEndSpanStatus(e.target.value)}
            >
              <option value="ok">OK</option>
              <option value="error">Error</option>
              <option value="cancelled">Cancelled</option>
            </select>
            <input
              className="audit-search"
              value={endSpanError}
              onChange={(e) => setEndSpanError(e.target.value)}
              placeholder="Error message (optional)"
            />
            <button
              type="button"
              className="audit-verify-btn"
              disabled={endingSpan || !endSpanId.trim()}
              onClick={() => void handleEndSpan()}
            >
              {endingSpan ? "Ending..." : "END SPAN"}
            </button>
          </div>
          {endSpanResult && (
            <div className="audit-result-msg mono">{endSpanResult}</div>
          )}
        </div>
      </div>

      {/* Trace List */}
      <div className="audit-panel">
        <div className="audit-panel-header">
          <h3 className="audit-panel-title">Traces</h3>
          <button
            type="button"
            className="audit-verify-btn"
            disabled={tracesLoading}
            onClick={() => void fetchTraces()}
          >
            {tracesLoading ? "Loading..." : "REFRESH"}
          </button>
        </div>
        {tracesError && <div className="audit-error-msg">{tracesError}</div>}
        {traces.length === 0 && !tracesLoading && !tracesError && (
          <p className="audit-empty-msg">No traces found. Start a new trace above.</p>
        )}
        {traces.length > 0 && (
          <div className="audit-table-wrap">
            <table className="audit-table">
              <thead>
                <tr>
                  <th className="audit-th">Trace ID</th>
                  <th className="audit-th">Operation</th>
                  <th className="audit-th">Agent</th>
                  <th className="audit-th">Status</th>
                  <th className="audit-th">Spans</th>
                  <th className="audit-th">Actions</th>
                </tr>
              </thead>
              <tbody>
                {traces.map((t, idx) => (
                  <tr
                    key={t.trace_id}
                    className={`audit-row ${idx % 2 === 0 ? "even" : "odd"} ${selectedTraceId === t.trace_id ? "expanded" : ""}`}
                  >
                    <td className="audit-td audit-td-mono">{t.trace_id.slice(0, 12)}...</td>
                    <td className="audit-td">{t.operation_name}</td>
                    <td className="audit-td">{t.agent_id ? shortAgent(t.agent_id) : "-"}</td>
                    <td className="audit-td">
                      <span className={`audit-status-badge ${(t.status ?? "active").toLowerCase()}`}>
                        {t.status ?? "active"}
                      </span>
                    </td>
                    <td className="audit-td audit-td-mono">{t.span_count ?? "-"}</td>
                    <td className="audit-td">
                      <div className="audit-action-btns">
                        <button
                          type="button"
                          className="audit-copy-btn"
                          onClick={() => void handleViewTrace(t.trace_id)}
                          title="View trace details"
                        >
                          View
                        </button>
                        <button
                          type="button"
                          className="audit-copy-btn"
                          onClick={() => void handleEndTrace(t.trace_id)}
                          disabled={endingTraceId === t.trace_id}
                          title="End trace"
                        >
                          {endingTraceId === t.trace_id ? "..." : "End"}
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Trace Detail */}
      {selectedTraceId && (
        <div className="audit-detail-panel">
          <div className="audit-detail-header">
            <h3>Trace: {selectedTraceId}</h3>
            <button type="button" className="audit-detail-close" onClick={() => { setSelectedTraceId(null); setTraceDetail(null); }}>&#x2715;</button>
          </div>
          {detailLoading && <p className="audit-empty-msg">Loading trace details...</p>}
          {detailError && <div className="audit-error-msg">{detailError}</div>}
          {traceDetail && !detailLoading && (
            <>
              <div className="audit-detail-grid">
                <div className="audit-detail-field">
                  <span className="audit-detail-label">Operation</span>
                  <span className="audit-detail-value">{traceDetail.operation_name}</span>
                </div>
                <div className="audit-detail-field">
                  <span className="audit-detail-label">Agent</span>
                  <span className="audit-detail-value mono">{traceDetail.agent_id ?? "-"}</span>
                </div>
                <div className="audit-detail-field">
                  <span className="audit-detail-label">Status</span>
                  <span className="audit-detail-value">{traceDetail.status ?? "active"}</span>
                </div>
                <div className="audit-detail-field">
                  <span className="audit-detail-label">Span Count</span>
                  <span className="audit-detail-value">{traceDetail.spans?.length ?? 0}</span>
                </div>
              </div>
              {traceDetail.spans && traceDetail.spans.length > 0 && (
                <div className="audit-table-wrap" style={{ maxHeight: "250px" }}>
                  <table className="audit-table">
                    <thead>
                      <tr>
                        <th className="audit-th">Span ID</th>
                        <th className="audit-th">Parent</th>
                        <th className="audit-th">Operation</th>
                        <th className="audit-th">Agent</th>
                        <th className="audit-th">Status</th>
                        <th className="audit-th">Error</th>
                      </tr>
                    </thead>
                    <tbody>
                      {traceDetail.spans.map((s, idx) => (
                        <tr key={s.span_id} className={`audit-row ${idx % 2 === 0 ? "even" : "odd"}`}>
                          <td className="audit-td audit-td-mono">{s.span_id.slice(0, 12)}...</td>
                          <td className="audit-td audit-td-mono">{s.parent_span_id ? s.parent_span_id.slice(0, 8) + "..." : "-"}</td>
                          <td className="audit-td">{s.operation_name}</td>
                          <td className="audit-td">{s.agent_id ? shortAgent(s.agent_id) : "-"}</td>
                          <td className="audit-td">
                            <span className={`audit-status-badge ${(s.status ?? "active").toLowerCase()}`}>
                              {s.status ?? "active"}
                            </span>
                          </td>
                          <td className="audit-td audit-td-mono">{s.error_message ?? "-"}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
              {(!traceDetail.spans || traceDetail.spans.length === 0) && (
                <p className="audit-empty-msg">No spans in this trace.</p>
              )}
              <div className="audit-detail-payload">
                <span className="audit-detail-label">Raw Trace JSON</span>
                <pre className="audit-detail-json">{JSON.stringify(traceDetail, null, 2)}</pre>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

// --- Governance Verification Tab ---

interface InvariantResult {
  name: string;
  status: string;
  message?: string;
}

function GovernanceVerificationTab(): JSX.Element {
  // Verify all invariants
  const [allResult, setAllResult] = useState<string | null>(null);
  const [allLoading, setAllLoading] = useState(false);
  const [allError, setAllError] = useState<string | null>(null);

  // Verify specific invariant
  const [invariantName, setInvariantName] = useState("");
  const [specificResult, setSpecificResult] = useState<string | null>(null);
  const [specificLoading, setSpecificLoading] = useState(false);
  const [specificError, setSpecificError] = useState<string | null>(null);

  // Export compliance report
  const [reportResult, setReportResult] = useState<string | null>(null);
  const [reportLoading, setReportLoading] = useState(false);
  const [reportError, setReportError] = useState<string | null>(null);

  async function handleVerifyAll(): Promise<void> {
    setAllLoading(true);
    setAllError(null);
    setAllResult(null);
    try {
      const raw = await verifyGovernanceInvariants();
      setAllResult(raw);
    } catch (err) {
      setAllError(err instanceof Error ? err.message : "Failed to verify invariants");
    }
    setAllLoading(false);
  }

  async function handleVerifySpecific(): Promise<void> {
    if (!invariantName.trim()) return;
    setSpecificLoading(true);
    setSpecificError(null);
    setSpecificResult(null);
    try {
      const raw = await verifySpecificInvariant(invariantName.trim());
      setSpecificResult(raw);
    } catch (err) {
      setSpecificError(err instanceof Error ? err.message : "Failed to verify invariant");
    }
    setSpecificLoading(false);
  }

  async function handleExportReport(): Promise<void> {
    setReportLoading(true);
    setReportError(null);
    setReportResult(null);
    try {
      const raw = await exportComplianceReport();
      setReportResult(raw);
    } catch (err) {
      setReportError(err instanceof Error ? err.message : "Failed to export compliance report");
    }
    setReportLoading(false);
  }

  function renderParsedResult(raw: string): JSX.Element {
    const parsed = parseJsonSafe<InvariantResult[] | Record<string, unknown> | null>(raw, null);
    if (parsed && Array.isArray(parsed)) {
      return (
        <div className="audit-table-wrap">
          <table className="audit-table">
            <thead>
              <tr>
                <th className="audit-th">Invariant</th>
                <th className="audit-th">Status</th>
                <th className="audit-th">Message</th>
              </tr>
            </thead>
            <tbody>
              {parsed.map((item: InvariantResult, idx: number) => (
                <tr key={item.name ?? idx} className={`audit-row ${idx % 2 === 0 ? "even" : "odd"}`}>
                  <td className="audit-td">{item.name ?? "-"}</td>
                  <td className="audit-td">
                    <span className={`audit-status-badge ${(item.status ?? "unknown").toLowerCase() === "pass" || (item.status ?? "").toLowerCase() === "ok" ? "success" : "failed"}`}>
                      {item.status ?? "unknown"}
                    </span>
                  </td>
                  <td className="audit-td">{item.message ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
    }
    return <pre className="audit-detail-json">{typeof raw === "string" ? raw : JSON.stringify(raw, null, 2)}</pre>;
  }

  return (
    <div className="audit-governance-tab">
      {/* Verify All Invariants */}
      <div className="audit-panel">
        <div className="audit-panel-header">
          <h3 className="audit-panel-title">Verify All Governance Invariants</h3>
          <button
            type="button"
            className="audit-verify-btn"
            disabled={allLoading}
            onClick={() => void handleVerifyAll()}
          >
            {allLoading ? "Verifying..." : "VERIFY ALL"}
          </button>
        </div>
        <p className="audit-panel-desc">
          Checks all kernel governance invariants: capability checks, fuel budgets, audit integrity, PII redaction, HITL approval, and unsafe code prohibition.
        </p>
        {allError && <div className="audit-error-msg">{allError}</div>}
        {allResult && renderParsedResult(allResult)}
      </div>

      {/* Verify Specific Invariant */}
      <div className="audit-panel">
        <h3 className="audit-panel-title">Verify Specific Invariant</h3>
        <div className="audit-form-row">
          <input
            className="audit-search"
            value={invariantName}
            onChange={(e) => setInvariantName(e.target.value)}
            placeholder="Invariant name (e.g., capability_checks, fuel_budget, audit_integrity, pii_redaction, hitl_approval, no_unsafe_code)"
          />
          <button
            type="button"
            className="audit-verify-btn"
            disabled={specificLoading || !invariantName.trim()}
            onClick={() => void handleVerifySpecific()}
          >
            {specificLoading ? "Verifying..." : "VERIFY"}
          </button>
        </div>
        {specificError && <div className="audit-error-msg">{specificError}</div>}
        {specificResult && renderParsedResult(specificResult)}
      </div>

      {/* Export Compliance Report */}
      <div className="audit-panel">
        <div className="audit-panel-header">
          <h3 className="audit-panel-title">Export Compliance Report</h3>
          <button
            type="button"
            className="audit-verify-btn"
            disabled={reportLoading}
            onClick={() => void handleExportReport()}
          >
            {reportLoading ? "Exporting..." : "EXPORT REPORT"}
          </button>
        </div>
        <p className="audit-panel-desc">
          Generates a full compliance report covering all governance invariants, audit chain integrity, and agent permission states.
        </p>
        {reportError && <div className="audit-error-msg">{reportError}</div>}
        {reportResult && (
          <div className="audit-detail-payload">
            <pre className="audit-detail-json">{reportResult}</pre>
          </div>
        )}
      </div>
    </div>
  );
}

// --- Main Audit Component ---

export function Audit({ events, onRefresh }: AuditProps): JSX.Element {
  const [activeTab, setActiveTab] = useState<AuditTab>("log");
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [chainStatus, setChainStatus] = useState<AuditChainStatusRow | null>(null);
  const [verifyState, setVerifyState] = useState<"idle" | "running" | "done">("idle");
  const [sortField, setSortField] = useState<SortField>("index");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [liveEvents, setLiveEvents] = useState<AuditEventRow[]>(events);
  const [refreshing, setRefreshing] = useState(false);
  const [timeRange, setTimeRange] = useState("all");
  const [severityFilter, setSeverityFilter] = useState("all");
  const [stats, setStats] = useState<AuditStatistics | null>(null);
  const [statsLoading, setStatsLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [backendResults, setBackendResults] = useState<AuditEventRow[] | null>(null);
  const [backendSearching, setBackendSearching] = useState(false);

  useEffect(() => {
    setLiveEvents(events);
  }, [events]);

  // Debounced backend search via auditSearch
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const q = query.trim();
    const hasFilters = q.length > 0 || agentFilter !== "all" || severityFilter !== "all" || timeRange !== "all";
    if (!hasFilters) {
      setBackendResults(null);
      return;
    }
    const timer = window.setTimeout(() => {
      const searchQuery: AuditSearchQuery = { limit: 500 };
      if (q.length > 0) searchQuery.text = q;
      if (agentFilter !== "all") searchQuery.agent_id = agentFilter;
      if (severityFilter !== "all") searchQuery.severity = severityFilter;
      if (timeRange !== "all") searchQuery.time_range = timeRange;
      setBackendSearching(true);
      auditSearch(searchQuery)
        .then((result) => {
          setBackendResults(result.entries);
        })
        .catch(() => {
          // Fall back to client-side filtering on error
          setBackendResults(null);
        })
        .finally(() => setBackendSearching(false));
    }, 300);
    return () => window.clearTimeout(timer);
  }, [query, agentFilter, severityFilter, timeRange]);

  async function loadStats(): Promise<void> {
    if (!hasDesktopRuntime()) return;
    setStatsLoading(true);
    try {
      const s = await auditStatistics(timeRange === "all" ? "all" : timeRange);
      setStats(s);
    } catch {
      // ignore
    }
    setStatsLoading(false);
  }

  async function handleExport(format: string): Promise<void> {
    setExporting(true);
    try {
      const data = await auditExportReport(format, timeRange === "all" ? "all" : timeRange);
      const blob = new Blob([data], { type: format === "csv" ? "text/csv" : "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `nexus-audit-${new Date().toISOString().slice(0, 10)}.${format}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      // ignore
    }
    setExporting(false);
  }

  const chronological = useMemo(
    () => [...liveEvents].sort((a, b) => a.timestamp - b.timestamp),
    [liveEvents]
  );

  const filtered = useMemo(() => {
    // Use backend search results when available (desktop runtime + active filters)
    if (backendResults !== null) {
      // Still apply statusFilter client-side since backend may not support it directly
      if (statusFilter === "all") return backendResults;
      return backendResults.filter((event) => eventStatus(event.event_type) === statusFilter);
    }
    // Fallback: client-side filtering
    const q = query.trim().toLowerCase();
    const now = Math.floor(Date.now() / 1000);
    const timeStart = timeRange === "1h" ? now - 3600
      : timeRange === "24h" ? now - 86400
      : timeRange === "7d" ? now - 604800
      : timeRange === "30d" ? now - 2592000
      : 0;
    return chronological.filter((event) => {
      if (timeStart > 0 && event.timestamp < timeStart) return false;
      if (agentFilter !== "all" && event.agent_id !== agentFilter) return false;
      if (statusFilter !== "all" && eventStatus(event.event_type) !== statusFilter) return false;
      if (severityFilter !== "all") {
        const sev = event.event_type.toLowerCase().includes("error") ? "error"
          : (event.payload as Record<string, unknown>)?.denied ? "denied"
          : (event.payload as Record<string, unknown>)?.blocked ? "denied"
          : "info";
        if (sev !== severityFilter) return false;
      }
      if (q.length > 0) {
        const text = `${event.event_id} ${event.event_type} ${event.agent_id} ${JSON.stringify(event.payload)}`.toLowerCase();
        if (!text.includes(q)) return false;
      }
      return true;
    });
  }, [chronological, query, agentFilter, statusFilter, severityFilter, timeRange, backendResults]);

  const sorted = useMemo(() => {
    const arr = [...filtered];
    arr.sort((a, b) => {
      let cmp = 0;
      if (sortField === "timestamp") cmp = a.timestamp - b.timestamp;
      else if (sortField === "agent") cmp = a.agent_id.localeCompare(b.agent_id);
      else if (sortField === "action") cmp = a.event_type.localeCompare(b.event_type);
      else if (sortField === "status") cmp = eventStatus(a.event_type).localeCompare(eventStatus(b.event_type));
      else if (sortField === "fuel") cmp = (fuelCost(a.payload) ?? 0) - (fuelCost(b.payload) ?? 0);
      else cmp = chronological.indexOf(a) - chronological.indexOf(b);
      return sortDir === "desc" ? -cmp : cmp;
    });
    return arr;
  }, [filtered, sortField, sortDir, chronological]);

  const agents = useMemo(
    () => Array.from(new Set(liveEvents.map((e) => e.agent_id))),
    [liveEvents]
  );

  const handleSort = useCallback((field: SortField) => {
    setSortField((prev) => {
      if (prev === field) {
        setSortDir((d) => (d === "asc" ? "desc" : "asc"));
        return prev;
      }
      setSortDir("asc");
      return field;
    });
  }, []);

  async function verifyChain(): Promise<void> {
    if (verifyState === "running") return;
    setVerifyState("running");
    setChainStatus(null);
    try {
      if (hasDesktopRuntime()) {
        const status = await getAuditChainStatus();
        setChainStatus(status);
      } else {
        // Client-side fallback for mock mode
        let valid = true;
        for (let i = 1; i < chronological.length; i++) {
          if (chronological[i].previous_hash !== chronological[i - 1].hash) {
            valid = false;
            break;
          }
        }
        setChainStatus({
          total_events: chronological.length,
          chain_valid: valid,
          first_hash: chronological[0]?.hash ?? "",
          last_hash: chronological[chronological.length - 1]?.hash ?? "",
        });
      }
    } catch {
      setChainStatus({ total_events: chronological.length, chain_valid: false, first_hash: "", last_hash: "" });
    }
    setVerifyState("done");
  }

  async function handleRefresh(): Promise<void> {
    setRefreshing(true);
    try {
      if (hasDesktopRuntime()) {
        const fresh = await getAuditLog(undefined, 500);
        setLiveEvents(fresh);
      }
      onRefresh?.();
    } catch {
      // ignore
    }
    setRefreshing(false);
  }

  useEffect(() => {
    if (verifyState === "done") {
      const timer = window.setTimeout(() => setVerifyState("idle"), 8000);
      return () => window.clearTimeout(timer);
    }
  }, [verifyState]);

  // Auto-refresh every 10 seconds when in desktop mode
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const timer = window.setInterval(() => {
      getAuditLog(undefined, 500).then(setLiveEvents).catch((e) => { if (import.meta.env.DEV) console.warn("[Audit]", e); });
    }, 10_000);
    return () => window.clearInterval(timer);
  }, []);

  function sortArrow(field: SortField): string {
    if (sortField !== field) return "";
    return sortDir === "asc" ? " \u25B2" : " \u25BC";
  }

  return (
    <section className="audit-forensic">
      <header className="audit-header">
        <div className="audit-header-left">
          <span className="audit-shield">&#x1F6E1;</span>
          <div>
            <h2 className="audit-title">AUDIT CHAIN // {chainStatus?.chain_valid === false ? "INTEGRITY BROKEN" : "INTEGRITY VERIFIED"}</h2>
            <p className="audit-subtitle">{chronological.length} events in chain</p>
          </div>
        </div>
        <div className="audit-header-right">
          {verifyState === "done" && chainStatus && (
            <span className={`audit-verify-result ${chainStatus.chain_valid ? "valid" : "invalid"}`}>
              {chainStatus.chain_valid
                ? `CHAIN VALID (${chainStatus.total_events} events)`
                : "CHAIN BROKEN"}
            </span>
          )}
          <button type="button" className="audit-verify-btn" onClick={() => void verifyChain()} disabled={verifyState === "running"}>
            {verifyState === "running" ? "Verifying..." : "VERIFY CHAIN"}
          </button>
          <button type="button" className="audit-verify-btn" onClick={() => void handleRefresh()} disabled={refreshing}>
            {refreshing ? "Refreshing..." : "REFRESH"}
          </button>
        </div>
      </header>

      {/* Tab Navigation */}
      <div className="audit-tabs">
        <button
          type="button"
          className={`audit-tab ${activeTab === "log" ? "active" : ""}`}
          onClick={() => setActiveTab("log")}
        >
          Audit Log
        </button>
        <button
          type="button"
          className={`audit-tab ${activeTab === "statistics" ? "active" : ""}`}
          onClick={() => { setActiveTab("statistics"); void loadStats(); }}
        >
          Statistics
        </button>
        <button
          type="button"
          className={`audit-tab ${activeTab === "tracing" ? "active" : ""}`}
          onClick={() => setActiveTab("tracing")}
        >
          Distributed Tracing
        </button>
        <button
          type="button"
          className={`audit-tab ${activeTab === "governance" ? "active" : ""}`}
          onClick={() => setActiveTab("governance")}
        >
          Governance Verification
        </button>
      </div>

      {/* Audit Log Tab */}
      {activeTab === "log" && (
        <>
          {chronological.length === 0 && (
            <div style={{ textAlign: "center", padding: "3rem 1rem", opacity: 0.6 }}>
              <p style={{ fontSize: "1.1rem" }}>No audit events yet. Start an agent to generate events.</p>
            </div>
          )}

          <div className="audit-filters">
            <input
              className="audit-search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search events, payloads, hashes..."
            />
            <select className="audit-select" value={agentFilter} onChange={(e) => setAgentFilter(e.target.value)}>
              <option value="all">All Agents</option>
              {agents.map((id) => (
                <option key={id} value={id}>{shortAgent(id)}</option>
              ))}
            </select>
            <select className="audit-select" value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)}>
              <option value="all">All Status</option>
              <option value="Success">Success</option>
              <option value="Failed">Failed</option>
              <option value="Pending">Pending</option>
            </select>
            <select className="audit-select" value={severityFilter} onChange={(e) => setSeverityFilter(e.target.value)}>
              <option value="all">All Severity</option>
              <option value="info">Info</option>
              <option value="warning">Warning</option>
              <option value="denied">Denied</option>
              <option value="error">Error</option>
            </select>
            <select className="audit-select" value={timeRange} onChange={(e) => setTimeRange(e.target.value)}>
              <option value="all">All Time</option>
              <option value="1h">Last 1h</option>
              <option value="24h">Last 24h</option>
              <option value="7d">Last 7d</option>
              <option value="30d">Last 30d</option>
            </select>
            <span className="audit-count">{backendSearching ? "Searching..." : `${sorted.length} / ${chronological.length} events`}</span>
            {hasDesktopRuntime() && (
              <div className="audit-action-btns">
                <button type="button" className="audit-verify-btn" style={{ fontSize: "0.66rem", padding: "0.35rem 0.6rem" }} disabled={exporting} onClick={() => void handleExport("json")}>
                  {exporting ? "..." : "JSON"}
                </button>
                <button type="button" className="audit-verify-btn" style={{ fontSize: "0.66rem", padding: "0.35rem 0.6rem" }} disabled={exporting} onClick={() => void handleExport("csv")}>
                  {exporting ? "..." : "CSV"}
                </button>
              </div>
            )}
          </div>

          <div className="audit-table-wrap">
            <table className="audit-table">
              <thead>
                <tr>
                  <th className="audit-th" onClick={() => handleSort("index")}>#{ sortArrow("index")}</th>
                  <th className="audit-th" onClick={() => handleSort("timestamp")}>Timestamp{sortArrow("timestamp")}</th>
                  <th className="audit-th" onClick={() => handleSort("agent")}>Agent{sortArrow("agent")}</th>
                  <th className="audit-th" onClick={() => handleSort("action")}>Action{sortArrow("action")}</th>
                  <th className="audit-th" onClick={() => handleSort("status")}>Status{sortArrow("status")}</th>
                  <th className="audit-th" onClick={() => handleSort("fuel")}>Fuel Cost{sortArrow("fuel")}</th>
                  <th className="audit-th">Hash</th>
                </tr>
              </thead>
              <tbody>
                {sorted.map((event, idx) => {
                  const globalIdx = chronological.indexOf(event) + 1;
                  const status = eventStatus(event.event_type);
                  const color = EVENT_TYPE_COLORS[event.event_type] ?? agentColor(event.agent_id);
                  const expanded = expandedId === event.event_id;
                  const fuel = fuelCost(event.payload);
                  return (
                    <tr
                      key={event.event_id}
                      className={`audit-row ${expanded ? "expanded" : ""} ${idx % 2 === 0 ? "even" : "odd"}`}
                      onClick={() => setExpandedId(expanded ? null : event.event_id)}
                    >
                      <td className="audit-td audit-td-index">{globalIdx}</td>
                      <td className="audit-td audit-td-time">{formatDateTime(event.timestamp)}</td>
                      <td className="audit-td">
                        <span className="audit-agent-dot" style={{ background: color }} />
                        {shortAgent(event.agent_id)}
                      </td>
                      <td className="audit-td audit-td-mono">{event.event_type}</td>
                      <td className="audit-td">
                        <span className={`audit-status-badge ${status.toLowerCase()}`}>{status}</span>
                      </td>
                      <td className="audit-td audit-td-mono">{fuel !== null ? fuel : "-"}</td>
                      <td className="audit-td audit-td-hash">
                        <span className="audit-hash-text">{event.hash.slice(0, 8)}...</span>
                        <button
                          type="button"
                          className="audit-copy-btn"
                          onClick={(e) => {
                            e.stopPropagation();
                            void navigator.clipboard.writeText(event.hash);
                          }}
                          title="Copy hash"
                        >
                          &#x2398;
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {expandedId && (() => {
            const event = chronological.find((e) => e.event_id === expandedId);
            if (!event) return null;
            return (
              <div className="audit-detail-panel">
                <div className="audit-detail-header">
                  <h3>Event Detail: {event.event_id}</h3>
                  <button type="button" className="audit-detail-close" onClick={() => setExpandedId(null)}>&#x2715;</button>
                </div>
                <div className="audit-detail-grid">
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Full Hash</span>
                    <span className="audit-detail-value mono">{event.hash}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Previous Hash</span>
                    <span className="audit-detail-value mono">{event.previous_hash}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Event Type</span>
                    <span className="audit-detail-value">{event.event_type}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Agent ID</span>
                    <span className="audit-detail-value mono">{event.agent_id}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Timestamp</span>
                    <span className="audit-detail-value">{formatDateTime(event.timestamp)}</span>
                  </div>
                </div>
                <div className="audit-detail-payload">
                  <span className="audit-detail-label">Payload JSON</span>
                  <pre className="audit-detail-json">{JSON.stringify(event.payload, null, 2)}</pre>
                </div>
              </div>
            );
          })()}
        </>
      )}

      {/* Statistics Tab */}
      {activeTab === "statistics" && (
        <div className="audit-governance-tab">
          <div className="audit-panel">
            <div className="audit-panel-header">
              <h3 className="audit-panel-title">Audit Statistics</h3>
              <div className="audit-action-btns">
                <select className="audit-select" value={timeRange} onChange={(e) => { setTimeRange(e.target.value); }}>
                  <option value="all">All Time</option>
                  <option value="1h">Last 1h</option>
                  <option value="24h">Last 24h</option>
                  <option value="7d">Last 7d</option>
                  <option value="30d">Last 30d</option>
                </select>
                <button type="button" className="audit-verify-btn" disabled={statsLoading} onClick={() => void loadStats()}>
                  {statsLoading ? "Loading..." : "REFRESH"}
                </button>
              </div>
            </div>
            {stats ? (
              <>
                <div className="audit-detail-grid" style={{ marginBottom: "1rem" }}>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Total Events</span>
                    <span className="audit-detail-value" style={{ fontSize: "1.4rem", color: "var(--text-accent)" }}>{stats.total_entries}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">HITL Approvals</span>
                    <span className="audit-detail-value" style={{ fontSize: "1.4rem", color: "#6ee7b7" }}>{stats.hitl_approvals}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">HITL Denials</span>
                    <span className="audit-detail-value" style={{ fontSize: "1.4rem", color: "#fca5a5" }}>{stats.hitl_denials}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Capability Denials</span>
                    <span className="audit-detail-value" style={{ fontSize: "1.4rem", color: "#fbbf24" }}>{stats.capability_denials}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">PII Redactions</span>
                    <span className="audit-detail-value" style={{ fontSize: "1.4rem", color: "#93c5fd" }}>{stats.pii_redactions}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Firewall Blocks</span>
                    <span className="audit-detail-value" style={{ fontSize: "1.4rem", color: "#f87171" }}>{stats.firewall_blocks}</span>
                  </div>
                  <div className="audit-detail-field">
                    <span className="audit-detail-label">Total Fuel</span>
                    <span className="audit-detail-value" style={{ fontSize: "1.4rem", color: "#a78bfa" }}>{stats.total_fuel_consumed}</span>
                  </div>
                </div>

                {/* Actions by type */}
                <h3 className="audit-panel-title" style={{ marginTop: "0.5rem" }}>Actions by Type</h3>
                <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "1rem" }}>
                  {Object.entries(stats.entries_by_action).sort(([,a],[,b]) => b - a).map(([action, count]) => (
                    <div key={action} style={{
                      padding: "0.4rem 0.7rem",
                      border: "1px solid rgba(0,255,157,0.2)",
                      background: "rgba(0,255,157,0.05)",
                      fontFamily: "var(--font-mono)",
                      fontSize: "0.72rem",
                    }}>
                      <span style={{ color: EVENT_TYPE_COLORS[action] ?? "var(--text-accent)" }}>{action}</span>
                      <span style={{ color: "var(--text-secondary)", marginLeft: "0.5rem" }}>{count}</span>
                    </div>
                  ))}
                </div>

                {/* Top agents */}
                <h3 className="audit-panel-title">Top Agents</h3>
                <div style={{ display: "flex", flexDirection: "column", gap: "0.35rem", marginBottom: "1rem" }}>
                  {Object.entries(stats.entries_by_agent).sort(([,a],[,b]) => b - a).slice(0, 10).map(([agent, count]) => {
                    const maxCount = Math.max(...Object.values(stats.entries_by_agent));
                    const pct = maxCount > 0 ? (count / maxCount) * 100 : 0;
                    return (
                      <div key={agent} style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        <span style={{ fontFamily: "var(--font-mono)", fontSize: "0.7rem", color: "var(--text-secondary)", width: "80px" }}>{agent}</span>
                        <div style={{ flex: 1, height: "16px", background: "rgba(0,255,157,0.06)", border: "1px solid rgba(0,255,157,0.1)", position: "relative" }}>
                          <div style={{ width: `${pct}%`, height: "100%", background: "linear-gradient(90deg, rgba(0,255,157,0.3), rgba(0,255,157,0.1))" }} />
                        </div>
                        <span style={{ fontFamily: "var(--font-mono)", fontSize: "0.7rem", color: "var(--text-accent)", width: "40px", textAlign: "right" }}>{count}</span>
                      </div>
                    );
                  })}
                </div>

                {/* Severity breakdown */}
                <h3 className="audit-panel-title">Severity Distribution</h3>
                <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
                  {Object.entries(stats.severity_counts).map(([sev, count]) => {
                    const sevColors: Record<string, string> = { info: "#6ee7b7", warning: "#fbbf24", denied: "#f87171", error: "#ef4444" };
                    return (
                      <div key={sev} style={{ textAlign: "center" }}>
                        <div style={{ fontSize: "1.3rem", fontFamily: "var(--font-mono)", color: sevColors[sev] ?? "var(--text-secondary)" }}>{count}</div>
                        <div style={{ fontSize: "0.66rem", fontFamily: "var(--font-display)", letterSpacing: "0.08em", color: "var(--text-secondary)", textTransform: "uppercase" }}>{sev}</div>
                      </div>
                    );
                  })}
                </div>
              </>
            ) : (
              <p className="audit-empty-msg">{statsLoading ? "Loading statistics..." : "Click REFRESH to load statistics."}</p>
            )}
          </div>
        </div>
      )}

      {/* Distributed Tracing Tab */}
      {activeTab === "tracing" && <DistributedTracingTab />}

      {/* Governance Verification Tab */}
      {activeTab === "governance" && <GovernanceVerificationTab />}
    </section>
  );
}
