import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError } from "../../test/setup";
import SwarmAudit, { filterEntries } from "../SwarmAudit";
import type { AuditEntry, AuditEventKind } from "../../lib/swarm/types";

const RUN_ID = "11111111-2222-3333-4444-555555555555";

function entry(overrides: Partial<AuditEntry> & Pick<AuditEntry, "seq" | "event_kind">): AuditEntry {
  const nowSecs = Math.floor(Date.now() / 1000);
  return {
    seq: overrides.seq,
    event_kind: overrides.event_kind,
    ticket_nonce: overrides.ticket_nonce ?? "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    node_id: overrides.node_id ?? null,
    timestamp: overrides.timestamp ?? { secs_since_epoch: nowSecs, nanos_since_epoch: 0 },
    payload_summary: overrides.payload_summary ?? "summary",
    // Bug AL: hash-chain fields. Tests use placeholder hex; the
    // production backend computes real SHA-256 in record_swarm_audit.
    previous_hash:
      overrides.previous_hash ??
      "0000000000000000000000000000000000000000000000000000000000000000",
    current_hash:
      overrides.current_hash ??
      "1111111111111111111111111111111111111111111111111111111111111111",
  };
}

const SAMPLE: readonly AuditEntry[] = [
  entry({ seq: 1, event_kind: "node_started", node_id: "node-a", payload_summary: "started a" }),
  entry({ seq: 2, event_kind: "node_failed", node_id: "node-b", payload_summary: "failed b" }),
  entry({ seq: 3, event_kind: "budget_update", node_id: null, payload_summary: "tokens=900" }),
  entry({
    seq: 4,
    event_kind: "oracle_runtime_denial",
    node_id: "node-c",
    payload_summary: "denied c",
  }),
];

describe("SwarmAudit page", () => {
  it("shows empty state when there are no entries to fetch", async () => {
    mockCommands({ swarm_audit_tail: [] });
    render(<SwarmAudit />);
    fireEvent.change(screen.getByTestId("swarm-audit-run-input"), {
      target: { value: RUN_ID },
    });
    fireEvent.click(screen.getByTestId("swarm-audit-refresh"));
    await waitFor(() =>
      expect(screen.getByTestId("swarm-audit-empty")).toHaveTextContent(
        /No audit entries for this run/i,
      ),
    );
  });

  it("renders audit rows after fetch", async () => {
    mockCommands({ swarm_audit_tail: SAMPLE });
    render(<SwarmAudit />);
    fireEvent.change(screen.getByTestId("swarm-audit-run-input"), {
      target: { value: RUN_ID },
    });
    fireEvent.click(screen.getByTestId("swarm-audit-refresh"));
    await waitFor(() =>
      expect(screen.getByTestId("swarm-audit-row-1")).toBeInTheDocument(),
    );
    expect(screen.getByTestId("swarm-audit-row-2")).toBeInTheDocument();
    expect(screen.getByTestId("swarm-audit-row-3")).toBeInTheDocument();
    expect(screen.getByTestId("swarm-audit-row-4")).toBeInTheDocument();
    expect(screen.getByTestId("swarm-audit-count-total")).toHaveTextContent("4");
    expect(screen.getByTestId("swarm-audit-count-filtered")).toHaveTextContent("4");
  });

  it("filters by event_kind chip toggle", async () => {
    mockCommands({ swarm_audit_tail: SAMPLE });
    render(<SwarmAudit />);
    fireEvent.change(screen.getByTestId("swarm-audit-run-input"), {
      target: { value: RUN_ID },
    });
    fireEvent.click(screen.getByTestId("swarm-audit-refresh"));
    await waitFor(() =>
      expect(screen.getByTestId("swarm-audit-row-1")).toBeInTheDocument(),
    );
    // Activate the node_failed chip — only seq=2 should remain.
    fireEvent.click(screen.getByTestId("swarm-audit-chip-node_failed"));
    await waitFor(() => {
      expect(screen.queryByTestId("swarm-audit-row-1")).not.toBeInTheDocument();
      expect(screen.getByTestId("swarm-audit-row-2")).toBeInTheDocument();
      expect(screen.queryByTestId("swarm-audit-row-3")).not.toBeInTheDocument();
      expect(screen.queryByTestId("swarm-audit-row-4")).not.toBeInTheDocument();
    });
    expect(screen.getByTestId("swarm-audit-count-filtered")).toHaveTextContent("1");
  });

  it("filters by node_id substring", async () => {
    mockCommands({ swarm_audit_tail: SAMPLE });
    render(<SwarmAudit />);
    fireEvent.change(screen.getByTestId("swarm-audit-run-input"), {
      target: { value: RUN_ID },
    });
    fireEvent.click(screen.getByTestId("swarm-audit-refresh"));
    await waitFor(() =>
      expect(screen.getByTestId("swarm-audit-row-1")).toBeInTheDocument(),
    );
    fireEvent.change(screen.getByTestId("swarm-audit-node-filter"), {
      target: { value: "node-a" },
    });
    await waitFor(() => {
      expect(screen.getByTestId("swarm-audit-row-1")).toBeInTheDocument();
      expect(screen.queryByTestId("swarm-audit-row-2")).not.toBeInTheDocument();
      // budget_update has node_id=null → excluded by any non-empty substring.
      expect(screen.queryByTestId("swarm-audit-row-3")).not.toBeInTheDocument();
      expect(screen.queryByTestId("swarm-audit-row-4")).not.toBeInTheDocument();
    });
  });

  it("filters by time-range window", async () => {
    // Two entries: one current, one 2 hours old.
    const nowSecs = Math.floor(Date.now() / 1000);
    const sample: readonly AuditEntry[] = [
      entry({
        seq: 10,
        event_kind: "node_started",
        node_id: "node-recent",
        timestamp: { secs_since_epoch: nowSecs, nanos_since_epoch: 0 },
      }),
      entry({
        seq: 11,
        event_kind: "node_started",
        node_id: "node-stale",
        timestamp: { secs_since_epoch: nowSecs - 7200, nanos_since_epoch: 0 },
      }),
    ];
    mockCommands({ swarm_audit_tail: sample });
    render(<SwarmAudit />);
    fireEvent.change(screen.getByTestId("swarm-audit-run-input"), {
      target: { value: RUN_ID },
    });
    fireEvent.click(screen.getByTestId("swarm-audit-refresh"));
    await waitFor(() =>
      expect(screen.getByTestId("swarm-audit-row-10")).toBeInTheDocument(),
    );
    fireEvent.change(screen.getByTestId("swarm-audit-time-range"), {
      target: { value: "1h" },
    });
    await waitFor(() => {
      expect(screen.getByTestId("swarm-audit-row-10")).toBeInTheDocument();
      // 2-hour-old entry must be excluded by the 1h window.
      expect(screen.queryByTestId("swarm-audit-row-11")).not.toBeInTheDocument();
    });
  });

  it("surfaces errors from the backend", async () => {
    mockCommandError("swarm_audit_tail", "run not found");
    render(<SwarmAudit />);
    fireEvent.change(screen.getByTestId("swarm-audit-run-input"), {
      target: { value: RUN_ID },
    });
    fireEvent.click(screen.getByTestId("swarm-audit-refresh"));
    await waitFor(() =>
      expect(screen.getByTestId("swarm-audit-error")).toHaveTextContent(/run not found/),
    );
  });

  it("expands a row to show JSON on click", async () => {
    mockCommands({ swarm_audit_tail: [SAMPLE[0]] });
    render(<SwarmAudit />);
    fireEvent.change(screen.getByTestId("swarm-audit-run-input"), {
      target: { value: RUN_ID },
    });
    fireEvent.click(screen.getByTestId("swarm-audit-refresh"));
    const row = await screen.findByTestId("swarm-audit-row-1");
    expect(within(row).queryByTestId("swarm-audit-json-1")).not.toBeInTheDocument();
    fireEvent.click(row);
    expect(within(row).getByTestId("swarm-audit-json-1")).toBeInTheDocument();
  });
});

