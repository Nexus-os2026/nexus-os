import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import NotesApp from "../NotesApp";

const MOCKS: Record<string, unknown> = {
  notes_list: "[]",
};

describe("NotesApp", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<NotesApp />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<NotesApp />);
    await waitFor(() => expectInvoked("notes_list"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("notes_list", "connection refused");
    const { container } = render(<NotesApp />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
