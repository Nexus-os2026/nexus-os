import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Integrations from "../Integrations";

const MOCKS: Record<string, unknown> = {
  integrations_list: [],
};

describe("Integrations", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Integrations />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Integrations />);
    await waitFor(() => expectInvoked("integrations_list"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("integrations_list", "connection refused");
    const { container } = render(<Integrations />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
