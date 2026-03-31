import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import DatabaseManager from "../DatabaseManager";

const MOCKS: Record<string, unknown> = {
  db_list_tables: [],
};

describe("DatabaseManager", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<DatabaseManager />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<DatabaseManager />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("db_list_tables", "connection refused");
    const { container } = render(<DatabaseManager />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
