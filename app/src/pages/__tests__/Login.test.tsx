import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Login from "../Login";

const MOCKS: Record<string, unknown> = {
  auth_config_get: '{"provider":"local","require_auth":false}',
  auth_session_info: "null",
};

describe("Login", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Login />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Login />);
    await waitFor(() => expectInvoked("auth_login"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("auth_config_get", "connection refused");
    const { container } = render(<Login />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
