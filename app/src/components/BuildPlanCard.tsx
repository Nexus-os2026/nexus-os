import { useState, useCallback } from "react";

/* === Design tokens (must match NexusBuilder) === */
const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  borderFocus: "#2d6a5a",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentBright: "#00f0c0",
  accentDim: "rgba(0,212,170,0.10)",
  accentGlow: "rgba(0,212,170,0.25)",
  err: "#f85149",
  ok: "#3fb950",
  warn: "#f0c040",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
  sans: "system-ui,-apple-system,sans-serif",
};

export interface ProductBrief {
  project_name: string;
  project_type: string;
  target_audience: string;
  sections: string[];
  design_direction: string;
  tone: string;
  template_suggestion: string;
  estimated_cost: string;
  estimated_time: string;
}

export interface AcceptanceCriteria {
  must_have: string[];
  must_not_have: string[];
  constraints: string[];
}

interface BuildPlanCardProps {
  brief: ProductBrief;
  criteria: AcceptanceCriteria;
  planCost: number;
  planTime: number;
  planModel: string;
  onApprove: (brief: ProductBrief, criteria: AcceptanceCriteria) => void;
  onCancel: () => void;
  disabled?: boolean;
}

export default function BuildPlanCard({
  brief,
  criteria,
  planCost,
  planTime,
  planModel,
  onApprove,
  onCancel,
  disabled,
}: BuildPlanCardProps) {
  const [editing, setEditing] = useState(false);
  const [editBrief, setEditBrief] = useState<ProductBrief>({ ...brief });
  const [editCriteria, setEditCriteria] = useState<AcceptanceCriteria>({
    must_have: [...criteria.must_have],
    must_not_have: [...criteria.must_not_have],
    constraints: [...criteria.constraints],
  });

  const handleApprove = useCallback(() => {
    if (editing) {
      onApprove(editBrief, editCriteria);
    } else {
      onApprove(brief, criteria);
    }
  }, [editing, editBrief, editCriteria, brief, criteria, onApprove]);

  const startEdit = useCallback(() => {
    setEditBrief({ ...brief });
    setEditCriteria({
      must_have: [...criteria.must_have],
      must_not_have: [...criteria.must_not_have],
      constraints: [...criteria.constraints],
    });
    setEditing(true);
  }, [brief, criteria]);

  const cancelEdit = useCallback(() => {
    setEditing(false);
  }, []);

  const updateSection = useCallback((idx: number, val: string) => {
    setEditBrief(prev => {
      const sections = [...prev.sections];
      sections[idx] = val;
      return { ...prev, sections };
    });
  }, []);

  const removeSection = useCallback((idx: number) => {
    setEditBrief(prev => ({
      ...prev,
      sections: prev.sections.filter((_, i) => i !== idx),
    }));
  }, []);

  const addSection = useCallback(() => {
    setEditBrief(prev => ({
      ...prev,
      sections: [...prev.sections, ""],
    }));
  }, []);

  const updateListItem = useCallback(
    (field: "must_have" | "must_not_have", idx: number, val: string) => {
      setEditCriteria(prev => {
        const list = [...prev[field]];
        list[idx] = val;
        return { ...prev, [field]: list };
      });
    },
    []
  );

  const removeListItem = useCallback(
    (field: "must_have" | "must_not_have", idx: number) => {
      setEditCriteria(prev => ({
        ...prev,
        [field]: prev[field].filter((_, i) => i !== idx),
      }));
    },
    []
  );

  const addListItem = useCallback(
    (field: "must_have" | "must_not_have") => {
      setEditCriteria(prev => ({
        ...prev,
        [field]: [...prev[field], ""],
      }));
    },
    []
  );

  const activeBrief = editing ? editBrief : brief;
  const activeCriteria = editing ? editCriteria : criteria;

  const inputStyle = {
    background: C.bg,
    color: C.text,
    border: `1px solid ${C.border}`,
    borderRadius: 4,
    padding: "4px 8px",
    fontSize: 11,
    fontFamily: C.sans,
    outline: "none",
    boxSizing: "border-box" as const,
    width: "100%",
  };

  const labelStyle = {
    fontSize: 10,
    color: C.dim,
    fontWeight: 600 as const,
    marginBottom: 2,
    textTransform: "uppercase" as const,
    letterSpacing: 0.8,
  };

  return (
    <div
      style={{
        background: C.surfaceAlt,
        border: `1px solid ${C.accent}`,
        borderRadius: 8,
        padding: "12px 14px",
        animation: "nbfadein 0.3s ease",
        marginBottom: 8,
      }}
    >
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
        <span style={{ fontSize: 14, color: C.accent }}>&#9830;</span>
        <span style={{ fontSize: 13, fontWeight: 700, color: C.accent }}>Build Plan</span>
        <div style={{ flex: 1 }} />
        <span style={{ fontSize: 10, color: C.dim, fontFamily: C.mono }}>
          Plan: ${planCost.toFixed(4)} · {planTime.toFixed(1)}s · {planModel}
        </span>
      </div>

      {/* Project Info */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginBottom: 10 }}>
        <div>
          <div style={labelStyle}>Project Name</div>
          {editing ? (
            <input
              value={editBrief.project_name}
              onChange={(e) => setEditBrief((p) => ({ ...p, project_name: e.target.value }))}
              style={inputStyle}
            />
          ) : (
            <div style={{ fontSize: 12, color: C.text, fontWeight: 600 }}>{activeBrief.project_name}</div>
          )}
        </div>
        <div>
          <div style={labelStyle}>Type</div>
          {editing ? (
            <input
              value={editBrief.project_type}
              onChange={(e) => setEditBrief((p) => ({ ...p, project_type: e.target.value }))}
              style={inputStyle}
            />
          ) : (
            <div style={{ fontSize: 12, color: C.muted }}>{activeBrief.project_type}</div>
          )}
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginBottom: 10 }}>
        <div>
          <div style={labelStyle}>Design Direction</div>
          {editing ? (
            <input
              value={editBrief.design_direction}
              onChange={(e) => setEditBrief((p) => ({ ...p, design_direction: e.target.value }))}
              style={inputStyle}
            />
          ) : (
            <div style={{ fontSize: 11, color: C.muted }}>{activeBrief.design_direction}</div>
          )}
        </div>
        <div>
          <div style={labelStyle}>Tone</div>
          {editing ? (
            <input
              value={editBrief.tone}
              onChange={(e) => setEditBrief((p) => ({ ...p, tone: e.target.value }))}
              style={inputStyle}
            />
          ) : (
            <div style={{ fontSize: 11, color: C.muted }}>{activeBrief.tone}</div>
          )}
        </div>
      </div>

      {/* Sections */}
      <div style={{ marginBottom: 10 }}>
        <div style={labelStyle}>Sections</div>
        <div style={{ display: "flex", flexWrap: "wrap" as const, gap: 4, marginTop: 2 }}>
          {activeBrief.sections.map((s, i) =>
            editing ? (
              <div key={i} style={{ display: "flex", gap: 2, alignItems: "center" }}>
                <input
                  value={s}
                  onChange={(e) => updateSection(i, e.target.value)}
                  style={{ ...inputStyle, width: 100 }}
                />
                <button
                  onClick={() => removeSection(i)}
                  style={{ background: "transparent", border: "none", color: C.dim, cursor: "pointer", fontSize: 10, padding: "2px 4px" }}
                >
                  &#10005;
                </button>
              </div>
            ) : (
              <span
                key={i}
                style={{
                  background: C.accentDim,
                  color: C.accent,
                  padding: "2px 8px",
                  borderRadius: 10,
                  fontSize: 10,
                  fontWeight: 500,
                }}
              >
                {s}
              </span>
            )
          )}
          {editing && (
            <button
              onClick={addSection}
              style={{
                background: "transparent",
                border: `1px dashed ${C.border}`,
                color: C.dim,
                padding: "2px 8px",
                borderRadius: 10,
                fontSize: 10,
                cursor: "pointer",
              }}
            >
              + Add
            </button>
          )}
        </div>
      </div>

      {/* Acceptance Criteria */}
      <div style={{ marginBottom: 10 }}>
        <div style={labelStyle}>Must Have</div>
        <div style={{ display: "flex", flexDirection: "column" as const, gap: 3, marginTop: 2 }}>
          {activeCriteria.must_have.map((item, i) =>
            editing ? (
              <div key={i} style={{ display: "flex", gap: 4, alignItems: "center" }}>
                <span style={{ color: C.ok, fontSize: 10, flexShrink: 0 }}>&#10003;</span>
                <input
                  value={item}
                  onChange={(e) => updateListItem("must_have", i, e.target.value)}
                  style={{ ...inputStyle, flex: 1 }}
                />
                <button
                  onClick={() => removeListItem("must_have", i)}
                  style={{ background: "transparent", border: "none", color: C.dim, cursor: "pointer", fontSize: 10, padding: "2px 4px" }}
                >
                  &#10005;
                </button>
              </div>
            ) : (
              <div key={i} style={{ fontSize: 11, color: C.text, display: "flex", alignItems: "baseline", gap: 6 }}>
                <span style={{ color: C.ok, fontSize: 10 }}>&#10003;</span>
                {item}
              </div>
            )
          )}
          {editing && (
            <button
              onClick={() => addListItem("must_have")}
              style={{ background: "transparent", border: "none", color: C.dim, cursor: "pointer", fontSize: 10, textAlign: "left" as const, padding: "2px 0" }}
            >
              + Add requirement
            </button>
          )}
        </div>
      </div>

      {activeCriteria.must_not_have.length > 0 && (
        <div style={{ marginBottom: 10 }}>
          <div style={labelStyle}>Must Not Have</div>
          <div style={{ display: "flex", flexDirection: "column" as const, gap: 3, marginTop: 2 }}>
            {activeCriteria.must_not_have.map((item, i) =>
              editing ? (
                <div key={i} style={{ display: "flex", gap: 4, alignItems: "center" }}>
                  <span style={{ color: C.err, fontSize: 10, flexShrink: 0 }}>&#10007;</span>
                  <input
                    value={item}
                    onChange={(e) => updateListItem("must_not_have", i, e.target.value)}
                    style={{ ...inputStyle, flex: 1 }}
                  />
                  <button
                    onClick={() => removeListItem("must_not_have", i)}
                    style={{ background: "transparent", border: "none", color: C.dim, cursor: "pointer", fontSize: 10, padding: "2px 4px" }}
                  >
                    &#10005;
                  </button>
                </div>
              ) : (
                <div key={i} style={{ fontSize: 11, color: C.muted, display: "flex", alignItems: "baseline", gap: 6 }}>
                  <span style={{ color: C.err, fontSize: 10 }}>&#10007;</span>
                  {item}
                </div>
              )
            )}
            {editing && (
              <button
                onClick={() => addListItem("must_not_have")}
                style={{ background: "transparent", border: "none", color: C.dim, cursor: "pointer", fontSize: 10, textAlign: "left" as const, padding: "2px 0" }}
              >
                + Add exclusion
              </button>
            )}
          </div>
        </div>
      )}

      {/* Estimates */}
      <div style={{ display: "flex", gap: 16, marginBottom: 12, fontSize: 10, color: C.dim }}>
        <span>Est. build cost: <span style={{ fontFamily: C.mono, color: C.muted }}>{activeBrief.estimated_cost}</span></span>
        <span>Est. time: <span style={{ fontFamily: C.mono, color: C.muted }}>{activeBrief.estimated_time}</span></span>
      </div>

      {/* Actions */}
      <div style={{ display: "flex", gap: 6 }}>
        <button
          onClick={handleApprove}
          disabled={disabled}
          style={{
            flex: 1,
            padding: "10px 0",
            background: disabled ? "#1a2332" : `linear-gradient(135deg, ${C.accent}, ${C.accentBright})`,
            color: disabled ? C.dim : "#ffffff",
            border: "none",
            borderRadius: 6,
            fontSize: 13,
            fontWeight: 700,
            cursor: disabled ? "not-allowed" : "pointer",
            textShadow: disabled ? "none" : "0 1px 2px rgba(0,0,0,0.3)",
            boxShadow: disabled ? "none" : `0 0 12px ${C.accentGlow}`,
            transition: "filter 0.15s, transform 0.1s",
          }}
          onMouseEnter={(e) => { if (!disabled) e.currentTarget.style.filter = "brightness(1.15)"; }}
          onMouseLeave={(e) => { e.currentTarget.style.filter = ""; e.currentTarget.style.transform = "scale(1)"; }}
          onMouseDown={(e) => { if (!disabled) e.currentTarget.style.transform = "scale(0.98)"; }}
          onMouseUp={(e) => { e.currentTarget.style.transform = "scale(1)"; }}
        >
          {editing ? "Approve Edited Plan" : "Approve & Build"}
        </button>

        {!editing ? (
          <button
            onClick={startEdit}
            disabled={disabled}
            style={{
              padding: "10px 16px",
              background: C.accentDim,
              color: C.accent,
              border: `1px solid rgba(0,212,170,0.25)`,
              borderRadius: 6,
              fontSize: 12,
              fontWeight: 600,
              cursor: disabled ? "not-allowed" : "pointer",
            }}
          >
            Edit Plan
          </button>
        ) : (
          <button
            onClick={cancelEdit}
            style={{
              padding: "10px 16px",
              background: "transparent",
              color: C.dim,
              border: `1px solid ${C.border}`,
              borderRadius: 6,
              fontSize: 12,
              cursor: "pointer",
            }}
          >
            Cancel Edit
          </button>
        )}

        <button
          onClick={onCancel}
          disabled={disabled}
          style={{
            padding: "10px 12px",
            background: "transparent",
            color: C.dim,
            border: `1px solid ${C.border}`,
            borderRadius: 6,
            fontSize: 12,
            cursor: disabled ? "not-allowed" : "pointer",
          }}
        >
          &#10005;
        </button>
      </div>
    </div>
  );
}
