import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  cogfsGetContext,
  cogfsGetEntities as cogfsGetEntitiesApi,
  cogfsGetGraph as cogfsGetGraphApi,
  cogfsIndexFile,
  cogfsQuery,
  cogfsSearch,
  cogfsWatchDirectory,
  neuralBridgeIngest,
  neuralBridgeSearch,
  neuralBridgeStatus,
  neuralBridgeToggle,
  neuralBridgeDelete,
  neuralBridgeClearOld,
} from "../api/backend";
import {
  ActionButton,
  EmptyState,
  EntityGroup,
  MetricBar,
  Panel,
  alpha,
  commandHeaderMetaStyle,
  commandInsetStyle,
  commandLabelStyle,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
  formatRelative,
  inputStyle,
  normalizeArray,
  toTitleCase,
} from "./commandCenterUi";

interface QueryResult {
  path: string;
  relevance_score: number;
  snippet: string;
  matched_entities: string[];
}

interface GraphLink {
  id?: string;
  source: string;
  target: string;
  link_type: string;
  strength: number;
}

interface TrackedPath {
  path: string;
  kind: "file" | "directory";
}

const STORAGE_KEY = "nexus-knowledge-tracked-paths";

function loadTrackedPaths(): TrackedPath[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    return normalizeArray<TrackedPath>(JSON.parse(raw));
  } catch {
    return [];
  }
}

function saveTrackedPaths(entries: TrackedPath[]): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
}

function classifyEntities(entities: string[], queryResult?: QueryResult | null): {
  people: string[];
  dates: string[];
  topics: string[];
  obligations: string[];
} {
  const people = new Set<string>();
  const dates = new Set<string>();
  const obligations = new Set<string>();
  const topics = new Set<string>();

  for (const entity of entities) {
    if (/^\d{4}-\d{2}-\d{2}$/.test(entity)) {
      dates.add(entity);
      continue;
    }
    if (/(must|shall|required|deadline|due)/i.test(entity)) {
      obligations.add(entity);
      continue;
    }
    if (/^[A-Z][a-z]+(?:\s+[A-Z][a-z]+)+$/.test(entity)) {
      people.add(entity);
      continue;
    }
    topics.add(entity);
  }

  for (const entity of queryResult?.matched_entities ?? []) {
    if (!people.has(entity) && !dates.has(entity) && !obligations.has(entity)) {
      topics.add(entity);
    }
  }

  return {
    people: Array.from(people),
    dates: Array.from(dates),
    topics: Array.from(topics),
    obligations: Array.from(obligations),
  };
}

async function openDialogPicker(directory: boolean): Promise<string | null> {
  try {
    const importer = new Function("specifier", "return import(specifier);") as (specifier: string) => Promise<{
      open: (options: { directory?: boolean; multiple?: boolean }) => Promise<string | string[] | null>;
    }>;
    const dialog = await importer("@tauri-apps/plugin-dialog");
    const picked = await dialog.open({ directory, multiple: false });
    if (Array.isArray(picked)) return picked[0] ?? null;
    return picked;
  } catch {
    return null;
  }
}

async function openBrowserPicker(directory: boolean): Promise<string | null> {
  if (typeof document === "undefined") return null;
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    if (directory) {
      (input as HTMLInputElement & { webkitdirectory?: boolean }).webkitdirectory = true;
    }
    input.onchange = () => {
      const files = input.files;
      if (!files || files.length === 0) {
        resolve(null);
        return;
      }

      const first = files[0] as File & { path?: string; webkitRelativePath?: string };
      if (first.path) {
        if (directory && first.webkitRelativePath) {
          const relative = first.webkitRelativePath.replace(/\\/g, "/");
          const raw = first.path.replace(/\\/g, "/");
          const base = raw.endsWith(relative) ? raw.slice(0, raw.length - relative.length) : raw;
          resolve(base.replace(/\/$/, ""));
          return;
        }
        resolve(first.path);
        return;
      }

      resolve(directory ? null : first.name);
    };
    input.click();
  });
}

