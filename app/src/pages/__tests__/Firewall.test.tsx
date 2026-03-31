import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, expectInvoked } from "../../test/setup";
import Firewall from "../Firewall";

describe("Firewall", () => {
  it("mounts without throwing", () => {
    expect(() => render(<Firewall />)).not.toThrow();
  });

  it("loads firewall status on mount", async () => {
    mockCommands({
      get_firewall_status: { status: "active", mode: "enforce", injection_patterns: 20, pii_patterns: 8, output_rules: 3, blocked_last_24h: 5 },
      get_firewall_patterns: { injection_patterns: 20, pii_patterns: 8, output_rules: 3 },
    });
    render(<Firewall />);
    await waitFor(() => expectInvoked("get_firewall_status"));
  });

  it("mounts gracefully without backend", () => {
    expect(() => render(<Firewall />)).not.toThrow();
  });
});