describe("filterEntries (pure)", () => {
  const baseNow = 1_700_000_000_000;
  const fixed = (offsetMs: number): AuditEntry =>
    entry({
      seq: offsetMs,
      event_kind: "node_started",
      node_id: `node-${offsetMs}`,
      timestamp: {
        secs_since_epoch: Math.floor((baseNow - offsetMs) / 1000),
        nanos_since_epoch: 0,
      },
    });

  it("returns input unchanged when no filters active", () => {
    const data = [fixed(1000), fixed(2000)];
    const out = filterEntries(
      data,
      { kinds: new Set(), nodeSubstring: "", timeRange: "all" },
      baseNow,
    );
    expect(out).toHaveLength(2);
  });

  it("kinds set narrows to allowed kinds", () => {
    const data: readonly AuditEntry[] = [
      entry({ seq: 1, event_kind: "node_started", node_id: "n1" }),
      entry({ seq: 2, event_kind: "budget_update" }),
    ];
    const kinds = new Set<AuditEventKind>(["budget_update"]);
    const out = filterEntries(
      data,
      { kinds, nodeSubstring: "", timeRange: "all" },
      baseNow,
    );
    expect(out.map((e) => e.seq)).toEqual([2]);
  });

  it("nodeSubstring excludes entries whose node_id is null", () => {
    const data: readonly AuditEntry[] = [
      entry({ seq: 1, event_kind: "node_started", node_id: "alpha" }),
      entry({ seq: 2, event_kind: "budget_update", node_id: null }),
    ];
    const out = filterEntries(
      data,
      { kinds: new Set(), nodeSubstring: "a", timeRange: "all" },
      baseNow,
    );
    expect(out.map((e) => e.seq)).toEqual([1]);
  });

  it("timeRange excludes entries older than the window", () => {
    const data = [fixed(30_000), fixed(120_000)]; // 30s old, 2m old
    const out = filterEntries(
      data,
      { kinds: new Set(), nodeSubstring: "", timeRange: "1m" },
      baseNow,
    );
    // 1m window keeps the 30s-old entry, drops the 2m-old one.
    expect(out.map((e) => e.seq)).toEqual([30_000]);
  });
});
