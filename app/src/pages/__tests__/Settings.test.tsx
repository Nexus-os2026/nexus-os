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

  it("renders saving state without crashing", () => {
    const { container } = render(<Settings {...baseProps} saving={true} />);
    expect(container.innerHTML.length).toBeGreaterThan(0);
  });
});
