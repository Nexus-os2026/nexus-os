import { useCallback, useEffect, useState } from "react";
import {
  p3Approve,
  p3DisplayStart,
  p3DisplayStop,
  p3EmergencyStop,
  p3Evidence,
  p3ImportAttachment,
  p3Deny,
  p3RequestGrant,
  p3Resume,
  p3RevokeGrant,
  p3Status,
  p3Submit,
  type P3Attachment,
  type P3Commitment,
  type P3Grant,
  type P3GrantRequest,
  type P3Output,
  type P3Status,
} from "../api/backend";
import { alpha, commandMutedStyle, commandPageStyle, normalizeArray } from "./commandCenterUi";

const ACCENT = "#06b6d4";
const GREEN = "#22c55e";
const RED = "#ef4444";
const YELLOW = "#eab308";

const CLASS_COLORS: Record<string, string> = { R0: GREEN, R1: YELLOW, R2: RED };

const cardStyle: React.CSSProperties = {
  background: alpha("#1e1e2e", 0.7),
  borderRadius: 10,
  padding: 16,
  border: "1px solid " + alpha("#ffffff", 0.08),
  marginBottom: 16,
};

function button(color: string): React.CSSProperties {
  return {
    borderRadius: 6,
    padding: "6px 12px",
    cursor: "pointer",
    fontSize: 13,
    marginRight: 8,
    background: alpha(color, 0.15),
    color,
    border: "1px solid " + alpha(color, 0.4),
  };
}

const inputStyle: React.CSSProperties = {
  background: alpha("#000000", 0.3),
  color: "#e5e7eb",
  border: "1px solid " + alpha("#ffffff", 0.15),
  borderRadius: 6,
  padding: "6px 10px",
  fontSize: 13,
  marginRight: 8,
};

const UNCONSUMED = new Set(["prepared", "authorized"]);

/** One grant request the owner may ask for; the backend shows the native dialog. */
export function grantRequest(kind: string, value: string, extra: string): P3GrantRequest | null {
  const items = value
    .split(/[\s,]+/)
    .map((item) => item.trim())
    .filter(Boolean);
  const details = extra.split(/[\s,]+/).filter(Boolean);
  switch (kind) {
    case "egress":
      return items[0] ? { kind: "egress", origin: items[0], methods: details.length ? details : ["GET"] } : null;
    case "tool":
      return items[0] ? { kind: "tool", tool: items[0] } : null;
    case "browser":
      return items.length ? { kind: "browser", origins: items } : null;
    case "perception":
      return { kind: "perception" };
    case "input": {
      const steps = Number.parseInt(value, 10);
      return Number.isFinite(steps) && steps > 0 ? { kind: "input", max_steps: steps, session_r1: extra.trim() === "r1" } : null;
    }
    case "connector": {
      const [connector, account] = items;
      return connector && account && details.length ? { kind: "connector", connector, account, operations: details } : null;
    }
    default:
      return null;
  }
}

function CommitmentCard({
  commitment,
  onRun,
  onDeny,
}: {
  commitment: P3Commitment;
  onRun?: (c: P3Commitment) => void;
  onDeny?: (c: P3Commitment) => void;
}) {
  const color = CLASS_COLORS[commitment.class] ?? ACCENT;
  const waiting = UNCONSUMED.has(commitment.state);
  return (
    <div style={{ ...cardStyle, marginBottom: 10, padding: 12 }} data-testid="p3-commitment">
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
        <span
          style={{ color, fontWeight: 700, fontSize: 12, border: "1px solid " + alpha(color, 0.5), borderRadius: 4, padding: "1px 6px" }}
        >
          {commitment.class}
        </span>
        <span style={{ fontWeight: 600 }}>{commitment.operation}</span>
        <span style={{ ...commandMutedStyle, fontSize: 12 }}>{commitment.target}</span>
        <span style={{ marginLeft: "auto", fontSize: 12, color: waiting ? YELLOW : "#9ca3af" }}>{commitment.state}</span>
      </div>
      {commitment.summary.map((line, index) => (
        <div key={index} style={{ fontSize: 12, color: "#d1d5db", fontFamily: "monospace" }}>
          {line}
        </div>
      ))}
      <div style={{ ...commandMutedStyle, fontSize: 11, marginTop: 6 }}>
        {commitment.agent} · {commitment.id} [{commitment.binding}]
      </div>
      {waiting && onRun && onDeny && (
        <div style={{ marginTop: 8 }}>
          <button style={button(commitment.requires_approval ? RED : GREEN)} onClick={() => onRun(commitment)}>
            {commitment.requires_approval ? "Approve…" : "Run"}
          </button>
          <button style={button("#9ca3af")} onClick={() => onDeny(commitment)}>
            Deny
          </button>
        </div>
      )}
    </div>
  );
}

