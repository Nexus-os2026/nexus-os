import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { builderGeneratePlan } from "../../api/backend";
import { mockCommandError, mockCommands, mockInvoke } from "../../test/setup";
import NexusBuilder from "../NexusBuilder";

const response = {
  project_id: "98a66a0e-14d4-4f29-a4da-12fb9b5633ac",
  model: "Test planner", cost_usd: 0, elapsed_seconds: 0,
  plan: {
    product_brief: { project_name: "Test site", project_type: "website", target_audience: "People", sections: ["Home"], design_direction: "Simple", tone: "Clear", template_suggestion: "landing", estimated_cost: "0", estimated_time: "1s" },
    acceptance_criteria: { must_have: [], must_not_have: [], constraints: [] },
  },
};

describe("Builder fresh planning authority contract", () => {
  it("sends only prompt even if an old JavaScript caller supplies extra identity", async () => {
    mockCommands({ builder_generate_plan: response });
    const legacyCaller = builderGeneratePlan as (...args: unknown[]) => Promise<unknown>;
    await expect(legacyCaller("site", "../../caller-selected")).resolves.toEqual(response);
    expect(mockInvoke).toHaveBeenCalledWith("builder_generate_plan", { prompt: "site" });
  });

  it.each([
    ["the governed response", response],
    // P0-002C5A: even a stale response that still carries a location must not
    // turn it into build authority.
    ["a stale response carrying a location", { ...response, project_dir: "/trusted/builds/98a66a0e-14d4-4f29-a4da-12fb9b5633ac" }],
  ])("never hands %s back to the backend as build authority", async (_label, planResponse) => {
    const closed = "conduct_build_streaming: the legacy Builder project surface is retired in Phase Zero; use the governed Builder project flow";
    mockCommandError("conduct_build_streaming", closed, {
      builder_list_projects: [], builder_get_budget: {},
      builder_get_model_config: { full_build: { provider: "ollama", model_id: "test", display_name: "Test" } },
      builder_generate_plan: planResponse,
    });
    render(<NexusBuilder />);
    const prompt = "Build a simple website";
    fireEvent.change(screen.getByPlaceholderText("Describe the website you want to build..."), { target: { value: prompt } });
    fireEvent.click(screen.getByRole("button", { name: /Build It/ }));
    await screen.findByRole("button", { name: "Approve & Build" });
    const planning = mockInvoke.mock.calls.filter(([name]) => name === "builder_generate_plan");
    expect(planning).toEqual([["builder_generate_plan", { prompt }]]);
    fireEvent.click(screen.getByRole("button", { name: "Approve & Build" }));
    await waitFor(() => {
      const build = mockInvoke.mock.calls.find(([name]) => name === "conduct_build_streaming");
      expect(build).toBeDefined();
      expect(build?.[1].outputDir).toBeUndefined();
      expect(build?.[1].output_dir).toBeUndefined();
      expect(JSON.stringify(build?.[1])).not.toContain("/trusted/builds");
    });
    expect(await screen.findByText(new RegExp("legacy Builder project surface is retired"))).toBeTruthy();
  });
});
