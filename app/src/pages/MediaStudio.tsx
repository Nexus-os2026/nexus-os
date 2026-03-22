import { convertFileSrc } from "@tauri-apps/api/core";
import RequiresLlm from "../components/RequiresLlm";
import { Dot } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  analyzeMediaFile,
  executeAgentGoal,
  fileManagerCreateDir,
  fileManagerList,
  listAgents,
} from "../api/backend";
import type { AgentSummary } from "../types";

type MediaEntry = {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
};

const DEFAULT_MEDIA_WORKSPACE = "/home/nexus/.nexus/media";

function detectMediaType(path: string): "image" | "video" | "audio" | "other" {
  const lower = path.toLowerCase();
  if (/\.(png|jpe?g|gif|webp|svg)$/i.test(lower)) {
    return "image";
  }
  if (/\.(mp4|mov|mkv|webm)$/i.test(lower)) {
    return "video";
  }
  if (/\.(mp3|wav|flac|m4a|ogg)$/i.test(lower)) {
    return "audio";
  }
  return "other";
}

function selectMediaAgent(agents: AgentSummary[]): string {
  return (
    agents.find((agent) => agent.capabilities?.includes("process.exec"))?.id ??
    agents[0]?.id ??
    ""
  );
}

export default function MediaStudio(): JSX.Element {
  const [workspace, setWorkspace] = useState(DEFAULT_MEDIA_WORKSPACE);
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [selectedAgentId, setSelectedAgentId] = useState("");
  const [entries, setEntries] = useState<MediaEntry[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [analysisQuery, setAnalysisQuery] = useState("Summarize this image and call out important UI elements.");
  const [analysisResult, setAnalysisResult] = useState("");
  const [ffmpegArgs, setFfmpegArgs] = useState("-vn -acodec libmp3lame -b:a 192k");
  const [outputName, setOutputName] = useState("processed-output.mp3");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refreshFiles = useCallback(async () => {
    await fileManagerCreateDir(workspace);
    const files = await fileManagerList<MediaEntry>(workspace);
    setEntries(
      files
        .filter((entry) => !entry.is_dir)
        .sort((left, right) => right.modified - left.modified),
    );
  }, [workspace]);

  useEffect(() => {
    async function load(): Promise<void> {
      try {
        const agentRows = await listAgents();
        setAgents(agentRows);
        setSelectedAgentId((current) => current || selectMediaAgent(agentRows));
        await refreshFiles();
      } catch (error) {
        setMessage(error instanceof Error ? error.message : String(error));
      }
    }

    void load();
  }, [refreshFiles]);

  const selectedEntry = useMemo(
    () => entries.find((entry) => entry.path === selectedPath) ?? null,
    [entries, selectedPath],
  );
  const selectedType = detectMediaType(selectedEntry?.path ?? "");
  const assetUrl = selectedEntry ? convertFileSrc(selectedEntry.path) : "";

  const runAnalysis = useCallback(async () => {
    if (!selectedEntry) {
      setMessage("Select an image file first.");
      return;
    }
    if (detectMediaType(selectedEntry.path) !== "image") {
      setMessage("Vision analysis is only enabled for image files.");
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const result = await analyzeMediaFile(selectedEntry.path, analysisQuery);
      setAnalysisResult(result);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }, [analysisQuery, selectedEntry]);

  const runFfmpegGoal = useCallback(async () => {
    if (!selectedEntry) {
      setMessage("Select an input file first.");
      return;
    }
    if (!selectedAgentId) {
      setMessage("Select an agent before dispatching ffmpeg work.");
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      await fileManagerCreateDir(workspace);
      const outputPath = `${workspace}/${outputName}`;
      const goalId = await executeAgentGoal(
        selectedAgentId,
        `Use the shell actuator to run ffmpeg on ${selectedEntry.path}. Produce ${outputPath} using these ffmpeg arguments after the input path: ${ffmpegArgs}. If the command succeeds, leave the processed file in place.`,
        80,
      );
      setMessage(`ffmpeg job dispatched. Goal ${goalId}. Refresh files after the agent completes.`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }, [ffmpegArgs, outputName, selectedAgentId, selectedEntry, workspace]);

  return (
    <RequiresLlm feature="Media Studio">
    <section className="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
      <header className="nexus-panel rounded-3xl p-6">
        <p className="text-xs uppercase tracking-[0.24em] text-cyan-300/70">Real Media Studio</p>
        <h2 className="nexus-display mt-2 text-3xl text-cyan-50">Media Studio</h2>
        <p className="mt-2 text-sm text-cyan-100/65">
          Lists real workspace media through `file_manager_list`, runs image analysis through the vision path, and dispatches ffmpeg work through the agent shell actuator.
        </p>
      </header>

      {message ? (
        <div className="rounded-2xl border border-cyan-500/20 bg-slate-950/60 p-4 text-sm text-cyan-100/70">
          {message}
        </div>
      ) : null}

      <div className="grid gap-4 xl:grid-cols-[0.86fr_1.14fr]">
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
            <label className="grid gap-2 text-sm text-cyan-100/70">
              Processing Agent
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
            <button
              type="button"
              onClick={() => void refreshFiles()}
              className="rounded-full border border-cyan-400/25 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100"
            >
              Refresh Workspace
            </button>
            <div className="space-y-3">
              {entries.length === 0 ? (
                <p className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4 text-sm text-cyan-100/60">
                  No media files found in the workspace.
                </p>
              ) : (
                entries.map((entry) => (
                  <button
                    key={entry.path}
                    type="button"
                    onClick={() => setSelectedPath(entry.path)}
                    className={`w-full rounded-2xl border p-4 text-left ${
                      selectedPath === entry.path
                        ? "border-cyan-400/40 bg-cyan-500/10"
                        : "border-cyan-500/15 bg-slate-950/50"
                    }`}
                  >
                    <strong className="block text-sm text-cyan-50">{entry.name}</strong>
                    <span className="mt-1 block text-xs text-cyan-100/55">
                      {detectMediaType(entry.path)} <Dot className="inline-block w-4 h-4 align-middle" /> {entry.size} bytes
                    </span>
                    <span className="mt-1 block text-xs text-cyan-100/45">
                      {new Date(entry.modified).toLocaleString()}
                    </span>
                  </button>
                ))
              )}
            </div>
          </div>
        </section>

        <section className="nexus-panel rounded-2xl p-5">
          <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
            <h3 className="text-lg text-cyan-50">Preview</h3>
            <p className="mt-1 text-sm text-cyan-100/60">
              {selectedEntry?.path ?? "Select a workspace file to preview."}
            </p>
            <div className="mt-4 min-h-[280px] overflow-hidden rounded-2xl border border-cyan-500/15 bg-slate-900/80 p-3">
              {!selectedEntry ? null : selectedType === "image" ? (
                <img src={assetUrl} alt={selectedEntry.name} className="max-h-[420px] w-full rounded-xl object-contain" />
              ) : selectedType === "video" ? (
                <video src={assetUrl} controls className="max-h-[420px] w-full rounded-xl" />
              ) : selectedType === "audio" ? (
                <audio src={assetUrl} controls className="w-full" />
              ) : (
                <p className="text-sm text-cyan-100/60">Preview is not available for this file type.</p>
              )}
            </div>
          </div>

          <div className="mt-4 grid gap-4 lg:grid-cols-2">
            <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
              <h3 className="text-lg text-cyan-50">Vision Analysis</h3>
              <textarea
                value={analysisQuery}
                onChange={(event) => setAnalysisQuery(event.target.value)}
                rows={4}
                className="mt-3 w-full rounded-2xl border border-cyan-500/20 bg-slate-950/70 px-3 py-3 text-sm text-cyan-50"
              />
              <button
                type="button"
                onClick={() => void runAnalysis()}
                disabled={busy || selectedType !== "image"}
                className="mt-3 rounded-full border border-cyan-400/30 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100"
              >
                Analyze Selected Image
              </button>
              <pre className="mt-3 whitespace-pre-wrap text-xs text-cyan-100/65">
                {analysisResult || "No analysis yet."}
              </pre>
            </div>

            <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
              <h3 className="text-lg text-cyan-50">Audio Processing</h3>
              <p className="mt-1 text-sm text-cyan-100/60">
                Dispatches a real agent goal that should execute `ffmpeg` through the shell actuator.
              </p>
              <label className="mt-3 grid gap-2 text-sm text-cyan-100/70">
                Output File
                <input
                  value={outputName}
                  onChange={(event) => setOutputName(event.target.value)}
                  className="rounded-xl border border-cyan-500/20 bg-slate-950/70 px-3 py-2 text-cyan-50"
                />
              </label>
              <label className="mt-3 grid gap-2 text-sm text-cyan-100/70">
                ffmpeg Arguments
                <textarea
                  value={ffmpegArgs}
                  onChange={(event) => setFfmpegArgs(event.target.value)}
                  rows={4}
                  className="rounded-2xl border border-cyan-500/20 bg-slate-950/70 px-3 py-3 text-sm text-cyan-50"
                />
              </label>
              <button
                type="button"
                onClick={() => void runFfmpegGoal()}
                disabled={busy || !selectedEntry || !selectedAgentId}
                className="mt-3 rounded-full border border-emerald-400/30 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-100"
              >
                Dispatch ffmpeg Job
              </button>
            </div>
          </div>
        </section>
      </div>
    </section>
    </RequiresLlm>
  );
}
