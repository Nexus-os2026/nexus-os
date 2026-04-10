import { useCallback, useEffect, useState } from "react";
import { builderGetBudget, builderSetBudget } from "../api/backend";

interface BudgetStatus {
  anthropic_initial: number;
  anthropic_spent: number;
  anthropic_remaining: number;
  openai_initial: number;
  openai_spent: number;
  openai_remaining: number;
  total_builds: number;
  avg_cost_per_build: number;
  estimated_builds_remaining: number;
}

const containerStyle: React.CSSProperties = {
  background: "#161b22",
  borderRadius: 10,
  padding: 16,
  border: "1px solid #30363d",
  marginBottom: 12,
};

const headingStyle: React.CSSProperties = {
  fontSize: 13,
  fontWeight: 700,
  color: "#e6edf3",
  marginBottom: 12,
  letterSpacing: 0.5,
};

const rowStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  marginBottom: 8,
};

const labelStyle: React.CSSProperties = {
  fontSize: 12,
  color: "#8b949e",
};

const valueStyle: React.CSSProperties = {
  fontSize: 12,
  color: "#e6edf3",
  fontWeight: 600,
  fontFamily: "monospace",
};

const barTrackStyle: React.CSSProperties = {
  height: 4,
  borderRadius: 2,
  background: "#30363d",
  marginBottom: 10,
  overflow: "hidden",
};

const statRowStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  padding: "4px 0",
  borderTop: "1px solid #30363d",
};

const inputStyle: React.CSSProperties = {
  width: 70,
  padding: "3px 6px",
  borderRadius: 4,
  border: "1px solid #30363d",
  background: "#0d1117",
  color: "#e6edf3",
  fontSize: 12,
  fontFamily: "monospace",
};

const btnStyle: React.CSSProperties = {
  padding: "3px 10px",
  borderRadius: 4,
  border: "none",
  background: "#58a6ff",
  color: "#0d1117",
  fontSize: 11,
  fontWeight: 600,
  cursor: "pointer",
};

function ProviderRow(props: {
  name: string;
  initial: number;
  spent: number;
  remaining: number;
  onSet: (amount: number) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [inputVal, setInputVal] = useState(String(props.initial));
  const pct = props.initial > 0 ? ((props.remaining / props.initial) * 100) : 0;
  const barColor = pct > 40 ? "#3fb950" : pct > 15 ? "#d29922" : "#f85149";

  return (
    <div>
      <div style={rowStyle}>
        <span style={labelStyle}>{props.name}</span>
        {editing ? (
          <span style={{ display: "flex", gap: 4, alignItems: "center" }}>
            <span style={{ fontSize: 12, color: "#8b949e" }}>$</span>
            <input
              style={inputStyle}
              value={inputVal}
              onChange={(e) => setInputVal(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  const v = parseFloat(inputVal);
                  if (!isNaN(v) && v >= 0) {
                    props.onSet(v);
                    setEditing(false);
                  }
                }
              }}
            />
            <button type="button"
              style={btnStyle}
              onClick={() => {
                const v = parseFloat(inputVal);
                if (!isNaN(v) && v >= 0) {
                  props.onSet(v);
                  setEditing(false);
                }
              }}
            >
              Set
            </button>
          </span>
        ) : (
          <span
            style={{ ...valueStyle, cursor: "pointer" }}
            onClick={() => { setInputVal(String(props.initial)); setEditing(true); }}
            title="Click to edit budget"
          >
            ${props.remaining.toFixed(2)} / ${props.initial.toFixed(2)}
          </span>
        )}
      </div>
      <div style={barTrackStyle}>
        <div
          style={{
            height: "100%",
            width: `${Math.min(pct, 100)}%`,
            background: barColor,
            borderRadius: 2,
            transition: "width 0.3s ease",
          }}
        />
      </div>
    </div>
  );
}

export default function BudgetPanel() {
  const [status, setStatus] = useState<BudgetStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    builderGetBudget()
      .then((s) => { setStatus(s); setError(null); })
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleSetBudget = useCallback((provider: string, amount: number) => {
    builderSetBudget(provider, amount)
      .then(() => load())
      .catch((e) => setError(String(e)));
  }, [load]);

  if (error) {
    return (
      <div style={{ ...containerStyle, borderColor: "#f85149" }}>
        <div style={{ fontSize: 12, color: "#f85149" }}>Budget: {error}</div>
      </div>
    );
  }

  if (!status) {
    return (
      <div style={containerStyle}>
        <div style={{ fontSize: 12, color: "#8b949e" }}>Loading budget...</div>
      </div>
    );
  }

  return (
    <div style={containerStyle}>
      <div style={headingStyle}>API Budget</div>
      <ProviderRow
        name="Anthropic"
        initial={status.anthropic_initial}
        spent={status.anthropic_spent}
        remaining={status.anthropic_remaining}
        onSet={(v) => handleSetBudget("anthropic", v)}
      />
      <ProviderRow
        name="OpenAI"
        initial={status.openai_initial}
        spent={status.openai_spent}
        remaining={status.openai_remaining}
        onSet={(v) => handleSetBudget("openai", v)}
      />
      <div style={statRowStyle}>
        <span style={labelStyle}>Builds completed</span>
        <span style={valueStyle}>{status.total_builds}</span>
      </div>
      <div style={statRowStyle}>
        <span style={labelStyle}>Avg cost / build</span>
        <span style={valueStyle}>
          {status.avg_cost_per_build > 0
            ? `$${status.avg_cost_per_build.toFixed(4)}`
            : "--"}
        </span>
      </div>
      <div style={statRowStyle}>
        <span style={labelStyle}>Est. builds remaining</span>
        <span style={valueStyle}>
          {status.estimated_builds_remaining > 0
            ? `~${status.estimated_builds_remaining}`
            : "--"}
        </span>
      </div>
    </div>
  );
}
