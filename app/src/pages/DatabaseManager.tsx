import { useState, useCallback, useMemo } from "react";
import { BarChart, Bar, PieChart, Pie, Cell, LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import {
  Play, Keyboard, LayoutGrid, Hexagon, PieChart as PieChartIcon,
  Clock, Table2, Diamond, Power, Check, X, RotateCcw, Zap,
  OctagonAlert, BarChart3, TrendingUp, Circle,
} from "lucide-react";
import { dbConnect, dbListTables, dbExecuteQuery, dbExportTable, dbDisconnect } from "../api/backend";
import "./database-manager.css";

/* ─── types ─── */
type TabId = "query" | "builder" | "schema" | "visualize" | "history";

interface DbConnection {
  id: string;
  name: string;
  path: string;
  status: "connected" | "disconnected" | "error";
  tables: DbTable[];
}

interface DbTable {
  name: string;
  rowCount: number;
  columns: DbColumn[];
}

interface DbColumn {
  name: string;
  type: string;
  nullable: boolean;
  primaryKey: boolean;
}

interface QueryResult {
  columns: string[];
  rows: Record<string, string | number | boolean | null>[];
  rowCount: number;
  duration: number;
  query: string;
}

interface QueryHistoryEntry {
  id: string;
  query: string;
  database: string;
  timestamp: number;
  duration: number;
  rowCount: number;
  status: "success" | "error";
  error?: string;
}

interface BuilderFilter {
  column: string;
  operator: string;
  value: string;
}

/* ─── constants ─── */
const PIE_COLORS = ["var(--nexus-accent)", "#a78bfa", "#34d399", "#fbbf24", "#f472b6", "#fb923c", "#818cf8", "#f87171"];
const BLOCKED_PATTERNS = ["DROP", "TRUNCATE", "DELETE", "ALTER", "GRANT", "REVOKE"];

/* ─── component ─── */
export default function DatabaseManager() {
  const [connections, setConnections] = useState<DbConnection[]>([]);
  const [selectedConnIdx, setSelectedConnIdx] = useState(0);
  const [selectedTable, setSelectedTable] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("query");
  const [sqlQuery, setSqlQuery] = useState("SELECT * FROM sqlite_master WHERE type='table';");
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [history, setHistory] = useState<QueryHistoryEntry[]>([]);
  const [fuelUsed, setFuelUsed] = useState(0);
  const [connectInput, setConnectInput] = useState("");
  const [connectName, setConnectName] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [executing, setExecuting] = useState(false);

  // builder state
  const [builderTable, setBuilderTable] = useState("");
  const [builderColumns, setBuilderColumns] = useState<string[]>(["*"]);
  const [builderFilters, setBuilderFilters] = useState<BuilderFilter[]>([]);
  const [builderOrderBy, setBuilderOrderBy] = useState("");
  const [builderLimit, setBuilderLimit] = useState("100");

  // visualize
  const [vizType, setVizType] = useState<"bar" | "pie" | "line">("bar");
  const [vizXCol, setVizXCol] = useState("");
  const [vizYCol, setVizYCol] = useState("");

  const selectedConn = connections[selectedConnIdx] ?? null;
  const tableObj = useMemo(() => selectedConn?.tables.find(t => t.name === selectedTable), [selectedConn, selectedTable]);

  /* ─── connect to database ─── */
  const connectToDb = useCallback(async () => {
    if (!connectInput.trim()) return;
    setConnecting(true);
    try {
      const raw: string = await dbConnect(connectInput.trim());
      const result = JSON.parse(raw);

      // Now get detailed table info
      const tablesRaw: string = await dbListTables(connectInput.trim());
      const tables: DbTable[] = JSON.parse(tablesRaw);

      const newConn: DbConnection = {
        id: result.conn_id,
        name: connectName.trim() || connectInput.trim().split("/").pop() || "database",
        path: connectInput.trim(),
        status: "connected",
        tables,
      };

      setConnections(prev => [...prev, newConn]);
      setSelectedConnIdx(connections.length);
      if (tables.length > 0) {
        setSelectedTable(tables[0].name);
        setBuilderTable(tables[0].name);
      }
      setConnectInput("");
      setConnectName("");
      setFuelUsed(f => f + 3);
    } catch (err) {
      setQueryError(String(err));
    } finally {
      setConnecting(false);
    }
  }, [connectInput, connectName, connections.length]);

  /* ─── execute query ─── */
  const executeQuery = useCallback(async (sql: string) => {
    if (!selectedConn) {
      setQueryError("No database connected. Connect to a database first.");
      return;
    }
    const trimmed = sql.trim().replace(/;$/, "").toUpperCase();
    // Client-side governance check
    const blocked = BLOCKED_PATTERNS.find(p => trimmed.split(/\s+/).includes(p));
    if (blocked) {
      const err = `BLOCKED: "${blocked}" queries require Tier2+ HITL approval. Agent write access is governed.`;
      setQueryError(err);
      setQueryResult(null);
      setHistory(prev => [{ id: `qh-${Date.now()}`, query: sql, database: selectedConn.name, timestamp: Date.now(), duration: 0, rowCount: 0, status: "error", error: err }, ...prev]);
      return;
    }

    setExecuting(true);
    setQueryError(null);
    try {
      const raw: string = await dbExecuteQuery(selectedConn.path, sql);
      const data = JSON.parse(raw);

      // Convert array-of-arrays rows to array-of-objects
      const columns: string[] = data.columns;
      const rows: Record<string, string | number | boolean | null>[] = (data.rows as (string | number | null)[][]).map(
        (row: (string | number | null)[]) => {
          const obj: Record<string, string | number | boolean | null> = {};
          columns.forEach((col, i) => { obj[col] = row[i] ?? null; });
          return obj;
        }
      );

      const result: QueryResult = {
        columns,
        rows,
        rowCount: data.row_count,
        duration: data.duration_ms,
        query: sql,
      };

      setQueryResult(result);
      const fuelCost = data.fuel_cost ?? (() => {
        const upper = sql.trim().toUpperCase();
        if (upper.startsWith("SELECT")) return 2;
        if (upper.startsWith("INSERT") || upper.startsWith("UPDATE") || upper.startsWith("DELETE")) return 5;
        return 8;
      })();
      setFuelUsed(f => f + fuelCost);
      setHistory(prev => [{ id: `qh-${Date.now()}`, query: sql, database: selectedConn.name, timestamp: Date.now(), duration: result.duration, rowCount: result.rowCount, status: "success" }, ...prev]);

      // Refresh tables after write operations
      const upper = sql.trim().toUpperCase();
      if (upper.startsWith("CREATE") || upper.startsWith("INSERT") || upper.startsWith("UPDATE")) {
        const tablesRaw: string = await dbListTables(selectedConn.path);
        const tables: DbTable[] = JSON.parse(tablesRaw);
        setConnections(prev => prev.map((c, i) => i === selectedConnIdx ? { ...c, tables } : c));
      }
    } catch (err) {
      const errStr = String(err);
      setQueryError(errStr);
      setQueryResult(null);
      setHistory(prev => [{ id: `qh-${Date.now()}`, query: sql, database: selectedConn.name, timestamp: Date.now(), duration: 0, rowCount: 0, status: "error", error: errStr }, ...prev]);
    } finally {
      setExecuting(false);
    }
  }, [selectedConn, selectedConnIdx]);

  /* ─── builder → SQL ─── */
  const builderSql = useMemo(() => {
    if (!builderTable) return "";
    const cols = builderColumns.length === 0 || builderColumns.includes("*") ? "*" : builderColumns.join(", ");
    let sql = `SELECT ${cols} FROM ${builderTable}`;
    if (builderFilters.length > 0) {
      const conditions = builderFilters.map(f => `${f.column} ${f.operator} '${f.value}'`).join(" AND ");
      sql += ` WHERE ${conditions}`;
    }
    if (builderOrderBy) sql += ` ORDER BY ${builderOrderBy}`;
    if (builderLimit) sql += ` LIMIT ${builderLimit}`;
    return sql + ";";
  }, [builderTable, builderColumns, builderFilters, builderOrderBy, builderLimit]);

  const addBuilderFilter = () => {
    const firstCol = selectedConn?.tables.find(t => t.name === builderTable)?.columns[0]?.name ?? "id";
    setBuilderFilters(prev => [...prev, { column: firstCol, operator: "=", value: "" }]);
  };

  const updateBuilderFilter = (idx: number, updates: Partial<BuilderFilter>) => {
    setBuilderFilters(prev => prev.map((f, i) => i === idx ? { ...f, ...updates } : f));
  };

  const removeBuilderFilter = (idx: number) => {
    setBuilderFilters(prev => prev.filter((_, i) => i !== idx));
  };

  const toggleBuilderColumn = (col: string) => {
    setBuilderColumns(prev => {
      if (prev.includes("*")) return [col];
      if (prev.includes(col)) {
        const next = prev.filter(c => c !== col);
        return next.length === 0 ? ["*"] : next;
      }
      return [...prev, col];
    });
  };

  const handleExport = async (format: "csv" | "json") => {
    if (!queryResult || !selectedConn) return;
    setFuelUsed(f => f + 2);
    try {
      // If we have a table name from the last query, export it via backend
      const tableName = sqlQuery.trim().match(/^SELECT\s.*\sFROM\s+["`]?(\w+)["`]?/i)?.[1];
      let exportData: string;
      if (tableName && selectedConn.path) {
        exportData = await dbExportTable(selectedConn.path, tableName, format);
      } else {
        // Fallback: format current query results client-side
        if (format === "csv") {
          const header = queryResult.columns.join(",");
          const rows = queryResult.rows.map((r) =>
            queryResult.columns.map((c) => String(r[c] ?? "")).join(","),
          ).join("\n");
          exportData = header + "\n" + rows;
        } else {
          exportData = JSON.stringify(queryResult.rows, null, 2);
        }
      }
      // Trigger download via blob
      const blob = new Blob([exportData], { type: format === "csv" ? "text/csv" : "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `export_${Date.now()}.${format}`;
      a.click();
      URL.revokeObjectURL(url);
      setHistory(prev => [{ id: `qh-${Date.now()}`, query: `EXPORT ${format.toUpperCase()}`, database: selectedConn?.name ?? "", timestamp: Date.now(), duration: 0, rowCount: queryResult.rowCount, status: "success" }, ...prev]);
    } catch {
      setHistory(prev => [{ id: `qh-${Date.now()}`, query: `EXPORT ${format.toUpperCase()}`, database: selectedConn?.name ?? "", timestamp: Date.now(), duration: 0, rowCount: 0, status: "error" }, ...prev]);
    }
  };

  const disconnectDb = async (idx: number) => {
    const conn = connections[idx];
    if (conn) {
      try {
        await dbDisconnect(conn.path);
      } catch (e) {
        if (import.meta.env.DEV) console.error("Disconnect error:", e);
      }
    }
    setConnections(prev => prev.filter((_, i) => i !== idx));
    if (selectedConnIdx >= connections.length - 1) {
      setSelectedConnIdx(Math.max(0, connections.length - 2));
    }
  };

  const formatTimestamp = (ts: number) => {
    const d = new Date(ts);
    const diff = Date.now() - ts;
    if (diff < 60000) return "just now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    return d.toLocaleTimeString();
  };

  const TOOLTIP_STYLE = { background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 };

  /* ─── render ─── */
  return (
    <div className="db-container">
      {/* ─── Sidebar ─── */}
      <aside className="db-sidebar">
        <div className="db-sidebar-header">
          <h2 className="db-sidebar-title">Databases</h2>
        </div>

        {/* connect form */}
        <div className="db-connect-form" style={{ padding: "8px 12px", borderBottom: "1px solid rgba(56,189,248,0.08)" }}>
          <input
            className="db-sql-editor"
            style={{ height: 28, fontSize: 11, marginBottom: 4, resize: "none" }}
            value={connectName}
            onChange={e => setConnectName(e.target.value)}
            placeholder="Connection name..."
          />
          <input
            className="db-sql-editor"
            style={{ height: 28, fontSize: 11, marginBottom: 4, resize: "none" }}
            value={connectInput}
            onChange={e => setConnectInput(e.target.value)}
            placeholder="SQLite path (e.g. ~/.nexus/data.db)"
            onKeyDown={e => e.key === "Enter" && connectToDb()}
          />
          <button className="db-btn-run" style={{ width: "100%", fontSize: 11 }} onClick={connectToDb} disabled={connecting}>
            {connecting ? "Connecting..." : "Connect"}
          </button>
        </div>

        {/* connections */}
        <div className="db-connections">
          {connections.length === 0 && (
            <div style={{ padding: "16px 12px", color: "#64748b", fontSize: 12, textAlign: "center" }}>
              No connections. Enter a SQLite path above.
            </div>
          )}
          {connections.map((conn, idx) => (
            <div key={conn.id} className={`db-conn ${selectedConnIdx === idx ? "active" : ""}`}>
              <div className="db-conn-header" onClick={() => { setSelectedConnIdx(idx); if (conn.tables.length > 0) setSelectedTable(conn.tables[0].name); }}>
                <span className="db-conn-engine" style={{ color: "#38bdf8" }}><Diamond size={12} /></span>
                <span className="db-conn-name">{conn.name}</span>
                <span className={`db-conn-status db-status-${conn.status}`}><Circle size={8} fill="currentColor" /></span>
                <button className="db-conn-toggle cursor-pointer" onClick={e => { e.stopPropagation(); disconnectDb(idx); }}><Power size={12} /></button>
              </div>
              {selectedConnIdx === idx && (
                <div className="db-tables-list">
                  {conn.tables.map(table => (
                    <div key={table.name} className={`db-table-item ${selectedTable === table.name ? "active" : ""}`} onClick={() => { setSelectedTable(table.name); setBuilderTable(table.name); }}>
                      <span className="db-table-icon"><Table2 size={12} /></span>
                      <span className="db-table-name">{table.name}</span>
                      <span className="db-table-count">{table.rowCount}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>

        {/* schema quick view */}
        {tableObj && (
          <div className="db-schema-quick">
            <div className="db-schema-quick-header">
              <span><Table2 size={12} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} />{tableObj.name}</span>
              <span className="db-schema-rows">{tableObj.rowCount} rows</span>
            </div>
            <div className="db-schema-cols">
              {tableObj.columns.map(col => (
                <div key={col.name} className="db-schema-col">
                  <span className={`db-col-icon ${col.primaryKey ? "pk" : ""}`}>
                    {col.primaryKey ? "PK" : "-"}
                  </span>
                  <span className="db-col-name">{col.name}</span>
                  <span className="db-col-type">{col.type}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </aside>

      {/* ─── Main Area ─── */}
      <div className="db-main">
        {/* tabs */}
        <div className="db-tabs">
          <div className="db-tabs-left">
            {(["query", "builder", "schema", "visualize", "history"] as TabId[]).map(tab => (
              <button key={tab} className={`db-tab cursor-pointer ${activeTab === tab ? "active" : ""}`} onClick={() => setActiveTab(tab)}>
                {tab === "query" ? <Keyboard size={14} /> : tab === "builder" ? <LayoutGrid size={14} /> : tab === "schema" ? <Hexagon size={14} /> : tab === "visualize" ? <PieChartIcon size={14} /> : <Clock size={14} />} {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>
          <div className="db-tabs-right">
            {selectedConn && <span className="db-conn-info"><Diamond size={10} style={{ display: "inline", verticalAlign: "middle", marginRight: 2 }} /> {selectedConn.name} (SQLite)</span>}
            <span className="db-fuel"><Zap size={10} style={{ display: "inline", verticalAlign: "middle" }} /> {fuelUsed} fuel</span>
          </div>
        </div>

        {/* ─── Query Tab ─── */}
        {activeTab === "query" && (
          <div className="db-query-tab">
            <div className="db-query-editor">
              <div className="db-query-toolbar">
                <button className="db-btn-run cursor-pointer" onClick={() => executeQuery(sqlQuery)} disabled={executing}>
                  {executing ? "Running..." : <><Play size={12} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} />Run Query</>}
                </button>
                <button className="db-btn-secondary" onClick={() => handleExport("csv")}>CSV</button>
                <button className="db-btn-secondary" onClick={() => handleExport("json")}>JSON</button>
                <button className="db-btn-secondary" onClick={() => setSqlQuery("")}>Clear</button>
              </div>
              <textarea
                className="db-sql-editor"
                value={sqlQuery}
                onChange={e => setSqlQuery(e.target.value)}
                onKeyDown={e => { if (e.ctrlKey && e.key === "Enter") { e.preventDefault(); executeQuery(sqlQuery); } }}
                placeholder="Enter SQL query... (Ctrl+Enter to run)"
                spellCheck={false}
              />
            </div>
            <div className="db-query-results">
              {queryError && (
                <div className="db-query-error">
                  <span className="db-error-icon"><OctagonAlert size={14} /></span>
                  <span>{queryError}</span>
                </div>
              )}
              {queryResult && (
                <>
                  <div className="db-result-header">
                    <span>{queryResult.rowCount} rows returned in {queryResult.duration}ms</span>
                    <span className="db-result-query">{queryResult.query.slice(0, 80)}{queryResult.query.length > 80 ? "..." : ""}</span>
                  </div>
                  <div className="db-result-table-wrap">
                    <table className="db-result-table">
                      <thead>
                        <tr>{queryResult.columns.map(col => <th key={col}>{col}</th>)}</tr>
                      </thead>
                      <tbody>
                        {queryResult.rows.map((row, i) => (
                          <tr key={i}>{queryResult.columns.map(col => <td key={col}>{row[col] === null ? <span className="db-null">NULL</span> : String(row[col])}</td>)}</tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </>
              )}
              {!queryResult && !queryError && (
                <div className="db-no-results">
                  <div className="db-no-results-icon"><Keyboard size={32} /></div>
                  <div>{selectedConn ? "Write a query and press Ctrl+Enter or click Run" : "Connect to a database first, then run queries"}</div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ─── Builder Tab ─── */}
        {activeTab === "builder" && (
          <div className="db-builder-tab">
            {!selectedConn ? (
              <div className="db-no-results"><div className="db-no-results-icon"><LayoutGrid size={32} /></div><div>Connect to a database first</div></div>
            ) : (
              <>
                <div className="db-builder-controls">
                  <div className="db-builder-section">
                    <label className="db-builder-label">Table</label>
                    <select className="db-builder-select" value={builderTable} onChange={e => { setBuilderTable(e.target.value); setBuilderColumns(["*"]); setBuilderFilters([]); }}>
                      {selectedConn.tables.map(t => <option key={t.name} value={t.name}>{t.name} ({t.rowCount})</option>)}
                    </select>
                  </div>

                  <div className="db-builder-section">
                    <label className="db-builder-label">Columns</label>
                    <div className="db-builder-chips">
                      <button className={`db-builder-chip ${builderColumns.includes("*") ? "active" : ""}`} onClick={() => setBuilderColumns(["*"])}>All (*)</button>
                      {selectedConn.tables.find(t => t.name === builderTable)?.columns.map(col => (
                        <button key={col.name} className={`db-builder-chip ${builderColumns.includes(col.name) ? "active" : ""}`} onClick={() => toggleBuilderColumn(col.name)}>
                          {col.primaryKey && "PK "}{col.name}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="db-builder-section">
                    <div className="db-builder-label-row">
                      <label className="db-builder-label">Filters</label>
                      <button className="db-btn-add-filter" onClick={addBuilderFilter}>+ Add</button>
                    </div>
                    {builderFilters.map((filter, idx) => (
                      <div key={idx} className="db-builder-filter-row">
                        <select className="db-builder-filter-select" value={filter.column} onChange={e => updateBuilderFilter(idx, { column: e.target.value })}>
                          {selectedConn.tables.find(t => t.name === builderTable)?.columns.map(col => <option key={col.name} value={col.name}>{col.name}</option>)}
                        </select>
                        <select className="db-builder-filter-op" value={filter.operator} onChange={e => updateBuilderFilter(idx, { operator: e.target.value })}>
                          <option value="=">=</option>
                          <option value="!=">!=</option>
                          <option value=">">&gt;</option>
                          <option value="<">&lt;</option>
                          <option value=">=">&gt;=</option>
                          <option value="<=">&lt;=</option>
                          <option value="LIKE">LIKE</option>
                          <option value="IN">IN</option>
                        </select>
                        <input className="db-builder-filter-val" value={filter.value} onChange={e => updateBuilderFilter(idx, { value: e.target.value })} placeholder="value" />
                        <button className="db-btn-remove" onClick={() => removeBuilderFilter(idx)}>×</button>
                      </div>
                    ))}
                  </div>

                  <div className="db-builder-row">
                    <div className="db-builder-section db-builder-half">
                      <label className="db-builder-label">Order By</label>
                      <select className="db-builder-select" value={builderOrderBy} onChange={e => setBuilderOrderBy(e.target.value)}>
                        <option value="">None</option>
                        {selectedConn.tables.find(t => t.name === builderTable)?.columns.map(col => (
                          <option key={col.name} value={col.name}>{col.name}</option>
                        ))}
                      </select>
                    </div>
                    <div className="db-builder-section db-builder-half">
                      <label className="db-builder-label">Limit</label>
                      <input className="db-builder-input" value={builderLimit} onChange={e => setBuilderLimit(e.target.value)} type="number" />
                    </div>
                  </div>
                </div>

                <div className="db-builder-preview">
                  <div className="db-builder-preview-header">
                    <span>Generated SQL</span>
                    <button className="db-btn-run cursor-pointer" onClick={() => { setSqlQuery(builderSql); executeQuery(builderSql); setActiveTab("query"); }}><Play size={12} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} />Run</button>
                  </div>
                  <pre className="db-builder-sql">{builderSql}</pre>
                </div>
              </>
            )}
          </div>
        )}

        {/* ─── Schema Tab ─── */}
        {activeTab === "schema" && (
          <div className="db-schema-tab">
            {!selectedConn ? (
              <div className="db-no-results"><div className="db-no-results-icon"><Hexagon size={32} /></div><div>Connect to a database first</div></div>
            ) : (
              <>
                <div className="db-schema-header">
                  <h3>Schema — {selectedConn.name} (SQLite)</h3>
                  <span className="db-schema-count">{selectedConn.tables.length} tables</span>
                </div>
                <div className="db-schema-grid">
                  {selectedConn.tables.map(table => (
                    <div key={table.name} className={`db-schema-card ${selectedTable === table.name ? "selected" : ""}`} onClick={() => setSelectedTable(table.name)}>
                      <div className="db-schema-card-header">
                        <span className="db-schema-card-icon"><Table2 size={12} /></span>
                        <span className="db-schema-card-name">{table.name}</span>
                        <span className="db-schema-card-rows">{table.rowCount} rows</span>
                      </div>
                      <div className="db-schema-card-cols">
                        {table.columns.map(col => (
                          <div key={col.name} className="db-schema-card-col">
                            <span className={`db-schema-key ${col.primaryKey ? "pk" : ""}`}>
                              {col.primaryKey ? "PK" : "  "}
                            </span>
                            <span className="db-schema-col-name">{col.name}</span>
                            <span className="db-schema-col-type">{col.type}</span>
                            {col.nullable && <span className="db-schema-nullable">?</span>}
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </>
            )}
          </div>
        )}

        {/* ─── Visualize Tab ─── */}
        {activeTab === "visualize" && (
          <div className="db-viz-tab">
            {queryResult ? (
              <>
                <div className="db-viz-controls">
                  <div className="db-viz-group">
                    <label>Chart Type</label>
                    <div className="db-viz-type-btns">
                      {(["bar", "pie", "line"] as const).map(t => (
                        <button key={t} className={`db-viz-type-btn cursor-pointer ${vizType === t ? "active" : ""}`} onClick={() => setVizType(t)}>
                          {t === "bar" ? <BarChart3 size={14} /> : t === "pie" ? <PieChartIcon size={14} /> : <TrendingUp size={14} />} {t.charAt(0).toUpperCase() + t.slice(1)}
                        </button>
                      ))}
                    </div>
                  </div>
                  <div className="db-viz-group">
                    <label>X Axis / Label</label>
                    <select className="db-viz-select" value={vizXCol || queryResult.columns[0]} onChange={e => setVizXCol(e.target.value)}>
                      {queryResult.columns.map(c => <option key={c} value={c}>{c}</option>)}
                    </select>
                  </div>
                  <div className="db-viz-group">
                    <label>Y Axis / Value</label>
                    <select className="db-viz-select" value={vizYCol || queryResult.columns.find(c => typeof queryResult.rows[0]?.[c] === "number") || queryResult.columns[1] || ""} onChange={e => setVizYCol(e.target.value)}>
                      {queryResult.columns.map(c => <option key={c} value={c}>{c}</option>)}
                    </select>
                  </div>
                </div>

                <div className="db-viz-chart">
                  {vizType === "bar" && (
                    <ResponsiveContainer width="100%" height={350}>
                      <BarChart data={queryResult.rows as Record<string, unknown>[]} >
                        <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                        <XAxis dataKey={vizXCol || queryResult.columns[0]} tick={{ fill: "#64748b", fontSize: 10 }} angle={-30} textAnchor="end" height={60} />
                        <YAxis tick={{ fill: "#64748b", fontSize: 10 }} />
                        <Tooltip contentStyle={TOOLTIP_STYLE} />
                        <Bar dataKey={vizYCol || queryResult.columns.find(c => typeof queryResult.rows[0]?.[c] === "number") || queryResult.columns[1] || ""} fill="var(--nexus-accent)" radius={[4, 4, 0, 0]} />
                      </BarChart>
                    </ResponsiveContainer>
                  )}
                  {vizType === "line" && (
                    <ResponsiveContainer width="100%" height={350}>
                      <LineChart data={queryResult.rows as Record<string, unknown>[]}>
                        <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                        <XAxis dataKey={vizXCol || queryResult.columns[0]} tick={{ fill: "#64748b", fontSize: 10 }} />
                        <YAxis tick={{ fill: "#64748b", fontSize: 10 }} />
                        <Tooltip contentStyle={TOOLTIP_STYLE} />
                        <Line type="monotone" dataKey={vizYCol || queryResult.columns.find(c => typeof queryResult.rows[0]?.[c] === "number") || queryResult.columns[1] || ""} stroke="var(--nexus-accent)" strokeWidth={2} dot={{ r: 4, fill: "var(--nexus-accent)" }} />
                      </LineChart>
                    </ResponsiveContainer>
                  )}
                  {vizType === "pie" && (
                    <ResponsiveContainer width="100%" height={350}>
                      <PieChart>
                        <Pie
                          data={queryResult.rows.map(r => ({ name: String(r[vizXCol || queryResult.columns[0]]), value: Number(r[vizYCol || queryResult.columns.find(c => typeof queryResult.rows[0]?.[c] === "number") || queryResult.columns[1] || ""] || 0) }))}
                          dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={120}
                          label={({ name, percent }: { name?: string; percent?: number }) => `${name ?? ""} ${((percent ?? 0) * 100).toFixed(0)}%`}
                          labelLine={false} fontSize={10}
                        >
                          {queryResult.rows.map((_, i) => <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />)}
                        </Pie>
                        <Tooltip contentStyle={TOOLTIP_STYLE} />
                      </PieChart>
                    </ResponsiveContainer>
                  )}
                </div>
              </>
            ) : (
              <div className="db-no-results">
                <div className="db-no-results-icon"><PieChartIcon size={32} /></div>
                <div>Run a query first, then visualize the results</div>
              </div>
            )}
          </div>
        )}

        {/* ─── History Tab ─── */}
        {activeTab === "history" && (
          <div className="db-history-tab">
            <div className="db-history-header">
              <h3>Query History</h3>
              <span className="db-history-count">{history.length} queries</span>
            </div>
            <div className="db-history-list">
              {history.length === 0 && (
                <div className="db-no-results">
                  <div className="db-no-results-icon"><Clock size={32} /></div>
                  <div>No queries executed yet</div>
                </div>
              )}
              {history.map(entry => (
                <div key={entry.id} className={`db-history-item ${entry.status}`}>
                  <div className="db-history-item-header">
                    <span className={`db-history-status ${entry.status}`}>{entry.status === "success" ? <Check size={12} /> : <X size={12} />}</span>
                    <span className="db-history-time">{formatTimestamp(entry.timestamp)}</span>
                    <span className="db-history-db" style={{ color: "#38bdf8" }}><Diamond size={10} style={{ display: "inline", verticalAlign: "middle", marginRight: 2 }} /> {entry.database}</span>
                    <span className="db-history-meta">{entry.rowCount} rows · {entry.duration}ms</span>
                    <button className="db-history-rerun cursor-pointer" onClick={() => { setSqlQuery(entry.query); setActiveTab("query"); }} title="Load query"><RotateCcw size={12} /></button>
                  </div>
                  <pre className="db-history-query">{entry.query}</pre>
                  {entry.error && <div className="db-history-error">{entry.error}</div>}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* ─── Status Bar ─── */}
      <div className="db-status-bar">
        {selectedConn ? (
          <>
            <span className="db-status-item"><Diamond size={10} style={{ display: "inline", verticalAlign: "middle", marginRight: 2 }} /> {selectedConn.name}</span>
            <span className="db-status-item">Connected</span>
            <span className="db-status-item">{selectedConn.tables.length} tables</span>
          </>
        ) : (
          <span className="db-status-item">No connection</span>
        )}
        {queryResult && <span className="db-status-item">{queryResult.rowCount} rows · {queryResult.duration}ms</span>}
        <span className="db-status-item db-status-right"><Zap size={10} style={{ display: "inline", verticalAlign: "middle" }} /> {fuelUsed} fuel</span>
        <span className="db-status-item">{history.length} queries logged</span>
        <span className="db-status-item">Ctrl+Enter run</span>
      </div>
    </div>
  );
}
