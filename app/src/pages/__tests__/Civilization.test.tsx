import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Civilization from "../Civilization";

const MOCKS: Record<string, unknown> = {
  civ_get_parliament_status: "{}",
  civ_get_economy_status: "{}",
  civ_get_roles: [],
  civ_get_governance_log: [],
  economy_get_wallet: "{}",
  economy_get_history: [],
  economy_get_stats: "{}",
  payment_list_plans: [],
  payment_get_revenue_stats: "{}",
  list_agents: [],
};

describe("Civilization", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Civilization />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Civilization />);
    await waitFor(() => expectInvoked("civ_get_parliament_status"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("civ_get_parliament_status", "connection refused");
    const { container } = render(<Civilization />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
