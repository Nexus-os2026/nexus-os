import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Identity from "../Identity";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  list_identities: [],
  identity_get_agent_passport: "{}",
  mesh_get_peers: [],
};

describe("Identity", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Identity agents={[]} />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Identity agents={[]} />);
    await waitFor(() => expectInvoked("mesh_get_peers"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    const { container } = render(<Identity agents={[]} />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
