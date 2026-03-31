import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, expectInvoked } from "../../test/setup";
import CodeEditor from "../CodeEditor";

const MOCKS = {
  file_manager_home: "/home/test",
  file_manager_list: [],
  get_git_repo_status: { detected: false, root: null, branch: null, changes: [], commits: [] },
};

describe("CodeEditor", () => {
  it("mounts without throwing", () => {
    mockCommands(MOCKS);
    expect(() => render(<CodeEditor />)).not.toThrow();
  });

  it("calls file_manager_home on mount", async () => {
    mockCommands(MOCKS);
    render(<CodeEditor />);
    await waitFor(() => expectInvoked("file_manager_home"));
  });

  it("mounts gracefully without backend", () => {
    // No mocks — verifies the component doesn't crash when invoke fails
    expect(() => render(<CodeEditor />)).not.toThrow();
  });
});
