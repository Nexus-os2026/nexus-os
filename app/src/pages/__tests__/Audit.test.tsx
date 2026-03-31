import { render } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { Audit } from "../Audit";
import type { AuditEventRow } from "../../types";

const EVENTS: AuditEventRow[] = [
  { event_id: "ev1", agent_id: "a1", timestamp: Date.now(), event_type: "ToolCall", payload: { tool: "web.search" }, hash: "abc123", previous_hash: "000000" },
];

describe("Audit", () => {
  it("mounts without throwing", () => {
    expect(() => render(<Audit events={EVENTS} onRefresh={vi.fn()} />)).not.toThrow();
  });

  it("renders event data from props", () => {
    render(<Audit events={EVENTS} onRefresh={vi.fn()} />);
    const body = document.body.textContent || "";
    expect(body).toContain("ToolCall");
  });

  it("renders with empty events", () => {
    expect(() => render(<Audit events={[]} onRefresh={vi.fn()} />)).not.toThrow();
  });

  it("accepts onRefresh callback", () => {
    const refresh = vi.fn();
    render(<Audit events={EVENTS} onRefresh={refresh} />);
    expect(document.body.textContent?.length).toBeGreaterThan(0);
  });
});
