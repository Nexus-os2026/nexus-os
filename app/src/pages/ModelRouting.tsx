import { useEffect, useState } from "react";
import {
  routerGetAccuracy,
  routerGetModels,
  routerGetFeedback,
  routerEstimateDifficulty,
} from "../api/backend";
import {
  ActionButton,
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
  inputStyle,
  normalizeArray,
} from "./commandCenterUi";

const ACCENT = "#06b6d4";

interface Accuracy { total_decisions: number; model_sufficient: number; model_insufficient: number; unnecessary_staging: number; missed_staging: number; accuracy: number }
interface ModelProfile { model_id: string; provider: string; display_name: string; cost_per_1k_input: number; avg_latency_ms: number; available: boolean; is_local: boolean; size_class: string; vector_scores: { reasoning_depth: number; planning_coherence: number; adaptation: number; tool_use: number } }
interface Estimate { reasoning_difficulty: number; planning_difficulty: number; adaptation_difficulty: number; tool_use_difficulty: number; dominant_vector: string; confidence: number; method: string }
interface Feedback { total_analyzed: number; accurate: number; over_estimated: number; under_estimated: number; threshold_recommendation: string | { [key: string]: unknown } }

export default function ModelRouting() {
  const [accuracy, setAccuracy] = useState<Accuracy | null>(null);
  const [models, setModels] = useState<ModelProfile[]>([]);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [taskInput, setTaskInput] = useState("");
  const [estimate, setEstimate] = useState<Estimate | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([routerGetAccuracy(), routerGetModels(), routerGetFeedback()])
      .then(([a, m, f]) => {
        setAccuracy(a as Accuracy);
        setModels(normalizeArray<ModelProfile>(m));
        setFeedback(f as Feedback);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const handleEstimate = () => {
    if (!taskInput.trim()) return;
    routerEstimateDifficulty(taskInput)
      .then((e) => setEstimate(e as Estimate))
      .catch(console.error);
  };

  if (loading) return <div style={commandPageStyle}><div style={{ textAlign: "center", padding: 48, color: "#888" }}>Loading router data...</div></div>;

  return (
    <div style={commandPageStyle}>
      <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0, marginBottom: 4 }}>Predictive Model Routing</h1>
      <p style={{ ...commandMutedStyle, marginBottom: 24, fontSize: 13 }}>
        Routes tasks to the smallest capable model based on empirical capability boundaries.
      </p>

      {/* Accuracy */}
      <Panel title="Routing Accuracy">
        {accuracy && accuracy.total_decisions > 0 ? (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 12 }}>
            {[
              ["Decisions", accuracy.total_decisions],
              ["Sufficient", accuracy.model_sufficient],
              ["Insufficient", accuracy.model_insufficient],
              ["Unnecessary Staging", accuracy.unnecessary_staging],
              ["Missed Staging", accuracy.missed_staging],
            ].map(([label, val]) => (
              <div key={label as string}>
                <div style={{ fontSize: 11, color: "#94a3b8" }}>{label as string}</div>
                <div style={{ ...commandMonoValueStyle, fontSize: 18 }}>{val as number}</div>
              </div>
            ))}
          </div>
        ) : (
          <EmptyState text="No routing decisions recorded yet. Tasks will be routed as agents execute." />
        )}
      </Panel>

      {/* Difficulty Estimator */}
      <Panel title="Task Difficulty Estimator" style={{ marginTop: 16 }}>
        <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          <input
            style={{ ...inputStyle, flex: 1 }}
            placeholder="Enter task text to estimate difficulty..."
            value={taskInput}
            onChange={(e) => setTaskInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleEstimate()}
          />
          <ActionButton accent={ACCENT} onClick={handleEstimate}>Estimate</ActionButton>
        </div>
        {estimate && (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 12, padding: 12, background: alpha("#0f172a", 0.5), borderRadius: 8 }}>
            {[
              ["Reasoning", estimate.reasoning_difficulty, "#818cf8"],
              ["Planning", estimate.planning_difficulty, "#34d399"],
              ["Adaptation", estimate.adaptation_difficulty, "#fbbf24"],
              ["Tool Use", estimate.tool_use_difficulty, "#f472b6"],
            ].map(([label, val, color]) => (
              <div key={label as string} style={{ textAlign: "center" }}>
                <div style={{ fontSize: 11, color: color as string }}>{label as string}</div>
                <div style={{ fontSize: 20, fontWeight: 700, fontFamily: "monospace", color: color as string }}>
                  {((val as number) * 100).toFixed(0)}%
                </div>
              </div>
            ))}
            <div style={{ gridColumn: "1 / -1", textAlign: "center", fontSize: 11, color: "#94a3b8", marginTop: 4 }}>
              Dominant: {estimate.dominant_vector} | Confidence: {(estimate.confidence * 100).toFixed(0)}% | Method: {estimate.method}
            </div>
          </div>
        )}
      </Panel>

      {/* Model Registry */}
      <Panel title={`Model Registry (${models.length})`} style={{ marginTop: 16 }}>
        {models.length === 0 ? (
          <EmptyState text="No models registered. Models will be added as providers are configured." />
        ) : (
          <div style={commandScrollStyle}>
            {models.map((m) => (
              <div key={m.model_id} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "6px 0", borderBottom: "1px solid #1e293b", fontSize: 12 }}>
                <div>
                  <span style={{ color: "#e2e8f0" }}>{m.display_name}</span>
                  <span style={{ color: "#64748b", marginLeft: 8, fontSize: 10 }}>{m.provider} | {m.size_class}</span>
                </div>
                <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
                  <StatusDot color={m.available ? "#22c55e" : "#ef4444"} />
                  <span style={commandMonoValueStyle}>${m.cost_per_1k_input.toFixed(3)}/1K</span>
                  <span style={{ color: "#94a3b8", fontSize: 10 }}>{m.avg_latency_ms}ms</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>

      {/* Feedback */}
      {feedback && feedback.total_analyzed > 0 && (
        <Panel title="Feedback Analysis" style={{ marginTop: 16 }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 12 }}>
            <div><div style={{ fontSize: 11, color: "#94a3b8" }}>Analyzed</div><div style={commandMonoValueStyle}>{feedback.total_analyzed}</div></div>
            <div><div style={{ fontSize: 11, color: "#22c55e" }}>Accurate</div><div style={{ ...commandMonoValueStyle, color: "#22c55e" }}>{feedback.accurate}</div></div>
            <div><div style={{ fontSize: 11, color: "#f97316" }}>Over-estimated</div><div style={{ ...commandMonoValueStyle, color: "#f97316" }}>{feedback.over_estimated}</div></div>
            <div><div style={{ fontSize: 11, color: "#ef4444" }}>Under-estimated</div><div style={{ ...commandMonoValueStyle, color: "#ef4444" }}>{feedback.under_estimated}</div></div>
          </div>
        </Panel>
      )}
    </div>
  );
}
