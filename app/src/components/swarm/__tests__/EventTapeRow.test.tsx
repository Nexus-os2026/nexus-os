import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { EventTapeRow } from "../EventTapeRow";
import type { SwarmEvent } from "../../../lib/swarm/types";

function row(event: SwarmEvent, seq = 1): JSX.Element {
  return <EventTapeRow event={event} seq={seq} timestamp="12:34:56" />;
}

describe("EventTapeRow", () => {
  it("tags the row with its category", () => {
    const plan: SwarmEvent = {
      event: "plan_approved",
      run_id: "11111111-2222-3333-4444-555555555555",
    };
    render(row(plan));
    const el = screen.getByTestId("event-tape-row-1");
    expect(el.getAttribute("data-category")).toBe("plan");
  });

  it("marks failure variants with the failure flag and red accent border", () => {
    const fail: SwarmEvent = {
      event: "node_failed",
      ref: { run_id: "r", node_id: "n0" },
      reason: "boom",
      ticket_nonce: "00000000-0000-0000-0000-000000000000",
    };
    render(row(fail));
    const el = screen.getByTestId("event-tape-row-1");
    expect(el.getAttribute("data-failure")).toBe("true");
    expect(el.getAttribute("style") ?? "").toContain("rgb(248, 113, 113)");
  });

  it("expands to show raw JSON on click and collapses on second click", () => {
    const ev: SwarmEvent = {
      event: "swarm_completed",
      run_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    };
    render(row(ev));
    const el = screen.getByTestId("event-tape-row-1");
    expect(screen.queryByTestId("event-tape-json-1")).not.toBeInTheDocument();
    fireEvent.click(el);
    expect(screen.getByTestId("event-tape-json-1")).toBeInTheDocument();
    fireEvent.click(el);
    expect(screen.queryByTestId("event-tape-json-1")).not.toBeInTheDocument();
  });
});
