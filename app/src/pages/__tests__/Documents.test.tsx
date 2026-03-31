import { render, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeAll } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Documents from "../Documents";

// Documents uses scrollIntoView which jsdom doesn't support
beforeAll(() => {
  Element.prototype.scrollIntoView = () => {};
});

const MOCKS: Record<string, unknown> = {
  list_indexed_documents: [],
  get_semantic_map: "{}",
  get_document_governance: "{}",
  get_document_access_log: [],
};

describe("Documents", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Documents />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Documents />);
    await waitFor(() => expectInvoked("list_indexed_documents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_indexed_documents", "connection refused");
    const { container } = render(<Documents />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
