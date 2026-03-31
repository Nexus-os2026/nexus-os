import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import KnowledgeGraph from "../KnowledgeGraph";

const MOCKS: Record<string, unknown> = {
  cogfs_get_entities: [],
  cogfs_get_graph: "{}",
  neural_bridge_status: "{}",
  cogfs_get_context: "{}",
  cogfs_search: [],
};

describe("KnowledgeGraph", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<KnowledgeGraph />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<KnowledgeGraph />);
    await waitFor(() => expectInvoked("neural_bridge_status"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("cogfs_get_entities", "connection refused");
    const { container } = render(<KnowledgeGraph />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
