import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ProjectManager from "../ProjectManager";

const MOCKS: Record<string, unknown> = {
  project_list: "[]",
};

describe("ProjectManager", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ProjectManager />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ProjectManager />);
    await waitFor(() => expectInvoked("project_list"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("project_list", "connection refused");
    const { container } = render(<ProjectManager />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
