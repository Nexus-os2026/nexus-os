import { useState, useCallback, useMemo } from "react";
import { BarChart, Bar, PieChart, Pie, Cell, LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import "./database-manager.css";

/* ─── types ─── */
type DbEngine = "sqlite" | "postgresql" | "mysql";
type TabId = "query" | "builder" | "schema" | "visualize" | "history";

interface DbConnection {
  id: string;
  name: string;
  engine: DbEngine;
  host: string;
  port: number;
  database: string;
  user: string;
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
  foreignKey?: { table: string; column: string };
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
  engine: DbEngine;
  timestamp: number;
  duration: number;
  rowCount: number;
  agent?: string;
  status: "success" | "error";
  error?: string;
}

interface BuilderFilter {
  column: string;
  operator: string;
  value: string;
}

/* ─── mock data ─── */
const MOCK_TABLES: DbTable[] = [
  {
    name: "agents", rowCount: 9,
    columns: [
      { name: "id", type: "UUID", nullable: false, primaryKey: true },
      { name: "name", type: "VARCHAR(255)", nullable: false, primaryKey: false },
      { name: "agent_type", type: "VARCHAR(100)", nullable: false, primaryKey: false },
      { name: "autonomy_level", type: "INT", nullable: false, primaryKey: false },
      { name: "fuel_budget", type: "DECIMAL(10,2)", nullable: false, primaryKey: false },
      { name: "fuel_used", type: "DECIMAL(10,2)", nullable: false, primaryKey: false },
      { name: "status", type: "VARCHAR(50)", nullable: false, primaryKey: false },
      { name: "created_at", type: "TIMESTAMP", nullable: false, primaryKey: false },
      { name: "manifest_id", type: "UUID", nullable: true, primaryKey: false, foreignKey: { table: "manifests", column: "id" } },
    ],
  },
  {
    name: "audit_events", rowCount: 12847,
    columns: [
      { name: "id", type: "BIGSERIAL", nullable: false, primaryKey: true },
      { name: "event_type", type: "VARCHAR(100)", nullable: false, primaryKey: false },
      { name: "agent_id", type: "UUID", nullable: true, primaryKey: false, foreignKey: { table: "agents", column: "id" } },
      { name: "action", type: "TEXT", nullable: false, primaryKey: false },
      { name: "fuel_cost", type: "DECIMAL(10,2)", nullable: true, primaryKey: false },
      { name: "risk_level", type: "VARCHAR(20)", nullable: false, primaryKey: false },
      { name: "hash", type: "VARCHAR(64)", nullable: false, primaryKey: false },
      { name: "prev_hash", type: "VARCHAR(64)", nullable: true, primaryKey: false },
      { name: "created_at", type: "TIMESTAMP", nullable: false, primaryKey: false },
    ],
  },
  {
    name: "fuel_ledger", rowCount: 3421,
    columns: [
      { name: "id", type: "BIGSERIAL", nullable: false, primaryKey: true },
      { name: "agent_id", type: "UUID", nullable: false, primaryKey: false, foreignKey: { table: "agents", column: "id" } },
      { name: "operation", type: "VARCHAR(50)", nullable: false, primaryKey: false },
      { name: "amount", type: "DECIMAL(10,2)", nullable: false, primaryKey: false },
      { name: "balance_after", type: "DECIMAL(10,2)", nullable: false, primaryKey: false },
      { name: "created_at", type: "TIMESTAMP", nullable: false, primaryKey: false },
    ],
  },
  {
    name: "manifests", rowCount: 9,
    columns: [
      { name: "id", type: "UUID", nullable: false, primaryKey: true },
      { name: "agent_id", type: "UUID", nullable: false, primaryKey: false, foreignKey: { table: "agents", column: "id" } },
      { name: "capabilities", type: "JSONB", nullable: false, primaryKey: false },
      { name: "hitl_tier", type: "INT", nullable: false, primaryKey: false },
      { name: "version", type: "VARCHAR(20)", nullable: false, primaryKey: false },
      { name: "updated_at", type: "TIMESTAMP", nullable: false, primaryKey: false },
    ],
  },
  {
    name: "workflows", rowCount: 24,
    columns: [
      { name: "id", type: "UUID", nullable: false, primaryKey: true },
      { name: "name", type: "VARCHAR(255)", nullable: false, primaryKey: false },
      { name: "status", type: "VARCHAR(50)", nullable: false, primaryKey: false },
      { name: "steps", type: "JSONB", nullable: false, primaryKey: false },
      { name: "created_by", type: "UUID", nullable: true, primaryKey: false, foreignKey: { table: "agents", column: "id" } },
      { name: "created_at", type: "TIMESTAMP", nullable: false, primaryKey: false },
    ],
  },
  {
    name: "permissions", rowCount: 47,
    columns: [
      { name: "id", type: "BIGSERIAL", nullable: false, primaryKey: true },
      { name: "agent_id", type: "UUID", nullable: false, primaryKey: false, foreignKey: { table: "agents", column: "id" } },
      { name: "resource", type: "VARCHAR(255)", nullable: false, primaryKey: false },
      { name: "action", type: "VARCHAR(50)", nullable: false, primaryKey: false },
      { name: "granted", type: "BOOLEAN", nullable: false, primaryKey: false },
      { name: "granted_by", type: "VARCHAR(100)", nullable: false, primaryKey: false },
      { name: "created_at", type: "TIMESTAMP", nullable: false, primaryKey: false },
    ],
  },
];

const MOCK_CONNECTIONS: DbConnection[] = [
  { id: "db-1", name: "nexus_primary", engine: "postgresql", host: "localhost", port: 5432, database: "nexus_os", user: "nexus_admin", status: "connected", tables: MOCK_TABLES },
  { id: "db-2", name: "nexus_audit", engine: "sqlite", host: "local", port: 0, database: "audit.db", user: "—", status: "connected", tables: MOCK_TABLES.filter(t => t.name === "audit_events" || t.name === "fuel_ledger") },
  { id: "db-3", name: "marketplace_db", engine: "mysql", host: "db.nexus.io", port: 3306, database: "marketplace", user: "mkt_reader", status: "disconnected", tables: [] },
];

const MOCK_AGENTS_DATA: Record<string, string | number | boolean | null>[] = [
  { id: "a1b2c3", name: "Coder Agent", agent_type: "coder", autonomy_level: 3, fuel_budget: 5000, fuel_used: 2340, status: "active", created_at: "2026-01-15" },
  { id: "d4e5f6", name: "Research Agent", agent_type: "researcher", autonomy_level: 2, fuel_budget: 3000, fuel_used: 1820, status: "active", created_at: "2026-01-15" },
  { id: "g7h8i9", name: "Planner Agent", agent_type: "planner", autonomy_level: 2, fuel_budget: 2000, fuel_used: 640, status: "active", created_at: "2026-01-15" },
  { id: "j1k2l3", name: "Self-Improve Agent", agent_type: "optimizer", autonomy_level: 4, fuel_budget: 4000, fuel_used: 2890, status: "active", created_at: "2026-01-20" },
  { id: "m4n5o6", name: "Content Agent", agent_type: "content", autonomy_level: 1, fuel_budget: 1500, fuel_used: 420, status: "idle", created_at: "2026-02-01" },
];

const MOCK_AUDIT_DATA: Record<string, string | number | boolean | null>[] = [
  { id: 12847, event_type: "code_gen", agent_id: "a1b2c3", action: "Generated React component", fuel_cost: 45, risk_level: "low", created_at: "2026-03-10 14:23" },
  { id: 12846, event_type: "web_search", agent_id: "d4e5f6", action: "Searched WASM benchmarks", fuel_cost: 12, risk_level: "low", created_at: "2026-03-10 14:20" },
  { id: 12845, event_type: "file_write", agent_id: "a1b2c3", action: "Wrote ProjectManager.tsx", fuel_cost: 8, risk_level: "medium", created_at: "2026-03-10 14:15" },
  { id: 12844, event_type: "approval", agent_id: "j1k2l3", action: "Requested deploy approval", fuel_cost: 2, risk_level: "high", created_at: "2026-03-10 13:50" },
  { id: 12843, event_type: "code_review", agent_id: "j1k2l3", action: "Reviewed fuel ledger fix", fuel_cost: 30, risk_level: "low", created_at: "2026-03-10 13:40" },
];

const INITIAL_HISTORY: QueryHistoryEntry[] = [
  { id: "qh-1", query: "SELECT * FROM agents WHERE status = 'active'", database: "nexus_primary", engine: "postgresql", timestamp: Date.now() - 300000, duration: 12, rowCount: 4, status: "success" },
  { id: "qh-2", query: "SELECT event_type, COUNT(*) as cnt FROM audit_events GROUP BY event_type ORDER BY cnt DESC", database: "nexus_primary", engine: "postgresql", timestamp: Date.now() - 600000, duration: 45, rowCount: 8, status: "success" },
  { id: "qh-3", query: "SELECT agent_id, SUM(amount) as total_fuel FROM fuel_ledger GROUP BY agent_id", database: "nexus_audit", engine: "sqlite", timestamp: Date.now() - 1200000, duration: 28, rowCount: 5, agent: "Research Agent", status: "success" },
  { id: "qh-4", query: "DROP TABLE agents", database: "nexus_primary", engine: "postgresql", timestamp: Date.now() - 1800000, duration: 0, rowCount: 0, agent: "Coder Agent", status: "error", error: "BLOCKED: Destructive query requires Tier2+ HITL approval" },
  { id: "qh-5", query: "INSERT INTO agents (name, agent_type) VALUES ('Test Agent', 'test')", database: "nexus_primary", engine: "postgresql", timestamp: Date.now() - 3600000, duration: 5, rowCount: 1, agent: "Coder Agent", status: "success" },
];

const ENGINE_ICONS: Record<DbEngine, string> = { sqlite: "◆", postgresql: "◈", mysql: "◇" };
const ENGINE_COLORS: Record<DbEngine, string> = { sqlite: "#38bdf8", postgresql: "#a78bfa", mysql: "#fbbf24" };
const PIE_COLORS = ["#22d3ee", "#a78bfa", "#34d399", "#fbbf24", "#f472b6", "#fb923c", "#818cf8", "#f87171"];
const BLOCKED_PATTERNS = ["DROP", "TRUNCATE", "DELETE FROM", "ALTER TABLE", "GRANT", "REVOKE"];

/* ─── component ─── */
export default function DatabaseManager() {
  const [connections, setConnections] = useState<DbConnection[]>(MOCK_CONNECTIONS);
  const [selectedConnId, setSelectedConnId] = useState("db-1");
  const [selectedTable, setSelectedTable] = useState<string | null>("agents");
  const [activeTab, setActiveTab] = useState<TabId>("query");
  const [sqlQuery, setSqlQuery] = useState("SELECT * FROM agents WHERE status = 'active';");
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [history, setHistory] = useState<QueryHistoryEntry[]>(INITIAL_HISTORY);
  const [fuelUsed, setFuelUsed] = useState(84);

  // builder state
  const [builderTable, setBuilderTable] = useState("agents");
  const [builderColumns, setBuilderColumns] = useState<string[]>(["*"]);
  const [builderFilters, setBuilderFilters] = useState<BuilderFilter[]>([]);
  const [builderOrderBy, setBuilderOrderBy] = useState("");
  const [builderLimit, setBuilderLimit] = useState("100");

  // visualize
  const [vizType, setVizType] = useState<"bar" | "pie" | "line">("bar");
  const [vizXCol, setVizXCol] = useState("");
  const [vizYCol, setVizYCol] = useState("");

  const selectedConn = useMemo(() => connections.find(c => c.id === selectedConnId) ?? connections[0], [connections, selectedConnId]);
  const tableObj = useMemo(() => selectedConn.tables.find(t => t.name === selectedTable), [selectedConn, selectedTable]);

  /* ─── execute query ─── */
  const executeQuery = useCallback((sql: string) => {
    const trimmed = sql.trim().replace(/;$/, "").toUpperCase();
    // governance: block destructive queries
    const blocked = BLOCKED_PATTERNS.find(p => trimmed.startsWith(p));
    if (blocked) {
      const err = `BLOCKED: "${blocked}" queries require Tier2+ HITL approval. Agent write access is governed.`;
      setQueryError(err);
      setQueryResult(null);
      setHistory(prev => [{ id: `qh-${Date.now()}`, query: sql, database: selectedConn.database, engine: selectedConn.engine, timestamp: Date.now(), duration: 0, rowCount: 0, status: "error", error: err }, ...prev]);
      return;
    }

    // mock query execution
    const start = performance.now();
    let result: QueryResult;

    if (trimmed.includes("FROM AGENTS") || trimmed.includes("FROM AGENTS")) {
      result = {
        columns: ["id", "name", "agent_type", "autonomy_level", "fuel_budget", "fuel_used", "status", "created_at"],
        rows: MOCK_AGENTS_DATA,
        rowCount: MOCK_AGENTS_DATA.length,
        duration: Math.round(Math.random() * 20 + 5),
        query: sql,
      };
    } else if (trimmed.includes("FROM AUDIT_EVENTS") || trimmed.includes("AUDIT")) {
      result = {
        columns: ["id", "event_type", "agent_id", "action", "fuel_cost", "risk_level", "created_at"],
        rows: MOCK_AUDIT_DATA,
        rowCount: MOCK_AUDIT_DATA.length,
        duration: Math.round(Math.random() * 50 + 10),
        query: sql,
      };
    } else if (trimmed.includes("FROM FUEL_LEDGER") || trimmed.includes("FUEL")) {
      result = {
        columns: ["agent_name", "total_fuel", "operations"],
        rows: [
          { agent_name: "Coder Agent", total_fuel: 2340, operations: 847 },
          { agent_name: "Research Agent", total_fuel: 1820, operations: 523 },
          { agent_name: "Self-Improve", total_fuel: 2890, operations: 412 },
          { agent_name: "Planner Agent", total_fuel: 640, operations: 189 },
          { agent_name: "Content Agent", total_fuel: 420, operations: 96 },
        ],
        rowCount: 5,
        duration: Math.round(Math.random() * 30 + 8),
        query: sql,
      };
    } else if (trimmed.includes("GROUP BY") || trimmed.includes("COUNT")) {
      result = {
        columns: ["category", "count", "avg_fuel"],
        rows: [
          { category: "code_gen", count: 4521, avg_fuel: 42 },
          { category: "web_search", count: 2847, avg_fuel: 12 },
          { category: "file_write", count: 1923, avg_fuel: 8 },
          { category: "code_review", count: 1456, avg_fuel: 28 },
          { category: "approval", count: 892, avg_fuel: 3 },
          { category: "deployment", count: 341, avg_fuel: 65 },
          { category: "test_run", count: 867, avg_fuel: 18 },
        ],
        rowCount: 7,
        duration: Math.round(Math.random() * 60 + 15),
        query: sql,
      };
    } else {
      result = {
        columns: ["id", "name", "status", "created_at"],
        rows: [
          { id: 1, name: "Sample Row 1", status: "active", created_at: "2026-03-10" },
          { id: 2, name: "Sample Row 2", status: "idle", created_at: "2026-03-09" },
          { id: 3, name: "Sample Row 3", status: "active", created_at: "2026-03-08" },
        ],
        rowCount: 3,
        duration: Math.round(performance.now() - start + Math.random() * 10),
        query: sql,
      };
    }

    setQueryResult(result);
    setQueryError(null);
    setFuelUsed(f => f + Math.floor(Math.random() * 5 + 3));
    setHistory(prev => [{ id: `qh-${Date.now()}`, query: sql, database: selectedConn.database, engine: selectedConn.engine, timestamp: Date.now(), duration: result.duration, rowCount: result.rowCount, status: "success" }, ...prev]);
  }, [selectedConn]);

  /* ─── builder → SQL ─── */
  const builderSql = useMemo(() => {
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
    const firstCol = selectedConn.tables.find(t => t.name === builderTable)?.columns[0]?.name ?? "id";
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

  /* ─── export ─── */
  const handleExport = (format: "csv" | "json") => {
    if (!queryResult) return;
    setFuelUsed(f => f + 2);
    setHistory(prev => [{ id: `qh-${Date.now()}`, query: `EXPORT ${format.toUpperCase()}`, database: selectedConn.database, engine: selectedConn.engine, timestamp: Date.now(), duration: 0, rowCount: queryResult.rowCount, status: "success" }, ...prev]);
  };

  const toggleConnection = (id: string) => {
    setConnections(prev => prev.map(c => c.id === id ? { ...c, status: c.status === "connected" ? "disconnected" : "connected", tables: c.status === "connected" ? [] : (c.id === "db-3" ? MOCK_TABLES.slice(0, 3) : c.tables.length ? c.tables : MOCK_TABLES) } : c));
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

        {/* connections */}
        <div className="db-connections">
          {connections.map(conn => (
            <div key={conn.id} className={`db-conn ${selectedConnId === conn.id ? "active" : ""}`}>
              <div className="db-conn-header" onClick={() => { setSelectedConnId(conn.id); if (conn.tables.length > 0) setSelectedTable(conn.tables[0].name); }}>
                <span className="db-conn-engine" style={{ color: ENGINE_COLORS[conn.engine] }}>{ENGINE_ICONS[conn.engine]}</span>
                <span className="db-conn-name">{conn.name}</span>
                <span className={`db-conn-status db-status-${conn.status}`}>●</span>
                <button className="db-conn-toggle" onClick={e => { e.stopPropagation(); toggleConnection(conn.id); }}>{conn.status === "connected" ? "⏏" : "▶"}</button>
              </div>
              {conn.status === "connected" && selectedConnId === conn.id && (
                <div className="db-tables-list">
                  {conn.tables.map(table => (
                    <div key={table.name} className={`db-table-item ${selectedTable === table.name ? "active" : ""}`} onClick={() => setSelectedTable(table.name)}>
                      <span className="db-table-icon">▤</span>
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
              <span>▤ {tableObj.name}</span>
              <span className="db-schema-rows">{tableObj.rowCount} rows</span>
            </div>
            <div className="db-schema-cols">
              {tableObj.columns.map(col => (
                <div key={col.name} className="db-schema-col">
                  <span className={`db-col-icon ${col.primaryKey ? "pk" : col.foreignKey ? "fk" : ""}`}>
                    {col.primaryKey ? "🔑" : col.foreignKey ? "🔗" : "·"}
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
              <button key={tab} className={`db-tab ${activeTab === tab ? "active" : ""}`} onClick={() => setActiveTab(tab)}>
                {tab === "query" ? "⌨" : tab === "builder" ? "⊞" : tab === "schema" ? "⬡" : tab === "visualize" ? "◔" : "⏱"} {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>
          <div className="db-tabs-right">
            <span className="db-conn-info">{ENGINE_ICONS[selectedConn.engine]} {selectedConn.name} ({selectedConn.engine})</span>
            <span className="db-fuel">⚡ {fuelUsed} fuel</span>
          </div>
        </div>

        {/* ─── Query Tab ─── */}
        {activeTab === "query" && (
          <div className="db-query-tab">
            <div className="db-query-editor">
              <div className="db-query-toolbar">
                <button className="db-btn-run" onClick={() => executeQuery(sqlQuery)}>▶ Run Query</button>
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
                  <span className="db-error-icon">⛔</span>
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
                  <div className="db-no-results-icon">⌨</div>
                  <div>Write a query and press Ctrl+Enter or click Run</div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ─── Builder Tab ─── */}
        {activeTab === "builder" && (
          <div className="db-builder-tab">
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
                      {col.primaryKey && "🔑 "}{col.name}
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
                      <option value=">=">≥</option>
                      <option value="<=">≤</option>
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
                <button className="db-btn-run" onClick={() => { setSqlQuery(builderSql); executeQuery(builderSql); setActiveTab("query"); }}>▶ Run</button>
              </div>
              <pre className="db-builder-sql">{builderSql}</pre>
            </div>
          </div>
        )}

        {/* ─── Schema Tab ─── */}
        {activeTab === "schema" && (
          <div className="db-schema-tab">
            <div className="db-schema-header">
              <h3>Schema — {selectedConn.name} ({selectedConn.engine})</h3>
              <span className="db-schema-count">{selectedConn.tables.length} tables</span>
            </div>
            <div className="db-schema-grid">
              {selectedConn.tables.map(table => (
                <div key={table.name} className={`db-schema-card ${selectedTable === table.name ? "selected" : ""}`} onClick={() => setSelectedTable(table.name)}>
                  <div className="db-schema-card-header">
                    <span className="db-schema-card-icon">▤</span>
                    <span className="db-schema-card-name">{table.name}</span>
                    <span className="db-schema-card-rows">{table.rowCount} rows</span>
                  </div>
                  <div className="db-schema-card-cols">
                    {table.columns.map(col => (
                      <div key={col.name} className="db-schema-card-col">
                        <span className={`db-schema-key ${col.primaryKey ? "pk" : col.foreignKey ? "fk" : ""}`}>
                          {col.primaryKey ? "PK" : col.foreignKey ? "FK" : "  "}
                        </span>
                        <span className="db-schema-col-name">{col.name}</span>
                        <span className="db-schema-col-type">{col.type}</span>
                        {col.nullable && <span className="db-schema-nullable">?</span>}
                      </div>
                    ))}
                  </div>
                  {/* relationships */}
                  {table.columns.filter(c => c.foreignKey).length > 0 && (
                    <div className="db-schema-rels">
                      {table.columns.filter(c => c.foreignKey).map(col => (
                        <div key={col.name} className="db-schema-rel">
                          <span className="db-schema-rel-arrow">→</span>
                          <span>{col.name} → {col.foreignKey!.table}.{col.foreignKey!.column}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>

            {/* ERD relationships summary */}
            <div className="db-erd">
              <h4 className="db-erd-title">Relationships (ERD)</h4>
              <div className="db-erd-diagram">
                {selectedConn.tables.flatMap(table =>
                  table.columns.filter(c => c.foreignKey).map(col => (
                    <div key={`${table.name}-${col.name}`} className="db-erd-line">
                      <span className="db-erd-from">{table.name}.{col.name}</span>
                      <span className="db-erd-arrow">──FK──▶</span>
                      <span className="db-erd-to">{col.foreignKey!.table}.{col.foreignKey!.column}</span>
                    </div>
                  ))
                )}
              </div>
            </div>
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
                        <button key={t} className={`db-viz-type-btn ${vizType === t ? "active" : ""}`} onClick={() => setVizType(t)}>
                          {t === "bar" ? "▥" : t === "pie" ? "◔" : "◠"} {t.charAt(0).toUpperCase() + t.slice(1)}
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
                        <Bar dataKey={vizYCol || queryResult.columns.find(c => typeof queryResult.rows[0]?.[c] === "number") || queryResult.columns[1] || ""} fill="#22d3ee" radius={[4, 4, 0, 0]} />
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
                        <Line type="monotone" dataKey={vizYCol || queryResult.columns.find(c => typeof queryResult.rows[0]?.[c] === "number") || queryResult.columns[1] || ""} stroke="#22d3ee" strokeWidth={2} dot={{ r: 4, fill: "#22d3ee" }} />
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
                <div className="db-no-results-icon">◔</div>
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
              {history.map(entry => (
                <div key={entry.id} className={`db-history-item ${entry.status}`}>
                  <div className="db-history-item-header">
                    <span className={`db-history-status ${entry.status}`}>{entry.status === "success" ? "✓" : "✗"}</span>
                    <span className="db-history-time">{formatTimestamp(entry.timestamp)}</span>
                    <span className="db-history-db" style={{ color: ENGINE_COLORS[entry.engine] }}>{ENGINE_ICONS[entry.engine]} {entry.database}</span>
                    {entry.agent && <span className="db-history-agent">⬢ {entry.agent}</span>}
                    <span className="db-history-meta">{entry.rowCount} rows · {entry.duration}ms</span>
                    <button className="db-history-rerun" onClick={() => { setSqlQuery(entry.query); setActiveTab("query"); }} title="Load query">↻</button>
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
        <span className="db-status-item">{ENGINE_ICONS[selectedConn.engine]} {selectedConn.name}</span>
        <span className="db-status-item">{selectedConn.status === "connected" ? "Connected" : "Disconnected"}</span>
        <span className="db-status-item">{selectedConn.tables.length} tables</span>
        {queryResult && <span className="db-status-item">{queryResult.rowCount} rows · {queryResult.duration}ms</span>}
        <span className="db-status-item db-status-right">⚡ {fuelUsed} fuel</span>
        <span className="db-status-item">{history.length} queries logged</span>
        <span className="db-status-item">Ctrl+Enter run</span>
      </div>
    </div>
  );
}
