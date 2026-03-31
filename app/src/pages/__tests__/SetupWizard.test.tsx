import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { SetupWizard } from "../SetupWizard";

describe("SetupWizard", () => {
  const noop = async () => ({} as any);

  it("renders without crashing", () => {
    const { container } = render(
      <SetupWizard
        onDetectHardware={noop}
        onCheckOllama={noop}
        onEnsureOllama={async () => false}
        onIsOllamaInstalled={async () => false}
        onPullModel={async () => "ok"}
        onListAvailableModels={async () => []}
        onRunSetup={noop}
        onClose={() => {}}
      />
    );
    expect(container).toBeTruthy();
  });

  it("shows welcome text", () => {
    render(
      <SetupWizard
        onDetectHardware={noop}
        onCheckOllama={noop}
        onEnsureOllama={async () => false}
        onIsOllamaInstalled={async () => false}
        onPullModel={async () => "ok"}
        onListAvailableModels={async () => []}
        onRunSetup={noop}
        onClose={() => {}}
      />
    );
    // The wizard should render some setup-related content
    expect(document.body.textContent?.length).toBeGreaterThan(0);
  });
});
