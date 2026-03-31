import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, expectInvoked } from "../../test/setup";
import TokenEconomy from "../TokenEconomy";

const MOCKS = {
  token_get_all_wallets: [{ agent_id: "a1", balance: 500, available_balance: 480, lifetime_earned: 1000, lifetime_burned: 500, lifetime_transferred: 0, lifetime_received: 0, escrowed: 20, burn_rate: 0.3, autonomy_level: 3, version: 1 }],
  token_get_ledger: [],
  token_get_supply: { total_supply: 10000, total_burned: 3000, total_minted: 13000, is_deflationary: true, active_wallets: 5, active_delegations: 2, total_escrowed: 100, net_flow: -50 },
  token_get_pricing: [],
  token_calculate_reward: { base: 10, quality_multiplier: 1.2, difficulty_multiplier: 1.0, speed_multiplier: 0.9, final_reward: 10.8 },
  token_calculate_burn: { cost_nxc: 0.5 },
};

describe("TokenEconomy", () => {
  it("renders heading after load", async () => {
    mockCommands(MOCKS);
    render(<TokenEconomy />);
    await waitFor(() => expect(screen.getByText(/Token Economy/i)).toBeInTheDocument());
  });

  it("loads all data on mount", async () => {
    mockCommands(MOCKS);
    render(<TokenEconomy />);
    await waitFor(() => expectInvoked("token_get_all_wallets"));
    expectInvoked("token_get_supply");
  });

  it("displays supply stats after load", async () => {
    mockCommands(MOCKS);
    render(<TokenEconomy />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body).toContain("10000");
    });
  });

  it("switches to wallets tab", async () => {
    mockCommands(MOCKS);
    render(<TokenEconomy />);
    await waitFor(() => expect(screen.getByText(/Token Economy/i)).toBeInTheDocument());
    fireEvent.click(screen.getByText("Agent Wallets"));
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body).toContain("a1");
    });
  });
});
