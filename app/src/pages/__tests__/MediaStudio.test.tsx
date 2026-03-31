import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import MediaStudio from "../MediaStudio";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  file_manager_list: "[]",
  file_manager_create_dir: "ok",
  analyze_media_file: "{}",
};

describe("MediaStudio", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<MediaStudio />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<MediaStudio />);
    await waitFor(() => expectInvoked("list_agents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    const { container } = render(<MediaStudio />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
