import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import FileManager from "../FileManager";

const MOCKS: Record<string, unknown> = {
  file_manager_list: "[]",
  file_manager_home: "/home/nexus",
  file_manager_read: "",
  file_manager_create_dir: "ok",
};

describe("FileManager", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<FileManager />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<FileManager />);
    await waitFor(() => expectInvoked("file_manager_home"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("file_manager_home", "connection refused");
    const { container } = render(<FileManager />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
