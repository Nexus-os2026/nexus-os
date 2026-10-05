import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, expectInvoked, mockInvoke } from "../../test/setup";
import GovernedControl, { grantRequest } from "../GovernedControl";

const PENDING_R2 = {
  id: "cmt-0123456789abcdef0123456789abcdef",
  agent: "owner-session",
  run: "run-0123456789abcdef0123456789abcdef",
  kind: "egress",
  class: "R2",
  operation: "egress.request",
  target: "https://example.com:443",
  summary: ["POST https://example.com/x", "Body: 2 bytes (digest 0a1b2c3d4e5f)"],
  state: "prepared",
  requires_approval: true,
  binding: "0a1b2c3d4e5f",
};

const STATUS = {
  status: {
    platform_supported: true,
    emergency_stopped: false,
    policy_generation: 0,
    display: null,
    tools: [["text.sha256", "R0", "/usr/bin/sha256sum"]],
    connector_operations: [],
  },
  commitments: [PENDING_R2],
  grants: [{ id: "grant-1", kind: "egress", lines: ["Network requests to https://example.com:443"], live: true }],
  runs: [],
};

const MOCKS = {
  p3_status: STATUS,
  p3_evidence: [],
  p3_submit: { understood: true, commitment: PENDING_R2 },
  p3_approve: { text: "done", bytes: null, meta: [["status", "200"]] },
  p3_emergency_stop: 1,
};

/** Every argument object sent to `command`. */
function argsOf(command: string): Record<string, unknown>[] {
  return mockInvoke.mock.calls
    .filter((call: unknown[]) => call[0] === command)
    .map((call: unknown[]) => (call[1] ?? {}) as Record<string, unknown>);
}

describe("GovernedControl (Phase Three)", () => {
  it("renders the governed control and loads its status", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    await waitFor(() => expect(screen.getByText(/Governed Real-World Control/i)).toBeInTheDocument());
    await waitFor(() => expectInvoked("p3_status"));
    expect(document.body.textContent).toContain("governance");
  });

  it("submits a command as data only", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    fireEvent.change(screen.getByLabelText("command"), { target: { value: "fetch https://example.com/" } });
    fireEvent.click(screen.getByText("Submit"));
    await waitFor(() => expectInvoked("p3_submit"));
    expect(argsOf("p3_submit")).toEqual([{ envelope: { text: "fetch https://example.com/" } }]);
  });

  it("approves a pending R2 commitment by its id, the native dialog decides", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    await waitFor(() => expect(screen.getByText("Approve…")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Approve…"));
    await waitFor(() => expectInvoked("p3_approve"));
    expect(argsOf("p3_approve")).toEqual([{ commitment: PENDING_R2.id }]);
    for (const sent of mockInvoke.mock.calls.map((call: unknown[]) => JSON.stringify(call[1] ?? {}))) {
      expect(sent).not.toMatch(/approv|confirm|binding|"grant"/i);
    }
  });

  it("stops everything with the emergency stop", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    await waitFor(() => expect(screen.getByText("Emergency stop")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Emergency stop"));
    await waitFor(() => expectInvoked("p3_emergency_stop"));
  });

  it("builds grant requests from the form, never approvals", () => {
    expect(grantRequest("egress", "https://example.com", "")).toEqual({
      kind: "egress",
      origin: "https://example.com",
      methods: ["GET"],
    });
    expect(grantRequest("input", "20", "r1")).toEqual({ kind: "input", max_steps: 20, session_r1: true });
    expect(grantRequest("connector", "gmail me@example.com", "gmail.messages.list")).toEqual({
      kind: "connector",
      connector: "gmail",
      account: "me@example.com",
      operations: ["gmail.messages.list"],
    });
    expect(grantRequest("tool", "", "")).toBeNull();
    expect(grantRequest("connector", "gmail", "")).toBeNull();
  });
});
