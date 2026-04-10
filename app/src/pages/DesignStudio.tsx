import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  executeAgentGoal,
  fileManagerCreateDir,
  fileManagerList,
  fileManagerRead,
  fileManagerWrite,
  listAgents,
} from "../api/backend";
import type { AgentSummary } from "../types";
import RequiresLlm from "../components/RequiresLlm";

type WorkspaceEntry = {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
};

const DEFAULT_WORKSPACE = "/home/nexus/.nexus/agents/design-studio";

function inferPreferredAgent(agents: AgentSummary[]): string {
  return (
    agents.find((agent) => agent.name.toLowerCase().includes("design"))?.id ??
    agents.find((agent) => agent.capabilities?.includes("fs.write"))?.id ??
    agents[0]?.id ??
    ""
  );
}

export default function DesignStudio(): JSX.Element {
  const [workspace, setWorkspace] = useState(DEFAULT_WORKSPACE);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [selectedAgentId, setSelectedAgentId] = useState("");
  const [format, setFormat] = useState<"html" | "svg">("html");
  const [filename, setFilename] = useState("dashboard-card.html");
  const [prompt, setPrompt] = useState(
    "A compact operator dashboard card with title, health badge, and two metrics.",
  );
  const [markup, setMarkup] = useState(
    "<section style=\"padding:20px;border-radius:18px;background:#08111f;color:#d8f5ff;border:1px solid rgba(34,211,238,.25);font-family:ui-sans-serif\"><p style=\"margin:0 0 8px;font-size:12px;letter-spacing:.18em;text-transform:uppercase;color:#67e8f9\">Design Draft</p><h2 style=\"margin:0 0 12px;font-size:24px\">Operator Card</h2><div style=\"display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px\"><div style=\"padding:12px;border-radius:14px;background:rgba(15,23,42,.85)\"><strong>Agents</strong><div>12 active</div></div><div style=\"padding:12px;border-radius:14px;background:rgba(15,23,42,.85)\"><strong>Fuel</strong><div>84% remaining</div></div></div></section>",
  );
  const [files, setFiles] = useState<WorkspaceEntry[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [goalId, setGoalId] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [generating, setGenerating] = useState(false);
  const genTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (genTimerRef.current) clearTimeout(genTimerRef.current);
    };
  }, []);

  const refreshWorkspace = useCallback(async () => {
    await fileManagerCreateDir(workspace);
    const entries = await fileManagerList<WorkspaceEntry>(workspace);
    const designFiles = entries
      .filter((entry) => !entry.is_dir && /\.(html|svg)$/i.test(entry.name))
      .sort((left, right) => right.modified - left.modified);
    setFiles(designFiles);
  }, [workspace]);

  useEffect(() => {
    async function load(): Promise<void> {
      setLoading(true);
      try {
        const agentRows = await listAgents();
        setAgents(agentRows);
        setSelectedAgentId((current) => current || inferPreferredAgent(agentRows));
        await refreshWorkspace();
      } catch (error) {
        setMessage(error instanceof Error ? error.message : String(error));
      } finally {
        setLoading(false);
      }
    }

    void load();
  }, [refreshWorkspace]);

  const selectedAgentName = useMemo(
    () => agents.find((agent) => agent.id === selectedAgentId)?.name ?? "No agent selected",
    [agents, selectedAgentId],
  );

  const selectedFilePath = selectedFile ?? `${workspace}/${filename}`;

  const loadFile = useCallback(async (path: string) => {
    try {
      const content = await fileManagerRead(path);
      setSelectedFile(path);
      setFilename(path.split("/").pop() ?? filename);
      setFormat(path.toLowerCase().endsWith(".svg") ? "svg" : "html");
      setMarkup(content);
      setMessage(`Loaded ${path}`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }, [filename]);

  const saveCurrentMarkup = useCallback(async () => {
    setSaving(true);
    setMessage(null);
    try {
      await fileManagerCreateDir(workspace);
      const path = `${workspace}/${filename}`;
      await fileManagerWrite(path, markup);
      setSelectedFile(path);
      await refreshWorkspace();
      setMessage(`Saved ${path}`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }, [filename, markup, refreshWorkspace, workspace]);

  const generateComponent = useCallback(async () => {
    if (!selectedAgentId) {
      setMessage("Select an agent before generating.");
      return;
    }
    setGenerating(true);
    setMessage(null);
    try {
      await fileManagerCreateDir(workspace);
      const targetPath = `${workspace}/${filename}`;
      const nextGoalId = await executeAgentGoal(
        selectedAgentId,
        `Generate a standalone ${format.toUpperCase()} UI component for this design request: ${prompt}. Save the completed markup to ${targetPath}. Use only valid ${format.toUpperCase()} markup.`,
        60,
      );
      setGoalId(nextGoalId);
      setMessage(`Generation dispatched to ${selectedAgentName}. Goal ${nextGoalId}.`);
      genTimerRef.current = window.setTimeout(() => {
        void refreshWorkspace().then(async () => {
          try {
            const content = await fileManagerRead(targetPath);
            setSelectedFile(targetPath);
            setMarkup(content);
          } catch {
            // The agent may still be working; the goal id is shown so the user can track it elsewhere.
          }
        });
      }, 4000);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setGenerating(false);
    }
  }, [filename, format, prompt, refreshWorkspace, selectedAgentId, selectedAgentName, workspace]);

  return (
    <RequiresLlm feature="Design Studio">
    <section className="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
      <header className="nexus-panel rounded-3xl p-6">
        <p className="text-xs uppercase tracking-[0.24em] text-cyan-300/70">Real Design Studio</p>
        <h2 className="nexus-display mt-2 text-3xl text-cyan-50">Design Studio</h2>
        <p className="mt-2 text-sm text-cyan-100/65">
          Draft markup locally, save it with `file_manager_write`, or dispatch a real agent goal that writes a generated component into the workspace.
        </p>
      </header>

      {message ? (
        <div className="rounded-2xl border border-cyan-500/20 bg-slate-950/60 p-4 text-sm text-cyan-100/70">
          {message}
        </div>
      ) : null}

      <div className="grid gap-4 xl:grid-cols-[0.95fr_1.05fr]">
        <section className="nexus-panel rounded-2xl p-5">
          <div className="grid gap-4">
            <label className="grid gap-2 text-sm text-cyan-100/70">
              Workspace
              <input
                value={workspace}
                onChange={(event) => setWorkspace(event.target.value)}
                className="rounded-xl border border-cyan-500/20 bg-slate-950/60 px-3 py-2 text-cyan-50"
              />
            </label>
            <div className="grid gap-4 md:grid-cols-2">
              <label className="grid gap-2 text-sm text-cyan-100/70">
                File Name
                <input
                  value={filename}
                  onChange={(event) => setFilename(event.target.value)}
                  className="rounded-xl border border-cyan-500/20 bg-slate-950/60 px-3 py-2 text-cyan-50"
                />
              </label>
              <label className="grid gap-2 text-sm text-cyan-100/70">
                Format
                <select
                  value={format}
                  onChange={(event) => setFormat(event.target.value as "html" | "svg")}
                  className="rounded-xl border border-cyan-500/20 bg-slate-950/60 px-3 py-2 text-cyan-50"
                >
                  <option value="html">HTML</option>
                  <option value="svg">SVG</option>
                </select>
              </label>
            </div>
            <label className="grid gap-2 text-sm text-cyan-100/70">
              Generation Prompt
              <textarea
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                rows={4}
                className="rounded-2xl border border-cyan-500/20 bg-slate-950/60 px-3 py-3 text-cyan-50"
              />
            </label>
            <label className="grid gap-2 text-sm text-cyan-100/70">
              Generation Agent
              <select
                value={selectedAgentId}
                onChange={(event) => setSelectedAgentId(event.target.value)}
                className="rounded-xl border border-cyan-500/20 bg-slate-950/60 px-3 py-2 text-cyan-50"
              >
                <option value="">Select agent</option>
                {agents.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {agent.name}
                  </option>
                ))}
              </select>
            </label>
            <div className="flex flex-wrap gap-3">
              <button type="button"
                onClick={() => void saveCurrentMarkup()}
                disabled={saving}
                className="rounded-full border border-emerald-400/30 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-100"
              >
                {saving ? "Saving..." : "Save Markup"}
              </button>
              <button type="button"
                onClick={() => void generateComponent()}
                disabled={generating || !selectedAgentId}
                className="rounded-full border border-cyan-400/30 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100"
              >
                {generating ? "Dispatching..." : "Generate With Agent"}
              </button>
              <button type="button"
                onClick={() => void refreshWorkspace()}
                className="rounded-full border border-cyan-400/20 bg-slate-950/60 px-4 py-2 text-sm text-cyan-100/75"
              >
                Refresh Files
              </button>
            </div>
            {goalId ? (
              <p className="text-xs text-cyan-100/55">Latest cognitive goal: {goalId}</p>
            ) : null}
          </div>
        </section>

        <section className="nexus-panel rounded-2xl p-5">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h3 className="text-lg text-cyan-50">Canvas Export</h3>
              <p className="text-sm text-cyan-100/60">Current file: {selectedFilePath}</p>
            </div>
          </div>
          <textarea
            value={markup}
            onChange={(event) => setMarkup(event.target.value)}
            rows={18}
            className="mt-4 w-full rounded-2xl border border-cyan-500/20 bg-slate-950/70 px-3 py-3 font-mono text-sm text-cyan-50"
          />
          <div className="mt-4 rounded-2xl border border-cyan-500/15 bg-white p-4">
            {format === "html" ? (
              <iframe
                title="Design Preview"
                srcDoc={markup}
                className="h-[320px] w-full rounded-xl border border-slate-200"
              />
            ) : (
              <div
                className="flex min-h-[320px] items-center justify-center rounded-xl border border-slate-200"
                dangerouslySetInnerHTML={{ __html: markup }}
              />
            )}
          </div>
        </section>
      </div>

      <section className="nexus-panel rounded-2xl p-5">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h3 className="text-lg text-cyan-50">Workspace Files</h3>
            <p className="text-sm text-cyan-100/60">
              Backed by `file_manager_list` and `file_manager_read`.
            </p>
          </div>
        </div>
        {loading ? (
          <p className="mt-4 text-sm text-cyan-100/60">Loading workspace...</p>
        ) : files.length === 0 ? (
          <p className="mt-4 rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4 text-sm text-cyan-100/60">
            No saved design files yet.
          </p>
        ) : (
          <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            {files.map((file) => (
              <button type="button"
                key={file.path}
                onClick={() => void loadFile(file.path)}
                className="rounded-2xl border border-cyan-500/15 bg-slate-950/55 p-4 text-left"
              >
                <strong className="block text-sm text-cyan-50">{file.name}</strong>
                <span className="mt-1 block text-xs text-cyan-100/55">
                  {new Date(file.modified).toLocaleString()}
                </span>
                <span className="mt-2 block text-xs text-cyan-100/55">{file.size} bytes</span>
              </button>
            ))}
          </div>
        )}
      </section>
    </section>
    </RequiresLlm>
  );
}