export default function GovernedControl() {
  const [status, setStatus] = useState<P3Status | null>(null);
  const [evidence, setEvidence] = useState<Record<string, unknown>[]>([]);
  const [command, setCommand] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [output, setOutput] = useState<P3Output | null>(null);
  const [attachment, setAttachment] = useState<P3Attachment | null>(null);
  const [grantKind, setGrantKind] = useState("egress");
  const [grantValue, setGrantValue] = useState("");
  const [grantExtra, setGrantExtra] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [next, records] = await Promise.all([p3Status(), p3Evidence().catch(() => [])]);
      setStatus(next);
      setEvidence(normalizeArray<Record<string, unknown>>(records));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const act = useCallback(
    async (work: () => Promise<unknown>) => {
      setBusy(true);
      setError(null);
      try {
        await work();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
        await refresh();
      }
    },
    [refresh],
  );

  const submit = () =>
    act(async () => {
      setNotice(null);
      setOutput(null);
      const answer = await p3Submit(command);
      if (!answer.understood) {
        setNotice(answer.reason ?? "not understood");
      } else {
        setNotice("committed: review it below, then run or approve it");
        setCommand("");
      }
    });

  const run = (commitment: P3Commitment) => act(async () => setOutput(await p3Approve(commitment.id)));
  const deny = (commitment: P3Commitment) => act(() => p3Deny(commitment.id));

  const commitments = normalizeArray<P3Commitment>(status?.commitments);
  const pending = commitments.filter((c) => UNCONSUMED.has(c.state));
  const done = commitments.filter((c) => !UNCONSUMED.has(c.state));
  const display = status?.status.display;

  return (
    <div style={commandPageStyle}>
      <div style={{ display: "flex", alignItems: "center", marginBottom: 16 }}>
        <h1 style={{ margin: 0, fontSize: 22, color: ACCENT }}>Governed Real-World Control</h1>
        <span style={{ ...commandMutedStyle, marginLeft: 12, fontSize: 12 }}>
          policy generation {status?.status.policy_generation ?? "…"}
          {status && !status.status.platform_supported ? " · Linux only" : ""}
        </span>
        <div style={{ marginLeft: "auto" }}>
          {status?.status.emergency_stopped ? (
            <button style={button(YELLOW)} disabled={busy} onClick={() => act(p3Resume)}>
              Resume…
            </button>
          ) : (
            <button style={button(RED)} disabled={busy} onClick={() => act(p3EmergencyStop)}>
              Emergency stop
            </button>
          )}
        </div>
      </div>

      <p style={{ ...commandMutedStyle, fontSize: 13 }}>
        Every real-world effect is a commitment under governance: prepared from your command, covered by a grant you
        gave natively, approved natively when it is sensitive (R2), run once, and recorded in the audit trail.
        Commands are data: fetch &lt;url&gt;, browse &lt;url&gt; [read &lt;selector&gt;], observe, hash &lt;text&gt;,
        say &lt;text&gt;, connector &lt;operation&gt; &lt;json&gt;.
      </p>

      {error && <div style={{ ...cardStyle, borderColor: alpha(RED, 0.5), color: RED }}>{error}</div>}

      <div style={cardStyle}>
        <input
          style={{ ...inputStyle, width: "60%" }}
          placeholder="fetch https://example.com/"
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          aria-label="command"
        />
        <button style={button(ACCENT)} disabled={busy || !command.trim()} onClick={submit}>
          Submit
        </button>
        <button style={button("#9ca3af")} disabled={busy} onClick={() => act(async () => setAttachment(await p3ImportAttachment()))}>
          Attach file…
        </button>
        {notice && <div style={{ marginTop: 8, fontSize: 13 }}>{notice}</div>}
        {attachment && (
          <div style={{ ...commandMutedStyle, marginTop: 8, fontSize: 12 }}>
            attached {attachment.name} ({attachment.bytes} bytes, sha256 {attachment.sha256.slice(0, 12)}): use
            &quot;hash attachment {attachment.id}&quot;
          </div>
        )}
      </div>

      <h2 style={{ fontSize: 16 }}>Waiting for you ({pending.length})</h2>
      {pending.length === 0 && <div style={commandMutedStyle}>Nothing is waiting.</div>}
      {pending.map((c) => (
        <CommitmentCard key={c.id} commitment={c} onRun={run} onDeny={deny} />
      ))}

      {output && (
        <div style={cardStyle} data-testid="p3-output">
          <div style={{ fontWeight: 600, marginBottom: 6 }}>Result</div>
          {output.text !== null && (
            <pre style={{ whiteSpace: "pre-wrap", fontSize: 12, maxHeight: 240, overflow: "auto" }}>{output.text}</pre>
          )}
          {output.bytes !== null && <div style={commandMutedStyle}>{output.bytes} bytes of data</div>}
          {normalizeArray<[string, string]>(output.meta).map(([k, v]) => (
            <div key={k} style={{ fontSize: 12, fontFamily: "monospace" }}>
              {k}: {v}
            </div>
          ))}
        </div>
      )}

      <div style={cardStyle}>
        <div style={{ fontWeight: 600, marginBottom: 8 }}>Grants</div>
        <select style={inputStyle} value={grantKind} onChange={(e) => setGrantKind(e.target.value)} aria-label="grant kind">
          <option value="egress">egress origin</option>
          <option value="tool">tool</option>
          <option value="browser">browser origins</option>
          <option value="connector">connector</option>
          <option value="perception">observe the agent display</option>
          <option value="input">input on the agent display</option>
        </select>
        <input
          style={inputStyle}
          placeholder="origin, tool, origins, or connector account"
          value={grantValue}
          onChange={(e) => setGrantValue(e.target.value)}
          aria-label="grant value"
        />
        <input
          style={inputStyle}
          placeholder="methods, operations, or r1"
          value={grantExtra}
          onChange={(e) => setGrantExtra(e.target.value)}
          aria-label="grant detail"
        />
        <button
          style={button(ACCENT)}
          disabled={busy}
          onClick={() => {
            const request = grantRequest(grantKind, grantValue, grantExtra);
            if (!request) {
              setError("the grant request is incomplete");
              return;
            }
            void act(() => p3RequestGrant(request, 3600));
          }}
        >
          Request grant…
        </button>
        {normalizeArray<P3Grant>(status?.grants).map((grant) => (
          <div key={grant.id} style={{ marginTop: 8, fontSize: 12, opacity: grant.live ? 1 : 0.5 }}>
            <span style={{ fontWeight: 600 }}>{grant.kind}</span> {grant.lines.join(" · ")}
            {grant.live && (
              <button style={{ ...button("#9ca3af"), marginLeft: 8 }} onClick={() => act(() => p3RevokeGrant(grant.id))}>
                Revoke
              </button>
            )}
          </div>
        ))}
      </div>

      <div style={cardStyle}>
        <div style={{ fontWeight: 600, marginBottom: 8 }}>Agent display</div>
        <div style={{ ...commandMutedStyle, fontSize: 12, marginBottom: 8 }}>
          {display
            ? `running: display ${display[1]}, ${display[2]}x${display[3]} (generation ${display[0]})`
            : "stopped: perception and input act only on this isolated display, never on your desktop"}
        </div>
        <button style={button(GREEN)} disabled={busy} onClick={() => act(p3DisplayStart)}>
          Start
        </button>
        <button style={button("#9ca3af")} disabled={busy} onClick={() => act(p3DisplayStop)}>
          Stop
        </button>
      </div>

      <h2 style={{ fontSize: 16 }}>Recent</h2>
      {done.slice(0, 20).map((c) => (
        <CommitmentCard key={c.id} commitment={c} />
      ))}

      <div style={cardStyle}>
        <div style={{ fontWeight: 600, marginBottom: 8 }}>Evidence (audit trail)</div>
        {evidence.slice(0, 50).map((record, index) => (
          <div key={index} style={{ fontSize: 11, fontFamily: "monospace", color: "#d1d5db" }}>
            {String(record.phase ?? "")} {String(record.operation ?? "")} {String(record.class ?? "")}{" "}
            {String(record.outcome ?? "")} {String(record.target ?? "")}
          </div>
        ))}
      </div>
    </div>
  );
}
