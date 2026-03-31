import { render, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { mockCommands, expectInvoked } from "../../test/setup";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const { default: FlashInference } = await import("../FlashInference");

const MOCKS = {
  flash_list_local_models: [{ filename: "model-2b.gguf", size_bytes: 2_000_000_000, quantization: "Q4_K_M" }],
  flash_detect_hardware: { ram_gb: 16, cores: 8 },
  flash_clear_sessions: null,
  flash_system_metrics: {},
  flash_speculative_status: {},
};

describe("FlashInference", () => {
  it("renders heading", () => {
    mockCommands(MOCKS);
    const { container } = render(<FlashInference />);
    expect(container.innerHTML).toContain("Flash Inference");
  });

  it("loads models and hardware on mount", async () => {
    mockCommands(MOCKS);
    render(<FlashInference />);
    await waitFor(() => expectInvoked("flash_list_local_models"));
    expectInvoked("flash_detect_hardware");
  });

  it("renders without crashing when no mocks", () => {
    expect(() => render(<FlashInference />)).not.toThrow();
  });
});
