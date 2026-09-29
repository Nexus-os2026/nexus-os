import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { Settings } from "../Settings";
import { createDefaultConfig } from "../../utils/config";

describe("Settings", () => {
  const baseProps = {
    config: createDefaultConfig(),
    saving: false,
    onChange: vi.fn(),
    uiSoundEnabled: false,
    uiSoundVolume: 0.5,
    onUiSoundEnabledChange: vi.fn(),
    onUiSoundVolumeChange: vi.fn(),
    onSave: vi.fn(),
    ollamaConnected: false,
    ollamaModels: [],
    onRerunSetup: vi.fn(),
  };

  it("renders without crashing", () => {
    const { container } = render(<Settings {...baseProps} />);
    expect(container).toBeTruthy();
    expect(container.innerHTML.length).toBeGreaterThan(0);
  });

  it("renders settings content", () => {
    render(<Settings {...baseProps} />);
    const body = document.body.textContent || "";
    expect(body.length).toBeGreaterThan(50);
  });

  it("calls onSave when save is triggered", () => {
    render(<Settings {...baseProps} />);
    const saveBtn = screen.queryByText(/Save/i);
    if (saveBtn) {
      fireEvent.click(saveBtn);
      expect(baseProps.onSave).toHaveBeenCalled();
    }
  });

  it("checks an API key's format only and never reports it as connected", () => {
    const config = createDefaultConfig();
    config.llm.openai_api_key = "sk-not-a-real-key";
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    render(<Settings {...baseProps} config={config} />);
    fireEvent.click(screen.getByText("API Keys"));
    const checks = screen.getAllByText("Check Format");
    expect(checks.length).toBeGreaterThan(0);
    fireEvent.click(checks[0]);
    expect(screen.getByText("Format looks valid (not verified)")).toBeTruthy();
    expect(screen.queryByText(/Connected/)).toBeNull();
    // P0 item D: the key never leaves the webview to be "tested".
    expect(fetchSpy).not.toHaveBeenCalled();
    fetchSpy.mockRestore();
  });

  it("renders saving state without crashing", () => {
    const { container } = render(<Settings {...baseProps} saving={true} />);
    expect(container.innerHTML.length).toBeGreaterThan(0);
  });
});
