import { useCallback, useEffect, useState } from "react";
import {
  policyList,
  policyValidate,
  policyTest,
  policyDetectConflicts,
} from "../api/backend";
import type { PolicyEntry, PolicyConflict, PolicyTestResult } from "../types";

const DEFAULT_TOML = `policy_id = "my-policy"
description = "Example policy"
effect = "Allow"
principal = "*"
action = "tool_call"
resource = "web.*"
priority = 100

[conditions]
max_fuel_cost = 500
`;

const EFFECT_COLORS: Record<string, string> = {
  Allow: "#00ff9d",
  Deny: "#ff4444",
};

export default function PolicyManagement() {
  const [policies, setPolicies] = useState<PolicyEntry[]>([]);
  const [conflicts, setConflicts] = useState<PolicyConflict[]>([]);
  const [toml, setToml] = useState(DEFAULT_TOML);
  const [validateMsg, setValidateMsg] = useState("");
  const [validateOk, setValidateOk] = useState(false);

  // Test panel state
  const [testPrincipal, setTestPrincipal] = useState("*");
  const [testAction, setTestAction] = useState("tool_call");
  const [testResource, setTestResource] = useState("web.search");
  const [testResult, setTestResult] = useState<PolicyTestResult | null>(null);
  const [testError, setTestError] = useState("");

  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<"list" | "editor" | "test" | "conflicts">("list");

  const refresh = useCallback(async () => {
    try {
      const [p, c] = await Promise.all([policyList(), policyDetectConflicts()]);
      setPolicies(p);
      setConflicts(c);
    } catch {
      setPolicies([]);
      setConflicts([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleValidate = async () => {
    setValidateMsg("");
    try {
      const result = await policyValidate(toml);
      setValidateMsg(`Valid policy: ${result.policy_id} (${result.effect}, priority ${result.priority})`);
      setValidateOk(true);
    } catch (e) {
      setValidateMsg(String(e));
      setValidateOk(false);
    }
  };

  const handleTest = async () => {
    setTestResult(null);
    setTestError("");
    try {
      const result = await policyTest(toml, testPrincipal, testAction, testResource);
      setTestResult(result);
    } catch (e) {
      setTestError(String(e));
    }
  };

  const effectBadge = (effect: string) => (
    <span
      style={{
        color: EFFECT_COLORS[effect] ?? "#aaa",
        border: `1px solid ${EFFECT_COLORS[effect] ?? "#555"}`,
        borderRadius: 4,
        padding: "2px 8px",
        fontSize: 12,
        fontWeight: 600,
      }}
    >
      {effect}
    </span>
  );

  return (
    <div style={{ padding: 32, color: "#e0e0e0", maxWidth: 1100, margin: "0 auto" }}>
      <h1 style={{ color: "#00ff9d", marginBottom: 8 }}>Policy Management</h1>
      <p style={{ color: "#888", marginBottom: 24 }}>
        Cedar-inspired governance policies. Default-deny; deny overrides allow.
      </p>

      {/* Tab bar */}
      <div style={{ display: "flex", gap: 4, marginBottom: 24 }}>
        {(["list", "editor", "test", "conflicts"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            style={{
              padding: "8px 20px",
              background: tab === t ? "#00ff9d22" : "#1a1a2e",
              color: tab === t ? "#00ff9d" : "#888",
              border: `1px solid ${tab === t ? "#00ff9d44" : "#333"}`,
              borderRadius: 6,
              cursor: "pointer",
              fontWeight: tab === t ? 600 : 400,
              textTransform: "capitalize",
            }}
          >
            {t === "conflicts" ? `Conflicts (${conflicts.length})` : t}
          </button>
        ))}
      </div>

      {/* List tab */}
      {tab === "list" && (
        <div>
          <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 12 }}>
            <h2 style={{ margin: 0 }}>Loaded Policies ({policies.length})</h2>
            <button
              onClick={refresh}
              style={{
                padding: "6px 16px",
                background: "#1a1a2e",
                color: "#00ff9d",
                border: "1px solid #00ff9d44",
                borderRadius: 4,
                cursor: "pointer",
              }}
            >
              Reload
            </button>
          </div>
          {loading ? (
            <p style={{ color: "#666" }}>Loading...</p>
          ) : policies.length === 0 ? (
            <p style={{ color: "#666" }}>
              No policies loaded. Place .toml files in ~/.nexus/policies/ or use the editor.
            </p>
          ) : (
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead>
                <tr style={{ borderBottom: "1px solid #333", color: "#888", textAlign: "left" }}>
                  <th style={{ padding: "8px 12px" }}>ID</th>
                  <th style={{ padding: "8px 12px" }}>Description</th>
                  <th style={{ padding: "8px 12px" }}>Effect</th>
                  <th style={{ padding: "8px 12px" }}>Priority</th>
                  <th style={{ padding: "8px 12px" }}>Scope</th>
                </tr>
              </thead>
              <tbody>
                {policies
                  .sort((a, b) => b.priority - a.priority)
                  .map((p) => (
                    <tr key={p.policy_id} style={{ borderBottom: "1px solid #222" }}>
                      <td style={{ padding: "8px 12px", fontFamily: "monospace", color: "#7ecbff" }}>
                        {p.policy_id}
                      </td>
                      <td style={{ padding: "8px 12px" }}>{p.description}</td>
                      <td style={{ padding: "8px 12px" }}>{effectBadge(p.effect)}</td>
                      <td style={{ padding: "8px 12px", fontFamily: "monospace" }}>{p.priority}</td>
                      <td style={{ padding: "8px 12px", fontFamily: "monospace", fontSize: 12, color: "#aaa" }}>
                        {p.principal} / {p.action} / {p.resource}
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {/* Editor tab */}
      {tab === "editor" && (
        <div>
          <h2 style={{ marginTop: 0 }}>Policy Editor</h2>
          <textarea
            value={toml}
            onChange={(e) => {
              setToml(e.target.value);
              setValidateMsg("");
            }}
            spellCheck={false}
            style={{
              width: "100%",
              minHeight: 300,
              background: "#0d0d1a",
              color: "#c8e6c9",
              border: "1px solid #333",
              borderRadius: 6,
              padding: 16,
              fontFamily: "'Fira Code', 'Cascadia Code', monospace",
              fontSize: 14,
              lineHeight: 1.6,
              resize: "vertical",
              outline: "none",
            }}
          />
          <div style={{ display: "flex", gap: 12, marginTop: 12 }}>
            <button
              onClick={handleValidate}
              style={{
                padding: "8px 20px",
                background: "#00ff9d22",
                color: "#00ff9d",
                border: "1px solid #00ff9d44",
                borderRadius: 4,
                cursor: "pointer",
              }}
            >
              Validate
            </button>
          </div>
          {validateMsg && (
            <p
              style={{
                marginTop: 12,
                padding: 12,
                background: validateOk ? "#00ff9d11" : "#ff444411",
                border: `1px solid ${validateOk ? "#00ff9d33" : "#ff444433"}`,
                borderRadius: 4,
                color: validateOk ? "#00ff9d" : "#ff4444",
                fontFamily: "monospace",
                fontSize: 13,
              }}
            >
              {validateMsg}
            </p>
          )}
        </div>
      )}

      {/* Test tab */}
      {tab === "test" && (
        <div>
          <h2 style={{ marginTop: 0 }}>Dry-Run Policy Test</h2>
          <p style={{ color: "#888", marginBottom: 16 }}>
            Evaluate the editor TOML against a simulated request.
          </p>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 12, marginBottom: 16 }}>
            <div>
              <label style={{ color: "#888", fontSize: 12 }}>Principal</label>
              <input
                value={testPrincipal}
                onChange={(e) => setTestPrincipal(e.target.value)}
                style={{
                  width: "100%",
                  padding: "8px 12px",
                  background: "#0d0d1a",
                  color: "#e0e0e0",
                  border: "1px solid #333",
                  borderRadius: 4,
                  fontFamily: "monospace",
                }}
              />
            </div>
            <div>
              <label style={{ color: "#888", fontSize: 12 }}>Action</label>
              <input
                value={testAction}
                onChange={(e) => setTestAction(e.target.value)}
                style={{
                  width: "100%",
                  padding: "8px 12px",
                  background: "#0d0d1a",
                  color: "#e0e0e0",
                  border: "1px solid #333",
                  borderRadius: 4,
                  fontFamily: "monospace",
                }}
              />
            </div>
            <div>
              <label style={{ color: "#888", fontSize: 12 }}>Resource</label>
              <input
                value={testResource}
                onChange={(e) => setTestResource(e.target.value)}
                style={{
                  width: "100%",
                  padding: "8px 12px",
                  background: "#0d0d1a",
                  color: "#e0e0e0",
                  border: "1px solid #333",
                  borderRadius: 4,
                  fontFamily: "monospace",
                }}
              />
            </div>
          </div>
          <button
            onClick={handleTest}
            style={{
              padding: "8px 20px",
              background: "#7ecbff22",
              color: "#7ecbff",
              border: "1px solid #7ecbff44",
              borderRadius: 4,
              cursor: "pointer",
            }}
          >
            Evaluate
          </button>
          {testResult && (
            <div
              style={{
                marginTop: 16,
                padding: 16,
                background: "#0d0d1a",
                border: "1px solid #333",
                borderRadius: 6,
              }}
            >
              <p style={{ margin: "0 0 8px 0" }}>
                <strong>Decision:</strong>{" "}
                <span
                  style={{
                    color: testResult.decision === "Allow" ? "#00ff9d" : "#ff4444",
                    fontWeight: 600,
                  }}
                >
                  {testResult.decision}
                </span>
              </p>
              <p style={{ margin: 0, color: "#888", fontSize: 13 }}>
                Matched: {testResult.matched_policies.length > 0 ? testResult.matched_policies.join(", ") : "none"}
              </p>
            </div>
          )}
          {testError && (
            <p style={{ color: "#ff4444", marginTop: 12, fontFamily: "monospace", fontSize: 13 }}>
              {testError}
            </p>
          )}
        </div>
      )}

      {/* Conflicts tab */}
      {tab === "conflicts" && (
        <div>
          <h2 style={{ marginTop: 0 }}>Policy Conflicts</h2>
          {conflicts.length === 0 ? (
            <p style={{ color: "#00ff9d" }}>No conflicts detected.</p>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              {conflicts.map((c, i) => (
                <div
                  key={i}
                  style={{
                    padding: 16,
                    background: "#ff444411",
                    border: "1px solid #ff444433",
                    borderRadius: 6,
                  }}
                >
                  <p style={{ margin: "0 0 4px 0", color: "#ff8888" }}>
                    <strong>{c.policy_a}</strong> vs <strong>{c.policy_b}</strong>
                  </p>
                  <p style={{ margin: 0, color: "#aaa", fontSize: 13 }}>{c.overlap}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