async function pickPath(directory: boolean): Promise<string | null> {
  const tauriPath = await openDialogPicker(directory);
  if (tauriPath) return tauriPath;
  return openBrowserPicker(directory);
}

export default function KnowledgeGraphPage(): JSX.Element {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<QueryResult[]>([]);
  const [selectedPath, setSelectedPath] = useState<string>("");
  const [selectedResult, setSelectedResult] = useState<QueryResult | null>(null);
  const [entities, setEntities] = useState<string[]>([]);
  const [links, setLinks] = useState<GraphLink[]>([]);
  const [trackedPaths, setTrackedPaths] = useState<TrackedPath[]>(() => loadTrackedPaths());
  const [searchMode, setSearchMode] = useState<"natural" | "keyword" | null>(null);
  const [searching, setSearching] = useState(false);
  const [indexing, setIndexing] = useState(false);
  const [rebuilding, setRebuilding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState("Ready to search your file graph");

  // -- Context lookup --
  const [contextTopic, setContextTopic] = useState("");
  const [contextResult, setContextResult] = useState<string | null>(null);
  const [contextLoading, setContextLoading] = useState(false);

  // -- Entities lookup --
  const [entitiesPath, setEntitiesPath] = useState("");
  const [entitiesResult, setEntitiesResult] = useState<string | null>(null);
  const [entitiesLoading, setEntitiesLoading] = useState(false);

  // -- Graph Links lookup --
  const [graphPath, setGraphPath] = useState("");
  const [graphResult, setGraphResult] = useState<string | null>(null);
  const [graphLoading, setGraphLoading] = useState(false);

  // -- Neural Bridge --
  const [nbStatus, setNbStatus] = useState<string | null>(null);
  const [nbEnabled, setNbEnabled] = useState(false);
  const [nbStatusLoading, setNbStatusLoading] = useState(false);

  const [nbSearchQuery, setNbSearchQuery] = useState("");
  const [nbSearchTimeRange, setNbSearchTimeRange] = useState("");
  const [nbSearchSourceFilter, setNbSearchSourceFilter] = useState("");
  const [nbSearchMaxResults, setNbSearchMaxResults] = useState("10");
  const [nbSearchResult, setNbSearchResult] = useState<string | null>(null);
  const [nbSearchLoading, setNbSearchLoading] = useState(false);

  const [nbIngestSourceType, setNbIngestSourceType] = useState("Screen");
  const [nbIngestContent, setNbIngestContent] = useState("");
  const [nbIngestMetadata, setNbIngestMetadata] = useState("{}");
  const [nbIngestResult, setNbIngestResult] = useState<string | null>(null);
  const [nbIngestLoading, setNbIngestLoading] = useState(false);

  const [nbDeleteId, setNbDeleteId] = useState("");
  const [nbDeleteResult, setNbDeleteResult] = useState<string | null>(null);
  const [nbDeleteLoading, setNbDeleteLoading] = useState(false);

  const [nbClearDays, setNbClearDays] = useState("30");
  const [nbClearResult, setNbClearResult] = useState<string | null>(null);
  const [nbClearLoading, setNbClearLoading] = useState(false);

  // Fetch Neural Bridge status on mount
  useEffect(() => {
    let cancelled = false;
    setNbStatusLoading(true);
    neuralBridgeStatus()
      .then((result) => {
        if (!cancelled) {
          setNbStatus(result);
          try {
            const parsed = JSON.parse(result);
            if (typeof parsed.enabled === "boolean") setNbEnabled(parsed.enabled);
          } catch { /* status is plain text */ }
        }
      })
      .catch(() => { if (!cancelled) setNbStatus("Failed to fetch status"); })
      .finally(() => { if (!cancelled) setNbStatusLoading(false); });
    return () => { cancelled = true; };
  }, []);

  const handleContextLookup = useCallback(async () => {
    if (!contextTopic.trim()) return;
    setContextLoading(true);
    setContextResult(null);
    try {
      const result = await cogfsGetContext(contextTopic.trim());
      setContextResult(result);
    } catch (err) {
      setContextResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setContextLoading(false);
    }
  }, [contextTopic]);

  const handleEntitiesLookup = useCallback(async () => {
    if (!entitiesPath.trim()) return;
    setEntitiesLoading(true);
    setEntitiesResult(null);
    try {
      const result = await cogfsGetEntitiesApi(entitiesPath.trim());
      setEntitiesResult(result);
    } catch (err) {
      setEntitiesResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setEntitiesLoading(false);
    }
  }, [entitiesPath]);

  const handleGraphLookup = useCallback(async () => {
    if (!graphPath.trim()) return;
    setGraphLoading(true);
    setGraphResult(null);
    try {
      const result = await cogfsGetGraphApi(graphPath.trim());
      setGraphResult(result);
    } catch (err) {
      setGraphResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setGraphLoading(false);
    }
  }, [graphPath]);

  const handleNbToggle = useCallback(async () => {
    setNbStatusLoading(true);
    try {
      const next = !nbEnabled;
      const result = await neuralBridgeToggle(next);
      setNbEnabled(next);
      setNbStatus(result);
    } catch (err) {
      setNbStatus(`Toggle error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setNbStatusLoading(false);
    }
  }, [nbEnabled]);

  const handleNbSearch = useCallback(async () => {
    if (!nbSearchQuery.trim()) return;
    setNbSearchLoading(true);
    setNbSearchResult(null);
    try {
      const timeRange: [number, number] | undefined = nbSearchTimeRange.trim()
        ? (nbSearchTimeRange.split(",").map(Number) as [number, number])
        : undefined;
      const sourceFilter = nbSearchSourceFilter.trim()
        ? nbSearchSourceFilter.split(",").map((s) => s.trim())
        : undefined;
      const maxResults = parseInt(nbSearchMaxResults, 10) || undefined;
      const result = await neuralBridgeSearch(nbSearchQuery.trim(), timeRange, sourceFilter, maxResults);
      setNbSearchResult(result);
    } catch (err) {
      setNbSearchResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setNbSearchLoading(false);
    }
  }, [nbSearchQuery, nbSearchTimeRange, nbSearchSourceFilter, nbSearchMaxResults]);

  const handleNbIngest = useCallback(async () => {
    if (!nbIngestContent.trim()) return;
    setNbIngestLoading(true);
    setNbIngestResult(null);
    try {
      let metadata: unknown = {};
      try { metadata = JSON.parse(nbIngestMetadata); } catch { /* keep empty */ }
      const result = await neuralBridgeIngest(nbIngestSourceType, nbIngestContent.trim(), metadata);
      setNbIngestResult(result);
      setNbIngestContent("");
    } catch (err) {
      setNbIngestResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setNbIngestLoading(false);
    }
  }, [nbIngestSourceType, nbIngestContent, nbIngestMetadata]);

  const handleNbDelete = useCallback(async () => {
    if (!nbDeleteId.trim()) return;
    setNbDeleteLoading(true);
    setNbDeleteResult(null);
    try {
      const result = await neuralBridgeDelete(nbDeleteId.trim());
      setNbDeleteResult(result);
      setNbDeleteId("");
    } catch (err) {
      setNbDeleteResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setNbDeleteLoading(false);
    }
  }, [nbDeleteId]);

  const handleNbClear = useCallback(async () => {
    setNbClearLoading(true);
    setNbClearResult(null);
    try {
      const days = parseInt(nbClearDays, 10) || 30;
      const beforeTimestamp = Date.now() - days * 86_400_000;
      const result = await neuralBridgeClearOld(beforeTimestamp);
      setNbClearResult(result);
    } catch (err) {
      setNbClearResult(`Error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setNbClearLoading(false);
    }
  }, [nbClearDays]);

  useEffect(() => {
    saveTrackedPaths(trackedPaths);
  }, [trackedPaths]);

  const loadFileDetails = useCallback(async (path: string, result?: QueryResult | null) => {
    setSelectedPath(path);
    setSelectedResult(result ?? null);
    setError(null);
    try {
      const [entityResult, linkResult] = await Promise.allSettled([
        invoke<string[]>("cogfs_get_entities", { filePath: path }),
        invoke<GraphLink[]>("cogfs_get_graph", { filePath: path }),
      ]);

      if (entityResult.status === "fulfilled") {
        setEntities(normalizeArray<string>(entityResult.value));
      } else {
        setEntities([]);
      }

      if (linkResult.status === "fulfilled") {
        const normalized = normalizeArray<GraphLink>(linkResult.value).sort((a, b) => (b.strength ?? 0) - (a.strength ?? 0));
        setLinks(normalized);
      } else {
        setLinks([]);
      }
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    }
  }, []);

  const handleSearch = useCallback(async () => {
    if (!query.trim()) return;
    setSearching(true);
    setError(null);
    setStatusMessage("Running semantic query...");
    try {
      const naturalResults = normalizeArray<QueryResult>(await cogfsQuery(query.trim()));
      if (naturalResults.length > 0) {
        setResults(naturalResults);
        setSearchMode("natural");
        setStatusMessage(`Semantic query returned ${naturalResults.length} result${naturalResults.length === 1 ? "" : "s"}`);
        return;
      }

      const keywordResults = normalizeArray<QueryResult>(await cogfsSearch(query.trim()));
      setResults(keywordResults);
      setSearchMode("keyword");
      setStatusMessage(keywordResults.length > 0 ? `Keyword fallback returned ${keywordResults.length} result${keywordResults.length === 1 ? "" : "s"}` : "No results found");
    } catch (searchError) {
      setError(searchError instanceof Error ? searchError.message : String(searchError));
      setResults([]);
      setSearchMode(null);
      setStatusMessage("Search failed");
    } finally {
      setSearching(false);
    }
  }, [query]);

  const registerTrackedPath = useCallback((entry: TrackedPath) => {
    setTrackedPaths((current) => {
      const next = current.filter((item) => item.path !== entry.path);
      return [entry, ...next];
    });
  }, []);

  const handleIndexPath = useCallback(async (kind: "file" | "directory") => {
    setIndexing(true);
    setError(null);
    try {
      const path = await pickPath(kind === "directory");
      if (!path) {
        setStatusMessage("Picker closed without a selection");
        return;
      }

      if (!path.includes("/") && !path.includes("\\")) {
        throw new Error("The picker did not return a filesystem path for this environment.");
      }

      if (kind === "directory") {
        await cogfsWatchDirectory(path);
        setStatusMessage(`Watching ${path}`);
      } else {
        await cogfsIndexFile(path);
        setStatusMessage(`Indexed ${path}`);
      }

      registerTrackedPath({ path, kind });
    } catch (indexError) {
      setError(indexError instanceof Error ? indexError.message : String(indexError));
    } finally {
      setIndexing(false);
    }
  }, [registerTrackedPath]);

  const handleRebuild = useCallback(async () => {
    if (trackedPaths.length === 0) {
      setStatusMessage("No tracked files or directories yet");
      return;
    }
    setRebuilding(true);
    setError(null);
    try {
      for (const entry of trackedPaths) {
        if (entry.kind === "directory") {
          await cogfsWatchDirectory(entry.path);
        } else {
          await cogfsIndexFile(entry.path);
        }
      }
      setStatusMessage(`Rebuilt ${trackedPaths.length} tracked item${trackedPaths.length === 1 ? "" : "s"}`);
    } catch (rebuildError) {
      setError(rebuildError instanceof Error ? rebuildError.message : String(rebuildError));
      setStatusMessage("Rebuild failed");
    } finally {
      setRebuilding(false);
    }
  }, [trackedPaths]);

  const entityGroups = useMemo(() => classifyEntities(entities, selectedResult), [entities, selectedResult]);
  const relatedFiles = useMemo(() => {
    return links.map((link) => ({
      path: link.target === selectedPath ? link.source : link.target,
      linkType: toTitleCase(link.link_type),
      strength: link.strength,
    }));
  }, [links, selectedPath]);

  return (
    <div style={commandPageStyle}>
      <div style={{ marginBottom: 20 }}>
        <h1 style={{ margin: 0, fontFamily: "monospace", fontSize: "1.8rem", color: "#00ffcc", letterSpacing: "0.16em", textTransform: "uppercase" }}>
          Knowledge Graph
        </h1>
        <div style={{ ...commandHeaderMetaStyle, marginTop: 10 }}>
          <span>Indexed: {trackedPaths.length} files</span>
          <span>Entities: {entities.length}</span>
          <span>Links: {links.length}</span>
        </div>
      </div>

      <Panel title="Search Nexus Files" accent="#00ffcc" style={{ marginBottom: 18 }}>
        <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 12, alignItems: "center" }}>
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                void handleSearch();
              }
            }}
            placeholder="Ask anything about your files..."
            style={{ ...inputStyle, height: 48, fontSize: "0.92rem" }}
          />
          <ActionButton accent="#00ffcc" disabled={searching} onClick={() => void handleSearch()}>
            {searching ? "Searching..." : "Search"}
          </ActionButton>
        </div>
        <div style={{ ...commandHeaderMetaStyle, marginTop: 12 }}>
          <span>{statusMessage}</span>
          {searchMode ? <span>Mode: {searchMode === "natural" ? "natural query" : "keyword fallback"}</span> : null}
        </div>
      </Panel>

      {error ? <div style={{ marginBottom: 16, color: "#fca5a5", fontSize: "0.82rem" }}>{error}</div> : null}

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(360px, 1fr))", gap: 18, marginBottom: 18 }}>
        <Panel title="Search Results" accent="#38bdf8" style={{ minHeight: 420 }}>
          <div style={{ ...commandScrollStyle, maxHeight: 360, paddingRight: 6 }}>
            {results.length === 0 ? <EmptyState text={searching ? "Searching..." : "No results yet"} /> : null}
            {results.map((result) => {
              const active = result.path === selectedPath;
              return (
                <button type="button"
                  key={result.path}
                  onClick={() => void loadFileDetails(result.path, result)}
                  style={{
                    ...commandInsetStyle,
                    marginBottom: 10,
                    width: "100%",
                    textAlign: "left",
                    cursor: "pointer",
                    borderColor: active ? "rgba(0, 255, 204, 0.4)" : "rgba(148, 163, 184, 0.16)",
                    boxShadow: active ? "0 0 0 1px rgba(0, 255, 204, 0.2)" : undefined,
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
                    <span style={{ ...commandMonoValueStyle, color: active ? "#00ffcc" : "#e2e8f0" }}>{result.path}</span>
                    <span style={{ ...commandMonoValueStyle, color: "#38bdf8" }}>{Math.round(result.relevance_score * 100)}%</span>
                  </div>
                  <MetricBar value={result.relevance_score * 100} color="#38bdf8" />
                  <div style={{ ...commandMutedStyle, marginTop: 10 }}>{result.snippet}</div>
                  {result.matched_entities.length > 0 ? (
                    <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: 10 }}>
                      {result.matched_entities.map((entity) => (
                        <span
                          key={`${result.path}-${entity}`}
                          style={{
                            padding: "5px 8px",
                            borderRadius: 999,
                            background: alpha("#38bdf8", 0.12),
                            border: "1px solid rgba(56, 189, 248, 0.2)",
                            color: "#bae6fd",
                            fontSize: "0.72rem",
                          }}
                        >
                          {entity}
                        </span>
                      ))}
                    </div>
                  ) : null}
                </button>
              );
            })}
          </div>
        </Panel>

        <Panel title="Entity Viewer" accent="#4ade80" style={{ minHeight: 420 }}>
          {!selectedPath ? <EmptyState text="Select a file from results to see extracted entities." /> : null}
          {selectedPath ? (
            <div>
              <div style={{ ...commandMonoValueStyle, marginBottom: 12, color: "#4ade80" }}>{selectedPath}</div>
              <EntityGroup title="People" items={entityGroups.people} />
              <EntityGroup title="Dates" items={entityGroups.dates} />
              <EntityGroup title="Topics" items={entityGroups.topics} />
              <EntityGroup title="Obligations" items={entityGroups.obligations} />

              <div style={{ marginTop: 18 }}>
                <div style={{ ...commandLabelStyle, marginBottom: 8 }}>Related Files</div>
                {relatedFiles.length === 0 ? <EmptyState text="No related files linked yet" compact /> : null}
                {relatedFiles.map((file) => (
                  <div key={`${file.path}-${file.linkType}`} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 10, marginBottom: 8 }}>
                      <span style={{ ...commandMonoValueStyle, color: "#e2e8f0" }}>{file.path}</span>
                      <span style={{ ...commandMonoValueStyle, color: "#00ffcc" }}>{Math.round(file.strength * 100)}%</span>
                    </div>
                    <MetricBar value={file.strength * 100} color="#00ffcc" />
                    <div style={{ ...commandMutedStyle, marginTop: 8 }}>{file.linkType}</div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </Panel>
      </div>

      <Panel title="Actions" accent="#00ffcc">
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap", marginBottom: 12 }}>
          <ActionButton accent="#00ffcc" disabled={indexing} onClick={() => void handleIndexPath("file")}>
            {indexing ? "Picking..." : "Index File"}
          </ActionButton>
          <ActionButton accent="#00ffcc" disabled={indexing} onClick={() => void handleIndexPath("directory")}>
            {indexing ? "Picking..." : "Watch Directory"}
          </ActionButton>
          <ActionButton accent="#38bdf8" disabled={rebuilding} onClick={() => void handleRebuild()}>
            {rebuilding ? "Rebuilding..." : "Rebuild Index"}
          </ActionButton>
        </div>
        <p style={{ ...commandMutedStyle, marginTop: 0 }}>
          Index file opens a file picker. Watch directory opens a folder picker. Rebuild replays the tracked file and directory registrations stored in this desktop session.
        </p>
        {trackedPaths.length > 0 ? (
          <div style={{ ...commandScrollStyle, maxHeight: 120, paddingRight: 6 }}>
            {trackedPaths.map((entry) => (
              <div key={`${entry.kind}-${entry.path}`} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12 }}>
                  <span style={{ ...commandMonoValueStyle, color: "#e2e8f0" }}>{entry.path}</span>
                  <span style={{ ...commandLabelStyle }}>{entry.kind}</span>
                </div>
                <div style={{ ...commandMutedStyle, marginTop: 8 }}>Tracked {formatRelative(Date.now())}</div>
              </div>
            ))}
          </div>
        ) : (
          <EmptyState text="No tracked files or directories yet" />
        )}
      </Panel>

      {/* ---- CogFS: Context, Entities, Graph Links ---- */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(340px, 1fr))", gap: 18, marginTop: 18, marginBottom: 18 }}>
        <Panel title="Context Lookup" accent="#c084fc">
          <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 12, alignItems: "center" }}>
            <input
              value={contextTopic}
              onChange={(e) => setContextTopic(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") void handleContextLookup(); }}
              placeholder="Enter a topic..."
              style={{ ...inputStyle, height: 42, fontSize: "0.88rem" }}
            />
            <ActionButton accent="#c084fc" disabled={contextLoading} onClick={() => void handleContextLookup()}>
              {contextLoading ? "Loading..." : "Get Context"}
            </ActionButton>
          </div>
          {contextResult !== null ? (
            <pre style={{ ...commandInsetStyle, marginTop: 12, whiteSpace: "pre-wrap", wordBreak: "break-word", maxHeight: 220, overflow: "auto", fontSize: "0.8rem", color: "#e2e8f0" }}>
              {contextResult}
            </pre>
          ) : (
            <EmptyState text="Enter a topic to retrieve context from CogFS" />
          )}
        </Panel>

        <Panel title="Entities (by path)" accent="#fb923c">
          <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 12, alignItems: "center" }}>
            <input
              value={entitiesPath}
              onChange={(e) => setEntitiesPath(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") void handleEntitiesLookup(); }}
              placeholder="File path (e.g. /home/user/doc.md)"
              style={{ ...inputStyle, height: 42, fontSize: "0.88rem" }}
            />
            <ActionButton accent="#fb923c" disabled={entitiesLoading} onClick={() => void handleEntitiesLookup()}>
              {entitiesLoading ? "Loading..." : "Get Entities"}
            </ActionButton>
          </div>
          {entitiesResult !== null ? (
            <pre style={{ ...commandInsetStyle, marginTop: 12, whiteSpace: "pre-wrap", wordBreak: "break-word", maxHeight: 220, overflow: "auto", fontSize: "0.8rem", color: "#e2e8f0" }}>
              {entitiesResult}
            </pre>
          ) : (
            <EmptyState text="Enter a file path to extract entities" />
          )}
        </Panel>

        <Panel title="Graph Links (by path)" accent="#22d3ee">
          <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 12, alignItems: "center" }}>
            <input
              value={graphPath}
              onChange={(e) => setGraphPath(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") void handleGraphLookup(); }}
              placeholder="File path (e.g. /home/user/doc.md)"
              style={{ ...inputStyle, height: 42, fontSize: "0.88rem" }}
            />
            <ActionButton accent="#22d3ee" disabled={graphLoading} onClick={() => void handleGraphLookup()}>
              {graphLoading ? "Loading..." : "Get Graph"}
            </ActionButton>
          </div>
          {graphResult !== null ? (
            <pre style={{ ...commandInsetStyle, marginTop: 12, whiteSpace: "pre-wrap", wordBreak: "break-word", maxHeight: 220, overflow: "auto", fontSize: "0.8rem", color: "#e2e8f0" }}>
              {graphResult}
            </pre>
          ) : (
            <EmptyState text="Enter a file path to view graph links" />
          )}
        </Panel>
      </div>

      {/* ---- Neural Bridge ---- */}
      <Panel title="Neural Bridge" accent="#f472b6" style={{ marginBottom: 18 }}>
        {/* Status + Toggle */}
        <div style={{ display: "flex", alignItems: "center", gap: 14, marginBottom: 14 }}>
          <span style={{ ...commandLabelStyle }}>Status:</span>
          {nbStatusLoading ? (
            <span style={{ ...commandMutedStyle }}>Loading...</span>
          ) : (
            <span style={{ ...commandMonoValueStyle, color: nbEnabled ? "#4ade80" : "#fca5a5" }}>
              {nbEnabled ? "Enabled" : "Disabled"}
            </span>
          )}
          <ActionButton accent={nbEnabled ? "#fca5a5" : "#4ade80"} disabled={nbStatusLoading} onClick={() => void handleNbToggle()}>
            {nbEnabled ? "Disable" : "Enable"}
          </ActionButton>
          {nbStatus ? (
            <span style={{ ...commandMutedStyle, marginLeft: 8, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {nbStatus}
            </span>
          ) : null}
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(340px, 1fr))", gap: 18 }}>
          {/* Search */}
          <div style={commandInsetStyle}>
            <div style={{ ...commandLabelStyle, marginBottom: 10 }}>Search</div>
            <input
              value={nbSearchQuery}
              onChange={(e) => setNbSearchQuery(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") void handleNbSearch(); }}
              placeholder="Search query..."
              style={{ ...inputStyle, height: 38, fontSize: "0.85rem", marginBottom: 8, width: "100%", boxSizing: "border-box" }}
            />
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 8, marginBottom: 10 }}>
              <input
                value={nbSearchTimeRange}
                onChange={(e) => setNbSearchTimeRange(e.target.value)}
                placeholder="Time range (start,end)"
                style={{ ...inputStyle, height: 34, fontSize: "0.8rem" }}
              />
              <input
                value={nbSearchSourceFilter}
                onChange={(e) => setNbSearchSourceFilter(e.target.value)}
                placeholder="Source filter (csv)"
                style={{ ...inputStyle, height: 34, fontSize: "0.8rem" }}
              />
              <input
                value={nbSearchMaxResults}
                onChange={(e) => setNbSearchMaxResults(e.target.value)}
                placeholder="Max results"
                style={{ ...inputStyle, height: 34, fontSize: "0.8rem" }}
              />
            </div>
            <ActionButton accent="#f472b6" disabled={nbSearchLoading} onClick={() => void handleNbSearch()}>
              {nbSearchLoading ? "Searching..." : "Search Bridge"}
            </ActionButton>
            {nbSearchResult !== null ? (
              <pre style={{ marginTop: 10, whiteSpace: "pre-wrap", wordBreak: "break-word", maxHeight: 180, overflow: "auto", fontSize: "0.78rem", color: "#e2e8f0" }}>
                {nbSearchResult}
              </pre>
            ) : null}
          </div>

          {/* Ingest */}
          <div style={commandInsetStyle}>
            <div style={{ ...commandLabelStyle, marginBottom: 10 }}>Ingest</div>
            <div style={{ marginBottom: 8 }}>
              <select
                value={nbIngestSourceType}
                onChange={(e) => setNbIngestSourceType(e.target.value)}
                style={{ ...inputStyle, height: 36, fontSize: "0.85rem", width: "100%", boxSizing: "border-box" }}
              >
                <option value="Screen">Screen</option>
                <option value="Document">Document</option>
                <option value="Clipboard">Clipboard</option>
              </select>
            </div>
            <textarea
              value={nbIngestContent}
              onChange={(e) => setNbIngestContent(e.target.value)}
              placeholder="Content to ingest..."
              rows={4}
              style={{ ...inputStyle, fontSize: "0.83rem", width: "100%", boxSizing: "border-box", resize: "vertical", marginBottom: 8 }}
            />
            <input
              value={nbIngestMetadata}
              onChange={(e) => setNbIngestMetadata(e.target.value)}
              placeholder='Metadata JSON e.g. {"tag":"notes"}'
              style={{ ...inputStyle, height: 34, fontSize: "0.8rem", width: "100%", boxSizing: "border-box", marginBottom: 10 }}
            />
            <ActionButton accent="#f472b6" disabled={nbIngestLoading} onClick={() => void handleNbIngest()}>
              {nbIngestLoading ? "Ingesting..." : "Ingest"}
            </ActionButton>
            {nbIngestResult !== null ? (
              <pre style={{ marginTop: 10, whiteSpace: "pre-wrap", wordBreak: "break-word", maxHeight: 100, overflow: "auto", fontSize: "0.78rem", color: "#e2e8f0" }}>
                {nbIngestResult}
              </pre>
            ) : null}
          </div>

          {/* Delete + Clear Old */}
          <div style={commandInsetStyle}>
            <div style={{ ...commandLabelStyle, marginBottom: 10 }}>Manage Entries</div>
            <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 8, alignItems: "center", marginBottom: 12 }}>
              <input
                value={nbDeleteId}
                onChange={(e) => setNbDeleteId(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") void handleNbDelete(); }}
                placeholder="Entry ID to delete"
                style={{ ...inputStyle, height: 36, fontSize: "0.83rem" }}
              />
              <ActionButton accent="#fca5a5" disabled={nbDeleteLoading} onClick={() => void handleNbDelete()}>
                {nbDeleteLoading ? "Deleting..." : "Delete"}
              </ActionButton>
            </div>
            {nbDeleteResult !== null ? (
              <pre style={{ marginBottom: 12, whiteSpace: "pre-wrap", wordBreak: "break-word", fontSize: "0.78rem", color: "#e2e8f0" }}>
                {nbDeleteResult}
              </pre>
            ) : null}

            <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 8, alignItems: "center" }}>
              <input
                value={nbClearDays}
                onChange={(e) => setNbClearDays(e.target.value)}
                placeholder="Older than N days"
                style={{ ...inputStyle, height: 36, fontSize: "0.83rem" }}
              />
              <ActionButton accent="#fca5a5" disabled={nbClearLoading} onClick={() => void handleNbClear()}>
                {nbClearLoading ? "Clearing..." : "Clear Old"}
              </ActionButton>
            </div>
            {nbClearResult !== null ? (
              <pre style={{ marginTop: 10, whiteSpace: "pre-wrap", wordBreak: "break-word", fontSize: "0.78rem", color: "#e2e8f0" }}>
                {nbClearResult}
              </pre>
            ) : null}
            <p style={{ ...commandMutedStyle, marginTop: 10, marginBottom: 0 }}>
              Delete removes a single entry by ID. Clear Old removes all entries older than the specified number of days.
            </p>
          </div>
        </div>
      </Panel>
    </div>
  );
}
